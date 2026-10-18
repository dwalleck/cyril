#!/usr/bin/env python3
"""Run-by-run parity of two SCENARIO=workflow captures (driver result record)."""
import json, sys
def res(f):
    r = None
    for l in open(f):
        o = json.loads(l)
        if o.get("probe") == "result": r = o
    return r
a, b = res(sys.argv[1]), res(sys.argv[2])
print("OLD", sys.argv[1], "elapsed", a.get("elapsed_s"), "| NEW", sys.argv[2], "elapsed", b.get("elapsed_s"))
print("workflowsEnabled", a.get("workflowsEnabled"), b.get("workflowsEnabled"))
for k in sorted(set(a["runs"]) | set(b["runs"])):
    ra, rb = a["runs"].get(k) or {}, b["runs"].get(k) or {}
    ea, eb = ra.get("events"), rb.get("events")
    ta, tb = ra.get("terminal"), rb.get("terminal")
    print(f"{k:10} {'MATCH' if ea == eb else 'DIFF '} terminal {ta}/{tb} n={len(ea or [])}/{len(eb or [])}")
    if ea != eb:
        print("   OLD", ea); print("   NEW", eb)
    for kk in sorted(set(ra) | set(rb)):
        if kk in ("events", "workflowId", "steps", "permissions"): continue
        if json.dumps(ra.get(kk), sort_keys=True) != json.dumps(rb.get(kk), sort_keys=True):
            print(f"   {kk}: OLD {json.dumps(ra.get(kk))[:200]} | NEW {json.dumps(rb.get(kk))[:200]}")
da, db = a.get("discovery") or {}, b.get("discovery") or {}
import re
norm = lambda s: re.sub(r'(sess|wf)_[0-9a-f-]+|kas-wf\w+-ws-\w+|\d{4}-\d\d-\d\dT[\d:.]+Z|"durationMs": \d+', 'X', json.dumps(s))
for k in sorted(set(da) | set(db)):
    if norm(da.get(k)) != norm(db.get(k)):
        print("DISC DIFF", k, "\n   OLD", norm(da.get(k))[:400], "\n   NEW", norm(db.get(k))[:400])
print("note_methods OLD", a.get("note_methods")); print("note_methods NEW", b.get("note_methods"))
print("perms", len(a.get("permissions") or []), len(b.get("permissions") or []))
