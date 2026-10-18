#!/usr/bin/env python3
"""litcmp.py <litfile> <binA> <binB> [...more bins]
Prints per literal: counts in each bin (masked: zstd payload frames removed) + up to 2 context
windows from the FIRST bin whose count>0 and from the first bin where it differs."""
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
lits=[l.strip() for l in open(sys.argv[1]) if l.strip() and not l.startswith('#')]
bins=[(p.split('/')[-1]+'@'+p.split('/')[-5], load(p)) for p in sys.argv[2:]]
for lit in lits:
    if lit.startswith('HEX:'): L=bytes.fromhex(lit[4:]); disp=lit
    else: L=lit.encode(); disp=repr(lit)
    counts=[d.count(L) for _,d in bins]
    if all(c==0 for c in counts): continue
    print(f'### {disp} :: ' + '  '.join(f'{n}={c}' for (n,_),c in zip(bins,counts)))
    shown=0
    for (n,d),c in zip(bins,counts):
        if c==0: continue
        if shown>=2: break
        s=d[max(0,d.find(L)-140):d.find(L)+len(L)+140]
        print('   ['+n+'] ...'+re.sub(rb'[^\x20-\x7e]',b'.',s).decode('ascii','replace')+'...')
        shown+=1
