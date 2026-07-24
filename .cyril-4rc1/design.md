# cyril-4rc1 — falsifiable design: worktree-per-session

## Purpose

Stop parallel Claude sessions from clobbering each other in the single shared
checkout `/home/dwalleck/repos/cyril`. Observed 3× (tpfd review P3 + R2-8; two
data-loss rebases: `d67e5b8→e6ba5c6`, transport point `→fa376ec`). The
prove-it probe (`.cyril-4rc1/`) established the fix is structural: one worktree
per session isolates each commit to its own branch (oracle 9/9), and git
refuses to check out an in-use branch twice (exit 128). Docs alone already
failed 3× (`feedback_verify_branch_before_commit` predates all three).

## The core move is ADDITIVE, not subtractive

This change removes no serialization point, guard, ordering, or uniqueness
property — it *adds* a provisioning helper, a documented convention, and an
optional commit guard (which only *forbids* more). Step-2b removed-invariant
sweep does not apply. (The guard adds a constraint; adding constraints cannot
break a "can't happen" that other code relied on.)

## Architecture (recommended shape — the design pause decides trims)

Three parts, "make the right thing easy + the wrong thing hard + fence it":

1. **`scripts/session-worktree.sh <branch> [base]`** — idempotent worktree
   provisioner. Creates a linked worktree at a sibling path
   `../cyril-wt-<sanitized-branch>`, branching from `base` (default `HEAD`) for
   a new branch or checking out an existing one; prints the path. Re-runs are
   no-ops. Bash; a Linux dev tool, not shipped in the product.
2. **Docs convention** — a "Parallel sessions" block in `CLAUDE.md`
   (Development Workflow) + a line in `AGENTS.md`: concurrent sessions MUST work
   in their own worktree via the helper; the primary checkout is for main-line
   chores. Includes the one-time `git config core.hooksPath .githooks` setup.
3. **Guard hook `.githooks/pre-commit`** (wired via `core.hooksPath`, tracked in
   the repo) — refuses a commit in the **primary** checkout when HEAD is not an
   allowlisted main-line branch (`main`), nudging to a worktree. Detects
   primary-vs-linked via `absolute-git-dir == common-dir` (probe-verified).
   Bypassable with `--no-verify` (documented). Feature commits in a linked
   worktree pass untouched.

**Pause decisions (resolved 2026-07-24):**
- **FD-1 = BLOCKING guard.** Part 3 ships and *blocks* (not warns) feature-branch
  commits in the primary checkout. Policy accepted: primary checkout is
  main-line only; feature work goes in a linked worktree. Opt-in via
  `core.hooksPath` (does not retroactively block this build session).
- **FD-2 = ADD CI job.** A minimal ubuntu-only workflow step runs
  `scripts/tests/worktree_test.sh` on every push. All G*/D1 fences are therefore
  deterministic-CI, not `manual`.

## Input shapes

**Helper `session-worktree.sh <branch> [base]`:**
- branch that does not exist → create from base. (H1)
- branch that exists locally, unused → check out in a worktree. (H2)
- branch already held by a worktree → helper is idempotent (returns existing
  path); a raw `git worktree add` on it is refused by git. (H3)
- branch name containing `/` (feat/cyril-4rc1) → dir leaf must not nest. (H4)
- re-run for an existing worktree → idempotent. (H5)
- missing branch arg → usage error. (H6)

**Guard hook (primary-vs-linked × branch × bypass):**
- primary checkout, HEAD=main, plain commit → allow. (G2)
- primary checkout, HEAD=feature, plain commit → block. (G1)
- linked worktree, HEAD=feature, plain commit → allow. (G3)
- any, `--no-verify` → bypass. (G4)

