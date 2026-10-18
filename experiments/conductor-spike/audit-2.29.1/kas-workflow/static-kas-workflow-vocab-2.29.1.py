#!/usr/bin/env python3
"""kas-workflow lane: workflow vocabulary SET diff, 0.66.26 (2.28.0) vs 0.66.33 (2.29.1).

Trap 3: diff vocabulary SETS over a broad pattern, never recount a fixed list.
Trap 2: every added/removed token is verified by raw occurrence count in BOTH bundles,
plus <=200 chars of surrounding context from the bundle where it occurs.

Categories:
  wf_methods   every quoted "_kiro/workflow/..." literal
  notify       "_kiro/session/notify" and other _kiro/session/* notify-ish literals
  wf_agents    every quoted "wf-*" agent id
  snake_lits   every quoted snake_case literal (workflow event / state / log vocabulary)
  zod_keys     object keys whose value starts a zod-ish builder
  meta_keys    _meta.kiro.<X> / _meta?.kiro?.<X> reached keys
Usage: python3 -I static-kas-workflow-vocab-2.29.1.py
"""
import json, re, collections
from pathlib import Path

OLD = Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
NEW = Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
OUT = Path("raw-vocab-diff.json")

PAT = {
 'wf_methods': re.compile(r'["\'`](_kiro/workflow/[A-Za-z0-9_/]+)["\'`]'),
 'notify':     re.compile(r'["\'`](_kiro/session/[A-Za-z0-9_/]+)["\'`]'),
 'wf_agents':  re.compile(r'["\'`](wf-[a-z0-9][a-z0-9-]*)["\'`]'),
 'snake_lits': re.compile(r'["\'`]([a-z][a-z0-9]*(?:_[a-z0-9]+){1,4})["\'`]'),
 'zod_keys':   re.compile(r'[{,]([A-Za-z_$][A-Za-z0-9_$]*):[A-Za-z_$][A-Za-z0-9_$]*\.(?:string|number|boolean|object|array|enum|literal|union|record|unknown|any|discriminatedUnion|nativeEnum|lazy|int|nullable|optional|coerce)\('),
 'meta_keys':  re.compile(r'_meta\??\.kiro\??\.([A-Za-z_$][A-Za-z0-9_$]*)'),
}
# workflow-ish context markers used to attribute a token to the workflow subsystem
WF_CTX = re.compile(r'workflow|Workflow|recipe|Recipe|nodeId|runId|node_id|run_id|watch|Watch|step|Step|iteration|Iteration|extendRepeat|onMaxIterations|joinPolicy|stopCondition|stopWhen|authoring|agent_?note|consentRound')

def counts(text, pat):
    return collections.Counter(m.group(1) for m in pat.finditer(text))

def ctx(text, tok, n=1, w=110):
    """return up to n contexts of <=2w chars around occurrences of tok"""
    out=[]
    for m in re.finditer(re.escape('"'+tok+'"')+r'|' + re.escape("'"+tok+"'")+r'|'+re.escape('`'+tok+'`'), text):
        a=max(0,m.start()-w); b=min(len(text),m.end()+w)
        out.append(text[a:b])
        if len(out)>=n: break
    return out

old = OLD.read_text(encoding='utf-8', errors='replace')
new = NEW.read_text(encoding='utf-8', errors='replace')
print(f"OLD {OLD.stat().st_size} bytes  NEW {NEW.stat().st_size} bytes")
res={'old_bytes':OLD.stat().st_size,'new_bytes':NEW.stat().st_size,'cats':{}}
for k,p in PAT.items():
    co, cn = counts(old,p), counts(new,p)
    add=sorted(set(cn)-set(co)); rem=sorted(set(co)-set(cn))
    cat={'old_distinct':len(co),'new_distinct':len(cn),'added':{},'removed':{}}
    for t in add:
        cat['added'][t]={'old':co.get(t,0),'new':cn[t],'wf_ctx':bool(WF_CTX.search(' '.join(ctx(new,t,3)))),'ctx':[c for c in ctx(new,t,2)][:2]}
    for t in rem:
        cat['removed'][t]={'old':co[t],'new':cn.get(t,0),'wf_ctx':bool(WF_CTX.search(' '.join(ctx(old,t,3)))),'ctx':[c for c in ctx(old,t,2)][:2]}
    res['cats'][k]=cat
    print(f"\n== {k}: {len(co)} -> {len(cn)} distinct  (+{len(add)} -{len(rem)})")
    for t in add:  print(f"   + {t:42} {co.get(t,0)}->{cn[t]}  wf={cat['added'][t]['wf_ctx']}")
    for t in rem:  print(f"   - {t:42} {co[t]}->{cn.get(t,0)}  wf={cat['removed'][t]['wf_ctx']}")
OUT.write_text(json.dumps(res, indent=2, sort_keys=True))
print(f"\nwrote {OUT}")
