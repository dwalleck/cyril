"""Run the real F3 theme source gate before/after on identical disposable inputs.

Parent-owned execution only; implementation workers do not run this harness.
Run: python .cyril-c6la/gate-review-f3.py --target-dir /nonempty/owned/target
The actual pre-F3 gate comes from reviewed fiv1 plus frozen x5xi/zj0m patches.
Only that test function is swapped; both sides retain the same F1 source splitter.
Printed observations are not assertions or an aggregate success declaration.
"""

import argparse
import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile


THEME = Path("crates/cyril-ui/src/theme.rs")
WIDGETS = Path("crates/cyril-ui/src/widgets")
GATE = "theme::tests::widgets_only_use_the_explicit_theme"
BASE = "7eefe275"
PATCHES = (
    (".cyril-x5xi/production.patch", "9ac057ccb15ee7e809330f41bb3abf39259018ea61a8066950635eb02bd93585"),
    (".cyril-zj0m/production.patch", "5c0d35de8fd980228f9f595199f86014b70e6f7a376ae51c40d7ceb915a655f9"),
)


def gate_span(source):
    marker = "    #[test]\n    fn widgets_only_use_the_explicit_theme() {"
    if source.count(marker) != 1:
        raise SystemExit("F3 gate anchor changed; review frozen checker inputs")
    start = source.index(marker)
    end = source.find("\n    #[test]\n", start + len(marker))
    if end < 0:
        raise SystemExit("F3 gate end anchor changed; review frozen checker inputs")
    return start, end


def pre_f3_checker(root, after):
    environment = os.environ.copy()
    with tempfile.TemporaryDirectory(prefix="cyril-c6la-f3-index-") as temporary:
        environment["GIT_INDEX_FILE"] = str(Path(temporary) / "index")

        def git(*arguments):
            result = subprocess.run(
                ["git", *arguments], cwd=root, env=environment,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
            )
            if result.returncode:
                raise SystemExit(f"Cannot freeze pre-F3 checker: {result.stderr}")
            return result.stdout

        git("read-tree", BASE)
        for relative, expected_hash in PATCHES:
            patch = root / relative
            actual_hash = hashlib.sha256(patch.read_bytes()).hexdigest()
            if actual_hash != expected_hash:
                raise SystemExit(f"Frozen predecessor changed: {relative} {actual_hash}")
            git("apply", "--cached", "--3way", str(patch))
        baseline_theme = git("show", f":{THEME.as_posix()}")
    before_start, before_end = gate_span(baseline_theme)
    after_start, after_end = gate_span(after)
    return after[:after_start] + baseline_theme[before_start:before_end] + after[after_end:]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", required=True, type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    target = args.target_dir.resolve()
    after = (root / THEME).read_text()
    before = pre_f3_checker(root, after)
    environment = os.environ.copy()
    if not environment.get("CARGO_TARGET_DIR"):
        environment.pop("CARGO_TARGET_DIR", None)

    def ignore(directory, names):
        return [
            name for name in names
            if name in {".git", "target", "node_modules"}
            or (Path(directory) / name).resolve() == target
        ]

    cases = (
        ("unchanged", None, None),
        ("forbidden-nested", WIDGETS / "chat/presentation.rs", "forbidden"),
        ("unlisted-nested", WIDGETS / "chat/f3_inventory_probe.rs", "unlisted"),
        ("forbidden-existing", WIDGETS / "chat.rs", "forbidden"),
    )
    print(f"SOURCE {root}\nTARGET {target}\nPREDECESSOR {BASE}", flush=True)
    print("Both checker sides retain the same F1 production splitter and source tree.", flush=True)
    with tempfile.TemporaryDirectory(prefix="cyril-c6la-f3-") as temporary:
        pristine = Path(temporary) / "pristine"
        shutil.copytree(root, pristine, symlinks=True, ignore=ignore)
        (pristine / THEME).write_text(after)
        for case, relative, mutation in cases:
            for label, checker in (("BEFORE", before), ("AFTER", after)):
                fixture = Path(temporary) / case / label.lower()
                shutil.copytree(pristine, fixture, symlinks=True)
                (fixture / THEME).write_text(checker)
                if relative is not None:
                    path = fixture / relative
                    if mutation == "forbidden":
                        path.write_text("// c6la F3 forbidden sentinel: ThemeId\n" + path.read_text())
                    else:
                        path.write_text("// c6la F3 unlisted nested source\n")
                    print(f"MUTATION {case} {relative} sha256={hashlib.sha256(path.read_bytes()).hexdigest()}", flush=True)
                # Preserve .cyril-* include_str! captures in the fixture and
                # invalidate package inputs when switching a shared target tree.
                for entrypoint in (
                    "crates/cyril-core/src/lib.rs",
                    "crates/cyril-ui/src/lib.rs",
                ):
                    (fixture / entrypoint).touch()
                command = [
                    "cargo", "test", "--manifest-path", str(fixture / "Cargo.toml"),
                    "--target-dir", str(target), "-p", "cyril-ui", "--lib", GATE,
                    "--", "--exact", "--nocapture",
                ]
                result = subprocess.run(
                    command, cwd=fixture, env=environment,
                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                )
                transcript = result.stdout
                executed = re.search(r"test result: (?:ok|FAILED)\. [01] passed; [01] failed;", transcript)
                one_test = "running 1 test\n" in transcript
                verdict = (("PASS" if result.returncode == 0 else "FAIL")
                           if executed and one_test else "NO TEST VERDICT")
                print(f"{case} {label}: {verdict}; exit={result.returncode}; checker_sha256={hashlib.sha256(checker.encode()).hexdigest()}", flush=True)
                print(transcript, end="" if transcript.endswith("\n") else "\n", flush=True)
    print("Expected: unchanged both PASS; forbidden-nested/unlisted-nested BEFORE PASS AFTER FAIL; forbidden-existing both FAIL.")
    print("Compiler errors or missing one-test receipts are not gate evidence.")


if __name__ == "__main__":
    main()
