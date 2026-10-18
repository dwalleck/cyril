#!/bin/bash
# cyril-zbu7 live acceptance: the fix build of cyril on the KAS Free path.
L=/home/dwalleck/.claude/tmp/claude-1000/-home-dwalleck-repos-cyril/0473cb2a-6b72-4739-a03f-aec2a7f97d90/scratchpad/cyril-zbu7/live
export HOME=$L/home XDG_DATA_HOME=/home/dwalleck/.local/share XDG_CONFIG_HOME=$L/home/.config TERM=xterm-256color
cd "$L/ws" || exit 9
"$L/zbu7-cyril-bin" --agent-engine kas 2> "$L/cyril.stderr"
echo "CYRIL-EXITED code=$?"
sleep 3600
