#!/usr/bin/env python3
"""Extract the KAS configuration settings catalog (the `_kiro/configuration/state` item names) from each carve.

    python3 -I static-kas-core-catalog-3.1.0.py

Anchors on the literal `_parallelTasks:` option-builder call that opens the catalog object and brace-matches it;
prints every `<key>:<builder>("<name>", <default>` row and the set delta per consecutive carve. Executes nothing.
"""
import os, re
from pathlib import Path
K = Path(os.path.expanduser('~/.local/share/kiro-research/kas-carves'))
REL = 'node_modules/@kiro/agent/dist/server/acp-server.js'
CARVES = ['2.32.0', '2.33.0', '2.33.1', '2.34.0', '2.35.0', '3.1.0']
KAS = {'2.32.0': '0.66.43', '2.33.0': '0.66.48', '2.33.1': '0.66.58', '2.34.0': '0.66.62', '2.35.0': '0.66.70', '3.1.0': '0.66.91'}
def catalog(t):
    m = re.search(r'=\{_parallelTasks:[\w$]+\("_parallelTasks"', t)
    if not m: return None, {}
    i = m.start() + 1; d = 0; j = i
    while j < len(t):
        if t[j] == '{': d += 1
        elif t[j] == '}':
            d -= 1
            if d == 0: break
        j += 1
    blk = t[i:j+1]
    rows = {}
    for mm in re.finditer(r'([A-Za-z_$][\w$]*):([\w$]+)\("([A-Za-z_]+)"(?:,("?[\w.-]*"?))?(?:,\{hidden:(!0|!1)\})?', blk):
        rows[mm.group(1)] = (mm.group(3), (mm.group(4) or '').strip('"'), {'!0': 'hidden', '!1': 'visible', None: '?'}[mm.group(5)])
    # rows assigned from elsewhere (e.g. sessionToolsMessaging:P_.messaging.setting)
    for mm in re.finditer(r'([A-Za-z_$][\w$]*):([\w$]+(?:\.[\w$]+)+)(?=[,}])', blk):
        rows.setdefault(mm.group(1), ('<ref ' + mm.group(2) + '>', '', '?'))
    return blk, rows
R = {}
for v in CARVES:
    blk, rows = catalog((K / v / REL).read_text(encoding='utf-8', errors='replace'))
    R[v] = rows
    print(f'{KAS[v]}: {len(rows)} rows (block {len(blk) if blk else 0} chars)')
for x, y in zip(CARVES, CARVES[1:]):
    a, b = R[x], R[y]
    add = sorted(set(b) - set(a)); rem = sorted(set(a) - set(b))
    chg = sorted(k for k in set(a) & set(b) if a[k] != b[k])
    print(f'{KAS[x]} -> {KAS[y]}: +{[(k, b[k]) for k in add]} -{[(k, a[k]) for k in rem]} changed={[(k, a[k], b[k]) for k in chg]}')
print('\nNEW catalog:')
for k, v in R[CARVES[-1]].items(): print(f'  {k}: {v}')
