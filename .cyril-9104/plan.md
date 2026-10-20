# Plan: cyril-9104

One delivery increment (one PR), stacked on #178. Four slices, each green.

## Slice 1: wire — attention and step/parent messages

**Outcome:** C1, C2.
**Changes:** `types/workflow.rs` (attention + message types; `attention` on the nine events and snapshot metadata), `convert/kas/workflow.rs` (lenient attention), new `convert/kas/notify.rs`, `protocol/engine.rs` (KasEngine dispatch), `types/event.rs` (`Notification::WorkflowMessage`), App: the new variant is consumed and not forwarded (debug log until slice 3), `docs/wire-ignored-fields.txt` (drop the `_kiro/session/notify` line; narrow `*.attention` to what stays unread).
**Verification:**
- converter tests on capture frames → red before, green after (receipt `receipts/red-converter.txt`)
- `cargo nextest run -p cyril-core --features kas` → green

## Slice 2: tracker — parked questions and the step-question wake

**Outcome:** C3, C4 (tracker half).
**Changes:** `cyril-core/src/workflow.rs`: run attention, `step_questions`, `take_raised_questions`, `record_message`, `WakeCause::StepQuestion`, `WakeLabel::step_question`, eviction.
**Verification:**
- tracker unit tests → red then green (`receipts/red-tracker.txt`)
- all existing tracker tests green

## Slice 3: transcript — question line, labelled wake, parent answer

**Outcome:** C4 (header), C5.
**Changes:** `turn_labels.rs`, `state.rs`, `subagent_ui.rs`, `app.rs` wiring.
**Verification:**
- `turn_labels` string tests; App replay tests on both captures → red then green (`receipts/red-transcript.txt`)
- existing replay tests unchanged; C21 oracle PASS

## Slice 4: answering — `/answer`, bridge, /workflow view

**Outcome:** C6–C9.
**Changes:** `types/event.rs` (`BridgeCommand::AnswerWorkflowStep`), `domain_mediator/commands/{mod,session}.rs`, new `commands/answer.rs`, `commands/mod.rs` registration, `commands/workflow.rs` tracker summary, `cyril-ui/src/workflow_format.rs`.
**Verification:**
- command, bridge, App fixture tests → red then green (`receipts/red-answer.txt`)
- full ci.yml gate; C21 oracle; live check (C11)
