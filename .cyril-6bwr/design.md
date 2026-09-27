# Design: cyril-6bwr — remove the producer-less `CommandResultKind::ShowPicker`

## Route and inputs

- **Route:** Structural (`route.md`, 2026-09-27, base `f9bc81d8`). T2 fired because a `pub` enum variant and a `pub` constructor leave `cyril-core`'s surface; responsibility ownership is unchanged.
- **Behavior set:** `spec.md` is `N/A — behavior fully explicit`; the complete given/when/then contract is `route.md` T4 evidence, B1–B5. Restated by id:
  - **B1** post-change `crates/` census for `ShowPicker` / `CommandResult::show_picker` → zero hits.
  - **B2** `cargo clippy --workspace --all-targets --all-features -- -D warnings` → exit 0; the orphaned `use crate::types::CommandOption;` (`commands/mod.rs:10`) is removed with its two uses.
  - **B3** `Notification::CommandOptionsReceived` with non-empty options still opens the picker via `UiState::show_picker`; `app.rs:1453-1465` and `UiState::show_picker` are unchanged; existing `UiState::show_picker` tests pass.
  - **B4** every "same split as" doc phrase in `commands/mod.rs` names only surviving variants and still describes the command/App split.
  - **B5** no living doc presents `CommandResult(Kind)::ShowPicker` as an existing path; `docs/omp-review-command-analysis.md:96` corrected; dated `docs/plans/2026-03-*` records left as history; `CLAUDE.md:190` already names the live path.
- **Empirical premises:** `N/A — no unverified premise` (`route.md` T1: zero producers proven by grep census, tethys indexed callers, and the orphaning commit `ed13a75c`).
- **Edge-case decisions / delivery increments:** none beyond B1–B5; a single increment (see `plan.md`).

## Input shapes

| # | Shape / decision cell | Cells | Status |
|---|---|---|---|
| S1 | `CommandResultKind` — the sum `App::handle_command_result` (`app.rs:2031-2118`) dispatches over | 13 variants at base; 12 after: `SystemMessage`, `NotACommand`, `ShowThemePicker`, `Dispatched`, `Steer`, `ClearSteer`, `ToggleVoice`, `ShowUsage`, `ShowPowers`, `MemoryStatus`, `MemoryAction`, `Quit` — every surviving arm unchanged; the removed arm `ShowPicker { title, options }` was unreachable (zero producers) | surviving arms: **C2** (exhaustive `match` — a missing arm is `E0004`, a stale arm is `E0599`); removed arm: **C1** |
| S2 | `CommandResult::show_picker(title: String, options: Vec<CommandOption>)` | zero callers (grep + tethys); the constructor and its two argument shapes | **C1** |
| S3 | "same split as" doc phrases in `commands/mod.rs` | five phrases at base (lines 184 → `ShowPicker`/`ToggleVoice`; 192 → `ShowPicker`; 197 → `Steer`; 201 → `Steer`/`ShowPicker`; 210 → `ShowUsage`); after rewording every cited name is a declared variant | **C4** (mechanical: cited ⊆ declared); prose accuracy: see Non-goals |
| S4 | `use crate::types::CommandOption;` (`commands/mod.rs:10`) | uses at base: variant field (179), constructor (239); after: zero → import removed | **C2** (`unused_imports` under `-D warnings`) |
| S5 | `Notification::CommandOptionsReceived { command, options }` at `app.rs:1453-1465` | `options` non-empty → `UiState::show_picker`; `options` empty → system message | non-empty: **C3**; empty: `N/A — permanent non-goal for this change: the handler is not modified (requester constraint "stays untouched"); the empty branch is pre-existing behavior outside this change's reach` |
| S6 | Documents naming the variant | living: `docs/omp-review-command-analysis.md:96` ("exists" — corrected), `CLAUDE.md:190` (already the live path, unchanged); dated history: `docs/plans/2026-03-21-cyril-v2-architecture-design.md:398`, `docs/plans/2026-03-22-v2-known-regressions.md:39` (status-stamped records of the pre-`ed13a75c` design) | living: **C5**; dated: `N/A — permanent non-goal: dated plan records are history, not documentation of the current tree; the oracle excludes docs/plans/` |

