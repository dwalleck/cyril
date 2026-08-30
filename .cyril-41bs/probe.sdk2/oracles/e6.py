#!/usr/bin/env python3
import json
import subprocess
from pathlib import Path

checkout = Path("/tmp/librarian-acp")
if not (checkout / ".git").exists():
    raise SystemExit(
        "pinned SDK checkout unavailable; clone agentclientprotocol/rust-sdk at "
        "ce023279824149008659dd8f4b8b70266a7e8210 into /tmp/librarian-acp"
    )
pinned_revision = "ce023279824149008659dd8f4b8b70266a7e8210"
revision = subprocess.run(
    ["git", "rev-parse", "HEAD"],
    cwd=checkout,
    text=True,
    capture_output=True,
    timeout=30,
    check=False,
)
if revision.returncode != 0 or revision.stdout.strip() != pinned_revision:
    raise SystemExit(
        f"SDK checkout revision changed: expected {pinned_revision}, "
        f"observed {revision.stdout.strip() or revision.stderr.strip()}"
    )
root = checkout / "src/agent-client-protocol-conductor"
manifest = checkout / "Cargo.toml"
probe_root = Path(__file__).resolve().parents[1]
probe_manifest = probe_root / "Cargo.toml"
probe_run = [
    "cargo",
    "run",
    "--quiet",
    "--manifest-path",
    str(probe_manifest),
    "--bin",
    "e6",
]
probe = subprocess.run(
    probe_run,
    cwd=probe_root,
    text=True,
    capture_output=True,
    check=False,
    timeout=180,
)
if probe.returncode != 0:
    raise SystemExit("default e6 probe failed:\n" + probe.stdout + probe.stderr)
try:
    default = json.loads(probe.stdout)
except json.JSONDecodeError as error:
    raise SystemExit(f"default e6 probe emitted invalid JSON: {error}") from error


def require(condition, message):
    if not condition:
        raise SystemExit(message)


def terminal_error_leaf(value, name):
    while isinstance(value, dict):
        require(
            set(value) == {"spawned_at", "data"},
            f"terminal data shape changed for {name}",
        )
        require(
            isinstance(value["spawned_at"], str) and value["spawned_at"],
            f"terminal source metadata changed for {name}",
        )
        value = value["data"]
    return value

require(default["claim_ids"] == ["C2", "C3", "C5", "C7"], "claim IDs changed")
require(default["sdk_version"] == "2.0.0", "SDK version changed")
require(default["wire_version"] == "V1", "wire version changed")
require(default["distinct_stage_order_preserved"] is True, "distinct stage order was lost")
require(default["repeated_stage_request_order_preserved"] is True, "repeated request order was lost")
require(default["repeated_stage_notification_order_preserved"] is True, "repeated notification order was lost")

expected_cases = {
    "zero-proxy": {
        "response": "agent:response:client",
        "notifications": ["agent:notify:client"],
    },
    "no-op-proxy": {
        "response": "agent:response:client",
        "notifications": ["agent:notify:client"],
    },
    "transforming-proxy": {
        "response": "agent:response:proxy:request:client",
        "notifications": [
            "proxy:notification:agent:notify:proxy:request:client",
        ],
    },
    "distinct-two-stage": {
        "response": "agent:response:proxy:request:client",
        "notifications": [
            "proxy:notification:agent:notify:proxy:request:client",
        ],
    },
    "repeated-transform-stage": {
        "response": "agent:response:second:request:first:request:client",
        "notifications": [
            "first:notification:second:notification:agent:notify:"
            "second:request:first:request:client",
        ],
    },
}
case_names = [case.get("name") for case in default["cases"]]
require(
    len(case_names) == len(expected_cases)
    and len(set(case_names)) == len(case_names)
    and set(case_names) == set(expected_cases),
    f"case names changed: {case_names}",
)
for case in default["cases"]:
    name = case.get("name")
    require(name in expected_cases, f"unexpected case: {name}")
    expected = expected_cases[name]
    require(case["response"] == expected["response"], f"response changed for {name}")
    require(
        case["notifications"] == expected["notifications"],
        f"notifications changed for {name}",
    )
    require(
        case["terminal_cancellation_event"] == "agent:cancellation-observed",
        f"cancellation event changed for {name}",
    )
    require(case["cancellation_outcome"] == "event", f"cancellation outcome changed for {name}")
    require(case["terminal_cancellation_observed"] is True, f"cancellation not observed for {name}")
    require(case["response_identity_preserved"] is True, f"response identity changed for {name}")
    require(case["cancellation_frames"] == 1, f"cancellation frame count changed for {name}")
    require(case["cancellation_forwarded_once"] is True, f"cancellation forwarding changed for {name}")
    require(case["wire_entries_parsed"] == len(case["wire"]), f"wire parse count changed for {name}")
    require(len(case["wire"]) <= 1_000, f"wire entry bound changed for {name}")
    require(
        "e6:wire-entry-limit-exceeded" not in case["wire"],
        f"wire overflow marker observed for {name}",
    )
    require(case["wire_parse_budget_ms"] == 100, f"wire parse budget changed for {name}")
    require(0 <= case["wire_parse_elapsed_ms"] <= 100, f"wire parse exceeded budget for {name}")
    require(case["wire_parse_within_budget"] is True, f"wire parse bound changed for {name}")

