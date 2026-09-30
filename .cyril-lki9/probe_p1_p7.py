#!/usr/bin/env python3
"""cyril-lki9 probe for P1 and P7, over committed KAS 0.66.15 captures.

P1: in each turn (ANY session; main-only scoping was a probe bug, see evidence.md) that cyril did NOT start (no preceding client
    session/prompt), which session/update kinds carry _meta.kiro.agentInitiated,
    and do the turn_start / turn_end session_info_update frames carry it?
P7: for each such turn, does the triggering _kiro/workflow/run_complete arrive
    BEFORE the turn's first agentInitiated-tagged frame?

Mechanism: parse JSONL, segment every session's frames into turns by turn_start /
turn_end, attribute each turn to "client" when a client session/prompt for that
session was sent after its previous turn_end, else "server".

Run (from experiments/conductor-spike/):
  python3 <worktree>/.cyril-lki9/probe_p1_p7.py \
    kas-workflow-channels-06615-restate-gateoff-tail-2.26.0.jsonl kas-workflow-new-06615-gateon-2.26.0.jsonl
"""
import collections, json, sys

def main_session(frames):
    for f in frames:
        r = f["msg"].get("result")
        if isinstance(r, dict) and "modes" in r and r.get("sessionId"):
            return r["sessionId"]

def analyze(path):
    frames = [json.loads(l) for l in open(path, encoding="utf-8") if l.strip()]
    main = main_session(frames)
    cur, pending_prompt = {}, set()
    turns, last_run_complete = [], None
    for f in frames:
        m = f["msg"]; p = m.get("params") if isinstance(m.get("params"), dict) else {}
        sid = p.get("sessionId")
        if f["dir"] == "client->agent" and m.get("method") == "session/prompt":
            pending_prompt.add(sid)
        if m.get("method") == "_kiro/workflow/run_complete":
            last_run_complete = f["ts"]
        if m.get("method") != "session/update":
            continue
        u = p.get("update") or {}
        kiro = ((u.get("_meta") or {}).get("kiro") or {})
        kind = kiro.get("kind") if u.get("sessionUpdate") == "session_info_update" else None
        if kind == "turn_start":
            t = {"session": "MAIN" if sid == main else sid[-8:], "origin": "client" if sid in pending_prompt else "server",
                 "start_tagged": "agentInitiated" in kiro, "tagged": collections.Counter(), "untagged": collections.Counter(),
                 "first_tagged_ts": None, "run_complete_before_start": last_run_complete, "end_tagged": None, "reasons": set()}
            pending_prompt.discard(sid); cur[sid] = t; turns.append(t); continue
        t = cur.get(sid)
        if t is None:
            continue
        if kind == "turn_end":
            t["end_tagged"] = "agentInitiated" in kiro; cur.pop(sid); continue
        su = u.get("sessionUpdate")
        if su in ("agent_message_chunk", "agent_thought_chunk", "tool_call", "tool_call_update"):
            if kiro.get("agentInitiated"):
                t["tagged"][su] += 1; t["reasons"].add(kiro.get("agentInitiatedReason"))
                if t["first_tagged_ts"] is None: t["first_tagged_ts"] = f["ts"]
            else:
                t["untagged"][su] += 1
    out = []
    for i, t in enumerate(turns):
        rc = t["run_complete_before_start"]
        wf_wake = "workflow-complete-wake" in t["reasons"]
        out.append({"turn": i, "session": t["session"], "origin": t["origin"], "turn_start_tagged": t["start_tagged"], "turn_end_tagged": t["end_tagged"],
                    "tagged": dict(t["tagged"]), "untagged": dict(t["untagged"]), "reasons": sorted(r for r in t["reasons"] if r),
                    "run_complete_precedes_first_tagged": (rc is not None and t["first_tagged_ts"] is not None and rc <= t["first_tagged_ts"]) if wf_wake else None})
    return out

for path in sys.argv[1:]:
    print("==", path.rsplit("/", 1)[-1])
    for row in analyze(path):
        print("  ", json.dumps(row, sort_keys=True))
