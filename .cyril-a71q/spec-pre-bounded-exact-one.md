# Feature: Per-turn terminal ownership across sessions and engines

## What this is

Cyril will associate every accepted prompt turn with an observable ownership boundary so that a late or foreign terminal event cannot end a different active turn. The boundary preserves v1/v2's synthesized global completion, makes scoped KAS `session_info_update.kind == "turn_end"` the sole ordinary KAS release/completion source, retains the global KAS prompt response only as secondary source/reason evidence for cyril-pnwb, and preserves session-scoped routing without selecting an implementation mechanism or resolving stop-reason authority.

## Users

- **Terminal operator**: sends prompts, cancels work, switches sessions, and needs Busy/Idle state and input availability to correspond to the turn actually running.
- **Cyril protocol maintainer**: consumes this contract when adding terminal-producing paths such as KAS rate-limit handling and needs those paths to identify which accepted turn they terminate.

## Behavior

### Same-session stale completion

- **Given**: turn A in session S completed, turn B in the same session was subsequently accepted, and the toolbar/input observably show B as Busy.
- **When**: a delayed terminal event belonging to A reaches the bridge before B's terminal event.
- **Then**: Cyril forwards 0 completions for A to the main-session consumer, B remains Busy and rejects another `SendPrompt` as already in progress, and B's own terminal event later produces exactly 1 main-session completion and returns input to Idle.

### Cross-session completion during a main turn

- **Given**: main-session turn B is Busy and a completion scoped to different session X reaches the bridge.
- **When**: the bridge observes X's completion before B completes.
- **Then**: Cyril forwards exactly 1 scoped completion to X's routed consumer, forwards 0 completions to the main-session consumer, and leaves B's bridge guard and Busy state unchanged until B's own completion.

### Synthesized global v1/v2 completion

- **Given**: a v1/v2 prompt turn T is the accepted active turn and its prompt response is converted to a global `RoutedNotification` whose `session_id` is `None`.
- **When**: that synthesized `TurnCompleted` reaches the bridge observer.
- **Then**: Cyril recognizes it as T's terminal event, forwards exactly 1 completion through the main pipeline, clears only T's busy guard, and accepts the next prompt.

### KAS release authority and secondary evidence

- **Given**: accepted KAS turn T is Busy and may produce a session-scoped `session_info_update.kind == "turn_end"` observation, a global prompt-response observation before or after it, or no `turn_end`.
- **When**: the observer receives a KAS observation while T is active, and the other observation may arrive later or remain absent.
- **Then**: only T's owned scoped `turn_end` forwards exactly 1 applicable App completion and releases T; the global KAS prompt response forwards 0 completions, clears 0 busy guards, and remains available only as source/reason evidence for cyril-pnwb. If no owned `turn_end` arrives, T remains Busy until an existing failure, death, or disconnect lifecycle ends it.

### Cancellation and late terminal signals

- **Given**: the terminal operator cancels active turn T and the bridge targets T's owning session, after which a newer turn U may be accepted.
- **When**: cancellation-related terminal sources for T arrive before or after U starts.
- **Then**: exactly 1 owned completion may release T; every later same-session T completion is dropped and cannot release U. For KAS, a prompt response releases 0 turns even if its `stop_reason` disagrees with `turn_end`; both source/reason observations remain available for cyril-pnwb, and this feature makes no stop-reason authority decision.

### Prompt error and connection death

- **Given**: active turn T encounters a prompt transport error or agent-process death.
- **When**: the existing cyril-l7tw path emits `BridgeError`, T's synthesized terminal marker, and any deferred `BridgeDisconnected`.
- **Then**: T's owned completion is forwarded exactly once, the observable order remains `BridgeError` → `TurnCompleted` → `BridgeDisconnected`, and no stale or differently scoped completion can release T or trigger its deferred disconnect.

### Shutdown and fresh process lifetime

- **Given**: a turn is active when `BridgeCommand::Shutdown` is received.
- **When**: the bridge aborts the prompt task and exits its run loop.
- **Then**: shutdown requires 0 additional `TurnCompleted` events, no event queued in that stopped bridge can affect a later fresh bridge process, and the fresh process begins a new isolated ownership domain.

