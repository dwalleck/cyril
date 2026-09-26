# Plan: cyril-k3lz — thinking on/off toggle

Inputs: `route.md` (Structural), `spec.md` (signed 2026-09-25), `design.md` (approved 2026-09-25, C1 PASS, no FAIL rows).

Technical corrections to design fence paths (Approval semantics — location only, oracle/mutation unchanged): `builtin.rs` has no test module; builtin command tests live in `crates/cyril-core/src/commands/mod.rs` `tests`, so C4–C7/C12 fences are `commands::tests::thinking_*`. The C8/C9 fences live in the in-process bridge harness module `protocol::bridge::tests::current_runtime_contract::thinking` (default build) — the harness gains a scripted ext/standard response hook.

## Module growth ledger

| Module | Baseline production lines | Projected final lines | Responsibility change | Interface change | Protected-parent rule |
|---|---:|---:|---|---|---|
| `crates/cyril-core/src/types/thinking.rs` | 0 | 130–190 | create: thinking-state derivation | `ThinkingState`, `ThinkingLever`, `THINKING_CONFIG_ID` | N/A |
| `crates/cyril-core/src/types/mod.rs` | 92 | 94–96 | none | re-export | N/A |
| `crates/cyril-core/src/types/event.rs` | 689 | 700–715 | none | +`Notification::ThinkingToggled`, +`BridgeCommand::SetThinking` | N/A |
| `crates/cyril-core/src/session.rs` | 400 | 410–420 | none (holds state) | +`thinking()` | S2 rule of shape.py |
| `crates/cyril-core/src/commands/builtin.rs` | 505 | 580–640 | add `/thinking` | +`ThinkingCommand` | S2/S3 rules of shape.py |
| `crates/cyril-core/src/commands/mod.rs` | 512 | 514–518 | none (registration) | none | N/A |
| `crates/cyril-core/src/protocol/domain_mediator/commands/mod.rs` | 185 | 188–195 | none (dispatch arm) | none | N/A |
| `crates/cyril-core/src/protocol/domain_mediator/commands/extensions.rs` | 220 | 260–290 | add v2 thinking call + ack mapping | `set_reasoning_thinking` (pub(super)) | N/A |
| `crates/cyril-ui/src/state.rs` | 2805 | 2825–2845 | none (holds state, two message arms) | `TuiState::thinking_enabled` impl | S2 rule of shape.py |
| `crates/cyril-ui/src/traits.rs` | 777 | 781–786 | none | +`thinking_enabled()` | N/A |
| `crates/cyril-ui/src/widgets/toolbar.rs` | 308 | 318–326 | add segment | none | N/A |
| `crates/cyril/src/app.rs` | 2902 | 2902 | none | none | protected: zero delta (S1) |
| `crates/cyril/examples/test_bridge.rs` | 729 | 729–733 | print arm if the match is exhaustive | none | N/A |

## Partition arithmetic

Slice diff estimates: 350 + 450 + 450 + 350 = 1,600 lines (impl + tests + fixtures; the two fixtures, 57 KB compact + 4 KB, count as ~15 lines). Churn margin 30 % (+480) — the mediator harness extension and exhaustive-match arms are the likeliest to grow. Total 2,080 ≤ 4,000 → **one increment**: `pr-1` (all slices), mergeable against the default branch (discovered via `origin/HEAD`), verified by `cargo test --workspace` (default and `--features kas`), clippy, fmt, and the shape oracle.

## Slice 1: engine-neutral thinking-state model

