#!/usr/bin/env python3
import hashlib, json, re, sys
from pathlib import Path

roots = [Path(p) for p in sys.argv[1:]] or [
 Path('/home/dwalleck/.local/share/kiro-research/kas-carves/2.21.2/2.21.1/tree'),
 Path('/home/dwalleck/.local/share/kiro-research/kas-carves/2.21.2/2.21.2/tree'),
]
out = Path('experiments/conductor-spike/static-2.21.2/static-kas-surface.json')

def one(root):
    server = next(root.glob('node_modules/@kiro/agent/dist/server/acp-server.js'))
    text = server.read_text(encoding='utf-8', errors='replace')
    pkg = json.loads((root/'node_modules/@kiro/agent/package.json').read_text())
    # Quoted method literals are the KAS wire vocabulary. Restrict to quoted
    # literals so prose and concatenated strings do not become methods.
    methods = sorted(set(re.findall(r'["\'](_kiro/[A-Za-z0-9_./:-]+)["\']', text)))
    # Environment reads, not arbitrary mentions in docs/comments.
    env = sorted(set(re.findall(r'process\.env\.([A-Z][A-Z0-9_]+)', text)))
    # Keep the declaration diff useful and bounded.
    deps = {k: pkg.get(k, {}) for k in ('version','dependencies','peerDependencies')}
    return {'server': str(server), 'server_sha256': hashlib.sha256(server.read_bytes()).hexdigest(),
            'package': deps, 'method_literals': methods, 'env_reads': env}

old, new = [one(r) for r in roots]
delta = {}
for key in ('method_literals','env_reads'):
    A,B=set(old[key]),set(new[key]); delta[key]={'old_count':len(A),'new_count':len(B),'added':sorted(B-A),'removed':sorted(A-B)}
if old['package'] != new['package']:
    delta['package']={'old':old['package'],'new':new['package']}
obj={'old':old,'new':new,'delta':delta}
out.write_text(json.dumps(obj, indent=2, sort_keys=True))
print(json.dumps({'old':{'package_version':old['package']['version'],'server_sha256':old['server_sha256']},'new':{'package_version':new['package']['version'],'server_sha256':new['server_sha256']},'delta':delta}, indent=2))
