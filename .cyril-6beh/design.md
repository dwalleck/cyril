# Design: KAS workflow lifecycle model

## Purpose

Convert the nine shipped KAS `_kiro/workflow/*` lifecycle notifications into a typed, wire-neutral core model and maintain one canonical current-state view per workflow run. The signed behavior is in `spec.md`. Live Kiro 2.16.2 failed/aborted evidence and its independent oracle agree in `findings.md`.

## Evidence baseline

- The wire methods are plain JSON-RPC notifications whose inbound method names reach Cyril without the leading underscore: `kiro/workflow/<kind>`.
- Eight payload kinds are live-observed in the 2.16.0 captures; `node_paused` is source-verified.
- Fresh 2.16.2 captures add live `failed` and `aborted` run/node states under `workflowsEnabled: false`.
- `completionSignal` and `completionSignalSource` are optional metadata. A completed 2.16.2 node normally has neither.
- The repeat capture contains final-state wrapper ids `loop#0`/`loop#1` and streamed paths `iter-0`/`iter-1` for the same runtime nodes.
- The aborted 2.16.2 capture contains `run_start → run_complete(aborted) → run_start → run_complete(aborted)` for one workflow id after `_kiro/workflow/retry`; the re-signed spec treats the second opening as a fresh incarnation.

## Input shapes

### Method sum type

Exactly nine known methods: `run_start`, `node_start`, `node_complete`, `node_paused`, `loop_iteration`, `watch_poll`, `paused`, `run_complete`, and `steps_queued`. The input space also includes an exact-prefix near miss such as `kiro/workflow/run_started`, an unrelated KAS method, and the superseded metadata facade; none is a lifecycle event.

### Status and outcome sum types

- Snapshot run status: `running`, `paused`, `completed`, `failed`, `aborted`; only the last three are terminal.
- Completion-event status: `paused`, `completed`, `failed`, `aborted`; `running` is rejected contextually on `run_complete`.
- Node status, including every status accepted on `node_complete` because the wire contract declares no narrower event subset: `pending`, `running`, `paused`, `completed`, `failed`, `aborted`, `skipped`.
- Node type: `step`, `sequence`, `repeat`, `parallel`, `watch`.
- Watch outcome: `new-activity`, `idle`, `idle-timeout`, `terminal-state`.
- Queue resolution: `applied`, `rejected`, `dropped`.
- Completion signal: `success`, `need_input`, `error`.
- Completion source: `send_message`, `status_update`.
- Repeat exhaustion action (`onMaxIterations`): `pause`, `abort`.
- Every enum also has an invalid/unknown-string input shape, which is rejected at conversion rather than represented as a sentinel.

### Node descriptors

- `step`: required node id and agent name; model and effort both absent/present independently.
- `sequence`: empty, one-child, multi-child, duplicate-child-path, and recursively nested shapes.
- `repeat`: empty/one/multi child; zero, one, observed large, exact `u32::MAX`, and `u32::MAX + 1` iteration limits; required `onMaxIterations` (`pause`/`abort` plus rejected unknown); and opaque `stopCondition`/`stopWhen` values absent/present independently.
- `parallel`: empty, one-branch, multi-branch, and duplicate-branch-path shapes.
- `watch`: required node id and handler name.

### Optional-field matrix

Every independently optional field is exercised as absent and present: parent session, step session, prompt, iteration, branch id, artifacts, captured output, failure reason, queue resolution/reason, completion signal/source, start/end timestamps, parent model, workspace path, model id, effort level, stop condition, and stop predicate. Required `onMaxIterations` is separately exercised with both valid variants, missing/wrong type, and unknown string. Every present known optional field also receives wrong-type and explicit-null rows; null remains valid only inside fields whose domain is opaque `serde_json::Value`. Partial `node_start` re-emission covers mixed presence. Generated merge fixtures vary the reachable presence bits independently instead of testing only “all absent” and “all present.”

### Collections and scalar boundaries

