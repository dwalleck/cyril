#!/usr/bin/env python3
"""
New-field sweep over ACP capture JSONL. Run this at the END of every audit —
targeted probes answer their own hypothesis and will not notice a field that
nobody asked about.

    python3 sweep-new-fields.py <capture.jsonl> [more.jsonl ...]        # inventory
    python3 sweep-new-fields.py --diff <live.jsonl> <pinned.jsonl>      # A/B

Inventory mode prints the distinct JSON path count per file, the union, and
every path whose name looks like usage/token/credit/quota metering.

Diff mode is the attribution test: run the SAME workload against the new bundle
and a pinned older one (KIRO_KAS_SERVER_PATH), then diff the path sets. Paths
present only under the new bundle are candidates for "new field"; remember that
a leg which errored early simply covers fewer paths, so a non-empty diff is a
lead, not a verdict.

Array indices collapse to `[]` so element-level churn does not inflate the diff.
"""
import json, sys

METER_KEYS = ("usage", "token", "credit", "cost", "quota", "limit",
              "meter", "billing", "overage", "plan", "tier", "spend")


def paths(o, pre=""):
    if isinstance(o, dict):
        for k, v in o.items():
            p = f"{pre}.{k}" if pre else k
            yield p
            yield from paths(v, p)
    elif isinstance(o, list):
        for v in o[:5]:
            yield from paths(v, pre + "[]")


def inventory(path):
    seen = set()
    for line in open(path, encoding="utf-8", errors="replace"):
        line = line.strip()
        if not line:
            continue
        try:
            rec = json.loads(line)
        except Exception:
            continue
        seen.update(paths(rec.get("msg", rec)))
    return seen


def main(argv):
    if not argv:
        raise SystemExit(__doc__)
    if argv[0] == "--diff":
        if len(argv) != 3:
            raise SystemExit("--diff needs exactly two capture files")
        live, pin = argv[1], argv[2]
        L, P = inventory(live), inventory(pin)
        print(f"live   {live}: {len(L)} paths")
        print(f"pinned {pin}: {len(P)} paths")
        only_l, only_p = sorted(L - P), sorted(P - L)
        print(f"\nONLY under live ({len(only_l)}) — new-field candidates:")
        for p in only_l:
            print("   +", p)
        print(f"\nONLY under pinned ({len(only_p)}) — usually just coverage:")
        for p in only_p:
            print("   -", p)
        if not only_l and not only_p:
            print("\nIDENTICAL field sets — no shape change on this workload.")
        return

    invs = {f: inventory(f) for f in argv}
    for f, s in invs.items():
        print(f"{len(s):6}  {f}")
    union = set().union(*invs.values()) if invs else set()
    print(f"\nunion: {len(union)} distinct paths")
    hits = sorted(p for p in union if any(k in p.lower() for k in METER_KEYS))
    print(f"\nusage/token/credit-shaped paths ({len(hits)}):")
    for p in hits:
        print("   ", p)


if __name__ == "__main__":
    main(sys.argv[1:])
