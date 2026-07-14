#!/usr/bin/env python3
"""Inventory legacy color sources after compiler-toolchain Rust parsing."""
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FILES = [
    "crates/cyril-ui/src/widgets/chat.rs",
    "crates/cyril-ui/src/widgets/markdown.rs",
    "crates/cyril-ui/src/widgets/input.rs",
    "crates/cyril-ui/src/widgets/suggestions.rs",
    "crates/cyril-ui/src/highlight.rs",
]
palette_source = (ROOT / "crates/cyril-ui/src/palette.rs").read_text(encoding="utf-8")
palette_colors = re.findall(r"pub const (\w+): Color", palette_source)
color_pattern = re.compile(
    rf"Color::[A-Za-z0-9_]+|palette::({'|'.join(palette_colors)})"
)
rows = []
for relative in FILES:
    command = ["rustfmt", "--edition", "2024", "--emit", "stdout", relative]
    result = subprocess.run(
        command, cwd=ROOT, check=True, capture_output=True, text=True, encoding="utf-8"
    )
    _, separator, formatted = result.stdout.partition("\n\n")
    if not separator:
        raise SystemExit(f"rustfmt emitted no source body for {relative}")
    production = formatted.partition("#[cfg(test)]")[0]
    for line, text in enumerate(production.splitlines(), start=1):
        for match in color_pattern.finditer(text):
            rows.append((relative, line, match.group(0)))
output = "".join(
    f"{path}\t{line}\t{token}\n" for path, line, token in sorted(set(rows))
)
sys.stdout.buffer.write(output.encode("utf-8"))
