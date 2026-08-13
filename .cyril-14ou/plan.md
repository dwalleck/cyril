# cyril-14ou budgeted plan — turn liveness

Approved design: `.cyril-14ou/design.md` (threshold 30s, once-per-quiet-period).
Claims C9-live / C10 / C11-replay already **passed** at design time; the build
turns pending claims into fences.

## Slice 1: `TurnLiveness` pure state machine

**Claim:** C1 (quiet ≥ T with nothing outstanding ⇒ one emission decision),
C2 (re-arm on traffic), C3 (outstanding reply parks), C5 (disarm on turn end).
**Oracle:** hand-computed emission decisions for synthetic timelines (written in
the tests before the impl); cross-checked against the design falsifier's rule
(falsifier_c11.py replayed the same rule over real captures and passed).
**Stress fixture:** timeline that interleaves ALL inputs: begin → frames every
1s ×3 → 40s quiet with outstanding=1 (expect NO fire) → reply → 31s quiet
(expect fire ONCE) → frame (re-arm) → 31s quiet (expect second fire) → end →
40s quiet (expect nothing). Defeats: forgotten `armed` reset (C2 bug), missing
outstanding park (C3 bug), missing disarm (C5 bug), double-fire (C1 bug).
Plus: `check` called with `now` equal to last stamp (zero elapsed — boundary),
and stamp-after-end (late frame for a dead turn: no panic, no arm).
**Loop budget:** none — all methods O(1); `check` runs once per 5s tick.
**Wall budget:** n/a (no always-on phase beyond the tick, counted in slice 3).
**Files:** `crates/cyril-core/src/protocol/turn_liveness.rs` (new),
`crates/cyril-core/src/protocol/mod.rs` (module decl).
**Note:** per the staged-module pattern, if run_loop consumption lands in a
later slice than the module, declare `#[cfg(test)]` first and lift to
`pub(crate)` in slice 3 — or land slices 1+3 in one commit window if trivial.
Time is an INPUT (`Instant` parameters); the struct never reads a clock.
**Verification:**
- [ ] Unit tests pass (`turn_liveness::{stall_emits_once, rearm_after_traffic, outstanding_reply_parks, disarm_on_turn_end}` + stress timeline)
- [ ] Stress fixture produces the pre-written expected emission sequence
- [ ] falsifier_c11.py still passes (rule unchanged)
- [ ] Budgets hold (O(1) methods; no loops)

## Slice 2: `Notification::TurnStalled` + mediator non-terminality

**Claim:** C6 — TurnStalled is forwarded by the mediator and leaves `is_busy`
and the companion ledger untouched.
**Oracle:** mediator `Disposition` output (enum, directly observable) for a
synthetic TurnStalled mid-turn; expected value written before the impl.
**Stress fixture:** TurnStalled arriving (a) mid-turn on the active session,
(b) with no active turn, (c) between a wire turn_end and its companion —
all three must Forward without touching `active`/`companion`. Defeats: adding
the variant to the terminal match arm (early release), or absorbing it as a
companion.
**Loop budget:** none.
**Files:** `crates/cyril-core/src/types/event.rs` (variant `TurnStalled { quiet: Duration }`),
`crates/cyril-core/src/protocol/turn_mediator.rs` (test only — `observe` already
matches only `TurnCompleted`, so non-terminality should hold with zero code; the
test is the fence).
**Verification:**
- [ ] `turn_mediator::stalled_is_forwarded_nonterminal` passes (all 3 fixture arms)
- [ ] Exhaustive-match fallout from the new variant resolved without weakening any match (no catch-all added where variants were explicit)
- [ ] Budgets hold

## Slice 3: run_loop wiring — tick arm, stamps, emission, SpawnConfig threshold

