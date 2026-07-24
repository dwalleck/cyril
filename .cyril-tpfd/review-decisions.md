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

## Round 2 — PR-63 two-axis review (2026-07-23)

Same protocol, run against the open PR. Each finding re-verified from
the working tree, not the reviewers' claims (the git state had moved
mid-review: a parallel session relocated the observed hitchhikers to
main while the review ran).

| # | Finding (one line) | Axis | Verified? | Decision | Note |
|---|---|---|---|---|---|
| R2-1 | `HookRunOutcome` is `pub(crate)` but never referenced outside hooks.rs | Standards | Yes — grep: zero external refs; every fn naming it in a signature is private | Accept | Now module-private |
| R2-2 | `(command, user_prompt, cwd, timeout)` clump wants a `HookInvocation` struct | Standards | Yes (it exists; two adjacent private fns) | Reject | Same settled class as S2: the params mirror the `executeHook` wire contract; a struct adds ceremony, enforces no new invariant |
| R2-3 | `HookDef.timeout: Option<u64>` where `Duration` is the concept | Standards | Yes | Reject | Field mirrors the on-disk hook-file schema (seconds as number); `effective_timeout()` is the single conversion point — wire/schema payloads are modeled verbatim in this repo |
| R2-4 | `SpawnFailed{message: String}` stringifies `io::Error`, losing `kind()` | Standards | — | Duplicate of S3 | Rejected there with settled rationale; nothing changed since |
| R2-5 | Matcher-carrying sessionStart hooks silently dropped; design line 55 promised "debug-logged by existing list filtering" | Spec | Yes — `matching()` has no log; no log anywhere on the list path | Modify | Reviewer's bug real, placement wrong: `debug!` at sessionStart membership (`session_start_hooks`), NOT generic list filtering, where matcher exclusion under a real `toolId` is routine wire semantics. Design text synced |
| R2-6 | Rivets issue said "blocked on: obtain the covenant .d.ts" — never obtained | Spec | Yes — findings.md documents no `.d.ts` ships in any bundle | Reject | The issue's intent was "don't guess the shape"; the two-site carve + live injection MATCH is stronger evidence than a `.d.ts`. Deviation documented in findings.md + PR body |
| R2-7 | plan.md slice 3b names nonexistent fence `session_start_timeout_seconds_not_millis` | Spec | Yes (grep) | Accept (docs) | plan.md now names the actual carrier fence + fold note (mirrors P2's design-table sync) |
| R2-8 | Hitchhiker commits `0bed3b0`/`d75427d` (cyril-5g2o) pollute the branch | Spec | Yes when observed, then overtaken — both dangling now; relocated to main (`707ff94`/`9b52d51`) by a parallel session mid-review | Reject (overtaken) | Residual recovery commits `fa376ec`/`e6ba5c6` kept deliberately: content converges with main (two-dot ROADMAP diff = tpfd's own slice-6 line only; lyrx/6beh rows exactly once on both sides) and removing them re-loses restored tracker rows. Recurring class (3rd occurrence) filed as cyril-4rc1 |
| R2-9 | Windows test + 8-line README caveat beyond plan text | Spec | — | Duplicate of P4 | Rejected there; the PR acceptance criteria name the Windows fence |

Outcome: R2-1 + R2-7 accepted, R2-5 modified (all three committed
separately); R2-2/R2-3/R2-6 rejected with rationale; R2-4/R2-9
duplicates of round-1 decisions; R2-8 overtaken by events, recurring
process class tracked at cyril-4rc1.
