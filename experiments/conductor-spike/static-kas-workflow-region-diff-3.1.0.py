#!/usr/bin/env python3
"""Token-level diff of one anchored code region between two KAS carves.

    python3 -I static-kas-workflow-region-diff-3.1.0.py OLDVER NEWVER ANCHOR [LEN] [--before N]
        [--new-anchor A2] [--ctx N] [--occ K]

Generalises static-kas-region-diff-2.32.0.py to any carve pair: finds ANCHOR (K-th
occurrence, default first) in each bundle, takes N chars before and LEN chars after,
tokenises, collapses short minified identifiers (<=4 chars, not Capitalised) to `$`,
and prints only the changed token runs with context. Executes nothing from Kiro.
"""
import argparse, difflib, os, re
from pathlib import Path
K = Path(os.path.expanduser('~/.local/share/kiro-research/kas-carves'))
REL = 'node_modules/@kiro/agent/dist/server/acp-server.js'
ap = argparse.ArgumentParser(); ap.add_argument('old'); ap.add_argument('new'); ap.add_argument('anchor')
ap.add_argument('len', nargs='?', type=int, default=6000); ap.add_argument('--before', type=int, default=0)
ap.add_argument('--new-anchor'); ap.add_argument('--ctx', type=int, default=12); ap.add_argument('--occ', type=int, default=1)
a = ap.parse_args()
O = (K / a.old / REL).read_text(encoding='utf-8', errors='replace'); N = (K / a.new / REL).read_text(encoding='utf-8', errors='replace')
def nth(s, needle, k):
    i = -1
    for _ in range(k):
        i = s.find(needle, i + 1)
        if i < 0: return -1
    return i
i = nth(O, a.anchor, a.occ); j = nth(N, a.new_anchor or a.anchor, a.occ)
print(f'anchor OLD@{i} NEW@{j}')
if i < 0 or j < 0: raise SystemExit(1)
tok = re.compile(r'"(?:[^"\\]|\\.)*"|\'(?:[^\'\\]|\\.)*\'|`(?:[^`\\]|\\.)*`|[A-Za-z0-9_$]+|\s+|.')
ot = tok.findall(O[max(0, i - a.before):i + a.len]); nt = tok.findall(N[max(0, j - a.before):j + a.len])
key = lambda t: '$' if re.fullmatch(r'[a-z_$][A-Za-z0-9_$]{0,3}|[A-Z][a-z]?[0-9$]?[a-z]?', t) else t
sm = difflib.SequenceMatcher(None, [key(t) for t in ot], [key(t) for t in nt], autojunk=False)
n = 0
for tag, i1, i2, j1, j2 in sm.get_opcodes():
    if tag == 'equal': continue
    n += 1
    print(f'[{tag}] …{"".join(ot[max(0, i1 - a.ctx):i1])[-200:]}  ‹-{"".join(ot[i1:i2])[:2000]}›  ‹+{"".join(nt[j1:j2])[:3000]}›  {"".join(ot[i2:i2 + a.ctx])[:200]}…')
print(f'{n} changed runs; old tokens {len(ot)} new tokens {len(nt)}')
