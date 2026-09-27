# Plan: cyril-ell0 — delete the dead `stream_buffer` module

## Design verification (step 1)

`design.md` read top to bottom on 2026-09-27: Falsification table has five rows (C1–C5), every cell filled; the cheapest falsifier (C2, pre-deletion form) is `PASS` with its run log; no row is `FAIL`; every `PENDING` row names `checkpointed-build, Slice 1 gate`; the Approval section carries the requester's verbatim words, the date, and `Risk acceptances approved: None`. Module shape applies (T2 yes): the ledger, protected parents, shape fence, and Length review `N/A` are present. Every row's oracle, mutation, and fence are mechanical.

## Module growth ledger

| Module | Baseline production lines | Projected final lines | Responsibility change | Interface change | Protected-parent rule |
|---|---:|---:|---|---|---|
| `crates/cyril-ui/src/stream_buffer.rs` | 67 | 0 | delete (debounced streaming flush, never live) | `StreamBuffer::{new,push,should_flush,flush}` removed | N/A — deleted |
| `crates/cyril-ui/src/lib.rs` | 22 | 21 | none | one `pub mod` declaration removed | declarations only; no `stream_buffer` token (C1) |

Protected parents not touched by any slice (fenced byte-identical by C5): `crates/cyril-ui/src/state.rs`, `crates/cyril/src/app.rs`. Non-code files touched: `CLAUDE.md`, `AGENTS.md` (one line each), `.cyril-ell0/**` (artifacts + oracle).

## Partition arithmetic

- Slice 1 diff estimate: 150 deleted code lines + 2 changed doc lines + ~520 artifact/oracle lines ≈ **672** changed lines.
- Churn margin: **25 %** (≈ 170 lines) — the artifacts grow as gate and review records are appended; code churn is nil.
- Total ≈ **842** ≤ 4,000 → **single PR increment** `cyril-ell0`, mergeable to the default branch (`origin/main`, discovered via `refs/remotes/origin/HEAD`) on its own; verified without any later increment by the Slice 1 gate below.

## Slice 1: Delete the dead module, its declaration, and its live-component doc mentions; add the shape fence

