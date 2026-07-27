# cyril-a71q — pre-PR review decisions

Two-axis review (`/code-review` against `origin/main`), 2026-07-26. 14 findings.
Every bug claim was verified against the code, `main`, or `spec.md` before any change.

**Headline: the review caught a regression I introduced.** Slice 7 changed a
`continue` to `{}` and started forwarding terminals that `main` dropped. No fixture
caught it; the review did. That alone justified running this before opening the PR.

Tally: **4 accepted** (3 fixed here, 1 fixed by another finding's fix), **1 rejected**,
**6 deferred with tracker IDs**, **3 rejected as already-documented**.

## Standards axis

| # | Finding | Category | Verified? | Decision | Note |
|---|---|---|---|---|---|
| S1 | `classify_notification_route(Some,None)` → `Main` flips behaviour vs the old `unwrap_or(false)` | Bug | **Yes** — read both; with no main session a *tracked* subagent's frames fell through to main state | **Accept** | Reverted to `Subagent`. Silent, narrow, real. |
| S2 | Corollary: `if !is_main && self.session.id().is_some()` is now dead | Bug | **Yes** — `!is_main` implied `is_some()` under the flip | **Accept** | Fixed *by* S1's revert; the guard is live again. No separate change. |
| S3 | False CLAUDE.md citation justifying `pub fn starting_at` | Style | **Yes** — grep for the cited rule: **0 hits**; only callers are in-module tests | **Accept** | Now `#[cfg(test)] pub(crate)`. A citation to a rule that doesn't exist is worse than no comment. |
| S4 | `matches!` + a second match with `unreachable!` on the same value | Design | **Yes** — a production panic in the hot notification path | **Accept** | Collapsed to one `if let`. CLAUDE.md bans `unwrap`; an `unreachable!` a refactor could reach is the same hazard. `main` has zero; so does the branch now. |
| S5 | Data Clumps / Divergent Change → extract a `TurnMediator` | Design | **Yes** — the trio does only mutate together | **Reject (defer)** | **cyril-b4y4.** Reshaping the mediator at review time would invalidate the slice-by-slice mutation testing that established these fences discriminate. Better done *with* the fences in place. |
| S6 | Duplicated absorb blocks / registrations | Design | Yes | **Reject (defer)** | **cyril-b4y4** — same extraction. |
| S7 | Message Chain: `scope_is` walks `session.0.as_ref()` | Polish | Yes | **Reject (defer)** | **cyril-b4y4** — same extraction. |

## Spec axis

| # | Finding | Category | Verified? | Decision | Note |
|---|---|---|---|---|---|
| P1 | "no active turn ⇒ drop" not implemented; regression vs `main` | Bug | **Yes** — `main`: `if turn_in_flight.is_none() { continue; }`; branch: `None => {}`; `spec.md:242` requires Drop | **Accept** | Fixed + fenced. My slice-7 error: I conflated "foreign, forward to its consumer" with "no active turn, forward" — the foreign case has a consumer, this one doesn't. Forwarding double-commits streaming and metering. |
| P2 | C8 has no bridge-level fence; no injection seam | Bug (gap) | **Yes** — `run_loop` hardcodes `TurnAllocator::new()` | **Reject (defer)** | **cyril-ns0o.** Fail-closed *is* fenced at the unit level (slice 1); the loop-level assertion `spec.md:225` names is not. |
| P3 | C9 bypasses the mediator (raw channel, 257th asserted to fail) | Bug (gap) | **Yes** — read the fixture | **Reject (defer)** | **cyril-ns0o.** |
| P4 | Evidence seam is log-only; no test asserts the tuples | Bug (gap) | **Yes** — but the falsifier carries 2 `both_evidence` assertions and M2 fails 9 | **Reject** | Not a gap: this is **blindness B16**, already documented with its structural reason. The bridge *cannot* assert it under supported input; the falsifier is where `spec.md:192`'s assertions live. |
| P5 | ≤2 future bound only logged, not enforced | Bug (gap) | **Yes** | **Reject (defer)** | **cyril-ns0o.** Deliberate at build time (exceeding it is drift, not programmer error, so a log beats a panic) — but `spec.md:284` asks for a fence. |
| P6 | Three design-named fences absent under their names | Bug (gap) | **Yes** | **Reject (defer)** | **cyril-ns0o.** Coverage is partial for `terminal_scope_owner_matrix` (1 of 9 cells); the other two are renames of equivalent fences. |
| P7 | `classify_notification_route` extraction is scope creep | Design | **Yes** — it is outside bridge mediation | **Reject** | It is *how* C7 is fenced: the foreign-routing early return had zero coverage anywhere before it. The flip it introduced was real and is fixed (S1); the extraction itself earns its place. |
| P8 | `ActiveTurn` omits `engine`, so the KAS-only companion registration is unconditional | Bug | **Yes** — design says "(KAS only)"; only `convert/kas.rs` emits identity-free completions, so benign today | **Reject (defer)** | **cyril-upjh.** Latent trap, not a live defect: a future unstamped v2 completion would be absorbed as a phantom companion. |

## The fence that wasn't

Worth recording separately, because it nearly shipped. My first fence for P1
(`duplicate_wire_terminal_with_no_active_turn_is_dropped`) **passed under mutation** —
restoring the bug did not fail it. Cause: the harness delivers the prompt response
before the `turn_end`, so the response releases the turn and registers a *main-scoped*
`Wire` expectation, which then **absorbs** the `turn_end` before it can reach the
no-active arm. The test never touched the branch it claimed to fence.

Rebuilt as `unowned_terminal_with_no_active_turn_is_dropped`, scoping the `turn_end` to
a foreign session so the absorb check misses. Mutation-verified: restoring `None => {}`
fails it.

That is the second fake fence this build produced (the first: slice 11's
`!matches!(RateLimited, TurnCompleted)`, a tautology over enum variants). Both were
caught only by mutation-testing the fence itself. A fence that has never been observed
to fail is not yet evidence.
