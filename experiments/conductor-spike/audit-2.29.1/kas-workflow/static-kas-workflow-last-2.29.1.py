#!/usr/bin/env python3
import re,textwrap
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')
print("### counts")
for t in ('_kiro/session/notify','recipes_changed','"recipes"','includedFrom','"include"','workflow-continuation',
          'agent-initiated-prompt','steering-injection','"served"','"notEnabled"','bundled://','generated://',
          'agent://','wf-workflow-creator','includeCycle','include cycle'):
    print(f"  {t:26} {old.count(t):4}->{new.count(t):4}")
def find(text,a,w=200,n=2):
    print(f"\n##### {a!r}")
    c=0
    for m in re.finditer(re.escape(a),text):
        print("  @%d: %s"%(m.start(),textwrap.shorten(text[max(0,m.start()-w):m.end()+w],520,placeholder=' …')))
        c+=1
        if c>=n: print("  ...more"); break
find(new,'_kiro/session/notify',220,2)
find(new,'recipes_changed',200,2)
find(old,'_kiro/session/notify',220,2)
# include documented in agent prompts?
print("\n### 'include' near 'recipe' in NEW doc-text (3.2M-3.5M)")
for m in re.finditer('include', new[3200000:3500000]):
    seg=new[3200000+m.start()-110:3200000+m.start()+110]
    if 'recipe' in seg.lower() or 'node' in seg.lower() or 'type' in seg:
        print("  %s"%textwrap.shorten(seg,300,placeholder=' …'))
