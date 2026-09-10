#!/usr/bin/env python3
import json, re, sys
from pathlib import Path

paths = [Path(p) for p in sys.argv[1:]] or [
 Path('/home/dwalleck/.local/share/kiro-research/kas-carves/2.21.1/2.21.0/tree/node_modules/@kiro/agent/dist/server/acp-server.js'),
 Path('/home/dwalleck/.local/share/kiro-research/kas-carves/2.21.1/2.21.1/tree/node_modules/@kiro/agent/dist/server/acp-server.js'),
]
out = Path('experiments/conductor-spike/static-2.21.1/static-kas-identifiers-delta.json')
valid = re.compile(r'[A-Za-z_][A-Za-z0-9_.:/@-]{2,120}$')
interesting = re.compile(r'(?i)(?:stall|reason|unified|agent|memory|skill|compact|permission|retry|workflow|hook|steer|model|tool|policy|output|turn|session|context)')

def get(path):
    text = path.read_text(encoding='utf-8', errors='replace')
    vals = set(); n = len(text); i = 0
    # Linear quoted-literal scanner. The KAS bundle is minified to a handful
    # of very long lines; a lookahead-heavy regex can become quadratic.
    while i < n:
        if text[i] not in ('"', "'"):
            i += 1; continue
        quote = text[i]; j = i + 1; escaped = False
        while j < n:
            c = text[j]
            if escaped: escaped = False
            elif c == '\\': escaped = True
            elif c == quote: break
            j += 1
        if j >= n: break
        value = text[i + 1:j]
        if 3 <= len(value) <= 120 and valid.fullmatch(value): vals.add(value)
        i = j + 1
    return sorted(vals)
old, new = [get(p) for p in paths]
A, B = set(old), set(new)
obj = {'old_count': len(A), 'new_count': len(B), 'added': sorted(B-A), 'removed': sorted(A-B),
       'relevant_added': sorted(x for x in B-A if interesting.search(x)),
       'relevant_removed': sorted(x for x in A-B if interesting.search(x))}
out.write_text(json.dumps(obj, indent=2, sort_keys=True))
print(json.dumps({k: obj[k] for k in ('old_count','new_count','relevant_added','relevant_removed')}, indent=2))
