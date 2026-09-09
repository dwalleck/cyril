# Plan: Five bundled Cyril palettes

Inputs: `.cyril-fkke/route.md` (Structural), `.cyril-fkke/spec.md`, `.cyril-fkke/design.md` (approved 2026-09-09).

## Module growth ledger

| Module | Baseline production lines | Projected final lines | Responsibility change | Interface change | Protected-parent rule |
|---|---:|---:|---|---|---|
| `crates/cyril-ui/src/theme.rs` | 421 | 660–720 | add five bundled palette tables + syntax variants + dispatcher | `ThemeId`/`SyntaxTheme` gain variants; `resolve*` signatures unchanged | `N/A` |
| `crates/cyril-ui/src/render.rs` | 155 | 168–180 | add frame canvas painting | `draw` signature unchanged | `N/A` |
| `crates/cyril-ui/src/highlight.rs` | 211 | 211 | none (tests only) | unchanged | `N/A` |
| `crates/cyril-ui/src/widgets/markdown.rs` | 1060 | 1060 | none (tests only) | unchanged | `N/A` |
| `crates/cyril/src/app.rs` | 2400 | 2400 | none | unchanged | zero production delta |
| `crates/cyril/src/main.rs` | 200 | 200 | none | unchanged | zero production delta |
| `crates/cyril-ui/src/state.rs` | 1200 | 1200 | none | unchanged | zero production delta |

## Partition arithmetic

- Slice 1 diff estimate: 900 lines (production 230, tests 300, issue-local oracle + shape fence 370).
- Slice 2 diff estimate: 120 lines (production 15, tests 105).
- Sum: 1,020. Churn margin: 300 (tests and palette tables typically grow 25–30% during review fixes).
- Total: 1,320 ≤ 4,000 → single PR increment.

PR increment **palette-bundle**: contains both slices; mergeable on its own because it only adds compile-time palette data, syntax variants, contract fences, and a canvas paint gated on a non-`Reset` canvas, leaving startup, configuration, and selection untouched. Verifies without any later increment (there are none).

## Slice 1: Bundled palette tables, syntax variants, and contract fences

**Claim IDs:** C1, C2, C3, C4, C5, C7, C8
**Expected behavior:** `resolve(ThemeId::<new>, ColorMode::<any>)` returns a complete `Theme` for all five new palettes; every role meets its tier on its declared backgrounds; ANSI-16 muted-family roles stay out of the protected speaker slots; the emitted probe agrees cell-by-cell with the Python oracle; each palette/mode pair yields distinct highlight and markdown cache keys.
**Oracle:** `.cyril-fkke/palette-oracle.py` (independent Python WCAG/nearest-index implementation) compared against `theme::tests::emit_source_probe` output by `.cyril-fkke/oracles/compare_probe.py`.
**Stress fixture:** `emit_source_probe` over all six palettes × four modes (744 role rows) plus the boundary case `CyrilDark.canvas == Reset` among otherwise-RGB canvases; expected: 6×31 rows, no `unexpected:` token, oracle agreement 1:1.
**Regression fence:** `crates/cyril-ui/src/theme.rs` tests `bundled_theme_registry_is_complete_and_unique`, `bundled_palette_contrast_contract`, `muted_family_never_projects_into_protected_slots`, `all_roles_project`, `all_bundled_syntax_themes_exist`, `no_color_resets_all_roles`; `crates/cyril-ui/src/highlight.rs` `cache_key_distinguishes_bundled_palettes`; `crates/cyril-ui/src/widgets/markdown.rs` `markdown_cache_key_distinguishes_bundled_palettes`.
**Named mutation:** set `CyrilLight.accent_tertiary` to `0xb8b8c0` (contrast fence red); set `GruvboxDark.muted` to `0x0000ff` (protected-slot fence red); rename `SyntaxTheme::InspiredGitHub`'s `name()` to `"inspired-github"` (syntax fence red); drop `theme.syntax` from `highlight_cache_key` (cache-key fence red).
**Complexity/production scale:** no new loop in production; palette lookup is one `match` per resolve call, O(1); tests iterate 6 palettes × 4 modes × 31 roles = 744 constant rows.
**Wall budget/phase:** `N/A — reason: one-off phase; palette resolution is per call, not a background phase, and adds no measurable wall cost.`
**Module shape:** responsibility added — five bundled palette tables plus syntax variants, owned by `crates/cyril-ui/src/theme.rs`; interface delta — `ThemeId`/`SyntaxTheme` variants added, `resolve*` signatures unchanged; owner after slice — `theme.rs`; protected parents touched — none, expected production delta 0; shape fence — `python3 .cyril-fkke/oracles/module_shape.py` → `PASS` with protected-parent deltas 0 and no widget theme-symbol references.
**Files:** `crates/cyril-ui/src/theme.rs`, `crates/cyril-ui/src/highlight.rs` (tests), `crates/cyril-ui/src/widgets/markdown.rs` (tests), `.cyril-fkke/palette-oracle.py`, `.cyril-fkke/oracles/compare_probe.py`, `.cyril-fkke/oracles/module_shape.py`
**Estimate:** 2–3 hours
**Diff estimate:** 900
**PR increment:** palette-bundle
**Commands and expected results:**
- `cargo test -p cyril-ui --lib theme::` → all theme tests pass, including the extended contract fences.
- `cargo test -p cyril-ui --lib -- --nocapture emit_source_probe` → 6×31 TSV rows with no `unexpected:` value; `python3 .cyril-fkke/oracles/compare_probe.py` → `PASS 744 rows agree`.
- `python3 .cyril-fkke/palette-oracle.py` → `PASS 6 palettes satisfy contrast and ANSI-16 constraints`.
- Apply each named mutation in turn → the corresponding fence fails with its localized message; restore → green.
- `python3 .cyril-fkke/oracles/module_shape.py` → `PASS`.

