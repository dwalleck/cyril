# cyril-leiq — budgeted plan

Approved design: per-tier targets (PRIMARY 4.5 / MUTED 3.0 / SATURATED keep-hue),
fixed-RGB (not remap), 5 role values change, bound to #1e1e2e + #000000. Two
slices: the atomic value+fence change (green), then the AC2 documentation.

Impact analysis (done, pre-plan): the value change breaks exactly —
- `theme.rs:854-862` value-pin test (the 5 changed roles) → update to new values;
  its ~9 UNCHANGED role assertions STAY and serve as claim-6's stability fence.
- `chrome_theme_tests.rs` ansi256/ansi16 `Color::Indexed(N)` assertions for
  emphasis/accent_quinary/accent_quaternary/subdued_negative (chrome uses them)
  → new derived index (the projection code is untouched; only its input moved).
- any `snapshots/*.snap` that render a changed role → `cargo insta` review.
- `highlight.rs` `assert_role_changes_key!` (distinctness, not values) and
  `render.rs:947` (named-color→RGB table) are UNAFFECTED — verified.

Gate per slice: `cargo nextest run -p cyril-ui`, `cargo nextest run --workspace`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`cargo fmt --check`, doctests — real exit codes (`&& echo OK`, never `| tail`).

---

## Slice 1: brighten the 5 dim roles + contrast-contract & hue fences + test fallout

**Claim:** design claims 1-6 — every PRIMARY conversation role ≥ 4.5:1 and every
MUTED role ≥ 3.0:1 vs both #1e1e2e and #000000, saturated hues kept, the link
role ≥ 4.5:1, changed roles keep their hue family, and already-passing roles are
byte-unchanged.
**Oracle:** the Rust contrast fence computes WCAG contrast independently of the
Python probe (`falsifier_proposed.py`); the two must agree per role, and the
in-test white/black == 21.0 anchor validates the Rust formula. Hue: dominant-
channel check.
**Stress fixture:** (a) NON-VACUITY — before flipping the values, the contrast
fence must FAIL on today's `accent_tertiary #000080` (1.02 < 4.5); confirm red,
then green after. (b) hue fence must FAIL if the link value were set to a
red-dominant color (`#ff6c6c`) — confirm by a throwaway flip. (c) the anchor:
if the Rust luminance formula were wrong, white/black ≠ 21.0 → fence aborts.
**Loop budget:** the fence loops 26 roles × 2 backgrounds = 52 O(1) contrast
computations. O(roles), roles = 26. Trivial; no syscalls.
**Wall budget:** n/a (a unit test, not an always-on phase).
**Files:** `crates/cyril-ui/src/theme.rs` (5 literals in `cyril_dark_source`,
update 5 value-pin assertions, add the two fences), `crates/cyril-ui/src/chrome_theme_tests.rs`
(new derived `Indexed` values for the 4 chrome-used changed roles),
`crates/cyril-ui/src/snapshots/*.snap` (accept legitimately-shifted colors).
Justified >2 files: one atomic value change + its mechanical test fallout;
splitting would leave a red intermediate (violates the per-slice green gate).

**Code (advisory):**
- `cyril_dark_source`: `accent_tertiary #6cb6ff`, `accent_quaternary #cd9ee6`,
  `accent_quinary #56c7d0`, `subdued_negative #d98a8a`, `emphasis #d7ba7d`.
- Fence `cyril_dark_contrast_contract` (colocated `#[cfg(test)]`): build
  `resolve_truecolor(CyrilDark)`, a local `contrast(fg,bg)` (WCAG sRGB→linear→
  luminance), assert `contrast((255,255,255),(0,0,0)) == 21.0` (anchor), then a
  per-role tier table asserting each role ≥ its tier vs (0,0,0) AND (0x1e,0x1e,0x2e).
  A helper extracts (r,g,b) from `Color::Rgb` (panic on non-Rgb — truecolor is
  always Rgb here; that panic is a test-only invariant, not release code).
