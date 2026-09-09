# Plan: Activate bundled themes and terminal color modes

Issue: **cyril-qaq0**. Design: `.cyril-qaq0/design.md` (approved 2026-09-09).
Route: Structural. Baselines are measured on the merge target after cyril-fkke
lands (six palettes, `ThemeId::ALL`).

## Module growth ledger

| Module | Baseline production lines | Projected final lines | Responsibility change | Interface change | Protected-parent rule |
|---|---:|---:|---|---|---|
| `crates/cyril-ui/src/theme.rs` | 655 | 760–820 | add: id/mode spelling, detection policy | `+parse_theme_id`, `parse_color_mode`, `detect_color_mode`, `ColorEnvironment`, `StartupAppearance` | N/A |
| `crates/cyril-ui/src/state.rs` | 2483 | 2560–2620 | add: committed appearance + preview + picker commit | `+set_appearance`, `open_theme_picker`; `TuiState::theme` unchanged | N/A |
| `crates/cyril-ui/src/traits.rs` | 929 | 935–945 | widen: picker view model | `PickerState + kind` | N/A |
| `crates/cyril-ui/src/render.rs` | 169 | 169 | none (test module only) | none | N/A |
| `crates/cyril-core/src/commands/mod.rs` | 471 | 475–485 | widen: result vocabulary | `+CommandResultKind::ShowThemePicker` | N/A |
| `crates/cyril-core/src/commands/builtin.rs` | 429 | 455–470 | widen: one local command | `+theme` | N/A |
| `crates/cyril-core/src/types/config.rs` | 126 | 145–160 | widen: schema | `UiConfig + theme, color_mode` | N/A |
| `crates/cyril/src/app.rs` | 2766 | 2820–2860 | widen: consume config, route result, picker Enter branch | `App::new(+ ColorEnvironment)` | N/A |
| `crates/cyril/src/main.rs` | 290 | 305–315 | widen: env probe | none (internal) | must not read `UiConfig` fields (existing C3 fence) |
| `crates/cyril-ui/src/widgets/` | — | — | none | none | zero production delta |
| `crates/cyril-memory/` | — | — | none | none | zero production delta |
| `crates/cyril-core/src/protocol/` | — | — | none | none | zero production delta |

No module gains a second responsibility cluster. `render.rs` changes only its
`#[cfg(test)]` module; `traits.rs` widens one struct by one field.

## Partition arithmetic

| Slice | Diff estimate |
|---|---:|
| Slice 1 — render fence | 45 |
| Slice 2 — vocabulary and detection | 320 |
| Slice 3 — config, startup, message | 380 |
| Slice 4 — picker | 480 |
| **Sum** | **1225** |
| Churn margin (25% — a new config key and a new command both ripple into doc surfaces and fences) | 306 |
| **Total** | **1531** |

1531 ≤ 4,000, so the plan has a single PR increment.

**PR increment: `qaq0-activation`** — slices 1–4, stacked on `main` after
cyril-fkke merges. Mergeable definition: the whole increment leaves
`cargo fmt --check`, `cargo clippy -- -D warnings`, and the workspace tests
green, and every falsifier row discharged. What verifies without later
increments: N/A — the plan has one increment.

---

## Slice 1: Render fence covers every bundled theme × mode

**Claim IDs:** C6
**Expected behavior:** the mode matrix renders all 6 themes × 4 modes × 4 scenes (96 passes, 96 distinct labels, 24 no-color passes with zero non-Reset cells) and every cell's color equals that theme's resolved role.
**Oracle:** the pinned 7,680-cell Cyril Dark fixture plus per-role projection computed from `resolve(id, mode)` — independent of the renderer.
**Stress fixture:** the 96-combination matrix itself; expected 96 passes and 24 zero counts. A marker built once for all themes must fail (proven: `syntax expected Rgb(204,153,204), actual Rgb(180,142,173)`).
**Regression fence:** `render::tests::all_scene_theme_mode_combinations_pass` (renamed from `all_sixteen_scene_mode_combinations_pass`), created in this slice.
**Named mutation:** make `theme::resolve` ignore its `id` argument and always return the Cyril Dark palette → the fence red for non-CyrilDark labels.
**Complexity/production scale:** `N/A — test-only`; no production loop is added.
**Wall budget/phase:** `N/A — reason: test-only slice; no runtime phase`.
**Module shape:** responsibility none (test module only); owner `render.rs`; interface unchanged; protected parents unchanged; shape fence `.cyril-qaq0/oracles/module_shape.py` reports no production delta for `render.rs`.
**Files:** `crates/cyril-ui/src/render.rs` (`#[cfg(test)]` module only).
**Estimate:** 30 minutes.
**Diff estimate:** 45 lines (+35/−10).
**PR increment:** `qaq0-activation`.
**Commands and expected results:**
- `cargo test -p cyril-ui --lib all_scene_theme_mode_combinations_pass` → passes with 96 combinations; the pre-extension 16-combination shape is gone.
- `cargo test -p cyril-ui --lib migrated_scenes_match_all_pinned_cells` → 0/7,680 differences (Cyril Dark baseline unchanged).
- named mutation → fence red naming a non-CyrilDark label; restore → green.

