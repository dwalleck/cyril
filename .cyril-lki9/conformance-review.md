# cyril-lki9 — design-conformance review (independent)

Scope: `git diff bfc498b1 HEAD -- crates/`, production code only (outside
`#[cfg(test)]`, `crates/*/tests/`, `crates/*/examples/`, `test_support.rs`).
Branch `fix/cyril-lki9-i2-labels`, HEAD `b52650d8`.

## Phase 1 — reconstruction (before reading the design)

Written from `git diff` and the source tree only; no `.cyril-lki9/`, ADR,
CONTEXT.md or commit bodies consulted.

### Production modules touched (confirmed from the diff)

core: `protocol/convert/kas.rs`, `protocol/convert/kas/workflow.rs`,
`protocol/engine.rs`, `protocol/turn_mediator.rs`,
`protocol/domain_mediator/inbound.rs`, `session.rs`, `workflow.rs`,
`types/event.rs`, `types/mod.rs` (re-export), `types/workflow.rs`.
ui: `turn_labels.rs` (new), `lib.rs` (mod decl), `state.rs`, `subagent_ui.rs`,
`workflow_ui.rs`, `traits.rs`. bin: `app.rs`.
Test-only files also changed: `bridge/tests/current_runtime_contract/*`,
`bridge/tests/harness.rs`, `test_support.rs`, `examples/test_bridge.rs`.

### Per-module interface, responsibility, adapters

