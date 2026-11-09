#!/usr/bin/env bash
# Serial lead runner: each line "probe|engine|extra env" ; engine = new|old. Never two drivers at once.
set -u
cd /home/dwalleck/repos/cyril/experiments/conductor-spike
L=$PWD/audit-3.1.0/live
OLD=$HOME/.local/share/kiro-research/kas-carves/2.32.0/node_modules/@kiro/agent/dist/server/acp-server.js
NEW=$HOME/.local/share/kiro-research/kas-carves/3.1.0/node_modules/@kiro/agent/dist/server/acp-server.js
NO=$HOME/.local/share/kiro-cli/node
NN=$HOME/.local/share/kiro-research/binaries/3.1.0/node
while IFS='|' read -r probe engine extra; do
  [ -z "$probe" ] && continue
  if [ "$engine" = new ]; then js=$NEW; nd=$NN; ver=3.1.0; else js=$OLD; nd=$NO; ver=2.32.0; fi
  tag=${TAGSUFFIX:-}
  out=$L/kas-lead-${probe}${tag}-${ver}.jsonl
  echo "== $(date -u +%H:%M:%S) START $probe $engine ($extra) -> $(basename $out)"
  t0=$(date +%s)
  env KAS=$js KAS_NODE=$nd KAS_OLD=$OLD KAS_NODE_OLD=$NO LABEL=lead-$probe-$ver PROBE_SCRATCH=$L/tmp $extra python3 -I probe-kas-leads-3.1.0.py "$probe" "$out" > $L/tmp/lead-${probe}${tag}-${ver}.stdout 2>&1
  rc=$?
  echo "== $(date -u +%H:%M:%S) END $probe $engine rc=$rc secs=$(( $(date +%s)-t0 )) frames=$(wc -l < "$out" 2>/dev/null)"
  pkill -f "$L/tmp" 2>/dev/null; pgrep -af acp-server.js | grep -q "$L/tmp" && echo "ORPHANS REMAIN" 
done
echo "== $(date -u +%H:%M:%S) CHAIN DONE"
