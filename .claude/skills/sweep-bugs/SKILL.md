---
name: sweep-bugs
description: Batch-close small tethys bugs in parallel — select 2-4 small P2-P4 ready bugs from rivets, fan out worktree-isolated subagents (reproduce → minimal fix → regression fence → PR each), watch CI, merge serially, close the issues. Use whenever the user says "sweep bugs", "burn down the bug backlog", "close some small bugs", "work a few bugs in parallel", or complains that issues are being opened faster than closed.
---

# sweep-bugs — the small-bug track

The gilfoyle loop earns its cost on features; running it on a 20-minute
bug is why small bugs pile up. This track trades the design gate for a
strict smallness filter plus a mandatory red-first repro: a bug that can
be reproduced by a failing test and fixed minimally doesn't need a design
— and any bug that turns out to need one gets kicked back out, not forced
through.

There is exactly **one pause**: batch confirmation before the fan-out.
Merges at the end are mechanical once CI is green (announce, don't ask,
unless a merge needs a rebase with conflicts).

## 1. Select the batch (orchestrator, on main)

- `rivets list --json`; filter: type `bug`, status `open`, priority 2-4,
  no open `blocks` dependencies (check via `rivets show`; note that
  `rivets ready` truncates to 10 by default — use `-n 100` for a full
  survey; only `blocks` edges and blocked parents gate readiness).
- Apply the **smallness test** per bug, reading the issue and skimming the
  code it names: single subsystem, reproducible by a test, no schema
  change, no design decision, no open substrate dependency. When in doubt,
  it's not small.
- Pick 2-4 (start small; each bug costs a full subagent context). Present
  the batch with one line each — symptom, suspected locus, why it's small
  — and **wait for confirmation**.
- On confirmation: `rivets update <ids> -s in_progress`, commit the jsonl
  on main (`chore(rivets): sweep batch <ids> in progress`), push main if
  it accepts direct pushes — otherwise keep the commit local and let it
  ride to main at close-out.

## 2. Fan out (one subagent per bug, worktree-isolated)

Spawn all subagents in one message, each with `isolation: "worktree"`.
Each prompt must carry, verbatim, the constraints the subagent cannot
discover on its own:

- **Branch**: `fix/<rivets-id>-<short-slug>` created in the worktree.
- **Red first**: reproduce with a failing test BEFORE touching product
  code. If the bug cannot be reproduced, stop and report "cannot
  reproduce" with evidence — do not fix blind.
- **Minimal fix** at the root cause, then the failing test becomes the
  regression fence (name it after the bug class, not the issue number).
- **Impact analysis with tethys**: if the fix changes a function's
  signature/name/semantics, enumerate callers with
  `tethys callers <Type::method|fn> --lsp` (run `tethys index` first; add
  `--rebuild` if it reports a stale schema) before editing — it bounds what the
  fix must touch; `grep` is the recall net.
  - Use **QUALIFIED** names: `tethys callers show_picker` errors
    `not found: symbol`, `tethys callers "UiState::show_picker"` resolves.
  - Use `--lsp`. It is mutually exclusive with `--exclude-speculative`, and it
    is the only tier that finds cross-module callers. Measured on cyril
    2026-08-02, all three tiers side by side:

        symbol                  bare    --exclude-speculative   --lsp
        UiState::show_picker    12/3f   12/3f                   14/4f
        parse_options_response  11/1f   11/1f                   12/2f
        NotificationRoute        1/1f    1/1f                    3/1f

    Bare and `--exclude-speculative` are IDENTICAL here — the documented
    "recall net" bare invocation recovers nothing extra. Only `--lsp` surfaced
    `App::handle_notification` + `App::handle_command_result` (the ONLY two
    production callers of `show_picker`; the other 12 are tests) and `run_loop`
    in `bridge.rs` (the ONLY production caller of `parse_options_response`).
    Blast radius is a RECALL question — a false positive costs a minute of
    reading, a false negative ships a bug.
  - If tethys contradicts `grep`, `grep` wins. Also sanity-check the binary is
    current (`tethys --version` is a frozen `0.1.0` and cannot signal
    staleness); rebuild from `~/repos/tethys` if results look impossible.
  TRAP: if the bug is IN tethys's own resolver/call-edge logic, tethys cannot
  analyze its own change — use `grep`. Callers you surface but don't fix go in
  the report (escape hatch), not silently dropped.
