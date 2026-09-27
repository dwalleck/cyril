#!/usr/bin/env bash
# cyril-brui full gate — mirrors .github/workflows/ci.yml. Each command runs
# with its REAL exit code (no pipes); a missing GATE-OK line IS a failure.
# Logs: /tmp/cyril-brui-<name>.log. Summary: .cyril-brui/gate-summary.txt
set -u
cd "$(dirname "$0")/.." || exit 2
SUMMARY=.cyril-brui/gate-summary.txt
: > "$SUMMARY"
echo "revision: $(git rev-parse --short HEAD) + working tree; branch: $(git branch --show-current); date: $(date -u +%Y-%m-%dT%H:%M:%SZ)" | tee -a "$SUMMARY"
gate() { # name, command...
  local name="$1"; shift
  if env -u CARGO_TARGET_DIR "$@" > "/tmp/cyril-brui-$name.log" 2>&1; then
    echo "GATE-OK-$name  ($*)" | tee -a "$SUMMARY"
  else
    echo "GATE-FAIL-$name  ($*)  see /tmp/cyril-brui-$name.log" | tee -a "$SUMMARY"
  fi
}
gate fmt            cargo fmt --all -- --check
gate clippy-all     cargo clippy --workspace --all-targets --all-features -- -D warnings
gate nextest-all    cargo nextest run --workspace --all-features
gate doctest-all    cargo test --doc --workspace --all-features
gate clippy-default cargo clippy --workspace --all-targets -- -D warnings
gate nextest-default cargo nextest run --workspace
gate clippy-kas     cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings
gate nextest-kas    cargo nextest run -p cyril -p cyril-core --features kas
gate clippy-nodefault cargo clippy -p cyril-core --no-default-features --all-targets -- -D warnings
gate nextest-nodefault cargo nextest run -p cyril-core --no-default-features
echo "done: $(date -u +%Y-%m-%dT%H:%M:%SZ)" | tee -a "$SUMMARY"
grep -c "^GATE-OK" "$SUMMARY" | sed 's/^/GATE-OK count (expect 10): /' | tee -a "$SUMMARY"
