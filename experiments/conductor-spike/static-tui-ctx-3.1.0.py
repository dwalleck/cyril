#!/usr/bin/env python3
"""Literal context extractor for minified tui.js bundles (kiro-cli 3.1.0 audit).

ugrep rejects `.{0,N}` contexts on 14 MB single-line bundles, so this does the
count-and-quote step of trap 2 in Python: for every bundle, print the number of
raw occurrences of NEEDLE and up to -n contexts of -b/-a chars around each hit.

Usage: python3 -I static-tui-ctx-3.1.0.py [-b BEFORE] [-a AFTER] [-n MAX|all] NEEDLE BUNDLE [BUNDLE ...]
Read-only; nothing is executed.
"""
import sys
from pathlib import Path


def main():
    args = sys.argv[1:]
    before, after, nmax = 300, 400, 2
    while args and args[0].startswith('-') and len(args[0]) == 2:
        flag, val = args[0], args[1]
        args = args[2:]
        if flag == '-b':
            before = int(val)
        elif flag == '-a':
            after = int(val)
        elif flag == '-n':
            nmax = None if val == 'all' else int(val)
    needle, bundles = args[0], args[1:]
    for b in bundles:
        p = Path(b)
        t = p.read_text(errors='replace')
        hits = []
        s = 0
        while True:
            i = t.find(needle, s)
            if i < 0:
                break
            hits.append(i)
            s = i + len(needle)
        print(f"######## {p.name}: count({needle!r})={len(hits)}")
        for i in (hits if nmax is None else hits[:nmax]):
            print(t[max(0, i - before): i + len(needle) + after].replace('\n', '\\n'))
            print('----')
    return 0


raise SystemExit(main())
