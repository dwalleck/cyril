# Design: cyril-k3lz — thinking on/off toggle

## Route and inputs

- Route: **Structural** (`route.md`: T2 yes — public domain types, `TuiState`, new builtin; T4 no → interrogated).
- Behavior set: `spec.md` B1–B9 and its thinking-state definitions (signed "yes, I agree", 2026-09-25).
- Empirical premises: none unverified (`route.md` T1). Wire facts used below, with their captures:
  - P1 KAS `thinking` select `{id:"thinking", currentValue:"on"|"off", options:[on,off]}` present only for toggleable models; `set_config_option` result carries the rebuilt `configOptions` — `kas-new-surface-noprompt-0668-2.24.0.jsonl` lines 13–27 → fixture `crates/cyril-core/tests/fixtures/kas/thinking/set-config-sequence-0668.json`.
  - P2 v2 `commands/execute {command:{command:"reasoning", args:{thinkingEnabled:false}}}` → `{success:true, message:"", data:{…}}`; the next `_kiro.dev/metadata` carries `reasoning.thinkingEnabled:false` — `v2-reasoning-args2-2.24.0-2.24.0.jsonl` lines 8, 24–30 → fixture `crates/cyril-core/tests/fixtures/v2/thinking/reasoning-args2-2.24.0.json`.
  - P3 v2 emits no `configOptions` (no `configOption`/`config_option` key in `v2-reasoning-2.24.0-2.24.0.jsonl` or `v2-turn-sweep-2.24.0-2.24.0.jsonl`); KAS emits no `_kiro.dev/metadata` (CLAUDE.md, `ContextBreakdownUpdated` note in `session.rs`).
- Fixtures are regenerated verbatim by `.cyril-k3lz/extract_fixtures.py`.

## Input shapes

Inputs (every production-reachable cell):

| # | Shape | Status |
|---|---|---|
| I1 | `ReasoningInfo.support` ∈ {Toggleable, AlwaysOn, Unavailable, Other(s)} | C2 |
| I2 | `thinking_enabled` on Toggleable ∈ {Some(true), Some(false), None} | C2 |
| I3 | `thinking_enabled` on non-Toggleable = Some(_) (kept by q1xs parser) | C2 (ignored: support decides) |
| I4 | `MetadataUpdated.reasoning` ∈ {Some, None} | C3 |
| I5 | configOptions list: empty; without `thinking`; `thinking` = "on"; = "off"; = other string; value None | C1, C2 |
| I6 | `ConfigOptionsUpdated` vs `ConfigOptionSet{config_id}` with config_id "thinking" / other | C3, C10 |
| I7 | `SessionCreated`, `BridgeDisconnected` | C3 |
| I8 | Subagent-scoped frames | N/A — reason: App routes non-main session frames to `apply_subagent_notification` before either state machine (cyril-fh06, existing fenced routing); this change adds no routing |
| I9 | `/thinking` args: "", "on", "off", "ON", "  off ", "maybe", "on off" | C5, C6 |
| I10 | Session: none / active | C6 |
| I11 | v2 ack response: `success:true`; `success:false` with `error`; with `message` only; neither; missing `success`; RPC error | C8 |
| I12 | KAS ack: rebuilt set with thinking on / off / absent; set failure | C9, C10 |

Decision cells:

| # | Cell | Status |
|---|---|---|
| D1 | `ThinkingState` dispatch in `/thinking` report: 6 states × message | C4 |
| D2 | `/thinking on\|off` dispatch: ToggleableByReasoning (Some/None), ToggleableByConfigOption → send; AlwaysOn × {on, off}, NotToggleable, Unreported → refuse | C5, C7 |
| D3 | Mediator dispatch over `ThinkingLever` {ReasoningCommand, ConfigOption} — both arms send | C8, C9 |
| D4 | Toolbar: segment iff `thinking_enabled()` is Some — 6 states | C11 |
| D5 | Boundary representation: KAS `bogus`→off is an *engine* coercion; cyril only ever sends "on"/"off" literals, so the coercion is unreachable from cyril | C7 (value set is `{"on","off"}` by construction) |
| D6 | Boundary representation: v2 `thinkingEnabled` absent → `None` (unknown), never defaulted to on | C2 |
| D7 | Bounds imposed on callers | N/A — reason: none imposed |

