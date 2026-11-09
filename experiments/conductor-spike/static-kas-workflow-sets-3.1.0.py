#!/usr/bin/env python3
"""Vocabulary-set census of the KAS workflow engine across carves (trap 3).

    python3 -I static-kas-workflow-sets-3.1.0.py OUTDIR VER [VER ...]

For every carve ~/.local/share/kiro-research/kas-carves/<VER>/.../acp-server.js
extracts, by broad regex, the sets of
  * `_kiro/workflow/<name>` method literals (with counts),
  * workflow lifecycle event kinds (`kind:"<x>",payload:` and `kind:"<x>"` near workflow),
  * every `_kiro/<ns>/<name>` literal (to see whether workflow-adjacent methods moved),
  * `workflowPlan`-adjacent keys, `settings.workflows`-adjacent keys, `agentInitiatedReason` values,
  * zod enum lists that mention workflow vocabulary (`x.enum([...])`),
and writes one JSON per version plus a set-diff table OLD -> each later VER.
Executes nothing from Kiro.
"""
import json, os, re, sys
from collections import Counter
from pathlib import Path
K = Path(os.path.expanduser('~/.local/share/kiro-research/kas-carves'))
REL = 'node_modules/@kiro/agent/dist/server/acp-server.js'
out = Path(sys.argv[1]); out.mkdir(parents=True, exist_ok=True)
vers = sys.argv[2:]
PATTERNS = {
    'wf_methods': r'_kiro/workflow/[A-Za-z_]+',
    'kiro_methods': r'_kiro/[A-Za-z_]+/[A-Za-z_]+',
    'event_kinds_payload': r'kind:"([a-z_]+)",payload:',
    'agent_initiated': r'agentInitiatedReason:"([a-z_-]+)"',
    'agent_initiated_any': r'"([a-z-]+-wake|[a-z-]+-note|wake-[a-z-]+)"',
    'workflow_meta_keys': r'\b(workflowPlan|workflowId|workflowRef|workflowPrompt|workflowPath|workflowsEnabled|workflowUse|workflowAutoStart|workflowStart|runLabel|parentSessionId|rootConversationId|planRevision|keptStepIds|remainingSteps|pendingSteps|extendRepeat|capturePinned|attention|consentRound|rejectionReason|suggestedChanges|suggestion|planSuggestion|stepCredits|credits|creditsUsed|creditUsage|usageCredits|worktree|worktreePath|branch|baseBranch|checkoutBranch|workingBranch)\b',
    'settings_keys_near_workflows': r'workflows:[a-zA-Z$_]*\.?[a-zA-Z$_(]*',
    'zod_enum_lists': r'\.enum\(\[((?:"[A-Za-z_-]+",?)+)\]\)',
    'bundled_uri': r'bundled://[A-Za-z0-9_-]+',
    'generated_uri': r'generated://[A-Za-z0-9_<>-]+',
    'agent_uri': r'agent://[A-Za-z0-9_-]+',
    'wf_agent_names': r'"wf-[a-z-]+"',
    'json_files': r'"[a-z-]+\.json"',
    'md_files': r'"[a-z-]+(?:-r\$\{[^}]+\})?\.md"',
    'kiro_env': r'KIRO_[A-Z0-9_]+',
    'ab_flags': r'AB_[A-Z0-9_]+',
}
res = {}
for v in vers:
    p = K / v / REL
    s = p.read_text(encoding='utf-8', errors='replace')
    r = {}
    for name, pat in PATTERNS.items():
        c = Counter()
        for m in re.finditer(pat, s):
            c[m.group(1) if m.groups() else m.group(0)] += 1
        r[name] = dict(sorted(c.items()))
    res[v] = r
    (out / f'sets-{v}.json').write_text(json.dumps(r, indent=1))
    print(v, {k: len(x) for k, x in r.items()})
old = vers[0]
lines = []
for name in PATTERNS:
    for v in vers[1:]:
        a, b = res[old][name], res[v][name]
        add = sorted(set(b) - set(a)); rem = sorted(set(a) - set(b))
        chg = sorted(k for k in set(a) & set(b) if a[k] != b[k])
        if add or rem or chg:
            lines.append(f'## {name}: {old} -> {v}')
            for k in add: lines.append(f'  + {k!r} (0 -> {b[k]})')
            for k in rem: lines.append(f'  - {k!r} ({a[k]} -> 0)')
            for k in chg: lines.append(f'  ~ {k!r} ({a[k]} -> {b[k]})')
(out / 'set-diff.txt').write_text('\n'.join(lines) + '\n')
print('wrote', out / 'set-diff.txt', len(lines), 'lines')
