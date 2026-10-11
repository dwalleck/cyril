import hashlib,re,sys
from pathlib import Path
B=Path.home()/'.local/share/kiro-research/binaries'
a=(B/'2.29.0/extracted/kirocli/bin/kiro-cli-chat').read_bytes()
b=(B/'2.29.1/extracted/kirocli/bin/kiro-cli-chat').read_bytes()
A=a[98765712:]; Bo=b[98770012:]
BS=32
idx={}
for i in range(0,len(Bo)-BS,8):
    idx.setdefault(hashlib.blake2b(Bo[i:i+BS],digest_size=8).digest(),i)
deltas=[]; prev=None; hits=0; miss=0
i=0
while i<len(A)-BS:
    j=idx.get(hashlib.blake2b(A[i:i+BS],digest_size=8).digest())
    if j is None:
        miss+=1; i+=8; continue
    hits+=1
    d=j-i
    if prev is not None and d!=prev: deltas.append((i,prev,d,d-prev))
    prev=d; i+=8
print('hits',hits,'misses',miss,'transitions',len(deltas))
tot=0
for o,pl,dl,jp in deltas:
    if abs(jp)<8: continue
    tot+=jp
    print(f'  OLD@{o} map {pl}->{dl} jump {jp:+d}')
    print('    OLD:',re.sub(rb'[^\x20-\x7e]',b'.',A[max(0,o-100):o+100]).decode())
    print('    NEW:',re.sub(rb'[^\x20-\x7e]',b'.',Bo[max(0,o+pl-100):o+dl+100]).decode())
print('sum of jumps',tot)
