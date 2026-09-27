#!/usr/bin/env bash
# cyril-ulhs CI-mirror gate. Each leg writes its own log and echoes GATE-OK-<name> only on a
# real exit 0 (no pipes). The three clippy legs are repeated with --keep-going as extra receipts
# (requester instruction). Logs go to the shared scratchpad under the issue id.
set -u
ROOT="$(git rev-parse --show-toplevel)" || exit 2
cd "$ROOT" || exit 2
LOGDIR="${ULHS_LOGDIR:-$HOME/.claude/tmp/claude-1000/-home-dwalleck-repos-cyril/3eae664a-3aa8-48bc-9dc5-496da275da18/scratchpad}"
mkdir -p "$LOGDIR"
SUMMARY="$ROOT/.cyril-ulhs/receipts/gate-summary.txt"
: > "$SUMMARY"
echo "# gate on $(git rev-parse --short HEAD) ($(git branch --show-current)), $(date -Is)" | tee -a "$SUMMARY"

leg() { # <name> <cmd...>
    local name="$1"; shift
    local log="$LOGDIR/cyril-ulhs-$name.log"
    if env -u CARGO_TARGET_DIR "$@" > "$log" 2>&1; then
        echo "GATE-OK-$name" | tee -a "$SUMMARY"
    else
        echo "GATE-FAIL-$name (exit $?) see $log" | tee -a "$SUMMARY"
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

echo "# picker tests in nextest-all:" | tee -a "$SUMMARY"
grep -h "picker_active_marker" "$LOGDIR/cyril-ulhs-nextest-all.log" | tee -a "$SUMMARY"
echo "GATE-DONE" | tee -a "$SUMMARY"
