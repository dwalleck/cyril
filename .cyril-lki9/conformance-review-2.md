# cyril-lki9 design-conformance review (second, isolated)

Scope: production code in `git diff bfc498b1 -- crates/`, base `bfc498b1`, branch `fix/cyril-lki9-i2-labels`, compared against the **working tree** (uncommitted changes included). Reviewer did not implement the change; the review was read-only.

---

## Phase 1: blind reconstruction

Written from the diff and the source tree alone, before any file under `.cyril-lki9/`, `docs/adr/` or CONTEXT.md, or any commit body, was opened. (One-line commit subjects on the branch were visible through `git log --oneline`.)

### 1.1 Touched production modules: interface and responsibility

| # | Module | Added / altered interface | Responsibility cluster |
|---|---|---|---|
| P1 | `cyril-core/src/protocol/convert/kas.rs` | `session_info_to_notification`: new `turn_start` → `Notification::TurnStarted`. `steering_injected` is split on the id: a `notify-` id becomes `EngineMessageInjected{message_id, content, severity, workflow_completion}`, where `notify-wf-` means `workflow_completion = true`. A step verdict has its `[notification/<sev>] ` prefix removed (`strip_notification_prefix`). `steering_cleared` drops `notify-` ids; a list that named only engine ids converts to `None`, not to drain-all. New `pub(crate) fn agent_initiation(&acp::SessionUpdate) -> Option<AgentInitiation>` reads `_meta.kiro.agentInitiated`/`agentInitiatedReason` on chunk, thought, tool_call and tool_call_update frames, and classifies `workflow-complete-wake`. Private consts hold `notify-`, `notify-wf-` and `workflow-complete-wake`. | KAS dialect: turn-start bracket, engine-injection classification, agent-initiated tag parse. All KAS wire literals stay here. |
| P2 | `cyril-core/src/protocol/convert/kas/workflow.rs` | `WireSnapshot.run_label` (`finalState.runLabel`, OptionalField). An empty value counts as absent. It flows into `WorkflowSnapshotMetadata::with_run_label`. | KAS workflow sub-converter: one more wire field. |
| P3 | `cyril-core/src/protocol/engine.rs` | New `Engine::turn_origin(&SessionNotification) -> Option<AgentInitiation>` defaults to `None`. `KasEngine` overrides it by delegating to `convert::kas::agent_initiation`. | Engine seam: per-engine envelope metadata. |
| P4 | `cyril-core/src/protocol/domain_mediator/inbound.rs` | Stamps `routed.with_origin(engine.turn_origin(args))` on each converted `session/update`. `handle_routed` now calls `turn_mediator.mediate(routed, active_session_id)`. `Disposition::BeginServerTurn` arms `turn_liveness.begin`. Source capture finishes from `mediated.forward.last()`. Every `mediated.forward` frame is forwarded in order. | Bridge inbound adapter: side effects only (liveness, source capture, send). The sequencing decision is delegated. |
| P5 | `cyril-core/src/protocol/turn_mediator.rs` | `Disposition::BeginServerTurn`. `ActiveTurn{origin: TurnOrigin, bracket_open}`. `observe(&routed, main: Option<&SessionId>)` gains a parameter. Private `observe_turn_start`: on idle main it begins a server turn, on the dispatched session it attaches the bracket, and otherwise it forwards. A server turn's `turn_end` registers no companion and does not clear an owed one. New `pub(crate) fn mediate(routed, main) -> Mediated{disposition, forward: Vec<RoutedNotification>}` prepends one `AgentInitiatedTurn` announcement per session per turn. The private `announced: HashSet<SessionId>` is cleared at that session's TurnStarted and TurnCompleted. | Turn ownership (ADR-0004-style single active turn), now covering agent-started turns, plus the per-turn announcement. |
| P6 | `cyril-core/src/types/event.rs` (+ `types/mod.rs` re-export) | `Notification::{EngineMessageInjected{..}, AgentInitiatedTurn(AgentInitiation), TurnStarted}`. `RoutedNotification.origin: Option<AgentInitiation>` + `with_origin`. `pub struct AgentInitiation{reason, workflow_completion}` with `new` (empty reason → None), `reason()` and `is_workflow_completion()`. | Domain vocabulary. The dialect classification crosses the boundary as a bool, so consumers never match on a wire string. |
| P7 | `cyril-core/src/types/workflow.rs` | `WorkflowSnapshotMetadata.run_label` + `with_run_label` / `run_label()`; `WorkflowSnapshot::run_label()`; `WorkflowSnapshotParts.run_label`. | Carries the snapshot's run label. |
| P8 | `cyril-core/src/workflow.rs` | `WorkflowRun.run_label` + accessor. `WorkflowTracker.wake_labels: HashMap<SessionId, VecDeque<WorkflowId>>` (completions queued since the last claim) and `claimed_wake_labels: HashMap<SessionId, Vec<WorkflowId>>` (claimed by the current turn). The completion path (`apply_completion`) enqueues on a *first* terminal transition of a parented run (`changed && terminal && parent`). New `pub fn claim_wake_labels(&SessionId)` moves the queue into the claim and replaces any earlier unconsumed claim. `pub fn take_wake_label(&SessionId) -> Option<WakeLabel>` returns the newest claimed run that resolves and consumes the whole claim. `pub fn take_injection_label(&SessionId) -> Option<WakeLabel>` pops the newest unclaimed completion (LIFO), skipping unresolvable ones. New `pub struct WakeLabel{name, status}` with a pub `new`. Private `label_for_run` applies the precedence `runLabel → workflowName → workflowId` (empties skipped) and debug-logs a run with no status as `None`. | Workflow-domain bookkeeping: which completed run a wake turn / injection names. |
| P9 | `cyril-core/src/session.rs` | `apply_notification(TurnStarted)`: Active/Error → Busy; every other status is held. | Session status: an agent-started turn is Busy, so Enter steers and Esc cancels. |
| P10 | `cyril-ui/src/traits.rs` | `pub enum Transcript<'a>{Main, Workflow(&SessionId), Subagent(&SessionId)}`. | Addressing type: the App picks the transcript and the UI renders into it. |
| P11 | `cyril-ui/src/turn_labels.rs` (+ `lib.rs` `pub(crate) mod`) | `pub(crate)` `header_text(origin, label)`, `workflow_notice_text(label, content)`, `step_notice_text(severity, content)`; private `status_word`. | Every header and notice string (B1/B3/B4/B5), as pure functions. |
| P12 | `cyril-ui/src/state.rs` | `apply_notification`: `TurnStarted` moves Idle/Ready → Waiting. `AgentInitiatedTurn`/`EngineMessageInjected` are explicit no-ops. New `pub fn begin_agent_initiated_turn(Transcript, &AgentInitiation, Option<&WakeLabel>)` and `pub fn show_engine_injection(Transcript, content, severity, workflow_completion, Option<&WakeLabel>)`, with private `push_transcript_system`. **Steer-echo change:** a Consumed id bound to no chip now falls back only to the oldest *id-less* Queued chip (new private `flip_oldest_idless_queued`, shared with the Cleared path). Before, it fell back FIFO over all Queued chips. | UI state: busy indicator, placing header and notice lines, steer-echo reconciliation (cyril-5n75 defense in depth). |
| P13 | `cyril-ui/src/subagent_ui.rs` | `SubagentStream::push_system` (`pub(crate)`) flushes streaming text first. `SubagentUiState::add_system_message(&SessionId, String)` creates the stream on first contact. | Per-session stream: a system line lands in chronological order. |
| P14 | `cyril-ui/src/workflow_ui.rs` | `WorkflowUiState::add_system_message(&SessionId, String)`, the same shape as P13. | Workflow-step stream: the same capability. |
| P15 | `cyril/src/app.rs` | Private `annotate_transcript(Transcript, &SessionId, &Notification)`: `TurnStarted` → `claim_wake_labels`; `AgentInitiatedTurn` → `take_wake_label`, only when the origin is a workflow completion, then `begin_agent_initiated_turn`; `EngineMessageInjected` → `take_injection_label`, only when `workflow_completion`, then `show_engine_injection`. There are three call sites: the Workflow route (before `apply_workflow_notification`), the Subagent route (before `apply_subagent_notification`), and the Main pipeline (after `SessionController`/`UiState` apply, keyed by `session_id` or the current main id). `origin` is destructured and carried into the pending buffer. | Orchestrator: cross-cutting wiring between the tracker (label queue) and the UI (text), in the transcript chosen by routing. |