## Slice 2: Appearance vocabulary and detection policy

**Claim IDs:** C1, C2, C3
**Expected behavior:** the accepted theme ids and mode strings map exactly; every other string returns `None`; detection returns the spec table's mode for every injected environment combination, with an explicit value overriding all rules.
**Oracle:** `.cyril-qaq0/oracle-theme-ids.py` and `.cyril-qaq0/oracle-precedence.py` — spec-encoded tables that never read the Rust implementation, compared by `.cyril-qaq0/oracles/compare_appearance.py`.
**Stress fixture:** rejected spellings (`""`, `CyrilDark`, `cyril_dark`, trailing space, `solarized`; `""`, `auto`, `24bit`, `ANSI256`, trailing space) must all return `None`; the 14-row precedence set must match row by row.
**Regression fence:** `theme::tests::parse_theme_id_covers_exactly_the_bundled_ids`, `theme::tests::parse_color_mode_covers_exactly_the_five_values`, `theme::tests::detection_precedence_matches_every_table_row` — created in this slice.
**Named mutation:** make `parse_theme_id` return `Some(ThemeId::CyrilDark)` for unknown input → id fence red; move the `NO_COLOR` check above the explicit-config check → precedence fence red.
**Complexity/production scale:** detection is O(1) (constant comparisons, no allocation); parse is O(1) table lookup; production input is one config value per startup. Maximum accepted cost: `N/A — reason: one-off startup resolution; the bound is a handful of comparisons`.
**Wall budget/phase:** `N/A — reason: one-off phase (startup); no wall budget`.
**Module shape:** adds the id/mode spelling and detection-policy responsibility to `theme.rs` (owner after slice: `theme.rs`); interface adds four functions and two structs; protected parents unchanged; shape fence reports `cyril-ui/theme.rs` as the only changed owner.
**Files:** `crates/cyril-ui/src/theme.rs`, `.cyril-qaq0/oracles/compare_appearance.py`.
**Estimate:** 2 hours.
**Diff estimate:** 320 lines (+180 implementation, +140 tests).
**PR increment:** `qaq0-activation`.
**Commands and expected results:**
- `python3 .cyril-qaq0/oracles/compare_appearance.py` → `PASS` with item-by-item agreement on the accepted sets and all 14 precedence rows.
- `cargo test -p cyril-ui --lib theme::tests::parse_` and `theme::tests::detection_precedence_matches_every_table_row` → pass.
- named mutations → the named fence red; restore → green.

## Slice 3: Config surface, startup resolution, and the invalid-value message

