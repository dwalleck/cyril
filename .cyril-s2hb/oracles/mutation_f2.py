#!/usr/bin/env python3
"""Rerunnable A/B harness for verified PR146 finding F2.

The harness freezes the reviewed-head checker and CI step, compares them with
repaired current files, and runs each comparison in a fresh temporary Git
fixture. Its scope is only CI dispatch and the source guard; it does not build
Rust or claim Rust behavioral proof. No captured run is embedded here.
"""
from contextlib import contextmanager
from pathlib import Path
import io
import os
import subprocess
import sys
import tarfile
import tempfile

REVIEWED_HEAD = "1dae375f2b0e3131de8dee2689f2e3fd414e43c4"
CHECKER_REL = ".cyril-s2hb/oracles/check_shape.py"
CI_REL = ".github/workflows/ci.yml"
LIB_REL = "crates/cyril-core/src/lib.rs"
BRIDGE_REL = "crates/cyril-core/src/protocol/bridge.rs"
HOST_SHELL_REL = "crates/cyril-core/src/protocol/kas/host_shell.rs"
BASE_REF = "refs/remotes/origin/fixture"

# This file is integrated at .cyril-s2hb/oracles/mutation_f2.py. Do not derive
# ROOT from the temporary handoff directory used while preparing this file.
SCRIPT = Path(__file__).resolve()
ROOT = SCRIPT.parents[2]
CURRENT_CHECKER = ROOT / CHECKER_REL
CURRENT_CI = ROOT / CI_REL


class Case:
    def __init__(self, name, path, needle, replacement, description):
        self.name = name
        self.path = path
        self.needle = needle
        self.replacement = replacement
        self.description = description


CASES = (
    Case(
        "bridge-getter-none",
        BRIDGE_REL,
        """    pub fn review_shell(&self) -> Option<crate::review::ShellDialect> {
        self.review_shell
    }""",
        """    pub fn review_shell(&self) -> Option<crate::review::ShellDialect> {
        None
    }""",
        "bridge getter returns None",
    ),
    Case(
        "resolved-assignment-none",
        BRIDGE_REL,
        """    handle.review_shell = {
        #[cfg(feature = "kas")]
        {
            host_shell.as_ref().map(|shell| shell.review_shell())
        }
        #[cfg(not(feature = "kas"))]
        {
            None
        }
    };""",
        """    handle.review_shell = None;""",
        "resolved-shell assignment is replaced with None",
    ),
    Case(
        "host-shell-mapping-swapped",
        HOST_SHELL_REL,
        """            ShellKind::Posix => ShellDialect::Posix,
            ShellKind::Fish => ShellDialect::Fish,""",
        """            ShellKind::Posix => ShellDialect::Fish,
            ShellKind::Fish => ShellDialect::Posix,""",
        "HostShell Posix/Fish mapping is swapped",
    ),
    Case(
        "unrelated-bridge-constant",
        BRIDGE_REL,
        "const COMMAND_CAPACITY: usize = 32;",
        "const COMMAND_CAPACITY: usize = 33;",
        "unrelated bridge constant/body change remains allowed in wiring-only mode",
    ),
    Case(
        "forbidden-main-owner",
        "crates/cyril/src/main.rs",
        "mod app;\n",
        "mod app;\n\nfn review_gather_body() {}\n",
        "original C10 named mutation must still fail both full checkers; main-only CI dispatch is unchanged",
    ),
)


def run_process(argv, cwd, *, check=False, text=False, env=None):
    result = subprocess.run(
        argv,
        cwd=cwd,
        check=False,
        capture_output=True,
        text=text,
        env=env,
    )
    if check and result.returncode:
        stderr = result.stderr if text else result.stderr.decode(errors="replace")
        raise RuntimeError(f"command failed ({result.returncode}): {' '.join(argv)}\n{stderr}")
    return result


def git_show(path):
    result = run_process(
        ["git", "show", f"{REVIEWED_HEAD}:{path}"],
        ROOT,
        check=True,
    )
    return result.stdout.decode("utf-8")


def git(cwd, *args, check=True):
    result = run_process(["git", *args], cwd, check=check, text=True)
    return result.stdout.strip()


def extract_review_step(ci):
    """Extract the workflow's existing shell body without retyping dispatch."""
    step = "      - name: Review prefix ownership fence\n"
    start = ci.index(step)
    run = ci.index("        run: |\n", start) + len("        run: |\n")
    body = []
    for line in ci[run:].splitlines(keepends=True):
        if line.startswith("      - name: "):
            break
        if line.startswith("          "):
            body.append(line[10:])
        elif line.strip():
            raise ValueError("review-prefix shell body has unexpected YAML indentation")
        else:
            body.append(line)
    if not body:
        raise ValueError("review-prefix shell body is empty")
    return "".join(body)


def assert_established_prefix_wiring(archive_files):
    """Reject a fixture base that predates the reviewed integration surface."""
    required = {
        LIB_REL: "pub mod review;",
        BRIDGE_REL: "review_shell: Option<crate::review::ShellDialect>,",
        HOST_SHELL_REL: "pub(crate) fn review_shell(&self) -> crate::review::ShellDialect",
    }
    for path, marker in required.items():
        if marker not in archive_files[path]:
            raise RuntimeError(
                f"{REVIEWED_HEAD} does not contain established prefix wiring in {path}"
            )
    if "handle.review_shell = {" not in archive_files[BRIDGE_REL]:
        raise RuntimeError(
            f"{REVIEWED_HEAD} does not contain the established bridge assignment"
        )


