#!/usr/bin/env bash
set -u
cd "$(dirname "$0")"
KOLD=/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js
KNEW=/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js
B291=/home/dwalleck/.local/share/kiro-research/binaries/2.29.1/extracted/kirocli/bin
while ! grep -q "\[final2\] DONE" tmp/final2.log 2>/dev/null; do sleep 10; done
echo "[cmds] start $(date +%T)"
for pair in "2.29.1 $KNEW" "2.28.0 $KOLD"; do set -- $pair
  echo "=== commands $1 ==="
  KAS=$2 PROBE_SCRATCH=$(pwd)/tmp timeout 300 python3 -I ./probe-kas-leads-2.29.1.py commands "kas-lead-commands-$1.jsonl" >"tmp/commands-$1.log" 2>&1; echo "rc=$?"
done
# H4: which binary carries the subcommand?  (`_` = `internal`)
for B in kiro-cli kiro-cli-chat; do
  H=$(mktemp -d tmp/h4b-XXXX)
  echo "=== $B internal --help ==="
  timeout 60 env HOME="$H" XDG_DATA_HOME=$HOME/.local/share $B291/$B internal --help >"tmp/h4b-$B-internal-help.log" 2>&1; echo "rc=$?"
  echo "=== $B _ cleanup-autocomplete-artifacts --dry-run ==="
  timeout 60 env HOME="$H" XDG_DATA_HOME=$HOME/.local/share $B291/$B _ cleanup-autocomplete-artifacts --dry-run >"tmp/h4b-$B-cleanup.log" 2>&1; echo "rc=$?"
done
echo "[cmds] DONE $(date +%T)"
