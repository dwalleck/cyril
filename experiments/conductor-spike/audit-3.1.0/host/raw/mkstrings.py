#!/usr/bin/env python3
"""Write zstd-frame-masked printable strings (>=4) of each host binary, one per line (3.1.0 audit).
usage: python3 -I mkstrings.py <outdir> <ver:bindir> ...
Every file >1 MiB in bindir is processed (2.x: kiro-cli, kiro-cli-chat, kiro-cli-term; 3.x: kiro-cli only).
Frames >50 KB are zeroed so KAS tar / Node / Bun / TUI text never counts (trap 9).
"""
import re, sys
from pathlib import Path
import zstandard
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
out = Path(sys.argv[1])
for a in sys.argv[2:]:
    v, dp = a.split(':', 1)
    (out / v).mkdir(parents=True, exist_ok=True)
    for bp in sorted(Path(dp).iterdir()):
        if not bp.is_file() or bp.stat().st_size < (1 << 20): continue
        b = bp.name
        d = bytearray(bp.read_bytes())
        fr = frames(bytes(d))
        for x, y in fr: d[x:y] = b'\0' * (y - x)
        n = 0
        with open(out / v / (b + '.txt'), 'wb') as f:
            for m in PRINT.finditer(d):
                f.write(b'%d\t' % m.start() + m.group() + b'\n'); n += 1
        print(v, b, bp.stat().st_size, 'frames', [(x, y - x) for x, y in fr], 'strings', n, flush=True)
print('MKSTRINGS-DONE')