**Claim IDs:** C7, C8, C11
**Expected behavior:** `[ui] theme` and `[ui] color_mode` select the startup appearance; absent keys take the defaults; an unknown string yields exactly one visible system message naming the value and the default while every other key is honored; a wrong-typed value keeps the whole-file default posture; with no config file the rendered output is byte-identical to today.
**Oracle:** the existing pinned 7,680-cell baseline (C7) and the existing nd4h legacy-compat test shape (C11), both independent of the new fields.
**Stress fixture:** `.cyril-qaq0/config-nowrite-fixture.toml` with `theme = "cyrl-light"` and with `theme = 3`, plus a non-default sibling `max_messages = 999` as the discriminator in each case.
**Regression fence:** `config::tests::wrong_typed_theme_falls_back_to_whole_file_defaults` (+ mode twin), `app::tests::unknown_theme_value_renders_one_system_message` (+ mode twin), `state::tests::new_state_uses_cyril_dark_truecolor`, `migrated_scenes_match_all_pinned_cells` — created in this slice.
**Named mutation:** drop the diagnostic push in `App::new` → message fence red; add `#[serde(deserialize_with)]` that silently defaults the field → wrong-type fence red; change the `UiState::new` default palette → pinned baseline red.
**Complexity/production scale:** O(1) per startup; two string comparisons plus one message push. Maximum accepted cost: `N/A — reason: one-off startup resolution`.
**Wall budget/phase:** `N/A — reason: one-off phase (startup); no wall budget`.
**Module shape:** config schema widens (`cyril-core/types/config.rs`), `UiState` gains appearance storage (`cyril-ui/state.rs`), `App::new` consumes the two fields (`cyril/src/app.rs`), `main.rs` probes the environment; protected parents `widgets/`, `cyril-memory/`, `protocol/` stay at zero production delta; shape fence reports exactly these four owners.
**Files:** `crates/cyril-core/src/types/config.rs`, `crates/cyril-ui/src/state.rs`, `crates/cyril/src/app.rs`, `crates/cyril/src/main.rs`, plus the tests named above and the `.agents/summary/codebase_info.md` `[ui]` key table.
**Estimate:** 3 hours.
**Diff estimate:** 380 lines (+200 implementation, +180 tests).
**PR increment:** `qaq0-activation`.
**Commands and expected results:**
- `cargo test -p cyril-core --lib config::tests::wrong_typed_theme_falls_back_to_whole_file_defaults` → passes; sibling `max_messages` is 500 (whole-file default), not 999.
- `cargo test -p cyril --lib app::tests::unknown_theme_value_renders_one_system_message` → exactly one message containing `cyrl-light` and `cyril-dark`.
- `cargo test -p cyril-ui --lib migrated_scenes_match_all_pinned_cells` → 0/7,680 differences.
- named mutations → the named fence red; restore → green.

## Slice 4: `/theme` picker, preview, session commit, and no writes

**Claim IDs:** C4, C5, C9, C10
**Expected behavior:** `/theme` opens a picker of the six bundled themes with no bridge traffic; moving the highlight previews that theme in the current color mode; Enter commits for the session and closes; Esc discards and closes; no configuration file is created, opened for writing, or modified by any picker path.
**Oracle:** raw file bytes of `.cyril-qaq0/config-nowrite-fixture.toml` before/after a picker session (C5), the test bridge's send log (C9), and a production source census (C10).
**Stress fixture:** the picker driven to its first, last, and middle rows, with a filter typed and cleared, then Enter and Esc; expected committed theme equals the highlighted row on Enter and equals the pre-open theme on Esc; the fixture file bytes identical in both sessions.
**Regression fence:** `app::tests::theme_picker_session_leaves_config_bytes_identical`, `state::tests::enter_commits_and_esc_restores`, `state::tests::theme_returns_preview_while_theme_picker_is_open`, `app::tests::theme_command_opens_picker_without_bridge_traffic`, `.cyril-qaq0/oracles/module_shape.py` — created in this slice.
**Named mutation:** write the config file inside the theme-commit path → byte-identity fence red; make `TuiState::theme()` ignore the preview → preview fence red; make the theme command send `QueryCommandOptions` → no-bridge fence red; build the option list in `cyril-core` from a hardcoded list → shape fence red.
**Complexity/production scale:** one `resolve` per selection change (O(1), 32-field copy); `TuiState::theme()` stays a field read. Production input is one selection event per keypress. Maximum accepted cost: one `resolve` per keypress, measured by the existing preview-latency assertion.
**Wall budget/phase:** one-off phase per keypress; `N/A — reason: bounded by the existing next-frame preview assertion`.
**Module shape:** adds picker kind + preview to `cyril-ui` (`traits.rs`, `state.rs`), one local command and one result kind to `cyril-core/commands`, and the Enter branch to `App`; protected parents `widgets/`, `cyril-memory/`, `protocol/` stay at zero production delta; shape fence reports no `ThemeId`/`ColorMode` in `cyril-core` production and no bridge command from the theme picker path.
**Files:** `crates/cyril-core/src/commands/mod.rs`, `crates/cyril-core/src/commands/builtin.rs`, `crates/cyril-ui/src/traits.rs`, `crates/cyril-ui/src/state.rs`, `crates/cyril/src/app.rs`, `.cyril-qaq0/oracles/module_shape.py`.
**Estimate:** 4 hours.
**Diff estimate:** 480 lines (+260 implementation, +220 tests).
**PR increment:** `qaq0-activation`.
**Commands and expected results:**
- `cargo test -p cyril --lib app::tests::theme_picker_session_leaves_config_bytes_identical` → bytes identical after both sessions.
- `cargo test -p cyril --lib app::tests::theme_command_opens_picker_without_bridge_traffic` → zero recorded bridge sends; the `/model` control still records one.
- `cargo test -p cyril-ui --lib state::tests::theme_returns_preview_while_theme_picker_is_open` → preview equals `resolve(highlighted id, current mode)`.
- `python3 .cyril-qaq0/oracles/module_shape.py` → `PASS`.
- named mutations → the named fence red; restore → green.

