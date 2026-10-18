#!/usr/bin/env bash
# Leg 2 — paired SCENARIO=workflow, gate off then on, OLD then NEW. Strictly serial.
set -u
cd "$(dirname "$0")"
KOLD=/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js
KNEW=/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js
leg(){ echo "=== workflow gate=$2 $3 ==="; KAS=$1 LABEL=wf-$2-$3 GATE=$2 SCENARIO=workflow PROBE_SCRATCH=$(pwd)/tmp \
  timeout 3000 python3 -I ./probe-kas-live-2.29.1.py "kas-workflow-gate$2-$3.jsonl" >"tmp/wf-$2-$3.log" 2>&1; echo "rc=$?"; }
leg "$KOLD" off 2.28.0
leg "$KOLD" on  2.28.0
leg "$KNEW" off 2.29.1
leg "$KNEW" on  2.29.1
echo "WORKFLOW LEGS DONE"
