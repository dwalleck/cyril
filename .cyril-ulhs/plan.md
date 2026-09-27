# Plan: cyril-ulhs

Design verified 2026-09-27: `design.md`'s Falsification table has every cell filled (C5/C6 fence cells carry `N/A — approved risk` with matching Approval entries), the cheapest runnable falsifier (C1 oracle half) is `PASS`, no row is `FAIL`, every `PENDING` names `checkpointed-build, slice 1 gate`, Module shape carries the T2-backed `N/A`, and the Approval section holds the requester's verbatim dated words and the two risk acceptances.

Module growth ledger: `N/A — route/design record no module-shape change`.

## Partition arithmetic

| Item | Changed lines |
|---|---:|
| `clippy.toml` (new) | 3 |
| `crates/cyril-ui/tests/picker_active_marker.rs` (−6 helper, 3 call sites rewritten) | 12 |
| 17 attribute deletions across 15 files (12 single-line outer + 2×4-line multi-line + 3 inner attrs with trailing blank) | 26 |
| `.cyril-ulhs/oracles/footprint.sh`, `.cyril-ulhs/receipts.sh` (checkpoint instruments) | ~150 |
| `.cyril-ulhs/{design,plan,evidence,route}.md` + receipts | ~450 |
| **Sum** | **~640** |
| Churn margin 25% (artifact prose and receipt logs tend to grow; code lines cannot) | ~160 |
| **Total** | **~800** ≤ 4,000 → single PR increment |

## Slice 1: land the test-scoped `expect_used` policy and the fence it makes real

