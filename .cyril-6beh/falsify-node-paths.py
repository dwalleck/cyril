#!/usr/bin/env python3
"""Falsify repeat identity translation and wrapper-metadata preservation."""
import json
from pathlib import Path

CAPTURE = Path(__file__).resolve().parents[1] / "experiments/conductor-spike/kas-repeat-watch-2.16.0.jsonl"
frames = [json.loads(line) for line in CAPTURE.read_text().splitlines()]
complete = next(
    frame for frame in reversed(frames)
    if frame.get("method") == "_kiro/workflow/run_complete"
    and frame["params"]["finalState"]["workflowName"] == "cyril-audit-repeat"
)
state = complete["params"]["finalState"]
workflow_id = state["workflowId"]
observed = {
    tuple(frame["params"]["nodePath"])
    for frame in frames
    if frame.get("method") in {
        "_kiro/workflow/node_start",
        "_kiro/workflow/node_complete",
        "_kiro/workflow/watch_poll",
    }
    and frame["params"].get("workflowId") == workflow_id
}
event_paths = set()
iterations = {}


def wrapper_iteration(parent, child):
    if parent.get("type") != "repeat" or child.get("type") != "sequence":
        return None
    prefix = f'{parent["nodeId"]}#'
    node_id = child["nodeId"]
    if not node_id.startswith(prefix):
        return None
    suffix = node_id[len(prefix):]
    iteration = child.get("iteration")
    if (
        not suffix.isdigit()
        or isinstance(iteration, bool)
        or not isinstance(iteration, int)
        or suffix != str(iteration)
    ):
        return None
    return iteration


def walk(node, path):
    for child in node.get("children", []):
        iteration = wrapper_iteration(node, child)
        wrapper = iteration is not None
        segment = f"iter-{iteration}" if wrapper else child["nodeId"]
        child_path = (*path, segment)
        if wrapper:
            iterations[child_path] = {
                "nodeId": child["nodeId"],
                "iteration": child["iteration"],
            }
        else:
            event_paths.add(child_path)
        walk(child, child_path)


parent = {"nodeId": "loop", "type": "repeat"}
controls = [
    ({"nodeId": "loop#2", "type": "sequence", "iteration": 2}, True),
    ({"nodeId": "loop#2", "type": "sequence"}, False),
    ({"nodeId": "loop#2", "type": "sequence", "iteration": 3}, False),
    ({"nodeId": "loop#2", "type": "step", "iteration": 2}, False),
    ({"nodeId": "loop#x", "type": "sequence", "iteration": 0}, False),
    ({"nodeId": "loop#二", "type": "sequence", "iteration": 2}, False),
    ({"nodeId": "loop#02", "type": "sequence", "iteration": 2}, False),
    ({"nodeId": "literal#2", "type": "sequence", "iteration": 2}, False),
]
for child, expected in controls:
    actual = wrapper_iteration(parent, child) is not None
    if actual != expected:
        raise SystemExit(f"C05 false: discriminator misclassified {child!r}")


walk(state["root"], (workflow_id,))
if event_paths != observed:
    raise SystemExit(
        f"C05 false: snapshot={sorted(event_paths)!r} wire={sorted(observed)!r}"
    )
for path, metadata in iterations.items():
    expected = int(path[-1].removeprefix("iter-"))
    if metadata != {"nodeId": f"loop#{expected}", "iteration": expected}:
        raise SystemExit(f"C05 false: {path!r} lost wrapper metadata: {metadata!r}")
print(
    f"C05 passed: {len(event_paths)} event paths match wire; "
    f"{len(iterations)} iteration entries preserve metadata; "
    f"{len(controls)} discriminator controls classify exactly"
)
