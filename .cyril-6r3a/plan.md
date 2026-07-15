# cyril-6r3a — Budgeted plan: palette contraction

Design: `.cyril-6r3a/design.md` (approved 2026-07-15). Claims C1–C7.
Branch discipline: every commit gated on
`[ "$(git branch --show-current)" = "feat/cyril-6r3a-palette-contraction" ]`;
`.rivets/issues.jsonl` never committed on this branch; discovered issues
queue in `.cyril-6r3a/to-file.md`.

## Slice 1: shared `spinner.rs` module + value pins

**Claim:** C2 substrate (single source exists; values byte-identical).
**Oracle:** while BOTH copies still exist, a temporary bridge test
asserts `spinner::SPINNER_CHARS == palette::SPINNER_CHARS` and
`spinner::SPINNER_FRAME_MS == palette::SPINNER_FRAME_MS` (the old module
is the ground truth, deleted in slice 4 along with the bridge test); a
permanent test pins the frozen 10-glyph array + 80ms literal.
**Stress fixture:** the permanent pin uses the exact braille sequence
`['⠋','⠙','⠹','⠸','⠼','⠴','⠦','⠧','⠇','⠏']` — a transcription typo in
ONE glyph (the plausible bug) fails both the bridge and the pin.
**Loop budget:** none (constants + O(10) test compare).
**Wall budget:** n/a.
**Files:** `crates/cyril-ui/src/spinner.rs` (new),
`crates/cyril-ui/src/lib.rs` (+1 line).

**Verification:**
- [ ] Bridge + pin tests pass; clippy/fmt clean

## Slice 2: toolbar (+ chrome edge fence) consume `spinner::`

**Claim:** C2/C3 for toolbar — relocation with zero render change.
**Oracle:** the dij8 chrome baseline (frozen TSV, 4 toolbar scenes with
live spinners) — any glyph/timing drift diffs.
**Stress fixture:** `toolbar_sending_full` (spinner at elapsed=5s index)
plus `edge_toolbar_idle_has_no_spinner` (scans for ALL 10 glyphs via the
constant — keeps working through the import swap).
**Loop budget:** none.
**Wall budget:** n/a.
**Files:** `crates/cyril-ui/src/widgets/toolbar.rs` (import swap),
`crates/cyril-ui/src/chrome_theme_tests.rs` (same swap in the edge test).

**Verification:**
- [ ] Full cyril-ui suite green (chrome baseline unmodified)
- [ ] toolbar.rs contains no `palette` token

## Slice 3: chat.rs drops its private duplicate

**Claim:** C2 for chat — the duplicate dies; rendering unchanged.
**Oracle:** conversation baseline TSV (frozen in ghuu, includes chat
scenes) — the strongest independent witness that chat's spinner didn't
change.
**Stress fixture:** grep `const SPINNER` in chat.rs == 0 post-change;
conversation + chrome suites green.
**Loop budget:** none.
**Wall budget:** n/a.
**Files:** `crates/cyril-ui/src/widgets/chat.rs`.

**Verification:**
- [ ] Full cyril-ui suite green, zero re-baselines
- [ ] Exactly one workspace definition of each spinner constant remains + palette's (dies next slice)

## Slice 4: delete `palette.rs` + module line + bridge test

**Claim:** C1/C4/C7 — module gone; compile is the fence; no stale refs.
**Oracle:** rustc — one-shot falsifier: plant `use crate::palette;` in
voice.rs → compile MUST fail → revert (C4 non-vacuity). C7: `grep -rn
"palette" crates/` → zero hits outside `.cyril-*` artifacts/history.
**Stress fixture:** the C7 grep also covers doc comments and test
scan-lists (the dij8 chrome fence's constant-name array dies in slice 5;
sequence note: run the C7 sweep FINAL after slice 5, re-checked in
slice 6).
**Loop budget:** none.
**Wall budget:** n/a.
**Files:** `crates/cyril-ui/src/palette.rs` (deleted),
`crates/cyril-ui/src/lib.rs` (−1 line), `crates/cyril-ui/src/spinner.rs`
(bridge test removed).

**Verification:**
- [ ] Workspace compiles + full suite green
- [ ] Plant-compile falsifier fired and reverted
- [ ] All existing baselines pass unmodified

## Slice 5: extend + rename the scanner; delete subsumed fences

**Claim:** C5 — one fence over all widget modules.
**Oracle:** plant mutations (one-shot, reverted): `Color::Red` in
voice.rs AND in picker.rs — the extended scanner must FAIL on both
(picker proves the deleted modal fence's territory is really covered);
plants verified against the scanner test run, not the in-crate fences
being deleted.
**Stress fixture:** a directory-completeness assert INSIDE the scanner
test: enumerate `src/widgets/*.rs` on disk and assert every file is in
the MODULES array — a widget added next month cannot silently dodge the
fence. (This is the anti-rot bug class: fence lists go stale.)
**Loop budget:** scanner iterates 13 files × O(bytes) ≤ ~400KB,
test-only. Trivial.
**Wall budget:** n/a.
**Files:** `crates/cyril-ui/tests/conversation_theme_sources.rs` →
renamed `crates/cyril-ui/tests/widget_theme_sources.rs` (git mv +
MODULES extension + dir-completeness test),
`crates/cyril-ui/src/theme.rs` (delete the two subsumed batch fences).

**Verification:**
- [ ] Scanner green on the real tree; both plants fire; reverted
- [ ] Dir-completeness assert present and passing
- [ ] theme.rs no longer contains the two batch fences; `widgets_only_use_the_explicit_theme` untouched

## Slice 6: close-out audit

**Claim:** C2 (final count), C3 (all suites), C6 (markdown clamp), C7
(final sweep) + workspace gate.
**Oracle:** grep counts (definitions == 1 per spinner constant;
markdown's `MAX_BORDER_WIDTH` + 3 clamp sites intact); full workspace
gates with real exit codes.
**Stress fixture:** the C7 sweep runs with the widest net
(`grep -rn "palette" crates/`) so any missed comment/doc reference
surfaces here.
**Loop budget:** none.
**Wall budget:** n/a.
**Files:** `.cyril-6r3a/build-audit.md`.

**Verification:**
- [ ] cargo test (workspace) + clippy `--all-targets -D warnings` + fmt + doctests, real exit codes
- [ ] Claim→fence map complete in build-audit.md

## Plan Self-Review

1. **Loops:** only the scanner's file iteration (test-only, ≤400KB text);
   no production loops added or changed. No gaps.
2. **Fixtures:** S1 glyph-typo pin (transcription bug), S2/S3 frozen
   baselines (render drift), S4 plant-compile (fence vacuity), S5 dual
   plants (coverage loss on fence consolidation) + dir-completeness
   (fence-list rot), S6 wide grep (stale references). All adversarial.
   No gaps.
3. **Doc-comment preconditions:** none added; spinner.rs docs state
   provenance only. No gaps.
4. **Write targets:** no runtime output changes; build-audit.md is a
   committed artifact; all test output is harness diagnostics. No gaps.
5. **Tracker references:** cyril-xv3e, cyril-qaq0, cyril-fkke, cyril-leiq
   all verified open during the dij8 run and re-listed in
   `.cyril-6r3a/related-issues.md`; no new deferrals introduced. No gaps.

Claim coverage: C1→S4, C2→S1 (final count S6), C3→S2/S3/S4 (suites) +
S6, C4→S4, C5→S5, C6→S6, C7→S4+S6. All 7 covered.
