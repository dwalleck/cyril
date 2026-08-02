# cyril-b4y4 — TurnMediator extraction: falsifiable design

Date: 2026-08-02. Inputs: `.cyril-b4y4/findings.md` (probe/oracle 17/17
agreement, f1–f12), issue cyril-b4y4 + its 2026-08-02 source-authority note,
ADR-0004 (whose Consequences already anticipate this pass amending its
turn-completion section), CONTEXT.md "Turn-end".

## Purpose

Extract `run_loop`'s inline turn-ownership policy — the `active_turn` /
`turn_alloc` / `companion` locals and the ~140-line notification-arm decision
(bridge.rs:2124-2281) plus the SendPrompt gate (1116-1147), cancel targeting
(1227-1230), and the io-death busy probe (2292) — into a pure, sync,
unit-testable `TurnMediator` type. Behavior-preserving except one additive
delta: the unowned drop gains a debug log line (probe finding 1).

## Architecture and placement (step 2c)

**Owner:** new file `crates/cyril-core/src/protocol/turn_mediator.rs`,
everything `pub(crate)`. `ActiveTurn`, `Companion`, `CompanionSource` move
there from bridge.rs. `TurnId`/`TurnAllocator` stay in `types/turn.rs` (they
cross to the App on `RoutedNotification`). Wins over `types/`: this is bridge
machinery no other layer may call; wins over staying in bridge.rs: a 5,432-line
file whose policy can only be observed through the async harness is the
problem being fixed.

**Interface** (prescribed by the ticket + arch-review Candidate 02 —
`design-an-interface` skipped as extract-existing, not a new seam):

```rust
pub(crate) enum BeginTurn { Accepted(TurnId), Busy, Exhausted }

pub(crate) enum Disposition {
    Forward,                       // pass through (non-terminals, foreign terminals)
    ForwardTurnComplete,           // pass through; the active turn just released
    Absorb {                       // expected second terminal — consumed
        owner: TurnId,
        first: (CompanionSource, StopReason),
        second: (CompanionSource, StopReason),
    },
    DropStale { stale: TurnId },   // stamped, matches nothing
    DropUnowned,                   // unstamped, nothing owed — now logged
}

impl TurnMediator {
    fn new() -> Self;
    fn begin_turn(&mut self, session: SessionId, expects_wire_terminal: bool) -> BeginTurn;
    fn observe(&mut self, routed: &RoutedNotification) -> Disposition;
    fn cancel_target(&self) -> Option<&SessionId>;   // dispatch-time snapshot
    fn is_busy(&self) -> bool;
}
```

`SessionId` is **cyril's** newtype (`types::SessionId`), not acp's. The
mediator imports zero `agent_client_protocol`: `begin_turn` receives the
SendPrompt command's own typed id; `cancel_target` returns the typed id and
the loop converts for `conn.cancel`. `scope_is` (the `session.0.as_ref()`
message chain) is deleted — comparison becomes newtype `PartialEq`.

**Source authority (issue note 2026-08-02):** `Engine` trait gains
`fn emits_wire_turn_end(&self) -> bool` (KasEngine → true, V2Engine → false).
`ActiveTurn.engine: AgentEngine` becomes
`expects_wire_terminal: bool`, snapshotted at `begin_turn` — the snapshot
semantics the a71q comment demanded survive; the `engine == AgentEngine::Kas`
kind-match dies. The mediator never names `AgentEngine`.

**Logging:** the five existing tracing lines move into the mediator verbatim
(fields `{owner, first, second}`, `{stale_owner, active}`, etc. — the pnwb
evidence trail); `DropUnowned` gains a new debug line. Absorb evidence is
ALSO carried on the `Absorb` variant payload so tests assert it without log
parsing.

**Loop after extraction:** SendPrompt arm matches `BeginTurn` (Busy/Exhausted
→ the two existing BridgeError sends, unchanged text); notification arm
becomes `match mediator.observe(&routed)` — three silent arms `continue`,
two forward arms send, `ForwardTurnComplete` additionally runs the
deferred-disconnect block; io_done arm uses `is_busy()`; cancel arm uses
`cancel_target()` with the existing `active_session_id` fallback.

**Forbidden:** no acp imports, no `AgentEngine`/`engine.kind()`, no channels,
no async, no JoinHandles in turn_mediator.rs; no `pub` re-export from
cyril-core's lib.rs; bridge.rs may not construct `Companion`/`ActiveTurn`
directly (they move out of its scope); cyril-ui/cyril cannot name the type
(`pub(crate)` makes that a compile error).

