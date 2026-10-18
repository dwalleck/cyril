#!/usr/bin/env python3
"""Rigorous body comparison: full lexical token stream incl. operators, numbers, keywords,
punctuation and template holes. Non-identifier lexemes must match exactly; identifiers may differ
ONLY by a single consistent bijective rename. Any change to an operator/number/string/keyword is reported.
"""
import re,json,difflib
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')
KEYWORDS=set("""return let const var if else for while do switch case default break continue function class
extends new delete typeof instanceof in of void this null true false async await try catch finally throw yield
import export static get set super""".split())
OPEN=set('{(['); CLOSE=set('})]')
def slice_body(text, anchor):
    i=text.find(anchor)
    if i<0: return None
    j=text.index('{', i+len(anchor)); depth=0;k=j;n=len(text);instr=None
    while k<n:
        c=text[k]
        if instr:
            if c=='\\': k+=2; continue
            if c==instr: instr=None
        else:
            if c in '"\'`': instr=c
            elif c in OPEN: depth+=1
            elif c in CLOSE:
                depth-=1
                if depth==0: return text[i:k+1]
        k+=1
    return text[i:i+4000]
# scanner: strings+template chunks opaque, identifiers/numbers/punct tokenized, template holes tokenized
SCAN=re.compile(r'''"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`|[A-Za-z_$][\w$]*|\d[\w.]*|[^\sA-Za-z0-9_$]''', re.S)
def toks(text):
    out=[]
    for m in SCAN.finditer(text):
        t=m.group(0)
        if t[0]=='`':
            # split template into literal chunks + ${...} expression tokens
            body=t[1:-1]; buf=''; k=0
            while k<len(body):
                if body[k]=='$' and k+1<len(body) and body[k+1]=='{':
                    if buf: out.append('`'+buf+'`'); buf=''
                    # find matching }
                    d=1; s=k+2
                    while s<len(body) and d: 
                        if body[s]=='{':d+=1
                        elif body[s]=='}':d-=1
                        s+=1
                    out.extend(toks(body[k+2:s-1])); k=s
                else: buf+=body[k]; k+=1
            if buf: out.append('`'+buf+'`')
            continue
        out.append(t)
    return out
def is_ident(t): return re.fullmatch(r'[A-Za-z_$][\w$]*', t) is not None
def compare(label, anchor):
    bo=slice_body(old,anchor); bn=slice_body(new,anchor)
    if bo is None or bn is None:
        print(f"[{label}] missing OLD={bo is not None} NEW={bn is not None}"); return
    Path(f'raw-lex-{label}-old.txt').write_text(bo); Path(f'raw-lex-{label}-new.txt').write_text(bn)
    a,b=toks(bo),toks(bn)
    sm=difflib.SequenceMatcher(a=a,b=b,autojunk=False)
    mapping={}; problems=[]; nrep=0
    for tag,i1,i2,j1,j2 in sm.get_opcodes():
        if tag=='equal': continue
        la,lb=a[i1:i2],b[j1:j2]
        if tag=='replace' and len(la)==len(lb):
            for x,y in zip(la,lb):
                if x==y: continue
                if is_ident(x) and is_ident(y) and x not in KEYWORDS and y not in KEYWORDS:
                    if mapping.get(x,y)!=y and x in mapping and mapping[x]!=y:
                        problems.append(f"inconsistent rename {x}->{mapping[x]} and {x}->{y}")
                    mapping[x]=y; nrep+=1
                else:
                    problems.append(f"NON-IDENT change: {x!r} -> {y!r}")
        else:
            problems.append(f"{tag}: OLD{list(la)[:8]} NEW{list(lb)[:8]}")
    # bijection check
    vals=list(mapping.values())
    if len(vals)!=len(set(vals)): problems.append("rename not injective")
    print(f"\n[{label}] len OLD={len(a)} NEW={len(b)} tokens; renames={nrep}; PROBLEMS={len(problems)}")
    for p in problems[:12]: print("   !",p)
    if not problems: print("   => pure consistent rename; all operators/numbers/strings/keywords/punct IDENTICAL")
    Path(f'raw-lex-{label}.json').write_text(json.dumps({'problems':problems,'mapping':mapping},indent=0))
for lab,anch in (('load','"_kiro/workflow/load":async'),('inspect','"_kiro/workflow/inspect":async'),
                 ('resume','"_kiro/workflow/resume":async'),('new','"_kiro/workflow/new":async'),
                 ('sendParentNote','sendParentNote('),('reserveWakePrompt','reserveWakePrompt('),
                 ('startInfoWake','startInfoWake(')):
    compare(lab,anch)