### Rate-limit consumer boundary

- **Given**: cyril-3zy4 eventually observes a KAS rate-limit outcome during active turn T, and this contract permits only T's scoped `turn_end` or an existing failure/death/disconnect lifecycle to release T before the terminal operator starts turn U.
- **When**: any secondary rate-limit or prompt-response observation for T arrives before or after U starts.
- **Then**: that observation forwards 0 KAS completions and releases 0 turns; only T's owned `turn_end` or existing lifecycle may end T, and only U's corresponding owned event may end U. This contract does not choose the rate-limit payload converter, message text, retry policy, or representation.

### Ownership identity exhaustion

- **Given**: one bridge process has allocated all 18,446,744,073,709,551,616 distinct values in its `u64` ownership domain.
- **When**: another prompt is requested.
- **Then**: Cyril starts 0 new turns, surfaces a visible lifecycle failure/disconnect, and requires a fresh bridge process rather than reusing an identity or accepting an ambiguously owned turn.

## Success criteria

- **Same-session isolation**: 2/2 terminal events in the scripted `A completes → B starts → late A → owned B` trace have the expected disposition—late A produces 0 App completions and owned B produces exactly 1—measured by a deterministic bridge-loop integration harness that also asserts B remains guarded after late A.  
  This method cannot see: scheduler interleavings not represented by the scripted trace or undocumented live-agent frames.
- **Cross-session routing**: 3/3 routing assertions pass for a foreign scoped completion during main turn B—1 event reaches X, 0 events reach main, and B remains guarded—measured by bridge/App routing integration tests followed by B's owned completion.  
  This method cannot see: future consumers that bypass `RoutedNotification` or mutate Busy state outside the tested pipeline.
- **Engine terminal compatibility and evidence preservation**: 5/5 traces have the required completion count, Busy disposition, and delivered-source evidence—v1/v2 global response emits 1 completion and releases its turn; KAS `turn_end` followed by response emits 1 completion total and retains both source/reason observations; KAS response followed by `turn_end` emits 0 completions and stays Busy until `turn_end` emits 1 while retaining both observations; KAS response only emits 0 completions, stays Busy, and retains its evidence; and KAS `turn_end` only emits 1 completion and releases—measured with a controlled in-process ACP/bridge harness and observer-seam source/reason ledger.  
  This method cannot see: which preserved KAS `stop_reason` cyril-pnwb should later make authoritative when `turn_end` and prompt response disagree.
- **Lifecycle interactions**: 4/4 scripted cases preserve ownership—cancel with a late duplicate, prompt error, mid-turn connection death, and shutdown—measured by event-sequence assertions; error/death must contain exactly 1 owned completion in the recorded cyril-l7tw order, while shutdown requires 0 completions.  
  This method cannot see: operating-system-specific process-death timing or a live KAS cancellation reason not present in the harness.
- **Rate-limit consumer contract**: 2/2 modeled assertions pass—a rate-limit observation alone emits 0 KAS completions and leaves T Busy, and a later T rate-limit or prompt-response observation emits 0 releases of U—measured by a consumer-contract test that injects rate-limit observations without selecting cyril-3zy4's converter design.  
  This method cannot see: the live `_kiro/error/rate_limit` payload, message rendering, retry timing, or how cyril-3zy4 maps the outcome onto an existing failure/death/disconnect lifecycle.
- **Identity exhaustion safety**: 2/2 injected boundary states pass—last unused `u64` identity is allocated once, and the following request starts 0 turns and emits 1 visible fail-closed lifecycle event—measured by dependency-injected counter-state tests.  
  This method cannot see: a naturally elapsed 18,446,744,073,709,551,616-turn process lifetime or memory corruption outside the ownership allocator.
- **Maximum supported backlog**: 257/257 notifications are accounted for in a trace containing the 256-notification channel capacity plus the terminal event blocked behind it, measured by a paused-consumer bridge harness that resumes and reconciles every routed event and final ownership state.  
  This method cannot see: an indefinitely wedged App consumer, unbounded external producer memory, or scheduling behavior beyond the bounded channels.
