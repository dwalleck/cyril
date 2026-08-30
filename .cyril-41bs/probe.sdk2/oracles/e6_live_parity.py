#!/usr/bin/env python3
"""Compose authenticated direct and conductor parity without trusting Rust claims."""

import copy
import json
import subprocess
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[1]
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
TOPOLOGIES = ["zero-proxy", "no-op-proxy", "transforming-proxy"]
CALLBACK_TOPOLOGIES = ["direct", *TOPOLOGIES]
LIVE_ENGINES = ["v2", "kas"]
LIVE_NOT_EXERCISED = ["typed_error", "outer_response_id", "cancellation"]
EXPECTED_TYPED_ERRORS = [{"code": -32602, "data": {"probe": "invalid-params"}}]
LIVE_SEQUENCE_EVENTS = {
    "request:_kiro/auth/getAccessToken",
    "response:initialize",
    "request:_kiro/terminal/shell_type",
    "response:session/new",
    "request:session/request_permission",
    "request:terminal/create",
    "request:terminal/wait_for_exit",
    "request:terminal/output",
    "request:terminal/release",
    "response:session/prompt",
}
V2_LIFECYCLE = [
    "response:initialize",
    "response:session/new",
    "response:session/prompt",
]
KAS_LIFECYCLE = [
    "request:_kiro/auth/getAccessToken",
    "response:initialize",
    "request:_kiro/terminal/shell_type",
    "response:session/new",
    "request:session/request_permission",
    "request:terminal/create",
    "request:terminal/wait_for_exit",
    "request:terminal/output",
    "request:terminal/release",
    "response:session/prompt",
]
V2_NAMED_NA = ["v2:host-callbacks-not-live-proven-without-tool-trigger"]


