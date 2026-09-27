"""Pin the kiro-cli release that renamed the /context item flag.

Counts the two candidate serde field-name literals in each archived
`kiro-cli-chat` (the v1/v2 engine binary) and `kiro-cli` launcher under
`~/.local/share/kiro-research/binaries/<ver>/` (see CLAUDE.md, Research
archive). A serde field name survives as a plain byte literal even in a
stripped binary, so a byte count is a sufficient oracle here.

Usage: python3 pin-rename.py 2.12.0 2.13.0 ... 2.17.0
Output for cyril-jhmi is recorded in pin-rename-output.txt.
"""

import os
import sys

root = os.path.expanduser("~/.local/share/kiro-research/binaries")
versions = sys.argv[1:]
needles = [b"autoIncluded", b"auto_included"]
for ver in versions:
    d = os.path.join(root, ver)
    if not os.path.isdir(d):
        print(f"{ver}: (no dir)")
        continue
    for name in ("kiro-cli-chat", "kiro-cli"):
        p = os.path.join(d, name)
        if not os.path.exists(p):
            continue
        try:
            data = open(p, "rb").read()
        except OSError as e:
            print(f"{ver}/{name}: read error {e}")
            continue
        counts = {n.decode(): data.count(n) for n in needles}
        print(f"{ver}/{name}: size={len(data) // 1_000_000}MB {counts}")
