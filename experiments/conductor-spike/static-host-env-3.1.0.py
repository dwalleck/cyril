#!/usr/bin/env python3
"""Env-var census of the Rust host across a build chain (3.1.0 audit, copied from 2.32.0; role-aware for the 3.0.0 single-binary merge).

    python3 -I static-host-env-2.28.0.py <outdir> <bindir1> <bindir2> [...]

Adapted from static-host-env-2.24.0.py. Changes: zstd payload frames masked
(TUI/KAS/Node/Bun excluded), chain of N builds, and glued env tables (Rust
packs the env-name literals back to back: `KIRO_PARENTQ_SET_PARENTKIRO_SET_...`)
are split before a known env prefix. Prefixes: KIRO_ Q_ AMAZON_Q_ ASBX_ KAS_
AWS_ CLOUD_CONFIG PROCESS_LAUNCHED NODE_ BUN_ HOMEBREW_ XDG_ RUST_ OTEL_.
Every add/remove is re-checked as a raw substring count in the other build so a
re-split shows up as substring-present (not a real add/remove).
Writes <outdir>/host-env.json + host-env-delta.json.
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
PFX = r'(?:KIRO_|AMAZON_Q_|Q_|ASBX_|KAS_|AWS_|CLOUD_CONFIG|PROCESS_LAUNCHED|NODE_|BUN_|HOMEBREW_|XDG_|RUST_|OTEL_)'
RUN = re.compile(rb'(?<![A-Z0-9_])[A-Z][A-Z0-9_]{3,}')  # lowercase may precede: literals glue after prose
# RUST_ / Q_ are not split points mid-run when they sit inside a longer KIRO_ name
# (KIRO_TRUST_... would split as KIRO_T + RUST_...); they still count at run start.
SPLITPFX = PFX.replace('|RUST_', '')
SPLIT = re.compile(r'(?<=[A-Z0-9])(?<!TRUST)(?=' + SPLITPFX + r'[A-Z0-9])')

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
                if end - i > 50000: out.append((i, end)); i = end; continue
        except zstandard.ZstdError: pass
        i += 4

def host(p):
    d = bytearray(Path(p).read_bytes())
    for a, b in frames(bytes(d)): d[a:b] = b'\0' * (b - a)
    return bytes(d)

def env_tokens(d):
    out = set()
    for m in RUN.finditer(d):
        for piece in SPLIT.split(m.group().decode()):
            if re.match(PFX, piece) and len(piece) > 4 and not piece.endswith('_'):
                out.add(piece)
    return out

outdir = Path(sys.argv[1]); outdir.mkdir(parents=True, exist_ok=True)
dirs = [Path(p) for p in sys.argv[2:]]
per = {}; raw = {}
for dp in dirs:
    v = dp.parts[-4]; rm = roles(dp); per[v] = {}
    for r in ROLES:
        if rm[r] is None: continue
        d = host(rm[r]); raw[(v, r)] = d
        per[v][r] = sorted(env_tokens(d))
    print(v, {r: len(per[v][r]) for r in per[v]}, flush=True)
(outdir / 'host-env.json').write_text(json.dumps(per, indent=1))
vers = [d.parts[-4] for d in dirs]; delta = []
for a, b in zip(vers, vers[1:]):
    rec = {'from': a, 'to': b, 'roles': {}}
    for r in ROLES:
        if r not in per[a] or r not in per[b]: continue
        A, B = set(per[a][r]), set(per[b][r])
        rec['roles'][r] = {
            'added': [{'tok': t, 'substring_in_old': raw[(a, r)].count(t.encode())} for t in sorted(B - A)],
            'removed': [{'tok': t, 'substring_in_new': raw[(b, r)].count(t.encode())} for t in sorted(A - B)]}
        for x in rec['roles'][r]['added']: print(f"{a}->{b} {r} + {x['tok']} (substring in old x{x['substring_in_old']})")
        for x in rec['roles'][r]['removed']: print(f"{a}->{b} {r} - {x['tok']} (substring in new x{x['substring_in_new']})")
    if len(per[a]) == 3 and len(per[b]) == 1:  # the merge: single binary vs union of the three
        U = set().union(*(set(per[a][r]) for r in per[a])); B = set(per[b]['host'])
        rawU = b''.join(raw[(a, r)] for r in per[a])
        rec['merge'] = {'added': [{'tok': t, 'substring_in_old_union': rawU.count(t.encode())} for t in sorted(B - U)],
                        'removed': [{'tok': t, 'substring_in_new': raw[(b, 'host')].count(t.encode())} for t in sorted(U - B)]}
        print(f"MERGE {a}(launcher+chat+term) -> {b}(single): +{len(B-U)} -{len(U-B)}")
        for x in rec['merge']['added']: print(f"  MERGE + {x['tok']} (substring in old union x{x['substring_in_old_union']})")
        for x in rec['merge']['removed']: print(f"  MERGE - {x['tok']} (substring in new x{x['substring_in_new']})")
    delta.append(rec)
    for r in ROLES: raw.pop((a, r), None)
(outdir / 'host-env-delta.json').write_text(json.dumps(delta, indent=1))
