"""Apply cyril-k3lz named mutations one at a time, expect red, restore, expect green.

Usage (repo root):  python .cyril-k3lz/mutate.py .cyril-k3lz/mutations/slice1.json [ID ...]

Each entry: {"id", "file", "from", "to", "cmd": [...], "expect": "<substring in red output>"}.
A mutation whose `from` text is absent is reported NOT-APPLIED (the fence cannot be
judged). Files are always restored byte-for-byte, even on interruption.
"""
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def run(cmd):
    p = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace")
    return p.returncode, p.stdout + p.stderr


def main():
    spec = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    only = set(sys.argv[2:])
    failures = 0
    for m in spec:
        if only and m["id"] not in only:
            continue
        path = ROOT / m["file"]
        original = path.read_bytes()
        text = original.decode("utf-8")
        frm, to = m["from"], m["to"]
        if text.count(frm) == 0 and "\r\n" in text:
            # autocrlf checkouts: match the CRLF form of a multi-line pattern.
            frm, to = frm.replace("\n", "\r\n"), to.replace("\n", "\r\n")
        if text.count(frm) != 1:
            print(f"{m['id']}: NOT-APPLIED ({text.count(frm)} matches for `from`)")
            failures += 1
            continue
        try:
            path.write_bytes(text.replace(frm, to).encode("utf-8"))
            code, out = run(m["cmd"])
        finally:
            path.write_bytes(original)
        red = code != 0 and m.get("expect", "") in out
        lines = [l for l in out.splitlines() if "panicked" in l or "assert" in l or "error[" in l]
        print(f"{m['id']}: {'RED' if red else 'STILL-GREEN/UNEXPECTED'} (exit {code})")
        for l in lines[:4]:
            print(f"    {l.strip()[:220]}")
        if not red:
            failures += 1
    # Restored-green check for every command used. A filter that matches no
    # test exits 0 too — that is a silent skip, not green (receipt rule).
    for cmd in {tuple(m["cmd"]) for m in spec if not only or m["id"] in only}:
        code, out = run(list(cmd))
        ran = cmd[:2] != ("cargo", "test") or ("test result: ok." in out and " 0 passed" not in out)
        verdict = "GREEN" if code == 0 and ran else f"NOT-GREEN (exit {code}, ran={ran})"
        print(f"restored {' '.join(cmd)}: {verdict}")
        failures += verdict != "GREEN"
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
