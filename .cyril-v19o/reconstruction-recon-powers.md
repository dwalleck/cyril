# Reconstruction (isolated, production code only) — reviewer `ReconPowers`

HEAD 88187579, base ab65e7a3. Produced by an agent that did not implement the change and was
instructed to read no `.cyril-v19o/` artifact.

## Summary

Reconstructed the `feat/cyril-v19o` change (HEAD 881875795b86cfa8d111cc9cdb13373e918b183e, base ab65e7a31ae7a84054f998ad056db65e1b12ef11) from production code only: a display-only KAS "powers" catalog. The change adds one wire adapter (crates/cyril-core/src/protocol/convert/kas/powers.rs), one domain type (crates/cyril-core/src/types/power.rs, re-exported as PowerInfo) plus a Notification variant, a session-side last-write-wins catalog + a /powers command that hands the catalog to the App, and a read-only cyril-ui panel (traits.rs::PowersPanelState, state.rs panel lifecycle, widgets/powers_panel.rs). Wire decoding is owned entirely by the KAS adapter behind Engine::convert_ext_notification; display is owned entirely by cyril-ui; the cyril binary's App is the only place that sees both and is pure wiring. No wire method name, JSON field spelling, or protocol concept appears in cyril-ui or in cyril/src (only a doc-comment path `~/.kiro/powers/`). Production growth in pre-existing files is 9-76 non-blank lines each, dominated by one dispatch arm, one accessor, one enum variant, one panel-state struct and its lifecycle methods; the remaining ~60% of the diff in those files is in-file #[cfg(test)] fences.

## Architecture

Two directions, three crates. Inbound (agent -> user): domain_mediator/inbound.rs:170 canonicalizes `_kiro/*` to `kiro/*` (mod.rs:692) and calls Engine::convert_ext_notification (engine.rs:193); KasEngine (engine.rs:323) runs workflow -> powers (engine.rs:340) -> shared kiro converter. convert/kas/powers.rs:104 decodes wire JSON into Notification::PowersChanged (types/event.rs:308). App::handle_notification fans it out: SessionController::apply_notification stores the catalog (session.rs:223) and UiState::refresh_powers_panel updates only an open panel (app.rs:1419). Outbound (user -> panel): `/powers` -> CommandRegistry (commands/mod.rs:396) -> PowersCommand::execute reads SessionController::powers (builtin.rs:489) -> CommandResultKind::ShowPowers (commands/mod.rs:215) -> App arm (app.rs:2037) -> UiState::show_powers_panel sorts (state.rs:2421) -> render.rs:156 -> widgets/powers_panel.rs:46. Dependency direction is cyril -> cyril-ui -> cyril-core only (cyril-ui/Cargo.toml declares cyril-core; cyril-core never names UI types). The wire/presentation seam holds in both directions: no `kiro/powers`, `items_changed`, `mcpServerNames`, `displayName` or `_kiro/` string exists in crates/cyril-ui/src or crates/cyril/src for powers (only a doc comment naming the on-disk path `~/.kiro/powers/` at powers_panel.rs:111), and cyril-core's command result carries the domain type, not a panel type.

# Reconstructed module map — `feat/cyril-v19o` (HEAD 88187579, base `ab65e7a31`)

Method: `git diff --stat $(git merge-base origin/main HEAD)..HEAD` inside `/home/dwalleck/repos/cyril-wt-feat-cyril-v19o` yields 47 files. 16 are production (non-test) sources; the remainder are `.cyril-v19o/` evidence (excluded by instruction), `experiments/conductor-spike/` probe artifacts, and tests/examples. Everything below is read off production code; where a claim is not derivable from code I say so.

