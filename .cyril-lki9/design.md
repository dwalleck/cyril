# Design: cyril-lki9 — agent-initiated turns and engine injections

Status: DRAFT — awaiting requester approval

## Route and inputs

- Route: **Empirical** (`route.md`: T1 yes, T2 yes, T3 no, T4 no).
- Behavior set: `spec.md` B1–B7, signed "yes" 2026-09-29 (re-signed after the prove-it stage re-opened it).
  - B1 workflow wake header · B2 full turn parity · B3 generic header for other reasons · B4 nameless header without a known run · B5 inline notices for `notify-*` injections · B6 injections never touch operator steers (cyril-5n75) · B7 busy from `turn_start`.
- Edge decisions: `spec.md` Decisions (FIFO naming per session; nameless fallback; `unspecified` reason; headers per rendered session; KAS owns concurrent admission; no retry of a failed wake; step-verdict notice without node id; silent wake = busy, no header).
- Empirical premises (`evidence.md`, all PASS):
  - P1 tag on every chunk/tool_call/tool_call_update of an agent-initiated turn, never on `turn_start`/`turn_end`; wakes also occur on step sessions (`send-message-wake`); engine-started step turns are untagged.
  - P2 `agent-client-protocol 2.0.0` (`schema::v1`) retains `_meta` on all three update kinds.
  - P3 the current bridge drops the wake `turn_end` (`DropUnowned`) — no `TurnCompleted` reaches the App.
  - P4 `session/cancel` ends a wake with `turn_end{cancelled}` in ~10 ms.
  - P5 `_session/steer` during a wake is queued and honored; `session/prompt` during any server-started turn pre-empts it (`turn_end{cancelled}`, new turn); a wake can be silent ≥ 60 s.
  - P6 busy-parent completion = `steering_injected{messageId: notify-wf-<uuid>, content: <wake text>, notificationSeverity}` inside the running turn, no wake turn; step verdicts = `steering_injected{messageId: notify-<uuid>, content: "[notification/<sev>] <msg>"}`.
  - P7 the triggering `run_complete` precedes the wake's first tagged frame.
- Captures (oracle fixtures): `experiments/conductor-spike/kas-workflow-channels-06615-restate-gateoff-tail-2.26.0.jsonl`, `…/kas-workflow-new-06615-gateon-2.26.0.jsonl`, `.cyril-lki9/lki9-live-{cancel,steer,prompt,busy}-06615-2.26.0.jsonl`.

## Input shapes

### Wire inputs the change reads

| # | Shape | Source | Status |
|---|---|---|---|
| S1 | `turn_start` on the main session with no cyril prompt in flight | P5 (silent wake), tail capture | claim (B7) |
| S2 | `turn_start` on the main session while a cyril prompt is in flight (cyril-owned turn) | existing behavior | claim (no regression) |
| S3 | `turn_start` on a non-main session (step / subagent) | P1 (step turns untagged; send-message-wake on step) | claim (per-session header; parity limited to what that stream renders today) |
| S4 | chunk / tool_call / tool_call_update with `agentInitiated: true` + `agentInitiatedReason = "workflow-complete-wake"` | P1 | claim (B1) |
| S5 | … with another reason string (e.g. `send-message-wake`) | P1 gate-on capture | claim (B3) |
| S6 | … with `agentInitiated: true` and reason absent | not observed; schema-possible | claim (B3 `unspecified`) |
| S7 | … with `agentInitiated` absent or `false` (ordinary turn frames) | all captures | claim (no header) |
| S8 | `_meta` absent entirely / `_meta.kiro` not an object | ACP generic | claim (no header, no error) |
| S9 | `turn_end` for a server-started turn (`end_turn`, `cancelled`) | P3, P4, P5 | claim (B2: exactly one TurnCompleted) |
| S10 | `turn_end` for a server-started turn that emitted no tagged frame (silent wake cancelled) | P5 prompt leg | claim (B7: idle, no header) |
| S11 | `steering_injected` with `messageId` `notify-wf-*` | P6 busy | claim (B5 workflow notice) |
| S12 | `steering_injected` with `messageId` `notify-*` (not `notify-wf-`), content `[notification/<sev>] <msg>` | P6 cancel leg, tail capture | claim (B5 step notice) |
| S13 | … `notify-*` content without the `[notification/<sev>] ` prefix | schema-possible | claim (content shown verbatim) |
| S14 | … `notificationSeverity` absent | schema-possible | claim (`info`) |
| S15 | `steering_injected` with `messageId` `steer-*` (operator steer) | P5 steer leg | claim (existing SteeringConsumed path unchanged) |
| S16 | `steering_injected` with `messageId` absent / empty | existing converter test (`kas.rs:1162`) | claim (existing path unchanged: operator-steer FIFO fallback) |
| S17 | `steering_cleared` with ids all `notify-*` | P6 | claim (B6: no operator chip flips; never forwarded as an empty "clear all") |
| S18 | `steering_cleared` with a mix of `notify-*` and `steer-*` ids | schema-possible | claim (B6: only the `steer-*` ids reach the operator-steer reconciler) |
| S19 | `steering_cleared` with an empty id list (old v2 dialect "clear all") | existing | claim (unchanged) |
| S20 | `_kiro/workflow/run_complete` for a run parented to the session, before the wake | P7 | claim (B1 name source) |
| S21 | several `run_complete`s pending for one session | spec FIFO row | claim (FIFO) |
| S22 | wake with no pending `run_complete` for the session | spec B4 | claim (nameless) |
| S23 | `run_complete` name fields: `runLabel` present / only `workflowName` / only `workflowId` | KAS precedence | claim (B1 precedence) |
| S24 | `run_complete` status `completed` / `failed` / `aborted` | wake policy | claim (status word) |

