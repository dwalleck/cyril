# Feature: Model KAS workflow lifecycle state

## What this is

Cyril will convert KAS's nine plain JSON-RPC `_kiro/workflow/*` lifecycle notifications into typed core events and fold them into a pure, deterministic workflow state model. The model exposes canonical current state for downstream W1 routing and control work; Cyril currently drops these extension notifications as unknown.

## Users

- **Cyril W1 contributor**: implements workflow routing, control, and later presentation against one typed state contract instead of parsing Kiro JSON or reconciling two node-identity schemes.

## Behavior

### Convert all lifecycle methods
- **Given**: one valid notification for each of `_kiro/workflow/run_start`, `node_start`, `node_complete`, `node_paused`, `loop_iteration`, `watch_poll`, `paused`, `run_complete`, and `steps_queued`.
- **When**: the KAS extension converter handles the notification.
- **Then**: it emits exactly one typed workflow notification preserving every documented field, including optional `parentSessionId`, `sessionId`, completion-signal fields, timestamps, artifacts, outputs, and reasons.

### Reject a malformed known lifecycle frame
- **Given**: a known workflow method whose payload omits a required field, contains an invalid field type, or uses an unknown enum value.
- **When**: the extension converter handles the notification.
- **Then**: Cyril emits a contextual warning, emits no typed workflow notification, leaves existing workflow state unchanged, and continues processing later ACP frames.

### Preserve unrelated extension behavior
- **Given**: an extension notification outside the nine workflow lifecycle methods.
- **When**: the KAS extension converter handles it.
- **Then**: existing conversion or unknown-extension behavior is unchanged; the workflow model receives nothing.

### Open a run from `run_start`
- **Given**: no canonical run exists for `workflowId`.
- **When**: `run_start` arrives with the recipe name, inputs, node descriptors, and optional parent session.
- **Then**: the model creates one run and one canonical descriptor entry per declared node, scoped by `workflowId`; no concrete status value is invented for fields the event did not provide.

### Reject an event before its run opening
- **Given**: no canonical run exists for `workflowId`.
- **When**: any lifecycle event other than `run_start`, or an explicit full-state seed, arrives.
- **Then**: the model warns, ignores the event, returns “state unchanged,” and creates no partial run.

### Seed or reconcile a full workflow snapshot
- **Given**: a valid `WorkflowState` from `new`, `list/load`, or `run_complete.finalState`.
- **When**: the model ingests the snapshot.
- **Then**: it atomically creates or reconciles the same canonical run and node state used by streamed events; an invalid snapshot returns a structured error and changes nothing.

### Canonicalize repeat iteration identity
- **Given**: a full-state tree whose repeat wrapper is named `loop#N`, and streamed node paths for the same iteration use `iter-N`.
- **When**: the snapshot is canonicalized.
- **Then**: both inputs address the same `(workflowId, nodePath)` entry; `nodeId` and iteration metadata remain data, not identity.

### Merge repeated `node_start` events
- **Given**: a node already exists at `(workflowId, nodePath)` and a later `node_start` supplies the same or additional fields, including the observed second emission with `sessionId`.
- **When**: the event is applied.
- **Then**: missing fields preserve prior values, present fields use the latest supplied value, exact duplicates return “state unchanged,” and no duplicate node is created.

### Reject an update for an unknown node
- **Given**: a known run but no *runtime* node — one materialized by a snapshot or a prior `node_start` — matches the event's `nodePath` or required node identity. (Clarified 2026-08-09: the declared opening plan alone does not make a node addressable; a declared-but-unstarted node receiving `loop_iteration` or any node-addressed update is an unknown node under this rule.)
- **When**: `node_complete`, `node_paused`, `watch_poll`, or `loop_iteration` is applied.
- **Then**: the model warns, ignores the event, returns “state unchanged,” and creates no placeholder node.

### Apply node completion without deriving liveness
- **Given**: a known node and a valid `node_complete` event.
- **When**: the event is applied.
- **Then**: status and supplied result fields update the node; absent optional fields preserve existing data. `completionSignal` and `completionSignalSource` are retained when present but never override node or run status, and their absence on a completed node is normal.

### Keep run and node pauses independent
- **Given**: a valid `paused` or `node_paused` event.
- **When**: the event is applied.
- **Then**: `paused` updates only the run and its pause reason; `node_paused` updates only the addressed node and its reason. Neither event synthesizes the other.

