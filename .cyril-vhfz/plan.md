# Plan: KAS workflow pause-order compatibility fence

Approved design: `.cyril-vhfz/design.md` at commit `f588037`.
Cheapest falsifier: the 49-line Rust probe and independent Python oracle agree on all 10 checkpoints for the legacy and KAS 0.38.7 orderings.

Every slice is a regression-characterization or documentation slice. No slice changes production state transitions: the probe established that the shipped tracker already has the required behavior. `tdd-scoped` does not require an artificial RED because no function is added or semantically modified; if a new characterization fails against the shipped code, execution stops as design/oracle drift rather than changing production behavior to force GREEN. After every logic slice, the oracle command is the exact probe/oracle comparison recorded in `.cyril-vhfz/findings.md`.

## Slice 1: Fence immediate node-scoped authority

**Claim:** Applying a node pause immediately sets only the addressed node's status and node reason while run status and run reason remain absent.

**Oracle:** `.cyril-vhfz/probe/src/main.rs` versus `.cyril-vhfz/oracle.py`; both must report the pre-summary checkpoint as one paused node with `wf_oracle/step=need-human`, no run status, and no run reason.

**Stress fixture:** A run start plus one step start followed by a node pause whose reason is `等待 human`. Expected: only that exact canonical node is paused with the Unicode reason; the run status/reason remain absent. This fails a plausible implementation that writes pause state onto the run or loses non-ASCII reasons.

**Loop budget:** No production loop. The unit fixture applies 3 events and performs keyed run/node lookups: $O(e)$ for $e=3$, below 10 operations at fixture and production event scale.

**Wall budget:** None; unit-test-only work, not an always-on phase.

**Files:**
- `crates/cyril-core/src/workflow.rs`

**Regression fence:** `pause_ordering_matrix_preserves_intermediate_authority` in `workflow.rs`.

**Verification:**
- [ ] Focused characterization test passes on the shipped state transition; any failure halts the slice
- [ ] Stress fixture produces exactly one paused node with the Unicode reason and no run pause
- [ ] Prove-it-prototype oracle still agrees with the binary
- [ ] No new production loop or wall cost exists

## Slice 2: Fence intervening-frame preservation

**Claim:** Queue/lifecycle frames between node pause and run pause preserve immediate node state without inventing run state.

**Oracle:** The independent direct JSON fold and the real tracker must agree at both pre-summary queue checkpoints.

**Stress fixture:** Apply both a resolution-free non-empty queue update and a resolution-bearing empty acknowledgement between node pause and run pause. Expected at both boundaries: the node remains paused with its reason, run status/reason remain absent, and the two existing queue semantics remain distinct. This fails replacement-based queue reconciliation or early run-summary synthesis.

**Loop budget:** No production loop. One test folds at most 6 events, $O(e)$ with $e\le6$; keyed state updates remain constant expected time per event.

**Wall budget:** None; unit-test-only work.

**Files:**
- `crates/cyril-core/src/workflow.rs`

**Regression fence:** Extend `pause_ordering_matrix_preserves_intermediate_authority` with named pre-summary queue checkpoints.

**Verification:**
- [ ] Focused unit test passes
- [ ] Both adversarial queue shapes preserve node authority and absent run authority
- [ ] Prove-it-prototype oracle still agrees with the binary
- [ ] Event count and lookup budget remain bounded as stated

## Slice 3: Preserve legacy summary-only resumability

**Claim:** A legacy run pause with no preceding node pause still sets run status/reason, and paused completion remains non-terminal/resumable.

**Oracle:** Run `.cyril-vhfz/oracle.py` directly on the committed live 2.16.0 repeat/watch capture and require the summary-first rows (`paused` with zero paused nodes, then paused `run_complete`). The existing `workflow_capture_replay_matches_independent_folder` test remains the permanent whole-capture fence.

**Stress fixture:** Start a run, apply run pause directly, then reconcile a paused snapshot and apply a later active-run update. Expected: run reason is present before completion, no node reason is invented, paused completion is accepted without terminal absorption, and the later update is accepted. This fails any `apply_paused` precondition on a paused node or any treatment of `Paused` as terminal.

**Loop budget:** No production loop. The fixture applies at most 4 events, $O(e)$ with $e\le4$.

**Wall budget:** None; unit-test-only work.

**Files:**
- `crates/cyril-core/src/workflow.rs`

**Regression fence:** `legacy_summary_only_pause_remains_resumable` in `workflow.rs`.

