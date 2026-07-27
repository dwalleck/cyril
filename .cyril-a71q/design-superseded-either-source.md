# Falsifiable design — cyril-a71q

Date: 2026-07-12

## DESIGN GATE FAILED

No production architecture is selected. The signed KAS contract is internally unimplementable with the observed ACP input: a KAS `turn_end` has session scope and reasons but no native turn identity, while the contract requires a first source to release immediately, permits either source to be absent indefinitely, permits a newer same-session turn to start, and requires a later old source not to release that newer turn. FIFO, timeout, and source-preference guesses are forbidden.

The cheapest falsifier proves that two production histories present the same complete observer input and require opposite outputs:

1. A's global prompt response completes A; B starts; the next `sess_main` KAS `turn_end` is A's late second source and must be dropped.
2. A's global prompt response completes A; A's KAS source is absent indefinitely; B starts; B's global response is also absent; the next identical `sess_main` KAS `turn_end` is B's first source and must complete B.

The observer sees the same active owner B, completed prompt-response history for A, source, session, and reason in both histories. Hidden ownership is the only difference, and the wire does not carry it. `DROP` violates history 2 liveness, `COMPLETE` violates history 1 safety, and `WAIT` violates first-source release plus indefinite-absence liveness.

### Exact resume condition

Design may resume only if one of these facts changes:

1. **Trustworthy correlation:** KAS/ACP supplies or normatively guarantees a correlation value that binds every terminal source, especially wire `turn_end`, to the accepted prompt turn; or
2. **Contract relaxation:** the signed requirement that the two KAS sources work in either order with either source indefinitely absent is relaxed enough to permit serialization/reconciliation before accepting the next turn.

A timeout, FIFO assignment, “next event belongs to active,” or preferred-source rule is not a resume condition because each merely chooses which signed trace to break.

## Purpose

The intended change is per-turn terminal ownership across engines and sessions: only an owned terminal observation may release a main turn, foreign scoped completion remains visible to its own consumer, and each accepted turn causes at most one applicable App completion. This document records why the complete signed behavior cannot currently be designed and preserves all obligations for a resumed design.

## Input shapes

Every production-reachable shape named by the specification and prototype is accounted for below.

| Shape | Required disposition | Claims |
| --- | --- | --- |
| v1/v2 successful synthesized prompt-response completion, global `session_id=None` | Bind to the originating accepted turn; forward once; release only that owner. | C2, C5, C9 |
| v1/v2 prompt-error synthesized completion, global | Preserve `BridgeError` before the owned completion; release once. | C2, C4, C5 |
| KAS scoped `turn_end` then global prompt response | Scoped first source releases once; the correlated global second source cannot affect a newer turn. | C1 gate; C8, C9 if correlation exists |
| KAS global prompt response then scoped `turn_end` | Global first source releases once; the uncorrelated scoped second source creates the proven contradiction after a newer same-session turn starts. | C1 |
| KAS scoped source present, prompt response absent indefinitely | Scoped source must release without waiting. | C1 |
| KAS prompt response present, scoped source absent indefinitely | Global source must release without waiting; a later new-turn scoped source must still work. | C1 |
| Both KAS sources absent | Emit no completion and retain Busy until another owned lifecycle path ends the turn; no invented timeout. | C1, C4 |
| `turn_end.stopReason` present and parseable | Preserve its exact source/reason. | C8 |
| `turn_end.stopReason` missing or malformed | Still treat as terminal and preserve the converter's `EndTurn` fallback with source identity. | C8 |
| Same-session late duplicate after newer turn B | Drop it; B stays guarded and its own source completes once. | C1, C3, C5 |
| Foreign-session scoped completion during main B | Forward once to the foreign routed consumer; main guard/session/UI are unchanged. | C3, C5 |
| No active turn + global or main-session terminal | Drop as unowned/stale; create no completion side effect. | C3, C5 |
| No active turn + foreign-session scoped completion | Forward once to that routed consumer without creating a main turn. | C3, C5 |
| Cancel T, with terminal source before U | Release T once; cancel targets T's immutable owner session. | C4, C8 |
| Cancel T, with duplicate after U starts | Drop T's later source; do not release U. | C1, C4 |
| Prompt transport error | `BridgeError` → one owned `TurnCompleted`; no silent failure. | C4, C5 |
| Mid-turn process death | `BridgeError` → one owned `TurnCompleted` → `BridgeDisconnected`; only T may satisfy T's pending disconnect. | C4 |
| Idle process death | No fabricated completion; visible disconnect. | C4 |
| Shutdown with active turn | Abort bridge-owned prompt work, exit, emit zero required completions. | C4, C9 |
| Fresh bridge process after shutdown/death | New isolated ownership domain; old channels cannot affect it. | C4, C6 |
| Rate-limit terminal outcome for T, then late T source during U | Owned outcome may release T; late T source cannot release U. Converter/payload choices remain in cyril-3zy4. | C3, C8 |
| Ownership allocator has an unused `u64::MAX` | Allocate that final distinct identity exactly once. | C6 |
| Allocation requested after `u64::MAX` was used | Start zero turns; emit one visible fail-closed lifecycle failure; never wrap. | C6 |
| 0–255 queued notifications | Preserve awaited, lossless forwarding while receivers exist. | C7 |
| 256 queued notifications plus a 257th terminal blocked behind them | On consumer resume, account for all 257 in channel order and reconcile final ownership. | C7 |

