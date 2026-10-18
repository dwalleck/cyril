# Independent review — cyril-vh69: KAS /model picker with per-model effort, plus /effort and /mode

Reviewer: independent (gilfoyle contract). Scope: `main..HEAD` on `feature/cyril-vh69-kas-model-picker`, commits baf0981f..a90ba0ac. Contract: `.cyril-vh69/design.md` + `.cyril-vh69/plan.md`, claims C1–C13.

## Verdict: changes-requested

One high-severity defect in the headline acceptance flow (effort confirm is revalidated against the wrong source of truth and can refuse spuriously — possibly always), plus a test gap that masks it. Everything else — gating, read-back, effort re-derive, v2 fence, conversion hardening, repo invariants — is faithful to the approved design and well tested.

## Findings (ordered by severity)

### 1. HIGH — Effort confirm revalidates against a catalog option that may not exist

`crates/cyril/src/app.rs:2071-2072` (`confirm_config_picker`), via `KasConfigSnapshot::contains_value` (`crates/cyril-core/src/types/kas_config.rs:71`).

**What:** Confirm-time revalidation checks `contains_value(config_id, value)` — membership in the catalog's select option *named* `config_id`. For `config_id == "effortLevel"` that means an `effortLevel` select option in the latest snapshot. But the effort picker rows are not sourced from an `effortLevel` option — they come from the model value's `_meta.kiro` `EffortMetadata.levels` (`effort_picker_options`, kas_config.rs:122). I checked the wire oracle: in `experiments/conductor-spike/audit-2.31.1/live/kas-live-sweep-2.31.1.jsonl`, **no catalog or `set_config_option` result ever contains an `effortLevel` option id** (frames 8/24/25/31/125/151/152/160/161 — ids are always `mode, model, memoryReflection, autopilot, contentCollection` ± `outputStyle`). The string `effortLevel` appears exactly once in the whole capture, inside a workflow recipe, not the config catalog.

**Why it matters:** Two failure modes, one deterministic, one likely-total:

- *Deterministic race:* in the chained `/model` → effort flow (the acceptance-criteria flow), the effort picker opens immediately after the model set is dispatched, while `SessionController`'s snapshot is still the **pre-switch** catalog. Switching from `auto` (the captured currentValue), that catalog provably has no `effortLevel` option. A user who confirms an effort level before the model-set ack round-trips to AWS gets the refusal "The options changed while the picker was open — reopen and try again." and nothing is sent. The message is also wrong for this case — the options didn't change; the value was never in that catalog to begin with.
- *Likely total:* if KAS never surfaces `effortLevel` as a select option (the capture suggests it does not — the design's "the effortLevel option appears post-switch, confirmed live" is not supported by any capture in the repo; the sweep only ever set `model` to `bogus-model-zz9`), then **every** effort confirm, chained or standalone `/effort`, is refused. The `/effort` command would open a picker whose every value fails revalidation.

Note the asymmetry the design itself established: KAS accepts an invalid effort level silently (SUCCESS, unchanged `currentValue`), and the read-back arm already handles that honestly. Refusing to *send* a valid level because membership was checked against the wrong source is strictly worse than sending and reading back.

**Suggested fix:** Revalidate effort confirms against the same source the rows came from — the model's `EffortMetadata`. E.g. in `confirm_config_picker`, when `config_id == EFFORT_CONFIG_ID`, check `snapshot.effort_for_model(model).is_some_and(|m| m.levels().contains(&value))` for the applicable model (the chosen model in the chain, the snapshot's `current_model()` for standalone `/effort` — the picker may need to carry the model id it was opened for, since the chained picker outlives the pre-switch snapshot). Keep `contains_value` for `model`/`mode`, where the picker rows genuinely come from the catalog option. The C13 smoke should record whether a post-switch catalog ever carries an `effortLevel` option, since the capture doesn't answer it.

### 2. MEDIUM — Test gap masks finding 1: no test ever confirms the effort step

`crates/cyril/src/app.rs` tests (`model_confirm_sends_one_set_and_chains_effort_picker`, ~line 8810).