No shape is owned by two lists: S1 enumerates the enum as data and records the dispatch verdict per arm in the same row.

## Subtractive sweep

The change is subtractive in form (it deletes a variant, a constructor, and a dispatch arm) but removes **no runtime constraint**: with zero producers the constructor never ran and the arm never matched, so no ordering, uniqueness, validation, or serialization guarantee depended on them. Walk of the "can't happen" chain:

- *A picker could open synchronously from `Command::execute`* — never happened (no producer); after the change it cannot happen by construction (no variant). Still-safe.
- *The picker opens on `CommandOptionsReceived`* — owned by the notification path (`app.rs:1453-1465` → `UiState::show_picker`), which this change does not touch. Guarded by **C3**.
- *Other `CommandResultKind` consumers* — `commands/workflow.rs` and `commands/subagent.rs` tests match `SystemMessage`/`Dispatched` only; `App` matches exhaustively. Guarded by **C2**.
- *Fences that referenced the variant* — none existed (grep census), so no gate is weakened.

No behavior is moved, rerouted, or re-homed; error taxonomy and serialized output are untouched.

## Placement

No new capability; this is a removal. Per existing capability touched:

| Capability | Owner (retain) | New seam | Forbidden |
|---|---|---|---|
| Typed command intents (`CommandResult` / `CommandResultKind`) | `crates/cyril-core/src/commands/mod.rs` | none — existing seam | adding a replacement variant or any synchronous picker escape hatch; touching `UiState`; changing lint levels; `#[allow]` |
| Projection of intents onto UI (`App::handle_command_result`) | `crates/cyril/src/app.rs` | none | new responsibility bodies; touching the `CommandOptionsReceived` handler (`app.rs:1453-1465`) |
| Picker opening (`UiState::show_picker`) | `crates/cyril-ui/src/state.rs` | none | any edit (requester: "do not modify `UiState::show_picker`") |

## Module shape

T2 records an interface narrowing, so the module-shape Design procedure was executed:

**1. Inventory**

| Module | Production lines (base) | Interface | Responsibility clusters | Dependencies | Callers / tests crossing | Adapters | Class |
|---|---:|---|---|---|---|---|---|
| `crates/cyril-core/src/commands/mod.rs` | 1938 | `Command` trait, `CommandRegistry`, `CommandContext`, `CommandResult{Kind}` + constructors | command parsing/dispatch; typed intent results; per-engine command sources | `crate::types`, `crate::protocol::BridgeSender` (write-only) | `App::handle_command_result`; `commands/{workflow,subagent}.rs` tests | none (concrete) | `retain` — interface narrows by one variant + one constructor; `CommandOption` import dropped |
| `crates/cyril/src/app.rs` | 7619 | `App` event loop | orchestration: notification routing, key dispatch, command-result projection | `cyril_core`, `cyril_ui`, `cyril_memory` | binary only | none | `retain` — one dead arm removed, no responsibility change (protected parent) |

**2. Seam tests** (the `CommandResult` seam between command layer and App)

- Deletion test: deleting the seam would push bridge sends, memory actions, and steer routing into `App` — hidden complexity reappears → the seam is real; retained.
- Interface test: `App` and the cyril-core tests both exercise `CommandResultKind` through the same public enum → pass.
- Adapter test: `N/A — existing seam: no new generic seam is introduced.`
- Locality test: intent owned by `commands/mod.rs`, projection by `app.rs`, picker opening by `state.rs` — one owner and one verification location each → pass.

**3. Three alternatives:** `N/A — existing seam: ownership is not a decision; the requester fixed "remove the variant, no replacement".`

**4. Module ledger**

