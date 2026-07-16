# cyril-a14l budgeted plan — height-aware layout at the 60×16 floor

Design: `.cyril-a14l/design.md` (approved 2026-07-15, D1–D5 resolved).
Gates per slice: `cargo test`, `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --check`. All new fences live in a new `#[cfg(test)]` module
`crates/cyril-ui/src/floor_tests.rs` (same pattern as `chrome_theme_tests.rs`)
unless colocated with the widget.

**Output streams:** every write target in this plan is a test assertion or a
committed fixture file — no new production output. The one production
diagnostic added (S7 budget-degradation `debug!`) goes through `tracing`
(stderr-file sink), classified diagnostic.

---

## Slice 0: Capture the roomy-frame baseline before anything changes

**Claim:** C6 (roomy sizes byte-identical to main).
**Oracle:** the frame rendered by *main's code* (this branch pre-impl),
serialized to `crates/cyril-ui/tests/fixtures/roomy-frame-baseline.tsv`
(symbol+style per cell, commit-stamped like the conversation baseline).
**Stress fixture:** three 80×24 states — idle; 3-line draft + 10 suggestions
(selected mid); 1-line input + approval — the states most likely to drift.
**Loop budget:** fixture gen/compare O(cells)=80×24×3 ≈ 5.8k per run. Trivial.
**Files:** `crates/cyril-ui/src/floor_tests.rs` (new), fixture TSV (new).
**Verification:** `roomy_frame_matches_main_fixture` passes on UNCHANGED
rendering code (proves the fixture is honest), gates pass.

## Slice 1: `modal::place()` — input-protected placement with legacy parity

**Claim:** C9 (parity wherever legacy rect doesn't overlap the input);
foundation for C7.
**Oracle:** verbatim transcription of today's `centered()` + approval inline
arithmetic as `legacy_*` fns in the test (cc5e C8 pattern).
**Stress fixture:** grid sweep area {5..200}×{3..100} × desired {0..80} ×
input_top {0,1,2,mid,height} — includes empty region (input_top ≤ 1) and
region smaller than borders (height 1–2). Expected: rect ⊂ rows [1,input_top),
= legacy rect whenever legacy doesn't overlap; empty rect when region empty.
**Loop budget:** test-only sweep ≈ 13×10×8×5 = 5.2k iterations of O(1) math.
**Files:** `crates/cyril-ui/src/widgets/modal.rs`.
**Verification:** parity sweep + region-containment property tests pass;
`centered()` untouched (other callers unaffected this slice); gates pass.

## Slice 2: Approval routes through `place()` and windows its selection

**Claim:** C7 (approval never covers input), C8 (selection visible when
clamped).
**Oracle:** buffer scan for `▸`+label within popup rect; expected window
start computed by independent inline arithmetic (selected, items, rows).
**Stress fixture:** 3 options in a 5-row region with selected=last (main
clips it — bug-class embed); trust phase with 3×3-row items in a 7-row
region selected=last; 1 option in a 3-row region; empty region (input_top=1)
→ nothing rendered, no panic.
**Loop budget:** window arithmetic O(options), options ≤ ~6. Trivial.
**Files:** `crates/cyril-ui/src/widgets/approval.rs`.
**Verification:** unit fences `approval_selection_visible_when_clamped`
(+trust variant) pass; render call site passes `input_top`; gates pass.
**Doc-contract:** `place()` documents "returns empty rect when region can't
hold borders" — load-bearing: callers must skip rendering on empty rect;
enforced by runtime `if rect.area()==0 return` in each caller (silently
rendering into an empty rect is ratatui-safe but Clear on a bogus rect is
not meaningful output).

## Slice 3: Picker and hooks panel route through `place()`

**Claim:** C7 (picker, hooks never cover input).
**Oracle:** same frame-diff mechanism as slice 4's fence (this slice adds
widget-level tests; the full-frame fence lands in slice 4).
**Stress fixture:** picker with filter row + 4 options in a 6-row region
selected=last (viewport from cc5e must still keep ▸ visible at the new
size); hooks panel with 12 hooks in a 5-row region scrolled to bottom.
**Loop budget:** O(visible options) per render, ≤ region height. Trivial.
**Files:** `crates/cyril-ui/src/widgets/picker.rs`,
`crates/cyril-ui/src/widgets/hooks_panel.rs`.
**Verification:** picker keeps cc5e visible-selection tests green at
constrained sizes; hooks scroll clamp test at 5 rows; gates pass.

## Slice 4: Code panel routes through `place()` + full-frame no-cover fence

