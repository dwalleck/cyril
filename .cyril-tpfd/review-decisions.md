# cyril-tpfd — pre-PR review decisions (2026-07-23)

Two-axis review (standards + spec) via parallel agents against `main`.
Each finding verified before applying (gilfoyle/assessing-review-feedback).

| # | Finding (one line) | Axis | Verified? | Decision | Note |
|---|---|---|---|---|---|
| S1 | `session_start_hooks()` hand-duplicates `list()`'s filter; doc claims an invariant two parallel filters don't enforce | Standards | Yes — two independently written predicates | Accept | Extracted shared `matching(trigger, tool_id)`; both consume it — the claimed invariant is now structural |
| S2 | `Vec<(&HookDef, HookRunOutcome)>` data clump wants a named struct | Standards | Yes (it exists) | Reject | Local plumbing between two adjacent private fns; a struct adds ceremony, no new invariant |
| S3 | `SpawnFailed{message: String}` stringifies `io::Error`, losing `ErrorKind` | Standards | Yes | Reject | Both consumers display-only; the enum is internal. Settled rationale, not deferred work — the type gains an `io::Error` field if a consumer ever needs kind-matching |
| P1 | Claim 2's named fence `session_start_results_in_registry_order` absent — order never tested through real execution | Spec | Yes (grep) | Accept | Fence added: two real hooks, reply order == within-file registry order |
| P2 | Claim 7's named fence folded into `session_start_skips_empty_and_timeout` without the design table saying so | Spec | Yes | Accept (docs) | Design table row 7 now names the actual carrier test |
| P3 | Scope creep: cyril-5g2o ROADMAP commit on the branch | Spec | Yes — a parallel-session hitchhiker (this working dir is shared) | Accept (process) | Relocated to main (`df527de`) + branch rebased; second occurrence this run (first: the 2.14.x audit commits + a rivets commit, now on main at `e63f4f6`) |
| P4 | README caveat 8 lines vs plan's "one line"; extra Windows test | Spec | Yes | Reject | Both content-correct and commit-noted; plan text is advisory |

Outcome: S1 + P1 + P2 applied (commits follow); S2/S3/P4 rejected with
rationale; P3 resolved by history relocation, not a code change.
