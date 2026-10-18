# Route: cyril-2lh8

Change: show credits on agent-initiated turns, and a per-run credit total on a
workflow run's completion line (ADR-0013 consequences).
Date: 2026-10-18. Stacked on cyril-6lql (PR #177): the repeat-cap wake's
credits are attributed to its run through 6lql's repeat-cap label.

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| T1 | Empirical premise | Every needed wire fact is in the live 0.66.43 capture `experiments/conductor-spike/audit-2.32.0/live/kas-workflow-gateoff-2.32.0.jsonl` (conn `main`). Each turn, step and parent wake alike, sends one `turn_completion` with `promptTurnSummaries[].usage` credits before its `turn_end`. An aborted step turn sends `promptTurnSummaries: []` (line 92), which is a real missing-metering case. A step `node_start` carries its `sessionId` before that step's turn. One more fact came from the mediator source, not the wire: a step session's `turn_end` reaches the App only while the parent is busy. `TurnMediator::observe` drops an idle foreign terminal as `DropUnowned`. So credits are attributed when `turn_completion` arrives, not when the turn ends. Kiro's TUI (`kiro-tui-2.32.0.js`, `pRe`/`$z`) sums step-session credits per run, excludes the parent, and prints `Credits: X` on its completion line. This issue widens the total to include attributed parent wakes. | no (resolved) |
| T2 | Structural module shape | No owner changes. Per-run and per-wake credit accounting joins the `WorkflowTracker`, which already owns the per-session wake claim (lki9 / 6lql). It gains `begin_turn`, `record_turn_metering`, `finish_turn`, `run_credits`, and the public value types `AgentTurnClose` and `RunCredits`. The footer text lives in `turn_labels.rs` (C21 R2). `UiState::end_agent_initiated_turn` pushes it through the existing transcript routing. `App::annotate_transcript` routes three notifications it already sees. C21 PASS: `app.rs` production +9, `state.rs` +13. | no |
| T3 | Production-scale risk | An O(1) map update per metering frame, plus one `session_owner` scan, which is the same scan routing already does per frame. | no |
| T4 | Explicit behavior | Behavior comes from the issue's acceptance criteria. The presentation choices below were made here and are flagged as open questions in the PR. (a) Every agent-initiated turn closes, after its `turn_end`, with `╰─ ⚙ agent follow-up · <c> credits`. (b) The wake that follows a run's terminal end is that run's completion line: `╰─ ⚙ workflow "<name>" <status> · run <total> credits · follow-up <c> credits`. The total counts step and creator sessions (`session_owner`) plus every parent wake attributed to the run, including repeat-cap wakes. (c) Missing metering shows no figure, never 0, and is debug-logged. With no figure at all there is no line. Credits are formatted `{:.2}`. | yes |

## Selected route

Local.

## Delivered

- Red-first receipts in `receipts/`:
  - `red-tracker.txt`: 5 failing
  - `red-footer.txt`: 3 failing
  - `red-app-replay.txt`: no footers rendered, and the render assertion failed
- Green, at state level:
  - tracker tests: `completion_wake_closes_with_its_credits_and_the_run_total`, `repeat_cap_and_generic_wakes_close_with_turn_credits_only`, `missing_metering_is_no_figure`
  - `turn_labels::footer_strings`
  - App live replay `wake_credits_and_run_totals_in_live_capture`: 7 footers, header/footer alternation, and run totals equal to the capture's own sums, with the loop run including its repeat-cap wake
- Green, at render level: `run_completion_line_renders`, drawn through `App::draw_frame`.

## Known limits

- A step-session agent-initiated turn (a creator `send-message-wake`) gets its footer only if its `turn_end` reaches the App, which happens only while the parent is busy. Its credits still count toward the run.
- A busy-parent completion (`notify-wf`, no wake) has no completion line that carries a total.

## Terminal criterion

Local: the behavior above is delivered, every ci.yml lane passes, CI is green on all platforms, and the C21 oracle passes.
