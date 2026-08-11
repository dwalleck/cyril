# cyril-jxfu — falsifiable design: workflow peer-session route + late-claim registry

## Purpose

KAS workflow steps are full peer sessions. Under today's classifier every step
frame lands in an optimistic subagent stream that nothing can list, drill into,
or render — 30 of 33 scoped frames in the live DAG capture misfile
(`.cyril-jxfu/findings.md`). This design adds a third routing destination for
workflow-owned sessions, an ownership query over cyril-6beh's tracker state,
and late-claim re-parenting for the ordering the capture proves dominant.

## Probe grounding (may not be contradicted)

- P1: All 30 step frames classify `Subagent` today; late claim is the ONLY
  observed path (first step frame beats its claim by 6+ frames, both steps).
- P2: `_meta.kiro.workflow` rides only tool-call frames (4/33) — per-frame
  self-identification cannot be the claim mechanism.
- P3: The shipped `WorkflowTracker`, replaying the capture, retains both step
  session ids in node state through `run_complete` — ownership is answerable
  from existing tracker state, including post-terminal.

## Core rule

> A scoped notification whose session id is claimed by any workflow node's
> `session_id` routes to `NotificationRoute::Workflow` — a workflow-owned
> stream store — unless the id IS the main session. Claims land via
> `node_start`/snapshot state (never per-frame meta), and a claim arriving
> after the stream started re-parents the optimistic subagent stream, history
> intact.

## Components

1. **`WorkflowTracker::session_owner(&SessionId) -> Option<(&WorkflowId, &WorkflowNodePath)>`**
   (cyril-core/workflow.rs). Linear scan over `runs × nodes` comparing
   `WorkflowNodeState::session_id`. Deliberately NO derived index: a scan
   cannot desync from the six mutation paths (node_start merge, snapshots,
   completion, preserve_event_only, …), and realistic scale (≤ a few runs ×
   ≤ ~100 nodes) makes it free. C9 budgets it.

2. **`NotificationRoute::Workflow` + fourth classifier input**
   (cyril/app.rs). `classify_notification_route(scope, main, tracked_subagent,
   workflow_owned)` stays a total pure function. Priority:
   - unscoped → `Main`
   - `scope == main` → `Main` — **even if workflow_owned** (a workflow claim
     on the main session is a wire anomaly; protecting main-pipeline
     continuity wins, per cyril-a71q C7)
   - `workflow_owned` → `Workflow` — **beats `tracked_subagent`** (ownership
     is a positive per-id claim from `node_start`; trackedness via
     `list_update` never names workflow steps on any shipped engine, so a
     collision means dialect mixing and the more specific claim wins) — and
     holds even when `main` is `None` (attributable without a main session)
   - remaining rows unchanged: foreign-with-main → `Subagent`;
     no-main-but-tracked → `Subagent`; no-main-unclaimed → `Drop`.

3. **`WorkflowUiState`** (cyril-ui, new `workflow_ui.rs`): per-step-session
   streams reusing the `SubagentStream` message/streaming/tool-index/activity
   machinery (create-on-first-contact `apply_notification`, `adopt(sid,
   stream)` for re-parenting, `streams()`, `any_active()`). Held as a private
   `UiState` field behind delegating methods, same shape as `subagents`.
   Nothing renders it yet — that is cyril-zd8u by design.

4. **Late-claim sweep** (cyril/app.rs, `Notification::Workflow` arm): after a
   workflow event applies with `Ok(true)`, sweep the subagent stream keys; any
   key now workflow-owned is re-parented: removed from `SubagentUiState`
   (unfocusing drill-in if focused) and adopted into `WorkflowUiState` with
   messages intact. Sweeping on state-change rather than event-kind covers
   every claim carrier uniformly: double-emit re-emission, resume-path
   single emission, and snapshot-borne node state.

5. **Frame rate**: `any_workflow_active()` joins the fast-tick disjunction at
   app.rs:313 beside `any_subagent_active()`.

`SubagentTracker`, crew_panel, and the `/sessions`-family commands are
untouched.

## Input shapes

Classifier (all cells reachable in tests; live-reachable subset noted):
- `scope`: `None` | `Some(== main)` | `Some(≠ main)`
- `main`: `None` | `Some`
- `tracked_subagent`: `false` | `true`
- `workflow_owned`: `false` | `true`

