#!/usr/bin/env python3
"""wf lane: locate the workflow region(s) and diff the literal sets inside them."""
import re, json
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')

def spread(text, pat):
    offs=[m.start() for m in re.finditer(pat,text)]
    return len(offs), (min(offs) if offs else None), (max(offs) if offs else None)

for tag,text in (('OLD',old),('NEW',new)):
    n,a,b=spread(text,'_kiro/workflow/')
    print(f"{tag}: _kiro/workflow/ count={n} off=[{a}..{b}] span={b-a if a is not None else None} total={len(text)}")
    for anchor in ('wf-workflow-creator','wf-planner','_kiro/workflow/run_complete','recipe','WorkflowNode','joinPolicy','onMaxIterations','listWatchHandlers'):
        n2,a2,b2=spread(text,re.escape(anchor))
        print(f"    {anchor:26} n={n2:5} off=[{a2}..{b2}]")

# raw counts for workflow-relevant literal keys
print("\n### raw substring counts (OLD -> NEW)")
for t in ('consentRound','allow_always','allowAlways','extendRepeat','onMaxIterations','joinPolicy','idleTimeoutSec',
          'injectOriginalUserRequest','planRevision','captureOutput','backgroundExecution','memoryConfigSource',
          'workflowNotifications','recipes_changed','watch_poll','agent_note','agent_notes','step.parent_note',
          'parent_note','workflow.step','sendParentNote','run_workflow','save_workflow_definition','validate_workflow',
          'creator_override_refused','runLabel','listRecipes','listWatchHandlers','background-process','github-pr',
          'crux-cr','workflowPrompt','original_user_request','workflowCreator','pause','resumeAll'):
    print(f"  {t:28} {old.count(t):4}->{new.count(t):4}")