## Placement

| Capability | Owner | New seam | Forbidden |
|---|---|---|---|
| Thinking-state model + derivation from both snapshot kinds | `cyril-core/src/types/thinking.rs` (create) — `ThinkingState`, `ThinkingLever`, `THINKING_NOT_TOGGLEABLE_MESSAGE` | `ThinkingState::apply_notification(&mut self, &Notification) -> bool` (pure, both state machines delegate) | Derivation (`"thinking"` key match, `support` match) anywhere else; any `AgentEngine`/`kind()` match (ADR-0001 rejects engine enum matching — the lever is a fact of the snapshot) |
| Session-side state for command gating | `SessionController` (field + `thinking()` accessor, delegates) | existing `apply_notification` | Own derivation logic |
| UI-side state, ack messages | `UiState` (field + delegate; `ThinkingToggled` and `ConfigOptionSet{config_id:"thinking"}` arms add the B2/B3 message) | `TuiState::thinking_enabled() -> Option<bool>` | Mutating via render; wire JSON |
| `/thinking` command | `commands/builtin.rs` `ThinkingCommand` | existing `Command` trait | Wire strings (`"reasoning"`, `"thinkingEnabled"`, `"on"`) — it sends a typed `BridgeCommand::SetThinking{lever, enabled}` |
| Wire dispatch of a toggle | domain mediator: `commands/mod.rs` arm → `extensions.rs::set_reasoning_thinking` (v2 lever) / existing `session.rs::set_config_option` (KAS lever) | `BridgeCommand::SetThinking{lever, enabled}` → `Notification::ThinkingToggled{enabled}` / existing `ConfigOptionSet` / existing `BridgeError` | Engine-kind match; a second copy of set_config_option response validation |
| Toolbar segment | `widgets/toolbar.rs` | `TuiState::thinking_enabled` | — |
| App | **none** (protected parent, zero production delta) | — | Any thinking logic in `app.rs` |

## Module shape

Current cluster (inventory; line counts are signals):

| Module | Interface / responsibilities | Change |
|---|---|---|
| `crates/cyril-core/src/types/session.rs` | `ReasoningInfo`, `ReasoningSupport`, effort types | retain (read only) |
| `crates/cyril-core/src/types/thinking.rs` | — | create: thinking-state model, derivation, wire vocabulary constants for the thinking config option, shared not-toggleable message |
| `crates/cyril-core/src/types/event.rs` | `Notification`, `BridgeCommand` enums (declarations) | retain: +1 variant each |
| `crates/cyril-core/src/session.rs` | `SessionController` session facts | retain: +field/accessor/delegate |
| `crates/cyril-core/src/commands/builtin.rs` | local builtin commands | deepen: +`ThinkingCommand` |
| `crates/cyril-core/src/commands/mod.rs` | registry + `/help` names | retain: registration only |
| `crates/cyril-core/src/protocol/domain_mediator/commands/{mod,extensions}.rs` | command dispatch → wire | retain/deepen: +1 arm, +1 extension call |
| `crates/cyril-ui/src/state.rs` | `UiState` | retain: +field/delegate/2 arms/accessor |
| `crates/cyril-ui/src/traits.rs` | `TuiState` | retain: +1 method (+mock) |
| `crates/cyril-ui/src/widgets/toolbar.rs` | toolbar render | retain: +segment |
| `crates/cyril/src/app.rs` | orchestrator | protected parent: zero delta |

Seam tests: deletion — removing `thinking.rs` would re-scatter the two-source derivation into both state machines (complexity reappears twice) → real. Interface — every test drives `apply_notification` / `Command::execute` / `BridgeCommand` / `TuiState`. Adapter — `ThinkingLever` is not a generic seam; it is a closed 2-variant fact with both variants real (v2, KAS). Locality — derivation has one owner (`thinking.rs`), wire dispatch one owner (mediator).

Alternatives for the disputed ownership (where the lever decision and the v2 ack value live):

