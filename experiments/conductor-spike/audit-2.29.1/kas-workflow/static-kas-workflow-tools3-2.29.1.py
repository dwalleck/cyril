#!/usr/bin/env python3
import re, textwrap
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')

def show(text,anchor,w=0,post=900,n=1):
    print(f"\n##### anchor={anchor!r}")
    c=0
    for m in re.finditer(re.escape(anchor),text):
        a=max(0,m.start()-w);b=min(len(text),m.start()+post)
        print(f"  @{m.start()}: {text[a:b]}")
        c+=1
        if c>=n: break

show(new,'Itu=k.object(',0,1200); show(old,'t4c=k.object(',0,1200)
print("\n### injectOriginalUserRequest contexts")
for tag,text in (('OLD',old),('NEW',new)):
    for m in re.finditer('injectOriginalUserRequest',text):
        a=max(0,m.start()-160);b=min(len(text),m.end()+120)
        print(f"  [{tag}] @{m.start()}: "+textwrap.shorten(text[a:b],420,placeholder=' …'))
print("\n### _kiro/workflow/new handler param region")
for m in re.finditer(r'"workflowPath"', new):
    a=max(0,m.start()-500); print("  NEW:", textwrap.shorten(new[a:m.end()+400],780,placeholder=' …'))
