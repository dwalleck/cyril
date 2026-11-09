#!/usr/bin/env python3
"""Count a regex in several KAS carves and print surrounding context (traps 2, 30).

    python3 -I static-kas-workflow-ctx-3.1.0.py REGEX [-v VER ...] [-w WIDTH] [-n MAX] [--only VER]

Default versions: 2.32.0 2.33.0 2.33.1 2.34.0 2.35.0 3.1.0 (the six distinct KAS
bundles 0.66.43/48/58/62/70/91). Prints the count per version, then up to MAX
occurrences with WIDTH chars either side for the versions named by --only
(default: first and last). Executes nothing from Kiro.
"""
import argparse, os, re
from pathlib import Path
K = Path(os.path.expanduser('~/.local/share/kiro-research/kas-carves'))
REL = 'node_modules/@kiro/agent/dist/server/acp-server.js'
ap = argparse.ArgumentParser()
ap.add_argument('regex'); ap.add_argument('-v', nargs='*', default=['2.32.0', '2.33.0', '2.33.1', '2.34.0', '2.35.0', '3.1.0'])
ap.add_argument('-w', type=int, default=200); ap.add_argument('-n', type=int, default=6); ap.add_argument('--only', nargs='*')
a = ap.parse_args()
pat = re.compile(a.regex)
texts = {v: (K / v / REL).read_text(encoding='utf-8', errors='replace') for v in a.v}
counts = {v: len(pat.findall(t)) for v, t in texts.items()}
print('COUNTS', ' '.join(f'{v}={c}' for v, c in counts.items()))
show = a.only or [a.v[0], a.v[-1]]
for v in show:
    t = texts[v]
    for i, m in enumerate(pat.finditer(t)):
        if i >= a.n: print(f'  [{v}] … {counts[v]-a.n} more'); break
        s = max(0, m.start() - a.w); e = min(len(t), m.end() + a.w)
        print(f'  [{v}@{m.start()}] {t[s:e]!r}')
