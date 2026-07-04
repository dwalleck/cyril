#!/usr/bin/env python3
"""cyril-dcc6 probe A: proposed versioned-dir discovery logic, standalone.

Smallest question: which acp-server.js should cyril spawn on this machine?
Proposed logic (what the Rust fix would do):
  1. glob ~/.local/share/kiro-cli/kas/<ver>-<sha>/ dirs (skip *.lock)
  2. prefer the dir whose <ver> == `kiro-cli --version`; else newest by version
  3. fall back to the legacy unversioned kas/node_modules/... path
Oracle: the entry path kiro-cli itself spawns (captured from /proc by
oracle-kiro-cli-v3.py).
"""
import os, re, subprocess

KAS = os.path.expanduser("~/.local/share/kiro-cli/kas")
REL = "node_modules/@kiro/agent/dist/server/acp-server.js"

def vertuple(v):
    return tuple(int(x) for x in v.split("."))

cli = subprocess.run(["kiro-cli", "--version"], capture_output=True, text=True).stdout.split()
cli_ver = cli[1] if len(cli) == 2 else None
print("kiro-cli version:", cli_ver)

candidates = []  # (version, server_path)
for name in sorted(os.listdir(KAS)) if os.path.isdir(KAS) else []:
    m = re.fullmatch(r"(\d+\.\d+\.\d+)-[0-9a-f]{64}", name)
    if not m:
        continue
    server = os.path.join(KAS, name, REL)
    if os.path.isfile(server):
        candidates.append((m.group(1), server))
print("versioned candidates:", [(v, "...present") for v, _ in candidates])

chosen = None
exact = [s for v, s in candidates if v == cli_ver]
if exact:
    chosen, how = exact[0], "exact version match"
elif candidates:
    chosen, how = max(candidates, key=lambda c: vertuple(c[0]))[1], "newest versioned"
else:
    legacy = os.path.join(KAS, REL)
    if os.path.isfile(legacy):
        chosen, how = legacy, "legacy unversioned fallback"
    else:
        how = "NOT FOUND"

print(f"\nPROBE A CHOSEN ({how}):\n  {chosen}")
