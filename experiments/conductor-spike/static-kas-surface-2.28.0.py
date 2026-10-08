#!/usr/bin/env python3
"""KAS wire-surface census across N carved trees, as string-literal SET diffs.

    python3 -I static-kas-surface-2.28.0.py --out <dir> <tree-root>...

Extends static-kas-surface-2.24.0.py. Minified bundles rename identifiers every
build, so every category here is a set of *string literals* or *object keys*
(which minification does not rename), diffed between consecutive trees.

Categories:
  method_literals   every quoted "_kiro/..." literal
  acp_methods       every quoted "session/...", "fs/...", "terminal/...", "authenticate"... literal
  env_reads         process.env.X and process.env["X"]
  env_literals      KIRO_*/AB_*/AWS_* style uppercase literals anywhere
  snake_kinds       snake_case quoted literals with lifecycle suffixes (session_info_update kinds, events)
  kind_values       kind:"x" / type:"x" / sessionUpdate:"x" / status:"x" / reason:"x" values
  zod_keys          object keys whose value starts a zod-ish builder (k:X.string()/.object(/.enum(/...)
  log_ids           quoted dotted lowercase ids ("workflow.checkpoint.restore") = log/telemetry event names
  meta_kiro_keys    keys reached via _meta?.kiro?.X / _meta.kiro.X
  settings_keys     keys reached via settings?.X / settings.X
Writes <out>/static-kas-surface.json.
"""
import argparse, hashlib, json, re
from pathlib import Path

ap = argparse.ArgumentParser(); ap.add_argument('--out', required=True); ap.add_argument('roots', nargs='+')
a = ap.parse_args()
roots = [Path(p) for p in a.roots]
out = Path(a.out); out.mkdir(parents=True, exist_ok=True)

Q = r'["\'`]'
PAT = {
  'method_literals': re.compile(r'["\'`](_kiro/[A-Za-z0-9_./:-]+)["\'`]'),
  'acp_methods': re.compile(r'["\'`]((?:session|fs|terminal|authenticate|initialize|_message|_session)(?:/[A-Za-z0-9_]+)+|authenticate|initialize)["\'`]'),
  'env_reads': re.compile(r'process\.env(?:\.([A-Z][A-Z0-9_]+)|\[["\']([A-Z][A-Z0-9_]+)["\']\])'),
  'env_literals': re.compile(r'["\'`]((?:KIRO|AB|AWS|AMAZON_Q|Q|KAS|NODE|OTEL)_[A-Z0-9_]{2,})["\'`]'),
  'snake_kinds': re.compile(r'["\'](turn_start|turn_completion|turn_end|[a-z_]+_changed|[a-z_]+_update|[a-z_]+_paused|[a-z_]+_complete|[a-z_]+_start|[a-z_]+_stall[a-z_]*|[a-z_]+_discarded|[a-z_]+_dropped|[a-z_]+_availability|[a-z_]+_resumed|[a-z_]+_failed|[a-z_]+_notice|[a-z_]+_fallback)["\']'),
  'kind_values': re.compile(r'\b(kind|type|sessionUpdate|status|reason|outcome|source|category|stopReason|action):["\']([A-Za-z0-9_./:-]{2,60})["\']'),
  'zod_keys': re.compile(r'[{,]([A-Za-z_$][A-Za-z0-9_$]*):[A-Za-z_$][A-Za-z0-9_$]*\.(?:string|number|boolean|object|array|enum|literal|union|record|unknown|any|discriminatedUnion|nativeEnum|lazy|int|nullable|optional)\('),
  'log_ids': re.compile(r'["\']([a-z][a-zA-Z0-9_]*(?:\.[a-zA-Z0-9_-]+){1,6})["\']'),
  'meta_kiro_keys': re.compile(r'_meta\??\.kiro\??\.([A-Za-z_$][A-Za-z0-9_$]*)'),
  'settings_keys': re.compile(r'\bsettings\??\.([A-Za-z_$][A-Za-z0-9_$]*)'),
}
NOISE_LOG = re.compile(r'\.(js|ts|json|md|mjs|cjs|py|html|css|txt|yaml|yml|toml|png|svg|d)$|^\d|^(www|node|github|npm|use)\.')

def one(root):
    server = next(root.glob('node_modules/@kiro/agent/dist/server/acp-server.js'))
    text = server.read_text(encoding='utf-8', errors='replace')
    pkg = json.loads((root/'node_modules/@kiro/agent/package.json').read_text())
    rec = {'server': str(server), 'server_sha256': hashlib.sha256(server.read_bytes()).hexdigest(),
           'bytes': server.stat().st_size, 'version': pkg.get('version'),
           'deps': pkg.get('dependencies', {})}
    for k, p in PAT.items():
        vals = set()
        for m in p.finditer(text):
            if k == 'env_reads': vals.add(m.group(1) or m.group(2))
            elif k == 'kind_values': vals.add(f'{m.group(1)}:{m.group(2)}')
            else: vals.add(m.group(1))
        if k == 'log_ids': vals = {v for v in vals if not NOISE_LOG.search(v)}
        rec[k] = sorted(vals)
    return rec

recs = [one(r) for r in roots]
for r in recs:
    print(f"{r['version']:8} sha={r['server_sha256'][:16]} bytes={r['bytes']} " + ' '.join(f"{k}={len(r[k])}" for k in PAT))
deltas = []
for A_, B_ in zip(recs, recs[1:]):
    d = {'from': A_['version'], 'to': B_['version']}
    print(f"\n=== {d['from']} -> {d['to']} ===")
    for k in PAT:
        A, B = set(A_[k]), set(B_[k])
        d[k] = {'old_count': len(A), 'new_count': len(B), 'added': sorted(B-A), 'removed': sorted(A-B)}
        print(f"  {k}: {len(A)} -> {len(B)} (+{len(B-A)} -{len(A-B)})")
        if k != 'log_ids':
            for x in sorted(B-A): print(f"     + {x}")
            for x in sorted(A-B): print(f"     - {x}")
    da, db = A_['deps'], B_['deps']
    d['deps'] = {k: {'old': da.get(k), 'new': db.get(k)} for k in sorted(set(da)|set(db)) if da.get(k) != db.get(k)}
    print('  deps changed:', json.dumps(d['deps']))
    deltas.append(d)
(out/'static-kas-surface.json').write_text(json.dumps({'trees': recs, 'deltas': deltas}, indent=2, sort_keys=True))
