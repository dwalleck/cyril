#!/usr/bin/env python3
import re
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')

def sdiff(pat,label):
    co=set(re.findall(pat,old)); cn=set(re.findall(pat,new))
    print(f"\n== {label}: {len(co)}->{len(cn)}  +{sorted(cn-co)}  -{sorted(co-cn)}")

sdiff(r'id:"(github-pr|command|background-process|[\w-]+)"', 'watch-handler id: literals')
sdiff(r'k\.enum\(\[([^\]]*)\]\)', 'zod enums (raw)')
# onMaxIterations / completionSignal enums
for a in ('onMaxIterations:','completionSignal:','joinPolicy:','stopCondition:'):
    print(f"\n-- {a}")
    for tag,text in (('OLD',old),('NEW',new)):
        for m in re.finditer(re.escape(a),text):
            print(f"   [{tag}] {text[m.start():m.start()+90]}")
            break
# watch handler registry
print("\n### watch handler registry")
for tag,text in (('OLD',old),('NEW',new)):
    i=text.find('listWatchHandlers')
    for m in re.finditer(r'id:"(github-pr|command|background-process)"',text):
        print(f"   [{tag}] @{m.start()}: {text[max(0,m.start()-120):m.end()+120]}")
        break
# settings keys workflows/goal
sdiff(r'"(workflows|goal|workflowNotifications|verifyFirstWorkflow)":', 'workflow settings keys')
print("\n### counts")
for t in ('"workflows"','"goal"','verifyFirstWorkflow','workflowNotifications','listWatchHandlers','"github-pr"','"command"','"background-process"','noteUserRequests','summaryUserRequests','summaryNoteTag'):
    print(f"  {t:30} {old.count(t):4}->{new.count(t):4}")
