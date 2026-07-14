#!/usr/bin/env bash
# Independent lexical inventory for comparison with the AST-based probe.
set -euo pipefail

files=(
  crates/cyril-ui/src/widgets/chat.rs
  crates/cyril-ui/src/widgets/markdown.rs
  crates/cyril-ui/src/widgets/input.rs
  crates/cyril-ui/src/widgets/suggestions.rs
  crates/cyril-ui/src/highlight.rs
)
pattern='Color::[A-Za-z0-9_]+|palette::(USER_BLUE|AGENT_GREEN|SYSTEM_MAUVE|MUTED_GRAY|CODE_BLOCK_BG)'

for file in "${files[@]}"; do
  awk '/#\[cfg\(test\)\]/{exit} {print}' "$file" |
    { grep -nEo "$pattern" || true; } |
    while IFS=: read -r line token; do
      printf '%s\t%s\t%s\n' "$file" "$line" "$token"
    done
done | sort -t$'\t' -k1,1 -k2,2n -k3,3 -u
