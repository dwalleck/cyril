#!/usr/bin/env python3
"""cyril-jxfu probe: replay kas-custom-dag-2.16.0.jsonl through cyril's CURRENT
routing rule and report (a) where every session/update frame lands, (b) the
node_start claim timeline, (c) whether any step frame precedes its claim.

The routing simulation mirrors classify_notification_route (app.rs:1184):
  unscoped -> Main; scoped==main -> Main; scoped!=main & main known -> Subagent;
  scoped, no main, tracked -> Subagent; else Drop.
KAS emits no subagent list_update, so tracked=False throughout.
"""
import json

CAPTURE = "experiments/conductor-spike/kas-custom-dag-2.16.0.jsonl"

main_sid = None          # learned from the session/new response
routes = {}              # sid -> [(line_no, route)]
claims = []              # (line_no, node_path, sid_or_None)  from node_start
first_update = {}        # sid -> first session/update line
first_claim = {}         # sid -> first sessionId-bearing node_start line

def classify(scope, main, tracked=False):
    if scope is None: return "Main"
    if main is not None and scope == main: return "Main"
    if main is not None: return "Subagent"
    return "Subagent" if tracked else "Drop"

for n, line in enumerate(open(CAPTURE), 1):
    m = json.loads(line)
    meth = m.get("method")
    if meth is None and "result" in m:                 # response to a client request
        sid = m["result"].get("sessionId") if isinstance(m["result"], dict) else None
        if sid and main_sid is None:
            main_sid = sid
            print(f"line {n:3d}: session/new response -> MAIN = {sid}")
        continue
    if meth == "_kiro/workflow/node_start":
        p = m["params"]
        sid = p.get("sessionId")
        claims.append((n, p.get("nodePath"), sid))
        if sid is not None:
            first_claim.setdefault(sid, n)
        continue
    if meth == "session/update":
        sid = m["params"].get("sessionId")
        first_update.setdefault(sid, n)
        routes.setdefault(sid, []).append((n, classify(sid, main_sid)))

print("\n-- node_start claim timeline --")
for n, path, sid in claims:
    print(f"line {n:3d}: nodePath={path!r:28} sessionId={sid}")

print("\n-- session/update routing under CURRENT classifier --")
for sid, entries in routes.items():
    dests = {r for _, r in entries}
    tag = "MAIN" if sid == main_sid else "step"
    print(f"{tag} {sid}: {len(entries):2d} frames -> {sorted(dests)} "
          f"(lines {entries[0][0]}..{entries[-1][0]})")

print("\n-- late-claim check (does a step frame beat its claim?) --")
verdicts = []
for sid, fu in sorted(first_update.items()):
    if sid == main_sid: continue
    fc = first_claim.get(sid)
    late = fc is None or fu < fc
    verdicts.append(late)
    print(f"step {sid}: first update line {fu}, first claim line {fc} "
          f"-> {'FRAME BEATS CLAIM (late claim REQUIRED)' if late else 'claim first'}")

misfiled = sum(len(v) for s, v in routes.items() if s != main_sid)
print(f"\nSUMMARY: main={main_sid} step_sessions={len(routes)-1} "
      f"misfiled_step_frames={misfiled} late_claim_needed={any(verdicts)}")
