# Design: Fence KAS workflow pause ordering

## Purpose

KAS 0.38.7 moved run-level `_kiro/workflow/paused` from individual park sites to the run-loop settle immediately before non-terminal paused `run_complete`. Cyril's tracker already converges under both sequences, but its committed fixtures encode only the earlier intermediate ordering and do not state which event is immediate.

This change makes that compatibility contract executable. It preserves node/run authority separation: `node_paused` changes node state immediately; run-level `paused` remains a summary that changes run state when it arrives; paused `run_complete` reconciles the persisted snapshot without terminating the run.

## Evidence

The probe and independent oracle in `.cyril-vhfz/findings.md` agree on all 10 intermediate-state rows across the old and new orderings.

- Live 2.16.0 evidence: `paused` precedes queue frames and paused `run_complete`.
- Extracted KAS 0.38.7 evidence: immediate park-site `node_paused`; centralized run-level `paused` at `acp-server.js` lines 439940–439952; `run_complete` immediately after at 439954–439967.
- Current Cyril behavior: new ordering leaves run status/reason absent during intervening queue frames while the paused node and its reason are already present; final paused state matches the old ordering.
- Shipped-consumer audit after cyril-0qe6 merged in PR #95: its `/workflow` surface consumes command outcomes and snapshots, not pause events/status/reasons. Pause accessors and statuses still have no consumer outside `cyril-core`; there is no immediate-pause caller to migrate in this PR.

## Input shapes

### Event order

1. **Legacy run-summary-first:** run-level `paused`, zero or more intervening lifecycle frames, paused `run_complete`. The live repeat-exhaustion capture has no `node_paused`.
2. **Legacy node plus early summary:** `node_paused`, other progress, run-level `paused`, queue frames, paused `run_complete`. The cyril-6beh synthetic fixture has this shape.
3. **KAS 0.38.7 node plus late summary:** `node_paused`, zero or more intervening lifecycle frames, run-level `paused`, paused `run_complete`.
4. **Repeated delivery:** either sequence replayed twice; every second application is idempotent under the existing replay contract.

### State presence

1. Run status absent or `running`; zero paused nodes.
2. Run status absent or `running`; one paused node with a node reason.
3. Run status absent or `running`; multiple paused nodes in a nested/parallel snapshot.
4. Run status `paused`; node reason absent (repeat exhaustion).
5. Run status `paused`; node and run reasons both present and not assumed equal.
6. Paused `run_complete`; root-only and nested paused nodes.
7. Terminal `completed`, `failed`, or `aborted`; existing absorbing semantics remain unchanged.

### Run-frame attribution fields

1. Required `workflowId` and `pauseReason` on `paused`, no optional fields.
2. Optional `parentSessionId` present or absent.
3. KAS 0.38.7 `initiator` and `initiatorReason` both present on `paused` and paused `run_complete`.
4. Unknown additional metadata. Serde's current non-`deny_unknown_fields` adapter contract ignores it.
5. Empty, ASCII, and Unicode pause reasons; existing converter coverage remains authoritative.

Production KAS validates that `initiatorReason` accompanies `initiator: "user"`; invalid presence combinations are not production-reachable and this change does not add a second validator for them.

The ticket's “always carries attribution” wording is stronger than the source: `pausedAttributionFields` and `runCompleteAttributionFields` contribute fields only when control attribution was stamped onto state. Organic parks can still be bare. Both presence shapes remain supported.

## Removed-invariant sweep

This is a subtractive upstream change: KAS removed the guarantee that run-level pause status and reason were written at the park site.

| Former “cannot happen” fact | KAS 0.38.7 reality | Required surviving invariant |
|---|---|---|
| No lifecycle frame appears after a park while run status is non-paused. | Queue/progress frames can arrive after `node_paused` and before run-level `paused`. | The node is already `Paused` with its reason throughout that window. |
| `WorkflowRun::status() == Paused` is the immediate pause signal. | Run status remains absent/running until executor unwind. | Consumers needing immediacy react to `WorkflowEvent::NodePaused`, not run status. |
| `WorkflowRun::run_pause_reason()` is immediately available. | Only the node reason is immediate. | The node reason survives late summary and snapshot reconciliation. |
| Every pause path emits `node_paused`. | Repeat exhaustion still settles a paused node without a `node_paused` frame, then immediately emits the centralized summary. | Legacy summary-only input continues to produce a paused run. |
| A paused `run_complete` ends the run. | It remains resumable and non-terminal. | Existing `is_terminal` behavior stays unchanged. |