- Fence `cyril_dark_hue_identity`: assert dominant channel per changed role
  (link blue-max, subdued_negative red-max, accent_quinary red-min, etc.).
- Update chrome `Indexed` assertions to the values the failing run reports (only
  after confirming each new index is a sensible projection of the brighter RGB).
Output-stream: test asserts only; no stdout/stderr. Doc-comment preconditions:
the fence's tier table is the contract — a load-bearing invariant enforced by
the test itself (it runs in CI via `cargo test`), not a `debug_assert`.

**Verification:**
- [ ] `cyril_dark_contrast_contract` fails on old `#000080` (red-first), passes after
- [ ] `cyril_dark_hue_identity` passes; fails if a changed role's hue is swapped
- [ ] Rust fence ratios agree with `falsifier_proposed.py` per role (spot 3)
- [ ] value-pin test updated; the 9 unchanged assertions still pass (claim 6)
- [ ] chrome_theme_tests + all snapshots green; each accepted snapshot verified
      to differ ONLY in the changed roles' colors
- [ ] full workspace gate green

---

## Slice 2: document the fixed-RGB + tier decision (AC2)

**Claim:** design claim 7 — the "fixed RGB, not terminal-palette remapping"
decision and the per-tier contrast targets are documented in-repo.
**Oracle:** `grep` — the ADR states fixed-RGB + the three tier numbers; the
`cyril_dark_source` / fence doc-comment points at it.
**Stress fixture:** grep for "fixed RGB" / "4.5" / "3.0" / "terminal-palette" in
the ADR — missing any → the decision is under-documented (the bug class: a
future contributor "simplifies" back to terminal-named colors with no record of
why that was rejected).
**Loop budget:** n/a (prose).
**Wall budget:** n/a.
**Files:** `docs/adr/0007-cyril-dark-contrast-contract.md` (new; follows the
0006 ADR pattern), `crates/cyril-ui/src/theme.rs` (a one-line doc-comment on
`cyril_dark_source` pointing at the ADR). 2 files.

**Code (advisory):** ADR sections — Context (the ghuu dim-VGA migration, probe
numbers: link 1.02:1), Decision (fixed RGB brightened to per-tier targets
PRIMARY 4.5 / MUTED 3.0 / SATURATED keep-hue, measured vs #1e1e2e+#000000;
remapping rejected because it reintroduces terminal-dependence the migration
removed — ANSI projection is cyril-q9dx), Consequences (chrome shares the roles
so it benefits; ansi256/16 derived values shift; the contract fence guards it).
Output-stream: n/a. Doc precondition: n/a.

**Verification:**
- [ ] ADR states fixed-RGB decision + all three tier numbers + the rejected alt
- [ ] theme.rs doc-comment references the ADR
- [ ] full gate green (docs don't affect tests, but run it)

---

## Plan Self-Review

1. **Every loop:** slice-1 contrast fence — O(roles×backgrounds) = 26×2 = 52
   O(1) ops, trivial. ✓ No other loops.
2. **Every fixture (bug class):** slice 1 — (a) fence red on old `#000080`
   (proves it catches the dim-VGA bug, not decoration), (b) hue fence red on a
   hue swap, (c) anchor guards a broken formula. slice 2 — grep catches a
   silent revert-to-terminal-colors. None happy-path-only. ✓
3. **Every doc-comment precondition:** slice-1 fence tier table is load-bearing
   and enforced by the running test (CI `cargo test`), not `debug_assert`; the
   `Color::Rgb` extraction panic is a test-only invariant. ✓
4. **Every write target:** slices are tests + literals + docs — no stdout/stderr
   writes. ✓
5. **Every tracker reference:** cyril-nx1q/q9dx/8r3u/qaq0/fkke/d43s (negative
   space) all verified present this session; cyril-q9dx closed, rest open. No
   new deferral introduced by the plan. ✓

No gaps.
