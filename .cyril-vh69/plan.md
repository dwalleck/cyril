# plan.md — cyril-vh69: KAS /model picker with per-model effort, plus /effort and /mode

Budgeted-plan stage artifact. Consumes `.cyril-vh69/route.md` (Structural) and the approved `.cyril-vh69/design.md` (user approval 2026-10-15). Design §Claims/Falsification defines the checks; each slice inherits its claims by reference. One delivery increment (single feature branch, single PR).

No requester- or repository-supplied feature budget exists → no Size stop arithmetic applies.

## Slice 1: Domain types + conversion — `ConfigOptionValue`, `EffortMetadata`, `KasConfigSnapshot`, `_meta.kiro` parsing

**Outcome:** `ConfigOption.options` carries per-option labels/descriptions and KAS effort metadata end-to-end from the wire; malformed `_meta.kiro` degrades to `effort: None` with a warning, never a catalog failure. (Claims: C2)
**Changes:** `crates/cyril-core/src/types/command.rs` — `ConfigOptionValue { value, name, description, effort: Option<EffortMetadata> }`, `EffortMetadata { levels, default_level }` (non-empty levels invariant; `has_effort` derived; out-of-list default → `None` + warn), `KasConfigSnapshot` + pure query helpers (`option`, `current_value`, `picker_options`, `effort_for_model`, `contains_value`, `current_model`) and const ids `MODEL_CONFIG_ID`/`MODE_CONFIG_ID`/`EFFORT_CONFIG_ID` (placement: design §Placement row 1; new module `types/kas_config.rs` permitted by design). `crates/cyril-core/src/protocol/convert/kas.rs` — `_meta.kiro` effort parse helper, called from `convert/mod.rs::to_config_options` (KAS quirks confined to the KAS converter). Caller migration of `Vec<String>` → `Vec<ConfigOptionValue>` everywhere `ConfigOption.options` is consumed: `ThinkingState::from_config_options` (key/value only — untouched semantics), UiState/SessionController arms, test fixtures in `command.rs`, `test_support.rs`, `session_configuration.rs`.
**Verification:**
- Conversion test replaying capture frame 24 (`experiments/conductor-spike/audit-2.31.1/live/kas-live-sweep-2.31.1.jsonl`) through the acp layer + `to_config_options` (pattern: `convert/mod.rs:2749-2756`, `test_support.rs:466-472`) → 21-option model catalog; per-cell effort outcomes: full block (claude-*), absent (`auto`), `hasEffort:true` with missing/empty levels → `None` + warn, default ∉ levels → `None` + warn, malformed types → `None` + warn
- `cargo test -p cyril-core` and `cargo clippy -- -D warnings` green with all existing fixtures migrated

## Slice 2: `SessionController` config-option snapshot state

**Outcome:** The session holds the latest KAS config catalog as `Option<KasConfigSnapshot>`, replaced wholesale per notification and reset on session lifecycle boundaries. (Claims: C3)
**Changes:** `crates/cyril-core/src/session.rs` — `config_options: Option<KasConfigSnapshot>` field + accessor; `apply_notification` replaces on `ConfigOptionsUpdated`/`ConfigOptionSet`, resets to `None` on `SessionCreated`/`BridgeDisconnected`; no merge semantics, no async, no bridge access (design §Placement row 3).
**Verification:**
- State lifecycle tests on `SessionController::apply_notification` (ThinkingState `apply_notification_rules` pattern): replace-on-update, replace-on-set, reset-on-created, reset-on-disconnect, unrelated notifications ignored
- `cargo test -p cyril-core` green
**Dependencies:** Slice 1 (snapshot type).

## Slice 3: `ConfigCommandSource` + `/model` `/effort` `/mode` builtins + `ShowConfigPicker`