### Preserve queued steps across acknowledgement frames
- **Given**: a populated `steps_queued` event without `resolution`, followed by `steps_queued { pendingSteps: [], resolution: ... }`.
- **When**: both are applied.
- **Then**: the first replaces the canonical pending-step descriptors; the acknowledgement records its resolution separately and does not clear the pending list merely because its array is empty.

### Track repeat and watch progress as current state
- **Given**: repeated valid `loop_iteration` or `watch_poll` events for known nodes.
- **When**: they are applied.
- **Then**: the model retains the latest typed iteration/poll state only; it does not append an unbounded event history.

### Treat paused completion as resumable
- **Given**: `run_complete { status: "paused", finalState }` for a known run.
- **When**: the event is applied.
- **Then**: final state is reconciled, the run remains nonterminal and eligible for later events, and the model does not tear down or freeze it.

### Make terminal completion absorbing within an incarnation
- **Given**: `run_complete` with `completed`, `failed`, or `aborted`.
- **When**: the event is applied.
- **Then**: final state is reconciled and that incarnation becomes terminal. Exact duplicate terminal frames are idempotent; later non-duplicate events other than `run_start` warn and are ignored.

### Start a new incarnation from a post-terminal opening
- **Given**: a terminal run and a later `run_start` with the same `workflowId`, as emitted after Kiro's workflow retry control.
- **When**: the opening is applied.
- **Then**: the model atomically replaces the prior runtime state, descriptors, queue/progress/completion fields, and terminal flag with the new active incarnation under the same persisted workflow id. No prior-incarnation history is retained.

### Isolate interleaved runs
- **Given**: lifecycle frames for 64 workflow ids are arbitrarily interleaved and reuse identical node paths across runs.
- **When**: the model applies them.
- **Then**: every change affects only the matching workflow id; no run or node data crosses identifiers.

## Success criteria

- **Lifecycle coverage**: 9/9 documented lifecycle methods have typed payloads and deterministic converter fixtures, measured by core fixture tests.
- **Live-shape coverage**: all workflow lifecycle frames in the committed 2.16.0 and 2.16.2 captures parse without loss of documented fields, measured by offline capture replay. `node_paused` remains explicitly source-verified rather than live-observed.
- **Terminal evidence**: one fresh Kiro 2.16.2 workflow run emits `run_complete.status = "failed"` and one emits `run_complete.status = "aborted"`, measured in committed credential-redacted JSONL captures before design approval. Source-derived fixtures cover node `failed`, `aborted`, and `skipped` statuses.
- **Retry evidence**: the committed 2.16.2 aborted capture's post-terminal same-id `run_start` begins a fresh incarnation and its second lifecycle reaches terminal state without stale nodes or fields from the first incarnation, measured by offline capture replay.
- **Replay determinism**: 2/2 consecutive replays of the same capture produce byte-equivalent canonical state; the second replay introduces 0 duplicate runs and 0 duplicate nodes, measured by probe/oracle comparison.
- **Scale isolation**: 64 runs × 256 nodes × 10 node events = 163,840 interleaved node events produce exact oracle-equal state with 0 dropped valid events and 0 cross-run mutations, measured by a deterministic stress fixture.
- **Regression fence**: 100% of existing non-workflow KAS conversion tests retain their prior outputs, measured by the cyril-core test suite with and without `kas`.

## Edge cases and decisions

