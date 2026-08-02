# cyril-b4y4 — budgeted plan

Approved design: `.cyril-b4y4/design.md` (claims C1–C12, all four pause
decisions approved 2026-08-02). Per-slice gate: `cargo nextest run -p
cyril-core` (± `--features kas`), `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --check`, doctests. `--all-targets` is load-bearing: slices S2–S4
build the mediator before the loop uses it, and unit-test usage is what
satisfies dead_code until S5 rewires.

Global budget note: the mediator introduces **no loops** — `observe()` and
`begin_turn()` are O(1) per event (Option/equality checks); production scale
is ~10^1 terminal frames per turn, ~10^3 turns per session. The only loop the
feature touches (`prompt_tasks.retain`, n ≤ 4 observed) is untouched by
design. Output streams: tracing diagnostics only (stderr via subscriber); no
stdout writes anywhere in the feature. No new doc-comment preconditions:
`begin_turn` internalizes the busy/exhausted guards, `observe`/`cancel_target`
accept any input — nothing for callers to violate.

---

## Slice 1: Engine declares its terminal-source shape

**Claim:** C6 (engine half) — terminal-source authority is an Engine fact.
**Oracle:** live capture ground truth (turn-end-ordering: KAS emits both
terminals; v2 emits only the prompt response — reference memory + kiro wire
audits), asserted as `terminal_source_matrix`.
**Stress fixture:** the matrix test with BOTH engines asserted in one test —
bug class: a trait *default* method silently making both engines identical.
Enforcement: no default impl; each engine must answer explicitly.
**Loop budget:** none (constant fn).
**Files:** `crates/cyril-core/src/protocol/engine.rs`

Advisory: `fn emits_wire_turn_end(&self) -> bool;` on `Engine`; V2Engine
`false`, KasEngine `true`; doc names CONTEXT.md "Turn-end" + companion
terminal. KAS assertion under `#[cfg(feature = "kas")]`.

**Verification:** [ ] unit tests [ ] fixture [ ] oracle [ ] budgets

## Slice 2: TurnMediator skeleton — state, begin_turn, cancel_target, is_busy

**Claims:** C7 (visibility), C12 (begin triple), C10 (snapshot half).
**Oracle:** probe model's PROMPT-ACCEPTED/REJECTED lines (p1–p5) +
`TurnAllocator` tests as the identity oracle.
**Stress fixture:** (a) busy AND exhausted allocator → must return `Busy`
(bug class: allocate-before-guard reorder burns an id and reports the wrong
error); (b) `cancel_target` after release → `None`, and re-begin on another
session → new session (bug class: stale snapshot); (c) exhaustion via
`TurnAllocator::starting_at(u64::MAX)` through `with_allocator` — last id
issued, then `Exhausted` forever.
**Loop budget:** none.
**Files:** `crates/cyril-core/src/protocol/turn_mediator.rs` (new),
`crates/cyril-core/src/protocol/mod.rs`

Advisory: module owns `ActiveTurn { owner, expects_wire_terminal: bool,
session: types::SessionId }`, `Companion`, `CompanionSource` (moved copies;
bridge.rs keeps its originals until S5), `BeginTurn`, `TurnMediator`
(`pub(crate)` everything), `#[cfg(test)] fn with_allocator`.

**Verification:** [ ] unit tests [ ] fixture [ ] oracle [ ] budgets

## Slice 3: observe() — stamped arm + Disposition

**Claims:** C5 (ForwardTurnComplete only on release), C8 (single registration
helper), C9 (stamped-absorb evidence payload), plus C1's stamped cells.
**Oracle:** probe/oracle capture lines f2, f3, f5, f8, f12
(`probes/oracle-output.txt`).
**Stress fixture:** (a) f12 — stamped absorb while a DIFFERENT turn is live
(bug class: active checked before companion → evidence lost as stale-drop);
(b) upjh cell — release with `expects_wire_terminal: false`, then an
unstamped same-session frame must NOT absorb (bug class: unconditional Wire
registration, the phantom-companion regression); (c) f3 — stale stamp with a
live turn (bug class: id-blind "is anything running" release, the original
a71q defect).
**Loop budget:** none.
**Files:** `crates/cyril-core/src/protocol/turn_mediator.rs`

