#!/usr/bin/env python3
import re, textwrap
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')

def find(text,anchor,w=260,n=3):
    print(f"\n##### {anchor!r}")
    c=0
    for m in re.finditer(re.escape(anchor),text):
        print(f"  @{m.start()}: "+textwrap.shorten(text[max(0,m.start()-w):m.end()+w],560,placeholder=' …'))
        c+=1
        if c>=n: print("   ...more"); break

print("### OLD interaction_resolved / pending_interaction zod")
find(old,'interaction_resolved",toolCallId',200,2)
find(old,'pending_interaction",toolCallId',200,2)
print("\n### OLD 'not available in this session' / notInForce")
for t in ('not available in this session','notInForce','inForce','recipeInForce','includeResolver','generated:// includes'):
    print(f"  {t:34} {old.count(t):4}->{new.count(t):4}")
print("\n### Tau( callers NEW")
find(new,'Tau(',160,6)
print("\n### who calls sxo / passes includeResolver to _kiro/workflow/new handler")
find(new,'workflowNew:',260,2)