def run_probe(binary: str, *args: str) -> dict:
    try:
        completed = subprocess.run(
            ["cargo", "run", "--quiet", "--bin", binary, "--", *args],
            cwd=root,
            text=True,
            capture_output=True,
            timeout=600,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        partial_stderr = error.stderr or ""
        raise SystemExit(
            f"{binary} {' '.join(args)} exceeded the 600-second composed bound; "
            f"partial stderr: {partial_stderr}"
        ) from error
    if completed.returncode != 0:
        raise SystemExit(completed.stdout + completed.stderr)
    try:
        return json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise SystemExit(f"{binary} emitted non-JSON output: {error}") from error


def transform_divergences(value: dict, phase: str) -> list[str]:
    topology = value.get("topology")
    markers = value.get("proxy_transformations", [])
    transformed_requests = value.get("transformed_callback_requests", 0)
    if not isinstance(markers, list):
        return ["transform_marker:not_list"]
    if topology != "transforming-proxy":
        divergences = ["transform_marker:unexpected"] if markers else []
        if phase == "callback" and transformed_requests != 0:
            divergences.append("transform_payload:unexpected")
        return divergences
    if phase == "callback":
        expected = [f"transformed:agent:{method}" for method in REQUEST_METHODS]
        divergences = [] if markers == expected else ["transform_marker:callback"]
        if transformed_requests != len(REQUEST_METHODS):
            divergences.append("transform_payload:callback")
        return divergences
    allowed = {
        "transformed:client:session/prompt",
        "transformed:agent:_kiro.dev/metadata",
        "transformed:agent:_kiro/mcp/status",
    }
    divergences = []
    if markers.count("transformed:client:session/prompt") != 1:
        divergences.append("transform_marker:session_prompt")
    if not any(marker.startswith("transformed:agent:") for marker in markers):
        divergences.append("transform_marker:agent_missing")
    if any(marker not in allowed for marker in markers):
        divergences.append("transform_marker:undeclared")
    return divergences


def callback_cell_divergences(value: dict) -> list[str]:
    divergences = []
    expected_events = [
        *[f"request:{method}" for method in REQUEST_METHODS],
        *[f"notification:{method}" for method in NOTIFICATION_METHODS],
    ]
    if value.get("normalized_events") != expected_events:
        divergences.append("callback_order_or_count")
    if value.get("contract") != {
        "request_methods": REQUEST_METHODS,
        "notification_methods": NOTIFICATION_METHODS,
        "request_count": len(REQUEST_METHODS),
        "notification_count": len(NOTIFICATION_METHODS),
    }:
        divergences.append("callback_contract")
    event_count = value.get("event_count")
    if (
        value.get("within_event_bound") is not True
        or not isinstance(event_count, int)
        or not 0 <= event_count <= 1_000
    ):
        divergences.append("event_bound")
    if value.get("all_requests_answered") is not True:
        divergences.append("all_requests_answered:false")
    if value.get("response_count") != len(REQUEST_METHODS):
        divergences.append("response_count:18")
    if value.get("cancellation_count") != 0:
        divergences.append(f"cancellation_count:{value.get('cancellation_count')}")
    request_ids = value.get("request_ids")
    response_ids = value.get("response_ids")
    pairs = value.get("response_id_pairs")
    ids_valid = (
        isinstance(request_ids, list)
        and isinstance(response_ids, list)
        and len(request_ids) == len(REQUEST_METHODS)
        and len(response_ids) == len(REQUEST_METHODS)
        and request_ids == response_ids
        and all(request_ids.count(request_id) == 1 for request_id in request_ids)
    )
    if not ids_valid:
        divergences.append("response_ids")
    expected_pairs = (
        [
            {"request_id": request_id, "response_id": response_id}
            for request_id, response_id in zip(request_ids, response_ids)
        ]
        if ids_valid
        else None
    )
    if pairs != expected_pairs:
        divergences.append("outer_response_id")
    if value.get("typed_errors") != EXPECTED_TYPED_ERRORS:
        divergences.append("typed_error_data")
    if value.get("typed_error_contract") != EXPECTED_TYPED_ERRORS[0]:
        divergences.append("typed_error_contract")
    divergences.extend(transform_divergences(value, "callback"))
    return divergences


def callback_matrix_divergences(direct: dict, candidate: dict) -> list[str]:
    divergences = [
        *(f"direct:{item}" for item in callback_cell_divergences(direct)),
        *callback_cell_divergences(candidate),
    ]
    if candidate.get("normalized_events") != direct.get("normalized_events"):
        divergences.append("callback_direct_sequence")
    return divergences


def normalize_raw_events(events: list[str]) -> list[str]:
    normalized_events = []
    for event in events:
        if ":id=" in event:
            normalized = event.split(":id=", 1)[0]
        elif event.startswith("notification:session/update:"):
            normalized = "notification:session/update"
        else:
            normalized = event
        repeated_session_update = (
            normalized == "notification:session/update"
            and normalized_events[-1:] == [normalized]
        )
        if not repeated_session_update:
            normalized_events.append(normalized)
    return normalized_events


def lifecycle_sequence(events: list[str]) -> list[str]:
    # Exact topology parity is meaningful for the first occurrence of stable
    # lifecycle milestones. Backend/model progress, optional reads, and a
    # repeated execution of the same requested tool are nondeterministic
    # inputs; the probes retain their complete event streams separately.
    sequence = []
    for event in events:
        if event in LIVE_SEQUENCE_EVENTS and event not in sequence:
            sequence.append(event)
    return sequence


def derive_live_contract(value: dict) -> dict:
    events = value.get("events")
    if not isinstance(events, list) or not all(isinstance(event, str) for event in events):
        events = []
    normalized_events = normalize_raw_events(events)
    prompt_index = next(
        (index for index, event in enumerate(events) if event == "response:session/prompt"),
        None,
    )
    turn_end_index = next(
        (
            index
            for index, event in enumerate(events)
            if event.startswith("notification:session/update:")
            and '"kind":"turn_end"' in event
            and '"stopReason":"end_turn"' in event
        ),
        None,
    )
    permission_index = next(
        (
            index
            for index, event in enumerate(normalized_events)
            if event == "request:session/request_permission"
        ),
        None,
    )
    tool_index = next(
        (
            index
            for index, event in enumerate(normalized_events)
            if event == "request:terminal/create"
        ),
        None,
    )
    methods = [
        event.split(":", 1)[1].split(":id=", 1)[0]
        for event in events
        if event.startswith(("request:", "notification:"))
    ]
    families = {
        "auth": "_kiro/auth/getAccessToken" in methods,
        "filesystem": any(
            method.startswith(("fs/", "_kiro/fs/")) for method in methods
        ),
        "terminal": any(
            method.startswith(("terminal/", "_kiro/terminal/")) for method in methods
        ),
        "permission": "session/request_permission" in methods,
        "hooks": any(method.startswith("_kiro/hooks/") for method in methods),
    }
    return {
        "events_valid": bool(events),
        "normalized_events": normalized_events,
        "prompt_response_last": bool(events) and events[-1] == "response:session/prompt",
        "terminal_before_prompt_response": (
            prompt_index is not None
            if value.get("engine") == "v2"
            else (
                turn_end_index is not None
                and prompt_index is not None
                and turn_end_index < prompt_index
            )
        ),
        "kas_turn_end_observed": turn_end_index is not None,
        "permission_before_tool": (
            permission_index < tool_index
            if permission_index is not None and tool_index is not None
            else None
        ),
        "kas_host_families": families,
        "agent_message_chunks": sum(
            event.startswith("notification:session/update:")
            and '"sessionUpdate":"agent_message_chunk"' in event
            for event in events
        ),
    }


def normalized_live(value: dict) -> dict:
    stop_reason = value.get("stop_reason")
    if stop_reason in {"EndTurn", "end_turn"}:
        stop_reason = "end_turn"
    derived = derive_live_contract(value)
    return {
        "protocol_version": str(value.get("protocol_version", "")).lower().replace("protocolversion(", "").replace(")", ""),
        "session_id_present": value.get("session_id_present") is True,
        "stop_reason": stop_reason,
        "normalized_lifecycle_sequence": lifecycle_sequence(
            derived["normalized_events"]
        ),
        **derived,
        "evidence_layers": value.get("evidence_layers"),
        "not_exercised": value.get("not_exercised"),
    }


def live_cell_divergences(engine: str, value: dict) -> list[str]:
    divergences = []
    normalized = normalized_live(value)
    if normalized["protocol_version"] != "1":
        divergences.append("protocol_version:not_v1")
    if normalized["stop_reason"] != "end_turn":
        divergences.append("stop_reason:not_end_turn")
    if not normalized["session_id_present"]:
        divergences.append("session_id_present:false")
    if not normalized["prompt_response_last"]:
        divergences.append("terminal_order:prompt_response_last")
    if normalized["not_exercised"] != LIVE_NOT_EXERCISED:
        divergences.append("not_exercised")
    events = value.get("events")
    if (
        value.get("within_event_bound") is not True
        or not isinstance(events, list)
        or len(events) > 1_000
    ):
        divergences.append("event_bound")
    if not normalized["events_valid"]:
        divergences.append("events")
    if value.get("normalized_events") != normalized["normalized_events"]:
        divergences.append("normalized_events")
    if value.get("prompt_response_last") is not normalized["prompt_response_last"]:
        divergences.append("prompt_response_last_evidence")
    if (
        value.get("agent_message_chunks") != normalized["agent_message_chunks"]
        or normalized["agent_message_chunks"] <= 0
    ):
        divergences.append("agent_message_chunks")
    evidence_layers = normalized["evidence_layers"]
    expected_na = V2_NAMED_NA if engine == "v2" else []
    if evidence_layers != {
        "authenticated_live": True,
        "deterministic_matrix": False,
        "capture_backed": False,
        "divergences": expected_na,
    }:
        divergences.append("evidence_layers")
    expected_lifecycle = KAS_LIFECYCLE if engine == "kas" else V2_LIFECYCLE
    if normalized["normalized_lifecycle_sequence"] != expected_lifecycle:
        divergences.append("lifecycle_milestones")
    if engine == "kas":
        if value.get("kas_turn_end_observed") is not normalized["kas_turn_end_observed"]:
            divergences.append("kas_turn_end_evidence")
        if (
            value.get("terminal_before_prompt_response")
            is not normalized["terminal_before_prompt_response"]
        ):
            divergences.append("kas_turn_end_order_evidence")
        if value.get("permission_before_tool") != normalized["permission_before_tool"]:
            divergences.append("permission_before_tool_evidence")
        if value.get("kas_host_families") != normalized["kas_host_families"]:
            divergences.append("kas_host_families_evidence")
        if normalized["kas_turn_end_observed"] is not True:
            divergences.append("kas_turn_end")
        if normalized["terminal_before_prompt_response"] is not True:
            divergences.append("kas_turn_end_order")
        if normalized["permission_before_tool"] is not True:
            divergences.append("permission_before_tool")
        families = normalized["kas_host_families"]
        if not isinstance(families, dict) or set(families) != {
            "auth", "filesystem", "terminal", "permission", "hooks"
        } or not all(families.values()):
            divergences.append("kas_host_families")
    divergences.extend(transform_divergences(value, "live"))
    return divergences


def live_divergences(engine: str, direct: dict, candidate: dict) -> list[str]:
    divergences = [
        *(f"direct:{item}" for item in live_cell_divergences(engine, direct)),
        *live_cell_divergences(engine, candidate),
    ]
    baseline = normalized_live(direct)
    observed = normalized_live(candidate)
    for field in ("protocol_version", "stop_reason", "normalized_lifecycle_sequence"):
        if baseline[field] != observed[field]:
            divergences.append(field)
    return divergences


def compare_fixture(name: str, direct: dict, candidate: dict) -> list[str]:
    if name == "missing_callback":
        candidate["normalized_events"] = [
            event for event in candidate["normalized_events"] if event != "request:terminal/create"
        ]
    elif name in {"terminal_order", "missing_agent_message"}:
        raw_events = [
            'request:_kiro/auth/getAccessToken:id=Number(0)',
            "response:initialize",
            'request:_kiro/terminal/shell_type:id=Number(1)',
            "response:session/new",
            'request:_kiro/fs/read_file:id=Number(2)',
            'request:_kiro/hooks/list:id=Number(3)',
            (
                'notification:session/update:{"update":'
                '{"sessionUpdate":"agent_message_chunk"}}'
            ),
            'request:session/request_permission:id=Number(4)',
            'request:terminal/create:id=Number(5)',
            'request:terminal/wait_for_exit:id=Number(6)',
            'request:terminal/output:id=Number(7)',
            'request:terminal/release:id=Number(8)',
            (
                'notification:session/update:{"update":{"sessionUpdate":'
                '"session_info_update","_meta":{"kiro":{"kind":"turn_end",'
                '"stopReason":"end_turn"}}}}'
            ),
            "response:session/prompt",
        ]
        live_base = {
            "engine": "kas",
            "topology": "direct",
            "protocol_version": "1",
            "session_id_present": True,
            "stop_reason": "end_turn",
            "normalized_events": normalize_raw_events(raw_events),
            "prompt_response_last": True,
            "terminal_before_prompt_response": True,
            "kas_turn_end_observed": True,
            "permission_before_tool": True,
            "kas_host_families": {
                "auth": True,
                "filesystem": True,
                "terminal": True,
                "permission": True,
                "hooks": True,
            },
            "agent_message_chunks": 1,
            "events": raw_events,
            "within_event_bound": True,
            "not_exercised": LIVE_NOT_EXERCISED,
            "evidence_layers": {
                "authenticated_live": True,
                "deterministic_matrix": False,
                "capture_backed": False,
                "divergences": [],
            },
            "proxy_transformations": [],
        }
        candidate_live = copy.deepcopy(live_base)
        if name == "terminal_order":
            events = candidate_live["events"]
            events[-2], events[-1] = events[-1], events[-2]
            candidate_live["normalized_events"] = normalize_raw_events(events)
        else:
            candidate_live["events"] = [
                event
                for event in candidate_live["events"]
                if '"sessionUpdate":"agent_message_chunk"' not in event
            ]
            candidate_live["normalized_events"] = normalize_raw_events(
                candidate_live["events"]
            )
        return sorted(set(live_divergences("kas", live_base, candidate_live)))
    elif name == "typed_error_data":
        candidate["typed_errors"] = [{"code": -32602, "data": {"fixture": "changed"}}]
    elif name == "outer_response_id":
        candidate["response_id_pairs"][0]["response_id"] = "wrong-outer-id"
    elif name == "cancellation_count":
        candidate["cancellation_count"] = 2
    elif name == "transform_marker":
        candidate["proxy_transformations"] = []
    elif name == "transform_marker_extra":
        candidate["proxy_transformations"].append("transformed:agent:undeclared")
    else:
        raise ValueError(f"unknown comparator fixture {name}")
    return sorted(set(callback_matrix_divergences(direct, candidate)))


def self_test() -> dict:
    request_ids = list(range(1, 19))
    base = {
        "topology": "transforming-proxy",
        "contract": {
            "request_methods": REQUEST_METHODS,
            "notification_methods": NOTIFICATION_METHODS,
            "request_count": 18,
            "notification_count": 2,
        },
        "normalized_events": [
            *[f"request:{method}" for method in REQUEST_METHODS],
            *[f"notification:{method}" for method in NOTIFICATION_METHODS],
        ],
        "all_requests_answered": True,
        "response_count": 18,
        "request_ids": request_ids,
        "response_ids": request_ids,
        "transformed_callback_requests": 18,
        "cancellation_count": 0,
        "protocol_version": "1",
        "session_id_present": True,
        "stop_reason": "end_turn",
        "prompt_response_last": True,
        "terminal_before_prompt_response": True,
        "not_exercised": LIVE_NOT_EXERCISED,
        "typed_errors": EXPECTED_TYPED_ERRORS,
        "typed_error_contract": EXPECTED_TYPED_ERRORS[0],
        "response_id_pairs": [
            {"request_id": index, "response_id": index}
            for index in request_ids
        ],
        "event_count": 20,
        "within_event_bound": True,
        "proxy_transformations": [
            f"transformed:agent:{method}" for method in REQUEST_METHODS
        ],
    }
    results = {}
    for name in [
        "missing_callback",
        "terminal_order",
        "missing_agent_message",
        "typed_error_data",
        "outer_response_id",
        "cancellation_count",
        "transform_marker",
        "transform_marker_extra",
    ]:
        results[name] = compare_fixture(name, base, copy.deepcopy(base))
    caught = {name: bool(divergences) for name, divergences in results.items()}
    return {
        "claim_ids": ["C2", "C7"],
        "evidence_phase": "deterministic comparator self-tests",
        "self_tests": results,
        "all_self_tests_caught": all(caught.values()),
        "caught": caught,
    }


def compare_live() -> dict:
    direct = run_probe("e5", "all")
    conductor = run_probe("e6_live", "all")
    direct_live = direct.get("live", [])
    conductor_live = conductor.get("topologies", [])
    conductor_callbacks = conductor.get("callback_matrices", [])
    shape_divergences = []

    direct_engines = [entry.get("engine") for entry in direct_live if isinstance(entry, dict)]
    if len(direct_live) != 2 or sorted(direct_engines) != sorted(LIVE_ENGINES):
        shape_divergences.append("direct_live_cells")
    expected_live_pairs = {
        (engine, topology) for engine in LIVE_ENGINES for topology in TOPOLOGIES
    }
    observed_live_pairs = [
        (entry.get("engine"), entry.get("topology"))
        for entry in conductor_live
        if isinstance(entry, dict)
    ]
    if (
        len(conductor_live) != 6
        or len(set(observed_live_pairs)) != 6
        or set(observed_live_pairs) != expected_live_pairs
    ):
        shape_divergences.append("conductor_live_cells")

    callback_direct = direct.get("callback_matrix", {})
    callback_entries = conductor_callbacks
    callback_topologies = [
        entry.get("topology") for entry in callback_entries if isinstance(entry, dict)
    ]
    if (
        len(callback_entries) != 4
        or len(set(callback_topologies)) != 4
        or sorted(callback_topologies) != sorted(CALLBACK_TOPOLOGIES)
    ):
        shape_divergences.append("callback_matrix_cells")

    baselines = {
        entry["engine"]: entry
        for entry in direct_live
        if isinstance(entry, dict) and "engine" in entry
    }
    callback_cells = []
    for candidate in callback_entries:
        if not isinstance(candidate, dict):
            callback_cells.append(
                {
                    "topology": "malformed",
                    "observed_transform_markers": [],
                    "divergences": ["malformed_callback_cell"],
                    "contract_matches_direct": False,
                }
            )
            continue
        topology = candidate.get("topology", "unknown")
        divergences = callback_matrix_divergences(callback_direct, candidate)
        callback_cells.append(
            {
                "topology": topology,
                "observed_transform_markers": candidate.get("proxy_transformations", []),
                "divergences": sorted(set(divergences)),
                "contract_matches_direct": not divergences,
            }
        )

    comparisons = []
    for topology in conductor_live:
        if not isinstance(topology, dict):
            comparisons.append(
                {
                    "engine": None,
                    "topology": "malformed",
                    "divergences": ["malformed_live_cell"],
                    "named_na": [],
                    "direct_lifecycle_sequence": None,
                    "conductor_lifecycle_sequence": None,
                    "contract_matches_direct": False,
                }
            )
            continue
        engine = topology.get("engine")
        baseline = baselines.get(engine)
        divergences = (
            ["missing_direct_baseline"]
            if baseline is None
            else live_divergences(engine, baseline, topology)
        )
        comparisons.append(
            {
                "engine": engine,
                "topology": topology.get("topology"),
                "divergences": sorted(set(divergences)),
                "named_na": topology.get("evidence_layers", {}).get("divergences", []),
                "direct_lifecycle_sequence": (
                    normalized_live(baseline)["normalized_lifecycle_sequence"]
                    if baseline
                    else None
                ),
                "conductor_lifecycle_sequence": normalized_live(topology)[
                    "normalized_lifecycle_sequence"
                ],
                "contract_matches_direct": not divergences,
            }
        )

    result = {
        "claim_ids": ["C2", "C7"],
        "evidence_phases": {
            "authenticated_live": "direct e5 and six conductor sessions",
            "deterministic": "18-request/2-notification matrix in direct and conductor paths",
            "capture_backed": "versioned extension references are not parity substitutes",
        },
        "shape_divergences": shape_divergences,
        "direct_vs_conductor_cells": comparisons,
        "callback_matrix_cells": callback_cells,
        "direct_vs_conductor_equivalent": not shape_divergences and all(
            item["contract_matches_direct"] for item in comparisons + callback_cells
        ),
        "approved_named_divergences": [
            item.get("named_na", []) for item in comparisons if item.get("named_na")
        ],
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    if not result["direct_vs_conductor_equivalent"]:
        raise SystemExit(1)
    return result


if __name__ == "__main__":
    if len(sys.argv) == 2 and sys.argv[1] == "--self-test":
        result = self_test()
        print(json.dumps(result, indent=2, sort_keys=True))
        if not result["all_self_tests_caught"]:
            raise SystemExit(1)
    else:
        compare_live()
