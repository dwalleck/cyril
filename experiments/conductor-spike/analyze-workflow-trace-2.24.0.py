#!/usr/bin/env python3
"""Post-hoc analysis of a kas-workflow-channels-*.jsonl trace: workflow event
vocabulary + payload key sets, per-step-session session_info_update kind
ordering around each node, main-session focus_update/activity frames, and the
capture/artifact verdict. Run on every leg and diff the printed summaries."""
import json, sys, collections
path = sys.argv[1]
frames = [json.loads(l) for l in open(path, encoding='utf-8') if l.strip()]
def paths(o, pre=""):
    if isinstance(o, dict):
        for k, v in o.items():
            p = f"{pre}.{k}" if pre else k
            yield p; yield from paths(v, p)
    elif isinstance(o, list):
        for v in o[:5]: yield from paths(v, pre + "[]")
wf_kinds = collections.Counter(); wf_keys = collections.defaultdict(set)
main_sid = None; step_sids = {}; siu_by_sid = collections.defaultdict(list); focus = []
t0 = frames[0]['ts'] if frames else 0
for f in frames:
    m = f['msg']; meth = m.get('method'); d = f['dir']
    if d == 'agent->client' and 'result' in m and isinstance(m['result'], dict) and 'sessionId' in m['result'] and main_sid is None and 'modes' in m['result']:
        main_sid = m['result']['sessionId']
    if meth and meth.startswith('_kiro/workflow/'):
        k = meth.split('/')[-1]; p = m.get('params') or {}
        wf_kinds[k] += 1; wf_keys[k].update(paths(p))
        if k == 'node_start' and p.get('sessionId'): step_sids[p['sessionId']] = p.get('nodeId')
    if meth == 'session/update':
        sid = m['params'].get('sessionId'); u = m['params'].get('update') or {}
        if u.get('sessionUpdate') == 'session_info_update':
            kind = ((u.get('_meta') or {}).get('kiro') or {}).get('kind')
            siu_by_sid[sid].append((round(f['ts']-t0,2), kind, {k: u[k] for k in u if k not in ('_meta','sessionUpdate')} if kind == 'focus_update' else None))
            if kind == 'focus_update':
                focus.append((round(f['ts']-t0,2), 'MAIN' if sid == main_sid else step_sids.get(sid, sid[-8:]), {k: v for k, v in u.items() if k not in ('sessionUpdate',)}, (u.get('_meta') or {}).get('kiro')))
print("== file:", path)
print("== main session:", main_sid, "| step sessions:", {v: k[-8:] for k, v in step_sids.items()})
print("== workflow event kinds:", dict(wf_kinds))
for k in sorted(wf_keys): print(f"   {k}: {sorted(wf_keys[k])}")
print("== per-session session_info_update kind sequence:")
for sid, seq in siu_by_sid.items():
    tag = 'MAIN' if sid == main_sid else step_sids.get(sid, sid[-8:])
    print(f"   [{tag}] " + " ".join(f"{k}@{t}" for t, k, _ in seq))
print("== focus_update frames (activity?):")
for t, who, body, meta in focus: print(f"   @{t} [{who}] {json.dumps(body)[:300]} meta={json.dumps(meta)[:200]}")
# ordering around node_complete: last turn_end in step vs node_complete ts
evts = [(round(f['ts']-t0,2), f['msg']['method'].split('/')[-1], (f['msg'].get('params') or {}).get('nodeId'), (f['msg'].get('params') or {}).get('status')) for f in frames if (f['msg'].get('method') or '').startswith('_kiro/workflow/')]
print("== workflow event timeline:")
for e in evts: print("   ", e)
rc = [f['msg']['params'] for f in frames if f['msg'].get('method') == '_kiro/workflow/run_complete']
if rc:
    fs = rc[-1].get('finalState') or {}
    print("== run_complete status:", rc[-1].get('status'), "| capturedOutputs:", json.dumps(fs.get('capturedOutputs'))[:300], "| artifacts:", json.dumps(fs.get('artifacts'))[:300])
    print("== finalState keys:", sorted(fs))
all_paths = set()
for f in frames:
    if f['dir'] == 'agent->client': all_paths.update(paths(f['msg']))
print("== agent->client distinct paths:", len(all_paths))
json.dump(sorted(all_paths), open(path.replace('.jsonl', '-paths.json'), 'w'))
