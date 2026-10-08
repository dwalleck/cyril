#!/usr/bin/env python3
"""Settings-key census (chat.*, voice.*, trust.*, memory.* ...) across a chain of kiro-cli-chat builds (2.28.0 audit).

    python3 -I static-host-settings-2.28.0.py <chat1> <chat2> [...]

Takes every printable run (>=200 chars) holding >=5 `chat.` keys (the glued
settings tables), splits before a known top-level prefix (not after a dot, so
`chat.keybindings.x` stays whole) and diffs the key sets per consecutive pair.
Verify each add/remove with a raw substring count before reporting.
"""
import re, sys
from pathlib import Path
# find the settings-key table: anchor on 'trust.classifier' or 'chat.enableWorkflows'; take the printable run containing 'chat.showThinking'
def run(d, anchor):
    i = d.find(anchor)
    a = i
    while a > 0 and 0x20 <= d[a-1] <= 0x7e: a -= 1
    b = i
    while b < len(d) and 0x20 <= d[b] <= 0x7e: b += 1
    return d[a:b].decode()
PFX = r'(?:chat|voice|trust|telemetry|mcp|api|app|autocomplete|codeIntelligence|compaction|knowledge|cli|ui|ssh|update|introspect|kas|hooks|workflow|memory|context|model|agent|tools|notifications|keybindings|display|experiments|telemetryClientId|kiro|q|aws|sandbox|herdr|lite|tui)'
out = {}
for p in sys.argv[1:]:
    d = Path(p).read_bytes()
    keys = set()
    # broader: all maximal printable runs with >= 5 'chat.' occurrences
    for m in re.finditer(rb'[\x20-\x7e]{200,}', d):
        s = m.group().decode()
        if s.count('chat.') >= 5:
            for k in re.split(r'(?<!\.)(?=' + PFX + r'\.[a-zA-Z])', s):
                mm = re.match(PFX + r'\.[A-Za-z0-9_.]+', k)
                if mm: keys.add(mm.group().rstrip('.'))
    out[p] = keys
    print(p.split('/')[-5], len(keys))
ps = sys.argv[1:]
for a, b in zip(ps, ps[1:]):
    A, B = out[a], out[b]
    print('###', a.split('/')[-5], '->', b.split('/')[-5])
    for k in sorted(B - A): print('  +', k)
    for k in sorted(A - B): print('  -', k)
