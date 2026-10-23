"""Run the actual widget color-policy checker before/after on identical inputs.

Parent execution only; implementation workers do not execute this harness.
Run: python .cyril-c6la/gate-review-f9.py --target-dir /nonempty/owned/target
Frozen checker snapshots are read once; each case/side receives a fresh tree.
All .cyril-* captures are retained for existing include_str! dependencies.
Verdicts are printed observations, not harness assertions or aggregate PASS.
"""

import argparse
import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile


CHECKER = Path("crates/cyril-ui/tests/widget_theme_sources.rs")
WIDGETS = Path("crates/cyril-ui/src/widgets")
HIGHLIGHT = Path("crates/cyril-ui/src/highlight.rs")
ENTRYPOINTS = (
    Path("crates/cyril-core/src/lib.rs"),
    Path("crates/cyril-ui/src/lib.rs"),
)
TEST_MODULE = "\n#[cfg(test)]\nmod tests {"
SNAPSHOTS = (
    ("BEFORE", ".cyril-c6la/widget-theme-before-f9.rs"),
    ("AFTER", ".cyril-c6la/widget-theme-after-f9.rs"),
)


def mutate(fixture, kind):
    if kind == "unchanged":
        return None
    if kind == "nested-color":
        relative = WIDGETS / "chat/presentation.rs"
        path = fixture / relative
        path.write_text("// F9 nested color sentinel Color::White\n" + path.read_text())
    elif kind in {"post-counter-lf", "post-counter-crlf"}:
        relative = WIDGETS / "markdown.rs"
        path = fixture / relative
        source = path.read_text().replace("\r\n", "\n")
        boundary = source.find(TEST_MODULE)
        counter = source.find("#[cfg(test)]")
        if boundary < 0 or counter < 0 or counter >= boundary:
            raise SystemExit("F9 input changed: markdown needs a cfgtest counter before its final test module")
        source = source[:boundary] + "\n// F9 post-counter production sentinel Color::White\n" + source[boundary:]
        if kind == "post-counter-crlf":
            source = source.replace("\n", "\r\n")
        path.write_bytes(source.encode())
    elif kind == "unlisted-nested":
        relative = WIDGETS / "chat/f9_inventory_probe.rs"
        path = fixture / relative
        if path.exists():
            raise SystemExit("F9 inventory fixture path already exists; choose a fresh explicit probe")
        path.write_text("// F9 unlisted nested Rust source sentinel\n")
    elif kind == "signed-conversion":
        relative = HIGHLIGHT
        path = fixture / relative
        source = path.read_text()
        signature = source.find("fn syntect_to_ratatui(")
        body = source.find("{", signature) if signature >= 0 else -1
        first_counter = source.find("#[cfg(test)]")
        if body < 0 or (first_counter >= 0 and body >= first_counter):
            raise SystemExit("F9 input changed: existing signed syntect conversion must precede the first test guard")
        path.write_text(source[:body + 1] + "\n    // F9 allowed signed-conversion sentinel Color::Rgb\n" + source[body + 1:])
    elif kind == "existing-color":
        relative = WIDGETS / "chat.rs"
        path = fixture / relative
        path.write_text("// F9 existing color sentinel Color::White\n" + path.read_text())
    else:
        raise SystemExit(f"Unknown F9 mutation: {kind}")
    return relative


def observed_verdict(result):
    transcript = result.stdout
    running = re.search(r"^running (\d+) tests?\s*$", transcript, re.MULTILINE)
    summary = re.search(
        r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;",
        transcript,
    )
    named = all(
        f"test {name} ..." in transcript
        for name in ("all_widget_modules_are_clean", "modules_list_covers_the_widgets_directory")
    )
    executed = (
        running is not None and summary is not None and named
        and int(running.group(1)) >= 8
        and int(summary.group(1)) + int(summary.group(2)) == int(running.group(1))
        and all(int(summary.group(index)) == 0 for index in (3, 4, 5))
    )
    if not executed:
        return "NO TEST VERDICT"
    return "PASS" if result.returncode == 0 else "FAIL"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", required=True, type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    target = args.target_dir.resolve()
    snapshots = tuple((label, (root / relative).read_bytes()) for label, relative in SNAPSHOTS)
    if snapshots[1][1] != (root / CHECKER).read_bytes():
        raise SystemExit("Frozen AFTER checker differs from working source; refresh the reviewed snapshot/pins before execution")
    environment = os.environ.copy()
    environment.pop("CARGO_TARGET_DIR", None)

    def ignore(directory, names):
        # Do not exclude .cyril-* proof directories: existing include_str!
        # expressions need .cyril-a5wo captures even in this test target.
        return [
            name for name in names
            if name in {".git", "target", "node_modules", "__pycache__"}
            or (Path(directory) / name).resolve() == target
        ]

    cases = (
        ("unchanged", "PASS", "PASS"),
        ("nested-color", "PASS", "FAIL"),
        ("post-counter-lf", "PASS", "FAIL"),
        ("post-counter-crlf", "PASS", "FAIL"),
        ("unlisted-nested", "PASS", "FAIL"),
        ("signed-conversion", "PASS", "PASS"),
        ("existing-color", "FAIL", "FAIL"),
    )
    print(f"SOURCE {root}\nTARGET {target}", flush=True)
    for label, checker in snapshots:
        print(f"FROZEN {label} bytes={len(checker)} sha256={hashlib.sha256(checker).hexdigest()}", flush=True)
    print("Only the checker differs between sides; every mutation uses a fresh identical source copy.", flush=True)
    with tempfile.TemporaryDirectory(prefix="cyril-c6la-f9-") as temporary:
        pristine = Path(temporary) / "pristine"
        shutil.copytree(root, pristine, symlinks=True, ignore=ignore)
        for case, expected_before, expected_after in cases:
            print(f"EXPECTED {case} BEFORE={expected_before} AFTER={expected_after}", flush=True)
            for label, checker in snapshots:
                fixture = Path(temporary) / case / label.lower()
                shutil.copytree(pristine, fixture, symlinks=True)
                (fixture / CHECKER).write_bytes(checker)
                relative = mutate(fixture, case)
                if relative is not None:
                    source = (fixture / relative).read_bytes()
                    print(f"MUTATION {case} {label} {relative} bytes={len(source)} sha256={hashlib.sha256(source).hexdigest()}", flush=True)
                # Cargo may reuse the shared target after switching fixture roots.
                # Touch BOTH package entrypoints so embedded manifest/source paths
                # and include_str! dependencies cannot come from another tree.
                for entrypoint in ENTRYPOINTS:
                    (fixture / entrypoint).touch()
                command = [
                    "cargo", "test", "--manifest-path", str(fixture / "Cargo.toml"),
                    "--target-dir", str(target), "-p", "cyril-ui",
                    "--test", "widget_theme_sources", "--", "--nocapture", "--test-threads=1",
                ]
                print("COMMAND " + " ".join(command), flush=True)
                result = subprocess.run(
                    command, cwd=fixture, env=environment,
                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                )
                verdict = observed_verdict(result)
                print(f"OBSERVED {case} {label}: {verdict}; exit={result.returncode}; checker_sha256={hashlib.sha256(checker).hexdigest()}", flush=True)
                # Full output includes every injected-path/token failure line.
                print(result.stdout, end="" if result.stdout.endswith("\n") else "\n", flush=True)
    print("Compiler errors, zero/filtered/ignored tests or missing named-test receipts are NOT gate evidence.")
    print("No aggregate PASS is asserted: compare each printed OBSERVED pair with its EXPECTED pair.")


if __name__ == "__main__":
    main()
