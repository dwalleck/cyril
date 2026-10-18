#!/usr/bin/env python3
import re, textwrap
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')

print("### counts")
for t in ('kiro-workflow-authoring','validationError','workflow.include.resolved','workflow.include.warnings',
          'workflow.include','"plan"','isPlan','includeCount','WorkflowIncludeExpansionError','agent_note',
          'run_workflow','listRecipes','"builtIn"','builtIn','"source"','"inputs"','not available in this session'):
    print(f"  {t:34} {old.count(t):4}->{new.count(t):4}")

print("\n### NEW doc: 'include' node documentation in skill text?")
for m in re.finditer(r'include', new):
    seg=new[max(0,m.start()-90):m.start()+90]
    if '\\n' in new[max(0,m.start()-200):m.start()+200] and ('type' in seg or 'node' in seg.lower() or 'include' in seg):
        pass
# targeted: search for the workflow-authoring doc block and print headings
i=new.find('kiro-workflow-authoring')
print("  kiro-workflow-authoring at",i)

print("\n### agent_note contexts NEW")
for m in re.finditer('agent_note', new):
    print("  @%d: %s"%(m.start(), textwrap.shorten(new[max(0,m.start()-140):m.end()+140],300,placeholder=' …')))
print("\n### listRecipes row build NEW")
for m in re.finditer('listRecipes', new):
    seg=new[max(0,m.start()-100):m.end()+220]
    if 'validationError' in seg or 'builtIn' in seg or 'source' in seg:
        print("  @%d: %s"%(m.start(), textwrap.shorten(seg,420,placeholder=' …')))
        break
i=new.find('validationError')
print("  validationError @",i, textwrap.shorten(new[max(0,i-250):i+250],480,placeholder=' …') if i>=0 else '')
