#!/usr/bin/env bash
set -euo pipefail
here=$(cd -- "$(dirname -- "$0")" && pwd)
fixtures=$(cd -- "$here/../crates/cyril-core/tests/fixtures/kas/workflow" && pwd)
jq -s -e '
  [.[] | (.parsed // .) | select(.method == "_kiro/workflow/run_complete") | .params.status] as $statuses
  | {failed: ($statuses | map(select(. == "failed")) | length), aborted: ($statuses | map(select(. == "aborted")) | length)}
  | if .failed >= 1 and .aborted >= 1 then . else error("missing required terminal status") end
' "$fixtures/terminal-failed-2.16.2.jsonl" "$fixtures/terminal-aborted-2.16.2.jsonl"
