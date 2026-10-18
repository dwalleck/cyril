#!/usr/bin/env python3
import re,textwrap
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')
def find(text,a,w=300,n=2,post=None):
    print(f"\n##### {a!r}")
    c=0
    for m in re.finditer(re.escape(a),text):
        b=m.start()+post if post else m.end()+w
        print("  @%d: %s"%(m.start(),textwrap.shorten(text[max(0,m.start()-w):b],700,placeholder=' …')))
        c+=1
        if c>=n: print("   ...more"); break
# workflow method-array usage + gate
for tag,text in (('OLD',old),('NEW',new)):
    for m in re.finditer(r'v\$r|v9r', text):
        print(f"  [{tag}] {m.group(0)} @{m.start()}: "+textwrap.shorten(text[max(0,m.start()-200):m.end()+260],560,placeholder=' …'))
        break
find(new,'allow_always',360,3); find(old,'allow_always',360,3)
# consent loop cap
print("\n### consent loop / round cap")
for t in ('maxRounds','MAX_CONSENT','consentRounds','roundLimit','allow_always','alwaysAllow','allowAlways','consentRound'):
    print(f"  {t:20} {old.count(t):3}->{new.count(t):3}")
# the OLD request_permission meta variable p: what sets it?
print("\n### OLD: where does p (consentRound) come from in requestPermission meta")
for m in re.finditer('consentRound:p', old):
    print("  ", textwrap.shorten(old[max(0,m.start()-900):m.end()+60],900,placeholder=' …'))
    break
