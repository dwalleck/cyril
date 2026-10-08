#!/usr/bin/env python3
"""Rust source-path / module / ACP-method / env inventory across a build chain (2.28.0 audit).

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

BINS = ['kiro-cli', 'kiro-cli-chat', 'kiro-cli-term']
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
per = {}
texts = {}
for dp in dirs:
    v = dp.parts[-4]; per[v] = {}
    for b in BINS:
        t = host_text(dp / b); texts[(v, b)] = t
        per[v][b] = {k: sorted(set(p.findall(t))) for k, p in PATS.items()}
        print(v, b, {k: len(x) for k, x in per[v][b].items()}, flush=True)
(outdir / 'host-paths.json').write_text(json.dumps(per, indent=1))
vers = [d.parts[-4] for d in dirs]
delta = []
for a, b in zip(vers, vers[1:]):
    rec = {'from': a, 'to': b, 'bins': {}}
    for bn in BINS:
        r = {}
        for k in PATS:
            A, B = set(per[a][bn][k]), set(per[b][bn][k])
            r[k] = {'added': [{'tok': t, 'substring_in_old': texts[(a, bn)].count(t)} for t in sorted(B - A)],
                    'removed': [{'tok': t, 'substring_in_new': texts[(b, bn)].count(t)} for t in sorted(A - B)]}
            print(f'{a}->{b} {bn} {k}: +{len(B-A)} -{len(A-B)}')
        rec['bins'][bn] = r
    delta.append(rec)
    for bn in BINS: texts.pop((a, bn), None)
(outdir / 'host-paths-delta.json').write_text(json.dumps(delta, indent=1))
