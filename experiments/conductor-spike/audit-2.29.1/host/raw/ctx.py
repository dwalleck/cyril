#!/usr/bin/env python3
"""ctx.py <lit> <limit> <bin> [<bin> ...] -> every context window of <lit> in each bin, masked."""
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
def load(p):
    raw=Path(p).read_bytes(); d=bytearray(raw)
    for a,b in frames(raw): d[a:b]=b'\0'*(b-a)
    return bytes(d)
lit=sys.argv[1]; lim=int(sys.argv[2]); L=lit.encode()
for p in sys.argv[3:]:
    d=load(p); n=p.split('/')[-5]; k=0
    print(f'##### {n} (count={d.count(L)})')
    for m in re.finditer(re.escape(L), d):
        k+=1
        if k>lim: print('   ... (more)'); break
        s=d[max(0,m.start()-200):m.end()+200]
        print(f'  [{k}] '+re.sub(rb'[^\x20-\x7e]',b'.',s).decode('ascii','replace'))
