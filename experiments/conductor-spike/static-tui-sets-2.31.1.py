#!/usr/bin/env python3
"""Trap-3 broad-family set extractor for tui.js bundles (kiro-cli 2.31.1 audit).

Generalises audit-2.29.1/tui/extract-sets.py: any number of bundles, version labels
from file names, consecutive pair diffs + first->last overall diff, extra families
(case-labels, type:"x", rollout gate checks, hidden chat _ subcommands).

Usage: python3 -I static-tui-sets-2.31.1.py OUTDIR BUNDLE_A BUNDLE_B [...]
Writes OUTDIR/sets-<ver>.<family>.txt and OUTDIR/sets-diff.txt. Read-only.
"""
import re, sys
from pathlib import Path

PATS = {
    'kiro_method': re.compile(r'_kiro/[A-Za-z_][A-Za-z0-9_/]*'),
    'kirodev_method': re.compile(r'_?kiro\.dev/[A-Za-z_][A-Za-z0-9_/]*'),
    'KIRO_ENV': re.compile(r'\bKIRO_[A-Z0-9_]+'),
    'AB_FLAG': re.compile(r'\bAB_[A-Z0-9_]+'),
    'AMAZON_ENV': re.compile(r'\b(?:AMAZON_Q|AWS|Q)_[A-Z0-9_]+'),
    'slashcmd': re.compile(r'''["'`](/(?:[a-z][a-z0-9-]{1,30}))["'`]'''),
    'slashcmd_name': re.compile(r'name:"(/[a-z][a-z0-9-]{1,30})"'),
    'sessionUpdate_kind': re.compile(r'sessionUpdate:\s*"([a-zA-Z_]+)"'),
    'kind_lit': re.compile(r'kind:\s*"([a-z_]+)"'),
    'kind_eq': re.compile(r'kind\s*===?\s*"([a-z_]+)"'),
    'type_eq': re.compile(r'type\s*===?\s*"([a-z_]+)"'),
    'case_snake': re.compile(r'case\s*"([a-z][a-z0-9]*(?:_[a-z0-9]+)+)"'),
    'rollout_check': re.compile(r'\b[A-Za-z$_][\w$]{0,3}\("((?:local_sandbox|background_execution|c2s|cloud_config|memory_controls|remote_sandbox|session_dashboard|workflows|memory|[a-z]+_[a-z_]+))"\)'),
    'chat_subcmd': re.compile(r'"chat","_","([a-z-]+)"'),
    'ext_method': re.compile(r'(?:sendExtMethod|extMethod|callExtMethod|sendExtNotification)\(\s*"([A-Za-z_./]+)"'),
}

def ver(p):
    m = re.search(r'(\d+\.\d+\.\d+)', p.name)
    return m.group(1) if m else p.stem

def collect(t):
    return {n: set(m.group(1) if m.groups() else m.group(0) for m in rx.finditer(t)) for n, rx in PATS.items()}

def main():
    out = Path(sys.argv[1]); out.mkdir(parents=True, exist_ok=True)
    bs = [Path(a) for a in sys.argv[2:]]
    T = {b: b.read_text(errors='replace') for b in bs}
    S = {b: collect(T[b]) for b in bs}
    for b in bs:
        for n in PATS:
            (out / f'sets-{ver(b)}.{n}.txt').write_text("\n".join(sorted(S[b][n])) + "\n")
    pairs = list(zip(bs, bs[1:]))
    if len(bs) > 2:
        pairs.append((bs[0], bs[-1]))
    rep = []
    for a, b in pairs:
        rep.append(f"# {ver(a)} -> {ver(b)}")
        for n in PATS:
            oa = sorted(x for x in S[a][n] - S[b][n] if x not in T[b])
            ob = sorted(x for x in S[b][n] - S[a][n] if x not in T[a])
            rep.append(f"## {n}: {len(S[a][n])}->{len(S[b][n])} only-old={oa} only-new={ob}")
    (out / 'sets-diff.txt').write_text("\n".join(rep) + "\n")
    print("\n".join(rep))

main()
