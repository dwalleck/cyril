#!/usr/bin/env python3
"""wf lane: where is the include expansion called and how is the resolver wired? Also reachability."""
import re, textwrap
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')

def dump(text,anchor,tag,w=300,n=6):
    print(f"\n##### [{tag}] anchor={anchor!r}")
    c=0
    for m in re.finditer(re.escape(anchor),text):
        a=max(0,m.start()-w);b=min(len(text),m.end()+w)
        print(f"  @{m.start()}: "+textwrap.shorten(text[a:b],520,placeholder=' …'))
        c+=1
        if c>=n: print("   ...more"); break

print("### counts OLD->NEW")
for t in ('sourceLocation','Ube','xae(','F6r(','g1t(','resolve:','resolve(o)','workflowRecipes','recipeResolver',
          'resolveWorkflowInclude','includeResolver','includedFrom','WorkflowIncludeExpansionError'):
    print(f"  {t:34} {old.count(t):4}->{new.count(t):4}")

for a in ('xae(','g1t(','sourceLocation'):
    dump(new,a,'NEW',260,6)
