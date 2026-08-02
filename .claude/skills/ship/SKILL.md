---
name: ship
description: Run the full issue-to-merge pipeline for one tethys rivets issue — verify/pick the issue, branch, gilfoyle loop (probe → falsifiable design → HARD PAUSE for approval → plan → checkpointed build), pre-PR code review with fixes, open PR, watch CI, merge on green after confirmation, close the issue. Use whenever the user says "ship <issue-id>", "work tethys-XXXX", "pick the next issue and build it", "run the pipeline", or asks to take any rivets issue end-to-end — even if they don't say "ship".
---

# ship — one rivets issue, end to end

The pipeline has exactly **two hard pauses**: design approval and merge
confirmation. Everything between them runs autonomously. The pauses are not
ceremony — in past runs the design pause caught decisions only the user
could make (manual fences, naming, output posture), and the checkpointed
build's own stop-on-drift rules caught bugs every fixture missed. Do not
soften either.

## 0. Select and verify the issue

- If the user named an issue: `rivets show <id>`. Confirm it's open and its
  `blocks` dependencies are closed.
- If not: propose one. `rivets ready` truncates to 10 results by default
  (hybrid sort) — with 30+ ready issues, P3/P4 candidates fall below the
  fold and look "hidden". Use `rivets ready -n 100` or `rivets list` for
  a full survey. (Only `blocks` edges to unclosed issues — and blocked
  parents, transitively — actually gate readiness; `related` and
  `parent-child` links to open issues do NOT. A previous version of this
  skill claimed otherwise; that was a truncation artifact misdiagnosed as
  dependency filtering.) Check blockers via `rivets show`, rank by
  priority and the PRD roadmap (tethys-l6nt), and present the pick with a
  one-line rationale before starting.
- Features get the full loop below. If the issue is a small bug
  (single-subsystem, reproducible by a test, no design decisions), say so
  and suggest `/sweep-bugs` instead — the full loop is wasteful there.
- **Epics**: the default is one issue → one PR after the full loop, but an
  epic whose description prescribes its own delivery structure (e.g.
  tethys-9z7i: "each slice its own PR; slice 2 through the gilfoyle loop")
  overrides the default — honor the issue's verbiage. Surface the
  structure at this step so the user knows the run will produce multiple
  PRs, run each prescribed slice through the pipeline stages appropriate
  to it (a docs slice needs the design pause, not a probe harness), and
  close the epic only when every non-deferred slice has merged.

## 1. Setup — branch etiquette

Every run starts from a freshly-pulled `main` and does all its work on a
new feature branch. Work is never committed to `main`.

```
git status --porcelain                        # clean, or only .rivets/issues.jsonl
git checkout main && git pull --ff-only       # not a fast-forward => STOP
rivets update <id> -s in_progress
git checkout -b feat/<id>-<short-slug>        # e.g. feat/tethys-xoxq-visibility-tightening
git branch --show-current                     # must print the new branch
```

- **`--ff-only` is the point.** If the pull refuses, local `main` has
  diverged — resolve it before branching with `git pull --rebase`, which
  replays the local commits and discards nothing. Branch off a diverged
  main and the PR diff carries commits the ticket never asked for, and the
  reviewer can't tell yours from theirs. (Local `main` merely being *ahead*
  of `origin/main` does not refuse — `--ff-only` reports "Already up to
  date" — so this only fires once the remote has also moved.)
- **One branch per issue, never reused.** `feat/<id>-<slug>` for
  features, `fix/<id>-<slug>` for bugs. The id is the rivets id, so the
  branch, the PR, and the ticket are greppable together months later.
- **Verify the branch, don't assume it.** `git branch --show-current`
  after creating it, and again in the same command as every commit (see
  Conventions). A commit that lands on `main` costs a revert to undo.
- If `.rivets/issues.jsonl` has uncommitted changes at this point, commit
  them on main FIRST (`chore(rivets): ...`) before branching — tracker
  state from a previous cycle should not ride into this branch's history
  unannounced.