**Deletion test (issue):** deleting turn_mediator.rs removes stale-completion
protection, KAS dual-terminal dedup, absorb-first precedence, and cancel
targeting in one cut — lifecycle knowledge is concentrated, not smeared.

## Input shapes (step 2)

`begin_turn`: state {idle, busy} × allocator {available, exhausted} → 4 cells;
busy+exhausted must yield `Busy` (guard-before-allocate order pinned, as
today).

`observe`, production-reachable cells (empirically pinned by probe f1–f12):

| Frame | State | Cell | Probe |
|---|---|---|---|
| other-notification | any | Forward (stamp/scope irrelevant) | (trivial) |
| stamped, owner matches Synthesized companion | any active | Absorb | f2, f12 |
| stamped, owner matches active | — | ForwardTurnComplete (+Wire companion iff expects_wire_terminal) | f5, f8 |
| stamped, matches neither | active present or not | DropStale | f3 |
| unstamped, session matches Wire companion | active on SAME session | Absorb (absorb-first, M3) | f6, f9 |
| unstamped, session matches active | — | ForwardTurnComplete (+Synthesized companion, unconditional) | f1, f10 |
| unstamped, foreign session, turn active | — | Forward | f4 |
| unstamped, no active turn, nothing owed | scoped or global | DropUnowned | f7, f11 |

Out-of-scope shape: global (session-`None`) unstamped terminal **while a turn
is active** — no producer exists (synthesis always stamps; convert::kas always
scopes); the matrix test still pins its current behavior (Forward, foreign-
shaped) so a future producer meets a decided cell, not an accident.
`StopReason` variants: policy is reason-agnostic (passed through / recorded as
evidence, never branched on) — one claim-free sentence, preserved by C1/C9.

## Removed-invariant sweep (step 2b)

Purely structural move: no serialization point, guard, ordering, or
uniqueness property is removed. The single-observer property (one loop arm
observes every terminal) is untouched — the mediator is called from that same
arm; `Rc`/channel topology unchanged. The engine→bool snapshot preserves the
a71q "release reads the turn's own properties" invariant by construction.
Not subtractive.

## Claims and falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| C1 | Post-extraction, the f1–f12 scenario driven through `run_loop` yields the identical forward/absorb/drop sequence | run promoted fence; any deviation falsifies | pre-extraction capture `probes/oracle-output.txt` (committed) | 5m | **baseline captured & passed pre-extraction (2026-08-02)** | promoted bridge fence `turn_mediation_probe_scenario` (kas feature) |
| C2 | Every existing bridge fence passes unchanged (AC3) | `cargo nextest run` ±kas pre/post, identical pass set | committed suite (written pre-extraction) | 10m | pending (post-build) | CI |
| C3 | All matrix cells reproducible in a sync unit test — no tokio, no harness (AC1, ri8q seam) | write `mediator_matrix_all_dispositions`; any cell needing the harness falsifies | probe f1–f12 expected outputs | 20m | pending | that test |
| C4 | B16 promoted: absorb clears the ledger, now assertable | mutation M2 (absorb w/o `take()`) must FAIL the f9→f10 cell pair (f10 would Absorb again instead of ForwardTurnComplete); revert after | mutation run | 10m | pending | matrix cells f9+f10 |
| C5 | B18-policy promoted: only owned/scoped releases return ForwardTurnComplete; foreign returns Forward | matrix cell f4 asserts Forward; buggy impl: any forwarded terminal reported as completion | matrix | 5m | pending | matrix cell f4 (loop-wiring half stays B18-residual, documented) |
| C6 | Mediator contains no AgentEngine / engine.kind() / acp; terminal-source fact enters via `Engine::emits_wire_turn_end()` | grep turn_mediator.rs; Engine matrix test (Kas→true, V2→false); buggy impl: kind-match inside module | grep + engine test | 5m | pending | `engine` test `terminal_source_matrix`; grep is review-time **manual** (needs approval) |
| C7 | `pub(crate)` visibility; no lib.rs re-export; UI/binary crates cannot name it | make it `pub` + re-export → external use compiles = falsified; buggy impl caught at compile | compiler | 0m | pending | compile-time visibility (mechanical, permanent) |
| C8 | Absorb logic exists once; Companion registration once (AC2) | grep: one `companion.take()`, one "absorbed expected companion" call site, Companion construction only in one helper | grep | 5m | pending | review-time **manual** (structural; needs approval) |
| C9 | pnwb evidence preserved: Absorb payload carries `{first, second}` pairs, both orders | matrix asserts payloads on f2/f6/f9/f12 (orders swap); buggy impl: evidence dropped or order collapsed | probe capture's logged pairs | 10m | pending | matrix payload asserts |
| C10 | cancel_target = dispatch-time snapshot; None when idle | matrix cell (begin S → target S; release → None); loop-level retarget fence already exists (a71q/84ca) | existing fence + matrix | 5m | pending | both tests |
| C11 | DropUnowned logs a debug line (only intentional behavior delta) | grep one tracing call in that arm; buggy impl: silent `continue` returns | grep | 2m | pending | review-time **manual** (log-only; needs approval) |
| C12 | begin_turn triple: Busy when active (even if also exhausted), Exhausted when allocator dry, Accepted otherwise | matrix cells using `TurnAllocator::starting_at(u64::MAX)`; buggy impl: allocate-before-guard reorder | types/turn.rs allocator tests + matrix | 10m | pending | matrix cells |

