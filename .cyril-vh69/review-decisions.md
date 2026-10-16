# review-decisions.md — cyril-vh69 independent review, round 1

Reviewer: `semantic_reviewer` agent (gilfoyle final-integration independent review), full report at `.cyril-vh69/independent-review.md`. Scope reviewed: `main..a90ba0ac` (slices 1–5). Verdict: changes-requested.

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F1 | Effort confirm revalidates via `contains_value("effortLevel", …)` against a catalog select option, but effort rows come from the model's `_meta.kiro` EffortMetadata; no catalog in the 2.31.1 capture carries an `effortLevel` option, so effort confirms are refused spuriously (chained race deterministic; possibly every effort confirm). Fix: revalidate against EffortMetadata levels | semantic_reviewer | Verified | Code reading of `confirm_config_picker` (app.rs:2068-2071): single `contains_value` path for all config ids. Capture grep: the only `"effortLevel"` in `kas-live-sweep-2.31.1.jsonl` is inside a `_kiro/workflow/listRecipes` frame (line 136), never a config catalog. Chained race confirmed in code: effort picker opens with rows from the pre-switch snapshot's `effort_picker_options`; confirm re-checks the same-or-newer snapshot for a nonexistent option | Modify | R1: effort confirms revalidate against the latest snapshot's `EffortMetadata` of the model the picker was opened for (picker carries that model id); `contains_value` retained for `model`/`mode` | blocking — headline acceptance flow (`/model` → effort chain, `/effort`) broken |
| F2 | No test presses Enter on the effort step; C7 asserted for model only, masking F1 | semantic_reviewer | Verified | Reviewer read of app.rs tests (`model_confirm_sends_one_set_and_chains_effort_picker` asserts open+Esc only; stale-refusal test covers `model` only) | Accept | R1 (same atomic repair as F1): add effort-confirm Enter tests — post-ack catalog AND chained pre-ack race — observed RED against pre-R1 code, GREEN after | blocking — missing required coverage for C7/C8 |
| F3 | `UiState::config_catalog` is dead production state (chain data comes from `SessionController::config_options()`) | semantic_reviewer | Verified | Grep: written (state.rs:1042), accessor (1677), consumers only in own tests (6435-6449); no callers in cyril/src or widgets | Accept | R3: remove field + accessor + test; design.md amended (SessionController is the chain-data source) | non-blocking — dead duplicated state; design deviation corrected toward the better owner |
| F4 | `pending_config_set` noted before the bridge send; a failed send leaves a stale pending entry → spurious returned≠requested debug log on the next unrelated ack | semantic_reviewer | Verified | Code reading: `note_config_set_requested` (app.rs:2085) precedes `send(...).await?`; on failure `?` propagates with the entry already set | Accept | R2: note only after a successful send | non-blocking — log-only false positive, but the log exists to surface KAS silent-ignores |
| F5 | Config picker preselects row 0, not the `is_current` row | semantic_reviewer | Verified | Code reading: `show_config_picker` sets `selected: 0` unconditionally (state.rs:2632) | Accept | R4: preselect the `is_current` row when present | non-blocking — UX polish; acceptance criteria ("default marked") already honored |

## Review errors

None — the reviewer's anchors, counts, and capture claims all checked out.

## Repairs

### R1 — effort confirm revalidates against the picker's row source (F1, F2)

