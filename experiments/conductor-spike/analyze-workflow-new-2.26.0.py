#!/usr/bin/env python3
"""Summarize a kas-workflow-new-*.jsonl trace: per-session tool-call timeline
(title, kind, _meta.kiro tool/kind, rawInput preview), delegation to
wf-workflow-creator, workflow-tool usage, fs writes and permission requests."""
import json, sys, collections

path = sys.argv[1]
frames = [json.loads(l) for l in open(path, encoding="utf-8") if l.strip()]
t0 = frames[0]["ts"] if frames else 0
main_sid = None
calls = collections.OrderedDict()  # (sid, toolCallId) -> summary
writes, perms, sessions = [], [], collections.Counter()
for f in frames:
    m = f["msg"]
    res = m.get("result")
    if isinstance(res, dict) and "modes" in res and main_sid is None:
        main_sid = res.get("sessionId")
    meth = m.get("method")
    p = m.get("params") if isinstance(m.get("params"), dict) else {}
    if meth == "session/update":
        sid = p.get("sessionId"); sessions[sid] += 1
        u = p.get("update") or {}
        if u.get("sessionUpdate") in ("tool_call", "tool_call_update"):
            key = (sid, u.get("toolCallId"))
            kiro = (u.get("_meta") or {}).get("kiro") or {}
            c = calls.setdefault(key, {"t": round(f["ts"] - t0, 1), "sid": sid, "title": None, "kind": None,
                                       "status": None, "kiro": {}, "rawInput": None})
            for k in ("title", "kind", "status"):
                if u.get(k) is not None: c[k] = u[k]
            if kiro: c["kiro"].update({k: v for k, v in kiro.items() if k in ("kind", "toolName", "agentSubtaskId", "agentName", "subagentName")})
            if u.get("rawInput") is not None: c["rawInput"] = u["rawInput"]
    elif meth in ("fs/write_text_file", "fs/write_file", "_kiro/fs/write_file"):
        writes.append((round(f["ts"] - t0, 1), p.get("sessionId"), p.get("path")))
    elif meth == "session/request_permission":
        tc = p.get("toolCall") or {}
        perms.append((round(f["ts"] - t0, 1), p.get("sessionId"), tc.get("title")))

def tag(sid):
    return "MAIN" if sid == main_sid else (sid or "?")[-8:]

print("== file:", path)
print("== sessions (frames):", {tag(s): n for s, n in sessions.items()})
print("== tool calls:")
for c in calls.values():
    ri = json.dumps(c["rawInput"])[:220] if c["rawInput"] is not None else ""
    print(f"  @{c['t']:>6} [{tag(c['sid'])}] {c['status'] or '':<11} {c['kind'] or '':<8} {c['title']!s:.70} kiro={c['kiro']} in={ri}")
blob = json.dumps([c for c in calls.values()])
print("== delegated to wf-workflow-creator:", "wf-workflow-creator" in blob)
for t in ("save_workflow_definition", "validate_workflow", "run_workflow"):
    print(f"== mentions {t}:", blob.count(t))
print("== fs writes:"); [print("  ", w) for w in writes]
print("== permission requests:"); [print("  ", x) for x in perms]
