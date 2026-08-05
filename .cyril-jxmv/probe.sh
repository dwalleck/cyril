#!/usr/bin/env bash
# cyril-jxmv probe driver. Run from the repo root:
#   bash .cyril-jxmv/probe.sh
# Compiles probe.rs against the REAL cyril-core rlib, runs it, runs the
# independent oracle (documented-contract Python), diffs the OUT|/IN| lines,
# then prints the static wiring evidence: the translation chain's complete
# ambient input surface (no agent-location input exists).
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build -p cyril-core --quiet

RLIB=$(ls target/debug/libcyril_core*.rlib | head -1)
rustc --edition 2024 .cyril-jxmv/probe.rs \
  --extern cyril_core="$RLIB" \
  -L dependency=target/debug/deps \
  -o .cyril-jxmv/probe-bin

.cyril-jxmv/probe-bin | tee .cyril-jxmv/probe-out.txt
python3 .cyril-jxmv/oracle.py | tee .cyril-jxmv/oracle-out.txt

echo "── probe vs oracle (OUT/IN lines) ──"
if diff <(grep -E '^(OUT|IN)\|' .cyril-jxmv/probe-out.txt) .cyril-jxmv/oracle-out.txt; then
  echo "AGREE-OK"
else
  echo "DISAGREE"
  exit 1
fi

echo "── wiring: ambient inputs of platform/path.rs (expect exactly the distro pair) ──"
grep -n 'env::var\|current_dir' crates/cyril-core/src/platform/path.rs

echo "── wiring: production callers of to_native/to_agent (grep view) ──"
grep -rn --include='*.rs' '\bto_native(\|\bto_agent(' crates/ \
  | grep -v 'platform/path.rs' | grep -v '^\s*//' | grep -v 'tests/'

echo "── wiring: no agent-location symbol anywhere in path.rs ──"
if grep -n 'AgentCommand\|agent_command\|program\|wsl.exe' crates/cyril-core/src/platform/path.rs; then
  echo "UNEXPECTED: agent-location symbol present"
  exit 1
else
  echo "NONE-OK"
fi