**Claim IDs:**             C1, C2, C3, C4, C5
**Expected behavior:**     After the slice, `bash .cyril-ell0/oracles/shape_fence.sh` prints `C1 PASS` … `C5 PASS` and exits 0; the six CI gate commands exit 0; the after-receipt `grep -rn 'StreamBuffer\|stream_buffer' crates/` lists only the legacy-config strings and the pre-existing `flush_stream_buffer` stub/caller (B2); `grep -in 'stream buffer' CLAUDE.md AGENTS.md README.md` is empty (B4).
**Oracle:**                The design rows' oracles: C1 — `git ls-files` + `cargo check --workspace --all-targets --all-features`; C2 — the same `cargo check` (rustc name resolution); C3 — `cargo nextest run -p cyril-core -p cyril --all-features -E 'test(nd4h)'`; C4 — cyril-ui `Owns:` line → module mapping against `ls crates/cyril-ui/src/`; C5 — `git diff --numstat <merge-base> -- crates/` = 0 added / 150 deleted.
**Stress fixture:**        N/A — the slice implements no logic (a file deletion, a declaration removal, two one-line prose edits); the fence and oracles are the check.
**Regression fence:**      `.cyril-ell0/oracles/shape_fence.sh` (created in this slice) — C1–C5; inherited repository gates (`cargo check`/`clippy`) back C1/C2; the inherited, untouched nd4h suite backs C3's behavior.
**Named mutation:**        M1 (C1) create `crates/cyril-ui/src/stream_buffer.rs` = `pub struct Placeholder;` → `C1 FAIL: crates/cyril-ui/src/stream_buffer.rs exists`; M2 (C2) append `pub use crate::stream_buffer::StreamBuffer;` to `crates/cyril-ui/src/lib.rs` → `C2 FAIL: crates/cyril-ui/src/lib.rs:…`; M3 (C3) append an empty line to `crates/cyril-core/tests/nd4h_legacy_config_compat.rs` → `C3 FAIL: … differs from merge-base`; M4 (C4) re-insert `, stream buffer` at the end of `CLAUDE.md`'s cyril-ui `Owns:` line → `C4 FAIL: CLAUDE.md:…`; M5 (C5) append `// cyril-ell0 mutation` to `crates/cyril-ui/src/state.rs` → `C5 FAIL: unexpected path crates/cyril-ui/src/state.rs`. Each mutation reddens exactly its own claim line; restoring returns all five to PASS.
**Complexity/production scale:** N/A — no new loop in production code; the fence is a dev-time script over the repository tree.
**Wall budget/phase:**     N/A — reason: one-off phase; no wall budget (the fence runs at the checkpoint; nothing runs at runtime).
**Module shape:**          Responsibility deleted: debounced streaming flush (never live). Interface delta: `cyril_ui::stream_buffer` removed from the crate surface. Owner after the slice: none (permanent non-goal). Protected parents: `lib.rs` −1 line (declaration only); `state.rs` 0; `app.rs` 0. Fence: `bash .cyril-ell0/oracles/shape_fence.sh` → `C1 PASS`, `C2 PASS`, `C3 PASS`, `C4 PASS`, `C5 PASS`, exit 0. Length review: N/A — no trigger (design.md).
**Files:**                 delete `crates/cyril-ui/src/stream_buffer.rs`; modify `crates/cyril-ui/src/lib.rs`, `CLAUDE.md`, `AGENTS.md`, and — added by the bounded repair after the isolated review — `.agents/summary/components.md`, `.agents/summary/review_notes.md`; create `.cyril-ell0/route.md`, `.cyril-ell0/design.md`, `.cyril-ell0/plan.md`, `.cyril-ell0/oracles/shape_fence.sh`.
**Estimate:**              ~1 h wall, dominated by the six gate builds on a machine shared with eleven other agents.
**Diff estimate:**         ≈ 672 changed lines (150 code deletions, 2 doc lines, ~520 artifact/oracle lines).
**PR increment:**          `cyril-ell0` (single increment).
**Commands and expected results:**
- `bash .cyril-ell0/oracles/shape_fence.sh` → five `C<n> PASS` lines, exit 0.
- Mutation cycle, one at a time: apply M<n> → fence prints exactly `C<n> FAIL: …` (other four PASS), exit 1 → restore → five PASS, exit 0.
- `grep -rn 'StreamBuffer\|stream_buffer' crates/` → exactly: `config.rs:153`, `nd4h_legacy_config_compat.rs:3`, `nd4h_legacy_config_compat.rs:40`, `nd4h_source_fences.rs:272`, `state.rs:2971`, `app.rs:875` (the after-receipt, B2).
- `grep -in 'stream buffer' CLAUDE.md AGENTS.md README.md` → no output (B4).
- `git diff --numstat $(git merge-base HEAD origin/main) -- crates/` → sums to 0 added / 150 deleted (C5 oracle); `git ls-files crates/cyril-ui/src/stream_buffer.rs` → no output (C1 oracle).
- `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core -p cyril --all-features -E 'binary(/nd4h/)' > <log> 2>&1 && echo GATE-OK-nd4h` → echo present (C3 oracle). Technical correction recorded at the checkpoint: the first draft used `-E 'test(nd4h)'`, which filters on test *names* and selected zero tests (nextest exit 4); the nd4h suites are the integration-test *binaries* `nd4h_legacy_config_compat` and `nd4h_source_fences`, so the filter is `binary(/nd4h/)`. Oracle meaning unchanged.
- `env -u CARGO_TARGET_DIR cargo fmt --all -- --check > /tmp/ell0-fmt.log 2>&1 && echo GATE-OK-fmt` → echo present.
- `env -u CARGO_TARGET_DIR cargo clippy --workspace --all-targets --all-features -- -D warnings > /tmp/ell0-clippy-all.log 2>&1 && echo GATE-OK-clippy-all` → echo present (C1/C2 oracle: compile clean).
- `env -u CARGO_TARGET_DIR cargo nextest run --workspace --all-features > /tmp/ell0-nextest-all.log 2>&1 && echo GATE-OK-nextest-all` → echo present.
- `env -u CARGO_TARGET_DIR cargo test --doc --workspace --all-features > /tmp/ell0-doctest.log 2>&1 && echo GATE-OK-doctest` → echo present.
- `env -u CARGO_TARGET_DIR cargo clippy --workspace --all-targets -- -D warnings > /tmp/ell0-clippy-default.log 2>&1 && echo GATE-OK-clippy-default` → echo present.
- `env -u CARGO_TARGET_DIR cargo nextest run --workspace > /tmp/ell0-nextest-default.log 2>&1 && echo GATE-OK-nextest-default` → echo present.

## Self-review (step 6)

