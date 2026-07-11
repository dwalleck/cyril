# Design: Semantic conversation colors

Status: approved with rendered-cell normalization amendment (2026-07-11)

## Purpose

Migrate the five conversation presentation modules to the resolved semantic
`Theme` while preserving Cyril Dark's normalized rendering. The design is
grounded by the signed 29-role specification and by the 81-location production
color inventory whose Rustfmt-parsed probe agrees item-for-item with an
independent lexical oracle.

## Probe ground truth

The agreed ticket-start inventory is frozen in
`.cyril-ghuu/legacy-color-baseline.tsv`: 81 qualified legacy color references
across 5/5 modules and 14 distinct tokens. The initial generic
ast-grep pattern found only 69/81, so the permanent source fence uses
Rustfmt-validated source plus lexical checks rather than that AST pattern alone.

The first design falsifier also corrected the specification. Ratatui named red,
green, yellow, and cyan normalize to ANSI base values `#800000`, `#008000`,
`#808000`, and `#008080`; they are not the bright RGB values originally
assumed. The revised 29-role contract covers all 13 fixed legacy RGB values.

<!-- markdownlint-disable MD013 -->

## Input shapes

| Input | Production-reachable shapes | Claim coverage |
| --- | --- | --- |
| Theme identifier | `ThemeId::CyrilDark` | 1–3 |
| Explicit color mode | True-color, ANSI-256, ANSI-16, no-color | 2, 3, 6–9 |
| Fixed theme shape | 29 fields: 28 RGB sources and one reset canvas; distinct roles may contain duplicate RGB values | 1, 2 |
| Syntax component | `Some(Base16EightiesDark)` in three colored modes; `None` in no-color; catalog lookup present or unexpectedly missing | 2, 7, 9 |
| Chat message kind | User text, agent text, thought, tool call, plan, system text, command output, and steer echo | 5, 10 |
| Steer status | Queued, applied, cleared, unsupported | 5, 10 |
| Activity | Sending, waiting, tool-running, streaming, idle, ready | 5, 10 |
| Tool kind | Read, write, execute, search, think, fetch, switch-mode, other | 5, 10 |
| Tool status | Pending, in-progress, completed, failed | 5, 10 |
| Tool optional data | Path/command/output/error/exit code each present or absent; exit code zero or non-zero; diff old text present or absent | 5, 10 |
| Tool collections | Empty, one item, multiple distinct items; diff tags equal/insert/delete; output below, at, and above truncation limits | 5, 10 |
| Markdown string | Empty, ASCII, Unicode, spaces, and multiple constructs in one document | 5, 11 |
| Markdown construct | Heading levels 1–6, strong, emphasis, strikethrough, inline code, fenced and unfenced code, known/unknown/absent language, list, blockquote, link, rule, and table | 5, 7, 11 |
| Markdown width | 0, 1, below border cap, exactly 80, exactly 120, and above 120 | 5, 11 |
| Input text | Empty, one line, multiple lines, ASCII, Unicode, and spaces | 5, 11 |
| Input cursor | Start, middle, end, and greater than byte length; all production cursors remain UTF-8 character boundaries | 5, 11 |
| Suggestions | Empty, single, 2–10, and more than 10; duplicate labels; description present/absent; ASCII/Unicode/spaces | 5, 11 |
| Suggestion selection | `None`, in-range first/middle/last, outside visible window, and greater than collection length | 5, 11 |
| Markdown cache | Cold, same-theme hit, same content under a different mode, capacity boundary, and poisoned/unavailable lock | 8, 12 |
| Highlight cache | Cold, same-theme hit, same code under colored then no-color mode, capacity boundary, concurrent access, and poisoned/unavailable lock | 9, 12 |
| Render comparison | Four pinned 80×24 scenes at the baseline commit and migrated revision | 5, 6 |

An input cursor inside a UTF-8 code point is not production-reachable because
`UiState` editing preserves character boundaries; existing state tests own that
invariant. Invalid UTF-8 is unrepresentable by Rust `str`. Negative widths and
heights are unrepresentable by the unsigned Ratatui layout types.

## Change classification and removed-invariant sweep

The migration is subtractive underneath its presentation framing: adding a
resolved-theme input removes the process-wide invariant that rendered colors
are fixed constants and therefore that cached output depends only on content,
language, and width.

