#!/usr/bin/env python3
"""Rename-normalised statement dump of a minified tui.js bundle, for `diff`.

Splits the bundle at ; { } (outside nothing clever: string-agnostic, good enough for
diffing), replaces every identifier of <=4 chars (bun's mangled names, incl. $/digit
forms) with `_`, and writes one statement per line. Run on OLD and NEW and `diff` the
outputs: minifier renames vanish, logic and literal changes remain.

Usage: python3 -I static-tui-stmtdiff-2.31.1.py BUNDLE OUT.txt
Read-only on BUNDLE; nothing is executed.
"""
import re, sys
from pathlib import Path
SHORT = re.compile(r'(?<![\w$.])[A-Za-z_$][\w$]{0,3}(?![\w$])')
t = Path(sys.argv[1]).read_text(errors='replace')
t = SHORT.sub('_', t)
parts = re.split(r'(?<=[;{}])', t)
Path(sys.argv[2]).write_text("\n".join(p.replace("\n", "\\n") for p in parts if p.strip()) + "\n")
print(len(parts))