expected_failure_names = {
    "zero-proxy-failure",
    "no-op-proxy-failure",
    "transforming-proxy-failure",
}
failure_names = [failure.get("name") for failure in default["failure_cases"]]
require(
    len(failure_names) == len(expected_failure_names)
    and len(set(failure_names)) == len(failure_names)
    and set(failure_names) == expected_failure_names,
    f"failure case names changed: {failure_names}",
)
for failure in default["failure_cases"]:
    name = failure.get("name")
    require(name in expected_failure_names, f"unexpected failure case: {name}")
    require(failure["connection_failed"] is True, f"failure did not terminate for {name}")
    require(failure["terminal_error_code"] == -32603, f"terminal code changed for {name}")
    require(
        terminal_error_leaf(failure["terminal_error_data"], name) == "agent-crash",
        f"terminal data changed for {name}",
    )
    require(failure["terminal_error_leaf"] == "agent-crash", f"terminal leaf changed for {name}")
    require(failure["terminal_error_code_is_internal"] is True, f"terminal code type changed for {name}")
    require(failure["terminal_error_data_is_agent_crash"] is True, f"terminal data check changed for {name}")
    require(failure["terminal_error_preserved"] is True, f"terminal error was not preserved for {name}")
    require("error" not in failure, f"unstructured terminal error leaked for {name}")

expected_diagnostics = {
    "wrong-cancellation-event": "Error: e6: wrong cancellation event",
    "cancellation-timeout": "Error: e6: cancellation wait timed out",
    "cancellation-channel-closed": "Error: e6: cancellation channel closed",
    "malformed-wire-entry": "Error: e6: malformed wire entry index=0 direction=client->agent",
    "wrong-terminal-data": "Error: e6: terminal error data mismatch",
    "wrong-response-id": "Error: e6: response identity mismatch",
}
for mode, diagnostic in expected_diagnostics.items():
    negative = subprocess.run(
        [*probe_run, "--", mode],
        cwd=probe_root,
        text=True,
        capture_output=True,
        check=False,
        timeout=180,
    )
    require(negative.returncode != 0, f"negative mode unexpectedly passed: {mode}")
    require(negative.stdout == "", f"negative mode emitted stdout: {mode}")
    require(negative.stderr.strip() == diagnostic, f"diagnostic changed for {mode}")

# Keep these pinned upstream tests as an independent SDK/conductor oracle.
tests = [
    ("initialization_sequence", "test_single_component_gets_initialize_request"),
    ("initialization_sequence", "test_two_components_proxy_gets_initialize_proxy"),
    ("initialization_sequence", "test_three_components_all_proxies_get_initialize_proxy"),
    ("request_cancellation", "client_cancellation_propagates_hop_by_hop_to_agent"),
    ("request_cancellation", "agent_cancellation_propagates_hop_by_hop_to_client"),
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
            "-p",
            "agent-client-protocol-conductor",
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
    results[name] = completed.returncode == 0
    if completed.returncode != 0:
        raise SystemExit(completed.stdout + completed.stderr)

source = (root / "src/conductor.rs").read_text()
lazy_agent_instantiation_present = "new_agent" in source and "FnOnce" in source
proxy_chain_constructor_present = "pub fn proxy" in source
require(lazy_agent_instantiation_present, "lazy agent source contract changed")
require(proxy_chain_constructor_present, "proxy chain constructor source contract changed")
independent_oracle_passed = (
    all(results.values())
    and lazy_agent_instantiation_present
    and proxy_chain_constructor_present
)
print(
    json.dumps(
        {
            "claim_ids": ["C2", "C3", "C5", "C7"],
            "official_tests": results,
            "pinned_sdk_revision": revision.stdout.strip(),
            "lazy_agent_instantiation_present": lazy_agent_instantiation_present,
            "proxy_chain_constructor_present": proxy_chain_constructor_present,
            "independent_oracle_passed": independent_oracle_passed,
            "e6_default_probe_passed": True,
            "e6_negative_modes_passed": True,
        },
        indent=2,
        sort_keys=True,
    )
)
if not independent_oracle_passed:
    raise SystemExit(1)