**Claim IDs:** C1, C3, C4, C5, C6 (all design rows; none can land apart — the config alone turns the 17 attributes red, the attribute deletions alone turn their modules red, the picker relocation alone is red without the config)
**Expected behavior:** `cargo clippy --workspace --all-targets --all-features -- -D warnings` exits 0 on a tree that has `clippy.toml`, three inline `.expect()` calls in `picker_active_marker.rs`'s `#[test]` fns, and zero `#[expect(clippy::expect_used)]` attributes; M1/M2/M3 each turn it red as named; the three picker tests and the whole suite pass under nextest.
**Oracle:** the evidence oracle (`probe.oracle/`, retained P2/P3/P4 comparisons) for C1/C3/C5; hand census of the 8 `unwrap_used` expectations for C4; `footprint.sh` + a hand read of `git diff --stat` for C6.
**Stress fixture:** `N/A — reason: configuration and attribute-only deletions, no logic; the worst admitted shape is the whole workspace under --all-targets --all-features, which the gate itself runs`
**Regression fence:** C1 — the three `#[test]` fns in `crates/cyril-ui/tests/picker_active_marker.rs` calling `ui.picker().expect("show_picker did not open a picker")` (created in this slice); C3 — CI clippy leg's `unfulfilled_lint_expectations` under `-D warnings` (existing); C4 — the 8 `#[expect(clippy::unwrap_used)]` attributes (existing); C5 — `N/A — approved risk: requester specified "Non-test discipline unchanged (receipt, not a commit)"`; C6 — `N/A — approved risk: a one-time diff-scope obligation of this PR (coordinator constraints 1–3); nothing permanent to fence`
**Named mutation:** M1 (`rm clippy.toml` → `expect_used` at the three picker lines), M2 (re-insert `#[expect(clippy::expect_used)]` at `cyril-memory/src/paths.rs` above `mod tests` → `unfulfilled_lint_expectations` there), M3 (append `allow-unwrap-in-tests = true` → `unfulfilled_lint_expectations` at exactly the 8 `unwrap_used` sites); C5, C6 — `N/A — approved risk: no fence to mutate`
**Complexity/production scale:** `N/A — reason: no new loop; lint-time configuration only`
**Wall budget/phase:** `N/A — reason: no runtime phase is introduced; the change is observed only by the lint gate`
**Module shape:** `N/A — route/design record no module-shape change`
**Files:**
- create `clippy.toml`
- modify `crates/cyril-ui/tests/picker_active_marker.rs`
- attribute-line-only deletions: `crates/cyril-core/src/commands/mod.rs` (515, 1659), `crates/cyril-core/src/voice.rs` (75–78), `crates/cyril-core/src/protocol/source_observer.rs` (458 + blank), `crates/cyril-core/src/types/memory.rs` (734), `crates/cyril-memory/src/client.rs` (278), `crates/cyril-memory/src/lesson.rs` (293), `crates/cyril-memory/src/paths.rs` (213), `crates/cyril-memory/src/permissions.rs` (155), `crates/cyril-memory/src/project.rs` (240), `crates/cyril-memory/src/runtime.rs` (421), `crates/cyril-memory/src/source_turn.rs` (1207 + blank), `crates/cyril-memory/src/wire.rs` (1357), `crates/cyril-voice/src/lib.rs` (138–141), `crates/cyril/src/capture_forwarder.rs` (204 + blank), `crates/cyril/src/memory_runtime.rs` (800, 928)
- create `.cyril-ulhs/oracles/footprint.sh`, `.cyril-ulhs/receipts.sh`; update `.cyril-ulhs/design.md` statuses and `route.md` terminal criterion
**Estimate:** ~1.5 h wall, dominated by five workspace clippy runs and the nextest legs on a shared machine
**Diff estimate:** ~45 code/config lines + ~600 artifact lines (see arithmetic)
**PR increment:** single — `chore(lint): allow expect in tests via clippy.toml (cyril-ulhs)`; mergeable on its own against the repository's default branch (`origin/main`, discovered via `git merge-base HEAD origin/main`); verified entirely by the CI-mirror gate below with nothing after it
**Commands and expected results:**
- `env -u CARGO_TARGET_DIR cargo clippy --workspace --all-targets --all-features --keep-going -- -D warnings` → exit 0; `probe-diag.py` over the JSON stream lists zero `clippy::expect_used`, zero `clippy::unwrap_used`, zero `unfulfilled_lint_expectations`; the compiled-units list includes `test picker_active_marker` (C1, C3, C4 green halves)
- `bash .cyril-ulhs/receipts.sh` → M1 red with `expect_used` at the three picker test lines; M2 red with `unfulfilled_lint_expectations` at `cyril-memory/src/paths.rs:213`; M3 red with `unfulfilled_lint_expectations` at exactly the 8 hand-listed `unwrap_used` sites and nowhere else; P2 re-run red with `expect_used` at the appended `cyril-core/src/lib.rs` line; after each, the tree restores to `git status --porcelain` clean for the touched paths and a final green re-run exits 0 (C1/C3/C4 mutations, C5 receipt)
- `bash .cyril-ulhs/oracles/footprint.sh` → every line `C6 PASS`, exit 0; `git diff origin/main --stat` read by hand agrees with the approved file list (C6, C5a)
- CI-mirror gate, each `cmd > <scratch>/cyril-ulhs-<name>.log 2>&1 && echo GATE-OK-<name>` with the echo present: `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo nextest run --workspace --all-features`; `cargo test --doc --workspace --all-features`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo nextest run --workspace`; `cargo clippy -p cyril-core --no-default-features --all-targets -- -D warnings`; plus the three clippy legs repeated with `--keep-going` as extra receipts → all seven `GATE-OK` echoes present, the three picker tests listed as passed in the nextest output (affected unit tests)

## Self-review

1. Every design row (C1, C3, C4, C5, C6) is assigned to slice 1 exactly once; every `PENDING` falsifier is discharged at its gate. ✔
2. All fourteen fields present; conditional fields carry `N/A — reason` / `N/A — approved risk` verbatim from the design. ✔
3. C1's fence is created in this slice; C3/C4 fences pre-exist and are named; C5/C6 copy the design's approved-risk values with `Named mutation: N/A — approved risk: no fence to mutate`. ✔
4. No new loop, no runtime phase. ✔
5. Module shape `N/A` per route/design; no seam crossed. ✔
6. Partition rule applied (≈800 ≤ 4,000, 25 % margin documented); single increment with a mergeable definition. ✔
7. Tracker taxonomy: permanent non-goals carry rationale in `design.md`; intended future work is recorded as report-only with the explicit note that the tracker is read-only for this session (deviation surfaced to the orchestrator in the hand-back). ✔
8. Fences assert the observable their mutations change (diagnostic presence at named file:line; exit code). ✔
9. No slice is declared complete here. ✔

## Slice 1 checkpoint (checkpointed-build record)

Revision judged: the code tree committed as `e37000e0` on `chore/cyril-ulhs-clippy-expect-in-tests` (merge-base `f9bc81d8`). The mutation/receipt runs (`receipts/{M1,M2,M3,C5,GREEN}.*`, `receipts-run.log`) and the first gate run were executed on that exact code tree before it was committed — the first `gate-summary.txt` was therefore labeled with the then-HEAD `37452cab` (the artifacts-only commit); review N1/N2 re-ran `gate.sh` on `e37000e0` itself and the committed `gate-summary.txt` now carries the code SHA with a code-path-clean proof (`review-decisions.md` R1). GitHub CI on `e37000e0`: run 36298681186, success. **Line numbers and counts in the receipts are as of `e37000e0`** (review N4): sibling PR #142 deletes `crates/cyril-ui/src/stream_buffer.rs` (one of the 8 `#[expect(clippy::unwrap_used)]` sites → M3 would list 7 after it merges) and #141 edits `commands/mod.rs` above line 250 (later line numbers shift); `bash .cyril-ulhs/receipts.sh` regenerates every receipt on any revision. Plan critique before implementing: no loop budgets, fixtures, or oracles coupled to the implementation exist in this plan; the only critique raised was that the C6 oracle must read the diff, not the working tree, for the file-set check — `footprint.sh` uses `git diff --name-only <base>` (done).

