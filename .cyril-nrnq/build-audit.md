# cyril-nrnq — checkpointed-build audit (2026-07-15)

## Slices (one commit each)

| Slice | Claims | Gate highlights |
|---|---|---|
| 1 baseline freeze | C11 | 9,600-row TSV, exactly 30 tuples; generator determinism sha-verified |
| 2 contract 29→31 | C1, C2 | representability falsifier missing=0 vs real source; ansi16 pins from independent Python brute-force (both new roles → Gray; #c0c0c0 is distance-0 at index 7); marker distinctness fence |
| 3 theme threading | substrate | TSV re-render byte-identical, ZERO normalization; ghuu's seam fence caught a fully-qualified `crate::theme` path in code_panel — fixed pre-commit |
| 4 approval | C3, C4 | zero normalized drift both phases; marker sets pinned (option: 20/30/5 on bg 4; trust: 23/24/30/5) |
| 5 picker | C3, C4 | zero drift; no-BOLD selection asymmetry + ITALIC survival pinned; theme_seam_picker re-baselined (symbols verified identical, style-only delta) |
| 6 hooks | C3, C4, C10 | zero drift; accent_violet's first consumer; probe's "matcher purple" corrected to TRIGGER column; empty-list scene themed |
| 7 code panel | C3, C4, C10 | zero drift; five-role marker set incl. all four LspStatus; Unknown+all-None edge scene |
| 8 no-color | C7, C8 | all scenes fully Reset, symbols identical across modes; selection identifiable color-free (AC2) |
| 9 source + frame fences | C5, C6, C9 | modal literal scan (negative control: slice-3-era approval.rs had 15 `Color::` lines); marker frame fence scoped to overlay footprint (chrome is dij8's); diff scope: zero state.rs/cyril-core/cyril lines |

## Final integration check

- Workspace tests, doctests, clippy `--all-targets -D warnings`, fmt: all green
- `representability-falsifier.py` vs real source: required=9 missing=0
- `modal_theme` suite: 14 fences + 1 ignored generator; `picker_viewport`: 11 (cc5e fences intact — C9)
- cc5e's `window-model-check.py` still passes (57,400/0)
- Zero TODO/FIXME/anonymous deferrals in touched files

## Fixes-of-my-own-fences during build (implementation was right, fence wrong)

1. Slice 5: ITALIC assertion searched a multi-char string in per-cell rows.
2. Slice 9: C6 initially banned Rgb over the WHOLE frame — chrome widgets
   (toolbar/status) are cyril-dij8's batch and legitimately still paint
   hardcoded colors; scoped to overlay-footprint cells (diff vs no-overlay
   frame). Also: the reused `picker_state()` fixture pins CyrilDark
   truecolor explicitly — C6 needed the marker default.

## Notable

- The probe mislabeled `Rgb(176,141,255)` as the matcher column; it is the
  TRIGGER column (matcher-present is Cyan). The canonical mapping was
  unaffected; fences encode the corrected reading.
- `accent_violet` ansi16-projects to Gray (Euclidean nearest for a
  desaturated light purple) — verified with an independent brute-force
  oracle before pinning; surprising but correct under the signed rules.
