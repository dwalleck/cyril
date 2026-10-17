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

Remaining: C13 live smoke — DONE 2026-10-15 (see below).

## C13 live smoke record — 2026-10-15, kiro-cli 2.31.1, branch head 3ce3ff3e (+records commits)

Driver: tmux + sent keystrokes against `target/debug/cyril --features kas --agent-engine kas` (IAM Identity Center login active). `KIRO_ACP_RECORD_PATH` produced no trace under the KAS engine (the hook is v1/v2-TUI-only), so wire-level statements below rest on the verified read-back feed paths, not a frame capture.

KAS session (`sess_06673024-…`):
- Session start: toolbar `vibe · auto` fed from the initial `config_option_update` catalog (C11).
- `/model`: picker over the offered models, `auto ✓` marked AND preselected (C4 + R4 live).
- Confirmed `claude-sonnet-5.5` → "Model set to claude-sonnet-5.5." (read-back message, C10); toolbar → `claude-sonnet-5.5 ◇ high`; effort chain opened with `low/medium/high✓/xhigh/max`, default marked and preselected (C8).
- Confirmed `max` → "Effort set to max.", toolbar `◇ max` — the F1 repair path live (pre-repair this confirm was refused).
- `/effort` standalone: opened with the model's levels; marks the model DEFAULT (`high`), not the current `max` — per the approved "default marked" contract; noted as a UX observation, not a defect.
- `/mode`: Default✓/Spec/Quick Spec/Bug Fix/Plan/Autonomous + custom agents, wire labels/descriptions; confirmed Plan → "Mode set to plan.", toolbar `plan`.
- Re-selected the current model (assumption 3): allowed, re-sends the set, chains effort again.
- Switched back to `auto`: "Model set to auto.", effort chip CLEARED, no chain opened (C9 claude→auto, C8 no-effort branch).
- v2 regression (default engine, separate session): `/model` via the kiro.dev path with credit badges and Agent-picker behavior unchanged (C12 live).

Open wire question ANSWERED (F1 follow-up): post-switch catalogs DO carry an `effortLevel` select option — the effort chip showed `high` immediately after the model-set ack, and the only KAS feed for that chip is the config-notification arm reading an `effortLevel` option's `currentValue` (verified in the round-1 review). The round-1 inference from the 2.31.1 sweep capture (no catalog ever carries it) is explained: the sweep only ever set a bogus model, so it never captured a successful post-switch catalog. This does not weaken the R1 repair: the chained pre-ack race (pre-switch snapshot from `auto` has no `effortLevel` option) still requires EffortMetadata-sourced revalidation, and per-source revalidation remains the correct contract.

Result: all acceptance criteria exercised live; no defects found.

## Review round 2 — Codex PR feedback

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F6 | A standalone effort picker can be opened for model B while a model change to A is awaiting acknowledgement; when A becomes current, confirming B's retained effort rows can apply the choice to A. Require the latest snapshot's current model to match the picker subject before dispatch. | chatgpt-codex-connector[bot] | Verified | Code reading of `confirm_config_picker` showed effort validation checked metadata membership but not `current_model()`. Added `standalone_effort_confirm_is_refused_after_model_ack_switches_away`, which passes with zero dispatches after the delayed ack. The existing chained pre-ack test also remains green because its exact model request is still pending. | Modify | R5: effort confirmation now requires the latest snapshot's current model to equal the picker subject, with a narrowly-scoped exception only for the exact model request that opened the intentional pre-ack model→effort chain. | blocking — without this guard a delayed model acknowledgement can apply a valid effort level to the wrong model |
| F7 | A config picker opened in one session can remain open across delayed `/new` or `/load` completion and apply its selection to the replacement session. Bind it to its opening session or close it on session changes. | chatgpt-codex-connector[bot] | Verified | Code reading showed `SessionCreated` reset session state without invalidating `PickerKind::Config`. Added `session_created_closes_open_config_picker`, which passes through the App notification route and leaves no picker to confirm after the boundary. | Accept | R6: close config pickers and clear pending config-set acknowledgements in `UiState` on `SessionCreated`; keep Cyril's local theme picker independent. | blocking — a stale selection must never be dispatched into the new session |

### Repairs

#### R5 — reject standalone effort confirmations for a no-longer-current model (F6)

