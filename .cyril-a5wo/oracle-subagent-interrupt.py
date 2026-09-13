#!/usr/bin/env python3
"""Independent JSONL oracle for live subagent interruption evidence."""
import json, glob, sys

paths = sys.argv[1:] or sorted(glob.glob(".cyril-a5wo/captures/attempt-*.jsonl"))
if not paths:
    raise SystemExit("no capture files")

def records(path):
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            try:
                yield json.loads(line)
            except json.JSONDecodeError:
                continue

def updates(path):
    for rec in records(path):
        if rec.get("method") != "session/update":
            continue
        u = ((rec.get("parsed") or {}).get("params") or {}).get("update") or {}
        if u.get("sessionUpdate") in {"tool_call", "tool_call_update"}:
            yield rec, u

passed = False
for path in paths:
    rows = list(updates(path))
    calls = {}
    cancel = any(rec.get("direction") == "client_to_agent" and rec.get("method") == "session/cancel"
                 for rec in records(path))
    for rec, u in rows:
        if rec.get("direction") == "client_to_agent" and rec.get("method") == "session/cancel":
            cancel = True
        meta = ((u.get("_meta") or {}).get("kiro") or {})
        if meta.get("kind") != "agent-subtask":
            continue
        tid = u.get("toolCallId")
        raw = u.get("rawInput", "<absent>")
        shape = "absent" if raw == "<absent>" else ("object:" + ",".join(sorted(raw)) if isinstance(raw, dict) else type(raw).__name__)
        calls.setdefault(tid, []).append((u.get("sessionUpdate"), shape, u.get("status")))
    partial = [(tid, frames) for tid, frames in calls.items()
               if any(shape == "absent" or shape.startswith("object:") and shape.count(",") < 1
                      for _, shape, _ in frames)]
    recovered = [(tid, frames) for tid, frames in calls.items() if len(frames) > 1]
    result = {"capture": path, "subagent_calls": calls, "cancel_frame": cancel,
              "partial_raw_input": partial, "recovery_frames": recovered}
    ok = bool(cancel and partial and recovered)
    result["verdict"] = "PASS" if ok else "FAIL"
    print(json.dumps(result, sort_keys=True))
    passed |= ok
if not passed:
    raise SystemExit(1)