| Module/path | Interface | Owns | Hides/reuses | Must not own | Adapters | Tests through | Change |
|---|---|---|---|---|---|---|---|
| `crates/cyril-core/src/commands/mod.rs` | `CommandResultKind` (12 variants) + constructors | typed command intents | command implementations | UI state; picker opening | N/A | `commands/*` unit tests; `App` compile | `retain` (narrow) |
| `crates/cyril/src/app.rs` | `App` | projection of intents onto `UiState` | — | picker option logic | N/A | workspace compile; App tests | `retain` |
| `crates/cyril-ui/src/state.rs` | `UiState::show_picker` | picker opening/marking | `PickerState` | command semantics | N/A | `tests/picker_*.rs`, `state.rs` tests | `retain` (untouched) |

Protected parent:

| Protected parent | Baseline responsibilities | Allowed change | Forbidden change | Exit condition |
|---|---|---|---|---|
| `crates/cyril/src/app.rs` | orchestration (routing, key dispatch, projection) | delete the `ShowPicker` arm (lines 2043-2045) | any new responsibility body; any edit to the `CommandOptionsReceived` handler | `git diff f9bc81d8 -- crates/cyril/src/app.rs` shows exactly one hunk, a pure 3-line deletion |

**5. Shape claim:** **C1** — mechanical fence `.cyril-6bwr/oracles/no-show-picker-variant.sh` (grep census, claim-id-prefixed output, exits 1 on red; takes an optional revision so the positive control is reproducible).

**Length review:** `N/A — no repository length gate exists (route.md T2 lookup: no hits in Cargo.toml/.github/scripts/docs/agents, clippy.toml absent) and the change only removes lines (mod.rs −13±2, app.rs −3).`

## Claims

- **C1** — After the change, no file under `crates/` names `ShowPicker` or `CommandResult::show_picker`.
- **C2** — The workspace builds and lints clean under `-D warnings` with the variant, constructor, dispatch arm, and the `CommandOption` import removed.
- **C3** — The notification-driven picker path is untouched: `UiState::show_picker` and the `CommandOptionsReceived` handler are byte-identical to base, and the existing `UiState::show_picker` tests pass.
- **C4** — Every variant a "same split as" doc phrase in `commands/mod.rs` cites is a declared variant of that file.
- **C5** — No living document under `docs/` or `CLAUDE.md` presents `CommandResult(Kind)::ShowPicker` as an existing path.