### Decision cells the design branches on

| # | Cell | Status |
|---|---|---|
| D1 | operator presses Enter: no turn / cyril-owned turn / server-started turn | claim (B2/B7: server-started → steer, never prompt) |
| D2 | operator presses Esc during a server-started turn | claim (B2: `session/cancel`) |
| D3 | permission request arrives inside a server-started turn | claim (B2: normal approval overlay) |
| D4 | stall watchdog during a server-started turn | claim C22 (B2: armed from `turn_start`) |
| D5 | `messageId` prefix classification: `notify-wf-` / `notify-` / other / absent | claim (B5/B6) |
| D6 | header already shown for this turn, further tagged frames arrive | claim (exactly one header per turn) |
| D7 | cyril dispatches a prompt at the same moment KAS starts a wake (race) | claim (KAS admission wins; cyril applies whatever terminal pair arrives without a stuck busy state) |
| D8 | engine = v1/v2 (no `_meta.kiro` kinds, no agent-initiated turns) | `N/A — reason`: v2 never emits `turn_start` or `agentInitiated` (spec out-of-scope); claim only that v2 paths are unchanged |
| D9 | replayed history after `session/load` containing tagged frames | `N/A — reason`: intended future work, cyril-99ds (replayMarking) |

## Removed invariants (subtractive sweep)

