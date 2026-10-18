#!/usr/bin/env python3
"""sentdiff.py <regex> [bin] : sentence-ish fragments matching regex, present in one version only (by 40-char prefix count == 0 in the other)."""
import re, sys
from pathlib import Path
R = Path(__file__).parent.parent / 'tmp' / 'strings'
rx = re.compile(sys.argv[1], re.I); b = sys.argv[2] if len(sys.argv) > 2 else 'kiro-cli-chat'
def t(v): return '\n'.join(l.partition('\t')[2] for l in (R / v / f'{b}.txt').read_text('ascii','replace').splitlines())
a, c = t('2.31.1'), t('2.32.0')
SENT = re.compile(r"[A-Z\[`(<][A-Za-z0-9 ,;:'’\-_./()`<>{}\[\]=|?!\"]{15,400}")
def frags(x): return set(m.group(0) for m in SENT.finditer(x) if rx.search(m.group(0)))
fa, fc = frags(a), frags(c)
def core(s):
    w = re.findall(r"[a-z]{3,}(?: [a-z]{2,}){3,}", s)   # longest prose run
    return max(w, key=len) if w else s
for s in sorted(fc - fa):
    k = core(s)
    if a.count(k) == 0: print(f'+ [{a.count(k)},{c.count(k)}] {s[:300]!r}')
for s in sorted(fa - fc):
    k = core(s)
    if c.count(k) == 0: print(f'- [{a.count(k)},{c.count(k)}] {s[:300]!r}')
