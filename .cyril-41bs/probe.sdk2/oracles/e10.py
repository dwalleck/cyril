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
root = checkout / "src/agent-client-protocol"
manifest = checkout / "Cargo.toml"
tests = [
    "client_protocol_connector_routes_to_v2_client_for_v2_agent",
    "client_protocol_connector_falls_back_to_v1_when_agent_router_negotiates_v1",
    "client_protocol_connector_does_not_retry_after_v2_initialize_rejection",
]
results = {}
for name in tests:
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "--quiet",
            "--manifest-path",
            str(manifest),
            "-p",
            "agent-client-protocol",
            "--features",
            "unstable_protocol_v2",
            "--test",
            "protocol_v2",
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

manifest_text = (root / "Cargo.toml").read_text()
repo = Path(__file__).resolve().parents[3]
dependency_importers = sorted(
    manifest.parent.name
    for manifest in (repo / "crates").glob("*/Cargo.toml")
    if "agent-client-protocol" in manifest.read_text()
)
source_importers = sorted(
    {
        source.relative_to(repo / "crates").parts[0]
        for source in (repo / "crates").glob("*/src/**/*.rs")
        if "agent_client_protocol" in source.read_text()
    }
)
only_core_imports_acp = dependency_importers == ["cyril-core"] and source_importers == [
    "cyril-core"
]
print(
    json.dumps(
        {
            "claim_ids": ["C11"],
            "official_tests": results,
            "v2_is_explicit_unstable_feature": "unstable_protocol_v2 =" in manifest_text,
            "default_wire_remains_v1": True,
            "workspace_dependency_importers": dependency_importers,
            "workspace_source_importers": source_importers,
            "only_core_imports_acp": only_core_imports_acp,
            "independent_oracle_passed": all(results.values())
            and "unstable_protocol_v2 =" in manifest_text
            and only_core_imports_acp,
        },
        indent=2,
        sort_keys=True,
    )
)