Out of production scope but touched: `test_support.rs` (`kas_trace_replay` drives `TurnMediator::mediate`), `bridge/tests/*` (harness `chunk_meta`, `cancelled_sessions`, the new `agent_initiated.rs`), and `examples/test_bridge.rs` (print arms).

### 1.2 Dependency direction and concrete adapters

- Wire → `convert::kas` (P1/P2) → domain types (P6/P7). `engine.rs` (P3) is the only caller of `agent_initiation`. `inbound.rs` (P4) is the only caller of `Engine::turn_origin` and `TurnMediator::mediate`.
- `TurnMediator` (P5) is pure and synchronous. `inbound.rs` is its single production adapter; `test_support::kas_trace_replay` is a second, test-only driver.
- The App (P15) depends on `WorkflowTracker` (P8, core) for labels and on `UiState` (P12, ui) for text. The UI depends on core only for the types `AgentInitiation`, `WakeLabel`, `WorkflowRunStatus` and `SessionId`. No `acp::` in cyril-ui (verified by grep). The UI never calls the tracker; the tracker never produces text. The direction is correct: core ← ui ← app.
- Seams:
  - `Engine::turn_origin`: a real seam with two adapters, `KasEngine` (override) and `V2Engine` (default `None`).
  - `Transcript`: a closed enum, not a trait.
  - No new trait has only one implementation.

