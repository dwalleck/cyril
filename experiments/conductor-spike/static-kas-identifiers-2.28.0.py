#!/usr/bin/env python3
"""Quoted-literal SET diff across N KAS bundles (from static-kas-identifiers-2.24.0.py).

    python3 -I static-kas-identifiers-2.28.0.py --out <dir> <acp-server.js>...

Double-, single- and backtick-free quoted literals are matched independently with a
bounded non-quote body so one stray quote cannot desynchronise the scan. Each
literal is also COUNTED in both bundles, so a bundler rename that only moves a
literal around shows as unchanged. Writes <out>/static-kas-identifiers-delta.json
and <out>/identifiers-<from>-<to>.txt (full added/removed, with occurrence counts).
Also diffs long prose literals (>=40 chars, prompt/user-facing text) separately.
"""
import argparse, json, re
from collections import Counter
from pathlib import Path
ap = argparse.ArgumentParser(); ap.add_argument('--out', required=True); ap.add_argument('paths', nargs='+')
a = ap.parse_args(); out = Path(a.out); out.mkdir(parents=True, exist_ok=True)
paths = [Path(p) for p in a.paths]
valid = re.compile(r'[A-Za-z_][A-Za-z0-9_.:/@-]{2,120}$')
interesting = re.compile(r'(?i)(?:stall|reason|unified|agent|memory|skill|compact|permission|retry|workflow|hook|steer|model|tool|policy|output|turn|session|context|artifact|capture|preview|snapshot|large|truncat|offload|mcp|plan|proxy|cloud|config|fallback|route|routing|cascade|tangent|terminal|background|validat|interrupt|notice|unavailable|capacity|refus|govern|configur|contribut|client|connect|initiali)')
dq = re.compile(r'"([^"\\\n]{3,400})"'); sq = re.compile(r"'([^'\\\n]{3,400})'")
def get(p):
    t = p.read_text(encoding='utf-8', errors='replace')
    c = Counter(m.group(1) for m in dq.finditer(t)); c.update(m.group(1) for m in sq.finditer(t))
    ids = Counter({k: v for k, v in c.items() if valid.fullmatch(k)})
    prose = Counter({k: v for k, v in c.items() if len(k) >= 40 and ' ' in k and not re.search(r'[=;{}]|\b[a-zA-Z$_]{1,3}\(', k)
                     and sum(ch.isalpha() or ch == ' ' for ch in k) / len(k) > 0.85})
    return ids, prose
sets = [get(p) for p in paths]
recs = []
for i in range(len(paths) - 1):
    (A, PA), (B, PB) = sets[i], sets[i+1]
    na, nb = paths[i].parts[-7], paths[i+1].parts[-7]
    added = sorted(set(B) - set(A)); removed = sorted(set(A) - set(B))
    padd = sorted(set(PB) - set(PA)); prem = sorted(set(PA) - set(PB))
    d = {'old': str(paths[i]), 'new': str(paths[i+1]), 'old_count': len(A), 'new_count': len(B),
         'added': added, 'removed': removed,
         'relevant_added': [x for x in added if interesting.search(x)],
         'relevant_removed': [x for x in removed if interesting.search(x)],
         'prose_added': padd, 'prose_removed': prem}
    recs.append(d)
    with open(out / f'identifiers-{na}-{nb}.txt', 'w') as f:
        f.write(f'# {na} -> {nb}: ids {len(A)} -> {len(B)} (+{len(added)} -{len(removed)}); prose +{len(padd)} -{len(prem)}\n')
        f.write('## ADDED ids (count in new)\n'); [f.write(f'+ {B[x]:3} {x}\n') for x in added]
        f.write('## REMOVED ids (count in old)\n'); [f.write(f'- {A[x]:3} {x}\n') for x in removed]
        f.write('## ADDED prose\n'); [f.write(f'+ {x}\n') for x in padd]
        f.write('## REMOVED prose\n'); [f.write(f'- {x}\n') for x in prem]
    print(f"=== {na} -> {nb}: ids {len(A)} -> {len(B)} (+{len(added)} -{len(removed)}) prose +{len(padd)} -{len(prem)}; relevant +{len(d['relevant_added'])} -{len(d['relevant_removed'])}")
(out / 'static-kas-identifiers-delta.json').write_text(json.dumps(recs, indent=2, sort_keys=True))
