#!/usr/bin/env bash
set -euo pipefail
here=$(cd -- "$(dirname -- "$0")" && pwd)
repo=$(cd -- "$here/.." && pwd)
fixtures=$repo/crates/cyril-core/tests/fixtures/kas/workflow
expected=$(mktemp)
trap 'rm -f -- "$expected"' EXIT

case "${1:-}" in
  manifest)
    cp -- "$fixtures/oracle-manifest.json" "$expected"
    test_name=workflow_oracle_manifest_matches_binary
    ;;
  terminal)
    "$here/oracle.sh" >"$expected"
    test_name=workflow_capture_terminal_projection_matches_oracle
    ;;
  snapshot)
    "$here/oracle-snapshot.py" \
      "$fixtures/terminal-failed-2.16.2.jsonl" \
      "$fixtures/terminal-aborted-2.16.2.jsonl" >"$expected"
    test_name=workflow_capture_state_matches_oracle
    ;;
  replay)
    # Order must match REPLAY_SOURCES in convert/kas/workflow.rs.
    "$here/oracle-replay.py" \
      "$fixtures/oracle-replay-events.jsonl" \
      "$fixtures/terminal-failed-2.16.2.jsonl" \
      "$fixtures/terminal-aborted-2.16.2.jsonl" \
      "$fixtures/kas-repeat-watch-2.16.0.jsonl" \
      "$fixtures/kas-custom-dag-2.16.0.jsonl" \
      "$fixtures/kas-csig-2.16.0-neutral.jsonl" \
      "$fixtures/kas-csig-2.16.2-neutral.jsonl" \
      "$fixtures/kas-csig-2.16.2-explicit.jsonl" \
      "$fixtures/pause-late-summary-2.18.0-source-derived.jsonl" >"$expected"
    test_name=workflow_capture_replay_matches_independent_folder
    ;;
  *)
    printf 'usage: %s manifest|terminal|snapshot|replay\n' "$0" >&2
    exit 2
    ;;
esac

cd -- "$repo"
CYRIL_WORKFLOW_ORACLE_EXPECTED="$expected" \
CYRIL_WORKFLOW_ORACLE_MODE="$1" \
  cargo test -p cyril-core --features kas "$test_name" -- --nocapture
