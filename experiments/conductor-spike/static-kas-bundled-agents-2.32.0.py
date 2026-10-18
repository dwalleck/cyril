#!/usr/bin/env python3
"""Extract bundled agent/skill profiles (string or template literals that start with
`---\\nname:`) from KAS bundles and write one file per (version, name) for diffing.

    python3 -I static-kas-bundled-agents-2.32.0.py OUTDIR OLD.js NEW.js

Uses the lexer from static-kas-bundle-diff-2.31.1.py. Template `${...}` holes are kept
verbatim as text between chunks (each tpl chunk is written in order). Executes nothing.
"""
import importlib.util, re, sys
from pathlib import Path
here = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('bd', here / 'static-kas-bundle-diff-2.31.1.py')
bd = importlib.util.module_from_spec(spec); spec.loader.exec_module(bd)
out = Path(sys.argv[1])
for tag, p in (('old', sys.argv[2]), ('new', sys.argv[3])):
    d = out / tag; d.mkdir(parents=True, exist_ok=True)
    toks = bd.lex(Path(p).read_text(encoding='utf-8', errors='replace'))
    n = 0; i = 0
    while i < len(toks):
        kind, t = toks[i]
        if kind in ('str', 'tpl'):
            body = t[1:-1] if kind == 'str' else t.lstrip('`').removesuffix('${').removesuffix('`')
            if body.startswith('---\\nname:') or body.startswith('---\nname:'):
                parts = [body]
                if kind == 'tpl' and t.endswith('${'):
                    # collect following template chunks until a chunk ending with a backtick
                    j = i + 1
                    while j < len(toks):
                        k2, t2 = toks[j]
                        if k2 == 'tpl':
                            parts.append('${…}' + t2.lstrip('}').removesuffix('${').removesuffix('`'))
                            if not t2.endswith('${'): break
                        j += 1
                text = ''.join(parts).replace('\\n', '\n').replace("\\'", "'").replace('\\`', '`')
                m = re.match(r'---\nname:\s*([^\n]+)', text)
                name = re.sub(r'[^A-Za-z0-9_.-]', '_', m.group(1).strip()) if m else f'anon{n}'
                f = d / f'{name}.md'
                k = 1
                while f.exists(): f = d / f'{name}.{k}.md'; k += 1
                f.write_text(text); n += 1
        i += 1
    print(tag, n, 'profiles')