**Verification:**
- [ ] Focused legacy summary-only test passes
- [ ] Paused completion demonstrably permits a subsequent active-run update
- [ ] Prove-it-prototype oracle still agrees with the binary
- [ ] Independent oracle on `kas-repeat-watch-2.16.0.jsonl` reports summary-first pause with zero paused nodes before completion
- [ ] No production complexity changes

## Slice 4: Exercise exact 0.38.7 wire extras

**Claim:** Late run-pause and paused-completion frames carrying `initiator`/`initiatorReason` convert successfully without changing pause authority.

**Oracle:** Extracted KAS 0.38.7 source positions and the independent JSON fold; the source-derived fixture must retain SHA-256 `1f39a6f6424680cb6b88435ca4075bfd0f8495b6970f61cc214decbcf31bb28e` when copied canonically.

**Stress fixture:** Canonicalize the source-derived ordering fixture and replay both attributed frames through the real KAS adapter. Include different `pauseReason` and `initiatorReason` strings. Expected: both frames convert; run pause reason remains `pauseReason`; attribution is ignored rather than reinterpreted. Bare legacy pause remains covered by existing converter tests. This fails `deny_unknown_fields` or accidental attribution-to-domain mapping.

**Loop budget:** No production loop. Test replay scans one fixture of fewer than 64 JSONL frames, $O(f)$ with $f<64$; this is offline test work. Existing adapter work per frame is unchanged.

**Wall budget:** None; fixture replay is not always-on production work.

**Files:**
- `crates/cyril-core/tests/fixtures/kas/workflow/pause-late-summary-2.18.0-source-derived.jsonl`
- `crates/cyril-core/src/protocol/convert/kas/workflow.rs`

**Regression fence:** `pause_frames_tolerate_attribution_extras` in the KAS workflow converter tests.

**Verification:**
- [ ] Canonical fixture hash matches the approved source-derived artifact
- [ ] Attributed `paused` and `run_complete` both convert
- [ ] Stress fixture proves `pauseReason` remains authoritative
- [ ] Prove-it-prototype oracle still agrees with the binary
- [ ] Replay scan remains below 64 frames and introduces no production loop

## Slice 5: Fence old/new final convergence and reason preservation

**Claim:** Old and new orderings converge to the same non-terminal paused run projection while preserving event-only node and run reasons.

**Oracle:** The 10-row probe/oracle comparison, including sorted `(node path, node reason)` state and byte-identical final rows.

**Stress fixture:** Fold two manually identical lifecycles differing only in summary placement. Give the node reason and run reason deliberately different values; reconcile a paused snapshot that contains neither reason. Expected: final runs compare equal, both reasons survive, and status is paused. This fails order-dependent replacement or `preserve_event_only` regressions.

**Loop budget:** No production loop. Two sequences of at most 7 events each are folded once: $O(o\times e)$ for $o=2$, $e\le7$, at most 14 event applications.

**Wall budget:** None; unit-test-only work.

**Files:**
- `crates/cyril-core/src/workflow.rs`

**Regression fence:** `pause_orderings_converge_after_completion` in `workflow.rs`.

**Verification:**
- [ ] Focused convergence test passes
- [ ] Distinct node/run reasons survive snapshot reconciliation in both orders
- [ ] Prove-it-prototype oracle still agrees with the binary
- [ ] Fixture operation count remains at most 14 event applications

## Slice 6: Fence replay idempotence and terminal absorption

**Claim:** Replaying either pause ordering twice is idempotent, and completed/failed/aborted terminal behavior is unchanged.

**Oracle:** `.cyril-6beh/oracle-replay.py` independently folds every `REPLAY_SOURCES` fixture once and twice and refuses any mismatch. Add the canonical 0.38.7 ordering to that source set and regenerate the byte-exact expected projection; the existing terminal-status matrix remains the terminal oracle.

**Stress fixture:** Add the 27-frame late-summary source to `REPLAY_SOURCES`. Expected: the independent Python folder emits `oneEqualsTwo: true`, the Rust one-pass/two-pass projections match it exactly, legacy sources remain byte-identical, and existing completed/failed/aborted terminal tests remain green. This fails first-delivery special cases, an oracle coupled only to the old ordering, or accidental terminality changes.

**Loop budget:** Test-only incremental work is $O(p\times f+t)$ with 2 passes over the 27-frame new fixture and 3 existing terminal statuses: at most 57 incremental event/status applications. No production loop changes.

**Wall budget:** None; unit-test-only work.

**Files:**
- `crates/cyril-core/src/protocol/convert/kas/workflow.rs`
- `crates/cyril-core/tests/fixtures/kas/workflow/oracle-replay-expected.json`

