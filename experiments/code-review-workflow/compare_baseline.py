#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Line up a workflow run's findings against a baseline review's summary table.

    compare_baseline.py <baseline.md> <rundir> [--window 15]

The baseline is a markdown review whose summary table rows look like
`| 3 | `crates/x/src/app.rs:1663` | CONFIRMED | issue text |`. A workflow
candidate "location-matches" a baseline row when it names the same file and a
line within --window. Location matches are LEADS, not verdicts: two different
defects can sit ten lines apart, so read the pair before calling it a hit.
Refuted candidates are included, because "found, then wrongly refuted" and
"never found" are different failures.
"""
import argparse
import json
import os
import re
import sys

for _stream in (sys.stdout, sys.stderr):  # Windows legacy code pages
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
ap.add_argument("baseline")
ap.add_argument("rundir")
ap.add_argument("--window", type=int, default=15)
a = ap.parse_args()

ROW = re.compile(r"^\|\s*(\d+)\s*\|(.+?)\|(.+?)\|(.+?)\|\s*$")
LOC = re.compile(r"`([^`:]+\.[A-Za-z]+):(\d+)")

baseline = []
for line in open(a.baseline, encoding="utf-8"):
    m = ROW.match(line)
    if m:
        locs = [(f, int(n)) for f, n in LOC.findall(m.group(2))]
        baseline.append({"n": int(m.group(1)), "where": m.group(2).strip(), "locs": locs,
                         "verdict": m.group(3).strip(), "issue": m.group(4).strip()})

v = json.load(open(os.path.join(a.rundir, "verified.json"), encoding="utf-8"))
cands = [dict(c, _state="kept") for c in v["kept"]] + [dict(c, _state="refuted") for c in v["refuted"]]


def line_of(c):
    return (c.get("verification") or {}).get("corrected_line") or c.get("line")


def near(c, locs):
    ln = line_of(c)
    return any(str(c.get("file", "")).endswith(f) or f.endswith(str(c.get("file", "x")))
               for f, n in locs if isinstance(ln, int) and abs(ln - n) <= a.window)


used = set()
print(f"baseline: {len(baseline)} findings   workflow: {len(v['kept'])} kept, {len(v['refuted'])} refuted\n")
print("== baseline findings and their location matches ==")
for b in baseline:
    hits = [c for c in cands if b["locs"] and near(c, b["locs"])]
    used.update(c["id"] for c in hits)
    tag = "LEAD " if hits else ("n/a  " if not b["locs"] else "MISS ")
    print(f"{tag} #{b['n']:<2d} {b['where'][:58]:58s} {b['issue'][:70]}")
    for c in hits:
        print(f"        -> {c['id']} [{c['verdict']}/{c['_state']}] {c.get('file')}:{line_of(c)}  {str(c.get('summary'))[:90]}")

print("\n== workflow findings with no baseline location match (new, or a different line for the same defect) ==")
for c in cands:
    if c["id"] not in used:
        print(f"  {c['id']} [{c['verdict']}/{c['_state']}] {c.get('file')}:{line_of(c)}  {str(c.get('summary'))[:110]}")
