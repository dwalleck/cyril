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
| T4 | Explicit behavior | Behavior comes from the issue's acceptance criteria. The presentation choices below were made here and are flagged as open questions in the PR. (a) Every agent-initiated turn closes, after its `turn_end`, with `╰─ ⚙ agent follow-up · <c> credits`. (b) The wake that follows a run's terminal end is that run's completion line: `╰─ ⚙ workflow "<name>" <status> · run <total> credits · follow-up <c> credits`. The total counts step and creator sessions (`session_owner`) plus every parent wake attributed to the run, including repeat-cap wakes. (c) Missing metering shows no figure, never 0, and is debug-logged. With no figure at all there is no line. Credits are floored to two decimals the way Kiro's TUI floors them: a turn figure as `yae`, a run total as `Gz` (with the epsilon). See review R2. (d) When KAS delivers a completion into a busy parent (`notify-wf`), there is no wake, so that notice is the run's completion line. It carries the run's credits so far: `⚙ workflow "<name>" <status> · run so far <total> credits (noted mid-turn):`. When nothing was metered the segment is left out and the case is debug-logged. This is the user decision of 2026-10-18, relayed by the coordinator: "append the run's steps-only credit total to the mid-turn `notify-wf` completion notice". The total shown is the run's credits so far. That is the steps only, unless an earlier wake was already attributed to the run. | yes |

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
- Busy-parent notice (d). Red-first receipt: `receipts/red-notice-total.txt`. Two tests went red before the change:
  - lki9 busy capture: the expectation is now `⚙ workflow "lki9-quick" completed · run 0.05 credits (noted mid-turn):`.
  - 2.32.0 live replay: the loop run's abort inside its busy repeat-cap wake now reads `… aborted · run 0.26 credits (noted mid-turn):`. That 0.26 is its two step turns; the interrupted wake has not been metered yet.
- Green for (d): tracker `busy_parent_notice_carries_the_run_total` (metered and unmetered, with the debug log), and the `turn_labels::notice_strings` rows.

## Review round 1 (cross-model review of `4affb3a0`, relayed 2026-10-18)

- **R1 (Medium), fixed.** A partial total used to render as complete. The tracker now counts a run's unmetered turns:
  - a workflow-owned turn whose `turn_completion` carries no credit charge
  - an attributed wake that ended unmetered

  Any nonzero count renders as `run ≥ <total> credits (<n> turn(s) unmetered)`. That applies to the completion footer and to the busy-parent notice. The capture fence is the linear run: its first step turn was aborted with `promptTurnSummaries: []` (capture line 92), so it now reads `run ≥ 0.62 credits (1 turn unmetered)`. Receipt: `receipts/red-partial-total.txt`. Tracker rstest: `partial_metering_marks_the_run_total_incomplete` (wake and busy cases).
- **R2 (Low), fixed.** Credits now floor the way `kiro-tui-2.32.0.js` does, read from the bundle:
  - Turn figure (`yae`): `(Math.floor(e*100)/100).toFixed(2)`.
  - Run total (`Gz`): `t=e*100; Math.floor(t+Number.EPSILON*Math.abs(t))/100`, then `toFixed(2)`.

  The capture's strings changed accordingly: linear follow-up 0.2379 → 0.23, bgwatch total 0.1751 → 0.17, park 0.208 → 0.20, and so on. The edge case 0.29 shows as 0.28 for a turn and 0.29 for a run, as it does in Kiro. Receipt: `receipts/red-floor.txt`. Rstest: `credits_floor_like_kiro`.
- **R5 (nit), fixed.** Added `TurnMeteringUpdate::credits()`, which follows the same rule as `TurnMetering::credits` (rstest `turn_metering_update_credits`). The tracker no longer clones the charges just to sum them.
- **R3 (Low), documented rule, not changed: each turn's credits count toward exactly one run.**
  - A wake counts toward the run it is the wake of. Any other turn counts toward the run whose step or creator owns its session.
  - A nested run (launched from inside a step session) keeps its own total. That total is not added up the ownership chain into the outer run, and a wake on the step session that names the nested run is credited to the nested run only.
  - Why not roll up: it would make outer totals include inner ones, so the per-run lines would double-count when summed. Kiro's TUI shows no nested roll-up either. The rule is unambiguous as it stands; a roll-up is a product choice for later.
- **R4 (Low), noted, bounded.** A `turns` record survives when its `turn_end` never reaches the App, until that session's next `begin_turn`. That happens when the mediator drops a foreign terminal while the parent is idle (`DropUnowned`), or for an old main session after `/new` or a disconnect. `run_credits` and `run_unmetered` are never pruned either: one small entry per run, the same as `runs` itself. Both maps are bounded by the sessions and runs seen in one process, the same bounded growth as `claimed_wake_labels`. Pruning would need a session-end signal the tracker does not get today.

## Review round 2 (delta review of `4affb3a0..28c9a44d`)

- **Medium, fixed (a regression introduced in `8c0ba133`).** An agent-initiated turn whose header named no run, on a workflow-owned session (a step woken by a message), credited no run. Both `record_turn_metering` and `finish_turn` now fall back to the session's owning run through `attributed_run`. This is the same wake-run-else-owner rule `4affb3a0` had, and it applies to the unmetered count too. Rstest: `owned_session_wake_counts_toward_its_run` (metered and unmetered cases).
- **Low, fixed.** The busy-parent notice reads `run so far <total> credits (noted mid-turn)`. It is a running total; the same run's later completion line can show more.
- **Nit, fixed.** `TurnMeteringUpdate::credits` now uses Kiro's own unit rule (`Hut`/`dRe`): `(unitPlural || unit).trim().toLowerCase()` must be `credit` or `credits`. Rstest rows cover case, padding, and the plural deciding the match. `TurnMetering::credits` (the toolbar's session cost, outside this change) still matches `unit == "credit"` exactly.

Receipt: `receipts/red-delta-review.txt`.

## Known limits

- A step-session agent-initiated turn (a creator `send-message-wake`) gets its footer only if its `turn_end` reaches the App, which happens only while the parent is busy. Its credits still count toward the run.

## Terminal criterion

Local: the behavior above is delivered, every ci.yml lane passes, CI is green on all platforms, and the C21 oracle passes.
