#!/usr/bin/env python3
"""Run the deterministic conductor-cutover contract and emit claim coverage."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CHECKPOINT = ROOT / ".cyril-gl5s/oracles/runtime-contract.json"
RUNTIME_CLAIMS = [
    "C1",
    "C2",
    "C3",
    "C4",
    "C5",
    "C6",
    "C7",
    "C8",
    "C9",
    "C10",
    "C11",
    "C13",
    "C14",
]

COMMANDS = [
    (
        "sdk2_bridge_contract",
        ["cargo", "test", "-p", "cyril-core", "protocol::bridge::tests::current_runtime_contract"],
        ["C1", "C5", "C6", "C7", "C8", "C11", "C13"],
        [
            "c5_every_bridge_command_has_an_explicit_current_runtime_outcome",
            "c5_new_session_rpc_failure_is_fatal",
            "c5_command_failures_preserve_legacy_operation_labels",
            "c1_c5_c6_sdk_handler_backpressure_preserves_every_frame",
            "c6_unknown_update_handler_precedes_typed_handler_without_poisoning_connection",
            "c7_malformed_standard_request_is_rejected_before_extension_fallback",
            "c7_unknown_standard_request_returns_method_not_found",
            "c8_host_request_drain_is_live_during_initialize",
            "c13_pending_permission_response_does_not_block_shutdown",
            "c11_sdk_runtime_negotiates_stable_wire_v1",
            "c13_extension_params_preserve_array_and_null_shapes",
        ],
    ),
    (
        "stage_chain_contract",
        [
            "cargo",
            "test",
            "-p",
            "cyril-core",
            "protocol::sdk_runtime::tests::topology",
        ],
        ["C2", "C3"],
        [
            "zero_stage_runtime_still_has_a_conductor_stage_chain",
            "ordered_stage_chain_preserves_runtime_frame_order",
        ],
    ),
    (
        "raw_ingress_contract",
        [
            "cargo",
            "test",
            "-p",
            "cyril-core",
            "c4_",
        ],
        ["C4"],
        [
            "c4_recording_reader_preserves_segmented_batch_malformed_and_numeric_bytes",
            "c4_process_adapter_captures_invalid_frames_before_sdk_rejection",
        ],
    ),
    (
        "process_adapter_contract",
        [
            "cargo",
            "test",
            "-p",
            "cyril-core",
            "process_adapter_preserves_raw_ingress_and_clean_eof",
        ],
        ["C4", "C6", "C13"],
        ["process_adapter_preserves_raw_ingress_and_clean_eof"],
    ),
    (
        "serial_mediator_contract",
        [
            "cargo",
            "test",
            "-p",
            "cyril-core",
            "protocol::domain_mediator::tests::serial",
        ],
        ["C7", "C8", "C9"],
        [
            "initialization_failure_drains_queued_callback_error",
            "prompt_terminal_is_processed_after_queued_source_frames",
        ],
    ),
    (
        "process_lifecycle_contract",
        ["cargo", "test", "-p", "cyril-core", "protocol::transport::tests::current_runtime_contract"],
        ["C6"],
        ["protocol::transport::tests::current_runtime_contract::"],
    ),
    (
        "source_contract",
        ["cargo", "test", "-p", "cyril-core", "protocol::source_observer::tests::current_runtime_contract"],
        ["C9"],
        ["protocol::source_observer::tests::current_runtime_contract::"],
    ),
    (
        "kas_runtime_contract",
        [
            "cargo",
            "test",
            "-p",
            "cyril-core",
            "--all-features",
            "kas_runtime_preserves_callbacks_commands_and_wire_terminal_order",
        ],
        ["C7", "C8", "C9", "C13"],
        ["kas_runtime_preserves_callbacks_commands_and_wire_terminal_order"],
    ),
    (
        "app_memory_shutdown_contract",
        ["cargo", "test", "-p", "cyril", "current_runtime_contract"],
        ["C8", "C9"],
        ["current_runtime_contract::"],
    ),
    (
        "callback_ownership_contract",
        [
            "cargo",
            "test",
            "-p",
            "cyril-core",
            "--all-features",
            "adapter_matrix_advertises_if_and_only_if_the_mediator_answers",
        ],
        ["C7"],
        ["adapter_matrix_advertises_if_and_only_if_the_mediator_answers"],
    ),
    (
        "bounded_handoff_contract",
        [
            "cargo",
            "test",
            "-p",
            "cyril-core",
            "domain_work_capacity_is_exact_and_lossless_until_full",
        ],
        ["C1"],
        ["domain_work_capacity_is_exact_and_lossless_until_full"],
    ),
    (
        "host_handoff_contract",
        [
            "cargo",
            "test",
            "-p",
            "cyril-core",
            "host_work_capacity_is_exact_and_fifo",
        ],
        ["C1", "C7"],
        ["host_work_capacity_is_exact_and_fifo"],
    ),
    (
        "session_model_contract",
        [
            "cargo",
            "test",
            "-p",
            "cyril-core",
            "session_created_from_response_preserves_kiro_model_catalog",
        ],
        ["C13"],
        ["session_created_from_response_preserves_kiro_model_catalog"],
    ),
    (
        "disconnect_diagnostics_contract",
        [
            "cargo",
            "test",
            "-p",
            "cyril-core",
            "initialization_failure_preserves_last_five_agent_stderr_lines",
        ],
        ["C6"],
        ["initialization_failure_preserves_last_five_agent_stderr_lines"],
    ),
]


def run(command: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )

def checkpoint_contracts(results: dict[str, object]) -> dict[str, object]:
    fields = (
        "passed",
        "claims",
        "command",
        "executed_tests",
        "required_tests",
        "missing_tests",
    )
    return {
        name: {field: result[field] for field in fields}
        for name, result in results.items()
        if name != "module_shape" and isinstance(result, dict)
    }

def runtime_source_hashes() -> dict[str, str]:
    patterns = (
        "crates/cyril-core/src/**/*.rs",
        "crates/cyril/src/app.rs",
        "crates/cyril/tests/*.rs",
    )
    paths = sorted({path for pattern in patterns for path in ROOT.glob(pattern)})
    return {
        path.relative_to(ROOT).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in paths
    }






def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase", choices=("runtime", "final"), required=True)
    parser.add_argument("--write-checkpoint", action="store_true")
    args = parser.parse_args()
    failures: list[str] = []
    covered: set[str] = set()
    results: dict[str, object] = {}
    for name, command, claims, required_tests in COMMANDS:
        completed = run(command)
        output = completed.stdout
        passed_counts = [
            int(count)
            for count in re.findall(r"test result: ok\. ([1-9][0-9]*) passed;", output)
        ]
        missing_tests = [
            test
            for test in required_tests
            if re.search(rf"^test [^\n]*{re.escape(test)}[^\n]* \.\.\. ok$", output, re.MULTILINE)
            is None
        ]
        passed = completed.returncode == 0 and bool(passed_counts) and not missing_tests
        results[name] = {
            "passed": passed,
            "claims": claims,
            "command": command,
            "executed_tests": sum(passed_counts),
            "required_tests": required_tests,
            "missing_tests": missing_tests,
            "last_lines": output.splitlines()[-8:],
        }
        if passed:
            covered.update(claims)
        else:
            failures.append(f"{name} failed with exit {completed.returncode}")

    shape = run(
        [
            sys.executable,
            ".cyril-gl5s/oracles/module_shape.py",
            "--phase",
            args.phase,
        ]
    )
    try:
        shape_result = json.loads(shape.stdout)
    except json.JSONDecodeError:
        shape_result = {"passed": False, "raw": shape.stdout}
    results["module_shape"] = shape_result
    if shape.returncode == 0 and shape_result.get("passed") is True:
        covered.update(shape_result.get("passed_claims", []))
    else:
        failures.append("module_shape failed")

    expected = list(RUNTIME_CLAIMS)
    if args.phase == "final":
        expected.insert(11, "C12")
    missing = sorted(set(expected) - covered)
    if missing:
        failures.append(f"missing claim coverage: {','.join(missing)}")
    if args.phase == "final":
        try:
            checkpoint = json.loads(CHECKPOINT.read_text())
            expected_contracts = checkpoint["contracts"]
            expected_sources = checkpoint["runtime_source_sha256"]
            actual_contracts = checkpoint_contracts(results)
            actual_sources = runtime_source_hashes()
            contracts_match = expected_contracts == actual_contracts
            sources_match = expected_sources == actual_sources
            checkpoint_passed = contracts_match and sources_match
            results["runtime_checkpoint"] = {
                "passed": checkpoint_passed,
                "path": str(CHECKPOINT.relative_to(ROOT)),
                "contracts_match": contracts_match,
                "runtime_sources_match": sources_match,
                "expected_contracts": len(expected_contracts),
                "actual_contracts": len(actual_contracts),
                "expected_runtime_sources": len(expected_sources),
                "actual_runtime_sources": len(actual_sources),
            }
            if not checkpoint_passed:
                failures.append("final contract differs from runtime checkpoint")
        except (OSError, KeyError, json.JSONDecodeError, TypeError) as error:
            results["runtime_checkpoint"] = {
                "passed": False,
                "path": str(CHECKPOINT.relative_to(ROOT)),
                "error": str(error),
            }
            failures.append("runtime checkpoint unavailable or invalid")
    passed_claims = expected if not failures else sorted(covered)
    report = {
        "phase": args.phase,
        "passed": not failures,
        "passed_claims": passed_claims,
        "failures": failures,
        "results": results,
    }
    if args.phase == "runtime" and args.write_checkpoint and not failures:
        CHECKPOINT.write_text(
            json.dumps(
                {
                    "phase": "runtime",
                    "contracts": checkpoint_contracts(results),
                    "runtime_source_sha256": runtime_source_hashes(),
                },
                indent=2,
                sort_keys=True,
            )
            + "\n"
        )
        report["runtime_checkpoint_written"] = str(CHECKPOINT.relative_to(ROOT))
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if not failures else 1


if __name__ == "__main__":
    sys.exit(main())
