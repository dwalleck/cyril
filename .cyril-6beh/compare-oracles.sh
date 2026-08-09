#!/usr/bin/env bash
set -euo pipefail
here=$(cd -- "$(dirname -- "$0")" && pwd)
repo=$(cd -- "$here/.." && pwd)
expected=$(mktemp)
trap 'rm -f -- "$expected"' EXIT

case "${1:-}" in
  manifest)
    cp -- "$here/oracle-manifest.json" "$expected"
    test_name=workflow_oracle_manifest_matches_binary
    ;;
  terminal)
    "$here/oracle.sh" >"$expected"
    test_name=workflow_capture_terminal_projection_matches_oracle
    ;;
  snapshot)
    "$here/oracle-snapshot.py" \
      "$here/terminal-failed-2.16.2.jsonl" \
      "$here/terminal-aborted-2.16.2.jsonl" >"$expected"
    test_name=workflow_capture_state_matches_oracle
    ;;
  replay)
    "$here/oracle-replay.py" \
      "$here/oracle-replay-events.jsonl" \
      "$here/terminal-failed-2.16.2.jsonl" \
      "$here/terminal-aborted-2.16.2.jsonl" \
      "$repo/experiments/conductor-spike/kas-repeat-watch-2.16.0.jsonl" >"$expected"
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
