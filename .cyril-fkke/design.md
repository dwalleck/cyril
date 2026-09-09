# Design: Five bundled Cyril palettes

## Route and inputs

- Route: Structural (`.cyril-fkke/route.md`).
- Behavior set: `.cyril-fkke/spec.md` (Cyril Light, High Contrast Dark, High Contrast Light, Catppuccin Mocha, Gruvbox Dark added to the bundled resolver; complete 31-role population; compatible syntax component; deterministic ANSI-256/ANSI-16/no-color projection; no selection, configuration, or persistence change; Cyril Dark unchanged).
- Empirical premises: `N/A — no external premise; projections are pure functions of compile-time palette data and Syntect's bundled theme set, which is verified locally by C5 rather than assumed.`
- `evidence.md` / `probe.*`: `N/A — no Empirical route.` The independent oracle for this design is `.cyril-fkke/palette-oracle.py` (Python; independent implementation of WCAG contrast, nearest-index search, and protected-slot policy).
- Readability policy adopted from ADR 0007 (accepted repository obligation) and extended by the requester's `continue` on the stated recommendation: primary ≥4.5:1, muted ≥3.0:1, saturated ≥4.5:1 on the palette's default terminal background and ≥3.0:1 on its chrome; high-contrast variants raise these to 7.0 / 4.5 / 7.0 / 4.5.

## Input shapes

| Shape | Status |
|---|---|
| `ThemeId` variants: six bundled identifiers | Covered by C1, C2, C4 |
| `ColorMode`: TrueColor, Ansi256, Ansi16, None | Covered by C4 |
| `SourceColor`: Rgb, Reset | Covered by C1, C6 |
| `SyntaxTheme` variants: seven bundled names, six selected | Covered by C5 |
| Palette background roles: canvas/chrome/code/selection/inset_background | Covered by C2, C6 |
| Foreground tiers: primary, muted, saturated | Covered by C2 |
| Canvas = Reset (Cyril Dark) versus canvas = Rgb (five new) | Covered by C6 |
| ANSI-16 muted-family roles projecting into protected speaker slots | Covered by C3 |
| Cache keys across six palettes × four modes | Covered by C7 |
| Protected-parent production delta (app.rs, main.rs, state.rs) | Covered by C8 |
| Empty/None syntax (`ColorMode::None`) | Covered by C4 |
| Operator-supplied palette values (custom themes) | `N/A — permanent non-goal: ADR 0005 rejects arbitrary user palettes.` |
| Palette selection, environment detection, persistence | `N/A — intended future work: cyril-qaq0 (verified open, activation only).` |
| Cache-identity structural redesign for all roles | `N/A — intended future work: cyril-x5xi (verified open).` |
| Broader purpose-built role binding (widget surfaces) | `N/A — intended future work: cyril-nx1q (verified open).` |
| Syntax token modifier/background fidelity | `N/A — intended future work: cyril-d43s (verified open).` |

## Placement

| Capability | Owner | New seam | Forbidden |
|---|---|---|---|
| Bundled palette data (31 roles per identifier) and syntax choice | `crates/cyril-ui/src/theme.rs` | None — extends `ThemeId`, `SyntaxTheme`, and the existing private `source(id) -> SourceTheme` behind the unchanged public `resolve(ThemeId, ColorMode) -> Theme` | No widget, renderer, state, or binary code may match on `ThemeId`/`SyntaxTheme`; no palette knowledge outside `theme.rs` |
| Palette readability contract | `crates/cyril-ui/src/theme.rs` test module | None — extends the existing per-tier WCAG fence | Contract must not read source text of production files or import the Python oracle |
| Frame canvas painting for palettes that declare an explicit canvas | `crates/cyril-ui/src/render.rs` (`draw_inner`) | None — consumes `Theme::canvas` already present in the read-only theme | No widget may paint the canvas; no per-palette branch; `Color::Reset` must keep today's terminal-owned background |
| Syntax component lookup | `crates/cyril-ui/src/highlight.rs` | None — existing `SyntaxTheme::name()` lookup is generic | No new theme name list, no per-palette highlighting branch |

## Module shape

Current cluster (inventory):

