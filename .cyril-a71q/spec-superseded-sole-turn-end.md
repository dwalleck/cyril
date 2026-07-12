# Feature: Per-turn terminal ownership across sessions and engines

## What this is

Cyril will associate every accepted prompt turn with an observable ownership boundary so that a late owner-stamped or foreign terminal observation cannot end a different active turn. The supported KAS input space permits at most one session-scoped `session_info_update.kind == "turn_end"` frame per accepted KAS prompt; that frame is the sole ordinary KAS release authority, and Cyril aborts or drops the owner's unresolved prompt-response work when the serialized bridge mediator receives it. A duplicate identity-free scoped KAS `turn_end` is unsupported upstream protocol drift that this ownership guarantee can neither detect nor defend against.

## Users

- **Terminal operator**: sends prompts, cancels work, switches sessions, and needs Busy/Idle state and input availability to correspond to the turn actually running.
- **Cyril protocol maintainer**: consumes this contract when maintaining terminal-producing paths such as KAS rate-limit handling and needs the supported frame cardinality, release authority, evidence cutoff, and resource bound to be explicit.

## Behavior

### Same-session late owner-stamped observation

- **Given**: turn A in session S ended, turn B in S was subsequently accepted, the toolbar/input observably show B as Busy, and either an owner-stamped global v1/v2 result for A or a KAS prompt response already queued for A remains unobserved by the bridge mediator.
- **When**: the mediator receives that A observation after B became active.
- **Then**: Cyril forwards 0 completions for A, releases 0 turns, records 0 late KAS source/reason evidence, B remains Busy and rejects another `SendPrompt` as already in progress, and B's supported owned terminal path later produces exactly 1 main-session completion.

### Cross-session completion during a main turn

- **Given**: main-session turn B is Busy and a completion scoped to different session X reaches the bridge.
- **When**: the mediator receives X's completion before B completes.
- **Then**: Cyril forwards exactly 1 scoped completion to X's routed consumer, forwards 0 completions to the main-session consumer, and leaves B's bridge guard, prompt future, and Busy state unchanged until B's own supported completion path.

### Synthesized global v1/v2 completion

- **Given**: a v1/v2 prompt turn T is the accepted active turn and its prompt response is converted to a global `RoutedNotification` whose `session_id` is `None` and whose owner was stamped at dispatch.
- **When**: the mediator receives that synthesized `TurnCompleted` while T owns Busy.
- **Then**: Cyril forwards exactly 1 completion through the main pipeline, clears only T's guard, resolves or drops T's prompt work, and accepts the next prompt; an owner-stamped result for an earlier turn forwards 0 completions and clears 0 guards.

### KAS authoritative release and prompt-work cutoff

- **Given**: accepted KAS turn T is Busy, no scoped `turn_end` for T has yet been received, and the supported input contains at most 1 such frame for T.
- **When**: the serialized mediator receives T's scoped `session_info_update.kind == "turn_end"`.
- **Then**: Cyril forwards exactly 1 applicable App completion, releases T without an intentional wait interval, aborts or drops any unresolved prompt-response future owned by T, retains only KAS source/reason observations received before this `turn_end`, and accepts the next prompt with 0 live prompt futures left from T.

### KAS deterministic response-before-turn_end order

- **Given**: accepted KAS turn T is Busy and its global prompt response has become the next observation received by the serialized mediator before T's scoped `turn_end`.
- **When**: the mediator receives the response and later receives the authoritative `turn_end`.
- **Then**: the response forwards 0 completions and releases 0 turns but preserves exactly 1 prompt-response source/reason observation for cyril-pnwb; T remains Busy until `turn_end`, which then forwards exactly 1 completion and releases T without choosing reason precedence.

### KAS deterministic turn_end-before-response order

- **Given**: accepted KAS turn T is Busy and its scoped `turn_end` becomes the next observation received by the serialized mediator before a prompt response, including a response already queued elsewhere but not yet received by the mediator.
- **When**: the mediator receives `turn_end` and a response later arrives or becomes observable.
- **Then**: `turn_end` forwards exactly 1 completion, releases T, and aborts or drops T's prompt-response work; the later response is discarded, forwards 0 completions, releases 0 turns, and supplies 0 prompt-response source/reason observations.

### Missing KAS turn_end