- Opaque recipe-input, artifact, repeat-stop-condition, and repeat-stop-predicate values: every `serde_json::Value` member; exact integer boundaries `i64::MIN` and `u64::MAX`; a finite fractional number; empty/ASCII/Unicode/spaced/large strings; arrays empty/single/multi with distinct or duplicate elements; objects empty/single/multi with empty/ASCII/Unicode/spaced/large keys; and nested array/object combinations. Value equality and every resolved object key are preserved; none is schema-filtered.
- Duplicate raw JSON object keys: the last value retained by `serde_json` before the converter receives a `Value` is the documented input; that resolved key/value is then preserved, including inside opaque JSON.
- Pending steps and child nodes: empty, single, multi, duplicate canonical path.
- Node path: empty, root-only, normal multi-segment, repeated-iteration segment, empty segment, Unicode, spaces, `#`, and a root that does not equal the workflow id.
- Iteration, maximum iterations, and plan revision use the established core `u32` convention: zero, one, observed maximum, and exact `u32::MAX` are accepted; negative JSON numbers and `u32::MAX + 1` are rejected with a structured warning.
- Workflow and node ids: standalone `run_start.workflowId` and descriptor `nodeId` rows each cover empty (rejected), ASCII, Unicode, spaces, `#`, `/`, `\`, and a large valid string; every non-empty value is accepted and byte-preserved.
- Every non-workflow/node-id scalar string—including workflow/agent/handler names, model/effort, prompt/reasons, timestamps, and session ids—requires string type and required-field presence where documented, but accepts and byte-preserves empty, ASCII, Unicode, spaces, `/`, `\`, and large values.
- Workspace paths: missing where required is rejected; present empty, relative, POSIX-absolute, Windows-absolute, Unicode, and spaced strings are accepted and preserved opaquely without `Path` conversion.
- Full snapshots: fresh seed; equivalent/newer reconciliation for an active run; same-status reconciliation for a terminal run; terminal-to-nonterminal or conflicting-terminal status; paused/terminal final state; malformed deep child; duplicate canonical node path; and every unequal `run_complete.status`/`finalState.status` pair.
- Unknown extra fields in typed wire objects: event payload top level, nested node descriptor, and nested node state; all known siblings still parse and the extra field is ignored. Keys inside opaque recipe-input/artifact JSON are data and are preserved instead.

### Ordering and replay

- Opening first; event before opening; node update before node introduction.
- Exact duplicate; partial repeat; conflicting repeat while active.
- Paused completion followed by resumed events.
- Terminal completion followed by exact terminal duplicate, non-opening conflicting event, same-id `run_start` reset, same-status snapshot reconciliation, or conflicting-status snapshot rejection.
- Interleaving across distinct workflow ids with identical node paths.

## Removed-invariant sweep

The core move is additive except for one narrow removed invariant: the nine exact workflow methods stop being “unknown extension notifications that are dropped.” That old drop behavior guaranteed no workflow frame reached the App and no workflow state grew. The design replaces only that exact set. Adjacent `kiro/workflow/*` names remain inert; session-scoped notification classification remains unchanged; `SessionController`, `UiState`, `TuiState`, and ACP `SessionUpdate` deserialization remain unaware of workflows. Claims 1 and 12 fence those surviving invariants.

## Architecture and placement

### Domain types — `cyril-core::types::workflow`

Owns `WorkflowId`, `WorkflowNodeId`, `WorkflowNodePath`, snapshot-run/status/outcome/repeat-exhaustion enums, the narrower `WorkflowCompletionStatus`, node descriptors, snapshots, lifecycle events, and immutable read models. Identifier newtypes reject only empty strings and preserve every non-empty value opaquely; absence uses `Option`; fixed vocabularies use enums. Workspace paths and every other non-ID string remain opaque because this issue performs no filesystem or content interpretation. Arbitrary recipe inputs, artifacts, repeat stop conditions, and repeat stop predicates retain the full `serde_json::Value` sum type. Iteration, maximum-iteration, and plan-revision counters use `u32`, matching the existing core `LoopState` convention. Fields remain private with accessors, matching existing core domain types. `CONTEXT.md` distinguishes the persisted workflow run from its current incarnation.

This module may not import ACP, ratatui, crossterm, App, bridge senders, or async primitives.

### KAS adapter — `protocol::convert::kas::workflow`

Owns private wire structs and exact-method dispatch from borrowed `serde_json::Value` into `WorkflowEvent`. `protocol::convert::kiro::to_ext_notification` delegates the exact `kiro/workflow/<kind>` set to this adapter and wraps the result as `Notification::Workflow(Box<WorkflowEvent>)`. Boxing prevents the recursive snapshot/event shape from inflating every unrelated `Notification` value; the allocation is paid only for workflow frames.

The adapter validates required non-empty workflow/node ids, required-field presence/type, enum values, node-path roots/segments, status/final-state agreement, and recursive descriptor/state shape at the point of use. Workspace paths and non-ID strings are not interpreted. Unknown fields are ignored only in typed wire objects; opaque recipe-input/artifact/stop-condition/stop-predicate `serde_json::Value` keys and values are preserved. A known malformed frame logs method plus parse context and returns `Ok(None)`; it never defaults or partially constructs domain state. The adapter does not clone the whole JSON value before parsing.

ACP types remain confined to `protocol::convert`; domain and state modules never import `agent_client_protocol`.

### Deep state module — `cyril-core::workflow`

Owns `WorkflowTracker`, `WorkflowRun`, `WorkflowNodeState`, and `WorkflowStateError`. Its small interface is:

```rust
pub fn apply_event(&mut self, event: WorkflowEvent) -> Result<bool, WorkflowStateError>;
pub fn apply_snapshot(&mut self, snapshot: WorkflowSnapshot) -> Result<bool, WorkflowStateError>;
pub fn get(&self, id: &WorkflowId) -> Option<&WorkflowRun>;
pub fn iter(&self) -> impl ExactSizeIterator<Item = (&WorkflowId, &WorkflowRun)>;
```

`apply_event` and `apply_snapshot` consume owned values, allowing snapshot trees to be flattened without cloning them. Both report canonicalization/transition failure through `WorkflowStateError`; ordinary ignored/duplicate transitions return `Ok(false)`. `get` exposes exact known/unknown lookup and `iter` exposes empty/multi-run exact-size traversal; dedicated interface tests fence both public methods. The tracker does not accept the broad `Notification` enum: callers need learn only the workflow interface, and unrelated notification variants cannot leak into state logic.

`WorkflowTracker` stores `HashMap<WorkflowId, WorkflowRun>`. Each run stores plan descriptors separately from runtime nodes, a `HashMap<WorkflowNodePath, WorkflowNodeState>`, a node-id index for events such as `loop_iteration` that omit `nodePath`, current pending steps/latest resolution, and current run metadata. It stores no raw event vector. Ordinary path-bearing node events perform average O(1) run and node lookup. Snapshot traversal is O(number of nodes), proportional to the input it must ingest. Private `#[cfg(test)]` counters mechanically fence ordinary-event lookup bounds without adding a public storage seam or production instrumentation.

### App ownership — `cyril::App`

`App` owns one private `WorkflowTracker`. `handle_notification` consumes `Notification::Workflow` exactly once before passing other notifications by reference to `SessionController` and `UiState`, applies the boxed event by value, and returns without forwarding it into either non-workflow state machine. A `WorkflowStateError` produces a structured warning with event kind and available workflow identity; state remains unchanged. This avoids a deep snapshot clone and keeps the pure model wired to real runtime traffic without adding rendering behavior. Private `#[cfg(test)]` App dispatch counters increment at the actual tracker/session/UI invocation sites so the App seam test distinguishes a no-op consumer call from no call.

No public App accessor is added in this issue. cyril-jxfu is the verified consumer that will add workflow-session routing against this state; cyril-0qe6 is the verified consumer for control and snapshot replies.

### Interface alternatives rejected

1. `apply_notification(&Notification)` was rejected because it couples the module to every unrelated domain event and forces deep clones from borrowed snapshots.
2. Storing workflow state inside `SessionController` was rejected because a workflow run outlives and is not owned by one ACP session.
3. Storing workflow state inside `UiState` was rejected because the model is control/routing state and would make rendering state the owner of protocol truth.
4. Pure `fold(old_state, event) -> new_state` was rejected because it copies the full run map per frame without providing a second adapter at a real seam.

## State rules

### Atomic snapshot path

A snapshot is first validated and flattened into a temporary canonical run. Nothing mutates on failure. The synthetic root uses `[workflowId]`. A direct child of a repeat is an iteration wrapper only when its type is `sequence`, its id is exactly `<repeat-id>#<digits>`, and its present `iteration` equals those digits. That wrapper's raw identity becomes one canonical runtime entry at `iter-N`; its `nodeId`, iteration number, status, and other supplied runtime metadata remain data on that entry. Missing/mismatched iteration, another node type, and every other `#` id remain literal. Duplicate canonical paths are errors.

`new`, `list/load`, and `run_complete.finalState` use one canonical snapshot validator/flattener behind distinct transition gates. Direct `apply_snapshot` seeds a missing run and lets an active run accept any valid snapshot status, including transition to terminal; it reconciles an already-terminal run only when the incoming status equals the current terminal status. A nonterminal or different-terminal direct snapshot for a terminal id returns `WorkflowStateError::TerminalSnapshotConflict` and changes nothing. A `run_complete` never seeds an unknown run. For an active run it reconciles a valid completion status; for a terminal run an exact duplicate is unchanged and every non-exact completion event warns and is ignored, even when its final snapshot has the same terminal status. Thus `run_start` remains the sole incarnation reset. Accepted snapshot-owned runtime fields replace prior values, including clearing optional completion metadata when omitted. Stream-only data absent from snapshots, such as raw `node_start.prompt`, remains attached to the canonical node.

Snapshot ownership is schema-based. Snapshot-owned run fields are workflow name/status/inputs/artifacts/captured outputs/created time/plan revision/parent session/workspace path and the descriptor tree. Snapshot-owned node fields are descriptor data plus status/session/artifacts/captured output/failure reason/iteration/branch/completion signal/source/start/end times. When an accepted snapshot omits one of those optional fields, the canonical value clears. Event-only fields absent from `WorkflowState`—node-start prompt, queued-step/acknowledgement data, pause reasons, and latest loop/watch event details—survive reconciliation. A fresh `run_start` incarnation clears both ownership classes before installing its opening data.

### Event application

- `run_start` creates a missing run plus plan descriptors. For an active run, exactness compares workflow name, inputs, complete descriptor tree, and optional parent session: an exact repeat is unchanged and any independently conflicting supplied field warns and is ignored. For a terminal run, any `run_start` atomically replaces the whole canonical run as a fresh active incarnation under the same id.
- `node_start` creates or guarded-merges the path entry. Missing fields preserve prior values; present fields replace them. A changed `nodeId` removes that path from the old node-id index bucket, removes the bucket if empty, and inserts the path under the new id.
- `node_complete` accepts every documented `NodeStatus`, because the source wire contract declares no narrower event-specific subset, and guarded-merges only a known node's status and supplied completion fields. Absent artifacts/captured output/failure reason/completion signal/source preserve prior values; present values replace them; unrelated node-start/session/progress fields never clear.
- `node_paused` updates only a known node; `paused` updates only the run.
- `steps_queued` without resolution replaces current pending descriptors. Any resolution-bearing frame is acknowledgement-only: it records the supplied outcome/reason and preserves current pending descriptors regardless of whether its supplied array is empty or non-empty.
- `loop_iteration` resolves only repeat-typed paths from the node-id index and replaces the unique repeat's latest iteration state; both `stopConditionMet: false` and `true` are retained exactly. A same-id non-repeat path does not create ambiguity; zero or multiple matching repeat paths warns and leaves state unchanged.
- `watch_poll` replaces the latest poll state on its known watch node.
- `run_complete(paused)` reconciles the snapshot and leaves an active run nonterminal.
- `run_complete(completed|failed|aborted)` reconciles and makes an active incarnation absorbing. `run_complete(running)` is rejected during conversion. For an already-terminal incarnation, an exact completion-event repeat is unchanged; every non-exact completion event warns and is ignored. A later `run_start` begins a fresh incarnation; every other later event warns and is ignored.
- Completion signal/source are retained metadata only. Node/run status is authoritative.

## Claims

1. The conversion boundary is exact: only the nine known method names produce workflow notifications, while near misses, unrelated KAS methods, and every existing non-workflow path preserve their prior outputs.
2. For every method, each required-field omission and each known-field wrong type, invalid enum, or disallowed explicit null logs a contextual warning, changes zero state, and does not prevent the next valid frame from converting.
3. Every documented node/snapshot-run/completion-status/outcome/repeat-exhaustion variant, opaque arbitrary-JSON shape and exact scalar/key boundary, identifier/scalar-string form, optional-field presence mask, collection/duplicate-key form, numeric/node-path boundary, opaque workspace-path form, typed-object unknown-extra boundary, duplicate canonical path, and outer/inner completion-status mismatch has a case-labelled lossless-or-rejected result; every rejected case includes stable structured warning context and unchanged-state evidence.
4. Direct snapshot ingestion and `run_complete.finalState` share one atomic validator/canonicalizer behind an explicit entrypoint × prior-status × incoming-status gate: direct snapshots may seed and reconcile persisted state; completion events never seed, apply only to active runs, and are duplicate-only after terminal.
5. Repeat wrappers translate only for a direct sequence child matching both `<repeat-id>#N` and `iteration == N`; they yield `iter-N` while preserving wrapper metadata, and every near miss remains literal.
6. Repeated partial node starts and completions merge idempotently: absent fields preserve, present fields replace, a changed node id moves only its path between possibly shared/occupied index buckets, and duplicate nodes never appear.
7. Every non-opening event for an unknown run, every node-addressed update for an unknown node, and every ambiguous repeat lookup changes zero records; repeat lookup filters out same-id non-repeat nodes.
8. A resolution-bearing queue acknowledgement records every outcome/reason form and never changes pending descriptors from its supplied array.
9. Run pause, node pause, both false/true loop stop-condition outcomes, and watch polling update independent current-state fields without retaining raw history.
10. Paused completion is nonterminal; completed/failed/aborted completion is absorbing within an incarnation; every contradictory completion signal/source combination remains metadata only; a later same-id `run_start` atomically clears every field/index from a fully populated prior incarnation and begins a fresh one.
11. The 163,840-event stress shape remains run-isolated, duplicate-free, current-state-only, and performs no full-map scan on ordinary node events under a fixed test-only lookup bound.
12. Workflow state is owned by `cyril-core::workflow` and App; no ACP or UI dependency crosses that seam.
13. Replaying a complete capture twice—including the live same-id retry reopening—produces the same deterministically ordered canonical projection and the same run/node cardinalities as one replay, with no stale prior-incarnation fields.
14. App consumes each workflow notification exactly once into `WorkflowTracker` and forwards it zero times to `SessionController` or `UiState`.
15. An exact repeated active `run_start` is unchanged; independently changing workflow name, inputs, descriptor tree, or optional parent session logs a structured warning and changes neither state nor cardinality.

## Falsification

| # | Claim | Falsifier | Independent oracle | Cost | Status | Specific buggy implementation killed | Regression fence |
|---|---|---|---|---|---|---|---|
| 1 | Exact conversion boundary | Feed the nine valid methods plus `run_started`, an unrelated KAS method, and the existing non-workflow fixture suite; emit a labelled result per input. Any missing workflow event, accepted near miss, or changed non-workflow result falsifies. | Literal 2.16.2 method inventory from capture/source audit plus committed pre-change expectations. | 11 offline frames + existing suites | pending | Prefix dispatch accepts `run_started`, underscore stripping happens twice, or adjacent converter output changes. | Core test `workflow_method_dispatch_is_exact` plus existing cyril-core/cyril-ui/cyril suites with and without `kas` |
| 2 | Exhaustive malformed isolation | For each method, generate a labelled row for omission of every required field and, for every known field, wrong JSON type, invalid enum where applicable, and explicit null where null is not part of the field domain; append a valid frame after each row. Any emitted malformed event, mutation, panic, missing warning fields, collapsed invalid optional value, or lost valid successor falsifies. | Standalone schema-driven fixture mutator plus before/after state digest and captured tracing level/fields. | Offline exhaustive field matrix | pending | `unwrap_or_default` fabricates one untested field, `Option` collapses invalid/null to absence, `?` aborts the receive loop, or a warning silently disappears. | Core test `malformed_workflow_field_matrix_isolated` using the existing tracing-subscriber capture pattern |
| 3 | Lossless shape coverage | Generate case-labelled rows for every node/snapshot-run/completion-status/outcome/repeat-exhaustion variant; opaque JSON sum member, exact integer/fraction/string/key boundary, and nested/duplicate-array form across recipe inputs, artifacts, and repeat stop fields; standalone id and scalar string; optional-field presence mask; empty/one/many collection; raw duplicate key (resolved last value preserved); numeric/node-path boundary; workspace path; typed-object unknown extra; duplicate canonical path; and unequal outer/inner completion statuses. Replay live/static fixtures as controls. Any missing family/case id, lost opaque key/value, mismatched normalized/drop result, rejection mutation, or missing stable warning falsifies. | Standalone expected manifest/folder with an exact required-family set; raw JSON parser oracle for duplicate keys; plus `jq` capture projection. | Offline generated matrix + capture replay | pending | A measurement family disappears, opaque JSON is schema-filtered, numbers/strings/keys are lossily converted, ids are overvalidated, a path is interpreted, an enum becomes a sentinel, additive typed fields break parsing, duplicate paths overwrite, or invalid data disappears silently. | Named core fences: `workflow_node_descriptor_shape_matrix`, `workflow_enum_domain_matrix`, `workflow_arbitrary_json_shape_matrix`, `workflow_scalar_string_matrix`, `workflow_identifier_string_matrix`, `workflow_optional_field_presence_matrix`, `workflow_collection_shape_matrix`, `workflow_duplicate_raw_json_key_last_wins`, `workflow_numeric_and_path_boundaries`, `workflow_workspace_path_is_opaque`, `workflow_typed_unknown_extra_is_ignored`, `workflow_duplicate_canonical_path_rejected`, and `workflow_run_complete_status_mismatch_rejected` |
| 4 | Snapshot entrypoint/status/ownership matrix | Cross direct snapshot versus completion-event entrypoints with missing/active/paused/each-terminal prior state and every valid incoming status. Assert: direct may seed; completion on missing warns/ignores; active/paused accepts valid completion status; direct same-terminal reconciles; terminal exact completion duplicate is unchanged; terminal non-exact completion warns/ignores; direct terminal-to-nonterminal/different-terminal returns `TerminalSnapshotConflict`. Inject a malformed deep child into every accepting entrypoint. Separately seed every snapshot-owned optional plus every event-only field, reconcile a snapshot omitting them, and require snapshot-owned values to clear while event-only values remain. Any transition-table, ownership, output, warning/error, or atomicity mismatch falsifies. | Independent Python canonical snapshot flattener, literal entrypoint × prior × incoming table, explicit schema ownership manifest, exact-event equality oracle, warning/error manifest, and hashes. | Offline fold/ownership matrix | pending | Shared parser accidentally shares incompatible gates, `run_complete` seeds unknown state, post-terminal completion mutates, direct reconciliation is over-rejected, insertion occurs before validation, optional snapshot state stays stale, or stream-only state is lost. | Core tests `snapshot_entrypoint_status_matrix`, `snapshot_entry_paths_are_equivalent`, `snapshot_field_ownership_matrix`, `active_snapshot_can_become_terminal`, `invalid_snapshot_is_atomic`, and `terminal_snapshot_conflict_is_atomic` |
| 5 | Exact repeat identity translation | Compare capture snapshot/event paths and wrapper metadata, then apply exact-pattern controls with missing/mismatched iteration, wrong type, non-digit/leading-zero suffix, Unicode, and non-wrapper `#` ids. Any path/metadata difference or rewritten near miss falsifies. | Raw Kiro paths/metadata plus literal discriminator truth table independent of Rust traversal. | One capture + 8 controls | **passed** — `C05 passed: 3 event paths match wire; 2 iteration entries preserve metadata; 8 discriminator controls classify exactly` | Id-only rewrite steals a literal node, or valid wrapper metadata is lost. | Core test `repeat_snapshot_paths_match_wire_paths` with exact/near-miss controls |
| 6 | Guarded merge and index matrices | Fold observed double `node_start`, every start-optional mask, and every `node_complete` completion-field absent/present mask while pre-seeding prompt/session/progress data. Then run changed-`nodeId` cases where the old bucket has two paths and the new bucket is already occupied. Compare node count, all fields, and private index membership/cardinality: only the changed path moves; old peers and new peers remain. Any cleared unrelated field, duplicate, or bucket corruption falsifies. | Literal transition/index tables generated outside Rust with per-field and per-bucket expected values. | Offline merge/index matrices | pending | Whole-struct replacement clears `sessionId`, node completion clears prompt, append duplicates nodes, removing one path deletes its old peers, or insertion overwrites new peers. | Core tests `node_start_merge_presence_matrix`, `node_complete_merge_presence_matrix`, and `node_index_bucket_cardinality_matrix` |
| 7 | Exhaustive unknown identity and typed repeat resolution | Generate unknown-run rows for all eight non-`run_start` lifecycle variants and, on a known run, unknown-node rows for `node_complete`, `node_paused`, `watch_poll`, and `loop_iteration`. For loop lookup, compare same-id step+repeat (unique repeat must update) with two same-id repeats (ambiguous, warning/unchanged). Any digest/cardinality change, wrong lookup, missing warning, or placeholder falsifies. Exercise `get` on empty/known/unknown ids as the public lookup fence. | Exact variant manifest, pre/post canonical hashes, literal typed-index truth table, direct lookup truth table, and tracing projection. | 12 identity rows + typed-index controls + lookup cases | pending | One event arm creates placeholders, lookup counts a same-id step as a repeat, arbitrary ambiguous match wins, public lookup breaks, or rejection is silent. | Core tests `unknown_workflow_event_matrix_no_placeholders`, `unknown_node_update_matrix_no_placeholders`, `loop_lookup_filters_type_and_rejects_ambiguity`, and `workflow_tracker_get_known_and_unknown` |
| 8 | Queue overload matrix | Seed populated pending steps; apply no-resolution frames with empty/non-empty arrays, then independently apply resolution-bearing empty/non-empty arrays for each `applied`/`rejected`/`dropped` outcome and absent/present reason. No-resolution frames must replace pending descriptors; every resolution frame must preserve them while recording exact outcome/reason. Any wrong pending list or acknowledgement data falsifies. | Literal resolution-presence × collection-cardinality × outcome × reason-presence truth table from the two source-verified wire forms. | Offline queue matrix | pending | Resolution+nonempty unexpectedly replaces pending work, empty acknowledgement clears it, an outcome becomes a sentinel, or reason presence is lost. | Core test `queue_resolution_and_pending_matrix` |
| 9 | Independent current progress | Permute run/node pause, loop iterations with `stopConditionMet` false then true, and two watch polls. Cross-field changes, lost boolean value, history length >1, or order-dependent unrelated state falsifies. | Standalone finite transition table with four named outputs and explicit false/true loop rows. | Offline permutation set | pending | Run pause cascades to nodes, stop-condition false becomes absent/true, or vectors append every poll. | Core test `workflow_progress_fields_are_independent_and_current` |
| 10 | Status-authoritative, absorbing, and retry-reset semantics | First cross every completion-event run status and documented node status with absent and contradictory `success`/`need_input`/`error` signals and both sources; status/liveness must equal the status-only oracle. Next seed a fully populated terminal incarnation containing nodes, shared index buckets, queue/ack, run/node pause, loop/watch progress, completion metadata, and every optional runtime field; a same-id `run_start` must remove all old state and install only its opening. Finally apply each of the eight non-opening lifecycle variants after terminal; every row warns/does nothing except the exact completion duplicate, which is unchanged without warning. | Cartesian status-only liveness table; independent exhaustive prior-field/reset manifest; exact post-terminal variant manifest; raw 2.16.2 terminal/retry frames as controls; tracing projection. | Offline metadata/reset/absorbing matrices | pending | Signal derives status only when present, retry clears nodes but leaks queue/index/progress, one post-terminal event arm mutates, exact duplicate warns, or retry opening is ignored. | Core tests `workflow_completion_metadata_never_controls_status`, `retry_opening_clears_full_prior_incarnation`, `post_terminal_event_matrix_is_absorbing`, and `workflow_terminal_capture_controls` |
| 11 | Scale/isolation budget | Generate 64 × 256 × 10 interleaved node events with repeated paths; compare canonical digest, record counts, and private test lookup counters. Any cross-run value, duplicate, raw-history entry, or ordinary path event exceeding one run lookup plus one node lookup (and one id-index lookup only for pathless events) falsifies. | Independent deterministic generator/folder and literal per-event lookup bound. | 163,840 offline events | pending | Linear search through all runs/nodes or global path key without workflow id. | Core stress test `workflow_tracker_scale_and_isolation` |
| 12 | Structural ownership | Compile core without UI imports; inspect the workflow module's imports/visibility and App ownership. Any ACP/UI import in domain/state or state ownership in SessionController/UiState falsifies. | Cargo dependency graph plus AST import query. | One compile + structural query | pending | Parser/state placed in App or UiState for convenience. | Crate dependency rule plus module-private wire types |
| 13 | Full-capture replay and iteration interface | Replay each capture once/twice and project state exclusively through `iter`; compare sorted output/cardinality. Assert empty/multi exact-size iteration, both retry openings applied, and no stale first-incarnation data. Any replay diff, wrong iterator size/content, duplicate, ignored reset, or stale field falsifies. | Independent JSONL folder plus literal empty/multi iterator truth table. | Two passes per capture + interface cases | pending | Replay appends/merges incarnations or iterator omits/miscounts runs. | Core tests `workflow_capture_replay_is_state_idempotent` and `workflow_tracker_iter_empty_and_exact_size` |
| 14 | Exact-once and error-isolated App dispatch | First inject one boxed workflow opening through `App::handle_notification`; require call-site counters tracker=1, SessionController=0, UiState=0 and one mutation. Then inject a validly converted event whose duplicate canonical paths make state application fail, followed by a valid workflow event; require structured warning, unchanged tracker digest for the failure, tracker counters advancing once per frame, zero session/UI calls, and successful successor mutation. | Test-only App call-site counters, canonical state hashes, exact warning projection, and unique workflow ids. | Two App seam sequences | pending | Workflow is dropped/applied twice/forwarded; state error panics or disappears silently; partial state remains; or one failed event loses the next frame. | App tests `workflow_notification_is_consumed_exactly_once` and `workflow_state_error_isolated_by_app` |
| 15 | Active opening conflict matrix | Apply one opening, its exact duplicate, then independently vary workflow name, inputs, complete descriptor tree, and optional parent session (`None→Some`, `Some→None`, changed `Some`). Any duplicate change, conflict mutation, run-count change, or missing warning falsifies. | Literal per-field transition matrix plus state/cardinality digest and tracing projection. | Active opening matrix | pending | Equality ignores one opening field, overwrite occurs, or conflict is silent. | Core test `active_run_start_conflict_presence_matrix` |

Falsifiers are ordered by observed execution cost within their dependency groups; claim 5 was the cheapest independent design experiment and has already passed via `.cyril-6beh/falsify-node-paths.py`.

## Negative space

- No bridge control methods, slash commands, list/load transport, or reattachment lifetime; cyril-0qe6 owns that verified work.
- No workflow peer-session route or late-claim behavior; cyril-jxfu owns that verified work.
- No render trait, widget, panel, drill-in, autocomplete, or input-handler change; the signed issue is core protocol/state only.
- No `workflows.enabled` setting and no agent-facing workflow tools; ADR-0011 rejects that control plane.
- No client-side DAG executor or cross-vendor scheduler; ROADMAP W2 is a separate architectural track.
- No full event history and no filesystem use of agent-reported workspace paths.

## Approval decisions

The design recommends the consuming `WorkflowEvent` interface rather than the issue note's borrowed `apply_notification(&Notification)` sketch. This is the only intentional deviation from that sketch: it keeps the module deep and avoids cloning full snapshots while preserving every signed observable behavior.