## Slice 2: Paint the declared canvas

**Claim IDs:** C6
**Expected behavior:** a frame rendered with a palette whose `canvas` is an explicit RGB paints that RGB as the background of every cell no widget claims; a frame rendered with `CyrilDark` (canvas `Reset`) is byte-identical to the frozen 7,680-cell baseline.
**Oracle:** `TestBackend` buffer inspection comparing each unclaimed cell's background against `theme.canvas` (independent of the palette table), plus the existing frozen baseline.
**Stress fixture:** render a light-palette message scene at 80×24; expected: every cell the widgets leave unset carries `Color::Rgb(<canvas>)`, and at least one cell is asserted (positive control) so the check cannot pass on an empty set.
**Regression fence:** `crates/cyril-ui/src/render.rs` `light_palette_paints_canvas_background`, plus the existing `migrated_scenes_match_all_pinned_cells`.
**Named mutation:** remove the `theme.canvas != Color::Reset` gate in `draw_inner` and paint unconditionally → the Cyril Dark baseline reports foreground/background differences (expected red), proving the Reset branch is load-bearing.
**Complexity/production scale:** one `Rect` fill per frame, O(width×height) cell writes already performed by the renderer's buffer; no new allocation.
**Wall budget/phase:** `N/A — reason: one-off phase; the fill runs once per frame and replaces no existing work; it is bounded by the existing frame cost.`
**Module shape:** responsibility added — frame background painting, owned by `crates/cyril-ui/src/render.rs`; interface delta — none (`draw` unchanged, consumes the existing `Theme::canvas`); owner after slice — `render.rs`; protected parents touched — none, expected production delta 0; shape fence — `python3 .cyril-fkke/oracles/module_shape.py` → `PASS`.
**Files:** `crates/cyril-ui/src/render.rs`
**Estimate:** 30–45 minutes
**Diff estimate:** 120
**PR increment:** palette-bundle
**Commands and expected results:**
- `cargo test -p cyril-ui --lib render::` → the new canvas fence passes and the frozen Cyril Dark baseline still matches 7,680/7,680 cells.
- Named mutation applied → baseline test reports the expected differences; restored → green.
- `python3 .cyril-fkke/oracles/module_shape.py` → `PASS`.

## Checkpoint records

Recorded by `checkpointed-build` on 2026-09-09, one checkpoint per slice.

