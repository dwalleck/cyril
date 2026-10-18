#!/usr/bin/env python3
"""Substring-count identifier diff across host builds (2.32.0 audit, copied from 2.31.1, new).

    python3 -I static-host-idents-2.31.1.py <strings-root> <bin> <ver1> <ver2> [...]

Candidates: camelCase (serde rename_all=camelCase fields) and snake_case
identifiers (>=2 segments) extracted from every version's masked strings
(audit-2.31.1/host/raw/mkstrings.py output). Each candidate is then COUNTED AS A
SUBSTRING of every version's full text, so a token is reported only when its
count goes 0 -> n or n -> 0 between consecutive versions (trap 2: count in both).
Glue can still forge a camelCase token from two adjacent literals; every reported
row must be confirmed with ctx before it is a finding.
"""
import re, sys
from pathlib import Path
root, binname, vers = sys.argv[1], sys.argv[2], sys.argv[3:]
T = {v: (Path(root) / v / f'{binname}.txt').read_text('ascii', 'replace') for v in vers}
T = {v: '\n'.join(l.partition('\t')[2] for l in t.splitlines()) for v, t in T.items()}
PATS = [re.compile(r'(?<![A-Za-z0-9_])[a-z]{2,}(?:[A-Z][a-z0-9]+){1,4}(?![a-z])'),
        re.compile(r'(?<![A-Za-z0-9_])[a-z][a-z0-9]+(?:_[a-z0-9]+){1,5}(?![A-Za-z0-9])')]
cands = set()
for t in T.values():
    for p in PATS: cands |= set(p.findall(t))
cands = {c for c in cands if 6 <= len(c) <= 48}
for a, b in zip(vers, vers[1:]):
    add = sorted(c for c in cands if T[a].count(c) == 0 and T[b].count(c) > 0)
    rem = sorted(c for c in cands if T[a].count(c) > 0 and T[b].count(c) == 0)
    print(f'### {binname} {a} -> {b}: +{len(add)} -{len(rem)}')
    for c in add: print(f'  + {c} [{", ".join(str(T[v].count(c)) for v in vers)}]')
    for c in rem: print(f'  - {c} [{", ".join(str(T[v].count(c)) for v in vers)}]')
