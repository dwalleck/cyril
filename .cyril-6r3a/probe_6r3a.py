#!/usr/bin/env python3
"""cyril-6r3a probe: who consumes each palette item, production vs test?

Scans every .rs file in crates/ for references to the 8 palette items and
`use ...palette` imports, classifying each hit by whether it falls before
or after the file's first `#[cfg(test)]` marker (production vs test).
"""
import re
from pathlib import Path

ITEMS = ["USER_BLUE", "AGENT_GREEN", "SYSTEM_MAUVE", "MUTED_GRAY",
         "CODE_BLOCK_BG", "MAX_BORDER_WIDTH", "SPINNER_CHARS", "SPINNER_FRAME_MS"]

hits = {}   # (item, file, section) -> count
imports = []
for path in sorted(Path("crates").rglob("*.rs")):
    if path.name == "palette.rs":
        continue
    text = path.read_text()
    cut = text.find("#[cfg(test)]")
    for m in re.finditer(r"\bpalette\b", text):
        line = text[:m.start()].count("\n") + 1
        section = "prod" if (cut == -1 or m.start() < cut) else "test"
        snippet = text.splitlines()[line - 1].strip()
        if snippet.startswith("use ") or "palette::" in snippet or "palette;" in snippet:
            pass
        item = next((i for i in ITEMS if i in snippet), None)
        if snippet.startswith("use "):
            imports.append(f"{path}:{line} [{section}] {snippet}")
        elif item:
            key = (item, str(path), section)
            hits[key] = hits.get(key, 0) + 1

print("== imports of the palette module ==")
for imp in imports:
    print(" ", imp)
print("== item consumers ==")
for item in ITEMS:
    rows = [(f, s, c) for (i, f, s), c in sorted(hits.items()) if i == item]
    total_prod = sum(c for _, s, c in rows if s == "prod")
    total_test = sum(c for _, s, c in rows if s == "test")
    print(f"{item}: prod={total_prod} test={total_test}")
    for f, s, c in rows:
        print(f"    {f} [{s}] x{c}")
