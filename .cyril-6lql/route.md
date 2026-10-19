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

## Review round 1 (independent review of `69a724d8`: APPROVE, one Medium fixed before merge)

- **Medium, fixed: a pending pause could go stale and mislabel a later wake.** Live sequence in the capture:
  1. The loop run pauses at its cap again (line 580) while the parent's repeat-cap wake is still running.
  2. KAS therefore injects the note mid-turn (583) and never wakes the parent for it.
  3. The run aborts (584).

  The `notify-wf` named the abort, but the pause stayed queued. It would have named the parent's next non-completion wake, for example a cyril-pd4b give-up or another step note on the same notify-parent → `send-message-wake` path, as `repeat "loop" … (2)`. Two guards now prevent that:
  - (a) Queueing a run's `Ended` or newer `RepeatCap` cause evicts that run's pending `RepeatCap`.
  - (b) `WorkflowTracker::end_turn` runs on the session's `TurnCompleted`. If the turn ends for any reason except a cancel, it evicts the session's pending `RepeatCap` causes. A Cancelled turn keeps them (see Review round 2). This matches KAS: a note gets a wake at once, never, or again after a cancel. `rewakeWithInfo` returns while the parent has a running turn and the note is buffered; only `cancel()` re-wakes the buffered notes. Terminal `Ended` causes stay queued, so cyril-lki9 is unchanged.

  Receipt: `receipts/red-stale-pause.txt`. Tests:
  - App `stale_repeat_cap_pause_never_labels_a_later_wake`: the live capture through line 653, then a synthetic `send-message-wake`, which must render generic.
  - Tracker `terminal_completion_evicts_its_runs_pending_pause`, `newer_pause_replaces_the_runs_pending_pause`, and `turn_end_evicts_pauses_that_never_woke`.
  - The 7-wake replay and every lki9 test pass unchanged.
- **Residual limitation (the coordinator files it).** An idle parent plus a retry that parks at an already-exhausted cap sends no note. In the capture, retry at 657–659, then pause at 665 with `iterationsRun:2` and no notify; `holdRepeatCapNote` is not re-armed. If the parent then stays idle and the run is not superseded, the queued pause is cleared only at the parent's next turn start. That turn claims it, and if it is a non-completion wake it would be named after that pause. Telling the two apart needs `_kiro/session/notify` correlation (cyril-fb1m, then cyril-pd4b).

## Review round 2 (delta review of `17a0b718`: APPROVE, one Medium fixed)

- **Medium, fixed.** Guard (b) did not follow KAS's cancel path. In the 0.66.43 bundle, `cancel()` calls `cancelLocalTurn(sid, {rewakeNotifications: true})`. That collects the unread `notification` steering entries and calls `rewakeClearedNotifications`, which waits for the turn to complete and then calls `rewakeWithInfo`, starting a `send-message-wake`. A step or info note that reaches a busy parent goes through the steering buffer's rewake registry, so a repeat-cap note can be re-woken.

  The failing sequence: a parent wake is running, the run pauses at its cap, the note is parked unread, and the user presses Esc. The turn ends Cancelled, KAS re-wakes the parent for the note, and that re-wake rendered a generic header.

  `end_turn` now takes the stop reason and keeps pending pauses on `StopReason::Cancelled`. I did not add the optional refinement (evict when the note was already shown as an `EngineMessageInjected`). KAS re-wakes only notes still *unread* in its steering buffer (`steeringBuffer.getUnread`). The wire does show a read (the note's `steering_injected` is the read event), but cyril cannot tie it to a run: there is no `notify-<uuid>` correlation key (cyril-fb1m), and `cancelLocalTurn` uses the minimum reader index across executions. So cyril keeps the pause and relies on the claim-at-turn-start residual (cyril-32sp). Round-3 delta review (APPROVE) corrected this wording and noted that a plain note left unread gets a `notification-rewake`, which claims the pause under the same residual; `MaxTurnRequests` added to the rstest. A pause whose note was read and so never re-woken stays queued until the next claim.
- **Low, fixed.** Guard (b)'s App wiring is now pinned. `turn_end_wires_pause_eviction` (rstest) runs `TurnStarted` → paused repeat-cap `run_complete` → `TurnCompleted` → `TurnStarted` → `send-message-wake`:
  - EndTurn gives a generic header.
  - Cancelled gives the repeat-cap label.

  Mutation receipt: with the `TurnCompleted` arm removed from `annotate_transcript`, the EndTurn case fails (`receipts/mutation-unwired-end-turn.txt`); restored, it passes.
- Red-first receipt: `receipts/red-cancel-rewake.txt`, covering the tracker rstest `turn_end_evicts_pauses_unless_cancelled` and the App rstest.

## Terminal criterion

Local: the behavior above is delivered, every ci.yml lane passes, CI is green on all platforms, and the lki9 C21 oracle passes.
