#!/usr/bin/env python3
"""cnt.py [-r role] [-v v1,v2,...] lit1 lit2 ... : substring counts per version in masked strings (3.1.0 audit).
role: host (default; kiro-cli-chat on 2.x, kiro-cli on 3.x), launcher (2.x kiro-cli), term (2.x kiro-cli-term), all (sum of the three on 2.x = the single binary on 3.x).
Default versions: 2.32.0,3.1.0. Prints [c_v1, c_v2, ...] and ADD/REM relative to first→last."""
import sys
from pathlib import Path
R = Path(__file__).parent.parent / 'tmp' / 'strings'
args = sys.argv[1:]; role = 'host'; vers = ['2.32.0', '3.1.0']
while args and args[0] in ('-r', '-v'):
    if args[0] == '-r': role = args[1]
    else: vers = args[1].split(',')
    args = args[2:]
def files(v):
    d = R / v
    chat = d / 'kiro-cli-chat.txt'
    if role == 'host': return [chat] if chat.exists() else [d / 'kiro-cli.txt']
    if role == 'launcher': return [d / 'kiro-cli.txt'] if chat.exists() else []
    if role == 'term': return [d / 'kiro-cli-term.txt'] if chat.exists() else []
    if role == 'all': return sorted(d.glob('*.txt'))
    raise SystemExit('bad role')
data = {}
for v in vers:
    data[v] = '\n'.join('\n'.join(l.partition('\t')[2] for l in f.read_text('ascii', 'replace').splitlines()) for f in files(v))
for a in args:
    c = [data[v].count(a) for v in vers]
    tag = 'ADD' if c[0] == 0 and c[-1] else 'REM' if c[-1] == 0 and c[0] else 'CNT' if c[0] != c[-1] else '   '
    print(f'{c} {tag} {a!r}')