| Module/path | Interface | Responsibility clusters | Dependency direction | Change |
|---|---|---|---|---|
| `crates/cyril-ui/src/theme.rs` | `ThemeId`, `SyntaxTheme`, `ColorMode`, `resolve`, `resolve_truecolor/ansi256/ansi16/no_color`, `Theme` | palette data, projection, ANSI-16 semantics, contract tests | depends only on `ratatui::style::Color` and `syntect` (tests) | `deepen` |
| `crates/cyril-ui/src/highlight.rs` | `highlight_block_with_theme`, `highlight_line_with_theme` | syntax lookup, highlight cache | consumes `Theme` | `retain` |
| `crates/cyril-ui/src/widgets/markdown.rs` | render + theme-keyed cache | markdown rendering | consumes `Theme` | `retain` |
| `crates/cyril-ui/src/render.rs` | `draw(frame, &dyn TuiState)` | frame layout, widget composition, fallback | consumes `TuiState` | `deepen` (canvas painting only) |
| `crates/cyril-ui/src/state.rs` | `UiState` | UI state machine, default theme | produces `Theme` for `TuiState` | `retain` |

Seam tests: deletion test — deleting `theme.rs` scatters palette/projection knowledge across renderers, so it is not pass-through. Interface test — all callers and tests reach palettes through `resolve`/`resolve_*`; tests use the same seam. Adapter test — `N/A — no new generic seam; the existing concrete resolver keeps one adapter (the bundled registry) by intrinsic ownership.` Locality test — palette data, projection, and the contrast contract share one owner and one verification file.

Three-way ownership alternatives for the new palette data:

1. **Extend `theme.rs` (selected).** Interface unchanged; `source(id)` dispatches to six compile-time tables; depth grows behind the same seam; tests stay in one file. Trade-off: one file grows by ~250 production lines.
2. **One module per palette (`theme/cyril_light.rs`, …).** Maximizes per-file locality but forces a new seam and six exports for data no caller needs, and splits the shared contract fence. Rejected: pass-through modules.
3. **Data-driven table file (`palettes.toml` + loader).** Extension-friendly but introduces runtime parsing, an error path, and a dependency for compile-time constants, contradicting ADR 0005's fixed bundled model. Rejected: new failure mode with no adapter.

Module ledger:

| Module/path | Interface | Owns | Hides/reuses | Must not own | Adapters | Tests through | Change |
|---|---|---|---|---|---|---|---|
| `crates/cyril-ui/src/theme.rs` | `resolve(ThemeId, ColorMode) -> Theme`; `ThemeId`/`SyntaxTheme`/`ColorMode` | six bundled palettes, projection, ANSI-16 semantics, contrast contract | private `SourceTheme`, nearest-index search | rendering, configuration, selection | `N/A — concrete bundled registry` | `resolve*` and the emitted probe | `deepen` |
| `crates/cyril-ui/src/render.rs` | `draw(frame, &dyn TuiState)` | frame layout, widget composition, canvas painting | widget modules | palette data, projection, theme choice | `N/A` | `draw` via `TestBackend` | `deepen` |
| `crates/cyril-ui/src/highlight.rs` | `highlight_*_with_theme` | syntax lookup + cache | Syntect sets | palette data | `N/A` | public highlight functions | `retain` |

Protected parents:

| Protected parent | Baseline responsibilities | Allowed change | Forbidden change | Exit condition |
|---|---|---|---|---|
| `crates/cyril/src/app.rs` | event loop, routing, wiring | none | any production delta | `git diff` vs upstream base is empty |
| `crates/cyril/src/main.rs` | startup, config load, App construction | none | any production delta | `git diff` vs upstream base is empty |
| `crates/cyril-ui/src/state.rs` | UI state, default theme | none | any production delta | `git diff` vs upstream base is empty |

Shape fence: `.cyril-fkke/oracles/module_shape.py` (production paths, dependency direction, protected-parent delta, widget theme-symbol ban).

## Claims

- **C1** Every bundled palette populates all 31 semantic roles.
- **C2** Every bundled palette meets its tier contrast targets on its declared backgrounds.
- **C3** Every bundled palette keeps ANSI-16 muted-family roles out of the protected speaker slots and resolves without panicking.
- **C4** Every bundled palette projects deterministically and correctly into all four color modes.
- **C5** Every selected `SyntaxTheme` exists in Syntect's bundled theme set.
- **C6** A palette with an explicit canvas paints that canvas as the frame background, and `Color::Reset` keeps the terminal-owned background byte-identical to today.
- **C7** Every bundled palette produces distinct highlight and markdown cache keys within each colored mode; the no-color mode intentionally shares one key because every palette resolves to the identical reset theme there. *(Amended 2026-09-09: the original claim required distinctness in every mode and was falsified by its own fence — no-color resolves identically, so a shared key is correct rather than stale.)*
- **C8** Palette work adds no production responsibility to protected parents or widgets.

