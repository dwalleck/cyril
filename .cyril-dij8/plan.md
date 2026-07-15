# cyril-dij8 — Budgeted plan: migrate application chrome to semantic colors

Design: `.cyril-dij8/design.md` (approved 2026-07-14). Claims C1–C10.
Method inherited from ghuu/nrnq. Fences that need `MockTuiState`/`UiState`
live in a new in-crate `#[cfg(test)] mod chrome_theme_tests` (they are
cfg(test)-gated types — probe finding); source-scan fences extend the
existing family in `theme.rs`. Normalization helpers are duplicated from
`render.rs`'s ghuu fences rather than refactored out — consolidation is
cyril-xv3e (verified open, P3).

Branch discipline (this run): a parallel sweep session owns main; every
commit is gated on `[ "$(git branch --show-current)" = "feat/cyril-dij8-chrome-semantic-colors" ]`;
`.rivets/issues.jsonl` is never committed on this branch; discovered
issues queue in `.cyril-dij8/to-file.md` and are filed on main at
close-out.

Scenario set (18 TSV scenes = the 13 probe scenarios + 5 edge scenes):
toolbar_sending_full(120×1), toolbar_streaming_nosession(80×1),
toolbar_toolrunning_nosession(80×1), toolbar_idle(80×1),
status_ok_tokens_credits(120×1), status_warn_breakdown_scroll(200×1),
status_crit_refused(80×1), status_cancelled(80×1), status_turnlimit(80×1),
status_empty_fallback(80×1), status_boundary_70(80×1),
status_boundary_90(80×1), crew_overflow(80×10), crew_small_pending(80×6),
crew_no_group(80×5), crew_multi_group(80×6), voice_listening(60×1),
voice_transcribing(60×1). (voice_idle renders nothing — asserted in the
edge test, not a TSV scene.)

## Slice 1: C1 fence — chrome legacy colors are representable

**Claim:** C1 (representability, 12 canonical values, 0 new roles).
**Oracle:** `.cyril-dij8/representability-falsifier.py` (already PASSED;
source-text parse, no rustc) — the Rust fence must agree with it.
**Stress fixture:** the fence's `required` array lists all 12 canonical
values AND asserts `required` has 12 pairwise-distinct entries — a
duplicated entry (11 distinct) is the plausible transcription bug that
would silently weaken the fence.
**Loop budget:** O(12 × 31) membership scan, test-only. Trivial.
**Wall budget:** n/a (test).
**Files:** `crates/cyril-ui/src/theme.rs` (test module only).

Code (advisory): mirror `modal_legacy_colors_are_representable` with the
12-value table from the design; add the distinctness assert.

**Verification:**
- [ ] Unit tests pass (`cargo test -p cyril-ui theme::`)
- [ ] Stress: mutating one required value to `#123456` fails the fence (one-shot check, then revert)
- [ ] Falsifier script still exits 0
- [ ] Budgets trivially hold

## Slice 2: chrome scenario builders + baseline generator + frozen TSV

**Claim:** C2 substrate (frozen pre-migration baseline; generated from
CURRENT unmigrated code, committed before any migration commit).
**Oracle:** `.cyril-dij8/probe-styles.txt` (frozen probe output) — the
generator's tuple inventory must reproduce all 23 styled tuples.
**Stress fixture:** generator runs twice; byte-identical output both times
(catches nondeterminism from HashMap iteration in `SubagentTracker` —
plausible: crew rows are sort-stabilized, but a scene builder that forgot
the deterministic `SubagentListUpdated` path would flap).
**Loop budget:** O(scenes × cells) ≈ 18 × ≤800 = ~4.4k cell visits per
run, test-only. Trivial.
**Wall budget:** n/a (test).
**Files:** `crates/cyril-ui/src/chrome_theme_tests.rs` (new),
`crates/cyril-ui/src/lib.rs` (+2 lines), fixture
`crates/cyril-ui/tests/fixtures/chrome-theme-baseline.tsv` (generated
output, pinned-commit header like the ghuu/conversation baseline).

