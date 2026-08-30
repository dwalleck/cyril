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
incoming_actor_awaits_handler_future = (
    ".handle_dispatch_from(dispatch, connection.clone())" in incoming_source
    and ".await" in incoming_source
)
result = {
    "claim_ids": ["C5"],
    "sdk_source": str(root),
    "official_tests": results,
    "dynamic_handler_guard_removes_on_drop": dynamic_guard_removes_on_drop,
    "incoming_actor_awaits_handler_future": incoming_actor_awaits_handler_future,
    "independent_oracle_passed": (
        all(test_result["passed"] for test_result in results.values())
        and dynamic_guard_removes_on_drop
        and incoming_actor_awaits_handler_future
    ),
}
print(json.dumps(result, indent=2, sort_keys=True))
if not result["independent_oracle_passed"]:
    raise SystemExit(1)
