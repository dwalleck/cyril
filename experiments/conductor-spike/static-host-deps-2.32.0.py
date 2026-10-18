#!/usr/bin/env python3
"""Third-party crate (name-version) set diff of the Rust host builds (2.32.0 audit, new).
usage: python3 -I static-host-deps-2.32.0.py <strings-root> <bin> <verA> <verB>

Input: masked-strings files (audit-<ver>/host/raw/mkstrings.py output: `<offset>\t<string>` per
line, zstd payload frames zeroed). Binaries are stripped (trap 9) but panic/tracing locations keep
cargo-registry paths `.../index.crates.io-<hash>/<crate>-<semver>/src/...` and git deps
`.../git/checkouts/<repo>-<hash>/<rev>/`. A crate that appears/disappears or bumps version is the
best static explanation for a large `.text` delta with no first-party source-file change.
Prints: removed crates, added crates, version bumps (same name), with per-version path counts.
"""
import re, sys
from collections import Counter, defaultdict
from pathlib import Path
root, b, va, vb = sys.argv[1:5]
REG = re.compile(r'index\.crates\.io-[0-9a-f]+/([A-Za-z0-9_\-]+?)-(\d+\.\d+\.\d+[A-Za-z0-9.\-+]*)/')
GIT = re.compile(r'git/checkouts/([A-Za-z0-9_\-]+)-[0-9a-f]+/([0-9a-f]{7,})/')
def load(v):
    c = Counter(); g = Counter()
    for ln in (Path(root) / v / f'{b}.txt').read_text('ascii', 'replace').splitlines():
        s = ln.partition('\t')[2]
        for m in REG.finditer(s): c[(m.group(1), m.group(2))] += 1
        for m in GIT.finditer(s): g[(m.group(1), m.group(2))] += 1
    return c, g
(ca, ga), (cb, gb) = load(va), load(vb)
def byname(c):
    d = defaultdict(dict)
    for (n, ver), k in c.items(): d[n][ver] = k
    return d
na, nb = byname(ca), byname(cb)
print(f'{va}: {len(ca)} crate-versions / {len(na)} crates; {vb}: {len(cb)} / {len(nb)}')
for n in sorted(set(na) - set(nb)): print(f'  - {n} {na[n]}')
for n in sorted(set(nb) - set(na)): print(f'  + {n} {nb[n]}')
for n in sorted(set(na) & set(nb)):
    if set(na[n]) != set(nb[n]): print(f'  ~ {n} {na[n]} -> {nb[n]}')
for n in sorted(set(na) & set(nb)):
    if set(na[n]) == set(nb[n]):
        da, db = sum(na[n].values()), sum(nb[n].values())
        if abs(da - db) >= max(10, da // 4): print(f'  # {n} path-count {da} -> {db}')
print('git deps:')
for k in sorted(set(ga) | set(gb)):
    if ga.get(k, 0) != gb.get(k, 0): print(f'  {k} {ga.get(k,0)} -> {gb.get(k,0)}')