## Falsification

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C1 | no source under `crates/` names the variant or constructor | S1 (removed arm), S2, S3 | Run `sh .cyril-6bwr/oracles/no-show-picker-variant.sh` on the post-change tree; expected `GREEN`, exit 0. Any `C1 FAIL:` line falsifies. Absence control: the same script with argument `f9bc81d8` must print the six known sites (mod.rs:177/184/192/201/241, app.rs:2043) — it does (run log), so a GREEN cannot be a blind pattern. Other cause of GREEN: none once the positive control holds. | POSIX `grep`/`git grep` text census — a different failure mechanism from rustc (a dead `pub` variant compiles warning-free, so the compiler cannot see this at all) | Re-add `    ShowPicker { title: String, options: Vec<CommandOption> },` to `CommandResultKind` in `crates/cyril-core/src/commands/mod.rs` (with the import). Expected red: `C1 FAIL: crates/cyril-core/src/commands/mod.rs:<n>:    ShowPicker {`, exit 1. | `.cyril-6bwr/oracles/no-show-picker-variant.sh` (permanent, issue-local) | seconds | positive control `PASS` (run log); absence half `PASS` — discharged 2026-09-27 at the slice 1 checkpoint (`plan.md` § Slice 1 checkpoint record, items 2/7/8/9) |
| C2 | workspace builds and lints clean | S1 (surviving arms), S4 | `env -u CARGO_TARGET_DIR cargo clippy --workspace --all-targets --all-features -- -D warnings > log 2>&1 && echo GATE-OK`; a missing `GATE-OK` falsifies. Other cause of a red: a pre-existing warning on base — excluded because base (`f9bc81d8`, main) is green in CI. Other cause of a green: none; clippy is the measuring instrument. | rustc type checker + clippy — independent of the grep census | (m1) Leave `use crate::types::CommandOption;` in place after removing its two uses → `error: unused import: `crate::types::CommandOption`` (`unused_imports`, `-D warnings`). (m2) Leave the `CommandResultKind::ShowPicker { title, options } =>` arm in `app.rs` after removing the variant → `error[E0599]: no variant named `ShowPicker``. Either red proves the fence; m1 is the one that would otherwise silently ship, so m1 is the one checkpointed-build applies. | CI clippy gate (`-D warnings`, `.github` workflow), reproduced by the gate commands in `plan.md` | 5–10 min on the shared box | `PASS` — discharged 2026-09-27 at the slice 1 checkpoint (`plan.md` § Slice 1 checkpoint record, items 2/7/8/9) |
| C3 | live picker path untouched and its tests pass | S5 (non-empty options) | (a) `git diff f9bc81d8 --stat -- crates/cyril-ui/src/state.rs` prints nothing and `git diff f9bc81d8 -U0 -- crates/cyril/src/app.rs` contains exactly one hunk, at the `ShowPicker` arm, none in 1450–1465; (b) `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-ui` lists `picker_marks_active_row_exactly_once_for_model`, `..._for_effort`, `..._from_wire_current_fallback` and the `picker_viewport` tests as PASS. A non-empty state.rs diff, a second app.rs hunk, or a red/absent test falsifies. Other cause of green tests: silent skip — excluded by requiring the named tests to appear in the nextest log. | (a) `git diff` census; (b) the existing tests render the open picker to a ratatui `TestBackend` and read the drawn marker row — a different mechanism from the `is_current` state mutation under test | In `UiState::show_picker` (`crates/cyril-ui/src/state.rs:2380`), change `opt.is_current = i == idx;` → `opt.is_current = false;`. Expected red: `picker_marks_active_row_exactly_once_for_model` fails its `marked_row` assertion. Applied and restored with the Edit tool only, after the change commit, so restoration is a verified inverse edit (never `git checkout` on uncommitted work). | existing `crates/cyril-ui/tests/picker_active_marker.rs`, `picker_viewport.rs`, and `state.rs` show_picker tests (permanent) | 3–8 min | `PASS` — discharged 2026-09-27 at the slice 1 checkpoint (`plan.md` § Slice 1 checkpoint record, items 2/7/8/9) |
| C4 | cited "same split as" names are declared variants | S3 | Run the oracle; any `C4 FAIL:` line falsifies. Sensitivity control: the base tree cites `ShowPicker` and is C4-green because the variant then existed, and the extraction pipeline lists every cited name at base (run log) — so a GREEN is not a blind extractor. | `grep -o`/`sed` name extraction compared with the four-space-indented declaration set — independent of rustc and of the C1 pattern | In `crates/cyril-core/src/commands/mod.rs`, change the `ShowThemePicker` doc citation `` `ToggleVoice` `` → `` `ToggleVoyce` ``. Expected red: `C4 FAIL: doc comment in crates/cyril-core/src/commands/mod.rs cites non-variant `ToggleVoyce``, exit 1. | oracle script | seconds | `PASS` — discharged 2026-09-27 at the slice 1 checkpoint (`plan.md` § Slice 1 checkpoint record, items 2/7/8/9) |
| C5 | no living doc presents the variant as existing | S6 (living docs) | Run the oracle; any `C5 FAIL:` line falsifies. Absence control: base prints `C5 FAIL: docs/omp-review-command-analysis.md:96` (run log). | `grep` census over `docs/` + `CLAUDE.md` + `CONTEXT.md`, excluding `docs/plans/` | Revert `docs/omp-review-command-analysis.md:96` to its base text. Expected red: `C5 FAIL: docs/omp-review-command-analysis.md:96:| Slash-command registry, `CommandResult::ShowPicker`, …`, exit 1. | oracle script | seconds | `PASS` — discharged 2026-09-27 at the slice 1 checkpoint (`plan.md` § Slice 1 checkpoint record, items 2/7/8/9) |

