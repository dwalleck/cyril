#!/usr/bin/env python3
"""Glue-proof tracing-source-location diff across a chain of host builds (3.1.0 audit, copied from 2.32.0; role-aware for the 3.0.0 single-binary merge).

    python3 -I static-host-srcloc-2.29.1.py <outdir> <ver1:bindir1> <ver2:bindir2> [...]

`crates/<crate>/src/<path>.rs:LINE` literals survive stripping (trap 9) and give the
per-file tracing-call inventory. A raw digit run on the wire may carry extra digits glued
from the FOLLOWING rodata literal (`...reqwest_client.rs:44` + `1.0...U...` renders as
`:441`), so single line values are ambiguous. The glue-proof metric is the NUMBER of
occurrences of `<path>.rs:` per file (digit glue cannot change that count) plus the
multiset of digit-run lengths. A changed count = a tracing call added or removed = the
source file was edited. Where counts are equal but the raw run lists differ, report the
modal delta of the *prefix-consistent* candidates so a pure line shift is visible.
Writes <outdir>/srcloc-<b>.json.
"""
import json, re, sys
from pathlib import Path
import zstandard
BINS = ['kiro-cli', 'kiro-cli-chat', 'kiro-cli-term']  # 2.x names; see roles()

# 3.1.0 audit: role mapping across the 3.0.0 merge. host = kiro-cli-chat on 2.x, the single kiro-cli on 3.x;
# launcher/term exist only on 2.x. A chain step from 3 binaries to 1 compares like roles (host vs host) and,
# in a separate "merge" record, the 3.x single binary against the UNION of the last 2.x launcher+chat+term.
def roles(bindir):
    d = Path(bindir)
    if (d / 'kiro-cli-chat').exists():
        return {'host': d / 'kiro-cli-chat', 'launcher': d / 'kiro-cli', 'term': d / 'kiro-cli-term'}
    return {'host': d / 'kiro-cli', 'launcher': None, 'term': None}
ROLES = ['host', 'launcher', 'term']
MAGIC = b'\x28\xb5\x2f\xfd'
PAT = re.compile(rb'(crates/[A-Za-z0-9_.-]+/src/[A-Za-z0-9_./-]+\.rs):(\d+)')

def frames(d):
    i = 0; out = []
    while True:
        i = d.find(MAGIC, i)
        if i < 0: return out
        try:
            o = zstandard.ZstdDecompressor().decompressobj(); pos = i
            while not o.eof and pos < len(d):
                c = d[pos:pos + (4 << 20)]; o.decompress(c); pos += len(c)
            if o.eof:
                end = pos - len(o.unused_data)
                if end - i > 50000: out.append((i, end)); i = end; continue
        except zstandard.ZstdError: pass
        i += 4

def scan(p):
    raw = Path(p).read_bytes(); d = bytearray(raw)
    for a, b in frames(raw): d[a:b] = b'\0' * (b - a)
    d = bytes(d)
    per = {}
    for m in PAT.finditer(d):
        per.setdefault(m.group(1).decode(), []).append(m.group(2).decode())
    return per

outdir = Path(sys.argv[1]); outdir.mkdir(parents=True, exist_ok=True)
chain = [a.split(':', 1) for a in sys.argv[2:]]
data = {}
for ver, bd in chain:
    rm = roles(bd)
    for r in ROLES:
        if rm[r] is None: continue
        per = scan(rm[r])
        data.setdefault(r, {})[ver] = per
        print(f'{ver} {r}: {len(per)} files, {sum(len(v) for v in per.values())} call sites', flush=True)
report = {}
for b in ROLES:
    vers = [v for v, _ in chain if v in data.get(b, {})]
    rep = []
    for a, c in zip(vers, vers[1:]):
        A, C = data[b][a], data[b][c]
        added = sorted(set(C) - set(A)); removed = sorted(set(A) - set(C))
        countdiff = {f: [len(A[f]), len(C[f])] for f in sorted(set(A) & set(C)) if len(A[f]) != len(C[f])}
        shift = {}
        for f in sorted(set(A) & set(C)):
            if A[f] == C[f]: continue
            raws = set()
            for v in (A, C):
                for r_ in v.get(f, []): raws.add(r_)
            def prefixes(r_): return {r_[:i] for i in range(1, len(r_) + 1)}
            ca = sorted({p for r_ in A[f] for p in prefixes(r_) if p in raws})
            cc = sorted({p for r_ in C[f] for p in prefixes(r_) if p in raws})
            if ca != cc:
                try:
                    d_ = [int(y) - int(x) for x, y in zip(ca, cc)]
                    shift[f] = {'old': ca, 'new': cc, 'deltas': sorted(set(d_))}
                except ValueError:
                    shift[f] = {'old': ca, 'new': cc, 'deltas': None}
        rep.append({'from': a, 'to': c, 'files_added': added, 'files_removed': removed,
                    'count_changed': countdiff, 'line_shift': shift})
        print(f'== {b} {a} -> {c}')
        print(f'   files added {added}')
        print(f'   files removed {removed}')
        for f, v in countdiff.items(): print(f'   count {f}: {v[0]} -> {v[1]} call sites')
        for f, v in shift.items(): print(f'   shift {f}: deltas {v["deltas"]} old {v["old"][:8]} new {v["new"][:8]}')
    report[b] = rep
# merge record: last 3-binary version's union of files vs first single-binary version
vers_all = [v for v, _ in chain]
for a, c in zip(vers_all, vers_all[1:]):
    if a in data.get('launcher', {}) and c not in data.get('launcher', {}):
        U = {}
        for r in ROLES:
            for f, sites in data[r].get(a, {}).items(): U.setdefault(f, []).extend(sites)
        C = data['host'][c]
        rep = {'from': a, 'to': c, 'files_added_vs_union': sorted(set(C) - set(U)), 'files_removed_vs_union': sorted(set(U) - set(C)),
               'launcher_only_files_kept': sorted((set(data['launcher'][a]) - set(data['host'][a])) & set(C)),
               'launcher_only_files_dropped': sorted((set(data['launcher'][a]) - set(data['host'][a])) - set(C)),
               'term_only_files_kept': sorted((set(data['term'][a]) - set(data['host'][a])) & set(C)),
               'term_only_files_dropped': sorted((set(data['term'][a]) - set(data['host'][a])) - set(C))}
        report['merge'] = rep
        print(f'== MERGE {a}(union of 3) -> {c}(single)')
        for k, v in rep.items():
            if isinstance(v, list): print(f'   {k} ({len(v)}): {v}')
(outdir / 'srcloc-report.json').write_text(json.dumps(report, indent=1))
