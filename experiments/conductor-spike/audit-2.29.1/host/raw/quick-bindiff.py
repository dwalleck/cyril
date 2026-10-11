import sys, hashlib
from pathlib import Path
a=Path(sys.argv[1]).read_bytes(); b=Path(sys.argv[2]).read_bytes()
n=min(len(a),len(b))
# common prefix
i=0
while i<n and a[i]==b[i]: i+=1
# common suffix
j=0
while j<n-i and a[len(a)-1-j]==b[len(b)-1-j]: j+=1
print(f"{sys.argv[1]} {len(a)} vs {sys.argv[2]} {len(b)} delta {len(b)-len(a):+d}")
print(f"common prefix {i} bytes; common suffix {j} bytes")
mid_a=a[i:len(a)-j]; mid_b=b[i:len(b)-j]
print(f"middle old {len(mid_a)} new {len(mid_b)}")
# rolling sha of 512-byte blocks to see structure
for name,mid,base in (('OLD',mid_a,i),('NEW',mid_b,i)):
    blocks=[hashlib.sha256(mid[k:k+512]).hexdigest()[:8] for k in range(0,len(mid),512)]
    print(name,'blocks',len(blocks))
    # find first/last differing block by index comparison
open(sys.argv[3],'wb').write(mid_a)
open(sys.argv[4],'wb').write(mid_b)
