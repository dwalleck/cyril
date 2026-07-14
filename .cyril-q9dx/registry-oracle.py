#!/usr/bin/env python3
"""Compare the ThemeId declaration macro with the compiled ALL registry."""
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
THEME_SOURCE = ROOT / "crates/cyril-ui/src/theme.rs"

source = THEME_SOURCE.read_text(encoding="utf-8")
declaration = re.search(
    r"bundled_theme_ids!\s*\{.*?pub enum ThemeId\s*\{([^}]*)\}",
    source,
    re.S,
)
if declaration is None:
    sys.stderr.write("C0 FAIL ThemeId macro invocation not found\n")
    raise SystemExit(1)
variants = re.findall(r"^\s*([A-Z][A-Za-z0-9_]*)\s*,?\s*$", declaration.group(1), re.M)
if not variants or len(variants) != len(set(variants)):
    sys.stderr.write(f"C0 FAIL invalid declared variants: {variants!r}\n")
    raise SystemExit(1)

result = subprocess.run(
    [
        "cargo",
        "test",
        "-p",
        "cyril-ui",
        "theme::tests::emit_theme_registry_probe",
        "--",
        "--exact",
        "--nocapture",
    ],
    cwd=ROOT,
    capture_output=True,
    text=True,
    check=False,
)
if result.returncode != 0:
    sys.stderr.write(f"C0 FAIL compiled registry probe did not run:\n{result.stderr}")
    raise SystemExit(1)
block = re.search(r"BEGIN_THEME_REGISTRY\s*(.*?)\s*END_THEME_REGISTRY", result.stdout, re.S)
if block is None:
    sys.stderr.write("C0 FAIL compiled registry markers missing\n")
    raise SystemExit(1)
rows = [line.split("\t", 1) for line in block.group(1).splitlines()[1:]]
try:
    indices = [int(row[0]) for row in rows]
    compiled = [row[1] for row in rows]
except (IndexError, TypeError, ValueError) as error:
    sys.stderr.write(f"C0 FAIL invalid compiled rows: {rows!r}: {error}\n")
    raise SystemExit(1) from error
if compiled != variants or indices != list(range(len(variants))):
    sys.stderr.write(f"C0 FAIL declared={variants!r} compiled={rows!r}\n")
    raise SystemExit(1)
sys.stdout.write(
    f"C0 PASS themes={len(compiled)} unique={len(set(compiled))} visits={len(rows)}\n"
)