**Claim:** C1 end-to-end (bridge emits session-scoped TurnStalled once per quiet
period), C4 (foreign-session frames don't stamp), plus C10's doc pointer.
**Oracle:** bridge harness tests using the existing stub-transport pattern
(bridge.rs test mod) — expected notification sequences written before the impl;
independent of TurnLiveness's own unit tests (they assert channel OUTPUT, not
struct state).
**Stress fixture:** harness run where (a) a turn goes quiet past a tiny test
threshold (50ms) with foreign-session frames arriving every 10ms — TurnStalled
MUST still fire (defeats stamp-on-any-scope, the C4 bug); (b) no active turn +
quiet — nothing fires (defeats ungated tick, the C5-at-loop-level bug); (c) an
outstanding host-callback window spanning the threshold — nothing fires until
after the reply (C3 at loop level, if host callbacks are loop-visible; if they
prove not to be, STOP per drift rules and surface — the design's C3 mechanism
depends on it).
**Loop budget:** interval tick: 1 wake / 5s while busy, O(1) work per wake —
≈ 0.2 ops/s, far under budget. Stamp updates: O(1) per notification, on the
existing per-frame path (adds no new loop).
**Wall budget:** the tick arm adds ≤1 branch per select wake; no measurable
always-on cost at production scale (frames ≈ tens/s peak).
**Files:** `crates/cyril-core/src/protocol/bridge.rs`,
`crates/cyril-core/src/types/config.rs` (SpawnConfig `stall_threshold: Duration`,
default 30s const, doc: tests deliberately pass tiny thresholds — no lower-bound
assert).
**Doc-comment contract:** `stall_threshold` doc states "values at or below the
healthy inter-frame ceiling (~8s) will false-positive" — sanity hint (wrong
output = a noisy chip, not corruption; no cyril caller passes one) ⇒ doc only,
no runtime check, per the classification rule.
**Verification:**
- [ ] Harness tests pass (`stall_emits_at_threshold`, `foreign_traffic_does_not_mask_stall`, `no_stall_without_active_turn`, `outstanding_callback_parks_loop_level`)
- [ ] Stress fixtures produce pre-written sequences
- [ ] falsifier_c11.py rule still matches implementation semantics (same decision on the same timelines — spot-replay)
- [ ] Budgets hold at fixture scale

## Slice 4: capture-derived replay fence (C11)

