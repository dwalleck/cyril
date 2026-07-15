# cyril-nrnq — Budgeted plan

Design: `.cyril-nrnq/design.md` (approved; representability falsifier passed,
negative control fires). Claim coverage: C11→S1 · C1,C2→S2 · (threading)→S3 ·
C3,C4→S4-S7 (one widget each) · C10→S6/S7 · C7,C8,C11-fence→S8 · C5,C6→S9 ·
C9→every slice's verification + S9 diff-scope check.

Ordering constraint: **S1 (baseline freeze) MUST land before any code
change** — the TSV is the pre-migration ground truth every equivalence fence
reads. Production render code introduces **zero new loops** in this feature
(literal→field swaps); all loop budgets below are test-only.

---

## Slice 1: freeze the pre-migration modal baseline

**Claim:** C11 (scene completeness) + the ground-truth input for C3.
**Oracle:** the frozen `probe-styles.txt` from prove-it (committed at
e4e6746): the distinct styled-tuple count in the TSV must equal its 30.
**Stress fixture:** the count assertion itself — a scene set that skips the
trust phase or the code panel's Failed status produces < 30 and fails the
generator.
**Loop budget:** 5 scenes × 80×24 cells ≈ 10⁴ writes, one-shot test-only.
**Wall budget:** n/a.
**Files:** `crates/cyril-ui/tests/gen_baseline_nrnq.rs` (temporary — archived
to `.cyril-nrnq/` after the run, like the probe), `.cyril-nrnq/modal-baseline.tsv`
(committed artifact).

**Code (advisory):** reuse the probe's five fixtures verbatim; TSV rows
`scene\tx\ty\tsymbol\tfg\tbg\tmods` for every cell with non-default style or
non-space symbol; assert distinct-tuple count == 30 before writing.

**Verification:**
- [ ] Generator runs; TSV committed; tuple count == 30
- [ ] Existing suites untouched and green
- [ ] Workspace gates (test, clippy `-D warnings`, fmt)

## Slice 2: expand the contract — `text_secondary` + `accent_violet` (29→31)

**Claim:** C1 (representability), C2 (projection).
**Oracle:** `.cyril-nrnq/representability-falsifier.py` re-run against the
REAL theme.rs (no `PROPOSED` injection — flag it off by editing required
constant or run `--without-new-roles` expecting missing=0 now); ansi
projections checked against a brute-force nearest-color computation in
Python over the transcribed `ANSI16_RGB`/ansi256 tables (independent of the
Rust `nearest_*` fns).
**Stress fixture:** `#c0c0c0` equals `ANSI16_RGB[7]` exactly — distance 0
must map to index 7 (catches off-by-one in table indexing); marker-theme
pairwise-distinctness assertion over all 31 roles (a duplicated marker value
would silently blind the C4 fences).
**Loop budget:** projection tests O(31 × 16) ≈ 500 ops; distinctness O(31²)
≈ 10³. Test-only.
**Wall budget:** n/a (role lookup is a field read).
**Files:** `crates/cyril-ui/src/theme.rs` (SourceTheme, Theme, resolve_with,
cyril_dark_source, roles() → 31, `modal_legacy_colors_are_representable`
test), `crates/cyril-ui/src/traits.rs` (marker_theme gains 2 pairwise-
distinct values + distinctness test if absent).

**Verification:**
- [ ] Unit tests incl. new representability + projection + distinctness
- [ ] Python falsifier reports missing=0 against real source, no injection
- [ ] Baseline fences N/A (no render change); existing suites green
- [ ] Gates green

## Slice 3: thread `&Theme` through the four modal render fns (mechanical)