1. Every design row (C1–C5) is assigned to Slice 1 exactly once; all IDs exist in the design table; every `PENDING` falsifier is discharged at the Slice 1 gate. ✔
2. All fourteen fields are present, conditional ones as `N/A — reason`. ✔
3. The fence (`shape_fence.sh`) is created in the slice that implements the claims; each row carries its named mutation; no fence-less claim. ✔
4. No new loop; no always-on phase; no latency obligation. ✔
5. Module shape field and growth ledger cover both touched modules and the three protected parents; Length review N/A is linked to the design record; no seam is crossed. ✔
6. Partition rule applied: 672 + 25 % ≈ 842 ≤ 4,000; single increment named; mergeable definition recorded. ✔
7. Tracker taxonomy: the three deferral-like phrases in design.md are permanent non-goals with rationale; no intended future work. ✔
8. Every fence assertion is a deterministic existence/text/byte check that proves its claim; no measurement-based claim. ✔
9. No slice is declared complete here; completion is checkpointed-build's. ✔

## Slice 1 gate record

Checked state: merge-base `f9bc81d8` + the working tree that became the slice commit (diff: `stream_buffer.rs` −149, `lib.rs` −1, `CLAUDE.md`/`AGENTS.md` one line each, `.cyril-ell0/**`). Environment: Linux x86_64 (cachyos 7.2.3), Rust 1.94.0 (`rust-toolchain.toml`), worktree-local `target/`, `cargo nextest`; machine shared with eleven concurrent agent builds (load average 23–37 during every run). Date: 2026-09-27. Logs: the shared scratchpad root was clobbered by another agent's gate script mid-run (run 1), so authoritative logs are the per-issue `scratchpad/ell0/logs/*.log` (run 2 onward); run-1 lines quoted below are the ones whose format and worktree paths identify them as this agent's.

Pre-implementation critique of the plan: no loop budget, fixture, or oracle field is implausible; the oracle for C4 (Owns-line → module mapping) and C5 (numstat magnitude) are independent of the fence's string/path checks; the module-shape field names every touched module and protected parent.

Impact analysis (step 1): the slice changes no function signature or semantics. Callers of the deleted type: none (`grep -rnw StreamBuffer crates/` at `f9bc81d8` → 13 hits, all inside the deleted file; `grep -rn 'stream_buffer::' crates/` → 0). Callers of `mod stream_buffer`: the declaration only. Helper search (step 2): no utility code written. TDD (step 3): no new function — the deletion's RED/GREEN is the fence: RED on the unchanged tree (C1, C2, C4 FAIL; C3, C5 PASS), GREEN after the slice (C1–C5 PASS). Symmetry audit (step 4): no parallel path added or predicate changed — N/A.

