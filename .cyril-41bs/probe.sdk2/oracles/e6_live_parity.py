#!/usr/bin/env python3
import json
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parents[1]


def run_probe(binary, *args):
    completed = subprocess.run(
        ["cargo", "run", "--quiet", "--bin", binary, "--", *args],
        cwd=root,
        text=True,
        capture_output=True,
        timeout=300,
        check=False,
    )
    if completed.returncode != 0:
        raise SystemExit(completed.stdout + completed.stderr)
    return json.loads(completed.stdout)


direct = run_probe("e5", "all")
conductor = run_probe("e6_live", "all")
direct_by_engine = {
    {"kiro-v2": "v2", "kiro-kas": "kas"}[entry["engine"]]: entry
    for entry in direct["live"]
}
required_methods = {
    "v2": {"session/update", "_kiro.dev/commands/available", "_kiro.dev/metadata"},
    "kas": {"session/update", "_kiro/auth/getAccessToken", "_kiro/mcp/status"},
}
comparisons = []
for topology in conductor["topologies"]:
    engine = topology["engine"]
    baseline = direct_by_engine[engine]
    observed_methods = {event.split(":", 1)[1] for event in topology["events"]}
    required = required_methods[engine]
    transformations = topology["proxy_transformations"]
    transforming = topology["topology"] == "transforming-proxy"
    outbound_transform = any(
        item == "transformed:client:session/prompt" for item in transformations
    )
    inbound_transform = any(
        item.startswith("transformed:agent:") for item in transformations
    )
    contract_matches = (
        baseline["protocol_version"] == "ProtocolVersion(1)"
        and baseline["stop_reason"] == "EndTurn"
        and baseline["session_id_present"]
        and baseline["prompt_response_last"]
        and baseline["agent_message_chunks"] > 0
        and topology["protocol_version"] == 1
        and topology["session_id_present"]
        and topology["stop_reason"] == "end_turn"
        and required.issubset(observed_methods)
        and ((outbound_transform and inbound_transform) if transforming else not transformations)
    )
    comparisons.append(
        {
            "engine": engine,
            "topology": topology["topology"],
            "required_methods": sorted(required),
            "observed_methods": sorted(observed_methods),
            "contract_matches_direct": contract_matches,
        }
    )

result = {
    "claim_ids": ["C2"],
    "direct_vs_conductor_cells": comparisons,
    "direct_vs_conductor_equivalent": all(
        item["contract_matches_direct"] for item in comparisons
    ),
}
print(json.dumps(result, indent=2, sort_keys=True))
if not result["direct_vs_conductor_equivalent"]:
    raise SystemExit(1)