Strings and content blocks do not participate in ownership. Empty prompt rejection/content validation, Unicode, paths, wall-clock values, persistence, soft deletion, replication, and time zones cannot alter the accepted ownership decision and therefore do not create additional ownership shapes.

## Removed invariants

### Classification

This change is **subtractive**. Accepting a new turn while an earlier KAS turn may still have an outstanding terminal source removes the serialization invariant: **“no terminal source from an older accepted turn can coexist with a newer active turn.”**

That invariant previously supplied this chain for free: one prompt operation → one relevant terminal epoch → one session-valued guard → any observed completion may clear it → App/session/UI may commit completion. Removing it breaks every downstream assumption in that chain.

| Sweep target | Invariant removed or exposed | Still-holds obligation | Claim |
| --- | --- | --- | --- |
| `prompt_task` | A first KAS source can release A while A's prompt task remains alive; accepting B can replace the only stored handle and leave A's task outstanding. | Prompt work and shutdown/cancellation bookkeeping must be owner-keyed; an old task's response cannot acquire B's ownership, and shutdown terminates all bridge-owned prompt work. | C4, C9 |
| `turn_in_flight` | `Option<SessionId>` no longer uniquely identifies the terminal epoch once same-session turns overlap in source lifetime. | Busy/rejection/cancel/release decisions use an explicit accepted-turn owner, never session presence alone. | C2, C3, C9 |
| Completed-source history | “No active turn means duplicate” stops working once B is active; a session/source history without correlation cannot tell late A from owned B. | No finite or unbounded history of the observed fields is claimed to solve C1; with trustworthy correlation, disposition compares explicit event owner to active owner. | C1, C9 |
| Pending disconnect | Any completion used to satisfy the single pending death sequence. | A pending disconnect is keyed to the dying owner and fires only after that owner's terminal marker; a stale or foreign marker cannot trigger it. | C4 |
| Cancellation target | Session was sufficient while one terminal epoch existed. | Cancel targets the immutable session in the active owner record even after session changes; late cancellation sources retain T's owner. | C4 |
| App/session/UI effects | Every forwarded main completion was assumed applicable. | Only an owned main completion reaches these consumers; exactly once it commits streaming, records turn summary/cost, changes SessionStatus to Active, and changes UI activity to Ready. | C5 |
| Foreign-session UI | Bridge guard and App routing could disagree, producing split-brain. | Foreign scoped completion reaches only its subagent stream and never mutates main Busy/guard state. | C3, C5 |
| Source/reason preservation | First-arrival dedup erased which second source/reason existed. | The observation seam retains `source` and original/fallback `reason` independently of ownership disposition so cyril-pnwb can later decide reason authority. | C8 |
| Identity uniqueness | A wrapping counter would recreate an old terminal epoch. | All `2^64` values are used at most once per bridge lifetime, then allocation fails closed. | C6 |
| Channel availability | Ownership logic could be correct in memory but its terminal/disconnect notification could be lost at capacity. | Awaited forwarding remains lossless through the 256+1 boundary while receivers exist. | C7 |

## Architecture

### Current gate

