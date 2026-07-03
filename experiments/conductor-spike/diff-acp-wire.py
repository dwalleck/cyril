#!/usr/bin/env python3
"""Diff two kiro-proxy-rs capture logs for ACP-communication drift — what the
agent SENDS (agent_to_client). Reports, per message kind:
  * STRUCTURAL: field-path ADDED / REMOVED (index-normalized, so array length
    changes don't create noise);
  * VALUE: names ADDED / REMOVED in enumerable arrays (commands, tools, models,
    modes, availableCommands, …) — catches new enum/name values a path-only
    diff misses.

Message kinds: notifications keyed by method (+ session/update variant);
responses keyed by `response:<request-method>` via the id→method map.
Usage: diff-acp-wire.py <old.jsonl> <new.jsonl> [--label-old X --label-new Y]"""
import json, sys
from collections import defaultdict

OLD, NEW = sys.argv[1], sys.argv[2]
LOLD = sys.argv[sys.argv.index("--label-old") + 1] if "--label-old" in sys.argv else "old"
LNEW = sys.argv[sys.argv.index("--label-new") + 1] if "--label-new" in sys.argv else "new"

# Fields whose array-of-objects entries carry a stable name we value-diff on.
NAME_KEYS = ("name", "id", "value", "commandName", "modelId", "label")


def norm(path):
    return ".".join("[]" if p.isdigit() else p for p in path.split("."))


def walk(obj, prefix=""):
    if isinstance(obj, dict):
        for k, v in obj.items():
            np = f"{prefix}.{k}" if prefix else k
            if isinstance(v, (dict, list)) and v:
                yield from walk(v, np)
            else:
                yield np
    elif isinstance(obj, list):
        for idx, v in enumerate(obj):
            np = f"{prefix}.{idx}"
            if isinstance(v, (dict, list)) and v:
                yield from walk(v, np)
            else:
                yield np


def value_sets(obj, prefix=""):
    """Collect {normalized-array-path -> set(names)} for arrays of named objects
    and {path -> set(scalars)} for arrays of scalars."""
    out = defaultdict(set)

    def rec(o, pfx):
        if isinstance(o, dict):
            for k, v in o.items():
                rec(v, f"{pfx}.{k}" if pfx else k)
        elif isinstance(o, list):
            for v in o:
                if isinstance(v, dict):
                    nm = next((str(v[k]) for k in NAME_KEYS if k in v and isinstance(v[k], (str, int))), None)
                    if nm is not None:
                        out[norm(pfx)].add(nm)
                    rec(v, pfx)  # descend without index
                elif isinstance(v, (str, int, float, bool)):
                    out[norm(pfx)].add(str(v))
    rec(obj, prefix)
    return out


def load(path):
    reqs = {}  # id -> method (from client_to_agent requests)
    msgs = []  # (group_key, parsed)
    raw = [json.loads(l) for l in open(path) if l.strip()]
    for o in raw:
        if o.get("direction") == "client_to_agent" and o.get("envelope") == "request":
            rid = o.get("parsed", {}).get("id")
            if rid is not None:
                reqs[rid] = o.get("method")
    for o in raw:
        if o.get("direction") != "agent_to_client":
            continue
        parsed = o.get("parsed", {})
        env = o.get("envelope")
        if env == "notification":
            key = o.get("method") or "notification"
            upd = parsed.get("params", {}).get("update", {})
            variant = upd.get("sessionUpdate") if isinstance(upd, dict) else None
            if variant:
                key += f"::{variant}"
            src = parsed.get("params", {})
        elif env == "response":
            rid = parsed.get("id")
            key = f"response:{reqs.get(rid, '?')}"
            src = parsed.get("result", parsed.get("error", {}))
        else:
            continue
        msgs.append((key, src))
    # aggregate per key
    paths = defaultdict(set)
    vals = defaultdict(lambda: defaultdict(set))
    for key, src in msgs:
        for p in walk(src):
            paths[key].add(norm(p))
        for ap, names in value_sets(src).items():
            vals[key][ap] |= names
    return paths, vals


op, ov = load(OLD)
np_, nv = load(NEW)
allkeys = sorted(set(op) | set(np_))
any_drift = False
for key in allkeys:
    pa, pb = op.get(key, set()), np_.get(key, set())
    padd, prem = pb - pa, pa - pb
    # value drift
    vdrift = []
    va, vb = ov.get(key, {}), nv.get(key, {})
    for ap in sorted(set(va) | set(vb)):
        add = vb.get(ap, set()) - va.get(ap, set())
        rem = va.get(ap, set()) - vb.get(ap, set())
        if add or rem:
            vdrift.append((ap, add, rem))
    only = "  [only in %s]" % (LNEW if key not in op else LOLD) if (key not in op or key not in np_) else ""
    if padd or prem or vdrift or only:
        any_drift = True
        print(f"\n### {key}{only}")
        for p in sorted(padd):
            print(f"  + field  {p}")
        for p in sorted(prem):
            print(f"  - field  {p}")
        for ap, add, rem in vdrift:
            for v in sorted(add):
                print(f"  + value  {ap} = {v!r}")
            for v in sorted(rem):
                print(f"  - value  {ap} = {v!r}")
if not any_drift:
    print(f"\nNO DRIFT: agent_to_client field-paths and array values are identical between {LOLD} and {LNEW}.")
print(f"\n(kinds compared: {len(allkeys)} — {', '.join(allkeys)})")
