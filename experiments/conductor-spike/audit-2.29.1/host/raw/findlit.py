#!/usr/bin/env python3
"""findlit.py <literal> [<literal> ...] reads binaries on stdin as stdin paths? -> usage:
   findlit.py <bin> <lit> [<lit> ...]
Prints count + up to 3 context windows (160 chars) per literal, payload frames masked."""
import sys, re
from pathlib import Path
import zstandard
MAGIC=b'\x28\xb5\x2f\xfd'
def frames(d):
    i=0; out=[]
    while True:
        i=d.find(MAGIC,i)
        if i<0: return out
        try:
            o=zstandard.ZstdDecompressor().decompressobj(); pos=i
            while not o.eof and pos<len(d):
                c=d[pos:pos+(4<<20)]; o.decompress(c); pos+=len(c)
            if o.eof:
                end=pos-len(o.unused_data)
                if end-i>50000: out.append((i,end)); i=end; continue
        except zstandard.ZstdError: pass
        i+=4
p=Path(sys.argv[1]); raw=p.read_bytes(); d=bytearray(raw)
for a,b in frames(raw): d[a:b]=b'\0'*(b-a)
d=bytes(d)
for lit in sys.argv[2:]:
    L=lit.encode()
    if lit.startswith('HEX:'):
        L=bytes.fromhex(lit[4:])
    n=d.count(L)
    print(f'### {lit!r} count={n} (raw_count={raw.count(L)})')
    for k,m in enumerate(re.finditer(re.escape(L), d)):
        if k>=3: break
        s=d[max(0,m.start()-100):m.end()+100]
        print('   ...'+re.sub(rb'[^\x20-\x7e]',b'.',s).decode('ascii',errors='replace')+'...')
