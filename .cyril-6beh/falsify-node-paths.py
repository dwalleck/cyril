#!/usr/bin/env python3
"""Falsify repeat identity translation and wrapper-metadata preservation."""
import json
import re
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

# Verbatim port of the KAS reference flattener (H1n, kiro-cli-chat 2.16.0),
# superseding the D21 discriminator (review finding CR1): under a repeat
# parent a present iteration wins outright; otherwise a trailing
# #<ascii-digits> suffix rewrites with the digits verbatim; child type and
# parent id are never consulted; everything else keeps its literal nodeId.
WRAPPER_SUFFIX = re.compile(r"#([0-9]+)$")


def wrapper_iteration_segment(parent, child):
    if parent.get("type") != "repeat":
        return None
    iteration = child.get("iteration")
    if iteration is not None:
        return f"iter-{iteration}"
    match = WRAPPER_SUFFIX.search(child["nodeId"])
    if match is not None:
        return f"iter-{match.group(1)}"
    return None


def walk(node, path):
    for child in node.get("children", []):
        segment = wrapper_iteration_segment(node, child)
        wrapper = segment is not None
        child_path = (*path, segment if wrapper else child["nodeId"])
        if wrapper:
            iterations[child_path] = {
                key: value for key, value in child.items() if key != "children"
            }
        else:
            event_paths.add(child_path)
        walk(child, child_path)


repeat_parent = {"nodeId": "loop", "type": "repeat"}
step_parent = {"nodeId": "loop", "type": "step"}
controls = [
    (repeat_parent, {"nodeId": "loop#2", "type": "sequence", "iteration": 2}, "iter-2"),
    # Suffix-only rewrite: no iteration supplied, ASCII digits after '#' win.
    (repeat_parent, {"nodeId": "loop#2", "type": "sequence"}, "iter-2"),
    # A present iteration wins outright over a disagreeing suffix.
    (repeat_parent, {"nodeId": "loop#2", "type": "sequence", "iteration": 3}, "iter-3"),
    # Child type is never consulted.
    (repeat_parent, {"nodeId": "loop#2", "type": "step", "iteration": 2}, "iter-2"),
    # Iteration wins even without any digit suffix.
    (repeat_parent, {"nodeId": "loop#x", "type": "sequence", "iteration": 0}, "iter-0"),
    # Non-ASCII digits never match the suffix rule.
    (repeat_parent, {"nodeId": "loop#二", "type": "sequence"}, "loop#二"),
    # Suffix digits are taken verbatim, leading zeros included.
    (repeat_parent, {"nodeId": "loop#02", "type": "sequence"}, "iter-02"),
    # Parent id is never consulted: a foreign prefix still rewrites.
    (repeat_parent, {"nodeId": "literal#2", "type": "sequence", "iteration": 2}, "iter-2"),
    (repeat_parent, {"nodeId": "literal#7", "type": "sequence"}, "iter-7"),
    # A non-repeat parent keeps every child literal.
    (step_parent, {"nodeId": "loop#2", "type": "sequence", "iteration": 2}, "loop#2"),
]
for control_parent, child, expected in controls:
    segment = wrapper_iteration_segment(control_parent, child)
    actual = child["nodeId"] if segment is None else segment
    if actual != expected:
        raise SystemExit(
            f"C05 false: vendor rule produced {actual!r} for {child!r}, wanted {expected!r}"
        )


walk(state["root"], (workflow_id,))
if event_paths != observed:
    raise SystemExit(
        f"C05 false: snapshot={sorted(event_paths)!r} wire={sorted(observed)!r}"
    )
RUNTIME_FIELDS = ("type", "status", "startedAt", "endedAt")
for path, metadata in iterations.items():
    expected = int(path[-1].removeprefix("iter-"))
    if metadata.get("nodeId") != f"loop#{expected}" or metadata.get("iteration") != expected:
        raise SystemExit(f"C05 false: {path!r} lost wrapper identity: {metadata!r}")
    missing = [field for field in RUNTIME_FIELDS if field not in metadata]
    if missing:
        raise SystemExit(
            f"C05 false: {path!r} iteration entry dropped supplied runtime fields {missing!r}: {metadata!r}"
        )
    if metadata["type"] != "sequence" or metadata["status"] != "completed":
        raise SystemExit(
            f"C05 false: {path!r} runtime fields not preserved verbatim: {metadata!r}"
        )
print(
    f"C05 passed: {len(event_paths)} event paths match wire; "
    f"{len(iterations)} iteration entries preserve identity + runtime fields "
    f"({', '.join(RUNTIME_FIELDS)}); "
    f"{len(controls)} vendor-rule controls produce exact segments"
)
