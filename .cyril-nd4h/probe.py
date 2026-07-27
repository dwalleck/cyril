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


def ui_config_fields(src):
    """Field names of `struct UiConfig`, read from the source.

    Derived rather than hardcoded (cyril-nd4h slice 8): a fixed list silently
    goes stale the moment a field is added or removed -- it would report a
    DELETED field as "ignored" forever, and would never probe a NEW one, which
    is exactly the blind spot this whole ticket is about.
    """
    body = re.search(r"pub struct UiConfig \{(.*?)\n\}", src, re.S)
    if not body:
        raise SystemExit("could not locate `pub struct UiConfig`")
    return re.findall(r"^\s*pub (\w+):", body.group(1), re.M)


def check():
    """Run cargo check over PRODUCTION targets; return (file, line, msg) errors.

    Deliberately NO `--all-targets` (cyril-nd4h slice 8). The claim is "every
    field has a PRODUCTION consumer", and `--all-targets` cannot answer it for
    two compounding reasons: a test that merely reads the field would count as
    a consumer, and -- worse -- cyril-core's own test target fails to compile
    first, so cargo never checks the downstream `cyril` crate where the real
    consumers (main.rs, app.rs) live. The first run of this audit reported only
    test-file hits and looked like a pass.

    `--all-features` IS kept: a consumer behind `#[cfg(feature = "kas")]` or
    `"voice"` is still a production consumer (cf. cyril-ykkc).
    """
    p = subprocess.run(
        ["cargo", "check", "--all-features", "--message-format=json"],
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
    fields = ui_config_fields(orig)
    print(f"UiConfig fields discovered: {fields}\n")
    results = {}
    try:
        for f in fields:
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
    ignored = [f for f in fields if f not in honored]
    print(f"\nPROBE VERDICT honored={honored}")
    print(f"PROBE VERDICT ignored={ignored}")
    print("AUDIT PASS" if not ignored else "AUDIT FAIL: fields with no consumer")


if __name__ == "__main__":
    sys.exit(main())
