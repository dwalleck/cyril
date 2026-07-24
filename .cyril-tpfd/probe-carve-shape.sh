#!/usr/bin/env bash
# cyril-tpfd static probe: carve the AcpPrecomputedHookResult element shape
# from a KAS bundle's acp-server.js, from TWO independent code sites:
#   PRODUCER  — the v2 standalone provider's extractPrecomputedResults,
#               which CONSTRUCTS the elements (object-literal keys).
#   CONSUMER  — handlePrecomputedTrigger, which reads fields off each
#               element (member accesses on `result.`).
# The two sites agreeing on a field set = the carved shape. Re-run per
# release as a drift fence (pass a bundle dir, default = newest on disk).
set -euo pipefail

KAS_ROOT="$HOME/.local/share/kiro-cli/kas"
BUNDLE="${1:-$(ls -d "$KAS_ROOT"/*/ | sort -V | tail -1)}"
JS="$BUNDLE/node_modules/@kiro/agent/dist/server/acp-server.js"
[ -f "$JS" ] || { echo "no acp-server.js under $BUNDLE" >&2; exit 1; }
echo "bundle: $(basename "$BUNDLE")"

echo "-- PRODUCER field keys (v2 extractPrecomputedResults push sites) --"
grep -A30 'for (const r5 of rs2.results)' "$JS" \
  | grep -oE '^[[:space:]]+(id|name|hookId|originalType|content):' \
  | tr -d ' :' | sort | uniq -c

echo "-- PRODUCER originalType values --"
grep -A30 'for (const r5 of rs2.results)' "$JS" \
  | grep -oE 'originalType: "[a-zA-Z]+"' | sort -u

echo "-- CONSUMER field accesses (handlePrecomputedTrigger body) --"
awk '/^async function handlePrecomputedTrigger/,/^}/' "$JS" \
  | grep -oE 'result\.[a-zA-Z]+' | sort | uniq -c

echo "-- CONSUMER unknown-originalType guard (assertNever = throws) --"
awk '/^function wireTypeToActionKind/,/^}/' "$JS"