**Claim:** C7 complete (all four overlays).
**Oracle:** frame-diff — render each overlay state vs its no-overlay twin at
{60×16, 80×24} × input {1-line, max-draft}; every differing cell must sit in
rows `[1, input_top)`. Input rect located by `┌`/`└` parse (independent of
layout code).
**Stress fixture:** max-draft at 60×16 (region = 3 rows — the D3 corner);
approval trust phase (tallest demand) over max-draft.
**Loop budget:** diff O(cells) = 4 overlays × 4 frames × 1920 ≈ 31k. Test-only.
**Files:** `crates/cyril-ui/src/widgets/code_panel.rs`,
`crates/cyril-ui/src/floor_tests.rs` (fence `modals_never_cover_input`).
**Verification:** fence fails against pre-slice-2 geometry (checked by
temporarily pointing it at `legacy` math in review), passes now; gates pass.

## Slice 5: Input builds char-wrapped visual rows (pure function)

**Claim:** C2 mechanics (exact cursor row math), part of C3.
**Oracle:** `.cyril-a14l/oracle-input-wrap.py` — independent Python
unicode-width char-wrap; generates
`crates/cyril-ui/tests/fixtures/input-wrap-oracle.tsv` (committed) with
expected (rows, cursor_row, cursor_col) for the fixture matrix; Rust tests
assert equality against the TSV (CI never runs Python).
**Stress fixture:** matrix = {empty, "ascii", 300-char single line, 10×
"draft-N" lines, `世界` lines where a 2-col char straddles the wrap
boundary, line of combining/zero-width, cursor at 0/mid-word/end/usize::MAX}
× widths {1, 2, 10, 58}. Expected outputs generated by the Python oracle
BEFORE the Rust impl is written.
**Loop budget:** O(draft chars) per call; stress ceiling 100 KiB (existing
input stress) ≈ 10^5 char ops per frame — parity with today's per-frame
`split('\n')`+Paragraph-wrap full scan, typical drafts <1 KiB. Justified:
no regression vs current cost; not a new always-on phase.
**Files:** `crates/cyril-ui/src/widgets/input.rs`,
`.cyril-a14l/oracle-input-wrap.py` (+ generated fixture).
**Verification:** `wrap_rows_match_python_oracle` passes; gates pass.

## Slice 6: Input render uses cursor-follow window

**Claim:** C2 (cursor always visible), C3 (window = the char-wrap window
containing the cursor, in order).
**Oracle:** the slice-5 TSV (window expectations added by the same Python
oracle); plus the F-A finding (scroll is visual-row-space) already encoded
by building rows ourselves.
**Stress fixture:** 10-line draft cursor-at-end in 7 content rows (main
shows NO cursor — bug-class embed); cursor at start after scrolling (window
snaps back); single long wrapped line with cursor mid-wrap; window at exact
last row boundary.
**Loop budget:** O(visible rows) slice of prebuilt rows. Trivial.
**Files:** `crates/cyril-ui/src/widgets/input.rs`,
`crates/cyril-ui/src/floor_tests.rs` (fences `input_cursor_always_visible`,
`input_scroll_window_matches_oracle`).
**Verification:** both fences fail on pre-slice code (cursor count 0 in
input rect for S1), pass now; existing input pinned-baseline test still
passes (no wrap in its scene); gates pass.

## Slice 7: Explicit vertical budget in `draw_inner`

**Claim:** C1 (floors hold at every size ≥60×16 for every state shape).
**Oracle:** hand-computed budget table in the test (per state: expected
input_h/chat_min) derived from the R1 arithmetic written in the design —
compared against `┌`/`└`-parsed geometry, not against the code's variables.
**Stress fixture:** adversarial matrix at {60×16, 60×17, 61×20, 80×24} ×
{max draft, draft+10 suggestions, crew=3 rows + draft, voice+crew+draft}.
Bug-class: budget that forgets crew/voice over-allocates input → chat <3.
**Loop budget:** no new production loop — arithmetic only. Matrix test
≈ 16 renders × O(cells). Trivial. Degradation path logs via
`tracing::debug!` (diagnostic, stderr-file sink) when floors force input
below its demand — satisfies "log before degrading" (CLAUDE.md silent-
failure rule).
**Files:** `crates/cyril-ui/src/render.rs`,
`crates/cyril-ui/src/floor_tests.rs` (fence
`layout_floors_hold_across_adversarial_matrix`).
**Verification:** fence passes; slice-0 roomy fixture STILL byte-identical
(Min(3)≡Min(5) with surplus — the empirical check of that equivalence);
gates pass.

## Slice 8: Suggestions window respects its actual area

