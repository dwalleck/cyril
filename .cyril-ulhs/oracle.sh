#!/usr/bin/env bash
# cyril-ulhs oracle runner: lint the standalone crate twice (without / with clippy.toml) and
# list the expect_used / unwrap_used / unfulfilled_lint_expectations diagnostics per site.
# Target dir is session-private (never the worktree's or any shared target/).
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
CRATE="$HERE/probe.oracle"
OUT="$HERE/probe-out"
mkdir -p "$OUT"
TARGET="${ULHS_ORACLE_TARGET:-$HERE/probe-out/oracle-target}"
DIAG="$HERE/probe-diag.py"
cd "$CRATE" || exit 2
# Clippy walks UP from the crate's manifest dir and takes the first clippy.toml it meets, so a
# config at the worktree root (the fix itself, or a concurrent probe) would leak into this crate.
# The "no-config" run therefore writes an explicit `false` here — semantically the default, and
# it shadows any ancestor. The file is left in the `false` state after the run.

run() { # <tag>
    local tag="$1"
    # --keep-going: cargo otherwise stops scheduling units after the first failing one, and an
    # unscheduled test target looks exactly like a suppressed lint (the first draft's flaw).
    env -u CARGO_TARGET_DIR cargo clippy --all-targets --keep-going --target-dir "$TARGET" --message-format=json -- -D warnings \
        > "$OUT/oracle-$tag.json" 2> "$OUT/oracle-$tag.stderr"
    local rc=$?
    echo "$rc" > "$OUT/oracle-$tag.exit"
    python3 "$DIAG" "$OUT/oracle-$tag.json" > "$OUT/oracle-$tag.diag"
    echo "== oracle $tag: exit=$rc"
    cat "$OUT/oracle-$tag.diag"
}

printf 'allow-expect-in-tests = false\n' > clippy.toml
touch src/lib.rs tests/helper.rs src/bin/nontest.rs
run no-config
printf 'allow-expect-in-tests = true\n' > clippy.toml
touch src/lib.rs tests/helper.rs src/bin/nontest.rs
run with-config
printf 'allow-expect-in-tests = false\n' > clippy.toml
echo "ORACLE-DONE"
