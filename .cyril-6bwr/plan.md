# Plan: cyril-6bwr — remove the producer-less `CommandResultKind::ShowPicker`

Inputs: `route.md` (Structural, base `f9bc81d8`), `design.md` (approved 2026-09-27, five claims C1–C5, every PENDING row names `checkpointed-build, slice 1 checkpoint`, cheapest falsifier — the C1/C5 positive controls and the C4 sensitivity control — `PASS`, risk acceptances `None`). `spec.md`, `evidence.md`: N/A per `route.md`.

## Step 1 — design verification

Falsification table: 5 rows, every cell filled; no `FAIL`; no `N/A — approved risk` anywhere; every `PENDING` names its discharge owner and step. Module shape branch: T2 interface narrowing → inventory (2 rows), seam tests (4, adapter and three-alternatives `N/A — existing seam`), module ledger (3 rows), one protected parent (`app.rs`) with a checkable exit condition (exactly one 3-line deletion hunk), shape claim C1 with the oracle script, Length review `N/A — no gate`. Every oracle, mutation, fence, and placement rule is mechanical (file, line, edit, expected red text). Verified: the design satisfies falsifiable-design's Output requirements.

## Module growth ledger

| Module | Baseline production lines | Projected final lines | Responsibility change | Interface change | Protected-parent rule |
|---|---:|---:|---|---|---|
| `crates/cyril-core/src/commands/mod.rs` | 1938 | 1922–1927 (−11 to −16: variant block 4, constructor 5, import 1, doc-line rewording ±3; margin ±2) | delete dead intent variant | `CommandResultKind` −`ShowPicker`; `CommandResult` −`show_picker()`; `CommandOption` no longer imported | N/A |
| `crates/cyril/src/app.rs` | 7619 | 7616 (−3, exact) | delete dead dispatch arm | none (binary crate) | protected parent: exactly one hunk, a pure 3-line deletion at the `ShowPicker` arm; `app.rs:1453-1465` untouched |
| `crates/cyril-ui/src/state.rs` | unchanged | unchanged | none | none | must not change (`git diff --stat` empty) |

No projection approaches any threshold (none exists); no Length review is triggered.

## Partition arithmetic

| Component | Changed lines (est.) |
|---|---:|
| `commands/mod.rs` (−13 removed, +3 reworded) | 16 |
| `app.rs` (−3) | 3 |
| `docs/omp-review-command-analysis.md` (1 line rewritten) | 2 |
| `.cyril-6bwr/oracles/no-show-picker-variant.sh` (new) | 70 |
| `.cyril-6bwr/{route,design,plan}.md` (new, prose) | ~330 |
| **Sum** | **~421** |
| Churn margin 30% (artifact prose grows while checkpoint receipts are appended; code churn is nil) | ~126 |
| **Total** | **~547** |

547 ≤ 4,000 → **one PR increment** holding the single slice.

**Increment 1** — slices: 1. Mergeable definition: the branch `chore/cyril-6bwr-remove-show-picker-variant` verifies alone against the repository's default branch (`main`, discovered via `git rev-parse --abbrev-ref origin/HEAD` / `gh repo view --json defaultBranchRef`) with the full CI-mirroring gate set below; nothing after it exists.

## Slice 1: remove the dead `ShowPicker` variant, its constructor, its dispatch arm, and every citation, fenced by the census oracle

**Claim IDs:** C1, C2, C3, C4, C5 (all five; C1/C4/C5 share one fence script and cannot land apart without committing a red fence; C2 is the compile of the same edit; C3 is the untouched-path proof of the same diff).

**Expected behavior:** On the post-change tree: `sh .cyril-6bwr/oracles/no-show-picker-variant.sh` prints `cyril-6bwr oracle: GREEN (C1 C4 C5)` and exits 0; every gate command below passes with its `GATE-OK-*` echo; `git diff f9bc81d8 --stat -- crates/cyril-ui/src/state.rs` prints nothing; `git diff f9bc81d8 -U0 -- crates/cyril/src/app.rs` shows exactly one hunk (`-3` lines at the former `ShowPicker` arm); the named `picker_*` tests appear as PASS in the nextest log; the three reworded doc comments cite `ToggleVoice` / `Steer` only.

