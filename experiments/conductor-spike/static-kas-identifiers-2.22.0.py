#!/usr/bin/env python3
"""Regex-based quoted-literal diff across N KAS bundles (replaces the linear
scanner from 2.21.2, which derailed on a regex literal and inflated 'added').
Double- and single-quoted literals are matched independently with a bounded
non-quote body so one stray quote cannot desynchronise the scan."""
import json, re, sys
from pathlib import Path
paths = [Path(p) for p in sys.argv[1:]]
out = Path('experiments/conductor-spike/static-2.22.0/static-kas-identifiers-delta.json')
valid = re.compile(r'[A-Za-z_][A-Za-z0-9_.:/@-]{2,120}$')
interesting = re.compile(r'(?i)(?:stall|reason|unified|agent|memory|skill|compact|permission|retry|workflow|hook|steer|model|tool|policy|output|turn|session|context|artifact|capture|preview|snapshot|large|truncat|offload|mcp|plan|proxy|cloud|config|fallback)')
dq = re.compile(r'"([^"\\\n]{3,120})"'); sq = re.compile(r"'([^'\\\n]{3,120})'")
def get(p):
    t = p.read_text(encoding='utf-8', errors='replace')
    vals = {m.group(1) for m in dq.finditer(t)} | {m.group(1) for m in sq.finditer(t)}
    return {v for v in vals if valid.fullmatch(v)}
sets = [get(p) for p in paths]
recs = []
for (pa, A), (pb, B) in zip(zip(paths, sets), zip(paths[1:], sets[1:])):
    d = {'old': str(pa), 'new': str(pb), 'old_count': len(A), 'new_count': len(B),
         'added': sorted(B-A), 'removed': sorted(A-B),
         'relevant_added': sorted(x for x in B-A if interesting.search(x)),
         'relevant_removed': sorted(x for x in A-B if interesting.search(x))}
    recs.append(d)
    print(f"=== {pa.parts[-6][:12]} -> {pb.parts[-6][:12]}: {len(A)} -> {len(B)} (+{len(B-A)} -{len(A-B)})")
    print("  relevant_added:"); [print("    +", x) for x in d['relevant_added']]
    print("  relevant_removed:"); [print("    -", x) for x in d['relevant_removed']]
out.write_text(json.dumps(recs, indent=2, sort_keys=True))
