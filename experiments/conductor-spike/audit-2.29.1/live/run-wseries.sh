#!/usr/bin/env bash
# kas-workflow W1 (include expansion) / W2 (include negatives) / W3 (consentRound on a step
# approval). Gate-independent (ext methods are advertised unconditionally) -> settings=None.
set -u
cd "$(dirname "$0")"
KOLD=/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js
KNEW=/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js
leg(){ echo "=== wseries $2 ==="; KAS=$1 LABEL=wseries-$2 SCENARIO=wseries PROBE_SCRATCH=$(pwd)/tmp \
  timeout 2400 python3 -I ./probe-kas-workflow-2.29.1.py "kas-wseries-$2.jsonl" >"tmp/wseries-$2.log" 2>&1; echo "rc=$?"; }
leg "$KNEW" 2.29.1
leg "$KOLD" 2.28.0
echo "WSERIES DONE"