The change does not relax concurrency, ownership, uniqueness, or error-absorption rules. Those invariants are unaffected.

## Architecture and placement

### Owner

`cyril-core::workflow` owns pause-state transitions and their regression tests. This is the existing deep module: callers provide typed `WorkflowEvent`s; the module owns ordering, partial-update, snapshot-reconciliation, and terminality rules.

`cyril-core::protocol::convert::kas::workflow` remains the adapter from KAS JSON to typed events. It owns tolerance of the 0.38.7 summary extras but does not reinterpret timing.

The canonical source-derived fixture belongs in `crates/cyril-core/tests/fixtures/kas/workflow/`, beside the cyril-6beh capture/oracle fixtures. `.cyril-vhfz/derive-new-ordering.py` regenerates it from the existing lifecycle fixture.

### Interface and seam

No new seam and no new public method. The existing interface is sufficient:

- `WorkflowEvent::NodePaused` is the immediate notification.
- `WorkflowNodeState::status()` and `node_pause_reason()` expose the immediate state.
- `WorkflowRun::status()` and `run_pause_reason()` expose the later run summary/snapshot.

Adding a convenience `WorkflowRun::is_paused()` would hide a resume ambiguity: a previously paused node can remain paused in the read model until later reconciliation. It would also force a policy for multiple paused nodes and terminal snapshots. The event interface is the accurate timing seam.

### Forbidden implementation

- Do not synthesize run status or run reason inside `apply_node_paused`; that erases the observed summary latency and conflates node/run authorities.
- Do not delay node mutation until run-level `paused`; that recreates the 0.38.7 UI latency bug.
- Do not make `Paused` terminal or tear down run/session ownership on paused `run_complete`.
- Do not parse KAS JSON in `cyril-ui` or `cyril`; only the core adapter sees wire fields.
- Do not add renderer or slash-command behavior in this PR. cyril-zd8u owns the verified run-panel scope; the now-merged cyril-0qe6 v1 command surface has no event-driven pause prompt.

## Claims

1. Applying `node_paused` immediately sets only the addressed node's status and node reason, even when run status and run reason are absent.
2. Intervening lifecycle frames between `node_paused` and run-level `paused` preserve that immediate node state without inventing run state.
3. Legacy summary-first input still sets run status/reason before paused completion even when no `node_paused` exists.
4. Late run-level `paused` and paused `run_complete` frames carrying `initiator`/`initiatorReason` convert successfully without changing pause authority.
5. Paused `run_complete` under old and new orderings converges to the same non-terminal run projection and preserves event-only node/run reasons.
6. Replaying either ordering twice is idempotent, and terminal `completed`/`failed`/`aborted` behavior is unchanged.
7. Pause timing knowledge remains local to `cyril-core`; no UI, orchestrator, or command module gains wire parsing or a duplicate ordering predicate.

## Falsification

