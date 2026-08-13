#!/usr/bin/env bash
# bh7g node shim: cyril-core's KAS free path invokes this as `node` via
# KIRO_AGENT_PATH. It interposes tap.py between the bridge and the real node.
# BH7G_TAP_LOG names the capture file (set by driver.py per run).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAL_NODE="$(command -v node)"
exec python3 "$HERE/tap.py" "${BH7G_TAP_LOG:?BH7G_TAP_LOG not set}" "$REAL_NODE" "$@"
