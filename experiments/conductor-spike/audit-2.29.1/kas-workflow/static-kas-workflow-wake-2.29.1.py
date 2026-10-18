#!/usr/bin/env python3
import re,textwrap
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')
print("### counts OLD->NEW")
for t in ('wakePolicy','pendingWakePrompts','sendParentNote','parent_note','runtime note','workflow-note',
          'network-give-up','network-waiting','deliverRuntimeNoteToParent','Tme(','noteUserRequests',
          'workflowCompletionWake','autoWake','wakeParent','run_complete','runCompleted','suppressWake'):
    print(f"  {t:32} {old.count(t):4}->{new.count(t):4}")
def find(text,a,w=200,n=3):
    print(f"\n##### {a!r}")
    c=0
    for m in re.finditer(re.escape(a),text):
        print("  @%d: %s"%(m.start(),textwrap.shorten(text[max(0,m.start()-w):m.end()+w],520,placeholder=' …')))
        c+=1
        if c>=n: print("   ...more"); break
for a in ('sendParentNote','wakePolicy','pendingWakePrompts'):
    find(new,a,220,3)
