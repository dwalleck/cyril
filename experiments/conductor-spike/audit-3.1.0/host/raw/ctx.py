#!/usr/bin/env python3
"""ctx.py <regex> [-w N] [-r role] [-v vers,comma] [-n max] : count + context per version from masked strings (3.1.0 audit)."""
import re, sys
from pathlib import Path
R = Path(__file__).parent.parent / 'tmp' / 'strings'
args = sys.argv[1:]; w = 100; role = 'host'; vers = ['2.32.0', '3.1.0']; maxs = 6; pat = None
i = 0
while i < len(args):
    a = args[i]
    if a == '-w': w = int(args[i+1]); i += 2
    elif a == '-r': role = args[i+1]; i += 2
    elif a == '-v': vers = args[i+1].split(','); i += 2
    elif a == '-n': maxs = int(args[i+1]); i += 2
    else: pat = re.compile(a); i += 1
def files(v):
    d = R / v; chat = d / 'kiro-cli-chat.txt'
    if role == 'host': return [chat] if chat.exists() else [d / 'kiro-cli.txt']
    if role == 'launcher': return [d / 'kiro-cli.txt'] if chat.exists() else []
    if role == 'term': return [d / 'kiro-cli-term.txt'] if chat.exists() else []
    if role == 'all': return sorted(d.glob('*.txt'))
for v in vers:
    hits = []
    for f in files(v):
        for ln in f.read_text('ascii', 'replace').splitlines():
            off, _, s = ln.partition('\t')
            for m in pat.finditer(s):
                hits.append((f.stem, int(off) + m.start(), s[max(0, m.start()-w):m.end()+w]))
    print(f'== {v} [{role}]: {len(hits)} hits')
    for b, o, c in hits[:maxs]: print(f'  {b}@{o}: {c!r}')
