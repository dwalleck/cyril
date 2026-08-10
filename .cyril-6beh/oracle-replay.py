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
# Typed snapshot-node fields (manifest snapshot_node_fields, children handled
# structurally): unknown sibling keys on a snapshot node are dropped, exactly
# like the Rust wire types ignore unrecognized fields.
SNAPSHOT_NODE_FIELDS = (
    "nodeId",
    "type",
    "status",
    "agentName",
    "modelId",
    "effortLevel",
    "maxIterations",
    "onMaxIterations",
    "stopCondition",
    "stopWhen",
    "sessionId",
    "artifacts",
    "capturedOutput",
    "failureReason",
    "iteration",
    "branchId",
    "completionSignal",
    "completionSignalSource",
    "startedAt",
    "endedAt",
    "watchCursor",
    "watchTerminal",
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
    """Flatten a snapshot tree to canonical-path → typed-node-data.

    Returns None when two nodes canonicalize to the same path — the Rust
    tracker's WorkflowStateError::DuplicateCanonicalPath — so the caller must
    reject the enclosing snapshot atomically (state unchanged).
    """
    nodes = {}

    def walk(node, path, parent=None):
        segment = node["nodeId"] if parent is None else wrapper_segment(parent, node)
        current = (workflow_id,) if parent is None else (*path, segment)
        if current in nodes:
            return False
        nodes[current] = {
            key: copy.deepcopy(node[key]) for key in SNAPSHOT_NODE_FIELDS if key in node
        }
        return all(walk(child, current, node) for child in node.get("children", []))

    if not walk(root, ()):
        return None
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
        # Raw opening plan, mirroring the Rust run's preserved opening_plan
        # field: it survives snapshot reconciliation (which swaps the projected
        # descriptor to the snapshot root) and is what an active-run run_start
        # duplicate is compared against.
        "openingPlan": descriptor_tree(params["nodeTree"]),
        "nodes": {},
        "terminal": False,
    }


def opening_plan_matches(state, incoming):
    """Mirror of the Rust opening_plan_matches: prefer the preserved raw
    opening plan; fall back to the snapshot root's children when a run was
    seeded without one (unreachable in event-only replay, kept for fidelity)."""
    opening_plan = state.get("openingPlan")
    if opening_plan is not None:
        return opening_plan == incoming
    snapshot = state["run"].get("descriptor")
    if not isinstance(snapshot, dict):
        return False
    key = "branches" if snapshot.get("type") == "parallel" else "steps"
    return snapshot.get(key, []) == incoming


def path_of(params):
    return tuple(params["nodePath"])


def reconcile_snapshot(state, final):
    current = flatten(final["root"], final["workflowId"])
    if current is None:
        # Duplicate canonical path: reject the whole snapshot atomically,
        # mirroring the Rust tracker's DuplicateCanonicalPath rejection.
        print(
            f"workflow event ignored: {final['workflowId']} run_complete duplicate_canonical_path",
            file=sys.stderr,
        )
        return
    prior = state["nodes"]
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
        state = runs.get(workflow_id)
        if state is None or state["terminal"]:
            # Missing run seeds; terminal run is atomically replaced by the
            # new incarnation.
            runs[workflow_id] = opening(params)
            return
        # Active run: an exact duplicate (same workflowName, inputs,
        # parentSessionId presence+value, and declared node tree by descriptor
        # projection) is a silent no-op; any non-exact conflict is warned and
        # ignored. Either way state is unchanged.
        exact_repeat = (
            state["run"].get("workflowName") == params["workflowName"]
            and state["run"].get("inputs") == params["inputs"]
            and state["run"].get("parentSessionId") == params.get("parentSessionId")
            and opening_plan_matches(state, descriptor_tree(params["nodeTree"]))
        )
        if not exact_repeat:
            print(
                f"workflow event ignored: {workflow_id} run_start active_run_start_conflict",
                file=sys.stderr,
            )
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