**Domain in one paragraph (as the code states it):** KAS (kiro-cli's agent) announces its installed *powers* with a single unprompted push, `_kiro/powers/items_changed`; cyril renders that catalog read-only in a `/powers` overlay and never requests it, because the sibling pull methods `_kiro/powers/list` (unadvertised) and `_kiro/powers/refresh` (answers `-32603 Unknown ext method`) are unusable — stated at `crates/cyril-core/src/protocol/convert/kas/powers.rs:17-33` and `crates/cyril-core/src/commands/builtin.rs:456-467`, and enforced by a production-source census fence (`crates/cyril/tests/powers_source_fence.rs:141`).

---

## 1. Changed production modules

### cyril-core — inbound (wire)

**`crates/cyril-core/src/protocol/convert/kas/powers.rs`** (NEW, 336 lines)
- Interface (crate-visible): `pub(crate) const METHOD: &str = "kiro/powers/items_changed"` (:44); `pub(crate) fn to_notification(method: &str, params: &serde_json::Value) -> crate::Result<Option<Notification>>` (:104).
- Private: `WirePowersChanged` (:52), `WirePower` (:61, camelCase, `#[serde(default)]` on `mcp_server_names`/`has_steering_files`), `impl From<WirePower> for PowerInfo` (:78), `fn parse` (:128, via `serde_path_to_error`).
- Responsibility: the KAS powers adapter — the only code that knows the wire method name and the wire field spellings; it decodes a push into `Notification::PowersChanged`, and maps "this method, undecodable payload" to a warn + `Ok(None)` (:113-121) rather than an error.

**`crates/cyril-core/src/protocol/convert/kas.rs`** (MOD, +1)
- Interface: module declaration only — `pub(crate) mod powers;` (:18).
- Responsibility: the KAS dialect module list; `powers` is a sibling of `workflow` (:19).

**`crates/cyril-core/src/protocol/engine.rs`** (MOD, +9 production lines; +48 test lines)
- Interface: `Engine::convert_ext_notification` (:193 trait, :227 V2 impl, :323 Kas impl); the change is inside the Kas impl, which now tries the powers adapter at :340 before falling through to `convert::kiro::to_ext_notification`.
- Responsibility of the inserted code: dialect dispatch order — claim the powers family before the shared converter so a malformed powers frame is reported by the adapter that recognizes it.
- Not determined from production code: whether v2 ever sees a powers frame at runtime. What the code shows is that the v2 path has no powers arm at all (`convert/kiro.rs:335` is a `match method` with no powers case), so `kiro/powers/items_changed` falls through to the unknown-method `Ok(None)`.

### cyril-core — domain vocabulary

**`crates/cyril-core/src/types/power.rs`** (NEW, 147 lines)
- Interface (public): `pub struct PowerInfo` (:19) with private fields; `pub fn new(name: impl Into<String>, display_name: Option<String>, description: Option<String>, mcp_server_names: Vec<String>, has_steering_files: bool) -> Self` (:36); accessors `name()` (:54), `title()` (:64), `description()` (:74), `mcp_server_names()` (:81), `has_steering_files()` (:88). Derives `Debug, Clone, PartialEq, Eq` (:18).
- Responsibility: the render-ready facts about one installed power, with the two normalization rules (empty string means absent; title falls back to name) applied once at construction (:48-50).

**`crates/cyril-core/src/types/event.rs`** (MOD, +17)
- Interface: new variant `Notification::PowersChanged { powers: Vec<PowerInfo> }` (:308).
- Responsibility: carries the full replacement catalog through the notification bus with its semantics attached (replacement not delta; must not pop a modal).

**`crates/cyril-core/src/types/mod.rs`** (MOD, +2)
- Interface: `pub mod power;` (:13), `pub use power::PowerInfo;` (:49).

### cyril-core — session state and command control flow

**`crates/cyril-core/src/session.rs`** (MOD, +24 production lines; +43 test lines)
- Interface: private field `powers: Option<Vec<PowerInfo>>` (:52); `pub fn powers(&self) -> Option<&[PowerInfo]>` (:137); the `apply_notification` arm (:223-227) returning whether the catalog changed.
- Responsibility: last-write-wins storage of the catalog, and the `None` vs `Some(vec![])` distinction (“not reported yet” vs “reported empty”).

**`crates/cyril-core/src/commands/mod.rs`** (MOD, +25 production lines; +114 test lines)
- Interface: `CommandResultKind::ShowPowers { powers: Vec<crate::types::PowerInfo> }` (:215); `pub fn CommandResult::show_powers(powers: Vec<PowerInfo>) -> Self` (:253); registration at :396-397.
- Responsibility: carrying the catalog from the command layer (where the session is reachable) to the App (where the UI is reachable), and owning the command name.

**`crates/cyril-core/src/commands/builtin.rs`** (MOD, +41, all production)
- Interface: `pub struct PowersCommand` (:468) + `impl Command` (:471) — `name()` → `"powers"`, `description()`, `async fn execute(&self, ctx: &CommandContext<'_>, args: &str) -> crate::Result<CommandResult>`.
- Responsibility: the `/powers` command — arg validation (:479-485), the unloaded-vs-empty decision (:489-497), no bridge traffic.

### cyril-ui — presentation

**`crates/cyril-ui/src/traits.rs`** (MOD, +19 production lines; +5 test)
- Interface: `TuiState::powers_panel(&self) -> Option<&PowersPanelState>` (:108); `pub struct PowersPanelState { pub powers: Vec<PowerInfo>, pub scroll_offset: usize }` (:527-535); test-double field/impl (:783, :928).
- Responsibility: the panel's data contract — display-ordered catalog + index-based scroll offset (not a line offset).

**`crates/cyril-ui/src/state.rs`** (MOD, +76 production lines; +60 test — the largest production growth besides the widget)
- Interface: private field (:111); `TuiState::powers_panel` impl (:301); init (:413); `pub fn show_powers_panel(&mut self, powers: Vec<PowerInfo>)` (:2421); `pub fn refresh_powers_panel(&mut self, powers: Vec<PowerInfo>) -> bool` (:2444); `pub fn hide_powers_panel` (:2457); `pub fn has_powers_panel` (:2462); `pub fn powers_panel_scroll_up/down(&mut self, n: usize)` (:2467, :2476).
- Responsibility: the panel's lifecycle and policy — display ordering on insert (:2422-2424), “refresh only if open + clamp scroll” (:2444-2453), scroll bounds. It explicitly does *not* handle the push (`Notification::PowersChanged { .. } => false`, :1144).

**`crates/cyril-ui/src/widgets/powers_panel.rs`** (NEW, 417 lines)
- Interface: `pub fn render(frame: &mut Frame, area: Rect, input_top: u16, state: &PowersPanelState, theme: &Theme)` (:46); private consts `LINES_PER_POWER = 3` (:30), `MAX_VISIBLE_POWERS = 5` (:31), `INDENT` (:33), `CHROME_ROWS = 4` (:35).
- Responsibility: cell layout only — place the popup (`modal::place`, :59), title count, the empty placeholder (:76-88), and three fixed lines per power (title / id + `mcp <server>` + `steering` / description) truncated to width.

**`crates/cyril-ui/src/widgets/mod.rs`** (MOD, +1): `pub mod powers_panel;` (:10).
**`crates/cyril-ui/src/render.rs`** (MOD, +3): draw order in `draw_inner` — hooks, then powers, then code/usage (:156-158).
**`crates/cyril-ui/src/theme.rs`** (MOD, +1): the widget joins the theme-source census (`include_str!` at :1827).

### cyril (binary orchestrator)

**`crates/cyril/src/app.rs`** (MOD, +39 production lines; +147 test)
- Interface added: free fn `dispatch_powers_panel_key(key: KeyEvent, ui_state: &mut cyril_ui::state::UiState)` (:2782) — Esc/Up/Down/PageUp/PageDown → `hide`/`scroll_*` (steps 1 and 5).
- Responsibility of the inserted code: the only place both the session and the UI exist — route the push to `refresh_powers_panel` (:1419-1423), include the panel in the modal key-routing chain (:1663-1667) and in the “input owns the keyboard when no overlay” guard (:1585), and open the panel from the command result (:2037-2042).

---

## 2. Responsibility clusters

| # | Cluster | Modules | The rule that separates it |
|---|---|---|---|
| 1 | **Wire adaptation** | `protocol/convert/kas/powers.rs`, `protocol/convert/kas.rs`, the dispatch arm in `protocol/engine.rs:340` | Code that may contain a wire method name or a wire JSON field spelling lives here and nowhere else. Its output is a `Notification`; it never touches session state, commands, or cells. |
| 2 | **Domain vocabulary** | `types/power.rs`, `types/event.rs` variant, `types/mod.rs` re-export | Types both the adapter and the renderer may name, with no I/O and no wire spellings; normalization/derivation rules on the data (`title` fallback, empty⇒absent) live with the data, not with a consumer. |
| 3 | **Catalog state + user-facing routing** | `session.rs` (store/replace), `commands/mod.rs` (`ShowPowers`, registration), `commands/builtin.rs` (`PowersCommand`), `crates/cyril/src/app.rs` (all five insertions) | State transitions caused by wire pushes or user input, plus the policy that depends on both layers being visible: what opens when, what a missing catalog says, which key goes where. May name UI methods (`has_powers_panel`) but never a wire name. |
| 4 | **Presentation** | `cyril-ui/src/traits.rs` (state struct), `state.rs` (panel lifecycle), `widgets/powers_panel.rs` (cells), `render.rs` (draw order), `theme.rs` (census entry) | Anything that decides what the user sees or how the panel behaves on screen. Rule of thumb visible in the code: ordering belongs to `UiState` (state.rs:2421), geometry belongs to the widget (powers_panel.rs:53-100), and no module here may decode JSON or name a wire method. |

Where the boundary is *observable*: cluster 4 can be exercised with zero knowledge of KAS (the widget tests build `PowersPanelState` directly, powers_panel.rs:237), and cluster 1 can be exercised with zero knowledge of ratatui (the converter test asserts on `Notification` values only, kas/powers.rs:152-176).

---

## 3. Dependency direction and concrete adapters

**Crate direction (manifests):** `cyril` → `cyril-ui` → `cyril-core`. `crates/cyril-ui/Cargo.toml` declares `cyril-core = { path = "../cyril-core" }`; nothing in cyril-core names a `cyril-ui` type — the command layer's payload is `crate::types::PowerInfo` (commands/mod.rs:216), which is why the `ShowPowers` variant exists at all (rationale in-code at commands/mod.rs:206-210: “the App is the only place both exist”).

**Inbound path (agent → screen), with the seams named:**

1. `crates/cyril-core/src/protocol/domain_mediator/mod.rs:692` `canonical_extension_method` strips the leading `_`, so the engine sees `kiro/powers/items_changed`, never the raw spelling.
2. `crates/cyril-core/src/protocol/domain_mediator/inbound.rs:170` — `handle_extension_notification` calls `self.config.engine.convert_ext_notification(method, params)`. **This is the adapter seam** (trait at engine.rs:193).
3. `crates/cyril-core/src/protocol/engine.rs:323-350` — KasEngine's dialect dispatch: workflow adapter → **powers adapter (engine.rs:340)** → shared v2 kiro converter. V2Engine (engine.rs:227) delegates straight to `convert::kiro::to_ext_notification`, whose `match method` (convert/kiro.rs:335) has no powers arm.
4. `crates/cyril-core/src/protocol/convert/kas/powers.rs:104` — the concrete adapter: method gate, path-tracked decode, `warn!` + drop on malformed payload.
5. `crates/cyril/src/app.rs:1351,1353` — `App::handle_notification` applies the notification to `self.session` and `self.ui_state`; the powers-specific routing is at :1419-1423.

**Outbound path (keystroke → screen):** `/powers` → `CommandRegistry` (registered at commands/mod.rs:396-397) → `PowersCommand::execute` reads `ctx.session.powers()` (builtin.rs:489) → `CommandResultKind::ShowPowers` (commands/mod.rs:215) → `App` arm (app.rs:2037-2042) → `UiState::show_powers_panel` (state.rs:2421) → `render::draw_inner` (render.rs:156) → `widgets::powers_panel::render` (powers_panel.rs:46).

**Wire-into-presentation leak: none found.** Grep over `crates/cyril-ui/src` and `crates/cyril/src` for `_kiro/`, `kiro/powers`, `items_changed`, `mcpServerNames`, `hasSteeringFiles`, `displayName` returns only: a doc comment naming the on-disk directory `~/.kiro/powers/` (widgets/powers_panel.rs:111) and unrelated pre-existing usage-account keys (`app.rs:2522-2525`). The presentation type holds the **domain** type (`PowersPanelState.powers: Vec<PowerInfo>`, traits.rs:532), not a decoded wire struct or a raw `serde_json::Value`.

**Presentation-into-wire leak: none.** `convert/kas/powers.rs` imports only `serde::Deserialize`, `crate::types::{Notification, PowerInfo}` and `serde_path_to_error` (powers.rs:40-41) — no ratatui, no panel type, no terminal concept. The UI-shaped field (`scroll_offset`) never crosses into cyril-core.

**One direction worth naming explicitly (not a leak, a deliberate choice):** the *policy* for the unprompted push lives in the App + UI, not in the transport. `UiState::apply_notification` explicitly refuses the notification (`Notification::PowersChanged { .. } => false`, cyril-ui/src/state.rs:1144) and the App does the “only if already open” refresh (app.rs:1419-1423, state.rs:2444-2453). So the transport layer emits a fact and the presentation layer owns the reaction; a reader looking for a subscription mechanism will not find one.

---

## 4. Pass-through modules and hypothetical seams

**Modules that only forward data (no logic of their own):**

- `crates/cyril-core/src/types/mod.rs:13,49` — module declaration + `pub use`. Textbook re-export.
- `crates/cyril-core/src/protocol/convert/kas.rs:18` — module declaration only.
- `crates/cyril-core/src/commands/mod.rs:253-257` — `show_powers` is a constructor that wraps its argument in the enum variant; the doc/behaviour split is documented as intentional at commands/mod.rs:206-210.
- `crates/cyril-core/src/session.rs:137-139` — `powers()` is `self.powers.as_deref()`.
- `crates/cyril-ui/src/state.rs:301-303` (`powers_panel()`), `:2457-2459` (`hide_powers_panel`), `:2462-2464` (`has_powers_panel`) — accessors/flag flips.
- `crates/cyril-ui/src/render.rs:156-158` — forwards the `Option<&PowersPanelState>` to the widget, exactly like every sibling panel.
- `crates/cyril/src/app.rs:2037-2042` — the `ShowPowers` arm is annotated in-code as “pure wiring”; it sets `redraw_needed` and nothing else.

The closest thing to a *non*-pass-through thin module is `PowersCommand::execute` (builtin.rs:473-498): 6 lines of behaviour (usage error, unloaded message, panel dispatch) — small, but it is the only place that decides “no catalog ⇒ say so rather than open an empty panel”.

**Seams a reviewer might claim should exist, and where they actually are not:**

1. **A request/refresh seam.** There is no bridge command, no method constant, and no request path for powers anywhere in production code. The only powers method any `crates/*/src/**/*.rs` file may name is the push — enforced by a census over the source tree (`crates/cyril/tests/powers_source_fence.rs:141`), with the transport-level negative asserted in `crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/powers.rs:98-112` (no ledger entry containing `powers`/`items_changed`).
2. **An identity/addressability seam.** `PowerInfo` deliberately has no `PowerId` and no per-power action (types/power.rs:8-14); `name()` exists only to key a row and break sort ties.
3. **A domain→presentation view-model seam.** None exists: `PowersPanelState.powers` is `Vec<cyril_core::types::PowerInfo>` (traits.rs:532), so cyril-ui is coupled to the core domain type directly (the same choice as `HooksPanelState.hooks: Vec<HookInfo>`). A reviewer wanting a `PowerRow`/`PowerView` mapping layer would be arguing against the file's own precedent.
4. **A single-owner seam for the catalog.** The catalog is copied at least twice per push: `SessionController` stores a clone (session.rs:225) and the App clones again per notification (`powers.clone()`, app.rs:1420) into the panel. Nothing coordinates the two copies (they cannot diverge in practice today only because both are written from the same notification).
5. **A widget/state seam for the window height.** The widget computes its own visible window (`(height - CHROME_ROWS) / LINES_PER_POWER`, powers_panel.rs:98-100) while `UiState` owns the scroll offset — and the App's page step hard-codes `5` (app.rs:2787-2788), duplicating `MAX_VISIBLE_POWERS` (powers_panel.rs:31) in a third file. There is no shared constant seam.
6. **A converter-injection seam.** `parse` is private and `to_notification` is `pub(crate)`; the module exposes no trait. Tests reach the fixture, not an injected decoder (kas/powers.rs:146 `include_str!`), and the fixture lives under `crates/cyril-core/tests/fixtures/` — the src↔tests coupling is test-build-only.

---

## 5. Protected-parent growth

Sizes at base → HEAD (`git show <base>:file | wc -l` vs `wc -l`), and the production/test split of added **non-blank** lines (computed from `git diff -U0` against the first `#[cfg(test)]` / `test_support` boundary in the HEAD file):

| File | base → HEAD | prod added | test added | What exactly grew | Behaviour/wiring or new responsibility |
|---|---|---|---|---|---|
| `crates/cyril-ui/src/state.rs` | 7926 → 8076 | 76 | 60 | 1 field (:111), 1 trait impl (:301), 1 init (:413), 1 notification arm (:1144), 6 panel methods (:2421-2480) | **Wiring + a new instance of an existing responsibility.** The panel lifecycle is the panel-pattern already used by hooks (`show_hooks_panel` :2344, `refresh_hooks_panel` :2369); the ordering rule is new *content*, not a new kind of duty. The notification arm adds one line and deliberately declines ownership. |
| `crates/cyril/src/app.rs` | 6857 → 7058 | 39 | 147 | 1 push-refresh arm (:1419-1423), 1 guard term (:1585), 1 key-routing branch (:1663-1667), 1 command-result arm (:2037-2042), 1 free key-dispatch fn (:2782-2791) | **Wiring.** Every insertion is a dispatch line; the only new function is a 10-line key map copied in shape from `dispatch_hooks_panel_key` (same file, documented as an extraction for testability at :2776-2781). |
| `crates/cyril-core/src/commands/mod.rs` | 1438 → 1586 | 25 | 114 | 1 enum variant (:215-217), 1 constructor (:253-257), 2 registration lines (:396-397) | **Wiring**, but note the payload: the variant carries `Vec<PowerInfo>` into the command-result vocabulary, which is the mechanism by which cyril-core can hand UI-bound data to the App without naming a UI type. |
| `crates/cyril-core/src/session.rs` | 1247 → 1321 | 24 | 43 | 1 field (:52), 1 accessor (:137-139), 1 match arm (:223-227) | **New state, existing responsibility.** Session already stores `kas_hooks` on the same pattern (:45-48); this is a second instance of “last push wins, accessor for readers”. |
| `crates/cyril-core/src/protocol/engine.rs` | 839 → 901 | 9 | 48 | 3-line `if let` inside the existing ext-conversion match (:340-342) + comment | **Wiring.** Dispatch order only. |
| `crates/cyril-core/src/types/event.rs` | 1128 → 1146 | 17 | 0 | 1 `Notification` variant + doc (:308-310) | **New responsibility in a shared enum.** This is the one pre-existing file where the change adds a *kind* rather than an instance: every `match` on `Notification` downstream is now obligation-bearing. The four panels' worth of precedent (HooksChanged at :291) makes it the established mechanism. |
| `crates/cyril-ui/src/traits.rs` | 1324 → 1348 | 19 | 5 | 1 trait method (:108), 1 state struct (:527-535), 3 test-double lines (:783, :827, :928) | **Interface growth.** Adding a method to `TuiState` is a contract change for every implementor (two in-tree: `UiState` and the test double). |
| `crates/cyril-ui/src/render.rs` | 1492 → 1495 | 3 | 0 | 3-line overlay block (:156-158) | **Wiring.** |
| `crates/cyril-core/src/commands/builtin.rs` | 455 → 500 | 41 | 0 | one command impl (:456-500) | **New responsibility, in the file whose whole responsibility is “one command per module-level item”.** No pre-existing behaviour was modified. |

Pattern across the six largest parents: **the majority of each file's diff is its own in-file regression fence** (commands/mod.rs 114/139, app.rs 147/186, engine.rs 48/57, session.rs 43/67 lines). Production growth per parent is 9–76 lines and is structurally 1–2 lines of dispatch plus one accessor/arm — no parent gained a new role except `commands/builtin.rs` (a new command) and `types/event.rs` (a new notification kind).

---

## 6. Tests reaching past an interface

Strongest cases, with the mechanism named:

1. **`crates/cyril/tests/powers_source_fence.rs` — the fence is entirely about source text, not behaviour.**
   - `no_production_source_names_an_unusable_powers_method` (:141): walks the filesystem (`production_sources`, :85-104), reads every `.rs` under `crates/*/src`, strips `//` comments (`strip_line_comments`, :63), and asserts a string census. It asserts nothing about how any module behaves.
   - `powers_census_detects_the_methods_it_exists_to_catch` (:194) and `powers_census_is_line_ending_agnostic` (:259) unit-test the **private** scanner helper `unimplemented_powers_methods` (:116) — reaching into the scanner's internals rather than any interface.
2. **`crates/cyril-ui/src/widgets/powers_panel.rs` tests construct the state struct directly, bypassing the module that owns ordering.** `PowersPanelState { powers, scroll_offset }` literals at :237, :272, :315, :330, :379, :392, :406 bypass `UiState::show_powers_panel` (state.rs:2421) — the only production code that sorts. The test helper then has to *restate* the ordering contract (“the display order `UiState` produces”, :212-213) in its own fixture, so the test passes even if the ordering rule changes in `UiState`. The struct's own doc concedes the hazard: “A caller constructing `PowersPanelState` by hand owns that ordering” (traits.rs:530-531).
3. **Theme-source censuses read widget source files as text.** `crates/cyril-ui/src/theme.rs:1824-1831` `include_str!`s each widget module (new entry `widgets/powers_panel.rs`, :1827) and asserts on the text; `crates/cyril-ui/tests/widget_theme_sources.rs:4-20` does the same list from the filesystem (new row, :14). It catches “widget hardcodes a color” by string inspection, not by rendering.
4. **`crates/cyril/src/app.rs` in-module tests read App private state through accessors.** `powers_push_updates_without_opening_and_command_opens` (:5499-5532) asserts on `app.ui_state.has_powers_panel()` / `app.ui_state.powers_panel().expect("panel").powers.len()` and `app.session.powers()` — public accessors, but on fields private to `App`, so the assertions are about App’s internal wiring rather than about anything the user could observe. `powers_panel_key_map` (:5576+) drives the extracted free function `dispatch_powers_panel_key` (:2782) and then reads `ui_state.powers_panel().expect("panel").scroll_offset` — a public field of the panel state, i.e. below the `TuiState` seam it was reached through.
5. **`crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/powers.rs:21`** drives the real transport harness (interface-level: `BridgeCommand::NewSession`, the notification stream) but then reaches into the harness’s recorded ledger (`ledger.borrow().received()`, :99-112) to assert the *absence* of a powers request. That ledger is the transport’s own instrumentation, not a production interface — the closest the suite comes to a black-box assertion on this claim, but still below a public seam.

Borderline-but-fine, for contrast: `crates/cyril-core/src/protocol/convert/kas/powers.rs` tests call the module’s real entry point (`to_notification`, :153) and assert on the resulting `Notification`; `session.rs:1219` and `commands/mod.rs:764` use the public `apply_notification`/`Command::execute` paths. Those stay on the interface side.

---

## 7. Reconstructed ledger

| Module path | Responsibility | Who owns the interface |
|---|---|---|
| `crates/cyril-core/src/protocol/convert/kas/powers.rs` | Decode `kiro/powers/items_changed` params into `Notification::PowersChanged`; drop malformed frames with a field-path warn | cyril-core (owns it); consumed by `protocol/engine.rs:340` only; `pub(crate)` |
| `crates/cyril-core/src/protocol/convert/kas.rs` | KAS dialect module list (declares `powers`, `workflow`) | cyril-core |
| `crates/cyril-core/src/protocol/engine.rs` (arm :340) | Dialect dispatch order: workflow → powers → shared kiro converter | The `Engine` trait (`:193`); implemented by `KasEngine` (`:323`) and `V2Engine` (`:227`) |
| `crates/cyril-core/src/types/power.rs` | `PowerInfo`: render-ready facts about one power + normalization (empty⇒absent, title fallback) | cyril-core, public (`types/mod.rs:49`); read by cyril-core commands and by cyril-ui |
| `crates/cyril-core/src/types/event.rs` (:308) | `Notification::PowersChanged` — full-replacement catalog on the notification bus | cyril-core, public; consumed by `SessionController` and the App |
| `crates/cyril-core/src/types/mod.rs` (:13, :49) | Module declaration + re-export | cyril-core, public |
| `crates/cyril-core/src/session.rs` (:52, :137, :223) | Store the last catalog; expose `Option<&[PowerInfo]>`; replace-not-merge on push | `SessionController`, public; sole production reader is `PowersCommand` (builtin.rs:489) |
| `crates/cyril-core/src/commands/mod.rs` (:215, :253, :396) | `CommandResultKind::ShowPowers` payload + constructor + command registration | `CommandRegistry` / `CommandResult`, public; consumed by `crates/cyril/src/app.rs:2037` |
| `crates/cyril-core/src/commands/builtin.rs` (:468) | `/powers` command: arg check, unloaded-vs-empty decision, hand the catalog on | `Command` trait; registered by `CommandRegistry` |
| `crates/cyril-ui/src/traits.rs` (:108, :527) | `TuiState::powers_panel` + `PowersPanelState` data contract (display-ordered catalog, index scroll) | `TuiState` trait (cyril-ui); implemented by `UiState` and the test double |
| `crates/cyril-ui/src/state.rs` (:111, :301, :413, :1144, :2421-) | Panel lifecycle and policy: display ordering, refresh-only-if-open, scroll clamping | `UiState`, public to the binary; the push is explicitly *not* handled here |
| `crates/cyril-ui/src/widgets/powers_panel.rs` (:46) | Draw the overlay: placement, title count, empty placeholder, 3 fixed lines per power, width truncation | `pub fn render(...)`, called by `render.rs:157`; consumes only `PowersPanelState` |
| `crates/cyril-ui/src/widgets/mod.rs` (:10) | Widget module declaration | cyril-ui |
| `crates/cyril-ui/src/render.rs` (:156) | Draw order for overlays | `pub fn draw` / `draw_inner`, called by the binary (`app.rs:797`, `:912`) |
| `crates/cyril-ui/src/theme.rs` (:1827) | Theme-source census entry for the new widget | cyril-ui test surface |
| `crates/cyril/src/app.rs` (:1419, :1585, :1663, :2037, :2782) | Route push → open-only-if-open refresh; route keys; open panel from command result | The binary crate; the only consumer of both `SessionController::powers` and `UiState::show_powers_panel` |

**Not counted as production:** `crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/{mod.rs,powers.rs}`, `crates/cyril-core/src/protocol/bridge/tests/harness.rs` (adds `emit_powers_changed` to the fake agent and `powers_push_notification`, harness.rs:54-59, :495-521), `crates/cyril-ui/tests/widget_theme_sources.rs`, `crates/cyril/tests/powers_source_fence.rs`, `crates/cyril/examples/test_bridge.rs:647-661` (an example whose `print_notification` gained a `PowersChanged` arm), `crates/cyril-core/tests/fixtures/kas/powers/items-changed-2.21.2.json` (the captured wire frame), and everything under `.cyril-v19o/` and `experiments/conductor-spike/` (excluded by instruction / not production).

**Honest gaps — undetermined from production code alone:**
- Whether a v2 session can ever deliver a powers frame at runtime. Code shows only that the v2 converter has no powers arm (convert/kiro.rs:335) and that `PowersCommand` is registered on every engine (commands/mod.rs:390-397); the justification (“25 advertised commands, none `powers`”) exists only as a code comment (builtin.rs:466-467), not as executable evidence.
- Whether `keywords`, `isAgentPlugin`, `_meta` carry anything cyril will later need — the adapter states they are ignored for lack of consumers (kas/powers.rs:34-36); nothing in production code contradicts that.
- Whether the duplicated page-size constant (app.rs:2787-2788 vs powers_panel.rs:31) is a known trade-off or an oversight; the code does not say.
