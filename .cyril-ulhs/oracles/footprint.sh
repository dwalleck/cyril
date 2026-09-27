#!/usr/bin/env bash
# cyril-ulhs C6 oracle: the diff against the default branch's merge-base is exactly the approved
# footprint. Prints one `C6 PASS|FAIL <check>` line per check; exit 1 on any FAIL.
set -u
ROOT="$(git rev-parse --show-toplevel)" || exit 2
cd "$ROOT" || exit 2
BASE="${ULHS_BASE:-$(git merge-base HEAD origin/main)}"
echo "# base $BASE  head $(git rev-parse --short HEAD)"
python3 - "$BASE" <<'PY'
import re, subprocess, sys
base = sys.argv[1]
def diff(*paths, extra=()):
    return subprocess.run(["git", "diff", base, "--", *paths, *extra], capture_output=True, text=True, check=True).stdout
fails = 0
def report(ok, what):
    global fails
    print(f"C6 {'PASS' if ok else 'FAIL'} {what}")
    if not ok: fails += 1

FORCED = [
    "crates/cyril-core/src/commands/mod.rs", "crates/cyril-core/src/voice.rs",
    "crates/cyril-core/src/protocol/source_observer.rs", "crates/cyril-core/src/types/memory.rs",
    "crates/cyril-memory/src/client.rs", "crates/cyril-memory/src/lesson.rs",
    "crates/cyril-memory/src/paths.rs", "crates/cyril-memory/src/permissions.rs",
    "crates/cyril-memory/src/project.rs", "crates/cyril-memory/src/runtime.rs",
    "crates/cyril-memory/src/source_turn.rs", "crates/cyril-memory/src/wire.rs",
    "crates/cyril-voice/src/lib.rs", "crates/cyril/src/capture_forwarder.rs",
    "crates/cyril/src/memory_runtime.rs",
]
assert len(FORCED) == 15  # 17 attribute sites: commands/mod.rs and memory_runtime.rs hold two each
PICKER = "crates/cyril-ui/tests/picker_active_marker.rs"
ALLOWED = set(FORCED) | {PICKER, "clippy.toml"}

# 1. file set
names = subprocess.run(["git", "diff", "--name-only", base], capture_output=True, text=True, check=True).stdout.split()
outside = [n for n in names if n not in ALLOWED and not n.startswith(".cyril-ulhs/")]
report(not outside, f"changed files within approved set (outside: {outside or 'none'})")
report("Cargo.toml" not in names, "root Cargo.toml untouched")

# 2. forced files: no additions; deletions are attribute-form lines only
ATTR = re.compile(r'^\s*(#!?\[expect\(clippy::expect_used\)\]|#\[expect\(|clippy::expect_used,|reason = ".*"|\)\])\s*$')
for f in FORCED:
    d = diff(f)
    added = [l[1:] for l in d.splitlines() if l.startswith("+") and not l.startswith("+++")]
    removed = [l[1:] for l in d.splitlines() if l.startswith("-") and not l.startswith("---")]
    bad = [l for l in removed if l.strip() and not ATTR.match(l)]
    report(d != "" and not added and not bad, f"{f}: attribute-only deletions (+{len(added)} -{len(removed)}; non-attr: {bad or 'none'})")

# 3. commands/mod.rs hunks disjoint from the sibling PR's 170-250 region
hunks = [int(m.group(1)) for m in re.finditer(r"^@@ -(\d+)", diff("crates/cyril-core/src/commands/mod.rs"), re.M)]
report(hunks and all(505 <= h <= 525 or 1650 <= h <= 1670 for h in hunks), f"commands/mod.rs hunks at {hunks} within 505-525 / 1650-1670")

# 4. clippy.toml content
try:
    body = [l for l in open("clippy.toml", encoding="utf-8").read().splitlines() if l.strip() and not l.lstrip().startswith("#")]
except FileNotFoundError:
    body = None
report(body == ["allow-expect-in-tests = true"], f"clippy.toml has exactly one key (got {body})")

# 5. picker: helper gone, three inline expects, no attributes
p = open(PICKER, encoding="utf-8").read()
report("render_open_picker" not in p, "picker: render_open_picker helper deleted")
report(p.count('ui.picker().expect("show_picker did not open a picker")') == 3, "picker: three inline .expect() sites")
report("#[allow" not in p and "#[expect" not in p and "#![allow" not in p and "#![expect" not in p, "picker: no lint attributes")

# 6. whole diff: no added allow/expect attributes anywhere in code
whole = diff(*sorted(ALLOWED))
added_attr = [l for l in whole.splitlines() if l.startswith("+") and re.search(r"#!?\[(allow|expect)\(", l)]
report(not added_attr, f"no added #[allow]/#[expect] in code (found: {added_attr or 'none'})")

print(f"# {fails} FAIL(s)")
sys.exit(1 if fails else 0)
PY
