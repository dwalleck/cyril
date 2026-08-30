#!/usr/bin/env python3
import json
from pathlib import Path

request = {
    "jsonrpc": "2.0",
    "id": "outer-7",
    "method": "_kiro/probe",
    "params": {"value": "client"},
}
proxied_request = json.loads(json.dumps(request))
proxied_request["id"] = "inner-19"
proxied_request["params"]["value"] = "proxy:request:client"
agent_response = {
    "jsonrpc": "2.0",
    "id": "inner-19",
    "result": {"value": "agent:response:proxy:request:client"},
}
outer_response = json.loads(json.dumps(agent_response))
outer_response["id"] = request["id"]
notification = {
    "jsonrpc": "2.0",
    "method": "_kiro/probe-notification",
    "params": {"value": "agent:notify:proxy:request:client"},
}
notification["params"]["value"] = (
    "proxy:notification:" + notification["params"]["value"]
)

def apply_stage(value, label):
    return f"{label}:{value}"

distinct_chain = "client"
for stage_label in ["alpha", "beta"]:
    distinct_chain = apply_stage(distinct_chain, stage_label)

repeated_chain = "client"
for instance_label in ["repeat-1", "repeat-2"]:
    repeated_chain = apply_stage(repeated_chain, instance_label)

probe_source = (Path(__file__).resolve().parents[1] / "src/bin/e6.rs").read_text()
facts = {
    "claim_ids": ["C2", "C7"],
    "multiple_distinct_stages_preserve_order": distinct_chain == "beta:alpha:client",
    "repeated_stage_instances_preserve_order": repeated_chain == "repeat-2:repeat-1:client",
    "request_transformed": proxied_request["params"]["value"] == "proxy:request:client",
    "response_identity_restored": outer_response["id"] == "outer-7",
    "notification_transformed": notification["params"]["value"].startswith(
        "proxy:notification:"
    ),
    "probe_uses_official_conductor": "ConductorImpl::new_agent" in probe_source,
    "probe_uses_forward_response_to": ".forward_response_to(responder)" in probe_source,
    "probe_duplicates_engine_conversion": "Engine" in probe_source,
    "probe_duplicates_host_callback_ownership": "HostCallback" in probe_source,
}
facts["independent_oracle_passed"] = (
    facts["request_transformed"]
    and facts["response_identity_restored"]
    and facts["notification_transformed"]
    and facts["multiple_distinct_stages_preserve_order"]
    and facts["repeated_stage_instances_preserve_order"]
    and facts["probe_uses_official_conductor"]
    and facts["probe_uses_forward_response_to"]
    and not facts["probe_duplicates_engine_conversion"]
    and not facts["probe_duplicates_host_callback_ownership"]
)
print(json.dumps(facts, indent=2, sort_keys=True))
if not facts["independent_oracle_passed"]:
    raise SystemExit(1)
