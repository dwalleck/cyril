#!/usr/bin/env bash
# TUI-lane T-series probes. Serial. T4 (TUI spawn env) and T6 (OAuth MCP server) are NOT run here:
# T4 needs a TUI-spawned process (the tui/host lanes own it); T6 needs an OAuth-requiring MCP server.
set -u
cd "$(dirname "$0")"
KOLD=/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js
KNEW=/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js
leg(){ tag="$1"; kas="$2"; probe="$3"; shift 3
  echo "=== $probe $tag ${*:-} ==="
  env $* KAS=$kas PROBE_SCRATCH=$(pwd)/tmp timeout 300 python3 -I ./probe-kas-leads-2.29.1.py "$probe" "kas-t-$probe-$tag.jsonl" >"tmp/t-$probe-$tag.log" 2>&1; echo "rc=$?"; }
leg 2.29.1 "$KNEW" ltoh KIRO_LOG_LEVEL=debug
leg 2.28.0 "$KOLD" ltoh KIRO_LOG_LEVEL=debug
leg 2.29.1 "$KNEW" memory
leg 2.28.0 "$KOLD" memory
leg 2.29.1 "$KNEW" override
leg 2.28.0 "$KOLD" override
leg 2.29.1 "$KNEW" interaction
leg 2.28.0 "$KOLD" interaction
echo "TSERIES DONE"