| # | Gate item | State | Evidence |
|---|---|---|---|
| 1 | Affected unit tests | PASS | The slice deletes the 9 unit tests whose only subject was the deleted type; no surviving test covers changed executable behavior (none changes). Full suites nonetheless run: `cargo nextest run --workspace --all-features` → 2048 run, 2048 passed, 13 skipped (`ell0/logs/nextest-all.log`); `cargo nextest run --workspace` run 3 → all passed (`ell0/logs/nextest-default-run3.log`; runs 1–2 discussed under "CI gate runs" below). |
| 2 | Falsifiers | PASS | C1–C5 discharged on the changed tree: fence GREEN (`C1 PASS … C5 PASS`, exit 0) in the post-slice and FINAL runs of `cyril-ell0-checkpoint.sh` and again after the C3 message correction. C2's pre-approval `PASS` (pre-deletion form at `f9bc81d8`) was rerun on the changed tree: `grep -rn 'StreamBuffer\|stream_buffer' crates/` → exactly the six B2 lines (`config.rs:153`, `nd4h_legacy_config_compat.rs:3,40`, `state.rs:2971`, `app.rs:875`, `nd4h_source_fences.rs:272`); zero identifiers, zero paths. |
| 3 | Stress fixture | N/A — plan records no fixture (the slice implements no logic) | — |
| 4 | Implementation vs independent oracle | PASS | C1: `git ls-files crates/cyril-ui/src/stream_buffer.rs` → empty; workspace compiles (`clippy-all`, `nextest-all` exit 0). C2: rustc name resolution clean (same compile). C3: `cargo nextest run -p cyril-core -p cyril --all-features -E 'binary(/nd4h/)'` → the `nd4h_legacy_config_compat` and `nd4h_source_fences` binaries run: 9 tests across 2 binaries, 9 passed (`ell0/logs/nd4h.log`). C4: every component on the cyril-ui `Owns:` line maps to a module in `lib.rs` (`state`, `traits`, `widgets`, `render`+`text`, `highlight`, `file_completer`); no stale name. C5: `git diff --numstat f9bc81d8 -- crates/` → `0 1 lib.rs`, `0 149 stream_buffer.rs` = 0 added / 150 deleted, item-for-item as predicted. |
| 5 | Approved module shape | PASS | Fence `bash .cyril-ell0/oracles/shape_fence.sh` → C1–C5 PASS, exit 0. Ledger holds: `stream_buffer.rs` deleted; `lib.rs` lost exactly one declaration (22 lines); protected parents `state.rs`, `app.rs` byte-identical to the merge-base (C5); `docs/` untouched (`git diff --stat f9bc81d8 -- docs/` empty). Dependency direction unchanged. Length review: N/A — no trigger (design.md). |
| 6 | Production-scale budget | N/A — plan records no loop or phase budget class | — |
| 7 | Regression fence | PASS | `shape_fence.sh` green on the changed tree: post-slice run, FINAL GREEN run, and the rerun after the C3 message correction — all `C1 PASS`…`C5 PASS`, exit 0. |
| 8 | Named mutation red | PASS | Each mutation reddened exactly its own claim, exit 1, others PASS: M1 → `C1 FAIL: crates/cyril-ui/src/stream_buffer.rs exists`; M2 → `C2 FAIL: crates/cyril-ui/src/lib.rs:23:pub use crate::stream_buffer::StreamBuffer;`; M3 → `C3 FAIL: crates/cyril-core/tests/nd4h_legacy_config_compat.rs differs from merge-base f9bc81d8…` (re-proved after the message-spacing fix to the fence, per Evidence validity); M4 → `C4 FAIL: CLAUDE.md:155:…file completer, stream buffer`; M5 → `C5 FAIL: unexpected path crates/cyril-ui/src/state.rs`. |
| 9 | Fence restored green | PASS | Every restore verified (`test ! -e` for M1; `cmp` against the pre-mutation copy for M2–M5, all `restored: yes`); FINAL GREEN exit 0; `git status --porcelain` afterwards shows only the intended four tracked changes plus `.cyril-ell0/`. |
| 10 | Parity and reuse | PASS | Symbols added: none in production. New non-production artifact `shape_fence.sh`: searched for a shared shape-gate entry point (`ls scripts/` = `session-worktree.sh`, `tests`; `grep -rIl -i 'max.lines\|line.count\|length.gate\|wc -l' scripts/ .github/workflows/` = none) and for the issue-local convention (`ls -d .cyril-*/oracles` → per-issue standalone oracles, e.g. `.cyril-fkke`, `.cyril-g0dg`, `.cyril-gl5s`, `.cyril-jlxx`, `.cyril-k3lz`) → a standalone issue-local oracle matches the established pattern; nothing to widen or wrap. Divergence kept beside the code: the existing oracles are Python (`module_shape.py`, `shape.py`, `mutations.py`) while this fence is bash, because its five checks are pure `git`/`grep`/`test` invocations with no data manifest — a Python wrapper would add nothing but a second language to read. No construction written twice; no parallel path; no predicate tightened or relaxed. |
| 11 | Preserved enforcement | PASS | Deleted: the 9 unit tests in `stream_buffer.rs`, whose detection set covered only the deleted type (no surviving code) — authorized by design.md's Approval (option (a)). No gate, fence, validator, oracle, or policy file is repointed, relaxed, or orphaned: `.github/workflows/ci.yml` untouched; the nd4h suites byte-identical (C3) and green (item 4). |

### CI gate runs (repository quality checks, mirrored from `.github/workflows/ci.yml`)

| Command | Run 1 (05:33Z, clobbered root logs) | Run 2 (05:39Z, `ell0/logs`) | Run 3 (05:42Z) |
|---|---|---|---|
| `cargo fmt --all -- --check` | line lost to clobber | **PASS** | — |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | line lost to clobber | **PASS** | — |
| `cargo nextest run --workspace --all-features` | PASS (126 s) | **PASS** (2048/2048) | — |
| `cargo nextest run -p cyril-core -p cyril --all-features -E 'binary(/nd4h/)'` | FAIL exit 4 with the draft filter `test(nd4h)` (0 tests selected — technical correction above) | **PASS** | — |
| `cargo test --doc --workspace --all-features` | PASS | **PASS** | — |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS | **PASS** | — |
| `cargo nextest run --workspace` | FAIL exit 100 | FAIL exit 100 | **PASS** |

