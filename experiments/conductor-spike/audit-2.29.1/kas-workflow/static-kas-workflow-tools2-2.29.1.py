#!/usr/bin/env python3
import re, textwrap
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')

def show(text,anchor,w=120,post=900,n=1):
    print(f"\n##### anchor={anchor!r}")
    c=0
    for m in re.finditer(re.escape(anchor),text):
        a=max(0,m.start()-w);b=min(len(text),m.start()+post)
        print(f"  @{m.start()}: "+textwrap.shorten(text[a:b],1400,placeholder=' …'))
        c+=1
        if c>=n: break

# save_workflow_definition schema var
print("### save_workflow_definition schema (Xtu new / _4c old)")
show(new,'Xtu=',100,900)
show(old,'_4c=',100,900)
print("\n### run_workflow schema (find its schema var near class)")
show(new,'static id="run_workflow"',100,700)
show(old,'static id="run_workflow"',100,700)

print("\n### bundled recipe registry list - OLD")
i=old.find('EKc='); 
for m in re.finditer(r'(\[\s*\[\s*"autoresearch")', old):
    a=m.start(); print("  OLD @",a,":",textwrap.shorten(old[a-80:a+400],640,placeholder=' …')); break
print("### bundled recipe registry list - NEW")
for m in re.finditer(r'\[\s*\[\s*"autoresearch"', new):
    a=m.start(); print("  NEW @",a,":",textwrap.shorten(new[a-80:a+400],640,placeholder=' …')); break
