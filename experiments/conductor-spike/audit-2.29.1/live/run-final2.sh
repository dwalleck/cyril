#!/usr/bin/env bash
set -u
cd "$(dirname "$0")"
B291=/home/dwalleck/.local/share/kiro-research/binaries/2.29.1/extracted/kirocli/bin
B280=/home/dwalleck/.local/share/kiro-research/binaries/2.28.0/extracted/kirocli/bin
# wait for the main chain to finish (single Kiro at a time)
while ! grep -q "FINAL DONE" tmp/remaining.log 2>/dev/null; do sleep 10; done
echo "[final2] start $(date +%T)"
for pair in "2.29.1 $B291" "2.28.0 $B280"; do set -- $pair
  echo "=== v2 sweep $1 ==="
  PROBE_SCRATCH=$(pwd)/tmp timeout 900 python3 -I ./probe-v2-live-sweep-2.29.1.py "$2" "v2-live-sweep-$1.jsonl" >"tmp/v2-sweep-$1.log" 2>&1; echo "rc=$?"
done
# H4 discovery + real command
echo "=== kiro-cli internal --help ==="
timeout 60 env HOME=$(mktemp -d tmp/h4help-XXXX) XDG_DATA_HOME=$HOME/.local/share $B291/kiro-cli internal --help >tmp/h4-help.log 2>&1; echo "rc=$?"; cat tmp/h4-help.log
echo "[final2] DONE $(date +%T)"