---

## Checkpoint records

Recorded by `checkpointed-build`. One checkpoint per completed slice.

### Slice 1 — Render fence covers every bundled theme × mode

| # | Gate | State | Evidence |
|---|---|---|---|
| 1 | Affected unit tests | PASS | `cargo test -p cyril-ui --lib render::` green; full `cargo test --workspace` green (every suite `ok`, no `FAILED`) |
| 2 | Falsifiers | PASS | C6 discharged: 96 combinations pass, 96 distinct labels, 24 no-color passes with zero non-Reset cells |
| 3 | Stress fixture | PASS | The 96-combination matrix itself; the pre-correction single-marker variant was red (`syntax expected Rgb(204,153,204), actual Rgb(180,142,173)`) |
| 4 | Implementation vs oracle | PASS | `migrated_scenes_match_all_pinned_cells` → 0/7,680 differences; per-role projection from `resolve(id, mode)` |
| 5 | Approved module shape | PASS | Test-only change: `git diff` touches only `render.rs`'s `#[cfg(test)]` module; no production line changes. The shape-fence script arrives with slice 4 |
| 6 | Production-scale budget | N/A — test-only slice; no production loop or phase |
| 7 | Regression fence | PASS | `render::tests::all_scene_theme_mode_combinations_pass` |
| 8 | Named mutation | PASS | `mutations.sh` M1 (no-color role leak) red: `CyrilDark/no-color/markdown foreground cell 122: syntax expected Reset, actual Rgb(255, 255, 255)` |
| 9 | Fence restored | PASS | green after restore |

Deviation recorded: the design's original C6 mutation (`resolve` ignoring its
`id`) was **unobservable** — the fence renders the marker and projected scenes
through the same code path, so a symmetric change cancels. `design.md`'s C6 row
now names the observable mutation. This is a technical proof correction, not a
decision change.

### Slice 2 — Appearance vocabulary and detection policy

| # | Gate | State | Evidence |
|---|---|---|---|
| 1 | Affected unit tests | PASS | `cargo test -p cyril-ui --lib theme::tests` → 42 passed; full workspace suite green |
| 2 | Falsifiers | PASS | C1, C2, C3 discharged: six ids round-trip, five spellings rejected per key, 14 precedence rows match |
| 3 | Stress fixture | PASS | Rejected spellings (`""`, `CyrilDark`, `cyril_dark`, trailing space, `solarized`; `""`, `auto`, `24bit`, `ANSI256`, `none `) all return `None`; one unknown key leaves the other key's explicit value intact |
| 4 | Implementation vs oracle | PASS | `python3 .cyril-qaq0/oracles/compare_appearance.py` → `PASS 30 rows agree with the independent oracle` |
| 5 | Approved module shape | PASS | Only `crates/cyril-ui/src/theme.rs` changed; protected parents at zero production delta |
| 6 | Production-scale budget | N/A — one-off startup resolution; constant comparisons, no allocation |
| 7 | Regression fence | PASS | `parse_theme_id_covers_exactly_the_bundled_ids`, `parse_color_mode_covers_exactly_the_five_values`, `detection_precedence_matches_every_table_row`, `startup_appearance_reports_one_diagnostic_per_unknown_key` |
| 8 | Named mutation | PASS | M2 (lenient parser) red `"" must be rejected`; M3 (NO_COLOR before explicit) red `explicit-beats-no-color resolved to the wrong mode` |
| 9 | Fence restored | PASS | green after restore (all three fences) |

