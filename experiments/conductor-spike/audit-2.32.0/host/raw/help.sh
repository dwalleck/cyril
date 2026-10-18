#!/bin/bash
# --help sweep of archived binaries, temp HOME + temp XDG_DATA_HOME, archive bin/ first on PATH (trap 8)
set -u
OUT=/home/dwalleck/repos/cyril/experiments/conductor-spike/audit-2.32.0/host/raw/help
for v in 2.31.1 2.32.0; do
  BIN=$HOME/.local/share/kiro-research/binaries/$v/extracted/kirocli/bin
  T=$(mktemp -d /tmp/kh-help-XXXX)
  run() { local name=$1; shift; env -i HOME=$T XDG_DATA_HOME=$T/xdg XDG_CONFIG_HOME=$T/cfg XDG_RUNTIME_DIR=$T/run TERM=dumb NO_COLOR=1 KIRO_NO_AUTO_UPDATE=1 PATH=$BIN:/usr/bin:/bin timeout 20 "$@" > $OUT/$name-$v.txt 2>&1; echo "$v $name rc=$?"; }
  mkdir -p $T/xdg $T/cfg $T/run
  run launcher-help-all $BIN/kiro-cli --help-all
  run launcher-acp $BIN/kiro-cli acp --help
  run launcher-chat $BIN/kiro-cli chat --help
  run launcher-agent $BIN/kiro-cli agent --help
  run launcher-login $BIN/kiro-cli login --help
  run launcher-user $BIN/kiro-cli user --help
  run launcher-settings $BIN/kiro-cli settings --help
  run chat-help $BIN/kiro-cli-chat --help
  run chat-acp $BIN/kiro-cli-chat acp --help
  run chat-chat $BIN/kiro-cli-chat chat --help
  run chat-agent $BIN/kiro-cli-chat agent --help
  run chat-agent-upgrade $BIN/kiro-cli-chat agent upgrade --help
  run chat-underscore $BIN/kiro-cli-chat _ --help
  run chat-login $BIN/kiro-cli-chat login --help
  run chat-user $BIN/kiro-cli-chat user --help
  run chat-mcp $BIN/kiro-cli-chat mcp --help
  run chat-settings $BIN/kiro-cli-chat settings --help
  run term-help-all $BIN/kiro-cli-term --help-all
  rm -rf "$T"
done
