#!/usr/bin/env python3
"""cyril-14ou probe Q1: what inter-frame silence distinguishes a healthy KAS turn
from the captured stall? Probe = tap wire captures (frame arrival times).
Oracle = KAS's persisted messages.jsonl timestamps (independent recorder).
"""
import json
import sys
from datetime import datetime, timezone

def wire_gaps(path):
    """Per-turn max inter-frame gap (agent->client frames only), keyed by turn index."""
    turns, cur, last = [], None, None
    for line in open(path):
        r = json.loads(line)
        if not isinstance(r.get("msg"), dict):
            continue
        if r["dir"] == "client->agent" and r["msg"].get("method") == "session/prompt":
            if cur is not None:
                turns.append(cur)
            cur, last = 0.0, r["ts"]
            continue
        if r["dir"] != "agent->client":
            continue
        m = r["msg"]
        is_frame = m.get("method") == "session/update" or ("result" in m and isinstance(m.get("result"), dict) and "stopReason" in m["result"])
        if not is_frame or cur is None:
            continue
        cur = max(cur, r["ts"] - last)
        last = r["ts"]
        if "result" in m:
            turns.append(cur)
            cur = None
    if cur is not None:
        turns.append(cur)  # unterminated (stalled) turn: gap up to last frame
    return turns

def transcript_gaps(path):
    """Oracle: per-turn max inter-record gap from KAS's own persisted transcript."""
    turns, cur, last = [], None, None
    for line in open(path):
        m = json.loads(line)
        t = datetime.fromisoformat(m["timestamp"].replace("Z", "+00:00")).timestamp()
        kind = m["payload"].get("type")
        if kind == "user":
            if cur is not None:
                turns.append(cur)
            cur, last = 0.0, t
            continue
        if cur is None:
            continue
        cur = max(cur, t - last)
        last = t
        if kind == "turn_end":
            turns.append(cur)
            cur = None
    if cur is not None:
        turns.append(cur)
    return turns

def wire_durations(path):
    """Per-turn prompt→response duration, paired by JSON-RPC id (order-based
    pairing misattributes turns under the turn_end/response interleave)."""
    prompts, resolved = {}, {}
    last_frame = None
    for line in open(path):
        r = json.loads(line)
        m = r.get("msg")
        if not isinstance(m, dict):
            continue
        if r["dir"] == "client->agent" and m.get("method") == "session/prompt":
            prompts[m["id"]] = r["ts"]
        if r["dir"] == "agent->client":
            if m.get("method") == "session/update":
                last_frame = r["ts"]
            if "result" in m and isinstance(m.get("result"), dict) and "stopReason" in m["result"] and m.get("id") in prompts:
                resolved[m["id"]] = r["ts"] - prompts[m["id"]]
    out = [round(resolved[i], 1) for i in sorted(resolved)]
    unresolved = [i for i in sorted(prompts) if i not in resolved]
    for i in unresolved:
        out.append("id=%s STALLED: last frame at +%.1fs, then silence" % (i, (last_frame or prompts[i]) - prompts[i]))
    return out

def transcript_durations(path):
    out, t0 = [], None
    for line in open(path):
        m = json.loads(line)
        t = datetime.fromisoformat(m["timestamp"].replace("Z", "+00:00")).timestamp()
        k = m["payload"].get("type")
        if k == "user":
            t0 = t
        elif k == "turn_end" and t0:
            out.append(t - t0)
            t0 = None
    return out

wire = sys.argv[1]
transcript = sys.argv[2] if len(sys.argv) > 2 else None
print(f"WIRE   max-gap/turn (s): {[round(g,1) for g in wire_gaps(wire)]}")
print(f"WIRE   duration/turn (s): {[g if isinstance(g,str) else round(g,1) for g in wire_durations(wire)]}")
if transcript:
    print(f"ORACLE max-record-gap/turn (s): {[round(g,1) for g in transcript_gaps(transcript)]}")
    print(f"ORACLE duration/turn (s): {[round(g,1) for g in transcript_durations(transcript)]}")
