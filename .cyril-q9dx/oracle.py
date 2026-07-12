#!/usr/bin/env python3
"""Independently derive q9dx role projections from production source literals."""
from itertools import product
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "crates/cyril-ui/src/theme.rs"
PROBE = Path(__file__).with_name("probe-output.tsv")
OUTPUT = Path(__file__).with_name("oracle-output.tsv")
ROLE_ORDER = [
    "canvas", "chrome", "code", "selection", "text", "muted", "border",
    "accent", "accent_alt", "user", "agent", "system", "info", "success",
    "warning", "danger", "diff_add", "diff_delete", "diff_context",
    "emphasis", "accent_tertiary", "accent_quaternary", "accent_quinary",
    "subdued", "subdued_positive", "subdued_negative", "soft_accent",
    "positive_accent", "inset_background",
]
ANSI16 = [
    (0, 0, 0), (128, 0, 0), (0, 128, 0), (128, 128, 0),
    (0, 0, 128), (128, 0, 128), (0, 128, 128), (192, 192, 192),
    (128, 128, 128), (255, 0, 0), (0, 255, 0), (255, 255, 0),
    (0, 0, 255), (255, 0, 255), (0, 255, 255), (255, 255, 255),
]
ANSI16_NAMES = [
    "Black", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan", "Gray",
    "DarkGray", "LightRed", "LightGreen", "LightYellow", "LightBlue",
    "LightMagenta", "LightCyan", "White",
]
FIXED = {"user": "LightBlue", "agent": "LightGreen", "system": "LightMagenta"}


def distance(left, right):
    return sum((a - b) ** 2 for a, b in zip(left, right, strict=True))


def read_sources():
    text = SOURCE.read_text(encoding="utf-8")
    body = text[text.index("fn cyril_dark_source"):text.index("pub struct Theme")]
    found = {}
    for name, kind, values in re.findall(
        r"^\s+(\w+): SourceColor::(Rgb|Reset)(?:\(([^)]*)\))?,?$", body, re.M
    ):
        found[name] = None if kind == "Reset" else tuple(int(x, 0) for x in values.split(","))
    if list(found) != ROLE_ORDER:
        raise SystemExit(f"source roles differ: {list(found)!r}")
    return found


def project(rgb, mode):
    if rgb is None or mode == "none":
        return "Reset"
    if mode == "truecolor":
        return f"Rgb({rgb[0]}, {rgb[1]}, {rgb[2]})"
    if mode == "ansi16":
        index = min(range(16), key=lambda i: (distance(rgb, ANSI16[i]), i))
        return ANSI16_NAMES[index]
    levels = [0, 95, 135, 175, 215, 255]
    palette = [(16 + i, value) for i, value in enumerate(product(levels, repeat=3))]
    palette += [(232 + i, (8 + 10 * i,) * 3) for i in range(24)]
    index = min(palette, key=lambda item: (distance(rgb, item[1]), item[0]))[0]
    return f"Indexed({index})"


sources = read_sources()
expected = ["mode\trole\tbefore\tafter"]
for mode in ("truecolor", "ansi256", "ansi16", "none"):
    for role in ROLE_ORDER:
        before = project(sources[role], mode)
        after = FIXED[role] if mode == "ansi16" and role in FIXED else before
        expected.append(f"{mode}\t{role}\t{before}\t{after}")
recorded = OUTPUT.read_text(encoding="utf-8").splitlines()
if recorded != expected:
    raise SystemExit("independent oracle output is stale")
actual = PROBE.read_text(encoding="utf-8").splitlines()
if actual != expected:
    for index, (left, right) in enumerate(zip(actual, expected, strict=False), 1):
        if left != right:
            sys.stderr.write(
                f"DISAGREE line={index} probe={left!r} oracle={right!r}\n"
            )
    raise SystemExit(1)
changed = [line for line in expected[1:] if line.split("\t")[2] != line.split("\t")[3]]
sys.stdout.write(
    f"AGREE rows={len(expected) - 1} changed={len(changed)} "
    "roles=user,agent,system\n"
)
