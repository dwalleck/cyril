#!/usr/bin/env bash
# Regression fence for the cyril-4rc1 worktree tooling: one `test_*` function
# per design claim (H1-H6 helper, G1-G4 guard, D1 docs). The runner
# auto-discovers every `test_*` function, so each slice just adds functions.
#
# Oracles are git plumbing (`git worktree list`, `git rev-parse`, commit exit
# codes) and `grep` — independent of the artifacts' own stdout. Run:
#   bash scripts/tests/worktree_test.sh
# Non-zero exit ⇔ at least one claim regressed.
set -uo pipefail

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
HELPER="$REPO_ROOT/scripts/session-worktree.sh"
HOOK="$REPO_ROOT/.githooks/pre-commit"

PASS=0
FAIL=0
CURRENT=""

ck() { # label expected actual
	if [ "$2" = "$3" ]; then
		echo "  PASS $CURRENT/$1 ($3)"
		PASS=$((PASS + 1))
	else
		echo "  FAIL $CURRENT/$1: want[$2] got[$3]" >&2
		FAIL=$((FAIL + 1))
	fi
}

# A throwaway git repo with one commit on main. Echoes its path.
new_repo() {
	local d
	d=$(mktemp -d)
	git init -q -b main "$d"
	git -C "$d" -c user.name=t -c user.email=t@x commit -q --allow-empty -m base
	echo "$d"
}

# ---- Slice 1: helper (H1-H6) ----

test_h1_create_new_branch() {
	local r
	r=$(new_repo)
	local dest
	dest=$(cd "$r" && bash "$HELPER" feat/cyril-x)
	ck head "feat/cyril-x" "$(git -C "$dest" branch --show-current)"
	ck exists yes "$([ -d "$dest" ] && echo yes || echo no)"
	rm -rf "$r" "$dest"
}

test_h2_existing_branch_no_dup() {
	local r
	r=$(new_repo)
	git -C "$r" branch feat-existing
	local before
	before=$(git -C "$r" branch --list | grep -c .)
	local dest
	dest=$(cd "$r" && bash "$HELPER" feat-existing)
	ck branch-count "$before" "$(git -C "$r" branch --list | grep -c .)"
	ck head feat-existing "$(git -C "$dest" branch --show-current)"
	rm -rf "$r" "$dest"
}

test_h3_inuse_branch_refused() {
	local r
	r=$(new_repo)
	local dest
	dest=$(cd "$r" && bash "$HELPER" feat-dup)
	local before
	before=$(git -C "$r" worktree list | grep -c .)
	# A raw second checkout of the in-use branch must be refused by git; the
	# helper's own idempotency would short-circuit, so probe git directly.
	local rc=0
	git -C "$r" worktree add -q "$r-dup2" feat-dup 2>/dev/null || rc=$?
	ck refused yes "$([ "$rc" -ne 0 ] && echo yes || echo no)"
	ck no-stray-wt "$before" "$(git -C "$r" worktree list | grep -c .)"
	rm -rf "$r" "$dest" "$r-dup2"
}

test_h4_slash_sanitized() {
	local r
	r=$(new_repo)
	local dest
	dest=$(cd "$r" && bash "$HELPER" feat/cyril-4rc1)
	ck leaf-no-slash yes "$(case "$(basename "$dest")" in */*) echo no ;; *) echo yes ;; esac)"
	ck leaf-name cyril-wt-feat-cyril-4rc1 "$(basename "$dest")"
	ck branch-preserved feat/cyril-4rc1 "$(git -C "$dest" branch --show-current)"
	rm -rf "$r" "$dest"
}

test_h5_idempotent_rerun() {
	local r
	r=$(new_repo)
	local d1 d2 before after
	d1=$(cd "$r" && bash "$HELPER" feat-idem)
	before=$(git -C "$r" worktree list | grep -c .)
	d2=$(cd "$r" && bash "$HELPER" feat-idem)
	after=$(git -C "$r" worktree list | grep -c .)
	ck same-path "$d1" "$d2"
	ck no-new-wt "$before" "$after"
	rm -rf "$r" "$d1"
}

# Stress fixture: run the helper from INSIDE a linked worktree — dest must still
# be a sibling of the PRIMARY, never nested under the current worktree. Fails an
# implementation that anchors dest on the current directory (a `$PWD`/relative
# path) rather than the primary checkout resolved via `--git-common-dir`.
test_h5b_anchored_from_worktree() {
	local r
	r=$(new_repo)
	local wt
	wt=$(cd "$r" && bash "$HELPER" feat-first)
	# Run from a SUBDIRECTORY of the worktree: a cwd-based anchor
	# (`dirname $(pwd)`) would resolve to the worktree root and NEST the new
	# worktree inside it; the primary anchor (--git-common-dir) still yields a
	# sibling of the primary regardless of depth.
	mkdir -p "$wt/deep/sub"
	local dest
	dest=$(cd "$wt/deep/sub" && bash "$HELPER" feat-second)
	local expect
	expect="$(dirname "$r")/cyril-wt-feat-second"
	ck sibling-not-nested "$expect" "$dest"
	rm -rf "$r" "$wt" "$dest"
}

test_h6_missing_arg_usage() {
	local rc=0 err
	err=$(bash "$HELPER" 2>&1 >/dev/null) || rc=$?
	ck exit2 2 "$rc"
	ck usage-msg yes "$(echo "$err" | grep -qi usage && echo yes || echo no)"
}

# ---- runner ----

main() {
	local fns
	fns=$(declare -F | awk '{print $3}' | grep '^test_' | sort)
	for fn in $fns; do
		CURRENT="$fn"
		echo "== $fn =="
		"$fn"
	done
	echo
	echo "worktree_test: $PASS passed, $FAIL failed" >&2
	[ "$FAIL" -eq 0 ]
}

main "$@"
