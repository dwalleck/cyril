#!/usr/bin/env python3
"""Probe an exact one-key, comment-preserving theme update."""
from pathlib import Path
import re
import sys

HERE = Path(__file__).resolve().parent
source = HERE / "config-persist-fixture.toml"
target = HERE / "config-persist-output.toml"
text = source.read_text(encoding="utf-8")
pattern = re.compile(r'^(\s*theme\s*=\s*)"[^"]*"([^\r\n]*)$', re.MULTILINE)
matches = list(pattern.finditer(text))
if len(matches) != 1:
    raise SystemExit(f"expected exactly one theme assignment, found {len(matches)}")
updated = pattern.sub(r'\1"gruvbox-dark"\2', text, count=1)
with target.open("w", encoding="utf-8", newline="\n") as handle:
    handle.write(updated)
changed = [(a, b) for a, b in zip(text.splitlines(), updated.splitlines()) if a != b]
output = [f"QAQ0 persist_matches={len(matches)} changed_lines={len(changed)}"]
for before, after in changed:
    output.extend((f"QAQ0 before={before}", f"QAQ0 after={after}"))
sys.stdout.write("\n".join(output) + "\n")
