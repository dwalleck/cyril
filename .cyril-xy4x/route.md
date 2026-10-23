# Route: cyril-xy4x

Change: Separate C9 lifecycle regression coverage from its isolated performance checkpoint.
Date: 2026-10-22

## Route tests

| Test | Verdict | Evidence |
|---|---|---|
| T1 Empirical premise | no | The existing observation already measures first worker event through awaited real capture IPC. Its source and retained 150 ms outside/inside controls are documented in `.cyril-n3j7/evidence.md`, C9 measurement-boundary repair. The current 163.159373 ms CI failure remains real; its cause need not be determined to separate the checks. |
| T2 Structural module shape | no | Only private Unix test fixtures and test registration in `crates/cyril/src/capture_forwarder.rs` change. Production ownership, interfaces, dependencies, and runtime behavior remain unchanged. |
| T3 Production-scale risk | no | No production path, batch limit, timing boundary, or shutdown bound changes. The existing 100 ms performance obligation survives as a required isolated pre-merge checkpoint. |
| T4 Explicit behavior | yes | Ordinary C9 retains real persistence, exact prompt/completed-state, contiguous three-event batching, completed drain, and graceful runtime shutdown assertions without a tight wall-clock ceiling. A separately invoked ignored test runs the same fixture and retains the exact <=100 ms worker-first-event-through-awaited-IPC assertion, including in-interval scheduling. C10 lifecycle coverage and bounded watchdogs remain unchanged. |

## Selected route and approval

**Local.** The requester explicitly selected **“Separate performance check”**: retain the existing 100 ms IPC-inclusive target as an isolated performance checkpoint before merging, while ordinary CI keeps deterministic batching, persistence, and shutdown assertions. This changes test scheduling, not the metric, threshold, product behavior, or historical results. No timeout, retry, warm-up exemption, scheduler subtraction, dependency, or accepted-risk waiver is introduced.

The original approved `.cyril-n3j7/design.md` and historical results remain unchanged. The existing [current C9 evidence section](../.cyril-n3j7/evidence.md#c9-ordinaryperformance-split--cyril-xy4x-2026-10-23) now owns the approved split, exact invocation, and scoped qualification; the existing Slice 2 plan references that policy.

## Artifacts and sequence

`route.md` is the only new workflow document required for this Local change. Compact portable raw qualification receipts live under `.cyril-n3j7/evidence/capture-timing-policy-20261023/`; the existing evidence section is their single current claim/result owner.

Implementation → coordinator's sole-HOST qualification and real-runtime checkpoint smoke → ordinary independent source review → evidence/plan amendment → separate PR and green CI before merge. Main owns Git, tracker, PR, merge, and qualification coordination. Isolated design reconstruction is not required by the ordinary independent-review default.

## Required commands and evidence

Run from this worktree on Unix, through the coordinator's sole-HOST lease:

- Ordinary C9: `cargo test -p cyril --bin cyril capture_forwarder::tests::c9_forwarder_batches_and_drains_before_runtime_shutdown -- --exact --nocapture`
- Isolated required performance checkpoint: `cargo test -p cyril --bin cyril capture_forwarder::tests::c9_forwarder_batch_stays_within_100ms_including_awaited_ipc -- --ignored --exact --nocapture --test-threads=1`
- Existing C10 lifecycle: `cargo test -p cyril --bin cyril c10_shutdown_waits_bridge_then_drains_capture_then_stops_memory -- --nocapture`
- Applicable repository checks: `cargo fmt --all -- --check`; `cargo clippy -p cyril --all-targets --all-features -- -D warnings`; required CI feature/platform matrix.

The isolated checkpoint receipt must show exactly one executed, passing case with zero ignored cases, its actual batch observation, and the checked source/profile/features/environment. A default-suite ignored/skipped result is not performance evidence. Run it alone, not concurrently with other Cargo/test workloads; do not repeat until green.

The affected checkpoint's outside-worker exclusion, inside-real-IPC assertion-specific rejection, and byte-exact restored GREEN proof have been executed and are recorded in the current C9 evidence section. Prior controls retain their historical source-state meaning; they are not substituted for this changed-registration proof.

## Scoped checkpoint and terminal criterion

**Scoped qualification and final independent review PASS; CI pending.** The coordinator completed the ordinary lifecycle checks, explicitly executed unchanged 100 ms real-runtime checkpoint, scoped controls/restorations, formatting check, and strict App lint. The source worker ran no checks. Existing evidence/plan wording records the split; [final ordinary independent review](review.md) found no actionable source, proof, or documentation defects. Main owns PR, CI, and merge gating. Original CI failures remain FAIL, not flaky or waived.

This route is complete only when the recorded scoped proof remains applicable to the delivered source, final ordinary independent review has no unresolved blockers, and the required CI feature/platform matrix is green. No standalone App/native/provider qualification is claimed. Scoped qualification and final review do not substitute for CI or merge authorization.
