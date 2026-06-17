#!/bin/sh
# Bypass the kiro-cli router; call kiro-cli-chat 2.7.1 directly through sacp-conductor.
exec "$HOME/.local/cargo-spike/bin/sacp-conductor" \
    --debug \
    --debug-dir /tmp/conductor-spike/logs-271 \
    agent "$HOME/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat acp"
