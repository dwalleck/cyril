# cyril-4rc1 — related issues (prove-it-prototype step 0)

Tracker sweep 2026-07-24 (keywords: worktree, parallel session, shared,
hitchhiker, rebase, working dir, checkout, verify branch, process).

## Direct lineage (this is a process class, not a code bug)

- **cyril-4rc1 itself** — the only tracker item about the shared-checkout /
  worktree process problem. No duplicate, no prior-art fix to reuse.
- **Origin context** (`.cyril-tpfd/review-decisions.md`): P3 (round 1) and
  R2-8 (round 2) both flagged parallel-session **hitchhiker commits** on the
  tpfd branch — commits from a concurrent session landing on whichever branch
  this shared checkout had out. R2-8's residual recovery commits
  (`fa376ec`/`e6ba5c6`) were kept because removing them re-lost restored
  tracker rows. The "3rd occurrence" that filed cyril-4rc1.
- **Two prior data-loss events named in the issue:**
  - `d67e5b8` rivets rows lost → restored by `e6ba5c6`.
  - transport-layering ROADMAP point dropped → re-applied by `fa376ec`.
  Both are **rebase-drop** failures during hitchhiker *relocation*.

## Not related (keyword false positives)

`process` matched a large cluster of KAS/bridge child-process issues
(cyril-0pms, cyril-ba5x, cyril-lw67, …) — OS-process lifecycle, unrelated to
this git-workflow class. `worktree` matched cyril-7z7u only via the word in a
comment. `checkout` matched cyril-xi4a (CRLF, unrelated).

## Existing guidance that did NOT prevent recurrence

Memory `feedback_verify_branch_before_commit` already says "check
`git branch --show-current` in the same command as every commit." It exists and
the problem still recurred 3×. Signal: a documented convention alone is
insufficient; the fix direction (worktree-per-session) removes the shared
checkout so there is no wrong branch to land on.
