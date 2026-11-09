#!/bin/bash
# First-version attribution: acp/chat/_/chat _ --help on every intermediate archived host binary (same isolation as help.sh).
set -u
OUT=/home/dwalleck/repos/cyril/experiments/conductor-spike/audit-3.1.0/host/raw/help
TMPROOT=/home/dwalleck/repos/cyril/experiments/conductor-spike/audit-3.1.0/host/tmp/help-home
LOG=$OUT/help-chain.log; : > "$LOG"
for v in 2.33.0 2.33.1 2.34.0 2.34.1 2.34.2 2.35.0 2.35.1 2.35.2 2.36.0 2.37.0; do
  BIN=$HOME/.local/share/kiro-research/binaries/$v/extracted/kirocli/bin
  T=$(mktemp -d "$TMPROOT/$v-XXXX"); mkdir -p "$T/xdg" "$T/cfg" "$T/run"
  run() { local name=$1; shift
    env -i HOME="$T" XDG_DATA_HOME="$T/xdg" XDG_CONFIG_HOME="$T/cfg" XDG_RUNTIME_DIR="$T/run" TERM=dumb NO_COLOR=1 KIRO_NO_AUTO_UPDATE=1 KIRO_DISABLE_TELEMETRY=1 PATH="$BIN:/usr/bin:/bin" timeout 20 "$@" > "$OUT/$name-$v.txt" 2>&1; local rc=$?
    echo "$v $name rc=$? argv=[$*]" >> "$LOG"; }
  run launcher-help-all      "$BIN/kiro-cli" --help-all
  run chat-help              "$BIN/kiro-cli-chat" --help
  run chat-acp               "$BIN/kiro-cli-chat" acp --help
  run chat-chat              "$BIN/kiro-cli-chat" chat --help
  run chat-underscore        "$BIN/kiro-cli-chat" _ --help
  run chat-chat-underscore   "$BIN/kiro-cli-chat" chat _ --help
  run chat-chat-gkt          "$BIN/kiro-cli-chat" chat _ get-kas-token --help
  run chat-agent-upgrade     "$BIN/kiro-cli-chat" agent upgrade --help
  rm -rf "$T"
done
echo CHAIN-DONE >> "$LOG"
