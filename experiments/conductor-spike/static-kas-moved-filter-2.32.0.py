#!/usr/bin/env python3
"""Filter moved statements out of a norm.diff (static-kas-bundle-diff output).

    python3 -I static-kas-moved-filter-2.32.0.py <bundle-out-dir>

A minifier/bundler reorder shows up as a statement deleted in one hunk and the
identical normalised statement added in another. Cancel such pairs (multiset)
and write <dir>/changed.txt: the remaining old-only / new-only statements as
`-old#N: raw` / `+new#N: raw`, in new-file order, truncated to 1500 chars each.
"""
import re, sys
from collections import Counter
D = sys.argv[1]
on = open(f'{D}/old.norm').read().split('\n'); nn = open(f'{D}/new.norm').read().split('\n')
orw = open(f'{D}/old.raw').read().split('\n'); nrw = open(f'{D}/new.raw').read().split('\n')
dels, adds = [], []
for l in open(f'{D}/norm.diff'):
    m = re.match(r'^(\d+)(?:,(\d+))?([acd])(\d+)(?:,(\d+))?$', l.strip())
    if not m: continue
    a1 = int(m.group(1)); a2 = int(m.group(2) or a1); op = m.group(3); b1 = int(m.group(4)); b2 = int(m.group(5) or b1)
    if op != 'a': dels += list(range(a1, a2 + 1))
    if op != 'd': adds += list(range(b1, b2 + 1))
cd = Counter(on[i - 1].strip() for i in dels); ca = Counter(nn[i - 1].strip() for i in adds)
common = cd & ca
keep_d, keep_a = [], []
used = Counter()
for i in dels:
    k = on[i - 1].strip()
    if used[('d', k)] < common[k]: used[('d', k)] += 1; continue
    if k: keep_d.append(i)
for i in adds:
    k = nn[i - 1].strip()
    if used[('a', k)] < common[k]: used[('a', k)] += 1; continue
    if k: keep_a.append(i)
out = [f'# dels {len(dels)} adds {len(adds)}; moved pairs cancelled {sum(common.values())}; kept -{len(keep_d)} +{len(keep_a)}']
for i in keep_d: out.append(f'-old#{i}: {orw[i - 1][:1500]}')
for i in keep_a: out.append(f'+new#{i}: {nrw[i - 1][:1500]}')
open(f'{D}/changed.txt', 'w').write('\n'.join(out) + '\n')
print(out[0])
