#!/usr/bin/env python3
"""Strict-boundary KIRO_* env-token census across host binaries of two builds.

    static-host-env-2.24.0.py <old-bin-dir> <new-bin-dir>

Token = KIRO_[A-Z0-9_]+ whose preceding byte is not [A-Za-z0-9_]. Glue noise
(a token that is a prefix/suffix-merge of a longer token or ends in a glued
uppercase word) cannot be fully excluded statically; every reported add/remove
is re-checked as a substring count in the OTHER build so re-cuts show up as
'substring-present'. Writes static-2.24.0/static-host-env-delta.json.
"""
import json, re, sys
from pathlib import Path
old, new = Path(sys.argv[1]), Path(sys.argv[2])
BINS = ['kiro-cli', 'kiro-cli-chat', 'kiro-cli-term']
pat = re.compile(rb'(?<![A-Za-z0-9_])KIRO_[A-Z0-9_]{2,}')
def toks(d): 
    from collections import Counter
    return Counter(m.group().decode() for m in pat.finditer(d))
out = {}
for b in BINS:
    do, dn = (old/b).read_bytes(), (new/b).read_bytes()
    to, tn = toks(do), toks(dn)
    added = sorted(set(tn) - set(to)); removed = sorted(set(to) - set(tn))
    rec = {'old_count': len(to), 'new_count': len(tn),
           'added': [{'tok': t, 'n_new': tn[t], 'substring_in_old': do.count(t.encode())} for t in added],
           'removed': [{'tok': t, 'n_old': to[t], 'substring_in_new': dn.count(t.encode())} for t in removed]}
    out[b] = rec
    print(f'== {b}: {len(to)} -> {len(tn)}')
    for a in rec['added']: print(f"   + {a['tok']} (new x{a['n_new']}, substring in old x{a['substring_in_old']})")
    for r in rec['removed']: print(f"   - {r['tok']} (old x{r['n_old']}, substring in new x{r['substring_in_new']})")
Path('experiments/conductor-spike/static-2.24.0/static-host-env-delta.json').write_text(json.dumps(out, indent=2))
