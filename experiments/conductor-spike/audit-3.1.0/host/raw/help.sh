#!/bin/bash
# --help sweep of ARCHIVED binaries (never the installed one). Fresh temp HOME + XDG_DATA_HOME + XDG_CONFIG_HOME under host/tmp
# (trap 6/7: the real store holds a single-use refresh token). Archive bin/ first on PATH (trap 8). Every argv is recorded in help.log.
set -u
OUT=/home/dwalleck/repos/cyril/experiments/conductor-spike/audit-3.1.0/host/raw/help
TMPROOT=/home/dwalleck/repos/cyril/experiments/conductor-spike/audit-3.1.0/host/tmp/help-home
mkdir -p "$OUT" "$TMPROOT"
LOG=$OUT/help.log; : > "$LOG"
for v in 2.32.0 2.37.1 3.0.0 3.1.0; do
  BIN=$HOME/.local/share/kiro-research/binaries/$v/extracted/kirocli/bin
  T=$(mktemp -d "$TMPROOT/$v-XXXX"); mkdir -p "$T/xdg" "$T/cfg" "$T/run"
  run() { local name=$1; shift
    local before=$(find "$T" -type f | wc -l)
    env -i HOME="$T" XDG_DATA_HOME="$T/xdg" XDG_CONFIG_HOME="$T/cfg" XDG_RUNTIME_DIR="$T/run" TERM=dumb NO_COLOR=1 KIRO_NO_AUTO_UPDATE=1 KIRO_DISABLE_TELEMETRY=1 PATH="$BIN:/usr/bin:/bin" timeout 20 "$@" > "$OUT/$name-$v.txt" 2>&1; local rc=$?
    local after=$(find "$T" -type f | wc -l)
    echo "$v $name rc=$rc files_in_tmp_home=$before->$after argv=[$*]" | tee -a "$LOG"; }
  if [ -f "$BIN/kiro-cli-chat" ]; then
    run launcher-help          "$BIN/kiro-cli" --help
    run launcher-help-all      "$BIN/kiro-cli" --help-all
    run launcher-acp           "$BIN/kiro-cli" acp --help
    run launcher-chat          "$BIN/kiro-cli" chat --help
    run launcher-integrations  "$BIN/kiro-cli" integrations --help
    run launcher-update        "$BIN/kiro-cli" update --help
    run launcher-internal      "$BIN/kiro-cli" internal --help
    run launcher-underscore    "$BIN/kiro-cli" _ --help
    run launcher-version       "$BIN/kiro-cli" version --help
    run launcher-settings      "$BIN/kiro-cli" settings --help
    run launcher-login         "$BIN/kiro-cli" login --help
    run launcher-user          "$BIN/kiro-cli" user --help
    run launcher-agent         "$BIN/kiro-cli" agent --help
    run launcher-mcp           "$BIN/kiro-cli" mcp --help
    run chat-help              "$BIN/kiro-cli-chat" --help
    run chat-help-all          "$BIN/kiro-cli-chat" --help-all
    run chat-acp               "$BIN/kiro-cli-chat" acp --help
    run chat-acp-engine-v2     "$BIN/kiro-cli-chat" acp --agent-engine v2 --help
    run chat-acp-engine-kas    "$BIN/kiro-cli-chat" acp --agent-engine kas --help
    run chat-acp-engine-v3     "$BIN/kiro-cli-chat" acp --agent-engine v3 --help
    run chat-acp-engine-classic "$BIN/kiro-cli-chat" acp --agent-engine classic --help
    run chat-chat              "$BIN/kiro-cli-chat" chat --help
    run chat-chat-underscore   "$BIN/kiro-cli-chat" chat _ --help
    run chat-chat-gkt          "$BIN/kiro-cli-chat" chat _ get-kas-token --help
    run chat-underscore        "$BIN/kiro-cli-chat" _ --help
    run chat-underscore-cleanup "$BIN/kiro-cli-chat" _ cleanup-autocomplete-artifacts --help
    run chat-agent             "$BIN/kiro-cli-chat" agent --help
    run chat-agent-upgrade     "$BIN/kiro-cli-chat" agent upgrade --help
    run chat-mcp               "$BIN/kiro-cli-chat" mcp --help
    run chat-settings          "$BIN/kiro-cli-chat" settings --help
    run chat-login             "$BIN/kiro-cli-chat" login --help
    run chat-user              "$BIN/kiro-cli-chat" user --help
    run chat-version           "$BIN/kiro-cli-chat" version --help
    run chat-integrations      "$BIN/kiro-cli-chat" integrations --help
    run chat-update            "$BIN/kiro-cli-chat" update --help
    run chat-internal          "$BIN/kiro-cli-chat" internal --help
    run term-help-all          "$BIN/kiro-cli-term" --help-all
  else
    run merged-help            "$BIN/kiro-cli" --help
    run merged-help-all        "$BIN/kiro-cli" --help-all
    run merged-acp             "$BIN/kiro-cli" acp --help
    run merged-acp-engine-v2   "$BIN/kiro-cli" acp --agent-engine v2 --help
    run merged-acp-engine-kas  "$BIN/kiro-cli" acp --agent-engine kas --help
    run merged-acp-engine-v3   "$BIN/kiro-cli" acp --agent-engine v3 --help
    run merged-acp-engine-classic "$BIN/kiro-cli" acp --agent-engine classic --help
    run merged-chat            "$BIN/kiro-cli" chat --help
    run merged-chat-underscore "$BIN/kiro-cli" chat _ --help
    run merged-chat-gkt        "$BIN/kiro-cli" chat _ get-kas-token --help
    run merged-chat-v3         "$BIN/kiro-cli" chat --v3 --help
    run merged-chat-engine-v2  "$BIN/kiro-cli" chat --agent-engine v2 --help
    run merged-underscore      "$BIN/kiro-cli" _ --help
    run merged-underscore-cleanup "$BIN/kiro-cli" _ cleanup-autocomplete-artifacts --help
    run merged-underscore-cleanup-dry "$BIN/kiro-cli" _ cleanup-autocomplete-artifacts --dry-run
    run merged-agent           "$BIN/kiro-cli" agent --help
    run merged-agent-upgrade   "$BIN/kiro-cli" agent upgrade --help
    run merged-mcp             "$BIN/kiro-cli" mcp --help
    run merged-settings        "$BIN/kiro-cli" settings --help
    run merged-login           "$BIN/kiro-cli" login --help
    run merged-user            "$BIN/kiro-cli" user --help
    run merged-version         "$BIN/kiro-cli" version --help
    run merged-integrations    "$BIN/kiro-cli" integrations --help
    run merged-update          "$BIN/kiro-cli" update --help
    run merged-internal        "$BIN/kiro-cli" internal --help
    run merged-setup           "$BIN/kiro-cli" setup --help
    run merged-doctor          "$BIN/kiro-cli" doctor --help
    run merged-uninstall       "$BIN/kiro-cli" uninstall --help
    run merged-issue           "$BIN/kiro-cli" issue --help
    run merged-diagnostic      "$BIN/kiro-cli" diagnostic --help
    run merged-completion      "$BIN/kiro-cli" completion --help
    run merged-theme           "$BIN/kiro-cli" theme --help
    run merged-telemetry       "$BIN/kiro-cli" telemetry --help
    run merged-translate       "$BIN/kiro-cli" translate --help
    run merged-inline          "$BIN/kiro-cli" inline --help
    run merged-hook            "$BIN/kiro-cli" hook --help
    run merged-launch          "$BIN/kiro-cli" launch --help
    run merged-dashboard       "$BIN/kiro-cli" dashboard --help
    run merged-debug           "$BIN/kiro-cli" debug --help
    run merged-init            "$BIN/kiro-cli" init --help
    run merged-logout          "$BIN/kiro-cli" logout --help
    run merged-whoami          "$BIN/kiro-cli" whoami --help
    run merged-profile         "$BIN/kiro-cli" profile --help
    run merged-serve           "$BIN/kiro-cli" serve --help
    run merged-workflow        "$BIN/kiro-cli" workflow --help
    run merged-sandbox         "$BIN/kiro-cli" sandbox --help
  fi
  echo "--- $v tmp-home contents after sweep:" | tee -a "$LOG"; find "$T" -type f | sed "s|$T|<T>|" | tee -a "$LOG"
  rm -rf "$T"
done
echo HELP-DONE | tee -a "$LOG"