- **Workspace regression gate**: 100% of workspace tests pass with 0 clippy warnings, measured by `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.  
  This method cannot see: live v1/v2/KAS wire drift, production scheduler timing, or behavior excluded from the workspace tests.

## Edge cases and decisions

| Edge | Decision | Source |
| --- | --- | --- |
| Empty set (no active turn) | Drop an unowned global or same-session terminal event; preserve a differently scoped session completion for its own routed consumer without creating or clearing a main turn. | `cyril-a71q`; scope-sensitive visibility gap answer, 2026-07-12 (`"A"`) |
| Max scale | The ownership domain contains 18,446,744,073,709,551,616 distinct `u64` values per bridge lifetime; the next prompt fails closed and starts 0 turns. | `cyril-a71q` note; exhaustion gap answer, 2026-07-12 (`"A"`) |
| Null / missing field | A synthesized global v1/v2 completion with no session scope must still own its originating turn. A KAS `turn_end` with missing/unparseable `stopReason` remains a completion and defaults to `EndTurn`; no native KAS turn identifier may be assumed. | `cyril-a71q`; `event.rs` global routing; `convert/kas.rs` fallback |
| Concurrent writes | The bridge accepts at most 1 active prompt turn. Terminal-source races are serialized by the existing mediator and may clear only their owner; another `SendPrompt` while owned Busy remains rejected. | `bridge.rs` ADR-0004 comments and current guard; `cyril-a71q` |
| Permission denied / unauthenticated | Ownership does not reinterpret refusal/auth outcomes. If they end a turn, the terminal marker must own that turn; prompt/auth failure visibility and ordering remain cyril-l7tw behavior. | closed `cyril-l7tw`; ownership-only gap correction, 2026-07-12 |
| Partial failure (one of N succeeded) | A KAS prompt response without scoped `turn_end` emits 0 completions and leaves the turn Busy until an existing failure/death/disconnect lifecycle ends it. A scoped owned `turn_end` releases without waiting for prompt response; a later prompt response remains secondary source/reason evidence and cannot release a newer turn. | failed design falsifier and requester option A, 2026-07-12; closed `cyril-j16p`; `cyril-a71q` |
| Retries / idempotency | Each accepted turn reaches the App as at most 1 applicable completion. A retry accepted after completion is a new turn; duplicates from the prior turn do not complete it. | closed `cyril-j16p`; `cyril-a71q` |
| Soft-deleted records | Not applicable: the bridge lifecycle contains no persisted or soft-deleted turn records. | `bridge.rs` process-local state |
| Multi-tenancy boundaries | ACP session scope is the isolation boundary: another session's completion remains visible only to that routed consumer and cannot mutate the active main turn. | `event.rs` routing contract; `cyril-a71q`; scope-sensitive visibility gap answer |
| Time-zone / DST | Not applicable: ownership and terminal ordering use no wall-clock or calendar value. | `bridge.rs`; `event.rs` |
| Replication lag | Not applicable: the mediator and channels are process-local; no replicated store participates. | `bridge.rs` channel architecture |
| Cache invalidation | Not applicable: no cache supplies ownership truth. A fresh bridge process creates an isolated ownership domain rather than restoring cached identities. | `bridge.rs`; `cyril-gua0`; exhaustion gap answer |
| Same-session stale terminal | Drop it; forwarding would falsely complete the newer main-session turn. | `cyril-a71q`; scope-sensitive visibility gap answer, 2026-07-12 |
| Differently scoped terminal | Forward exactly once to that session's routed consumer and do not change the active main turn or bridge busy guard. | `cyril-a71q` cross-session note; scope-sensitive visibility gap answer, 2026-07-12 |
| KAS source disagreement | Preserve both same-turn source/reason observations for cyril-pnwb, but only scoped `turn_end` releases/completes the KAS turn; prompt response is never terminal, regardless of arrival order or reason disagreement. Stop-reason precedence remains undecided. | failed design falsifier and requester option A, 2026-07-12; open `cyril-pnwb` |
| Cancellation | Cancel continues targeting the in-flight owner session; stale cancellation terminal signals cannot clear a later turn. Terminal-child reap policy is unchanged. | `bridge.rs` CancelRequest arm; closed `cyril-3lh8`; open `cyril-pnwb` |
| Prompt error / engine death | Preserve visible `BridgeError` → owned `TurnCompleted` → `BridgeDisconnected`; a stale terminal cannot satisfy deferred disconnect for the active turn. | closed `cyril-l7tw`; current `bridge.rs` deferred-disconnect path |
| Shutdown | Abort the prompt task, exit the loop, and require 0 terminal events. Old bridge events cannot cross into a fresh bridge's channels. | current `bridge.rs` Shutdown arm; `cyril-gua0` one-shot bridge fact |
| Channel capacity / backpressure | Preserve bounded 256-notification channels and lossless awaited forwarding while receivers exist; ownership decisions remain correct at a full-capacity boundary. | `bridge.rs` `NOTIFICATION_CAPACITY` and channel sends |

## Out of scope

This change does NOT include:

- selecting `turn_end` or prompt response as the authoritative KAS `stop_reason`, including cancellation disagreement; that remains cyril-pnwb;
- adding `_kiro/error/rate_limit` conversion, message wording, retry timing, or deciding how it maps onto an existing failure/death/disconnect lifecycle; that remains cyril-3zy4, and a rate-limit observation alone does not gain KAS release authority here;
- reordering streamed agent content relative to completion; that remains cyril-9akh;
- reconnect/respawn UX or preserving sessions across bridge restart; that remains cyril-gua0;
- changing Enter-while-Busy/steering UX, cancellation target policy, or terminal-child reap semantics;
- choosing a native KAS turn-id field, counter layout, registry shape, event variant shape, or other implementation design;
- probing KAS or changing production code during this specification audit.

## Constraints

| Dimension | Limit | How measured |
| --- | --- | --- |
| Concurrent active prompt turns | At most 1 turn per bridge process | Controlled dual-`SendPrompt` bridge test; second request receives the existing Busy error |
| Ownership domain | 2^64 distinct identities per bridge lifetime; 0 ambiguous reuses | Counter boundary injection and fail-closed assertion |
| Applicable App completions | At most 1 per accepted turn | Event reconciliation in every scripted terminal-source permutation |
| Ordinary KAS release sources | Exactly 1 source type: scoped `session_info_update.kind == "turn_end"`; 0 prompt-response releases | Harness injects each source separately and reconciles App completions and Busy state |
| Missing KAS `turn_end` | 0 ordinary completions; Busy persists until 1 existing failure/death/disconnect lifecycle ends the turn | Prompt-response-only harness followed by each existing lifecycle fixture |
| Notification channels | 256 notifications per bounded channel | Capacity constant inspection plus 257-event paused-consumer harness |
| Added terminal delay | 0 intentional wait intervals after an owned scoped KAS `turn_end` is observed | Harness with prompt response absent asserts immediate `turn_end` completion without advancing a timeout |

## Decisions

| # | Decision | Source | Why |
| --- | --- | --- | --- |
| 1 | Ownership must be per accepted turn and globally trustworthy across sessions, not a bare session-id comparison. | Rivets `cyril-a71q`; `workflow.md` | A bare session guard cannot isolate foreign scoped completion or delayed owned synthesized events from a newer turn. |
| 2 | Same-session stale completion is dropped. | Scope-sensitive visibility gap answer, 2026-07-12 (`"A"`) | Forwarding it would falsely complete newer turn B in the main pipeline. |
| 3 | A differently scoped completion remains visible to its routed session consumer without touching main ownership or Busy state. | Scope-sensitive visibility gap answer, 2026-07-12 (`"A"`) | Session isolation should prevent split-brain without erasing a legitimate secondary-session event. |
| 4 | Synthesized global v1/v2 completion remains supported. | Rivets `cyril-a71q`; `event.rs` | A session-id-only match would freeze every v1/v2 turn. |
| 5 | Scoped KAS `session_info_update.kind == "turn_end"` is the sole ordinary KAS release/completion source. A global KAS prompt response is secondary source/reason evidence for cyril-pnwb and never releases a KAS turn; without `turn_end`, Busy persists until an existing failure/death/disconnect lifecycle ends the turn. | Failed design falsifier and requester option A, 2026-07-12 (`"A"`) | This contract relaxation removes the observational contradiction while preserving both source/reason observations and v1/v2 global completion. |
| 6 | cyril-a71q does not select KAS stop-reason precedence, but preserves both source/reason observations for cyril-pnwb even though prompt response is nonterminal. | Gap correction, 2026-07-12; requester option A, 2026-07-12 | Completion authority and stop-reason authority are separate: no live cancel capture settles which preserved reason cyril-pnwb should display. |
| 7 | Exhausting the `u64` ownership domain fails closed and requires a fresh bridge process. | Exhaustion gap answer, 2026-07-12 (`"A"`) | Saturation/reuse would destroy the ownership guarantee; a reset cannot prove indefinitely late KAS responses are gone. |
| 8 | Error/death ordering remains `BridgeError` → owned `TurnCompleted` → `BridgeDisconnected`. | Closed `cyril-l7tw`; current `bridge.rs` | Ownership must preserve the existing visible fail-stop contract. |
| 9 | Shutdown aborts the prompt task and needs no completion; process restart creates isolated channels/state. | Current `bridge.rs`; open `cyril-gua0` | Old process-local events cannot safely participate in the new ownership domain, and reconnect is separate work. |
| 10 | The cyril-3zy4 rate-limit path is a consumer of ownership, not an implementation dependency or additional KAS completion source chosen here. | Rivets `cyril-3zy4`; requester option A, 2026-07-12 | Its converter/UI design and any mapping onto an existing failure/death/disconnect lifecycle are separate; a rate-limit observation alone releases 0 turns here. |

## Sign-off

Consequences stated to the requester:

For the terminal operator, a delayed completion from an earlier prompt in the same session will not end the prompt currently running, while a completion scoped to another session will still reach that session without unlocking or desynchronizing the main session. Synthesized global v1/v2 prompt responses will continue to complete their own turns. For KAS, only the scoped `session_info_update.kind == "turn_end"` observation will complete and release a turn; a global KAS prompt response will be retained only as source/reason evidence for cyril-pnwb and will never unlock input, whether it arrives before or after `turn_end`. If KAS never sends `turn_end`, the terminal operator will continue to see Busy until an existing failure, death, or disconnect lifecycle ends the turn. Cancellation targeting, error/death ordering, shutdown isolation, and fail-closed exhaustion after 2^64 turn identities retain their prior consequences.

Named oracle blindness: (1) the same-session scripted harness cannot see unmodeled scheduler interleavings or undocumented live-agent frames; (2) routing tests cannot see future consumers that bypass `RoutedNotification`; (3) the KAS harness preserves disagreeing source/reason observations but cannot decide which reason cyril-pnwb should later make authoritative; (4) lifecycle fixtures cannot see OS-specific death timing or an uncaptured live KAS cancellation reason; (5) the rate-limit fixture cannot see the live payload, rendered wording, retry timing, or how cyril-3zy4 maps the outcome onto an existing failure/death/disconnect lifecycle; (6) counter injection cannot see a naturally elapsed 18,446,744,073,709,551,616-turn lifetime or memory corruption; (7) the 257-event backlog fixture cannot see an indefinitely wedged consumer or behavior beyond bounded channels; and (8) workspace tests cannot see live v1/v2/KAS wire drift or production scheduling.

Visible absences: this change will not make a KAS prompt response or rate-limit observation unlock a turn by itself; add the rate-limit message/retry UX; choose which preserved KAS stop reason is displayed when sources disagree; reorder content that trails completion; reconnect the agent automatically; or change steering and terminal-child reap behavior.

The requester replied, verbatim: "I confirm these revised consequences"

Date: 2026-07-12
