#!/usr/bin/env python3
"""KAS wire-surface census across N self-extracted or carved trees.

    static-kas-surface-2.22.0.py <tree-root>... 

Prints per-tree package version, acp-server.js sha, quoted `_kiro/*` method
literal count, process.env read count; then the pairwise delta between each
consecutive pair. Writes experiments/conductor-spike/static-2.24.0/static-kas-surface.json.
"""
import hashlib, json, re, sys
from pathlib import Path

roots = [Path(p) for p in sys.argv[1:]]
out = Path('experiments/conductor-spike/static-2.24.0/static-kas-surface.json')

def one(root):
    server = next(root.glob('node_modules/@kiro/agent/dist/server/acp-server.js'))
    text = server.read_text(encoding='utf-8', errors='replace')
    pkg = json.loads((root/'node_modules/@kiro/agent/package.json').read_text())
    methods = sorted(set(re.findall(r'["\'](_kiro/[A-Za-z0-9_./:-]+)["\']', text)))
    env = sorted(set(re.findall(r'process\.env\.([A-Z][A-Z0-9_]+)', text)))
    envlit = sorted(set(re.findall(r'KIRO_[A-Z0-9_]+', text)))
    # session_info_update kinds + workflow event names are quoted literals too
    sinfo = sorted(set(re.findall(r'["\'](turn_start|turn_completion|turn_end|[a-z_]+_changed|[a-z_]+_update|[a-z_]+_paused|[a-z_]+_complete|[a-z_]+_start|[a-z_]+_stall[a-z_]*|[a-z_]+_discarded|[a-z_]+_dropped)["\']', text)))
    deps = {k: pkg.get(k, {}) for k in ('version','dependencies','peerDependencies')}
    return {'server': str(server), 'server_sha256': hashlib.sha256(server.read_bytes()).hexdigest(),
            'bytes': server.stat().st_size, 'lines': text.count('\n'),
            'package': deps, 'method_literals': methods, 'env_reads': env, 'env_literals': envlit,
            'snake_kinds': sinfo}

recs = [one(r) for r in roots]
for r in recs:
    print(f"{r['package']['version']:8} sha={r['server_sha256'][:16]} bytes={r['bytes']} lines={r['lines']} methods={len(r['method_literals'])} env_reads={len(r['env_reads'])} env_lit={len(r['env_literals'])} kinds={len(r['snake_kinds'])}")
deltas = []
for a, b in zip(recs, recs[1:]):
    d = {'from': a['package']['version'], 'to': b['package']['version']}
    for key in ('method_literals','env_reads','env_literals','snake_kinds'):
        A,B=set(a[key]),set(b[key]); d[key]={'old_count':len(A),'new_count':len(B),'added':sorted(B-A),'removed':sorted(A-B)}
    da, db = a['package']['dependencies'], b['package']['dependencies']
    d['deps'] = {k: {'old': da.get(k), 'new': db.get(k)} for k in sorted(set(da)|set(db)) if da.get(k)!=db.get(k)}
    deltas.append(d)
    print(f"\n=== {d['from']} -> {d['to']} ===")
    for key in ('method_literals','env_reads','env_literals','snake_kinds'):
        print(f"  {key}: {d[key]['old_count']} -> {d[key]['new_count']}")
        for x in d[key]['added']: print(f"     + {x}")
        for x in d[key]['removed']: print(f"     - {x}")
    print("  deps changed:", json.dumps(d['deps']))
out.write_text(json.dumps({'trees': recs, 'deltas': deltas}, indent=2, sort_keys=True))
