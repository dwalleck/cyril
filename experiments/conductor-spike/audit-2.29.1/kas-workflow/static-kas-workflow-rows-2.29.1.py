#!/usr/bin/env python3
import re, textwrap
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')

def allctx(text,anchor,w=200,n=8):
    print(f"\n##### anchor={anchor!r}")
    c=0
    for m in re.finditer(re.escape(anchor),text):
        print(f"  @{m.start()}: "+textwrap.shorten(text[max(0,m.start()-w):m.end()+w],440,placeholder=' …'))
        c+=1
        if c>=n: print("   ...more"); break

allctx(new,'validationError:',220,6)
print("\n### listRecipes reply build (search 'recipes:' with builtIn/plan)")
for m in re.finditer(r'builtIn[:=]', new):
    seg=new[max(0,m.start()-260):m.end()+260]
    if 'valid' in seg or 'plan' in seg or 'recipe' in seg.lower():
        print(f"  NEW @{m.start()}: "+textwrap.shorten(seg,520,placeholder=' …'))
print("\n### runLabel / planRevision on _kiro/workflow/new")
for a in ('runLabel','planRevision'):
    allctx(new,a,160,4)