Code (advisory): scene builders return `(name, Buffer)`; normalization
copied from render.rs tests (`normalized_color`, `symbol_hex`, TSV row
shape `scene\tx\ty\tsymbol_hex\tfg\tbg\tmod_bits`); `emit_chrome_baseline`
test prints BEGIN/END-fenced TSV captured via `--nocapture`.

**Verification:**
- [ ] Unit tests pass; generator emits 18 scenes
- [ ] Stress: two runs byte-identical
- [ ] Distinct fg≠Reset tuple count across TSV == 23 (matches probe-styles.txt)
- [ ] Budgets hold

## Slice 3: equivalence + completeness + edge fences (green pre-migration)

**Claim:** C2 (equivalence test, trivially green until migration starts —
then it guards every migration slice), C8 (completeness == 23), C10 (edge
shapes: toolbar Idle has no spinner glyph and no styled fg cells;
pct=70 → normalized `RGB:008000`; pct=90 → normalized `RGB:808000`; crew
header variants render; `Working(None)` shows "Working"; voice Idle
`height_for == 0` and an untouched buffer).
**Oracle:** the frozen TSV from slice 2 (committed artifact, generated
from code that no longer exists once migration lands).
**Stress fixture:** C10's boundary fixtures pin the STRICT `>` semantics
(70 → OK-green, 90 → warn-yellow). The plausible bug: migration rewrites
the threshold chain and flips `>` to `>=`. Expected outputs written here,
pre-implementation: pct=70.0 → `RGB:008000`, pct=90.0 → `RGB:808000`.
**Loop budget:** O(TSV rows) ≈ 3.7k parse + compare, test-only. Trivial.
**Wall budget:** n/a (test).
**Files:** `crates/cyril-ui/src/chrome_theme_tests.rs`.

**Verification:**
- [ ] Equivalence: actual normalized cells == fixture cells (all 18 scenes; per-scene assert messages)
- [ ] Completeness: 23 distinct styled tuples in fixture
- [ ] Edge tests pass with pre-written expected values
- [ ] Budgets hold

## Slice 4: migrate toolbar.rs (toolbar + status bar) + caller + snapshot re-baselines

**Claim:** C2 for toolbar/status (equivalence stays green through the
swap); dead `(White, EndTurn)` arm maps inertly to `theme.text`.
**Oracle:** slice-3 equivalence fence against the frozen TSV (normalized:
`Color::Yellow` and `theme.emphasis` both → `RGB:808000`).
**Stress fixture:** the 6 status scenes jointly cover all four stop-reason
labels + all three gauge bands + fallback — a single-role mis-mapping
(e.g. gauge Green→`success` `#00ff00` instead of `subdued_positive`
`#008000`) fails equivalence at named cells in `status_ok_tokens_credits`
and `status_boundary_70` specifically.
**Loop budget:** no new loops (constant swaps in existing span builders).
**Wall budget:** n/a (render path unchanged asymptotically).
**Files:** `crates/cyril-ui/src/widgets/toolbar.rs`,
`crates/cyril-ui/src/render.rs` (caller passes `&theme`), plus sanctioned
insta re-baselines of `theme_seam_idle` / `theme_seam_tool_diff` /
`theme_seam_picker` (all three include toolbar+status cells; subtractive
sweep, design step 2b).

Code (advisory): `render`/`render_status_bar`/`status_bar_spans` gain
`theme: &Theme`; mapping per design table; keep `Modifier` usage
untouched; keep `palette::SPINNER_*` (non-color).

**Verification:**
- [ ] Full `cargo test -p cyril-ui` green (equivalence + existing toolbar text tests unmodified)
- [ ] Snapshot re-baselines contain ONLY toolbar/status cell changes (inspect diff)
- [ ] Stress: equivalence green including both boundary scenes
- [ ] clippy pedantic + fmt + doctests green

## Slice 5: migrate crew_panel.rs + caller

