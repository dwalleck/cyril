#!/usr/bin/env python3
"""Independently fold pause frames from raw or audit-envelope KAS captures."""
import json
import sys
from pathlib import Path


def frames(path):
    for line in Path(path).read_text().splitlines():
        frame = json.loads(line)
        yield frame.get("parsed", frame)


def paused_paths(node, parent=()):
    path = parent + (node["nodeId"],)
    found = {path} if node.get("status") == "paused" else set()
    for child in node.get("children", []):
        found.update(paused_paths(child, path))
    return found


for source in sys.argv[1:]:
    capture = list(frames(source))
    state = {}
    for frame in capture:
        method = frame.get("method", "").removeprefix("_kiro/workflow/")
        params = frame.get("params", {})
        workflow_id = params.get("workflowId")
        if method not in {"node_paused", "paused", "steps_queued", "run_complete"}:
            continue
        run = state.setdefault(workflow_id, {"status": None, "paths": {}, "reason": None})
        if method == "node_paused":
            run["paths"][tuple(params["nodePath"])] = params["reason"]
        elif method == "paused":
            run["status"], run["reason"] = "Paused", params["pauseReason"]
        elif method == "run_complete":
            final = params["finalState"]
            run["status"] = final["status"].title()
            prior_reasons = run["paths"]
            run["paths"] = {
                path: prior_reasons.get(path) for path in paused_paths(final["root"])
            }
        status = "None" if run["status"] is None else f"Some({run['status']})"
        node_reasons = sorted(
            f"{'/'.join(path)}={reason}"
            for path, reason in run["paths"].items()
            if reason is not None
        )
        rendered_node_reasons = json.dumps(node_reasons, ensure_ascii=False)
        reason = "None" if run["reason"] is None else f"Some({json.dumps(run['reason'], ensure_ascii=False)})"
        print(f"{Path(source).name}\t{method}\trun={status}\tpaused_nodes={len(run['paths'])}\tnode_reasons={rendered_node_reasons}\trun_reason={reason}")
        if method == "run_complete" and run["status"] == "Paused":
            break
