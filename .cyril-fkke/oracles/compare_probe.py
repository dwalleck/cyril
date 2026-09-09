#!/usr/bin/env python3
"""Compare the compiled Rust palette probe against the independent Python oracle.

Runs `cargo test -p cyril-ui --lib -- --nocapture emit_source_probe`, parses the
`BEGIN_THEME_PROBE` block, and compares every row against
`.cyril-fkke/palette-oracle.py --emit`. Any divergence prints a localized
failure naming the claim (C1/C4) and the exact theme/role/column.

Usage: python3 .cyril-fkke/oracles/compare_probe.py
"""

from __future__ import annotations

import pathlib
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parents[2]
ORACLE = REPO / ".cyril-fkke" / "palette-oracle.py"


def oracle_rows() -> dict[tuple[str, str], tuple[str, str, str]]:
    out = subprocess.run(
        [sys.executable, str(ORACLE), "--emit"],
        check=True,
        capture_output=True,
        text=True,
        cwd=REPO,
    ).stdout
    rows = {}
    for line in out.splitlines()[1:]:
        palette, role, source, ansi256, ansi16 = line.split("\t")
        rows[(palette, role)] = (source, ansi256, ansi16)
    return rows


def rust_rows() -> tuple[dict[tuple[str, str], tuple[str, str, str]], list[tuple[str, str]]]:
    out = subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "cyril-ui",
            "--lib",
            "--",
            "--nocapture",
            "emit_source_probe",
        ],
        check=True,
        capture_output=True,
        text=True,
        cwd=REPO,
    ).stdout
    rows: dict[tuple[str, str], tuple[str, str, str]] = {}
    no_color: list[tuple[str, str]] = []
    inside = False
    for line in out.splitlines():
        if line.strip() == "BEGIN_THEME_PROBE":
            inside = True
            continue
        if line.strip() == "END_THEME_PROBE":
            inside = False
            continue
        if not inside or line.startswith("theme\t"):
            continue
        theme, role, source, ansi256, ansi16, no_color_value, color_syntax, no_color_syntax = (
            line.split("\t")
        )
        rows[(theme, role)] = (source, ansi256, ansi16)
        no_color.append((f"{theme}/{role}", no_color_value))
        if color_syntax == "none" or no_color_syntax != "none":
            print(f"FAIL\tC4\t{theme}/{role}\tsyntax columns: {color_syntax}/{no_color_syntax}")
            sys.exit(1)
    return rows, no_color


def main() -> int:
    oracle = oracle_rows()
    rust, no_color = rust_rows()

    failures = []
    for key in sorted(set(oracle) | set(rust)):
        expected = oracle.get(key)
        actual = rust.get(key)
        if expected is None:
            failures.append(f"C1\t{key[0]}/{key[1]}\tmissing from oracle")
            continue
        if actual is None:
            failures.append(f"C1\t{key[0]}/{key[1]}\tmissing from compiled probe")
            continue
        for column, (want, got) in zip(("source", "ansi256", "ansi16"), zip(expected, actual)):
            if want != got:
                failures.append(f"C4\t{key[0]}/{key[1]}\t{column}: oracle {want}, rust {got}")
        if "unexpected:" in "".join(actual):
            failures.append(f"C1\t{key[0]}/{key[1]}\tunexpected projection value {actual}")

    for label, value in no_color:
        if value != "reset":
            failures.append(f"C4\t{label}\tno-color role is {value}, expected reset")

    for failure in failures:
        print(f"FAIL\t{failure}")
    if failures:
        return 1
    print(f"PASS\t{len(rust)} rows agree with the independent oracle")
    return 0


if __name__ == "__main__":
    sys.exit(main())
