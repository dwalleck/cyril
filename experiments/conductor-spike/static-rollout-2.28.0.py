#!/usr/bin/env python3
"""Host rollout registry + cohort-override table across a chain of builds (2.28.0 audit).

    python3 -I static-rollout-2.28.0.py <outdir> <bin1> <bin2> [...]

Adapted from static-rollout-2.24.0.py: anchors on every `"tui": {` and
brace-matches outward, accepting the object holding tui+cloud_config+v3_prompt.
Also captures the FeatureOverride table ({"<feature>": {"treatment": [...],
"control": [...]}}) found by brace-matching around `"treatment": [`.
Reports EVERY row delta (added/removed/field change) between consecutive builds.
Writes <outdir>/rollout.json and <outdir>/rollout-delta.json.
"""
import hashlib, json, re, sys
from pathlib import Path

def match_obj(data, start):
    depth = 0; q = False; e = False
    for i in range(start, min(len(data), start + 2_000_000)):
        c = data[i]
        if q:
            if e: e = False
            elif c == 92: e = True
            elif c == 34: q = False
            continue
        if c == 34: q = True
        elif c == 123: depth += 1
        elif c == 125:
            depth -= 1
            if depth == 0:
                try: return json.loads(data[start:i + 1])
                except (json.JSONDecodeError, UnicodeDecodeError): return None
    return None

def find_registry(data):
    found = []
    for m in re.finditer(rb'"tui":\s*\{', data):
        s = m.start()
        for _ in range(4):
            s = data.rfind(b'{', 0, s)
            if s < 0: break
            obj = match_obj(data, s)
            if isinstance(obj, dict) and {'tui', 'cloud_config'} <= obj.keys():
                found.append((s, obj)); break
    return found

def find_overrides(data):
    out = []
    for m in re.finditer(rb'"(?:treatment|control)"\s*:\s*\[', data):
        s = m.start()
        for _ in range(3):
            s = data.rfind(b'{', 0, s)
            if s < 0: break
            obj = match_obj(data, s)
            if isinstance(obj, dict) and obj and all(isinstance(v, dict) and ({'treatment', 'control'} & v.keys()) for v in obj.values()):
                out.append((s, obj)); break
    uniq = {}
    for s, o in out: uniq[json.dumps(o, sort_keys=True)] = (s, o)
    return list(uniq.values())

outdir = Path(sys.argv[1]); outdir.mkdir(parents=True, exist_ok=True)
records = []
for p in map(Path, sys.argv[2:]):
    data = p.read_bytes()
    regs = find_registry(data)
    uniq = {json.dumps(o, sort_keys=True): (s, o) for s, o in regs}
    if not uniq: raise SystemExit(f'registry not found in {p}')
    if len(uniq) > 1: print('WARNING: multiple distinct registries in', p, [s for s, _ in uniq.values()])
    off, reg = next(iter(uniq.values()))
    ovs = find_overrides(data)
    rec = {'path': str(p), 'sha256': hashlib.sha256(data).hexdigest(), 'offset': off,
           'copies': len(regs), 'registry': reg,
           'overrides': [{'offset': s, 'summary': {k: {kk: (len(vv) if isinstance(vv, list) else vv) for kk, vv in v.items()} for k, v in o.items()},
                          'digest': hashlib.sha256(json.dumps(o, sort_keys=True).encode()).hexdigest()[:16]} for s, o in ovs]}
    records.append(rec)
    print(f'{p}: offset {off} copies {len(regs)} rows {len(reg)} overrides {rec["overrides"]}')
    for k in sorted(reg):
        v = reg[k]
        print(f'  {k}: treatment={v.get("treatment_percent")} segment={v.get("segment")} channel={v.get("channel")} :: {v.get("description", "")}')
(outdir / 'rollout.json').write_text(json.dumps(records, indent=2, sort_keys=True))
deltas = []
for a, b in zip(records, records[1:]):
    old, new = a['registry'], b['registry']
    d = {}
    for k in sorted(set(old) | set(new)):
        if old.get(k) == new.get(k): continue
        if k not in old: d[k] = {'kind': 'ADDED', 'new': new[k]}
        elif k not in new: d[k] = {'kind': 'REMOVED', 'old': old[k]}
        else:
            d[k] = {'kind': 'CHANGED', 'fields': {f: [old[k].get(f), new[k].get(f)] for f in sorted(set(old[k]) | set(new[k])) if old[k].get(f) != new[k].get(f)}}
    ov = [o['digest'] for o in a['overrides']] != [o['digest'] for o in b['overrides']]
    deltas.append({'from': a['path'], 'to': b['path'], 'rows': d, 'overrides_changed': ov})
    print(f"\n{a['path']} -> {b['path']}: {len(d)} row deltas; overrides changed: {ov}")
    for k, v in d.items(): print('  ', k, json.dumps(v))
(outdir / 'rollout-delta.json').write_text(json.dumps(deltas, indent=2, sort_keys=True))
