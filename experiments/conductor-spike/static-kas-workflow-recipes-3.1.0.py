#!/usr/bin/env python3
"""Extract the bundled workflow recipe object literals from KAS carves for diffing.

    python3 -I static-kas-workflow-recipes-3.1.0.py OUTDIR VER [VER ...]

For each carve finds `name:"<recipe>"` for the bundled recipe names, walks back to
the enclosing `{` and forward with brace balancing (string- and template-aware), and
writes OUTDIR/<ver>/<recipe>.js. Also writes the `workflow-from-prompt` creator
recipe when present. Executes nothing from Kiro.
"""
import os, re, sys
from pathlib import Path
K = Path(os.path.expanduser('~/.local/share/kiro-research/kas-carves'))
REL = 'node_modules/@kiro/agent/dist/server/acp-server.js'
NAMES = ['autoresearch', 'feature-pipeline', 'goal', 'investigate', 'publish-pr', 'ralph',
         'semantic-review-multi-model', 'workflow-from-prompt']
def balanced(s, start):
    """start at the `{` index; return end index (exclusive) of the balanced object."""
    depth = 0; i = start; n = len(s)
    while i < n:
        c = s[i]
        if c in '"\'':
            j = i + 1
            while j < n and s[j] != c:
                if s[j] == '\\': j += 1
                j += 1
            i = j + 1; continue
        if c == '`':
            j = i + 1; tdepth = 0
            while j < n:
                if s[j] == '\\': j += 2; continue
                if s.startswith('${', j): tdepth += 1; j += 2; continue
                if tdepth and s[j] == '}': tdepth -= 1; j += 1; continue
                if not tdepth and s[j] == '`': break
                j += 1
            i = j + 1; continue
        if c in '{[(': depth += 1
        elif c in '}])':
            depth -= 1
            if depth == 0: return i + 1
        i += 1
    return n
out = Path(sys.argv[1])
for v in sys.argv[2:]:
    s = (K / v / REL).read_text(encoding='utf-8', errors='replace')
    d = out / v; d.mkdir(parents=True, exist_ok=True)
    for name in NAMES:
        for m in re.finditer(r'name:"%s",' % re.escape(name), s):
            # walk back to the `{` that opens this object (skip whitespace only)
            i = m.start() - 1
            while i >= 0 and s[i] in ' \n\t': i -= 1
            if i < 0 or s[i] != '{':
                # may be `{...,name:` with earlier keys; search back for the nearest unmatched `{`
                depth = 0; j = m.start() - 1
                while j >= 0:
                    if s[j] in '}])': depth += 1
                    elif s[j] in '{[(':
                        if depth == 0: break
                        depth -= 1
                    j -= 1
                i = j
            e = balanced(s, i)
            text = s[i:e]
            if 'steps:' not in text and 'description:' not in text:
                continue
            f = d / f'{name}.js'
            k = 1
            while f.exists(): f = d / f'{name}.{k}.js'; k += 1
            f.write_text(text.replace('\\n', '\n'))
            print(v, name, len(text), 'chars ->', f.name)
