#!/usr/bin/env bash
set -euo pipefail
here=$(cd -- "$(dirname -- "$0")" && pwd)
fixtures=$(cd -- "$here/../crates/cyril-core/tests/fixtures/kas/workflow" && pwd)

# Per-file gate: the union count alone cannot see a wrong split (both
# statuses in one capture, none in the other, still sums the same), so each
# capture must itself contain at least one run_complete with its own status.
require_terminal() {
  jq -s -e --arg status "$2" '
    [.[] | (.parsed // .) | select(.method == "_kiro/workflow/run_complete") | .params.status]
    | map(select(. == $status)) | length >= 1
  ' "$1" >/dev/null || {
    printf 'oracle.sh: %s contains no %s run_complete\n' "$1" "$2" >&2
    exit 1
  }
}
require_terminal "$fixtures/terminal-failed-2.16.2.jsonl" failed
require_terminal "$fixtures/terminal-aborted-2.16.2.jsonl" aborted

jq -s -e '
  [.[] | (.parsed // .) | select(.method == "_kiro/workflow/run_complete") | .params.status] as $statuses
  | {failed: ($statuses | map(select(. == "failed")) | length), aborted: ($statuses | map(select(. == "aborted")) | length)}
  | if .failed >= 1 and .aborted >= 1 then . else error("missing required terminal status") end
' "$fixtures/terminal-failed-2.16.2.jsonl" "$fixtures/terminal-aborted-2.16.2.jsonl"
