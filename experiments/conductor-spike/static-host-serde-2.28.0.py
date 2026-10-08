#!/usr/bin/env python3
"""Serde type-shape inventory of the Rust host across a build chain (2.28.0 audit).

    python3 -I static-host-serde-2.28.0.py <outdir> <bin1> <bin2> [...]

Serde's derived visitors embed `expecting` strings that survive stripping and
are glue-resistant because they carry their own terminators:
  "struct Foo with 3 elements", "struct variant Enum::Var with 2 elements",
  "tuple struct Foo with 1 element", "adjacently tagged enum Foo",
  "internally tagged enum Foo", "variant identifier", "field identifier".
A changed element count is a field added/removed on that type (wire-relevant
when the type is ACP/ext-method payload). Each pair prints ADDED/REMOVED shapes
and count changes keyed by type name. Writes <outdir>/serde-shapes.json.
"""
import json, re, sys
from pathlib import Path
PAT = re.compile(rb'(struct variant|tuple variant|tuple struct|struct|adjacently tagged enum|internally tagged enum|untagged enum) ([A-Z][A-Za-z0-9_]*(?:::[A-Z][A-Za-z0-9_]*)?) with (\d+) elements?')
TAG = re.compile(rb'(adjacently tagged enum|internally tagged enum|untagged enum) ([A-Z][A-Za-z0-9]*)(?=[A-Z][a-z]|[^A-Za-z0-9]|$)')
outdir = Path(sys.argv[1]); outdir.mkdir(parents=True, exist_ok=True)
recs = {}
for p in sys.argv[2:]:
    d = Path(p).read_bytes()
    shapes = {}
    for m in PAT.finditer(d):
        k = (m.group(1).decode() + ' ' + m.group(2).decode())
        shapes.setdefault(k, set()).add(int(m.group(3)))
    tags = sorted({(m.group(1) + b' ' + m.group(2)).decode() for m in TAG.finditer(d)})
    recs[p] = {'shapes': {k: sorted(v) for k, v in shapes.items()}, 'tagged': tags}
    print(p.split('/')[-5], len(shapes), 'shapes', len(tags), 'tagged enums')
ps = sys.argv[2:]
out = []
for a, b in zip(ps, ps[1:]):
    A, B = recs[a]['shapes'], recs[b]['shapes']
    r = {'from': a, 'to': b,
         'added': {k: B[k] for k in sorted(set(B) - set(A))},
         'removed': {k: A[k] for k in sorted(set(A) - set(B))},
         'count_changed': {k: [A[k], B[k]] for k in sorted(set(A) & set(B)) if A[k] != B[k]},
         'tagged_added': sorted(set(recs[b]['tagged']) - set(recs[a]['tagged'])),
         'tagged_removed': sorted(set(recs[a]['tagged']) - set(recs[b]['tagged']))}
    out.append(r)
    print('###', a.split('/')[-5], '->', b.split('/')[-5])
    for k, v in r['added'].items(): print('  +', k, v)
    for k, v in r['removed'].items(): print('  -', k, v)
    for k, v in r['count_changed'].items(): print('  ~', k, v[0], '->', v[1])
    for k in r['tagged_added']: print('  +tag', k)
    for k in r['tagged_removed']: print('  -tag', k)
(outdir / 'serde-shapes.json').write_text(json.dumps({'records': recs, 'deltas': out}, indent=1))
