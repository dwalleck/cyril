#!/bin/bash
# Auto-format Rust files after Claude edits them.
set -euo pipefail

INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')

# Skip if no file path or not a .rs file
if [ -z "$FILE_PATH" ] || [[ "$FILE_PATH" != *.rs ]]; then
  exit 0
fi

# Skip if file doesn't exist (e.g. failed write)
if [ ! -f "$FILE_PATH" ]; then
  exit 0
fi

# Bare `rustfmt` defaults to edition 2015, which disagrees with `cargo fmt`
# (CI runs `cargo fmt --all`, passing each crate's edition from Cargo.toml).
# Edition 2024 changed the formatting of method chains inside macros like
# `assert!`, so a bare invocation rewrites this repo's edition-2024 code into a
# style CI then rejects. Pass the workspace edition so the hook matches CI.
# Derived from [workspace.package] rather than hardcoded (single source of
# truth); falls back to 2024 if it can't be read.
EDITION=$(grep -m1 -E '^[[:space:]]*edition[[:space:]]*=' "${CLAUDE_PROJECT_DIR:-.}/Cargo.toml" 2>/dev/null \
  | sed -E 's/.*"([0-9]+)".*/\1/')
rustfmt --edition "${EDITION:-2024}" "$FILE_PATH" 2>/dev/null || true
exit 0