def archive_reviewed_tree():
    result = run_process(
        ["git", "archive", "--format=tar", REVIEWED_HEAD],
        ROOT,
    )
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors="replace"))
    return result.stdout


@contextmanager
def fixture_tree(archive, checker, ci, case):
    """Create one isolated base+mutation tree for exactly one case/variant."""
    with tempfile.TemporaryDirectory(prefix=f"cyril-f2-{case.name}-") as directory:
        tree = Path(directory)
        with tarfile.open(fileobj=io.BytesIO(archive), mode="r:") as tar:
            tar.extractall(tree)

        run_process(["git", "init", "-q"], tree, check=True)
        git(tree, "config", "user.name", "F2 mutation harness")
        git(tree, "config", "user.email", "f2-harness@example.invalid")
        (tree / CHECKER_REL).parent.mkdir(parents=True, exist_ok=True)
        (tree / CI_REL).parent.mkdir(parents=True, exist_ok=True)
        (tree / CHECKER_REL).write_text(checker, encoding="utf-8")
        (tree / CI_REL).write_text(ci, encoding="utf-8")
        git(tree, "add", "-A")
        git(tree, "commit", "-qm", f"fixture base ({case.name})")
        base = git(tree, "rev-parse", "HEAD")
        git(tree, "update-ref", BASE_REF, base)

        target = tree / case.path
        original = target.read_text(encoding="utf-8")
        if original.count(case.needle) != 1:
            raise RuntimeError(
                f"{case.name}: expected one mutation needle in {case.path}, "
                f"found {original.count(case.needle)}"
            )
        mutated = original.replace(case.needle, case.replacement, 1)
        if mutated == original:
            raise RuntimeError(f"{case.name}: mutation did not change {case.path}")
        target.write_text(mutated, encoding="utf-8")
        git(tree, "add", case.path)
        git(tree, "commit", "-qm", f"mutation ({case.name})")
        changed = git(tree, "diff", "--name-only", f"{base}...HEAD").splitlines()
        if changed != [case.path]:
            raise RuntimeError(
                f"{case.name}: fixture has more than one production mutation: {changed}"
            )
        yield tree


def run_ci(tree, ci):
    environment = os.environ.copy()
    environment.update(
        {
            "REVIEW_BASE": "fixture",
            "GITHUB_BASE_REF": "fixture",
            "GITHUB_EVENT_NAME": "pull_request",
        }
    )
    body = extract_review_step(ci)
    return run_process(
        ["bash", "-e", "-u", "-o", "pipefail", "-c", body],
        tree,
        text=True,
        env=environment,
    )


def run_full_checker(tree):
    return run_process(
        [
            sys.executable,
            CHECKER_REL,
            "--phase",
            "prefix",
            "--base",
            BASE_REF,
        ],
        tree,
        text=True,
    )


def combined_output(result):
    return (result.stdout or "") + (result.stderr or "")


def verdict(result):
    output = combined_output(result)
    if result.returncode == 0 and "No review-prefix ownership changes" in output:
        return "SKIP"
    if result.returncode == 0 and "PASS C10:" in output:
        return "PASS"
    if result.returncode != 0 or "FAIL C10:" in output:
        return "FAIL"
    return "UNKNOWN"


def localized_diagnostics(result):
    lines = [line.strip() for line in combined_output(result).splitlines() if line.strip()]
    selected = [
        line
        for line in lines
        if (
            "No review-prefix ownership changes" in line
            or "PASS C10:" in line
            or "FAIL C10:" in line
            or "protected-parent body changed" in line
            or "review_shell" in line
            or "resolved-shell assignment" in line
            or "HostShell" in line
        )
    ]
    if not selected:
        selected = lines[-3:]
    return " | ".join(selected) if selected else "(no diagnostic output)"


def report(label, result):
    print(f"    {label}: exit={result.returncode} verdict={verdict(result)}")
    print(f"      diagnostics: {localized_diagnostics(result)}")



def run_case(case, archive, old_checker, old_ci, new_checker, new_ci):
    print(f"CASE {case.name}: {case.description}")
    with fixture_tree(archive, old_checker, old_ci, case) as tree:
        old_ci_result = run_ci(tree, old_ci)
        old_full_result = run_full_checker(tree)
        report("old CI step", old_ci_result)
        report("old full checker", old_full_result)
    with fixture_tree(archive, new_checker, new_ci, case) as tree:
        new_ci_result = run_ci(tree, new_ci)
        new_full_result = run_full_checker(tree)
        report("new CI step", new_ci_result)
        report("new full checker", new_full_result)


def main():
    old_checker = git_show(CHECKER_REL)
    old_ci = git_show(CI_REL)
    new_checker = CURRENT_CHECKER.read_text(encoding="utf-8")
    new_ci = CURRENT_CI.read_text(encoding="utf-8")
    archive_files = {
        path: git_show(path)
        for path in (LIB_REL, BRIDGE_REL, HOST_SHELL_REL)
    }
    assert_established_prefix_wiring(archive_files)
    archive = archive_reviewed_tree()
    print("A/B scope: CI dispatch and checker source guard only; no Rust behavioral proof")
    for case in CASES:
        run_case(case, archive, old_checker, old_ci, new_checker, new_ci)


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError) as error:
        print(f"F2 harness failed: {error}", file=sys.stderr)
        sys.exit(1)
