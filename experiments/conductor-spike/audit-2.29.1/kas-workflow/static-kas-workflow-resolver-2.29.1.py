#!/usr/bin/env python3
import re, textwrap
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')

print("### counts OLD->NEW")
for t in ('includeResolver','recipeInForce','autoresearch','feature-pipeline','"goal"','investigate','publish-pr',
          'ralph','semantic-review-multi-model','recipeFs','inForce','workflow.include.warnings','workflow.include',
          'WorkflowInclude','kIt(','T1t(','fwo(','agentLookup'):
    print(f"  {t:30} {old.count(t):4}->{new.count(t):4}")

print("\n### NEW: all includeResolver occurrences")
for m in re.finditer('includeResolver', new):
    a=max(0,m.start()-300);b=min(len(new),m.end()+300)
    print(f"  @{m.start()}: "+textwrap.shorten(new[a:b],560,placeholder=' …'))

print("\n### bundled recipe registry OLD")
i=old.find('autoresearch')
for m in re.finditer('autoresearch', old):
    a=max(0,m.start()-200);b=min(len(old),m.end()+420)
    print(f"  @{m.start()}: "+textwrap.shorten(old[a:b],600,placeholder=' …'))
    break
print("\n### bundled recipe registry NEW")
for m in re.finditer('autoresearch', new):
    a=max(0,m.start()-200);b=min(len(new),m.end()+420)
    print(f"  @{m.start()}: "+textwrap.shorten(new[a:b],600,placeholder=' …'))
    break
