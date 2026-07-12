# Feature: Per-turn terminal ownership across sessions and engines (re-anchored)

Date: 2026-07-12
Supersedes: `spec-superseded-sole-turn-end.md` (choice-A sole-`turn_end` contract, voided
per `timing-audit.md`) and `spec-pre-bounded-exact-one.md`.

## What this is

Cyril will associate every accepted prompt turn with a process-local `TurnId(u64)` owner
so that a late or foreign terminal observation cannot end a different active turn. The
KAS wire contract is anchored on the researched emission order (`timing-audit.md`): a
normal KAS turn emits one scoped `session_info_update.kind == "turn_end"` **followed by**
the `session/prompt` RPC response, back-to-back on one ordered stream. **First-source-wins
release is retained** (cyril-j16p): whichever terminal source the serialized mediator
receives first releases the turn and forwards exactly one completion; the other source is
the turn's *expected companion* and is absorbed as recorded evidence, never as a second
completion. Liveness never depends on which source arrives; no supported input leaves the
terminal Busy forever.

## Users

- **Terminal operator**: sends prompts, cancels work, switches sessions, and needs
  Busy/Idle state and input availability to correspond to the turn actually running —
  including never being frozen Busy by a missing signal and never having a running turn
  falsely ended by a stale one.
- **Cyril protocol maintainer**: consumes this contract when maintaining
  terminal-producing paths (KAS rate-limit handling in cyril-3zy4, stop-reason precedence
  in cyril-pnwb) and needs release authority, companion absorption, evidence retention,
  and resource bounds to be explicit.

## Behavior

### Ownership stamping

- **Given**: `SendPrompt` is accepted while no turn is active.
- **When**: the bridge allocates the next `TurnId` (checked, never reused) before RPC
  dispatch and records the active turn `{owner, engine, session}`.
- **Then**: every completion the bridge itself synthesizes for that turn (v1/v2 prompt
  response, KAS prompt response, error-path completion) carries that owner id. The wire
  KAS `turn_end` cannot be stamped (converter has no turn identity; prototype evidence:
  no native turn-id candidate on the frame) and is matched by scoped session instead.

### Normal KAS turn — researched order (`turn_end` then response)

- **Given**: accepted KAS turn T is Busy and the wire delivers T's scoped `turn_end`
  followed by T's prompt response.
- **When**: the mediator receives the scoped `turn_end` matching T's session.
- **Then**: Cyril forwards exactly 1 completion carrying `turn_end`'s stop reason,
  releases T without any intentional wait, records the `turn_end` source/reason
  observation, and registers T's synthesized response as the one expected companion.
  When the id-stamped response completion arrives (typically the next event), it is
  absorbed: 0 completions forwarded, 0 turns released, its source/reason recorded
  alongside `turn_end`'s for cyril-pnwb. The next prompt is accepted from release onward;
  the companion's later arrival does not disturb the newer turn.

### Inverted receipt order (defensive)

- **Given**: accepted KAS turn T is Busy and internal scheduling delivers T's id-stamped
  response completion to the mediator before T's scoped `turn_end`.
- **When**: the mediator receives the response first.
- **Then**: the id match releases T, forwards exactly 1 completion carrying the
  response's stop reason, records its source/reason, and registers one expected wire
  companion for `(T.session, T.owner)`. The later scoped `turn_end` matching that
  expectation is absorbed: 0 completions, 0 releases, source/reason recorded. Observable
  behavior (one completion, correct turn released, both observations retained) is
  identical to the normal order.

### Degenerate KAS input — one source absent (unsupported-producer drift, handled safely)

- **Given**: accepted KAS turn T is Busy and the producer emits only one terminal source
  (response without `turn_end`, or `turn_end` without a response that ever resolves).
- **When**: the mediator receives the single source.
- **Then**: it releases T and forwards exactly 1 completion (liveness — Busy never
  persists past an available terminal source). The companion expectation dangles; a later
  same-session signal matching a dangling expectation is absorbed as stale rather than
  ending a newer turn, and every newer turn still releases exactly once via its own
  id-stamped source. No freeze, no wrong clear, at most 1 completion per turn.

### Same-session late stamped completion (the original cyril-a71q residual)

- **Given**: turn A in session S ended, turn B in S was subsequently accepted and is
  Busy, and an id-stamped completion for A (v1/v2 or KAS synthesized) remains unobserved
  by the mediator.