No architecture over the currently observed KAS/ACP fields can meet C1. Adding a `TurnId` only to synthesized prompt responses is insufficient: the ambiguous source is wire `turn_end`. Stamping `turn_end` at receipt with the then-active owner is exactly the buggy “COMPLETE” policy. Assigning it to the oldest owner missing that source is FIFO and exactly the buggy “DROP” policy when the old source is absent. Waiting is exactly the forbidden serialization policy.

### Minimum architecture after trustworthy correlation exists

This is a boundary contract, not a selected implementation:

- A checked `TurnId(u64)` allocator creates one identity per accepted prompt and enters an explicit exhausted state after `u64::MAX`.
- An active record contains `{ owner, session, prompt task ownership, pending disconnect }`; cancel reads this record rather than `active_session_id`.
- Every terminal observation entering the mediator contains `{ owner, source, scope, reason }`. `source` distinguishes synthesized prompt response, KAS `turn_end`, transport-failure marker, and any cyril-3zy4 rate-limit terminal boundary.
- The mediator forwards/release-completes only when `observation.owner == active.owner`; a foreign scoped observation is routed without touching active ownership; all other observations are stale/unowned.
- Prompt-task handles are owner-keyed so accepting B cannot orphan A's handle and shutdown can abort all bridge-owned work.
- Source and reason remain represented at the observer seam. Any reason-precedence storage policy is governed by verified tracker cyril-pnwb.
- SessionController and UiState remain passive consumers: they receive only already-owned applicable main completions.

Without the resume condition, these types merely label synthesized sources and cannot create missing KAS ownership information.

## Claims

1. **C1 — impossibility:** With the observed no-turn-id KAS wire and the signed either-order/indefinite-absence liveness requirement, no deterministic observer can both drop a late A `turn_end` and complete an observationally identical B `turn_end`.
2. **C2 — global compatibility:** A synthesized global v1/v2 success or error terminal is owned by the prompt task that created it and releases exactly that accepted turn once.
3. **C3 — scope isolation:** A stale main-session terminal is dropped, a foreign scoped terminal is forwarded once, and neither foreign nor unowned input changes the active main owner.
4. **C4 — lifecycle ownership:** Cancel, prompt error, process death, shutdown, and a fresh bridge lifetime preserve their specified target, count, and ordering without allowing another owner to satisfy them.
5. **C5 — consumer effects:** Exactly one owned main completion causes exactly one SessionController/UI completion transition; stale and foreign completions cause zero main completion transitions.
6. **C6 — exhaustion:** The final unused `u64` owner is allocated once and the next request starts zero turns, emits one visible fail-closed lifecycle event, and requires a fresh ownership domain.
7. **C7 — backpressure:** A paused-consumer trace accounts for the 256-event capacity plus the blocked 257th terminal without loss, duplication, reordering, or incorrect final ownership while the receiver remains live.
8. **C8 — source/reason seam:** Ownership classification preserves terminal source and original or fallback reason, without deciding KAS stop-reason authority assigned to cyril-pnwb.
9. **C9 — explicit owner state:** After trustworthy terminal correlation exists, prompt-task bookkeeping, active guard, stale disposition, pending disconnect, and cancel targeting are all keyed by immutable turn owner rather than mutable session state or arrival order.

## Falsification