**Outcome:** On a KAS-built registry the three commands parse as builtins and return `CommandResultKind::ShowConfigPicker` with validated, snapshot-derived options; on v2 nothing registers and agent commands keep the names; `/help` lists them on KAS only. Commands never send bridge commands themselves. (Claims: C1, C4, C5, C6, and C12's v2-registry assertion)
**Changes:** `crates/cyril-core/src/commands/mod.rs` — `ConfigCommandSource` enum resolved from `AgentEngine` (4th source, beside Hooks/Workflow/Usage), registration in `with_builtin_sources` with names pushed into the `/help` snapshot before `HelpCommand` is built; `CommandResultKind::ShowConfigPicker { config_id, title, options }`. `crates/cyril-core/src/commands/builtin.rs` — `ModelCommand`/`EffortCommand`/`ModeCommand` with the design's refusal matrix (no session → `ErrorKind::NoSession`; no snapshot → "haven't been reported yet"; option absent → "not offered by this session"; `/effort` on effort-less model → refusal message; args → usage line). No `BridgeCommand::SetModel` emission anywhere (design §Placement row 4 forbids).
**Verification:**
- Registry unit tests mirroring `thinking_command_registered_on_every_engine` (`commands/mod.rs:1953-1970`): KAS registry parses `/model` as builtin; v2 registry → `None`; `register_agent_commands` on v2 still lands `model`; `/help` contains the names on KAS only
- Command tests in the `thinking_command_tests` style (`commands/mod.rs:1696-1970`): per command, per refusal-matrix cell; `/model` happy path → 21 options with `is_current` on `currentValue`; `/effort` → exactly the current model's levels with `is_current` on `defaultEffortLevel` (auto/claude-sonnet-5.5/claude-opus-5.5 cells from frame 24); `/mode` → wire labels/descriptions
- `cargo test -p cyril-core` + `cargo clippy -- -D warnings` green
**Dependencies:** Slices 1–2 (snapshot type + session accessor).

## Slice 4: cyril-ui picker plumbing + UiState read-back / effort re-derive / toolbar feed

**Outcome:** The picker overlay supports `PickerKind::Config` carrying a `config_id`; UiState tracks the latest catalog, feeds model/mode/effort toolbar chips purely from returned `currentValue`s, posts set confirmations from `ConfigOptionSet`, and never carries effort across a model switch. (Claims: C9, C10, C11)
**Changes:** `crates/cyril-ui/src/traits.rs` — `PickerKind::Config`, `PickerState.config_id: Option<String>` (`Some` iff `Config`), `picker_confirm` grows to return `PickerConfirmation { kind, title, value, config_id }`. `crates/cyril-ui/src/state.rs` — `show_config_picker(config_id, title, options)` (honors `is_current` as sent; no `active_option_value` join); latest-catalog field for chain data; `ConfigOptionsUpdated | ConfigOptionSet` arm gains `mode` → `current_mode` feed, `effortLevel` → `effort` via `EffortLevel::from_wire(currentValue)` (repopulation only after the existing `set_current_model` clear, `state.rs:1606-1612`), and `Model/Mode/Effort set to <currentValue>` confirmation messages mirroring the thinking-ack pattern (`state.rs:1031-1043`); `debug!` when returned value ≠ requested. No optimistic application of requested values (design §Placement row 6 forbids).
**Verification:**
- UiState state lifecycle tests: auto→claude (effort `None` until `effortLevel.currentValue` arrives), claude→claude (old level cleared), claude→auto (cleared, no repopulation), unknown level → `EffortLevel::Other`; bogus-effort replay (`ConfigOptionSet` with unchanged `currentValue:"medium"` after requesting `"none"`) → chip + message show `medium`
- Render tests on `TestBackend`: toolbar mode/model/effort chip text after a KAS catalog sequence (MockTuiState pattern)
- `cargo test -p cyril-ui` + `cargo clippy -- -D warnings` green
**Dependencies:** Slice 1 (domain types). Independent of Slices 2–3 (different crate, non-overlapping files).

## Slice 5: App chain + confirm dispatch + registry wiring

Amended in place 2026-10-17 (review round 4, F9/F10/F15 — design.md §2): the effort step is gated on the model ack and owned by `SessionController`; delivered as bounded repair R10 (`f936ca13`), with R13's typing guard (`980f0c51`).

**Outcome:** Confirming a `PickerKind::Config(target)` entry revalidates via `KasConfigSnapshot::accepts` against the latest snapshot and sends `BridgeCommand::SetConfigOption` (never `ExecuteCommand`, never `SetModel`). The effort picker opens only when the model set's `ConfigOptionSet` read-back arrives with the requested model current and carrying effort metadata, built from the rebuilt catalog; Esc on the effort step sends nothing further. The registry is built with `ConfigCommandSource::resolve(agent_engine)`. (Claims: C7, C8)
**Changes:** `crates/cyril/src/app.rs` — `handle_picker_key` Enter arm for `PickerKind::Config(target)` → `confirm_config_picker(target, value)`: `accepts` (stale → "options changed while the picker was open" message, nothing sent, `warn!`), send `KasConfigRequest::to_command()`, then `SessionController::note_config_set_requested`. `open_effort_chain` runs after routing a `ConfigOptionSet`, takes `SessionController::take_effort_chain()`, and drops it when a modal overlay is open or the input buffer is non-empty. `ShowConfigPicker(ConfigPicker)` command-result handling → `UiState::show_config_picker`. Registry composed in `app.rs` via `with_builtin_sources` with `ConfigCommandSource::resolve(agent_engine)`. Composition only; the accept rule and the chain live in `KasConfigSnapshot` / `SessionController` (design §Placement).
**Verification:**
- Bridge-harness integration test with scripted `new_session_config_options` + scripted `session/set_config_option` results: confirm → exactly one `session/set_config_option` frame with the right `{configId, value}`; scripted `session/new` configOptions land as the snapshot
- App-handler tests: no effort step before the ack, effort step after it (model then effort frames, in order), no step for `hasEffort:false` / a failed set / an open overlay / typed input, stale-picker refusal, standalone effort refused after the model switches away
- `cargo test` (workspace) + `cargo clippy -- -D warnings` green
**Dependencies:** Slices 3–4.

## Slice 6 (final gate): full assembled verification + live smoke

**Outcome:** The assembled branch satisfies the ticket acceptance criteria; v2 is unchanged end-to-end. (Claims: C12 full gate, C13)
**Changes:** none (verification only; fixes route back as bounded repairs).
**Verification:**
- Full `cargo test` workspace + `cargo clippy -- -D warnings` + `cargo fmt --check` green on the branch head
- v2 end-to-end: existing v2 command/picker tests unmodified and green; `/model` with no args still sends `QueryCommandOptions`, confirm still sends `ExecuteCommand {value}`
- Manual live smoke against kiro-cli 2.31.1: `cargo run` on a KAS session — `/model` chains to effort, `/effort` and `/mode` work, toolbar follows returned values, a bogus-effort attempt shows the unchanged `currentValue`; then a v2 session — `/model` unchanged. Checklist = ticket acceptance criteria. (User/implementer; recorded with binary version and date)
**Dependencies:** Slices 1–5.

## Tracker taxonomy

- Absorbed: **cyril-838u** (effort re-derive) — close as covered when this ships.
- Stays open: **cyril-cxwb**, **cyril-lxuo**, **cyril-v2ol** (design §Non-goals).
- Permanent non-goals (design): arg forms, autopilot/memoryReflection/contentCollection commands, KAS `set_mode`, any v2 change.

## Hand-off

checkpointed-build judges each slice's delivered behavior and performed verification; one checkpoint per completed slice, never per unit-test cycle. C13's live smoke and the independent review complete the Structural terminal criterion in `route.md`.
