#!/usr/bin/env python3
"""Role-aware independent oracle for compiled bundled-theme projections."""
from io import StringIO
from itertools import product
from pathlib import Path
from typing import NoReturn
import csv
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
FIXED = {"user": 12, "agent": 10, "system": 13}
MUTED = ("muted", "border", "subdued", "diff_context")
EXPECTED_SYNTAX = {"CyrilDark": "base16-eighties.dark"}
ANSI16 = [
    (0, 0, 0), (128, 0, 0), (0, 128, 0), (128, 128, 0),
    (0, 0, 128), (128, 0, 128), (0, 128, 128), (192, 192, 192),
    (128, 128, 128), (255, 0, 0), (0, 255, 0), (255, 255, 0),
    (0, 0, 255), (255, 0, 255), (0, 255, 255), (255, 255, 255),
]


def fail(claim: str, message: str) -> NoReturn:
    sys.stderr.write(f"{claim} FAIL {message}\n")
    raise SystemExit(1)


def run(command):
    result = subprocess.run(
        command, cwd=ROOT, capture_output=True, text=True, check=False
    )
    if result.returncode != 0:
        fail("PROBE", result.stderr)
    return result.stdout


def probe(test_name: str, begin: str, end: str) -> str:
    output = run([
        "cargo", "test", "-p", "cyril-ui", test_name,
        "--", "--exact", "--nocapture",
    ])
    block = re.search(rf"{begin}\s*(.*?)\s*{end}", output, re.S)
    if block is None:
        fail("PROBE", f"markers missing for {test_name}")
    return block.group(1).strip()


def distance(left, right):
    return sum((a - b) ** 2 for a, b in zip(left, right, strict=True))


def source_rgb(value):
    if value == "reset":
        return None
    try:
        return tuple(bytes.fromhex(value))
    except ValueError as error:
        fail("C3", f"invalid source color {value!r}: {error}")


def ansi256_index(rgb):
    levels = [0, 95, 135, 175, 215, 255]
    palette = [(16 + i, value) for i, value in enumerate(product(levels, repeat=3))]
    palette += [(232 + i, (8 + 10 * i,) * 3) for i in range(24)]
    return min(palette, key=lambda item: (distance(rgb, item[1]), item[0]))[0]


registry = run(["python", ".cyril-q9dx/registry-oracle.py"]).strip()
match = re.fullmatch(r"C0 PASS themes=(\d+) unique=(\d+) visits=(\d+)", registry)
if match is None or len(set(match.groups())) != 1:
    fail("C0", f"registry oracle disagreed: {registry!r}")
try:
    theme_count = int(match.group(1))
except (TypeError, ValueError) as error:
    fail("C0", f"invalid registry count: {error}")

source_first = probe(
    "theme::tests::emit_source_probe", "BEGIN_THEME_PROBE", "END_THEME_PROBE"
)
source_second = probe(
    "theme::tests::emit_source_probe", "BEGIN_THEME_PROBE", "END_THEME_PROBE"
)
if source_first != source_second:
    fail("C6", "fresh compiled source probes differ")
rows = list(csv.DictReader(StringIO(source_first), delimiter="\t"))
themes = sorted({row["theme"] for row in rows})
if len(themes) != theme_count or set(themes) != set(EXPECTED_SYNTAX):
    fail("C0", f"registry themes={theme_count}, probe themes={themes!r}")
if len(rows) != theme_count * 29:
    fail("C3", f"expected {theme_count * 29} role rows, got {len(rows)}")

speaker_checked = ansi16_checked = ansi256_checked = stable_other = 0
cyril_dark_changes = 0
syntax_checked = set()
for row in rows:
    role, theme = row["role"], row["theme"]
    rgb = source_rgb(row["source"])
    expected256 = "reset" if rgb is None else str(ansi256_index(rgb))
    if row["ansi256"] != expected256:
        fail("C5", f"theme={theme} role={role} ansi256={row['ansi256']} expected={expected256}")
    if rgb is not None:
        ansi256_checked += 1
    geometric16 = "reset" if rgb is None else str(
        min(range(16), key=lambda i: (distance(rgb, ANSI16[i]), i))
    )
    expected16 = str(FIXED[role]) if role in FIXED else geometric16
    if row["ansi16"] != expected16:
        fail("C1" if role in FIXED else "C5", f"theme={theme} role={role} ansi16={row['ansi16']} expected={expected16}")
    if role in FIXED:
        speaker_checked += 1
        if theme == "CyrilDark" and row["ansi16"] != geometric16:
            cyril_dark_changes += 1
    else:
        stable_other += 1
        if rgb is not None:
            ansi16_checked += 1
    if row["no_color"] != "reset":
        fail("C3", f"theme={theme} role={role} no_color={row['no_color']}")
    if row["color_syntax"] != EXPECTED_SYNTAX[theme] or row["no_color_syntax"] != "none":
        fail("C7", f"theme={theme} syntax={row['color_syntax']}/{row['no_color_syntax']}")
    syntax_checked.add(theme)

collision_block = probe(
    "theme::tests::emit_ansi16_collision_probe",
    "BEGIN_ANSI16_COLLISION_PROBE",
    "END_ANSI16_COLLISION_PROBE",
)
collision_rows = list(csv.DictReader(StringIO(collision_block), delimiter="\t"))
expected_collisions = {(role, str(color)) for role in MUTED for color in FIXED.values()}
actual_collisions = {
    (row["input_role"], row["input_color"]) for row in collision_rows
    if row["input_role"] == row["result_role"] and row["input_color"] == row["result_color"]
}
if actual_collisions != expected_collisions or len(collision_rows) != 12:
    fail("C2", f"expected={sorted(expected_collisions)!r} actual={collision_rows!r}")

tie_block = probe(
    "theme::tests::emit_ansi16_tie_probe", "BEGIN_ANSI16_TIE_PROBE", "END_ANSI16_TIE_PROBE"
)
tie_rows = list(csv.DictReader(StringIO(tie_block), delimiter="\t"))
if tie_rows != [{"rgb": "400000", "index": "0"}]:
    fail("C5", f"tie rows={tie_rows!r}")

if cyril_dark_changes != 3:
    fail("C3", f"CyrilDark changed speaker rows={cyril_dark_changes}")
sys.stdout.write(registry + "\n")
sys.stdout.write(f"C1 PASS speaker_rows={speaker_checked}\n")
sys.stdout.write("C2 PASS collisions=12\n")
sys.stdout.write(
    f"C3 PASS non_speaker_ansi16={stable_other} other_mode_rows={theme_count * 87}\n"
)
sys.stdout.write(
    f"C5 PASS ansi16_nearest={ansi16_checked} ansi256_nearest={ansi256_checked} tie_index=0\n"
)
sys.stdout.write(f"C6 PASS deterministic_runs=2 bytes={len(source_first.encode('utf-8'))}\n")
sys.stdout.write(f"C7 PASS syntax_themes={len(syntax_checked)}\n")
