#!/usr/bin/env python3
"""Extract the KAS `session_info_update` kind vocabulary from N minified bundles.

    python3 -I static-kas-session-info-kinds-2.28.0.py <acp-server.js>...

Anchor (string-shape only): the brace-balanced `switch(` block that contains
`case"pending_interaction":return{pendingInteraction:` — the converter that maps
every session_info kind onto its `_meta.kiro` projection. Prints the case labels
per bundle and the set delta between consecutive bundles.
"""
import re, sys
def kinds(t):
    i = t.find('case"pending_interaction":return{pendingInteraction:')
    if i < 0: return None
    s = t.rfind('switch(', 0, i); j = t.find('{', s); d = 0; k = j
    while k < len(t):
        if t[k] == '{': d += 1
        elif t[k] == '}':
            d -= 1
            if d == 0: break
        k += 1
    return set(re.findall(r'case"([a-z_]+)"', t[s:k+1]))
ps = sys.argv[1:]; res = {}
for p in ps:
    res[p] = kinds(open(p, encoding='utf-8', errors='replace').read())
    print(p.split('/')[-7], len(res[p] or ()), sorted(res[p] or ()))
for a, b in zip(ps, ps[1:]):
    print(f"{a.split('/')[-7]} -> {b.split('/')[-7]}: +{sorted(res[b]-res[a])} -{sorted(res[a]-res[b])}")