1. **Minimal interface (selected).** `ThinkingState` carries the lever as a snapshot fact; the command sends `BridgeCommand::SetThinking{lever, enabled}`; the mediator maps lever → wire and, knowing `enabled`, emits `ThinkingToggled{enabled}` for v2; KAS reuses `set_config_option` → `ConfigOptionSet`. Caller: `ctx.bridge.send(BridgeCommand::SetThinking{lever, enabled: true})`. Hides all wire strings in core protocol; App untouched. Trade-off: one new `BridgeCommand` + one new `Notification`.
2. **Common caller (reuse existing primitives).** The command sends `ExecuteCommand{command:"reasoning", args}` / `SetConfigOption{"thinking", "on"}` directly; the App formats the v2 ack from `CommandExecuted{command:"reasoning"}`. Rejected: `CommandExecuted` does not carry the request, so the ack value must be remembered in App (protected parent grows a pending-request cluster) or guessed from `defaultThinkingEnabled` (a persisted-default field, not the session value — cyril-v2ol); wire strings leak into the command layer.
3. **Extension flexibility (engine-owned lever).** `Engine::thinking_lever()` trait method; `BridgeCommand::SetThinking{enabled}`; mediator asks the bound engine. Rejected: toggleability must still come from snapshot state, so the lever would be decided twice (engine + state) and can disagree; widens the ADR-0001 `Engine` trait for one feature.

Approved module ledger:

| Module/path | Interface | Owns | Hides/reuses | Must not own | Adapters | Tests through | Change |
|---|---|---|---|---|---|---|---|
| `crates/cyril-core/src/types/thinking.rs` | `ThinkingState` (+`lever()`, `enabled()`, `apply_notification`), `ThinkingLever` (+`config_value`), `THINKING_CONFIG_ID`, `THINKING_NOT_TOGGLEABLE_MESSAGE` | two-source derivation; snapshot replacement rules; shared B1/B3/B4 message contract | `ReasoningInfo`, `ConfigOption` | engine-kind matching; wire JSON | N/A — closed enum, no seam | `apply_notification`, constructors | create |
| `crates/cyril-core/src/commands/builtin.rs` | `ThinkingCommand: Command` | B1, B4–B6 messages; lever choice from state | `SessionController::thinking` | wire strings; engine kind | N/A | `Command::execute` + bridge channel | deepen |
| `crates/cyril-core/src/protocol/domain_mediator/commands/extensions.rs` | `set_reasoning_thinking` | v2 wire request + ack/failure mapping | `spawn_extension_command` | derivation | N/A | mediator test harness | deepen |
| `crates/cyril-ui/src/state.rs` | `UiState` + `TuiState::thinking_enabled` | holding state; ack messages | `ThinkingState` | derivation | N/A | `apply_notification` | retain |
| `crates/cyril-ui/src/widgets/toolbar.rs` | `render` | segment | `TuiState` | state | N/A | `TestBackend` | retain |

Protected parents:

| Protected parent | Baseline responsibilities | Allowed change | Forbidden change | Exit condition |
|---|---|---|---|---|
| `crates/cyril/src/app.rs` | event loop, routing, cross-cutting wiring | none | any change | `git diff <base> -- crates/cyril/src/app.rs` empty |
| `crates/cyril-core/src/session.rs` | session facts | field, accessor, one delegating call | `"thinking"` literal / support matching in production code | shape oracle S2 |
| `crates/cyril-ui/src/state.rs` | UI state machine | field, delegate call, two message arms, accessor | `"thinking"` config-key literal / support matching in production code | shape oracle S2 |

Shape fence: `.cyril-k3lz/oracles/shape.py` (issue-local; discovers the base via `git merge-base HEAD <upstream default>`), checks S1 app.rs zero delta; S2 no `"thinking"` literal, `ReasoningSupport::` match, or `thinkingEnabled` in production sections of `session.rs`/`state.rs`/`builtin.rs`; S3 no `AgentEngine`/`.kind()` in `types/thinking.rs` or `ThinkingCommand`; S4 `thinkingEnabled` wire key appears in production code only under `protocol/`; S5 cyril-ui does not import `cyril_core::commands::builtin`. Reports the rule ID and path.

## Claims

