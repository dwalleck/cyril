#!/usr/bin/env python3
import re, textwrap
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')

def ctx(text,anchor,w=200,n=3):
    print(f"\n##### anchor={anchor!r}")
    c=0
    for m in re.finditer(re.escape(anchor),text):
        a=max(0,m.start()-w);b=min(len(text),m.end()+w)
        print(f"  @{m.start()}: "+textwrap.shorten(text[a:b],700,placeholder=' …'))
        c+=1
        if c>=n: print("   ...more"); break

print("### counts")
for t in ('static id="save_workflow_definition"','static id="run_workflow"','static id="inspect_workflow"',
          'save_workflow_definition','run_workflow','inspect_workflow','workflowPrompt','workflowPath',
          'generated://','bundled://','recipeRef','workflowRef','"include"','includedFrom'):
    print(f"  {t:44} {old.count(t):4}->{new.count(t):4}")

for a in ('static id="save_workflow_definition"','static id="run_workflow"'):
    ctx(new,a,220,2)
    ctx(old,a,220,2)