| # | Claim | Falsifier and falsifying result | Independent oracle | Concrete buggy implementation caught | Cost | Status | Regression fence / distinct output |
| --- | --- | --- | --- | --- | --- | --- | --- |
| C1 | KAS contract is observationally impossible | Construct the two hidden-owner histories above and project every production-visible field. If their visible inputs differ or they require the same disposition, C1 is false. | Artifact script excludes hidden owner from `Visible`; hidden fixture labels independently supply required disposition. | A mistaken design that assumes completed-source history disambiguates an untagged event. | <1s | **passed** | `.cyril-a71q/probes/design_indistinguishability.py`; `C1-PASSED` |
| C2 | Global v1/v2 ownership | In an in-process ACP fixture, accept A then B and deliver A's delayed synthesized response while B is active. Any release of B, or not exactly one completion on B's own response, falsifies C2. | Fake server request/response IDs and hidden accepted-turn ledger, not bridge state. | Stamping a global completion with the current owner at receive time. | 2m | blocked by C1 gate | bridge integration `global_response_keeps_origin_owner`; `C2-GLOBAL` |
| C3 | Scope isolation | During main B inject same-session stale, foreign X, and no-active global/main/foreign cases. Any main release outside B, loss of X, or main mutation from X falsifies C3. | Separate receiver-count ledger: expected main=0/X=1 before B, plus fake server proves a third prompt is rejected. | `if turn_in_flight.is_some() { clear }` regardless of owner/scope. | 3m | blocked by C1 gate | bridge/App integration `terminal_scope_matrix`; `C3-SCOPE` |
| C4 | Lifecycle ownership | Script cancel-before/after-U, prompt error, mid-turn and idle death, shutdown, and fresh bridge. Wrong cancel session, wrong event sequence/count, stale-triggered disconnect, surviving prompt work, or cross-lifetime event falsifies C4. | Fake agent wire transcript, process-kill signal, and ordered channel recorder. | Pending disconnect as `Option<String>` consumed by any completion; shutdown aborting only the newest overwritten handle. | 5m | blocked by C1 gate | lifecycle matrix `owned_terminal_lifecycle`; `C4-LIFECYCLE` |
| C5 | Exactly-once consumer effects | Snapshot public SessionController/UI observables before and after owned, stale, and foreign events. Any stale/foreign main summary/cost/status/activity change, or owned change count other than one, falsifies C5. | External expected-transition ledger and routed receiver identity. | Forwarding stale main completion and trusting consumers to deduplicate. | 4m | blocked by C1 gate | App integration `completion_effects_are_owner_filtered`; `C5-EFFECTS` |
| C6 | Fail-closed `u64` exhaustion | Inject allocator at final-unused and exhausted states. If `u64::MAX` is not allocated once, the next prompt reaches the fake server, identity wraps, or visible failure count is not one, falsify C6. | Arithmetic boundary fixture and fake server prompt count. | `wrapping_add`, `saturating_add`, or resetting the counter in the same bridge. | 1m | blocked by C1 gate | allocator test `turn_owner_exhaustion_fails_closed`; `C6-EXHAUSTION` |
| C7 | 256+1 lossless backpressure | Pause receiver, fill IDs 0..255, block terminal ID 256, resume, and reconcile ordered IDs plus owner state. Missing/duplicate/out-of-order ID or wrong guard falsifies C7. | Independently generated set/range `0..=256` and channel-order recorder. | `try_send` dropping the terminal at capacity or clearing ownership before delivery is durable. | 3m | blocked by C1 gate | bridge integration `owned_terminal_survives_256_backlog`; `C7-BACKPRESSURE` |
| C8 | Preserve source/reason without authority choice | Feed scoped/global same-turn sources in both orders with distinct reasons and missing/malformed scoped reason. If the observer loses source, rewrites a valid reason, drops the `EndTurn` fallback, or asserts precedence, falsify C8. | Captured frame fields plus separately encoded expected source/reason tuples. | Collapsing both sources immediately to bare `TurnCompleted { stop_reason }` before ownership observation. | 2m | blocked by C1 gate | converter/observer matrix `terminal_observation_preserves_source_reason`; `C8-SOURCE-REASON` |
| C9 | Explicit owner-keyed state | With correlated fixture IDs, interleave A outstanding task, B active, session switch, cancel, death, and shutdown. Any decision keyed to current session/order, overwritten live handle, or wrong pending owner falsifies C9. | Model transition table keyed only by fixture owner IDs and wire command transcript. | `Option<SessionId>` guard plus single replaceable `prompt_task`. | 5m | blocked by C1 gate | bridge state-machine test `owner_state_survives_interleavings`; `C9-OWNER-STATE` |

“Blocked by C1 gate” is a settled halt, not a promise of untracked work: production fences cannot honestly be built until one exact resume condition is met on target issue cyril-a71q.

## Cheapest falsifier result

Command recorded by the parent session:

```text
python .cyril-a71q/probes/design_indistinguishability.py > .cyril-a71q/probes/output/design-cheapest-falsifier.txt
```

Persisted output:

```text
C1-OBSERVATION-EQUALITY: True
C1-WORLD late_A_turn_end: visible=Visible(active_owner=2, active_session='sess_main', completed_sources=((1, 'prompt_response'),), event_scope='sess_main', event_source='kas_turn_end', event_reason='end_turn') hidden_owner=1 required=DROP_STALE
C1-WORLD B_turn_end_old_source_absent: visible=Visible(active_owner=2, active_session='sess_main', completed_sources=((1, 'prompt_response'),), event_scope='sess_main', event_source='kas_turn_end', event_reason='end_turn') hidden_owner=2 required=COMPLETE_B
C1-REQUIRED-DISPOSITIONS: COMPLETE_B,DROP_STALE
C1-DETERMINISTIC-OBSERVER-OUTPUTS: 1
C1-POLICY DROP_STALE: FAILS=B turn_end liveness when both old turn_end and B response are absent
C1-POLICY COMPLETE_B: FAILS=late_A_turn_end safety
C1-POLICY WAIT: FAILS=late_A_turn_end safety + B turn_end liveness when both old turn_end and B response are absent
C1-PASSED: identical production-visible input requires opposite outputs
DESIGN-GATE-FAILED: correlation or a contract relaxation is required
```

This is a passing falsifier for C1 and a failing design gate for the feature contract. Its output is non-vacuous: adding a trustworthy owner field to `Visible` would make the observations differ and fail the C1 equality premise.

## Material-boundary accounting

| Prototype boundary | Design accounting |
| --- | --- |
| Live KAS terminal wire shape | C1 uses all observed ownership-relevant leaves and the absence of a native turn ID; wire-version blindness is R1/R8. |
| KAS conversion/routing entry | C1, C3, C8, C9 cross converter → scoped RoutedNotification → mediator. |
| KAS dual-source liveness/dedup | C1 covers both orders and each absent-source shape and proves the contradiction. |
| Notification backpressure substrate | C7 covers 256+1. |
| Active-turn guard | Removed-invariant sweep plus C9 replaces session-only semantics only after correlation exists. |
| Completion release guard | C2, C3, C9 require owner equality rather than presence. |
| Global v1/v2 path | C2. |
| Scoped KAS path | C1, C3, C8. |
| App foreign-session boundary | C3 and C5. |
| Same-session stale ownership | C1, C3, C5. |
| Cross-session ownership | C3 and C5. |
| KAS duplicate ownership | C1 and C8. |
| Shutdown/process lifetime | C4 and C9. |
| Prompt error/process death | C4 and C5. |
| Cancellation | C4; reason authority remains verified tracker cyril-pnwb. |
| Rate-limit consumer | C3/C8 define only ownership consumption; payload/UI/retry behavior is verified tracker cyril-3zy4. |
| Ownership identity mechanism | C6/C9; impossible for untagged KAS input under C1. |
| Identity exhaustion | C6. |
| 257-notification backlog | C7. |

## Oracle blindness ledger

Every `This method cannot see:` sentence from the signed spec is carried here.

| ID | Erased/unseen difference | Disposition |
| --- | --- | --- |
| R1 | Scripted same-session tests cannot see scheduler interleavings not represented by the trace or undocumented live-agent frames. | Named accepted risk; C1 is stronger than a schedule sample because the two histories have identical complete observer input, while C2–C5 fences would cover named deterministic interleavings after resume. |
| R2 | Routing tests cannot see consumers that bypass `RoutedNotification` or mutate Busy outside the tested pipeline. | Named accepted risk; C5 plus source review fences current consumers, not unknown added bypasses. |
| R3 | KAS compatibility tests cannot see which disagreeing same-turn `stop_reason` should win. | Intentionally undecided and covered by verified tracker cyril-pnwb; C8 preserves the seam. |
| R4 | Lifecycle fixtures cannot see OS-specific process-death timing or a live KAS cancellation reason absent from the harness. | Named accepted risk; C4 uses deterministic death/cancel schedules and C8 does not infer authority. |
| R5 | Rate-limit contract tests cannot see the live `_kiro/error/rate_limit` payload, message rendering, retry timing, or the terminal boundary cyril-3zy4 chooses. | Covered by verified tracker cyril-3zy4; only owner consumption is asserted here. |
| R6 | Counter injection cannot see a naturally elapsed `2^64`-turn process lifetime or memory corruption outside the allocator. | Named accepted risk; C6 tests both exact injected boundary states. |
| R7 | The 257-event fixture cannot see an indefinitely wedged App consumer, unbounded external producer memory, or scheduling beyond bounded channels. | Named accepted risk; C7 is conditioned on a live receiver that resumes. |
| R8 | Workspace tests cannot see live v1/v2/KAS wire drift, production scheduler timing, or behavior excluded from workspace tests. | Named accepted risk; pinned captures and C1 establish only observed KAS 2.11.0 shape. |
| R9 | Sanitizing genuine `sessionId` values erases their original opaque values. | Separate same/foreign fixtures test equality semantics; accepted risk that an undocumented value encoding carries meaning beyond equality. |
| R10 | Two genuine KAS captures cannot prove every KAS version lacks a turn identifier. | This uncertainty is the trustworthy-correlation resume path, not evidence for a heuristic. |
| R11 | Lexical source inventory sees declarations/spellings, not runtime ordering, forwarding, or state effects. | Runtime C2–C9 falsifiers are required; lexical evidence alone proves none of those claims. |
| R12 | The hidden-owner oracle uses labels unavailable to production. | Deliberate and load-bearing: it independently states required outcomes; C1 compares only the label-free `Visible` projection. |
| R13 | Deterministic fixture normalization erases unscripted event timing and live wire drift. | R1/R8 name the accepted risk; no timing-based correctness claim is made. |