- **Given**: accepted KAS turn T is Busy and the supported producer emits 0 scoped `turn_end` frames for T.
- **When**: T's prompt response is received or remains absent.
- **Then**: a received response forwards 0 completions and preserves its source/reason while T is still active, and T remains Busy until an existing owned failure, process-death, or disconnect lifecycle ends the bridge turn, or shutdown exits the bridge; the response alone admits 0 later prompts.

### Cancellation and late observations

- **Given**: the terminal operator cancels active turn T and the bridge targets T's immutable owning session.
- **When**: cancellation, a response, an authoritative scoped KAS `turn_end`, or an existing failure/death/disconnect lifecycle is received, and a newer turn U may be accepted only after T releases.
- **Then**: cancellation targets only T and never adds a KAS release authority; for supported KAS input, T releases through its sole scoped `turn_end` or an existing failure/death/disconnect lifecycle, while prompt work remains bounded to T and is controlled by cancellation until it resolves or is aborted/dropped. An owner-stamped late v1/v2 result or already-queued KAS response for T forwards 0 completions and cannot release U; cyril-a71q selects no cancellation reason precedence.

### Prompt error and connection death

- **Given**: active turn T encounters a prompt transport error or agent-process death before an authoritative completion has released T.
- **When**: the existing cyril-l7tw lifecycle is received by the mediator.
- **Then**: Cyril controls and drops T's prompt work, forwards exactly 1 owned completion, preserves observable order `BridgeError` → `TurnCompleted` → `BridgeDisconnected`, exits that failed bridge lifetime, and permits no stale owner-stamped result or queued KAS response to satisfy another owner's deferred disconnect.

### Shutdown and fresh process lifetime

- **Given**: zero or one bridge-owned prompt future exists when `BridgeCommand::Shutdown` is received.
- **When**: the bridge aborts that future and exits its run loop.
- **Then**: shutdown requires 0 additional `TurnCompleted` events, leaves 0 bridge-owned live prompt futures, no event queued in that stopped bridge can affect a later fresh bridge process, and the fresh process begins a new isolated ownership domain.

### Rate-limit consumer boundary

- **Given**: cyril-3zy4 eventually observes a KAS rate-limit outcome during active turn T.
- **When**: one or more rate-limit observations or a prompt response for T are received before or after T's supported release path.
- **Then**: each rate-limit observation forwards 0 KAS completions, releases 0 turns, and does not create another prompt future; before `turn_end`, a prompt response follows the response-before-turn_end rule, while after `turn_end` it is discarded. Only T's sole supported scoped `turn_end` or an existing failure/death/disconnect lifecycle ends T; payload conversion, message text, retry policy, and any future mapping to a failure lifecycle remain cyril-3zy4 decisions.

### Ownership identity exhaustion

- **Given**: one bridge process has allocated all 18,446,744,073,709,551,616 distinct values in its `u64` ownership domain.
- **When**: another prompt is requested.
- **Then**: Cyril starts 0 new turns and 0 new prompt futures, surfaces exactly 1 visible fail-closed lifecycle signal, and requires a fresh bridge process rather than reusing an identity.

### Unsupported duplicate KAS turn_end

- **Given**: a KAS producer emits a second identity-free scoped `session_info_update.kind == "turn_end"` for one accepted prompt.
- **When**: that duplicate is observationally identical to a newer same-session turn's own `turn_end`.
- **Then**: the input is outside the supported KAS protocol space, and this feature makes no detection, drop, completion, liveness, or ownership-safety guarantee for that frame.

## Success criteria

- **Same-session stamped isolation**: 4/4 assertions pass in the scripted `A releases → B starts → late A → owned B` traces—an owner-stamped late global v1/v2 result and an already-queued KAS response each produce 0 App completions and 0 releases of B, B remains guarded after each, and B's owned terminal path produces exactly 1 completion—measured by a deterministic bridge-loop integration harness.  
  This method cannot see: scheduler interleavings not represented by the two scripted late-observation traces, identity-free duplicate scoped KAS `turn_end`, or undocumented live-agent frames.
- **Cross-session routing**: 3/3 routing assertions pass for a foreign scoped completion during main turn B—1 event reaches X, 0 events reach main, and B remains guarded—measured by bridge/App routing integration tests followed by B's supported owned completion.  
  This method cannot see: future consumers that bypass `RoutedNotification` or mutate Busy state outside the tested pipeline.