- **Root cause:** effort metadata membership alone does not identify the active model; a standalone picker can retain metadata for a prior model while an asynchronous model set changes the catalog's current model.
- **Delivered behavior:** `confirm_config_picker` checks `KasConfigSnapshot::current_model()` against the effort picker subject before accepting a level. The only pre-ack exception is when `UiState::has_pending_config_set("model", subject)` proves the picker is the continuation of the model selection just dispatched; this preserves the approved chained pre-ack behavior without allowing a standalone picker opened for the old model to pass.
- **Performed checks and results:** `cargo test -p cyril --bin cyril app::tests::standalone_effort_confirm_is_refused_after_model_ack_switches_away -- --exact` PASS; `cargo test -p cyril --bin cyril app::tests::chained_effort_confirm_before_ack_sends_both_sets -- --exact` PASS; `cargo test -p cyril-ui --lib state::tests::pending_config_set_matches_exact_request -- --exact` PASS. Checked revision: working tree after `cargo fmt --all`.
- **Unresolved failures:** none.

#### R6 — invalidate config pickers at session boundaries (F7)

- **Root cause:** `SessionCreated` reset session-projected values but left the config overlay and its pending request metadata alive.
- **Delivered behavior:** `UiState::apply_notification(SessionCreated)` closes only `PickerKind::Config` and clears `pending_config_set`; the local theme picker remains available. The App-level regression test exercises the routed notification and verifies the picker is gone.
- **Performed checks and results:** `cargo test -p cyril --bin cyril app::tests::session_created_closes_open_config_picker -- --exact` PASS; `git diff --check` PASS; `cargo fmt --all` PASS. Checked revision: working tree after `cargo fmt --all`.
- **Unresolved failures:** none.

### Repair re-review

DONE — scoped review after repair commit `d87fc6bc`, independent of the initial finding report. Scope: R5/R6 production paths, picker lifecycle, pending-request interaction, and regression tests. The current-model predicate blocks the verified delayed-ack standalone reproduction; the exact pending model request is the only pre-ack exception and preserves the approved chained race. `SessionCreated` closes only config pickers and clears pending acknowledgements, while theme picker behavior remains unchanged. No additional production defect or invalidated prior conclusion found.

Final checks at `d87fc6bc`: `cargo test --workspace` PASS; `cargo clippy --workspace --all-targets -- -D warnings` PASS; `cargo fmt --all -- --check` PASS; `git diff --check` PASS.

## Review round 3 — Codex PR feedback

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F8 | A failed `set_config_option` model request leaves `pending_config_set` live, so confirming the chained effort picker can send effort for a model that never became active. Clear the pending request and invalidate the chained picker on failure. | chatgpt-codex-connector[bot] | Verified | Code reading confirmed `BridgeError { operation: "set_config_option" }` previously only added a message; the new `failed_model_set_closes_chained_effort_picker` regression drives model confirm → failure → late Enter and observes no effort dispatch. | Accept | R7: handle failed config-set notifications in `UiState`, clear the pending request, and close the effort picker only when it is chained for the failed model. | blocking — a failed model request must not leave an actionable effort step |
| F9 | The chained effort picker opens before the model-set acknowledgement; confirming it can dispatch `effortLevel` against the previous model. Wait for `ConfigOptionSet` read-back before enabling effort confirmation. | chatgpt-codex-connector[bot] | Verified | Code reading and the existing `chained_effort_confirm_before_ack_sends_both_sets` regression confirm the pre-ack path is intentional and currently dispatches model then effort. The proposed acknowledgement-gated flow changes the approved round-2 interaction and user-visible timing. | Accept | R10 (round 4): the effort step opens only when the model set's `ConfigOptionSet` read-back arrives with the requested model current, built from the rebuilt catalog; the pre-ack exception is deleted. Product decision recorded in design.md § Approval (round 4). | blocking — an `effortLevel` set must never reach KAS before the model switch it belongs to |

### Repair

#### R7 — clear failed KAS config requests (F8)

