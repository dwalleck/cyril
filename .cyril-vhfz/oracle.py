#!/usr/bin/env python3
"""Independently fold pause frames from raw or audit-envelope KAS captures."""
import json
import sys
from pathlib import Path


def frames(path):
    for line in Path(path).read_text().splitlines():
        frame = json.loads(line)
        yield frame.get("parsed", frame)


def paused_tree(node):
    return (node.get("status") == "paused") + sum(
        paused_tree(child) for child in node.get("children", [])
    )


for source in sys.argv[1:]:
    capture = list(frames(source))
    state = {}
    for frame in capture:
        method = frame.get("method", "").removeprefix("_kiro/workflow/")
        params = frame.get("params", {})
        workflow_id = params.get("workflowId")
        if method not in {"node_paused", "paused", "steps_queued", "run_complete"}:
            continue
        run = state.setdefault(workflow_id, {"status": None, "paths": set(), "reason": None})
        if method == "node_paused":
            run["paths"].add(tuple(params["nodePath"]))
        elif method == "paused":
            run["status"], run["reason"] = "Paused", params["pauseReason"]
        elif method == "run_complete":
            final = params["finalState"]
            run["status"] = final["status"].title()
            run["paths"] = {("snapshot", index) for index in range(paused_tree(final["root"]))}
        status = "None" if run["status"] is None else f"Some({run['status']})"
        reason = "None" if run["reason"] is None else f"Some({json.dumps(run['reason'], ensure_ascii=False)})"
        print(f"{Path(source).name}\t{method}\trun={status}\tpaused_nodes={len(run['paths'])}\trun_reason={reason}")
        if method == "run_complete" and run["status"] == "Paused":
            break