- **Engine terminal compatibility and receipt order**: 12/12 assertions pass across 6 traces—v1/v2 owned response emits 1 completion and releases; KAS response→`turn_end` emits 1 total completion, preserves 1 response evidence observation, and releases only at `turn_end`; KAS `turn_end`→response emits 1 total completion, preserves 0 response evidence observations, and discards the response; KAS response-only emits 0 completions, preserves 1 evidence observation, and stays Busy; KAS `turn_end`-only emits 1 completion, preserves 0 response evidence observations, and leaves 0 old prompt futures; owner-stamped stale v1/v2 emits 0 completions and 0 releases—measured with a controlled in-process ACP/bridge harness and observer-seam source/reason ledger.  
  This method cannot see: which pre-`turn_end` KAS `stop_reason` cyril-pnwb should later make authoritative, production schedules that produce a different mediator receipt order, or excluded duplicate KAS `turn_end` frames.
- **KAS cardinality support boundary**: 3/3 fixture classifications pass—0 and 1 scoped `turn_end` frame per accepted KAS prompt are classified supported, while 2 frames for one prompt are classified unsupported protocol drift with 0 ownership-safety assertions applied to the duplicate—measured by a fixture-schema contract test and assertion inventory review.  
  This method cannot see: whether a live or future KAS producer honors the normative at-most-one contract, because identity-free frames cannot prove producer cardinality.
- **Prompt-work resource bound**: 8/8 sampled state transitions report no more than 1 bridge-owned live prompt future—idle, accepted KAS, response-before-turn_end, turn_end-before-response, accepted v1/v2, cancellation, failure/death, and shutdown—and every post-release state reports 0 futures from the released owner, measured by task-drop sentinels and a mediator state-transition ledger.  
  This method cannot see: leaked work outside bridge ownership, executor bookkeeping not exposed by the sentinels, or memory corruption.
- **Lifecycle interactions**: 8/8 ordered assertions pass across 4 scripted cases—cancel with a queued late response, prompt error, mid-turn connection death, and shutdown; cancellation adds 0 release sources and the queued response releases 0 turns, error/death each contain exactly 1 owned completion in cyril-l7tw order, and shutdown contains 0 completions and 0 surviving prompt futures—measured by event-sequence assertions and task-drop sentinels.  
  This method cannot see: operating-system-specific process-death timing, a live KAS cancellation reason absent from the harness, or cyril-pnwb's future reason policy.
- **Rate-limit consumer contract**: 3/3 modeled assertions pass—a rate-limit observation alone emits 0 KAS completions, leaves T Busy, and creates 0 prompt futures; a later rate-limit observation releases 0 turns; and a prompt response received after authoritative `turn_end` supplies 0 evidence and releases 0 turns—measured by a consumer-contract test that injects observations without selecting cyril-3zy4's converter design.  
  This method cannot see: the live `_kiro/error/rate_limit` payload, message rendering, retry timing, or how cyril-3zy4 might map the outcome onto an existing failure/death/disconnect lifecycle.
- **Identity exhaustion safety**: 3/3 injected boundary assertions pass—the last unused `u64` identity is allocated once, the following request starts 0 turns and 0 prompt futures, and exactly 1 visible fail-closed lifecycle signal is emitted—measured by dependency-injected counter-state tests.  
  This method cannot see: a naturally elapsed 18,446,744,073,709,551,616-turn process lifetime or memory corruption outside the ownership allocator.
- **Maximum supported backlog**: 257/257 notifications are accounted for in a trace containing the 256-notification channel capacity plus the terminal observation blocked behind it, and 2/2 response/`turn_end` queue orders produce the specified evidence cutoff, measured by a paused-consumer bridge harness that resumes and reconciles every routed event, mediator receipt order, and final ownership state.  
  This method cannot see: an indefinitely wedged App consumer, unbounded external producer memory, scheduling beyond the bounded channels, or live wire cardinality drift.