The removed fixed-theme invariant previously guaranteed these facts for free:

1. Markdown cache entries could be keyed only by content and width.
2. Highlight cache entries could be keyed only by code and language.
3. Every widget in one frame necessarily observed the same hard-coded palette.
4. Syntax highlighting always used one process-wide syntax name.
5. Plain fallback could hard-code white because the only active appearance used
   white primary text.

After the migration, facts 1–5 can all become false. Claims 3 and 7–9 preserve
the required replacement invariants: one copied theme per frame, theme-aware
cache keys, typed syntax selection, and primary-text fallback. Cache capacity,
mutex serialization, compute-on-lock-failure behavior, content truncation, and
layout ownership remain unchanged and are covered by claims 10–12.

The existing `u64` hash-collision risk is neither introduced nor relaxed by
this change: both caches already trust a 64-bit content hash, and the migration
only adds theme fields to that same key domain.

## Architecture

### Theme ownership seam

`UiState` owns one resolved `Theme`, initialized by `UiState::new` with
`resolve(ThemeId::CyrilDark, ColorMode::TrueColor)`. `TuiState` exposes that
copyable value through one read-only `theme()` method, and `MockTuiState`
provides the second adapter needed by renderer tests.

`render::draw_inner` reads `state.theme()` exactly once at frame start and
passes `&Theme` only to the conversation widgets in this migration. This keeps
one-frame consistency behind a small seam, preserves the read-only renderer
interface, and gives verified sibling migrations `cyril-nrnq` and `cyril-dij8`
the same resolved value without making widgets resolve themes independently.

Theme mutation, configuration, automatic detection, and selection are owned by
verified ticket `cyril-qaq0`; this design adds no mutation method or public
configuration field.

### Expanded fixed-field contract

`SourceTheme` and `Theme` gain the ten signed generic fields, producing exactly
29 roles. Their fixed-field representation keeps missing roles a construction
error. `SourceTheme::roles`, resolved-theme inspection, all four resolution
paths, and no-color enumeration become exhaustive over 29 fields.

The ten new source values are the signed table in `.cyril-ghuu/spec.md`.
Resolution reuses the existing pure projector: true-color preserves all 28 RGB
values, ANSI modes independently project all 28, and no-color resets all 29
roles and removes syntax.

### Explicit renderer dependency

The five module interfaces become:

```text
chat::render(frame, area, state, &theme)
markdown::render(markdown, width, &theme) -> Vec<Line>
input::render(frame, area, state, &theme)
suggestions::render(frame, area, state, &theme)
highlight::{highlight_block, highlight_line}(..., &theme)
```

Internal chat and Markdown helpers receive `&Theme` from their module entry
point instead of reading global state. This parameter is the seam: callers and
tests provide the same resolved value, while role-to-color mapping stays local
to each presentation module.

Unstyled spans remain unstyled. The migration replaces only explicit legacy
color choices; forcing primary text onto previously inherited terminal text
would violate normalized compatibility.

### Syntax presentation

`highlight.rs` removes its process-wide `THEME_NAME` constant. For
`theme.syntax = Some(id)`, it resolves `id.name()` in Syntect's loaded catalog;
ANSI-256 and ANSI-16 modes retain Syntect's RGB token output as signed. For
`theme.syntax = None`, failed catalog lookup, or highlighting error, it emits
plain spans using `theme.text`. No-color therefore emits reset rather than a
concrete color. Private result-normalization helpers accept catalog `Option` and
Syntect `Result` values so tests can supply `None` and `Err` without adding a
public adapter or requiring a malformed production string.

The existing RGB conversion and diff-tint color math remain internal
conversion logic. They do not select displayed policy colors and are the
source-fence exceptions named by the specification.

### Theme-aware caches

Markdown's cache hash includes Markdown content, width, and every field of the
resolved `Theme`, including the optional syntax component. Highlight's block
cache hash includes code, language, and the same complete theme identity.
Hashing the complete resolved value, rather than only `ThemeId` or
`ColorMode`, makes synthetic test themes and the `cyril-fkke` bundled palettes
safe at the same interface.

Both caches remain 256-entry `HashCache` values behind their existing mutexes.
A failed lock still bypasses caching, computes the requested themed output, and
does not panic. Highlighted single lines remain uncached.

