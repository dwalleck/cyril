#!/usr/bin/env python3
"""Prove each cutover phase rejects its forbidden architecture regressions."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SHAPE = ROOT / ".cyril-gl5s/oracles/module_shape.py"


def run_shape(phase: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(SHAPE), "--phase", phase],
        cwd=ROOT,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )


def mutate(
    phase: str, path: Path, transform, required: list[str]
) -> dict[str, object]:
    original = path.read_bytes()
    try:
        path.write_text(transform(original.decode()))
        completed = run_shape(phase)
        rejected = completed.returncode != 0 and all(
            needle in completed.stdout for needle in required
        )
        return {
            "passed": rejected,
            "exit": completed.returncode,
            "required_diagnostics": required,
            "diagnostic": completed.stdout,
        }
    finally:
        path.write_bytes(original)

def mutate_behavior(
    path: Path, transform, command: list[str], required: list[str]
) -> dict[str, object]:
    original = path.read_bytes()
    baseline = subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    test_name = required[0]
    baseline_green = (
        baseline.returncode == 0
        and test_name in baseline.stdout
        and "test result: ok" in baseline.stdout
    )
    mutated = None
    restored_bytes = False
    try:
        path.write_text(transform(original.decode()))
        mutated = subprocess.run(
            command,
            cwd=ROOT,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
    finally:
        path.write_bytes(original)
        restored_bytes = path.read_bytes() == original

    restored = subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    restored_green = (
        restored.returncode == 0
        and test_name in restored.stdout
        and "test result: ok" in restored.stdout
    )
    rejected = mutated is not None and mutated.returncode != 0 and all(
        needle in mutated.stdout for needle in required
    )
    return {
        "passed": baseline_green and rejected and restored_bytes and restored_green,
        "baseline_green": baseline_green,
        "baseline_diagnostic": baseline.stdout,
        "mutation_exit": None if mutated is None else mutated.returncode,
        "required_diagnostics": required,
        "mutation_diagnostic": "" if mutated is None else mutated.stdout,
        "restored_bytes": restored_bytes,
        "restored_green": restored_green,
        "restored_diagnostic": restored.stdout,
    }


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if text.count(old) != 1:
        raise RuntimeError(f"{label}: expected one mutation target, found {text.count(old)}")
    return text.replace(old, new, 1)




def legacy_dependency(text: str) -> str:
    marker = "[workspace.dependencies]\n"
    insertion = (
        'old_acp = { package = "agent-client-protocol", version = "=0.10.2" }\n'
    )
    if marker not in text:
        raise RuntimeError("workspace dependency table missing")
    return text.replace(marker, marker + insertion, 1)

def direct_schema_dependency(text: str) -> str:
    marker = "[workspace.dependencies]\n"
    insertion = (
        'schema_alias = { package = "agent-client-protocol-schema", version = "=1.5.0" }\n'
    )
    if marker not in text:
        raise RuntimeError("workspace dependency table missing")
    return text.replace(marker, marker + insertion, 1)


def legacy_rust_import(text: str) -> str:
    return text + "\n// mutation sentinel: agent_client_protocol_legacy\n"


def custom_endpoint(text: str) -> str:
    return text + "\n// mutation sentinel: AgentEndpoint\n"


def direct_process_adapter(text: str) -> str:
    return text + "\n// mutation sentinel: AcpAgent\n"


def corrupt_recording_capture(text: str) -> str:
    return replace_once(
        text,
        "&buffer.filled()[before..]",
        "&buffer.filled()[..before]",
        "recording boundary",
    )


def reverse_unknown_fence(text: str) -> str:
    return replace_once(
        text,
        "!KNOWN_SESSION_UPDATE_TAGS.contains(&tag)",
        "KNOWN_SESSION_UPDATE_TAGS.contains(&tag)",
        "unknown-first handler",
    )


def ignore_requested_cwd(text: str) -> str:
    return replace_once(
        text,
        ".current_dir(cwd)",
        ".current_dir(std::env::temp_dir())",
        "process cwd",
    )


def remove_host_io_adapter(text: str) -> str:
    return replace_once(
        text,
        "host_io: Some(HostIoAdapter)",
        "host_io: None",
        "KAS host-I/O adapter",
    )


def corrupt_source_disposition(text: str) -> str:
    return replace_once(
        text,
        "Some(source_disposition)",
        "Some(crate::types::SourceTurnDisposition::Abandoned)",
        "source terminal disposition",
    )


def remove_extension_prefix(text: str) -> str:
    return replace_once(
        text,
        'format!("_{method}")',
        "method.to_owned()",
        "extension wire prefix",
    )




def forbidden_connection(text: str) -> str:
    return text + "\n// mutation sentinel: ClientSideConnection\n"

def enable_unstable_wire(text: str) -> str:
    marker = '"unstable_end_turn_token_usage"'
    if marker not in text:
        raise RuntimeError("stable SDK feature marker missing")
    return text.replace(marker, f'{marker}, "unstable_protocol_v2"', 1)


def remove_conductor(text: str) -> str:
    marker = "ConductorImpl::new_agent"
    if marker not in text:
        raise RuntimeError("conductor constructor missing")
    return text.replace(marker, "DirectConductor::new_agent", 1)


def add_observer(text: str) -> str:
    return text + "\n// mutation sentinel: observer registration\n"


def add_client_domain_state(text: str) -> str:
    return text + "\n// mutation sentinel: DomainMediator\n"

def add_bridge_command_body(text: str) -> str:
    return "// mutation sentinel: BridgeCommand::Shutdown\n" + text


def unbound_domain_queue(text: str) -> str:
    marker = "mpsc::channel(WORK_CAPACITY)"
    if marker not in text:
        raise RuntimeError("bounded domain queue missing")
    return text.replace(marker, "mpsc::unbounded_channel()", 1)


def delay_host_drain(text: str) -> str:
    marker = "let host_task = host::run"
    if marker not in text:
        raise RuntimeError("host drain startup missing")
    return text.replace(marker, "let delayed_host_task = host::run", 1)


def touch_protected_app(text: str) -> str:
    return "// mutation sentinel: production App change\n" + text


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase", choices=("runtime", "final"), required=True)
    args = parser.parse_args()

    baseline = run_shape(args.phase)
    if baseline.returncode != 0:
        print(
            json.dumps(
                {
                    "phase": args.phase,
                    "passed": False,
                    "passed_claims": [],
                    "failures": ["baseline module shape is not green"],
                    "baseline": baseline.stdout,
                },
                indent=2,
                sort_keys=True,
            )
        )
        return 1

    results = {}
    if args.phase == "final":
        results["legacy_dependency"] = mutate(
            args.phase,
            ROOT / "Cargo.toml",
            legacy_dependency,
            ["legacy ACP dependency alias", "old_acp"],
        )
    results["legacy_rust_import"] = mutate(
        args.phase,
        ROOT / "crates/cyril-core/src/protocol/client.rs",
        legacy_rust_import,
        ["legacy Rust import", "agent_client_protocol_legacy"],
    )
    results.update({
        "forbidden_connection": mutate(
            args.phase,
            ROOT / "crates/cyril-core/src/protocol/client.rs",
            forbidden_connection,
            ["ClientSideConnection", "crates/cyril-core/src/protocol/client.rs"],
        ),
        "direct_schema_dependency": mutate(
            args.phase,
            ROOT / "Cargo.toml",
            direct_schema_dependency,
            ["schema dependency must remain transitive"],
        ),
        "custom_endpoint": mutate(
            args.phase,
            ROOT / "crates/cyril-core/src/protocol/client.rs",
            custom_endpoint,
            ["custom endpoint symbol", "AgentEndpoint"],
        ),
        "direct_process_adapter": mutate(
            args.phase,
            ROOT / "crates/cyril-core/src/protocol/client.rs",
            direct_process_adapter,
            ["forbidden direct process adapter", "AcpAgent"],
        ),
        "unstable_wire": mutate(
            args.phase,
            ROOT / "Cargo.toml",
            enable_unstable_wire,
            ["unstable_protocol_v2 is enabled"],
        ),
        "direct_conductor_bypass": mutate(
            args.phase,
            ROOT / "crates/cyril-core/src/protocol/sdk_runtime/mod.rs",
            remove_conductor,
            ["expected exactly one ConductorImpl::new_agent topology"],
        ),
        "observer_parameter": mutate(
            args.phase,
            ROOT / "crates/cyril-core/src/protocol/sdk_runtime/mod.rs",
            add_observer,
            ["observer/inspection marker in SDK runtime"],
        ),
        "bridge_command_body": mutate(
            args.phase,
            ROOT / "crates/cyril-core/src/protocol/bridge.rs",
            add_bridge_command_body,
            ["protected bridge owns forbidden runtime body: BridgeCommand::"],
        ),
        "client_domain_state": mutate(
            args.phase,
            ROOT / "crates/cyril-core/src/protocol/client.rs",
            add_client_domain_state,
            ["client owns forbidden domain state: DomainMediator"],
        ),
        "unbounded_domain_queue": mutate(
            args.phase,
            ROOT / "crates/cyril-core/src/protocol/domain_mediator/mod.rs",
            unbound_domain_queue,
            ["unbounded protocol queue"],
        ),
        "late_host_drain": mutate(
            args.phase,
            ROOT / "crates/cyril-core/src/protocol/domain_mediator/mod.rs",
            delay_host_drain,
            ["host drain must start before initialize"],
        ),
        "protected_app_change": mutate(
            args.phase,
            ROOT / "crates/cyril/src/app.rs",
            touch_protected_app,
            ["protected parent changed: crates/cyril/src/app.rs"],
        ),
    })
    results.update(
        {
            "recording_boundary": mutate_behavior(
                ROOT / "crates/cyril-core/src/protocol/sdk_runtime/process.rs",
                corrupt_recording_capture,
                [
                    "cargo",
                    "test",
                    "-p",
                    "cyril-core",
                    "c4_recording_reader_preserves_segmented_batch_malformed_and_numeric_bytes",
                ],
                [
                    "c4_recording_reader_preserves_segmented_batch_malformed_and_numeric_bytes",
                    "FAILED",
                ],
            ),
            "unknown_first_handler": mutate_behavior(
                ROOT / "crates/cyril-core/src/protocol/client.rs",
                reverse_unknown_fence,
                [
                    "cargo",
                    "test",
                    "-p",
                    "cyril-core",
                    "c6_unknown_update_handler_precedes_typed_handler_without_poisoning_connection",
                ],
                [
                    "c6_unknown_update_handler_precedes_typed_handler_without_poisoning_connection",
                    "FAILED",
                ],
            ),
            "process_cwd": mutate_behavior(
                ROOT / "crates/cyril-core/src/protocol/transport.rs",
                ignore_requested_cwd,
                [
                    "cargo",
                    "test",
                    "-p",
                    "cyril-core",
                    "agent_process_uses_requested_working_directory_and_arguments",
                ],
                [
                    "agent_process_uses_requested_working_directory_and_arguments",
                    "FAILED",
                ],
            ),
            "kas_host_callback": mutate_behavior(
                ROOT / "crates/cyril-core/src/protocol/engine.rs",
                remove_host_io_adapter,
                [
                    "cargo",
                    "test",
                    "-p",
                    "cyril-core",
                    "--all-features",
                    "kas_runtime_preserves_callbacks_commands_and_wire_terminal_order",
                ],
                [
                    "kas_runtime_preserves_callbacks_commands_and_wire_terminal_order",
                    "FAILED",
                ],
            ),
            "source_disposition": mutate_behavior(
                ROOT / "crates/cyril-core/src/protocol/domain_mediator/mod.rs",
                corrupt_source_disposition,
                [
                    "cargo",
                    "test",
                    "-p",
                    "cyril-core",
                    "prompt_terminal_is_processed_after_queued_source_frames",
                ],
                [
                    "prompt_terminal_is_processed_after_queued_source_frames",
                    "FAILED",
                ],
            ),
            "extension_wire_prefix": mutate_behavior(
                ROOT
                / "crates/cyril-core/src/protocol/domain_mediator/commands/extensions.rs",
                remove_extension_prefix,
                [
                    "cargo",
                    "test",
                    "-p",
                    "cyril-core",
                    "c13_extension_params_preserve_array_and_null_shapes",
                ],
                [
                    "c13_extension_params_preserve_array_and_null_shapes",
                    "FAILED",
                ],
            ),
        }
    )
    restored = run_shape(args.phase)
    failures = [name for name, result in results.items() if result["passed"] is not True]
    if restored.returncode != 0:
        failures.append("restored_baseline")
    claims = [
        "C1", "C2", "C3", "C4", "C5", "C6", "C7", "C8", "C9", "C10", "C11", "C13", "C14"
    ]
    if args.phase == "final":
        claims.insert(11, "C12")
    report = {
        "phase": args.phase,
        "passed": not failures,
        "passed_claims": claims if not failures else [],
        "failures": failures,
        "results": results,
        "restored_baseline": {
            "passed": restored.returncode == 0,
            "diagnostic": restored.stdout,
        },
    }
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if not failures else 1


if __name__ == "__main__":
    sys.exit(main())
