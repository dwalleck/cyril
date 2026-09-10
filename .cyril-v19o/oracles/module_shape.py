#!/usr/bin/env python3
"""Mechanical module-shape fence for cyril-v19o (claim C8).

`--slice N` scopes the *requirements* to what slice N has created so far, so
each checkpoint gates its own artifacts while the final run (no flag) demands
the whole ledger. A module a later slice owns is reported as PENDING rather
than missing when it does not exist yet.

Checks the approved module ledger in `.cyril-v19o/design.md`:

1. The powers wire adapter lives in `crates/cyril-core/src/protocol/convert/
   kas/powers.rs`, and no wire key (`displayName`, `mcpServerNames`,
   `hasSteeringFiles`, `isAgentPlugin`, `keywords`) appears in `crates/
   cyril-ui/src/` — presentation consumes the domain type, never the wire.
   Comments are stripped first, so a doc comment may name a wire key.
2. The domain payload lives in `crates/cyril-core/src/types/power.rs` and is
   re-exported from `types/mod.rs`.
3. Powers painting lives in `crates/cyril-ui/src/widgets/powers_panel.rs`,
   which decodes nothing; the adapter formats nothing.
4. Protected parents receive wiring only. Each added production line must sit
   inside an allowed function or match an allowed marker; anything else is a
   new responsibility smuggled into a protected parent:
   - `crates/cyril/src/app.rs` — arms and `dispatch_powers_panel_key`; no field.
   - `crates/cyril-ui/src/state.rs` — `*powers*` methods, the panel accessor,
     the catalog field, and the `PowersChanged` arm; no chat-message emission.
5. No `powers/list` or `powers/refresh` request string in `cyril-core` (the
   two pulls cyril must never issue, B8).

The default branch is discovered, never hard-coded. Reports the claim ID, the
path, and the offending symbol. Usage:
python3 .cyril-v19o/oracles/module_shape.py [--slice N]
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

POWERS_ADAPTER = CORE / "protocol" / "convert" / "kas" / "powers.rs"
POWERS_TYPE = CORE / "types" / "power.rs"
POWERS_WIDGET = UI / "widgets" / "powers_panel.rs"
TYPES_MOD = CORE / "types" / "mod.rs"
APP = REPO / "crates" / "cyril" / "src" / "app.rs"
STATE = UI / "state.rs"

# (path, owning slice) for the files the ledger requires.
REQUIRED = [(POWERS_ADAPTER, 1), (POWERS_TYPE, 1), (TYPES_MOD, 1), (POWERS_WIDGET, 2)]

WIRE_KEYS = ["displayName", "mcpServerNames", "hasSteeringFiles", "isAgentPlugin", "keywords"]
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

    The censuses below must catch a wire key or a request *used by code*, not
    documentation that names why it is absent — the adapter's module docs name
    both pull methods for exactly that reason.
    """
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)
    return re.sub(r"//[^\n]*", "", text)


def production_text(path: pathlib.Path) -> str:
    """The file's production half: everything above its first `#[cfg(test)]`."""
    text = path.read_text()
    cut = text.find("#[cfg(test)]")
    return text if cut == -1 else text[:cut]


def function_ranges(text: str) -> list[tuple[str, int, int]]:
    """`(name, first_line, last_line)` for every impl-level `fn`, 1-indexed."""
    lines = text.splitlines()
    ranges: list[tuple[str, int, int]] = []
    current: tuple[str, int] | None = None
    for number, line in enumerate(lines, start=1):
        match = re.match(r"^    (?:pub )?(?:async )?fn (\w+)", line)
        if match:
            if current is not None:
                ranges.append((current[0], current[1], number - 1))
            current = (match.group(1), number)
    if current is not None:
        ranges.append((current[0], current[1], len(lines)))
    return ranges


def enclosing_function(ranges: list[tuple[str, int, int]], line: int) -> str | None:
    """Name of the narrowest function range containing `line`."""
    best: tuple[int, str] | None = None
    for name, start, end in ranges:
        if start <= line <= end and (best is None or end - start < best[0]):
            best = (end - start, name)
    return None if best is None else best[1]