**Oracle:** per design rows — C1/C4/C5: POSIX grep census (oracle script), independent of rustc; C2: rustc/clippy under `-D warnings`, independent of the census; C3: `git diff` census plus the existing `TestBackend`-rendering picker tests (a different mechanism from the `is_current` state mutation).

**Stress fixture:** N/A — reason: the slice implements no logic and handles no input; it deletes dead code, rewords three comments, corrects one doc line, and adds a census script whose only loop is over ≤5 extracted names.

**Regression fence:** `.cyril-6bwr/oracles/no-show-picker-variant.sh` — created in this slice (written during design, committed here) — for C1, C4, C5; the existing CI clippy gate (`cargo clippy --workspace --all-targets --all-features -- -D warnings`) for C2; the existing `crates/cyril-ui/tests/picker_active_marker.rs`, `picker_viewport.rs`, and `state.rs` show_picker tests for C3.

**Named mutation:** (C1) re-add `    ShowPicker { title: String, options: Vec<CommandOption> },` to `CommandResultKind` in `crates/cyril-core/src/commands/mod.rs` → oracle prints `C1 FAIL: crates/cyril-core/src/commands/mod.rs:<n>:    ShowPicker {`, exit 1. (C2, m1) leave `use crate::types::CommandOption;` at `mod.rs:10` after its uses are gone → clippy `error: unused import: `crate::types::CommandOption``. (C3) in `crates/cyril-ui/src/state.rs:2380` change `opt.is_current = i == idx;` → `opt.is_current = false;` → `picker_marks_active_row_exactly_once_for_model` red. (C4) change the `ShowThemePicker` doc citation `` `ToggleVoice` `` → `` `ToggleVoyce` `` → oracle prints `C4 FAIL: doc comment in crates/cyril-core/src/commands/mod.rs cites non-variant `ToggleVoyce``, exit 1. (C5) revert `docs/omp-review-command-analysis.md:96` to base → oracle prints `C5 FAIL: docs/omp-review-command-analysis.md:96:…`, exit 1. Every mutation is applied and restored with the Edit tool after the slice commit; restoration is verified by the fence returning green and `git diff --stat` being empty.

**Complexity/production scale:** N/A — reason: no new production loop; the only loop is the oracle script's iteration over ≤5 doc-cited names, non-production.

**Wall budget/phase:** N/A — reason: no runtime phase is introduced or changed; the removed arm never executed.

**Module shape:** delete the dead intent variant and constructor from `crates/cyril-core/src/commands/mod.rs` (interface delta: `CommandResultKind` −1 variant, `CommandResult` −1 constructor, `CommandOption` import dropped); owner after the slice: unchanged (`commands/mod.rs` owns typed intents; `app.rs` owns projection; `state.rs` owns picker opening). Protected parent `crates/cyril/src/app.rs`: expected production delta −3 lines in exactly one hunk; the `CommandOptionsReceived` handler (`app.rs:1453-1465`) untouched. Shape-fence command: `sh .cyril-6bwr/oracles/no-show-picker-variant.sh` → `cyril-6bwr oracle: GREEN (C1 C4 C5)`, exit 0. Length review: N/A — no gate exists (`route.md` T2) and every module shrinks.

**Files:**
- modify `crates/cyril-core/src/commands/mod.rs` (lines 10, 177-180, 184, 192, 201, 239-243)
- modify `crates/cyril/src/app.rs` (lines 2043-2045)
- modify `docs/omp-review-command-analysis.md` (line 96)
- add `.cyril-6bwr/oracles/no-show-picker-variant.sh`, `.cyril-6bwr/route.md`, `.cyril-6bwr/design.md`, `.cyril-6bwr/plan.md` (and the checkpoint record appended by checkpointed-build)

