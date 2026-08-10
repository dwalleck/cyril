#!/usr/bin/env python3
"""Independently fold KAS workflow JSONL into deterministic current-state views."""

import copy
import re
import json
import sys
from pathlib import Path

TERMINAL = {"completed", "failed", "aborted"}
NODE_OPTIONALS = {
    "node_start": ("agentName", "sessionId", "prompt", "iteration", "branchId"),
    "node_complete": (
        "artifacts",
        "capturedOutput",
        "failureReason",
        "completionSignal",
        "completionSignalSource",
    ),
}
EVENT_ONLY_NODE = ("prompt", "nodePauseReason", "latestLoopIteration", "latestWatchPoll")
RUN_FIELDS = (
    "workflowId",
    "workflowName",
    "status",
    "inputs",
    "artifacts",
    "capturedOutputs",
    "createdAt",
    "planRevision",
    "parentSessionId",
    "workspacePath",
)


def body(frame):
    parsed = frame.get("parsed")
    return parsed if isinstance(parsed, dict) else frame


def frames(path):
    return [body(json.loads(line)) for line in Path(path).read_text().splitlines() if line]


def method_kind(frame):
    method = frame.get("method", "")
    if not isinstance(method, str):
        return None
    for prefix in ("_kiro/workflow/", "kiro/workflow/"):
        if method.startswith(prefix):
            return method[len(prefix) :]
    return None


def descriptor(node):
    result = {"nodeId": node["nodeId"], "type": node["type"]}
    for key in ("agentName", "modelId", "effortLevel", "maxIterations", "onMaxIterations", "stopCondition", "stopWhen", "handlerName"):
        if key in node:
            result[key] = copy.deepcopy(node[key])
    children = node.get("steps", node.get("branches", node.get("children")))
    if children is not None:
        key = "branches" if node["type"] == "parallel" else "steps"
        result[key] = [descriptor(child) for child in children]
    return result

def descriptor_tree(value):
    if isinstance(value, list):
        return [descriptor(node) for node in value]
    return descriptor(value)


# Verbatim port of the KAS reference flattener (H1n, kiro-cli-chat 2.16.0):
# a present iteration wins outright; otherwise a trailing #<ascii-digits>
# rewrites with the digits verbatim; child type and parent id never consulted.
WRAPPER_SUFFIX = re.compile(r"#([0-9]+)$")


def wrapper_segment(parent, child):
    if parent.get("type") == "repeat":
        iteration = child.get("iteration")
        if iteration is not None:
            return f"iter-{iteration}"
        match = WRAPPER_SUFFIX.search(child["nodeId"])
        if match is not None:
            return f"iter-{match.group(1)}"
    return child["nodeId"]


def flatten(root, workflow_id):
    nodes = {}

    def walk(node, path, parent=None):
        segment = node["nodeId"] if parent is None else wrapper_segment(parent, node)
        current = (workflow_id,) if parent is None else (*path, segment)
        data = {key: copy.deepcopy(value) for key, value in node.items() if key != "children"}
        nodes[current] = data
        for child in node.get("children", []):
            walk(child, current, node)

    walk(root, ())
    return nodes


def opening(params):
    return {
        "run": {
            "workflowId": params["workflowId"],
            "workflowName": params["workflowName"],
            "inputs": copy.deepcopy(params["inputs"]),
            **({"parentSessionId": params["parentSessionId"]} if "parentSessionId" in params else {}),
            "descriptor": descriptor_tree(params["nodeTree"]),
        },
        "nodes": {},
        "terminal": False,
    }


def path_of(params):
    return tuple(params["nodePath"])


