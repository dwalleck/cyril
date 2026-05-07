"""Compare JSON-RPC message field sets between 2.1.0 capture and 2.2.0 spike log."""
import json, re, sys
from collections import defaultdict

def walk_paths(obj, prefix=""):
    if isinstance(obj, dict):
        for k, v in obj.items():
            np = f"{prefix}.{k}" if prefix else k
            if isinstance(v, (dict, list)) and v:
                yield from walk_paths(v, np)
            else:
                yield np
    elif isinstance(obj, list):
        if obj:
            yield from walk_paths(obj[0], f"{prefix}[]")

def parse_concat_json(path):
    msgs = []
    with open(path) as f:
        text = f.read()
    dec = json.JSONDecoder()
    pos = 0
    while pos < len(text):
        rem = text[pos:].lstrip()
        if not rem: break
        skip = len(text[pos:]) - len(rem)
        try:
            obj, idx = dec.raw_decode(rem)
            msgs.append(obj)
            pos += skip + idx
        except json.JSONDecodeError:
            pos += skip + 1
    return msgs

def parse_conductor_log(path):
    msgs = []
    with open(path) as f:
        for line in f:
            m = re.search(r'\{"jsonrpc".*\}', line)
            if not m: continue
            try:
                msgs.append(json.loads(m.group()))
            except json.JSONDecodeError:
                pass
    return msgs

def categorize(msgs):
    """For each method, collect all field paths in its params or result."""
    by_method = defaultdict(set)
    # Index requests by id so we can attribute responses
    req_id_to_method = {}
    for m in msgs:
        if "method" in m and "id" in m:
            req_id_to_method[str(m["id"])] = m["method"]
    for m in msgs:
        if "method" in m:
            for p in walk_paths(m.get("params", {}), "params"):
                by_method[m["method"] + " (request/notif)"].add(p)
        elif "result" in m:
            mid = str(m.get("id", "?"))
            method = req_id_to_method.get(mid, f"unknown-id-{mid}")
            for p in walk_paths(m["result"], "result"):
                by_method[method + " (response)"].add(p)
    return by_method

cap_210 = categorize(parse_concat_json("/home/dwalleck/repos/cyril/docs/kiro-acp-capture-2.1.0.json"))
cap_220 = categorize(parse_conductor_log("/tmp/conductor-spike/logs/20260503-140353.log"))

# Methods seen in both
common = sorted(set(cap_210) & set(cap_220))
only_220 = sorted(set(cap_220) - set(cap_210))
only_210 = sorted(set(cap_210) - set(cap_220))

print("=" * 70)
print("METHODS IN BOTH — checking field-level deltas")
print("=" * 70)
for method in common:
    new_fields = cap_220[method] - cap_210[method]
    removed_fields = cap_210[method] - cap_220[method]
    if new_fields or removed_fields:
        print(f"\n[{method}]")
        for f in sorted(new_fields):
            print(f"  + {f}")
        for f in sorted(removed_fields):
            print(f"  - {f}")
    else:
        print(f"\n[{method}] — no field deltas ({len(cap_220[method])} fields)")

print()
print("=" * 70)
print("METHODS ONLY IN 2.2.0 SPIKE")
print("=" * 70)
for method in only_220:
    print(f"\n[{method}]")
    for f in sorted(cap_220[method]):
        print(f"  + {f}")

print()
print("=" * 70)
print("METHODS ONLY IN 2.1.0 CAPTURE (probably scenario coverage gaps, not removals)")
print("=" * 70)
for method in only_210:
    print(f"\n[{method}]")