**Claim:** none directly — the compile-enforced substrate for C3-C8.
**Oracle:** byte-identical render: each widget rendered before/after this
slice with identical fixtures produces identical buffers (theme accepted but
unused). Compare via the slice-1 TSV: re-run the generator logic against the
threaded code; diff against committed TSV must be empty WITHOUT any
normalization (colors haven't moved yet).
**Stress fixture:** the TSV diff itself — any accidental style edit during
the mechanical change shows as a non-empty diff.
**Loop budget:** none (signature change).
**Wall budget:** n/a.
**Files:** 6, deviating from the 2-file rule — declared: this is one
atomic compile unit (`approval.rs`, `picker.rs`, `hooks_panel.rs`,
`code_panel.rs` each gain `_theme: &Theme`; `render.rs` passes `&theme` at
the four call sites; `tests/picker_viewport.rs` updates its render helper).
~20 changed lines, all mechanical, no behavior.

**Verification:**
- [ ] Workspace compiles; all existing suites green unmodified
- [ ] TSV re-render diff empty (no normalization)
- [ ] Gates green

## Slices 4-7: migrate one widget each — approval (S4), picker (S5), hooks (S6), code panel (S7)

Common shape; per-slice specifics below.

**Claim:** C3 (equivalence, that widget's scenes) + C4 (marker wiring);
S6/S7 also C10 (edge shapes).
**Oracle:** committed `modal-baseline.tsv` with the normalization table
(named→canonical RGB: ghuu NAMED + Gray→`#c0c0c0`, violet literal→`#b08dff`)
transcribed into the test — the baseline was generated before ANY migration
commit, fully independent of the code under test. Marker tables are
hand-pinned from the design mapping, not read from render output.
**Stress fixture (per widget):**
- S4 approval: the trust-phase scene — its `DarkGray`-on-`selection`-bg
  tuple is the only place two roles compose; mapping `subdued` to `muted`
  (`#8c8c8c` vs `#808080`) fails the normalized diff by 12 units.
- S5 picker: the ITALIC description tuple (modifier must survive the color
  swap) + the `theme_seam_picker` snapshot re-baseline (sanctioned by the
  design's subtractive sweep: marker colors now flow into picker cells;
  symbols must be unchanged — assert in the slice before accepting).
- S6 hooks: the matcher purple is `accent_violet`'s FIRST consumer — a
  transposed Rgb literal (`Rgb(141,176,255)`) fails marker wiring; plus the
  empty-hooks-list scene (C10).
- S7 code panel: all four `LspStatus` variants incl. `Unknown` + all-None
  optionals (C10); mapping ✓/✗ to swapped roles fails the marker table.
**Loop budget:** per-slice fence renders ≈ 2-3 scenes × 10⁴ cells, test-only.
**Wall budget:** n/a.
**Files (per slice):** the widget file + `crates/cyril-ui/tests/modal_theme.rs`
(fences accrete per slice; created in S4). S5 additionally re-baselines the
insta snapshot (declared third file, artifact not code).

**Verification (each):**
- [ ] `baseline_equivalence_{scene}` zero normalized diff
- [ ] `marker_wiring_{widget}` matches the hand-pinned table
- [ ] All prior slices' fences + existing suites still green (C9)
- [ ] Gates green

## Slice 8: no-color projection fences (C7, C8) + inventory fence (C11)

**Claim:** C7 (NoColor scenes fully reset, symbols identical to truecolor),
C8 (selection identifiable without color), C11 (permanent fence form).
**Oracle:** symbol equality is diffed against the truecolor render of the
SAME fixtures (mechanically independent of color projection); ▸ located by
symbol scan; C11 recounts the committed TSV.
**Stress fixture:** a no-color render where one new role forgot its Reset
projection shows a colored cell — the fence prints the offending
scene/x/y/value (localized). C8: strip ALL styles, then locate the selected
row purely by ▸ + label text.
**Loop budget:** 5 scenes × 2 modes × 10⁴ cells ≈ 10⁵, test-only.
**Wall budget:** n/a.
**Files:** `crates/cyril-ui/tests/modal_theme.rs`.

**Verification:**
- [ ] `no_color_scenes_reset`, `no_color_selection_distinguishable`,
      `baseline_covers_inventory` pass
- [ ] Existing suites green; gates green

## Slice 9: source fence (C5) + single-theme-flow frame fence (C6) + close-out

**Claim:** C5 (zero `Color::` literals in the four widget files), C6 (one
resolved theme feeds the whole overlay frame), C9 (final diff-scope check).
**Oracle:** C5 reads raw source text (grep mechanism, same as ghuu claim 4)
— non-vacuous by construction (it fails on the pre-migration files); C6
uses the marker theme via `MockTuiState` with all four overlays active; a
widget internally resolving CyrilDark shows non-marker values. C9: `git
diff origin/main...HEAD --stat` contains no `state.rs` and no geometry hunks
(reviewed by hand, recorded in build-audit).
**Stress fixture:** C5 run against the S3-era file (pre-swap) must fail —
run once as a negative control against `git show`n old content; C6 with a
deliberately-injected `resolve_truecolor()` in a scratch build must fail
(TDD-inversion check, not committed).
**Loop budget:** source scan O(4 files × lines ≈ 10³); frame render 10⁴.
**Wall budget:** n/a.
**Files:** `crates/cyril-ui/src/theme.rs` (source-fence test),
`crates/cyril-ui/src/render.rs` (frame fence test).

**Verification:**
- [ ] Both fences pass; negative controls fired during development
- [ ] Full workspace gate: tests, clippy `--all-targets -D warnings`, fmt, doctests
- [ ] Final: ALL fences + falsifier scripts re-run green; build-audit.md written

---

## Plan Self-Review

1. **Loops:** all test-only (largest 10⁵ cell comparisons); production adds
   zero loops (field reads replace constants). All stated. **No gaps.**
2. **Fixtures:** S1 under-coverage count; S2 distance-0 index + marker
   distinctness; S3 unnormalized TSV diff; S4 two-role composition tuple;
   S5 modifier survival + sanctioned snapshot; S6 first consumer of the new
   role + empty list; S7 status-variant matrix + all-None; S8 forgotten
   Reset + color-free selection; S9 negative controls for both fences. All
   adversarial with pre-written expectations. **No gaps.**
3. **Doc-comment preconditions:** none introduced — render fns accept any
   `&Theme` (total functions); the only contract ("baseline TSV is
   pre-migration ground truth") is enforced by slice ORDER, recorded here
   and in the TSV's committed provenance line. **No gaps.**
4. **Write targets:** TSV artifact (data, committed file); fence failure
   messages to test stderr/stdout (diagnostics, test-only). No production
   writes. **No gaps.**
5. **Tracker references:** leiq, qaq0, a14l, uw20, dij8, 6r3a, x5xi, fkke —
   all verified open in this session's scans (see related-issues.md);
   ixua/ghuu verified closed. No new deferrals. **No gaps.**