- **When**: the mediator receives A's stamped completion after B became active.
- **Then**: the owner id does not match B: Cyril forwards 0 completions, releases 0
  turns, absorbs it as A's companion evidence if A's expectation is registered (drops it
  otherwise), and B remains Busy, rejecting another `SendPrompt` as already in progress
  until B's own terminal source releases it.

### Cross-session completion during a main turn

- **Given**: main-session turn B is Busy and a completion scoped to a different session X
  reaches the bridge.
- **When**: the mediator receives X's scoped completion before B completes.
- **Then**: Cyril forwards exactly 1 scoped completion to X's routed consumer, forwards 0
  completions to the main-session consumer, and leaves B's guard, prompt work, and Busy
  state unchanged (split-brain fix: bridge guard and App routing agree).

### Synthesized global v1/v2 completion

- **Given**: v1/v2 prompt turn T is the active turn; its prompt response is synthesized
  into a global `RoutedNotification` (`session_id: None`) stamped with T's owner at
  dispatch.
- **When**: the mediator receives it while T owns Busy.
- **Then**: the id match releases T, forwards exactly 1 completion through the main
  pipeline, and accepts the next prompt. Because matching keys on the owner id — never a
  bare session id — the global signal can never be mistaken for unmatched and freeze a
  v1/v2 turn (the constraint named in the tracker), and a stamped result for an earlier
  owner forwards 0 completions.

### Cancellation

- **Given**: the terminal operator cancels active turn T.
- **When**: the bridge sends `session/cancel` targeting T's immutable owning session
  (unaffected by mid-turn `NewSession` retargeting).
- **Then**: T still releases via its first-arriving terminal source exactly once; both
  sources' stop reasons are recorded when both arrive (the live capture shows both report
  `cancelled`); cyril-a71q selects no reason precedence (cyril-pnwb). A late stamped
  completion for T cannot release a newer turn U. Terminal-child reap policy is unchanged
  (cyril-3lh8).

### Prompt error and connection death

- **Given**: active turn T encounters a prompt transport error or agent-process death
  before a terminal source released T.
- **When**: the existing cyril-l7tw lifecycle runs.
- **Then**: Cyril emits `BridgeError` → T-owned `TurnCompleted` → `BridgeDisconnected` in
  that order, exits the failed bridge lifetime, and accepts no further prompt in it. The
  deferred disconnect is keyed to T's owner: a stale stamped completion or absorbed
  companion cannot satisfy another owner's deferred disconnect.

### Shutdown and fresh process lifetime

- **Given**: `BridgeCommand::Shutdown` arrives with zero, one, or two bridge-owned prompt
  futures live (the active turn's, plus at most one companion-pending future from the
  immediately prior turn).
- **When**: the bridge aborts every bridge-owned prompt future and exits its run loop.
- **Then**: 0 additional `TurnCompleted` events are required, 0 bridge-owned futures
  survive `run_loop` exit (none may linger holding the connection — cyril-atjw), no event
  queued in the stopped bridge can affect a later fresh bridge, and the fresh process
  begins a new isolated ownership domain with a fresh allocator.

### Rate-limit consumer boundary (cyril-3zy4 restored)

- **Given**: cyril-3zy4 eventually surfaces a KAS `_kiro/error/rate_limit` observation
  during active turn T.
- **When**: rate-limit observations and T's terminal sources arrive in any order.
- **Then**: a rate-limit observation alone forwards 0 completions and releases 0 turns —
  but T still releases via its first-arriving terminal source (in the rate-limited case,
  typically the prompt response). **This restores cyril-j16p's non-blocking criterion**:
  a rate-limited turn releases the busy guard without cyril-3zy4 needing a new release
  mechanism. Payload conversion, message text, and retry policy remain cyril-3zy4.

### Ownership identity exhaustion

- **Given**: one bridge process has allocated all 2^64 `TurnId` values.
- **When**: another prompt is requested.
- **Then**: Cyril starts 0 new turns and 0 new prompt futures, surfaces exactly 1 visible
  fail-closed lifecycle signal, and requires a fresh bridge process; identities are never
  wrapped or reused.

### Unsupported duplicate scoped KAS `turn_end`

- **Given**: a KAS producer emits a second identity-free scoped `turn_end` for one
  accepted prompt, and it arrives while a newer same-session turn B is active with no
  matching companion expectation.