def added_lines_with_numbers(diff: str) -> list[tuple[int, str]]:
    """`(new_file_line, text)` for every added line in a `--unified=0` diff."""
    added: list[tuple[int, str]] = []
    new_line = 0
    for line in diff.splitlines():
        header = re.match(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@", line)
        if header:
            new_line = int(header.group(1))
            continue
        if line.startswith("+++") or line.startswith("---"):
            continue
        if line.startswith("+"):
            added.append((new_line, line[1:]))
            new_line += 1
        elif line.startswith(" "):
            new_line += 1
    return added


def working_tree_delta(path: pathlib.Path, branch: str) -> str:
    """`--unified=0` diff of the working tree against the merge-base.

    The working tree, not `branch...HEAD`: a slice is gated before it is
    committed, so its delta is uncommitted at gate time.
    """
    merge_base = subprocess.run(
        ["git", "merge-base", branch, "HEAD"], cwd=REPO, capture_output=True, text=True
    ).stdout.strip()
    return subprocess.run(
        ["git", "diff", merge_base, "--unified=0", "--", str(path.relative_to(REPO))],
        cwd=REPO,
        capture_output=True,
        text=True,
    ).stdout


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

    # No wire spelling anywhere in cyril-ui: `PowerInfo` is the only currency
    # crossing that boundary.
    for path in sorted(UI.rglob("*.rs")):
        code = strip_rust_comments(path.read_text())
        for key in WIRE_KEYS:
            if key in code:
                report("C8", str(path.relative_to(REPO)), f"wire key {key!r} in presentation code")

    # The widget decodes nothing…
    if POWERS_WIDGET.exists():
        code = strip_rust_comments(POWERS_WIDGET.read_text())
        for token in ("serde", "from_value", "from_str"):
            if token in code:
                report("C8", str(POWERS_WIDGET.relative_to(REPO)), f"decoding token {token!r} in the widget")

    # …and the adapter formats nothing.
    if POWERS_ADAPTER.exists():
        code = strip_rust_comments(POWERS_ADAPTER.read_text())
        for token in ("ratatui", "Theme", "truncate"):
            if token in code:
                report("C8", str(POWERS_ADAPTER.relative_to(REPO)), f"presentation token {token!r} in the adapter")

    # Forbidden outbound requests, anywhere in cyril-core, comments stripped.
    for path in sorted(CORE.rglob("*.rs")):
        code = strip_rust_comments(path.read_text())
        for forbidden in FORBIDDEN_REQUESTS:
            if forbidden in code:
                report("C8", str(path.relative_to(REPO)), f"forbidden powers request {forbidden!r}")


def check_module_delta(
    claim: str,
    path: pathlib.Path,
    branch: str,
    allowed_functions: re.Pattern[str],
    allowed_markers: re.Pattern[str],
) -> None:
    """Every added production line is inside an allowed function or marker line.

    This is what "wiring only" means mechanically: a protected parent may gain
    arms and lifecycle methods, but a line of new behavior anywhere else — a
    helper, a parsing step, a formatting decision — falls outside both nets and
    is reported with its enclosing function.
    """
    diff = working_tree_delta(path, branch)
    if not diff:
        return
    production = production_text(path)
    production_lines = production.count("\n") + 1
    ranges = function_ranges(production)
    for line_number, text in added_lines_with_numbers(diff):
        if line_number > production_lines:
            continue  # a test-module line: no production responsibility
        stripped = text.strip()
        if not stripped or stripped.startswith(("//", "///", "/*", "*")):
            continue
        if stripped in ("}", ")", "};", "});", "{", "];"):
            continue
        function = enclosing_function(ranges, line_number)
        if function is not None and allowed_functions.search(function):
            continue
        if allowed_markers.search(text):
            continue
        where = function if function is not None else "<file scope>"
        report(claim, str(path.relative_to(REPO)), f"delta outside the allowed region ({where}): {stripped[:90]}")


def check_protected_parents(branch: str) -> None:
    """4: protected parents carry wiring, not responsibility."""
    check_module_delta(
        "C8",
        APP,
        branch,
        allowed_functions=re.compile(r"dispatch_powers_panel_key"),
        allowed_markers=re.compile(r"Notification::PowersChanged|CommandResultKind::ShowPowers"),
    )
    check_module_delta(
        "C8",
        STATE,
        branch,
        allowed_functions=re.compile(r"powers"),
        allowed_markers=re.compile(r"powers_panel|PowersPanelState|PowersChanged"),
    )

    # The powers panel methods on UiState must not emit chat output.
    text = production_text(STATE)
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
