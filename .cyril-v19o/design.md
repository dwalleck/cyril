# Design: cyril-v19o — `/powers` panel from the KAS powers push

## Route and inputs

- **Route**: Empirical (`route.md`) — T1 yes (installed-version wire premise), T2 yes (new modules + interface), T3 no, T4 no.
- **Behavior set**: `spec.md` (signed 2026-09-10, approval verbatim `"Approve spec.md"`, requester answers `"/powers overlay panel"`, `"Log in now, verify live"`, `"Rich: title + meta + description"`, `"Push only (as specced)"`). Behaviors B1–B9 and success criteria SC1–SC7 are the behavior source; this design does not restate them.
- **Empirical premises**: `evidence.md` P1 (2.21.2 emits the frame in cyril's spawn shape with the documented item schema — PASS), P1b (the push is unprompted — PASS), P2 (`refresh` is uncallable — PASS), P3 (v2 has no powers surface — PASS), P5 (powers are user-level — PASS); P4 and P6 are `N/A` per that file. Independent oracles: filesystem/registry ground truth via `jq`, the sibling `list` handler in the same capture, `jq` over the raw traces, and three prior committed verdicts on other binaries.
- **Probe artifacts**: `.cyril-v19o/probe-kas-powers-2.21.2.py`, `.cyril-v19o/probe-kas-powers-unprompted-2.21.2.py`, `.cyril-v19o/probe-v2-command-advertisement.py`; captures `.cyril-v19o/kas-powers-2.21.2-2.21.2.jsonl`, `.cyril-v19o/kas-powers-unprompted-2.21.2.jsonl`, `.cyril-v19o/v2-commands-2.21.2.jsonl`.
- **Wire contract consumed** (live 2.21.2): `_kiro/powers/items_changed` params `{sessionId, status: "success", powers: [{name, description, displayName, keywords[], mcpServerNames[], hasSteeringFiles, isAgentPlugin, _meta{kiro{resource{resourceType,source{origin}}}}}]}`, pushed once, unprompted, 18 ms after `session/new` replies.
- **Spec correction recorded under Approval semantics** (technical, behavior-preserving): `spec.md`'s Decisions cell reads "`keywords` is parsed but never displayed". The approved observable behavior is that `keywords` is not displayed; parsing a field with no consumer is weightless, so this design **does not parse** `keywords`, `isAgentPlugin`, or `_meta`. Approved behavior, oracle meaning, architecture and accepted risk are unchanged; the cell's implication is corrected here rather than re-approved.

## Input shapes

Enumeration is of `_kiro/powers/items_changed` frames and item payloads; sources are the spec's edge rows, `evidence.md`, and the live capture.

| # | Shape | Status |
|---|-------|--------|
| S1 | `powers` absent from params | Covered by C3 |
| S2 | `powers` present but not an array (object, string, null) | Covered by C3 |
| S3 | `powers: []` (live-confirmed empty graph shape) | Covered by C2 |
| S4 | `powers` with exactly one item | Covered by C1 |
| S5 | `powers` with many items, distinct (the live 3-item set is this shape) | Covered by C1 |
| S6 | `powers` with two items sharing a `name` | Covered by C5 (state keeps a list, not a keyed map, so nothing collapses) |
| S7 | Item missing `name` (required identifier) | Covered by C3 (frame-level rejection) |
| S8 | Item missing `displayName` | Covered by C4 (`title()` falls back to `name`) |
| S9 | Item with `displayName: ""` | Covered by C4 (empty means not provided; fall back to `name`) |
| S10 | Item missing `description` | Covered by C4 (description line omitted) |
| S11 | Item with `mcpServerNames: []` | Covered by C4 (no `mcp` token rendered) |
| S12 | Item with two or more `mcpServerNames` | Covered by C4 |
| S13 | `hasSteeringFiles: true` / `false` | Covered by C4 |
| S14 | Unknown extra item keys (forward compatibility) | Covered by C1 (serde ignores unknown fields) |
| S15 | Item is not an object (string/number inside the array) | Covered by C3 |
| S16 | `displayName`/`description` containing CJK or other wide characters | Covered by C6 |
| S17 | `displayName`/`description` longer than the panel width | Covered by C6 |
| S18 | Catalog larger than the panel viewport | Covered by C6 (viewport clamp) and C5 (scroll clamp) |
| S19 | Terminal/panel area too small for `modal::place` | Covered by C6 (`place` returns `None` → nothing painted; existing invariant) |
| S20 | Two pushes in one session, second shorter than the first, panel open and scrolled past the new end | Covered by C6 (clamp on refresh) |
| S21 | Push arrives with no panel open | Covered by C5 (state updates, panel stays closed) |
| S22 | `/powers` submitted while the engine is v2 | Covered by C5 (no catalog → one system line) |
| S23 | Numeric/boolean boundaries: none exist on this wire surface (no counters, no timestamps) | `N/A — permanent non-goal: the payload carries no numeric or temporal field` |
| S24 | Paths: none cross this feature (no file I/O, no path translation) | `N/A — permanent non-goal: display-only; cyril never reads `~/.kiro/powers`` |
| S25 | Permission/auth shapes: none — the frame is agent→client and needs no capability | `N/A — permanent non-goal: no client capability or permission participates` |

## Placement

| Capability | Owner | New seam | Forbidden |
|---|---|---|---|
| Recognize and convert `kiro/powers/items_changed` into the domain payload | `crates/cyril-core/src/protocol/convert/kas/powers.rs` (new module beside `workflow.rs`), reached from `KasEngine::convert_ext_notification` | `pub(crate) fn to_notification(method: &str, params: &Value) -> crate::Result<Option<Notification>>` — the KAS-dialect conversion seam already used by the workflow adapter | Must not format for display, must not know about panels, must not hold state; must not be reachable from the v2 engine path |
| Domain payload type crossing core→ui | `crates/cyril-core/src/types/power.rs` (new module; `types/mod.rs` registers it) | `PowerInfo` with accessor methods (`name()`, `title()`, `description()`, `mcp_server_names()`, `has_steering_files()`); fields private | Must not carry wire key names, `serde` derives for wire shape, or UI concepts (widths, colors, rows) |
| Hold the process-global catalog | `crates/cyril-core/src/session.rs` (`SessionController`), beside `kas_hooks` | `Notification::PowersChanged { powers: Vec<PowerInfo> }` plus `powers() -> Option<&[PowerInfo]>` | Must not sort, format, or render; must not decide whether a panel is open |
| Answer `/powers` | `crates/cyril-core/src/commands/builtin.rs` (`PowersCommand`), with `CommandResultKind::ShowPowers { powers: Vec<PowerInfo> }` in `commands/mod.rs` | The existing `Command` trait; the existing `CommandContext::session` read | Must not touch UI state, must not send any bridge command, must not parse wire JSON |
| Panel view state and opening/closing/scrolling | `crates/cyril-ui/src/state.rs` (`UiState`), `PowersPanelState` declared in `crates/cyril-ui/src/traits.rs` | `TuiState::powers_panel() -> Option<&PowersPanelState>` beside `hooks_panel()` | Must not parse wire JSON, must not call the bridge, must not add chat messages |
| Paint the panel | `crates/cyril-ui/src/widgets/powers_panel.rs` (new module; `widgets/mod.rs` registers it) | `pub fn render(frame, area, input_top, state: &PowersPanelState, theme: &Theme)` | Must not read `UiState`, must not mutate state, must not compute domain values |
| Route the push, key, and command-result to the owners | `crates/cyril/src/app.rs` (protected parent) | Existing event-loop arms | No new responsibility body: no parsing, no formatting, no ordering, no state beyond what the owners expose |

## Module shape

### 1. Current cluster inventory

| Module | Prod lines | Interface (what callers must know) | Responsibility clusters | Dependencies / direction | Callers & tests crossing the interface | Concrete adapters | Class |
|---|---:|---|---|---|---|---|---|
| `crates/cyril-core/src/protocol/convert/kiro.rs` | 1861 | `to_ext_notification(method, params) -> Result<Option<Notification>>`; v2 `kiro.dev/*` dialect plus two `kiro/*` arms; unknown → `debug!` + `Ok(None)` | v2 dialect conversion; the shared fallback for KAS frames | depends on `types::`, `acp::`; nothing depends on it except the two engines | `V2Engine`, `KasEngine` (fallback), converter tests | none (pure functions) | retain — untouched by this change |
| `crates/cyril-core/src/protocol/convert/kas.rs` | 1405 | KAS `session/update` sub-kind conversion (`_meta.kiro.kind`) | KAS session-carried conversion | `types::` | `KasEngine::convert_session_update` | none | retain — this change adds a sibling module, not an arm here |
| `crates/cyril-core/src/protocol/convert/kas/workflow.rs` | 5670 | `to_notification` → `WorkflowFrameOutcome{Converted,Dropped,NotWorkflow}` | workflow lifecycle conversion | `types::workflow` | `KasEngine::convert_ext_notification` (tried first) | none | retain — pattern source for the new module |
| `crates/cyril-core/src/protocol/engine.rs` | 839 | `Engine` trait: `convert_ext_notification`, `convert_session_update`, `emits_wire_turn_end`, `adapters`, `settings_extra` | engine identity + dialect dispatch | `convert::`, `types::` | domain mediator (`inbound.rs:170`) | `V2Engine`, `KasEngine` | retain — gains one dispatch arm |
| `crates/cyril-core/src/types/event.rs` | 1128 | `Notification` — the single domain channel; ~40 variants | domain notification vocabulary | `types::*` | every consumer (`SessionController`, `UiState`, `App`) | none | retain |
| `crates/cyril-core/src/session.rs` | 1247 | `SessionController::apply_notification -> bool` and readers (`kas_hooks()`, `resolve_kas_hook_id`) | session data state machine | `types::` | App, commands (`builtin.rs:425`) | none | deepen — gains one catalog + getter |
| `crates/cyril-core/src/commands/builtin.rs` | 455 | `Command` impls; `KasHooksCommand` is the shape precedent | local/builtin slash commands | `types::`, `protocol::bridge::BridgeCommand` | `CommandRegistry`, `App::submit_input` | none | retain — gains one small command |
| `crates/cyril-ui/src/widgets/hooks_panel.rs` | 324 | `render(frame, area, input_top, &HooksPanelState, &Theme)`; self-sizes via `modal::place` | panel painting | `crate::theme`, `types::HookInfo` | `render.rs:153`, widget tests | none | retain — pattern source |
| `crates/cyril-ui/src/traits.rs` | 1324 | `TuiState` read-only projection; `HooksPanelState` struct | rendering contract | `cyril_core::types` | renderer, `MockTuiState` | `UiState`, `MockTuiState` (2 real adapters) | retain — gains struct + accessor |
| `crates/cyril-ui/src/state.rs` | 7926 | `UiState` mutation surface | UI state for every surface | `cyril_core::types` | App | none | protected parent |
| `crates/cyril-ui/src/render.rs` | 1492 | `draw(&dyn TuiState)`; overlay block | frame composition | widgets, `theme` | `App::render` | none | retain — gains one overlay arm |
| `crates/cyril/src/app.rs` | 6857 | event loop, routing, overlay key dispatch | orchestration | everything | `main.rs` | none | protected parent |

### 2. Seam tests for the new seams

- **Deletion test** — deleting `convert/kas/powers.rs` forces wire-shape knowledge (`displayName` case, the required-`name` rule, the all-or-nothing rule) into `engine.rs` or into `cyril-ui`; complexity reappears across callers. Deleting `types/power.rs` forces `cyril-ui` to depend on serde wire structs. Deleting `widgets/powers_panel.rs` would move layout, truncation, and the steering marker into `render.rs`, which is shared by every surface. Each module earns its place.
- **Interface test** — the converter is exercised through `to_notification` (the same entry the engine uses); the widget is exercised through `render` with a `PowersPanelState` the state layer would produce; the command through `Command::execute` with a real `CommandContext`. No test reaches past its module's interface.
- **Adapter test** — no new generic seam is introduced. `TuiState` already has two real adapters (`UiState`, `MockTuiState`), so the added accessor rides an existing real seam; the converter has one concrete caller by design (the engine dispatch), which is why it is a concrete private module rather than a trait.
- **Locality test** — wire validation lives only in the converter; ordering lives only in `UiState`'s setters; painting lives only in the widget; routing lives only in `App`. One owner per responsibility.

### 3. Alternatives for the disputed shape (catalog state + panel data)

**Alternative 1 — hooks-mirror (selected).** Catalog in `SessionController` as `Option<Vec<PowerInfo>>`; panel in `UiState` as `PowersPanelState { powers: Vec<PowerInfo>, scroll_offset }`; `/powers` reads `ctx.session.powers()`, returns `ShowPowers { powers }` or a system message; the App arm opens the panel and, on a later push, refreshes it only when open.
- Caller example: `PowersCommand::execute` → `match ctx.session.powers() { None => system_message("…not reported…"), Some(p) => Ok(CommandResult::show_powers(p.to_vec())) }`.
- Hidden: wire parsing, the three-state model, sort, clamp, layout.
- Trade-off: the catalog is duplicated (session + panel) exactly as `kas_hooks`/`HooksPanelState` already is.

**Alternative 2 — `UiState`-only catalog.** `SessionController` untouched; `/powers` returns a payload-free `ShowPowers`; the App calls `ui_state.open_powers_panel()` which returns `Opened | NotLoaded` and the App turns `NotLoaded` into the chat line.
- Trade-off: fewer fields, but chat-output responsibility leaks into `UiState` (it currently owns messages, not command replies), and the command layer can no longer answer `/powers` — it becomes a pure token, diverging from every other list command.

**Alternative 3 — dedicated `PowersCatalog` type in core.** `PowersCatalog { loaded: bool, powers: Vec<PowerInfo> }` owning ordering and the three-state semantics, held by both `SessionController` and `UiState`.
- Trade-off: deeper in principle, but the deletion test exposes it as a wrapper over `Option<Vec<PowerInfo>>` whose only extra behavior (ordering) the state setter already owns; it would also duplicate the catalog a third time.

**Selection: Alternative 1.** Reason: it is the only shape with an existing, fully-rendered precedent in this repository (`HooksChanged` → `SessionController::kas_hooks` → `UiState::refresh_hooks_panel` → `TuiState::hooks_panel` → `widgets/hooks_panel.rs`), so a maintainer learns no new pattern; it keeps the command layer able to answer `/powers` (the reason `kas_hooks` exists in `SessionController` at all — `builtin.rs:425`); and it keeps chat-line ownership in the command/App layers.

### 4. Approved module ledger

| Module/path | Interface | Owns | Hides/reuses | Must not own | Adapters | Tests through | Change |
|---|---|---|---|---|---|---|---|
| `crates/cyril-core/src/protocol/convert/kas/powers.rs` | `to_notification(method, params) -> Result<Option<Notification>>` | `kiro/powers/items_changed` recognition, wire structs, per-frame validation (required `name`, list type), empty-vs-missing distinction | serde plumbing, field naming, the all-or-nothing rule | display formatting, ordering, state | `N/A — one concrete caller by design (engine dispatch)` | `to_notification` | create |
| `crates/cyril-core/src/types/power.rs` | `PowerInfo::new(...)` + accessors: `name`, `title`, `description`, `mcp_server_names`, `has_steering_files` | the domain payload's invariants (title falls back to id, empty string means absent) | field storage, wire key mapping | wire names, widths, colors, panels | `N/A — plain data type` | accessors | create |
| `crates/cyril-core/src/protocol/engine.rs` | `Engine::convert_ext_notification` (unchanged) | engine dispatch order (workflow → powers → shared kiro) | — | powers wire knowledge | `V2Engine`, `KasEngine` | engine tests | retain |
| `crates/cyril-core/src/types/event.rs` | `Notification` enum | the new `PowersChanged` variant | — | ordering/formatting | `N/A` | matches in consumers | retain |
| `crates/cyril-core/src/session.rs` | `apply_notification`, `powers()` | catalog storage and last-write-wins replacement | — | sorting, rendering, panel state | `N/A` | `apply_notification` + `powers()` | deepen |
| `crates/cyril-core/src/commands/builtin.rs` | `Command` trait | `/powers` argument handling and the not-loaded reply | — | UI state, bridge traffic, wire parsing | `N/A` | `Command::execute` | retain |
| `crates/cyril-ui/src/traits.rs` | `TuiState::powers_panel()`; `PowersPanelState` | the read-only projection of the panel | — | mutation, parsing | `UiState`, `MockTuiState` | trait method | retain |
| `crates/cyril-ui/src/state.rs` | `show/hide/has/scroll/powers` methods | panel lifecycle, ordering, scroll clamp, push-while-open refresh | — | wire parsing, chat lines | `N/A` | panel methods | deepen (protected parent) |
| `crates/cyril-ui/src/widgets/powers_panel.rs` | `render(frame, area, input_top, &PowersPanelState, &Theme)` | painting the approved three-line layout, placeholder, width clamping | `modal::place`, theme tokens | state, domain computation | `N/A` | `render` via `TestBackend` | create |
| `crates/cyril-ui/src/render.rs` | `draw` | overlay composition | — | powers layout | `N/A` | render tests | retain |
| `crates/cyril/src/app.rs` | event loop | routing the notification/key/command-result | — | parsing, formatting, ordering, new state fields | `N/A` | app tests | protected parent |
| `crates/cyril-core/src/commands/mod.rs` | `CommandResultKind::ShowPowers`, `CommandResult::show_powers` | carrying the catalog from the command layer (session reachable) to the App (UI reachable); owning the `powers` name | — | UI types, panel state | `N/A` | `Command::execute` + result match | retain |
| `crates/cyril-core/src/protocol/convert/kas.rs` | module list | declaring the `powers` sibling adapter | — | any powers logic | `N/A` | compile | retain |
| `crates/cyril-core/src/types/mod.rs`, `crates/cyril-ui/src/widgets/mod.rs` | re-export / module list | registering the new module | — | — | `N/A` | compile | retain |
| `crates/cyril-ui/src/theme.rs` | theme-source census | listing the new widget among the censused widget sources | — | any powers behavior | `N/A` | `widgets_only_use_the_explicit_theme` | retain |

### 5. Protected parents

| Protected parent | Baseline responsibilities | Allowed change | Forbidden change | Exit condition |
|---|---|---|---|---|
| `crates/cyril/src/app.rs` | event loop, notification routing, overlay key dispatch, command-result projection | one `Notification::PowersChanged` arm (update session state + refresh-if-open), one key-dispatch branch, one `ShowPowers` command-result arm, one term in the modal overlay predicate that guards mouse-scroll, module wiring in `main.rs` | a new state field on `App`; any parsing, formatting, sorting, or wire knowledge; an auto-open on push | production delta confined to those arms; no new `App` field; the shape fence's protected-parent census stays green |
| `crates/cyril-ui/src/state.rs` | UI state for every surface, one `apply_notification` arm per variant | panel lifecycle methods mirroring the hooks-panel set, one `apply_notification` arm returning `false` | wire parsing, domain formatting, chat-message emission | the powers methods are the only production delta; `hooks_panel`/`usage_panel`/`code_panel` methods untouched |

### 5a. Conformance-review record corrections (applied after the build)

The isolated design-conformance review (`.cyril-v19o/checkpoints.md`, "Final
design-conformance review") returned **PASS** with no code drift and four
record-completeness gaps in *this* document. Corrected here: the
protected-parent allowed-change list now names the modal overlay predicate term
the new overlay must join; the registration modules
(`commands/mod.rs`, `convert/kas.rs`, `types/mod.rs`, `widgets/mod.rs`) and the
theme-source census (`theme.rs`) have ledger rows instead of appearing only in
prose; and row 2's interface cell names the public constructor that row 2's own
"fields private, invariants at construction" rule requires.

None of these changes an approved behavior, ownership boundary, interface
contract, or accepted risk — each records what the review confirmed the code
already does under the design's own rules. No re-approval is required.

### 6. Mechanical shape claim (see C8)

Enforced by `.<change-slug>/oracles/module_shape.py` (issue-local, standalone, discovers the default branch rather than hard-coding one) plus `.cyril-v19o/oracles/mutations.sh`.

## Claims

- **C1** — A `kiro/powers/items_changed` frame converts one-to-one into `PowersChanged` items whose values are the wire values (verified against the live capture).
- **C2** — A frame with `powers: []` converts to `PowersChanged { powers: [] }` and is never dropped.
- **C3** — A frame with a missing, non-array, or invalid-`powers` payload is dropped with a `warn!` and leaves any previously held catalog unchanged.
- **C4** — `PowerInfo::title()` returns the display name when present and non-empty, else the id; empty optional strings are treated as absent; absent optional fields render as nothing, never as a placeholder.
- **C5** — `/powers` opens the panel only when a catalog is held; with none it adds exactly one system line; a push never opens a panel by itself; ordering is by display name, ties by id; two pushes replace the catalog, and duplicates are not collapsed.
- **C6** — The panel paints the approved layout (title / id + `mcp` servers + `steering` / description), renders `No powers installed` for a known-empty catalog, clamps width and viewport, and never covers the input line.
- **C7** — cyril emits no `kiro/powers/list` and no `kiro/powers/refresh` request from any code path.
- **C8** — Production files match the approved module ledger and the protected-parent rules.

## Falsification

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C0 | The committed test fixture is a verbatim slice of the live 2.21.2 capture, so the converter tests assert real wire data | the live 3-item frame | `jq -S` the `items_changed` frame from `.cyril-v19o/kas-powers-2.21.2-2.21.2.jsonl` and canonicalize both sides: a non-empty diff falsifies. Other cause: a hand-edited fixture | `jq` extraction of the raw capture, compared against the raw capture re-read with Python `json` — two languages, neither of which is the Rust implementation | `N/A — approved risk: no fence to mutate` | `N/A — approved risk: one-shot data-provenance check run before approval; the values it pins become permanent literals in C1's fence, and the provenance stays auditable because the raw capture, probe script and verdict are committed under `experiments/conductor-spike/` alongside the existing `kas-powers-2.20.1`/`2.21.x` set` | seconds | PASS (run log) |
| C1 | The converter maps each wire field to the matching `PowerInfo` field with the wire's values | S4, S5, S14 | Convert the committed 3-item fixture and compare every field against literal values taken from the live capture: any mismatch falsifies. Other cause: a serde rename collision is ruled out because the control also flips one wire value (`hasSteeringFiles`) and asserts the output flips | the literal values in the test are the `jq`-extracted capture values (C0's oracle), not values produced by the converter | in `convert/kas/powers.rs`, swap `display_name` and `name` when constructing `PowerInfo` → `cargo test -p cyril-core powers` fails naming the mismatched field | `convert/kas/powers.rs` test `powers_frame_maps_every_field_from_the_capture` — offline, no I/O | minutes | PENDING — checkpointed-build, slice 1 |
| C2 | An empty `powers: []` frame converts to an empty catalog, not to a drop | S3 | Convert the live empty-graph frame (committed capture `kas-powers-2.21.1-2.21.1.jsonl` shape, re-derived in `.cyril-v19o/`) and assert `Some(PowersChanged{..})` with an empty list; a `None` falsifies. Other cause: the malformed-frame path — ruled out by the control asserting the same test's malformed frame returns `None` | the empty frame is validated independently with `jq` (`(.params.powers\|type)=="array" and (.params.powers\|length)==0`) | in the converter, return `Ok(None)` when the parsed list is empty → test fails | `convert/kas/powers.rs` test `empty_catalog_is_loaded_not_dropped` | minutes | PENDING — checkpointed-build, slice 1 |
| C3 | A malformed frame is dropped with a warn and cannot clear or corrupt an existing catalog | S1, S2, S7, S15 | Convert each malformed shape → `None`; then drive `SessionController` with a valid push followed by a malformed one and assert the catalog still equals the first. A `Some(empty)` or a cleared catalog falsifies. Other cause: the required-field check firing on valid data — ruled out because the same test's valid frame must still convert | the frames are classified malformed by an independent `jq` predicate (`has("powers") and (.params.powers\|type=="array")` and per-item `has("name")`), not by the Rust code | in the converter, replace the `Option<Vec<_>>` extraction with `unwrap_or_default()` → the malformed-frame test fails (empty catalog instead of `None`) | `convert/kas/powers.rs` test `malformed_powers_frames_drop_and_do_not_clear` + `session.rs` catalog test | minutes | PENDING — checkpointed-build, slice 1 |
| C4 | Optional item fields fall back exactly as specified and never render placeholders | S8, S9, S10, S11, S12, S13 | Convert items with each optional field absent/empty and assert `title()`/`description()`/`mcp_server_names()`; render them and assert the empty case paints no token. A blank title, a literal "N/A", or a phantom `mcp` token falsifies. Other cause: the fallback hiding a decode failure — ruled out because the required `name` still fails the frame when absent (C3) | expected values are the spec's B1/B6 wording plus the live capture for the present-field case; the absent cases are hand-computed literals, not converter output | in `PowerInfo::title()`, drop the `is_empty` guard → an empty `displayName` yields a blank title and the fence fails | `types/power.rs` accessor test + `widgets/powers_panel.rs` test | minutes | PENDING — checkpointed-build, slice 2 |
| C5 | Command, push and ordering semantics hold | S6, S18, S21, S22 | Command test: no catalog → exactly one `SystemMessage` and no panel; catalog held → `ShowPowers` carrying the rows. State test: shuffled input renders in display-name order; two pushes replace; duplicates both render. App test: a push with no panel open leaves the panel closed; with one open, replaces content and clamps scroll. Other cause: the closed-panel refresh being a no-op for the wrong reason — ruled out by asserting the closed-panel catalog still updated | expected order comes from `sort -f` over the fixture's display names (shell oracle), and the duplicate case from an explicitly constructed two-item list | in `UiState::show_powers_panel`, remove the sort → the order fence fails; in the App arm, call `show_powers_panel` unconditionally → the no-auto-open fence fails | `commands` test `powers_without_catalog_reports_and_with_catalog_opens`; `state.rs` `powers_panel_orders_and_replaces`; `app.rs` `powers_push_updates_without_opening` | minutes | PENDING — checkpointed-build, slice 3 |
| C6 | The painted panel matches the approved layout and its bounds invariants | S16, S17, S18, S19, S20 | `TestBackend` renders at fixed sizes: 3-item layout at 80×24 (title, id, `mcp` servers, `steering`, truncated description), the placeholder for empty, a wide-CJK title at the exact panel width (no border overflow), a too-small area (nothing painted), a truncated refresh (scroll clamped). A missing marker, an overflowing row, or a covered input line falsifies. Other cause: assertions passing on an unrelated frame region — ruled out by asserting cell positions, not substrings alone | the expected layout is a hand-written table of cell columns for the fixed width, computed from the widget's documented geometry, and the existing `modals_never_cover_input` invariant | in `powers_panel.rs`, drop the width clamp on the title → the CJK test overflows the border and fails; drop the `steering` token → the layout test fails | `widgets/powers_panel.rs` tests + `floor_tests.rs` input-coverage case | minutes | PENDING — checkpointed-build, slice 2 |
| C7 | No outbound powers traffic exists, and the fence can see powers strings at all | the whole production tree | (a) census `crates/` for `powers/list`/`powers/refresh` → any hit falsifies; (b) drive `/powers` through the bridge test harness and assert the recorded outbound methods contain no powers method while containing at least one other method (the positive control an absence assertion requires) | (a) an independent `grep -rn` over the tree; (b) the bridge harness records frames at the transport boundary, downstream of the command's code path | add `ctx.bridge.send(...)` with a `_kiro/powers/list` frame in `/powers` → both the census and the bridge test fail | `crates/cyril/tests/powers_source_fence.rs` (grep census, pattern of `nd4h_source_fences.rs`) + bridge test | minutes | PENDING — checkpointed-build, slice 3 |
| C8 | Production paths, imports and protected-parent deltas match the ledger | the whole diff | Run `.cyril-v19o/oracles/module_shape.py`: a missing/renamed module, a wire type under `cyril-ui`, a `serde_json` import in the widget, or a protected-parent delta outside the allowed arms fails with the claim ID and path | the oracle is a source/diff census written independently of the Rust code and of the in-crate tests | move `WirePower` into `widgets/powers_panel.rs`, or add a `serde_json` import to the widget, or add an `App` field → the oracle reports the exact path and fails | `.cyril-v19o/oracles/module_shape.py` (+ `mutations.sh` for the red/green proof) | minutes | PENDING — checkpointed-build, per-slice gate |

## Non-goals and future work

**Permanent non-goals** (rationale recorded; no tracker issue):

- Parsing `keywords`, `isAgentPlugin`, or `_meta.kiro.resource` — no consumer exists for them under the approved layout (the row shows what a power activates: MCP servers and steering files). Adding a `plugin` marker to the meta line would change approved behavior and needs re-approval.
- Reading `~/.kiro/powers` from disk, or deriving any field (including the steering marker) from the filesystem — `hasSteeringFiles` is a file-count fact the wire already computes, and the design reads it verbatim (`evidence.md` P1 learning).
- Rendering powers anywhere but this panel (toolbar, status line, crew panel, autocomplete).
- Any pull or refresh path (`powers/list`, `powers/refresh`) — spec out-of-scope, re-confirmed by the requester.
- v2 powers support — no wire surface exists (`evidence.md` P3).

**Intended future work** (verified tracker IDs):

- **cyril-q159** (open, P2) — the open Agent Plugin format: what it is, and whether cyril consumes or supplies plugins. The `keywords[]`/`@powers` mention flow belongs to it.
- **cyril-58uv** (in_progress, P1) — make the extension-notification drop path visible. This design keeps its converter out of `convert/kiro.rs` so that work's catch-all instrumentation is unaffected; once it lands, the powers frame stops being counted as a silent drop.
- **cyril-nk4o** (open, P3) — the `_kiro/mcp/*` panel, the sibling that will reuse this panel's shape.
- **cyril-oiyt** (open, P4) — extending the hooks panel with the KAS registry; shares the drawer pattern but not this code.

## Falsifier run log

Cheapest falsifier first: **C0** (cost: seconds), run 2026-09-10 before design approval.

```
$ jq -S 'select(.msg.method == "_kiro/powers/items_changed") | .msg' \
      .cyril-v19o/kas-powers-2.21.2-2.21.2.jsonl > /tmp/frame.json
$ jq -S '.params.powers | length' /tmp/frame.json
3
# canonicalized comparison of the extracted frame against the raw capture,
# re-read with a second language (python json) — both must agree:
byte-identical after canonicalization: True
item fields: ['_meta', 'description', 'displayName', 'hasSteeringFiles',
              'isAgentPlugin', 'keywords', 'mcpServerNames', 'name']
names: ['aws-infrastructure-as-code', 'datadog', 'markdownlint']
displayNames: ['Build AWS infrastructure with CDK and CloudFormation',
               'Datadog Observability', 'Markdownlint']
mcp: [['awslabs.aws-iac-mcp-server'], ['datadog'], ['markdownlint']]
steering: [False, True, True]
```

Result: **PASS** — the fixture content is the live frame, so C1–C6 assert real wire data rather than values transcribed from a document.

Every other row is `PENDING — checkpointed-build` with its slice named; no row is `FAIL`.

## Approval

Requester approval (verbatim): `"Approve design.md"`
Date: 2026-09-10
Approved risk acceptances: C0's regression fence is `N/A — approved risk` (one-shot fixture-provenance check run before approval; the pinned values become permanent literals in C1's fence, and the raw capture, probe script and verdict are committed under `experiments/conductor-spike/` so provenance stays auditable).