### 1.3 Pass-throughs and hypothetical seams

- `KasEngine::turn_origin` is a one-line delegation to `convert::kas::agent_initiation`. This matches the existing Engine-to-converter pattern, where the literals stay in `kas.rs`, so it is acceptable.
- `turn_labels::status_word` is a pure pass-through to `WorkflowRunStatus::as_str()`. Trivial; a naming point only.
- `SubagentUiState::add_system_message` and `WorkflowUiState::add_system_message` are identical three-liners over `SubagentStream::push_system`. This is duplication, not a seam.
- `UiState::show_engine_injection` takes a `workflow_completion: bool` and then branches, which is a mild boolean-parameter smell. The two injection kinds could be two methods or a small enum. Not blocking.
- `WakeLabel::new` is `pub` and documented as existing "so presentation code can be exercised with a label directly". In production only `label_for_run` calls it. This is a test-motivated public constructor (a hypothetical-seam smell). It cannot break an invariant (name and status are free-form), so it is not blocking.
- `TurnMediator::observe` stays `pub(crate)`, but its only production caller is now `mediate` in the same module. It could be private, since the in-module tests can still reach it. Not blocking.
- `EngineMessageInjected.message_id` is carried to the App, but no production consumer reads it (the App matches with `..`). Only `examples/test_bridge.rs` prints it. It is a diagnostic field; acceptable under "model all fields".
- `RoutedNotification.origin` is bound in the App only so the pending buffer can re-emit it. The App never reads it, by design (documented).

### 1.4 Protected-parent growth (production lines, counted by hand before the oracle)

The production region is taken as everything before the file's first top-level `#[cfg(test)]` (`mod tests`). No hunk touches the inline `#[cfg(test)]` items above it.

| File | Added | Removed | Net |
|---|---|---|---|
| `crates/cyril/src/app.rs` | 59 | 1 | **+58**: `annotate_transcript` 46 incl. doc, import reflow +2/−1, `origin` binding +4, 3 call sites +3, pending-buffer field +1, main-call guard +3 |
| `crates/cyril-ui/src/state.rs` | 87 | 16 | **+71**: TurnStarted/no-op arms 18, begin/show/push 40, steer-echo doc +6/−3, Consumed fallback +1, Cleared refactor +2/−13, `flip_oldest_idless_queued` +20 |