### Slice 1 — Bundled palette tables, syntax variants, and contract fences

| # | Gate | State | Evidence |
|---|---|---|---|
| 1 | Affected unit tests | PASS | `cargo test -p cyril-ui --lib theme::` → 37 passed; full `cargo test --workspace` green |
| 2 | Falsifiers | PASS | C1/C2/C3/C4/C5/C7 discharged: `compare_probe.py` → `PASS 186 rows agree`; mutation run M1–M4 red then restored green |
| 3 | Stress fixture | PASS | `emit_source_probe` over 6 palettes × 4 modes → 744 rows, no `unexpected:` token |
| 4 | Implementation vs oracle | PASS | `compare_probe.py` compares all 186 role rows against the independent Python oracle; initial run correctly flagged the three ANSI-16 speaker roles as an oracle gap, fixed by encoding the accepted cyril-q9dx contract in the oracle |
| 5 | Approved module shape | PASS | `python3 .cyril-fkke/oracles/module_shape.py` → `PASS`; protected parents unchanged |
| 6 | Production-scale budget | N/A — no new loop or always-on phase; palette lookup is a constant-time match |
| 7 | Regression fence | PASS | theme/highlight/markdown fences green in the full workspace run |
| 8 | Named mutation | PASS | M1 contrast, M2 protected slot, M3 syntax name, M4 cache key — all red with the expected localized message |
| 9 | Fence restored | PASS | `mutations.sh` green-after-restore for C2, C3, C5, C7 |

Deviation recorded: claim C7 was falsified by its own fence on first run (no-color resolves identically for every palette, so one shared key is correct). `design.md` C7 was amended accordingly; the fence now pins per-colored-mode distinctness plus exactly one shared no-color key.

### Slice 2 — Paint the declared canvas

| # | Gate | State | Evidence |
|---|---|---|---|
| 1 | Affected unit tests | PASS | `cargo test -p cyril-ui --lib render::tests` → 12 passed; full workspace green |
| 2 | Falsifiers | PASS | C6 discharged: light palette leaves zero terminal-background cells; Cyril Dark paints no light canvas; frozen 7,680-cell baseline unchanged |
| 3 | Stress fixture | PASS | 80×24 populated frame: 1760 canvas cells + 160 chrome cells per palette; positive control fires when painting is removed |
| 4 | Implementation vs oracle | PASS | `TestBackend` buffer inspection compared against `theme.canvas`; frame dump confirmed per palette |
| 5 | Approved module shape | PASS | `module_shape.py` → `PASS`; `render.rs` gains one gated `set_style`, no `ThemeId` branch |
| 6 | Production-scale budget | N/A — one bounded fill per frame; no new allocation |
| 7 | Regression fence | PASS | `light_palette_paints_canvas_background`, `reset_canvas_leaves_terminal_background_untouched`, `migrated_scenes_match_all_pinned_cells` |
| 8 | Named mutation | PASS | M5 (remove the paint) red on the positive control |
| 9 | Fence restored | PASS | green after restore |

Deviation recorded: the marker-theme floor snapshots (`roomy_idle/inflow/approval_80x24`) changed because the marker theme declares an explicit canvas `Indexed(1)`, which is now painted. Verified line-by-line: 88 changed lines, 0 differences outside the `bg` field. The Cyril Dark frozen baseline is unaffected.

Named mutation corrected: the design's original C6 mutation (paint unconditionally) was empirically unobservable — `Reset` projects to `Reset` — so `design.md` now names the observable mutation (remove the paint).

## Self-review

1. Every design row C1–C8 is assigned to exactly one slice; every `PENDING` falsifier is discharged by its implementing slice.
2. Every slice records all fourteen fields.
3. Every claim's fence is created in the slice implementing it and carries its named mutation.
4. No new production loop; the canvas fill is bounded by the existing frame cost, with rationale recorded.
5. The module growth ledger covers every touched module and protected parent.
6. Partition arithmetic recorded; single increment with a mergeable definition.
7. No deferral phrase lacks a classification (design non-goals/future work carry verified IDs).
8. No slice is declared complete; completion belongs to `checkpointed-build`.
