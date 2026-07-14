#!/usr/bin/env python3
"""Cheapest falsifier for design claim C1."""
from pathlib import Path
from io import StringIO
import csv
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = Path(__file__).with_name("Cargo.toml")
EXPECTED = {
    "user": "LightBlue",
    "agent": "LightGreen",
    "system": "LightMagenta",
}

result = subprocess.run(
    [
        "cargo",
        "run",
        "--quiet",
        "--manifest-path",
        str(MANIFEST),
        "--bin",
        "q9dx-probe",
    ],
    cwd=ROOT,
    capture_output=True,
    text=True,
    check=False,
)
if result.returncode != 0:
    sys.stderr.write(f"C1 FAIL probe did not run:\n{result.stderr}")
    raise SystemExit(1)
rows = list(csv.DictReader(StringIO(result.stdout), delimiter="\t"))
actual = {
    row["role"]: row["after"]
    for row in rows
    if row["mode"] == "ansi16" and row["role"] in EXPECTED
}
if actual != EXPECTED:
    sys.stderr.write(f"C1 FAIL expected={EXPECTED!r} actual={actual!r}\n")
    raise SystemExit(1)
sys.stdout.write(
    "C1 PASS user=LightBlue agent=LightGreen system=LightMagenta\n"
)
