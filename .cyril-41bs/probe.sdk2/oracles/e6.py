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
root = checkout / "src/agent-client-protocol-conductor"
manifest = checkout / "Cargo.toml"
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
    )
    results[name] = completed.returncode == 0
    if completed.returncode != 0:
        raise SystemExit(completed.stdout + completed.stderr)

source = (root / "src/conductor.rs").read_text()
print(
    json.dumps(
        {
            "claim_ids": ["C2", "C3", "C5"],
            "official_tests": results,
            "lazy_agent_instantiation_present": "new_agent" in source and "FnOnce" in source,
            "proxy_chain_constructor_present": "pub fn proxy" in source,
            "independent_oracle_passed": all(results.values()),
        },
        indent=2,
        sort_keys=True,
    )
)