- **C1** The captured KAS `thinking` option reaches `ConfigOption{key:"thinking", value}` unchanged through `to_config_options` at every captured step (premise fence).
- **C2** `ThinkingState` construction maps every I1×I2 reasoning cell and every I5 configOptions cell to exactly the spec's state (I3 ignored; D6: absent → unknown).
- **C3** `ThinkingState::apply_notification` replaces from `MetadataUpdated{reasoning:Some}`, `ConfigOptionsUpdated`, `ConfigOptionSet`; resets to `Unreported` on `SessionCreated`/`BridgeDisconnected`; leaves every other notification (incl. `reasoning: None`) unchanged; returns true iff the state changed.
- **C3a** Replaying the captured v2 frame sequence and the captured KAS step sequence through the real converters and `SessionController` yields the spec-table state after every frame, and `UiState` yields the same `enabled()` after every frame.
- **C4** Bare `/thinking` emits the B1 message for each of the six states and sends zero bridge commands.
- **C5** `/thinking on|off` in AlwaysOn (both args), NotToggleable, Unreported emits the B4 message and sends zero bridge commands.
- **C6** Invalid args emit `Usage: /thinking [on|off]` with zero sends; `ON` / `  off ` parse; no session with a valid arg returns the NoSession error with zero sends.
- **C7** In a toggleable state `/thinking on|off` sends exactly one `BridgeCommand::SetThinking{lever: <state's lever>, enabled}`.
- **C8** The mediator's `ReasoningCommand` arm sends exactly one `kiro.dev/commands/execute` with params `{sessionId, command:{command:"reasoning", args:{thinkingEnabled}}}` (args has exactly one key) and maps `success:true` → `ThinkingToggled{enabled}`, any other outcome → `BridgeError{operation:"Thinking change", message}` with the I11 message rule.
- **C9** The mediator's `ConfigOption` arm sends exactly one `session/set_config_option {configId:"thinking", value:"on"|"off"}` via the existing handler, yielding `ConfigOptionSet{config_id:"thinking"}` on ack.
- **C10** `UiState` adds exactly one message: `ThinkingToggled{true|false}` → `Thinking turned on.`/`off.`; `ConfigOptionSet{config_id:"thinking"}` → message from the rebuilt state (on / off / can't be toggled); `ConfigOptionSet` for another id adds none.
- **C11** The toolbar shows ` · think` / ` · no think` exactly in toggleable-on / toggleable-off and no thinking segment in the other four states.
- **C12** `/thinking` is registered on both engines' registries and listed by `/help`.
- **C13** Shape: app.rs zero delta; derivation only in `types/thinking.rs`; no engine-kind matching in thinking code; `thinkingEnabled` only under `protocol/` (S1–S4).

Subtractive sweep: purely additive — no guard, serialization point, or validation is removed; existing `ConfigOptionsUpdated`/`ConfigOptionSet` model handling is untouched and still fenced by its tests.

## Falsification

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C1 | captured thinking option survives conversion | I5 (captured on/off/absent) | Deserialize each fixture step, convert, compare `thinking` value; falsified if any step differs from § 7.1 | audit § 7.1 table values transcribed by hand (on,on,off,on,off,absent) | `convert/mod.rs::to_config_options`: add `.filter(\|o\| o.id.to_string() != "thinking")` → red at `cfg_model` | `protocol::convert::tests::kas_thinking_option_survives_config_conversion_per_captured_step` | 1 min | PASS |
| C2 | construction covers every cell | I1, I2, I3, I5, D6 | Table test over 4 supports × 3 enabled + 6 configOption shapes; falsified by any mismatch | spec "Thinking state" definitions (hand-written expected table) | `thinking.rs::from_reasoning`: map `thinking_enabled: None` → `Some(true)` → red at toggleable/None row | `types::thinking::tests::from_reasoning_matrix`, `from_config_options_matrix` | 5 min | PENDING — checkpointed-build, slice 1 |
| C3 | apply rules | I4, I6, I7 | Apply each notification kind from a non-default state; assert state + return; positive control: a replacing frame does change it | spec B8 | `thinking.rs::apply_notification`: treat `reasoning: None` as `Unreported` → red at the "absent block keeps state" row | `types::thinking::tests::apply_notification_rules` | 5 min | PENDING — checkpointed-build, slice 1 |
| C3a | captured replay, both state machines agree | I2, I4, I5, I6 captured | Replay v2 fixture frames through `kiro::to_ext_notification` and KAS steps through `to_config_options`→`ConfigOptionSet`; assert per-frame state in `SessionController` and `UiState` | per-frame expected list transcribed from the capture lines (v2: 8 toggleable/None … 30 off; KAS: § 7.1) | `session.rs`: drop the `self.thinking.apply_notification` call → red at first frame; `state.rs` drop → red at UiState assertion | `cyril-core/tests/thinking_capture_replay.rs` (core) + `cyril-ui` state test `thinking_follows_captured_sequences` | 10 min | PENDING — checkpointed-build, slice 2 (core) / slice 4 (ui) |
| C4 | report messages | D1 | Execute bare `/thinking` against a controller in each state; assert exact text, `try_recv` empty | spec B1 strings | `builtin.rs::ThinkingCommand`: swap AlwaysOn and NotToggleable messages → red | `commands::builtin::tests::thinking_report_messages` | 5 min | PENDING — checkpointed-build, slice 3 |
| C5 | refusals | D2 refuse arms | Execute `on`/`off` per refusing state; assert text + zero sends | spec B4 strings | `ThinkingCommand`: treat `AlwaysOn` as toggleable via reasoning → red (a send appears) | `commands::builtin::tests::thinking_refuses_without_toggleable_support` | 5 min | PENDING — checkpointed-build, slice 3 |
| C6 | args + no session | I9, I10 | Execute each I9 arg / no-session case; assert text or `ErrorKind::NoSession`; zero sends | spec B5/B6 | `ThinkingCommand`: drop `.to_ascii_lowercase()` → red at `ON`; move session check after send → red at no-session | `commands::builtin::tests::thinking_args_and_no_session` | 5 min | PENDING — checkpointed-build, slice 3 |
| C7 | exactly one typed send | D2 send arms | Execute `on`/`off` in each toggleable state; `recv` one command, assert variant+fields, then `try_recv` empty | lever expected from the fixture state kind (reasoning→ReasoningCommand, config→ConfigOption) | `ThinkingCommand`: hard-code `ThinkingLever::ReasoningCommand` → red at KAS row | `commands::builtin::tests::thinking_sends_one_typed_toggle` | 5 min | PENDING — checkpointed-build, slice 3 |
| C8 | v2 wire + ack mapping | D3 reasoning arm, I11 | Mediator harness with scripted agent: assert the exact outbound params JSON and resulting notification per I11 response | P2 captured request/response shape | `extensions.rs::set_reasoning_thinking`: add `"setAsDefault": false` → red (args key count); map missing `success` to ok → red at that row | `protocol::domain_mediator::tests::…::set_thinking_reasoning_wire_and_ack` | 30 min | PENDING — checkpointed-build, slice 2 |
| C9 | KAS wire | D3 config arm | Same harness: assert outbound `session/set_config_option` params and `ConfigOptionSet{config_id:"thinking"}` from a captured rebuilt set | P1 fixture step `cfg_thinking_off` | mediator arm: send `config_value(!enabled)` → red on value | `…::set_thinking_config_option_wire_and_ack` | 30 min | PENDING — checkpointed-build, slice 2 |
| C10 | ack messages | I6, I12 | Apply each ack notification to `UiState`; assert last message text and message-count delta = 1 (0 for other id) | spec B2/B3 strings | `state.rs`: emit the thinking message on any `ConfigOptionSet` → red at other-id row (delta 1) | `cyril-ui state::tests::thinking_ack_messages` | 5 min | PENDING — checkpointed-build, slice 4 |
| C11 | toolbar | D4 | Render toolbar on `TestBackend` for each of six states; assert `no think`/`think` substring presence exactly as specified; positive control: on-state contains `think` | spec B7 | `toolbar.rs`: render segment when `thinking_enabled()` is `None` as `think` → red at unknown row | `widgets::toolbar::tests::thinking_segment_per_state` | 5 min | PENDING — checkpointed-build, slice 4 |
| C12 | registered + help | — | Build registry for V2 and KAS sources; parse `/thinking`; render `/help` and assert it lists `/thinking` | spec B9 | `commands/mod.rs`: register after `HelpCommand::new(&names)` without pushing the name → red (existing `help_lists_every_registered_command`) | existing `help_lists_every_registered_command` + new `thinking_command_registered_on_every_engine` | 5 min | PENDING — checkpointed-build, slice 3 |
| C13 | module shape | placement, protected parents | `python .cyril-k3lz/oracles/shape.py`; falsified by any S1–S4 hit | source/diff census (not production code) | add `if o.key == "thinking"` to `session.rs` production section → S2 red; touch app.rs → S1 red | `.cyril-k3lz/oracles/shape.py` | 1 min | PENDING — checkpointed-build, every slice |

## Non-goals and future work

- Permanent non-goals (spec out-of-scope): bare-toggle `/thinking`; autocomplete of on/off; per-subagent thinking; mirroring KAS's effort cap in cyril (the engine rebuilds effort itself).
- Intended future work (verified tracker IDs): picker badges `thinkingToggleable`/`defaultThinkingEnabled` — cyril-lxuo; KAS effort badge incl. post-toggle cap — cyril-4jt7; KAS `/model` / `/effort` surface — KAS-4 (cyril-cxwb, cyril-838u); v2 persisted reasoning default — cyril-v2ol.
- P3 residual: if a future v2 starts sending `configOptions` without `thinking`, the state would read not-toggleable until the next metadata snapshot (every turn). Revisit trigger → intended future work, tracked in cyril-xdll (re-probe v1/v2 configOptions).

## Falsifier run log

- 2026-09-25 C1: `cargo test -p cyril-core --lib kas_thinking_option_survives` on worktree `feat/cyril-k3lz` @ 9b995bd + fixture/test (Windows, toolchain 1.94.0) → `1 passed; 0 failed`. PASS.

## Approval

Requester approval (verbatim): "yes, I approve"
Date: 2026-09-25
Risk acceptances: None


## Final design-conformance review and bounded repair

Independent review method: fresh subagent was given only the production diff (`git diff 0db49e3 -- crates`) first, then this design's Placement and Module shape sections. It reconstructed every changed production module, interface, responsibility cluster, dependency direction, protected parent, and test boundary before comparison.

Initial verdict: **FAIL**, one mismatch only. `crates/cyril-ui/src/state.rs` imported `cyril_core::commands::builtin::THINKING_NOT_TOGGLEABLE_MESSAGE`, adding a cyril-ui → command-layer dependency not present in the approved ledger or the crate boundary rule (cyril-ui depends on cyril-core types, not commands). The reviewer also noted that v2 response parsing belonged at `protocol/convert/kiro.rs`, but treated that as an internal consistency observation rather than a ledger mismatch.

Bounded repair (approved-contract technical correction; no behavior, oracle, ownership, interface, architecture, or accepted-risk decision changed):

1. Moved the shared `THINKING_NOT_TOGGLEABLE_MESSAGE` constant into `cyril-core/src/types/thinking.rs` and re-exported it from `types/mod.rs`; both `commands/builtin.rs` and `cyril-ui/src/state.rs` now consume the domain type constant. This restores the approved types-only dependency direction and gives the message one owner.
2. Moved v2 `reasoning` response-shape parsing into `cyril-core/src/protocol/convert/kiro.rs::parse_reasoning_command_ack`; the mediator maps the parser result to `ThinkingToggled` / `BridgeError`. This follows the existing Kiro conversion boundary without changing the C8 observable contract.
3. Extended the shape fence with S5: cyril-ui production code must not import `cyril_core::commands::builtin`. Added `repair1.json` mutations proving S5, S1, and S2 detect their named violations.

Post-repair comparison: **PASS**. The independent ledger comparison now has no mismatches: `types/thinking.rs` owns derivation and the shared message; cyril-ui imports only `cyril_core::types::*`; `app.rs` remains unchanged; no engine-kind matching or duplicated derivation exists; the mediator reuses the Kiro conversion boundary. `python .cyril-k3lz/oracles/shape.py` returns `C13 PASS (base 0db49e3565)` after restoration. The repair retains the original requester approval because it is a technical placement correction under the approved ledger, not a changed decision.

Repair evidence: `python .cyril-k3lz/mutate.py .cyril-k3lz/mutations/repair1.json` → C13-S5, C13-S1, C13-S2 all RED; restored shape fence GREEN. `cargo test --workspace` exits 0; 31 test result lines are green. `cargo clippy -p cyril-core -p cyril-ui -p cyril -- -D warnings`, `cargo fmt --check`, and the shape fence pass. Full `--all-targets` clippy remains blocked only by pre-existing warnings in untouched platform/test code documented in `plan.md`; `cargo check --workspace --all-targets` passes.

Final isolated-review verdict: **PASS**.