**Claim:** C4 (selected `▸` row always inside the rendered area).
**Oracle:** expected window start/size from independent arithmetic over
(total, selected, effective_visible) transcribed in the test — distinct
from the widget's code path (computed from first principles in the test).
**Stress fixture:** (total=10, selected=7, area=4) — main renders no `▸`
(bug-class embed); (total=11, area=1) selected first/last; (total=3,
area=10); selected=None; area=0.
**Loop budget:** O(visible) per render, visible ≤ area height. Trivial.
**Files:** `crates/cyril-ui/src/widgets/suggestions.rs`.
**Verification:** fence `suggestion_selection_always_visible` fails pre-
slice, passes now; existing pinned suggestion baseline still passes (its
scene area = 10 = MAX_VISIBLE, unaffected); gates pass.

## Slice 9: Suggestions overlay above the input under pressure

**Claim:** C5 (input box doesn't move when suggestions open under
pressure), completes C6.
**Oracle:** frame-diff open-vs-closed at 60×16: input `┌` row equal, status
row equal, suggestion text present above input_top. Roomy side: slice-0
fixture equality (in-flow path untouched).
**Stress fixture:** 60×16 + 10 suggestions + 1-line input (main moves input
row 10→6 — bug-class embed); 60×16 + max draft + suggestions (overlay
region 3 rows, windowed selection per C4); 80×24 + max draft + 10
suggestions (predicate flips to overlay ABOVE the floor — the D4-accepted
behavior change, pinned here on purpose).
**Loop budget:** predicate O(1); overlay render O(visible rows). Trivial.
**Files:** `crates/cyril-ui/src/render.rs`,
`crates/cyril-ui/src/floor_tests.rs` (fence
`suggestions_overlay_under_pressure`).
**Verification:** fence fails pre-slice, passes now; slice-0 roomy fixture
still passes; gates pass.

## Slice 10: Browse-mode fence, permanent no-fallback sweep, probe removal

**Claim:** C10 (browse usable at floor), C11 (no fallback down to 1×1).
**Oracle:** C10 — expected top visible message computed from the message
list + row arithmetic independent of chat.rs; C11 — fallback banner text.
**Stress fixture:** 30 messages, `chat_scroll_back = Some(10_000)`
(clamp-to-oldest) at 60×16; scroll_back=Some(0) ≡ follow; C11 = the F-B
matrix (400 renders incl. 1×1) re-run over the NEW code, promoted to a
permanent test.
**Loop budget:** 400 renders × O(cells ≤ 20k) ≈ 8×10^6 — test-only, one-off
per CI run (~50 ms measured on the F-B baseline). Justified.
**Files:** `crates/cyril-ui/src/floor_tests.rs` (fences
`browse_mode_usable_at_floor`, `no_fallback_size_sweep`);
DELETE `crates/cyril-ui/src/probe_a14l.rs` + its `lib.rs` line (probe
scaffolding superseded by permanent fences; audit trail lives in
`.cyril-a14l/probe-testbackend-output.txt`).
**Verification:** both fences pass; probe module gone; full gates pass.

---

## Plan Self-Review

1. **Loops:** production loops added: char-wrap row build (S5, O(draft
   chars), ceiling 10^5 = parity with existing per-frame scan, justified);
   suggestion/approval/picker windows (O(visible rows) ≤ area height);
   budget arithmetic O(1). Test-only sweeps: S1 5.2k, S4 31k, S10 8×10^6
   one-off (justified, measured ~50ms). No gaps.
2. **Fixtures:** every slice's fixture embeds a named bug class — five
   reproduce main's real bugs (S2 clipped selection, S4 covered input, S6
   invisible cursor, S8 off-screen ▸, S9 moving input); adversarial
   entries: wide-char wrap straddle, usize::MAX cursor, empty region,
   area=0, clamp-to-oldest scroll, 1×1 frame. No happy-path-only fixture.
3. **Doc-comment preconditions:** one introduced — `place()` empty-rect
   contract (load-bearing → runtime skip in every caller, S2). Removed one
   documentation lie (input.rs "content scrolls within the box" becomes
   true in S6). No unenforced contracts remain.
4. **Write targets:** committed fixtures (data, generated offline);
   tracing debug on budget degradation (diagnostic, S7); everything else
   is test assertions. No new stdout writes.
5. **Tracker references:** cyril-lme2 (crew collapse), cyril-uw20 (hooks
   table content), cyril-9ode (gauges), cyril-4vvw (input editing),
   cyril-2mfa (completer) — all verified to exist and cover their
   deferrals during design; no new deferrals introduced by this plan.

Claim coverage: C1→S7, C2→S5+S6, C3→S6, C4→S8, C5→S9, C6→S0+S9, C7→S2-S4,
C8→S2, C9→S1, C10→S10, C11→S10. All 11 design claims covered; F-A/F-B were
design-time falsifiers (passed, recorded in design.md).