- **Root cause:** one revalidation path (`KasConfigSnapshot::contains_value`) was used for every config id, but effort picker rows are sourced from `EffortMetadata.levels`, not from a catalog `effortLevel` select option. Governing claims: C7, C8; owning slice: 5.
- **Delivered behavior:** the effort picker carries the model id it was opened for — `subject: Option<String>` on `CommandResultKind::ShowConfigPicker` (set by `EffortCommand` from `current_model()`), `PickerState`, and `PickerConfirmation` (set by the App's chain from the just-confirmed model value). `confirm_config_picker` revalidates per config id: `effortLevel` checks the LATEST `SessionController` snapshot's `effort_for_model(subject)` for level membership; `model`/`mode` keep `contains_value`. A stale effort (metadata gone or level absent) keeps the existing refusal message + warn + zero sends; a missing subject is an error-logged construction bug and refused. Commit `36a29e75`.
- **Performed checks and results:** RED receipts (pre-fix, `cargo test -p cyril --bins effort_confirm`): `effort_confirm_sends_one_effort_level_set` → `expected one SetConfigOption, got Err(Empty)`; `chained_effort_confirm_before_ack_sends_both_sets` → `expected the effortLevel set second, got Err(Empty)` — the effort confirm was refused and sent nothing, exactly F1. GREEN receipts (post-fix): both tests pass (post-ack confirm sends exactly one `{effortLevel, <level>}` frame; chained pre-ack sends model then effortLevel, two frames total). `cargo test -p cyril -p cyril-core -p cyril-ui` green; `cargo clippy --all-targets -- -D warnings` green. Scoped repair re-review: pending (orchestrator).
- **Unresolved failures:** none

### R2 — pending config-set noted after successful send (F4)

- **Root cause:** ordering — note preceded `send(...).await?`. Owning slice: 5.
- **Delivered behavior:** `note_config_set_requested` moved to after `bridge_sender.send(...).await?`; a failed send now propagates with no pending entry, so the next unrelated ack cannot compare against a stale requested value. The pending-comparison consumer (`UiState::ack_config_set`) is unchanged and still behaves (takes pending only on config-id match; shown value always the returned one). Commit `627283a8`.
- **Performed checks and results:** `cargo test -p cyril` green (210 passed); `cargo clippy --all-targets -- -D warnings` green; workspace suite/clippy/fmt green at the final gate.
- **Unresolved failures:** none

### R3 — dead `UiState::config_catalog` removed (F3)

- **Root cause:** design said UiState keeps the chain-data catalog; implementation sourced it from SessionController, leaving the UiState copy write-only. Owning slice: 4. design.md amended in place (technical correction; approved behavior unchanged).
- **Delivered behavior:** field, initialization, wholesale `clone_from` in the config-notification arm, accessor, and the `ui_catalog_replaces_wholesale` test removed (state.rs, 42 deletions). `SessionController::config_options()` remains the sole chain-data source. Commit `6eba475e`.
- **Performed checks and results:** `rg config_catalog` finds zero references in `crates/`; `cargo test -p cyril-ui` green (651 passed); `cargo clippy --all-targets -- -D warnings` green; workspace suite/clippy/fmt green at the final gate.
- **Unresolved failures:** none

### R4 — config picker preselects the current row (F5)

- **Root cause:** unconditional `selected: 0`. Owning slice: 4.
- **Delivered behavior:** `show_config_picker` opens with `selected` at the first `is_current` row (filter starts empty, so filtered order is option order), falling back to row 0 when nothing is marked. Commit `5ebeb5d4`.
- **Performed checks and results:** adjusted `config_picker_honors_sent_current_and_confirms_with_config_id` to assert the preselection (`selected == 1`) and confirm without navigation; added `config_picker_without_a_current_row_starts_at_row_zero` for the fallback. `cargo test -p cyril-ui -p cyril` green; `cargo clippy --all-targets -- -D warnings` green; workspace suite/clippy/fmt green at the final gate.
- **Unresolved failures:** none

## Repair re-review

DONE — orchestrator (independent of the repair implementer), 2026-10-15. Scope: R1–R4 diffs (36a29e75, 627283a8, 6eba475e, 5ebeb5d4) + affected confirm/picker interactions. Checked state: branch head 3ce3ff3e.

- R1: `subject` threaded ShowConfigPicker → PickerState → PickerConfirmation, `Some` iff effortLevel, documented at each layer; EffortCommand sets it from `current_model()`, the chain from the just-confirmed model value. Confirm revalidates per config id: effort → latest snapshot's `effort_for_model(subject)` level membership; model/mode → `contains_value`. Missing subject is error-logged and refused. The stale-effort refusal, zero-sends discipline, and no-SetModel invariant all hold. Red receipts (both new tests failed pre-fix with empty-channel — the spurious refusal) and green receipts verified in the record; tests assert exactly-one-frame and frame order.
- R2: note moved after `send(...).await?`; failure path leaves no pending entry.
- R3: field/init/clone_from/accessor/test removed; zero `config_catalog` references remain in `crates/`.
- R4: preselects first `is_current` row, fallback row 0; both cases tested.
- One edge considered and dismissed: confirming effort for a subject model while the latest snapshot's current model differs is unreachable within one cyril session (single client, one open picker; the model changes only through this same confirm path, whose ack makes current == subject).
- Result: no new findings. Full workspace gate re-run by the reviewer on 3ce3ff3e: `cargo test --workspace` 0 failures, `clippy --workspace --all-targets -- -D warnings` clean, `fmt --check` clean. Initial full review (independent-review.md) remains valid for all unrepaired scope.

Remaining: C13 live smoke (user/implementer, per plan) — including recording whether a post-switch catalog ever carries an `effortLevel` select option.
