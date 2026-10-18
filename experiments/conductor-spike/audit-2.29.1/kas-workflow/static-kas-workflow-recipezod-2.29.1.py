#!/usr/bin/env python3
"""wf lane: dump the recipe zod-schema region (node defs + top-level recipe object) in both bundles."""
import re
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')

def region(text, anchor, pre, post):
    i=text.find(anchor)
    if i<0: return f"ANCHOR {anchor!r} NOT FOUND"
    return text[max(0,i-pre):i+post]

for tag,text,anchor in (('OLD',old,'type:k.literal("repeat")'),('NEW',new,'type:k.literal("repeat")')):
    print(f"\n================ {tag} recipe-node zod region (anchor literal(\"repeat\")) ================")
    print(region(text, anchor, 900, 2600))

for tag,text in (('OLD',old),('NEW',new)):
    print(f"\n================ {tag} top-level recipe object (anchor name:k.string(),description) ================")
    print(region(text, 'name:k.string(),description:k.string().optional()', 120, 900))

for tag,text in (('OLD',old),('NEW',new)):
    print(f"\n================ {tag} 'includedFrom' occurrences ================")
    for m in re.finditer('includedFrom', text):
        print(f"  @{m.start()}: ...{text[max(0,m.start()-260):m.start()+160]}...")