**Cheapest falsifier — run and passed:** the extended probe/oracle (f8–f12
precedence cells) ran against the real `run_loop` before this design was
written; 17/17 agreement (`probes/oracle-output.txt`). A disagreement on f9
(absorb-first) or f12 (owner-keyed absorb during a live turn) would have
falsified the matrix this design stands on.

## Affected-fence inventory

- `wire_companion_is_owed_only_under_kas` (cyril-upjh, NOT an a71q fence) —
  asserts on `ActiveTurn::owes_wire_companion(engine)`, which this design
  dissolves. REWRITTEN as: (a) Engine-trait matrix `terminal_source_matrix`
  (KasEngine emits wire turn_end, V2Engine does not); (b) mediator matrix
  cell: release with `expects_wire_terminal: false` leaves the ledger empty
  (upjh's phantom-companion regression, now directly assertable). The RULE
  survives; its seam moves.
- All a71q-named fences drive the loop black-box and are untouched (AC3).
- The temp oracle test is promoted to the permanent `turn_mediation_probe_scenario`
  fence (C1), markers and all, minus the tracing/eprintln scaffolding.

## ADR-0004 amendment (in this pass, AC5)

ADR-0004's Consequences already schedule this: "Candidate 02 … amends *this
ADR's* turn-completion section." The amendment updates the turn-completion
paragraph — `turn_in_flight: Option<SessionId>` is two generations stale
(a71q made it `ActiveTurn`; this pass makes it `TurnMediator`) — to state:
the loop remains the single observation point for terminals; ownership
policy lives in `TurnMediator` (pub(crate)); terminal-source authority is an
Engine fact (`emits_wire_turn_end`), per CONTEXT.md "Turn-end". The early
conversion of both raw terminal sources into pre-mediation `TurnCompleted`
markers is explicitly RETAINED (the "reopening" the review's amber box
worried about is not taken).

## Negative space

1. **No terminal-production changes**: convert/kas.rs mapping and off-loop
   prompt synthesis/stamping stay byte-identical; early-conversion design
   retained.
2. **No pnwb precedence decision**: evidence recording preserved verbatim;
   which signal's stop_reason wins on cancel stays open at cyril-pnwb
   (needs a live cancel capture first).
3. **Prompt-task lifetime, Shutdown abort, deferred-disconnect drain stay in
   run_loop**: JoinHandles and channel drains are I/O; the mediator is a pure
   state machine (same rule as SessionController). B17 remains a signed
   blindness with its a71q rationale. (Scope decision — flagged for the
   design pause.)
4. **No bridge-level exhaustion fence through run_loop**: stays cyril-ns0o
   item 1 (verified open); this pass advances only its mediator half
   (Exhausted is now unit-drivable).
5. **No new public API**: nothing on cyril-core's pub surface; no
   consumer-less capability sub-traits (engine.rs:12 rule) —
   `emits_wire_turn_end` lands WITH its consumer.

## Open decisions for the pause

1. **Scope**: pure-state-machine scope as designed (trio + observe +
   begin/cancel/busy), vs the review's wider cut (prompt tasks, shutdown,
   disconnect ordering). Design recommends narrow; negative-space #3 is the
   rationale; AC5 is still satisfied because the ADR amendment happens
   regardless.
2. **Engine method name**: `emits_wire_turn_end()` (wire-fact phrasing) vs
   `dual_turn_terminals()`. Design recommends the former — it names the
   observable, not the count.
3. **Manual fences**: C6's grep, C8, C11 are review-time structural checks,
   not CI tests. Skill requires explicit approval for `manual`.
4. **ri8q disposition**: this design implements ri8q option (a) (observation
   seam). Close ri8q with this PR, or leave open pending its own review?
