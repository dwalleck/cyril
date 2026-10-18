#!/usr/bin/env bash
# Leg 2: paired SCENARIO=workflow, strictly serial: gate off OLD, NEW; gate on OLD, NEW.
L=/home/dwalleck/repos/cyril/experiments/conductor-spike/audit-2.31.1/live
D=/home/dwalleck/repos/cyril/experiments/conductor-spike/probe-kas-live-2.31.1.py
C=$HOME/.local/share/kiro-research/kas-carves
for g in off on; do for v in 2.29.1 2.31.1; do
  echo "START $g $v $(date -u +%T)"
  KAS=$C/$v/node_modules/@kiro/agent/dist/server/acp-server.js LABEL=wf$g-$v SCENARIO=workflow GATE=$g PROBE_SCRATCH=$L/tmp \
    python3 -I $D $L/kas-workflow-gate$g-$v.jsonl > $L/tmp/wf-$g-$v.log 2>&1
  echo "END $g $v rc=$? $(date -u +%T)"
  pkill -f "$L/tmp/kas-wf" || true
done; done
echo ALLDONE
