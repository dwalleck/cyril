#!/usr/bin/env python3
import re,textwrap
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')
def sect(text,tag):
    print(f"\n##### {tag} '### 5.'..'### 7.' node docs")
    i=text.find('### 6.')
    while i>=0:
        seg=text[i:i+200]
        if 'include' in seg or 'parallel' in seg or 'watch' in seg or 'sequence' in seg or 'repeat' in seg:
            print("  @%d: %s"%(i,textwrap.shorten(text[i:i+320],320,placeholder=' …'))); break
        i=text.find('### 6.',i+1)
    else:
        print("  (no matching '### 6.' section)")
sect(old,'OLD'); sect(new,'NEW')
# find the '### N.' list in each and print their titles
for tag,text in (('OLD',old),('NEW',new)):
    print(f"\n### {tag} node-type section titles")
    for m in re.finditer(r'\\n### (\d+)\. ([a-z_]+)', text):
        print("   ", m.group(1), m.group(2))
print("\n### _kiro/session/notify payload builder")
for fn in ('cAt','v0t'):
    text = new if fn=='cAt' else old
    m=re.search(r'function '+fn+r'\(', text)
    if m:
        print(f"  [{fn}] {textwrap.shorten(text[m.start():m.start()+320],340,placeholder=' …')}")
