# Route: cyril-6lql

Change: label the unsolicited, billed parent wake KAS 0.66.43 starts after a
workflow repeat pauses at its iteration cap, naming the run and the repeat.
Date: 2026-10-18.

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| T1 | Empirical premise | Every wire fact the design uses is already observed. The live 0.66.43 gate-off capture `experiments/conductor-spike/audit-2.32.0/live/kas-workflow-gateoff-2.32.0.jsonl` (conn `main`) shows a `paused` `run_complete` whose `finalState.pause` is `{kind:"maxIterations", nodeId:"loop", maxIterations:1, iterationsRun:1}` (line 445), a `_kiro/session/notify` (451), and a parent `turn_start` (452) whose tagged frames carry `agentInitiatedReason:"send-message-wake"` (482). The shipped bundle (`acp-server.js`, 0.66.43) shows `pause` is a zod union discriminated by `kind` with one member (`maxIterations`), and `sendHeldRepeatCapNote` sends the note, with `wake=true`, right after `run_complete`. A retry that pauses again at its cap without a note (line 665) shows a repeat-cap pause does not always wake. The claim-at-turn-start correlation covers that case. | no |
| T2 | Structural module shape | No owner, seam or dependency moves. The change adds a domain field to existing owners: `types/workflow.rs` (`WorkflowRunPause`, `WorkflowRepeatCap`, `WorkflowSnapshotMetadata::pause`), the KAS workflow converter (`convert/kas/workflow.rs`, lenient `finalState.pause`), the tracker's existing wake queue (`workflow.rs`: `WakeCause`, `WakeLabel` cause, `take_turn_label`), and one header string in `cyril-ui/src/turn_labels.rs`. `App::annotate_transcript` makes one call where it made one before. lki9's C21 module-shape oracle passes (`app.rs` production −2, `state.rs` 0). | no |
| T3 | Production-scale risk | Per-wake work is a scan of one session's queue. No new I/O. | no |
| T4 | Explicit behavior | The issue's acceptance criteria. (a) A repeat-cap `paused` run_complete on a cyril-launched (parented) run labels the next agent-initiated non-completion turn with the run and the repeat node. (b) cyril-lki9's terminal-status labels, including `notify-wf` mid-turn notices, are unchanged. (c) cyril-pd4b's give-up wakes are unchanged: a pause without a structured `maxIterations` reason never queues a label, so pd4b still owns them. Header literal: `─── ⚙ workflow "<name>" paused · repeat "<node>" reached its iteration limit (<max>) · agent follow-up ───`. An unknown `pause.kind` is debug-logged and a malformed one is warned. Neither drops the completion. | yes |

## Selected route

Local. All four tests resolve within existing owners, and the behavior is fully specified by the issue.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec/design/plan | — | N/A — Local route |

## Delivered

- Red-first receipts: `receipts/red-tracker.txt` (4 fail), `receipts/red-converter.txt` (7 fail), `receipts/red-header.txt` (2 fail), `receipts/red-app-replay.txt` (the live replay shows `agent-initiated · send-message-wake` where the label belongs).
- Green: tracker `repeat_cap_*` / `only_a_structured_*` tests; converter `run_pause_parsed_from_final_state` / `unreadable_pause_is_skipped_not_fatal`; `turn_labels::repeat_cap_header_strings`; App `repeat_cap_wake_is_labelled_in_live_capture` (live 0.66.43 capture, all seven parent wakes labelled in wire order, abort notice inside the repeat-cap wake still named); mediator `repeat_cap_wake_is_announced_as_a_message_wake` (characterization). All lki9 replays and tracker tests pass unchanged.
- Gate and CI: see the PR body.

## Terminal criterion

Local: the behavior above is delivered, every ci.yml lane passes, CI is green on all platforms, and the lki9 C21 oracle passes.
