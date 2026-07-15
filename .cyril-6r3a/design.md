# cyril-6r3a — Design: remove legacy palette access after semantic migration

## Purpose

The contract step of the semantic-theme refactor: delete the obsolete
`palette.rs` surface and enforce the resolved `Theme` (plus the syntax
component) as the only widget color source — with zero rendering change.
The probe proved the module is already ~dead: the 5 color constants and
`MAX_BORDER_WIDTH` have ZERO consumers (compiler-verified), and the two
spinner constants have exactly one production consumer (toolbar) plus a
byte-identical PRIVATE DUPLICATE in chat.rs (ghuu inlined rather than
imported; markdown.rs likewise carries its own private
`MAX_BORDER_WIDTH`).

## Architecture

1. **Delete** `palette.rs` and its `pub mod palette;` line — the whole
   module, not just the dead items. Any future `palette::` reference
   fails to COMPILE, the strongest possible fence.
2. **Single-source the spinner**: new module `crates/cyril-ui/src/spinner.rs`
   holding exactly `SPINNER_CHARS` and `SPINNER_FRAME_MS` (values
   byte-identical to both existing copies); `toolbar.rs` imports it and
   `chat.rs` drops its private duplicate for the same import. No
   animation logic moves — constants only.
3. **Unify the widget color-source fence (AC3)**: extend the existing
   integration scanner (`tests/conversation_theme_sources.rs`, already
   CRLF-hardened per cyril-xi4a, with function-scoped exemptions for
   highlight's signed conversions and Reset comparisons) from 5 modules
   to all 12 `widgets/*.rs` + `highlight.rs`, and rename the file to
   `widget_theme_sources.rs` (the "conversation" name would lie). The two
   in-crate batch fences it subsumes
   (`theme::tests::modal_widgets_have_no_legacy_color_sources`,
   `theme::tests::chrome_widgets_have_no_legacy_color_sources`) are
   deleted; `widgets_only_use_the_explicit_theme` (a different property —
   theme-seam discipline) stays.
4. **markdown's private `MAX_BORDER_WIDTH` stays** — single consumer,
   private constant, correct as-is.

## Input shapes (each covered by ≥1 claim)

| Shape | Values | Claim |
|---|---|---|
| palette items | 6 dead (5 colors + MAX_BORDER_WIDTH) / 2 live (spinner) | C1 (dead), C2 (live) |
| Spinner-constant copies | palette's + chat's private duplicate → exactly one survivor | C2 |
| Widget files under the fence | all 12 `widgets/*.rs` + highlight.rs (exemptions: signed conversions, Reset comparisons) | C5 |
| Existing render baselines | conversation TSV, modal TSV, chrome TSV (18 scenes), 3 insta snapshots | C3 |
| Module reference forms | `use crate::palette` / `palette::X` / re-export | C4 (compile-enforced) |

## Subtractive sweep (step 2b)

Constraint removed: "`palette` exists and exports 8 named constants."
What it silently guaranteed:

1. *toolbar's spinner resolves* → relocation is compile-enforced (C2).
2. *chat/markdown private duplicates shadow nothing* → chat's duplicate
   is deleted with the module in the same change (C2); markdown's stays
   by design (C6).
3. *the conversation scanner's `palette::` token check has a real target*
   → after deletion the PaletteAccess check still guards the PATTERN
   (text-level), and compile failure guards the reality; scanner stays.
4. *test scan-lists naming palette constants stay meaningful* — the dij8
   chrome fence's string array names the constants; that fence is deleted
   as subsumed (C5), so no stale references remain (C7 sweeps).
5. No lock/ordering/uniqueness properties involved; nothing reads
   palette values at runtime beyond the enumerated consumers.

## Claims

1. **C1 — dead-set exactness.** The 6 items {USER_BLUE, AGENT_GREEN,
   SYSTEM_MAUVE, MUTED_GRAY, CODE_BLOCK_BG, MAX_BORDER_WIDTH} have zero
   consumers: deleting only them compiles clean.
2. **C2 — spinner single-source.** Post-change the workspace contains
   exactly ONE definition each of `SPINNER_CHARS`/`SPINNER_FRAME_MS`
   (in `spinner.rs`), consumed by toolbar and chat, values byte-identical
   to both prior copies.
3. **C3 — rendering unchanged.** Every existing equivalence suite passes
   unmodified: conversation baseline, modal baseline, chrome baseline
   (18 scenes), all 3 insta snapshots — no re-baselines this time.
4. **C4 — module gone.** `palette.rs` absent, lib.rs carries no
   `pub mod palette`; any `palette::` reference is a compile error.
5. **C5 — unified fence.** The renamed `widget_theme_sources.rs` scanner
   covers all 12 widget modules + highlight, rejecting `Color::` and
   `crate::palette`/`palette::` tokens (existing exemptions preserved),
   and the two subsumed in-crate batch fences are gone.
6. **C6 — markdown clamp intact.** markdown.rs keeps its private
   `MAX_BORDER_WIDTH = 120` and all three clamp sites.
7. **C7 — no stale references.** No source/comment references to
   `crate::palette` or the deleted constants remain outside `.cyril-*`
   audit artifacts and git history.

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| 1 | C1 | surgically delete only the 6 items; `cargo check --all-targets` | rustc name resolution (probe's gutted-module run proves the mechanism CAN fire: spinner deletion → E0425) | **1m** | **PASSED** (0 errors) | C4's compile enforcement supersedes (module gone) |
| 2 | C2 | `grep -rc "SPINNER_CHARS\s*:" crates/` == 1 definition; unit test pins values + both consumers compile | grep over source text (vs the compiler that enforces resolution) | 5m | pending | `spinner::tests::values_match_frozen_history` (pins the exact char array + 80ms) |
| 3 | C3 | run all suites unmodified | pre-existing baselines (authored in ghuu/nrnq/dij8, frozen commits) | 5m | pending | the baselines themselves (already CI) |
| 4 | C4 | temporarily add `use crate::palette;` to voice.rs → expect compile FAILURE; revert | rustc | 2m | pending | the compiler, permanently |
| 5 | C5 | plant `Color::Red` in voice.rs AND in picker.rs (previously chrome/modal fence territory) → scanner must fire on both; revert | the scanner test run on mutated source (vs the in-crate fences it replaces) | 10m | pending | `widget_theme_sources::rejects_*` suite over 13 modules |
| 6 | C6 | grep markdown.rs for the const + 3 clamp sites post-change | source text | 1m | pending | existing markdown render tests (border width behavior) + C5 scanner unaffected |
| 7 | C7 | `grep -rn "palette" crates/` post-change → only spinner.rs history notes / zero hits | grep | 1m | pending | C4 compile enforcement + C5 token scan |

Non-vacuity (buggy implementation each catches): C1 — a missed consumer
(control: gutting spinner consts DID fire E0425); C2 — chat keeping its
private copy (definition count 2, silent divergence risk); C3 — a typo'd
spinner glyph during relocation (chrome toolbar scenes diff); C4 — a
lingering `pub mod palette` re-export; C5 — the extension silently
dropping picker/approval coverage when the modal batch fence is deleted
(the picker plant fires only if the scanner really covers it); C6 —
over-zealous cleanup deleting markdown's private constant; C7 — a stale
doc comment pointing at a dead module.

Distinctness: each falsifier prints/fails through a separately named test
or a distinct compile/grep invocation.

## Negative space (deliberately NOT in this change)

1. **No color or value changes** — every baseline must pass byte-stable;
   no re-baselines are sanctioned in this issue (unlike dij8).
2. **No touching markdown's private `MAX_BORDER_WIDTH`** — single
   consumer, private is correct.
3. **No scanner-architecture rework** — the extension adds modules to the
   existing MODULES array and renames the file; fixture-plumbing
   consolidation stays cyril-xv3e (verified open).
4. **No theme activation, no new roles, no bundled palettes** —
   cyril-qaq0 / cyril-fkke (verified open; fkke's "palettes" are
   `SourceTheme` value sets, unrelated to this module).
5. **`widgets_only_use_the_explicit_theme` stays untouched** — it fences
   the theme-resolve seam, not color sources.
6. **No spinner behavior/animation changes** — constants move, logic
   doesn't; frame timing stays 80ms.

## Open decisions (for design approval)

1. **Spinner home** — recommended: new shared `crates/cyril-ui/src/spinner.rs`
   module (two constants), toolbar + chat both import it (dedupes chat's
   private copy in-scope; zero behavior change, C2/C3-fenced).
   Alternative: leave chat's private copy and give toolbar its own —
   avoids a new module but perpetuates the duplication this issue exists
   to clean up.
2. **Fence consolidation** — recommended: extend + rename the integration
   scanner to `widget_theme_sources.rs` (13 modules) and DELETE the two
   subsumed in-crate batch fences (modal + chrome no-legacy-source
   scans). Alternative: add-alongside (keep all three) — no coverage
   gain, three places to update per future widget.

## Approval

Pending.