Default-features failure, runs 1 and 2 — one test, `cyril-workbench::reviewer runtime_drop_retains_evidence_until_core_completion` (`crates/cyril-workbench/tests/reviewer.rs:712`): `assertion left == right failed, left: Some("R"), right: Some("Z")` — the test reads `/proc/<pid>/stat` of a spawned child and expects zombie state; it observed a still-running process. Classification: **pre-existing load-sensitive race unrelated to this change**, evidence: (a) `cargo tree -p cyril-workbench -e normal,dev,build` contains zero `cyril-ui` edges (`ell0/logs/workbench-tree.log`) — the deleted crate module is unreachable from that test binary; (b) the same test passed in both `--all-features` runs and in 5/5 isolated runs (`ell0/logs/flaky-{1..5}.log`); (c) the identical failure appears in other agents' shared-scratchpad logs on other branches (`characterize/single-{2,3,9}.log`, `gate/nextest-ws-default.log`); (d) machine load average 23–37 (twelve concurrent agent builds). The tracker was searched read-only (`.rivets/issues.jsonl`, 335 issues) and no issue covers this test; filing is reported to the batch orchestrator (this session is read-only on the tracker). The gate state above is the run-3 result on the identical tree; runs 1–2 are recorded, not discarded — CI on the PR carries the same risk until the race is fixed.

Stale-reference sweep (step 7): the only prose this slice falsified was the cyril-ui `Owns:` line (fixed in `CLAUDE.md`/`AGENTS.md`). `app.rs:874` `// Flush stream buffer on tick` describes the pre-existing `UiState::flush_stream_buffer` stub, not the deleted module — unchanged, reported. No forward references, no tracker phrases in code.

Drift check (step 9): `origin/main` fetched; merge-base `f9bc81d8` = branch base; no upstream movement during the slice. Size tripwire (step 10): actual diff ≈ 150 code deletions + 2 doc lines + artifacts, far below 4,000; single increment as planned.

### Bounded repair after the isolated review (same slice, before commit)

Finding (design.md, "Final design-conformance review", item iv): `.agents/summary/components.md:186–188` still catalogued `stream_buffer.rs` as a live `cyril-ui` module and `.agents/summary/review_notes.md:57` cited the 149-LOC file. Governing obligation: checkpointed-build step 7, falsified prose ("Rewrite each to describe current behavior factually, wherever it sits"); the requester's approved scope already includes "remove any that describe it as a live component". Owning claims: C4 (file set extended), C5 (approved path set extended) — technical corrections recorded in design.md; approved behavior, ownership, interfaces, and risk unchanged.

Repair: removed the catalog entry (4 lines) and the review-notes row (1 line); extended `shape_fence.sh` C4 to grep `StreamBuffer\|stream_buffer` in both files and C5's allowlist to include them. Changed fence ⇒ renewed proof (`ell0/cyril-ell0-checkpoint2.sh`, 2026-09-27):

| Check | Result |
|---|---|
| Fence on the repaired tree | `C1 PASS` … `C5 PASS`, exit 0 |
| M4 (`CLAUDE.md`) | `C4 FAIL: CLAUDE.md:155:…file completer, stream buffer` only, exit 1; restored (`cmp`) → green |
| M4b (`.agents/summary/components.md`, append the heading) | `C4 FAIL: .agents/summary/components.md:202:### \`stream_buffer.rs\` — StreamBuffer` only, exit 1; restored (`cmp`) → green |
| M5 (`crates/cyril-ui/src/state.rs`) | `C5 FAIL: unexpected path crates/cyril-ui/src/state.rs` only, exit 1; restored (`cmp`) → green |
| FINAL GREEN | exit 0 |
| Receipt `grep -rn 'StreamBuffer\|stream_buffer\|stream buffer' .agents/` | no output |
| Receipt `git diff --stat f9bc81d8` | 6 files, +2/−157: `components.md` −4, `review_notes.md` −1, `AGENTS.md` ±1, `CLAUDE.md` ±1, `lib.rs` −1, `stream_buffer.rs` −149 |
| `cargo fmt --all -- --check` | PASS (`ell0/logs/fmt-after-repair.log`) |

Evidence validity for the retained gate results: the repair touched only two Markdown files outside every cargo target and the fence script; no Rust source, manifest, or lockfile changed, so the clippy/nextest/doctest/nd4h results above remain valid for the committed tree (the C1/C2/C3 fence checks, `git ls-files`, and the numstat oracle were re-run by the fence and are unchanged). Gate items 1–11 reconciled: 4 (oracle for C4 now also maps the catalog entries to modules — none stale), 5, 7, 8, 9 refreshed by the table above; the rest retained.
