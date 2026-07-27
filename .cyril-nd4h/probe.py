#!/usr/bin/env python3
"""cyril-nd4h probe: which UiConfig fields have a real production consumer?

Mechanism: the COMPILER. Rename one field at a time (inside config.rs only,
so the file stays internally consistent), then `cargo check --all-features
--all-targets`. Any error whose primary span is outside config.rs is a real
consumer -- semantic proof, immune to comments/cfg/aliasing that fool grep.

Restore is a byte-exact reverse-edit from an in-memory copy (never git
checkout: that would nuke uncommitted work in the same file).
"""
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CONFIG = ROOT / "crates/cyril-core/src/types/config.rs"
FIELDS = [
    "max_messages",
    "highlight_cache_size",
    "stream_buffer_timeout_ms",
    "mouse_capture",
]


def check():
    """Run cargo check; return list of (file, line, msg) for errors."""
    p = subprocess.run(
        ["cargo", "check", "--all-features", "--all-targets",
         "--message-format=json"],
        cwd=ROOT, capture_output=True, text=True,
    )
    out = []
    for line in p.stdout.splitlines():
        try:
            rec = json.loads(line)
        except ValueError:
            continue
        if rec.get("reason") != "compiler-message":
            continue
        m = rec["message"]
        if m.get("level") != "error":
            continue
        for s in m.get("spans", []):
            if s.get("is_primary"):
                out.append((s["file_name"], s["line_start"], m["message"]))
    return out


def main():
    orig = CONFIG.read_text()
    results = {}
    try:
        for f in FIELDS:
            CONFIG.write_text(re.sub(rf"\b{f}\b", f + "_PROBEX", orig))
            errs = [e for e in check() if "types/config.rs" not in e[0]]
            results[f] = errs
            print(f"{f:28} consumers={len(errs)}")
            for fn, ln, msg in errs:
                print(f"      {fn}:{ln}  {msg.splitlines()[0][:80]}")
    finally:
        CONFIG.write_text(orig)  # byte-exact restore
        assert CONFIG.read_text() == orig, "RESTORE FAILED"
        print("\nrestored config.rs (byte-exact)")

    Path(ROOT / ".cyril-nd4h/probe-output.json").write_text(
        json.dumps({k: v for k, v in results.items()}, indent=2))
    honored = [f for f, e in results.items() if e]
    print(f"\nPROBE VERDICT honored={honored}")
    print(f"PROBE VERDICT ignored={[f for f in FIELDS if f not in honored]}")


if __name__ == "__main__":
    sys.exit(main())
