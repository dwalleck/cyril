#!/usr/bin/env python3
"""Regex vocabulary SET diff across every carved KAS bundle of the 3.1.0 audit (trap 3).

    python3 -I static-kas-core-sets-3.1.0.py [-g GROUP] [--carves a,b,c] REGEX...

For each REGEX, extracts the set of matches (group GROUP, default 1 when the pattern has a group,
else the whole match) per carve and prints counts plus the +/- delta between consecutive carves.
Executes nothing from the bundle.
"""
import argparse, os, re
from pathlib import Path
K = Path(os.path.expanduser('~/.local/share/kiro-research/kas-carves'))
REL = 'node_modules/@kiro/agent/dist/server/acp-server.js'
CARVES = ['2.32.0', '2.33.0', '2.33.1', '2.34.0', '2.35.0', '3.1.0']
KAS = {'2.32.0': '0.66.43', '2.33.0': '0.66.48', '2.33.1': '0.66.58', '2.34.0': '0.66.62', '2.35.0': '0.66.70', '3.1.0': '0.66.91'}
ap = argparse.ArgumentParser()
ap.add_argument('-g', type=int, default=None); ap.add_argument('--carves', default=','.join(CARVES))
ap.add_argument('--full', action='store_true', help='print the full NEW set too')
ap.add_argument('regexes', nargs='+')
a = ap.parse_args()
carves = a.carves.split(',')
T = {v: (K / v / REL).read_text(encoding='utf-8', errors='replace') for v in carves}
for rx in a.regexes:
    p = re.compile(rx)
    g = a.g if a.g is not None else (1 if p.groups else 0)
    S = {v: set(m.group(g) for m in p.finditer(T[v])) for v in carves}
    print(f'### /{rx}/ ' + ' '.join(f'{KAS.get(v, v)}={len(S[v])}' for v in carves))
    for x, y in zip(carves, carves[1:]):
        add, rem = sorted(S[y] - S[x]), sorted(S[x] - S[y])
        if add or rem:
            print(f'  {KAS[x]} -> {KAS[y]}: +{add} -{rem}')
    if a.full:
        print('  NEW set:', sorted(S[carves[-1]]))
