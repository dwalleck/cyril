#!/usr/bin/env python3
import re, textwrap
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')

def ctx(text,anchor,w=340,n=4):
    print(f"\n##### anchor={anchor!r}")
    c=0
    for m in re.finditer(re.escape(anchor),text):
        a=max(0,m.start()-w);b=min(len(text),m.end()+w)
        print(f"  @{m.start()}: "+textwrap.shorten(text[a:b],660,placeholder=' …'))
        c+=1
        if c>=n: print("   ...more"); break

for a in ('function mxo','bundledSource','generatedBaseDir','inForce(','recipeCatalog='):
    ctx(new,a,320,4)
print("\n### flag/env mentions of include")
for t in ('include_enabled','includeEnabled','recipeIncludeEnabled','KIRO_FEATURE_WORKFLOW_INCLUDE','workflow_include',
          'workflowInclude','includesEnabled','_include','includeDepth'):
    print(f"  {t:34} {old.count(t):4}->{new.count(t):4}")
