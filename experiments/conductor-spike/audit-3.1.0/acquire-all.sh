#!/usr/bin/env bash
# acquire-all.sh — serial acquisition of every release after OLD into the research archive, plus feed copies.
set -uo pipefail
cd /home/dwalleck/repos/cyril
AUD=experiments/conductor-spike/audit-3.1.0
for v in "$@"; do
  echo "=== $v $(date -u +%FT%TZ)"
  curl -s --max-time 30 -o "$AUD/feed-$v.json" "https://prod.download.cli.kiro.dev/stable/$v/feed.json" || echo "feed fetch failed $v"
  env -u CARGO_TARGET_DIR bash .claude/skills/kiro-release-audit/scripts/acquire.sh "$v" > "$AUD/acquire-$v.log" 2>&1
  rc=$?
  tail -3 "$AUD/acquire-$v.log"
  echo "=== $v rc=$rc"
done
echo "ACQUIRE-ALL-DONE"