## Falsification

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C1 | All 31 roles populated for six palettes | ThemeId × SourceColor | `emit_source_probe` emits 6×31 rows with no `unexpected:` value; any `unexpected:` or missing row falsifies | `.cyril-fkke/palette-oracle.py --emit` independent TSV compared cell-by-cell | delete `text_secondary` field from one palette literal | `theme::tests::emit_source_probe` + `.cyril-fkke/oracles/compare_probe.py` | seconds | PASS — 2026-09-09 `compare_probe.py` → `PASS 186 rows agree with the independent oracle` |
| C2 | Tier contrast met per palette | tiers × backgrounds | `bundled_palette_contrast_contract` recomputes WCAG in Rust; any role below tier falsifies | Python `palette-oracle.py` (independent luminance/contrast implementation) | set `CyrilLight.accent_tertiary = 0xb8b8c0` (1.97:1) | `theme::tests::bundled_palette_contrast_contract` | seconds | PASS — cheapest falsifier, run 2026-09-09: `python3 .cyril-fkke/palette-oracle.py` → `PASS 6 palettes…`; mutation run produced the expected two `FAIL` lines |
| C3 | Protected ANSI-16 slots stay free | muted-family roles × 16 colors | `ansi16_semantics_change_only_speaker_roles` + new `muted_family_never_projects_into_protected_slots` over `ThemeId::ALL`; a panic or assertion falsifies | Python oracle recomputes nearest index for each muted-family role | set `GruvboxDark.muted = 0x0000ff` (projects to LightBlue) | `theme::tests::muted_family_never_projects_into_protected_slots` | seconds | PASS — 2026-09-09 mutation run (`mutations.sh` M2) red on the protected-slot panic, green after restore |
| C4 | Deterministic, correct projection | ColorMode × six palettes | `resolution_is_deterministic_in_every_mode`, `all_roles_project`, `no_color_resets_every_role` over `ThemeId::ALL`, plus `.cyril-fkke/oracles/compare_probe.py` comparing all 186 probe rows; a mismatch falsifies | Python oracle nearest-index TSV vs Rust emitted TSV, with the accepted cyril-q9dx speaker-slot contract encoded independently in the oracle | change `nearest_ansi256` start index 16 → 0 | `theme::tests::all_roles_project` + `.cyril-fkke/oracles/compare_probe.py` | seconds | PASS — 2026-09-09 `compare_probe.py` → `PASS 186 rows agree with the independent oracle` |
| C5 | Syntax components exist | SyntaxTheme variants | `all_bundled_syntax_themes_exist` iterates `SyntaxTheme::ALL` against `ThemeSet::load_defaults()`; a missing key falsifies | `N/A — Syntect's bundled set is the external authority; the fence asserts membership directly, and the probe that listed the seven names is recorded in the run log` | rename `SyntaxTheme::InspiredGitHub` name to `"inspired-github"` | `theme::tests::all_bundled_syntax_themes_exist` | seconds | PASS — 2026-09-09 mutation run (`mutations.sh` M3) red on the missing-name assertion, green after restore |
| C6 | Explicit canvas painted; Reset unchanged | canvas Rgb × Reset | New render tests assert a light palette leaves zero cells on the terminal background and Cyril Dark paints no light canvas; the frozen 7,680-cell Cyril Dark baseline must stay identical | `TestBackend` buffer inspection (independent of the palette table) | remove the canvas paint entirely (`mutations.sh` M5). *(Corrected 2026-09-09: the original mutation — painting unconditionally — was verified unobservable, because `Reset` projects to `Reset`.)* | `render::tests::light_palette_paints_canvas_background`, `render::tests::reset_canvas_leaves_terminal_background_untouched`, `migrated_scenes_match_all_pinned_cells` | seconds | PASS — 2026-09-09 mutation run red on the positive control, green after restore |
| C7 | Cache keys discriminate palettes | palette × colored mode | New tests assert pairwise-distinct highlight and markdown cache keys per colored mode and exactly one shared no-color key | Direct key comparison; independent of rendering | make `highlight_cache_key` ignore the theme colors (`mutations.sh` M4) and drop the role-color loop from `markdown_cache_key` (`mutations.sh` M7) — Cyril Dark and Gruvbox Dark share a syntax component, so they collide | `highlight::tests::cache_key_distinguishes_bundled_palettes`, `widgets::markdown::tests::markdown_cache_key_distinguishes_bundled_palettes` | seconds | PASS — 2026-09-09 mutation run red (`collision in TrueColor: CyrilDark and GruvboxDark`), green after restore |
| C8 | No protected-parent or widget responsibility | paths | `.cyril-fkke/oracles/module_shape.py` compares the tree and diff against the upstream base and the approved ledger; a violation falsifies | `git diff`/source census, independent of test code | add `use crate::theme::ThemeId;` to `widgets/chat.rs` | `.cyril-fkke/oracles/module_shape.py` | seconds | PASS — 2026-09-09 `PASS module shape matches the approved ledger`; mutation run (`mutations.sh` M6) red on the widget leak, green after restore |