| Edge | Decision | Rationale |
|---|---|---|
| No runs or nodes | Empty canonical collections are valid; no sentinels are created. | Absence is not an error until an event references absent identity. |
| `node_paused` lacks a live capture | Model all nine events from the source-verified 2.16.2 shape and an offline fixture; label the evidence static-only. | The requester accepted source verification for this elective mechanism. |
| Run `failed` and `aborted` lack live captures | Require one live 2.16.2 run for each status before design approval. | The requester made representative live terminal runs a ship gate. |
| Node `failed`, `aborted`, `skipped` lack live captures | Use source-derived deterministic fixtures; no separate live node-status gate. | Representative run-level failure and abort are sufficient. |
| Optional field absent | Preserve existing `Some` data on partial merge; otherwise keep `None`. | Missing is not a sentinel update. |
| Required field malformed | Warn and drop only the frame; state remains unchanged. | External data cannot create illegal domain state or kill the ACP connection. |
| Workflow or node identifier string | Reject empty; preserve every non-empty value byte-for-byte, including Unicode, spaces, and `#`. | Ids are opaque equality keys; format restrictions not present in Kiro's contract would reject future valid ids. |
| Workspace path string | Require the documented field where its enclosing shape requires it, but preserve its value opaquely, including empty, relative, absolute, Unicode, and spaced forms. | This issue never resolves or accesses the agent-reported path, so host path semantics and canonicalization would be false validation. |
| Unknown workflow method | Preserve existing unknown-extension handling. | Forward compatibility remains outside this nine-method model. |
| Event precedes `run_start` | Warn and ignore; do not create a partial run. | Kiro's buffered subscription contract guarantees the opening first. |
| Node update precedes declaration/start | Warn and ignore; do not create a placeholder node. | Canonical nodes must have a valid descriptor or snapshot origin. |
| Exact duplicate | Return unchanged and create no duplicate. | Reattach/replay must be idempotent. |
| Conflicting repeated supplied field | Latest present value wins until the run is terminal. | Kiro deliberately re-emits partial `node_start` data. |
| Same node path in different runs | Scope identity by workflow id. | `nodePath` is stable only within a run. |
| `run_complete.status = paused` | Reconcile the snapshot but remain nonterminal. | Live evidence shows paused completion is resumable. |
| Event after terminal run | Exact terminal duplicate is unchanged; a same-id `run_start` atomically starts a fresh active incarnation; any other event warns and is ignored. | Live 2.16.2 retry reuses the persisted workflow id and emits a new opening. |
| Empty `pendingSteps` with resolution | Preserve pending steps and record resolution separately. | The empty array is an acknowledgement form, not “drained.” |
| Run pause without node pause | Update only run pause state. | `paused` and `node_paused` are independently emitted. |
| Node pause without run pause | Update only the node. | No state is synthesized from an absent event. |
| Completed node without completion signal | Keep it completed; both signal fields may be absent. | This is normal on Kiro 2.16.2. |
| Full snapshot with `loop#N` wrappers | Canonicalize to streamed `iter-N` paths atomically. | Downstream consumers receive one identity scheme. |
| Unknown extra JSON field | Ignore the extra field while parsing all known fields. | Additive Kiro fields must not break known event handling. |
| Interleaved concurrent runs | Apply by keyed workflow/node lookup; never scan or mutate unrelated runs. | Parallel workflows are a primary W1 use case. |
| Retry or replay | A post-terminal `run_start` replaces current state as a new incarnation; within an incarnation, guarded merges and absorbing terminals make replay state-idempotent. | Reattachment can replay frames, while Kiro retry deliberately reopens the same persisted id. |
| Time zones and DST | Preserve wire timestamps; perform no local calendar arithmetic. | Lifecycle ordering comes from frame order, not wall-clock conversion. |
| Permissions, authentication, tenancy, deletion | No state behavior in this issue. | These are control-plane or workspace-lifetime concerns outside protocol folding. |

## Out of scope

This change does NOT include:

- `/workflow` commands, response-carrying bridge commands, recipe discovery, run invocation, list/load, attach, cancel, pause, resume, retry, update, or delete.
- Enabling `workflows.enabled`, registering Kiro's five agent-facing workflow tools, or proxying Kiro's four advertised workflow slash commands.
- Workflow peer-session routing, late claim, approval labels, `SubagentUiState`, panels, drill-in, rendering, or autocomplete.
- Cyril executing DAGs, cross-vendor orchestration, or model-launched workflow runs.
- Persisting runs, probing at startup, or defining reattach process lifetime.
- Retaining a full in-memory lifecycle event history.
- Modeling the superseded pre-2.16.0 `workflow-progress` metadata facade.
- Requiring live captures for `node_paused` or every node terminal variant.

## Constraints

