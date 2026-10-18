#!/bin/bash
# cyril-zbu7 local gate: every ci.yml lane. Usage: gate.sh <out-dir> [static]
#   static = fmt + clippy lanes only (no tests run).
# Each lane logs to <out-dir>/<lane>.log and prints <lane>-OK / <lane>-FAIL;
# a missing OK line is a failure. Summary goes to <out-dir>/summary.txt.
cd "$(dirname "$0")/.." || exit 99
out=$1; mode=${2:-full}
mkdir -p "$out"
summary=$out/summary.txt
{
  echo "# rev: $(git rev-parse HEAD) dirty-paths: $(git status --porcelain | wc -l)"
  echo "# date: $(date -u +%FT%TZ) mode: $mode"
} > "$summary"
lane() {
  local name=$1; shift
  if env -u CARGO_TARGET_DIR "$@" > "$out/$name.log" 2>&1; then
    echo "$name-OK" | tee -a "$summary"
  else
    echo "$name-FAIL (exit $?)" | tee -a "$summary"
  fi
}
lane fmt cargo fmt --all -- --check
lane clippy-all-features cargo clippy --workspace --all-targets --all-features -- -D warnings
lane clippy-kas cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings
lane clippy-default cargo clippy --workspace --all-targets -- -D warnings
lane clippy-core-no-default cargo clippy -p cyril-core --no-default-features --all-targets -- -D warnings
[ "$mode" = static ] && exit 0
lane nextest-all-features cargo nextest run --workspace --all-features --no-fail-fast
lane doctest cargo test --doc --workspace --all-features
lane nextest-kas cargo nextest run -p cyril -p cyril-core --features kas --no-fail-fast
lane nextest-default cargo nextest run --workspace --no-fail-fast
lane nextest-core-no-default cargo nextest run -p cyril-core --no-default-features --no-fail-fast
