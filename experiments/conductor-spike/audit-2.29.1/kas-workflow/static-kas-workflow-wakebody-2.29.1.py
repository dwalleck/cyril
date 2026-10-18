#!/usr/bin/env python3
import re,json,difflib
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')
TOK=re.compile(r"""(["'`])(?:\\.|(?!\1).)*\1|\.([A-Za-z_$][\w$]*)|([A-Za-z_$][\w$]*)\s*:""", re.S)
def sig(s):
    out=[]
    for m in TOK.finditer(s):
        if m.group(1): out.append(re.sub(r'\$\{[^}]*\}','${}',m.group(0)))
        elif m.group(2): out.append('.'+m.group(2))
        else: out.append(m.group(3)+':')
    return out
OPEN=set('{(['); CLOSE=set('})]')
def body(text, anchor):
    i=text.find(anchor)
    if i<0: return None
    j=text.index('{', i+len(anchor))
    depth=0; k=j; n=len(text); instr=None
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
    return text[i:i+3000]
def cmp(label, anchor):
    bo=body(old,anchor); bn=body(new,anchor)
    if bo is None or bn is None:
        print(f"[{label}] missing OLD={bo is not None} NEW={bn is not None}"); return
    so,sn=sig(bo),sig(bn)
    print(f"[{label}] OLD len={len(bo)} NEW len={len(bn)} sig {len(so)}/{len(sn)} IDENTICAL={so==sn}")
    if so!=sn: print('\n'.join(list(difflib.unified_diff(so,sn,lineterm='',n=1))[:40]))
for lab,a in (('sendParentNote','sendParentNote('),('reserveWakePrompt','reserveWakePrompt('),('startInfoWake','startInfoWake(')):
    cmp(lab,a)
i=new.find('reserveWakePrompt(')
print("\nNEW reserveWakePrompt usage ctx:", new[max(0,i-800):i+240].replace('\n',' ')[:1000])
