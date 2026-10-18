#!/usr/bin/env python3
"""wf lane: per-token occurrence counts in BOTH bundles for the workflow vocabulary,
plus context attribution for workflow-ish tokens (Trap 2 + Trap 3)."""
import re, collections, json
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')

def occ(text,tok):  # raw substring occurrences
    return text.count(tok)
def qocc(text,tok):  # quoted-literal occurrences
    return sum(text.count(q+tok+q) for q in ('"',"'","`"))

print("### wf_methods (quoted _kiro/workflow/* ) quoted-occurrence old -> new")
meth=re.compile(r'["\'`](_kiro/workflow/[A-Za-z0-9_/]+)["\'`]')
s=sorted(set(m.group(1) for m in meth.finditer(old)) | set(m.group(1) for m in meth.finditer(new)))
for t in s:
    print(f"  {t:46} {qocc(old,t):3}->{qocc(new,t):3}")

print("\n### wf_agents (quoted wf-*)")
ag=re.compile(r'["\'`](wf-[a-z0-9][a-z0-9-]*)["\'`]')
s2=sorted(set(m.group(1) for m in ag.finditer(old)) | set(m.group(1) for m in ag.finditer(new)))
for t in s2:
    print(f"  {t:30} {qocc(old,t):3}->{qocc(new,t):3}")

print("\n### notify (_kiro/session/*)")
nt=re.compile(r'["\'`](_kiro/session/[A-Za-z0-9_/]+)["\'`]')
s3=sorted(set(m.group(1) for m in nt.finditer(old)) | set(m.group(1) for m in nt.finditer(new)))
for t in s3:
    print(f"  {t:40} {qocc(old,t):3}->{qocc(new,t):3}")

print("\n### workflow lifecycle / event literal candidates")
EV=re.compile(r'["\']((?:node|run|step|watch|workflow|recipe|iteration|branch|repeat|parallel|loop|port|dispatch|agent)_[a-z0-9]+(?:_[a-z0-9]+)*)["\']')
s4=sorted(set(m.group(1) for m in EV.finditer(old)) | set(m.group(1) for m in EV.finditer(new)))
for t in s4:
    print(f"  {t:46} {qocc(old,t):3}->{qocc(new,t):3}")

print("\n### renamed-literal probes for removed snake tokens")
for t in ('orchestrate_subagent','content_filter','inline_agent','stage_failure'):
    pat=re.compile(r'[\'"`]([a-z_]*'+re.escape(t.split('_')[-1])+r'[a-z_]*)["\'`]')
    cands=sorted(set(m.group(1) for m in pat.finditer(new)))
    print(f"  {t}: old_raw={occ(old,t)} new_raw={occ(new,t)} new_raw_variants={ {c:occ(new,c) for c in cands if occ(new,c) and occ(old,c)==0} }")

print("\n### context: consentRound (zod key region)")
for tag,text in (('OLD',old),('NEW',new)):
    for m in re.finditer(r'consentRound', text):
        a=max(0,m.start()-160);b=min(len(text),m.end()+200)
        print(f"  [{tag}] ...{text[a:b]}...")
