#!/usr/bin/env python3
"""Consumer + gate census for minified tui.js bundles (kiro-cli 2.32.0 audit).

Complements static-tui-sets-2.32.0.py. For each bundle, extracts:
  ext_notifications - onExtNotification("<method>") registrations (what the TUI consumes)
  kind_case         - kind==="x" and case"x" labels (session_info_update kinds, UI kinds)
  kiro_meta_reads   - ._meta?.kiro?.<key> / .kiro?.<key> reads
  ext_calls         - callExtMethod/extMethod/request("<_kiro|kiro.dev|session>/...")
  rollout_gates     - call-site census of the rollout gate function (located by its
                      `if(e==="local_sandbox")return process.env.<ENV>==="1"` body, so the
                      minified name is resolved per bundle), as name -> count
and prints OLD -> NEW set/census diffs for consecutive bundles.

Usage: python3 -I static-tui-consumers-2.32.0.py BUNDLE_A BUNDLE_B [...]
Read-only; nothing is executed.
"""
import collections
import re
import sys
from pathlib import Path


def census(t):
    out = {
        'ext_notifications': set(re.findall(r'onExtNotification\("([^"]+)"', t)),
        'kind_case': set(re.findall(r'kind==="([a-z_]+)"', t)) | set(re.findall(r'case"([a-z_]+)"', t)),
        'kiro_meta_reads': set(re.findall(r'\.kiro\?\.([A-Za-z]+)', t)),
        'ext_calls': set(re.findall(r'(?:callExtMethod|extMethod|sendRequest|request)\("((?:_kiro|_?kiro\.dev|session)/[^"]+)"', t)),
    }
    m = re.search(r'function ([A-Za-z0-9_$]+)\(e\)\{if\(e==="local_sandbox"\)return process\.env\.([A-Z_]+)==="1"', t)
    gates = collections.Counter()
    if m:
        fn = re.escape(m.group(1))
        gates.update(re.findall(r'(?<![\w$.])' + fn + r'\("([a-z_0-9]+)"\)', t))
    out['rollout_gates'] = gates
    out['_gate_fn'] = m.group(1) if m else None
    return out


def main():
    bs = [Path(a) for a in sys.argv[1:]]
    C = [census(b.read_text(errors='replace')) for b in bs]
    for (a, ca), (b, cb) in zip(zip(bs, C), zip(bs[1:], C[1:])):
        print(f"# {a.name} -> {b.name} (gate fn {ca['_gate_fn']} -> {cb['_gate_fn']})")
        for k in ('ext_notifications', 'kind_case', 'kiro_meta_reads', 'ext_calls'):
            print(f"## {k}: {len(ca[k])} -> {len(cb[k])} added={sorted(cb[k]-ca[k])} removed={sorted(ca[k]-cb[k])}")
        ga, gb = ca['rollout_gates'], cb['rollout_gates']
        print(f"## rollout_gates old={sorted(ga.items())}")
        print(f"## rollout_gates new={sorted(gb.items())}")
        print(f"## rollout_gates changed={ {k: (ga.get(k, 0), gb.get(k, 0)) for k in set(ga) | set(gb) if ga.get(k, 0) != gb.get(k, 0)} }")
        print(f"## ext_notifications(new)={sorted(cb['ext_notifications'])}")


main()