- **When**: that frame is observationally identical to B's own first `turn_end`.
- **Then**: the input is outside the supported producer contract (at most one scoped
  `turn_end` per accepted prompt). Cyril cannot distinguish it from B's frame and makes
  no detection, drop, or ownership-safety guarantee for it. (If it arrives while a
  companion expectation for its session dangles, or while no turn is active, it is
  absorbed/dropped harmlessly — but this is best-effort, not a guarantee.)

## Success criteria

- **Same-session stamped isolation**: 4/4 assertions pass in scripted
  `A releases → B starts → late stamped A → owned B` traces — a late id-stamped v1/v2
  result and a late id-stamped KAS response each produce 0 App completions and 0 releases
  of B; B remains guarded after each; B's own terminal source produces exactly 1
  completion — measured by a deterministic bridge-loop integration harness.
  This method cannot see: scheduler interleavings not represented by the scripted traces,
  identity-free duplicate scoped `turn_end`, or undocumented live-agent frames.
- **Receipt-order equivalence**: 3 order permutations of a KAS turn (turn_end→response,
  response→turn_end, single-source) each yield exactly 1 forwarded completion, the
  correct released owner, and the full set of arrived source/reason observations recorded
  — measured with a controlled in-process ACP/bridge harness and an observer-seam ledger.
  This method cannot see: which recorded reason cyril-pnwb should later make
  authoritative, or production schedules producing receipt orders outside the harness.
- **Companion absorption**: in the normal-order trace, the absorbed response forwards 0
  completions and its {source, reason} tuple is present in the ledger; in the
  inverted-order trace, the absorbed `turn_end` likewise; in the drift trace (response
  only, then next turn), the next turn's `turn_end` is absorbed by the dangling
  expectation and the next turn still completes exactly once via its stamped response —
  6/6 assertions, same harness.
  This method cannot see: producer behavior beyond the modeled drift shape.
- **Cross-session routing**: 3/3 routing assertions pass for a foreign scoped completion
  during main turn B — 1 event reaches X's routed consumer, 0 events reach main, B
  remains guarded — measured by bridge/App routing integration tests.
  This method cannot see: future consumers that bypass `RoutedNotification` or mutate
  Busy state outside the tested pipeline.
- **v1/v2 compatibility**: the id-stamped global completion releases its owner and
  forwards exactly 1 completion in an end-to-end v2 harness turn; a stale stamped global
  forwards 0 — no v1/v2 turn can freeze on session matching, proven by the absence of any
  session-id comparison on the global path.
  This method cannot see: live v2 wire drift.
- **Prompt-work resource bound**: across sampled transitions (idle, accepted KAS, both
  receipt orders, drift, accepted v1/v2, cancellation, failure/death, shutdown), at most
  2 bridge-owned prompt futures are live (active + companion-pending), 0 survive shutdown
  or `run_loop` exit, and every released owner's future is resolved, absorbed, or aborted
  — measured by task-drop sentinels and a mediator state-transition ledger.
  This method cannot see: leaked work outside bridge ownership or memory corruption.
- **Lifecycle interactions**: 8/8 ordered assertions across cancel-with-late-companion,
  prompt error, mid-turn death, and shutdown: error/death contain exactly 1 owner-keyed
  completion in cyril-l7tw order; a stale/companion signal cannot satisfy another owner's
  deferred disconnect; shutdown contains 0 completions and 0 surviving futures.
  This method cannot see: operating-system-specific death timing or uncaptured live
  cancellation schedules.
- **Rate-limit consumer contract**: 3/3 modeled assertions — a rate-limit observation
  alone forwards 0 completions and leaves T Busy; T's subsequent prompt response releases
  T exactly once (the restored non-blocking criterion); a later stale observation
  releases 0 turns — measured by a consumer-contract test without selecting cyril-3zy4's
  converter design.
  This method cannot see: the live `_kiro/error/rate_limit` payload, rendering, or retry
  timing.
- **Identity exhaustion safety**: 3/3 injected boundary assertions — the last unused
  `u64` identity allocates once; the next request starts 0 turns/futures; exactly 1
  visible fail-closed signal — via dependency-injected counter state.
  This method cannot see: a naturally elapsed 2^64-turn lifetime.
- **Maximum supported backlog**: 257/257 notifications accounted for in a
  paused-consumer trace (256-capacity channel + blocked terminal), with release and
  companion absorption applied in mediator receipt order after resume.
  This method cannot see: an indefinitely wedged consumer or live wire cardinality drift.
