#!/usr/bin/env bash
# cyril-ulhs probe: P1/P2/P3 against the REAL workspace. Throwaway instrument.
#
# Three gate runs, each `cargo clippy --workspace --all-targets --all-features -- -D warnings`
# (the CI command), captured as cargo JSON (+ the human rendering derived from it):
#   A  picker helper restored to `Option::expect`, NO clippy.toml            -> P1 red half (which sites fire?)
#   B  same + clippy.toml `allow-expect-in-tests = true`                     -> P1 green half, P3 (#[expect] fallout)
#   C  same as B + a deliberate non-test `.expect("probe")` in cyril-core   -> P2 (non-test discipline unchanged)
# Every edit is reverted on exit; the script refuses to start on a dirty tree for the files it touches.
#
# Run from anywhere inside the worktree:  bash .cyril-ulhs/probe.sh
set -u
ROOT="$(git rev-parse --show-toplevel)" || exit 2
cd "$ROOT" || exit 2
OUT="$ROOT/.cyril-ulhs/probe-out"
mkdir -p "$OUT"
PICKER=crates/cyril-ui/tests/picker_active_marker.rs
CORE_LIB=crates/cyril-core/src/lib.rs
DIAG="$ROOT/.cyril-ulhs/probe-diag.py"
GATE=(env -u CARGO_TARGET_DIR cargo clippy --workspace --all-targets --all-features --message-format=json -- -D warnings)

if ! git diff --quiet -- "$PICKER" "$CORE_LIB" || [ -e clippy.toml ] || [ -e .clippy.toml ]; then
    echo "PRECONDITION-FAIL: $PICKER / $CORE_LIB modified or a clippy config already exists" >&2
    exit 2
fi
restore() {
    git checkout -- "$PICKER" "$CORE_LIB"
    rm -f clippy.toml
    touch_members
}
# clippy only tracks clippy.toml in dep-info once it has seen it; force every member to re-lint.
touch_members() {
    touch crates/*/src/lib.rs crates/cyril/src/main.rs
}
trap restore EXIT

render() { # <json-log> <text-log>
    python3 - "$1" > "$2" <<'PY'
import json, sys
for raw in open(sys.argv[1], encoding="utf-8"):
    raw = raw.strip()
    if not raw.startswith("{"):
        print(raw); continue
    try:
        obj = json.loads(raw)
    except json.JSONDecodeError:
        print(raw); continue
    if obj.get("reason") == "compiler-message":
        sys.stdout.write((obj.get("message") or {}).get("rendered") or "")
PY
}

run_step() { # <letter> <label>
    local step="$1" label="$2"
    echo "== step $step: $label"
    "${GATE[@]}" > "$OUT/$step.json" 2> "$OUT/$step.stderr"
    local rc=$?
    echo "$rc" > "$OUT/$step.exit"
    render "$OUT/$step.json" "$OUT/$step.txt"
    python3 "$DIAG" "$OUT/$step.json" > "$OUT/$step.diag"
    echo "   exit=$rc"
    cat "$OUT/$step.diag"
}

# ---- A: restore the Option::expect the issue names; no config ------------------------------------
python3 - "$PICKER" <<'PY'
import sys
p = sys.argv[1]
src = open(p, encoding="utf-8").read()
old = ('    let Some(state) = ui.picker() else {\n'
       '        panic!("show_picker did not open a picker");\n'
       '    };\n')
new = '    let state = ui.picker().expect("show_picker did not open a picker");\n'
assert src.count(old) == 1, "let-else workaround not found exactly once"
open(p, "w", encoding="utf-8").write(src.replace(old, new))
PY
git diff -- "$PICKER" > "$OUT/A.picker.diff"
touch_members
run_step A "picker Option::expect restored, no clippy.toml (expect RED on expect_used)"

# ---- B: add clippy.toml ----------------------------------------------------------------------------
printf 'allow-expect-in-tests = true\n' > clippy.toml
touch_members
run_step B "clippy.toml allow-expect-in-tests=true (expect: no expect_used in tests; P3 = unfulfilled #[expect]?)"

# ---- C: non-test .expect("probe") with clippy.toml present ------------------------------------------
cat >> "$CORE_LIB" <<'RS'

/// cyril-ulhs P2 probe: a NON-test `.expect()` must still trip `clippy::expect_used`.
pub fn ulhs_nontest_probe() -> u8 {
    Some(1u8).expect("probe")
}
RS
git diff -- "$CORE_LIB" > "$OUT/C.core_lib.diff"
run_step C "non-test .expect(\"probe\") in $CORE_LIB with clippy.toml (expect RED at that site)"

git checkout -- "$CORE_LIB"
echo "== restored; git status for touched paths:"
git status --porcelain -- "$PICKER" "$CORE_LIB" clippy.toml
echo "PROBE-DONE"
