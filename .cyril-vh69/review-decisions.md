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
- **Delivered behavior:** PENDING — implementer (bounded repair, checkpointed-build)
- **Performed checks and results:** PENDING — required: new effort-confirm Enter tests (post-ack and chained pre-ack) observed RED against pre-repair code (sensitivity), then GREEN; full `cargo test --workspace`, `clippy --workspace --all-targets -- -D warnings`, `fmt --check`; scoped repair re-review
- **Unresolved failures:** none yet

### R2 — pending config-set noted after successful send (F4)

- **Root cause:** ordering — note preceded `send(...).await?`. Owning slice: 5.
- **Delivered behavior:** PENDING — implementer (bounded repair)
- **Performed checks and results:** PENDING — send-failure path leaves no pending entry; workspace suite/clippy/fmt
- **Unresolved failures:** none yet

### R3 — dead `UiState::config_catalog` removed (F3)

- **Root cause:** design said UiState keeps the chain-data catalog; implementation sourced it from SessionController, leaving the UiState copy write-only. Owning slice: 4. design.md amended in place (technical correction; approved behavior unchanged).
- **Delivered behavior:** PENDING — implementer (bounded repair)
- **Performed checks and results:** PENDING — workspace suite/clippy/fmt
- **Unresolved failures:** none yet

### R4 — config picker preselects the current row (F5)

- **Root cause:** unconditional `selected: 0`. Owning slice: 4.
- **Delivered behavior:** PENDING — implementer (bounded repair)
- **Performed checks and results:** PENDING — preselection test; workspace suite/clippy/fmt
- **Unresolved failures:** none yet

## Repair re-review

PENDING — scoped independent review of the R1–R4 repair diff after the repairs land (reviewer: orchestrator, independent of the repair implementer; scope: repair diff + affected confirm/picker interactions).
