#!/usr/bin/env python3
"""Compare cyril-qaq0's Rust appearance vocabulary against the independent oracles.

The Rust side emits a probe (`theme::tests::emit_appearance_probe`); the Python
side re-derives the accepted sets and the detection precedence from the spec's
tables. They must agree item by item.

Usage: python3 .cyril-qaq0/oracles/compare_appearance.py
"""
from __future__ import annotations

import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
ORACLE_DIR = ROOT / ".cyril-qaq0"


def rust_probe() -> dict[str, list[tuple[str, ...]]]:
    result = subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "cyril-ui",
            "--lib",
            "emit_appearance_probe",
            "--",
            "--nocapture",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print("FAIL rust probe did not run")
        print(result.stdout[-2000:])
        print(result.stderr[-2000:])
        raise SystemExit(1)
    lines = result.stdout.splitlines()
    try:
        start = lines.index("BEGIN_APPEARANCE_PROBE") + 1
        end = lines.index("END_APPEARANCE_PROBE")
    except ValueError:
        print("FAIL probe markers not found in the captured output")
        raise SystemExit(1)
    sections: dict[str, list[tuple[str, ...]]] = {}
    for line in lines[start:end]:
        fields = line.split("\t")
        sections.setdefault(fields[0], []).append(tuple(fields[1:]))
    return sections


def python_oracle(script: str) -> list[tuple[str, ...]]:
    result = subprocess.run(
        [sys.executable, str(ORACLE_DIR / script)],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"FAIL {script} did not run")
        print(result.stderr[-2000:])
        raise SystemExit(1)
    return [tuple(line.split("\t")) for line in result.stdout.splitlines() if line]


def main() -> int:
    probe = rust_probe()
    id_rows = python_oracle("oracle-theme-ids.py")
    precedence_rows = python_oracle("oracle-precedence.py")

    problems: list[str] = []
    rows = 0

    rust_theme_ids = [row[0] for row in probe.get("theme", [])]
    oracle_theme_ids = [row[1] for row in id_rows if row[0] == "theme" and row[2] == "accepted"]
    rows += len(oracle_theme_ids)
    if rust_theme_ids != oracle_theme_ids:
        problems.append(
            f"theme ids differ\n  rust:   {rust_theme_ids}\n  oracle: {oracle_theme_ids}"
        )

    rust_theme_rejected = {row[0] for row in probe.get("theme_rejected", [])}
    oracle_theme_rejected = {
        row[1] for row in id_rows if row[0] == "theme" and row[2] == "rejected"
    }
    rows += len(oracle_theme_rejected)
    if rust_theme_rejected != oracle_theme_rejected:
        problems.append(
            "rejected theme spellings differ\n"
            f"  rust:   {sorted(rust_theme_rejected)}\n"
            f"  oracle: {sorted(oracle_theme_rejected)}"
        )

    rust_mode_rejected = {row[0] for row in probe.get("mode_rejected", [])}
    oracle_mode_rejected = {
        row[1] for row in id_rows if row[0] == "color_mode" and row[2] == "rejected"
    }
    rows += len(oracle_mode_rejected)
    if rust_mode_rejected != oracle_mode_rejected:
        problems.append(
            "rejected color_mode spellings differ\n"
            f"  rust:   {sorted(rust_mode_rejected)}\n"
            f"  oracle: {sorted(oracle_mode_rejected)}"
        )

    rust_cases = {row[0]: row[1] for row in probe.get("case", [])}
    oracle_cases = {row[0]: row[1] for row in precedence_rows}
    rows += len(oracle_cases)
    if rust_cases != oracle_cases:
        for name in sorted(set(rust_cases) | set(oracle_cases)):
            rust = rust_cases.get(name, "<missing>")
            oracle = oracle_cases.get(name, "<missing>")
            if rust != oracle:
                problems.append(f"precedence case {name}: rust={rust} oracle={oracle}")

    if problems:
        for problem in problems:
            print(f"FAIL {problem}")
        return 1
    print(f"PASS {rows} rows agree with the independent oracle")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