| Module | Interface added/altered | Responsibility for this feature |
|---|---|---|
| `convert/kas.rs` | `session_info_to_notification`: `turn_start` → `TurnStarted` (:358); `steering_injected` with `notify-` id → `EngineMessageInjected{workflow_completion = notify-wf-}` (:433); `steering_cleared` strips `notify-` ids, all-notify list → `None` (:455). New `pub(crate) fn agent_initiation(&acp::SessionUpdate) -> Option<AgentInitiation>` (:488). Private consts `notify-`, `notify-wf-`, `workflow-complete-wake` (:512-520). | Owns every KAS dialect literal: parse turn_start, classify engine injections by id, parse `_meta.kiro.agentInitiated{,Reason}` and classify workflow-completion wakes. |
| `convert/kas/workflow.rs` | `WireSnapshot.run_label` (:393), empty→absent, fed into `with_run_label` (:1016). | Parse `finalState.runLabel`. |
| `engine.rs` | `Engine::turn_origin(&self, &acp::SessionNotification) -> Option<AgentInitiation>`, default `None` (:198); `KasEngine` override delegates to `convert::kas::agent_initiation` (:341). | Engine seam deciding the agent-initiated origin of a frame. Two concrete adapters: V2 (default None), KAS (real). |
| `types/event.rs` | `Notification::EngineMessageInjected{message_id, content, severity, workflow_completion}` (:257), `AgentInitiatedTurn(AgentInitiation)` (:428), `TurnStarted` (:434); `RoutedNotification.origin: Option<AgentInitiation>` (pub field, :495) + `with_origin` (:565); new `pub struct AgentInitiation{reason, workflow_completion}` with `new/reason/is_workflow_completion` (:503). | Domain vocabulary; origin as envelope metadata, not content. |
| `types/workflow.rs` | `WorkflowSnapshotMetadata::{with_run_label, run_label}`, `WorkflowSnapshot::run_label`, `WorkflowSnapshotParts.run_label`. | Carry run label. |
| `turn_mediator.rs` | `Disposition::BeginServerTurn` (:116); private `TurnOrigin{Dispatched,Server}` (:164), `ActiveTurn.{origin,bracket_open}`; `announced: HashSet<SessionId>` (:202); `observe(&routed, main: Option<&SessionId>)` signature change (:270); new `pub(crate) fn mediate(routed, main) -> Mediated` (:404); `pub(crate) fn announce` (:440); private `observe_turn_start` (:450); `pub(crate) struct Mediated{disposition, forward: Vec<RoutedNotification>}` (:514). Wire `turn_end` of a Server turn registers no companion. | Owns turn ownership incl. server (agent-started) turns, turn-start bracket attachment to dispatched turns, and the once-per-session-per-turn `AgentInitiatedTurn` announcement ordered before the frame. |
| `domain_mediator/inbound.rs` | `handle_routed` calls `mediate` (:44), arms `turn_liveness.begin` on `BeginServerTurn`, forwards every `mediated.forward` frame in order; `session/update` path stamps `with_origin(engine.turn_origin(&args))` (~:138). | Async side effects only (liveness, source capture, send); decision sequence lives in the mediator. |
| `session.rs` | `apply_notification`: `TurnStarted` moves `Active`/`Error` → `Busy` (:311); other states held. | Enter-steers / Esc-cancels during an agent-started turn. |
| `workflow.rs` (core) | `WorkflowRun::run_label` (:367); `WorkflowTracker.wake_labels: HashMap<SessionId, VecDeque<WorkflowId>>` (:457) filled on first terminal completion with a parent; `pub fn take_wake_label(&mut self, &SessionId) -> Option<WakeLabel>` (:852); `pub struct WakeLabel{name,status}` + `pub fn new` (doc: "exists so presentation code can be exercised") (:464). | Per-parent FIFO of completed runs and name precedence runLabel→workflowName→id. |
| `cyril-ui/turn_labels.rs` (new, 78 prod lines) | `pub fn header_text(&AgentInitiation, Option<&WakeLabel>) -> String`, `pub fn workflow_notice_text(Option<&WakeLabel>, Option<&str>)`, `pub fn step_notice_text(Option<&str>, Option<&str>)`; private `strip_notification_prefix`, `status_word`. | All header/notice text for agent-initiated turns and engine injections. |
| `cyril-ui/traits.rs` | `pub enum Transcript<'a>{Main, Workflow(&SessionId), Subagent(&SessionId)}` (:14). | Names the destination transcript (App decides). |
| `cyril-ui/subagent_ui.rs` | `SubagentStream::push_system` (pub(crate), :77), `SubagentUiState::add_system_message` (:173). | Chronological system line in a subagent stream. |
| `cyril-ui/workflow_ui.rs` | `WorkflowUiState::add_system_message` (:47) — body identical to the subagent one. | Same, for workflow-step streams. |
| `cyril-ui/state.rs` | `apply_notification`: `TurnStarted` Idle/Ready → Waiting (:1204); `AgentInitiatedTurn`/`EngineMessageInjected` → `false` explicitly. New `pub fn begin_agent_initiated_turn(Transcript, &AgentInitiation, Option<&WakeLabel>)` (:1306), `pub fn show_engine_injection(Transcript, content, severity, workflow_completion, label)` (:1318), private `push_transcript_system` (:1334). Steer-echo reconciler: Consumed with an unbound id now falls back only to the oldest *id-less* Queued chip; extracted `flip_oldest_idless_queued` (:1555) shared with Cleared. | Busy indicator on turn start; render headers/notices into the chosen transcript (text delegated to `turn_labels`); cyril-5n75 steer-chip fix. |
| `cyril/app.rs` | New private `annotate_transcript(Transcript, &SessionId, &Notification)` (:1168); called on Workflow (:1372), Subagent (:1389) and Main (:1430) routes; destructures and carries `origin` through the pending-session buffer. | Cross-cutting: take a wake label from the App-owned `WorkflowTracker` only for workflow-completion announcements/injections and hand it to `UiState`. |

### Dependency direction

- `convert/kas.rs` ← `engine.rs` (KasEngine::turn_origin) ← `domain_mediator/inbound.rs` → `turn_mediator.rs` (mediate). Types in `types/event.rs`. Direction core-internal and downward; no new cycle.
- `cyril-ui` → `cyril_core::types::{AgentInitiation, WorkflowRunStatus}` and `cyril_core::workflow::WakeLabel` (types only). **No** `agent_client_protocol` / `acp::` import in cyril-ui or app.rs (grep clean).
- `app.rs` → `WorkflowTracker::take_wake_label` + `UiState::{begin_agent_initiated_turn, show_engine_injection}`.
- KAS wire literals in production outside `convert/kas.rs`: all hits are doc comments (`event.rs`, `turn_mediator.rs`, `engine.rs`, `workflow.rs`, `state.rs:1487`) **except one code literal**: `crates/cyril-ui/src/turn_labels.rs:70` `strip_prefix("[notification/")` — the KAS step-verdict content prefix is parsed in cyril-ui. `turn_start`, `notify-`, `agentInitiated`, `workflow-complete-wake` literals in code occur only in `convert/kas.rs`.

