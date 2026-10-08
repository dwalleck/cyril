#!/usr/bin/env python3
"""Extract the KAS AgentSettings zod schema (the `_meta.kiro.settings` contract)
from N minified bundles and diff its top-level + nested keys.

    python3 -I static-kas-settings-2.28.0.py <acp-server.js>...

Anchors on string shape, never minified names: the block that starts at
`X=Y.object({enabled:Y.boolean()})` (the shared {enabled} toggle schema) and
ends at the following `=Z.catchall(` (AgentSettings = W.catchall(unknown)).
Also prints the "known extra keys" list that 0.66.26's settings-intake uses to
classify undeclared keys (`["c2s","verifyFirstWorkflow",...]`), if present.
"""
import re, sys
def block(t):
    m = re.search(r'\b\w+=(\w+)\.object\(\{enabled:\1\.boolean\(\)\}\);', t)
    if not m: return None
    e = t.find('.catchall(', m.start())
    return t[m.start(): e + 40]
def keys(b):
    # top-level-ish keys: `name:` preceded by { or , and followed by a schema builder or identifier
    return set(re.findall(r'[{,]([A-Za-z_$][\w$]*):(?:[\w$]+\.(?:object|boolean|number|string|enum|array|record|union|literal|unknown)\(|[\w$]+(?:\.optional\(\)|\b))', b))
res = {}
for p in sys.argv[1:]:
    t = open(p, encoding='utf-8', errors='replace').read()
    b = block(t)
    extra = re.search(r'=\[("c2s"[^\]]*)\]', t)
    res[p] = (b, keys(b) if b else set())
    print(f'### {p}\n  block={len(b) if b else None} chars, keys={len(res[p][1])}')
    if extra: print('  known-extra-keys:', extra.group(1))
ps = sys.argv[1:]
for a, b in zip(ps, ps[1:]):
    A, B = res[a][1], res[b][1]
    print(f'\n=== {a.split("/")[-7]} -> {b.split("/")[-7]}: +{sorted(B-A)}  -{sorted(A-B)}')
print('\n--- last block ---\n' + (res[ps[-1]][0] or ''))
