#!/usr/bin/env python3
"""ctx.py <pattern(regex)> [-w N] [-b bin] [-v vers,comma] : count + context per version from masked strings.
"""
import re, sys
from pathlib import Path
R = Path(__file__).parent.parent / "tmp" / "strings"
args = sys.argv[1:]; w = 100; b = 'kiro-cli-chat'; vers = ["2.31.1","2.32.0"]; maxs = 6
pat = None
i = 0
while i < len(args):
    a = args[i]
    if a == '-w': w = int(args[i+1]); i += 2
    elif a == '-b': b = args[i+1]; i += 2
    elif a == '-v': vers = args[i+1].split(','); i += 2
    elif a == '-n': maxs = int(args[i+1]); i += 2
    else: pat = re.compile(a); i += 1
for v in vers:
    lines = (R / v / f'{b}.txt').read_text('ascii', 'replace').splitlines()
    hits = []
    for ln in lines:
        off, _, s = ln.partition('\t')
        for m in pat.finditer(s):
            hits.append((int(off) + m.start(), s[max(0, m.start()-w):m.end()+w]))
    print(f'== {v} {b}: {len(hits)} hits')
    for o, c in hits[:maxs]: print(f'  @{o}: {c!r}')
