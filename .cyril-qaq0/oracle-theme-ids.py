#!/usr/bin/env python3
"""Independent table of the operator-facing appearance spellings.

This is an oracle, not an implementation: it encodes the accepted values from
`.cyril-qaq0/spec.md` directly, so a Rust mapping that drifts from the spec is
detected by comparing the two rather than by reading the Rust code.

Usage:
  python3 oracle-theme-ids.py            # print the accepted sets as TSV
  python3 oracle-theme-ids.py --check    # self-check the table's shape
"""
from __future__ import annotations

import sys

THEME_IDS = [
    "cyril-dark",
    "cyril-light",
    "high-contrast-dark",
    "high-contrast-light",
    "catppuccin-mocha",
    "gruvbox-dark",
]

COLOR_MODES = [
    "automatic",
    "truecolor",
    "ansi256",
    "ansi16",
    "none",
]

# Spellings the spec's examples and edge rows use but that are NOT accepted.
REJECTED_THEMES = ["", "CyrilDark", "cyril_dark", "cyril-dark ", "solarized"]
REJECTED_MODES = ["", "auto", "24bit", "ANSI256", "none "]


def rows() -> list[tuple[str, str, str]]:
    out: list[tuple[str, str, str]] = []
    for value in THEME_IDS:
        out.append(("theme", value, "accepted"))
    for value in COLOR_MODES:
        out.append(("color_mode", value, "accepted"))
    for value in REJECTED_THEMES:
        out.append(("theme", value, "rejected"))
    for value in REJECTED_MODES:
        out.append(("color_mode", value, "rejected"))
    return out


def self_check() -> int:
    problems: list[str] = []
    if len(set(THEME_IDS)) != len(THEME_IDS):
        problems.append("theme ids must be unique")
    if len(set(COLOR_MODES)) != len(COLOR_MODES):
        problems.append("color modes must be unique")
    for value in THEME_IDS + COLOR_MODES:
        if value != value.strip() or value != value.lower():
            problems.append(f"{value!r} must be lowercase and trimmed")
        if " " in value or "_" in value:
            problems.append(f"{value!r} must be kebab-case")
    overlap = set(THEME_IDS) & set(COLOR_MODES)
    if overlap:
        problems.append(f"theme ids and modes must not overlap: {sorted(overlap)}")
    if problems:
        for problem in problems:
            print(f"FAIL {problem}")
        return 1
    print(
        f"PASS {len(THEME_IDS)} theme ids, {len(COLOR_MODES)} color modes, "
        f"{len(REJECTED_THEMES) + len(REJECTED_MODES)} rejected spellings"
    )
    return 0


def main() -> int:
    if "--check" in sys.argv[1:]:
        return self_check()
    for field, value, verdict in rows():
        print(f"{field}\t{value}\t{verdict}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
