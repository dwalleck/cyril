#!/usr/bin/env python3
"""cyril-nrnq cheapest falsifier: every canonical modal legacy value must be
representable in the (expanded) role contract.

Independent oracle: canonical values derive from ghuu's NAMED table + the two
raw Rgb literals read straight from probe-styles.txt (frozen pre-design);
role values are parsed from theme.rs source text, NOT via the Rust compiler.

Negative control (--without-new-roles): drop the two proposed additions and
require the check to report exactly the two known-missing values — proving
the falsifier can fire.
"""
import re
import sys
from pathlib import Path

ROOT = Path("/home/dwalleck/repos/cyril")
NAMED = {  # ghuu canon (.cyril-ghuu/cheapest-falsifier.py) + Gray extension
    "Color::Red": (0x80, 0x00, 0x00),
    "Color::Green": (0x00, 0x80, 0x00),
    "Color::Yellow": (0x80, 0x80, 0x00),
    "Color::Cyan": (0x00, 0x80, 0x80),
    "Color::DarkGray": (0x80, 0x80, 0x80),
    "Color::White": (0xFF, 0xFF, 0xFF),
    "Color::Gray": (0xC0, 0xC0, 0xC0),
}
PROPOSED = {(0xC0, 0xC0, 0xC0), (0xB0, 0x8D, 0xFF)}  # text_secondary, accent_violet

# Required set: named colors observed in probe-styles.txt + raw Rgb literals.
probe = (ROOT / ".cyril-nrnq/probe-styles.txt").read_text()
required = set()
for name, rgb in NAMED.items():
    if name.split("::")[1] in probe:
        required.add(rgb)
for m in re.finditer(r"Rgb\((\d+), (\d+), (\d+)\)", probe):
    required.add(tuple(int(g) for g in m.groups()))

# Available set: RGB literals in the current cyril_dark_source + proposed.
theme_src = (ROOT / "crates/cyril-ui/src/theme.rs").read_text()
source_block = theme_src.split("fn cyril_dark_source")[1].split("/// Resolved")[0]
available = {
    tuple(int(g, 16) for g in m.groups())
    for m in re.finditer(
        r"SourceColor::Rgb\(0x([0-9a-fA-F]{2}), 0x([0-9a-fA-F]{2}), 0x([0-9a-fA-F]{2})\)",
        source_block,
    )
}
if "--without-new-roles" not in sys.argv:
    available |= PROPOSED

missing = sorted(required - available)
tag = " control=without-new-roles" if "--without-new-roles" in sys.argv else ""
print(f"required={len(required)} available={len(available)} missing={len(missing)}{tag}")
for rgb in missing:
    print(f"  missing #{rgb[0]:02x}{rgb[1]:02x}{rgb[2]:02x}")
sys.exit(0 if (len(missing) == (2 if tag else 0)) else 1)
