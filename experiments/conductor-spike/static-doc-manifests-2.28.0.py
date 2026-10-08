#!/usr/bin/env python3
"""Extract embedded doc manifests ({generated_at,total_docs,documents[]}) from
two kiro-cli-chat builds and diff documents[] by path (+title/validated/status).

    static-doc-manifests-2.28.0.py <outdir> <chat1> <chat2> [...]  (chain; diffs each consecutive pair)
Writes <outdir>/doc-manifests-delta.json and <outdir>/doc-manifests-<i>.json (merged documents per build).
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


outdir = Path(sys.argv[1]); outdir.mkdir(parents=True, exist_ok=True)
paths = sys.argv[2:]
def key(ms):
    d = {}
    for m in ms:
        for doc in m['documents']:
            d[doc.get('path')] = {k: doc.get(k) for k in ('title','validated','status','category','description','file','keywords')}
    return d
keyed = []
for i, p in enumerate(paths):
    ms = manifests(Path(p).read_bytes())
    print(p, [(m.get('generated_at'), m.get('total_docs'), len(m['documents'])) for m in ms])
    k = key(ms); keyed.append((p, ms, k))
    (outdir / f'doc-manifests-{i}.json').write_text(json.dumps({'path': p, 'manifests': [(m.get('generated_at'), m.get('total_docs')) for m in ms], 'documents': k}, indent=1))
all_res = []
for (pa, msa, old), (pb, msb, new) in zip(keyed, keyed[1:]):
    added = {k: new[k] for k in sorted(set(new) - set(old))}
    removed = {k: old[k] for k in sorted(set(old) - set(new))}
    changed = {k: {'old': old[k], 'new': new[k]} for k in sorted(set(old) & set(new)) if old[k] != new[k]}
    all_res.append({'from': pa, 'to': pb, 'old': [(m.get('generated_at'), m.get('total_docs')) for m in msa],
                    'new': [(m.get('generated_at'), m.get('total_docs')) for m in msb], 'added': added, 'removed': removed, 'changed': changed})
    print('=====', pa.split('/')[-5], '->', pb.split('/')[-5])
    print('added', len(added)); [print('  +', k, '|', v['title'], '|', v['category'], '|', v['validated']) for k, v in added.items()]
    print('removed', len(removed)); [print('  -', k, '|', v['title']) for k, v in removed.items()]
    print('changed', len(changed))
    for k, v in changed.items():
        diffs = {f: (v['old'][f], v['new'][f]) for f in v['old'] if v['old'][f] != v['new'][f]}
        print('  ~', k, {f: (str(a)[:100], str(b)[:100]) for f, (a, b) in diffs.items()})
(outdir / 'doc-manifests-delta.json').write_text(json.dumps(all_res, indent=2))
