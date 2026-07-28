#!/usr/bin/env python3
"""cyril-nd4h probe: which UiConfig fields have a real production consumer?

Mechanism: the COMPILER, via TWO independent signals. One is not enough, and
the reason is worth stating because the first version of this probe got it
wrong and reported a false pass.

  SIGNAL A -- "named at a consumption site".
    Rename one field at a time (inside config.rs only, so that file stays
    self-consistent) and run `cargo check`. Any error outside config.rs means
    something out there mentions the field. Semantic, so immune to the
    comments/cfg/aliasing that fool grep.

  SIGNAL B -- "the bound value is actually USED".
    Signal A alone cannot see this. When a destructuring pattern names a field
    that no longer exists, rustc RECOVERS by creating the binding anyway, so
    every downstream use still resolves and emits no diagnostic. The result:
    deleting `ui_state.set_mouse_captured(mouse_capture)` -- which makes the
    field genuinely dead -- still produced two errors (E0026 "does not have a
    field named", E0027 "pattern does not mention field"), both of them from
    the PATTERN, and the probe happily printed AUDIT PASS.
    So: on a clean build, collect rustc's `unused_variables` warnings. A field
    that is bound but never used shows up there by name.

  A field is honored iff  (named outside config.rs)  AND  (not bound-unused).

`--all-targets` is deliberately NOT passed. The claim is about PRODUCTION
consumers, and `--all-targets` breaks it twice over: a test that merely reads
the field would count, and cyril-core's own test target fails to compile first,
so cargo never reaches the downstream `cyril` crate where the real consumers
live. `--all-features` IS kept -- a consumer behind `#[cfg(feature = "kas")]`
or `"voice"` is still a production consumer (cyril-ykkc).

Restore is a byte-exact reverse-edit from an in-memory copy of the file's
BYTES (never git checkout, which would nuke uncommitted work in the same file;
and never text mode, which would silently rewrite a CRLF checkout to LF and
then compare equal).

Exit status is 0 only when every field is honored.
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

    Derived rather than hardcoded: a fixed list silently goes stale the moment
    a field is added or removed -- it would report a DELETED field as "ignored"
    forever, and would never probe a NEW one, which is exactly the blind spot
    this whole ticket is about.
    """
    body = re.search(r"pub struct UiConfig \{(.*?)\n\}", src, re.S)
    if not body:
        raise SystemExit("could not locate `pub struct UiConfig`")
    return re.findall(r"^\s*pub (\w+):", body.group(1), re.M)


def cargo_check_messages():
    """Run cargo check over production targets; yield compiler messages."""
    p = subprocess.run(
        ["cargo", "check", "--all-features", "--message-format=json"],
        cwd=ROOT, capture_output=True, text=True,
    )
    for line in p.stdout.splitlines():
        try:
            rec = json.loads(line)
        except ValueError:
            continue
        if rec.get("reason") == "compiler-message":
            yield rec["message"]


def errors_outside_config():
    """SIGNAL A: errors whose primary span is outside config.rs."""
    out = []
    for m in cargo_check_messages():
        if m.get("level") != "error":
            continue
        for s in m.get("spans", []):
            if s.get("is_primary") and "types/config.rs" not in s["file_name"]:
                out.append((s["file_name"], s["line_start"], m["message"]))
    return out


def bound_but_unused():
    """SIGNAL B: field names rustc reports as bound-but-never-used."""
    unused = set()
    for m in cargo_check_messages():
        code = (m.get("code") or {}).get("code")
        if code == "unused_variables":
            found = re.search(r"unused variable: `(\w+)`", m.get("message", ""))
            if found:
                unused.add(found.group(1))
    return unused


def main():
    orig = CONFIG.read_bytes()
    fields = ui_config_fields(orig.decode())
    print(f"UiConfig fields discovered: {fields}\n")

    # Signal B first, against the tree exactly as committed.
    unused = bound_but_unused()
    if unused:
        print(f"bound-but-unused bindings on a clean build: {sorted(unused)}\n")

    named = {}
    try:
        for f in fields:
            patched = re.sub(rf"\b{f}\b", f + "_PROBEX", orig.decode())
            CONFIG.write_bytes(patched.encode())
            named[f] = errors_outside_config()
    finally:
        CONFIG.write_bytes(orig)
        if CONFIG.read_bytes() != orig:
            raise SystemExit("RESTORE FAILED -- config.rs left modified")
        print("restored config.rs (byte-exact)\n")

    ignored = []
    for f in fields:
        sites = named.get(f) or []
        is_unused = f in unused
        verdict = "HONORED" if sites and not is_unused else "IGNORED"
        if verdict == "IGNORED":
            ignored.append(f)
        why = "no consumer names it" if not sites else (
            "bound but never used" if is_unused else "named and used")
        print(f"{f:28} {verdict:8} ({why})")
        for fn, ln, msg in sites[:3]:
            print(f"      {fn}:{ln}  {msg.splitlines()[0][:78]}")

    Path(ROOT / ".cyril-nd4h/probe-output.json").write_text(
        json.dumps({"fields": fields, "ignored": ignored,
                    "bound_but_unused": sorted(unused)}, indent=2))

    print(f"\nPROBE VERDICT honored={[f for f in fields if f not in ignored]}")
    print(f"PROBE VERDICT ignored={ignored}")
    if ignored:
        print("AUDIT FAIL: fields with no production consumer")
        return 1
    print("AUDIT PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
