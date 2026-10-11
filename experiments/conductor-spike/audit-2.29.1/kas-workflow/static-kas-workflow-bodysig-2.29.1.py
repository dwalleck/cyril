#!/usr/bin/env python3
"""Minifier-stable structural signature for a JS body slice:
ordered sequence of (string literals | .property accesses | label:). Identifiers (locals) are
renamed by the minifier and are NOT included, so an identical signature across bundles is strong
evidence the body is equivalent modulo renames."""
import re, sys, json
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')

def slice_from(text, key):
    """from the handler-object key, scan to the matching close of its arrow body."""
    i=text.find(key)
    if i<0: return None
    # find first '{' after the key
    j=text.index('{', i)
    depth=0; k=j; n=len(text); instr=None
    while k<n:
        c=text[k]
        if instr:
            if c=='\\': k+=2; continue
            if c==instr: instr=None
        else:
            if c in '"\'`': instr=c
            elif c=='{' or c=='(' or c=='[': depth+=1
            elif c=='}' or c==')' or c==']':
                depth-=1
                if depth==0: return text[i:k+1]
        k+=1
    return text[i:i+4000]

TOK=re.compile(r"""(["'`])(?:\\.|(?!\1).)*\1|\.([A-Za-z_$][\w$]*)|([A-Za-z_$][\w$]*)\s*:""", re.S)
def sig(body):
    out=[]
    for m in TOK.finditer(body):
        if m.group(1):
            s=m.group(0); 
            s=re.sub(r'\$\{[^}]*\}','${}',s)  # normalize template holes
            out.append(s)
        elif m.group(2): out.append('.'+m.group(2))
        else: out.append(m.group(3)+':')
    return out

for label,key in (('load','"_kiro/workflow/load":async'),('inspect','"_kiro/workflow/inspect":async'),
                  ('resume','"_kiro/workflow/resume":async'),('new','"_kiro/workflow/new":async')):
    bo=slice_from(old,key); bn=slice_from(new,key)
    if bo is None or bn is None:
        print(f"[{label}] slice missing OLD={bo is not None} NEW={bn is not None}"); continue
    so,sn=sig(bo),sig(bn)
    same = so==sn
    print(f"\n=== {label}: OLD len={len(bo)} NEW len={len(bn)} sig_tokens {len(so)} vs {len(sn)}  IDENTICAL={same}")
    if not same:
        import difflib
        d=[x for x in difflib.unified_diff(so,sn,lineterm='',n=1)][:60]
        print('\n'.join(d))
    else:
        print("   first 24 sig tokens:", so[:24])
    Path(f"raw-bodysig-{label}.txt").write_text(json.dumps({'old':so,'new':sn,'identical':same},indent=0))

# allow_always / maxRounds loop
for label,anch in (('maxRounds','maxRounds'),('allow_always','allow_always')):
    io=old.find(anch); inn=new.find(anch)
    print(f"\n=== {label}: OLD@{io} NEW@{inn}")
    for tag,t,i in (('OLD',old,io),('NEW',new,inn)):
        if i<0: print(f"  [{tag}] absent"); continue
        print(f"  [{tag}] "+t[max(0,i-260):i+360].replace('\n',' '))
