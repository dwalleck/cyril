#!/usr/bin/env python3
"""Extract bundled agent/skill profiles (`---\\nname:` literals) from N KAS carves.

    python3 -I static-kas-workflow-agents-3.1.0.py OUTDIR VER [VER ...]

Same extraction as static-kas-bundled-agents-2.32.0.py (lexer from
static-kas-bundle-diff-2.31.1.py) but one subdirectory per carve version so the
wf-* agents can be bisected across the 2.33.0..3.1.0 carves. Executes nothing.
"""
import importlib.util, os, re, sys
from pathlib import Path
here = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('bd', here / 'static-kas-bundle-diff-2.31.1.py')
bd = importlib.util.module_from_spec(spec); spec.loader.exec_module(bd)
K = Path(os.path.expanduser('~/.local/share/kiro-research/kas-carves'))
REL = 'node_modules/@kiro/agent/dist/server/acp-server.js'
out = Path(sys.argv[1])
for v in sys.argv[2:]:
    d = out / v; d.mkdir(parents=True, exist_ok=True)
    toks = bd.lex((K / v / REL).read_text(encoding='utf-8', errors='replace'))
    n = 0; i = 0
    while i < len(toks):
        kind, t = toks[i]
        if kind in ('str', 'tpl'):
            body = t[1:-1] if kind == 'str' else t.lstrip('`').removesuffix('${').removesuffix('`')
            if body.startswith('---\\nname:') or body.startswith('---\nname:'):
                parts = [body]
                if kind == 'tpl' and t.endswith('${'):
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
    print(v, n, 'profiles')
