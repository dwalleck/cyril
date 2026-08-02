#!/usr/bin/env python3
"""Probe: what refusal wire keys does the v2 TUI (the metadata CONSUMER) handle?

Carves every `model_refusal` / `refusal` handling site out of the newest
sha-verified carved tui.js bundle (2.15.0) and prints (a) the raw snippets,
(b) the derived key set. Oracle is oracle-wire-refusal.sh (Rust PRODUCER
binary strings — independent artifact, independent language, independent
extraction mechanism).
"""

import re
import sys
from pathlib import Path

BUNDLE = Path.home() / ".local/share/kiro-research/tui-bundles/kiro-tui-2.15.0.js"

src = BUNDLE.read_text(errors="replace")
print(f"bundle: {BUNDLE.name} ({len(src)} bytes)")

# 1. Every model_refusal emit/consume site, with context.
sites = [m.start() for m in re.finditer(r"model_refusal", src)]
print(f"\nmodel_refusal sites: {len(sites)}")
for i, pos in enumerate(sites):
    snippet = src[max(0, pos - 260) : pos + 260].replace("\n", " ")
    print(f"\n--- site {i} @ {pos} ---\n{snippet}")

# 2. Derived key set: identifiers that appear inside refusal destructuring
#    or object literals within the carved snippets.
keys = set()
for pos in sites:
    window = src[max(0, pos - 400) : pos + 400]
    keys.update(re.findall(r"\b(category|explanation|recommendedModel|stopReason|refusal)\b", window))
print(f"\nDERIVED KEYS: {sorted(keys)}")

# 3. stopReason literals associated with refusal handling anywhere in bundle.
lits = sorted(set(re.findall(r'"(CONTENT_FILTERED|refusal)"', src)))
print(f"STOP-REASON LITERALS PRESENT: {lits}")

# 4. Which notification method carries it? Find metadata handler mentioning refusal.
meta_sites = [
    m.start()
    for m in re.finditer(r"refusal", src)
    if "metadata" in src[max(0, m.start() - 2000) : m.start() + 2000].lower()
]
print(f"refusal mentions within 2k chars of 'metadata': {len(meta_sites)}")
sys.exit(0)
