#!/bin/bash
# Run `cargo fmt` after any Bash invocation that runs `cargo clippy --fix`.
#
# The per-file rustfmt hook (configured in settings.json) fires on Write/Edit
# tool calls, but `cargo clippy --fix` edits files via Bash — so those edits
# bypass the per-file formatter. This hook closes that gap.
set -euo pipefail

INPUT=$(cat)
COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty')

# Skip unless this looks like a clippy autofix invocation.
if [[ "$COMMAND" != *"cargo clippy"* ]] || [[ "$COMMAND" != *"--fix"* ]]; then
  exit 0
fi

cd "$CLAUDE_PROJECT_DIR"
# Let cargo fmt's stderr surface — a format failure usually means broken
# code from clippy's rewrite, which the user wants to see, not swallow.
# Don't fail the hook itself on a fmt error (PostToolUse hooks shouldn't
# gate the tool call); the stderr above makes the issue visible.
cargo fmt || true
exit 0
