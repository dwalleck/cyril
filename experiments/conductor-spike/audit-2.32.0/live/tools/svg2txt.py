#!/usr/bin/env python3
"""hex1b SVG screenshot -> plain-text screen (10x20 px cells). Usage: svg2txt.py <in.svg> [<out.txt>]"""
import re, sys, html
src = open(sys.argv[1], encoding="utf-8").read()
cells = {}
for m in re.finditer(r'<text x="([\d.]+)" y="([\d.]+)"[^>]*>([^<]*)</text>', src):
    x, y, t = float(m.group(1)), float(m.group(2)), html.unescape(m.group(3))
    col, row = int(round(x / 10)), int(round((y - 15) / 20))
    for i, ch in enumerate(t):
        cells[(row, col + i)] = ch
rows = max((r for r, _ in cells), default=-1) + 1
lines = []
for r in range(rows):
    cols = [c for rr, c in cells if rr == r]
    w = max(cols) + 1 if cols else 0
    lines.append("".join(cells.get((r, c), " ") for c in range(w)).rstrip())
out = "\n".join(lines).rstrip() + "\n"
if len(sys.argv) > 2: open(sys.argv[2], "w", encoding="utf-8").write(out)
else: sys.stdout.write(out)
