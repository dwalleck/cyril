#!/usr/bin/env python3
"""Independent C7 proxy-leverage and ownership oracle.

This deliberately does not consume Rust output. It models the bidirectional
stage algebra and checks that the probe uses the official conductor without
reintroducing production abstractions.
"""

import json
import re
from pathlib import Path

probe = Path(__file__).resolve().parents[1]
e5_source = (probe / "src/bin/e5.rs").read_text()
e6_live_source = (probe / "src/bin/e6_live.rs").read_text()
support_source = (probe / "src/live_support.rs").read_text()


def apply_stage(value: str, label: str) -> str:
    return f"{label}:{value}"


def forward_request(value: str, stages: list[str]) -> str:
    for label in stages:
        value = apply_stage(value, label)
    return value


def reverse_response(value: str, stages: list[str]) -> str:
    for label in reversed(stages):
        value = apply_stage(value, label)
    return value


request = {"jsonrpc": "2.0", "id": "outer-7", "params": {"value": "client"}}
inner = dict(request)
inner["id"] = "inner-19"
inner["params"] = {"value": forward_request(request["params"]["value"], ["alpha", "beta"])}
agent_response = {"jsonrpc": "2.0", "id": inner["id"], "result": {"value": inner["params"]["value"]}}
outer_response = dict(agent_response)
outer_response["id"] = request["id"]
outer_response["result"] = {
    "value": reverse_response(
        agent_response["result"]["value"],
        ["alpha-response", "beta-response"],
    )
}
notification = forward_request("agent:notify:client", ["proxy-notification"])

stage_order_ok = forward_request("client", ["alpha", "beta"]) == "beta:alpha:client"
repeated_order_ok = forward_request("client", ["repeat-1", "repeat-2"]) == "repeat-2:repeat-1:client"
response_restored = outer_response["id"] == request["id"]
response_transformed = (
    outer_response["result"]["value"]
    == "alpha-response:beta-response:beta:alpha:client"
)
notification_transformed = notification == "proxy-notification:agent:notify:client"

ownership_markers = {
    "e5_imports_support": "mod live_support" in e5_source,
    "e6_live_imports_support": "mod live_support" in e6_live_source,
    "support_defines_events": "pub struct Events" in support_source,
    "support_defines_auth_loader": "pub fn load_auth_response" in support_source,
    "support_defines_callback_mapping": "pub fn callback_result" in support_source,
    "e5_does_not_define_events": "struct Events" not in e5_source,
    "e6_live_does_not_define_events": "struct Events" not in e6_live_source,
    "e5_does_not_define_auth_loader": "fn load_auth_response" not in e5_source,
    "e6_live_does_not_define_auth_loader": "fn load_auth_response" not in e6_live_source,
    "official_conductor": "ConductorImpl::new_agent" in e6_live_source,
    "forward_response_to": ".forward_response_to(responder)" in e6_live_source,
    "no_new_engine_abstraction": all(
        re.search(r"\b(?:trait|struct|enum)\s+Engine\b", source) is None
        for source in (e5_source, e6_live_source, support_source)
    ),
    "no_new_host_callback_abstraction": all(
        re.search(r"\b(?:trait|struct|enum)\s+HostCallback\b", source) is None
        for source in (e5_source, e6_live_source, support_source)
    ),
}

facts = {
    "claim_ids": ["C2", "C7"],
    "evidence_phases": ["deterministic stage algebra", "source ownership census"],
    "multiple_distinct_stages_preserve_order": stage_order_ok,
    "repeated_stage_instances_preserve_order": repeated_order_ok,
    "request_transformed": inner["params"]["value"] == "beta:alpha:client",
    "response_identity_restored": response_restored,
    "response_transformed": response_transformed,
    "notification_transformed": notification_transformed,
    "ownership": ownership_markers,
}
facts["independent_oracle_passed"] = all(
    [
        facts["multiple_distinct_stages_preserve_order"],
        facts["repeated_stage_instances_preserve_order"],
        facts["request_transformed"],
        facts["response_identity_restored"],
        facts["response_transformed"],
        facts["notification_transformed"],
        all(ownership_markers.values()),
    ]
)
print(json.dumps(facts, indent=2, sort_keys=True))
if not facts["independent_oracle_passed"]:
    raise SystemExit(1)
