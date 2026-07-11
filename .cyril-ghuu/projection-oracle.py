#!/usr/bin/env python3
"""Compare compiled theme rows with the two signed role specifications."""
from itertools import product
from pathlib import Path
import argparse
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
OLD_NAMES = {
    "Canvas background": "canvas",
    "Chrome background": "chrome",
    "Code background": "code",
    "Selection background": "selection",
    "Primary text": "text",
    "Muted text": "muted",
    "Border": "border",
    "Primary accent": "accent",
    "Secondary accent": "accent_alt",
    "User message": "user",
    "Agent message": "agent",
    "System message": "system",
    "Info": "info",
    "Success": "success",
    "Warning": "warning",
    "Danger": "danger",
    "Diff addition": "diff_add",
    "Diff deletion": "diff_delete",
    "Diff context": "diff_context",
}
NEW_NAMES = {
    "Emphasis": "emphasis",
    "Tertiary accent": "accent_tertiary",
    "Quaternary accent": "accent_quaternary",
    "Quinary accent": "accent_quinary",
    "Subdued": "subdued",
    "Subdued positive": "subdued_positive",
    "Subdued negative": "subdued_negative",
    "Soft accent": "soft_accent",
    "Positive accent": "positive_accent",
    "Inset background": "inset_background",
}


def table(path: Path, names: dict[str, str]) -> list[tuple[str, tuple[int, int, int] | None]]:
    text = path.read_text(encoding="utf-8")
    rows = []
    for label, value in re.findall(r"\| ([^|]+?) \| `([^`]+)` \|", text):
        if label not in names:
            continue
        rgb = None if value == "Color::Reset" else tuple(bytes.fromhex(value.removeprefix("#")))
        rows.append((names[label], rgb))
    return rows


parser = argparse.ArgumentParser()
parser.add_argument("--input", required=True, type=Path)
parser.add_argument("--role-count", required=True, type=int, choices=(19, 24, 29))
args = parser.parse_args()
expected = (
    table(ROOT / ".cyril-ixua/spec.md", OLD_NAMES)
    + table(ROOT / ".cyril-ghuu/spec.md", NEW_NAMES)
)[: args.role_count]
expected_rgb = [(name, rgb) for name, rgb in expected if rgb is not None]

lines = args.input.read_text(encoding="utf-8").splitlines()
if not lines:
    sys.stderr.write("DISAGREE empty compiled probe\n")
    raise SystemExit(1)
header = lines[0].split("\t")
rows = [dict(zip(header, line.split("\t"), strict=True)) for line in lines[1:]]

levels = [0, 95, 135, 175, 215, 255]
xterm = [(16 + index, rgb) for index, rgb in enumerate(product(levels, repeat=3))]
xterm += [(232 + index, (8 + 10 * index,) * 3) for index in range(24)]
ansi16 = [
    (0, 0, 0), (128, 0, 0), (0, 128, 0), (128, 128, 0),
    (0, 0, 128), (128, 0, 128), (0, 128, 128), (192, 192, 192),
    (128, 128, 128), (255, 0, 0), (0, 255, 0), (255, 255, 0),
    (0, 0, 255), (255, 0, 255), (0, 255, 255), (255, 255, 255),
]

def distance(left: tuple[int, ...], right: tuple[int, ...]) -> int:
    return sum((a - b) ** 2 for a, b in zip(left, right, strict=True))


def integer_field(row: dict[str, str], field: str, role: str, errors: list[str]) -> int | None:
    try:
        return int(row[field])
    except (KeyError, ValueError) as error:
        errors.append(f"{role} {field} is invalid: {error}")
        return None


errors = []
actual_names = [row["role"] for row in rows]
expected_names = [name for name, _ in expected_rgb]
if actual_names != expected_names:
    errors.append(f"roles expected={expected_names!r} actual={actual_names!r}")
for row, (name, rgb) in zip(rows, expected_rgb, strict=False):
    actual_rgb = tuple(bytes.fromhex(row["rgb"]))
    if row["role"] != name or actual_rgb != rgb:
        errors.append(f"{name} rgb expected={rgb!r} actual={actual_rgb!r}")
    if "ansi256" in row:
        wanted = min(xterm, key=lambda item: (distance(actual_rgb, item[1]), item[0]))[0]
        actual = integer_field(row, "ansi256", name, errors)
        if actual is not None and actual != wanted:
            errors.append(f"{name} ansi256 expected={wanted} actual={actual}")
    if "ansi16" in row:
        wanted = min(range(16), key=lambda index: (distance(actual_rgb, ansi16[index]), index))
        actual = integer_field(row, "ansi16", name, errors)
        if actual is not None and actual != wanted:
            errors.append(f"{name} ansi16 expected={wanted} actual={actual}")
if errors:
    sys.stderr.write("DISAGREE compiled-theme\n" + "\n".join(errors) + "\n")
    raise SystemExit(1)
print(f"AGREE compiled-theme roles={args.role_count} rgb={len(expected_rgb)}")
