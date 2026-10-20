# cyril-1v25 — missing-start tool updates

Date: 2026-10-19. Base: main 8e5c325e. Owner: MissingToolResults; integration/verifier/review/publication owner: Main.

## Request and outcomes

Authoritative request: full `rivets show cyril-1v25 --json`, including the 2.32.0 live-seat note. Unknown-id `ToolCallUpdated` notifications must become ordinary tracked chat rows using the update's own fields. Commit them at arrival, flushing preceding user/agent/thought streams exactly as an initial tool call does. KAS `Tool Load` and `Subagent Response` headers must render. Repeated updates and a later start for the same retained ID must not duplicate the row; normal known-ID `TrackedToolCall::update` / `ToolCall::merge_update` behavior remains unchanged. Preserve chronological position across turn completion and history trimming. Subagent nesting/routing (`cyril-ebqu`) and tool-load input unwrapping (`cyril-2yn8`) are not part of this repair.

## Route tests

| Test | Verdict and evidence |
| --- | --- |
| T1 — empirical premise | No unresolved consequential premise. The issue provides committed live captures and the exact symptom. Existing `UiState::apply_notification` starts commit/index a row; its update branch only merges existing IDs. Production KAS conversion replay is already available through `cyril_core::test_support::kas_capture_to_routed`. Capture selection is repository evidence, not a runtime-design uncertainty. |
| T2 — structural module shape | No. UI chronology, committed rows, the live list, and the tool-call index already belong to `crates/cyril-ui/src/state.rs` (AGENTS component ownership). Repair stays there, with behavioral integration tests. No production schema, public signature, dependency direction, or new responsibility owner changes. LSP references for `UiState::apply_notification` were requested; rust-analyzer is ready but returned no references for this linked-worktree path. No symbol rename/signature migration is planned. |
| T3 — production-scale risk | No introduced material scale risk. Keep indexed known-ID lookup and update index offsets only when existing history pruning drains messages. The index remains bounded by retained chat history; do not introduce per-frame full-history search or an unbounded ID tombstone set. |
| T4 — explicit behavior | Yes. The issue and assignment establish visible rows, update-owned fields, arrival chronology, no duplicate retained-ID rows, unchanged normal merge semantics, capture-backed render qualification, and real TUI smoke. No product decision is missing. |

Selected route: **Local** (T1=no, T2=no, T3=no, T4=yes).

## Delivery boundary

This issue owns the shared production repair and KAS regression coverage. A separately reviewable, stacked cyril-fiv1 PR owns genuine v2 capture conversion/replay and failed-validation error visibility qualification, plus any v2-specific residual rendering defect. Do not duplicate the shared implementation in that PR. No commits or pushes from this worker.

Only workflow artifact required: this route. Spec/design/probe/plan artifacts are not required for the Local route; no new public architecture or unverified external premise is introduced.

## Regression and consumer proof

- Pre-fix-sensitive behavioral fence: replay real KAS orphan Tool Load/Subagent Response updates through production conversion into UiState and the actual chat widget; assert both labels exist exactly once. Original handler produces no rows, so these assertions fail on base.
- State fences cover preceding streamed user/agent/thought chronology, subsequent text, repeated updates, later starts, late updates after commit, pruning/clear index correctness, and unchanged field-preserving known-ID merge.
- Real TUI smoke: `.cyril-1v25/smoke.py` feeds captured payloads through the actual built Cyril process with `--agent-engine kas` and the normal KAS Free spawn path (owned Node wrapper, existing fake kiro-cli token helper, isolated HOME/PATH/cwd). It normalizes only session scope and adds chronology markers/repeated updates. Parent owns execution/receipts and the post-smoke permanent documentation/changelog update. The harness also accepts v2 captures for the stacked consumer issue; it is not a live-upstream qualification claim.
- Parent owns red-before/green-after execution, focused/full checks, independent issue review, and a nonempty issue PR.

## Completed edit slice and issue boundary

- Production: `crates/cyril-ui/src/state.rs` shares first-observed row insertion between starts/updates; flushes prior streams, indexes retained rows through commit, adjusts indices when trimming, clears indices with history, reuses rows on repeated starts/updates, and invalidates message rendering on merge. Existing `TrackedToolCall::update` remains the normal merge. F1 additionally retires index/live state only when SessionCreated changes main session identity, preserving old history without cross-session ID collisions.
- Permanent behavioral fences: `crates/cyril-ui/tests/missing_tool_updates.rs` and `tests/fixtures/kas_missing_start_2_31_1.jsonl`.
- Runnable owned smoke fixture: `.cyril-1v25/smoke.py`. It uses the existing auth-helper fixture; no auth database or real account changes.
- This PR must not include the fiv1 `traits.rs`, core test-support, v2 test/fixture edits.

Parent commands, from this worktree (`BIN` is the absolute freshly built binary; output directories must not already exist):

