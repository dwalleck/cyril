#!/usr/bin/env python3
"""cyril-jxfu probe 2: does _meta.kiro.workflow ride EVERY step session/update
frame, or only some update kinds? Per-frame inventory of (sid, kind, meta?)."""
import json

CAPTURE = "experiments/conductor-spike/kas-custom-dag-2.16.0.jsonl"
MAIN = "sess_2bc0cfdc-ccba-47b7-a3ab-224b23a63d60"

per_sid = {}   # sid -> {kind: [with_meta, without_meta]}
for n, line in enumerate(open(CAPTURE), 1):
    m = json.loads(line)
    if m.get("method") != "session/update":
        continue
    p = m["params"]
    sid = p["sessionId"]
    u = p["update"]
    kind = u.get("sessionUpdate")
    wf = (u.get("_meta") or {}).get("kiro", {}).get("workflow")
    # also check the params-level _meta, in case it rides the envelope instead
    wf_env = (p.get("_meta") or {}).get("kiro", {}).get("workflow")
    slot = per_sid.setdefault(sid, {}).setdefault(kind, [0, 0, 0])
    if wf is not None:
        slot[0] += 1
    elif wf_env is not None:
        slot[2] += 1
    else:
        slot[1] += 1
        print(f"  NO-META frame: line {n} sid=...{sid[-8:]} kind={kind}")

for sid, kinds in per_sid.items():
    tag = "MAIN" if sid == MAIN else "step"
    print(f"{tag} ...{sid[-8:]}:")
    for kind, (w, wo, env) in sorted(kinds.items()):
        print(f"   {kind:22} update._meta={w:2d}  params._meta={env:2d}  none={wo:2d}")
