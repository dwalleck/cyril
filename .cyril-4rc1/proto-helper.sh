#!/usr/bin/env bash
# cyril-4rc1 DESIGN PROTOTYPE of session-worktree.sh — throwaway, lives in the
# probe dir. The real helper (scripts/) is the build. Just enough to run the
# cheapest falsifiers (H1 create, H4 slash-name, H5 idempotency, H6 usage, H3
# in-use refusal) before the design is approved.
set -euo pipefail

usage() { echo "usage: session-worktree.sh <branch> [base-ref]" >&2; exit 2; }
[ $# -ge 1 ] || usage
branch="$1"; base="${2:-HEAD}"

# Sibling dir outside the primary checkout; slashes in the branch → dashes in
# the leaf so feat/cyril-x doesn't create nested dirs.
repo_root=$(git rev-parse --show-toplevel)
leaf="cyril-wt-${branch//\//-}"
dest="$(dirname "$repo_root")/$leaf"

# Idempotent: if a worktree for this branch already exists, print it and stop.
if existing=$(git worktree list --porcelain | awk -v b="refs/heads/$branch" '
    $1=="worktree"{p=$2} $1=="branch"&&$2==b{print p}'); [ -n "$existing" ]; then
  echo "$existing"; exit 0
fi

# New branch vs existing local branch: -b only when it doesn't exist yet.
if git show-ref --verify --quiet "refs/heads/$branch"; then
  git worktree add -q "$dest" "$branch"
else
  git worktree add -q -b "$branch" "$dest" "$base"
fi
echo "$dest"
