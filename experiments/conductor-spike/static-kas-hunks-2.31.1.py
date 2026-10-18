#!/usr/bin/env python3
"""Render norm.diff hunks from static-kas-bundle-diff-2.31.1.py with token-level context.

    python3 -I static-kas-hunks-2.31.1.py <bundle-out-dir>   (writes <dir>/hunks.txt)
"""
import re, difflib, sys
D = sys.argv[1]
oraw = open(f'{D}/old.raw').read().split('\n'); nraw = open(f'{D}/new.raw').read().split('\n')
tokre = re.compile(r'"(?:[^"\\]|\\.)*"|\'(?:[^\'\\]|\\.)*\'|[A-Za-z0-9_$]+|\s+|.')
def toks(s): return tokre.findall(s)
def norm_tok(t):
    return t
out = []
for l in open(f'{D}/norm.diff'):
    m = re.match(r'^(\d+)(?:,(\d+))?([acd])(\d+)(?:,(\d+))?$', l.strip())
    if not m: continue
    a1 = int(m.group(1)); a2 = int(m.group(2) or a1); op = m.group(3); b1 = int(m.group(4)); b2 = int(m.group(5) or b1)
    olines = [] if op == 'a' else oraw[a1-1:a2]
    nlines = [] if op == 'd' else nraw[b1-1:b2]
    out.append(f'@@ {l.strip()}  old#{a1} new#{b1}')
    ot = toks(' '.join(olines)); nt = toks(' '.join(nlines))
    if olines and nlines and len(ot) + len(nt) > 60:
        # token level diff on raw tokens with identifiers collapsed
        def key(t): return '$' if re.fullmatch(r'[A-Za-z_$][A-Za-z0-9_$]*', t) and not t[0].isupper() and len(t) <= 4 else t
        sm = difflib.SequenceMatcher(None, [key(t) for t in ot], [key(t) for t in nt], autojunk=False)
        for tag, i1, i2, j1, j2 in sm.get_opcodes():
            if tag == 'equal': continue
            ctx_b = ''.join(ot[max(0, i1-25):i1])[-160:]
            ctx_a = ''.join(ot[i2:i2+25])[:160]
            out.append(f'  [{tag}] ...{ctx_b}  ‹-{"".join(ot[i1:i2])[:1500]}›  ‹+{"".join(nt[j1:j2])[:3000]}›  {ctx_a}...')
    else:
        for x in olines: out.append('  - ' + x[:3000])
        for x in nlines: out.append('  + ' + x[:3000])
open(f'{D}/hunks.txt', 'w').write('\n'.join(out) + '\n')
print(len(out))