### Seams / adapters / pass-throughs

- `Engine::turn_origin`: real seam, two adapters (V2 default None, KAS delegating to converter). Justified by the existing engine trait pattern.
- `TurnMediator::mediate` / `Mediated`: new deep entry point that owns ordering (announcement then frame). `observe` and `announce` stay `pub(crate)` but their only production caller is `mediate`; `announce` could be private (it is `pub(crate)` for a census test).
- `Transcript` enum: three variants all used by app.rs; not speculative.
- `WorkflowUiState::add_system_message` and `SubagentUiState::add_system_message` are identical pass-throughs to `SubagentStream::push_system` (minor duplication).
- `WakeLabel::new` is `pub` motivated by tests (documented as such).
- `push_transcript_system` is a small private dispatcher; fine.

### Orchestrator growth vs bfc498b1 (production lines only)

| File | +add | −del | net | Kind of code |
|---|---|---|---|---|
| `crates/cyril/src/app.rs` | 55 | 1 | **+54** | ~42 lines `annotate_transcript` (routing + one policy decision: take a label only when `is_workflow_completion()`/`workflow_completion`), 3 call sites, `origin` bind/carry, import. No string formatting, no FIFO or state machine. |
| `crates/cyril-ui/src/state.rs` | 91 | 16 | **+75** | TurnStarted activity transition (8 lines logic); `begin_agent_initiated_turn`/`show_engine_injection`/`push_transcript_system` (~40, delegation to `turn_labels` + transcript dispatch, one `if workflow_completion` choice between two formatters); steer reconciler refactor + cyril-5n75 behaviour change (~20 net). No header string built here. |

Other production deltas: turn_mediator +171, convert/kas.rs +100, event.rs +80,
turn_labels.rs +78 (new), workflow.rs +77, engine.rs +22, inbound.rs +21,
session.rs +18, types/workflow.rs +16, subagent_ui +16, traits +10, workflow_ui +8,
kas/workflow.rs +6.

### Defects / smells observed in Phase 1

1. **Doc-comment misattachment (production)** `app.rs:1158-1167`: `annotate_transcript` was inserted between `handle_notification_inner`'s doc comment ("Route one notification, optionally observing usage … double-count the turn.") and that function, so the old doc now documents `annotate_transcript` and `handle_notification_inner` (:1206) has none. Same pattern in tests: `enter_steers_during_server_turn` stole `esc_marks_cancel_sent_during_stall`'s doc (app.rs ~:3843).
2. **Stale comment** `state.rs:1196-1199`: "rendered by the I2 increment … until then they change nothing here" directly above the code that now does change state (TurnStarted) and says the App renders headers.
3. KAS content literal `"[notification/"` in cyril-ui (`turn_labels.rs:70`).
4. `wake_labels` is never pruned when a run is evicted or when no wake ever arrives; `take_wake_label` skips evicted ids lazily. Bounded only by completed parented runs per session.
5. `announced` is cleared on TurnStarted/TurnCompleted regardless of disposition (incl. absorbed companion terminals) — benign.

### Tests reaching past an interface (noted, not failures)

- `turn_mediator.rs` tests read private `m.active` (`origin`, `bracket_open`) to classify ATTACH (~:1002-1009) and `m.companion` (~:1044, :1196).
- `announce_matches_tagged_turn_census` (:1241) drives `observe` + `announce` directly, re-implementing `mediate`'s sequencing (and calls `announce` even for dropped/absorbed frames, which `mediate` does not).
- app.rs tests use `app.ui_state` / `app.session` fields (same-module, normal for App tests).

