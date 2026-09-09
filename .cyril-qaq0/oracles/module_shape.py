#!/usr/bin/env python3
"""Mechanical module-shape fence for cyril-qaq0 (claims C10 and the ledger).

Checks the approved module ledger in `.cyril-qaq0/design.md`:

1. Palette vocabulary (`ThemeId`, `ColorMode`, bundled id literals) stays
   outside `cyril-core` and `cyril-memory` production code. `UiConfig` carries
   `Option<String>`, not an enum, precisely so this holds.
2. Widgets render from the resolved `Theme` only -- never the registry.
3. Protected parents (`cyril-ui/src/widgets/`, `cyril-memory/`,
   `cyril-core/src/protocol/`) carry zero production delta against the default
   branch.

`--selftest` proves the scanners fire: it plants a violating file in a temp tree
and requires a reported violation, then plants a clean file and requires none.

Usage: python3 .cyril-qaq0/oracles/module_shape.py [--selftest]
"""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys
import tempfile

REPO = pathlib.Path(__file__).resolve().parents[2]
PALETTE_FREE_ROOTS = [
    REPO / "crates" / "cyril-core" / "src",
    REPO / "crates" / "cyril-memory" / "src",
]
WIDGETS = REPO / "crates" / "cyril-ui" / "src" / "widgets"
PROTECTED = [
    "crates/cyril-ui/src/widgets",
    "crates/cyril-memory",
    "crates/cyril-core/src/protocol",
]
# Mirrors the in-crate widget fence; checked here from the filesystem rather
# than from source text embedded in a production test.
ALLOWED_WIDGET_IMPORTS = {
    "use crate::theme::Theme;",
    "use crate::theme::{ColorMode, Theme, ThemeId};",
}
PALETTE_SYMBOLS = ("ThemeId", "ColorMode")
BUNDLED_ID_LITERALS = (
    '"cyril-dark"',
    '"cyril-light"',
    '"high-contrast-dark"',
    '"high-contrast-light"',
    '"catppuccin-mocha"',
    '"gruvbox-dark"',
)


def production_source(text: str) -> str:
    """Production code only: no test module, no comments.

    Tests may name bundled ids to build fixtures and doc comments may use one as
    an example; neither creates a second owner of the catalog. Only code can.
    """
    index = text.find("#[cfg(test)]")
    source = text if index < 0 else text[:index]
    source = re.sub(r"/\*.*?\*/", "", source, flags=re.DOTALL)
    return "\n".join(line.split("//", 1)[0] for line in source.splitlines())


def scan_palette_free(root: pathlib.Path) -> list[str]:
    """Report palette vocabulary found in `root`'s production source."""
    failures: list[str] = []
    if not root.is_dir():
        return failures
    for path in sorted(root.rglob("*.rs")):
        source = production_source(path.read_text())
        for symbol in PALETTE_SYMBOLS:
            if symbol in source:
                failures.append(
                    f"C10\t{path}\t{symbol} outside cyril-ui -- the config surface "
                    "carries Option<String>, not palette types"
                )
        for literal in BUNDLED_ID_LITERALS:
            if literal in source:
                failures.append(
                    f"C10\t{path}\tbundled id literal {literal} outside cyril-ui -- "
                    "the catalog has one owner"
                )
    return failures


def check_widgets() -> list[str]:
    failures: list[str] = []
    for path in sorted(WIDGETS.glob("*.rs")):
        source = production_source(path.read_text())
        for allowed in ALLOWED_WIDGET_IMPORTS:
            source = source.replace(allowed, "")
        for symbol in (*PALETTE_SYMBOLS, "SyntaxTheme", "resolve("):
            if symbol in source:
                failures.append(
                    f"C10\t{path}\twidget references {symbol!r} -- rendering reads "
                    "the resolved Theme only"
                )
    return failures


def default_branch() -> str:
    for command in (
        ["git", "symbolic-ref", "refs/remotes/origin/HEAD"],
        ["git", "rev-parse", "--abbrev-ref", "origin/HEAD"],
    ):
        result = subprocess.run(command, cwd=REPO, capture_output=True, text=True)
        if result.returncode == 0 and result.stdout.strip():
            return result.stdout.strip().replace("refs/remotes/", "")
    result = subprocess.run(
        ["git", "rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"],
        cwd=REPO,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0 and result.stdout.strip():
        return result.stdout.strip()
    print("FAIL\tledger\tcannot discover the default/upstream branch")
    sys.exit(1)


def check_protected_parents() -> list[str]:
    base = default_branch()
    result = subprocess.run(
        ["git", "diff", "--numstat", base, "--", *PROTECTED],
        cwd=REPO,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"FAIL\tledger\tgit diff against {base} failed: {result.stderr.strip()}")
        sys.exit(1)
    failures = []
    for line in result.stdout.splitlines():
        added, deleted, path = line.split("\t")
        if added != "0" or deleted != "0":
            failures.append(f"ledger\t{path}\tprotected parent changed +{added}/-{deleted}")
    return failures


def selftest() -> int:
    with tempfile.TemporaryDirectory() as raw:
        root = pathlib.Path(raw)
        (root / "planted.rs").write_text(
            "pub fn planted() -> ThemeId { ThemeId::CyrilDark }\n"
        )
        if not scan_palette_free(root):
            print("FAIL\tselftest\ta planted ThemeId reference was not reported")
            return 1
        (root / "planted.rs").write_text('pub fn clean() -> &\'static str { "ok" }\n')
        if scan_palette_free(root):
            print("FAIL\tselftest\ta clean file was reported as a violation")
            return 1
    print("PASS\tselftest\tplanted violation reported, clean file accepted")
    return 0


def main() -> int:
    if "--selftest" in sys.argv:
        return selftest()
    failures = (
        scan_palette_free(PALETTE_FREE_ROOTS[0])
        + scan_palette_free(PALETTE_FREE_ROOTS[1])
        + check_widgets()
        + check_protected_parents()
    )
    for failure in failures:
        print(f"FAIL\t{failure}")
    if failures:
        return 1
    print("PASS\tmodule shape matches the approved ledger")
    return 0


if __name__ == "__main__":
    sys.exit(main())