### Compatibility fixtures

A test-only normalized-cell representation stores symbol, modifiers,
foreground, and background. Named ANSI colors from the baseline are converted
through the canonical table before comparison; RGB and indexed colors retain
distinct normalized forms. Ratatui `Buffer::Cell` irreversibly represents both
an absent style color and explicit `Color::Reset` as `Color::Reset`, so both
normalize to one `DEFAULT` rendered-cell value. The requester approved this
reality-driven amendment on 2026-07-11 after the Slice 15A preflight falsified
the earlier distinction.

A test-only scoped scene renderer composes only the five conversation modules
into each 80×24 buffer; it excludes chrome and modal surfaces owned by verified
tickets `cyril-dij8` and `cyril-nrnq`. The four signed scenes are rendered in an
isolated worktree at commit
`80f3ffa5a7ced20e33c9b98c782c08af704407d5` to create immutable normalized
fixtures. The migrated renderer must match those fixtures in true-color. These
fixtures become deterministic CI regression fences rather than a one-time
manual comparison.

Separate scoped mode-matrix tests render all four scenes under all four explicit
modes. They report results per scene and mode, reject any no-color cell
containing a non-reset color, and permit RGB syntax tokens in both ANSI modes.

### Static migration fence

A test reads the five production sources after Rustfmt validation. It reports
legacy palette imports and hard-coded displayed color calls per module.

- `chat.rs`, `markdown.rs`, `input.rs`, and `suggestions.rs` permit zero
  production `palette::` or `Color::` color selections.
- `highlight.rs` permits no palette reference and permits direct `Color`
  construction only in the signed Syntect conversion and diff-tint conversion
  bodies.
- A hard-coded fallback such as `.fg(Color::White)` remains forbidden.

This fence deliberately does not use the generic ast-grep pattern that missed
12 locations during the prove-it loop.

## Claims

1. The 29-role Cyril Dark source contains every fixed canonical RGB value selected by the 81-location legacy inventory.
2. All 28 RGB roles project by the signed nearest-color rules in both ANSI modes, while no-color resets 29/29 roles and removes syntax.
3. Production initializes Cyril Dark true-color once in `UiState`, and every conversation module in one frame receives the same copied resolved theme.
4. The five scoped production modules contain no legacy palette access or hard-coded displayed color outside the two signed conversion bodies.
5. The migrated true-color renderer differs from the baseline by zero normalized symbols, modifiers, foregrounds, or backgrounds across the four 80×24 scenes.
6. All 16 scene-mode combinations use only resolved UI roles or selected syntax output, and the four no-color scenes contain zero non-reset colors.
7. Syntax rendering covers present, absent, missing-catalog, successful-highlight, and highlight-error shapes without panic and uses primary-text fallback where required.
8. Markdown cache results are distinguished by complete resolved theme identity as well as content and width.
9. Highlight-block cache results are distinguished by complete resolved theme identity as well as code and language.
10. Every chat-message, activity, tool-kind, tool-status, optional tool-data, and truncation shape retains its pre-migration symbols, modifiers, ordering, and limits.
11. Every enumerated Markdown, input, and suggestion shape retains its pre-migration content and layout behavior, including zero width, Unicode, empty collections, out-of-window selection, and the 10-row cap.
12. Both color-bearing caches retain 256-entry capacity, mutex serialization, deterministic same-input output, and compute-on-lock-failure behavior.

## Falsification

