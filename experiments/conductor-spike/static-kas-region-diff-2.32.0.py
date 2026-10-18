#!/usr/bin/env python3
"""Token-level diff of one code region in the OLD and NEW KAS bundles.

    python3 -I static-kas-region-diff-2.32.0.py ANCHOR [LEN] [--new-anchor A2] [--ctx N]

Finds ANCHOR (first occurrence) in each bundle (A2 in NEW when given), takes LEN chars
(default 6000), tokenises, collapses short minified identifiers (<=4 chars, not Capitalised)
to `$`, and prints only the changed token runs with N tokens of context. Executes nothing.
"""
import argparse, difflib, os, re
from pathlib import Path
K = Path(os.path.expanduser('~/.local/share/kiro-research/kas-carves'))
REL = 'node_modules/@kiro/agent/dist/server/acp-server.js'
ap = argparse.ArgumentParser(); ap.add_argument('anchor'); ap.add_argument('len', nargs='?', type=int, default=6000)
ap.add_argument('--new-anchor'); ap.add_argument('--ctx', type=int, default=12)
a = ap.parse_args()
O = (K / '2.31.1' / REL).read_text(encoding='utf-8', errors='replace'); N = (K / '2.32.0' / REL).read_text(encoding='utf-8', errors='replace')
i = O.find(a.anchor); j = N.find(a.new_anchor or a.anchor)
print(f'anchor OLD@{i} NEW@{j}')
if i < 0 or j < 0: raise SystemExit(1)
tok = re.compile(r'"(?:[^"\\]|\\.)*"|\'(?:[^\'\\]|\\.)*\'|`(?:[^`\\]|\\.)*`|[A-Za-z0-9_$]+|\s+|.')
ot = tok.findall(O[i:i + a.len]); nt = tok.findall(N[j:j + a.len])
key = lambda t: '$' if re.fullmatch(r'[a-z_$][A-Za-z0-9_$]{0,3}|[A-Z][a-z]?[0-9$]?[a-z]?', t) else t
sm = difflib.SequenceMatcher(None, [key(t) for t in ot], [key(t) for t in nt], autojunk=False)
for tag, i1, i2, j1, j2 in sm.get_opcodes():
    if tag == 'equal': continue
    print(f'[{tag}] …{"".join(ot[max(0, i1 - a.ctx):i1])[-200:]}  ‹-{"".join(ot[i1:i2])[:2000]}›  ‹+{"".join(nt[j1:j2])[:3000]}›  {"".join(ot[i2:i2 + a.ctx])[:200]}…')