- **Worktree-enforcing repos**: if the repo mandates one worktree per
  concurrent session (its CLAUDE.md/AGENTS.md says so, and a pre-commit
  hook refuses feature commits in the primary checkout), create the
  branch with that repo's helper instead of `git checkout -b`, and swap
  the close-out `git branch -d` for `git worktree remove`.

## 2. The gilfoyle loop

Run the four skills in order. Each one's own gates apply in full; this
skill only sequences them.

1. `/gilfoyle:prove-it-prototype <id>` — probes + independent oracle +
   `related-issues.md` + `findings.md`, committed as
   `chore(<slug>): prove-it-prototype probes + findings (<id>)`.
   Artifacts live in `.<id-short>/` (e.g. `.tethys-xoxq/`) and are
   committed to the branch — they are the audit trail.
2. `/gilfoyle:falsifiable-design` — claims table, cheapest falsifier RUN
   before presenting, negative space, tracker-clean deferrals. Commit it.
3. **HARD PAUSE.** Present the design summary and every flagged open
   decision. Wait for explicit approval. Do not start the plan on a hedge
   ("looks fine I guess") — ask for a real yes, and apply any requested
   changes to the design first.
4. **Glossary reconciliation.** The approved design is where domain terms
   crystallise, and the write trigger must sit on this path — `CONTEXT.md`
   went six ADRs stale (0005–0010, backfilled 2026-08-02) because term
   *resolution* flowed through this pipeline while term *recording* lived
   only in skills the pipeline never invokes. Diff the design's vocabulary
   against `CONTEXT.md` (and `CONTEXT-MAP.md` contexts where present):
   - the design coins or resolves a term, or an ADR written this run
     introduces one → add/amend the entry now, in `/domain-modeling`
     format (tight definition + `_Avoid_` list);
   - the design uses a term in conflict with an existing entry → resolve
     the collision now (rename in the design or amend the glossary), not
     at review;
   - nothing new → say "glossary: no new terms" in one line and move on;
     never invent entries to have something to write.
   Commit glossary changes on the branch (`docs: ...`) so the PR carries
   the vocabulary with the code it names.
5. `/gilfoyle:budgeted-plan` — slices with claims/oracles/fixtures/budgets.
   Commit it.
6. `/gilfoyle:checkpointed-build` — one commit per slice, all gates per
   slice, STOP on drift per that skill's rules (drift stops surface to the
   user; they are not this skill's to adjudicate).

## 3. Pre-PR review (before opening the PR, not after)

Run `/code-review` with `--fix` against `main` on the finished branch.
Verify each finding before applying (the reviewer can be wrong in both
directions); commit fixes as their own conventional commits. Doing this
pre-PR means the PR opens already-reviewed instead of collecting bot
churn.

Backstop the step-4 glossary reconciliation here: if the branch adds or
amends an ADR, or the diff introduces a new domain noun, `CONTEXT.md`
must change in the same PR — or the "glossary: no new terms" call from
step 4 must still hold against the *built* code, which sometimes coins
vocabulary the design didn't.

## 4. Open the PR

