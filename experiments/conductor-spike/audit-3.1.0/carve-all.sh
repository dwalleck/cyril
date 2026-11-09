#!/usr/bin/env bash
# carve-all.sh <ver>... — carve KAS tar, TUI bundle, ELF frames from each release's chat binary (kiro-cli-chat, or the merged kiro-cli in 3.x).
set -uo pipefail
cd /home/dwalleck/repos/cyril
ARCHIVE=$HOME/.local/share/kiro-research
AUD=experiments/conductor-spike/audit-3.1.0
for v in "$@"; do
  bin="$ARCHIVE/binaries/$v/extracted/kirocli/bin/kiro-cli-chat"
  [ -f "$bin" ] || bin="$ARCHIVE/binaries/$v/extracted/kirocli/bin/kiro-cli"
  echo "=== $v $(date -u +%FT%TZ) binary=$bin"
  if [ -d "$ARCHIVE/kas-carves/$v" ] && [ -n "$(ls -A "$ARCHIVE/kas-carves/$v")" ]; then kasarg=(); echo "kas-carves/$v exists, not re-extracting"; else kasarg=(--kas-dir "$ARCHIVE/kas-carves/$v"); fi
  python3 -I .claude/skills/kiro-release-audit/scripts/carve-frames.py "$bin" --frames-out "$AUD/frames-$v.json" "${kasarg[@]}" --tui-out "$ARCHIVE/tui-bundles/kiro-tui-$v.js" --elf-dir "$ARCHIVE/binaries/$v/elf" > "$AUD/carve-$v.log" 2>&1
  rc=$?; cat "$AUD/carve-$v.log"; echo "=== $v carve rc=$rc"
done
echo "CARVE-ALL-DONE"
