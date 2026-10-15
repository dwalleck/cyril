#!/usr/bin/env python3
"""Direct printable-string SET diff + tracing source-location diff between two host builds (2.31.1 audit).

    python3 -I static-host-strdiff-2.29.1.py <outdir> <bindir-old> <bindir-new>

Built to answer "the three Rust binaries differ 2.29.0 vs 2.29.1 although BUILD_HASH is identical:
what actually differs?". For each of kiro-cli / kiro-cli-chat / kiro-cli-term:
  1. mask the large zstd payload frames (KAS tar, Node, Bun, TUI bundle; > 50 KB compressed) so only the Rust host counts;
  2. report the payload frame table (start offsets, sizes) for both builds -- a shifted start means the host code
     before the payload grew or shrank;
  3. extract every maximal printable run (>= 4 chars) as a SET and diff the two sets; each add/remove is re-checked
     as a raw substring of the other build's masked bytes (glue filter: LTO re-glue concatenates literals but cannot
     mint a new one). Survivors are the real string-level changes;
  4. extract every `crates/<crate>/src/<path>.rs:LINE` tracing location and diff by FILE: a file whose line set
     changed had its source edited (same BUILD_HASH or not), a file present in one build only is a module add/remove.
Writes <outdir>/strdiff-<bin>.json and prints a summary with the surviving strings.
"""
import json, re, sys
from pathlib import Path
import zstandard

BINS = ['kiro-cli', 'kiro-cli-chat', 'kiro-cli-term']
PRINT = re.compile(rb'[\x20-\x7e]{4,}')
SRCLOC = re.compile(rb'(crates/[A-Za-z0-9_-]+/src/[A-Za-z0-9_./-]+?\.rs):(\d+)')
VERS = re.compile(rb'\d+\.\d+\.\d+')

def frames(d):
    magic = b'\x28\xb5\x2f\xfd'; i = 0; out = []
    while True:
        i = d.find(magic, i)
        if i < 0: return out
        try:
            o = zstandard.ZstdDecompressor().decompressobj(); pos = i; total = 0
            while not o.eof and pos < len(d):
                chunk = d[pos:pos + (4 << 20)]; total += len(o.decompress(chunk)); pos += len(chunk)
            if o.eof:
                end = pos - len(o.unused_data)
                if end - i > 50000: out.append((i, end, total)); i = end; continue
        except zstandard.ZstdError: pass
        i += 4

def host(p):
    raw = Path(p).read_bytes(); d = bytearray(raw); fr = frames(raw)
    for a, b, _ in fr: d[a:b] = b'\0' * (b - a)
    return bytes(d), fr, len(raw)

outdir = Path(sys.argv[1]); outdir.mkdir(parents=True, exist_ok=True)
old, new = Path(sys.argv[2]), Path(sys.argv[3])
vo, vn = old.parts[-4], new.parts[-4]
for b in BINS:
    do, fo, so = host(old / b); dn, fn, sn = host(new / b)
    po = {m.group() for m in PRINT.finditer(do)}; pn = {m.group() for m in PRINT.finditer(dn)}
    added = sorted(pn - po); removed = sorted(po - pn)
    def classify(items, other):
        real, glue, ver = [], [], []
        for s in items:
            if VERS.search(s) and VERS.sub(b'X', s) in {VERS.sub(b'X', t) for t in [s]} and other.count(VERS.sub(b'', s)[:0] + s) == 0 and VERS.sub(b'', s) and any(VERS.sub(b'', s) == VERS.sub(b'', t) for t in (PRINT.findall(other[max(0, other.find(VERS.sub(b'', s)[:12]) - 200): other.find(VERS.sub(b'', s)[:12]) + 400]) if other.find(VERS.sub(b'', s)[:12]) >= 0 else [])):
                ver.append(s.decode()); continue
            if other.count(s) > 0: glue.append(s.decode())
            else: real.append(s.decode())
        return real, glue, ver
    ra, ga, va = classify(added, do); rr, gr, vr = classify(removed, dn)
    lo = {}; ln = {}
    for m in SRCLOC.finditer(do): lo.setdefault(m.group(1).decode(), set()).add(int(m.group(2)))
    for m in SRCLOC.finditer(dn): ln.setdefault(m.group(1).decode(), set()).add(int(m.group(2)))
    files_added = sorted(set(ln) - set(lo)); files_removed = sorted(set(lo) - set(ln))
    files_changed = {f: {'old_only': sorted(lo[f] - ln[f]), 'new_only': sorted(ln[f] - lo[f])} for f in sorted(set(lo) & set(ln)) if lo[f] != ln[f]}
    rec = {'bin': b, 'old': str(old / b), 'new': str(new / b), 'size': [so, sn],
           'frames_old': fo, 'frames_new': fn,
           'printable_runs': [len(po), len(pn)],
           'added_real': ra, 'added_glue': ga, 'added_version_only': va,
           'removed_real': rr, 'removed_glue': gr, 'removed_version_only': vr,
           'srcloc_files': [len(lo), len(ln)], 'srcloc_files_added': files_added, 'srcloc_files_removed': files_removed,
           'srcloc_files_changed': files_changed}
    (outdir / f'strdiff-{b}.json').write_text(json.dumps(rec, indent=1))
    print(f'===== {b}: {vo} -> {vn}  size {so} -> {sn} (delta {sn - so:+d})')
    print('  payload frames old:', [(a, e - a, t) for a, e, t in fo])
    print('  payload frames new:', [(a, e - a, t) for a, e, t in fn])
    print(f'  printable runs {len(po)} -> {len(pn)}; added {len(added)} (real {len(ra)}, glue {len(ga)}, version-only {len(va)}); removed {len(removed)} (real {len(rr)}, glue {len(gr)}, version-only {len(vr)})')
    print(f'  srcloc files {len(lo)} -> {len(ln)}; files added {files_added}; removed {files_removed}; line-set changed {len(files_changed)}')
    for f, v in files_changed.items(): print(f'    ~ {f}: old-only {v["old_only"][:12]} new-only {v["new_only"][:12]}')
    for s in ra: print('  + ' + s[:220])
    for s in rr: print('  - ' + s[:220])
