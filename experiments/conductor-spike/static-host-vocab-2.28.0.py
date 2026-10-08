#!/usr/bin/env python3
"""Full-vocabulary host-string diff across a chain of kiro-cli builds (2.28.0 audit).

    python3 -I static-host-vocab-2.28.0.py <outdir> <bindir-v1> <bindir-v2> [...]

Each <bindir> holds kiro-cli, kiro-cli-chat, kiro-cli-term. Large zstd frames
(KAS tar, Node, Bun, TUI bundle; >50 KB compressed) are masked out first so
the census covers the Rust host only. Tokens = identifier-ish runs
[A-Za-z_][A-Za-z0-9_]* optionally joined by . / : - (len >= 4). For each
consecutive pair, ADDED tokens are kept only if the token is not a substring
anywhere in the old build's host printable text (filters LTO re-glue); REMOVED tokens
symmetric. Each survivor gets a +-100 byte context snippet. Avoids the
fixed-vocabulary blind spot: nothing here is a known-token recount.
Writes <outdir>/host-vocab-delta.json and prints a summary.
"""
import json, re, sys
from pathlib import Path
import zstandard

BINS = ['kiro-cli', 'kiro-cli-chat', 'kiro-cli-term']
TOK = re.compile(rb'[A-Za-z_][A-Za-z0-9_]*(?:[./:-][A-Za-z0-9_]+)*')

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

def host_bytes(path):
    d = bytearray(Path(path).read_bytes())
    for a, b in frames(bytes(d)):
        d[a:b] = b'\0' * (b - a)
    return bytes(d)

PRINT = re.compile(rb'[\x20-\x7e]{4,}')

SRCLOC = re.compile(rb'(?:event )?(?:crates|library|/github/home/\.cargo/registry/src/[^/]+)/[A-Za-z0-9_./-]+?\.rs(?::\d+)?')
TYNAME = re.compile(rb'(?:[a-z0-9_]+::)+')

def compact(d):
    # printable runs only: the substring re-glue check runs over this (~10x smaller).
    # Rust source locations (`event crates/x/src/y.rs:LINE`, panic paths) and
    # `module::path::` prefixes are cut to newlines: line numbers churn every
    # build and glue neighbours into phantom tokens. Source paths have their own
    # lane (static-host-paths-2.28.0.py).
    t = b'\n'.join(m.group() for m in PRINT.finditer(d))
    t = SRCLOC.sub(b'\n', t)
    return TYNAME.sub(b'\n', t)

def vocab(c):
    return {m.group() for m in TOK.finditer(c) if len(m.group()) >= 4}

def ctx(d, tok):
    i = d.find(tok)
    s = d[max(0, i - 100): i + len(tok) + 100]
    return re.sub(rb'[^\x20-\x7e]', b'.', s).decode()

outdir = Path(sys.argv[1]); dirs = [Path(p) for p in sys.argv[2:]]
outdir.mkdir(parents=True, exist_ok=True)
cache = {}
def get(dirp, b):
    k = (str(dirp), b)
    if k not in cache:
        c = compact(host_bytes(dirp / b)); cache[k] = (c, vocab(c))
        print('  scanned', dirp.parts[-4], b, len(c), 'bytes printable,', len(cache[k][1]), 'tokens', flush=True)
    return cache[k]

result = []
for old, new in zip(dirs, dirs[1:]):
    pair = {'from': str(old), 'to': str(new), 'bins': {}}
    for b in BINS:
        do, vo = get(old, b); dn, vn = get(new, b)
        added = sorted(t for t in vn - vo if do.find(t) < 0)
        removed = sorted(t for t in vo - vn if dn.find(t) < 0)
        pair['bins'][b] = {
            'added': [{'tok': t.decode(), 'ctx': ctx(dn, t)} for t in added],
            'removed': [{'tok': t.decode(), 'ctx': ctx(do, t)} for t in removed]}
        print(f'{old.parts[-4]} -> {new.parts[-4]} {b}: +{len(added)} -{len(removed)}', flush=True)
    result.append(pair)
    # drop cache entries for old to bound memory
    for b in BINS: cache.pop((str(old), b), None)
(outdir / 'host-vocab-delta.json').write_text(json.dumps(result, indent=1))
