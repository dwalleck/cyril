#!/usr/bin/env python3
"""cyril-dij8 cheapest falsifier (claim C1): every canonical chrome legacy
value must already be representable in the 31-role contract — dij8 claims to
be a PURE re-mapping batch (no contract expansion).

Independent oracle: the required set derives from the ghuu NAMED canon plus
the raw Rgb literals read straight from probe-styles.txt (frozen pre-design);
role values are parsed from theme.rs source TEXT, not via the Rust compiler.

Negative control (--with-phantom): inject a phantom legacy value (#123456)
into the required set and require the check to report exactly that one value
missing — proving the falsifier can fire.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NAMED = {  # ghuu canon (.cyril-ghuu/cheapest-falsifier.py) + nrnq Gray + Magenta
    "Color::Red": (0x80, 0x00, 0x00),
    "Color::Green": (0x00, 0x80, 0x00),
    "Color::Yellow": (0x80, 0x80, 0x00),
    "Color::Cyan": (0x00, 0x80, 0x80),
    "Color::Magenta": (0x80, 0x00, 0x80),
    "Color::DarkGray": (0x80, 0x80, 0x80),
    "Color::White": (0xFF, 0xFF, 0xFF),
    "Color::Gray": (0xC0, 0xC0, 0xC0),
}

# Required set: named colors observed in probe-styles.txt + raw Rgb literals
# (chrome bg 30,30,46 and the three palette constants surface as raw Rgb).
probe = (ROOT / ".cyril-dij8/probe-styles.txt").read_text()
required = set()
for name, rgb in NAMED.items():
    if re.search(rf"\b{name.split('::')[1]}\|", probe):
        required.add(rgb)
for m in re.finditer(r"Rgb\((\d+), (\d+), (\d+)\)", probe):
    required.add(tuple(int(g) for g in m.groups()))
if "--with-phantom" in sys.argv:
    required.add((0x12, 0x34, 0x56))

# Available set: RGB literals parsed from the current cyril_dark_source text.
theme_src = (ROOT / "crates/cyril-ui/src/theme.rs").read_text()
source_block = theme_src.split("fn cyril_dark_source")[1].split("/// Resolved")[0]
available = {
    tuple(int(g, 16) for g in m.groups())
    for m in re.finditer(
        r"SourceColor::Rgb\(0x([0-9a-fA-F]{2}), 0x([0-9a-fA-F]{2}), 0x([0-9a-fA-F]{2})\)",
        source_block,
    )
}

missing = sorted(required - available)
print(f"required={len(required)} available_roles={len(available)} missing={len(missing)}")
for rgb in missing:
    print("MISSING #%02x%02x%02x" % rgb)
if "--with-phantom" in sys.argv:
    ok = missing == [(0x12, 0x34, 0x56)]
    print("NEGATIVE CONTROL:", "fires correctly" if ok else "FAILED TO FIRE")
    sys.exit(0 if ok else 1)
sys.exit(1 if missing else 0)
