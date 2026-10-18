#!/bin/bash
# usage: k8wc-run-cyril.sh <arm> <binary> [shell]  — launched by hex1b
L=/home/dwalleck/.claude/tmp/claude-1000/-home-dwalleck-repos-cyril/0473cb2a-6b72-4739-a03f-aec2a7f97d90/scratchpad/cyril-k8wc/live
A="$L/$1"
export HOME="$A/home" XDG_DATA_HOME=/home/dwalleck/.local/share XDG_CONFIG_HOME="$A/home/.config" TERM=xterm-256color SHELL="${3:-/bin/bash}"
unset KIRO_HOME
cd "$A/ws"
"$2" --agent-engine kas 2> "$A/cyril.stderr"
echo "CYRIL-EXITED code=$?"
sleep 900
