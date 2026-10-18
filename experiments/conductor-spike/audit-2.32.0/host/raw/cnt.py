#!/usr/bin/env python3
"""cnt.py [-b bin] lit1 lit2 ... : substring counts [2.31.1, 2.32.0] in masked strings (whole-file, cross-line safe within a string)."""
import sys
from pathlib import Path
R = Path(__file__).parent.parent / 'tmp' / 'strings'
args = sys.argv[1:]; b = 'kiro-cli-chat'
if args[:1] == ['-b']: b = args[1]; args = args[2:]
data = {}
for v in ['2.31.1', '2.32.0']:
    data[v] = '\n'.join(l.partition('\t')[2] for l in (R / v / f'{b}.txt').read_text('ascii', 'replace').splitlines())
for a in args:
    c = [data[v].count(a) for v in data]
    print(f'{c} {"ADD" if c[0]==0 and c[1] else "REM" if c[1]==0 and c[0] else "   "} {a!r}')
