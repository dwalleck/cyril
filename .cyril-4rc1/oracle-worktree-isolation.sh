#!/usr/bin/env bash
# cyril-4rc1 ORACLE: recompute the probe's verdicts from the repo it left
# behind, using a DIFFERENT mechanism — full-history commit-subject membership
# per branch (git log --format=%s + git branch --contains), not the probe's
# tip-only `git log -1` + `git for-each-ref --contains`.
#
# Usage: oracle-worktree-isolation.sh <ROOT>   (ROOT printed by the probe)
set -u
ROOT="${1:?pass the ROOT the probe printed}"
repo="$ROOT/repo"
cd "$repo" || { echo "no repo at $repo"; exit 1; }

subjects() { git log --format='%s' "$1"; }          # full history, not just tip
has()      { subjects "$1" | grep -qxF "$2"; }       # exact-line subject membership

pass=0; fail=0
check() { # name expected actual
  if [ "$2" = "$3" ]; then echo "PASS $1 ($3)"; pass=$((pass+1))
  else echo "FAIL $1: want $2 got $3"; fail=$((fail+1)); fi
}

# BUG: the hitchhiker "A-work (meant for feat-a)" is on feat-b, NOT feat-a.
has feat-b "A-work (meant for feat-a)" && b=yes || b=no
has feat-a "A-work (meant for feat-a)" && a=yes || a=no
check "bug: hitchhiker on feat-b"        yes "$b"
check "bug: NOT on intended feat-a"      no  "$a"

# FIX: each isolated commit is on its own branch, and NOT on the other.
has feat-a "A-work isolated" && aa=yes || aa=no
has feat-b "B-work isolated" && bb=yes || bb=no
has feat-a "B-work isolated" && ax=yes || ax=no
has feat-b "A-work isolated" && bx=yes || bx=no
check "fix: A-work on feat-a"            yes "$aa"
check "fix: B-work on feat-b"            yes "$bb"
check "fix: A-work NOT on feat-b"        no  "$bx"
check "fix: B-work NOT on feat-a"        no  "$ax"

# Independent cross-check via `git branch --contains` (plumbing, not %s).
awtip=$(git rev-parse feat-a); btgt=$(git branch --contains "$awtip" --format='%(refname:short)' | tr '\n' ' ')
check "fix: feat-a tip lives only where expected" "feat-a " "$btgt"

# STRUCTURAL GUARD: 3 worktrees exist; a 4th on an in-use branch is refused.
wc_count=$(git worktree list | grep -c .)
check "worktrees present" 3 "$wc_count"
if git worktree add -q "$ROOT/wt-dup2" feat-b 2>/dev/null; then dup=allowed; else dup=refused; fi
check "dup-branch worktree refused" refused "$dup"

echo
echo "oracle: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
