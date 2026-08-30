#!/usr/bin/env python3
import json
import subprocess
from pathlib import Path

roots = list((Path.home() / ".cargo/registry/src").glob("*/agent-client-protocol-2.0.0"))
if len(roots) != 1:
    raise SystemExit(f"expected one pinned SDK source, found {roots}")
root = roots[0]
manifest = root / "Cargo.toml"
tests = [
    ("jsonrpc_connection_builder", "test_handler_priority_ordering"),
    ("jsonrpc_connection_builder", "test_fallthrough_behavior"),
    ("jsonrpc_connection_builder", "test_handler_claims_notification"),
    ("jsonrpc_advanced", "ordered_callback_installs_dynamic_handler_before_later_batch_entry"),
    ("jsonrpc_advanced", "test_out_of_order_responses"),
]
results = {}
for target, name in tests:
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "--quiet",
            "--manifest-path",
            str(manifest),
            "--test",
            target,
            name,
            "--",
            "--exact",
        ],
        text=True,
        capture_output=True,
        check=False,
        timeout=180,
    )
    results[name] = {
        "passed": completed.returncode == 0,
        "stdout": completed.stdout.strip(),
        "stderr": completed.stderr.strip(),
    }
    if completed.returncode != 0:
        raise SystemExit(json.dumps(results, indent=2, sort_keys=True))

source = (root / "src/jsonrpc.rs").read_text()
incoming_source = (root / "src/jsonrpc/incoming_actor.rs").read_text()
dynamic_guard_removes_on_drop = (
    "impl<R: Role> Drop for DynamicHandlerGuard<R>" in source
    and "remove_dynamic_handler(uuid)" in source
    and "DynamicHandlerMessage::RemoveDynamicHandler" in source
)
incoming_compact = " ".join(incoming_source.split())
incoming_actor_awaits_handler_future = (
    ".handle_dispatch_from(dispatch, connection.clone()) .await" in incoming_compact
)

repo = Path(__file__).resolve().parents[3]
probe_manifest = Path(__file__).resolve().parents[1] / "Cargo.toml"


def run_probe(mode=None):
    command = [
        "cargo",
        "run",
        "--quiet",
        "--manifest-path",
        str(probe_manifest),
        "--bin",
        "e3",
    ]
    if mode is not None:
        command.extend(["--", mode])
    return subprocess.run(
        command,
        cwd=repo,
        text=True,
        capture_output=True,
        check=False,
        timeout=180,
    )


default_run = run_probe()
default_json = None
default_json_valid = default_run.returncode == 0
if default_json_valid:
    try:
        default_json = json.loads(default_run.stdout)
    except json.JSONDecodeError:
        default_json_valid = False


def exact_case(case, name, events):
    return (
        isinstance(case, dict)
        and case.get("name") == name
        and case.get("connection_ok") is True
        and case.get("error") is None
        and case.get("events") == events
    )


default_cases_match = (
    default_json_valid
    and default_json.get("claim_ids") == ["C5"]
    and exact_case(
        default_json.get("untyped_first", {}),
        "untyped-first",
        ["untyped:unknown-contained"],
    )
    and exact_case(
        default_json.get("typed_first_mutation", {}),
        "typed-first-mutation",
        [],
    )
    and exact_case(
        default_json.get("untyped_removed_mutation", {}),
        "untyped-removed-mutation",
        [],
    )
    and default_json.get("slow_handler_blocks_dispatch") is True
    and default_json.get("mutation_expected_to_fail") is True
)

expected_diagnostics = {
    "wrong-containment-expectation": (
        "Error: wrong-containment-expectation: "
        "untyped-first containment unexpectedly matched"
    ),
    "closed-channel-control": (
        "Error: closed-channel-control: "
        "event channel closed while waiting for slow:started"
    ),
    "unexpected-event-control": (
        "Error: unexpected-event-control: "
        "unexpected event while waiting for slow:started"
    ),
}
negative_results = {}
for mode, expected_diagnostic in expected_diagnostics.items():
    completed = run_probe(mode)
    stderr_lines = completed.stderr.strip().splitlines()
    negative_results[mode] = {
        "failed_nonzero": completed.returncode != 0,
        "diagnostic": stderr_lines[-1] if stderr_lines else "",
        "exact_diagnostic": bool(stderr_lines)
        and stderr_lines[-1] == expected_diagnostic,
        "no_panic": "panicked" not in completed.stderr,
    }

result = {
    "claim_ids": ["C5"],
    "sdk_source": str(root),
    "official_tests": results,
    "dynamic_handler_guard_removes_on_drop": dynamic_guard_removes_on_drop,
    "incoming_actor_awaits_handler_future": incoming_actor_awaits_handler_future,
    "probe_default_returncode": default_run.returncode,
    "probe_default_json": default_json,
    "probe_default_json_valid": default_json_valid,
    "probe_default_cases_match": default_cases_match,
    "probe_negative_modes": negative_results,
    "independent_oracle_passed": (
        all(test_result["passed"] for test_result in results.values())
        and dynamic_guard_removes_on_drop
        and incoming_actor_awaits_handler_future
        and default_cases_match
        and all(
            mode_result["failed_nonzero"]
            and mode_result["exact_diagnostic"]
            and mode_result["no_panic"]
            for mode_result in negative_results.values()
        )
    ),
}
print(json.dumps(result, indent=2, sort_keys=True))
if not result["independent_oracle_passed"]:
    raise SystemExit(1)
