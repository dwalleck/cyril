#!/usr/bin/env python3
"""Lexer-exact string-literal multiset diff of two KAS bundles (trap 2/3).

    python3 -I static-kas-literals-2.31.1.py OLD.js NEW.js OUT.txt

Uses the tokenizer from static-kas-bundle-diff-2.31.1.py (strings and template
chunks are lexed, not regex-scanned, so one stray quote cannot desynchronise the
census). Reports every literal whose occurrence count differs, with both counts.
"""
import importlib.util, sys
from collections import Counter
from pathlib import Path
here = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('bd', here / 'static-kas-bundle-diff-2.31.1.py')
bd = importlib.util.module_from_spec(spec); spec.loader.exec_module(bd)
def lits(p):
    c = Counter()
    for kind, t in bd.lex(Path(p).read_text(encoding='utf-8', errors='replace')):
        if kind == 'str': c[t[1:-1]] += 1
        elif kind == 'tpl': c['`' + t.strip('`}').removesuffix('${')] += 1
    return c
a, b = lits(sys.argv[1]), lits(sys.argv[2])
rows = []
for k in sorted(set(a) | set(b)):
    if a[k] != b[k]: rows.append(f'{a[k]:4d} -> {b[k]:4d}  {k[:400]!r}')
Path(sys.argv[3]).write_text('\n'.join(rows) + '\n')
print(len(rows), 'literals with changed counts;', sum(1 for k in b if k not in a), 'new-only;', sum(1 for k in a if k not in b), 'old-only')