Deviation recorded: `parse_color_mode` returns a `ColorModeRequest`
(`Automatic | Fixed(ColorMode)`) rather than a bare `ColorMode`, because
`automatic` is not itself a mode. `design.md` C2 records the refinement.

### Slice 3 — Config surface, startup resolution, invalid-value message

| # | Gate | State | Evidence |
|---|---|---|---|
| 1 | Affected unit tests | PASS | `cargo test --workspace` green; `cyril-core` config tests 26 passed; `cyril` bin `app::tests` 121 passed |
| 2 | Falsifiers | PASS | C7, C8, C10, C11 discharged |
| 3 | Stress fixture | PASS | `theme`/`color_mode` wrong-typed as `3`, `true`, `[]`, inline table; each rejects the whole file (`max_messages` stays 500) |
| 4 | Implementation vs oracle | PASS | `compare_appearance.py` PASS 30 rows (unchanged by this slice) |
| 5 | Approved module shape | PASS | `module_shape.py` PASS; `--selftest` PASS (planted `ThemeId` reported, clean file accepted); protected parents at zero delta |
| 6 | Production-scale budget | N/A — one resolution per process |
| 7 | Regression fence | PASS | `absent_appearance_keys_are_silent`, `configured_appearance_reaches_startup_state`, `unknown_theme_value_reports_one_visible_message`, `unknown_color_mode_value_reports_one_visible_message`, `startup_detection_honors_no_color`, `wrong_typed_{theme,color_mode}_falls_back_to_whole_file_defaults`, `default_ui_config_schema_is_exactly_four_fields` |
| 8 | Named mutation | PASS | M4 (dropped diagnostic) red; M5 (field-skipping deserializer) red `rejection must be whole-file, not field-skipping`; M6 (catalog leak into `cyril-core`) red; M7 (changed startup default) red |
| 9 | Fence restored | PASS | green after restore (all four fences) |

Deviation recorded: `App::new` reached 8 arguments and tripped
`clippy::too_many_arguments`. Rather than an `#[allow]` (forbidden), the two
process-level inputs are grouped as `StartupInputs { ui, environment }`, keeping
the signature at 7 and the two "read once, from outside the crate" inputs
adjacent. The exhaustive `UiConfig` destructure is unchanged, so the nd4h C6
consumption fence still holds.

Deviation recorded: C7's named mutation was unobservable — the baseline fence
resolves its theme through `truecolor_theme()`, never `UiState::new`. The
observable fence is `state::tests::new_state_uses_cyril_dark_truecolor`; the
baseline test independently keeps proving the pixels (0/7,680 differences).

## Tracker taxonomy

- Persisting a theme choice — **permanent non-goal** (ADR 0005 + the requester's
  session-only decision). No issue.
- User-defined palettes — **permanent non-goal** (ADR 0005). No issue.
- A live `/mode` command — **permanent non-goal for this feature** (spec decision
  5 settles the axis separation). No issue; a future request files its own.
- Structurally complete cache identities — **intended future work**, verified
  open: **cyril-x5xi**.
- Remaining dead `[ui]` knobs — **intended future work**, verified closed:
  **cyril-nd4h** (its seam is what this plan consumes).

## Self-review

1. Every design row is assigned to exactly one slice (C6 → 1; C1–C3 → 2; C7, C8, C11 → 3; C4, C5, C9, C10 → 4); every `PENDING` falsifier is discharged by the slice implementing its claim. ✅
2. Every slice records all fourteen fields; conditional cells carry `N/A — reason`. ✅
3. Every claim's fence is created in its own slice and carries the design row's named mutation. ✅
4. Every new loop states O(1) cost with its rationale; every one-off phase records `N/A — reason`. ✅
5. The module growth ledger covers every touched module and names the protected parents with zero-delta rules. ✅
6. Partition arithmetic recorded (sum 1225, margin 306, total 1531); single increment; every slice names it. ✅
7. Tracker taxonomy applied with verified IDs. ✅
8. No slice is declared complete; completion is `checkpointed-build`'s. ✅