**Estimate:** 45–90 min, dominated by the CI-mirroring cargo gates on the shared build host.

**Diff estimate:** ~421 changed lines (code 21, docs 2, oracle 70, artifacts ~330).

**PR increment:** Increment 1.

**Commands and expected results:** (every cargo command prefixed `env -u CARGO_TARGET_DIR`; each redirected to a log and followed by `&& echo GATE-OK-<name>` — a missing echo is a FAIL)
- `sh .cyril-6bwr/oracles/no-show-picker-variant.sh` → `cyril-6bwr oracle: GREEN (C1 C4 C5)`, exit 0 (C1, C4, C5 absence halves discharged; positive controls already logged in `design.md`).
- `sh .cyril-6bwr/oracles/no-show-picker-variant.sh f9bc81d8` → RED with the six C1 sites and one C5 site, exit 1 (re-run of the positive control against the same base, proving the committed script still sees the thing).
- `git diff f9bc81d8 --stat -- crates/cyril-ui/src/state.rs` → empty; `git diff f9bc81d8 -U0 -- crates/cyril/src/app.rs` → exactly one `@@` hunk, three `-` lines, zero `+` lines, none in 1450–1465 (C3a).
- `env -u CARGO_TARGET_DIR cargo fmt --all -- --check` → GATE-OK-fmt.
- `env -u CARGO_TARGET_DIR cargo clippy --workspace --all-targets --all-features -- -D warnings` → GATE-OK-clippy-all (C2).
- `env -u CARGO_TARGET_DIR cargo nextest run --workspace --all-features` → GATE-OK-nextest-all, and the log lists `picker_marks_active_row_exactly_once_for_model`, `picker_marks_active_row_exactly_once_for_effort`, `picker_marks_active_row_from_wire_current_fallback` and the `picker_viewport` tests as PASS (C3b).
- `env -u CARGO_TARGET_DIR cargo test --doc --workspace --all-features` → GATE-OK-doctest.
- `env -u CARGO_TARGET_DIR cargo clippy --workspace --all-targets -- -D warnings` → GATE-OK-clippy-default.
- `env -u CARGO_TARGET_DIR cargo nextest run --workspace` → GATE-OK-nextest-default.
- `env -u CARGO_TARGET_DIR cargo clippy -p cyril-core --no-default-features --all-targets -- -D warnings` → GATE-OK-clippy-core-nodefault; `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --no-default-features` → GATE-OK-nextest-core-nodefault.
- Mutation C1 applied → oracle exit 1 with the `C1 FAIL: …:    ShowPicker {` line; restored → GREEN.
- Mutation C4 applied → oracle exit 1 with the `C4 FAIL: … `ToggleVoyce`` line; restored → GREEN.
- Mutation C5 applied → oracle exit 1 with the `C5 FAIL: docs/omp-review-command-analysis.md:96` line; restored → GREEN.
- Mutation C2 (m1) applied → `env -u CARGO_TARGET_DIR cargo clippy -p cyril-core --all-targets -- -D warnings` red with `unused import: `crate::types::CommandOption``; restored → green.
- Mutation C3 applied → `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-ui --test picker_active_marker` red on `picker_marks_active_row_exactly_once_for_model`; restored → green and `git diff --stat` empty.

## Tracker taxonomy

Deferral phrases in this plan: none. The design's four permanent non-goals carry their rationale in `design.md`; the pre-existing App-level wiring-test gap is reported to the orchestrator (tracker owner) rather than filed here, because this agent operates the tracker read-only.

## Self-review