Advisory: `Disposition` enum exactly as designed (Absorb carries
`{owner, first, second}`); non-`TurnCompleted` → `Forward` unconditionally;
one `Companion::expected(...)`-style constructor used by both arms.

**Verification:** [ ] unit tests [ ] fixture [ ] oracle [ ] budgets

## Slice 4: observe() — unstamped arm, absorb-first, logging, full matrix

**Claims:** C3 (sync matrix, ri8q seam), C4's fence cells, C9 (wire-absorb
evidence), C11 (DropUnowned logged), C1's unstamped cells.
**Oracle:** capture lines f1, f4, f6, f7, f9, f10, f11 + the full 17-step
sequence replay against `probes/probe-output.txt` expectations.
**Stress fixture:** (a) f9 — dangling Wire companion on the SAME session as
the live turn: must Absorb, must NOT release turn#3 (bug class: release
checked before absorb — falsifier mutation M3); (b) f9→f10 pair (bug class:
absorb without `companion.take()` — mutation M2 — makes f10 Absorb instead of
ForwardTurnComplete); (c) f11 — session-`None` frame (bug class: assuming
scope is always `Some`); (d) the pinned out-of-scope cell: global unstamped
WITH a live turn → Forward (foreign-shaped), so a future producer meets a
decided cell. All five tracing lines land here verbatim + the new DropUnowned
debug line.
**Loop budget:** the matrix test iterates 17 scripted steps — test-only,
O(steps).
**Files:** `crates/cyril-core/src/protocol/turn_mediator.rs`

**Verification:** [ ] unit tests [ ] fixture [ ] oracle [ ] budgets

## Slice 5: Rewire run_loop; delete the inline policy

**Claims:** C1 (loop equivalence), C2 (suite unchanged), C6 (module purity),
C8 (duplicates gone from bridge.rs).
**Oracle:** the full committed test suite pre/post (identical pass set, both
`cargo nextest run -p cyril-core` and `--features kas`), which includes every
a71q fence (AC3).
**Stress fixture:** the suite's existing adversarial fences ARE the fixture —
notably `v2_release_leaves_companion_ledger_empty` (phantom companion),
`companion_ledger_absorbs_one_and_does_not_leak_across_turns` (freeze), the
cancel-retarget fence, and the 256-backlog fence. Expected: zero behavioral
diffs.
**Loop budget:** none added; ~140 lines deleted.
**Files:** `crates/cyril-core/src/protocol/bridge.rs`

Advisory: SendPrompt arm matches `BeginTurn` (error texts unchanged);
notification arm becomes the 5-arm `Disposition` match (silent arms
`continue`; `ForwardTurnComplete` gates the deferred-disconnect block);
io_done uses `is_busy()`; cancel uses `cancel_target()` + existing
`active_session_id` fallback (typed→acp conversion at the call site). DELETE:
bridge-local `ActiveTurn`/`Companion`/`CompanionSource`/`scope_is`/
`owes_wire_companion`, and the `wire_companion_is_owed_only_under_kas` test
(replacement halves landed in S1 + S3(b); deletion comment points at both).
Exceeds the 50-line guideline deliberately: the cutover is atomic — an
intermediate state referencing both policies does not compile. Budget 45 min.

**Verification:** [ ] unit tests [ ] fixture [ ] oracle [ ] budgets

## Slice 6: Promote the probe-oracle scenario to a permanent fence

