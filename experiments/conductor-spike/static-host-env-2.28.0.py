#!/usr/bin/env python3
"""Env-var census of the Rust host across a build chain (2.28.0 audit).

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
BINS = ['kiro-cli', 'kiro-cli-chat', 'kiro-cli-term']
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
    v = dp.parts[-4]
    for b in BINS:
        d = host(dp / b); raw[(v, b)] = d
        per.setdefault(v, {})[b] = sorted(env_tokens(d))
    print(v, {b: len(per[v][b]) for b in BINS}, flush=True)
(outdir / 'host-env.json').write_text(json.dumps(per, indent=1))
vers = [d.parts[-4] for d in dirs]; delta = []
for a, b in zip(vers, vers[1:]):
    rec = {'from': a, 'to': b, 'bins': {}}
    for bn in BINS:
        A, B = set(per[a][bn]), set(per[b][bn])
        rec['bins'][bn] = {
            'added': [{'tok': t, 'substring_in_old': raw[(a, bn)].count(t.encode())} for t in sorted(B - A)],
            'removed': [{'tok': t, 'substring_in_new': raw[(b, bn)].count(t.encode())} for t in sorted(A - B)]}
        for x in rec['bins'][bn]['added']: print(f"{a}->{b} {bn} + {x['tok']} (substring in old x{x['substring_in_old']})")
        for x in rec['bins'][bn]['removed']: print(f"{a}->{b} {bn} - {x['tok']} (substring in new x{x['substring_in_new']})")
    delta.append(rec)
    for bn in BINS: raw.pop((a, bn), None)
(outdir / 'host-env-delta.json').write_text(json.dumps(delta, indent=1))
