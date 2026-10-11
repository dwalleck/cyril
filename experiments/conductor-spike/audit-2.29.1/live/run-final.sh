#!/usr/bin/env bash
# Final legs: fallback-debug, v2 host pair (leg 5 / H5), H1 auth-method, H3, H4.
set -u
cd "$(dirname "$0")"
KNEW=/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js
KOLD=/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js
B291=/home/dwalleck/.local/share/kiro-research/binaries/2.29.1/extracted/kirocli/bin
B280=/home/dwalleck/.local/share/kiro-research/binaries/2.28.0/extracted/kirocli/bin

# 1. model-fallback with debug logging
for pair in "2.29.1 $KNEW" "2.28.0 $KOLD"; do set -- $pair
  echo "=== fallback-debug $1 ==="
  KIRO_LOG_LEVEL=debug KAS=$2 PROBE_SCRATCH=$(pwd)/tmp timeout 300 python3 -I ./probe-kas-leads-2.29.1.py fallback "kas-lead-fallback-debug-$1.jsonl" >"tmp/lead-fallback-debug-$1.log" 2>&1; echo "rc=$?"
done

# 2. v2 host pair (leg 5). PATH prepended by the probe; /proc exe recorded.
for pair in "2.29.1 $B291" "2.28.0 $B280"; do set -- $pair
  echo "=== v2 sweep $1 ==="
  PROBE_SCRATCH=$(pwd)/tmp timeout 900 python3 -I ./probe-v2-live-sweep-2.28.0.py "$2" "v2-live-sweep-$1.jsonl" >"tmp/v2-sweep-$1.log" 2>&1; echo "rc=$?"
done

# 3. H3 agent upgrade fixture
FIX=$(pwd)/tmp/h3-fixture; rm -rf "$FIX"; mkdir -p "$FIX"
printf '%s\n' '{"name":"legacy","description":"legacy fs_read hooks","hooks":{"preToolUse":[{"matcher":"fs_read","command":"echo legacy"}]},"prompt":"legacy agent"}' > "$FIX/legacy.json"
printf '%s\n' '{"name":"universal","description":"universal config","universal":true,"prompt":"u"}' > "$FIX/universal.json"
printf '%s\n' '{"hello":"world"}' > "$FIX/notes.json"
printf '%s\n' '{"name":"outside","prompt":"outside the dir"}' > "$(pwd)/tmp/h3-outside.json"
ln -sf "$(pwd)/tmp/h3-outside.json" "$FIX/link.json"
H3HOME=$(mktemp -d tmp/h3-home-XXXX)
for CMD in "agent upgrade $FIX --dry-run" "agent upgrade $FIX"; do
  echo "=== kiro-cli-chat $CMD ==="
  timeout 120 env HOME="$H3HOME" XDG_DATA_HOME=$HOME/.local/share $B291/kiro-cli-chat $CMD >"tmp/h3-$(echo $CMD|tr ' /' '__').log" 2>&1; echo "rc=$?"
done
echo "--- fixture after ---"; ls -la "$FIX"
for f in "$FIX"/*; do echo "== $f"; cat "$f"; done

# 4. H4 cleanup-autocomplete-artifacts
H4HOME=$(mktemp -d tmp/h4-home-XXXX)
mkdir -p "$H4HOME/.local/bin"
for n in q qchat kiro-cli-term; do ln -sf "$B291/kiro-cli" "$H4HOME/.local/bin/$n"; done
printf '%s\n' 'eval "$(q init)"' 'export PATH=~/.local/bin:$PATH' > "$H4HOME/.bashrc"
echo "=== h4 cleanup --dry-run ==="
timeout 120 env HOME="$H4HOME" XDG_DATA_HOME=$HOME/.local/share $B291/kiro-cli _ cleanup-autocomplete-artifacts --dry-run >tmp/h4-dryrun.log 2>&1; echo "rc=$?"
cat tmp/h4-dryrun.log
echo "--- links after ---"; ls -la "$H4HOME/.local/bin"

# 5. acp flag discovery
echo "=== kiro-cli acp --help ==="
timeout 60 env HOME=$(mktemp -d tmp/acphelp-XXXX) XDG_DATA_HOME=$HOME/.local/share $B291/kiro-cli acp --help 2>&1 | head -40

# 6. H1 auth-method pair (client then cli; cli last)
for M in client cli; do
  echo "=== H1 --auth-method $M ==="
  ACP_ARGS="acp --agent-engine v3 --auth-method $M" PROBE_SCRATCH=$(pwd)/tmp timeout 600 \
    python3 -I ./probe-v2-acpmethod-2.29.1.py "$B291" "v2-acpmethod-$M.jsonl" >"tmp/h1-$M.log" 2>&1; echo "rc=$?"
done
# 7. telemetry handshake pair (CONNECT-logging proxy counts *.kiro.dev connects)
for pair in "2.29.1 $KNEW" "2.28.0 $KOLD"; do set -- $pair
  echo "=== telemetry $1 ==="
  TELEMETRY_IDLE=25 KAS=$2 PROBE_SCRATCH=$(pwd)/tmp timeout 900 \
    python3 -I ./probe-kas-initsettings-2.29.1.py telemetry "kas-telemetry-$1.jsonl" >"tmp/telemetry-$1.log" 2>&1; echo "rc=$?"
done

echo "FINAL DONE"
