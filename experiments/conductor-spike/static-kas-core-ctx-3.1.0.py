#!/usr/bin/env python3
"""Count a literal in EVERY carved KAS bundle of the 3.1.0 audit and print context (traps 2, 3).

    python3 -I static-kas-core-ctx-3.1.0.py [-b BEFORE] [-a AFTER] [-n MAXHITS] [-c CARVE] [-r] NEEDLE...

Default carves: 2.32.0 (0.66.43) 2.33.0 (0.66.48) 2.33.1 (0.66.58) 2.34.0 (0.66.62) 2.35.0 (0.66.70)
3.1.0 (0.66.91). The count row across all six bisects a change to the first release where it appears.
Context is printed from CARVE (default 3.1.0); -r treats NEEDLE as a regex. Executes nothing from the bundle.
"""
import argparse, os, re
from pathlib import Path
K = Path(os.path.expanduser('~/.local/share/kiro-research/kas-carves'))
REL = 'node_modules/@kiro/agent/dist/server/acp-server.js'
CARVES = ['2.32.0', '2.33.0', '2.33.1', '2.34.0', '2.35.0', '3.1.0']
KAS = {'2.32.0': '0.66.43', '2.33.0': '0.66.48', '2.33.1': '0.66.58', '2.34.0': '0.66.62', '2.35.0': '0.66.70', '3.1.0': '0.66.91'}
ap = argparse.ArgumentParser()
ap.add_argument('-b', type=int, default=150); ap.add_argument('-a', type=int, default=250)
ap.add_argument('-n', type=int, default=3); ap.add_argument('-c', default='3.1.0'); ap.add_argument('-r', action='store_true')
ap.add_argument('--carves', default=','.join(CARVES))
ap.add_argument('needles', nargs='+')
a = ap.parse_args()
carves = a.carves.split(',')
T = {v: (K / v / REL).read_text(encoding='utf-8', errors='replace') for v in carves}
for nd in a.needles:
    pat = re.compile(nd) if a.r else re.compile(re.escape(nd))
    counts = ' '.join(f'{KAS.get(v, v)}={len(pat.findall(T[v]))}' for v in carves)
    print(f'### {nd!r}: {counts}')
    t = T[a.c]; k = 0
    for m in pat.finditer(t):
        if k >= a.n: break
        j, e = m.start(), m.end()
        print(f'  @{j}: ...{t[max(0, j - a.b):e + a.a]!r}...')
        k += 1
