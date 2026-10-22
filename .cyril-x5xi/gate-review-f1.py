"""Run the real F1 gate, before/after, over disposable identical source inputs.

Parent-owned execution only; this script was not run by the implementation worker.
Run: python .cyril-x5xi/gate-review-f1.py --target-dir /nonempty/owned/target
It prints observations, not assertions or an aggregate success declaration.
"""

import argparse
import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", required=True, type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    target = args.target_dir.resolve()
    theme_path = Path("crates/cyril-ui/src/theme.rs")
    markdown_path = Path("crates/cyril-ui/src/widgets/markdown.rs")
    after = (root / theme_path).read_text()
    markdown = (root / markdown_path).read_text()
    seam = '                .replace("use crate::theme::{MarkdownStyle, Theme};", "")\n'
    consumer = (
        "pub fn render_with_theme(markdown: &str, width: usize, "
        "theme: &Theme) -> Vec<Line<'static>> {\n"
    )
    if after.count(seam) != 1 or markdown.count(consumer) != 1:
        raise SystemExit("F1 source anchors changed; review the frozen inputs before running")
    before = after.replace(seam, "", 1)
    forbidden = markdown.replace(
        consumer, consumer + "    let _ = crate::theme::ThemeId::CyrilDark;\n", 1
    )
    environment = os.environ.copy()
    if not environment.get("CARGO_TARGET_DIR"):
        environment.pop("CARGO_TARGET_DIR", None)

    def ignore(directory, names):
        return [
            name
            for name in names
            if name in {".git", "target", "node_modules"}
            or name == ".cyril-x5xi"
            or (Path(directory) / name).resolve() == target
        ]

    print(f"SOURCE {root}")
    print(f"TARGET {target}")
    print("Only the exact whitelist line differs between the two frozen checkers.")
    with tempfile.TemporaryDirectory(prefix="cyril-x5xi-f1-") as temporary:
        for case, widget_source in (("approved-import", markdown), ("forbidden-access", forbidden)):
            fixture = Path(temporary) / case
            shutil.copytree(root, fixture, symlinks=True, ignore=ignore)
            (fixture / markdown_path).write_text(widget_source)
            print(f"CASE {case} widget_sha256={hashlib.sha256(widget_source.encode()).hexdigest()}")
            for label, checker in (("BEFORE", before), ("AFTER", after)):
                # Each checker is freshly copied from immutable captured input.
                (fixture / theme_path).write_text(checker)
                # Bind shared-target artifacts to this fixture, not a newer sibling worktree.
                for entrypoint in ("cyril-core", "cyril-ui"):
                    (fixture / "crates" / entrypoint / "src/lib.rs").touch()
                command = [
                    "cargo", "test", "--manifest-path", str(fixture / "Cargo.toml"),
                    "--target-dir", str(target), "-p", "cyril-ui", "--lib",
                    "theme::tests::widgets_only_use_the_explicit_theme",
                    "--", "--exact", "--nocapture",
                ]
                result = subprocess.run(command, cwd=fixture, env=environment,
                                        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
                transcript = result.stdout
                executed = re.search(r"test result: (?:ok|FAILED)\. [01] passed; [01] failed;", transcript)
                one_test = "running 1 test\n" in transcript
                verdict = ("PASS" if result.returncode == 0 else "FAIL") if executed and one_test else "NO TEST VERDICT"
                print(f"{case} {label}: {verdict}; exit={result.returncode}; "
                      f"checker_sha256={hashlib.sha256(checker.encode()).hexdigest()}")
                # Retain complete output, including the actual failing assertion.
                print(transcript, end="" if transcript.endswith("\n") else "\n")
    print("Expected: approved-import BEFORE FAIL / AFTER PASS; forbidden-access both FAIL.")
    print("Compiler errors or missing one-test receipts are not gate evidence.")


if __name__ == "__main__":
    main()
