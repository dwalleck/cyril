#!/usr/bin/env python3
"""Extract source-path/mangled-symbol leads without treating minifier strings as semantics."""
import hashlib, re, subprocess, sys
from pathlib import Path

old = Path(sys.argv[1]) if len(sys.argv) > 1 else Path('/home/dwalleck/.local/share/kiro-research/binaries/2.21.0/kiro-cli-chat')
new = Path(sys.argv[2]) if len(sys.argv) > 2 else Path('/home/dwalleck/.local/share/kiro-research/binaries/2.21.1/extracted/kiro-cli-chat')
out = Path(sys.argv[3]) if len(sys.argv) > 3 else Path('experiments/conductor-spike/static-2.21.1/static-host-paths-delta.json')

def extract(path):
    text = subprocess.run(['strings','-a','-n','4',str(path)], check=True, capture_output=True, text=True, errors='replace').stdout
    # Source path markers are emitted by Rust panic/event locations and are
    # much less collision-prone than words such as "model" or "rewind".
    src = sorted(set(re.findall(r'crates/(?:chat-cli|chat-cli-v2)/src/[A-Za-z0-9_./-]+\.rs', text)))
    mods = sorted(set(re.findall(r'\b(?:chat_cli|chat_cli_v2)::[A-Za-z0-9_:]+', text)))
    # Use full token extraction for env names, then compare set members; this
    # avoids concatenated strings from `strings` lines.
    env = sorted(set(re.findall(r'(?<![A-Z0-9_])KIRO_[A-Z0-9_]+(?![A-Z0-9_])', text)))
    # Source-side ACP namespace/path leads (not a claim of advertised methods).
    methods = sorted(set(re.findall(r'(?<![A-Za-z0-9_./:-])(?:_kiro\.dev|kiro\.dev)/(?:[A-Za-z0-9_./:-]+)', text)))
    return {'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'source_paths': src, 'module_tokens': mods, 'env': env, 'method_leads': methods}

a, b = extract(old), extract(new)
delta = {}
for key in a:
    if key == 'sha256': continue
    A, B = set(a[key]), set(b[key])
    delta[key] = {'old_count':len(A), 'new_count':len(B), 'added':sorted(B-A), 'removed':sorted(A-B)}
obj={'old':a,'new':b,'delta':delta}
out.write_text(__import__('json').dumps(obj, indent=2, sort_keys=True))
for key, value in delta.items():
    print(key, value['old_count'], '->', value['new_count'], '+', len(value['added']), '-', len(value['removed']))
