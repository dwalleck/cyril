# cyril-4rc1 — prove-it-prototype findings

**Headline: the fix direction is proven, and git gives a free structural
guard on top of it.** A worktree-per-session regime makes the hitchhiker-commit
failure *impossible*, not just less likely — and git itself refuses to check
out a branch already in use by another worktree, so two sessions cannot even
share a branch. A repo-tracked pre-commit hook is separately enforceable if we
want belt-and-suspenders.

## The smallest question

*Does a git worktree structurally prevent one session's commit from landing on
another session's branch?* — then two follow-ons: is the shared-checkout bug
actually reproducible, and is an interim guard hook enforceable?

## Probe A — worktree isolation (`probe-worktree-isolation.sh`)

Two "sessions" in a throwaway repo, two regimes:

- **SHARED checkout (the bug, reproduced):** session B switches the shared
  checkout to `feat-b`; session A commits without looking → **A's commit landed
  on `feat-b`** (`feat-a` stayed at base). This is exactly the tpfd hitchhiker.
- **WORKTREE per session (the fix):** `git worktree add` for each; both sessions
  commit concurrently → `feat-a` = "A-work isolated", `feat-b` = "B-work
  isolated". **Zero cross-contamination.**
- **Free structural guard:** a second `git worktree add feat-a` while `feat-a`
  is already checked out → **REFUSED, exit 128** (`'feat-a' is already used by
  worktree at …`). Git enforces one-branch-one-worktree.

## Oracle — independent recompute (`oracle-worktree-isolation.sh`) → 9/9 PASS

Reads the repo the probe left behind and recomputes every verdict by a
**different mechanism**: full-history commit-subject membership
(`git log --format=%s` + exact-line match) and `git branch --contains`
plumbing, versus the probe's tip-only `git log -1` + `git for-each-ref
--contains`. All nine agree: hitchhiker on `feat-b` not `feat-a`; both isolated
commits on their own branch and neither on the other; 3 worktrees; dup-branch
worktree refused. `oracle-output.txt`.

## Probe B — is the interim guard enforceable? (`probe-hook-fires.sh`)

A repo-tracked `.githooks/pre-commit` wired via `git config core.hooksPath
.githooks/`:

- Plain `git commit` on `main` → **BLOCKED** (exit 1) and the hook's marker
  file was written (proves it *executed*, independent of the commit's exit).
- `git commit` on a feature branch → **allowed**.
- `git commit --no-verify` on `main` → **succeeded** — the hook is bypassable,
  as documented. So a guard is advisory-strong: it fires on every normal commit
  (and Claude's commit path does not pass `--no-verify`), but is not an
  ironclad sandbox.

## What I learned that I didn't know before

**Worktree-per-session is stronger than "just use worktrees to stay organized":
git structurally refuses a second checkout of an in-use branch (exit 128), so
the degenerate "two sessions on one branch" case is impossible at the git
level, not merely discouraged.** And a repo-tracked hook via `core.hooksPath`
does fire under a plain commit, so an interim branch-guard is a real,
enforceable option — bounded only by `--no-verify`. The existing memory
convention (`feedback_verify_branch_before_commit`) is documentation that
already failed 3×; the structural fix removes the shared checkout so there is no
wrong branch to land on.

## Design fork this surfaces (for the HARD PAUSE)

The mechanism is settled; the deliverable *shape* is the open decision:
- **A. Docs-only** convention — weakest; the failed 3× convention shows docs
  alone don't hold.
- **B. Docs + a worktree-provisioning helper** (`scripts/…`) sessions run at
  start — operationalizes the primary fix.
- **C. B + an enforced `core.hooksPath` guard** — belt-and-suspenders; needs a
  decision on *what* it blocks (protect `main`? require a non-default branch for
  feature commits?), since with worktrees the cross-branch case is already
  structurally impossible and the guard mainly hardens the shared-checkout
  interim.
