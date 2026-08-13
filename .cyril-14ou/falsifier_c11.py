#!/usr/bin/env python3
"""cyril-14ou cheapest falsifier (C11 + C1 skeleton): replay REAL captured frame
timings through the design's decision rule. Hardened per PR #94 review:

- SP9: turn semantics match the mediator exactly (shared with emit_table.py) —
  a turn releases at its wire `turn_end` session_info frame (first terminal
  source) or at the response bearing ITS OWN JSON-RPC id; late responses for
  superseded turns are ignored. Threshold comparison is inclusive (>=), same
  as TurnLiveness::check.
- SP6: turn counts are asserted per capture (a truncated/empty capture fails
  loudly instead of passing vacuously) and a tight-threshold arm requires the
  healthy corpus to be OBSERVABLE (>=1 emission at 3s).

Default captures are the committed copies next to this script, so the run is
reproducible from a checkout: kas-turn-healthy-{a,b}-2.16.2.jsonl and
kas-turn-stall-run-2.16.2.jsonl (sha256s in review-decisions.md).
"""
import json
import os
import sys

T = 30.0
TIGHT = 3.0

def turns(path):
    """(events_rel, completed, horizon_rel) per turn — mediator-faithful."""
    out, active, file_max = [], None, 0.0
    for line in open(path):
        r = json.loads(line)
        file_max = max(file_max, r.get("ts", 0.0))
        m = r.get("msg")
        if not isinstance(m, dict):
            continue
        if r["dir"] == "client->agent" and m.get("method") == "session/prompt":
            if active is not None:
                out.append((active[1], active[2], None))
            active = (m["id"], r["ts"], [])
            continue
        if r["dir"] != "agent->client" or active is None:
            continue
        if m.get("method") == "session/update":
            active[2].append(r["ts"] - active[1])
            u = m.get("params", {}).get("update", {})
            kind = u.get("_meta", {}).get("kiro", {}).get("kind")
            if u.get("sessionUpdate") == "session_info_update" and kind == "turn_end":
                out.append((active[1], active[2], active[2][-1]))
                active = None
        elif "result" in m and isinstance(m.get("result"), dict) and "stopReason" in m["result"]:
            if m.get("id") == active[0]:
                active[2].append(r["ts"] - active[1])
                out.append((active[1], active[2], active[2][-1]))
                active = None
    if active is not None:
        out.append((active[1], active[2], None))
    return [(ev, end is not None, (end if end is not None else file_max - t0)) for t0, ev, end in out]

def stalls(turn, threshold):
    events, completed, horizon = turn
    last, armed, count = 0.0, True, 0
    for e in events:
        if armed and e - last >= threshold:
            count += 1
            armed = False
        last, armed = e, True
    if not completed and armed and horizon - last >= threshold:
        count += 1
    return count

here = os.path.dirname(os.path.abspath(__file__))
if len(sys.argv) > 1:
    base = sys.argv[1]
    paths = {
        "healthy-a": (f"{base}/run-5/wire.jsonl", 4, 0),
        "healthy-b": (f"{base}/run-6/wire.jsonl", 4, 0),
        "stall": (f"{base}/run-1-stall-archived/wire.jsonl", 4, 1),
    }
else:
    paths = {
        "healthy-a": (f"{here}/kas-turn-healthy-a-2.16.2.jsonl", 4, 0),
        "healthy-b": (f"{here}/kas-turn-healthy-b-2.16.2.jsonl", 4, 0),
        "stall": (f"{here}/kas-turn-stall-run-2.16.2.jsonl", 4, 1),
    }

ok = True
tight_total = 0
for label, (path, want_turns, want_stalls) in paths.items():
    ts = turns(path)
    if len(ts) != want_turns:
        print(f"{label}: FAIL — parsed {len(ts)} turns, expected {want_turns} (truncated capture?)")
        ok = False
        continue
    uncompleted = sum(1 for _, c, _ in ts if not c)
    got = sum(stalls(t, T) for t in ts)
    print(f"{label}: turns={len(ts)} uncompleted={uncompleted} stalls@{T:.0f}s={got} (expect {want_stalls})")
    if got != want_stalls:
        ok = False
    if label.startswith("healthy"):
        tight_total += sum(stalls(t, TIGHT) for t in ts)

print(f"tight-arm: healthy stalls@{TIGHT:.0f}s={tight_total} (expect >=1 — observability guard)")
if tight_total < 1:
    ok = False
print("C11/C1 FALSIFIER:", "PASSED" if ok else "FAILED")
sys.exit(0 if ok else 1)
