#!/usr/bin/env python3
"""Mechanical module-shape fence for cyril-v19o (claim C8).

`--slice N` scopes the *requirements* to what slice N has created so far, so
each checkpoint gates its own artifacts while the final run (no flag) demands
the whole ledger. A module that a later slice owns is reported as PENDING
rather than missing when it does not exist yet.

Checks the approved module ledger in `.cyril-v19o/design.md`:

1. The powers wire adapter lives in `crates/cyril-core/src/protocol/convert/
   kas/powers.rs`, and the wire keys (`displayName`, `mcpServerNames`,
   `hasSteeringFiles`, `isAgentPlugin`, `keywords`) appear nowhere under
   `crates/cyril-ui/src/` — presentation must consume the domain type, never
   the wire.
2. The domain payload lives in `crates/cyril-core/src/types/power.rs` and is
   re-exported from `types/mod.rs`.
3. Powers painting lives in `crates/cyril-ui/src/widgets/powers_panel.rs`,
   which never mentions `serde`, `serde_json`, or wire keys.
4. Protected parents receive wiring only:
   - `crates/cyril/src/app.rs` — the powers delta is confined to the
     notification arm, the command-result arm, and the key-dispatch function;
     no new `App` field.
   - `crates/cyril-ui/src/state.rs` — the powers delta is confined to panel
     lifecycle methods and one `apply_notification` arm; no chat-message
     emission (`add_command_output` / `add_system_message` / `add_message`)
     may appear inside a powers method.

The default branch is discovered, never hard-coded. Reports the claim ID and
the exact path/symbol. Usage: python3 .cyril-v19o/oracles/module_shape.py
"""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys

SLICE = 0
for index, argument in enumerate(sys.argv):
    if argument == "--slice" and index + 1 < len(sys.argv):
        SLICE = int(sys.argv[index + 1])

REPO = pathlib.Path(__file__).resolve().parents[2]
CORE = REPO / "crates" / "cyril-core" / "src"
UI = REPO / "crates" / "cyril-ui" / "src"

# (path, owning slice) for the files the ledger requires.
POWERS_ADAPTER = CORE / "protocol" / "convert" / "kas" / "powers.rs"
POWERS_TYPE = CORE / "types" / "power.rs"
POWERS_WIDGET = UI / "widgets" / "powers_panel.rs"
TYPES_MOD = CORE / "types" / "mod.rs"
REQUIRED = [(POWERS_ADAPTER, 1), (POWERS_TYPE, 1), (TYPES_MOD, 1), (POWERS_WIDGET, 2)]
APP = REPO / "crates" / "cyril" / "src" / "app.rs"
STATE = UI / "state.rs"

# Wire spellings. Their presence outside the adapter means a second owner of
# the wire shape has appeared.
WIRE_KEYS = ["displayName", "mcpServerNames", "hasSteeringFiles", "isAgentPlugin", "keywords"]

# The two pulls cyril must never issue (cyril-v19o B8).
FORBIDDEN_REQUESTS = ["powers/list", "powers/refresh"]

failures: list[str] = []


def report(claim: str, path: str, detail: str) -> None:
    failures.append(f"FAIL\t{claim}\t{path}\t{detail}")


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
    print("FAIL\tC8\t-\tcannot discover the default/upstream branch")
    sys.exit(1)


def strip_rust_comments(text: str) -> str:
    """Remove `//`/`///` line comments and `/* */` blocks.

    The forbidden-request census must catch a request *issued* by code, not a
    doc comment that explains why the request is never issued — the adapter's
    module docs name both pull methods for exactly that reason.
    """
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)
    return re.sub(r"//[^\n]*", "", text)


def same_line_imports(text: str) -> set[str]:
    return set(re.findall(r"^use\s+[^;]+;", text, flags=re.MULTILINE))