## Non-goals and future work

Permanent non-goals: operator-defined palettes (ADR 0005); terminal-capability remapping of palette colors (ADR 0005/0007 fixed-RGB decision).

Intended future work, verified in Rivets on 2026-09-09: cyril-qaq0 (activation), cyril-x5xi (cache identity), cyril-nx1q (purpose-built role binding), cyril-d43s (syntax modifier/background fidelity).

## Falsifier run log

- 2026-09-09 `python3 .cyril-fkke/palette-oracle.py` → `PASS 6 palettes satisfy contrast and ANSI-16 constraints`.
- 2026-09-09 mutation run (in-process, two mutations) → `FAIL CyrilLight/accent_tertiary #b8b8c0 on white: 1.97 < 4.5`, `FAIL GruvboxDark/muted #0000ff projects to protected slot 12`, exit 1 — the oracle is not vacuous.
- 2026-09-09 full mutation run `bash .cyril-fkke/oracles/mutations.sh` → `PASS all named mutations red, all fences green after restore`; M1 contrast, M2 protected slot, M3 syntax name, M4 highlight cache key, M5 canvas paint, M6 widget shape leak, M7 markdown cache key — each red with the localized message named in the claim row.
- 2026-09-09 oracle agreement `python3 .cyril-fkke/oracles/compare_probe.py` → `PASS 186 rows agree with the independent oracle`.
- 2026-09-09 module-shape fence `python3 .cyril-fkke/oracles/module_shape.py` → `PASS module shape matches the approved ledger`.
- 2026-09-09 throwaway probe `crates/cyril-ui/tests/zz_probe_syntax_themes.rs` (since deleted) → bundled Syntect themes are `InspiredGitHub`, `Solarized (dark)`, `Solarized (light)`, `base16-eighties.dark`, `base16-mocha.dark`, `base16-ocean.dark`, `base16-ocean.light`.

## Final design-conformance review

Independent reviewer (`scout`, read-only, no access to this design or the plan, no prior transcript) reconstructed the module shape from production code on `feat/cyril-fkke` on 2026-09-09. Its reconstruction agrees with the ledger on every verdict:

- `ThemeId` (6 variants), `SyntaxTheme` (5 variants), and `ColorMode` are confined to `theme.rs`; the only production use outside it is the single hardcoded `resolve(ThemeId::CyrilDark, ColorMode::TrueColor)` in `state.rs:333`.
- `render.rs` contains no theme-identifier branching; its one palette-dependent branch is the value test `theme.canvas != Color::Reset`.
- No widget reaches palette data; every widget takes `&Theme` and styles from role fields.
- `app.rs`/`main.rs` contain zero theme/palette/color symbols; `state.rs` gains no theme responsibility.
- No pass-through module and no new seam; `source(id)` is private, exhaustive over six variants, and is the single transcription point for palette data.

Reviewer observation, dispositioned: both cache keys hash 29 of the 31 roles. Verified **not** a gap — `grep -o 'theme\.[a-z_]*'` over `highlight.rs` and `widgets/markdown.rs` production shows neither reads `text_secondary` or `accent_violet`, so each key covers exactly the roles its renderer consumes. The fence pins that exact set, so a future palette that differs only in an unconsumed role correctly shares a key.

## Approval

Requester approval (verbatim): "continue"
Date: 2026-09-09
Risk acceptances approved: none.

Context for the approval: the requester answered the readability-policy question ("Approve this readability policy?") with `continue`, authorizing the stated recommendation (WCAG AA for ordinary palettes, AAA for high-contrast variants, branded colors adjusted only where tiers require). Syntax components are documented as approximations of the branded palettes because Syntect bundles no Catppuccin or Gruvbox component.
