#!/usr/bin/env python3
"""Falsify whether the signed role set covers every fixed legacy color."""
import argparse
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument("--negative-control-26", action="store_true")
args = parser.parse_args()
NAMED = {
    "Color::Red": "#800000",
    "Color::Green": "#008000",
    "Color::Yellow": "#808000",
    "Color::Blue": "#000080",
    "Color::Magenta": "#800080",
    "Color::Cyan": "#008080",
    "Color::DarkGray": "#808080",
    "Color::White": "#ffffff",
}

def section_hex(path: Path, start: str, end: str) -> set[str]:
    text = path.read_text(encoding="utf-8")
    _, start_found, tail = text.partition(start)
    section, end_found, _ = tail.partition(end)
    if not start_found or not end_found:
        raise SystemExit(f"missing section markers in {path}")
    return {value.lower() for value in re.findall(r"`(#[0-9a-fA-F]{6})`", section)}


def decimal_byte(text: str) -> int:
    try:
        value = int(text)
    except ValueError as error:
        raise SystemExit(f"invalid RGB byte {text!r}: {error}") from error
    if not 0 <= value <= 255:
        raise SystemExit(f"RGB byte out of range: {value}")
    return value


palette_text = (ROOT / "crates/cyril-ui/src/palette.rs").read_text(encoding="utf-8")
palette = {
    name: f"#{decimal_byte(r):02x}{decimal_byte(g):02x}{decimal_byte(b):02x}"
    for name, r, g, b in re.findall(
        r"pub const (\w+): Color = Color::Rgb\((\d+), (\d+), (\d+)\)", palette_text
    )
}
tokens = {
    line.rstrip("\n").split("\t")[2]
    for line in (ROOT / ".cyril-ghuu/legacy-color-baseline.tsv")
    .read_text(encoding="utf-8")
    .splitlines()
}
required = set()
for token in tokens:
    if token in NAMED:
        required.add(NAMED[token])
    elif token.startswith("palette::"):
        required.add(palette[token.removeprefix("palette::")])
available = section_hex(
    ROOT / ".cyril-ixua/spec.md",
    "## Cyril Dark compatibility mapping",
    "## Success criteria",
) | section_hex(
    ROOT / ".cyril-ghuu/spec.md",
    "## Expanded semantic contract",
    "### Legacy-to-semantic mapping rules",
)
if args.negative_control_26:
    available -= {"#008000", "#008080", "#800000", "#808000"}
missing = sorted(required - available)
control = " control=26" if args.negative_control_26 else ""
print(
    f"required={len(required)} available={len(available)} missing={len(missing)}{control}"
)
for color in missing:
    sources = sorted(token for token in tokens if NAMED.get(token) == color)
    print(f"MISSING\t{color}\t{','.join(sources)}")
raise SystemExit(1 if missing else 0)
