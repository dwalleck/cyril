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

rustfmt "$FILE_PATH" 2>/dev/null || true
exit 0
