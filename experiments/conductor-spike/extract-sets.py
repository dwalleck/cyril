#!/usr/bin/env python3
"""Trap-3 set extractor: broad token families, raw text, both bundles, set diff."""
import re,sys
from pathlib import Path
PATS={
 'kiro_method': re.compile(r'_kiro/[A-Za-z_][A-Za-z0-9_/]*'),
 'kirodev_method': re.compile(r'_?kiro\.dev/[A-Za-z_][A-Za-z0-9_/]*'),
 'KIRO_ENV': re.compile(r'\bKIRO_[A-Z0-9_]+'),
 'AB_FLAG': re.compile(r'\bAB_[A-Z0-9_]+'),
 'AMAZON_ENV': re.compile(r'\b(AMAZON_Q|AWS|Q)_[A-Z0-9_]+'),
 'slashcmd': re.compile(r'''["'`](/(?:[a-z][a-z0-9-]{1,30}))["'`]'''),
 'sessionUpdate_kind': re.compile(r'sessionUpdate:\s*"([a-zA-Z_]+)"'),
 'sessinfo_kind': re.compile(r'kind:\s*"([a-z_]+)"'),
 'ext_method': re.compile(r'(?:sendExtMethod|extMethod|callExtMethod|_kiro[A-Za-z]*)\(\s*"([A-Za-z_/]+)"'),
}
def collect(t):
    out={}
    for name,rx in PATS.items():
        out[name]=set(m.group(1) if m.groups() else m.group(0) for m in rx.finditer(t))
    return out
def main():
    a=Path(sys.argv[1]); b=Path(sys.argv[2])
    ta=a.read_text(errors='replace'); tb=b.read_text(errors='replace')
    A=collect(ta); B=collect(tb)
    rep=[]
    for name in PATS:
        only_a=sorted(x for x in A[name]-B[name] if x not in tb)
        only_b=sorted(x for x in B[name]-A[name] if x not in ta)
        rep.append(f"## {name}: 2.28.0={len(A[name])} 2.29.1={len(B[name])} only-old={only_a} only-new={only_b}")
        print(rep[-1])
    Path('sets-diff.txt').write_text("\n".join(rep)+"\n")
    # full dumps
    for name in PATS:
        Path(f'sets-2.28.0.{name}.txt').write_text("\n".join(sorted(A[name]))+"\n")
        Path(f'sets-2.29.1.{name}.txt').write_text("\n".join(sorted(B[name]))+"\n")
main()
