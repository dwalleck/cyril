#!/usr/bin/env python3
import re,textwrap
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')
def find(text,a,w=260,n=3,post=None):
    print(f"\n##### {a!r}")
    c=0
    for m in re.finditer(re.escape(a),text):
        b=m.start()+post if post else m.end()+w
        print("  @%d: %s"%(m.start(),textwrap.shorten(text[max(0,m.start()-w):b],640,placeholder=' …')))
        c+=1
        if c>=n: print("   ...more"); break
print("### OLD request_permission meta consentRound (baseline)")
for m in re.finditer('consentRound', old):
    print("  @%d: ...%s..."%(m.start(), old[max(0,m.start()-220):m.end()+120]))
print("\n### gate: workflows setting read")
for tag,text in (('OLD',old),('NEW',new)):
    for m in re.finditer(r'workflows\?\.enabled|workflows\.enabled|"workflows"|workflowsSetting|enabled\).*workflow', text):
        seg=text[max(0,m.start()-120):m.end()+120]
        if 'workflow' in seg.lower():
            print(f"  [{tag}] @{m.start()}: "+textwrap.shorten(seg,300,placeholder=' …'))
            break
print("\n### counts gate")
for t in ('workflows?.enabled','workflows.enabled','"workflows"','_meta.workflowsEnabled','workflowsEnabled','workflowToolsEnabled'):
    print(f"  {t:28} {old.count(t):4}->{new.count(t):4}")
print("\n### extensionMethods listing of workflow methods")
for tag,text in (('OLD',old),('NEW',new)):
    i=text.find('extensionMethods')
    print(f"  [{tag}] first extensionMethods @{i}")
    j=text.find('_kiro/workflow',i)
    # find the array containing workflow methods
    k=text.find('workflowNew:',i)
print("\n### load vs inspect subscription")
for tag,text in (('OLD',old),('NEW',new)):
    for a in ('_kiro/workflow/load','_kiro/workflow/inspect'):
        for m in re.finditer(re.escape(a),text):
            print(f"  [{tag}] {a} @{m.start()}: "+textwrap.shorten(text[max(0,m.start()-180):m.end()+180],400,placeholder=' …'))
            break
