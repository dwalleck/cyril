#!/usr/bin/env python3
"""Anchored-region extractor + rename-normalised differ for minified tui.js bundles.

Usage:
  python3 -I static-tui-region-2.32.0.py ANCHOR BEFORE AFTER BUNDLE [BUNDLE ...]
      prints, per bundle, the count of ANCHOR and the region around its Nth hit
      (env NTH=k selects the k-th hit, default 0; env ALL=1 prints every hit).
  python3 -I static-tui-region-2.32.0.py --diff ANCHOR BEFORE AFTER OLD NEW
      splits both regions at ; { } and diffs them after replacing every
      identifier of <=4 chars (and $-containing ones) with `_`, so minifier
      renames vanish and only logic/literal changes remain.
Read-only; nothing is executed.
"""
import difflib, os, re, sys
from pathlib import Path

SHORT = re.compile(r'(?<![\w$."\'])[A-Za-z_$][\w$]{0,3}(?![\w$])')

def regions(t, a, before, after):
    out, s = [], 0
    while True:
        i = t.find(a, s)
        if i < 0:
            return out
        out.append(t[max(0, i - before): i + len(a) + after])
        s = i + len(a)

def norm(s):
    return SHORT.sub('_', s)

def split(s):
    return [x for x in re.split(r'(?<=[;{}])', s) if x.strip()]

def main():
    args = sys.argv[1:]
    if args and args[0] == '--diff':
        _, a, b, af, old, new = args
        ro = regions(Path(old).read_text(errors='replace'), a, int(b), int(af))
        rn = regions(Path(new).read_text(errors='replace'), a, int(b), int(af))
        print(f"anchor count: old={len(ro)} new={len(rn)}")
        k = int(os.environ.get('NTH', '0'))
        if not ro or not rn:
            return 1
        lo, ln = split(norm(ro[min(k, len(ro)-1)])), split(norm(rn[min(k, len(rn)-1)]))
        for line in difflib.unified_diff(lo, ln, 'old', 'new', n=1, lineterm=''):
            print(line[:400])
        return 0
    a, b, af, *bundles = args
    for p in bundles:
        t = Path(p).read_text(errors='replace')
        rs = regions(t, a, int(b), int(af))
        print(f"######## {Path(p).name}: count({a!r})={len(rs)}")
        if not rs:
            continue
        sel = rs if os.environ.get('ALL') else [rs[min(int(os.environ.get('NTH', '0')), len(rs)-1)]]
        for r in sel:
            print(r.replace('\n', ' '))
            print('----')
    return 0

raise SystemExit(main())