- **Root cause:** `UiState::BridgeError` rendered the failure but did not reconcile the request-side `pending_config_set`; the chained effort picker remained actionable after the model set failed.
- **Delivered behavior:** `UiState::config_set_failed` consumes the failed pending request. For a failed model request, it closes only the matching `effortLevel` picker whose subject is that requested model; unrelated pickers remain untouched. The App-level regression proves a late Enter cannot dispatch an effort set.
- **Performed checks and results:** commit `dc9a82f7`; `env -u CARGO_TARGET_DIR cargo test -p cyril --bin cyril app::tests::failed_model_set_closes_chained_effort_picker -- --exact` PASS; `env -u CARGO_TARGET_DIR cargo test -p cyril -p cyril-ui` PASS; `env -u CARGO_TARGET_DIR cargo clippy --all-targets -- -D warnings` PASS; `git diff --check` PASS; `cargo fmt --all` PASS.
- **Unresolved failures:** none for F8. (R7's close-on-failure path is retired by round 4's R10: no effort step exists before the model ack, so a failed model set leaves nothing to close.)

F9 was escalated in this round and decided in round 4.

## Review round 4 — `/code-review xhigh PR 171` (Standards + Spec axes)

Reviewers: Standards sub-agent (CLAUDE.md + Fowler smell baseline) and Spec sub-agent (cyril-vh69 + cyril-838u acceptance criteria), both against `b99d3cd0...8e378ad0`. Requester direction for this round: "Address the 3 issues and all 6 smells"; F9 and F19 product decisions in design.md § Approval (round 4). Repeated product-repair stop (CONTRACT): F1 (round 1), F6 (round 2) and F8 (round 3) are verified product defects in one mechanism — the pre-ack model→effort chain — so this round takes the class-wide correction (R10: no pre-ack chain) instead of a fourth local patch; disposition recorded in design.md §2.

