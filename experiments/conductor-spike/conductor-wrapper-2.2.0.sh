#!/bin/sh
# Wrapper: cyril's bridge always appends "acp" arg.
# We swallow it and run the conductor with kiro-cli as its sole component (no proxies).
exec "$HOME/.local/cargo-spike/bin/sacp-conductor" \
    --debug \
    --debug-dir /tmp/conductor-spike/logs \
    agent "kiro-cli acp"
