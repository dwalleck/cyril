#!/usr/bin/env python3
"""Empirical probe of the existing Python oracle; never imports native feature code."""
import argparse
import contextlib
import datetime
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
from types import SimpleNamespace


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("oracle", type=Path)
    args = parser.parse_args()
    oracle_path = args.oracle.resolve()
    spec = importlib.util.spec_from_file_location("crtool_oracle", oracle_path)
    oracle = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(oracle)
    receipts = []
    newline = b"\r\n" if os.name == "nt" else b"\n"
    original_cwd = Path.cwd()
    with tempfile.TemporaryDirectory(prefix="cyril-s2hb-premises-") as temporary:
        root = Path(temporary)
        os.chdir(root)
        try:
            def git(*argv):
                return subprocess.run(["git", *argv], check=True, capture_output=True).stdout
            git("init", "-b", "main")
            git("config", "user.name", "Cyril fixture")
            git("config", "user.email", "fixture@example.invalid")
            git("config", "core.autocrlf", "false")
            (root / "src").mkdir()
            (root / "src/lib.rs").write_text("pub fn existing() {}\n", encoding="utf-8", newline="\n")
            (root / "NOTES.md").write_text("before\n", encoding="utf-8", newline="\n")
            git("add", ".")
            git("commit", "-m", "fixture")
            (root / "src/lib.rs").write_text("pub fn existing() { new_helper(); }\npub fn new_helper() {}\n", encoding="utf-8", newline="\n")
            (root / "NOTES.md").write_text("after\n", encoding="utf-8", newline="\n")
            run = root / "run"
            fixed = datetime.datetime(2026, 9, 30, 0, 0, 0, tzinfo=datetime.timezone.utc)
            class FixedDateTime(datetime.datetime):
                @classmethod
                def now(cls, tz=None):
                    return fixed if tz is not None else fixed.replace(tzinfo=None)
            oracle.datetime = SimpleNamespace(datetime=FixedDateTime, timezone=datetime.timezone)
            gather_args = SimpleNamespace(rundir=str(run), target="HEAD", scope="src")
            stream = io.StringIO()
            with contextlib.redirect_stdout(stream):
                oracle.cmd_gather(gather_args)
            manifest = json.loads((run / "manifest.json").read_text(encoding="utf-8"))
            symbols = json.loads((run / "facts/symbols.json").read_text(encoding="utf-8"))
            helper = next(s for s in symbols if s["name"] == "new_helper")
            assert helper["status"] == "added" and helper["line"] == 2
            assert helper["usages"] == [{"file": "src/lib.rs", "line": 1, "text": "pub fn existing() { new_helper(); }"}]
            assert manifest["change_docs"] == [{"path": "NOTES.md", "bytes": 6}]
            assert manifest["gathered_at"] == "2026-09-30T00:00:00+00:00"
            assert "crtool_version" not in manifest
            expected_json = (json.dumps(manifest, indent=2, ensure_ascii=False) + "\n").encode().replace(b"\n", newline)
            assert (run / "manifest.json").read_bytes() == expected_json
            assert (run / "changed-files.txt").read_bytes() == b"src/lib.rs" + newline
            assert (run / "diff.patch").read_bytes() == git("diff", "--no-renames", "HEAD", "--", "src")
            receipts.append({"premise": "python-gather", "result": "PASS", "clock_input": fixed.isoformat(), "text_newline_hex": newline.hex(), "source_has_version_stamp": False, "oracle": "hand-authored helper/caller/doc expectations and direct git diff bytes"})
            before = (run / "manifest.json").read_bytes()
            with contextlib.redirect_stdout(io.StringIO()) as repeated:
                oracle.cmd_gather(gather_args)
            assert (run / "manifest.json").read_bytes() == before
            assert repeated.getvalue() == "already gathered: 1 files, target=HEAD\n"
            receipts.append({"premise": "python-gather-idempotence", "result": "PASS", "oracle": "unchanged manifest bytes and exact independently specified stdout"})

            child = root / "child.py"
            child.write_text("import os,sys,time\nmode=sys.argv[1]\nif mode == 'argv': print(repr(sys.argv[2:]))\nelif mode == 'bytes': os.write(1,b'src/lib.rs:2: bad\\xff\\n'); os.write(2,b'err\\n'); sys.exit(7)\nelif mode == 'noisy': os.write(1,b'x'*131072); os.write(2,b'y'*131072)\nelif mode == 'timeout': os.write(1,b'partial\\n'); time.sleep(10)\n", encoding="utf-8", newline="\n")
            def command(*argv):
                if os.name == "nt":
                    return subprocess.list2cmdline(argv)
                import shlex
                return shlex.join(argv)
            def diagnostics(mode, timeout=5, suffix=()):
                times = iter([100.0, 102.0])
                oracle.time = SimpleNamespace(time=lambda: next(times))
                with contextlib.redirect_stdout(io.StringIO()) as output:
                    oracle.cmd_diagnostics(SimpleNamespace(rundir=str(run), command=command(sys.executable, str(child), mode, *suffix), timeout=timeout))
                return (run / "facts/diagnostics-raw.txt").read_bytes(), output.getvalue()
            raw, output = diagnostics("argv", suffix=("two words", 'quote"inside', "back\\slash"))
            expected_args = repr(["two words", 'quote"inside', "back\\slash"]) + "\n"
            child_bytes = expected_args.encode().replace(b"\n", newline)
            expected_raw = (child_bytes + b"\n").decode(errors="replace").encode().replace(b"\n", newline)
            assert raw == expected_raw, (raw, expected_raw)
            receipts.append({"premise": "python-command-grammar", "result": "PASS", "oracle": "manually specified argv including spaces, quote and backslash", "raw_newline_hex": raw[-8:].hex()})
            raw, output = diagnostics("bytes")
            expected = (b"src/lib.rs:2: bad\xff\n\nerr\n").decode(errors="replace").encode().replace(b"\n", newline)
            assert raw == expected
            assert output == "diagnostics: FAILED (exit 7) in 2s; 1 line(s) on changed files -> facts/diagnostics.txt\n"
            receipts.append({"premise": "python-diagnostics-bytes", "result": "PASS", "oracle": "literal child bytes, UTF-8 replacement, stdout then separator then stderr, and fixed independent duration"})
            raw, output = diagnostics("noisy")
            assert raw == b"x" * 131072 + newline + b"y" * 131072
            receipts.append({"premise": "python-dual-pipe-drain", "result": "PASS", "stdout_bytes": 131072, "stderr_bytes": 131072, "oracle": "known byte counts exceeding both pipe buffers"})
            raw, output = diagnostics("timeout", timeout=0.2)
            assert raw == b"partial" + newline + newline
            assert "TIMED OUT in 2s" in output
            receipts.append({"premise": "python-timeout-output", "result": "PASS", "oracle": "literal pre-sleep output, externally enforced subprocess deadline, fixed independent reported duration"})
            oracle.time = SimpleNamespace(time=lambda: 100.0)
            with contextlib.redirect_stderr(io.StringIO()) as errors:
                try:
                    oracle.cmd_diagnostics(SimpleNamespace(rundir=str(run), command="cyril-s2hb-nonexistent-command-91904", timeout=1))
                except SystemExit as exit_error:
                    assert exit_error.code == 2
                else:
                    raise AssertionError("unstartable command was accepted")
            assert errors.getvalue().startswith("crtool: error: cannot run ")
            receipts.append({"premise": "python-cannot-start", "result": "PASS", "oracle": "guaranteed-absent executable and exit/error capture"})
        finally:
            os.chdir(original_cwd)
    print(json.dumps({"platform": sys.platform, "python": sys.version.split()[0], "receipts": receipts}, indent=2))


if __name__ == "__main__":
    main()