Blocking rows first: F9 (above), F10, F11, F19.

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F10 | `App::confirm_config_picker` holds the confirm-acceptance rule (effort: subject current or chained switch pending, level in the model's levels; model/mode: `contains_value`) despite its "composition only" doc; move the rule into one component method | Standards sub-agent | Verified | Code reading `app.rs:2078-2116` at `8e378ad0`: the per-config-id predicate, the pending lookup and the level scan all live in the App | Modify | R10: `KasConfigSnapshot::accepts(&ConfigTarget, value)` owns the rule; the pending exception the reviewer would have threaded in is deleted by F9, so the rule is a pure snapshot query | blocking — documented CLAUDE.md breach ("if logic can live in a component, it should not be in App") |
| F11 | `PickerState.config_id`/`subject`, `PickerConfirmation.config_id`/`subject` and `ShowConfigPicker.subject` rely on comment-only "Some iff" invariants (hence two `error!` construction-bug branches); carry the data in the variant | Standards sub-agent | Verified | Code reading `traits.rs:746-772`, `commands/mod.rs:268-281`, `app.rs:2071-2076, 2105-2110` | Accept | R10: `PickerKind::Config(ConfigTarget)` with `ConfigTarget::{Model, Mode, Effort { model_id }}`; `CommandResultKind::ShowConfigPicker(ConfigPicker)`; the optional fields and both `error!` branches are deleted | blocking — documented CLAUDE.md breach ("Make illegal states unrepresentable") |
| F12 | `#![allow(clippy::unwrap_used)]` at `session.rs:1535` and `#![allow(clippy::unwrap_used, clippy::expect_used)]` at `kas_config.rs:143` suppress nothing (no `.unwrap()`; `expect` already allowed in tests by `clippy.toml`) | Standards sub-agent | Verified | `awk` over each module from its allow line: zero `.unwrap()`; `clippy.toml` `allow-expect-in-tests = true`; both directives are new in this PR's diff | Accept | R8: delete both directives | non-blocking — dead suppressions; removal keeps clippy as the signal |
| F13 | Config ids are bare `&str` switched on in several places (`state.rs:1098`, the `_ => "Effort"` catch-all at `state.rs:1727`, `app.rs:2078`, `app.rs:2139`), and `UiState` keys on the `"set_config_option"` operation literal (`state.rs:1211`); introduce a config-id type | Standards sub-agent | Verified | Code reading at the cited lines; the literal is also produced at `domain_mediator/commands/session.rs:355` and matched in `cyril-workbench/src/reviewer.rs:305` | Modify | R10: `KasConfigId::{Model, Mode, Effort}` (`as_str`, `from_wire`, `noun`) replaces the three `*_CONFIG_ID` consts and every switch; the operation label becomes the shared `SET_CONFIG_OPTION_OPERATION` const (the repo's stable-operation-label idiom — `REASONING_OPERATION`, `workflow_command.rs:100`) used by the mediator and its consumers, and the UI no longer matches it at all | non-blocking — judgement-call smell |
| F14 | `pending_config_set: Option<(String, String)>` is a data clump travelling with `SetConfigOption` and `PickerConfirmation` | Standards sub-agent | Verified | Code reading `state.rs:83`, `app.rs:2125-2138` | Modify | R10: `KasConfigRequest { id: KasConfigId, value }` with `to_command()`; `PickerConfirmation` no longer carries a config id (its kind does). `BridgeCommand::SetConfigOption` keeps its generic `String` fields — `/thinking` shares it and it predates this PR | non-blocking — judgement-call smell |
| F15 | The App asks `ui_state.has_pending_config_set` to make a domain decision; in-flight request tracking is session state | Standards sub-agent | Verified | Code reading `app.rs:2094` | Accept | R10: `SessionController` owns the pending request (`note_config_set_requested`, consumed by the matching ack, cleared on a set failure and on session boundaries) and computes the post-ack effort chain; `UiState` keeps no request state | non-blocking — judgement-call smell; fixed together with F9 |
| F16 | `ModelCommand` and `ModeCommand` have the same body apart from id and strings | Standards sub-agent | Verified | Code reading `builtin.rs:622-680, 723-765` | Accept | R11: one `ConfigSelectCommand` with `model()`/`mode()` constructors (private fields — Effort is unconstructible) | non-blocking — judgement-call smell |
| F17 | `kas_config_snapshot` returns `crate::Result<Result<&Snapshot, CommandResult>>` whose inner `Err` is a user message | Standards sub-agent | Verified | Code reading `builtin.rs:603` | Accept | R11: returns `crate::Result<Option<&KasConfigSnapshot>>`; callers answer `None` with their own "not reported yet" line | non-blocking — judgement-call smell |
| F18 | `with_builtins_and_usage` now also takes `ConfigCommandSource`; the name undersells the parameters | Standards sub-agent | Verified | `commands/mod.rs:485` signature: hooks, workflows, usage_account, config | Accept | R9: rename to `with_builtin_sources` (all callers) | non-blocking — naming |
| F19 | Standalone `/effort` ticks `defaultEffortLevel`, not the level in use (C13 smoke: `high✓` while `max` was current); the issue's "default marked" applies to the chained step | Spec sub-agent | Verified | `kas_config.rs:134` sets `is_current` from the default; `.cyril-vh69/review-decisions.md` C13 record line "marks the model DEFAULT (`high`), not the current `max`" | Accept | R12: effort rows tick the `effortLevel` option's `currentValue` when it names one of the model's levels and the model is current, else the default. With R10 the chained step is built post-ack, where KAS has just applied the model's default, so "default marked" still holds there | blocking — requester decision: the picker must show the level in use |
| F20 | Comments at `app.rs:2050-2051`, `app.rs:8941-8942` and design.md §2 still say no catalog carries an `effortLevel` option; C13 disproved it | Spec sub-agent | Verified | C13 record "post-switch catalogs DO carry an `effortLevel` select option"; the cited comments | Accept | R10: the comments go with the rewritten confirm path and tests; design.md §2 amended in place | non-blocking — stale documentation |
| F21 | Effort levels never come from the `effortLevel` option's own values — always the model's `_meta.kiro` levels | Spec sub-agent | Verified | `builtin.rs:702`, `app.rs:2097-2101`, documented at `kas_config.rs:22-24` | Reject | N/A — the acceptance criteria say the effort step "offers only its effortLevels", i.e. the chosen model's `_meta.kiro.effortLevels`; that source exists in every catalog, while the `effortLevel` option exists only after a successful switch. R12 now reads that option's `currentValue` for the tick | non-blocking — permanent design choice (design.md §6), not deferred work |
| F22 | Scope notes: read-back confirmation messages, preselecting the current row (F5), closing config pickers on `SessionCreated` (F7) | Spec sub-agent | Not-applicable | N/A — the reviewer classed these "minor and supportive"; no defect claimed | Reject | N/A — keep | non-blocking — supportive behavior already reviewed in rounds 1–2 |

### Review errors

- None found: every cited anchor, line and quoted string checked out at `8e378ad0`.

### Repairs

Gate used for every repair (the CI lanes of `.github/workflows/ci.yml`): `cargo nextest run` for `-p cyril -p cyril-core --features kas`, `--workspace --all-features`, `-p cyril-core --no-default-features`; `cargo clippy --all-targets -- -D warnings` for `--workspace --all-features`, `--workspace`, `-p cyril -p cyril-core --features kas`, `-p cyril-core --no-default-features`; `cargo fmt --all -- --check`; `cargo test --doc --workspace --all-features` (R10). All commands under `env -u CARGO_TARGET_DIR` (the shell exports an empty value).

#### R8 — drop the no-op `#[allow]`s (F12)

- **Delivered behavior:** both directives deleted (`session.rs` `config_options_tests`, `kas_config.rs` `tests`). No production change. Commit `0f82aabe`.
- **Performed checks and results:** clippy `-p cyril-core --all-targets --all-features` and `-p cyril-core --no-default-features --all-targets` with `-D warnings`: clean — the suppressions were suppressing nothing.
- **Unresolved failures:** none.

#### R9 — `with_builtins_and_usage` → `with_builtin_sources` (F18)

- **Delivered behavior:** rename across every caller (`commands/mod.rs`, `app.rs`) and the active design/plan text; signature unchanged. Commit `ef0053c0`.
- **Performed checks and results:** `cargo check --workspace --all-targets --all-features` clean; `cargo test -p cyril-core --all-features --lib commands` 86 passed; fmt clean.
- **Unresolved failures:** none.

#### R10 — ack-gated effort chain + typed config vocabulary (F9, F10, F11, F13, F14, F15, F20)

- **Root cause:** one mechanism — the effort step opened before the model ack — needed a pre-ack exception (F1, F6), a UI-side pending slot the App consulted (F15) and a close-on-failure path (F8); the string ids and optional picker fields (F11, F13, F14) carried the "which picker is this" facts it depended on. Repeated product-repair stop disposition: class-wide correction (design.md §2).
- **Delivered behavior:**
  - `crates/cyril-core/src/types/kas_config.rs`: `KasConfigId::{Model, Mode, Effort}` (`as_str`, `from_wire`, `noun`) replaces `MODEL_/MODE_/EFFORT_CONFIG_ID`; `ConfigTarget::{Model, Mode, Effort { model_id }}`; `ConfigPicker { target, options }`; `KasConfigRequest { id, value }` + `to_command()`; `KasConfigSnapshot::picker(target)` (replaces `picker_options`/`effort_picker_options`) and `accepts(&target, value)` (replaces the App's inline rule and `contains_value`). `EffortMetadata::offers`.
  - `crates/cyril-core/src/session.rs`: `pending_config_set: Option<KasConfigRequest>` + `effort_chain: Option<ConfigPicker>`; `note_config_set_requested`, `take_effort_chain`; the `ConfigOptionSet` arm consumes the matching request, debug-logs returned≠requested, and prepares the effort picker only when the returned current model is the requested one; `BridgeError{SET_CONFIG_OPTION_OPERATION}` clears the request; `SessionCreated`/`BridgeDisconnected` clear both.
  - `crates/cyril-core/src/types/event.rs`: `SET_CONFIG_OPTION_OPERATION`, used by the mediator (`domain_mediator/commands/session.rs`), the session, and `cyril-workbench/src/reviewer.rs`.
  - `crates/cyril-ui`: `PickerKind::Config(ConfigTarget)` (no longer `Copy`; `picker_kind()` returns `Option<&PickerKind>`); `PickerState`/`PickerConfirmation` lose `config_id`/`subject`; `show_config_picker(ConfigPicker)` derives the title from the target's noun; `UiState` loses `pending_config_set`, `note_config_set_requested`, `has_pending_config_set`, `config_set_failed` and the `"set_config_option"` match; `ack_config_set` switches on `KasConfigId`. The five fixture files that only gained `config_id: None, subject: None` are back to their main-branch content.
  - `crates/cyril/src/app.rs`: `confirm_config_picker(target, value)` = `accepts` → send `KasConfigRequest::to_command()` → `note_config_set_requested`; `open_effort_chain()` runs after routing a `ConfigOptionSet` and drops the step when a modal overlay is open. Both `error!` construction-bug branches deleted.
  - Commit `f936ca13`.
- **Performed checks and results:**
  - Sensitivity: `model_confirm_chains_effort_only_after_the_model_ack` written first against `ef0053c0` — RED, panicked at `app.rs:8929` ("no effort step before KAS acknowledges the model switch"); GREEN after R10 (`cargo test -p cyril --bin cyril app::tests::model_confirm_chains_effort_only_after_the_model_ack -- --exact`).
  - New/rewritten coverage: session `model_set_ack_prepares_the_effort_chain_from_the_rebuilt_catalog`, `model_set_that_did_not_take_effect_prepares_no_chain`; kas_config `config_ids_round_trip_the_wire`, `accepts_revalidates_against_the_row_source`; App `effort_step_after_the_ack_sends_model_then_effort`, `failed_model_set_opens_no_effort_step` (replaces F8's `failed_model_set_closes_chained_effort_picker`), `effort_step_never_replaces_an_open_overlay`. Retired: `chained_effort_confirm_before_ack_sends_both_sets` (asserted the behavior F9 removes) and UiState `pending_config_set_matches_exact_request` (state no longer exists).
  - Gate at `f936ca13`: nextest kas lane 1445 passed, workspace (default features) 2327 passed, core no-default 861 passed; all four clippy lanes clean; doc tests pass; fmt clean.
- **Unresolved failures:** none. Consumer smoke for the changed live interaction: PENDING — see Final integration below.

#### R11 — one `ConfigSelectCommand`; `Option` snapshot helper (F16, F17)

- **Delivered behavior:** `ConfigSelectCommand::{model(), mode()}` (private fields) replaces `ModelCommand`/`ModeCommand`; name, usage, not-reported and not-offered lines derive from the target, byte-identical to before. `kas_config_snapshot` returns `crate::Result<Option<&KasConfigSnapshot>>`. Commit `be318b35`.
- **Performed checks and results:** existing refusal-matrix and happy-path command tests (which pin every user-facing string) pass unchanged in substance; nextest kas lane 1445, workspace all-features 2334, core no-default 861 passed; all four clippy lanes clean; fmt clean.
- **Unresolved failures:** none.

#### R12 — `/effort` ticks the level in use (F19)

- **Delivered behavior:** the `Effort` branch of `KasConfigSnapshot::picker` marks the `effortLevel` select's `currentValue` when `model_id` is current and the value is offered, else `defaultEffortLevel`; `/effort` doc updated. Commit `be0d3767`.
- **Performed checks and results:**
  - Sensitivity: `commands::config_command_tests::effort_command_ticks_the_level_in_use` written first against `be318b35` — RED (`left: ["high"]`, `right: ["max"]`); GREEN after. Added `kas_config::tests::effort_picker_marks_the_level_in_use` (in use / unoffered fallback / other-model fallback).
  - Gate at `be0d3767`: nextest kas lane 1447, workspace all-features 2336, core no-default 863 passed; all four clippy lanes clean; fmt clean.
- **Unresolved failures:** none.

### Repair re-review

DONE — rust-code-reviewer agent (independent of the repair implementation), scope `8e378ad0..be0d3767` with this log and design.md §2 as context; it ran `cargo clippy --workspace --all-targets -- -D warnings` (clean) and `cargo test -p cyril-core -p cyril-ui -p cyril` (0 failures). Verdict: **nothing blocking**. It confirmed the ack-gated chain only fails *closed* (every ordering hole yields no effort step, none a wrong-model set; F6's `accepts` current-model check still guards a double model confirm), that F4/F5/F6/F7, the read-back messages, the byte-identical refusal strings and the v2 Agent path survive, that R7's retirement is sound, and that the new types remove the named illegal states and switches with no new `#[allow]`/`let _ =`/sentinel. Its three non-blocking findings are round 5 below.

## Review round 5 — repair re-review findings

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F23 | The session's set-failure arm claims a failure can only belong to the pending request; `/thinking`'s KAS lever and the stale-session replacement (`domain_mediator/mod.rs` `ForSession`) emit the same label, and a newer request overwrites the single slot — each silently drops the chain for a model switch that succeeded. Tag requests or key the slot | rust-code-reviewer | Verified | Code reading `domain_mediator/commands/mod.rs:74-80` (SetThinking → `set_config_option`), `domain_mediator/mod.rs` stale replacement, `session.rs` single `Option` slot | Modify | R13: comment corrected to state the shared label and the fail-safe choice; behavior kept | non-blocking — fails closed (no effort step; `/effort` is the fallback), never a wrong set; exact matching would need failure tagging in the mediator, a protocol change disproportionate to a convenience step — permanent design choice, no tracker issue |
| F24 | The ack can land while the user is typing; the effort step then takes the keyboard (typed characters filter it, Enter confirms a level) — new with F9 | rust-code-reviewer | Verified | Red test `effort_step_never_steals_keystrokes_from_the_input` at `be0d3767` panicked at `app.rs:8984` (step opened over typed input) | Accept | R13: `open_effort_chain` also drops the step when the input buffer is non-empty | non-blocking — UX hazard, but fixed because a stray Enter would send a level the user did not pick |
| F25 | `model_set_that_did_not_take_effect_prepares_no_chain`: the "unrelated `BridgeError` keeps the request" step is masked by the following `SessionCreated`; `BridgeDisconnected` clearing untested | rust-code-reviewer | Verified | Code reading of the test; mutation (clear on any `BridgeError`) passed the old test | Accept | R13: assert the request survives an unrelated failure (the next ack opens the chain); loop both boundaries | non-blocking — test gap |

### Repair

#### R13 — typing guard, honest failure comment, tighter boundary test (F23, F24, F25)

- **Delivered behavior:** `App::open_effort_chain` drops the post-ack step when `has_modal_overlay()` OR the input buffer is non-empty; session set-failure comment rewritten (no behavior change); session test tightened. Commit `980f0c51`.
- **Performed checks and results:**
  - F24 sensitivity: `cargo test -p cyril --bin cyril app::tests::effort_step_never_steals_keystrokes_from_the_input -- --exact` RED at `be0d3767` (panic `app.rs:8984`), GREEN at `980f0c51`.
  - F25 sensitivity: with the session arm mutated to `Notification::BridgeError { .. } =>` the tightened test FAILED (panic `session.rs:1820`); restored from a byte copy (not git), GREEN.
  - Gate at `980f0c51`: nextest kas lane 1448, workspace all-features 2337, core no-default 863 passed; all four clippy lanes clean; fmt clean.
- **Unresolved failures:** none. Scoped re-review: see below.

### R13 scoped re-review

DONE — same rust-code-reviewer agent, scope `be0d3767..980f0c51`; targeted tests pass at `980f0c51`. Verdict: **nothing blocking, no new findings.** Input guard has no false drops: `submit_input` empties the buffer via `take_input()` before `/model` runs, there is no draft-restore path, and autocomplete only shows while the user is typing — so a non-empty buffer at ack time means typed, pasted or voice-transcribed text arrived after the confirm. Key routing is complete: the picker receives keys only as `topmost_overlay()`'s `Overlay::Picker`, and paste/voice inserts are gated on `has_modal_overlay()`. F23's comment is now accurate; F25's test discriminates.

**Round status:** every finding F9–F25 has one evidence state and decision; every behavior-changing repair has sensitivity receipts, a passing gate and independent review; consumer smoke PASS. No blocking defect or required check outstanding.

### Final integration

- Assembled-branch repository gate: PASS at `980f0c51` (R13 record above; the lanes cover every crate).
- Consumer smoke: PASS — 2026-10-17, kiro-cli 2.32.0 (IAM Identity Center login), cyril built at `be0d3767` with `--features kas` (R13 only adds a guard for the typing case, which the smoke does not exercise), driven by tmux keystrokes under an isolated `HOME` with the real `XDG_DATA_HOME`.
  - KAS session: toolbar `vibe · auto` from the initial catalog. `/model` opened with `auto ✓` preselected. Confirming `claude-sonnet-5.5`: a capture taken immediately after Enter showed no picker; then "Model set to claude-sonnet-5.5." followed by the effort step `low/medium/high ✓/xhigh/max`, toolbar `◇ high` (F9 + C8 live). Confirmed `max` → "Effort set to max.", toolbar `◇ max`. `/effort` reopened with `max ✓` (F19 live; C13 showed `high✓` here). `/model` → `auto` → "Model set to auto.", effort chip cleared, no step opened. `/mode` opened with `Default ✓`; Plan → "Mode set to plan.", toolbar `plan`.
  - v2 session (default engine): `/model` opened the kiro.dev picker with credit badges and the `[active]` marker — unchanged (C12).
  - Not observed on the wire: this run wrote no `cyril.log` in its working directory, so the model-before-effort frame order rests on the pre-ack capture plus `effort_step_after_the_ack_sends_model_then_effort`.
  - Noted, not a defect of this change: kiro-cli 2.32.0 has no wire audit yet (latest is 2.31.1).
