#!/usr/bin/env python3
"""Project the last KAS workflow finalState without using Cyril code."""

import json
import re
import sys
from pathlib import Path

MANIFEST = json.loads(
    (
        Path(__file__).resolve().parent.parent
        / "crates/cyril-core/tests/fixtures/kas/workflow/oracle-manifest.json"
    ).read_text(encoding="utf-8")
)
RUN_FIELDS = {"workflowId"} | (
    set(MANIFEST["snapshot_owned_run_fields"]) - {"root"}
)
DESCRIPTOR_FIELDS = {
    field
    for shape in MANIFEST["descriptor_fields"].values()
    for field in shape["required"] + shape["optional"]
} - {"steps", "branches"}
NODE_FIELDS = DESCRIPTOR_FIELDS | (
    set(MANIFEST["snapshot_owned_node_fields"]) - {"descriptor"}
)


def select_fields(value, fields):
    return {key: item for key, item in value.items() if key in fields}


def frame_body(frame):
    parsed = frame.get("parsed")
    return parsed if isinstance(parsed, dict) else frame


# Verbatim port of the KAS reference flattener (H1n, kiro-cli-chat 2.16.0):
# a present iteration wins outright; otherwise a trailing #<ascii-digits>
# rewrites with the digits verbatim; child type and parent id never consulted.
WRAPPER_SUFFIX = re.compile(r"#([0-9]+)$")


def wrapper_segment(parent, child):
    if parent.get("type") == "repeat":
        iteration = child.get("iteration")
        if iteration is not None and not isinstance(iteration, bool):
            return f"iter-{iteration}"
        node_id = child.get("nodeId")
        if isinstance(node_id, str):
            match = WRAPPER_SUFFIX.search(node_id)
            if match is not None:
                return f"iter-{match.group(1)}"
    return child.get("nodeId")


def descriptor(node):
    result = select_fields(node, DESCRIPTOR_FIELDS)
    children = node.get("children")
    if children is not None:
        key = "branches" if node["type"] == "parallel" else "steps"
        result[key] = [descriptor(child) for child in children]
    return result


def project(path):
    frames = [json.loads(line) for line in path.read_text().splitlines() if line]
    completions = [
        frame_body(frame)
        for frame in frames
        if frame_body(frame).get("method") == "_kiro/workflow/run_complete"
    ]
    if not completions:
        raise SystemExit(f"no workflow completion in {path}")
    state = completions[-1]["params"]["finalState"]
    workflow_id = state["workflowId"]
    entries = []
    seen = set()

    def walk(node, node_path):
        if node_path in seen:
            raise SystemExit(f"duplicate canonical path: {node_path!r}")
        seen.add(node_path)
        data = select_fields(node, NODE_FIELDS)
        entries.append({"path": list(node_path), "data": data})
        for child in node.get("children", []):
            walk(child, (*node_path, wrapper_segment(node, child)))

    walk(state["root"], (workflow_id,))
    run = select_fields(state, RUN_FIELDS)
    run["descriptor"] = descriptor(state["root"])
    entries.sort(key=lambda entry: entry["path"])
    return {"run": run, "nodes": entries}


def main():
    if len(sys.argv) < 2:
        raise SystemExit("usage: oracle-snapshot.py CAPTURE.jsonl [CAPTURE.jsonl ...]")
    projections = [project(Path(arg)) for arg in sys.argv[1:]]
    json.dump(projections, sys.stdout, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
