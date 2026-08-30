#!/usr/bin/env python3
"""Independent contract fence for the direct SDK 2 parity probe.

The Rust probe owns execution. This oracle owns the method/family tables and
keeps authenticated-live, deterministic, and capture-backed evidence separate.
"""

import json
from pathlib import Path

repo = Path(__file__).resolve().parents[3]
covenant_path = repo / "docs/kiro-kas-acp-covenant.md"
covenant = covenant_path.read_text() if covenant_path.is_file() else ""

REQUEST_METHODS = [
    "_kiro/auth/getAccessToken",
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
    "_kiro/terminal/shell_type",
    "session/request_permission",
    "_kiro/hooks/list",
    "_kiro/hooks/executeHook",
    "_kiro/hooks/sessionStart",
]
NOTIFICATION_METHODS = ["_kiro/hooks/cancel", "_kiro/hooks/didChange"]
FAMILIES = {
    "auth": ["_kiro/auth/getAccessToken"],
    "filesystem_standard": ["fs/read_text_file", "fs/write_text_file"],
    "filesystem_kiro": [
        "_kiro/fs/read_file",
        "_kiro/fs/write_file",
        "_kiro/fs/stat",
        "_kiro/fs/read_directory",
        "_kiro/fs/delete",
    ],
    "terminal": [
        "terminal/create",
        "terminal/output",
        "terminal/wait_for_exit",
        "terminal/release",
        "terminal/kill",
        "_kiro/terminal/shell_type",
    ],
    "permission": ["session/request_permission"],
    "hooks": [
        "_kiro/hooks/list",
        "_kiro/hooks/executeHook",
        "_kiro/hooks/sessionStart",
        "_kiro/hooks/cancel",
        "_kiro/hooks/didChange",
    ],
}


def capture_methods(path: Path) -> dict:
    """Return direction-separated methods and reject malformed frames."""
    by_direction = {"in": set(), "out": set()}
    malformed = []
    if not path.is_file():
        return {"missing": True, "methods": {}, "malformed": [str(path)]}
    for line_number, line in enumerate(path.read_text().splitlines(), 1):
        try:
            frame = json.loads(line)
            if not isinstance(frame, dict):
                malformed.append(line_number)
                continue
            message = frame.get("msg", frame)
            direction = {
                "client->agent": "out",
                "agent->client": "in",
            }.get(frame.get("dir"), frame.get("dir"))
            if not isinstance(message, dict):
                malformed.append(line_number)
                continue
            method = message.get("method")
            has_method = isinstance(method, str) and bool(method)
            has_result = "result" in message
            has_error = "error" in message
            is_request_or_notification = has_method and not has_result and not has_error
            is_response = (
                not has_method
                and "id" in message
                and (has_result ^ has_error)
            )
            if (
                direction not in by_direction
                or message.get("jsonrpc") != "2.0"
                or not (is_request_or_notification or is_response)
            ):
                malformed.append(line_number)
                continue
            if is_request_or_notification:
                by_direction[direction].add(method)
        except (json.JSONDecodeError, AttributeError, TypeError):
            malformed.append(line_number)
    return {
        "missing": False,
        "methods": {direction: sorted(methods) for direction, methods in by_direction.items()},
        "malformed": malformed,
    }


captures = {
    "v2_extension_reference": repo / "experiments/conductor-spike/v2-live-session-trace-2.11.0.jsonl",
    "kas_extension_reference": repo / "experiments/conductor-spike/kas-workflow-channels-live-2.20.1.jsonl",
}
capture_results = {name: capture_methods(path) for name, path in captures.items()}
contract_presence = {
    method: method in covenant for method in REQUEST_METHODS + NOTIFICATION_METHODS if method.startswith("_kiro/")
}
reference_expectations = {
    "v2_extension_reference": {
        "in": [
            "session/update",
            "_kiro.dev/commands/available",
            "_kiro.dev/metadata",
        ],
    },
    "kas_extension_reference": {
        "in": ["_kiro/auth/getAccessToken", "_kiro/mcp/status"],
    },
}
reference_checks = {}
for name, directions in reference_expectations.items():
    observed = capture_results[name]["methods"]
    reference_checks[name] = {
        direction: {
            method: method in observed.get(direction, [])
            for method in expected
        }
        for direction, expected in directions.items()
    }

all_methods = REQUEST_METHODS + NOTIFICATION_METHODS
family_methods = [method for methods in FAMILIES.values() for method in methods]
independent_method_table_complete = (
    len(REQUEST_METHODS) == 18
    and len(NOTIFICATION_METHODS) == 2
    and len(set(REQUEST_METHODS)) == len(REQUEST_METHODS)
    and len(set(NOTIFICATION_METHODS)) == len(NOTIFICATION_METHODS)
    and set(REQUEST_METHODS).isdisjoint(NOTIFICATION_METHODS)
    and len(family_methods) == len(all_methods)
    and len(set(family_methods)) == len(family_methods)
    and set(family_methods) == set(all_methods)
)
facts = {
    "claim_ids": ["C2", "C7"],
    "evidence_phases": {
        "authenticated_live": "e5 Rust run_live only; model/tool-dependent families are not inferred",
        "deterministic": "direct SDK callback matrix, exactly 18 requests and 2 notifications",
        "capture_backed": "versioned extension references only; captures are not parity baselines",
    },
    "required_matrix_size": len(REQUEST_METHODS) + len(NOTIFICATION_METHODS),
    "request_methods": REQUEST_METHODS,
    "notification_methods": NOTIFICATION_METHODS,
    "method_families": FAMILIES,
    "kiro_covenant_presence": contract_presence,
    "reference_captures": capture_results,
    "reference_expectations": reference_checks,
    "independent_method_table_complete": independent_method_table_complete,
    "capture_contract_valid": all(
        not value["missing"] and not value["malformed"] for value in capture_results.values()
    ),
    "capture_expectations_present": all(
        all(methods.values())
        for directions in reference_checks.values()
        for methods in directions.values()
    ),
    "live_sdk2_parity_proven_by_offline_oracle": False,
    "live_sdk2_parity_scope": "N/A — authenticated parity is owned by the direct/conductor probes",
}
facts["offline_contract_oracle_passed"] = all(
    [
        facts["independent_method_table_complete"],
        facts["capture_contract_valid"],
        facts["capture_expectations_present"],
        all(contract_presence.values()),
    ]
)
print(json.dumps(facts, indent=2, sort_keys=True))
if not facts["offline_contract_oracle_passed"]:
    raise SystemExit(1)
