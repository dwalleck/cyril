#!/bin/sh
# Bypass the kiro-cli router; call kiro-cli-chat 2.1.0 directly.
exec /home/dwalleck/.local/cargo-spike/bin/sacp-conductor \
    --debug \
    --debug-dir /tmp/conductor-spike/logs-210 \
    agent "/home/dwalleck/repos/cyril/docs/kiro-binaries-2.1.0/kiro-cli-chat acp"
