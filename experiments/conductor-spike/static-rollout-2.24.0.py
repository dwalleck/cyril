#!/usr/bin/env python3
import json, re, sys
from pathlib import Path

paths = [Path(p) for p in sys.argv[1:]] or [
    Path('/home/dwalleck/.local/share/kiro-research/binaries/2.21.1/kiro-cli-chat'),
    Path('/home/dwalleck/.local/share/kiro-research/binaries/2.21.2/extracted/kiro-cli-chat'),
]

def find_table(data: bytes):
    # The table has a stable first key. Try every occurrence because each
    # engine crate embeds a copy; accept only a full object containing the
    # known cloud_config and v3_prompt keys.
    for m in re.finditer(b'"tui":\\s*\\{', data):
        start = data.rfind(b'{', 0, m.start())
        if start < 0:
            continue
        depth = 0; quoted = False; esc = False
        for i in range(start, len(data)):
            c = data[i]
            if quoted:
                if esc: esc = False
                elif c == 92: esc = True
                elif c == 34: quoted = False
                continue
            if c == 34: quoted = True
            elif c == 123: depth += 1
            elif c == 125:
                depth -= 1
                if depth == 0:
                    try: obj = json.loads(data[start:i+1])
                    except json.JSONDecodeError: break
                    if isinstance(obj, dict) and {'tui','cloud_config','v3_prompt'} <= obj.keys():
                        return obj, start
                    break
    raise RuntimeError('rollout registry not found')

all_records = []
for path in paths:
    obj, offset = find_table(path.read_bytes())
    record = {'path': str(path), 'sha256': __import__('hashlib').sha256(path.read_bytes()).hexdigest(), 'offset': offset, 'keys': sorted(obj), 'registry': obj}
    all_records.append(record)
    print(path, 'offset', offset, 'keys', len(obj))
    for k in sorted(obj):
        v = obj[k]
        print(f'{k}: treatment={v.get("treatment_percent")} segment={v.get("segment")} channel={v.get("channel")} description={v.get("description", "")}')
Path('experiments/conductor-spike/static-2.24.0/static-rollout-2.24.0.json').write_text(json.dumps(all_records, indent=2, sort_keys=True))
deltas = []
for a, b in zip(all_records, all_records[1:]):
    old, new = a['registry'], b['registry']
    delta = {k: {'old': old.get(k), 'new': new.get(k)} for k in sorted(set(old) | set(new)) if old.get(k) != new.get(k)}
    deltas.append({'from': a['path'], 'to': b['path'], 'delta': delta})
    print(f"\n{a['path'].split('/')[-5]} -> {b['path'].split('/')[-5]}: changed entries:", ', '.join(delta) if delta else 'none')
    for k, v in delta.items(): print('  ', k, json.dumps(v))
Path('experiments/conductor-spike/static-2.24.0/static-rollout-delta.json').write_text(json.dumps(deltas, indent=2, sort_keys=True))
