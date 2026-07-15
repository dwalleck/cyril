# cyril-6r3a — build audit (slice 6, close-out)

Branch: `feat/cyril-6r3a-palette-contraction`, base `a58d02c`.
Final integration 2026-07-15: full workspace `cargo test` 15/15 result
groups ok, `cargo clippy --all-targets -- -D warnings` clean,
`cargo fmt --check` clean, doctests clean.

## Claim → landed fence map (all 7)

| Claim | Evidence / permanent fence |
|---|---|
| C1 dead-set exactness | Cheapest falsifier PASSED pre-plan (surgical 6-item deletion compiled clean); superseded permanently by C4 |
| C2 spinner single-source | Exactly 1 `const SPINNER_CHARS` definition in crates/ (was 2); `spinner::tests::values_match_frozen_history` pins glyphs+80ms; slice-1 bridge test proved byte-identity with the legacy module while both existed, retired with it |
| C3 rendering unchanged | Conversation, modal, and chrome baselines + all 3 insta snapshots pass UNMODIFIED — zero re-baselines in this PR |
| C4 module gone | `palette.rs` deleted, lib.rs line removed; one-shot plant (`use crate::palette;` in voice.rs) fired E0432 and was reverted; the compiler is the permanent fence |
| C5 unified fence | `tests/widget_theme_sources.rs` (renamed from conversation_theme_sources.rs): MODULES 5→14 (every widgets/*.rs + highlight), one labeled loop test, dir-completeness anti-rot fence; one-shot plants in picker.rs (ex-modal territory) AND voice.rs both fired; the two subsumed in-crate batch fences deleted |
| C6 markdown clamp intact | `MAX_BORDER_WIDTH` private const + 3 clamp sites present (5 token hits) in markdown.rs |
| C7 no stale references | `crate::palette`/`palette::` grep: only the scanner's own detection tokens + 3 theme.rs comments reworded to "was palette::X (module removed)"; remaining word-hits are the ANSI-palette sense (`nearest_palette`, xterm docs) and spinner.rs provenance |

## Issue AC → evidence

- AC1 (no production palette imports/access): C4 (compile) + C5 scanner.
- AC2 (obsolete surface removed, rendering unchanged): module deleted;
  C3 baselines byte-stable, zero re-baselines.
- AC3 (regression fence vs new direct widget color sources): C5 —
  14-module scanner with dir-completeness anti-rot, plants proven on
  both an ex-modal and an ex-chrome module.
- AC4 (workspace gate): this audit's integration run.

## Deviations from plan

1. Slice 5 initially committed through a `;`-broken gate chain (clippy
   `expect_used` failure + a fmt artifact escaped into the commit);
   caught immediately post-commit, fixed, and amended before push. The
   binding convention ("gates use real exit codes — never split the
   chain") held only because of the post-commit re-check; noted for the
   review log.
2. Slice 6 rewords three theme.rs comments (stale dead-module names) —
   within C7's scope, not planned as a code change.

## Discovered issues to file at close-out

None — `.cyril-6r3a/to-file.md` never accumulated entries.
