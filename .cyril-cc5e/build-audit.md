# cyril-cc5e — checkpointed-build audit (2026-07-14)

## Slices (one commit each)

| Slice | Commit | Claims | Gates |
|---|---|---|---|
| 1 modal::centered helper | `feat(cyril-ui): shared centered-modal geometry helper` | C8, C10-arith | parity sweep 21k cases; degenerate/clamp/odd-remainder; probe unchanged |
| 2 selection-centered viewport | `feat(cyril-ui): selection-centered picker viewport` | C1, C2, C6, C7 | 5 fences; probe↔oracle-v2 byte agreement; snapshot re-baseline (verified border-shift-only) |
| 3 scrollbar + desc reserve | `feat(cyril-ui): overflow scrollbar + stable description reserve fences` | C3, C4 | height constant over 20 selections; mixed-desc jitter fixture; exact-fit boundary; `crates/cyril` diff = 0 (display-only) |
| 4 floor walk + degenerate | (this commit) | C5, C10 | 15-step walk matches oracle literals incl. k=14 (the probe's original invisible case); 20×8/5×5/1×1 no-panic |

## Final integration check

- `probe-output-v2.txt` ↔ `oracle-v2-output.txt` (first 4 scenarios): **byte-identical**
- `window-model-check.py`: 57,400 cases, 0 violations (re-run)
- Full workspace: tests OK, doctests OK, clippy `--all-targets -D warnings` OK, fmt OK
- C9: existing `state::tests` suite untouched and passing; `PickerState` unchanged

## Deviations from plan (all within advisory-code latitude)

1. `oracle-v2.py` created in slice 2 (plan listed S4) — S2 fences needed its literals.
2. `theme_seam_picker` insta snapshot re-baselined in slice 2: the approved
   height formula removes the legacy dead slack row (popup 1 row shorter for
   fitting lists). Diff inspected: border shift only, no content/color deltas.
3. Scrollbar fence glyphs corrected to ratatui 0.30 defaults (▲/▼ not ↑/↓) —
   fence bug, implementation was correct.
4. Slice-1 parity sweep is 21k cases (plan estimated 1.8k) — still test-only, trivial.

## Fence inventory (permanent CI form)

`crates/cyril-ui/tests/picker_viewport.rs`: selection_always_visible,
window_contiguous_fill, small_list_layout_unchanged, empty_filter_no_panic,
duplicate_labels_single_marker, description_contained_height_stable,
scrollbar_iff_overflow, floor_60x16_full_walk, degenerate_sizes_no_panic.
`crates/cyril-ui/src/widgets/modal.rs`: centered_parity_sweep,
degenerate_areas_yield_empty_rects, clamps_desired_size_to_area_margin,
odd_remainder_lands_on_trailing_margin.
