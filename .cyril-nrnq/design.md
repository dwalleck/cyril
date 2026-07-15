# cyril-nrnq — Design: migrate modal surfaces to semantic colors

## Purpose

Move the four modal overlays (approval both phases, picker, hooks panel, code
panel) off hardcoded `Color::` literals onto the semantic theme contract,
following the ghuu method exactly: frozen legacy inventory → canonical ANSI
RGB mapping → representability → zero-normalized-diff equivalence. The probe
showed the batch requires a **contract expansion**: two legacy values
(`Color::Gray` → `#c0c0c0`, hooks matcher `Rgb(176,141,255)` → `#b08dff`)
have no role — the same failure mode that grew ghuu's draft from 26 to 29
roles.

## Architecture

1. **Expand (theme.rs):** add 2 roles — proposed names `text_secondary`
   (`#c0c0c0`; modal body/list text) and `accent_violet` (`#b08dff`; hooks
   matcher accent) — to `SourceTheme`, `Theme`, `resolve_with`, and the
   `roles()` test array (29 → 31). Projections are mechanical via the
   existing `SourceColor` rules.
2. **Migrate (4 widget files):** each modal render fn gains `theme: &Theme`
   (the caller `render.rs::draw_inner` already holds `state.theme()`);
   every literal swaps per the canonical mapping:

   | Legacy | Role |
   |---|---|
   | `Rgb(50,50,70)` | `selection` |
   | `Color::White` | `text` |
   | `Color::Cyan` | `accent_quinary` |
   | `Color::DarkGray` | `subdued` |
   | `Color::Yellow` | `emphasis` |
   | `Color::Green` | `subdued_positive` |
   | `Color::Red` | `subdued_negative` |
   | `Color::Gray` | `text_secondary` (new) |
   | `Rgb(176,141,255)` | `accent_violet` (new) |

   VGA-exact roles (not intent roles like `warning`/`danger`) keep the
   equivalence contract byte-clean and keep nrnq orthogonal to cyril-leiq
   (verified open, P1): leiq owns role VALUES, nrnq owns role ASSIGNMENT —
   a leiq re-valuation later flows into modals with zero modal edits.
3. **Fence (tests):** frozen pre-migration baseline TSV + marker-theme
   wiring tests + no-legacy-source scan, replicating ghuu's fence family.

## Input shapes (each covered by ≥1 claim)

| Shape | Values | Claim |
|---|---|---|
| Widget × phase | approval SelectOption / SelectTrust, picker, hooks, code | C3, C4 (5 scenes) |
| Approval options | selected / unselected; `is_destructive` both (probe: no style effect today — preserved as-is) | C3 |
| Picker rows | selected / unselected / description / group / `is_current` / overflow scrollbar (style untouched: default-styled widget, no literal) | C3 |
| Hooks | matcher Some / None; multi-trigger headers; empty list; `scroll_offset > 0` (style-identical, geometry only) | C3, C10 |
| Code panel | `LspStatus` ×4 incl. `Unknown`; warning/message/root/config Some+None; `lsps` empty/multi; duration Some/None | C3, C10 |
| `ColorMode` | TrueColor / Ansi256 / Ansi16 / NoColor | C2, C3 (truecolor), C7 (no-color); ansi modes via C2 projection rules |
| New-role values | `#c0c0c0`, `#b08dff` in all 4 projections | C1, C2 |
| Theme flow | production single-resolve; marker theme | C4, C6 |

Out of scope shapes: Unicode/width in labels (unchanged by color migration);
`ThemeId` beyond CyrilDark (single variant today; bundled palettes are
cyril-fkke, verified open, and will supply values for all 31 roles at that
point); modal geometry (owned by cyril-a14l / cyril-uw20, verified open).

## Subtractive sweep (step 2b)

