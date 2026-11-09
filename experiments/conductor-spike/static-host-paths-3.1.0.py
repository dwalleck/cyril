#!/usr/bin/env python3
"""Rust source-path / module / ACP-method / env inventory across a build chain (3.1.0 audit, copied from 2.32.0; role-aware for the 3.0.0 single-binary merge).

    python3 -I static-host-paths-2.28.0.py <outdir> <bindir-v1> <bindir-v2> [...]

Binaries are stripped since 2.23.0, so `crates/<crate>/src/**.rs` strings (tracing
`event crates/...:LINE` + panic locations) are the module inventory. Adapted
from static-host-paths-2.24.0.py: ALL crates (not just chat-cli*), all three
binaries, zstd payload frames masked (so TUI/KAS/Node/Bun strings never count).
Also: `chat_cli*::` module tokens, `kiro.dev/...` method leads, strict-boundary
KIRO_* env tokens, and generic UPPER_SNAKE env-shaped tokens (each add/remove
re-checked as a substring in the other build to flag re-glue).
Writes <outdir>/host-paths.json (per version sets) and host-paths-delta.json.
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
PRINT = re.compile(rb'[\x20-\x7e]{4,}')

def frames(d):
    magic = b'\x28\xb5\x2f\xfd'; i = 0; out = []
    while True:
        i = d.find(magic, i)
        if i < 0: return out
        try:
            o = zstandard.ZstdDecompressor().decompressobj(); pos = i
            while not o.eof and pos < len(d):
                chunk = d[pos:pos + (4 << 20)]; o.decompress(chunk); pos += len(chunk)
            if o.eof:
                end = pos - len(o.unused_data)
                if end - i > 50000:
                    out.append((i, end)); i = end; continue
        except zstandard.ZstdError:
            pass
        i += 4

def host_text(path):
    d = bytearray(Path(path).read_bytes())
    for a, b in frames(bytes(d)): d[a:b] = b'\0' * (b - a)
    return b'\n'.join(m.group() for m in PRINT.finditer(d)).decode('ascii')

PATS = {
    'source_paths': re.compile(r'crates/[A-Za-z0-9_-]+/src/[A-Za-z0-9_./-]+?\.rs'),
    'module_tokens': re.compile(r'\b(?:chat_cli(?:_v2)?|kiro_[a-z_]+|fig_[a-z_]+)::[A-Za-z0-9_:]+'),
    'method_leads': re.compile(r'(?<![A-Za-z0-9_./:-])_?kiro(?:\.dev)?/[A-Za-z0-9_/.-]+'),
    'kiro_env': re.compile(r'(?<![A-Za-z0-9_])KIRO_[A-Z0-9_]{2,}'),
    'upper_snake': re.compile(r'(?<![A-Za-z0-9_])[A-Z][A-Z0-9]{1,}(?:_[A-Z0-9]+){1,}(?![a-z])'),
}

outdir = Path(sys.argv[1]); outdir.mkdir(parents=True, exist_ok=True)
dirs = [Path(p) for p in sys.argv[2:]]
per = {}; texts = {}
for dp in dirs:
    v = dp.parts[-4]; rm = roles(dp); per[v] = {}
    for r in ROLES:
        if rm[r] is None: continue
        t = host_text(rm[r]); texts[(v, r)] = t
        per[v][r] = {k: sorted(set(p.findall(t))) for k, p in PATS.items()}
        print(v, r, {k: len(x) for k, x in per[v][r].items()}, flush=True)
(outdir / 'host-paths.json').write_text(json.dumps(per, indent=1))
vers = [d.parts[-4] for d in dirs]
delta = []
for a, b in zip(vers, vers[1:]):
    rec = {'from': a, 'to': b, 'roles': {}}
    for r in ROLES:
        if r not in per[a] or r not in per[b]: continue
        rr = {}
        for k in PATS:
            A, B = set(per[a][r][k]), set(per[b][r][k])
            rr[k] = {'added': [{'tok': t, 'substring_in_old': texts[(a, r)].count(t)} for t in sorted(B - A)],
                     'removed': [{'tok': t, 'substring_in_new': texts[(b, r)].count(t)} for t in sorted(A - B)]}
            print(f'{a}->{b} {r} {k}: +{len(B-A)} -{len(A-B)}')
        rec['roles'][r] = rr
    if len(per[a]) == 3 and len(per[b]) == 1:
        tU = '\n'.join(texts[(a, r)] for r in per[a]); m = {}
        for k in PATS:
            U = set().union(*(set(per[a][r][k]) for r in per[a])); B = set(per[b]['host'][k])
            m[k] = {'added': [{'tok': t, 'substring_in_old_union': tU.count(t)} for t in sorted(B - U)],
                    'removed': [{'tok': t, 'substring_in_new': texts[(b, 'host')].count(t)} for t in sorted(U - B)],
                    'launcher_only_kept': sorted((set(per[a]['launcher'][k]) - set(per[a]['host'][k])) & B),
                    'launcher_only_dropped': sorted((set(per[a]['launcher'][k]) - set(per[a]['host'][k])) - B),
                    'term_only_kept': sorted((set(per[a]['term'][k]) - set(per[a]['host'][k])) & B),
                    'term_only_dropped': sorted((set(per[a]['term'][k]) - set(per[a]['host'][k])) - B)}
            print(f"MERGE {a}(union) -> {b}(single) {k}: +{len(B-U)} -{len(U-B)}; launcher-only kept {len(m[k]['launcher_only_kept'])} dropped {len(m[k]['launcher_only_dropped'])}; term-only kept {len(m[k]['term_only_kept'])} dropped {len(m[k]['term_only_dropped'])}")
        rec['merge'] = m
    delta.append(rec)
    for r in ROLES: texts.pop((a, r), None)
(outdir / 'host-paths-delta.json').write_text(json.dumps(delta, indent=1))
