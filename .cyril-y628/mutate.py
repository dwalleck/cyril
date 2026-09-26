#!/usr/bin/env python3
"""Exact, one-at-a-time mutation proofs with byte-preserving restoration."""
import argparse
import hashlib
import json
import os
import pathlib
import re
import subprocess
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent


def digest(data):
    return hashlib.sha256(data).hexdigest()


def run(command):
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = "/home/dwalleck/repos/cyril/target"
    started = time.monotonic()
    process = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True, timeout=1200)
    return process.returncode, process.stdout + process.stderr, round(time.monotonic() - started, 3)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--only", nargs="*")
    args = parser.parse_args()
    manifest = json.loads((ROOT / ".cyril-y628/mutations.json").read_text())
    receipt_path = ROOT / ".cyril-y628/mutation-results.json"
    receipts = json.loads(receipt_path.read_text()) if receipt_path.exists() else {}
    for mutation in manifest:
        name = mutation["name"]
        if args.only and name not in args.only:
            continue
        path = ROOT / mutation["file"]
        original = path.read_bytes()
        old, new = mutation["old"].encode(), mutation["new"].encode()
        assert original.count(old) == 1, f"{name}: expected one source anchor, got {original.count(old)}"
        mutated = original.replace(old, new, 1)
        assert mutated != original, f"{name}: no observable source mutation"
        with tempfile.TemporaryDirectory(prefix=f"cyril-y628-{name}-") as directory:
            backup = pathlib.Path(directory) / path.name
            backup.write_bytes(original)
            path.write_bytes(mutated)
            try:
                red, red_output, red_seconds = run(mutation["command"])
            finally:
                current = path.read_bytes()
                if current != mutated:
                    # Do not overwrite a third party's edit with our snapshot.
                    persistent = pathlib.Path(tempfile.mkdtemp(prefix="cyril-y628-conflict-")) / path.name
                    persistent.write_bytes(original)
                    raise RuntimeError(f"{name}: concurrent source mutation; original retained at {persistent}")
                path.write_bytes(backup.read_bytes())
            assert digest(path.read_bytes()) == digest(original), f"{name}: restore mismatch"
        green, green_output, green_seconds = run(mutation["command"])
        valid_red = red != 0 and re.search(mutation["failure"], red_output) is not None
        if "could not compile" in red_output or re.search(r"error\[E\d+\]", red_output):
            valid_red = False
        record = {
            "claim": mutation["claim"], "file": mutation["file"], "source_sha256": digest(original),
            "command": mutation["command"], "red_exit": red, "green_exit": green,
            "red_seconds": red_seconds, "green_seconds": green_seconds,
            "result": "PASS" if valid_red and green == 0 else "FAIL",
            "red_evidence": red_output, "restored_green_evidence": green_output,
        }
        receipts[name] = record
        receipt_path.write_text(json.dumps(receipts, indent=2))
        print(f"{name}: {record['result']} (red={red}, restored={green})", flush=True)
        if record["result"] != "PASS":
            print(red_output)
            print(green_output)
            raise SystemExit(1)


if __name__ == "__main__":
    main()