**What:** The C8 chain test confirms the model, asserts the effort picker opens with the right levels and default marked, then asserts Esc sends nothing. It never presses Enter on the effort picker. The stale-refusal test (`stale_config_confirm_is_refused_and_sends_nothing`) only exercises `config_id == "model"`. No test anywhere covers a *successful* effort confirm — exactly the path finding 1 breaks. C7's claim ("Enter → SetConfigOption{config_id, value}") is asserted for model only.

**Why it matters:** The plan's verification for slice 5 requires "confirm → exactly one `session/set_config_option` frame with the right `{configId, value}`" — for the effort step that assertion is absent, so the revalidation bug compiled green through the full gate.

**Suggested fix:** Add an App test: catalog with current model `claude-sonnet-5.5` (or post-ack snapshot), open the effort picker, Enter → exactly one `SetConfigOption { config_id: "effortLevel", value }` frame. Add the chained variant (confirm effort before any ack arrives) to pin the race behavior whichever way finding 1 is fixed.

### 3. LOW — `UiState::config_catalog` is dead production state

`crates/cyril-ui/src/state.rs:84, 1042, 1677`.

**What:** UiState wholesale-replaces a `config_catalog: Vec<ConfigOption>` on every config notification and exposes `config_catalog()` — but the only callers are its own tests. The design (§ Supporting types) said "UiState keeps its own latest catalog for the confirm-time chain data"; the implementation sources the chain data from `SessionController::config_options()` instead (app.rs:2070), which is arguably the better owner. The UI copy is write-only in production.

**Why it matters:** Duplicated state with no consumer is drift waiting to happen — a future reader will reasonably assume the App reads it (the design says so). Clippy can't catch it (pub method).

**Suggested fix:** Either remove the field + accessor + test, or add a short comment at the field saying the session snapshot is the chain-data source and this copy is retained for <named future consumer>. Removal is cleaner.

### 4. LOW — `pending_config_set` goes stale when the send fails

`crates/cyril/src/app.rs:2085-2091`, `crates/cyril-ui/src/state.rs:1686` (`note_config_set_requested`).

**What:** The request side is noted *before* `bridge_sender.send(...)`. If the send fails (`?` propagates to the App error path), the pending entry survives; the next `ConfigOptionSet` ack for that config id (from a later, unrelated set) then compares its returned value against the stale requested value and emits a spurious "returned a different value than requested" debug log.

**Why it matters:** Impact is log-only — the shown value is always the returned one regardless — so this is cosmetic. But the log exists to surface KAS's silent-ignore behavior, and a false positive undermines trust in it.

**Suggested fix:** Note after a successful send, or clear the pending entry on send failure.

### 5. NIT — Config picker always preselects row 0, not the current value

`crates/cyril-ui/src/state.rs:2620` (`show_config_picker`).

**What:** `selected: 0` unconditionally. For `/model`, the current model is marked with `✓` but the cursor starts on the first row (usually `auto`). The acceptance criteria only require "default marked", which is honored, so this is UX polish, not a defect — but preselecting the `is_current` row would make the common "confirm what I already have / see where I am" interaction one keystroke shorter and matches what most pickers do.

## Scope vs plan

