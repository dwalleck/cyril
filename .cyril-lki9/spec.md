# Spec: No surprise model turns when a cyril-launched workflow run ends

## Request (verbatim)
> start on cyril-lki9 in a worktree

(cyril-lki9 title: "Cyril-invoked workflow runs trigger an unsolicited, billed model turn on the main session (KAS auto-wake on run_complete)")

## What this is
When a workflow run launched from cyril ends, KAS starts a model turn on the parent session that no one in cyril requested. Today cyril renders that turn as unlabeled agent text appearing without a prompt, and its turn machinery (busy state, stall watchdog, cancel) is keyed to cyril's own prompts. After this change cyril recognizes agent-initiated turns, labels them with a header line, and runs them through the same turn machinery as operator-started turns (busy from `turn_start`). Engine messages KAS injects into a running turn (busy-case workflow completion, relayed step verdicts) are shown as inline notices and no longer disturb the operator's queued steers. The wake itself is kept.

## Roles
- **Cyril operator**: the person typing in cyril's chat who launches `/workflow run` or `/review`. Pays for every model turn; sees the chat transcript.
- **KAS engine**: the agent process. On a terminal `run_complete` it auto-wakes the run's parent session with a model turn (`autoWakeParentOnComplete`, default policy always wakes).

## Behavior

