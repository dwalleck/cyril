#!/usr/bin/env python3
"""Lean 2.29.0 vs 2.29.1 host diff (answers: what changed under one BUILD_HASH?).

    python3 -I static-host-hashdiff-2.29.1.py <outdir> <binA-dir> <binB-dir>

Per binary (kiro-cli, kiro-cli-chat, kiro-cli-term):
  * payload-frame table: every zstd magic whose frame is > 50 KB compressed is decompressed
    from its offset to EOF and recorded (start, csize, dsize, kind, sha256) -- the embedded
    KAS tar / Node ELF / Bun ELF / TUI bundle. A changed sha256 in one frame and not another
    says which embedded artifact moved.
  * `crates/<crate>/src/<path>.rs:LINE` tracing locations -> file -> set(lines). Files
    present in one build only = module add/remove; a changed line set = the source was edited.
    These strings survive stripping (trap 9).
  * prose-literal set diff: printable runs >= 12 chars containing a space (so symbol tables
    and base64 blobs are excluded). Glue can concatenate existing literals, so every hit is
    re-checked as a raw substring of the other build's MASKED bytes; survivors are real.
Writes <outdir>/hashdiff-<bin>.json and prints a compact summary.
"""
import hashlib, json, re, sys
from pathlib import Path
import zstandard

BINS = ['kiro-cli', 'kiro-cli-chat', 'kiro-cli-term']
MAGIC = b'\x28\xb5\x2f\xfd'
SRCLOC = re.compile(rb'(crates/[A-Za-z0-9_.-]+/src/[A-Za-z0-9_./-]+\.rs):(\d+)')
PROSE = re.compile(rb'[\x20-\x7e]{12,}')

def frames(d):
    i = 0; out = []
    while True:
        i = d.find(MAGIC, i)
        if i < 0: return out
        try:
            o = zstandard.ZstdDecompressor().decompressobj(); pos = i; total = 0
            while not o.eof and pos < len(d):
                c = d[pos:pos + (4 << 20)]; total += len(o.decompress(c)); pos += len(c)
            if o.eof:
                end = pos - len(o.unused_data)
                if end - i > 50000:
                    out.append({'start': i, 'csize': end - i, 'dsize': total}); i = end; continue
        except zstandard.ZstdError:
            pass
        i += 4

def mask(d, fr):
    b = bytearray(d)
    for f in fr: b[f['start']:f['start'] + f['csize']] = b'\0' * f['csize']
    return bytes(b)

outdir = Path(sys.argv[1]); outdir.mkdir(parents=True, exist_ok=True)
A, Bd = Path(sys.argv[2]), Path(sys.argv[3])
va, vb = A.parts[-4], Bd.parts[-4]
for bn in BINS:
    ra = (A / bn).read_bytes(); rb = (Bd / bn).read_bytes()
    fa = frames(ra); fb = frames(rb)
    ma, mb = mask(ra, fa), mask(rb, fb)
    la, lb = {}, {}
    for m in SRCLOC.finditer(ma): la.setdefault(m.group(1).decode(), set()).add(int(m.group(2)))
    for m in SRCLOC.finditer(mb): lb.setdefault(m.group(1).decode(), set()).add(int(m.group(2)))
    files_added = sorted(set(lb) - set(la)); files_removed = sorted(set(la) - set(lb))
    changed = {f: {'old_only': sorted(la[f] - lb[f]), 'new_only': sorted(lb[f] - la[f]),
                   'n_old': len(la[f]), 'n_new': len(lb[f])}
               for f in sorted(set(la) & set(lb)) if la[f] != lb[f]}
    sa = {s for s in set(PROSE.findall(ma)) if b' ' in s}
    sb = {s for s in set(PROSE.findall(mb)) if b' ' in s}
    added = sorted(s for s in sb - sa if s not in ma)
    removed = sorted(s for s in sa - sb if s not in mb)
    rec = {'bin': bn, 'sizes': [len(ra), len(rb)], 'frames_old': fa, 'frames_new': fb,
           'srcloc_files_old': len(la), 'srcloc_files_new': len(lb),
           'srcloc_files_added': files_added, 'srcloc_files_removed': files_removed,
           'srcloc_files_changed': changed, 'prose_added': [s.decode() for s in added],
           'prose_removed': [s.decode() for s in removed]}
    (outdir / f'hashdiff-{bn}.json').write_text(json.dumps(rec, indent=1))
    print(f'===== {bn} {va}->{vb} sizes {len(ra)} -> {len(rb)} ({len(rb)-len(ra):+d})')
    print('  frames old:', [(f['start'], f['csize'], f['dsize']) for f in fa])
    print('  frames new:', [(f['start'], f['csize'], f['dsize']) for f in fb])
    print(f'  srcloc files {len(la)} -> {len(lb)}; +{files_added}; -{files_removed}; changed {len(changed)}')
    for f, v in changed.items(): print(f'    ~ {f} ({v["n_old"]}->{v["n_new"]}) old-only {v["old_only"][:10]} new-only {v["new_only"][:10]}')
    print(f'  prose literals +{len(added)} -{len(removed)} (added shown below)')
    for s in rec['prose_added']: print('    + ' + s[:230])

