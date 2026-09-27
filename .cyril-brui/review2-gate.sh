#!/usr/bin/env bash
# cyril-brui review round 1 re-review nits N1+N2 (ios cfg arm + wording,
# `env_wrappers_resolve_xdg_data_home_like_kiro_cli`): the legs asked to
# re-run locally — fmt, the kas clippy/nextest legs, the default
# clippy/nextest legs. Real exit codes; a missing GATE-OK IS a failure.
# Logs: /tmp/cyril-brui-review2-<name>.log. Summary: review2-gate-summary.txt
set -u
cd "$(dirname "$0")/.." || exit 2
SUMMARY=.cyril-brui/review2-gate-summary.txt
: > "$SUMMARY"
echo "revision: $(git rev-parse --short HEAD) + working tree; branch: $(git branch --show-current); date: $(date -u +%Y-%m-%dT%H:%M:%SZ)" | tee -a "$SUMMARY"
gate() {
  local name="$1"; shift
  if env -u CARGO_TARGET_DIR "$@" > "/tmp/cyril-brui-review2-$name.log" 2>&1; then
    echo "GATE-OK-$name  ($*)" | tee -a "$SUMMARY"
  else
    echo "GATE-FAIL-$name  ($*)  see /tmp/cyril-brui-review2-$name.log" | tee -a "$SUMMARY"
  fi
}
gate fmt             cargo fmt --all -- --check
gate clippy-kas      cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings
gate nextest-kas     cargo nextest run -p cyril -p cyril-core --features kas
echo "done: $(date -u +%Y-%m-%dT%H:%M:%SZ)" | tee -a "$SUMMARY"
grep -c "^GATE-OK" "$SUMMARY" | sed 's/^/GATE-OK count (expect 5): /' | tee -a "$SUMMARY"
grep -h "Summary" /tmp/cyril-brui-review2-nextest-*.log | tee -a "$SUMMARY"
