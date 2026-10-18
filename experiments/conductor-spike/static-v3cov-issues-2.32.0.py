#!/usr/bin/env python3
"""v3-coverage lane: grep .rivets/issues.jsonl (title+description+notes) for each
regex argument and print id/status/priority/title. Read-only.
usage: static-v3cov-issues-2.32.0.py <repo-root> REGEX...
"""
import json, re, sys
root, pats = sys.argv[1], sys.argv[2:]
iss = [json.loads(l) for l in open(f"{root}/.rivets/issues.jsonl") if l.strip()]
for pat in pats:
    rx = re.compile(pat, re.I)
    print("=== ", pat)
    for i in iss:
        txt = i.get("title", "") + " " + (i.get("description") or "") + " " + json.dumps(i.get("notes", ""))
        if rx.search(txt):
            print(f"  {i['id']} [{i.get('status')}] P{i.get('priority')} {','.join(i.get('labels') or [])[:40]} | {i.get('title','')[:105]}")
