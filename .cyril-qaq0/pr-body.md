Activate the bundled palettes and terminal color modes. Closes `cyril-qaq0`.

Two surfaces, one vocabulary: `[ui] theme` / `[ui] color_mode` at startup, and a session-local `/theme` picker. Neither ever writes configuration.

## Startup

`UiConfig` gains `theme` and `color_mode` as `Option<String>`:

- **absent** → silent default (`cyril-dark`; color mode detected from `NO_COLOR` / `COLORTERM` / `TERM` / platform)
- **present but unrecognized** → that key falls back to its own default and prints one visible system message naming the key, the value, and the default
- **wrong type** → the whole file is rejected with defaults, matching every other key (no field-skipping)

Strings rather than enums keep the palette catalog in `cyril-ui` and keep a typo from failing the entire config file. `main.rs` is the one place the process reads the terminal environment; `cyril-ui` still has zero `std::env` reads, so detection is a pure function over an injected `ColorEnvironment`.

## `/theme`

`CommandResultKind::ShowThemePicker` plus a local `theme` builtin return only the intent — `cyril-core` never learns a palette id or label. `PickerState` gains `kind: PickerKind`, so the existing viewport/filter/scrollbar machinery serves both an agent picker (confirm sends a bridge command) and Cyril's palette picker (confirm commits locally, no session required).

- opens on the committed palette, marked `✓`
- moving the selection previews immediately — `TuiState::theme` returns the preview, so every renderer follows with no second code path
- **Enter** commits for the session and preserves the active color mode
- **Esc** drops the preview and leaves the committed appearance exactly as it was

## Verification

Gate: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` — all green.

11 falsifiable claims, each with a fence and a named mutation; all 11 PASS. Independent oracle `compare_appearance.py` → **PASS 30 rows**. Module-shape fence `module_shape.py` → **PASS** + `--selftest` (reports a planted `ThemeId` in `cyril-core`, accepts a clean file); it also asserts the two `/theme` regions of `app.rs` construct no `BridgeCommand`.

Mutation proofs, all red then restored green (`bash .cyril-qaq0/oracles/mutations.sh`):

| # | Mutation | Fence that caught it |
|---|---|---|
| M1 | `resolve_no_color` leaks the `text` role | render matrix |
| M2 | lenient theme-id parser | `parse_theme_id_covers_exactly_the_bundled_ids` |
| M3 | `NO_COLOR` checked before the explicit value | `detection_precedence_matches_every_table_row` |
| M4 | startup drops the unknown-value diagnostic | `unknown_theme_value_reports_one_visible_message` |
| M5 | field-skipping deserializer | `wrong_typed_theme_falls_back_to_whole_file_defaults` |
| M6 | bundled id literal leaks into `cyril-core` | `module_shape.py` |
| M7 | startup default palette changes | `new_state_uses_cyril_dark_truecolor` |
| M8 | `/theme` reaches the bridge | `theme_command_opens_picker_without_bridge_traffic` |
| M9 | Esc keeps the preview | `picker_cancel_discards_the_preview` |
| M10 | `theme()` ignores the preview | `theme_preview_drives_the_rendered_theme_until_commit` |

Real-surface check: the picker rendered through `cyril_ui::render::draw` shows the title, all six labels, and `✓` on the committed palette; moving the selection changed the rendered border foreground `Rgb(86,199,208)` → `Rgb(142,192,124)`, proving the preview reaches pixels rather than only state.

No-write proof: `.cyril-qaq0/config-nowrite-fixture.toml` (comments, alignment, inline comment) is byte-identical after a full picker session.

Two design corrections are recorded in `.cyril-qaq0/design.md`: `parse_color_mode` returns a `ColorModeRequest` (`automatic` is not a `ColorMode`), and two originally named mutations were unobservable and were replaced with the fences that actually move.

Docs: the `[ui]` key table in `.agents/summary/codebase_info.md` documents both new keys.
