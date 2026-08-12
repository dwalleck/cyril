#!/usr/bin/env python3
"""cyril-14ou cheapest falsifier (C11 + C1 skeleton): replay REAL captured frame
timings through the design's decision rule (threshold 30s, stamp on
active-session/global inbound frames, one emission per quiet period, re-arm on
traffic). Expectation: ZERO false stalls across 12 healthy turns; EXACTLY ONE
stall on the bh7g stalled turn. Oracle = the captures themselves (ground truth
of what a healthy/stalled turn looks like), independent of any cyril code.
"""
import json, sys

T = 30.0

def replay(path, kill_ts=None):
    """Feed agent->client session frames + prompt boundaries through the rule."""
    events = []  # (ts, kind)
    file_max_ts = 0.0
    for line in open(path):
        r = json.loads(line)
        file_max_ts = max(file_max_ts, r.get("ts", 0.0))
        m = r.get("msg")
        if not isinstance(m, dict):
            continue
        if r["dir"] == "client->agent" and m.get("method") == "session/prompt":
            events.append((r["ts"], "turn-start"))
        elif r["dir"] == "agent->client":
            if m.get("method") == "session/update":
                events.append((r["ts"], "frame"))
            elif "result" in m and isinstance(m.get("result"), dict) and "stopReason" in m["result"]:
                events.append((r["ts"], "turn-end"))
    stalls, active, last, armed = 0, False, None, True
    # Horizon = the last moment the tap was provably alive and recording
    # (eof/exit records included) — replay time must not stop at the last
    # FRAME, or an end-of-capture stall is invisible by construction.
    horizon = kill_ts or file_max_ts
    for ts, kind in events:
        if active and armed and last is not None and ts - last > T:
            stalls += 1
            armed = False
        if kind == "turn-start":
            active, last, armed = True, ts, True
        elif kind == "turn-end":
            active, last = False, None
        elif kind == "frame" and active:
            if not armed:
                armed = True  # traffic resumed: re-arm (C2)
            last = ts
    if active and last is not None and horizon - last > T:
        stalls += 1  # quiet tail of an unterminated turn
    return stalls

base = sys.argv[1]
healthy = 0
for run in ("run-5", "run-6"):
    s = replay(f"{base}/{run}/wire.jsonl")
    healthy += s
    print(f"{run}: stalls={s} (expect 0)")
stall_run = replay(f"{base}/run-1-stall-archived/wire.jsonl")
print(f"run-1-stall: stalls={stall_run} (expect 1)")
ok = healthy == 0 and stall_run == 1
print("C11/C1 FALSIFIER:", "PASSED" if ok else "FAILED")
sys.exit(0 if ok else 1)