- **Slice 1 (types + conversion, C2):** delivered as planned. `ConfigOptionValue`/`EffortMetadata` in `types/command.rs`, `KasConfigSnapshot` in new `types/kas_config.rs` (design-permitted), `_meta.kiro` parsing confined to `convert/kas.rs` with `convert/mod.rs` delegating through a feature-gated `select_option_effort` shim — the non-kas build converts effort-free by construction. Caller migrations cover `thinking.rs`, `session_configuration.rs`, `cyril-workbench/reviewer.rs` (the `.value` field accesses — planned migration, not scope creep), and UI test fixtures (`render.rs`, `floor_tests.rs`, `picker_viewport.rs`, `modal_theme.rs` — mechanical `config_id: None` additions).
- **Slice 2 (session snapshot, C3):** delivered. Wholesale replace on both config notifications, `None` on `SessionCreated`/`BridgeDisconnected`, lifecycle test covers replace/replace/ignore/reset/reset.
- **Slice 3 (commands, C1/C4–C6):** delivered. `ConfigCommandSource` resolved from `AgentEngine` at registry construction; `with_builtins` pins `Agent` (v2 shape); names pushed before `HelpCommand`'s eager copy (the documented trap); refusal matrix matches the design table cell-for-cell including message wording; no engine branching inside `execute`; no `BridgeCommand::SetModel` anywhere (grepped).
- **Slice 4 (cyril-ui, C9–C11):** delivered. `PickerKind::Config` + `config_id` + `PickerConfirmation`; mode/effort feeds ordered after the `set_current_model` clear; read-back acks from `ConfigOptionSet` only; toolbar render test drives a real `UiState` through a catalog sequence. No `acp::` import in cyril-ui (the shim keeps `_meta` parsing in cyril-core; UI sees only domain types). One deviation: the design put the chain-data catalog in UiState; the implementation put it in SessionController only, leaving the UiState copy dead (finding 3).
- **Slice 5 (App, C7/C8):** delivered with the finding-1 defect and finding-2 gap. Registry wiring at `app.rs:420` passes `ConfigCommandSource::resolve(agent_engine)`. `ShowConfigPicker` handling is pure dispatch. v2 fence test (`slash_model_on_v2_still_queries_agent_options`) confirms the v2 path end-to-end at the App layer.
- **Slice 6 (final gate):** `cargo test --workspace --all-features`, `cargo clippy --workspace --all-targets -- -D warnings` (both default and `--all-features`), and `cargo fmt --check` all green on a90ba0ac — I ran these, not just trusted the report. C13 live smoke is pending per plan (user/implementer, post-build).
- **Nothing in the diff outside the plan.** The three `.cyril-vh69/` artifacts are the planned baf0981f commit.

## Claim-by-claim spot check (requested)

- **Frame-24 replay (C2, two tests):** genuinely assertive — pins the frame position with a `sessionUpdate == "config_option_update"` guard (fails loudly if the capture file shifts), deserializes through the real acp layer, asserts all 21 model values by id, per-cell effort outcomes (full block / `hasEffort:false` absence / GPT's `none` level verbatim), and mode labels/descriptions. The malformed-cell test covers wrong-typed levels, non-string entries, empty strings, out-of-list default, wrong-typed default. This is a real oracle, not code exercise.
- **Refusal matrix (C4–C6):** every design-table cell is asserted with exact message wording *and* zero bridge sends (the `run` helper drains the channel). The no-session case asserts `ErrorKind::NoSession`. Real assertions.
- **Read-back / bogus-effort (C10):** replays the 0.66.34 shape (request `none`, ack `medium`), asserts chip *and* message show `medium`, and that the requested value appears nowhere. Real.
- **Effort re-derive (C9):** auto→claude (cleared until level arrives), claude→claude (old level cleared), claude→auto (cleared, no repopulation), unknown level → `Other("turbo")` verbatim. Matches the design's sequence list exactly.
- **Chain (C7/C8):** model confirm → one frame, picker opens, Esc → silent — all asserted. The missing half (effort confirm) is finding 2.

## What I verified vs did not check

Verified by reading: the full diff of all 25 changed files; the design and plan; the 2.31.1 capture (frame-24 shape, all catalog/result frames' option ids, all `set_config_option` requests — including frame 159/160 confirming KAS accepts `bogus-model-zz9` and reports it back as `currentValue`, which validates the design's client-side-validation premise). Verified by running: `cargo check --workspace --all-features`, `cargo clippy --workspace --all-targets -- -D warnings` (default and all-features), `cargo test --workspace --all-features` (all green, ~2000 tests), `cargo fmt --check`.

Not checked: live smoke against kiro-cli 2.31.1 (C13 — plan assigns it to user/implementer; finding 1 should be re-confirmed there, including whether a post-switch catalog ever carries an `effortLevel` select option); the Windows CI leg; `cyril-memory`/`cyril-review` crates (untouched by the diff); rendering of the picker widget beyond the existing render tests.