| Dimension | Limit | How measured |
|---|---|---|
| Scope | 9 lifecycle methods; protocol conversion + pure cyril-core state only | Diff and crate dependency review |
| Compatibility | Kiro 2.16.0 and 2.16.2 committed wire evidence | Offline replay of both capture sets |
| Scale | 64 concurrent runs, 256 nodes/run, 10 node events/node | 163,840-event deterministic stress fixture |
| Identity | Exactly one canonical record per `(workflowId, nodePath)` | Oracle equality and duplicate-count assertions |
| Retention | At most current run/node/pending state; 0 raw event-history entries | State-shape assertion in stress fixture |
| Event cost | Average O(1) keyed run/node lookup; no full-state scan on ordinary node events | Implementation inspection plus scale fixture |
| Error isolation | 1 malformed frame changes 0 state records and does not stop the next valid frame | Malformed-then-valid converter test |
| Boundary | 0 UI, bridge-control, routing, or ACP session-lifecycle behavior changes | Diff review and existing regression suite |

## Decisions log

| # | Question | Decision | Why |
|---|---|---|---|
| 1 | Is source verification sufficient for unobserved `node_paused`? | Yes; model all nine with an offline fixture. | Preserve the nine-event contract without an indefinite elective-event probe. |
| 2 | Who owns `loop#N` → `iter-N` reconciliation? | The core canonical model. | Downstream code should consume one identity scheme. |
| 3 | Are source-derived terminal fixtures enough? | No; require live terminal evidence. | The requester wants representative runtime proof. |
| 4 | How much live terminal coverage? | One failed run and one aborted run. | Node terminal variants may remain source-derived. |
| 5 | What determines liveness across 2.16.0/2.16.2? | Node/run status; completion signals are optional metadata. | 2.16.2 completes neutral nodes with both signal fields absent. |
| 6 | Who is the direct audience? | Cyril W1 contributor. | This issue has no terminal-facing UI behavior. |
| 7 | How are repeated partial events merged? | Guarded, idempotent, latest present field wins. | Matches deliberate double `node_start` and replay. |
| 8 | What happens before `run_start`? | Warn and ignore unknown-run events. | Preserve legal state and rely on Kiro's opening-order guarantee. |
| 9 | What happens for an unknown node? | Warn and ignore the node-scoped event. | Do not invent incomplete placeholder nodes. |
| 10 | What happens for a malformed known payload? | Warn and drop only that frame. | Isolate external-data failure without poisoning the connection. |
| 11 | Does an acknowledgement clear pending steps? | No; preserve the queue and record resolution. | Empty `pendingSteps` is an overloaded acknowledgement form. |
| 12 | Do run and node pause imply each other? | No; keep them independent. | Live evidence proves they are not paired. |
| 13 | Can a terminal run transition again? | Only a later `run_start` may atomically replace it as a new incarnation under the same workflow id; every other non-duplicate event is ignored. | The requester chose to represent Kiro 2.16.2 retry, which reuses the id and emits a fresh opening. |
| 14 | What scale is guaranteed? | 64 runs × 256 nodes × 10 events/node. | Fences parallel isolation and repeated merges. |
| 15 | Is full history retained? | No; retain canonical current state only. | Avoid memory growth and duplicate reattach history. |
| 16 | Is full-snapshot ingestion part of this issue? | Yes; one canonical ingestion path serves `new`, `list/load`, and `finalState`. | Prevent a second state path in cyril-0qe6. |
| 17 | Which identifier strings are valid? | Any non-empty workflow/node id is valid and opaque; empty is malformed. | Canonical maps need a real key, but Kiro declares no prefix/character grammar. |
| 18 | What path semantics apply to `workspacePath`? | None; preserve the provided string opaquely, whether empty, relative, or absolute. | No filesystem operation occurs in this protocol/state issue. |

## Sign-off

> Cyril will model all nine KAS workflow lifecycle events as canonical current state, use one snapshot-reconciliation path, treat status as authoritative, merge repeats idempotently, isolate malformed or unattributable frames, and keep each run incarnation absorbing after terminal completion. A later same-id `run_start` is the sole reset: it atomically replaces prior runtime state as a new active incarnation so Kiro retry remains observable. Workflow/node ids reject only empty strings; workspace paths remain opaque strings with no filesystem semantics. Source-derived fixtures are sufficient for `node_paused` and node terminal variants; fresh Kiro 2.16.2 captures of one failed run and one aborted run are mandatory before design. Commands, routing, UI, gating, persistence, and full event history remain out of scope. The deterministic scale fence is 64 runs × 256 nodes × 10 events.

The requester agreed: "Yes, I agree"

Date: 2026-08-09
