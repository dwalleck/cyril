#!/usr/bin/env bash
# cyril-4rc1 PROBE: does a git worktree structurally prevent one session's
# commit from landing on another session's branch?
#
# Two scenarios in a throwaway repo, each simulating two concurrent "sessions":
#   SHARED   — one checkout, both sessions commit in it (the current regime).
#              Session B checks out its branch, session A commits without
#              looking -> A's commit hitchhikes onto B's branch.
#   WORKTREE — one worktree per session (the fix). Each has its own HEAD;
#              cross-contamination is structurally impossible, AND git refuses
#              to check out a branch already checked out elsewhere.
#
# The probe PRINTS raw git facts (branch tips, worktree list, the refusal exit
# code). The oracle (oracle-worktree-isolation.sh) recomputes the verdicts from
# `git log`/`git worktree list` independently and must agree.
set -u
ROOT=$(mktemp -d)
export GIT_AUTHOR_NAME=probe GIT_AUTHOR_EMAIL=p@x GIT_COMMITTER_NAME=probe GIT_COMMITTER_EMAIL=p@x
cd "$ROOT" || exit 1

repo="$ROOT/repo"
git init -q -b main "$repo"; cd "$repo" || exit 1
git commit -q --allow-empty -m "base"          # a root commit on main
git branch feat-a
git branch feat-b

echo "### SHARED-CHECKOUT REGIME (the bug) ###"
# Session B takes the shared checkout onto feat-b.
git switch -q feat-b
# Session A, unaware, commits "its" work — lands on feat-b (B's branch).
git commit -q --allow-empty -m "A-work (meant for feat-a)"
echo "feat-a tip:  $(git log --oneline -1 feat-a)"
echo "feat-b tip:  $(git log --oneline -1 feat-b)"
echo "A-work landed on: $(git for-each-ref --format='%(refname:short)' --contains HEAD refs/heads | grep -vx main | tr '\n' ' ')"

echo
echo "### WORKTREE-PER-SESSION REGIME (the fix) ###"
git switch -q main
wa="$ROOT/wt-a"; wb="$ROOT/wt-b"
git worktree add -q "$wa" feat-a
git worktree add -q "$wb" feat-b
# Two sessions commit concurrently, each in its own worktree.
( cd "$wa" && git commit -q --allow-empty -m "A-work isolated" )
( cd "$wb" && git commit -q --allow-empty -m "B-work isolated" )
echo "feat-a tip:  $(git log --oneline -1 feat-a)"
echo "feat-b tip:  $(git log --oneline -1 feat-b)"
echo "worktrees:"
git worktree list | sed 's/^/  /'

echo
echo "### STRUCTURAL GUARD: can a 2nd worktree grab an already-checked-out branch? ###"
if git worktree add -q "$ROOT/wt-dup" feat-a 2>"$ROOT/dup.err"; then
  echo "dup-checkout: ALLOWED (unexpected)"
else
  echo "dup-checkout: REFUSED (exit $?) -> $(tr -d '\n' <"$ROOT/dup.err" | tail -c 120)"
fi

echo
echo "REPO=$repo"
echo "ROOT=$ROOT"