**Claim:** C11 — T=30s yields zero false stalls on real healthy traffic and one
stall on the real stall trace; T=8s yields ≥1 on healthy traffic (tight-bound
guard proving the fixture can observe emissions at all).
**Oracle:** the committed timing tables themselves — extracted from the bh7g
wire captures (real production-shape traffic), independent of cyril code.
**Stress fixture:** the three timing tables (run-5, run-6 healthy; run-1 stall
including its post-last-frame horizon — the horizon IS the bug the design
falsifier caught in itself; the table must encode it explicitly). Defeats:
threshold edited below the ceiling; clock-advances-only-on-frames; fixture
regenerated too coarsely (the ≥1-at-8s assert fails if the table can't observe).
**Loop budget:** O(events) per table, events ≈ 700 — trivial, test-only.
**Files:** `crates/cyril-core/tests/turn_liveness_replay.rs` (new),
`crates/cyril-core/tests/fixtures/turn_liveness_timings.json` (new, generated
by `.cyril-14ou/falsifier_c11.py --emit-table` mode added in this slice — the
generator is committed with the audit trail, not run in CI).
**Verification:**
- [ ] Replay test passes all three assertions (0 @30s healthy, 1 @30s stall, ≥1 @8s healthy)
- [ ] Table values spot-match probe_gaps.py output (same captures, independent parser)
- [ ] Budgets hold

## Slice 5: UiState stall state (C7)

**Claim:** C7 — TurnStalled sets a stalled state; any other notification clears
it; TurnCompleted clears it alongside busy.
**Oracle:** UiState field assertions with expected values written first;
sequences mirror the real capture order (stall → late completion).
**Stress fixture:** apply TurnStalled → AgentMessage (clears) → TurnStalled
again (second quiet period sets again) → TurnCompleted (clears). Plus:
TurnStalled arriving with NO active busy state (late signal after completion —
must not set a chip for an idle session; defeats unconditional set). Plus double
TurnStalled without traffic between (idempotent set, no flicker state).
**Loop budget:** none — O(1) field updates.
**Files:** `crates/cyril-ui/src/state.rs`, `crates/cyril-ui/src/traits.rs`
(TuiState read accessor `stall()`).
**Verification:**
- [ ] `state::stall_set_and_cleared` + adversarial arms pass
- [ ] Budgets hold

## Slice 6: toolbar chip render + approval suppression (C8)

**Claim:** C8 — the chip renders when stalled && busy, is suppressed while the
approval overlay is active, and escalates wording when `cancel_sent`.
**Oracle:** TestBackend rendered buffer text (position + content asserted),
expected strings written before the impl.
**Stress fixture:** render matrix: {stall × approval-overlay × cancel_sent} —
8 cells, expected chip presence/text per cell written first. Defeats:
render-whenever-stalled (chip over the overlay), inverted suppression, wrong
escalation copy. Narrow-terminal render (width 40) must truncate, not panic
(defeats unchecked width arithmetic).
**Loop budget:** none beyond existing per-frame render (chip is O(1) spans).
**Files:** `crates/cyril-ui/src/widgets/toolbar.rs` (or the toolbar's actual
module — locate at build time), render tests colocated.
**Verification:**
- [ ] `toolbar::stall_suppressed_during_approval` + 8-cell matrix pass
- [ ] Narrow-width render doesn't panic
- [ ] Budgets hold

## Slice 7: App wiring — cancel-sent escalation (C9 plumbing)

**Claim:** C9 (plumbing half) — Esc during a stalled busy turn sends the
existing CancelRequest AND marks the UI stall state `cancel_sent`; the live
half (engine honors cancel) passed at design time and is archived in
findings.md.
**Oracle:** App-level key-dispatch test asserting both effects; expected
BridgeCommand written first (existing app test pattern for key chains).
**Stress fixture:** Esc while stalled+busy (both effects); Esc while busy but
NOT stalled (cancel sent, no stall mutation — defeats unconditional marking);
Esc while stalled but a drill-in/overlay consumes Esc first (no cancel — the
existing key-layer priority must hold; defeats bypassing the layer chain).
**Loop budget:** none.
**Files:** `crates/cyril/src/app.rs`.
**Verification:**
- [ ] `esc_marks_cancel_sent_during_stall` + both adversarial arms pass
- [ ] Existing Esc-layer tests still pass (no key-chain regression)
- [ ] Budgets hold

## Plan self-review

1. **Loops:** one new interval tick (1/5s, O(1)) — stated in slice 3, under
   budget by ~7 orders; replay test O(≈700) test-only; all other slices O(1)
   field/enum work. No unstated loops.
2. **Fixtures:** every slice names the bug class its fixture fails under
   (armed-reset, outstanding-park, scope-stamping, ungated tick, terminal-match
   pollution, unconditional set, overlay suppression inversion, unconditional
   cancel-marking, horizon bug, coarse-table blindness). No happy-path-only
   fixtures.
3. **Doc-comment preconditions:** one — `stall_threshold` lower bound,
   classified sanity-hint (noisy chip, not wrong data; tests legitimately pass
   tiny values) ⇒ documentation only, deliberately no assert. No load-bearing
   preconditions introduced.
4. **Write targets:** none new — notifications ride the existing bounded
   channel; the only prints are `tracing::debug!` on emission (diagnostic, per
   house style). No stdout/stderr writes added.
5. **Tracker references:** cyril-w9oi (kill-and-respawn second tier — verified,
   filed this session); cyril-bh7g (closed, evidence base); C3's
   host-callback-visibility risk is a STOP-on-drift condition in slice 3, not a
   deferral. No other deferral phrases present.

Claim coverage: C1 (slices 1+3), C2 (1), C3 (1+3), C4 (3), C5 (1+3), C6 (2),
C7 (5), C8 (6), C9 (7 + design-time live pass), C10 (design-time pass; doc
pointer in slice 3), C11 (4). All 11 covered.