About 20 lines of the state.rs delta (net about +14) are the cyril-5n75 steer-echo fallback, not wake rendering.

### 1.5 Tests that reach past an interface

- `turn_mediator.rs::run_scenario` and `server_turn_owes_no_companion` read the private `m.active` (`origin`, `bracket_open`) and `m.companion`. `run_scenario` uses them to derive the "ATTACH" log line, which states the reason (so a mutation that skips attachment turns the test red). These are in-module white-box tests. They are acceptable, but they couple to private field names.
- `app.rs::lki9_wake_header_and_injection_routing` / `lki9_step_wake_leaves_main_label` call `app.workflow_tracker.claim_wake_labels` / `take_wake_label` / `take_injection_label` directly to assert queue residue. That re-drives the tracker's claim sequencing outside the App's own notification path, reaching through the App into the tracker. Mild; the same assertions could be made by sending a further `TurnStarted` + `AgentInitiatedTurn` through `handle_notification`.
- `app.rs::lki9_wake_header_and_injection_routing` sends two `AgentInitiatedTurn`s inside one turn (generic, then workflow). The mediator never produces that sequence (one announcement per session per turn). The fixture is synthetic, not wire-shaped, but it does isolate "a generic wake never consumes".
- `state.rs::lki9_notice_lands_in_arrival_order` and `turn_labels.rs` tests build `WakeLabel::new` directly. That is fine for presentation.
- `workflow.rs::parented_completion` rebuilds a snapshot through `into_parts()` (a `pub(crate)` internal) inside the owning crate. Fine.

### 1.6 Non-ledger defects noticed (blind)

