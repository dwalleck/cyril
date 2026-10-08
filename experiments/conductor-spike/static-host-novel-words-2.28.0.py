#!/usr/bin/env python3
"""Glue-proof novel-word census of the Rust host across a build chain (2.28.0 audit).

    python3 -I static-host-novel-words-2.28.0.py <outdir> <bin> <ver1:bindir1> <ver2:bindir2> [...]

LTO packs string literals back to back, so whole-token diffs drown in re-glue
churn (static-host-vocab-2.28.0.py: +600/-600 per patch release). Glue can only
CONCATENATE existing literals; it cannot mint a new word. So: split the masked
host text (zstd payloads removed) into camelCase/snake_case/kebab words (>= 5
chars), and report words of the new build that do not occur ANYWHERE (raw
substring) in the old build -- and vice versa. Each hit gets up to 3 context
windows. Writes <outdir>/novel-words-<bin>.json.
"""
import json, re, sys
from pathlib import Path
import zstandard
PRINT = re.compile(rb'[\x20-\x7e]{4,}')
WORD = re.compile(r'[A-Z]?[a-z]{4,}|[A-Z]{5,}(?![a-z])')

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

def text(p):
    d = bytearray(Path(p).read_bytes())
    for a, b in frames(bytes(d)): d[a:b] = b'\0' * (b - a)
    return '\n'.join(m.group().decode() for m in PRINT.finditer(d))

def ctxs(t, w, n=3):
    out = []
    for m in re.finditer(re.escape(w), t):
        out.append(t[max(0, m.start() - 120): m.end() + 120].replace('\n', '|'))
        if len(out) >= n: break
    return out

outdir = Path(sys.argv[1]); outdir.mkdir(parents=True, exist_ok=True)
binname = sys.argv[2]; chain = [a.split(':', 1) for a in sys.argv[3:]]
prev = None; res = []
for ver, d in chain:
    t = text(Path(d) / binname); words = set(WORD.findall(t))
    if prev:
        pv, pt, pw = prev
        added = sorted(w for w in words - pw if w not in pt)
        removed = sorted(w for w in pw - words if w not in t)
        res.append({'from': pv, 'to': ver,
                    'added': {w: ctxs(t, w) for w in added},
                    'removed': {w: ctxs(pt, w) for w in removed}})
        print(f'{pv} -> {ver} {binname}: +{len(added)} -{len(removed)}', flush=True)
        print('   +', ' '.join(added)); print('   -', ' '.join(removed))
    prev = (ver, t, words)
(outdir / f'novel-words-{binname}.json').write_text(json.dumps(res, indent=1))

# ---- second stage: glue explanation -------------------------------------
# A lowercase glued run ("databaseinsert") is still flagged above. Build an
# atom dictionary from the OLD build's delimited words (a word bounded by
# non-lowercase on both sides, e.g. prose "database" / snake "insert") and
# DP-segment each flagged word into atoms (ends may be truncated: first piece
# = any old-text substring, last piece = any old-text substring, >= 2 chars).
# Words that segment are re-glue; the rest are reported as NOVEL.
def atoms_of(t):
    return {m.group().lower() for m in re.finditer(r'(?<![a-z])[A-Za-z]?[a-z]{2,}(?![a-z])', t) if len(m.group()) >= 3}

def explained(w, atoms, oldtext):
    # pieces: atoms (>= 3 chars) anywhere; truncated ends (>= 4 chars, any old
    # substring); at most 4 pieces; never the whole word as one piece.
    s = w.lower(); n = len(s); INF = 99
    best = [INF] * (n + 1); best[0] = 0
    for i in range(n):
        if best[i] >= 4: continue
        for j in range(i + 3, n + 1):
            if i == 0 and j == n: continue
            piece = s[i:j]
            if piece in atoms or ((i == 0 or j == n) and j - i >= 4 and piece in oldtext):
                best[j] = min(best[j], best[i] + 1)
    return best[n] <= 4

final = []
prev = None
for ver, d in chain:
    t = text(Path(d) / binname)
    if prev:
        pv, pt = prev
        rec = next(r for r in res if r['to'] == ver)
        atoms_old, atoms_new = atoms_of(pt), atoms_of(t)
        lo, ln = pt.lower(), t.lower()
        nov_add = {w: c for w, c in rec['added'].items() if not explained(w, atoms_old, lo)}
        nov_rem = {w: c for w, c in rec['removed'].items() if not explained(w, atoms_new, ln)}
        final.append({'from': pv, 'to': ver, 'novel_added': nov_add, 'novel_removed': nov_rem})
        print(f'NOVEL {pv} -> {ver} {binname}: +{len(nov_add)} -{len(nov_rem)}')
        print('   +', ' '.join(nov_add)); print('   -', ' '.join(nov_rem))
    prev = (ver, t)
(outdir / f'novel-words-{binname}-final.json').write_text(json.dumps(final, indent=1))