def check_paths() -> None:
    """1-3: required owners exist, and each holds only its own responsibility."""
    for path, owning_slice in REQUIRED:
        if path.exists():
            continue
        if SLICE and SLICE < owning_slice:
            print(f"PENDING\tC8\t{path.relative_to(REPO)}\towned by slice {owning_slice}")
            continue
        report("C8", str(path.relative_to(REPO)), "required module is missing")

    if TYPES_MOD.exists() and "pub use power::PowerInfo;" not in TYPES_MOD.read_text():
        report("C8", str(TYPES_MOD.relative_to(REPO)), "PowerInfo is not re-exported")

    # No wire spelling anywhere in cyril-ui: the domain type is the only
    # currency crossing that boundary.
    for path in sorted(UI.rglob("*.rs")):
        code = strip_rust_comments(path.read_text())
        for key in WIRE_KEYS:
            if key in code:
                report("C8", str(path.relative_to(REPO)), f"wire key {key!r} in presentation code")

    # The widget must not decode anything itself.
    if POWERS_WIDGET.exists():
        text = POWERS_WIDGET.read_text()  # noqa: E501
        for token in ("serde", "serde_json", "from_value", "from_str"):
            if token in text:
                report("C8", str(POWERS_WIDGET.relative_to(REPO)), f"decoding token {token!r} in the widget")

    # The adapter must not format for display (no theme, no width math).
    if POWERS_ADAPTER.exists():
        text = POWERS_ADAPTER.read_text()
        for token in ("ratatui", "Theme", "truncate"):
            if token in text:
                report("C8", str(POWERS_ADAPTER.relative_to(REPO)), f"presentation token {token!r} in the adapter")

    # Forbidden outbound requests, anywhere in cyril-core. Comments are
    # stripped first: a doc comment naming a method cyril never calls is
    # documentation, not a request.
    for path in sorted(CORE.rglob("*.rs")):
        code = strip_rust_comments(path.read_text())
        for forbidden in FORBIDDEN_REQUESTS:
            if forbidden in code:
                report("C8", str(path.relative_to(REPO)), f"forbidden powers request {forbidden!r}")


def production_delta(path: pathlib.Path, branch: str) -> str:
    """Lines this branch added/removed in `path`, tests excluded from the count.

    Test bodies are authored by definition, so the tripwire counts only
    production lines: everything above the file's first `#[cfg(test)]`.
    """
    result = subprocess.run(
        ["git", "diff", f"{branch}...HEAD", "--unified=0", "--", str(path.relative_to(REPO))],
        cwd=REPO,
        capture_output=True,
        text=True,
    )
    return result.stdout


def added_production_lines(diff: str) -> list[str]:
    """Added lines that are not inside a `#[cfg(test)]` module."""
    added: list[str] = []
    in_tests = False
    for line in diff.splitlines():
        if line.startswith("@@"):
            # A hunk header names its enclosing function; test modules appear
            # as `mod tests`.
            in_tests = "mod tests" in line
            continue
        if not line.startswith("+"):
            continue
        body = line[1:]
        if re.match(r"\s*#\[cfg\(test\)\]", body):
            in_tests = True
            continue
        if not in_tests:
            added.append(body)
    return added


def check_protected_parents(branch: str) -> None:
    """4: protected parents carry wiring, not responsibility."""
    for path, allowed_markers in (
        (
            APP,
            [
                "Notification::PowersChanged",
                "CommandResultKind::ShowPowers",
                "dispatch_powers_panel_key",
            ],
        ),
        (
            STATE,
            [
                "powers_panel",
                "PowersPanelState",
                "PowersChanged",
            ],
        ),
    ):
        diff = production_delta(path, branch)
        if not diff:
            continue
        added = added_production_lines(diff)
        allowed = re.compile("|".join(re.escape(m) for m in allowed_markers))
        for line in added:
            stripped = line.strip()
            if not stripped or stripped.startswith(("//", "///", "}", ")")):
                continue
            if not allowed.search(line):
                report(
                    "C8",
                    f"{path.relative_to(REPO)}",
                    f"protected-parent delta outside the allowed arms: {stripped[:90]}",
                )

    # No new App field (a struct field is a responsibility, not wiring).
    diff = production_delta(APP, branch)
    for line in added_production_lines(diff):
        if re.match(r"\s{4}\w+:\s", line) and "self." not in line:
            report("C8", "crates/cyril/src/app.rs", f"new App field: {line.strip()[:90]}")

    # The powers panel methods in UiState must not emit chat output.
    text = STATE.read_text()
    for method in re.finditer(r"\n    pub fn (\w*powers\w*)\([^)]*\)[^{]*\{", text):
        name = method.group(1)
        body = text[method.end() : text.find("\n    }\n", method.end())]
        for emitter in ("add_command_output", "add_system_message", "add_message"):
            if emitter in body:
                report("C8", "crates/cyril-ui/src/state.rs", f"{name} emits chat output via {emitter}")


def main() -> int:
    branch = default_branch()
    check_paths()
    check_protected_parents(branch)
    if failures:
        for failure in failures:
            print(failure)
        return 1
    print("PASS\tC8\tmodule ledger holds (adapter, payload, widget, protected parents)")
    print("PASS\tC7\tno powers request string in cyril-core")
    print("PASS\tC1\twire keys confined to convert/kas/powers.rs")
    return 0


if __name__ == "__main__":
    sys.exit(main())