**Claim IDs:** C2, C3 (C1 already PASS; its fence lands with this slice's commit)
**Expected behavior:** `ThinkingState::from_reasoning` / `from_config_options` / `apply_notification` produce the spec's state for every I1–I7 cell.
**Oracle:** spec "Thinking state" definitions and B8 transcribed as hand-written expected tables.
**Stress fixture:** Toggleable with `thinking_enabled: None` (must stay unknown, not default on); `Other("futureValue")` support with `Some(true)` (must be not-toggleable); configOptions with `thinking` value `"bogus"` and value `None` (must be not-toggleable, never on); `MetadataUpdated{reasoning: None}` after a toggleable-off state (must stay off). Expected outcomes are as listed.
**Regression fence:** `types::thinking::tests::from_reasoning_matrix`, `from_config_options_matrix`, `apply_notification_rules`; plus C1 `protocol::convert::tests::kas_thinking_option_survives_config_conversion_per_captured_step`.
**Named mutation:** C2 — `from_reasoning`: `enabled: info.thinking_enabled()` → `enabled: Some(info.thinking_enabled().unwrap_or(true))` → red at toggleable/None row. C3 — `apply_notification`: `MetadataUpdated{reasoning: None}` → set `Unreported` → red at "absent block keeps state". C1 — `to_config_options` `.filter(|o| o.id.to_string() != "thinking")` → red at `cfg_model`.
**Complexity/production scale:** `from_config_options` is one linear scan over config options (≤ ~5 top-level options live; model choices are nested, not scanned) — O(n), n ≤ 10, max accepted cost: one pass per snapshot (snapshots arrive ≤ a few per turn).
**Wall budget/phase:** N/A — reason: no runtime phase yet (type only; wired in slices 3–4).
**Module shape:** create `types/thinking.rs` owning derivation; interface `ThinkingState`, `ThinkingLever`, `THINKING_CONFIG_ID`; protected parents untouched (0 delta); `python .cyril-k3lz/oracles/shape.py` → `C13 PASS`.
**Files:** `crates/cyril-core/src/types/thinking.rs` (new), `crates/cyril-core/src/types/mod.rs`, `crates/cyril-core/src/protocol/convert/mod.rs` (C1 test, done), fixtures under `crates/cyril-core/tests/fixtures/{kas,v2}/thinking/` (done).
**Estimate:** 45 min
**Diff estimate:** 350
**PR increment:** pr-1
**Commands and expected results:**
- `cargo test -p cyril-core --lib types::thinking` → every matrix row matches the spec table.
- `cargo test -p cyril-core --lib kas_thinking_option_survives` → on/on/off/on/off/absent per step.
- each named mutation applied → the named row fails with its step/cell in the message; restored → green.
- `cargo clippy -p cyril-core --all-targets -- -D warnings` → no warnings; `python .cyril-k3lz/oracles/shape.py` → PASS.

## Slice 2: typed toggle request through the domain mediator

**Claim IDs:** C8, C9, C10 (the `ThinkingToggled` half — the variant's UiState arm must land with the variant)
**Expected behavior:** `BridgeCommand::SetThinking{ReasoningCommand, enabled}` sends exactly one `_kiro.dev/commands/execute` with `{sessionId, command:{command:"reasoning", args:{thinkingEnabled}}}` and yields `ThinkingToggled{enabled}` on `success:true`, else `BridgeError{operation:"Thinking change", message}`; `SetThinking{ConfigOption, enabled}` sends one `session/set_config_option {configId:"thinking", value:"on"|"off"}` and yields `ConfigOptionSet{config_id:"thinking"}`; `UiState` shows `Thinking turned on.`/`off.` on `ThinkingToggled`.
**Oracle:** P2 captured request/response shape (`v2-reasoning-args2` lines 25/29 responses) and P1 fixture step `cfg_thinking_off`; spec B2 strings; I11 message rule written in the test.
**Stress fixture:** scripted v2 responses: `{success:true,message:""}` → toggled; `{success:false,error:"nope"}` → "Thinking change failed: nope"; `{success:false,message:"denied"}` → "…: denied"; `{success:false}` → "…: unknown error"; `{}` (missing success) → failure "response missing success"; RPC error → failure with its text. KAS scripted result: fixture `cfg_thinking_off` configOptions → `ConfigOptionSet` whose thinking value is `off`.
**Regression fence:** `protocol::bridge::tests::current_runtime_contract::thinking::set_thinking_reasoning_wire_and_ack`, `::set_thinking_config_option_wire_and_ack`; `cyril-ui` `state::tests::thinking_toggled_message`; C5 contract cell for `SetThinking` in `c5_every_bridge_command_has_an_explicit_current_runtime_outcome`.
**Named mutation:** C8 — add `"setAsDefault": false` to the args → red (args key set); treat missing `success` as ok → red at that row. C9 — send `config_value(!enabled)` → red on value. C10a — swap on/off message text → red.
**Complexity/production scale:** N/A — reason: no loop; one request per command.
**Wall budget/phase:** N/A — reason: one-off phase (per operator command); no wall budget. Response wait bounded by existing `COMMAND_RPC_TIMEOUT` (10 s).
**Module shape:** mediator gains the v2 thinking call (extensions.rs) and one dispatch arm; `event.rs` +2 declarations; `state.rs` +1 message arm; `app.rs` 0 delta; `python .cyril-k3lz/oracles/shape.py` → PASS.
**Files:** `crates/cyril-core/src/types/event.rs`, `crates/cyril-core/src/protocol/domain_mediator/commands/{mod,extensions}.rs`, `crates/cyril-core/src/protocol/bridge/tests/harness.rs`, `crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/{mod,commands,thinking}.rs`, `crates/cyril-ui/src/state.rs`, `crates/cyril/examples/test_bridge.rs` (if exhaustive).
**Estimate:** 90 min
**Diff estimate:** 450
**PR increment:** pr-1
**Commands and expected results:**
- `cargo test -p cyril-core --lib current_runtime_contract` → every I11 row maps to its expected notification; KAS arm sends `thinking`/`off`.
- `cargo test -p cyril-core --features kas --lib` → compiles and passes (kas build has the `SetThinking` arm too).
- `cargo test -p cyril-ui --lib thinking_toggled_message` → exactly one message with the spec text.
- mutations → red at the named row; restored → green. Shape oracle → PASS.

## Slice 3: `/thinking` command and session-side state

**Claim IDs:** C3a (core half), C4, C5, C6, C7, C12
**Expected behavior:** `SessionController::thinking()` follows the captured v2 and KAS sequences; bare `/thinking` reports the B1 text per state with zero sends; refusals (B4), usage (B5), no-session (B6) send nothing; toggleable states send exactly one `SetThinking` with the state's lever; `/thinking` registered on both engines and in `/help`.
**Oracle:** capture-line expected lists (v2: line 8 toggleable/unknown, 24 on, 26 on, 28 on, 30 off; KAS: session_new not-toggleable, cfg_model on, cfg_effort_max on, cfg_thinking_off off, cfg_effort_max2 on, cfg_thinking_bogus off, cfg_model_gpt not-toggleable); spec B1/B4/B5/B6 strings.
**Stress fixture:** the two captured sequences replayed through the real converters (`kiro::to_ext_notification` for v2 metadata, `to_config_options` → `ConfigOptionsUpdated` for KAS); args `"ON"`, `"  off "`, `"maybe"`, `"on off"`; AlwaysOn with both `on` and `off`.
**Regression fence:** `crates/cyril-core/tests/thinking_capture_replay.rs::session_controller_follows_captured_{v2,kas}_sequence`; `commands::tests::thinking_report_messages`, `thinking_refuses_without_toggleable_support`, `thinking_args_and_no_session`, `thinking_sends_one_typed_toggle`, `thinking_command_registered_on_every_engine`; existing `help_lists_every_registered_command`.
**Named mutation:** C3a — drop `self.thinking.apply_notification(n)` from `SessionController` → red at first frame. C4 — swap AlwaysOn/NotToggleable report texts → red. C5 — treat AlwaysOn as toggleable-by-reasoning → red (a send appears). C6 — drop `to_ascii_lowercase` → red at `ON`; check session after sending → red at no-session. C7 — hard-code `ThinkingLever::ReasoningCommand` → red at KAS row. C12 — register `ThinkingCommand` without pushing `"thinking"` into `names` → `help_lists_every_registered_command` red.
**Complexity/production scale:** N/A — reason: no new loop beyond slice 1's scan.
**Wall budget/phase:** N/A — reason: one-off phase (per command / per snapshot); no wall budget.
**Module shape:** `builtin.rs` gains `ThinkingCommand` (no wire strings, no engine kind); `session.rs` +field/accessor/delegate only; `commands/mod.rs` registration only; `app.rs` 0 delta; shape oracle → PASS.
**Files:** `crates/cyril-core/src/session.rs`, `crates/cyril-core/src/commands/{builtin,mod}.rs`, `crates/cyril-core/tests/thinking_capture_replay.rs` (new).
**Estimate:** 90 min
**Diff estimate:** 450
**PR increment:** pr-1
**Commands and expected results:**
- `cargo test -p cyril-core --test thinking_capture_replay` → per-frame state equals the capture-line list.
- `cargo test -p cyril-core --lib commands::tests::thinking` → exact texts; zero sends where required; exactly one typed send where required.
- mutations → red at the named row; restored → green. Shape oracle → PASS.

## Slice 4: UI state, KAS ack message, toolbar

**Claim IDs:** C3a (UI half), C10 (`ConfigOptionSet` half), C11
**Expected behavior:** `UiState` holds the same thinking state as `SessionController` for the captured sequences; `ConfigOptionSet{config_id:"thinking"}` adds one message from the rebuilt state; other ids add none; toolbar shows ` · think` / ` · no think` exactly in toggleable-on/off.
**Oracle:** same capture-line lists as slice 3; spec B3/B7 strings.
**Stress fixture:** `ConfigOptionSet{config_id:"model"}` (must add no thinking message); rebuilt set without `thinking` after a set (→ "Thinking can't be toggled on the current model."); toolbar in toggleable-unknown (must show nothing — the on/unknown confusion bug).
**Regression fence:** `cyril-ui` `state::tests::thinking_follows_captured_sequences`, `thinking_config_ack_messages`; `widgets::toolbar::tests::thinking_segment_per_state`.
**Named mutation:** C3a — drop the UiState delegate call → red. C10b — emit the thinking message on every `ConfigOptionSet` → red at other-id row. C11 — render `think` when `thinking_enabled()` is `None` (treat unknown as on) → red at unknown row. (State `thinking_enabled()` only exposes known values; the mutation is in the widget via a `.or(Some(true))`.)
**Complexity/production scale:** N/A — reason: no new loop.
**Wall budget/phase:** toolbar render is always-on (every frame): budget = one `Option<bool>` read + ≤1 span push per frame, no allocation beyond the existing span vector; rationale: matches the effort badge's cost.
**Module shape:** `state.rs` +field/delegate/message arm/accessor; `traits.rs` +1 method (+mock); `toolbar.rs` +segment; `app.rs` 0 delta; shape oracle → PASS.
**Files:** `crates/cyril-ui/src/{state,traits}.rs`, `crates/cyril-ui/src/widgets/toolbar.rs`, `crates/cyril-ui` test fixtures via `include_str!` of the core fixtures (read-only).
**Estimate:** 60 min
**Diff estimate:** 350
**PR increment:** pr-1
**Commands and expected results:**
- `cargo test -p cyril-ui --lib thinking` → per-frame agreement with the lists; one message per ack; toolbar substrings exactly per state.
- mutations → red at the named row; restored → green. Shape oracle → PASS.
- Final: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features kas -- -D warnings`, `cargo test --workspace`, `cargo test --workspace --features kas`.

## Tracker taxonomy

Deferral phrases in this plan: none beyond the design's non-goals (already classified with cyril-lxuo, cyril-4jt7, cyril-cxwb, cyril-838u, cyril-v2ol, cyril-xdll).

## Self-review

1. Every design row C1–C13 is assigned: C1 S1 (PASS, fence commits with S1), C2/C3 S1, C8/C9/C10a S2, C3a/C4–C7/C12 S3 (C3a core), C3a-ui/C10b/C11 S4, C13 every slice. C3a and C10 are split by crate half with distinct fences — each half once. ✔
2. All fourteen fields present per slice. ✔
3. Fences created in the implementing slice; each carries its design mutation. ✔
4. Only loop: slice 1 scan (bounded); always-on phase: toolbar (budgeted). ✔
5. Growth ledger covers every touched module; app.rs protected at 0. ✔
6. Partition: 2,080 ≤ 4,000, one increment `pr-1`. ✔
7. Taxonomy applied. ✔
8. Fence assertions name the observable (exact text, exact args key set, state per frame, send count). ✔
9. No slice declared complete here. ✔


## Checkpoint records (checkpointed-build)

Environment for every record: Windows 11 arm64, toolchain 1.94.0, worktree `feat/cyril-k3lz`, base `0db49e3`. Pre-existing on this host at base (untouched files, Windows-only): `cargo clippy --all-targets` fails on unused imports in `sdk_runtime/tests/process.rs` (unix-gated tests), and `cargo clippy --features kas` fails on `kas/host_io.rs:155` unused `mut`. Lints are therefore judged per target: `cargo clippy -p <crate> -- -D warnings` (default features) must be clean, and new code must add no warnings in any target. There is no Linux toolchain here (WSL has no cargo), so the Linux and full-lint legs are CI-verified.

### Slice 1 — gate

Caller analysis: brand-new symbols (`ThinkingState`, `ThinkingLever`, `THINKING_CONFIG_ID`), no callers. `to_config_options` unchanged (test-only addition).

1. Affected unit tests: PASS — `cargo test -p cyril-core --lib types::thinking` 4 passed; `kas_thinking_option_survives…` passed.
2. Falsifiers: PASS — C2 matrix and C3 rules match the spec tables; C1 retained PASS (design run log; same source state for the converter).
3. Stress fixture: PASS — toggleable/None stays unknown, `Other` with `Some(true)` is not-toggleable, `bogus`/absent value not-toggleable, `reasoning: None` keeps state (all rows in the fences).
4. Implementation vs oracle: PASS — hand-written spec tables agree row by row.
5. Module shape: PASS — `python .cyril-k3lz/oracles/shape.py` → `C13 PASS (base 0db49e3565)`; only `types/thinking.rs` created, `types/mod.rs` re-export.
6. Budget: PASS — one linear scan per snapshot, n ≤ 10.
7. Fence: PASS — fences green.
8. Mutation: PASS — `python .cyril-k3lz/mutate.py .cyril-k3lz/mutations/slice1.json`: C1 RED ("step cfg_model: thinking option value after conversion"), C2 RED ("reasoning cell toggleable/None"), C3 RED ("metadata without reasoning keeps: state").
9. Restored: PASS — all three commands GREEN after restore.
10. Parity/reuse: PASS — searched (grep `ReasoningSupport::`, `key == "model"`, `from_wire`) for existing thinking/support mapping: none; reuses `ReasoningInfo` accessors and `ConfigOption` as-is. Wire literals `on`/`off`/`thinking` defined once here (`THINKING_CONFIG_ID`, `config_value`). No parallel path.
11. Preserved enforcement: N/A — no gate, fence, validator, or policy touched.
Lint: `cargo clippy -p cyril-core -- -D warnings` clean; `cargo fmt --check` clean.


### Slice 2 — gate

Caller analysis: new variants `BridgeCommand::SetThinking`, `Notification::ThinkingToggled`; new `DomainMediator::set_reasoning_thinking`, `reasoning_toggle_outcome`. Exhaustive-match sites found by `cargo check --workspace --all-targets` (compiler-complete for enums): `current_runtime_contract/mod.rs::command_name`, `cyril/examples/test_bridge.rs` print match, `cyril-ui/src/state.rs::apply_notification`, mediator `handle_command`. `grep BridgeCommand::SetConfigOption` confirms `saturation.rs` lists commands non-exhaustively (no update owed). Harness `Script` gained two defaulted fields; every existing `Script {..}` literal uses `..Script::default()` or `Default` (compiles).

1. Affected unit tests: PASS — `cyril-core --lib` 753 passed (default) / 986 passed (`--features kas`); `cyril-ui --lib` 605 passed.
2. Falsifiers: PASS — C8 (5 scripted responses + no-session + RPC error), C9 (captured `cfg_thinking_off`/`cfg_effort_max2` results), C10a.
3. Stress fixture: PASS — every I11 row maps as planned (`Ok(false)`, `nope`, `denied` (empty `error` skipped), `unknown error`, `response missing success`, RPC error non-empty); KAS rebuilt sets report off then on.
4. Implementation vs oracle: PASS — outbound params equal the captured request shape; ack/failure equal the spec B2 table; KAS request/ack equal P1.
5. Module shape: PASS — `C13 PASS (base 0db49e3565)`; app.rs 0 delta; wire strings only in mediator.
6. Budget: N/A — no loop; one-off phase, bounded by `COMMAND_RPC_TIMEOUT`.
7. Fence: PASS — `current_runtime_contract::thinking::{set_thinking_reasoning_wire_and_ack, set_thinking_reasoning_rpc_error_is_a_thinking_failure, set_thinking_config_option_wire_and_ack}`, `cyril-ui state::tests::thinking_toggled_message` green.
8. Mutation: PASS — `python .cyril-k3lz/mutate.py .cyril-k3lz/mutations/slice2.json`: C8-args RED ("args = {thinkingEnabled} only"), C8-missing-success RED (ack assert at the `{message}` row), C9 RED ("exactly one set_config_option per toggle with the on/off literal"), C10a RED.
9. Restored: PASS — all three commands GREEN.
10. Parity/reuse: PASS — searched (grep `spawn_extension_command`, `set_config_option`, `"kiro.dev/commands/execute"`): v2 path reuses `spawn_extension_command` + the existing `execute_command` param shape; KAS path reuses `set_config_option` (and its response validation) rather than a second copy. Symmetry vs `execute_command`: that path forwards any response as `CommandExecuted`; the new path is deliberately stricter (typed ack; missing `success` is a failure, warned) per design C8 / "errors are not default values". No-session handling mirrors `set_config_option` (BridgeError, same wording). Timeout: same `COMMAND_RPC_TIMEOUT`.
11. Preserved enforcement: PASS — harness change is additive: an empty `config_option_responses` keeps the historical method-not-found answer, and unscripted ext calls still answer `{}` (C5 ledger tests unchanged and green).
Lint: `cargo clippy -p cyril-core -p cyril-ui -- -D warnings` clean; `--features kas` only the pre-existing `host_io.rs:155`; fmt clean.


### Slice 3 — gate

Technical corrections (fence locations only; oracle and mutation meaning unchanged — Approval semantics): the capture replay is a unit test `session::thinking_tests::session_controller_follows_captured_thinking_sequences` fed by a new `test_support::thinking_capture_sequences()` (integration tests cannot reach the crate-private converters; `test_support` is the existing precedent, cf. `kas_capture_to_routed`). Command fences live in `commands::thinking_command_tests::*`. C5's mutation is applied at the lever mapping it guards (`thinking.rs::lever()` gives `AlwaysOn` a lever) — same bug class ("AlwaysOn treated as toggleable → a send appears"). The mutation runner now refuses a restored run that executed zero tests (a first run of C3a-core was a silent skip: its filter named the wrong module; caught, fixed, rerun red).

Caller analysis: `SessionController::apply_notification` body restructured (`let changed = match …; changed || thinking_changed`) — callers unchanged (`app.rs`, tests); the only early `return` (UsageUpdated size 0) cannot coincide with a thinking change. New: `SessionController::thinking`, `builtin::ThinkingCommand`, `test_support::thinking_capture_sequences`.

1. Affected unit tests: PASS — `cyril-core --lib` 759 passed (default), 992 (`--features kas`).
2. Falsifiers: PASS — C3a-core per-frame states equal the hand-read capture lists; C4–C7 texts/sends equal the spec tables; C12 registered on both registry shapes and listed by `/help`.
3. Stress fixture: PASS — real captured sequences; `ON`/`  off `/`Off` parse; `maybe`/`on off`/`1`/`enable` → usage; AlwaysOn on vs off texts differ; already-on `on` still sends.
4. Implementation vs oracle: PASS — as 2.
5. Module shape: PASS — `C13 PASS (base 0db49e3565)`; `builtin.rs` has no wire strings or engine kind (S2/S3); `session.rs` holds + delegates only; app.rs 0 delta.
6. Budget: N/A — no new loop; one-off phase.
7. Fence: PASS — all named fences green.
8. Mutation: PASS — `mutate.py …/slice3.json`: C3a-core RED ("v2 capture line 8"), C4 RED ("report for AlwaysOn"), C5 RED (refusal replaced by `Dispatched`), C6-case RED, C6-session RED, C7 RED, C12 RED ("`/thinking` is registered but absent from /help").
9. Restored: PASS — every command GREEN with tests executed.
10. Parity/reuse: PASS — no-session check reuses the agent-command idiom verbatim (`ctx.session.id().ok_or_else(|| Error::from_kind(ErrorKind::NoSession))`); usage-message style follows `/steer`/`/powers`; registration follows the `names`-before-`HelpCommand` rule. Lever comes from `ThinkingState::lever` (no second state→lever match); `refusal` matches only lever-less states with a `debug_assert!` sanity hint for the toggleable arms. Searched: grep `NoSession`, `Usage: /`, `names.push`.
11. Preserved enforcement: N/A — no gate or validator touched; `help_lists_every_registered_command` is reused as C12's fence unchanged.
Lint: clippy (default) clean for cyril-core and cyril-ui; fmt clean; `cargo check --workspace --all-targets` clean.


### Slice 4 — gate

Deviation (technical, parity-driven): the not-toggleable text is shared as `commands::builtin::THINKING_NOT_TOGGLEABLE_MESSAGE` so the command (B1/B4) and the UI's KAS ack (B3) cannot drift; approved strings unchanged. This invalidated slice 3's builtin.rs evidence: C4 re-run with a corrected anchor (`C4-recheck` in slice4.json) and C5/C6/C7 re-run from slice3.json — all RED then GREEN.

Caller analysis: `TuiState` gained `thinking_enabled()` — implementors: `UiState`, `MockTuiState` (both updated; compiler-complete). `UiState::apply_notification` config arm now returns `model_changed || thinking_acked`; the thinking state change itself is OR-ed at the end (`changed || stall_cleared || thinking_changed`).

1. Affected unit tests: PASS — `cyril-ui --lib` 608 passed; `cargo test --workspace` all green (cyril 134, cyril-core 992 unified-kas, cyril-ui 608, integration suites); `cyril-core --lib` default 759.
2. Falsifiers: PASS — C3a-ui per-frame equals the core list; C10b messages per rebuilt set; C11 segment exactly in the two toggleable states.
3. Stress fixture: PASS — `model` set adds no thinking message; rebuilt set without `thinking` → not-toggleable text; toggleable-unknown renders no segment; `Unavailable` with `thinkingEnabled:true` renders none.
4. Implementation vs oracle: PASS — as 2.
5. Module shape: PASS — `C13 PASS (base 0db49e3565)`; app.rs 0 delta; state.rs holds + delegates + two message arms; toolbar one segment.
6. Budget: PASS — toolbar: one `Option<bool>` read and ≤2 span pushes per frame (same class as the effort badge); no loop.
7. Fence: PASS — `state::tests::{thinking_follows_captured_sequences, thinking_config_ack_messages}`, `widgets::toolbar::tests::thinking_segment_per_state` green.
8. Mutation: PASS — `mutate.py …/slice4.json`: C3a-ui RED ("v2 capture line 24"), C10b RED ("a `model` set adds no thinking message"), C11 RED ("unreported: expected no thinking segment"), C4-recheck RED.
9. Restored: PASS — all GREEN with tests executed.
10. Parity/reuse: PASS — ack text reuses `thinking_toggled_message` (v2 and KAS share it); not-toggleable text shared from core; toolbar segment mirrors the effort badge's span/style idiom. Symmetry v2 ack vs KAS ack: both add exactly one message; v2 reports the requested value (ack has no session value), KAS reports the rebuilt value (source of truth) — spec B2/B3 divergence by design. Searched: grep `add_system_message(`, `fn effort(`, `"◇`.
11. Preserved enforcement: N/A — nothing relaxed; the pre-existing model arm behavior is unchanged (its tests green).
Lint: `cargo clippy -p cyril-core -p cyril-ui -p cyril -- -D warnings` clean; fmt clean.

### Final integration

- Assembled state: `cargo test --workspace` PASS; `cargo test -p cyril-core --lib` (default features, runs the not-kas contract module) PASS 759; `cargo check --workspace --all-targets` PASS; clippy (default, lib targets) PASS; fmt PASS; shape oracle PASS.
- Diff: 19 files, +1,843/−14 under `crates/` (incl. ~57 KB compact fixture ≈ 8 lines) — within the 2,080 plan budget; one increment.
- Not verifiable here: Linux/macOS CI legs, `clippy --all-targets` (pre-existing Windows-only failures in untouched test files), and a live kiro-cli session (no kiro-cli on this host). Live wire behavior rests on the committed 2.24.0 captures.