## Phase 2 — comparison with the approved ledger

Design read: `.cyril-lki9/design.md` §Placement, §Module shape (Length review,
Inventory, Alternatives, Approved module ledger, Protected parents), §Technical
proof corrections; C21 claim text consulted for the literal rule the protected
parents cite.

### Ledger rows

| # | Ledger row | Verdict | Evidence |
|---|---|---|---|
| L1 | `convert/kas.rs`: `session_info_to_notification`, crate-private `agent_initiation`; owns `turn_start`, `notify-`, `notify-wf-`, `agentInitiated`, `agentInitiatedReason`, `notificationSeverity` | MATCH | kas.rs:358, :433-453, :455-475, :488 (`pub(crate)`), consts :512/:516, `notificationSeverity` :443. Also owns `workflow-complete-wake` (:520) — consistent with C21. No turn state or label logic. |
| L2 | `engine.rs`: `Engine::turn_origin` default `None`; V2 default, KAS override delegating to `convert::kas::agent_initiation`; no parsing | MATCH | engine.rs:198, :341; test `v2_turn_origin_is_always_none`. |
| L3 | `types/event.rs`: `TurnStarted`, `AgentInitiatedTurn{reason}`, `EngineMessageInjected{message_id, content, severity}`, `RoutedNotification::with_origin`, `AgentInitiation` | **MISMATCH (shape, unrecorded)** | event.rs:428 is `AgentInitiatedTurn(AgentInitiation)` (tuple, carries `reason` + `workflow_completion`); :257 `EngineMessageInjected` adds `workflow_completion: bool`; `AgentInitiation` (:503) adds `workflow_completion`. `TurnStarted` :434, `origin` :495, `with_origin` :565 MATCH. The extra classification keeps dialect strings out of consumers (serves C21) but is not in the ledger or corrections. |
| L4 | `turn_mediator.rs`: `observe()` → `Disposition::BeginServerTurn`; `announce(&RoutedNotification) -> bool`; owns server turns, bracket attach, announce-once | **MISMATCH (interface grew, unrecorded)** | `BeginServerTurn` :116, `observe` :270 MATCH but signature gained `main: Option<&SessionId>`; `announce` :440 MATCH. Unledgered: `mediate(routed, main) -> Mediated` (:404) and `Mediated{disposition, forward}` (:514) — the mediator now *constructs* the `AgentInitiatedTurn` frame and orders it before the tagged frame. `announce` has no production caller outside `mediate` yet stays `pub(crate)`. |
| L5 | `domain_mediator/inbound.rs`: stamp `origin`; `BeginServerTurn` → liveness begin; **emit `AgentInitiatedTurn` before the first tagged frame** | **MISMATCH (responsibility relocated)** | stamping :139 MATCH; `BeginServerTurn` → `turn_liveness.begin` :53 MATCH; emission is no longer here — inbound only iterates `mediated.forward` (:78), the frame is built in `turn_mediator.rs` `mediate`. Net effect is the Alternative-B3 "one module owns each frame's effect" direction, but the ledger assigns emission to inbound. |
| L6 | `session.rs`: `TurnStarted` → Busy | MATCH | session.rs:311 (Active/Error → Busy only). |
| L7 | `workflow.rs`: `WorkflowTracker::take_wake_label(&SessionId) -> Option<WakeLabel>`; completion log + consumption; no UI text | MATCH | :852, log :457 fed only from the completion path (`apply` of `RunCompleted`), `WakeLabel` :464 carries name+status, no text. |
| L8 | `convert/kas/workflow.rs`: `WireSnapshot.run_label` | MATCH | :393, :1016 (empty → absent). |
| L9 | `cyril-ui/src/turn_labels.rs`: `header_text(..)`, `notice_text(..)` **crate-private**; every header/notice string | **MISMATCH (visibility + interface)** | `lib.rs:18 pub mod turn_labels`; `pub fn header_text` :19, `pub fn workflow_notice_text` :40, `pub fn step_notice_text` :59 — public cyril-ui API although only `state.rs` calls them; `notice_text` split in two. Holds all header/notice literals: MATCH. Also parses the KAS content literal `"[notification/"` (:70) — see M6. |
| L10 | `cyril-ui/src/state.rs` (protected): `begin_agent_initiated_turn`, `show_engine_injection` + arms; `TurnStarted` activity; C6 fallback narrowing; no formatting, no workflow data | MATCH with one flag | methods :1306, :1318 delegate to `turn_labels`; `TurnStarted` arm :1204; new-variant arms :1215; C6 narrowing :1510 + helper `flip_oldest_idless_queued` :1555 (helper recorded by the Length review). No `───`/`⚙`/`noted mid-turn` literal in production. Flag: signatures name `cyril_core::workflow::WakeLabel` (:1310, :1324) — see M5. |
| L11 | `cyril-ui/src/subagent_ui.rs`: "same two operations per stream" | **MISMATCH (shape)** | subagent_ui provides a generic `add_system_message(session, text)` (:173) + `SubagentStream::push_system` (:77); the two operations exist only on `UiState` and dispatch via `Transcript`. `workflow_ui.rs:47` gets an identical method and has no ledger row. |
| L12 | `app.rs` (protected): route the two notifications to tracker + UI; no text/FIFO/turn rules | MATCH (one borderline) | `annotate_transcript` :1168 has exactly the two arms; calls `take_wake_label` + the two UiState methods; no literals, no FIFO, no turn-state fields. Borderline: 3 helper call sites (:1372 Workflow, :1389 Subagent, :1430 Main) vs "≤ 2 new match arms / helper calls". |