## Negative space

1. No FIFO assignment, timeout, source preference, “oldest missing source,” or “current active owner” stamping is accepted as ownership.
2. No production code, implementation slice, or budgeted plan is produced while the design gate is failed.
3. Stop-reason precedence is not selected; verified tracker cyril-pnwb owns that decision.
4. Rate-limit conversion, wording, retry policy, and terminal representation are excluded under verified tracker cyril-3zy4.
5. Streamed-content reordering is excluded under verified tracker cyril-9akh.
6. Reconnect/respawn UX and history preservation are excluded under verified tracker cyril-gua0.
7. Enter-while-Busy steering, cancellation target policy changes, and terminal-child reap changes are not reopened; cyril-2vcc and cyril-3lh8 are closed records of those behaviors.

## Tracker audit

The repository-local `.rivets/issues.jsonl` was read and each reference below was verified for existence and coverage:

- **cyril-a71q** — open target; covers per-turn ownership, same-session stale completion, cross-session guard corruption, global v1/v2 compatibility, and the untagged KAS seam.
- **cyril-j16p** — closed; covers current KAS first-source liveness and two-source dedup.
- **cyril-pnwb** — open; covers KAS source disagreement and stop-reason authority.
- **cyril-3zy4** — open; covers live KAS rate-limit conversion, rendering/retry behavior, and terminal release interaction.
- **cyril-l7tw** — closed; covers `BridgeError` → `TurnCompleted` → `BridgeDisconnected` and full-channel disconnect delivery.
- **cyril-9akh** — open; covers streamed notifications trailing completion.
- **cyril-gua0** — open; covers respawn/reconnect and fresh bridge/session UX.
- **cyril-3lh8** — closed; covers cancellation terminal-child reaping.
- **cyril-2vcc** — closed; covers visible Busy/input behavior.

No phantom reference was found. No new technical-debt promise is created: the halt and both resume alternatives remain on target cyril-a71q.

## Self-review

- **Claim count:** 9, within the required 3–15.
- **Input coverage:** every requested production shape maps to at least one claim; the signed KAS response-first/late-scoped pair maps to C1 and cannot pass.
- **Removed-invariant coverage:** prompt tasks, active guard, source history, pending disconnect, cancellation target, App/session/UI effects, source/reason preservation, identity uniqueness, and backpressure are each swept and claimed.
- **Falsifier independence:** every row names an oracle outside the implementation decision under test.
- **Non-vacuity:** every row names a concrete buggy implementation; C1 would fail if trustworthy correlation made the visible inputs differ.
- **Distinctness:** each claim has a unique `C#-*` output label.
- **Cost:** all proposed fences are deterministic and five minutes or less; no empirical-only or manual merge fence is used.
- **Material boundaries:** all 19 prototype inventory rows are mapped to claims or named blindness.
- **Blindness:** all eight signed `cannot see` statements and five prototype/oracle normalizations are recorded.
- **Negative space:** seven explicit exclusions; tracker-owned exclusions cite verified IDs.
- **Gate result:** cheapest falsifier passed C1, thereby proving the signed input/output contract has no implementable observer on the observed wire. Design remains halted.