1. Every design row (C1–C5) is assigned to exactly one slice (slice 1); every claim ID exists in the design table; every PENDING falsifier is discharged at slice 1's checkpoint — holds.
2. Slice 1 records all fourteen fields; conditional fields carry `N/A — reason` — holds.
3. The C1/C4/C5 fence is created in slice 1; C2 and C3 fences are existing permanent gates named explicitly; every fence carries its named mutation; no fence-less claim exists — holds.
4. No new loop, no always-on phase, no latency obligation — holds (both fields `N/A — reason`).
5. Module shape field and growth ledger cover `commands/mod.rs`, `app.rs`, and `state.rs` (the untouched owner); no Length review is triggered; the slice crosses no seam — holds.
6. Partition arithmetic recorded (421 + 30% ≈ 547 ≤ 4,000 → one increment); the slice names Increment 1; the increment has a mergeable definition against the discovered default branch — holds.
7. Tracker taxonomy applied (no deferrals) — holds.
8. Every fence is a deterministic assertion or a census script with claim-id-localized output; no budget or latency qualification exists — holds.
9. No slice is declared complete here; completion is checkpointed-build's — holds.

---

# Slice 1 checkpoint record (checkpointed-build, 2026-09-27)

Owner of this record: the slice-1 record above. Source state judged: base `f9bc81d8` plus the working tree that the slice commit on `chore/cyril-6bwr-remove-show-picker-variant` captures byte-for-byte (`git status --short -- crates docs` listed exactly `crates/cyril-core/src/commands/mod.rs`, `crates/cyril/src/app.rs`, `docs/omp-review-command-analysis.md` before the commit; the tree is clean after it). Every command ran from the linked worktree with `env -u CARGO_TARGET_DIR`; logs live under the session scratchpad `6bwr/cyril-6bwr-*.log` (private, issue-id-prefixed, because the shared scratchpad collided with another agent's output on the first run — that first run's eight `GATE-OK` echoes were still this script's own exit codes, but its logs were discarded and the whole chain re-run cleanly).

**Plan critique (before the slice):** no loop budget, no fixture, oracles independent of the implementation (grep census vs rustc vs git-diff+TestBackend tests), module shape covers every touched module and the untouched owner — nothing implausible.

**Step 1 — impact analysis.** Callers of `CommandResult::show_picker` / `CommandResultKind::ShowPicker`: none in production. Sites at base: `commands/mod.rs:177` (variant), `:184` `:192` `:201` (doc citations), `:239-243` (constructor), `app.rs:2043-2045` (dispatch arm). Tools: `grep -rn` (route.md T1) and `tethys callers "CommandResult::show_picker" --lsp` → "No callers found" (rust-analyzer degraded in the worktree, index-only); after the edit `tethys index --rebuild` + `tethys callers` → "not found: symbol" (second tool confirms removal).

**Step 2 — helper search.** N/A: no Rust utility code is written; the census oracle is POSIX `sh`/`grep`/`sed`, with no in-source or manifest helper to reuse.

**Step 3 — implement.** Deletion plus three comment rewordings and one doc-line correction; the census fence was RED on the pre-change tree (positive control) and GREEN after the edit — the RED→GREEN cycle for the census claims. No unit-level logic to TDD.

**Step 4 — symmetry audit.** N/A: no parallel path added or repaired; no acceptance predicate tightened or relaxed.

## Gate (eleven items)

| # | Item | State | Evidence |
|---|---|---|---|
| 1 | Affected unit tests | PASS | `cargo nextest run --workspace --all-features` → 2057 run / 2057 passed / 13 skipped (`cyril-6bwr-gate-nextest-all.log`), covering every `commands::*` test in cyril-core and the picker tests; default features 2055/2055; `-p cyril-core --no-default-features` 782/782. |
| 2 | Falsifiers | PASS | C1: oracle `GREEN (C1 C4 C5)` exit 0 on the post-change tree; positive control at `f9bc81d8` RED with 6×`C1 FAIL` (mod.rs:177/184/192/201/241, app.rs:2043) + 1×`C5 FAIL` (omp doc:96). C2: `GATE-OK-clippy-all` (+ `clippy-default`, `clippy-core-nodefault`). C3: `git diff f9bc81d8 --stat -- crates/cyril-ui/src/state.rs` empty; `git diff f9bc81d8 -U0 -- crates/cyril/src/app.rs` = exactly one hunk `@@ -2043,3 +2042,0 @@`, 3 deletions / 0 additions; named picker tests PASS (item 7). C4: oracle GREEN; extractor at base saw `ShowPicker ShowUsage Steer ToggleVoice`. C5: oracle GREEN. |
| 3 | Stress fixture | N/A — plan records no fixture (no logic implemented) | — |
| 4 | Implementation vs oracle | PASS | C1/C4/C5: edited tree vs grep census agree (GREEN) and the census is proven sighted (positive control RED). C2: rustc/clippy agrees with the census — no stale `ShowPicker` reference compiles into an error and no orphaned import warns. C3: git-diff census (untouched) and the TestBackend picker tests (green) agree. |
| 5 | Module shape | PASS | `commands/mod.rs` 1938→1926 (projection 1922–1927); `app.rs` 7619→7616 (exact −3); `state.rs` unchanged; interface delta exactly −1 variant, −1 constructor, −1 import; protected parent `app.rs`: one 3-line deletion hunk, `app.rs:1453-1465` untouched; dependency direction unchanged — `git diff --stat f9bc81d8 -- '*Cargo.toml' Cargo.lock` empty. Shape fence: oracle GREEN. Length review: N/A (no gate; every module shrinks). |
| 6 | Production-scale budget | N/A — plan records no loop or phase budget | — |
| 7 | Regression fence | PASS | Oracle GREEN (C1/C4/C5); clippy gates GATE-OK (C2); `picker_marks_active_row_exactly_once_for_model`, `..._for_effort`, `..._from_wire_current_fallback` and the 11 `picker_viewport` tests PASS (C3; `cyril-6bwr-gate-nextest-all.log` and `cyril-6bwr-mut-c3-green.log`). |
| 8 | Named mutation red | PASS | C1 re-added variant → `C1 FAIL: crates/cyril-core/src/commands/mod.rs:175:    ShowPicker { title: String, options: Vec<CommandOption> },` exit 1. C4 `ToggleVoyce` → `C4 FAIL: doc comment in crates/cyril-core/src/commands/mod.rs cites non-variant `ToggleVoyce`` exit 1. C5 base doc line restored → `C5 FAIL: docs/omp-review-command-analysis.md:96:…` exit 1. C2 (m1) import left in → `cargo clippy -p cyril-core --all-targets -- -D warnings` exit 101, `error: unused import: `crate::types::CommandOption``. C3 `opt.is_current = false;` → `cargo nextest run -p cyril-ui --test picker_active_marker` exit 100, `picker_marks_active_row_exactly_once_for_model` FAIL (and `_for_effort`, panicked at `picker_active_marker.rs:141`). |
| 9 | Fence restored green | PASS | Every restoration copied the pre-mutation backup back and matched its sha256 (`C1/C4/C5/C2/C3 RESTORED-OK`); oracle GREEN ×3, clippy green, 14/14 picker tests PASS; `git diff f9bc81d8 --stat -- crates/cyril-ui/src/state.rs` empty after restoration. |
| 10 | Parity and reuse | N/A — reason: the slice adds no production symbol, writes no construction a second time, adds/repairs no parallel path, tightens/relaxes no predicate; the only new artifact is the non-production oracle script. Searches run: `grep -rn 'ShowPicker\|CommandResult::show_picker\|show_picker(' crates/` and tethys (index) — both recorded above. | — |
| 11 | Preserved enforcement | N/A — reason: the diff repoints, relaxes, deletes, or orphans no gate, fence, validator, oracle, or policy file; the removed arm and constructor were not enforcement artifacts; no CI, lint, or manifest file is touched (`git status` lists only the three files). | — |

**Step 7 — stale-reference sweep.** `commands/mod.rs`: the three reworded citations name `ToggleVoice` / `Steer`, both declared; no other prose names the removed variant. `app.rs`: no comment references it (census). `docs/omp-review-command-analysis.md:96` now names the live path. `CLAUDE.md:190` already names the live path — unchanged. `docs/plans/2026-03-21…:398` and `2026-03-22…:39` are dated history (design non-goal). No tracker phrases in code or the commit message beyond the issue id. Nothing else found.

**Step 9 — drift check.** `git fetch origin main` → `git rev-list --count f9bc81d8..origin/main` = 0; no upstream movement; nothing to merge.

**Step 10 — size tripwire.** Actual: code+docs `3 files changed, 4 insertions(+), 19 deletions(-)` (mod.rs 3/15, app.rs 0/3, doc 1/1); artifacts 336 lines (+ this record). ≈ 420 changed lines vs the plan's 421 + margin — under 4,000; single increment as planned.

## Final integration check

Single slice ⇒ the assembled implementation is the slice. Fresh proof on the final tree: the clean gate re-run (all eight `GATE-OK`, 2026-09-27T05:41:21Z–05:41:53Z) executed after every mutation was restored, so it is the assembled-state evidence. Design-conformance review (module shape applies): see the record appended below.

### Final design-conformance review (isolated)

**Isolation method:** a separate read-only reviewer agent started in a fresh context carrying only the production tree of this worktree and the reconstruction task (no design/plan/route artifacts, no `docs/`, no `*.md`); it reported on 2026-09-27 before `design.md` was compared.

**Reconstructed map (reviewer's findings, from source and manifests only):**

- `crates/cyril-core/src/commands/mod.rs` — `pub enum CommandResultKind` (lines 170–218) has 12 variants in order: `SystemMessage(String)`, `NotACommand(String)`, `ShowThemePicker`, `Dispatched`, `Steer { text }`, `ClearSteer`, `ToggleVoice`, `ShowUsage { account_query_started }`, `ShowPowers { powers }`, `MemoryStatus(..)`, `MemoryAction(..)`, `Quit`; `impl CommandResult` (220–295) has 12 constructors, one per variant; "there is no `show_picker` constructor".
- `crates/cyril/src/app.rs::handle_command_result` (2031–2115) — 12 arms, one per variant; `ShowThemePicker → ui_state.open_theme_picker()`, `ToggleVoice → self.toggle_voice()`, `ShowPowers → ui_state.show_powers_panel`, `ShowUsage → open_usage_panel + request_usage_snapshot (+ QueryUsageAccount)`, `Quit → request_quit`, `Steer`/`ClearSteer`/`MemoryAction` → routing-bug log only, `NotACommand`/`Dispatched` → no-op. No `ShowPicker` arm.
- `show_picker(` — single definition `crates/cyril-ui/src/state.rs:2375` (`impl UiState`); production call sites: exactly one, `crates/cyril/src/app.rs:1462`; all other call sites are tests (`state.rs` tests ×10, `tests/picker_viewport.rs:274`, `tests/picker_active_marker.rs:93/130/156`).
- Dependency direction: `cyril-core/Cargo.toml` lists `cyril-ui`: **no**; `cyril-ui/Cargo.toml` lists `cyril-core`: yes; `cyril/Cargo.toml` lists both: yes.
- "split as" doc lines in `commands/mod.rs`: `:178` `ToggleVoice` (declared: yes), `:186` `ToggleVoice` (yes), `:190` `Steer` (yes), `:195` `Steer` (yes), `:203-204` `ShowUsage` (yes).
- `ShowPicker` / `CommandResult::show_picker` under `crates/`: **none**.
- `Notification::CommandOptionsReceived` handler: `app.rs:1452-1465`; non-empty options → `self.ui_state.show_picker(command.clone(), options.clone())` at `:1462`; empty → system message.

**Comparison with `design.md` (module ledger, protected parent, placement):** interface of `commands/mod.rs` = 12 variants + matching constructors, exactly the approved narrowing (−`ShowPicker`, −`show_picker`); `app.rs` projection arms = one per surviving variant, no new responsibility body; picker opening owned solely by `cyril-ui/state.rs` with the App wiring as its single production caller; dependency direction unchanged; every doc citation resolves; census zero. **Mismatches: none. Result: PASS.**
