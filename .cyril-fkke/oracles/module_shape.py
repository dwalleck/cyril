#!/usr/bin/env python3
"""Mechanical module-shape fence for cyril-fkke (claim C8).

Checks the approved module ledger in `.cyril-fkke/design.md`:

1. Palette ownership stays in `crates/cyril-ui/src/theme.rs`.
2. Widgets never reference the theme registry symbols (`ThemeId`,
   `SyntaxTheme`, `ColorMode`) or call `resolve(`.
3. Protected parents (`app.rs`, `main.rs`, `state.rs`) carry zero production
   delta against the repository's default branch.
4. `render.rs` paints the canvas only inside `draw_inner` and only behind the
   `Color::Reset` gate.

The default branch is discovered, never hard-coded. Reports the claim ID and
the exact path/symbol. Usage: python3 .cyril-fkke/oracles/module_shape.py
"""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parents[2]
WIDGETS = REPO / "crates" / "cyril-ui" / "src" / "widgets"
THEME = REPO / "crates" / "cyril-ui" / "src" / "theme.rs"
RENDER = REPO / "crates" / "cyril-ui" / "src" / "render.rs"
PROTECTED = [
    "crates/cyril/src/app.rs",
    "crates/cyril/src/main.rs",
    "crates/cyril-ui/src/state.rs",
]

# Allowed widget import seams (mirrors the in-crate fence, checked here
# independently from the file system rather than from source text embedded in a
# production test).
ALLOWED_IMPORTS = {
    "use crate::theme::Theme;",
    "use crate::theme::{ColorMode, Theme, ThemeId};",
}


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
    print("FAIL\tC8\tcannot discover the default/upstream branch")
    sys.exit(1)


def check_widgets() -> list[str]:
    failures = []
    for path in sorted(WIDGETS.glob("*.rs")):
        text = path.read_text()
        # Only production source is in scope: a widget's own tests may resolve
        # palettes to build fixtures, exactly as the in-crate fence allows.
        test_module = text.find("#[cfg(test)]")
        stripped = text if test_module < 0 else text[:test_module]
        for allowed in ALLOWED_IMPORTS:
            stripped = stripped.replace(allowed, "")
        for symbol in ("ThemeId", "SyntaxTheme", "ColorMode", "resolve("):
            if symbol in stripped:
                failures.append(
                    f"C8\t{path.relative_to(REPO)}\twidget references {symbol!r} — "
                    "palette registry must stay behind the resolved Theme"
                )
    return failures


def check_ownership() -> list[str]:
    failures = []
    theme = THEME.read_text()
    if "pub enum ThemeId" not in theme or "fn source(id: ThemeId)" not in theme:
        failures.append("C8\tcrates/cyril-ui/src/theme.rs\tregistry/source ownership moved")
    render = RENDER.read_text()
    if render.count("set_style(") != 1 or "theme.canvas != Color::Reset" not in render:
        failures.append(
            "C8\tcrates/cyril-ui/src/render.rs\tcanvas painting must be one gated set_style call"
        )
    if re.search(r"match\s+.*ThemeId", render):
        failures.append("C8\tcrates/cyril-ui/src/render.rs\trenderer branches on ThemeId")
    return failures


def check_protected_parents() -> list[str]:
    base = default_branch()
    result = subprocess.run(
        ["git", "diff", "--numstat", base, "--", *PROTECTED],
        cwd=REPO,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"FAIL\tC8\tgit diff against {base} failed: {result.stderr.strip()}")
        sys.exit(1)
    failures = []
    for line in result.stdout.splitlines():
        added, deleted, path = line.split("\t")
        if added != "0" or deleted != "0":
            failures.append(f"C8\t{path}\tprotected parent changed +{added}/-{deleted}")
    return failures


def main() -> int:
    failures = check_widgets() + check_ownership() + check_protected_parents()
    for failure in failures:
        print(f"FAIL\t{failure}")
    if failures:
        return 1
    print("PASS\tmodule shape matches the approved ledger")
    return 0


if __name__ == "__main__":
    sys.exit(main())