The migration removes one constraint: "modal colors are compile-time
constants, independent of `state.theme()`." The facts it guaranteed:
(1) *render output ignores the theme* — the `theme_seam_picker` snapshot and
any marker-theme frame test will now see modal cells change with the theme;
that is the FEATURE, fenced deliberately by C4/C6, and the snapshot
re-baseline is sanctioned (expected: picker cells under the marker theme
shift to marker role colors, symbols unchanged). (2) *no caller needs a
`Theme`* — signature change is compile-enforced; the single caller
(`render.rs:61-72`) already holds the theme. No lock, ordering, or
uniqueness property is involved; no other code reads modal render output.

## Claims

1. **C1 — representability.** The expanded Cyril Dark source contains every
   canonical RGB value in the modal legacy inventory (9 values, 2 new).
2. **C2 — projection.** All 31 roles (incl. both new) project by the
   existing nearest-color rules in ansi256/ansi16 and reset under no-color.
3. **C3 — equivalence.** Migrated true-color renders of the 5 scenes differ
   from the frozen pre-migration baseline by zero normalized cells
   (normalization: named → canonical RGB per the ghuu table + Gray/violet
   extensions; absent-vs-Reset collapse as ghuu recorded).
4. **C4 — role wiring.** Under the pairwise-distinct marker theme, each
   migrated element renders its MAPPED role's marker color — distinguishing
   roles that share Cyril Dark values (`muted` = `border` = `#8c8c8c`;
   equivalence alone cannot catch cross-wiring).
5. **C5 — no legacy sources.** The four modal widget files contain zero
   `Color::` literals after migration (allowlist: empty).
6. **C6 — single theme flow.** A full frame with all overlays active renders
   modal cells from `UiState`'s one resolved theme (no internal `resolve`).
7. **C7 — no-color reset.** Under the NoColor projection the 5 scenes carry
   zero non-Reset colors, with symbols identical to the truecolor scenes.
8. **C8 — non-color distinguishability (AC2).** In NoColor renders, the
   selected approval/picker row is identifiable by the ▸ prefix (and BOLD in
   approval) alone.
9. **C9 — interaction/geometry untouched.** Existing state tests and the
   cc5e viewport fences pass unmodified; the diff contains no state.rs or
   geometry-arithmetic changes.
10. **C10 — edge shapes.** Empty hooks list, empty `lsps`, `LspStatus::
    Unknown`, and all-None optional fields render under the theme param
    without panic, styled per mapping.