## Non-goals and future work

- **Permanent non-goal — no replacement variant.** The requester chose issue option (a). A future engine that answers options synchronously would still ride the bridge → `Notification::CommandOptionsReceived` → `UiState::show_picker` path; a synchronous escape hatch from `Command::execute` would violate the "event loop never blocks on command execution" rule in `CLAUDE.md`.
- **Permanent non-goal — dated `docs/plans/2026-03-*` records stay as written.** They are status-stamped history of the pre-`ed13a75c` design, not documentation of the current tree; the oracle excludes `docs/plans/`.
- **Permanent non-goal for this change — prose accuracy of the reworded doc comments has no mechanical oracle.** C4 fences the checkable half (every cited name exists); the "still describes the split accurately" half is discharged by the PR reviewer reading three sentences. No behavior depends on it.
- **Permanent non-goal for this change — no App-level test for the `CommandOptionsReceived` → `show_picker` wiring is added.** The requester fixed the scope to the removal and forbade touching the live path; the gap is pre-existing (only the production site at `app.rs:1453-1465` exists) and is reported to the orchestrator under "Discovered, not fixed" for the tracker owner to file — this agent must not mutate the tracker.

## Falsifier run log

- 2026-09-27 | `sh .cyril-6bwr/oracles/no-show-picker-variant.sh f9bc81d8` | RED, exit 1 — exactly as the positive controls require: `C1 FAIL` ×6 (`commands/mod.rs:177`, `:184`, `:192`, `:201`, `:241`; `app.rs:2043`), `C5 FAIL` ×1 (`docs/omp-review-command-analysis.md:96`), `C4` silent (variant existed at base). → C1 and C5 absence controls **PASS**; C4 sensitivity control **PASS**.
- 2026-09-27 | same script, no argument, on the pre-change working tree | identical output, exit 1 (tree == base).
- 2026-09-27 | `git show f9bc81d8:crates/cyril-core/src/commands/mod.rs | grep -o 'split as [^.]*' | grep -o '`[A-Za-z]*`' | tr -d '`' | sort -u` | `ShowPicker ShowUsage Steer ToggleVoice` — the C4 extractor sees every cited name at base (sensitivity control **PASS**).
- 2026-09-27 | `gh run list --branch main --limit 3 --json conclusion,name,headSha` | CI `success` at `f9bc81d83c22e8abbf25194fcd245e6b8b207310` — base is clippy-green, so a C2 red can only be caused by this change (C2 "other cause" excluded).

## Approval

Requester decisions, relayed verbatim by the orchestrator on 2026-09-27 as "the user's approved scope; treat them as the approved spec/design decisions the contract's approval semantics require":

> "Option (a) from the issue: REMOVE `CommandResultKind::ShowPicker` and the `CommandResult::show_picker` constructor from crates/cyril-core/src/commands/mod.rs (about lines 177 and 239) and the unreachable dispatch arm in crates/cyril/src/app.rs (about line 2043, `self.ui_state.show_picker(title, options)`). No replacement. The notification-driven picker path (`Notification::CommandOptionsReceived` handler in app.rs → `UiState::show_picker` in crates/cyril-ui/src/state.rs) is the only live path and stays untouched — do not modify `UiState::show_picker`."

> "Reword the doc comments in commands/mod.rs (about lines 184, 192, 201) that cite `ShowPicker` as the "same split" example so they reference a surviving variant (e.g. `ToggleVoice` or `Steer`) and still describe the command/App responsibility split accurately."

> "Also check docs/ and CLAUDE.md for mentions of `CommandResultKind::ShowPicker` and correct any that present it as live."

> "If the route comes out Structural because a public enum variant leaves cyril-core's surface: the requester's design approval is this exact scope (remove the variant, no replacement, zero producers). Record that approval verbatim in design.md and proceed through the stages the skill requires without waiting."

Date: 2026-09-27. Risk acceptances approved: **None** (no row carries `N/A — approved risk`).
