#!/usr/bin/env python3
"""Count a literal in OLD and NEW KAS bundles and print context (trap 2: count in both versions).

    python3 -I static-kas-ctx-2.32.0.py [-b BEFORE] [-a AFTER] [-n MAXHITS] [-o|-N] NEEDLE...

Bundles default to the 2.31.1 / 2.32.0 carves. -o prints OLD contexts only, -N NEW only
(default: NEW contexts, counts for both). Executes nothing from the bundle.
"""
import argparse, os
from pathlib import Path
K = Path(os.path.expanduser('~/.local/share/kiro-research/kas-carves'))
REL = 'node_modules/@kiro/agent/dist/server/acp-server.js'
ap = argparse.ArgumentParser()
ap.add_argument('-b', type=int, default=150); ap.add_argument('-a', type=int, default=250)
ap.add_argument('-n', type=int, default=3); ap.add_argument('-o', action='store_true')
ap.add_argument('--old', default=str(K / '2.31.1' / REL)); ap.add_argument('--new', default=str(K / '2.32.0' / REL))
ap.add_argument('needles', nargs='+')
a = ap.parse_args()
O = Path(a.old).read_text(encoding='utf-8', errors='replace'); N = Path(a.new).read_text(encoding='utf-8', errors='replace')
for nd in a.needles:
    print(f'### {nd!r}: OLD {O.count(nd)} -> NEW {N.count(nd)}')
    t = O if a.o else N
    i = 0; k = 0
    while k < a.n:
        j = t.find(nd, i)
        if j < 0: break
        print(f'  @{j}: ...{t[max(0, j - a.b):j + len(nd) + a.a]!r}...')
        i = j + 1; k += 1