| # | Claim | Falsifier | Independent oracle | Cost | Status | Regression fence |
| ---: | --- | --- | --- | ---: | --- | --- |
| 1 | The 29 roles cover every fixed legacy RGB value. | Parse all fixed tokens in the frozen 81-row ticket-start inventory and compare their actual palette or canonical ANSI RGB values with the two signed role tables; any missing RGB falsifies the claim. The rejected 26-role draft is the concrete buggy implementation and reports four missing values. | `.cyril-ghuu/cheapest-falsifier.py` derives palette RGB from production declarations and named RGB from the canonical ANSI table, independently of theme implementation. | 2s | passed: 13 required values, 0 missing | Contract test `theme::tests::conversation_legacy_colors_are_representable`; reverting emphasis or omitting quinary accent must fail with the missing RGB. |
| 4 | Scoped production has no forbidden color source. | Run the Rustfmt-backed source fence and report each module separately; any palette token or non-conversion hard-coded displayed color falsifies the claim. Leaving one of the current 81 references in `chat.rs` is the buggy implementation. | Raw-source `awk`/`grep` oracle with a manually pinned palette allowlist, retained from the prove-it artifact. | 5s | pending | Test `theme::tests::conversation_widgets_have_no_legacy_color_sources`, with one assertion/output label per module. |
| 3 | One true-color theme value feeds the whole production frame. | Give `MockTuiState` a marker theme whose role colors are pairwise distinct and render one frame; any scoped cell using a default-resolved or second theme falsifies the claim. A widget calling `resolve_truecolor` internally is the buggy implementation. | Hand-pinned marker-role-to-cell table checked from the Ratatui buffer; production default is separately compared with `resolve(CyrilDark, TrueColor)`. | 10s | pending | Tests `render::tests::conversation_frame_uses_state_theme_once` and `state::tests::new_state_uses_cyril_dark_truecolor`. |
| 8 | Markdown cache keys include complete theme identity. | Render identical Markdown and width under four marker themes sequentially; a repeated color from the first render falsifies the claim. Hashing only content and width is the buggy implementation. | Expected role color at a named Markdown span is taken directly from each marker theme, not from another renderer path. | 10s | pending | Test `markdown::tests::cache_distinguishes_resolved_themes`, reporting one result per mode. |
| 9 | Highlight cache keys include complete theme identity. | Render identical code/language under colored then no-color themes and reverse the order; any stale RGB/reset result falsifies the claim. Omitting theme from the existing code/language hash is the buggy implementation. | Syntect catalog output establishes colored spans; the signed no-color theme independently requires reset spans. | 10s | pending | Test `highlight::tests::cache_distinguishes_resolved_themes`, with colored-first and no-color-first assertions. |
| 7 | Every syntax shape follows the selected component or primary-text fallback. | Exercise syntax `Some`/`None`, valid/invalid catalog lookup, known/unknown language, and injected highlighter error; a panic or non-primary fallback falsifies the claim. Keeping `THEME_NAME` or `Color::White` fallback is the buggy implementation. | Syntect's packaged catalog plus a hand-pinned fallback theme color; each branch emits a distinct `SYNTAX_*` label. | 20s | pending | Tests `highlight::tests::{selected_component,none_uses_primary_text,missing_uses_primary_text,error_uses_primary_text}`. |
| 2 | All 28 RGB roles project correctly and no-color resets all 29. | Compare 56 ANSI projections with brute force and enumerate no-color roles; any index mismatch, concrete no-color role, or retained syntax falsifies the claim. Extending `Theme` but forgetting a field in `resolve_with` is the buggy implementation. | Python generates both ANSI palettes independently and parses the signed 29-role table. | 30s | pending | Exhaustive tests `theme::tests::all_roles_project` and `theme::tests::no_color_resets_all_29_roles`. |
| 12 | Cache capacity and failure/concurrency behavior remain intact. | Fill each cache through 256 entries, cross the boundary, run concurrent themed requests, and inject a poisoned local mutex; wrong eviction, mixed colors, panic, or missing computed output falsifies the claim. Unwrapping the mutex or changing capacity is the buggy implementation. | A local insertion-order ledger and direct uncached themed computation report `CAPACITY`, `CONCURRENT`, and `POISON` independently. | 45s | pending | Tests `markdown::tests::cache_policy_is_preserved` and `highlight::tests::cache_policy_is_preserved`, using an injectable private cache helper. |
| 6 | The 16 scene-mode buffers contain only allowed mode output. | Render each scene in each mode; report forbidden UI colors and non-reset no-color cells per scene/mode. A single retained `Color::Cyan` in input is the buggy implementation. | Python projection output supplies allowed UI role colors; Syntect's catalog independently supplies allowed syntax RGB colors. | 1m | pending | Integration test `render::tests::conversation_mode_matrix`, with 16 distinctly labeled assertions. |
| 10 | Chat/tool non-color behavior is unchanged for every enumerated shape. | Render a compact per-variant matrix before and after migration and compare symbols, modifiers, order, and truncation counters while ignoring color; changing a match arm or limit during migration is the buggy implementation. | Fixtures rendered from the pinned baseline commit, one label per message/activity/tool shape. | 2m | pending | Parameterized widget tests `chat::tests::semantic_migration_preserves_shape_matrix`. |
| 11 | Markdown/input/suggestion behavior is unchanged for every enumerated shape. | Run the full shape matrix against baseline and migrated code; any text, row, width, cursor, selection-window, or panic difference falsifies the claim. Accidentally styling raw text or changing a width calculation is the buggy implementation. | Pinned baseline text/geometry fixtures plus `catch_unwind` for panic cases, with separate `MARKDOWN`, `INPUT`, and `SUGGESTIONS` outputs. | 2m | pending | Existing edge tests plus new parameterized tests `semantic_migration_preserves_{markdown,input,suggestions}_shapes`. |
| 5 | Four true-color scenes are normalized-cell equivalent. | Compare 7,680 normalized cells against baseline fixtures; any per-scene symbol, modifier, foreground, or background count above zero falsifies the claim. Mapping named cyan to bright cyan is the known buggy implementation. | Immutable normalized fixtures generated in an isolated worktree at the pinned commit. | 3m | pending | Snapshot tests `render::tests::conversation_compat_{messages,tools,markdown,input}` over normalized cells. |