```sh
env -u CARGO_TARGET_DIR cargo test -p cyril-ui --test missing_tool_updates
env -u CARGO_TARGET_DIR cargo test -p cyril-ui cancellation_captures_converge_to_one_failed_committed_call
env -u CARGO_TARGET_DIR cargo test -p cyril-ui tool_call_update
uv run --script .cyril-1v25/smoke.py --binary "$BIN" --engine kas --capture crates/cyril-ui/tests/fixtures/kas_missing_start_2_31_1.jsonl --expect "Tool Load" --expect "Subagent Response" --output /tmp/cyril-1v25-smoke
```

Pre-fix sensitivity was exercised by Main: on base `8e5c325e` with only the five new KAS test cases/fixture applied, `cargo test -p cyril-ui --test missing_tool_updates` failed all five cases (artifact 79). The capture case observed zero rows instead of two. This receipt predates the F1 session-boundary regression and does not claim coverage for it.


## Recorded parent evidence

Checked source state for this checkpoint: base `8e5c325e` plus the uncommitted `crates/cyril-ui/src/state.rs`, `traits.rs`, `crates/cyril-core/src/test_support.rs`, the named KAS/v2 integration tests and three fixtures. This is the **pre-F1 session-boundary repair** state, not an invented commit hash. Main used `/home/dwalleck/repos/cyril/target` as the selected build target directory.

Main supplied the exact successful artifact 92 command chain:

```sh
env -u CARGO_TARGET_DIR cargo fmt --all &&
env CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target cargo test -p cyril-ui --test missing_tool_updates --test v2_failed_validation &&
env CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target cargo test --workspace &&
env CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target cargo clippy --workspace --all-targets -- -D warnings &&
env CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target cargo build -p cyril
```

| Obligation | Result and owning receipt |
| --- | --- |
| Shared-repair regression sensitivity | PASS — the expected RED on original source is recorded above, artifact 79. |
| Repaired focused tests | PASS — original five KAS cases plus two v2 cases, seven passed; artifact 92. |
| Assembled repository tests | PASS — Main's workspace test run completed; artifact 92. Ignored tests in that run remain ignored, not claimed as exercised. |
| Clippy and executable build | PASS — Main's exact command chain above identifies workspace/all-targets clippy with `-D warnings` and the cyril executable build; artifact 92 records their successful stage output. |
| Formatting check | PENDING — Main: `cargo fmt --check` for final source/doc state. No formatting-check PASS receipt has been supplied. |
| Real-process KAS TUI | PASS — `.cyril-1v25/smoke.py`, KAS Free fixture path, exact expected labels once and arrival ordering. Original outputs: `/tmp/cyril-1v25-smoke-shell-20261020/{result.json,screen.txt,terminal.ansi,agent-wire.jsonl}`. Portable copies: `receipts/smoke-kas.json` and `receipts/smoke-kas-screen.txt`. |
| Permanent user documentation | Delivered after smoke — README's **Tool call history** section documents update-first rows, KAS examples, chronology and merging. This is this PR's README hunk; the separate v2 validation paragraph belongs to fiv1. |

The successful smoke followed an initial harness startup failure caused by no supported shell in its deliberately private PATH. Main corrected only the fixture environment to `SHELL=/bin/bash`; production code was unchanged. The successful receipts above, not the initial failure, own the TUI conclusion. No live-upstream KAS behavior or native Windows/macOS qualification is claimed.

## Current verification state

COMPLETE — Main verified the isolated `fix/cyril-1v25` delivery tree, excluding fiv1 production/helper/tests/fixtures. Artifact 188 records successful `cargo test --workspace` (including all six named KAS integration cases), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build -p cyril`, and `cargo fmt --all --check`. Build commands used `CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target`; fmt unset the ambient empty target variable. Before switching worktrees, Main touched the local crate entrypoint mtimes to force recompilation rather than trust a sibling's newer shared-target artifact. The raw receipt names this delivery checkout for compilation.

Independent F1 re-review by ReviewMissingKas: no blocking findings, confidence 0.98; same-session acknowledgments, pruning and history clearing remain correct. See `review-decisions.md`.

Final real-process smoke on that freshly built binary:

```sh
uv run --script .cyril-1v25/smoke.py --binary /home/dwalleck/repos/cyril/target/debug/cyril --engine kas --capture crates/cyril-ui/tests/fixtures/kas_missing_start_2_31_1.jsonl --expect 'Tool Load' --expect 'Subagent Response' --two-sessions --output /tmp/cyril-1v25-final-two-session-20261020
```

PASS: actual `/new` followed by an ordinary second prompt created two distinct session IDs; the same captured tool IDs retained separate chronological rows in both sessions. Portable result and terminal text are in `receipts/final-two-session.json` and `receipts/final-two-session-screen.txt`. This is an owned ACP peer through the real Cyril process, not a live-upstream or native Windows qualification. The README documents only this issue's tool-history behavior; the v2-only validation-error paragraph is excluded.
