#!/usr/bin/env bash
# cyril-ulhs CI-mirror gate. Each leg writes its own log and echoes GATE-OK-<name> only on a
# real exit 0 (no pipes). The three clippy legs are repeated with --keep-going as extra receipts
# (requester instruction). Logs go to the shared scratchpad under the issue id.
set -u
ROOT="$(git rev-parse --show-toplevel)" || exit 2
cd "$ROOT" || exit 2
# Repo-local by default (review N1): full logs are regenerable and gitignored; the committed
# receipt is gate-summary.txt, which carries each leg's verdict plus its closing log line.
LOGDIR="${ULHS_LOGDIR:-$ROOT/.cyril-ulhs/receipts/gate-logs}"
mkdir -p "$LOGDIR"
SUMMARY="$ROOT/.cyril-ulhs/receipts/gate-summary.txt"
: > "$SUMMARY"
# Anchor the receipt to the CODE revision (review N2): HEAD, plus proof that the code tree the
# gate sees is exactly HEAD's (no uncommitted edits under the code paths).
CODE_DIRTY=$(git status --porcelain --untracked-files=all -- clippy.toml Cargo.toml Cargo.lock crates | wc -l)
echo "# gate on $(git rev-parse HEAD) ($(git branch --show-current)), $(date -Is); uncommitted code-path changes: $CODE_DIRTY" | tee -a "$SUMMARY"

leg() { # <name> <cmd...>
    local name="$1"; shift
    local log="$LOGDIR/cyril-ulhs-$name.log"
    if env -u CARGO_TARGET_DIR "$@" > "$log" 2>&1; then
        echo "GATE-OK-$name | $(tail -1 "$log" | sed 's/^ *//')" | tee -a "$SUMMARY"
    else
        echo "GATE-FAIL-$name (exit $?) | $(tail -1 "$log" | sed 's/^ *//')" | tee -a "$SUMMARY"
    fi
}

leg fmt            cargo fmt --all -- --check
leg clippy-all     cargo clippy --workspace --all-targets --all-features -- -D warnings
leg nextest-all    cargo nextest run --workspace --all-features
leg doctest-all    cargo test --doc --workspace --all-features
leg clippy-default cargo clippy --workspace --all-targets -- -D warnings
leg nextest-default cargo nextest run --workspace
leg clippy-core-nodefault cargo clippy -p cyril-core --no-default-features --all-targets -- -D warnings
# extra receipts: the same three clippy legs with --keep-going
leg clippy-all-keepgoing     cargo clippy --workspace --all-targets --all-features --keep-going -- -D warnings
leg clippy-default-keepgoing cargo clippy --workspace --all-targets --keep-going -- -D warnings
leg clippy-core-nodefault-keepgoing cargo clippy -p cyril-core --no-default-features --all-targets --keep-going -- -D warnings

echo "# nextest totals:" | tee -a "$SUMMARY"
grep -h "Summary \[" "$LOGDIR/cyril-ulhs-nextest-all.log" "$LOGDIR/cyril-ulhs-nextest-default.log" | sed 's/^ *//' | tee -a "$SUMMARY"
echo "# picker tests in nextest-all:" | tee -a "$SUMMARY"
grep -h "picker_active_marker" "$LOGDIR/cyril-ulhs-nextest-all.log" | sed 's/^ *//' | tee -a "$SUMMARY"
echo "GATE-DONE" | tee -a "$SUMMARY"