### Protected parents

| Parent | Rule | Observed | Verdict |
|---|---|---|---|
| `app.rs` | prod delta ≤ +60 | +55 −1 = **+54** (no `#[cfg(test)]` items added above the test module) | MATCH |
| `app.rs` | no header/notice text literals; no `notify`/`agentInitiated` literal; no turn-state fields | grep over lines 1-2993: none | MATCH |
| `app.rs` | ≤ 2 new arms / helper calls | 2 arms, 3 call sites (one per transcript) | borderline → M7 |
| `state.rs` | prod delta ≤ +80 (recorded raise from +70) | +91 −16 = **+75** | MATCH (recorded) |
| `state.rs` | no `───`/`⚙`/`noted mid-turn` construction | none in production | MATCH |
| `state.rs` | no workflow types | `WakeLabel` (a `cyril_core::workflow` type) in two pub signatures, passed through opaquely | MISMATCH → M5 |

### Placement "Forbidden" column

| Forbidden | Verdict |
|---|---|
| Kiro literals in `convert/mod.rs`; `acp::` outside convert + engine | MATCH — `convert/mod.rs` unchanged; inbound uses the pre-existing `args`; no `acp::` in cyril-ui/app. |
| changing `convert_session_update` return; origin fields on `AgentMessage`/`ToolCall` | MATCH — neither changed. |
| wrapper `Notification` variant | MATCH — `AgentInitiatedTurn` is a standalone announcement, not a wrapper. |
| second module deciding busy; App/UiState inferring turn boundaries | MATCH — busy single-sourced in `TurnMediator`; UiState only sets activity on `TurnStarted` (allowed arm). |
| App setting Busy on `TurnStarted` | MATCH — SessionController does it (session.rs:311). |
| second consumer of `Notification::Workflow`; App-held run queues | MATCH — queue lives in `WorkflowTracker`. |
| rendering decisions in core or App | MATCH — App picks transcript + label only; core only classifies. |
| formatting in `state.rs`; new render path in `widgets/chat.rs` | MATCH — widgets untouched; `ChatMessageKind::System` reused. |
| label/text formatting, FIFO, turn-boundary logic in App | MATCH. |

### Technical proof corrections / recorded deviations

- C10 / M10b (announced set reset at turn START and END): code resets at both — turn_mediator.rs:276-284. MATCH.
- C8 / M8 (ATTACH judged on effect): scenario runner reads pre/post `bracket_open` around `observe` (~:1002-1009). MATCH.
- Length review (+80 cap, shared id-less helper, deferral cyril-dgyz): observed +75 and the helper at :1555. MATCH.

