#!/usr/bin/env python3
import json
from pathlib import Path

repo = Path(__file__).resolve().parents[3]
covenant = (repo / "docs/kiro-kas-acp-covenant.md").read_text()
required_methods = [
    "_kiro/auth/getAccessToken",
    "_kiro/terminal/shell_type",
    "fs/read_text_file",
    "fs/write_text_file",
    "_kiro/fs/read_file",
    "_kiro/fs/write_file",
    "_kiro/fs/stat",
    "_kiro/fs/read_directory",
    "_kiro/fs/delete",
    "terminal/create",
    "terminal/output",
    "terminal/wait_for_exit",
    "terminal/release",
    "terminal/kill",
    "session/request_permission",
    "_kiro/hooks/list",
    "_kiro/hooks/executeHook",
    "_kiro/hooks/sessionStart",
    "_kiro/hooks/cancel",
    "_kiro/hooks/didChange",
]
# The covenant renders some standard ACP names as Rust/API spellings. Kiro extension
# names remain literal and provide the independent exhaustive contract check.
kiro_methods = [method for method in required_methods if method.startswith("_kiro/")]
contract_presence = {method: method in covenant for method in kiro_methods}

captures = [
    repo / "experiments/conductor-spike/v2-live-session-trace-2.11.0.jsonl",
    repo / "experiments/conductor-spike/kas-workflow-channels-live-2.20.1.jsonl",
]
capture_methods = {}
for capture in captures:
    methods = set()
    for line in capture.read_text().splitlines():
        frame = json.loads(line)
        message = frame.get("msg", frame)
        method = message.get("method")
        if method:
            methods.add(method)
    capture_methods[capture.name] = sorted(methods)

facts = {
    "claim_ids": ["C7"],
    "required_matrix_size": len(required_methods),
    "kiro_covenant_presence": contract_presence,
    "reference_capture_methods": capture_methods,
    "offline_contract_oracle_passed": all(contract_presence.values())
    and all(capture_methods.values()),
    "live_sdk2_parity_proven_by_offline_oracle": False,
    "live_sdk2_parity_scope": "N/A — authenticated parity is owned by the e5 live probe",
}
print(json.dumps(facts, indent=2, sort_keys=True))
if not facts["offline_contract_oracle_passed"]:
    raise SystemExit(1)
