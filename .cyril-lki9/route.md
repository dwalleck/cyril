# Route: cyril-lki9

Change: Stop cyril-invoked KAS workflow runs from producing an unsolicited, billed model turn in the user's main session (KAS `autoWakeParentOnComplete`), or make any such turn first-class and visible.
Date: 2026-09-29

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | **Covered:** the wake itself. `docs/kiro-2.26.0-wire-audit.md` § 8c; capture `experiments/conductor-spike/kas-workflow-channels-06615-restate-gateoff-tail-2.26.0.jsonl` (kiro-cli 2.26.0 / KAS 0.66.15, 2026-09-29): gate-off `new{parentSessionId: main}` → `invoke` → `run_complete:completed` → `turn_start` on main at +0.0 s → model turn → `turn_end` +13.1 s, with no client prompt. Static: `autoWakeParentOnComplete` keys on `parentSessionId ?? finalState.parentSessionId ?? registry`; the default `wakePolicy` always wakes. **Not covered (each option rests on unprobed KAS behavior):** (a) whether `_kiro/workflow/new` accepts a missing `parentSessionId`; if so, where step `send_message` relays, step and watch permission requests, `node_paused` "awaiting parent session" parks, `focus_update.activity` counters and `workflow/list` rows go, and whether `rewakeWithInfo` still fires. (b) behavior of a client `session/prompt` or `session/cancel` sent during a server-started turn (queue? reject? parallel?) and whether cancelling a wake turn is clean. (c) whether a second, cyril-owned session on the same connection gets the wake instead of the main one, and whether wakes into an unloaded parent trigger `loadSessionOnDemand`. | **yes** |
| 2 | Structural module shape | The fix touches: `cyril-core/src/protocol/domain_mediator/commands/kas.rs:297-303` (`Op::Run` → `kiro/workflow/new` params, which carry `parentSessionId: session_id`); the turn model: `protocol/turn_liveness.rs` ("the bridge's single active turn (ADR-0004: at most one)", armed via `begin` on cyril-originated prompts); `convert/kas.rs` (`session_info_update` kind dispatch has no `turn_start` arm; `turn_end` → `Notification::TurnCompleted` at :346); `cyril-ui/src/state.rs` activity machine (Streaming on first chunk, Idle on TurnCompleted); App routing (`crates/cyril/src/app.rs`, 7,792 lines, a multi-responsibility orchestrator). Options (b) and (c) add a `Notification` variant or change turn ownership across core → UI → App, i.e. a responsibility change in the turn-ownership seam. Option (a) changes the workflow wire contract. Length: `app.rs` 7,792 and `state.rs` 9,098 lines; any new behavior in App triggers the module-shape length review. | **yes** |
| 3 | Production-scale risk | Not scale-bound (one wake per run). Cost is per-turn billing, not throughput. | no |
| 4 | Explicit behavior | Unresolved decisions: which of (a) / (b) / (c), or a combination; whether a wake turn that does happen should be shown, suppressed, or cancelled; whether cyril should forgo the step→parent relay that `parentSessionId` enables. | **no** |

Unknown tests: none

## Selected route

Empirical — T1 fires first: every candidate fix rests on KAS behavior not yet probed (missing-parent semantics, prompt/cancel during a server turn, control-session wake routing).

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | required — T4 no: fix direction and wake-turn UX undecided |
| evidence.md, probe.* | prove-it-prototype | required — Empirical route (T1 yes) |
| design.md | falsifiable-design | required |
| plan.md | budgeted-plan | required |

Oracle checkpoint in `checkpointed-build`: required — Empirical route

## Downstream sequence

interrogated-spec → prove-it-prototype → falsifiable-design → budgeted-plan → checkpointed-build

## Terminal criterion

Empirical — prove-it-prototype records PASS for every empirical premise, every later artifact satisfies its owning stage's completion criterion, and checkpointed-build records no FAIL.