**Docs:** convention present + helper + hook-setup referenced. (D1)

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| H1 | `<new-branch>` creates a linked worktree with HEAD=branch, branched from base | run helper for a fresh branch in a temp repo | `git -C <dest> branch --show-current` + dir exists | 3m | **passed** (proto) | `worktree_test.sh::h1_create` |
| H2 | `<existing-local-branch>` adds a worktree on it, no duplicate branch | pre-create branch, run helper | `git branch --list` count unchanged; worktree on branch | 3m | pending | `worktree_test.sh::h2_existing` |
| H3 | a branch already held by a worktree is not duplicated: the helper is idempotent (returns the existing path, exit 0), and a *raw* `git worktree add` on that branch is refused by git (exit ≠ 0) | run helper twice, then a raw `git worktree add` on the in-use branch | `git worktree list` count unchanged + git exit ≠ 0 on the raw add | 3m | **passed** | `worktree_test.sh::test_h3_inuse_branch_refused` |
| H4 | a `/`-containing branch name → worktree dir leaf has no `/`; branch preserved | run helper for `feat/x` | `basename <dest>` has no slash; branch = `feat/x` | 3m | **passed** (proto) | `worktree_test.sh::h4_slash` |
| H5 | re-run for an existing worktree is idempotent (exit 0, same path, no 2nd wt) | run helper twice | worktree count equal; same path printed | 3m | **passed** (proto) | `worktree_test.sh::h5_idempotent` |
| H6 | missing branch arg → non-zero exit + usage | run helper with no args | exit 2; stderr contains "usage" | 2m | **passed** (proto) | `worktree_test.sh::h6_usage` |
| G1 | in the PRIMARY checkout, a feature-branch commit is blocked | commit on a feature branch in the primary temp repo | commit exit ≠ 0; HEAD unmoved | 5m | pending | `worktree_test.sh::g1_primary_block` |
| G2 | in the PRIMARY checkout, a `main` commit is allowed | commit on main | commit exit 0; HEAD moved | 4m | pending | `worktree_test.sh::g2_primary_main_ok` |
| G3 | in a LINKED worktree, a feature-branch commit is allowed | commit on a feature branch in a linked worktree | commit exit 0; HEAD moved | 5m | pending | `worktree_test.sh::g3_linked_ok` |
| G4 | `--no-verify` bypasses the guard | `git commit --no-verify` on a feature branch in primary | commit exit 0 | 3m | **passed** (probe-hook-fires) | `worktree_test.sh::g4_bypass` |
| D1 | CLAUDE.md + AGENTS.md state the convention + helper + hook-setup line | grep the docs | both files contain the helper path + `core.hooksPath` | 2m | pending | `worktree_test.sh::d1_docs` (grep) |

Cheapest falsifier (H6/H4/H5/H1) run against `proto-helper.sh`: **8/8 pass**.
The cheapest claim (H1) is `passed` — gate satisfied.

## Regression fence

Deterministic `scripts/tests/worktree_test.sh` (plain-bash, self-contained temp
repos) with one function per claim above. No claim here is an empirical
measurement, so each fence = its own deterministic test (rule 2). FD-2 decides
whether a CI job runs it; if declined, D1/G* carry `manual` status and need the
pause's explicit sign-off. The fence embeds the bug class: pre-fix (no guard)
fails `g1_primary_block`; a naive `branch==main`-only guard fails `g3_linked_ok`
(it would block linked-worktree feature commits too).

## Negative space (what this deliberately does NOT do)

1. **Does not auto-provision a worktree.** A session must run the helper; the
   design provides the tool + convention + guard, not automatic session
   startup wiring. (A SessionStart hook that auto-creates worktrees is a
   separate idea — filed as **cyril-d4zp** if we want it; not this issue.)
2. **Does not make the guard un-bypassable.** `--no-verify` escapes it by
   design — it is an advisory net, not a sandbox. A determined/scripted commit
   can still land wrong.
3. **Does not touch the two already-recovered data losses** (`e6ba5c6`,
   `fa376ec`) — history is settled; this is forward prevention only.
4. **Does not enforce cargo `target/` sharing across worktrees.** Each worktree
   builds independently (disk cost); not optimized here.
5. **Does not add a session registry / lock.** Isolation is git-structural
   (one branch, one worktree), not a coordinator.

## Tracker note

FD-1/FD-2 are design forks resolved at the pause, not deferrals. Negative-space
item 1 (auto-provision on session start) is the one genuine deferral — filed as
cyril-d4zp before this line was committed (verified below).
