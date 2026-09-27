#!/usr/bin/env bash
# cyril-ulhs checkpoint receipts on the FINAL tree: named mutations M1/M2/M3 (design.md) and the
# C5 non-test probe, each applied, linted with the CI gate (+ --keep-going + JSON), and reverted.
# Ends with a green re-run. Outputs under .cyril-ulhs/receipts/<tag>.{diag,exit,txt}.
#
# Run from anywhere inside the worktree on a clean tree:  bash .cyril-ulhs/receipts.sh
set -u
ROOT="$(git rev-parse --show-toplevel)" || exit 2
cd "$ROOT" || exit 2
OUT="$ROOT/.cyril-ulhs/receipts"
mkdir -p "$OUT"
DIAG="$ROOT/.cyril-ulhs/probe-diag.py"
PATHS_RS=crates/cyril-memory/src/paths.rs
CORE_LIB=crates/cyril-core/src/lib.rs
GATE=(env -u CARGO_TARGET_DIR cargo clippy --workspace --all-targets --all-features --keep-going --message-format=json -- -D warnings)

if [ ! -f clippy.toml ]; then
    echo "PRECONDITION-FAIL: clippy.toml missing" >&2
    exit 2
fi
# Snapshot the files the mutations touch (the working tree may hold the slice's own uncommitted
# edits, so restoration is from these copies, never from git).
cp clippy.toml "$OUT/clippy.toml.orig"
cp "$PATHS_RS" "$OUT/paths.rs.orig"
cp "$CORE_LIB" "$OUT/core_lib.rs.orig"
touch_members() { touch crates/*/src/lib.rs crates/cyril/src/main.rs; }
restore() {
    cp "$OUT/clippy.toml.orig" clippy.toml
    cp "$OUT/paths.rs.orig" "$PATHS_RS"
    cp "$OUT/core_lib.rs.orig" "$CORE_LIB"
    touch_members
}
trap restore EXIT

render() { # <json> <txt>
    python3 - "$1" > "$2" <<'PY'
import json, sys
for raw in open(sys.argv[1], encoding="utf-8"):
    raw = raw.strip()
    if not raw.startswith("{"):
        continue
    try:
        obj = json.loads(raw)
    except json.JSONDecodeError:
        continue
    if obj.get("reason") == "compiler-message":
        sys.stdout.write((obj.get("message") or {}).get("rendered") or "")
PY
}
run_step() { # <tag> <label>
    local tag="$1" label="$2"
    echo "== $tag: $label"
    "${GATE[@]}" > "$OUT/$tag.json" 2> "$OUT/$tag.stderr"
    local rc=$?
    echo "$rc" > "$OUT/$tag.exit"
    render "$OUT/$tag.json" "$OUT/$tag.txt"
    python3 "$DIAG" "$OUT/$tag.json" > "$OUT/$tag.diag"
    # Units receipt: cargo stops scheduling after the first failing unit unless --keep-going;
    # record that every workspace target (esp. the picker test crate) was actually linted.
    python3 - "$OUT/$tag.json" > "$OUT/$tag.units" <<'PY'
import json, sys
kinds = {}
names = set()
for raw in open(sys.argv[1], encoding="utf-8"):
    raw = raw.strip()
    if not raw.startswith("{"):
        continue
    try:
        obj = json.loads(raw)
    except json.JSONDecodeError:
        continue
    if obj.get("reason") in ("compiler-artifact", "compiler-message"):
        t = obj.get("target") or {}
        src = t.get("src_path") or ""
        if "/crates/" in src:
            names.add((",".join(t.get("kind", [])), t.get("name")))
for k, n in sorted(names):
    print(f"{k:10s} {n}")
print(f"# {len(names)} workspace units; picker_active_marker linted: {('test', 'picker_active_marker') in names}")
PY
    rm -f "$OUT/$tag.json"
    echo "   exit=$rc"
    cat "$OUT/$tag.diag"
    tail -1 "$OUT/$tag.units"
}

# M1: config absent -> expect_used at the three picker #[test] sites (plus collateral)
rm clippy.toml; touch_members
run_step M1 "rm clippy.toml (expect RED: expect_used at picker_active_marker.rs test lines)"
cp "$OUT/clippy.toml.orig" clippy.toml

# M2: one #[expect(clippy::expect_used)] re-added -> unfulfilled_lint_expectations there
python3 - "$PATHS_RS" <<'PY'
import sys
p = sys.argv[1]
src = open(p, encoding="utf-8").read()
old = "#[cfg(test)]\nmod tests {\n"
assert src.count(old) == 1, "expected exactly one `#[cfg(test)]\\nmod tests {` in paths.rs"
open(p, "w", encoding="utf-8").write(src.replace(old, "#[cfg(test)]\n#[expect(clippy::expect_used)]\nmod tests {\n"))
PY
diff -u "$OUT/paths.rs.orig" "$PATHS_RS" > "$OUT/M2.diff"
touch_members
run_step M2 "re-add #[expect(clippy::expect_used)] in $PATHS_RS (expect RED: unfulfilled_lint_expectations there)"
cp "$OUT/paths.rs.orig" "$PATHS_RS"

# M3: allow-unwrap-in-tests -> the 8 #[expect(clippy::unwrap_used)] become unfulfilled
printf 'allow-unwrap-in-tests = true\n' >> clippy.toml
touch_members
run_step M3 "append allow-unwrap-in-tests = true (expect RED: unfulfilled_lint_expectations at the 8 unwrap_used sites)"
cp "$OUT/clippy.toml.orig" clippy.toml

# C5 receipt: non-test .expect() still trips with the config present
cat >> "$CORE_LIB" <<'RS'

/// cyril-ulhs C5 receipt: a NON-test `.expect()` must still trip `clippy::expect_used`.
pub fn ulhs_nontest_probe() -> u8 {
    Some(1u8).expect("probe")
}
RS
diff -u "$OUT/core_lib.rs.orig" "$CORE_LIB" > "$OUT/C5.diff"
run_step C5 "non-test .expect(\"probe\") in $CORE_LIB with clippy.toml (expect RED at that line)"
cp "$OUT/core_lib.rs.orig" "$CORE_LIB"

# restored green
touch_members
run_step GREEN "final tree restored (expect exit 0, zero diagnostics of interest)"
echo "== restoration check (cmp against snapshots):"
for pair in "clippy.toml.orig:clippy.toml" "paths.rs.orig:$PATHS_RS" "core_lib.rs.orig:$CORE_LIB"; do
    if cmp -s "$OUT/${pair%%:*}" "${pair#*:}"; then echo "RESTORED-OK ${pair#*:}"; else echo "RESTORED-FAIL ${pair#*:}"; fi
done
trap - EXIT  # restoration verified above; the trap must not re-run against deleted snapshots
rm -f "$OUT"/*.orig
echo "RECEIPTS-DONE"
