"""Compare JSON-RPC message field sets between a reference capture and a fresh conductor log.

Usage:
    diff_fields.py <reference.json> <capture.log> [--label-ref NAME] [--label-cap NAME]

The reference is a concatenated-JSON dump (one or more JSON-RPC objects, no framing).
The capture is a sacp-conductor --debug log (one JSON-RPC object embedded per line).
"""
import argparse, json, re
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

def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("reference", help="Reference capture (concat JSON, e.g. docs/kiro-acp-capture-2.1.0.json)")
    ap.add_argument("capture", help="Fresh conductor log (e.g. /tmp/conductor-spike/logs/<latest>.log)")
    ap.add_argument("--label-ref", default="REFERENCE", help="Label for reference in output")
    ap.add_argument("--label-cap", default="CAPTURE", help="Label for capture in output")
    args = ap.parse_args()

    ref = categorize(parse_concat_json(args.reference))
    cap = categorize(parse_conductor_log(args.capture))

    common = sorted(set(ref) & set(cap))
    only_cap = sorted(set(cap) - set(ref))
    only_ref = sorted(set(ref) - set(cap))

    print("=" * 70)
    print(f"METHODS IN BOTH ({args.label_ref} vs {args.label_cap}) — field-level deltas")
    print("=" * 70)
    deltas_found = False
    for method in common:
        new_fields = cap[method] - ref[method]
        removed_fields = ref[method] - cap[method]
        if new_fields or removed_fields:
            deltas_found = True
            print(f"\n[{method}]")
            for f in sorted(new_fields):
                print(f"  + {f}")
            for f in sorted(removed_fields):
                print(f"  - {f}")
        else:
            print(f"\n[{method}] — no field deltas ({len(cap[method])} fields)")
    if not deltas_found:
        print("\n  (no field-level deltas in any common method)")

    print()
    print("=" * 70)
    print(f"METHODS ONLY IN {args.label_cap}")
    print("=" * 70)
    if not only_cap:
        print("  (none)")
    for method in only_cap:
        print(f"\n[{method}]")
        for f in sorted(cap[method]):
            print(f"  + {f}")

    print()
    print("=" * 70)
    print(f"METHODS ONLY IN {args.label_ref} (probably scenario coverage gaps, not removals)")
    print("=" * 70)
    if not only_ref:
        print("  (none)")
    for method in only_ref:
        print(f"\n[{method}]")

if __name__ == "__main__":
    main()