Core move classification: **subtractive**. The change removes the bridge invariant "every turn is begun by cyril's own `SendPrompt`" (ADR-0004: at most one active turn, owner = cyril's dispatch).

Facts that invariant guaranteed, and whether they can now be violated:

1. *A terminal with no active turn is stale* → the mediator drops it (`DropUnowned`). **Now false for server-started turns** → claim: a server-started turn's `turn_end` is forwarded exactly once.
2. *`is_busy()` false ⇒ no turn is running on the main session* → drives the SendPrompt busy-guard and (via App) Enter = prompt. **Now false during a wake** → claim: busy is true from a server `turn_start` to its `turn_end`; a `SendPrompt` is not dispatched while it is true (the operator's text goes to steer).
3. *A turn's liveness window begins at dispatch* → `turn_liveness.begin` on SendPrompt. **Now missing for server turns** → claim: liveness begins at server `turn_start`.
4. *Every `turn_end` pairs with a prompt response (companion terminal) on KAS* → the mediator expects a companion for cyril-owned turns. **Server turns have no prompt response** → claim: a server-started turn releases on `turn_end` alone and owes no companion (no absorb expectation left dangling).
5. *At most one active turn* → still true on the wire for the main session (P5: KAS pre-empts), but the race D7 (cyril dispatch vs server turn_start) can put two starts in flight → claim: the mediator never ends with a stuck busy state after any interleaving of {cyril dispatch, server turn_start, turn_end{cancelled}, turn_end, prompt response}.
6. *`SteeringConsumed` / `SteeringCleared` always concern operator steers* → UiState's FIFO fallbacks assume it. **False** (P6) → claim B6.

Still-safe: subagent/step-session terminals (`Forward` as foreign) — unchanged; server turns on non-main sessions stay foreign.

## Placement

| Capability | Owner | New seam | Forbidden |
|---|---|---|---|
| Recognize KAS `turn_start`; classify `notify-*` steering; strip `notify-*` from `steering_cleared` | `cyril-core/src/protocol/convert/kas.rs` (KAS dialect) | N/A — existing seam: `session_info_to_notification` kind dispatch | Kiro literals (`turn_start`, `notify-`, `agentInitiated`) in `convert/mod.rs` (generic ACP); any `acp::` type outside `protocol/convert` + engine |
| Extract the agent-initiated tag from chunk / thought / tool frames | `convert/kas.rs` helper, exposed through a new defaulted `Engine::turn_origin` (`protocol/engine.rs`); KAS overrides, v2 keeps the default `None` | `Engine::turn_origin(&acp::SessionNotification) -> Option<AgentInitiation>` (selected in Alternatives A) | changing `convert_session_update`'s return type; adding origin fields to `AgentMessage`/`ToolCall` domain types |
| Carry the tag with the frame | `types/event.rs` `RoutedNotification` gains `origin: Option<AgentInitiation>` + `with_origin` (envelope metadata beside `session_id`, `turn`) | N/A — existing envelope | a wrapper `Notification` variant |
| Own server-started turns; forward their terminal; announce once per turn | `protocol/turn_mediator.rs` (deepened) + wiring in `domain_mediator/inbound.rs` | N/A — existing seam: `observe()` → `Disposition` gains `BeginServerTurn` and an announce decision | a second module deciding busy (Alternatives B); App/UiState inferring turn boundaries |
| Session busy during a server turn | `cyril-core/src/session.rs` `SessionController::apply_notification` (`TurnStarted` → Busy) | N/A — existing seam | App setting Busy on `TurnStarted` |
| Oldest-unconsumed completed run per parent session | `cyril-core/src/workflow.rs` `WorkflowTracker::take_wake_label` (+ `runLabel` parsed in `convert/kas/workflow.rs`) | method on the existing tracker (Alternatives C) | a second consumer of `Notification::Workflow`; App-held run queues |
| Header / notice text | new `cyril-ui/src/turn_labels.rs` (pure formatting) | module-private fns called by `UiState` / `SubagentUiState` | rendering decisions in `cyril-core` or `cyril` (App) |
| Insert header/notice into a session's transcript; activity on `TurnStarted`; steer fallback narrowing (B6) | `cyril-ui/src/state.rs` (protected parent: thin methods + arms), `subagent_ui.rs` for non-main streams | N/A — existing seams (`apply_notification`, `add_system_message`) | formatting logic in `state.rs`; any new render path in `widgets/chat.rs` (reuses `ChatMessageKind::System`) |
| Route the new notifications; resolve wake labels | `crates/cyril/src/app.rs` (protected parent: routing arms only) | N/A | label/text formatting, FIFO bookkeeping, or turn-boundary logic in App |

## Module shape

**Length review:** `N/A — no trigger`. No repository length gate exists (inventory Q9: CI runs fmt/clippy/nextest + `.cyril-jlxx/oracles/windows-construction.py` only; `clippy.toml` sets only `allow-expect-in-tests`; no `too_many_lines`; issue-local ledgers `.cyril-gl5s`, `.cyril-y628`, `.cyril-k3lz` are historical and not in CI). Precedent: `.cyril-6bwr`, `.cyril-ell0`. Growth is still bounded by the protected-parent rules below.

### Inventory (production lines = before the first `#[cfg(test)] mod`, inventory Q10 at `bfc498b1`)

| Module | Prod lines | Interface / responsibilities today | Change |
|---|---:|---|---|
| `cyril-core/src/protocol/convert/kas.rs` | 616 | KAS dialect: `session_info_to_notification` kind dispatch (no `turn_start`; `_ => None`), steering kinds → `Steering{Queued,Consumed,Cleared}` | deepen |
| `cyril-core/src/protocol/convert/mod.rs` | 530 | generic ACP conversion; reads `_meta` only for modes/trust options | retain (no change) |
| `cyril-core/src/protocol/engine.rs` | — | `Engine` trait: `convert_session_update -> Option<Notification>`; KAS delegates non-SIU updates to generic | deepen (defaulted `turn_origin`) |
| `cyril-core/src/types/event.rs` | 719 | `Notification`, `RoutedNotification{session_id, notification, turn}` | deepen |
| `cyril-core/src/protocol/turn_mediator.rs` | 370 | single active turn, cyril-owned only; `observe` terminal table; companion ledger | deepen |
| `cyril-core/src/protocol/domain_mediator/inbound.rs` | 196 | converts + routes; stamps liveness; applies dispositions | deepen (wiring) |
| `cyril-core/src/protocol/turn_liveness.rs` | 112 | begin/stamp/end/check | retain (called from the new server-turn begin) |
| `cyril-core/src/session.rs` | — | `SessionController` status machine | deepen (one arm) |
| `cyril-core/src/workflow.rs` | 1,095 | `WorkflowTracker` (unordered map; no completion order) | deepen |
| `cyril-core/src/protocol/convert/kas/workflow.rs` | — | snapshot parsing (`WireSnapshot`, no `runLabel`) | deepen (one optional field) |
| `cyril-ui/src/turn_labels.rs` | 0 | — | create |
| `cyril-ui/src/state.rs` | 2,983 | UiState: transcript, activity, steer echoes, … (multi-responsibility) | protected parent |
| `cyril-ui/src/subagent_ui.rs` | — | per-session streams | deepen (header/notice insert) |
| `cyril-ui/src/widgets/chat.rs` | 569 | renders `ChatMessageKind` incl. `System` (italic `theme.system`) | retain (no change) |
| `crates/cyril/src/app.rs` | 2,939 | orchestrator (routing, key handling, tracker, pickers, …) | protected parent |

### Alternatives

**A — transporting the agent-initiated tag** (interface choice; three shapes):
1. *Minimal interface:* `convert_session_update` returns `Vec<Notification>`; KAS emits `AgentInitiated{reason}` beside the content notification. Deep for KAS, but widens the trait for every engine and every call site; per-frame duplicates flood routing.
2. *Common caller trivial:* wrapper variant `Notification::AgentInitiated{reason, inner: Box<Notification>}`. Every consumer (`SessionController`, `UiState`, App routing, subagent routing) must unwrap; missing one silently drops content.
3. *Extension without leakage (selected):* envelope metadata `RoutedNotification.origin`, filled in `inbound.rs` from a defaulted `Engine::turn_origin`. Content notifications and every existing match are untouched; only the mediator reads `origin`. v2 inherits `None`. Passes deletion (removing it re-spreads tag parsing into consumers), interface (tested via `turn_origin` + `observe`), locality (KAS parsing stays in `convert/kas.rs`). Adapter test: the trait already has two real adapters (V2Engine, KasEngine).

**B — who owns server-started turns and the once-per-turn announcement:**
1. *Consumers infer:* SessionController/UiState set busy on `TurnStarted` and dedupe the header themselves; the bridge stays cyril-only. Fails P3/C8: the bridge still drops the wake `turn_end`; the race leaves turns stuck (model, current rules 2/10).
2. *Separate `wire_turns.rs` beside `TurnMediator`:* tracks wire brackets; `DomainMediator` composes both for busy. Locality test fails: two modules decide "is a turn active", and the race needs them to agree atomically.
3. *Deepen `TurnMediator` (selected):* one module owns "what each inbound frame does to the active turn", now including server-owned turns (C7), bracket attachment for cyril turns (C8), and the once-per-turn announce (C10). Busy stays single-sourced (`is_busy`). Model-verified (10/10).

**C — naming the wake's run (FIFO over completed runs per parent):**
1. *App keeps a FIFO fed from `Notification::Workflow`:* forbidden (CLAUDE.md: workflow notifications are consumed exactly once, by the tracker).
2. *UiState keeps a FIFO fed by App-forwarded completion summaries:* a second consumer by proxy; UI gains workflow bookkeeping.
3. *`WorkflowTracker::take_wake_label(session)` (selected):* the tracker already owns every run's parent and terminal status; it adds an arrival-ordered completion log per parent with a consumed mark. App calls one method; no new consumer.

### Approved module ledger

| Module/path | Interface | Owns | Hides/reuses | Must not own | Adapters | Tests through | Change |
|---|---|---|---|---|---|---|---|
| `convert/kas.rs` | `session_info_to_notification`, `agent_initiation(&acp::SessionUpdate)` (crate-private) | KAS literals: `turn_start`, `notify-`, `notify-wf-`, `agentInitiated`, `agentInitiatedReason`, `notificationSeverity` | `steering_*` helpers | turn state, labels | N/A | unit tests in `kas.rs` | deepen |
| `protocol/engine.rs` | `Engine::turn_origin` (default `None`) | per-engine origin dispatch | `convert::kas::agent_initiation` | parsing logic | V2Engine (default), KasEngine | unit tests | deepen |
| `types/event.rs` | `Notification::{TurnStarted, AgentInitiatedTurn{reason}, EngineMessageInjected{message_id, content, severity}}`, `RoutedNotification::with_origin`, `AgentInitiation` | the types | — | behavior | N/A | compile + unit | deepen |
| `protocol/turn_mediator.rs` | `observe()` → `Disposition::{…, BeginServerTurn}`; `announce(&RoutedNotification) -> bool` | server turns, bracket attach, announce-once per session | companion ledger | notification conversion, UI | N/A | table tests in `turn_mediator.rs` | deepen |
| `domain_mediator/inbound.rs` | existing | stamping `origin`; applying `BeginServerTurn` (liveness begin); emitting `AgentInitiatedTurn` before the first tagged frame | — | turn rules | N/A | bridge harness tests | deepen |
| `session.rs` | `apply_notification` | `TurnStarted` → Busy | — | UI | N/A | unit | deepen |
| `workflow.rs` | `WorkflowTracker::take_wake_label(&SessionId) -> Option<WakeLabel>` | completion log + consumption | run map | UI text | N/A | unit | deepen |
| `convert/kas/workflow.rs` | `WireSnapshot.run_label` | parse `runLabel` | — | — | N/A | unit | deepen |
| `cyril-ui/src/turn_labels.rs` | `header_text(..)`, `notice_text(..)` (crate-private) | all header/notice strings (B1/B3/B4/B5) | — | state, rendering | N/A | unit tests in the module | create |
| `cyril-ui/src/state.rs` | `UiState::{begin_agent_initiated_turn, show_engine_injection}` + arms | calling `turn_labels`, `add_system_message`; `TurnStarted` activity; C6 fallback narrowing | existing steer echo machinery | string formatting; workflow data | N/A | UiState tests | protected parent |
| `cyril-ui/src/subagent_ui.rs` | same two operations per stream | non-main transcripts | — | formatting | N/A | unit | deepen |
| `crates/cyril/src/app.rs` | existing | routing `AgentInitiatedTurn` / `EngineMessageInjected` to tracker + UI | — | text, FIFO, turn rules | N/A | App tests | protected parent |

### Protected parents

| Protected parent | Baseline responsibilities | Allowed change | Forbidden change | Exit condition |
|---|---|---|---|---|
| `crates/cyril/src/app.rs` | orchestration, key handling, routing, tracker ownership | ≤ 2 new `match` arms / helper calls routing the two new notifications (call `take_wake_label`, call the two UiState methods); production delta ≤ +60 lines | string literals of header/notice text; any `notify`/`agentInitiated` literal; turn-state fields | shape fence C21: prod delta ≤ 60, forbidden literals absent |
| `crates/cyril-ui/src/state.rs` | transcript, activity, steering, overlays | two thin methods delegating to `turn_labels`; `TurnStarted` arm; C6 fallback narrowing inside the existing function; production delta ≤ +70 lines | header/notice string construction (`"───"`, `"⚙"`, `"noted mid-turn"` literals); workflow types | shape fence C21 |

## Claims

- **C1** — KAS `session_info_update{kind:"turn_start"}` converts to `Notification::TurnStarted`; the v2 engine never produces it.
- **C2** — `Engine::turn_origin` returns `Some(AgentInitiation{reason})` exactly when an agent message/thought chunk, tool_call or tool_call_update carries `_meta.kiro.agentInitiated == true` (reason `None` when `agentInitiatedReason` is absent), and `None` for every other update, for absent/non-object `_meta`, for `false`, and always on v2.
- **C3** — Every routed frame from `inbound.rs` carries `origin` equal to `turn_origin` of its source update.
- **C4** — `steering_injected` whose `messageId` starts `notify-` converts to `EngineMessageInjected{message_id, content, severity}` and never to `SteeringConsumed`; all other `steering_injected` frames convert exactly as today.
- **C5** — `steering_cleared` drops `notify-` ids; if no id remains the frame converts to nothing (never an empty "clear all"); a frame with an originally empty list still converts to the empty clear-all.
- **C6** — `SteeringConsumed` with an id that matches no steer echo flips only the oldest *id-less* Queued echo; an echo bound to another id is never flipped, and the queued counter changes only when an echo flips.
- **C7** — With no active turn, a wire `TurnStarted` on the mediator's main session begins a server-owned turn (`is_busy` true, liveness begun), whose unstamped `turn_end` is forwarded exactly once as `TurnCompleted` with no companion expected.
- **C8** — Under every interleaving of {dispatch, wake `turn_start`/`turn_end`, prompt `turn_start`/`turn_end`, prompt response} that KAS can produce, the mediator ends idle with exactly one forwarded `TurnCompleted` per wire bracket and no dangling companion.
- **C9** — `TurnStarted` on a session other than the main session never begins or alters the main turn and is forwarded.
- **C10** — For each session and each wire turn, exactly one `AgentInitiatedTurn{reason}` is forwarded, immediately before that turn's first origin-tagged frame; untagged turns produce none.
- **C11** — `SessionController` is Busy from `TurnStarted` until `TurnCompleted`, so Enter during a server turn dispatches `SteerSession` (not `SendPrompt`) and Esc dispatches `CancelRequest`.
- **C12** — `CancelRequest` during a server turn sends `session/cancel` for that turn's session.
- **C13** — `WorkflowTracker::take_wake_label(s)` returns the oldest not-yet-taken terminal completion whose parent is `s`, as `{name: runLabel → workflowName → workflowId, status}`, and `None` when none remains.
- **C14** — App: `AgentInitiatedTurn{reason:"workflow-complete-wake"}` on session `s` takes one wake label from the tracker and inserts the B1 header (B4 when `None`); any other or absent reason inserts the B3 header without taking a label; `EngineMessageInjected` with `notify-wf-` takes one label and inserts the B5 workflow notice; any other `notify-` inserts the B5 step notice.
- **C15** — `turn_labels` produces exactly the approved strings for B1, B3 (incl. `unspecified`), B4, B5 (both kinds; `[notification/<sev>] ` prefix stripped; severity default `info`).
- **C16** — Headers and notices enter the target session's transcript as system-style lines at the current position, after flushing streaming text, for the main transcript and for subagent/step streams.
- **C17** — `TurnStarted` on the main session sets UI activity busy (`Waiting`, rendered "Thinking..."); the existing `TurnCompleted` arm returns it to Ready.
- **C18** — Permission requests during a server turn show the normal approval overlay (the approval path consults no turn state).
- **C19** — v2 behavior is unchanged: the full existing suite passes, and v2 never yields `TurnStarted`, `AgentInitiatedTurn`, `EngineMessageInjected`, or a non-`None` origin.
- **C20** — End-to-end: replaying the committed captures through `KasEngine` conversion + `TurnMediator` + `SessionController` + `UiState` satisfies spec success criteria B1, B2 (activity/TurnCompleted), B5 (busy and step), B7.
- **C22** — During a server turn with no inbound frames for longer than the stall threshold, the bridge emits `TurnStalled` scoped to that turn's session, exactly as for a cyril-owned turn.
- **C21** — Module shape matches the approved ledger: KAS literals only in `convert/kas.rs`; `turn_labels.rs` holds every header/notice literal; protected-parent production deltas within bounds; no `acp::` in `cyril-ui`; `convert/mod.rs` production unchanged.

## Falsification

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C1 | turn_start → TurnStarted | S1–S3, D8 | feed a `turn_start` SIU (from the tail capture) to `session_info_to_notification` → `Some(TurnStarted)`; v2 `convert_session_update` on the same frame → not `TurnStarted`. Other cause: none (direct call). | raw capture census: `grep -c '"kind": "turn_start"'` per capture vs count of `TurnStarted` produced by converting every frame | M1: delete the `Some("turn_start")` arm in `kas.rs` → fence `turn_start_converts_to_turn_started` red (`None` ≠ `Some(TurnStarted)`) | unit test in `convert/kas.rs` | minutes | PENDING — checkpointed-build, slice assigned in plan.md |
| C2 | tag extraction | S4–S8 | table test: 4 update kinds × {true+reason, true no reason, false, absent key, `_meta` absent, `_meta.kiro` non-object}; v2 row. Other cause: a reason read from the wrong key → covered by distinct-reason rows. | P1/P2 oracle numbers: tagged counts per kind per capture (57/2/4; 292/6/8) must equal `turn_origin` Some-counts over the captures | M2: make KAS `turn_origin` ignore `ToolCallUpdate` (return `None`) → capture-count row red (4 ≠ 0) | unit + capture-count test in `convert/kas.rs` | minutes | PENDING — checkpointed-build |
| C3 | origin stamped | S4 transport | bridge harness: fake agent emits a tagged chunk; App receiver sees `routed.origin == Some(..)`; untagged chunk → `None`. Positive control: tagged frame present on the wire. | harness script's own frame list (what it sent) | M3: remove `.with_origin(..)` in `inbound.rs` → harness test red | bridge harness test | minutes | PENDING — checkpointed-build |
| C4 | notify → EngineMessageInjected | S11–S16, D5 | frames: `notify-wf-x`, `notify-y` w/ and w/o prefix, `steer-z`, empty id → expected variants. Other cause: prefix check on content instead of id → covered by a `steer-` id with `[notification/…]` content row. | P6 live frames (busy/cancel legs) as fixtures; expected variants hand-derived from the ids | M4: route `notify-` ids to `SteeringConsumed` (remove the prefix branch) → fence red | unit test in `convert/kas.rs` | minutes | PENDING — checkpointed-build |
| C5 | cleared filtering | S17–S19 | `[notify-a]` → `None`; `[notify-a, steer-b]` → `[steer-b]`; `[]` → `SteeringCleared{[]}`. Absence control: `[steer-b]` → unchanged. | hand-derived table from spec B6 + the v2 empty-list rule (`event.rs:252` doc) | M5: forward the filtered empty list instead of `None` → `[notify-a]` row red (`Some([])` ≠ `None`) | unit test in `convert/kas.rs` | minutes | PENDING — checkpointed-build |
| C6 | steer fallback narrowing | S16, cyril-5n75 | UiState: echo bound `steer-A` Queued; `SteeringConsumed{notify-X}` → `steer-A` Queued, counter unchanged; control: id-less echo + unknown id → flips (legacy path still works). | cyril-5n75 acceptance (hand-written expected states) | M6: restore the unconditional FIFO fallback in `flip_consumed_steer_echo` → fence red (`Applied` ≠ `Queued`) | UiState test in `state.rs` | minutes | PENDING — checkpointed-build |
| C7 | server turn owned | S1, S9, S10 | mediator: idle + main `TurnStarted` → `BeginServerTurn`, `is_busy`; unstamped `turn_end` → `ForwardTurnComplete`, companion none; second `turn_end` → `DropUnowned`. | `oracles/mediator_model.py` (proposed rules) scenario `idle-wake` | M7: map idle `TurnStarted` to `Forward` (today's behaviour) → fence red (`DropUnowned` on first `turn_end`) | table test in `turn_mediator.rs` | minutes | PENDING — checkpointed-build |
| C8 | no stuck state | D7, removed invariants 1–5 | Rust table test replaying the 10 model scenarios through `TurnMediator`; each ends idle, forwards = brackets, no companion. | `oracles/mediator_model.py` (independent Python model; positive control `MEDIATOR_RULES=current` fails 8/10) | M8: in the "active cyril, bracket unopened" branch, begin a server turn instead of attaching (drop the attach rule) → `prompt E<R` row red (`Busy` stays / forwards 1 ≠ …) | table test in `turn_mediator.rs` | minutes (model: seconds) | **PASS (model)** — run log below; Rust fence PENDING — checkpointed-build |
| C9 | foreign turn_start | S3 | mediator idle + `TurnStarted` on `child-7` → `Forward`, `!is_busy`. | model: main-only rule; wire fact P1 (step turns engine-started) | M9: drop the main-session check → fence red (`is_busy` true) | table test in `turn_mediator.rs` | minutes | PENDING — checkpointed-build |
| C10 | announce once | D6, S4–S7 | per session: 57 tagged frames in one wire turn → exactly 1 `AgentInitiatedTurn`, positioned before the first tagged frame; next turn re-arms; untagged turn → 0; two sessions interleaved → one each. Absence control: an untagged capture turn yields 0 while tagged yields 1. | P1 probe per-turn table (`probe_p1_p7.py` output: tagged turns per session) | M10: never reset the announced flag at `turn_end` → `two-wakes` row red (1 ≠ 2) | bridge harness test | minutes | PENDING — checkpointed-build |
| C11 | busy → steer / cancel | D1, D2 | SessionController: `TurnStarted` → Busy; `TurnCompleted` → Active; App test: after `TurnStarted`, Enter with text → `SteerSession` sent (not `SendPrompt`); Esc → `CancelRequest`. | P5 live: steer accepted during a wake; prompt pre-empts (the behaviour being prevented) | M11: remove the `TurnStarted` arm from `SessionController::apply_notification` → App test red (`SendPrompt` observed) | unit + App test | minutes | PENDING — checkpointed-build |
| C12 | cancel targets server turn | D2 | bridge harness: server turn active on `s`; `CancelRequest` → fake agent receives `session/cancel{sessionId: s}`. | P4 live leg (cancel ends wake) + harness recorded calls | M12: make `cancel_active` ignore server turns (use only cyril-owned sessions) → harness red (no cancel / wrong session) | bridge harness test | minutes | PENDING — checkpointed-build |
| C13 | FIFO wake label | S20–S24, B4 | tracker: runs A(parent s, completed t1), B(parent s, failed t2), C(parent other) → take(s)=A, take(s)=B, take(s)=None; name precedence rows; non-terminal runs never returned. | hand table from spec FIFO row + KAS precedence (`runLabel||workflowName||workflowId`, bundle static) | M13: return the newest instead of the oldest → fence red (B ≠ A) | unit test in `workflow.rs` | minutes | PENDING — checkpointed-build |
| C14 | App routing | S20–S24, B1/B3/B4/B5 | App test: tracker holds completion for `s`; route `AgentInitiatedTurn{workflow-complete-wake}` → header with name; `{send-message-wake}` → generic header and tracker label still available; `EngineMessageInjected{notify-wf-…}` → notice with name. | spec B1/B3/B5 strings (hand-written expected) | M14: take a label for every reason → `send-message-wake` row red (label consumed) | App test | minutes | PENDING — checkpointed-build |
| C15 | exact strings | B1, B3, B4, B5 | table test over every format branch. | spec text (approved literals) copied into expected values by hand | M15: drop the `[notification/<sev>] ` strip → step-notice row red | unit tests in `turn_labels.rs` | minutes | PENDING — checkpointed-build |
| C16 | insertion position | B1/B5 position | UiState: streaming "abc" then notice then "def" + `TurnCompleted` → messages `[AgentText abc, System notice, AgentText def]`; subagent stream same. | P6 busy trace order (injection mid-turn) | M16: append the notice without flushing streaming first → order row red | UiState + subagent_ui tests | minutes | PENDING — checkpointed-build |
| C17 | activity | B2/B7 | UiState: `TurnStarted` → `Waiting`; `TurnCompleted` → `Ready`. | spec B7 | M17: remove the `TurnStarted` arm in UiState → red (`Idle`) | UiState test | minutes | PENDING — checkpointed-build |
| C18 | approvals unaffected | D3 | UiState: activity `Waiting` (server turn) + `show_approval` → approval overlay topmost. | gate-on capture: wake-turn Write File permission raised (P1 learning) | M18: gate `show_approval` on `activity != Waiting` → red | UiState test | minutes | PENDING — checkpointed-build |
| C19 | v2 unchanged | D8 | full suite + v2 unit: V2Engine `turn_origin` on a tagged frame → `None`; v2 SIU conversion unchanged. | `cargo nextest` baseline at merge-base vs branch (same v2 test set green) | M19: make the default `turn_origin` parse `_meta` → v2 unit red | unit test in `engine.rs` + CI suite | minutes | PENDING — checkpointed-build |
| C20 | end-to-end replay | B1, B2, B5, B7 success criteria | replay tail, gate-on, busy, cancel, prompt captures through conversion + mediator + SessionController + UiState; assert spec success-criteria observables. | P1–P7 probe/oracle outputs (independent Python/grep) | M20: remove the `BeginServerTurn` application in `inbound.rs` → tail replay red (no `TurnCompleted`, activity stuck) | replay tests (`cyril-core` + `cyril-ui` fixtures derived from captures) | tens of minutes | PENDING — checkpointed-build (final slice) |
| C22 | stall watchdog armed | D4 | bridge harness with a short `stall_threshold`: inject `TurnStarted` (main, no dispatch), then silence → `TurnStalled` scoped to main within threshold + one tick; control: after its `turn_end`, silence emits no `TurnStalled`. Other cause: a stale cyril turn still active → excluded by starting from a fresh session with no dispatch. | P5 prompt leg: a real wake silent ≥ 60 s (the condition the watchdog exists for) | M22: skip `turn_liveness.begin` when applying `BeginServerTurn` → no `TurnStalled` (stamp is a no-op without begin) → fence red | bridge harness test (`stall.rs` pattern) | minutes | PENDING — checkpointed-build |
| C21 | module shape | ledger + protected parents | `.cyril-lki9/oracles/shape.py`: literal census, prod-line deltas vs merge-base, `acp::` in cyril-ui, `convert/mod.rs` delta 0. | `git diff --numstat` + `git grep` computed independently of the script's own parsing | M21: add `"notify-"` literal to `app.rs` → shape red `C21 FAIL app.rs:<n> forbidden literal`; M21b: add 80 lines to `state.rs` → delta red | `.cyril-lki9/oracles/shape.py` | seconds | PENDING — checkpointed-build (every slice) |

## Non-goals and future work

Permanent non-goals (rationale in `spec.md` Decisions / Out of scope):
- Preventing or diverting KAS wakes (requester: keep the wake).
- Displaying the idle-wake prompt text (not on the wire).
- Changing the rendered style of `System` lines (reused as-is).

Intended future work (verified tracker IDs):
- Step node ids on step-verdict notices; modeling `_kiro/session/notify` — **cyril-fb1m**.
- Labeling wake turns in replayed history after `session/load` — **cyril-99ds**.
- A standalone "run finished" notice when no wake happens — **cyril-zd8u**.
- Full snapshot field coverage beyond `runLabel` (`memoryConfig`, `rootConversationId`, recipe rows) — **cyril-4u4a**.

## Falsifier run log

- **C8 (cheapest), 2026-09-29**, worktree `a299c630` + uncommitted `.cyril-lki9/oracles/mediator_model.py`:
  - `python3 .cyril-lki9/oracles/mediator_model.py` → `== C8 model: 10/10 scenarios PASS`, exit 0 (`logs/c8-model-proposed.txt`).
  - Positive control `MEDIATOR_RULES=current python3 .cyril-lki9/oracles/mediator_model.py` → `2/10 scenarios PASS`, exit 1; `idle-wake` = `DROP_UNOWNED`, forwarded 0 (reproduces P3); every race ordering loses a completion (`logs/c8-model-current-positive-control.txt`).
  - Correction during the run: the first version generated two invalid traces (a rejected dispatch followed by a prompt bracket that could not exist on the wire); removed, with the reason recorded in the script.

## Approval

Requester approval (verbatim): <pending>
Date: <pending>
Approved risk acceptances: None proposed.