1. **Injection-label ordering (semantic, low).** `take_injection_label` pops the NEWEST unclaimed completion. Suppose two runs complete during one busy turn before KAS drains its steering buffer (run_complete e1, run_complete e2, then notify-wf(e1), then notify-wf(e2)). The first notice is then named after e2 and the second after e1. The notice body (KAS's own text) still names the right run, but the cyril-built head does not. FIFO pop would match arrival order. Check this against the spec's wording ("newest completion since the turn started").
2. **Severity lost when the field is absent (low).** For a step verdict, the converter strips `[notification/<sev>] ` from the content, but it takes severity only from `notificationSeverity`. If KAS sends the prefix without the field, the prefix's severity is discarded and the UI shows `info`. The fallback could parse the prefix's severity.
3. **Label state bounds (informational).**
   - `wake_labels` holds at most one entry per terminal parented run and drains on the parent's next `TurnStarted` or on injections. A parent session that never starts another turn (for example, the old main after `/new`) keeps its queue for the process lifetime. It is still bounded by the number of runs, and the tracker's `runs` map is itself never evicted, so this is no new growth class.
   - `claimed_wake_labels` holds at most one entry per session. `claim_wake_labels` inserts an entry even when the queue is empty, so every step session that ever emits `TurnStarted` leaves an empty `Vec` behind.
   - `take_injection_label` leaves an empty `VecDeque` behind.
   - `TurnMediator.announced` is bounded by sessions, but a session whose agent-initiated turn never delivers a `turn_end` (a killed step) is never removed.
   - Cosmetic: the entry could be skipped when the queue is empty, and empty entries could be removed.
4. **Doc drift (cosmetic).**
   - The `WakeLabel::new` doc says "The tracker builds these via `take_wake_label`"; `take_injection_label` builds them too.
   - The `WorkflowTracker.wake_labels` doc says it names "the newest of these", which matches the code but see item 1.
5. **Main-path annotate cost (cosmetic).** `annotate_transcript` is called for every main-pipeline notification, cloning a `SessionId` each time, although only three variants act. A `matches!` guard before the clone would avoid that.
6. **Main-path ordering asymmetry (cosmetic).** On the main path, `annotate_transcript` runs after `UiState::apply_notification`; on the workflow and subagent paths it runs before the stream apply. Order does not matter today, because the stream and UiState applies are no-ops for the two rendering variants and `TurnStarted` only claims. It is still worth a one-line comment.
7. **SessionController Busy without mediator tracking (edge).** `TurnStarted` makes the main session Busy unconditionally (from Active/Error). The mediator can decline to begin a server turn: turn-id exhaustion, or another session's turn active after a `/new` retarget. In the retarget case, the wire `turn_end` still reaches the App and returns the session to Active. In the exhaustion case (u64 space, practically unreachable), the unowned `turn_end` would be dropped and Busy would stick. Informational.
8. **Header and wire literals.** No KAS wire literal appears in production outside `convert/kas.rs` and its `kas/` sub-converter (the `runLabel` field is implicit through serde in `kas/workflow.rs`). Mentions elsewhere are doc comments or tests. No header or notice literal appears in production outside `cyril-ui/src/turn_labels.rs`. `turn_labels` is `pub(crate)`, and its callers are only `UiState` methods. No `acp::` in cyril-ui.
9. **State.rs scope creep.** The Consumed-fallback narrowing in `state.rs` addresses cyril-5n75 (the operator chip drained by a `notify-*` Consumed). With P1 now routing `notify-` ids away from `SteeringConsumed`, this is defense in depth rather than a wake-rendering change. Check that the ledger assigns it to a row (it looks like "C6").

### 1.7 Wake-label semantics as implemented (blind summary)

- Enqueue: on the *first* terminal `run_complete` of a run with a `parent_session_id`, onto that parent's queue. A paused (non-terminal) completion and an absorbed duplicate never enqueue, and neither does a snapshot-attach path.
- Claim: at every wire `TurnStarted` for the session (cyril-dispatched or agent-started, on any transcript route). The claim takes everything queued and discards any earlier unconsumed claim.
- Header: only a workflow-completion wake consumes the claim. It names the newest claimed run that resolves; with none, the header falls back to "workflow ended". A generic wake (`send-message-wake`, unspecified) never touches the claim, which is then discarded at the next claim.
- Injection: a `notify-wf` injection pops the newest completion enqueued since the turn's claim. A step verdict (`notify-` non-wf) never touches labels.

---

## Phase 2: comparison against the approved ledger (as amended)

Read after Phase 1 was written:
- `.cyril-lki9/design.md`: the whole file, including "Isolated design-conformance review (2026-09-30) and approved amendments", the amended C13, and the Length review.
- `.cyril-lki9/spec.md`: B1–B7 and Decisions, where the wake-label row is now "claim at turn start".
- `evidence.md` P8 (one grep line).
- `.cyril-lki9/oracles/shape.py`, executed read-only:
  `C21 PASS (… protected-parent prod deltas {'crates/cyril/src/app.rs': 58, 'crates/cyril-ui/src/state.rs': 71})`.
  This matches the hand count in 1.4 exactly. Caps: app.rs +60, state.rs +80 (raised by the Length review).

`.cyril-lki9/conformance-review.md` was opened only after the mapping below was complete.

### 2.1 Row-by-row mapping (production modules → amended ledger)

| Phase-1 module | Ledger row (as amended) | Verdict | Notes |
|---|---|---|---|
| P1 `convert/kas.rs` | `convert/kas.rs` row; M6 amendment (prefix strip lives here) | MATCH | Holds every KAS literal: `turn_start`, `notify-`, `notify-wf-`, `agentInitiated`, `agentInitiatedReason`, `notificationSeverity`, `[notification/`, `workflow-complete-wake`. `agent_initiation` is `pub(crate)`. No turn state or labels. |
| P2 `convert/kas/workflow.rs` | `convert/kas/workflow.rs` row | MATCH | `WireSnapshot.run_label`; an empty value counts as absent. |
| P3 `engine.rs` | `protocol/engine.rs` row | MATCH | Defaulted `turn_origin`; KAS delegates to `convert::kas::agent_initiation`; V2 default `None` (fence `v2_turn_origin_is_always_none`). |
| P4 `domain_mediator/inbound.rs` | `inbound.rs` row + M3 amendment | MATCH | Stamps `origin`; applies `BeginServerTurn` as `turn_liveness.begin`; forwards `mediated.forward` in order. Announcement construction moved to the mediator per M3. |
| P5 `turn_mediator.rs` | `turn_mediator.rs` row + M3 amendment | MATCH | `observe(routed, main)` → `Disposition::BeginServerTurn`. `mediate` → `Mediated{disposition, forward}`. `announce` is private. Server-turn, attach and announce-once rules are as C7–C10 describe, with M10/M10b turn start and end resets both present. |
| P6 `types/event.rs` (+ `types/mod.rs` re-export) | `types/event.rs` row + M2 amendment | MATCH | `AgentInitiatedTurn(AgentInitiation)`, `EngineMessageInjected{…, workflow_completion}`, `TurnStarted`, `RoutedNotification.origin` / `with_origin`. The `mod.rs` line is a re-export of this row's type. |
| **P7 `types/workflow.rs`** | **none** | **MISMATCH (N1)** | `WorkflowSnapshotMetadata::{with_run_label, run_label}`, `WorkflowSnapshot::run_label`, `WorkflowSnapshotParts.run_label` (+23 lines). This carries `runLabel` from P2 to P8. The ledger names the parse (P2) and the consumer (P8) but has no row, inventory entry or amendment for the domain carrier. The prior review listed its +16 delta but did not flag it. |
| P8 `workflow.rs` | `workflow.rs` row + C13 amendment + M5 (`WakeLabel`) | MATCH | `claim_wake_labels` / `take_wake_label` / `take_injection_label` / `WakeLabel`. The queue is fed only from the completion path (no second `Notification::Workflow` consumer). No UI text. |
| P9 `session.rs` | `session.rs` row | MATCH | `TurnStarted`: Active/Error → Busy; other statuses held. |
| P10 `cyril-ui/traits.rs` `Transcript` | M4 amendment | MATCH | App-decided target. |
| P11 `cyril-ui/turn_labels.rs` + `lib.rs` | `turn_labels.rs` row + M1 record | MATCH | `pub(crate) mod`, `pub(crate) fn header_text / workflow_notice_text / step_notice_text`. It holds every header and notice literal; no KAS literal (the strip moved out). |
| P12 `cyril-ui/state.rs` | `state.rs` protected row + M5 + Length review | MATCH | Two thin delegating methods, a private `push_transcript_system`, `TurnStarted` activity arm, explicit no-op arms, and the C6 narrowing with the recorded shared helper. No header literal. `WakeLabel` appears only as an opaque pass-through (M5). +71 ≤ +80. |
| P13 `cyril-ui/subagent_ui.rs` | `subagent_ui.rs` row reworded by M4 | MATCH | `add_system_message` + `SubagentStream::push_system`. |
| P14 `cyril-ui/workflow_ui.rs` | M4 amendment | MATCH | `add_system_message` via `push_system`. |
| P15 `cyril/app.rs` | `app.rs` protected row + M7 | MATCH | One helper, three call sites (M7). It calls `claim_wake_labels` on `TurnStarted` (C13 amendment), `take_wake_label` / `take_injection_label`, and the two UiState methods. No literals, no queue, no turn-state fields. +58 ≤ +60. |

### 2.2 Wake-label semantics against the amended spec and design

| Amended rule (spec Decisions row, B1, B5; design C13/C14) | Implementation | Verdict |
|---|---|---|
| Each `TurnStarted` on a session claims every completion queued for it so far, discarding any earlier claim | `annotate_transcript` → `claim_wake_labels(session)` on every route. It replaces `claimed_wake_labels[session]` and moves `wake_labels[session]` into it. | MATCH |
| The header names the NEWEST claimed completion | `take_wake_label`: `claimed.iter().rev().find_map(...)`, which consumes the claim | MATCH |
| A `notify-wf` injection names the newest completion that arrived after the running turn started, consuming it | `take_injection_label` → `pop_back` on the post-claim queue | MATCH (see observation O1) |
| Generic wakes never consume a label | The App calls `take_wake_label` only when `origin.is_workflow_completion()`. A step verdict (`workflow_completion == false`) calls nothing. | MATCH |
| B4: no known run gives the nameless header | `None` → `header_text(.., None)` = `─── ⚙ workflow ended · agent follow-up ───` | MATCH (debug-log clause: see N2) |
| Name precedence `runLabel → workflowName → workflowId`; status word | `label_for_run` skips empties, and `WorkflowRunStatus::as_str()` supplies the status word | MATCH |
| Only a terminal `run_complete` for a parented run enqueues | `changed && is_terminal && parent` | MATCH |
| Regression: a silent wake's unused label never names the next wake | `wake_labels_claim_at_turn_start` (tracker) and `lki9_silent_wake_label_never_names_next_wake` (App) | MATCH |

### 2.3 Mismatches and proposed dispositions

- **N1: `crates/cyril-core/src/types/workflow.rs` has no ledger row** (module mapping; low severity, no behavior at issue).
  - What it is: the `run_label` carrier on `WorkflowSnapshotMetadata` / `WorkflowSnapshot` / `WorkflowSnapshotParts`. It is necessary plumbing between the `convert/kas/workflow.rs` parse and the `workflow.rs` consumer, and it maps to neither row as written.
  - Disposition: **ledger amendment (record only)**, which needs requester approval under the design's process. Add a `types/workflow.rs` row, or extend the `convert/kas/workflow.rs` row to "parse `runLabel` and carry it on `WorkflowSnapshotMetadata`". No code change.
  - In the same edit, refresh the ledger text the C13 amendment left stale. These are doc-only (not separately blocking, because the amendment paragraph governs):
    - Placement row "Oldest-unconsumed completed run per parent session".
    - Ledger row `workflow.rs` interface (lists only `take_wake_label`).
    - Falsification row C13, whose falsifier is the old FIFO table and whose named mutation M13 ("return the newest instead of the oldest") is now the *correct* behavior.
    - Route line 10 "FIFO naming per session".

- **N2: spec B3/B4 debug-log clauses are not implemented** (spec conformance; low severity).
  - B3 says "a debug log records the reason". B4 says "a debug log records the missing correlation".
  - No production path logs either: not `annotate_transcript`, not `TurnMediator::mediate`/`announce`, not `take_wake_label`, not `turn_labels`. Every `tracing::` call the diff adds was checked. `label_for_run` logs only the no-status case, and `take_wake_label` returns `None` silently when nothing was claimed, which also runs against CLAUDE.md "Log before returning `None`".
  - Disposition: **code fix**, about two lines:
    - `tracing::debug!` in `WorkflowTracker::take_wake_label` when the claim is absent, empty, or unresolvable (session named).
    - `tracing::debug!` of the reason where the announcement is built (`TurnMediator::mediate`) or where the generic header is chosen (`annotate_transcript`).
  - Alternative: a spec amendment dropping the two log clauses (needs requester approval).

### 2.4 Prior review (`conformance-review.md`) item status

| Prior item | Status now | Evidence |
|---|---|---|
| M1: `turn_labels` visibility; `notice_text` split | **Resolved** (code) + recorded | `lib.rs`: `pub(crate) mod turn_labels`; three `pub(crate) fn`. The split is recorded in the amendments. |
| M2: event type shape | **Resolved** (recorded amendment) | Amendment M2; code unchanged, as intended. |
| M3: announcement in `mediate`; `announce` visibility; census test re-sequencing | **Resolved** | Amendment M3. `announce` is now private (`fn announce`). `announce_matches_tagged_turn_census` iterates `m.mediate(routed, Some(&main)).forward`. Residual nit: `observe` is still `pub(crate)` with no production caller outside the module (O4). |
| M4: `Transcript`, `workflow_ui`, `subagent_ui` shape | **Resolved** (recorded) | Amendment M4. The optional dedupe of the two identical `add_system_message` bodies was not done (O5, non-blocking). |
| M5: `WakeLabel` in `state.rs` | **Resolved** (recorded exception) | Amendment M5; `state.rs` never stores or inspects it. |
| M6: KAS `[notification/` literal in cyril-ui | **Resolved** (code) | The strip is now `convert/kas.rs::strip_notification_prefix`; `turn_labels::step_notice_text` takes bare content; the `shape.py` literal list includes `"[notification/"`. |
| M7: App call-site count | **Resolved** (recorded) | Amendment M7; +58 ≤ +60. |
| Defect: `annotate_transcript` stole `handle_notification_inner`'s doc; test doc stolen | **Resolved** | `annotate_transcript` now sits after `handle_notification` with its own doc, and "Route one notification…" is directly above `handle_notification_inner`. `enter_steers_during_server_turn` has its own doc, and the cyril-14ou C9 doc sits on its own test. |
| Defect: stale `state.rs` "I2 increment" comment | **Resolved** | The arm comment now reads "cyril-lki9 C17 (B7) …" and "the App renders them via `begin_agent_initiated_turn` / `show_engine_injection`". |
| Defect: `wake_labels` never pruned | **Resolved by spec change** (claim at turn start) | Pruned at every `TurnStarted` of the parent session. Residual: a parent session that never starts another turn keeps its queue, still bounded by completed runs, and `runs` is itself never evicted (O3). |
| Note: `announced` cleared regardless of disposition | Unchanged, benign | Still cleared at the session's TurnStarted/TurnCompleted before the disposition is decided; harmless. |

### 2.5 Non-blocking observations

- **O1: Injection naming order.** `take_injection_label` pops LIFO. Suppose two runs complete inside one busy turn before KAS drains the steering buffer (`rc(e1)`, `rc(e2)`, `notify-wf(e1)`, `notify-wf(e2)`). The two cyril-built heads then swap names. The injected body (KAS's own text, shown verbatim) stays correct. This matches the approved "newest" wording, so it is not a conformance defect. If it matters, a spec amendment to FIFO for injections (oldest unclaimed) would fix it.
- **O2: Severity from the prefix.** The converter strips `[notification/<sev>] ` but takes severity only from `notificationSeverity`. With the field absent and the prefix present, the notice shows `info` and the prefix's severity is dropped. This conforms to B5 as written ("or `info` when absent"). A converter-side fallback to the prefix's severity would lose less information.
- **O3: Bounds housekeeping.**
  - `claim_wake_labels` inserts a `claimed_wake_labels` entry even for an empty queue, one per session that ever emits `TurnStarted`, step sessions included.
  - `take_injection_label` leaves empty `VecDeque`s behind.
  - `TurnMediator.announced` keeps a session whose agent-initiated turn never delivers `turn_end`.
  - All three are bounded by the number of sessions or runs, with no new growth class.
- **O4: `TurnMediator::observe` could be private.** Its only production caller is `mediate`; the in-module tests can still reach it.
- **O5: Duplicate bodies.** `SubagentUiState::add_system_message` and `WorkflowUiState::add_system_message` are identical. `turn_labels::status_word` is a pass-through to `as_str()`.
- **O6: Test-motivated constructor.** `WakeLabel::new` is `pub` only for tests. Its doc says labels are built "via `take_wake_label`"; `take_injection_label` builds them too.
- **O7: Main-path annotate cost and order.**
  - `annotate_transcript` runs, and clones a `SessionId`, for every main-pipeline notification.
  - It runs after `UiState::apply_notification` on the main path, but before the stream apply on the workflow and subagent paths.
  - Both are harmless today; a `matches!` pre-check and a one-line comment would make the intent explicit.
- **O8: App tests reach into the tracker.** Two tests call `app.workflow_tracker.claim_wake_labels` / `take_*` directly to assert residue. The same facts could be observed through `handle_notification` alone. `lki9_wake_header_and_injection_routing` sends two `AgentInitiatedTurn`s inside one turn, a sequence `mediate` never produces.
- **O9: Busy without a tracked turn.** `SessionController` goes Busy on `TurnStarted` even when the mediator declines to track a server turn (turn-id exhaustion). The forwarded `TurnCompleted` restores Active in the `/new`-retarget case. Only the practically unreachable exhaustion case could stick Busy.

RESULT: FAIL