**Regression fence:** `workflow_capture_replay_matches_independent_folder`, `workflow_capture_replay_is_state_idempotent`, and the existing terminal-status matrix tests.

**Verification:**
- [ ] Independent Python oracle accepts one and two passes over both pause orderings
- [ ] Rust replay projections match the regenerated nine-source oracle byte-exactly
- [ ] Existing completed/failed/aborted terminal-status tests pass unchanged
- [ ] Prove-it-prototype oracle still agrees with the binary
- [ ] Incremental matrix stays within the 57-operation budget

## Slice 7: State the public timing contract at the core seam

**Claim:** Pause timing knowledge remains local to core and the existing public interface states the node-immediate/run-summary distinction without adding a duplicate predicate.

**Oracle:** Public API inspection against the approved design: `WorkflowEvent::NodePaused` plus node accessors are immediate; run accessors are summary state; no `is_paused` helper exists.

**Stress fixture:** Search the two edited core files for every pause-related public comment and API. Expected: comments cannot be read as promising immediate run status, no KAS method string is introduced in the state module, and no unenforced caller precondition is added. This fails stale/misleading contract text or boundary leakage.

**Loop budget:** No runtime logic and no new loops; comments only.

**Wall budget:** None; no executable change.

**Files:**
- `crates/cyril-core/src/workflow.rs`
- `crates/cyril-core/src/types/workflow.rs`

**Regression fence:** Manual API/comment inspection; this slice changes no behavior.

**Doc-comment enforcement classification:** Descriptive timing semantics only. No caller precondition, so neither runtime enforcement nor `debug_assert!` is required.

**Verification:**
- [ ] Public comments distinguish node pause from run pause
- [ ] No new method, predicate, field, branch, or loop is added
- [ ] Prove-it-prototype oracle still agrees with the binary
- [ ] Core API tests remain green

## Slice 8: Document evidence provenance and prove placement

**Claim:** The compatibility fixture and pause-ordering knowledge remain in `cyril-core`; no UI, App, or command module receives wire parsing or a duplicate ordering predicate.

**Oracle:** Repository architecture rules plus a mechanical changed-file allowlist against `main`.

**Stress fixture:** Enumerate every changed path and reject any production path outside `CONTEXT.md`, `.cyril-vhfz/`, `crates/cyril-core/src/workflow.rs`, `crates/cyril-core/src/types/workflow.rs`, `crates/cyril-core/src/protocol/convert/kas/workflow.rs`, and `crates/cyril-core/tests/fixtures/kas/workflow/`. Expected: zero disallowed paths. This fails accidental UI/App/command placement. Update the fixture table with source hash, derivation path, attribution mirror, and old/new ordering role.

**Loop budget:** Mechanical review loops once over at most 20 changed paths: $O(p)$ with $p\le20$ and one `git diff` process. No runtime loop.

**Wall budget:** None; review/documentation only.

**Files:**
- `crates/cyril-core/tests/fixtures/kas/workflow/README.md`

**Regression fence:** Manual changed-file allowlist and fixture hash/provenance check.

**Verification:**
- [ ] Fixture provenance and immutable/source-derived status are explicit
- [ ] Changed-file allowlist reports zero violations
- [ ] Prove-it-prototype oracle still agrees with the binary
- [ ] Path scan remains within 20 paths and one process invocation

## Plan Self-Review

1. **Loops:** No production loop is introduced. Every test/review loop has an explicit formula and concrete bound; maximum is one `<64`-frame offline fixture scan or 31 synthetic event applications.
2. **Fixtures:** Each slice names a plausible bug: authority conflation, queue replacement, summary precondition/terminality, strict unknown-field parsing, reason loss, replay special-casing, misleading API contract, or cross-layer leakage. Expected outcomes are fixed before implementation.
3. **Doc-comment preconditions:** Slice 7 adds descriptive timing semantics only; no `must`, non-empty requirement, or other precondition is introduced. No enforcement gap exists.
4. **Write targets:** Production code writes nothing new. Test assertions are diagnostics on failure. The prove-it probe/oracle output is data on stdout; Cargo/compiler diagnostics remain stderr.
5. **Tracker references:** Implementation defers no work. The approved design's negative space cites verified issues `cyril-zd8u` (renderer/run panel) and `cyril-0qe6` (v1 command surface); neither scope is silently moved into this plan.

Claim coverage is complete: slices 1–6 cover design claims 1–6; slices 7–8 jointly cover claim 7 and its documentation/placement evidence.
