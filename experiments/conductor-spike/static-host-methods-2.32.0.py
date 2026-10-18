#!/usr/bin/env python3
"""ACP extension-method vocabulary diff of the Rust host (2.32.0 audit, new).
usage: python3 -I static-host-methods-2.32.0.py <strings-root> <bin> <verA> <verB>

Extracts every `_kiro.dev/...`, `_kiro/...`, `kiro.dev/...` and bare `kiro/...` path token from the
masked strings of both builds (trap 3: set diff, not a recount), then trims each token to its
longest prefix that ends on a path-segment boundary present in BOTH-or-EITHER build, and reports
every trimmed token whose substring count differs. Glue (an adjacent literal fused onto the last
segment) shows up as a pair of tokens with mirrored counts; read them by hand.
"""
import re, sys
from pathlib import Path
root, b, va, vb = sys.argv[1:5]
TOK = re.compile(r'(?<![A-Za-z0-9])(?:_kiro\.dev|kiro\.dev|_kiro|kiro)/[A-Za-z_]+(?:/[A-Za-z_]+)*')
def text(v):
    return '\n'.join(l.partition('\t')[2] for l in (Path(root) / v / f'{b}.txt').read_text('ascii', 'replace').splitlines())
ta, tb = text(va), text(vb)
toks = set(TOK.findall(ta)) | set(TOK.findall(tb))
# also every segment-boundary prefix of a token (a/b/c -> a/b), so "method + glue" collapses
cands = set()
for t in toks:
    parts = t.split('/')
    for i in range(2, len(parts) + 1): cands.add('/'.join(parts[:i]))
rows = []
for t in sorted(cands):
    ca, cb = ta.count(t), tb.count(t)
    if ca != cb: rows.append((t, ca, cb))
print(f'{va}->{vb} {b}: {len(cands)} candidate tokens, {len(rows)} with differing counts')
for t, ca, cb in rows:
    tag = 'ADD' if ca == 0 else 'REM' if cb == 0 else 'CNT'
    print(f'  {tag} [{ca}, {cb}] {t}')
