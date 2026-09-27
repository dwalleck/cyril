#!/usr/bin/env bash
# cyril-ulhs probe, step B repeated with `--keep-going`: cargo stops scheduling new units after
# the first failing unit, so probe.sh's step B could not show whether cyril-ui's integration
# test target (picker line 70) was linted at all, nor list every `#[expect]` site. This run
# forces every unit to build. Same edits as probe.sh step B; everything is reverted on exit.
#
# Run from anywhere inside the worktree:  bash .cyril-ulhs/probe-keepgoing.sh
set -u
ROOT="$(git rev-parse --show-toplevel)" || exit 2
cd "$ROOT" || exit 2
OUT="$ROOT/.cyril-ulhs/probe-out"
mkdir -p "$OUT"
PICKER=crates/cyril-ui/tests/picker_active_marker.rs
DIAG="$ROOT/.cyril-ulhs/probe-diag.py"
GATE=(env -u CARGO_TARGET_DIR cargo clippy --workspace --all-targets --all-features --keep-going --message-format=json -- -D warnings)

if ! git diff --quiet -- "$PICKER" || [ -e clippy.toml ] || [ -e .clippy.toml ]; then
    echo "PRECONDITION-FAIL: $PICKER modified or a clippy config already exists" >&2
    exit 2
fi
touch_members() { touch crates/*/src/lib.rs crates/cyril/src/main.rs; }
restore() {
    git checkout -- "$PICKER"
    rm -f clippy.toml
    touch_members
}
trap restore EXIT

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
printf 'allow-expect-in-tests = true\n' > clippy.toml
touch_members
echo "== step B-keepgoing: picker Option::expect + clippy.toml, --keep-going"
"${GATE[@]}" > "$OUT/B-keepgoing.json" 2> "$OUT/B-keepgoing.stderr"
rc=$?
echo "$rc" > "$OUT/B-keepgoing.exit"
python3 "$DIAG" "$OUT/B-keepgoing.json" > "$OUT/B-keepgoing.diag"
echo "   exit=$rc"
cat "$OUT/B-keepgoing.diag"
echo "== units that produced artifacts or messages:"
python3 - "$OUT/B-keepgoing.json" <<'PY'
import json, sys, collections
seen = collections.Counter()
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
        seen[(",".join(t.get("kind", [])), t.get("name"), "/".join((t.get("src_path") or "").split("/")[-3:]))] += 1
for k, n in sorted(seen.items()):
    print(f"{n:4d}  {k[0]:12s} {k[1]:28s} {k[2]}")
PY
echo "PROBE-KEEPGOING-DONE"