- **Workspace regression gate**: 100% of workspace tests pass with 0 clippy warnings and
  0 formatting differences (`cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`).
  This method cannot see: live wire drift or production scheduling.

## Edge cases and decisions

| Edge | Decision | Source |
| --- | --- | --- |
| Empty set (no active turn) | Drop an unowned global or same-session terminal observation unless it matches a dangling companion expectation (then absorb + record); forward a differently scoped completion to its routed consumer without creating or clearing a main turn. | `cyril-a71q`; scope-sensitive visibility answer 2026-07-12 (`"A"`, premise-independent, survives re-anchor) |
| Max scale | 2^64 distinct `TurnId` values per bridge lifetime; the next prompt fails closed, starting 0 turns and 0 futures. | exhaustion answer 2026-07-12 (`"A"`, survives re-anchor) |
| Null / missing field | The synthesized global v1/v2 completion has no session scope and must match by stamped owner id. A KAS `turn_end` with missing/unparseable `stopReason` still releases and defaults to `EndTurn`. No native KAS turn identifier may be assumed (prototype evidence). | `event.rs` global routing; `convert/kas.rs` fallback; `prototype.md` |
| Concurrent writes | The serialized mediator accepts at most 1 active turn. Receipt order is mediator dequeue order; under the researched wire order + same-thread FIFO enqueue, a turn's `turn_end` normally precedes its synthesized response, but correctness must not depend on it (receipt-order equivalence criterion). | `bridge.rs` ADR-0004; `timing-audit.md` §2 |
| Partial failure (one source arrives) | The arriving source releases; the companion expectation dangles and later absorbs at most one matching stale signal. Busy never persists past an available terminal source. | `timing-audit.md`; cyril-j16p liveness (`kas_turn_end_completes_without_prompt_response`; first-wins dedup) |
| Retries / idempotency | Each accepted turn reaches the App as at most 1 completion. A retry accepted after release is a new turn with a new owner; prior stamped results and companions cannot complete it. No idempotency claim for duplicate identity-free `turn_end`. | closed `cyril-j16p`; `cyril-a71q` |
| Multi-tenancy boundaries | ACP session scope is the isolation boundary: another session's completion is visible only to its routed consumer and cannot mutate the active main turn, guard, or prompt work. | `event.rs` routing contract; cross-session note in `cyril-a71q` |
| Same-session stale observation | An id-stamped completion for a released owner absorbs (if expected) or drops; it can never release a newer turn. Identity-free duplicate scoped `turn_end` remains excluded. | `cyril-a71q`; tracker note option (b) 2026-07-01 |
| Differently scoped terminal | Forward exactly once to that session's routed consumer; change no main state. | `cyril-a71q` cross-session note |
| KAS source disagreement | Record BOTH source/reason observations whenever both arrive, in either order (companion absorption records, never discards). The forwarded completion carries the first-arriving source's reason without claiming authority. Precedence remains cyril-pnwb. | tracker note (2) 2026-07-01; open `cyril-pnwb`; reverses choice-A discard |
| Cancellation | Cancel targets the active owner's immutable session; release still comes from the first terminal source; both reasons recorded. Reap policy unchanged. | `bridge.rs` CancelRequest arm; closed `cyril-3lh8` |
| Prompt error / engine death | `BridgeError` → owner-keyed `TurnCompleted` → `BridgeDisconnected`; deferred disconnect satisfied only by its owner's marker. | closed `cyril-l7tw` |
| Shutdown | Abort all (≤2) bridge-owned prompt futures, exit, require 0 completions; none survives `run_loop` exit holding the connection. | current `bridge.rs` Shutdown arm; cyril-atjw comment at `bridge.rs:662-665` |
| Channel capacity / backpressure | Bounded 256-notification channels with lossless awaited forwarding while receivers exist; release/absorption semantics apply in receipt order after backlog delay. | `bridge.rs` `NOTIFICATION_CAPACITY` |
| KAS scoped `turn_end` cardinality | Supported producer emits at most 1 scoped `turn_end` per accepted prompt (all captures agree). A second identity-free frame is unsupported drift with no ownership-safety guarantee. | decision 1A 2026-07-12 (premise-independent, survives re-anchor); `design-superseded-sole-turn-end.md` Blocker 1 |
| Companion ledger size | At most 1 expected-companion entry (the immediately prior released turn's). Registering a new expectation replaces a dangling one. | tracker note option (b); researched ordering makes deeper history unreachable in supported input |

## Out of scope

This change does NOT include:

- selecting `turn_end` or the prompt response as the authoritative KAS `stop_reason`
  (including cancellation disagreement) — cyril-pnwb; this spec only guarantees both
  observations are recorded when both arrive;
- detecting or defending against a second identity-free scoped `turn_end` for one
  accepted prompt;
- `_kiro/error/rate_limit` conversion, message wording, or retry timing — cyril-3zy4
  (whose busy-release requirement this spec restores rather than revises);
- reordering streamed agent content relative to completion — cyril-9akh;
- reconnect/respawn UX or preserving sessions across bridge restart — cyril-gua0;
- changing Enter-while-Busy/steering UX, cancellation target policy, or terminal-child
  reap semantics;
- choosing the event-variant shape, counter layout, registry shape, or other
  implementation design (design phase);
- aborting or racing the in-flight prompt RPC on `turn_end` receipt (explicitly voided
  choice-A behavior — the response resolves naturally and is absorbed).

## Constraints

| Dimension | Limit | How measured |
| --- | --- | --- |
| Concurrent active prompt turns | At most 1 per bridge process | Dual-`SendPrompt` test; second receives the Busy error |
| Bridge-owned live prompt futures | At most 2 (active + companion-pending); 0 after shutdown/`run_loop` exit | Task-drop sentinels across the transition ledger |
| Ownership domain | 2^64 identities per bridge lifetime; 0 reuses | Counter boundary injection, fail-closed assertion |
| Forwarded completions | At most 1 per accepted turn | Event reconciliation across all order permutations |
| Companion ledger | At most 1 entry; absorbs at most 1 signal per released turn | Harness ledger assertions |
| Source/reason observations | At most 2 per turn (one per source); every arrived source recorded, in any order | Observer-seam ledger, both order permutations |
| Supported scoped `turn_end` per prompt | At most 1; a second frame has 0 guarantees | Fixture classification + assertion inventory |
| Missing-source liveness | 0 turns left Busy when any terminal source for them has been received | Single-source harness traces |
| Notification channels | 256 per bounded channel | Capacity constant + 257-event paused-consumer harness |
| Added terminal delay | 0 intentional wait intervals on any release path | Harness asserts immediate completion without advancing a timeout |

## Decisions

| # | Decision | Source | Why |
| --- | --- | --- | --- |
| 1 | Ownership is per accepted turn (`TurnId(u64)` stamped at dispatch) and trustworthy across sessions — never a bare session-id comparison. | Rivets `cyril-a71q`; workflow decision 2026-07-12 (survives) | A bare session guard cannot isolate foreign or delayed stamped events from a newer turn. |
| 2 | First-source-wins release is retained for KAS; the second source is an absorbed, recorded companion. | cyril-j16p; `bridge.rs:1639-1646` design intent; `timing-audit.md` | Liveness must not depend on which signal arrives; the shipped dedup was a robustness property, not a defect. |
| 3 | The KAS producer contract is anchored on researched ordering: one scoped `turn_end` then one RPC response per accepted prompt, both normally present. Single-source turns are handled degenerate-safely, not modeled as normal input. | live capture `kas-live-session-trace-2.11.0.jsonl`; `timing-audit.md` §1–2 | The "either source absent indefinitely" space was synthetic; anchoring on evidence dissolves the superseded design's impossibility. |
| 4 | Wire `turn_end` (unstampable) matches by scoped session against the active owner, with a one-entry expected-companion ledger absorbing the released turn's remaining signal. | tracker note options (a)/(b) 2026-07-01; prototype: no native turn id | Dual matching (id for synthesized, session+ledger for wire) covers both signals without wire identity. |
| 5 | Both sources' {source, stop_reason} are recorded whenever both arrive, in either order; the forwarded completion carries the first source's reason without authority. | tracker note (2); `cyril-pnwb`; reverses voided choice-A discard | cyril-pnwb cannot decide precedence over evidence that was systematically discarded on every real turn. |
| 6 | A rate-limited or otherwise `turn_end`-less turn releases via its response; rate-limit observations alone release nothing. | `cyril-3zy4` P2 requirement; cyril-j16p non-blocking criterion | Restores the product requirement the voided contract demanded be revised. |
| 7 | Exhausting the `u64` domain fails closed and requires a fresh bridge process. | exhaustion answer 2026-07-12 (survives) | Reuse would destroy the ownership guarantee. |
| 8 | Error/death ordering remains `BridgeError` → owner-keyed `TurnCompleted` → `BridgeDisconnected`; deferred disconnect is owner-keyed. | closed `cyril-l7tw` | Ownership must preserve the visible fail-stop contract. |
| 9 | Shutdown aborts all bridge-owned prompt futures (≤2) with no required completion; restart creates an isolated domain. | current `bridge.rs`; cyril-atjw; `cyril-gua0` | An orphaned companion-pending future must not hold the connection past `run_loop` exit. |
| 10 | Cross-session scoped completions forward once to their routed consumer and never mutate main state. | cross-session answer 2026-07-12 (survives) | Session isolation without erasing legitimate secondary-session events. |
| 11 | At most 1 scoped `turn_end` per accepted prompt is the supported producer contract; duplicates are unsupported drift with no safety claim. | decision 1A 2026-07-12 (survives re-anchor) | Identity-free duplicate and owned frames are observationally identical; only producer cardinality bounds it. |
| 12 | The voided choice-A behaviors — sole-`turn_end` release authority, abort-on-`turn_end`, post-`turn_end` response discard, Busy-forever on missing `turn_end`, and the demanded revision of cyril-3zy4 — are superseded and no longer normative. Prior consequence sign-offs predicated on them are void. | `timing-audit.md`; requester takeover direction 2026-07-12 ("Start the re-anchored spec") | Their shared premise (either-source/indefinite-absence input space) contradicts the wire research. |

## Gap audit

No unanswered behavioral, measurement, edge-case, scope, or constraint gap remains under
the re-anchored contract. Stop-reason precedence is a deliberate cyril-pnwb exclusion;
duplicate scoped `turn_end` is an explicit unsupported-input boundary; the
absorb-versus-release rule when a dangling expectation and an active same-session owner
both match a `turn_end` is pinned (absorb first — observationally equivalent under
supported input, degrade-safe under drift); consequence sign-off for this re-anchored
contract is pending below.

## Sign-off

Consequences stated to the requester:

For the terminal operator, cyril keeps the shipped liveness behavior: a KAS turn unlocks
input at the FIRST terminal signal cyril receives — normally the scoped `turn_end`, with
the RPC response arriving immediately after and being silently absorbed as evidence. If a
producer ever completes a turn with only one signal (e.g. a rate-limited turn that only
resolves the RPC), input still unlocks; nothing in this feature can leave you frozen Busy
while a terminal signal for your turn has arrived. What changes is safety: every prompt
gets a process-unique owner id, so a slow or stale completion from an earlier turn — or a
completion belonging to a different session — can no longer end the turn you are
currently running. A duplicate identity-free `turn_end` from a drifted producer remains
undetectable and is the one named unsafety.

For the maintainer, the in-flight prompt RPC is never aborted on `turn_end`; it resolves
naturally and its stop reason is recorded next to `turn_end`'s, in whichever order they
arrive, so cyril-pnwb inherits complete evidence. cyril-3zy4's busy-release requirement
stands restored. Up to two bridge-owned prompt futures may briefly coexist (the active
turn's and the just-released turn's companion-pending future); shutdown aborts both and
none may outlive the run loop. The `u64` owner space fails closed at exhaustion.

Named oracle blindness: (1) scripted traces cannot see unscripted schedules, undocumented
frames, or duplicate scoped `turn_end`; (2) routing tests cannot see consumers bypassing
`RoutedNotification`; (3) receipt-order fixtures cannot choose cyril-pnwb precedence or
predict production receipt order; (4) cardinality fixtures cannot prove a live producer
honors at-most-one; (5) task sentinels cannot see work outside bridge ownership;
(6) lifecycle fixtures cannot see OS death timing; (7) rate-limit fixtures cannot see the
live payload or retry UX; (8) counter injection cannot see a natural 2^64 lifetime;
(9) the 257-event fixture cannot see an indefinitely wedged consumer or live wire drift;
(10) workspace gates cannot see production scheduling; (11) the two-turn live capture
cannot prove every KAS version preserves the turn_end-then-response order — which is why
correctness is required to be order-independent.

Visible absences: this change will not detect duplicate identity-free `turn_end`; choose
displayed stop-reason precedence; add rate-limit message/retry UX; reorder content that
trails completion; reconnect the agent automatically; or change steering, cancellation
targeting, and terminal-child reap behavior.

The requester's verbatim reply to this consequence statement: **"I confirm these consequences"** (2026-07-12). **SPEC GATE PASSED.**

Date: 2026-07-12
