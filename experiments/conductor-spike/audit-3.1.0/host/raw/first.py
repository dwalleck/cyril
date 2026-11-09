#!/usr/bin/env python3
"""first.py [-r role] lit... : per-literal count across ALL 14 versions (host role default) -> first version where count changes."""
import sys
from pathlib import Path
R = Path(__file__).parent.parent / 'tmp' / 'strings'
VERS = ['2.32.0','2.33.0','2.33.1','2.34.0','2.34.1','2.34.2','2.35.0','2.35.1','2.35.2','2.36.0','2.37.0','2.37.1','3.0.0','3.1.0']
args = sys.argv[1:]; role = 'host'
if args[:1] == ['-r']: role = args[1]; args = args[2:]
def files(v):
    d = R / v; chat = d / 'kiro-cli-chat.txt'
    if role == 'host': return [chat] if chat.exists() else [d / 'kiro-cli.txt']
    if role == 'launcher': return [d / 'kiro-cli.txt'] if chat.exists() else []
    if role == 'term': return [d / 'kiro-cli-term.txt'] if chat.exists() else []
    if role == 'all': return sorted(d.glob('*.txt'))
data = {v: '\n'.join('\n'.join(l.partition('\t')[2] for l in f.read_text('ascii','replace').splitlines()) for f in files(v)) for v in VERS}
for a in args:
    c = [data[v].count(a) for v in VERS]
    changes = [f'{VERS[i]}:{c[i-1]}->{c[i]}' for i in range(1, len(c)) if c[i] != c[i-1]]
    print(f'{a!r}: {c}  changes: {changes if changes else "none"}')