### Phase-1 responsibilities with no ledger owner

- `TurnMediator::mediate` / `Mediated` (frame ordering + announcement construction) — M3.
- `Transcript` enum in `cyril-ui/src/traits.rs:14` (destination selection vocabulary) — M4.
- `WorkflowUiState::add_system_message` (`workflow_ui.rs:47`) — M4.
- Engine-side `workflow_completion` classification on `AgentInitiation` / `EngineMessageInjected` — M2.
- KAS step-verdict prefix stripping in cyril-ui (`turn_labels.rs:70`) — M6.

### Ledger interfaces not found as specified

- `turn_labels::notice_text` (crate-private) → found as public `workflow_notice_text` + `step_notice_text`.
- `Notification::AgentInitiatedTurn{reason}` → found as `AgentInitiatedTurn(AgentInitiation)`.
- `subagent_ui` "same two operations per stream" → found as generic `add_system_message`.
- inbound "emitting `AgentInitiatedTurn`" → found in `TurnMediator::mediate`.

### Non-ledger defects to fix regardless (from Phase 1)

- `app.rs:1158-1167`: `annotate_transcript` inserted inside `handle_notification_inner`'s doc comment — the old doc now documents the wrong function; same misattachment in tests (`enter_steers_during_server_turn` took `esc_marks_cancel_sent_during_stall`'s doc, ~:3843).
- `state.rs:1196-1199`: stale "rendered by the I2 increment … until then they change nothing here" comment above code that now changes state.
- `WorkflowTracker.wake_labels` is never pruned on run eviction or for completions no wake consumes (lazy skip only).

RESULT: FAIL

1. **M1 — `turn_labels` visibility** (L9): `pub mod` + three `pub fn` where the ledger says crate-private; the split of `notice_text` is unrecorded. Disposition: code fix — `pub(crate) mod turn_labels` / `pub(crate) fn` (no external caller exists); record the `workflow_notice_text`/`step_notice_text` split in the ledger.
2. **M2 — event type shape** (L3): `AgentInitiatedTurn(AgentInitiation)` and the added `workflow_completion` fields differ from the ledger. Disposition: record as a ledger amendment (it moves the `workflow-complete-wake`/`notify-wf-` classification into the converter, which strengthens C21); no code change.
3. **M3 — announcement emission relocated** (L4/L5): `mediate`/`Mediated` in `turn_mediator.rs` build and order `AgentInitiatedTurn`; inbound only forwards; `observe` gained `main`. Disposition: record the ledger amendment (consistent with Alternative B3); optionally make `announce` private and point `announce_matches_tagged_turn_census` at `mediate` (it currently re-implements `mediate`'s sequencing and calls `announce` on absorbed/dropped frames).
4. **M4 — non-main transcript shape** (L11): generic `add_system_message` on `subagent_ui.rs` and on unledgered `workflow_ui.rs`, plus unledgered `Transcript` enum in `traits.rs`. Disposition: record (add `workflow_ui.rs` and `traits.rs` rows; reword the `subagent_ui.rs` row); optionally dedupe the two identical `add_system_message` bodies.
5. **M5 — workflow type in protected `state.rs`** (L10): `WakeLabel` appears in `begin_agent_initiated_turn`/`show_engine_injection` signatures, against "Forbidden: workflow types". Disposition: record it as an allowed opaque pass-through to `turn_labels` (no workflow data is read or stored in `state.rs`), or tighten the rule's wording; a code change would push text into App, which is worse.
6. **M6 — KAS content literal in cyril-ui** (L9/C21): `turn_labels.rs:70` strips KAS's `"[notification/<severity>] "` prefix, so a KAS dialect literal lives outside `convert/kas.rs`. Disposition: preferred — strip in `convert/kas.rs` so `EngineMessageInjected.content` carries the bare message; otherwise record an explicit C21 exception.
7. **M7 — App call-site count** (L12): 3 helper call sites vs "≤ 2 arms / helper calls"; delta +54 is within the cap. Disposition: record (one call per transcript route is inherent); no code change.
