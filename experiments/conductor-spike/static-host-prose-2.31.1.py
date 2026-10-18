#!/usr/bin/env python3
"""Glue-proof NOVEL-PROSE diff of Rust host builds (2.31.1 audit, new).

    python3 -I static-host-prose-2.31.1.py <strings-root> <bin> <verA> <verB> [minlen] [minnov]

Input: masked-strings files written by audit-2.31.1/host/raw/mkstrings.py
(<strings-root>/<ver>/<bin>.txt, one "offset\\tprintable-run" per line, zstd payload
frames already zeroed, so KAS/TUI/Node/Bun text never counts).

Why: LTO re-glues adjacent .rodata literals, so whole-run set diffs report
hundreds of false adds per patch release. Glue only touches the FIRST and LAST
word of a run, so this compares INNER word-trigrams only: a run in B is reported
when it has >=MINNOV inner PROSE trigrams (three short dictionary-shaped words,
source paths and digits stripped first) absent from every run of A. Prints the run
(trimmed) and its offset. Reverse direction (removed prose) is printed too.
"""
import re, sys
from pathlib import Path
root, binname, va, vb = sys.argv[1:5]
minlen = int(sys.argv[5]) if len(sys.argv) > 5 else 24
MINNOV = int(sys.argv[6]) if len(sys.argv) > 6 else 2
WORD = re.compile(r"\S+")
def load(v):
    runs = []
    for ln in (Path(root) / v / f'{binname}.txt').read_text('ascii', 'replace').splitlines():
        off, _, s = ln.partition('\t')
        if len(s) >= minlen and s.count(' ') >= 3: runs.append((int(off), s))
    return runs
SRC = re.compile(r'(?:event )?crates/[A-Za-z0-9_./-]+?\.rs(?::\d+)?')
PROSE = re.compile(r"^[A-Za-z][a-z'’,.:;()-]{0,13}$")
def inner_tris(s):
    s = SRC.sub(' ', s); s = re.sub(r'\d+', '', s)
    w = WORD.findall(s)
    if len(w) < 5: return set()
    w = w[1:-1]
    # prose-like trigrams only: three short dictionary-shaped words (glue makes long words)
    return {tuple(w[i:i + 3]) for i in range(len(w) - 2) if all(PROSE.match(x) for x in w[i:i + 3])}
A, B = load(va), load(vb)
TA = set().union(*(inner_tris(s) for _, s in A)); TB = set().union(*(inner_tris(s) for _, s in B))
def report(runs, other, label):
    n = 0
    for off, s in runs:
        nov = inner_tris(s) - other
        if not nov: continue
        n += 1
        # locate the novel span to quote around it
        if len(nov) < MINNOV: continue
        print(f'{label} @{off} novel_tris={len(nov)}: ' + ' | '.join(' '.join(t) for t in sorted(nov)[:12]))
    print(f'## {label} total {n}')
report(B, TA, f'+{vb}')
report(A, TB, f'-{va}')
