#!/usr/bin/env python3
"""Extract embedded doc manifests ({generated_at,total_docs,documents[]}) from
two kiro-cli-chat builds and diff documents[] by path (+title/validated/status).

    static-doc-manifests-2.24.0.py <old-chat> <new-chat>
Writes static-2.24.0/static-doc-manifests-delta.json.
"""
import json, re, sys
from pathlib import Path

def manifests(data):
    out = []
    for m in re.finditer(rb'"generated_at"\s*:', data):
        start = data.rfind(b'{', max(0, m.start() - 4096), m.start())
        if start < 0: continue
        depth = 0; q = False; e = False
        for i in range(start, len(data)):
            c = data[i]
            if q:
                if e: e = False
                elif c == 92: e = True
                elif c == 34: q = False
                continue
            if c < 9: break
            if c == 34: q = True
            elif c == 123: depth += 1
            elif c == 125:
                depth -= 1
                if depth == 0:
                    try:
                        obj = json.loads(data[start:i+1])
                        if isinstance(obj, dict) and 'documents' in obj: out.append(obj)
                    except Exception: pass
                    break
    return out

res = {}
sets = []
for p in sys.argv[1:3]:
    ms = manifests(Path(p).read_bytes())
    print(p, [(m.get('generated_at'), m.get('total_docs'), len(m['documents'])) for m in ms])
    sets.append(ms)
def key(ms):
    d = {}
    for m in ms:
        for doc in m['documents']:
            d[doc.get('path')] = {k: doc.get(k) for k in ('title','validated','status','category','description')}
    return d
old, new = key(sets[0]), key(sets[1])
added = {k: new[k] for k in sorted(set(new) - set(old))}
removed = {k: old[k] for k in sorted(set(old) - set(new))}
changed = {k: {'old': old[k], 'new': new[k]} for k in sorted(set(old) & set(new)) if old[k] != new[k]}
res = {'old': [(m.get('generated_at'), m.get('total_docs')) for m in sets[0]],
       'new': [(m.get('generated_at'), m.get('total_docs')) for m in sets[1]],
       'added': added, 'removed': removed, 'changed': changed}
Path('experiments/conductor-spike/static-2.24.0/static-doc-manifests-delta.json').write_text(json.dumps(res, indent=2))
print('added', len(added)); [print('  +', k, '|', v['title'], '|', v['category'], '|', v['validated']) for k, v in added.items()]
print('removed', len(removed)); [print('  -', k) for k in removed]
print('changed', len(changed))
for k, v in changed.items():
    diffs = {f: (v['old'][f], v['new'][f]) for f in v['old'] if v['old'][f] != v['new'][f]}
    print('  ~', k, {f: (str(a)[:80], str(b)[:80]) for f, (a, b) in diffs.items()})