**Claim:** C1's regression fence.
**Oracle:** `probes/oracle-output.txt` (pre-extraction capture) — the fence's
expected sequence is transcribed from it, not from the new code.
**Stress fixture:** the scenario itself is adversarial by construction
(markers prove FIFO position of invisible outcomes; f9/f12 precedence cells;
five prompt acceptances prove guard/ledger state between frames).
**Loop budget:** test-only; 17 steps.
**Files:** `crates/cyril-core/src/protocol/bridge.rs`

Advisory: adapt `probes/oracle_scenario.rs` minus the tracing subscriber and
`eprintln!` scaffolding; name `turn_mediation_probe_scenario`; keep
`#[cfg(feature = "kas")]`; doc header cites the b4y4 capture as provenance.

**Verification:** [ ] unit tests [ ] fixture [ ] oracle [ ] budgets

## Slice 7: ADR-0004 turn-completion amendment

**Claim:** AC5 / design's ADR section — the doc matches the shipped shape.
**Oracle:** the merged code itself (section references `TurnMediator`,
`emits_wire_turn_end`, retained early-conversion — all grep-verifiable
against the branch).
**Stress fixture:** doc slice — exempt from a logic fixture; verification is
the grep cross-check that every named identifier exists in the code.
**Loop budget:** none.
**Files:** `docs/adr/0004-bridge-loop-mediates-both-acp-directions.md`

Advisory: update the turn-completion paragraph (stale `turn_in_flight:
Option<SessionId>` → mediator ownership; loop remains the single observation
point; terminal-source authority = Engine fact per CONTEXT.md Turn-end);
mark the Consequences bullet (line ~153) landed.

**Verification:** [ ] doc matches code (grep) [ ] links resolve

## Slice 8: Mutation audit + manual fences + status fill

**Claims:** C4 (M2 verified by mutation), plus M3; C6/C8/C11 manual greps
(approved at the pause).
**Oracle:** the mutations themselves — each must flip named matrix cells red
while the rest of the suite stays green (localization proof).
**Stress fixture:** M2 = remove `companion.take()` clearing on absorb →
f9→f10 pair fails; M3 = swap absorb/release check order → f9 fails. Both
REVERSED BY EDIT afterward (never `git checkout` on uncommitted work —
standing feedback rule).
**Loop budget:** none.
**Files:** `.cyril-b4y4/audit.md` (new), `.cyril-b4y4/design.md` (Status
column fill).

**Verification:** [ ] both mutations fail the named cells only [ ] greps
recorded [ ] design table updated

---

## Plan self-review

1. **Loops:** no production loops introduced (S1–S8); observe/begin O(1);
   test-only iteration bounded at 17 steps. `prompt_tasks.retain` untouched.
   No gaps.
2. **Fixtures:** every logic slice names its bug class (S1 trait-default;
   S2 guard-order/stale-snapshot/exhaustion-boundary; S3 precedence/phantom-
   companion/id-blind-release; S4 M2/M3/None-scope/undecided-cell; S5
   existing adversarial fences; S6 FIFO-marker scenario; S8 mutations). S7 is
   a doc slice — exempt, with a grep cross-check instead. No gaps.
3. **Doc-comment preconditions:** none introduced; guards are internalized
   in `begin_turn`'s return type. No enforcement debt.
4. **Write targets:** tracing (diagnostic/stderr) only. No data writes. No
   gaps.
5. **Tracker references:** cyril-ns0o (verified open — bridge-level
   exhaustion fence stays there; S2 advances only the mediator half),
   cyril-ri8q (verified open — closes WITH this PR per pause decision 4),
   cyril-pnwb (verified open — evidence preserved, precedence untouched),
   cyril-g9vt (verified open — unblocked by this landing). B17 residual is
   settled rationale recorded in design negative-space #3, not deferred work.
   No gaps.

Claim coverage: C1(S5,S6) C2(S5) C3(S4) C4(S4,S8) C5(S3) C6(S1,S5,S8)
C7(S2) C8(S3,S5,S8) C9(S3,S4) C10(S2+existing loop fence) C11(S4,S8)
C12(S2) — matches the design's claim list, no orphans.
