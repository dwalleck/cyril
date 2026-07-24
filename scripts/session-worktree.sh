#!/usr/bin/env bash
# session-worktree.sh — provision a per-session git worktree (cyril-4rc1).
#
# Parallel Claude sessions must NOT share one checkout: commits land on whatever
# branch happens to be checked out, and relocation rebases have dropped commits
# (see cyril-4rc1). Each session runs in its own linked worktree instead.
#
#   dest=$(scripts/session-worktree.sh feat/cyril-xyz)   # create/reuse
#   cd "$dest"                                           # work there
#
# The worktree is placed as a SIBLING of the primary checkout
# (../cyril-wt-<branch>, slashes → dashes) so it never nests, even when this
# script is run from inside another worktree. Idempotent: a second call for the
# same branch prints the existing path and exits 0. An already-checked-out
# branch is refused by git (a branch can live in at most one worktree).
set -euo pipefail

usage() {
	echo "usage: session-worktree.sh <branch> [base-ref]" >&2
	exit 2
}

[ $# -ge 1 ] || usage
branch="$1"
base="${2:-HEAD}"

# Anchor the destination to the PRIMARY checkout, not the current worktree:
# --git-common-dir resolves to <primary>/.git from any linked worktree, so the
# sibling placement is stable regardless of where this runs.
common=$(cd "$(git rev-parse --git-common-dir)" && pwd)
primary=$(dirname "$common")
dest="$(dirname "$primary")/cyril-wt-${branch//\//-}"

# Idempotent: if a worktree already holds this branch, report it and stop.
# Assign on its own line (not inside the `if`) so a failed `git worktree list`
# aborts under `set -e`/`pipefail` rather than being masked as "no existing
# worktree" and silently creating a duplicate.
existing=$(git worktree list --porcelain | awk -v b="refs/heads/$branch" '
	$1 == "worktree" { p = $2 }
	$1 == "branch" && $2 == b { print p }
')
if [ -n "$existing" ]; then
	echo "$existing"
	exit 0
fi

# New branch → create it from base; existing local branch → check it out.
if git show-ref --verify --quiet "refs/heads/$branch"; then
	git worktree add -q "$dest" "$branch"
else
	git worktree add -q -b "$branch" "$dest" "$base"
fi
echo "$dest"
