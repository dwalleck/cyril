#!/usr/bin/env python3
import re,textwrap
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')
print("### counts OLD->NEW")
for t in ('subscribe','unsubscribe','attached','attachment','workflowSubscription','onWorkflowEvent','notifySubscribers',
          'broadcastOutboundFor','replayState','catchUp','catch-up','readPath','reconcile'):
    print(f"  {t:24} {old.count(t):4}->{new.count(t):4}")
def find(text,a,w=300,n=3):
    print(f"\n##### {a!r}")
    c=0
    for m in re.finditer(re.escape(a),text):
        print("  @%d: %s"%(m.start(),textwrap.shorten(text[max(0,m.start()-w):m.end()+350],700,placeholder=' …')))
        c+=1
        if c>=n: print("   ...more"); break
find(new,'"subscribeWorkflow',300,2)
find(new,'workflowSubscription',300,2)
# look at the persistence classification switch
find(new,'case"_kiro/workflow/load"',200,2)