- **Workspace regression gate**: 100% of workspace tests pass with 0 clippy warnings and 0 formatting differences, measured by `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.  
  This method cannot see: live v1/v2/KAS wire drift, production scheduler timing, unsupported duplicate KAS `turn_end`, or behavior excluded from workspace tests.

## Edge cases and decisions

| Edge | Decision | Source |
| --- | --- | --- |
| Empty set (no active turn) | Drop an unowned global or same-session terminal observation; preserve a differently scoped session completion for its routed consumer without creating or clearing a main turn. A duplicate KAS `turn_end` remains outside the supported input contract rather than becoming safely classifiable when no owner exists. | `cyril-a71q`; scope-sensitive visibility gap answer, 2026-07-12 (`"A"`); requester decision 1A, this session (`"1A, 2A"`) |
| Max scale | The ownership domain contains 18,446,744,073,709,551,616 distinct `u64` values per bridge lifetime; the next prompt fails closed and starts 0 turns and 0 futures. | `cyril-a71q` note; exhaustion gap answer, 2026-07-12 (`"A"`) |
| Null / missing field | A synthesized global v1/v2 completion with no session scope must own its dispatch-stamped turn. A KAS `turn_end` with missing/unparseable `stopReason` remains the one supported ordinary completion and defaults to `EndTurn`; no native KAS turn identifier may be assumed. | `cyril-a71q`; `event.rs` global routing; `convert/kas.rs` fallback; `prototype.md` genuine wire evidence |
| Concurrent writes | The serialized mediator accepts at most 1 active prompt turn and owns at most 1 live prompt future. Receipt order is the order in which the mediator dequeues observations for handling; wire timestamps, producer readiness, and evidence queued elsewhere do not override it. | `bridge.rs` ADR-0004 comments and current guard; requester decision 2A, this session (`"1A, 2A"`) |
| Permission denied / unauthenticated | Ownership does not reinterpret refusal/auth outcomes. If the existing failure lifecycle ends the bridge turn before KAS `turn_end`, preserve cyril-l7tw visibility/order, drop the owner's work, and do not admit a newer turn in that failed bridge lifetime. | closed `cyril-l7tw`; ownership-only gap correction, 2026-07-12 |
| Partial failure (one of N succeeded) | A KAS response received before scoped `turn_end` emits 0 completions, ends its prompt future, preserves 1 source/reason observation, and leaves Busy. Scoped `turn_end` releases immediately and aborts/drops unresolved prompt work; a response received afterward is discarded with 0 evidence. With no `turn_end`, Busy persists until an existing failure/death/disconnect lifecycle or shutdown. | requester decision 2A, this session (`"1A, 2A"`); closed `cyril-j16p`; `design.md` Blocker 2 |
| Retries / idempotency | Each accepted turn in the supported input space reaches the App as at most 1 applicable completion. A retry accepted after release is a new turn; owner-stamped prior results and queued prior KAS responses cannot complete it. No idempotency guarantee is claimed for an excluded duplicate KAS `turn_end`. | closed `cyril-j16p`; `cyril-a71q`; requester decisions 1A and 2A, this session (`"1A, 2A"`) |
| Soft-deleted records | Not applicable: the bridge lifecycle contains no persisted or soft-deleted turn records. | `bridge.rs` process-local state |
| Multi-tenancy boundaries | ACP session scope is the isolation boundary: another session's completion remains visible only to that routed consumer and cannot mutate the active main turn, guard, or prompt future. | `event.rs` routing contract; `cyril-a71q`; scope-sensitive visibility gap answer |
| Time-zone / DST | Not applicable: ownership, evidence cutoff, and receipt order use no wall-clock or calendar value. | `bridge.rs`; requester decision 2A, this session (`"1A, 2A"`) |
| Replication lag | Not applicable: the mediator and channels are process-local; no replicated store participates. | `bridge.rs` channel architecture |
| Cache invalidation | Not applicable: no cache supplies ownership truth. A fresh bridge process creates an isolated ownership domain rather than restoring cached identities or prompt work. | `bridge.rs`; `cyril-gua0`; exhaustion gap answer |
| Same-session stale observation | Drop an owner-stamped late global v1/v2 result and any KAS prompt response received after its authoritative `turn_end`; neither can release a newer turn. This claim excludes identity-free scoped KAS `turn_end` duplicates. | `cyril-a71q`; requester decisions 1A and 2A, this session (`"1A, 2A"`) |
| Differently scoped terminal | Forward exactly once to that session's routed consumer and do not change the active main turn, bridge busy guard, or prompt future. | `cyril-a71q` cross-session note; scope-sensitive visibility gap answer, 2026-07-12 |
| KAS source disagreement | Preserve prompt-response source/reason only if the mediator receives it before authoritative `turn_end`; preserve the `turn_end` source/reason itself; discard a prompt response received after `turn_end`. Stop-reason precedence remains undecided. | requester decision 2A, this session (`"1A, 2A"`); open `cyril-pnwb` |
| Cancellation | Cancel targets the active owner's immutable session and controls only that owner's prompt work. It adds 0 ordinary KAS release sources; supported release still requires the sole scoped `turn_end` or an existing failure/death/disconnect lifecycle. Terminal-child reap policy remains unchanged. | `bridge.rs` CancelRequest arm; closed `cyril-3lh8`; requester decision 2A, this session (`"1A, 2A"`); open `cyril-pnwb` |
| Prompt error / engine death | Before authoritative release, preserve `BridgeError` → owned `TurnCompleted` → `BridgeDisconnected`, control/drop the owner's prompt work, and end that bridge lifetime; a stale stamped result or queued response cannot satisfy another owner's deferred disconnect. | closed `cyril-l7tw`; current `bridge.rs` deferred-disconnect path; requester decision 2A, this session (`"1A, 2A"`) |
| Shutdown | Abort the at-most-1 live prompt future, exit the loop, require 0 terminal events, and leave 0 work able to cross into a fresh bridge's channels. | current `bridge.rs` Shutdown arm; `cyril-gua0`; requester decision 2A, this session (`"1A, 2A"`) |
| Channel capacity / backpressure | Preserve bounded 256-notification channels and lossless awaited forwarding while receivers exist. Apply response-versus-`turn_end` semantics in mediator receipt order after backlog delay; a queued response received after `turn_end` is discarded. | `bridge.rs` `NOTIFICATION_CAPACITY` and channel sends; requester decision 2A, this session (`"1A, 2A"`) |
| KAS scoped `turn_end` cardinality | The supported producer emits 0 or 1 scoped `turn_end` per accepted KAS prompt. A second identity-free frame for the same prompt is unsupported upstream drift; Cyril cannot distinguish it from the active same-session owner's own frame and offers no defense. | requester decision 1A, this session (`"1A, 2A"`); `design.md` Blocker 1 |
| Prompt-response work lifetime | Across all supported traces, at most 1 bridge-owned prompt future is live under the serialized Busy guard. Authoritative KAS `turn_end` aborts/drops unresolved work; response resolves it; cancellation targets it; failure/death controls and drops it; shutdown aborts it. | requester decision 2A, this session (`"1A, 2A"`); `design.md` Blocker 2 |

## Out of scope

This change does NOT include:

- detecting, correlating, deduplicating, or defending against a second identity-free scoped KAS `turn_end` for one accepted prompt; that frame is unsupported upstream protocol drift under requester decision 1A;
- selecting `turn_end` or any pre-`turn_end` prompt response as the authoritative KAS `stop_reason`, including cancellation disagreement; that remains cyril-pnwb;
- retaining prompt-response source/reason evidence that arrives or becomes observable after authoritative `turn_end`; requester decision 2A requires it to be discarded;
- adding `_kiro/error/rate_limit` conversion, message wording, retry timing, or deciding how it maps onto an existing failure/death/disconnect lifecycle; that remains cyril-3zy4, and a rate-limit observation alone gains no KAS release authority;
- reordering streamed agent content relative to completion; that remains cyril-9akh;
- reconnect/respawn UX or preserving sessions across bridge restart; that remains cyril-gua0;
- changing Enter-while-Busy/steering UX, cancellation target policy, or terminal-child reap semantics;
- choosing a native KAS turn-id field, counter layout, registry shape, event variant shape, or other implementation design;
- probing KAS or changing production code during this specification audit.

## Constraints

| Dimension | Limit | How measured |
| --- | --- | --- |
| Concurrent active prompt turns | At most 1 turn per bridge process | Controlled dual-`SendPrompt` bridge test; second request receives the existing Busy error |
| Bridge-owned live prompt futures | At most 1 future while Busy; 0 futures from an owner after its release, failure/death disposal, or shutdown | Task-drop sentinels across the 8-transition resource ledger |
| Ownership domain | 2^64 distinct identities per bridge lifetime; 0 ambiguous reuses | Counter boundary injection and fail-closed assertion |
| Applicable App completions | At most 1 per accepted turn for supported inputs | Event reconciliation in every scripted supported terminal-source permutation |
| Supported scoped KAS `turn_end` count | At most 1 frame per accepted KAS prompt; a second frame has 0 guarantees | Fixture-schema classification plus assertion-inventory review |
| Ordinary KAS release sources | Exactly 1 source type: scoped `session_info_update.kind == "turn_end"`; 0 prompt-response and 0 rate-limit releases | Harness injects each source separately and reconciles App completions and Busy state |
| Prompt-response evidence cutoff | Exactly 1 response observation retained when received before `turn_end`; 0 retained when received after `turn_end` | Two-order mediator ledger with response/`turn_end` queue permutations |
| Missing KAS `turn_end` | 0 ordinary completions; Busy persists until 1 existing failure/death/disconnect lifecycle ends the bridge turn or shutdown exits it | Prompt-response-only and absent-response harness followed by each lifecycle fixture |
| Notification channels | 256 notifications per bounded channel | Capacity constant inspection plus 257-event paused-consumer harness |
| Added terminal delay | 0 intentional wait intervals after a supported scoped KAS `turn_end` is received | Prompt-response-absent harness asserts immediate completion without advancing a timeout |

## Decisions

| # | Decision | Source | Why |
| --- | --- | --- | --- |
| 1 | Ownership is per accepted turn and globally trustworthy across sessions, not a bare session-id comparison. | Rivets `cyril-a71q`; `workflow.md` | A bare session guard cannot isolate foreign scoped completion or delayed owner-stamped synthesized events from a newer turn. |
| 2 | A same-session owner-stamped late global v1/v2 result is dropped. | Scope-sensitive visibility gap answer, 2026-07-12 (`"A"`) | Forwarding it would falsely complete newer turn B in the main pipeline. |
| 3 | A differently scoped completion remains visible to its routed session consumer without touching main ownership, prompt work, or Busy state. | Scope-sensitive visibility gap answer, 2026-07-12 (`"A"`) | Session isolation must prevent split-brain without erasing a legitimate secondary-session event. |
| 4 | Synthesized global v1/v2 completion remains supported through dispatch-stamped ownership. | Rivets `cyril-a71q`; `event.rs` | A session-id-only match would freeze every v1/v2 turn. |
| 5 | Scoped KAS `session_info_update.kind == "turn_end"` is the sole ordinary KAS release/completion source; without it, Busy persists until an existing failure/death/disconnect lifecycle or shutdown. | Failed prior design falsifier; requester option A, 2026-07-12 (`"A"`) | Prompt response cannot safely act as KAS release authority and remains nonterminal. |
| 6 | cyril-a71q does not select KAS stop-reason precedence. | Gap correction, 2026-07-12; open `cyril-pnwb` | Completion authority and reason authority are separate decisions. |
| 7 | Exhausting the `u64` ownership domain fails closed and requires a fresh bridge process. | Exhaustion gap answer, 2026-07-12 (`"A"`) | Saturation or reuse would destroy the ownership guarantee. |
| 8 | Error/death ordering remains `BridgeError` → owned `TurnCompleted` → `BridgeDisconnected`. | Closed `cyril-l7tw`; current `bridge.rs` | Ownership must preserve the existing visible fail-stop contract. |
| 9 | Shutdown aborts bridge-owned prompt work and needs no completion; process restart creates isolated channels/state. | Current `bridge.rs`; open `cyril-gua0` | Old process-local events cannot participate in a fresh ownership domain, and reconnect is separate work. |
| 10 | The cyril-3zy4 rate-limit path is a consumer of ownership, not an additional KAS completion source chosen here. | Rivets `cyril-3zy4`; requester option A, 2026-07-12 (`"A"`) | Converter/UI design and any future mapping to failure lifecycle remain separate; observation alone releases 0 turns. |
| 11 | The supported KAS input space normatively permits at most 1 scoped `turn_end` frame per accepted KAS prompt; a duplicate identity-free frame is unsupported upstream drift that the ownership guarantee cannot detect or defend against. | Requester decision 1A, this session, 2026-07-12; verbatim reply `"1A, 2A"` | Without native correlation, duplicate A and owned B frames are observationally identical; bounding producer cardinality removes the duplicate history from supported inputs rather than claiming a classifier exists. |
| 12 | On receipt of authoritative KAS `turn_end`, Cyril aborts or drops that owner's prompt-response work; a response received afterward is discarded and supplies no source/reason evidence. Only a response received before `turn_end` is preserved. | Requester decision 2A, this session, 2026-07-12; verbatim reply `"1A, 2A"` | The cutoff bounds bridge-owned unresolved prompt work to at most 1 live future and makes response-versus-`turn_end` races deterministic by mediator receipt order. |
| 13 | Earlier language promising preservation of a KAS prompt response arriving after `turn_end`, including the prior signed consequence statement and earlier Decisions 5–6 wording, is superseded and no longer normative. | Requester decision 2A, this session, 2026-07-12; verbatim reply `"1A, 2A"`; prior `.cyril-a71q/spec.md` preserved as `spec-pre-bounded-exact-one.md` | Late evidence conflicts with the selected bounded abort/discard contract. |
| 14 | Owner-stamped late global v1/v2 results and KAS responses queued but received after `turn_end` cannot release B; no corresponding safety claim is made for excluded duplicate KAS `turn_end`. | Requester decisions 1A and 2A, this session, 2026-07-12; verbatim reply `"1A, 2A"` | This states only distinctions Cyril can observe under the supported input contract. |

## Gap audit

No unanswered behavioral, measurement, proxy, edge-case, role, scope, constraint, or decision-citation gap remains after decisions 1A and 2A. Stop-reason precedence is a deliberate cyril-pnwb exclusion, duplicate scoped KAS `turn_end` is an explicit unsupported-input boundary, and post-revision consequence confirmation remains pending rather than an unanswered contract choice.

## Sign-off

Consequences stated to the requester:

For the terminal operator, Cyril supports KAS only when each accepted prompt produces no more than one scoped `session_info_update.kind == "turn_end"`. That frame is the only ordinary KAS event that unlocks input. When Cyril's serialized mediator receives it, Cyril completes the turn immediately, aborts or drops any unresolved prompt-response work for that owner, and permits the next prompt; if a response arrives or becomes observable afterward, Cyril discards it, records no prompt-response source/reason from it, and it cannot release the next turn. If the response is received first, Cyril preserves its source/reason for cyril-pnwb but keeps input Busy until `turn_end`; this feature does not decide which preserved reason wins. A KAS producer that emits a duplicate identity-free scoped `turn_end` has drifted outside the supported protocol contract, and Cyril cannot detect that duplicate or prevent it from being mistaken for a newer same-session turn's frame.

At most one bridge-owned prompt future exists under the serialized Busy guard. KAS `turn_end` aborts or drops it, v1/v2 response resolves it, cancellation targets only its owning session without adding a KAS release source, failure/death controls and drops it in the existing visible order, and shutdown aborts it with no required completion. A delayed owner-stamped global v1/v2 result or KAS response queued but received after `turn_end` cannot unlock turn B; no duplicate-`turn_end` safety is implied. Cross-session completions still route to their own consumers without changing the main turn. If KAS emits no `turn_end`, a response or rate-limit observation alone leaves the terminal operator Busy until an existing failure/death/disconnect lifecycle ends the bridge turn or shutdown exits it.

Named oracle blindness: (1) scripted same-session traces cannot see unscripted schedules, undocumented frames, or excluded duplicate KAS `turn_end`; (2) routing tests cannot see future consumers that bypass `RoutedNotification`; (3) receipt-order fixtures cannot choose cyril-pnwb reason precedence or predict production receipt order; (4) cardinality fixtures cannot prove a live producer honors at-most-one; (5) task sentinels cannot see work outside bridge ownership or memory corruption; (6) lifecycle fixtures cannot see operating-system death timing or uncaptured cancellation reasons; (7) rate-limit fixtures cannot see live payload, rendering, retry timing, or future lifecycle mapping; (8) counter injection cannot see a naturally elapsed 2^64-turn lifetime; (9) the 257-event fixture cannot see an indefinitely wedged consumer, unbounded external producer memory, or live wire drift; and (10) workspace gates cannot see production scheduling or excluded behavior.

Visible absences: this change will not recover safely from duplicate identity-free KAS `turn_end`; retain prompt-response evidence observed after authoritative `turn_end`; make a response or rate-limit observation unlock KAS input; add rate-limit message/retry UX; choose displayed stop-reason precedence; reorder content that trails completion; reconnect the agent automatically; or change steering, cancellation targeting, and terminal-child reap behavior.

The requester's verbatim reply to this consequence statement: **PENDING — NOT PASSED.**

Date: 2026-07-12
