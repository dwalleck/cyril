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

## Original missing-start delivery verification

COMPLETE — Main verified the isolated `fix/cyril-1v25` delivery tree, excluding fiv1 production/helper/tests/fixtures. Artifact 188 records successful `cargo test --workspace` (including all six named KAS integration cases), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build -p cyril`, and `cargo fmt --all --check`. Build commands used `CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target`; fmt unset the ambient empty target variable. Before switching worktrees, Main touched the local crate entrypoint mtimes to force recompilation rather than trust a sibling's newer shared-target artifact. The raw receipt names this delivery checkout for compilation.

Independent F1 re-review by ReviewMissingKas: no blocking findings, confidence 0.98; same-session acknowledgments, pruning and history clearing remain correct. See `review-decisions.md`.

Final real-process smoke on that freshly built binary:

```sh
uv run --script .cyril-1v25/smoke.py --binary /home/dwalleck/repos/cyril/target/debug/cyril --engine kas --capture crates/cyril-ui/tests/fixtures/kas_missing_start_2_31_1.jsonl --expect 'Tool Load' --expect 'Subagent Response' --two-sessions --output /tmp/cyril-1v25-final-two-session-20261020
```

PASS: actual `/new` followed by an ordinary second prompt created two distinct session IDs; the same captured tool IDs retained separate chronological rows in both sessions. Portable result and terminal text are in `receipts/final-two-session.json` and `receipts/final-two-session-screen.txt`. This is an owned ACP peer through the real Cyril process, not a live-upstream or native Windows qualification. The README documents only this issue's tool-history behavior; the v2-only validation-error paragraph is excluded.

## Approved successful KAS output extension — 2026-10-20

Main's assignment explicitly records user approval to extend the missing-results batch / PR #183 with the newly captured successful `rawOutput.output` presentation residual. This does not expand into authentication, approval, terminal ownership, or shell execution. The earlier missing-start checks and receipts above remain evidence for their original inputs only; none qualifies this newly captured shape.

The requester directs: “Narrow existing output_text interpretation to string-valued output after established extraction precedence; no generic serialization/message fallback or Execute-only guard (completedkindabsent). No new abstraction.”

Source pin: published `fix/cyril-1v25`, PR #183, `a5d2c4b5d5db88889ba5bc5e3f560375715977cd`. Canonical sanitized input: `/tmp/cyril-xavt-live.ZiHasL/evidence/completed-tool-call-nonsecret.json`, SHA-256 `0d613a49514f698c5dd3862d62a24b0fe5616f544d0078880898a441b68720cc`. Only this supplied sanitized source is consumed. Its `parsed` frame has Completed status, absent `kind`, terminal-reference content, and the original `rawOutput` with `exitCode: 0`, formatted `message`, and `output: "XAVT_BEGIN XAVT_END\n"`.

### Extension route and conservative projection decision

| Test | Verdict and evidence |
| --- | --- |
| T1 — empirical premise | No unresolved external premise. The supplied sanitized completion and InspectLiveShellOutput establish the real shape and missing body. No new account access or live shell execution is needed; replay exercises the existing production conversion/state/render path. |
| T2 — structural module shape | No. `TrackedToolCall::output_text` already owns raw-output display interpretation. Add one string-valued direct field at that boundary, preserving raw data and all public signatures; no transport, terminal, state-owner, or schema change. LSP references were requested for this linked-worktree symbol and returned none; its existing renderer use is visible in `widgets/chat.rs`. |
| T3 — production-scale risk | No introduced material scale risk. The approved repair adds one constant-key string probe to an existing bounded extraction list, using its existing return-string convention. |
| T4 — explicit behavior | Yes. Preserve the captured frame and retained raw output, one completed chronological row, exact `output_text() == "XAVT_BEGIN XAVT_END\n"`, and actual body line `| XAVT_BEGIN XAVT_END`. Existing stdout/stderr, item envelopes, and text/content/result fields keep precedence. Ignore non-string `output`; do not use formatted `message`, serialize otherwise-unrecognized objects, require Execute kind, or invent terminal text. |

Selected route remains **Local**. This section owns the extension's design decision and approval; no additional workflow document or abstraction is required.

### Extension regression and actual-consumer obligations

- Permanent captured-input regression: `kas_completed_output_capture_renders_exact_body_without_kind` in `crates/cyril-ui/tests/missing_tool_updates.rs`, using byte-preserved `tests/fixtures/kas_completed_raw_output.json`. Parse its `parsed` envelope without payload normalization, then use the existing `kas_capture_to_routed` → `UiState::apply_notification` → actual chat renderer. Assert retained raw input/output, Completed status, Other kind from absent wire kind, exact output text, one completed header, and the separate exact body line (not the command/header marker).
- Narrow projection controls live beside existing `TrackedToolCall::output_text` unit tests: established extraction keys beat `output`; empty string is retained as an ordinary direct string; null/bool/number/array/object `output` values and a formatted `message` alone do not become display output.
- Controlled-peer actual-binary replay emits the captured `parsed` frame unchanged, retaining its session identity as well as terminal content, absent kind, raw input, and every raw-output field. The existing `.cyril-1v25/smoke.py` fixture isolation, fake token helper, PTY TUI consumer, duplicate-update check, and chronology markers remain the harness. Its opt-in `--preserve-session-id` returns the fixture's sole session identity from `session/new`, disables payload rebinding, and asserts captured-frame equality against emitted wire frames. It is mutually exclusive with `--two-sessions`; legacy normalization is unchanged without the flag. Only notifications are replayed; the command in `rawInput` stays inert data, no permission request/grant is introduced, and no real account is accessed.
- **PASS — Main's ActualRED822:** published `1v25` source plus the frozen new tests executed one regression and failed at the intended consumer oracle: `(None, 0 body lines)` versus `(Some("XAVT_BEGIN XAVT_END\n"), 1)`. Completed-row/header and retained-raw-output controls reached that oracle. Main explicitly authorized only the approved string-valued `output` fallback after this actual RED.
- **PASS — extension qualification:** Main's artifact 847 supplies focused GREEN, workspace checks, build and formatting; fresh ReviewMissingKas source review found no blocking findings; MissingToolResults supplied one actual old RED and one actual repaired PASS using the same frozen input/harness. Portable evidence and the publication manifest are indexed by `receipts/kas-output-final-handoff.json`. Main retains commit/PR/descendant-stack publication ownership.

### Immutable first-phase handoff

`receipts/kas-output-red-handoff.json` records source pin, exact test names, file hashes, and the Main-owned RED/consumer commands. `receipts/kas-output-test-only.patch` is the immutable regression-only delta against the published source pin: the captured fixture, one public conversion/state/render integration regression, and two colocated projection controls; it contains no production repair. Patch SHA-256: `84f7af326722e14ea9d7a00c948e410b6c6790a6bb9fee763a9f1fa1b13dd635`.

The smoke harness is a separate **worktree-applied test-harness-only edit**, not part of the regression-only patch. Its reproducible delta is `receipts/kas-output-harness-only.patch`, SHA-256 `57cd631affdc7f9082fc5f8c338313da4246fcb406b90f7a1a4952868903f40b`; `receipts/kas-output-harness-handoff.json` records the base/prepared harness hashes and reconstruction instructions. Reconstruct the consumer from the source pin plus **both** patches and the separately pinned trace input. The original immutable test-only patch and handoff remain unchanged.

The controlled-peer input is `receipts/kas-completed-output-trace.jsonl`, SHA-256 `f80aae8a8fac961f4224c505d112231a669c71784c4b7b993026bd5b5a4c5e71`. It adds only the existing trace envelope around the canonical source's unchanged `parsed` frame. Main can run the actual binary using:

```sh
uv run --script .cyril-1v25/smoke.py --binary "$BIN" --engine kas --capture .cyril-1v25/receipts/kas-completed-output-trace.jsonl --preserve-session-id --expect "✓ Run the exact sed/tr pipeline requested" --expect "| XAVT_BEGIN XAVT_END" --output "$FRESH_OUTPUT"
```

First-phase worker receipt: source HEAD and canonical capture SHA were read and matched the pins above. The worker performed no builds, tests, lint, formatter, or smoke runs and claimed no actual RED/GREEN. No commit, push, or tracker mutation was performed. Production was held until Main supplied ActualRED822 and explicit GO.

### Authorized production delta

After ActualRED822, `crates/cyril-ui/src/traits.rs` appends `"output"` to the existing ordered direct-string probe list in `TrackedToolCall::output_text` and documents that field in the method's extraction list. This is the complete production delta: existing `as_str` interpretation and all earlier precedence remain unchanged; no generic-object serialization, `message` fallback, Execute guard, status change, terminal behavior, or new abstraction was added. Fixture, regression, smoke harness, and immutable handoff artifacts remain frozen.

Main supplied separately hashed pre-extension and repaired private binaries for the actual-consumer comparison. MissingToolResults ran exactly one authorized replay per binary, without retries, retaining the same captured frame, harness, and session identity. The old body assertion failed as intended; independent saved-wire/screen checks qualify that RED because the frozen harness reaches its own frame-equality assertion only after presentation checks pass. The repaired replay passed, including the unchanged-frame assertion. Canonical portable receipts are `receipts/kas-output-old-replay.json` and `receipts/kas-output-new-replay.json`; their original paths, binary/input hashes, exact invocations, cleanup checks, and evidence hashes remain intact.

No worker builds, tests, lint, formatter, smoke, commit, push, or tracker mutation were performed. The independently integrated auth-fixture prerequisite `6c0f41c74bd94fa49ab1b3a2ae06e3ed70d3de69` (merge `9e537238d88f7afea4eb676710db9ca36e016b90`) is test-only and already part of the checked base; auth source and the git index were outside this worker's edits and are excluded from the extension publication manifest.

### Final extension evidence and publication handoff

Checked source: the named `1v25` worktree at merge `9e537238d88f7afea4eb676710db9ca36e016b90` plus the uncommitted successful-output extension. This is not an invented extension commit. Final named-file hashes and the exact publication allowlist live in `receipts/kas-output-final-handoff.json`; the initial frozen RED/harness handoffs remain immutable historical preparation records.

| Obligation | Result and portable evidence |
| --- | --- |
| Actual regression sensitivity | PASS — Main's ActualRED822: one intended failure at `(None, 0)` versus exact stdout and one body line; `receipts/kas-output-red-regression.log`. Row, header, and raw-output controls reached the oracle. |
| Focused repaired tests | PASS — artifact 847: the captured conversion/state/render regression and both precedence/non-string controls executed; `receipts/kas-output-green-gates.log`. |
| Repository tests and checks | PASS — Main reports 2,830 workspace tests passed, 13 ignored (not exercised), workspace/all-targets clippy with `-D warnings`, default executable build, and `cargo fmt --all --check`. The full captured output is `receipts/kas-output-green-gates.log`; Main's exact bg127 command chain and original-output provenance are explicit in the canonical final handoff. |
| Actual binary comparison | PASS — one old behavioral RED and one repaired PASS, no retries. Completed headers stayed at one; exact body count changed zero to one between the chronology markers. Both wires contain the complete original captured update twice (intentional duplicate-update probe), with preserved session identity. See the old/new portable replay receipts, screens, wire, terminal captures, and fake-helper argv receipts. |
| Safety and qualification boundary | PASS for the owned replay — no payload shell execution, agent permission/terminal requests, permission grants, or real-account access; cleanup found no owned app/agent matches. This is controlled-peer actual-Cyril execution with PTY-decoded text, not a new live-upstream run, native-terminal glyph qualification, or a claimed separately recorded Cyril exit status. |
| Independent review | PASS — fresh ReviewMissingKas source review: correct, no blocking findings, confidence 0.98. Portable output: `receipts/kas-output-source-review.txt`; this review itself claims no test or consumer execution. |
| Permanent documentation | Delivered after repaired actual replay — README's existing **Tool call history** section documents successful string-valued `rawOutput.output`, omitted kind, retained precedence, and the non-string/formatted-message boundary. |

The canonical handoff includes every extension publication path and maps portable copies back to original evidence. All specified implementation, regression, consumer, review, and documentation obligations are satisfied for this checked state. **PENDING — Main publication only:** commit the explicit manifest, update PR #183, and refresh the descendant stack under the existing publication authorization. No worker commit, push, tracker update, or independent claim of publication is made.