Claim carriers (registry population):
- `node_start` without `sessionId` (pre-session emission) → no claim
- `node_start` with `sessionId` as a RE-emission (double-emit path) → claim
- `node_start` with `sessionId` as the FIRST emission (resume path) → claim
- snapshot-borne node state carrying `sessionId` (pause/terminal snapshots) → claim
- duplicate claim (same sid again) → idempotent, no state churn
- terminal run (`run_complete` applied) → ownership PERSISTS (stragglers stay attributed)

Stream-side:
- pre-claim frames exist (optimistic subagent stream, ≥1 message) → adopt with history
- no pre-claim frames → fresh workflow stream on first post-claim frame
- optimistic stream focused at claim time → unfocus (reachable only
  programmatically today; guarded anyway, cost ~3 lines)
- notification kinds: the `SubagentStream`-absorbable set (AgentMessage,
  ToolCallStarted/Updated/Chunk, TurnCompleted, PlanUpdated) vs the ignored
  rest (session_info sub-kinds that don't convert, MetadataUpdated, …) —
  identical to subagent stream behavior, deliberately.

Out of scope shapes: `Notification::Workflow` frames themselves are never
session-scoped (global extension notifications; the App consumes them before
routing — unchanged); v2 sessions (no workflow events exist, `workflow_owned`
is always false there, zero behavior change).

## Removed-invariant sweep (step 2b)

The change is subtractive: it removes "every scoped-not-main frame reaches
`SubagentUiState`". What that guaranteed silently:

1. *Phantom subagent streams kept the fast tick alive while a step streamed*
   (via `any_subagent_active`). After diversion the disjunction loses those
   streams → **C7 restores the property via `any_workflow_active`**. (Main is
   usually busy during a run — the driver tool call holds the turn — but
   attach-to-foreign-run (cyril-0qe6) breaks that assumption later, so wire it
   now.)
2. *`SubagentListUpdated` reconciliation could mark any stream terminated.*
   After re-parenting, list updates can't see workflow streams. Safe: no
   shipped engine emits `list_update` naming workflow step ids (KAS emits
   none; v2 has no workflows) — and if one did, ownership beats trackedness by
   the C1 priority rule.
3. *Drill-in focus target stability.* Focus validates against subagent
   streams; re-parenting removes one → **C8 unfocuses**. `/kill`, `/msg`,
   `/sessions` resolve via `SubagentTracker` (never held step ids) — safe.
4. *Metadata frames for foreign sids landed in subagent streams (absorbed or
   ignored).* They now land in workflow streams with identical
   absorbed-or-ignored semantics — no main-toolbar stamping either way
   (cyril-fh06 unaffected).

## Claims

- **C1** — With `workflow_owned = true` and `scope ≠ main`, the classifier
  returns `Workflow` for every combination of `main` and `tracked_subagent`;
  with `workflow_owned = false` every pre-existing row is byte-identical to
  today's table.
- **C2** — `scope == main` returns `Main` even when `workflow_owned = true`.
- **C3** — `session_owner` resolves a sid claimed by a sessionId-bearing
  `node_start` whether it is the re-emission or the first emission, and by
  snapshot-borne node state; emissions without `sessionId` claim nothing.
- **C4** — Ownership persists through `run_complete`: a straggler frame after
  the terminal snapshot still classifies `Workflow`.
- **C5** — A claim landing after frames created an optimistic subagent stream
  re-parents that stream — same messages, same order — into the workflow
  store, and afterwards no subagent stream key is workflow-owned.
- **C6** — Replaying the real `kas-custom-dag-2.16.0.jsonl` through the real
  conversion path attributes every forwarded frame: parent → main pipeline,
  both step sessions → workflow streams (each holding its step's tool call),
  `SubagentUiState` ends with zero streams, `SubagentTracker` ends empty.
- **C7** — While any workflow stream is Streaming/ToolRunning,
  `any_workflow_active()` is true and the App selects the fast tick.
- **C8** — Re-parenting a focused stream clears subagent drill-in focus.
- **C9** — `session_owner` at adversarial scale (1 run × 200 claimed nodes ×
  10,000 queries) completes within the CI-safe wall-clock budget.

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| 0 | Substrate: fixture + conversion machinery behave as the fences assume | Run existing `workflow_capture_replay_matches_independent_folder`, `workflow_capture_replay_is_state_idempotent`, `schema_deserializes_captured_kas_session_updates` | Committed independent-folder projection + acp deser layer | 2m | **passed** (2026-08-10, see below) | same three cyril-core tests, already in CI |
| 1 | C1 | Exhaustive truth-table enumeration; a build that ORs `workflow_owned` into `tracked_subagent` (→Subagent) fails the owned rows; one that forgets a row fails totality | Expected table hand-derived in-test from this doc, not from the impl | 10m | pending | `classify_notification_route_truth_table` (extended) |
| 2 | C2 | Row `scope==main, owned=true`; a build that tests `workflow_owned` before `scope==main` returns `Workflow` and fails | same hand-derived table | 5m | pending | same test, distinct assert message |
| 3 | C3 | Tracker unit: (a) no-sid emission → `None`; (b) re-emission with sid → `Some`; (c) FIRST-emission sid (fresh tracker) → `Some`; (d) snapshot-only claim → `Some`. A scan reading opening-plan descriptors instead of node state fails (b,c); one keyed off double-emit bookkeeping fails (c); one skipping snapshot-born nodes fails (d) | sids asserted literally against hand-built event fixtures | 15m | pending | new `workflow.rs` unit tests, one assert per shape |
| 4 | C4 | Replay full DAG fixture events through tracker, then `session_owner(step)` → `Some`. A `preserve_event_only`/canonicalize path that drops node session ids fails | probe 3: committed replay projection shows sids survive (independent artifact) | 10m | pending | tracker unit test over the same fixture |
| 5 | C5 | App unit: main created → 2 frames for sid X (→ optimistic stream, 2 messages) → workflow `node_start` claims X → assert subagent streams lack X, workflow stream X has the SAME 2 messages, next X-frame appends there. A sweep that drops instead of adopts fails the message-count assert; a missing sweep fails the key-intersection assert | message texts asserted literally | 30m | pending | new App unit test |
| 6 | C6 | Replay the REAL fixture line-by-line through a `test-support` conversion helper into `App::handle_notification`; assert destination counts and final store states | probe 1 + `oracle.sh` (text-only pipeline, committed) fix the expected ids/counts | 60m | pending | new App replay fence (deterministic, CI) — the AC1 test |
| 7 | C7 | Unit: drive a workflow stream to Streaming; a frame-rate disjunction that omits workflow activity stays slow-tick and fails | assert on the tick selector's inputs/output, not on subagent state | 15m | pending | new UiState/App unit test |
| 8 | C8 | Unit: focus stream X programmatically, claim X, assert `focused_session_id() == None`. Adopt-without-unfocus fails | focus accessor | 10m | pending | same App unit family |
| 9 | C9 | 200 claimed nodes, 10k `session_owner` calls under the test profile against a CI-safe ceiling; an accidentally quadratic or per-call-allocating rewrite blows the budget | wall-clock vs fixed budget (house style, cf. commit 7383590) | 15m | pending | new scale-budget test in `workflow.rs` |

**Cheapest falsifier run (row 0): PASSED 2026-08-10** —
`cargo test -p cyril-core --features kas -- workflow_capture_replay
schema_deserializes_captured_kas` → 3 passed, 0 failed (0.02s):
`workflow_capture_replay_matches_independent_folder`,
`workflow_capture_replay_is_state_idempotent`,
`schema_deserializes_captured_kas_session_updates`. The fixture, the
conversion layer, and the tracker-replay projection all behave as the fences
above assume.

Per-claim distinctness: rows 1/2 share one test but carry distinct assert
messages naming the claim; every other row is its own test with its own
failure output.

## Negative space (deliberately not done)

1. **No rendering.** No run panel, no drill-in, no crew_panel change, no
   TuiState additions; `changed` stays unwired from `redraw_needed` for
   workflow state. Tracked at **cyril-zd8u** (filed from this design).
2. **No `SubagentTracker → SessionTracker` rename.** Decided against doing it
   inline (the issue explicitly left the choice here): the rename touches the
   crew panel and four commands, none on the routing path. Tracked at
   **cyril-kzke** (filed from this design; folds naturally into cyril-mys8).
3. **No `/workflow` commands, no run lifecycle driving.** Tracked at
   **cyril-0qe6** (verified open; explicitly depends on this issue).
4. **No use of `_meta.kiro.workflow` tool-call meta as a claim source.**
   Settled rationale, not deferred work: probe 2 shows it covers 4/33 frames —
   a partial second claim path would mask registry bugs while never sufficing.
5. **No buffering for the pre-main Drop arm.** The `(scoped, no main,
   unclaimed, unowned)` → `Drop` semantics and its warn are byte-unchanged
   (cyril-tglp rationale still holds).
6. **No agent-subtask channel changes.** `OrchestrateSubAgent` pipelines are a
   different mechanism — **cyril-ebqu** / **cyril-fjfu** (verified open).

## Sequencing note

Lands after cyril-6beh (shipped, PR #92): the registry reads the tracker state
that PR created. Blocks cyril-0qe6.