11. **C11 — scene completeness.** The 5 baseline scenes jointly exercise all
    30 frozen legacy tuples — otherwise C3 is vacuous for unreached tuples.

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| 1 | C1 | representability script; negative control drops the 2 new roles | ghuu NAMED canon + probe-styles.txt, parsed from source text (no rustc) | **2m** | **PASSED** (missing=0; control fires exactly {#b08dff, #c0c0c0}) | `theme::tests::modal_legacy_colors_are_representable` |
| 2 | C2 | project both new roles in all modes; compare to hand-computed nearest-ansi values | hand-derived: `#c0c0c0`→ansi16 `Gray`, `#b08dff`→computed at build vs `nearest_ansi16` table transcribed | 10m | pending | existing `theme::tests` projection suite extended to `roles()==31` |
| 3 | C3 | render 5 scenes post-migration, diff against committed TSV | **frozen pre-migration baseline TSV** (generated from CURRENT code in plan slice 1, before any migration commit) | 30m | pending | `modal_theme::baseline_equivalence_{scene}` (5 named tests) |
| 4 | C4 | render scenes under `marker_theme()`; assert per-element marker colors | hand-pinned element→role→marker-color table written from the mapping, not from render output | 30m | pending | `modal_theme::marker_wiring_{widget}` (4 tests) |
| 5 | C5 | scan the 4 files for `Color::` | raw grep over source text (same mechanism as ghuu claim 4) | 5m | pending | `theme::tests::modal_widgets_have_no_legacy_color_sources` |
| 6 | C6 | full-frame render with overlays + marker theme | marker-role table (as C4) applied to frame cells; a widget calling `resolve` internally shows Cyril Dark values instead | 10m | pending | `render::tests::modal_frame_uses_state_theme` |
| 7 | C7 | render 5 scenes under NoColor | assert every cell fg/bg == Reset; symbols diffed against truecolor scene symbols | 10m | pending | `modal_theme::no_color_scenes_reset` |
| 8 | C8 | NoColor render; strip styles entirely | ▸ located by symbol scan; selected row text equality | 5m | pending | `modal_theme::no_color_selection_distinguishable` |
| 9 | C9 | run existing suites unmodified; inspect diff scope | pre-existing tests (authored before this design) + `git diff --stat` | 2m | pending | existing `state::tests` + `picker_viewport` suites |
| 10 | C10 | render edge-shape fixtures | no-panic + tuple subset check vs mapping table | 10m | pending | `modal_theme::edge_shapes_render` |
| 11 | C11 | count distinct styled tuples in the baseline TSV | frozen probe-styles.txt count (30) | 2m | pending | assertion inside the baseline generator + `modal_theme::baseline_covers_inventory` |

Non-vacuity (buggy implementation each catches): C1 — the 29-role contract
(control run PROVES it fires); C2 — projecting `#b08dff` with the ansi256
index table off-by-one; C3 — mapping Cyan→`accent` (`#00ffff`) instead of
`accent_quinary` (normalized diff ≠ 0 at every border cell); C4 — swapping
`subdued`↔`muted` (invisible to C3 only when values coincide — here they
don't: `#808080` vs `#8c8c8c`; but border/muted DO coincide, which is why
C4 exists); C5 — leaving one literal in `code_panel.rs`; C6 — a widget
calling `theme::resolve_truecolor()` internally; C7 — forgetting the
no-color reset for a new role; C8 — dropping the ▸ prefix while migrating
the selected-row style; C9 — "improving" a modal's geometry mid-migration;
C10 — unwrapping an optional field while threading the theme param; C11 —
a baseline scene set that skips the trust phase (tuple count < 30).

Distinctness: every fence is a separately named test (per-scene, per-widget
where applicable); the two falsifier scripts print labeled counts.

## Negative space (deliberately NOT in this change)

1. **No role VALUES change** — the dim-VGA readability problem is
   cyril-leiq (open, P1); nrnq's canonical mapping neither fixes nor
   worsens it, and leiq's future re-valuation reaches modals for free.
2. **No intent re-bucketing** — approval's Yellow maps to `emphasis`
   (VGA-exact), not `warning`; re-bucketing roles by intent would break the
   equivalence contract and belongs to the theme-activation era
   (cyril-qaq0, verified open).
3. **No geometry or interaction changes** — modal layout/keys are
   cyril-a14l / cyril-uw20 territory; `modal::centered` adoption by
   approval/hooks/code panels stays out.
4. **No cache changes** — modals are uncached render paths; theme-identity
   cache keys are cyril-x5xi (verified open).
5. **No new theme, no theme switching** — CyrilDark remains the only
   `ThemeId`; activation/switching is cyril-qaq0; bundled palettes
   (which must then define the 2 new roles' values) are cyril-fkke
   (verified open).
6. **No scrollbar/Clear styling** — elements with no color literal today
   stay default-styled.

## Open decisions (for design approval)

1. **New role names** — proposal: `text_secondary` (`#c0c0c0`) and
   `accent_violet` (`#b08dff`). Alternatives: ghuu's ordinal scheme
   (`accent_senary`) or visual names (`silver`, `lavender`).
2. **Named-color mapping posture** — recommended: VGA-exact roles
   (`emphasis`/`subdued_positive`/`subdued_negative`/`accent_quinary`),
   byte-preserving and leiq-orthogonal, matching ghuu precedent.
   Alternative: intent roles (`warning`/`success`/`danger`/`accent`) —
   semantically cleaner but visibly brightens every modal NOW and breaks
   the zero-diff equivalence method.
3. **Mode coverage depth** — recommended: equivalence fences at TrueColor +
   NoColor with projection rules covering ansi modes via C2 (ghuu ran all
   16 scene-mode combos; modals get 5 scenes × 2 modes + projection unit
   coverage — cheaper, same failure surface).
