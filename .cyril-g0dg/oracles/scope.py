#!/usr/bin/env python3
"""Fail closed if cyril-g0dg escapes its approved test-only surface."""

from __future__ import annotations

import argparse
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
PARENTS = {
    "crates/cyril-core/src/protocol/transport.rs": (3, 0),
    "crates/cyril-core/src/protocol/source_observer.rs": (1, 0),
    "crates/cyril-core/src/protocol/bridge.rs": (10, 3),
    "crates/cyril/src/app.rs": (35, 7),
}
CHILD_PREFIXES = (
    "crates/cyril-core/src/protocol/transport/tests/current_runtime_contract",
    "crates/cyril-core/src/protocol/source_observer/tests/current_runtime_contract",
    "crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract",
    "crates/cyril/src/app/tests/current_runtime_contract",
)
REQUIRED_CHILDREN = {
    "crates/cyril-core/src/protocol/transport/tests/current_runtime_contract.rs",
    "crates/cyril-core/src/protocol/source_observer/tests/current_runtime_contract.rs",
    "crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/mod.rs",
    "crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/commands.rs",
    "crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/routing.rs",
    "crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/saturation.rs",
    "crates/cyril/src/app/tests/current_runtime_contract/mod.rs",
    "crates/cyril/src/app/tests/current_runtime_contract/ordering.rs",
    "crates/cyril/src/app/tests/current_runtime_contract/memory.rs",
    "crates/cyril/src/app/tests/current_runtime_contract/shutdown.rs",
}
FORBIDDEN_PATH_PARTS = ("Cargo.toml", "Cargo.lock", "rust-toolchain")
FORBIDDEN_RUST = re.compile(
    r"(?i)(agent[-_]client[-_]protocol[-_]sdk|sdk\s*2|ConductorImpl|trait\s+\w*Runtime)"
)
MAX_CHILD_LINES = 600


def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "base",
        nargs="?",
        default=None,
        help="git base revision (default: merge-base of HEAD and origin/main)",
    )
    parser.add_argument(
        "--claim",
        choices=("C1", "C11"),
        help="report only this claim's findings — the C12 mutation-localization "
        "doctrine runs each named mutation against only its own fence",
    )
    return parser.parse_args()


def base_revision(base: str | None) -> str:
    if base is not None:
        return base
    return git("merge-base", "HEAD", "origin/main").strip()


def changed_paths(base: str) -> set[str]:
    committed = set(git("diff", "--name-only", f"{base}...HEAD").splitlines())
    working = set(git("diff", "--name-only", base, "--").splitlines())
    untracked = set(git("ls-files", "--others", "--exclude-standard").splitlines())
    return committed | working | untracked


def allowed(path: str) -> bool:
    return (
        path == ".rivets/issues.jsonl"
        # Review-approved on this branch: the default-features CI lane that
        # keeps the cfg(not(kas)) fences running (commit c9d665d8).
        or path == ".github/workflows/ci.yml"
        or path.startswith(".cyril-g0dg/")
        or path in PARENTS
        or path.startswith(CHILD_PREFIXES)
    )


def numstat(base: str) -> dict[str, tuple[int, int]]:
    rows: dict[str, tuple[int, int]] = {}
    for raw in git("diff", "--numstat", base, "--").splitlines():
        added, deleted, path = raw.split("\t", 2)
        if added != "-" and deleted != "-":
            rows[path] = (int(added), int(deleted))
    return rows


def added_rust(base: str) -> str:
    lines = git("diff", "--unified=0", base, "--", "*.rs").splitlines()
    return "\n".join(
        line[1:]
        for line in lines
        if line.startswith("+") and not line.startswith("+++")
    )


def validate(base: str) -> list[str]:
    paths = changed_paths(base)
    errors: list[str] = []

    for path in sorted(paths):
        if not allowed(path):
            errors.append(f"C11 unexpected changed path: {path}")
        if any(part in path for part in FORBIDDEN_PATH_PARTS):
            errors.append(f"C11 forbidden manifest/toolchain change: {path}")
        if path.endswith(".rs") and path not in PARENTS and not path.startswith(CHILD_PREFIXES):
            errors.append(f"C1 Rust addition is not in an approved child test module: {path}")

    missing = sorted(path for path in REQUIRED_CHILDREN if not (ROOT / path).is_file())
    errors.extend(f"C1 required child test module missing: {path}" for path in missing)

    for path in sorted(REQUIRED_CHILDREN):
        file = ROOT / path
        if file.is_file():
            line_count = len(file.read_text(encoding="utf-8").splitlines())
            if line_count > MAX_CHILD_LINES:
                errors.append(
                    f"C1 child module exceeds {MAX_CHILD_LINES} lines: {path} ({line_count})"
                )

    stats = numstat(base)
    for path, expected in PARENTS.items():
        actual = stats.get(path, (0, 0))
        if actual != expected:
            errors.append(
                f"C1 oversized parent diff changed: {path} expected +{expected[0]}/-{expected[1]}, "
                f"got +{actual[0]}/-{actual[1]}"
            )

    forbidden = FORBIDDEN_RUST.search(added_rust(base))
    if forbidden is not None:
        errors.append(f"C11 forbidden production concept in Rust diff: {forbidden.group(0)!r}")

    return errors


def main() -> int:
    args = parse_args()
    errors = validate(base_revision(args.base))
    if args.claim is not None:
        errors = [error for error in errors if error.startswith(f"{args.claim} ")]
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(
        f"{args.claim or 'C1/C11'} scope PASS: test-only child modules, "
        "parent budgets exact, no manifests/SDK2/runtime traits/conductor/"
        "production observer"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
