# Design: cyril-9104 — workflow step questions

Status: APPROVED (delegated, see Approval)

## Route and inputs

- Route: Structural (`route.md`: T1 resolved, T2 yes, T3 no, T4 yes with the delegated decisions below).
- Behavior source: `rivets show cyril-9104` acceptance criteria; ADR-0013 decision 3 and its consequences.
- Wire evidence: `route.md` T1 (gate-off park capture, gate-on wfplan capture, KAS 0.66.43 bundle shapes).

## Decisions (delegated; reported as open questions)

D1. **When a question shows.** A step is *parked with a question* while its run's
run-level `attention[]` holds `{kind:"question"}` for it with no `toolCallId`
(KAS `parkAwaitingUser`). A `question` with a `toolCallId` is a pending
user_input interaction: it already arrives as `session/request_permission` and
is answered in the approval overlay, so it is not a step question here. The
question shows once per park episode (step session + `since`), when the
attention entry first appears, in the main transcript:

```
⚙ workflow "<run>" · step "<node>" is waiting for your answer:
<question text>
↳ /answer @<selector> <your answer>
```

Question text, in order: the step's latest `send_message` question
(`_kiro/session/notify{sender:"step", severity:"warning"}`) since the step's
node last started; else the step's latest agent text from its stream (the park
case: the step's own reply is what it asked); else no text line.

D2. **The parent's wake names the question.** A step's `warning` message to its
parent queues a wake cause for the parent, like a repeat-cap pause (cyril-6lql).
The parent's next message wake (`send-message-wake`) header is
`─── ⚙ workflow "<run>" · step "<node>" asked a question · agent follow-up ───`.
Same eviction as repeat-cap causes: a turn that ends other than Cancelled
evicts it (KAS buffers a note for a busy parent and never wakes for it); a
newer cause for the same run replaces it. `info`/`error` step messages are
not labelled here (cyril-pd4b owns give-up notes).

D3. **The parent's answer is visible.** `_kiro/session/notify{sender:"parent"}`
whose caller is the main session renders where it arrives — inside the
labelled wake — as `⚙ answered "<node>" for you: <message>` (step named via
the tracker's session ownership; an unknown step renders
`⚙ answered a workflow step for you: <message>`). From any other caller it is
debug-logged and not rendered (no transcript of that session is shown).

D4. **Inline answer UX (`/answer`).** Registered with `/workflow` (KAS only).
- `/answer` lists the parked questions (run, step, selector, question text),
  or says none is waiting.
- `/answer <text>` answers the only parked question; with several it lists
  them and asks for a selector; with none it says so. Nothing is sent.
- `/answer @<selector> <text>` picks one. The selector is the step's node id,
  or `<workflow-id>/<node-id>` when several parked steps share that node id.
- A target that is not parked (resumed, answered by the agent, finished,
  unknown) is refused with its state; nothing is sent. Rationale: a
  `session/prompt` to a running step pre-empts its turn (`.cyril-lki9`
  P5), which would cancel the step's work on the agent's answer.
- The answer is sent as `session/prompt` to the step session outside cyril's
  main-turn mediation, so the main session stays usable and its busy guard is
  untouched. A failed prompt surfaces as a bridge error. On dispatch the
  transcript notes `Sent your answer to step "<node>" of workflow "<run>".`

D5. **The /workflow view.** `/workflow status` (tracker view) adds, under each
run, one line per parked question naming the step and the `/answer` selector.
`/workflow status <id>` / `attach` mark a parked step's node line
`— waiting for your answer`.

## Claims / Falsification