Push (`git push -u origin <branch>`), then `gh pr create` with the house
body shape (see PR #10/#11 for worked examples):

- **The ticket is closed in the PR body**, first line:
  `Closes <rivets-id> (<one-line what>)`. If the work also has a GitHub
  issue, add a real `Closes #<n>` — GitHub's auto-close keys only on its
  own issue numbers, so the rivets id in that line is documentation and
  the actual `rivets close` still happens at step 6. Both belong in the
  body: after merge, the PR is the permanent link between the code and
  the ticket that motivated it.
- **What this does** — the design's core rule in prose, with the numbers
  that justify it (probe measurements).
- **Acceptance criteria** — checklist, each AC mapped to the named test
  fence that proves it.
- **Method + evidence** — pointer to the `.<id-short>/` artifacts (probes,
  design, plan, audits) and the headline audit results.
- **Discovered and filed** — issues filed during the loop, with IDs.
- **Notable behavior changes** beyond the ticket.

## 5. Watch CI

Use a Monitor (poll `gh pr checks` / `mergeStateStatus`, 30s interval,
emit on pass OR fail — silence must not look like success). Known quirks,
all hit in practice:

- **Commitlint**: CI validates every subject against
  `^(feat|fix|docs|style|refactor|perf|test|build|ci|chore)(\([a-z][a-z0-9-]*\))?!?: .{3,}`
  — ONE lowercase scope, hyphens ok, **no commas** (`feat(db,lib)` fails;
  pick the primary scope and note the second file in the body). Check
  subjects against this at commit time, not at PR time — rewording a
  merged-in commit means a rebase and force-push.
- **Zero checks reported within ~2 min of pushing** = merge conflict
  fingerprint, not a queue delay. Run `git diff origin/main --stat`.
- The workflow runs on both `push` and `pull_request`, with identical job
  names. Branch protection waits on the **latest run per context**, so a
  green PR-event run can still read BLOCKED while the push-event twin
  finishes. Wait; don't re-push.
- Auto-merge is **enabled** in repo settings — `gh pr merge --auto --merge`
  queues the PR to merge automatically once `CI Success` passes, so you need
  not babysit to the finish. Manual `gh pr merge --merge` on a `CLEAN`
  `mergeStateStatus` still works if you'd rather merge on green yourself.

## 6. Merge and close out

- **Before merging: fetch review comments, if any exist.** Check all three
  surfaces, since they're distinct APIs and a finding in one won't appear
  in the others:
  ```
  gh api repos/<owner>/<repo>/pulls/<n>/comments   # inline, on a diff line
  gh api repos/<owner>/<repo>/pulls/<n>/reviews    # review bodies + verdicts
  gh api repos/<owner>/<repo>/issues/<n>/comments  # plain PR conversation
  ```
  **Gemini Code Assist is sunset** (confirmed on PR #70, 2026-07) — it now
  posts only a notice that review has ceased. No bot currently reviews this
  repo, so an empty result is the expected outcome, not a signal to wait.
  Don't sit on a green PR expecting a review that isn't coming.
- **Whatever the source — bot, human, or a review you ran yourself —
  verify each finding before applying it.** `/gilfoyle:assessing-review-feedback`
  is the discipline: each finding is two separable claims (the bug is real;
  the proposed fix is right), and either can be wrong alone. Reproduce the
  bug before accepting it exists, then judge the fix on its own merits.
  Record accept/modify/reject per finding with a one-line rationale.
  Calibration from experience: factual checks (wrong identifier names,
  inconsistent tracker IDs, a doc surface missed) are usually real — fix
  pre-merge. Speculative perf/hardening suggestions usually fail
  verification — reject with evidence. A finding that converges with an
  already-known limitation gets FILED, not hotfixed. And watch for the
  **right-bug-wrong-fix** case: on PR #70 a reviewer correctly found the
  audit probe reported a false pass, but its proposed remedy would have
  made the probe report *every* field as unconsumed. Applying that verbatim
  would have shipped a worse tool than the one being fixed. Fixes push a new
  commit → CI re-runs → the merge waits.
- **PAUSE**: confirm with the user before merging (skippable only if they
  already said "merge when green" this session).
- `gh pr merge <n> --merge` (merge-commit convention, matching history).
  Add `--delete-branch` unless the repo already deletes merged branches
  automatically.
- **Get the merge commit into local `main`** — the next step's `git branch -d`
  refuses without it.
  ```
  git checkout main && git fetch origin
  git rev-list --left-right --count main...origin/main   # left=local-only right=remote-only
  ```
  If left is `0`, `git pull --ff-only` and move on. If left is **non-zero**,
  `main` has diverged and `--ff-only` will refuse — **this is the normal
  case here, not an anomaly**: the `chore(rivets)` commits this pipeline
  makes on `main` (issue filed at step 0, issues filed mid-run) are local
  until someone pushes, and the PR merge moves `origin/main` underneath
  them. `git pull --rebase` replays them on top; it discards nothing.
- **Before rebasing, look at what those local commits actually are.** On
  this run, two of the four belonged to a *different session* working in
  the same primary checkout (`cyril-4rc1` — the worktree rule protects
  feature branches, but main-line `chore(rivets)` commits land in the
  primary checkout by design, so they still collide). Rebasing them is
  safe. **Pushing them is not your call** — a foreign commit may be
  mid-flight work its author hasn't finished. Rebase so `main`
  fast-forwards, then say plainly what's unpushed and let the user push.
  Never `git push origin main` carrying another session's commits without
  asking.
- **Delete the local branch**: `git branch -d feat/<id>-<slug>`, then
  `git fetch --prune` to drop the stale remote-tracking ref. Use
  lowercase `-d`, never `-D`: `-d` refuses to delete an unmerged branch,
  which is exactly the check you want here — a refusal means the merge
  did not land the way you think it did, so investigate rather than
  force. Left-behind branches are how a later run branches off the wrong
  base and reopens work that already shipped.
- `rivets close <id> -r "Shipped: PR #<n> merged to main (<sha>). <AC/fence
  summary>. Fixed in-branch: <ids>. Filed: <ids>."`
- Commit the tracker mutation the close just made:
  `git add .rivets/issues.jsonl && git commit -m "chore(rivets): close <id> (PR #<n> merged)"`.
  Commit it now rather than leaving it to ride with the next branch — an
  uncommitted jsonl is what step 1 then has to clean up before it can
  branch, and a `rivets` mutation left uncommitted is invisible to everyone
  else.
- Offer the next pick; do not start it unprompted.

## Conventions that bind throughout

- **Verify the branch in the same command as the commit.**
  `git branch --show-current && git commit -m "..."` — a branch check made
  in an earlier, separate call proves nothing by the time the commit runs,
  and in a session with parallel work it is routinely already stale.
  Applies to every commit in the run, not just the first.
- **Gates use real exit codes.** `cargo clippy --all-targets -- -D warnings
  > /dev/null 2>&1 && echo OK` — never `cmd | tail -1` (the pipe swallows
  the exit code; a gate leaked a clippy failure into a commit exactly this
  way once).
- Full gate per slice: `cargo nextest run`, clippy pedantic `-D warnings`,
  `cargo fmt --check`, doctests.
- **Impact analysis dogfoods tethys.** For a slice that changes a function's
  signature/name/semantics, list callers with
  `tethys callers <Type::method|fn> --lsp` (run `tethys index` first; add
  `--rebuild` on a stale-schema error), `grep` as the recall net — EXCEPT when
  the slice edits tethys's own resolver/call-edge logic, where the tool can't
  oracle a change to itself and `grep` is the source of truth. See AGENTS.md →
  "Dogfood tethys for impact analysis."
  - **QUALIFIED names only** — a bare method name errors `not found: symbol`
    (`"UiState::show_picker"`, not `show_picker`).
  - `--lsp` and `--exclude-speculative` are MUTUALLY EXCLUSIVE. Prefer `--lsp`:
    it is the only tier that finds cross-module callers. Measured on cyril
    2026-08-02 — bare and `--exclude-speculative` returned IDENTICAL results
    (`UiState::show_picker` 12/3f, `parse_options_response` 11/1f,
    `NotificationRoute` 1/1f) while `--lsp` returned 14/4f, 12/2f, 3/1f. The
    extras are the production callers: `App::handle_notification` +
    `App::handle_command_result` for `show_picker` (the other 12 are tests),
    and `run_loop` in `bridge.rs` for `parse_options_response`. NOTE this
    contradicts AGENTS.md step 3, which treats a bare invocation as the recall
    net — on this workspace it recovers nothing over the precision tier.
    Blast radius wants recall; use `--exclude-speculative` only to ask
    "which of these edges can I trust?"
  - `tethys --version` is a frozen `0.1.0`, so it cannot signal a stale build.
    If output looks impossible, rebuild from `~/repos/tethys` before
    concluding the tool or these docs are wrong.
- Tracker discipline everywhere: every deferral names a verified rivets ID;
  discovered bugs are filed before (or with) their fix; duplicates searched
  before filing.
- If parallel work is in flight (other open PRs from ship/sweep sessions),
  keep `.rivets/issues.jsonl` OUT of this branch: file issues from a
  separate main checkout, or queue them in `.<id-short>/to-file.md` and
  file at close-out. The jsonl is one-line-per-issue and conflicts at
  merge almost every time two branches touch it.