### Cheapest falsifier result

The cheapest falsifier was run after revising and re-signing the specification:

```text
required=13 available=19 missing=0
```

The passing command output is in
`.cyril-ghuu/design-falsifier-output.txt`. The earlier 26-role run is preserved
in `.cyril-ghuu/design-falsifier-failed-26.txt`; it produced four distinct
failures for `#008000`, `#008080`, `#800000`, and `#808000`. The 29-role design
therefore survives a falsifier that the rejected design did not.

## Negative space

- Startup configuration, automatic capability detection, and session-local
  selection are owned by verified ticket `cyril-qaq0`.
- Five additional bundled palettes are owned by verified ticket `cyril-fkke`.
- Approval, picker, hooks, and code-overlay migration is owned by verified ticket
  `cyril-nrnq`.
- Toolbar, status, crew, voice, and shared-frame migration is owned by verified
  ticket `cyril-dij8`.
- Global removal of legacy palette access is owned by verified ticket
  `cyril-6r3a`.
- Responsive 60×16 layout behavior is owned by verified ticket `cyril-a14l`.
- Multiline navigation and undo/redo are owned by verified ticket `cyril-4vvw`.
- ANSI projection of Syntect token colors is deliberately not performed; the
  signed behavior keeps Syntect RGB in both ANSI modes.
- The historical `.cyril-ixua/spec.md` remains immutable evidence; this ticket
  extends production contract code and current tests instead.

## Self-review

- **Claim count**: 12; one migration seam, one fixed contract extension, two
  caches, syntax behavior, compatibility, modes, and preserved shape behavior.
- **Input coverage**: every reachable enum variant, optional branch, collection
  cardinality, numeric boundary, string shape, cache state, and render scene is
  assigned to at least one claim.
- **Removed invariants**: all five consequences of removing fixed process-wide
  colors are covered by claims 3 and 7–9; unchanged cache/layout behavior is
  covered by claims 10–12.
- **Falsifier independence**: canonical palette generation, Syntect's catalog,
  marker themes, raw-source grep, pinned baseline fixtures, and local cache
  ledgers do not reuse the implementation mechanism they judge.
- **Non-vacuity**: every row names a concrete rejected implementation, including
  the already-failing 26-role contract, retained constants, per-widget resolve,
  stale content-only keys, fixed syntax name, omitted projection field, mutex
  unwrap, match-arm drift, width drift, and bright/base ANSI confusion.
- **Distinctness**: each claim has its own output prefix or named test; scene,
  mode, module, cache, and input-shape failures identify their source directly.
- **Cost distribution**: all falsifiers cost three minutes or less; no staging,
  production data, or manual-only regression fence is required.
- **Tracker references**: `cyril-qaq0`, `cyril-fkke`, `cyril-nrnq`,
  `cyril-dij8`, `cyril-6r3a`, `cyril-a14l`, and `cyril-4vvw` were verified in
  Rivets on 2026-07-11, and each description covers the named negative space.

## Approval

The requester approved this design on 2026-07-11.

<!-- markdownlint-enable MD013 -->
