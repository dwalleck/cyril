#!/bin/bash
# usage: k8wc-setup.sh <arm>  — fresh private HOME + workspace with the V3 allow-rule agent
set -euo pipefail
L=/home/dwalleck/.claude/tmp/claude-1000/-home-dwalleck-repos-cyril/0473cb2a-6b72-4739-a03f-aec2a7f97d90/scratchpad/cyril-k8wc/live
arm=${1:?arm}
A="$L/$arm"
if [ -e "$A" ]; then echo "arm dir $A exists; pick a new arm name" >&2; exit 1; fi
mkdir -p "$A/home/.config/cyril" "$A/ws/.kiro/agents"
cd "$A/ws"
git init -q
echo "# k8wc probe workspace" > README.md
git add README.md
git -c user.name=probe -c user.email=probe@example.invalid commit -q -m init
cat > .kiro/agents/auditx.json <<'EOF'
{
  "name": "auditx",
  "description": "cyril-k8wc shell_type probe agent",
  "prompt": "You are a terse probe agent. Run shell commands exactly as asked.",
  "tools": ["shell", "read"],
  "permissions": {
    "rules": [
      { "capability": "shell", "match": ["echo AUDITX*"], "effect": "allow" }
    ]
  }
}
EOF
echo "setup $A"
