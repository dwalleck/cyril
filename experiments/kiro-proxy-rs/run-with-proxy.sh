#!/bin/sh
# Run Kiro's v2 TUI with the Rust logging proxy in the middle.
#
# Why this script exists: `kiro-cli chat --tui` goes through chat_cli_v2,
# which unconditionally overwrites KIRO_AGENT_PATH before spawning bun.
# To insert a proxy you must bypass that wrapper and invoke bun directly.
# See `reference_kiro_backend_interception.md` in session memory for
# the full reasoning.
#
# Usage:
#     ./run-with-proxy.sh                         # debug build, default log path
#     KIRO_PROXY_LOG=/tmp/custom.jsonl ./run-with-proxy.sh
#     PROFILE=release ./run-with-proxy.sh         # use release binary
#
# After quitting the TUI, inspect the captured traffic at:
#     $KIRO_PROXY_LOG  (default: /tmp/kiro-proxy-poc/messages-rs.jsonl)

set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROFILE="${PROFILE:-debug}"
PROXY_BIN="${SCRIPT_DIR}/target/${PROFILE}/kiro-proxy-rs"

# Build if missing. Prefers the already-built binary to avoid surprise
# compile delays every invocation.
if [ ! -x "$PROXY_BIN" ]; then
    echo "Building ${PROFILE} proxy (${PROXY_BIN} not found)..." >&2
    if [ "$PROFILE" = "release" ]; then
        (cd "$SCRIPT_DIR" && cargo build --release)
    else
        (cd "$SCRIPT_DIR" && cargo build)
    fi
fi

# Defaults for proxy env; users can override by exporting before running.
: "${KIRO_PROXY_LOG:=/tmp/kiro-proxy-poc/messages-rs.jsonl}"
: "${KIRO_PROXY_REAL_BACKEND:=$HOME/.local/bin/kiro-cli-chat}"

BUN="$HOME/.local/share/kiro-cli/bun"
TUI_JS="$HOME/.local/share/kiro-cli/tui.js"

if [ ! -x "$BUN" ]; then
    echo "error: bun runtime not found at $BUN" >&2
    echo "(has Kiro ever been run on this machine? bun is extracted on first 'kiro-cli chat --tui')" >&2
    exit 1
fi

if [ ! -f "$TUI_JS" ]; then
    echo "error: tui.js not found at $TUI_JS" >&2
    exit 1
fi

if [ ! -x "$KIRO_PROXY_REAL_BACKEND" ]; then
    echo "error: real kiro-cli-chat not found at $KIRO_PROXY_REAL_BACKEND" >&2
    exit 1
fi

mkdir -p "$(dirname "$KIRO_PROXY_LOG")"

echo "kiro-proxy-rs: starting v2 TUI via proxy" >&2
echo "  proxy:   $PROXY_BIN" >&2
echo "  backend: $KIRO_PROXY_REAL_BACKEND" >&2
echo "  log:     $KIRO_PROXY_LOG" >&2
echo "" >&2

export KIRO_AGENT_PATH="$PROXY_BIN"
export KIRO_PROXY_LOG
export KIRO_PROXY_REAL_BACKEND

exec "$BUN" "$TUI_JS" chat --tui "$@"
