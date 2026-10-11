#!/usr/bin/env python3
"""Minifier-invariant skeleton diff: identifiers -> I, numbers -> N, strings -> S(content),
templates -> T(content with identifiers normalised to I), comments dropped.
Compares token multisets so renames/LTO re-glue vanish and only logic/literal changes remain."""
import re,sys
from collections import Counter
from pathlib import Path

IDENT=re.compile(r'[A-Za-z_$][\w$]*')
NUM=re.compile(r'\d[\w.]*')
OP=re.compile(r'[^\sA-Za-z0-9_$"\'`]+')

def tokens(t):
    out=[];i=0;n=len(t)
    while i<n:
        c=t[i]
        if c in ' \t\r\n': i+=1; continue
        if c=='/' and i+1<n and t[i+1]=='/':
            j=t.find('\n',i); i=n if j<0 else j; continue
        if c=='/' and i+1<n and t[i+1]=='*':
            j=t.find('*/',i+2); i=n if j<0 else j+2; continue
        if c in '"\'`':
            q=c;j=i+1
            while j<n:
                if t[j]=='\\': j+=2; continue
                if t[j]==q or t[j]=='\n': break
                j+=1
            body=t[i+1:j]
            # normalise identifiers inside template code to I so renames vanish
            body=IDENT.sub('I',body)
            out.append(('T' if q=='`' else 'S')+body[:120])
            i=j+1; continue
        m=IDENT.match(t,i)
        if m: out.append('I'); i=m.end(); continue
        m=NUM.match(t,i)
        if m: out.append('N'); i=m.end(); continue
        m=OP.match(t,i)
        if m: out.append(m.group(0)); i=m.end(); continue
        i+=1
    return out

a=Counter(tokens(Path(sys.argv[1]).read_text(errors='replace')))
b=Counter(tokens(Path(sys.argv[2]).read_text(errors='replace')))
only_a=b-a; only_b=a-b  # Counter subtraction: positive counts are extras
extra_b=[(k,v) for k,v in (b-a).items() if v>0]
extra_a=[(k,v) for k,v in (a-b).items() if v>0]
print(f"tokens: 2.28.0={sum(a.values())} 2.29.1={sum(b.values())}")
print(f"tokens present only/extra in 2.29.1: {len(extra_b)}")
for k,v in sorted(extra_b,key=lambda x:-x[1]):
    if k.startswith('S') or k.startswith('T') or not k.isalpha():
        print(f"  +{v:6d} {k[:150]!r}")
print(f"tokens only in 2.28.0: {len(extra_a)}")
for k,v in sorted(extra_a,key=lambda x:-x[1]):
    print(f"  -{v:6d} {k[:150]!r}")
