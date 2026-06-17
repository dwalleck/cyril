#!/bin/sh
# Bypass the kiro-cli router; call kiro-cli-chat 2.6.1 directly.
# Binary lives in the external research archive (see CLAUDE.md "Research archive").
exec "$HOME/.local/cargo-spike/bin/sacp-conductor" \
    --debug \
    --debug-dir /tmp/conductor-spike/logs-261 \
    agent "$HOME/.local/share/kiro-research/binaries/2.6.1/kiro-cli-chat acp"