- **Full gate with real exit codes**: `cargo nextest run`,
  `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`,
  doctests — `cmd > /dev/null 2>&1 && echo OK`, never `cmd | tail` (pipes
  swallow exit codes).
- **Commit format**: conventional, ONE lowercase scope, no commas —
  CI regex `^(feat|fix|docs|style|refactor|perf|test|build|ci|chore)(\([a-z][a-z0-9-]*\))?!?: .{3,}`.
  Subject cites the rivets ID: `fix(languages): ... (tethys-lwsc)`.
- **Never touch `.rivets/`** — the tracker belongs to the orchestrator;
  a jsonl change in two branches conflicts at merge nearly every time.
  Discovered side-issues go in the report, not the tracker.
- **Escape hatch**: if the fix turns out to need a design decision, a
  schema change, or edits across subsystems, STOP — report the assessment
  and the evidence instead of forcing a "small" fix. A kicked-back bug is
  a good outcome; a disguised feature merged without a design is not.
- **Finish**: push the branch, `gh pr create` with body sections
  *Repro* (the failing test and what it showed), *Cause*, *Fix*, *Fence*,
  `Closes`-free (the orchestrator closes rivets, and GitHub can't close
  rivets IDs anyway — reference the ID in prose).
- **Return** (final message is data for the orchestrator): PR URL, files
  touched, fence test name, anything discovered-but-not-fixed.

## 3. Collect, review, merge (orchestrator)

- As each subagent reports: sanity-check the PR diff — no `.rivets/`
  changes, fence present and failing-before/passing-after is plausible,
  no scope creep beyond the bug.
- Watch CI per PR (Monitor; emit on pass AND fail). Same CI quirks as
  `/ship` §5: commitlint single-scope, push/PR twin runs, auto-merge
  now enabled (`gh pr merge --auto --merge` queues merge-on-green).
- **Freeze main pushes while the queue drains**: every push to origin/main
  (including your own tracker chore commits) flips every queued PR to
  BEHIND and forces another update-branch + full CI cycle. Batch ALL
  tracker mutations as LOCAL commits and push once at close-out, after
  the last merge. (Learned the slow way: a mid-queue epic close-out
  added two full CI cycles.)
- **Before each merge, fetch bot review comments** on that PR
  (`gh api .../pulls/<n>/comments`) and assess with the verify-first
  discipline (ship §6): factual findings fix pre-merge; findings that
  converge with the fixing agent's own discovered-but-not-fixed list get
  FILED at close-out (convergence raises priority); speculative
  perf/hardening claims are verified against measurements and usually
  rejected with evidence.
- Merge serially (`gh pr merge --merge`), pulling main between merges.
  Small bugs rarely collide; if a later PR conflicts after an earlier
  merge, rebase it (or hand it back to a subagent to rebase) rather than
  merge-committing main into it.
- If a subagent kicked a bug back: relabel or re-scope the issue in
  rivets (it wasn't small — say why in a comment/description update), and
  leave it for `/ship`.

## 4. Close out (orchestrator, on main)

- `git checkout main && git pull --ff-only`.
- `rivets close <id> -r "Fixed: PR #<n> (<sha>). Repro: <fence test>."`
  for each merged bug; file any side-issues the subagents reported
  (search for duplicates first); one `chore(rivets)` commit for the lot.
- Report the batch: merged / kicked back / failed, with the open-vs-closed
  delta this sweep achieved.

## Sizing guidance

2-4 bugs per sweep until the failure modes are known. Signs the batch was
too ambitious: subagents hitting the escape hatch, PRs colliding on the
same files, review findings on more than one PR. Shrink before scaling.
