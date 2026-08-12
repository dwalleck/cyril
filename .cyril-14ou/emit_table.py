#!/usr/bin/env python3
"""cyril-14ou slice 4: emit the capture-derived timing table for the replay
fence (crates/cyril-core/tests/fixtures/turn_liveness_timings.json).

Each turn = {"events": [seconds-after-prompt of each qualifying inbound frame],
"completed": bool, "horizon": seconds-after-prompt the recorder was last alive}.
The horizon is load-bearing for the stall turn: replay time must not stop at
the last frame (the falsifier_c11 horizon bug), so it is encoded explicitly.
"""
import json
import sys

def turns_from_wire(path):
    """A turn is ACTIVE from its prompt until the response bearing ITS OWN
    JSON-RPC id (the mediator's release, id-paired — order-based pairing
    misattributes turns when a late response crosses the next prompt)."""
    turns = []
    active = None  # (id, t0, events)
    file_max = 0.0
    for line in open(path):
        r = json.loads(line)
        file_max = max(file_max, r.get("ts", 0.0))
        m = r.get("msg")
        if not isinstance(m, dict):
            continue
        if r["dir"] == "client->agent" and m.get("method") == "session/prompt":
            if active is not None:  # unreleased predecessor (the stall)
                turns.append({"t0": active[1], "events": active[2], "completed": False})
            active = (m["id"], r["ts"], [])
            continue
        if r["dir"] != "agent->client" or active is None:
            continue
        if m.get("method") == "session/update":
            active[2].append(round(r["ts"] - active[1], 3))
            # KAS dual terminals: the wire turn_end frame is the FIRST
            # terminal source — the mediator releases here, not at the RPC
            # response (which arrives ~1ms later and is absorbed).
            u = m.get("params", {}).get("update", {})
            kind = u.get("_meta", {}).get("kiro", {}).get("kind")
            if u.get("sessionUpdate") == "session_info_update" and kind == "turn_end":
                turns.append({"t0": active[1], "events": active[2], "completed": True})
                active = None
        elif "result" in m and isinstance(m.get("result"), dict) and "stopReason" in m["result"]:
            if m.get("id") == active[0]:
                active[2].append(round(r["ts"] - active[1], 3))
                turns.append({"t0": active[1], "events": active[2], "completed": True})
                active = None
            # A late response for an already-superseded turn stamps nothing.
    if active is not None:
        turns.append({"t0": active[1], "events": active[2], "completed": False})
    return [
        {
            "events": t["events"],
            "completed": t["completed"],
            "horizon": round(file_max - t["t0"], 3),
        }
        for t in turns
    ]

base = sys.argv[1]
table = {
    "healthy": turns_from_wire(f"{base}/run-5/wire.jsonl")
    + turns_from_wire(f"{base}/run-6/wire.jsonl"),
    "stall": turns_from_wire(f"{base}/run-1-stall-archived/wire.jsonl"),
}
dest = sys.argv[2]
with open(dest, "w") as f:
    json.dump(table, f, indent=1)
h = sum(1 for t in table["healthy"])
s = sum(1 for t in table["stall"])
print(f"wrote {dest}: {h} healthy turns, {s} stall-capture turns "
      f"(uncompleted: {sum(1 for t in table['stall'] if not t['completed'])})")