| # | Claim | Falsifier | Independent oracle | Cost | Status | Specific buggy implementation | Regression fence |
|---|---|---|---|---|---|---|---|
| 1 | Node pause is immediate and node-scoped. | Replay a known node, apply `node_paused`, inspect before any summary. Falsified if node status/reason are absent or run status/reason are synthesized. | Direct JSON fold in `.cyril-vhfz/oracle.py`. | <1 min | passed | `apply_node_paused` updates `run.status` instead of the node. | Core test `pause_ordering_matrix_preserves_intermediate_authority`. |
| 2 | Intervening frames preserve node state while run state remains absent. | Put two `steps_queued` frames between `node_paused` and `paused`. Falsified if either checkpoint loses node pause or gains run pause. | Source-derived new-order fixture plus direct JSON fold. | <1 min | passed | Queue reconciliation replaces the run and drops node progress, or copies final summary early. | Same matrix test, distinct per-frame assertions naming `new_before_summary`. |
| 3 | Legacy summary-only pause remains supported. | Replay live 2.16.0 repeat exhaustion. Falsified if `paused` is ignored without a preceding `node_paused` or if paused completion becomes terminal. | Committed live 2.16.0 capture fields, folded by `.cyril-vhfz/oracle.py`. | <1 min | passed | `apply_paused` requires a paused node as a precondition. | Core test `legacy_summary_only_pause_remains_resumable`. |
| 4 | 0.38.7 attribution extras are tolerated without changing authority. | Add `initiator` and `initiatorReason` to the late summary and paused completion. Falsified if either conversion rejects, node state changes, or run reason differs from `pauseReason`. | Exact KAS 0.38.7 source fields and probe checkpoints. | <1 min | passed | A wire struct gains `deny_unknown_fields`, or the parser maps initiator reason over pause reason. | Converter test `pause_frames_tolerate_attribution_extras`. |
| 5 | Both orderings converge and preserve reasons. | Compare final projections after paused `run_complete`. Falsified by any difference in status, node statuses, or node/run reasons. | Probe/oracle 10-row comparison includes sorted `(node path, node reason)` state; final rows are byte-identical. | <1 min | passed | `apply_completion` replaces event-only reasons instead of `preserve_event_only`. | Core test `pause_orderings_converge_after_completion`. |
| 6 | Replays stay idempotent and terminal behavior unchanged. | Apply each sequence twice, then apply completed/failed/aborted snapshots. Falsified by a second-pass change or a non-absorbing terminal state. | Existing cyril-6beh replay oracle and terminal status matrix. | 2 min | pending | New ordering branch special-cases first delivery or changes `is_terminal`. | `workflow_capture_replay_is_state_idempotent` plus the existing terminal tests. |
| 7 | Placement remains inside core. | Inspect the branch diff and dependency graph. Falsified by KAS field names outside the core adapter or ordering logic in UI/App/commands. | Repository architecture rules plus changed-file set. | <1 min | pending | UI matches `_kiro/workflow/paused` directly. | Changed-files assertion during review; crate dependency rules make ACP imports outside core fail compilation. |

### Cheapest falsifier run

Executed before approval:

```text
cargo run --manifest-path .cyril-vhfz/probe/Cargo.toml -- \
  crates/cyril-core/tests/fixtures/kas/workflow/oracle-replay-events.jsonl \
  .cyril-vhfz/source-derived-new-ordering.jsonl
python .cyril-vhfz/oracle.py <same two inputs>
```

Result: 10/10 checkpoint rows agreed. The decisive `new_before_summary` row is `run=None`, `paused_nodes=1`, `node_reasons=["wf_oracle/step=need-human"]`, `run_reason=None`. This kills the design variant that treats run status as immediate while proving the node-specific reason is sufficient and survives intervening frames.

## Negative space

1. No run-level pause synthesis from `node_paused`; preserving separate authorities is the compatibility behavior, not missing implementation.
2. No workflow renderer or pause chip; cyril-zd8u owns the verified presentation scope.
3. No `/workflow` prompt/control changes; the merged cyril-0qe6 v1 surface has no event-driven pause prompt, so this compatibility fence has no command consumer to migrate.
4. No new domain field for `initiator`/`initiatorReason`; this PR proves forward-compatible tolerance, while no accepted behavior reads summary attribution.
5. No fresh live 2.18.0 capture claim; the authenticated attempt failed before the pause trigger, so the design relies on the extracted production source plus committed live 2.16.0 evidence.
6. No changes to retries, terminal absorption, session ownership, node-path canonicalization, or queue-resolution semantics.

## Documentation effect

Tighten the public comments on `WorkflowRun::status`, `WorkflowRun::run_pause_reason`, and `WorkflowNodePaused`/`apply_node_paused` to state the timing contract. Update the fixture README with the source hash, derivation path, and exact old/new roles. No new domain noun is introduced; “node pause” and “run pause” already exist in the core types and cyril-6beh artifacts.
