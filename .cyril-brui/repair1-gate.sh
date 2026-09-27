#!/usr/bin/env bash
# cyril-brui bounded repair 1 (Windows path-rendering assertion in
# `nothing_found_names_search_root`): the legs the coordinator asked to
# re-run locally — fmt, the kas clippy/nextest legs, the default
# clippy/nextest legs. Real exit codes; a missing GATE-OK IS a failure.
# Logs: /tmp/cyril-brui-repair1-<name>.log. Summary: repair1-gate-summary.txt
set -u
cd "$(dirname "$0")/.." || exit 2
SUMMARY=.cyril-brui/repair1-gate-summary.txt
: > "$SUMMARY"
echo "revision: $(git rev-parse --short HEAD) + working tree; branch: $(git branch --show-current); date: $(date -u +%Y-%m-%dT%H:%M:%SZ)" | tee -a "$SUMMARY"
gate() {
  local name="$1"; shift
  if env -u CARGO_TARGET_DIR "$@" > "/tmp/cyril-brui-repair1-$name.log" 2>&1; then
    echo "GATE-OK-$name  ($*)" | tee -a "$SUMMARY"
  else
    echo "GATE-FAIL-$name  ($*)  see /tmp/cyril-brui-repair1-$name.log" | tee -a "$SUMMARY"
  fi
}
gate fmt             cargo fmt --all -- --check
gate clippy-kas      cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings
gate nextest-kas     cargo nextest run -p cyril -p cyril-core --features kas
gate clippy-default  cargo clippy --workspace --all-targets -- -D warnings
gate nextest-default cargo nextest run --workspace
echo "done: $(date -u +%Y-%m-%dT%H:%M:%SZ)" | tee -a "$SUMMARY"
grep -c "^GATE-OK" "$SUMMARY" | sed 's/^/GATE-OK count (expect 5): /' | tee -a "$SUMMARY"
grep -h "Summary" /tmp/cyril-brui-repair1-nextest-*.log | tee -a "$SUMMARY"
