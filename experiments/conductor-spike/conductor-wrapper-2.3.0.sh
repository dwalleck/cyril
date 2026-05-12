#!/bin/sh
# Bypass the kiro-cli router; call kiro-cli-chat 2.3.0 directly.
# Binary lives in the external research archive (see CLAUDE.md "Research archive").
exec /home/dwalleck/.local/cargo-spike/bin/sacp-conductor \
    --debug \
    --debug-dir /tmp/conductor-spike/logs-230 \
    agent "$HOME/.local/share/kiro-research/binaries/2.3.0/kiro-cli-chat acp"
