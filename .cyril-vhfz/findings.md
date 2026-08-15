# Prove-it findings

## Smallest question

Does the shipped `WorkflowTracker` preserve an immediate node pause and converge to the same resumable paused snapshot under both KAS orderings: the 2.16.0 park-time run summary and the 0.38.7 pre-`run_complete` run summary?

## Production evidence

Two independent upstream observations pin the orderings:

- The committed live 2.16.0 repeat/watch capture emits run-level `paused`, then `steps_queued`, then paused `run_complete` (`crates/cyril-core/tests/fixtures/kas/workflow/kas-repeat-watch-2.16.0.jsonl`, frames 139–142).
- The extracted kiro-cli 2.18.0 / KAS 0.38.7 `acp-server.js` (SHA-256 `965ae084945a48eb73fe2049feed7e3deb6fb8d8a9cf49aa4713b172ed3fb70a`) emits immediate `node_paused` at each park site, unwinds the executor, emits run-level `paused` at source lines 439940–439952, and immediately emits `run_complete` at 439954–439967. `pausedAttributionFields` at 437581–437583 contributes `initiator` and `initiatorReason` when present.

A live 2.18.0 node-pause recapture was attempted with the repository's existing harness. The host credential had expired, so the step failed before the pause trigger. The failed capture was discarded; it is not evidence for this issue. The extracted production bundle and existing live 2.16.0 capture are the evidence boundary.

## Probe

`.cyril-vhfz/probe/src/main.rs` is a 63-line standalone Rust probe. It enables cyril-core's dev-only `test-support` seam, sends each JSONL frame through the same KAS conversion path as the live bridge, applies each `WorkflowEvent` to the shipped `WorkflowTracker`, and prints the run status, paused-node count, and run pause reason at `node_paused`, queue, run-summary, and completion boundaries.

Inputs:

1. The committed old-ordering synthetic lifecycle fixture used by cyril-6beh.
2. `.cyril-vhfz/source-derived-new-ordering.jsonl`, the same production-shaped lifecycle sequence with the run summary moved to the exact 0.38.7 source position immediately before `run_complete`; it also carries the new `initiator`/`initiatorReason` extras.
   `.cyril-vhfz/derive-new-ordering.py` regenerates this fixture byte-for-byte (SHA-256 `bcf915cadd6b1ec5611a028f68c7b53f2c131f300b4e3e92bce556be371bc49d`).

## Independent oracle

`.cyril-vhfz/oracle.py` folds the JSON wire fields directly without importing Cyril. It independently counts paused nodes from `nodePath` and the final snapshot tree, applies wire statuses, and prints the same checkpoint projection.

Observed comparison: 10/10 rows byte-identical after excluding command timing text.

```text
old: node_paused  run=None          paused_nodes=1  run_reason=None
old: paused       run=Some(Paused)  paused_nodes=1  run_reason=Some("operator")
old: steps_queued run=Some(Paused)  paused_nodes=1  run_reason=Some("operator")
old: run_complete run=Some(Paused)  paused_nodes=2  run_reason=Some("operator")

new: node_paused  run=None          paused_nodes=1  run_reason=None
new: steps_queued run=None          paused_nodes=1  run_reason=None
new: paused       run=Some(Paused)  paused_nodes=1  run_reason=Some("operator")
new: run_complete run=Some(Paused)  paused_nodes=2  run_reason=Some("operator")
```

The existing converter ignores unknown run-summary attribution fields because `WirePaused` does not deny unknown fields; both extras passed through the real adapter without rejecting the event.

## What I learned

The final tracker state was already order-tolerant, but the intermediate run status is intentionally not: under 0.38.7 it remains absent through intervening queue frames while the paused node is already observable. Therefore `WorkflowNodeStatus::Paused` plus the node pause reason is the only cross-version immediate pause contract; run-level `paused` is a late summary and cannot drive prompt/chip timing.

## Evidence boundary

This proves conversion tolerance, per-frame intermediate state, and final convergence for both orderings. It does not prove a fresh authenticated 2.18.0 live run, renderer behavior from cyril-zd8u, or `/workflow` behavior from the still-in-progress cyril-0qe6 branch. Those consumers are not present on `main` and must consume the core immediate-node contract when they land.
