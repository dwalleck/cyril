#!/usr/bin/env bash
# One cyril-5nep live run: fresh git workspace with the given hook file, an
# isolated HOME (real XDG_DATA_HOME keeps the KAS bundle + auth store
# reachable), KAS spawned through relay.py, one prompt.
# Usage: run.sh <label> <probe-binary> <hooks.json> <relay-tags-json|-> <prompt>
set -euo pipefail
# Probe binary: copy hook_live_probe_5nep.rs.txt to crates/cyril/examples/hook_live_probe_5nep.rs
# and run `cargo build --features kas --example hook_live_probe_5nep`.
label=$1 probe=$2 hooks=$3 tags=$4 prompt=$5
here=$(cd "$(dirname "$0")" && pwd)
real_home=$HOME
out="$here/runs/$label"
mkdir -p "$out"
work=$(mktemp -d "${TMPDIR:-/tmp}/cyril-5nep-live.XXXXXX")
mkdir -p "$work/home" "$work/ws/.kiro/hooks" "$work/marks"
sed "s#@MARKS@#$work/marks#g" "$hooks" > "$work/ws/.kiro/hooks/live.json"
git -C "$work/ws" init -q
export RELAY_REAL_NODE="$HOME/.local/share/kiro-cli/node"
export RELAY_LOG="$out/relay.jsonl"
: > "$RELAY_LOG"
if [ "$tags" != "-" ]; then export RELAY_TAGS="$tags"; else unset RELAY_TAGS; fi
(
  cd "$work/ws"
  HOME="$work/home" XDG_DATA_HOME="$real_home/.local/share" KIRO_AGENT_PATH="$here/relay.py" \
    timeout 300 "$probe" "$prompt" > "$out/probe.out" 2> "$out/probe.err" || echo "probe exit $?" >> "$out/probe.out"
)
cp "$work/ws/.kiro/hooks/live.json" "$out/hooks.json"
ls -1 "$work/marks" > "$out/marks.txt" || true
ls -1 "$work/ws" > "$out/ws-files.txt" || true
cat "$work/ws/ran.txt" > "$out/ran.txt" 2>/dev/null || echo "(absent)" > "$out/ran.txt"
echo "work=$work"
echo "marks:"; sed 's/^/  /' "$out/marks.txt"
echo "ran.txt: $(cat "$out/ran.txt")"
echo "hooks/list frames: $(wc -l < "$RELAY_LOG")"
