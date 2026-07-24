# cyril-4rc1 — budgeted plan

Approved design: helper + docs + **blocking** guard + **CI** fence (FD-1/FD-2
resolved). 4 slices, each ≤2 files, each ≤30 min. This is dev-tooling (bash +
markdown + workflow YAML), so "loop budgets" are bounded by the number of git
worktrees/hooks in play (< ~10 in any real checkout) — stated per slice but
never the binding constraint. The prove-it oracle here is
`.cyril-4rc1/oracle-worktree-isolation.sh` (git-structural isolation) plus the
per-claim asserts in `scripts/tests/worktree_test.sh` (each claim its own
function → distinct failure localization, design self-review #4).

Gate per slice: `bash scripts/tests/worktree_test.sh` green + `shellcheck`
clean on the touched scripts (if available) + the touched artifact runs. No
cargo gates — no Rust changes. Commit one slice at a time; STOP on drift.

---

## Slice 1: worktree provisioner helper + its fences

**Claim:** H1–H6 — `session-worktree.sh <branch> [base]` creates/reuses a
linked worktree at a primary-anchored sibling path, sanitizing slash-names,
handling new-vs-existing branches, idempotent re-runs, surfacing git's in-use
refusal, and erroring on a missing arg.
**Oracle:** `git worktree list --porcelain`, `git branch --show-current`,
`basename`, and exit codes — all independent of the helper's own stdout.
**Stress fixture:** a temp repo where (a) the branch name is `feat/cyril-4rc1`
(slash → must not nest), (b) the helper is run **from inside a linked
worktree** (must still anchor dest as a sibling of the PRIMARY, not nest under
the current worktree), (c) a second run reuses (idempotent), (d) a raw
`git worktree add` on the in-use branch is refused, (e) no-arg → exit 2. Bug it
must fail under: a helper anchoring `dest` off `--show-toplevel` (current
worktree) instead of the primary → nested worktree dir under case (b).
**Loop budget:** one pass over `git worktree list --porcelain` (O(worktrees),
worktrees < ~10). No syscall storm.
**Wall budget:** n/a (on-demand dev tool, not an always-on phase).
**Files:** `scripts/session-worktree.sh` (new), `scripts/tests/worktree_test.sh` (new).

**Code (advisory):** harden `.cyril-4rc1/proto-helper.sh`, anchoring dest to the
primary via `--git-common-dir` (not `--show-toplevel`):
```bash
#!/usr/bin/env bash
set -euo pipefail
usage(){ echo "usage: session-worktree.sh <branch> [base-ref]" >&2; exit 2; }
[ $# -ge 1 ] || usage
branch="$1"; base="${2:-HEAD}"
common=$(cd "$(git rev-parse --git-common-dir)" && pwd)   # .../repo/.git
primary=$(dirname "$common")                              # .../repo
dest="$(dirname "$primary")/cyril-wt-${branch//\//-}"     # sibling of primary
if existing=$(git worktree list --porcelain | awk -v b="refs/heads/$branch" \
    '$1=="worktree"{p=$2} $1=="branch"&&$2==b{print p}'); [ -n "$existing" ]; then
  echo "$existing"; exit 0
fi
if git show-ref --verify --quiet "refs/heads/$branch"; then
  git worktree add -q "$dest" "$branch"
else
  git worktree add -q -b "$branch" "$dest" "$base"
fi
echo "$dest"
```
Output-stream: the worktree path is **data** → stdout (a caller does
`cd "$(session-worktree.sh feat/x)"`); usage/errors → stderr. Doc-comment
preconditions: none load-bearing (a bad branch name surfaces as git's own
error, non-zero exit — not silent wrong output).

**Verification:**
- [ ] `worktree_test.sh` h1–h6 pass
- [ ] Stress fixture (run-from-linked-worktree) yields a sibling dest, not nested
- [ ] Oracle (`git worktree list`) shows exactly the expected worktrees
- [ ] Loop/wall budget hold (worktrees < 10)

---

## Slice 2: blocking pre-commit guard + its fences

**Claim:** G1–G4 — a `.githooks/pre-commit` blocks a feature-branch commit in
the **primary** checkout (exit 1), allows `main`/detached-HEAD there, allows
feature commits in a **linked** worktree, and is bypassable via `--no-verify`.
**Oracle:** `git commit` exit code + `git rev-parse HEAD` movement, in temp
repos wired with `core.hooksPath` — independent of the hook's stderr text.
**Stress fixture:** one temp repo, four commits: (G2) on `main` in primary →
succeeds; (G1) on `feat-x` in primary → blocked, HEAD unmoved; (G3) on `feat-x`
in a linked worktree → succeeds; (G4) `--no-verify` on `feat-x` in primary →
succeeds. Bug it must fail under: a naive `branch==main`-only guard (no
primary-vs-linked test) → **G3 fails** (blocks the linked-worktree feature
commit too). A guard using `--git-dir` string-equality without resolving the
relative `.git` → false "linked" in primary → **G1 fails** (feature commit
wrongly allowed).
**Loop budget:** no loops — three `git rev-parse` calls + one `case`. O(1).
**Wall budget:** n/a.
**Files:** `.githooks/pre-commit` (new), `scripts/tests/worktree_test.sh` (add g1–g4).

**Code (advisory):**
```bash
#!/usr/bin/env bash
# cyril-4rc1 guard: feature work belongs in a linked worktree; the primary
# shared checkout is main-line only (parallel sessions collide there).
branch=$(git rev-parse --abbrev-ref HEAD)
case "$branch" in main|HEAD) exit 0 ;; esac      # main-line + detached (rebase/merge)
gitdir=$(git rev-parse --absolute-git-dir)
common=$(cd "$(git rev-parse --git-common-dir)" && pwd)
[ "$gitdir" != "$common" ] && exit 0             # linked worktree → allow
echo "cyril-4rc1: refusing a '$branch' commit in the primary shared checkout." >&2
echo "Use a worktree:  scripts/session-worktree.sh $branch   (override: --no-verify)" >&2
exit 1
```
Output-stream: the refusal is **diagnostic** → stderr (a commit hook's job is
to talk to the human/agent, not a pipe). Doc-comment precondition: the
"main-line only" rule is **load-bearing** and enforced at runtime here (the
hook exits 1 in release — no `debug_assert` fiction; hooks aren't compiled).

**Verification:**
- [ ] `worktree_test.sh` g1–g4 pass
- [ ] Stress fixture: naive `branch==main` guard fails g3 (proves non-vacuity)
- [ ] Oracle (commit exit + HEAD movement) matches per arm
- [ ] O(1), no loop

---

## Slice 3: docs convention (CLAUDE.md + AGENTS.md)

**Claim:** D1 — CLAUDE.md (Development Workflow) and AGENTS.md state the
worktree-per-session convention, name `scripts/session-worktree.sh`, and give
the one-time `git config core.hooksPath .githooks` enable step.
**Oracle:** `grep` for the helper path + `core.hooksPath` in both files
(slice-4's `d1_docs` test) — independent of prose.
**Stress fixture:** the `d1_docs` grep asserts BOTH files contain BOTH tokens;
bug it must fail under: convention added to only one file (grep on the other
fails), or the enable step omitted (grep for `core.hooksPath` fails).
**Loop budget:** n/a (prose).
**Wall budget:** n/a.
**Files:** `CLAUDE.md` (edit — the existing "Development Workflow" section),
`AGENTS.md` (edit). (The `d1_docs` test function lands in slice 4 with the CI
runner, keeping this slice at 2 files.)

**Code (advisory):** a "### Parallel sessions" subsection under Development
Workflow: "Concurrent Claude sessions MUST NOT share this checkout — each runs
in its own linked worktree. Start one with `scripts/session-worktree.sh
<branch>` and `cd` into the printed path. The primary checkout is for main-line
chores only; a `.githooks/pre-commit` guard enforces it once enabled with
`git config core.hooksPath .githooks` (a one-time per-clone step). Override a
guard block with `git commit --no-verify`." Mirror one line into AGENTS.md.
Output-stream: n/a. Doc precondition: n/a.

**Verification:**
- [ ] Both files contain `scripts/session-worktree.sh` and `core.hooksPath`
- [ ] `d1_docs` (added in slice 4) passes
- [ ] Prose matches the enforced behavior (no over-claim: guard is opt-in + bypassable)

---

## Slice 4: CI fence job + d1 test + test runner

**Claim:** the deterministic fence runs in CI — a `worktree-tooling` ubuntu job
runs `scripts/tests/worktree_test.sh` on every push/PR, and it is required by
`ci-success`; the test file has a `main` that runs all claim functions and
exits non-zero on any failure, including `d1_docs`.
**Oracle:** the workflow run itself (the job's pass/fail) + local
`bash scripts/tests/worktree_test.sh; echo $?`.
**Stress fixture:** temporarily break one claim (e.g. rename the helper) and
confirm `worktree_test.sh` exits non-zero locally — proves the runner
aggregates failures rather than masking them (the `| tail -1` / swallowed-exit
class from the ship conventions). Restore before commit.
**Loop budget:** the runner loops over ~11 test functions (O(claims),
claims ≈ 11). Each function spins one temp git repo (a handful of git
syscalls). Total < 10^3 syscalls — within the 10^3 syscall bound for a
CI-invoked (not always-on) phase.
**Wall budget:** CI job < ~30s (11 tiny temp-repo git ops); not an always-on
phase, but bounded well under the other jobs.
**Files:** `.github/workflows/ci.yml` (add `worktree-tooling` job + add it to
`ci-success` `needs`), `scripts/tests/worktree_test.sh` (add `d1_docs` +
`main` runner). 2 files.

**Code (advisory):** the test file's `main` runs every `hN_/gN_/d1_` function,
tracks failures, exits 1 if any fail (real exit code, never `| tail`). The CI
job:
```yaml
  worktree-tooling:
    name: Worktree Tooling
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Worktree helper + guard tests
        run: bash scripts/tests/worktree_test.sh
```
and `ci-success` gains `worktree-tooling` in `needs:` + its result check.
Output-stream: test progress → stdout (data a reader wants), the final
FAIL summary → stderr; non-zero exit is the machine signal.

**Verification:**
- [ ] `worktree_test.sh` runs all 11 claim functions, exits 0 clean
- [ ] Breaking one claim makes the runner exit non-zero (fixture)
- [ ] `ci-success.needs` includes `worktree-tooling`
- [ ] YAML valid (job parses; the PR's own CI run is the live oracle)

---

## Plan Self-Review

1. **Every loop:**
   - Slice 1 helper: one pass over `git worktree list` — O(worktrees), < 10. ✓
   - Slice 2 guard: no loop, O(1). ✓
   - Slice 4 runner: O(claims ≈ 11), each a handful of git syscalls, < 10^3 total. ✓
   - Slices 3: no loops (prose). ✓
2. **Every fixture (bug class it fails under):**
   - S1: helper anchored on `--show-toplevel` → nests when run from a worktree. ✓
   - S2: naive `branch==main` guard → blocks linked-worktree feature commits (g3). ✓
   - S3: convention in only one doc / missing enable step → grep fails. ✓
   - S4: runner swallows a sub-failure (`| tail`) → break-one-claim fixture catches it. ✓
   None is happy-path-only.
3. **Every doc-comment precondition:**
   - S1 helper: no load-bearing precondition (bad input → git error + non-zero, not silent wrong output). ✓
   - S2 guard: "primary = main-line only" is load-bearing → enforced by runtime `exit 1` (hooks run in release; no `debug_assert`). ✓
4. **Every write target:**
   - S1: worktree path = data → stdout; usage/errors = diagnostic → stderr. ✓
   - S2: refusal = diagnostic → stderr. ✓
   - S4: test progress = stdout, summary = stderr, exit code = signal. ✓
5. **Every tracker reference:**
   - `cyril-d4zp` (auto-provision deferral) — filed + linked `discovered-from cyril-4rc1` (verified this session). ✓
   - No other deferrals in the plan.

No gaps.