**Claim:** C2 for crew (equivalence green; border stays default-styled —
negative space #3).
**Oracle:** slice-3 equivalence fence (frozen TSV).
**Stress fixture:** `crew_overflow` + `crew_small_pending` +
`crew_no_group` + `crew_multi_group` jointly reach every crew tuple
(C3/C7-adjacent bug: styling the border with `theme.border` — Reset today
— fails equivalence on every border cell of all four crew scenes).
**Loop budget:** no new loops.
**Wall budget:** n/a.
**Files:** `crates/cyril-ui/src/widgets/crew_panel.rs`,
`crates/cyril-ui/src/render.rs`.

**Verification:**
- [ ] Full test suite green (incl. crew text/height tests unmodified)
- [ ] Stress: equivalence green on all four crew scenes
- [ ] clippy pedantic + fmt green

## Slice 6: migrate voice.rs + caller

**Claim:** C2 for voice; twin assignment = `soft_accent`/`muted`/
`accent_alt` (approved decision #1).
**Oracle:** slice-3 equivalence fence (frozen TSV).
**Stress fixture:** the value-level bug (mapping the hint to `subdued`
`#808080` instead of `muted` `#8c8c8c`) fails equivalence on the hint
cells; the twin-level bug (`user` instead of `soft_accent`) is INVISIBLE
here by design and is exactly what slice 7's marker fence exists for —
recorded so the residual risk is explicit.
**Loop budget:** no new loops.
**Wall budget:** n/a.
**Files:** `crates/cyril-ui/src/widgets/voice.rs`,
`crates/cyril-ui/src/render.rs`.

**Verification:**
- [ ] Full test suite green
- [ ] Stress: equivalence green on both voice scenes
- [ ] clippy pedantic + fmt green

## Slice 7: marker wiring fences (C3) + single-theme-flow frame fence (C5)

**Claim:** C3 (per-element MAPPED role under `marker_theme()` — separates
the three twin pairs), C5 (full frame renders chrome from state's one
resolved theme).
**Oracle:** hand-pinned element→role→marker-index table transcribed from
the design mapping (NOT from render output): e.g. voice listening →
`soft_accent` → `Indexed(27)` (a `user` cross-wire renders `Indexed(10)`);
crew status text → `subdued` → `Indexed(24)`; toolbar bg → `chrome` →
`Indexed(2)`; model → `accent_quaternary` → `Indexed(22)`.
**Stress fixture:** the three twin pairs asserted on BOTH sides
(soft_accent≠user, muted≠border, accent_alt≠system as marker indices) —
the bug class equivalence provably cannot catch (values coincide under
Cyril Dark). Frame fence: MockTuiState (marker theme default) with crew
active; assert toolbar bg cell == `Indexed(2)` and crew icon ==
`Indexed(25)`; also assert `render.rs` production contains exactly one
`state.theme()` call (source count, mirroring the conversation fence).
**Loop budget:** O(cells) per scene, test-only. Trivial.
**Wall budget:** n/a.
**Files:** `crates/cyril-ui/src/chrome_theme_tests.rs` (marker fences),
`crates/cyril-ui/src/render.rs` (frame fence in existing test module).

**Verification:**
- [ ] `marker_wiring_{toolbar,status,crew,voice}` pass with hand-pinned indices
- [ ] `chrome_frame_uses_state_theme` passes
- [ ] Twin-pair asserts present for all three pairs
- [ ] Budgets hold

## Slice 8: no-color fences (C6 reset + C7 label distinguishability)

**Claim:** C6 (NoColor renders carry zero non-Reset colors; symbols
identical to truecolor), C7 (status meaning carried by text/symbols alone:
"Token limit"/"Turn limit"/"Refused"/"Cancelled", "Context: N%", ●/◆/○,
"+N more", "⇄ N steers", 🎙/⏳ words).
**Oracle:** for C6, `resolve_no_color` contract already pinned by
theme.rs tests (independent of widgets); symbol equality diffed against
the truecolor scene buffers. For C7, plain substring scan on style-less
text (zero style info used).
**Stress fixture:** C7 scans the CANCELLED scene for the word — the
plausible bug is dropping the label during migration and signaling by
color alone (AC2 violation invisible to every other fence under NoColor).
**Loop budget:** O(scenes × cells) test-only. Trivial.
**Wall budget:** n/a.
**Files:** `crates/cyril-ui/src/chrome_theme_tests.rs`.

**Verification:**
- [ ] `no_color_scenarios_reset`: every cell fg/bg == Reset across all 18 scenes rendered with the NoColor theme
- [ ] Symbols identical truecolor↔nocolor per scene
- [ ] `no_color_status_distinguishable`: all label/symbol probes found
- [ ] Budgets hold

## Slice 9: source-scan fences (C4) + close-out audit (C9)

**Claim:** C4 (zero `Color::` literals and zero palette COLOR-constant
references in the three chrome files' production sections; spinner consts
allowlisted), C9 (existing suites pass unmodified; diff scope contains no
geometry/state changes beyond the sanctioned snapshot re-baselines).
**Oracle:** raw source-text scan (same mechanism as the ghuu/nrnq fences,
different from rustc); one-shot control: run the same scan predicate
against `git show d4f105f:...toolbar.rs` and confirm it FIRES on the
pre-migration file (proves non-vacuity). C9: `git diff d4f105f --stat`
inspected + full workspace gate.
**Stress fixture:** the scan covers `production_source()` only (pre-
`#[cfg(test)]`) and matches both `Color::` and the palette color idents
(`USER_BLUE|AGENT_GREEN|SYSTEM_MAUVE|MUTED_GRAY|CODE_BLOCK_BG`) — the
plausible bug is a migration that leaves `palette::MUTED_GRAY` in
voice.rs, which the `Color::` scan alone would miss.
**Loop budget:** O(file bytes), test-only, ≤300KB. Trivial.
**Wall budget:** n/a.
**Files:** `crates/cyril-ui/src/theme.rs` (fence),
`.cyril-dij8/build-audit.md` (audit artifact).

**Verification:**
- [ ] `chrome_widgets_have_no_legacy_color_sources` green; control fires on d4f105f version
- [ ] Full gate: `cargo nextest run` (or `cargo test`) + `cargo clippy --all-targets -- -D warnings` + `cargo fmt --check` + doctests — real exit codes, no pipes
- [ ] `git diff d4f105f --stat` scope audit recorded in build-audit.md
- [ ] All 10 design claims mapped to landed fences in build-audit.md

## Plan Self-Review

1. **Loops:** no new production loops anywhere (constant swaps inside
   existing span builders); all test loops O(scenes × cells) ≤ ~15k cell
   visits per test — far under 10^6. No gaps.
2. **Fixtures:** S1 dup-guard (transcription bug), S2 determinism
   (nondeterministic scene builder), S3 strict-`>` boundaries (threshold
   flip), S4 per-band gauge mis-map, S5 border-styling regression, S6
   twin-invisible residual explicitly routed to S7, S7 twin cross-wiring
   (equivalence-blind class), S8 label-drop under NoColor, S9 palette-ref
   leak the Color:: scan misses. All adversarial, none happy-path-only.
   No gaps.
3. **Doc-comment preconditions:** none added; render fns gain a param
   with no new preconditions; `height_for` contracts untouched. No gaps.
4. **Write targets:** baseline generator prints TSV to stdout inside
   BEGIN/END fences (data, extracted via `--nocapture` — same convention
   as the probe and ghuu generator); all other output is test-harness
   diagnostics. Fixture TSV + build-audit.md are committed files. No gaps.
5. **Tracker references:** cyril-xv3e (fixture-plumbing consolidation —
   verified open, P3), cyril-6r3a (palette contraction — verified open),
   cyril-leiq (role values — verified open P1), cyril-qaq0/fkke
   (activation/palettes — verified open). No uncited deferrals. No gaps.

Claim coverage: C1→S1, C2→S3 (enforced through S4–S6), C3→S7, C4→S9,
C5→S7, C6→S8, C7→S8, C8→S3, C9→S9, C10→S3. All 10 design claims covered.