### B1 — Workflow wake turn is labeled
- **Given**: cyril is connected to KAS; the main session is idle; a run launched from that session (`_kiro/workflow/new` with `parentSessionId` = the session) reaches `_kiro/workflow/run_complete` with `status` ∈ {completed, failed, aborted}.
- **When**: KAS emits frames on that session carrying `_meta.kiro.agentInitiated: true, agentInitiatedReason: "workflow-complete-wake"` (no preceding cyril `session/prompt`).
- **Then**: the transcript shows exactly one header line `─── ⚙ workflow "<name>" <status> · agent follow-up ───` immediately before the turn's first agent text or tool-call block, where `<name>` = the run's `runLabel`, else `workflowName`, else `workflowId` (KAS's own precedence), taken from that run's `run_complete`; the turn's agent text and tool calls then render exactly as in an operator-started turn.

### B2 — Wake turn has full turn parity
- **Given**: a wake turn (B1 or B3) is in progress.
- **When**: the turn runs, stalls, requests permission, the operator presses Esc, or the operator types and submits text.
- **Then**: identical to an operator-started turn: the busy indicator is shown from the turn's start until its `turn_end`; the stall watchdog arms and can raise the stalled chip; permission requests show the normal approval overlay; Esc sends `session/cancel` for the session; submitted text takes the existing mid-turn steering path. On `turn_end`, activity returns to idle and exactly one `TurnCompleted` is applied.

### B3 — Other agent-initiated reasons get a generic header
- **Given**: an agent-initiated turn whose `agentInitiatedReason` is anything other than `workflow-complete-wake` (or absent).
- **When**: its first tagged frame arrives.
- **Then**: the transcript shows one header line `─── ⚙ agent-initiated · <reason> ───` (`<reason>` = the raw string, or `unspecified` when absent) before the turn's first block; B2 applies; a debug log records the reason.

### B4 — Workflow wake without a known run
- **Given**: a `workflow-complete-wake` turn arrives but cyril holds no unconsumed `run_complete` for that session (e.g. the run was launched by an earlier cyril process).
- **When**: its first tagged frame arrives.
- **Then**: the header reads `─── ⚙ workflow ended · agent follow-up ───`; B2 applies; a debug log records the missing correlation.

### B5 — Engine messages injected into a turn are shown inline
- **Given**: any turn is in progress on a session cyril renders (operator-started or agent-initiated).
- **When**: a `session_info_update{kind: steering_injected}` arrives whose `messageId` starts with `notify-` (an engine injection, not an operator steer).
- **Then**: the transcript shows one dim notice at that position in the turn:
  - `messageId` starting `notify-wf-` (workflow completion while the session was busy): `⚙ workflow "<name>" <status> (noted mid-turn):` followed by the injected `content` verbatim, where `<name>`/`<status>` come from the oldest unconsumed `run_complete` for that session (same FIFO as B1; consumes it), or `⚙ workflow ended (noted mid-turn):` when none is known;
  - any other `notify-` id (a relayed step verdict): `⚙ workflow step · <severity>: <message>`, where `<severity>` = `notificationSeverity` (or `info` when absent) and `<message>` = `content` with a leading `[notification/<severity>] ` prefix removed when present.

### B6 — Engine injections never touch operator steers
- **Given**: the operator has one or more queued steers (Queued steer chips, `steering_queued` counter N).
- **When**: a `notify-*` `steering_injected` or `steering_cleared` naming only `notify-*` ids arrives.
- **Then**: every operator steer chip keeps its status and the counter stays N. (Absorbs cyril-5n75.)

### B7 — Busy starts at the turn, not at the first tagged frame
- **Given**: the main session has no cyril-dispatched prompt in flight.
- **When**: `session_info_update{kind: turn_start}` arrives for it.
- **Then**: B2 turn parity applies from that frame (busy indicator, watchdog armed, typing steers rather than prompts). The B1/B3 header appears when the first `agentInitiated` frame arrives; if the turn ends with no tagged frame, no header is shown and activity returns to idle on its `turn_end`.

## Success criteria
- **B1 replay**: replaying `experiments/conductor-spike/kas-workflow-channels-06615-restate-gateoff-tail-2.26.0.jsonl` through the converter + UiState yields a transcript where the header `─── ⚙ workflow "audit-channels-2.26.0" completed · agent follow-up ───` appears exactly once, and precedes the wake turn's first agent text; checked by a replay test asserting message order.
- **B2 activity**: in the same replay, activity is busy for every frame between the wake's `turn_start` and `turn_end`, idle after, and exactly one `TurnCompleted` is applied for the wake turn; checked by the replay test.
- **B2 watchdog**: a synthetic wake turn with no frames for longer than the stall threshold raises `TurnStalled` exactly as an operator turn does; checked by a turn-liveness test.
- **B2 approvals**: replaying `kas-workflow-new-06615-gateon-2.26.0.jsonl` shows the Write File permission request of the wake turn in the approval overlay under the wake header; checked by a replay test.
- **B2 cancel/steer**: live probe on KAS 0.66.15: `session/cancel` during a wake turn ends it with `turn_end` (stopReason recorded), and a mid-turn steer is accepted; checked by probe receipts in `.cyril-lki9/evidence.md`.
- **B3**: a synthetic frame with `agentInitiatedReason: "send-message-wake"` produces the generic header once; checked by a converter/UiState test.
- **B4**: a synthetic wake with no prior `run_complete` produces the nameless header once; checked by a UiState test.
- **B5 busy**: replaying `.cyril-lki9/lki9-live-busy-06615-2.26.0.jsonl` shows exactly one notice `⚙ workflow "lki9-quick" completed (noted mid-turn):` + the wake text, positioned among the operator turn's text where the injection arrived; checked by a replay test.
- **B5 step verdict**: replaying `.cyril-lki9/lki9-live-cancel-06615-2.26.0.jsonl` shows `⚙ workflow step · success: OK` once in the wake turn; checked by a replay test.
- **B6**: UiState test: steer queued with id `steer-A`, then `steering_injected{messageId: notify-X}` and `steering_cleared{messageIds: [notify-X]}`: `steer-A` stays Queued, counter unchanged; checked by the test (cyril-5n75 acceptance).
- **B7**: replaying `.cyril-lki9/lki9-live-prompt-06615-2.26.0.jsonl` (silent wake): activity is busy from the wake's `turn_start`, and cyril's input during it is dispatched as a steer, not a prompt; no header is shown for the tagless wake; checked by replay + App test.
- **No regression**: operator-started turns render no header; the full existing test suite and `cargo clippy -- -D warnings` pass.

## Out of scope
This change does NOT include:
- Preventing or diverting the wake (options a and c): rejected in Decisions.
- Displaying the idle-wake prompt text (KAS sends it as a hidden prompt; it is not on the wire). Busy-case wake text IS shown (B5).
- Step node ids on step-verdict notices (the injected frame carries none; correlating with `_kiro/session/notify` is cyril-fb1m).
- A standalone "run finished" notice when no wake turn happens: cyril-zd8u.
- Labeling wake turns in replayed history after session/load: cyril-99ds (replayMarking).
- `/workflow new` authoring: cyril-0asq.
- The v1/v2 (Rust) engine: it has no workflows and no agent-initiated turns.
- Explaining when `rewakeWithInfo` fires: B3 covers whatever it produces.

## Related issues

- cyril-lki9: this change. Live evidence `docs/kiro-2.26.0-wire-audit.md` § 8c.
- ADR-0011 (`docs/adr/0011-ungated-client-driven-workflow-control-plane.md`): cyril owns the workflow control plane, the gate stays off, and "the model cannot launch, author, or mutate a run. Cyril decides what runs." An automatic model turn on completion puts the model back in the post-run path; the decision here must stay consistent with ADR-0011.
- ADR-0004 / `protocol/turn_liveness.rs`: the bridge models at most one active turn, begun by cyril's own prompt. A server-started turn is outside that model.
- cyril-5gb3 / cyril-queu (`/review`, W3): review runs are workflow runs; "steps run in the parent session's workspace" (queu input from p0xt). Any change to parenting must keep `/review` working.
- cyril-0asq: the `/workflow new` authoring design; its gate-on flow depends on a server wake to finish. Blocked on this change.
- cyril-5n75: notify-* injections flip the operator's queued steer chip. **Folded into this change** (B6).
- Evidence: `.cyril-lki9/evidence.md` (P1–P7).
- cyril-fb1m: `_kiro/session/notify` (step→parent relay) is unmodeled. That relay rides the same parent link.
- cyril-otpi: parent `focus_update.activity {turnActive, runningWorkflows, pausedWorkflows}` also rides the parent link.
- cyril-zd8u: workflow step rendering; its step→parent verdicts ride the parent link.
- Code fact: cyril already receives `_kiro/workflow/run_complete` (status + `finalState`) into `WorkflowTracker::apply_event` independently of the wake, but renders nothing from it (`crates/cyril/src/app.rs:1252`: "Tracker state itself still renders nothing — redraw wiring for the run view is cyril-zd8u"). Today the wake turn is therefore the operator's only on-screen completion cue. Suppressing it without a cyril-side completion notice loses that cue. Unverified premise for option (a): whether `run_complete` is still delivered to cyril when `new` omits `parentSessionId`.
- Code fact: the only production `_kiro/workflow/new` sender is `Op::Run` at `crates/cyril-core/src/protocol/domain_mediator/commands/kas.rs:297`, which always sends `parentSessionId: <session>`.

## Decisions

| Question | Decision | Rationale | Implication |
|---|---|---|---|
| When a cyril-launched run ends and KAS auto-wakes the parent session, does cyril prevent the wake turn, allow it visibly, or divert it to another session? | **Allow it, model it, label it** (option b). Cyril does not suppress or divert the wake. | Requester, 2026-09-29: "lets take a very conservative approach: model it, accept it, and label it". Reason given: "as workflows evolve, it could have more useful information". | `parentSessionId` stays on `_kiro/workflow/new` (no launch-side change). Options (a) and (c) are out of scope. The wake turn remains billed; the change makes it first-class and identifiable in cyril. |
| How is a wake turn labeled in the transcript? | **One header line** before the turn: `─── ⚙ workflow "<name>" <status> · agent follow-up ───`. The agent's text and tool calls render normally beneath it. No reconstructed prompt, no per-block tags. | Requester selected "Header line" (2026-09-29). Rejected: the reconstructed prompt (cyril would be inventing text it never received; drifts if KAS changes wording) and per-block tags. | The label is built from cyril-side data only: the `run_complete` that triggered the wake (name, status) plus the wake frames' `_meta.kiro.agentInitiatedReason`. The wake prompt text is never displayed (it is not on the wire). |
| While a wake turn runs, does cyril treat it like an operator-started turn? | **Full parity**: busy indicator shown, Esc cancels it, stall watchdog armed, tool approvals prompted normally, operator typing takes the existing mid-turn steering path. Only the header label differs. | Requester selected "Yes, full parity" (2026-09-29). | One turn model for both origins. Empirical premises for the probe stage: KAS's response to `session/cancel` and to a mid-turn prompt/steer during a server-started turn. |
| What does cyril do with agent-initiated turns whose reason is not `workflow-complete-wake`? | **Generic header** `─── ⚙ agent-initiated · <reason> ───`, same parity treatment, debug log. | Requester selected "Generic header" (2026-09-29). | B3. Future engine wake reasons are always visible, never silent. |
| Which run names the header when several `run_complete`s are pending? | The oldest unconsumed `run_complete` for that session (FIFO); each header consumes one. | Consistency: KAS wakes once per terminal run while the parent is idle; a run that ends while the parent is busy goes to steering and produces no wake. | B1 correlation rule. |
| Null/missing: a workflow wake with no known run. | Nameless header `─── ⚙ workflow ended · agent follow-up ───` + debug log. | Label must never be dropped; no invented name. | B4. |
| Null/missing: `agentInitiatedReason` absent but `agentInitiated: true`. | Generic header with reason `unspecified`. | Same as B3. | B3. |
| Which sessions do headers apply to? | Any session whose transcript cyril renders (main, and step/subagent drill-in streams). | Parity with where frames already render. | B1 and B3 are per-session. |
| Concurrent: the operator submits a prompt at the same moment a wake turn starts. | Whatever KAS does is recorded by the probe; cyril treats the result under B2. No cyril-side queueing. | Parity decision; KAS owns turn admission. | Probe premise. |
| Should engine-injected messages (busy-case wake text `notify-wf-*`, relayed step verdicts `notify-*`) be shown? | **Show both as dim inline notices** at the injection point, with the engine's text. | Requester selected "Show both, dim inline" (2026-09-29), after P6 showed the text is on the wire. Consistent with the earlier rationale to keep the message visible as it evolves. | B5. Replaces the out-of-scope line "busy-parent case not detectable". |
| Fold cyril-5n75 into this change? | **Yes.** | Requester (2026-09-29): showing injections needs the same injection-vs-steer distinction. | B6; cyril-5n75 closes with this change. |
| Step-verdict notice content (the preview showed a node id) | `⚙ workflow step · <severity>: <message>` — no node id. | The injected frame carries only content + severity (P6); node id would need session/notify correlation (cyril-fb1m). Correction disclosed at re-sign-off. | B5 format. |
| A server-started main-session turn that never emits a tagged frame (silent wake, P5) | Busy from `turn_start` (parity); no header; idle on `turn_end`. | P5: a wake can be silent ≥ 60 s; busy cannot wait for the tag. No invented label. | B7. |
| Edge: permission denied / unauthenticated | N/A — auth failure mid-wake behaves as for any turn (existing error path). | Parity. | None. |
| Edge: partial failure, retries/idempotency | A cancelled or failed wake turn is not retried by cyril. | Cyril never initiates the wake. | None. |
| Edges: empty set, max scale, soft-deleted, multi-tenancy, time-zone, replication lag, cache invalidation | N/A — one wake per terminal run on one local session; no storage, tenancy, or time arithmetic involved. | — | None. |

## Approval

Superseded approval (verbatim): "yes" — 2026-09-29, covered B1–B4 before the prove-it stage. Re-opened by evidence P5/P6 (busy-case detectable, wake text on the wire, silent wakes).

Requester approval (verbatim): "yes"
Date: 2026-09-29

Scope of approval: every Decisions row and behaviors B1–B7, including the re-opened rows (injection notices, cyril-5n75 folded in, step-verdict format without node id, silent-wake handling).