def reconcile_snapshot(state, final):
    prior = state["nodes"]
    current = flatten(final["root"], final["workflowId"])
    for path, node in current.items():
        old = prior.get(path, {})
        for key in EVENT_ONLY_NODE:
            if key in old:
                node[key] = copy.deepcopy(old[key])
    preserved = {
        key: copy.deepcopy(state["run"][key])
        for key in ("pendingSteps", "queueResolution", "runPauseReason")
        if key in state["run"]
    }
    state["run"] = {key: copy.deepcopy(final[key]) for key in RUN_FIELDS if key in final}
    state["run"]["descriptor"] = descriptor(final["root"])
    state["run"].update(preserved)
    state["nodes"] = current
    state["terminal"] = final["status"] in TERMINAL


def apply(runs, frame):
    kind = method_kind(frame)
    if kind is None:
        return
    params = frame.get("params")
    if not isinstance(params, dict) or "workflowId" not in params:
        return
    workflow_id = params["workflowId"]
    if kind == "run_start":
        runs[workflow_id] = opening(params)
        return
    state = runs.get(workflow_id)
    if state is None or state["terminal"]:
        return
    if kind == "node_start":
        path = path_of(params)
        node = state["nodes"].setdefault(path, {})
        node["nodeId"] = params["nodeId"]
        node["type"] = params["type"]
        for key in NODE_OPTIONALS[kind]:
            if key in params:
                node[key] = copy.deepcopy(params[key])
    elif kind == "node_complete":
        node = state["nodes"].get(path_of(params))
        if node is not None:
            node["status"] = params["status"]
            for key in NODE_OPTIONALS[kind]:
                if key in params:
                    node[key] = copy.deepcopy(params[key])
    elif kind == "node_paused":
        node = state["nodes"].get(path_of(params))
        if node is not None:
            node["status"] = "paused"
            node["nodePauseReason"] = params["reason"]
    elif kind == "loop_iteration":
        matches = [node for node in state["nodes"].values() if node.get("nodeId") == params["loopId"] and node.get("type") == "repeat"]
        if len(matches) == 1:
            matches[0]["latestLoopIteration"] = {
                "iteration": params["iteration"],
                "stopConditionMet": params["stopConditionMet"],
            }
    elif kind == "watch_poll":
        node = state["nodes"].get(path_of(params))
        if node is not None:
            node["latestWatchPoll"] = {"outcome": params["outcome"], "at": params["at"]}
    elif kind == "paused":
        state["run"]["status"] = "paused"
        state["run"]["runPauseReason"] = params["pauseReason"]
    elif kind == "steps_queued":
        resolution = params.get("resolution")
        if resolution is None:
            state["run"]["pendingSteps"] = copy.deepcopy(params["pendingSteps"])
        else:
            state["run"]["queueResolution"] = copy.deepcopy(resolution)
    elif kind == "run_complete":
        final = params["finalState"]
        if final.get("status") == params.get("status"):
            reconcile_snapshot(state, final)


def projection(runs):
    result = []
    for workflow_id, state in sorted(runs.items()):
        nodes = [
            {"path": list(path), "data": copy.deepcopy(data)}
            for path, data in sorted(state["nodes"].items())
        ]
        result.append({"workflowId": workflow_id, "run": copy.deepcopy(state["run"]), "nodes": nodes})
    return result


def fold(sequence, passes):
    runs = {}
    checkpoints = {}
    for _ in range(passes):
        for frame in sequence:
            apply(runs, frame)
            checkpoint = frame.get("checkpoint")
            if checkpoint:
                checkpoints[checkpoint] = projection(runs)
    return {"checkpoints": checkpoints, "final": projection(runs)}


def main():
    if len(sys.argv) < 2:
        raise SystemExit(f"usage: {Path(sys.argv[0]).name} CAPTURE.jsonl...")
    results = []
    for name in sys.argv[1:]:
        sequence = frames(name)
        one = fold(sequence, 1)
        two = fold(sequence, 2)
        if one != two:
            raise SystemExit(f"one/two replay mismatch in {name}")
        results.append({"source": Path(name).name, "expected": one, "oneEqualsTwo": True})
    json.dump(results, sys.stdout, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