| ID | Claim and covered cases | Falsifier / verification | Regression coverage | Status |
|---|---|---|---|---|
| C1 | Run-level `attention[]` is read on all nine lifecycle events and on snapshot state (`state`/`initialState`/`finalState`): kind, since, toolCallId, consentRound, nodeId, nodePath, sessionId. An unreadable entry is skipped with a warn, an unknown kind is skipped with a debug log; neither drops the event. Absent ≠ empty. | Converter tests on capture frames (gateoff 986–988, wfplan 157) and synthetic malformed / unknown-kind entries; fails if the event drops or a field is lost. | `convert/kas/workflow.rs` tests | PASS — receipts/red-converter.txt; gate c3a02be6 |
| C2 | `_kiro/session/notify` converts to `Notification::WorkflowMessage` with every field (recipient, caller?, message, severity, sender, workflowId?, nodeId?, agentName?), both senders. A missing required field or unknown sender/severity drops the frame with a warn. | Converter tests on wfplan 110 (step question), 165 (parent answer), gateoff `FINISHED` success note; malformed rows. | `convert/kas/notify.rs` tests | PASS — receipts/red-converter.txt; gate c3a02be6 |
| C3 | Tracker: parked questions = attention questions without `toolCallId` on non-terminal runs; raised once per (session, since); gone once a later event's attention no longer lists them; question text by D1 precedence (core part). | Tracker unit tests incl. duplicate re-emission (node_paused → paused → run_complete carry the same entry: one raise), resume (`attention:[]`), terminal run, toolCallId entry ignored. | `workflow.rs` tests | PASS — receipts/red-tracker.txt; gate c3a02be6 |
| C4 | Step-question wake cause labels the parent's message wake (D2); evicted on a non-cancelled turn end; newer cause for the run replaces it; workflow-completion and repeat-cap labels unchanged. | Tracker tests + existing lki9/6lql/2lh8 tests unchanged. | `workflow.rs`, `turn_labels.rs` | PASS — red-tracker.txt, red-transcript.txt |
| C5 | Transcript (fixture-driven, App replay through the real bridge path): gate-off park capture shows the question line naming run `park`, step `ask`, text `WAITING`, before the run's completion wake; gate-on wfplan capture shows the parent wake header naming step `create-workflow`, the question line, and `⚙ answered "create-workflow" for you: The proposed plan is exactly right…` inside that wake (after its header, before its footer). | App replay tests on both captures; fail if a line is missing, unnamed, or out of order. Existing replay assertions (seven gateoff headers, 2lh8 footers) still pass. | `app.rs` tests | PASS — receipts/red-transcript.txt |
| C6 | `/answer` resolution (D4): list, single, several → selector needed, `@node`, `@wf/node`, not parked → refused, empty text → usage, no session → message; nothing sent on refusal. | Command unit tests with a tracker built from capture-shaped events and a channel-backed bridge; fail if a refusal sends or a valid answer targets the wrong session. | `commands/answer.rs` tests | PASS — receipts/red-answer.txt |
| C7 | `BridgeCommand::AnswerWorkflowStep` sends `session/prompt{sessionId:<step>, prompt:[text]}` and does not engage the turn mediator: a main `SendPrompt` while the answer is outstanding is still accepted; a prompt error becomes `BridgeError`. | Bridge test against the in-process fake agent. | bridge tests | PASS — receipts/red-answer.txt |
| C8 | End to end on fixtures: gateoff replay to the park (≤ 993), `/answer` → `AnswerWorkflowStep{session_id: sess_e7ec…}`; replaying 995–1019 clears the parked question and completes the run. | App test; fails if the command targets another session or the question survives the resume. | `app.rs` tests | PASS — receipts/red-answer.txt |
| C9 | `/workflow status` and fetched views name the parked step (D5). | Command/format unit tests. | `commands/workflow.rs`, `workflow_format.rs` tests | PASS — receipts/red-answer.txt |
| C10 | Ownership fences: `Notification::Workflow`/`WorkflowSnapshot` still consumed once by the tracker; no KAS literal outside `convert/kas*`; all `⚙` text in `turn_labels.rs`; protected-parent deltas within caps; no ACP type in cyril-ui. | `.cyril-lki9/oracles/shape.py` (C21) + review. | C21 oracle | PASS — C21 oracle in gate c3a02be6 (app.rs +43, state.rs +38); independent review (review-decisions.md) |
| C11 | Live: real cyril (KAS 0.66.43) runs a recipe whose step parks; the question line shows with run and step; `/answer` resumes the step and the run completes. | hex1b-driven TUI run, screen captures in `receipts/`. | — | PASS 2026-10-19 (receipts/live.md, rerun section; head 51d06456) |

## Placement

- `crates/cyril-core/src/types/workflow.rs`: `WorkflowAttentionKind`, `WorkflowAttention`, `WorkflowMessageSender`, `WorkflowMessageSeverity`, `WorkflowMessage`; `attention` on the nine event types and on `WorkflowSnapshotMetadata`.
- `crates/cyril-core/src/protocol/convert/kas/workflow.rs`: lenient attention parsing. New `convert/kas/notify.rs`: `_kiro/session/notify`. `protocol/engine.rs` dispatches it (KasEngine only).
- `crates/cyril-core/src/workflow.rs` (`WorkflowTracker`, owner of run state and wake causes): run attention, parked questions, raise-once, step message text, step-question wake cause, parent-answer resolution. Forbidden: rendering text.
- `crates/cyril-core/src/types/event.rs`: `Notification::WorkflowMessage(Box<…>)`, `BridgeCommand::AnswerWorkflowStep`.
- `crates/cyril-core/src/protocol/domain_mediator/commands/session.rs`: send the step prompt, spawned await, error → `BridgeError`. Forbidden: touching `TurnMediator`.
- `crates/cyril-core/src/commands/answer.rs`: `/answer`; registered beside `/workflow`.
- `crates/cyril-ui/src/turn_labels.rs`: every new `⚙` string. `state.rs`: thin `show_step_question` / `show_parent_answer`. `subagent_ui.rs`: `latest_agent_text`.
- `crates/cyril/src/app.rs`: after each tracker apply, render raised questions; route `WorkflowMessage` to the tracker once and render a parent answer. No business logic.

## Non-goals and future work

- Overriding the agent's answer while the step is already running again (steer the step via `_session/steer`, or pre-empt it) — open product question, reported; nothing implemented.
- Node-level `attention` inside snapshot trees, `_kiro/workflow/list` row attention, permission attention rendering — cyril-4u4a.
- `checkpoint_restore_availability`, buffered step notes replayed as `user_message_chunk` on `session/load` — cyril-fb1m.
- Labels for `info`/`error` step wakes (give-up notes) — cyril-pd4b.
- Rendering step streams (drill-in) — cyril-zd8u.

## Approval

DG 2026-10-18 — delegated by the coordinating agent's brief: "Design the
inline-answer UX deliberately (how the user picks which parked step to answer
when several are parked; what happens if the step is no longer parked) —
record the choices in your gilfoyle design artifacts and list them as open
questions in your report; STOP and report if a product decision is genuinely
needed." D1–D5 are recorded under that delegation and listed as open
questions in the PR and report; none was judged to block the acceptance
criteria.
