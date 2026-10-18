#!/usr/bin/env python3
import re
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')

def dump(text,anchor,tag,w=240,n=4):
    print(f"\n##### [{tag}] anchor={anchor!r}")
    for i,m in enumerate(re.finditer(re.escape(anchor),text)):
        if i>=n: print("   ...more"); break
        a=max(0,m.start()-w);b=min(len(text),m.end()+w)
        print(f"  @{m.start()} ...{text[a:b]}...")

# 1) find raw occurrences of _kiro/workflow/ that are NOT exact quoted literals (the +3 OLD)
qpat=re.compile(r'["\'`]_kiro/workflow/[A-Za-z0-9_/]+["\'`]')
for tag,text in (('OLD',old),('NEW',new)):
    quoted={m.start() for m in qpat.finditer(text)}
    raw=[m.start() for m in re.finditer(r'_kiro/workflow/',text)]
    extra=[o for o in raw if o not in quoted]
    print(f"[{tag}] raw={len(raw)} quoted={len(quoted)} extra={len(extra)}")
    for o in extra[:8]:
        print("   ..."+text[max(0,o-200):o+160]+"...")

for anchor in ('literal("repeat")','literal("watch")','literal("parallel")','onMaxIterations','joinPolicy'):
    dump(new,anchor,'NEW',260,3)
dump(old,'onMaxIterations','OLD',260,2)