**Impact analysis (step 1):** the only changed signature is the deletion of `render_open_picker(&UiState) -> String` in `crates/cyril-ui/tests/picker_active_marker.rs`. Callers (`grep -n render_open_picker` on main): lines 103, 140, 158 — the three `#[test]` fns; all three rewritten in this slice; no other file references it (`grep -rn render_open_picker crates/` → only this file). The 17 deleted attributes have no callers by construction.
**Helper search (step 2):** the inlined call reuses the existing `render_text(&PickerState)` helper in the same file; no new utility, no new dependency (`Cargo.toml` untouched, C5a).
**Symmetry audit (step 4):** the three inline sites are byte-identical and parallel each other; the failure path (`None` → panic with the same message PR #75 used) is unchanged in wording and severity; no acceptance predicate tightened or relaxed; no error variant, logging, or fallback exists in a test helper of this shape.

| # | Gate item | State | Evidence |
|---|---|---|---|
| 1 | Affected unit tests | PASS (`cargo nextest run --workspace --all-features`: 2057 run, 2057 passed, 13 skipped; `picker_marks_active_row_exactly_once_for_model`, `…_for_effort`, `…_from_wire_current_fallback` each PASS; default-features leg 2055/2055; doctests ok) | the three `picker_active_marker` tests under `cargo nextest run --workspace --all-features` (gate leg `nextest-all`) |
| 2 | Falsifiers (C1, C3, C4, C5, C6) | PASS (GREEN exit 0, 0 diagnostics, 29 workspace units incl. `test picker_active_marker`; M1/M2/M3/C5 red as named; footprint 24/24; Cargo.toml diff 0 lines) | C6: `oracles/footprint.sh` 24/24 PASS on the final diff (run 2026-09-27, base `f9bc81d8`); C5a: `git diff origin/main -- Cargo.toml` → 0 lines; C1/C3/C4 green halves + C5b: `receipts/GREEN.diag`, `receipts/C5.diag` |
| 3 | Stress fixture | `N/A — reason: plan records no fixture (configuration and attribute-only deletions; the gate itself is the whole-workspace shape)` | plan.md slice 1 |
| 4 | Implementation vs oracle | PASS (M3's 8 `unfulfilled_lint_expectations` file:line set equals the hand census item by item — `commands/mod.rs:514`, `commands/subagent.rs:202`, `kiro_agent_config.rs:288`, `types/config.rs:139`, `types/hook.rs:114`, `cyril-memory/config.rs:318`, `cyril-memory/store.rs:2082`, `cyril-ui/stream_buffer.rs:69`; M1's picker sites 96/133/151 = the three `grep -n 'ui.picker().expect'` lines; C5 `lib.rs:23` fires with the config exactly as oracle `src/bin/nontest.rs:7`; `footprint.sh` file set = hand-read `git diff --stat` 16 files +3 −36) | evidence oracle retained (P2/P3/P4, `probe-out/oracle-*-keepgoing.diag`) vs `receipts/{GREEN,M1,M2,M3,C5}.diag`; hand census of 8 `unwrap_used` sites vs `receipts/M3.diag`; hand read of `git diff --stat` vs `footprint.sh` |
| 5 | Module shape | `N/A — route and design record no module-shape change` | route.md T2, design.md § Module shape |
| 6 | Production-scale budget | `N/A — reason: plan records no loop or runtime phase (lint-time configuration only)` | plan.md slice 1 |
| 7 | Regression fence green | PASS (`receipts/GREEN.exit` = 0; 0 `expect_used` / `unwrap_used` / `unfulfilled_lint_expectations`; C5/C6 carry `N/A — approved risk` per design.md § Approval) | `receipts/GREEN.exit` = 0 with the picker `.expect()`s and zero `#[expect(clippy::expect_used)]`; C5/C6: `N/A — approved risk` (design.md § Approval) |
| 8 | Named mutation red | PASS (M1 exit 101: `expect_used` at `picker_active_marker.rs:96/133/151` plus 247 collateral sites in the formerly `#[expect]`-guarded modules — the coverage the config now owns; M2 exit 101: exactly one diagnostic, `unfulfilled_lint_expectations` at `cyril-memory/src/paths.rs:213`; M3 exit 101: exactly 8, the `unwrap_used` sites; C5/C6: `N/A — approved risk: no fence to mutate`) | M1 `receipts/M1.diag` (picker lines 96/133/151), M2 `receipts/M2.diag` (`paths.rs:213`), M3 `receipts/M3.diag` (the 8 `unwrap_used` sites); C5/C6: `N/A — approved risk: no fence to mutate` |
| 9 | Fence restored green | PASS (GREEN ran after all four mutations: exit 0; `RESTORED-OK` ×3 by `cmp` against pre-mutation snapshots; the `cp: cannot stat` lines after `RECEIPTS-DONE` in `receipts-run.log` are the EXIT trap re-firing on already-deleted snapshots — after verification, no effect; fixed in `receipts.sh` with `trap - EXIT`) | `receipts/GREEN.*` after all mutations; `RESTORED-OK` ×3 in `receipts/receipts-run.log` |
| 10 | Parity and reuse | PASS | symbols added/changed: none in production; test change reuses `render_text`; the inline call is written three times because each `#[test]` owns its setup (the helper it replaces was the workaround the issue names; folding three tests into a shared helper would recreate the non-`#[test]` shape the config cannot cover — design.md § Non-goals); search: `grep -rn render_open_picker crates/` and `grep -rn "fn render_text" crates/cyril-ui/tests/`; no new dep (manifest untouched) |
| 11 | Preserved enforcement | PASS | artifacts touched: (a) 17 `#[expect(clippy::expect_used)]` attributes deleted — each was a fulfilled promise that `expect_used` fires inside its `#[cfg(test)]` module; the config suppresses that lint there by approved design (C3, Approval), so the promise is unfulfillable and rustc turns it into an error — the enforcement it carried (test-scoped `expect` policy) is now owned by `clippy.toml`, strictly stronger in scope (every `#[cfg(test)]` module and `#[test]` fn, not only the 17 attributed ones) and unchanged in non-test detection power (C5 receipt); (b) `render_open_picker` let-else deleted — its detection (panic on a missing picker, same message) is carried by the inline `.expect()` in each test; (c) no gate, validator, oracle, or policy file repointed or relaxed; the 8 `#[expect(clippy::unwrap_used)]` attributes are preserved and M3 proves they still enforce |
