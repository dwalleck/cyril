# cyril-x5xi: structural rendering inputs and identities

## Route and inputs

Structural; behavior and routing evidence are in [route.md](route.md). This is the first independently mergeable PR in the proposed three-issue stack. [verification.md](verification.md) owns Main's completed current-source workspace, actual-App consumer, F1 source-gate A/B and independent review receipts, the original-base sensitivity boundary and remaining parent publication status. No check was executed by the documentation worker.

## Selected design

Make the values accepted by the actual renderer implementation be the values hashed by its cache. A separate manually maintained identity projection would recreate the original failure mechanism.

1. Derive `Hash` alongside existing equality for resolved `Theme` and `SyntaxTheme`. This supplies structurally complete identity for a consumer that really depends on the entire resolved Theme; adding a field automatically participates. Do not hash ThemeId, requested ColorMode, or a syntax name string in place of the actual resolved rendering inputs.
2. Add two concrete crate-private Copy/Debug/Eq/Hash rendering-input structs owned by the theme module:
   - `HighlightStyle { syntax: Option<SyntaxTheme>, text: Color }`.
   - `MarkdownStyle { highlight: HighlightStyle, code: Color, subdued: Color, emphasis: Color, accent_tertiary: Color, accent_quinary: Color }`.
3. Preserve public `highlight_block_with_theme(..., &Theme)` and `markdown::render_with_theme(..., &Theme)` as real adapters: resolve the appropriate style once, then delegate. They contain no rendering decisions after conversion.
4. The cache-key helper and **all actual renderer/helper bodies** take the corresponding concrete style, not Theme. Markdown's nested highlighting call accepts its nested `HighlightStyle` directly. This makes the dependency closure structural: a helper cannot start reading another Theme field without changing its concrete input type; all fields of that input automatically participate through derived `Hash`.
5. Content/language remain highlight key inputs; content/width remain Markdown key inputs. Existing `HashCache` policy and locking are unchanged. This issue does not expand into replacing its hash-only key storage or changing collision policy, which predates the request and is not needed for theme dependency completeness.

The struct fields above are exactly the current source closure, not a fixed list of every future relevant field. Adding a renderer dependency means adding it to the renderer's input type, which changes identity automatically; merely adding an unrelated role to Theme has no effect on narrow cached output. The full Theme derive remains available to the later conversation block-layout cache, whose output includes all message/header/thought/tool roles.

## Placement and module shape

| Module | Disposition / owned responsibility | Interface / forbidden responsibility | Check |
|---|---|---|---|
| `theme.rs` | Retain theme ownership; add structural derives and concrete style projections. | Own resolved inputs, not cache storage or rendering loops. No manual color-hashing list. | Compiler derives and independent review. |
| `highlight.rs` | Deepen existing highlighting owner. | Public Theme adapter; internal style-only renderer/key/fallback paths. No Markdown geometry or UI-state mutation. | Existing tests plus dependency/isolation fixtures. |
| `widgets/markdown.rs` | Deepen existing Markdown owner. | Public Theme adapter; style-only parsing, tables, code frames and inline helpers. No App or core mutation. | Existing fixtures and narrow-identity checks. |
| `cache.rs` | Unchanged existing cache policy. | Do not introduce a generic theme dependency registry or new cache implementation. | Review scope. |

No module split, generic strategy trait, global invalidation bus, runtime feature dependency set, palette registry redesign or production dependency is needed. These small input values earn their seam by simultaneously constraining implementation dependencies and identity; they are not pass-through layers with duplicate field lists.

## Claims / falsification

| ID | Claim and cases | Falsifier / verification | Regression coverage | Status |
|---|---|---|---|---|
| C1 | Full Theme identity includes every field structurally; a newly used renderer field must enter its hash-derived input. | Compiler derives on Theme/SyntaxTheme/style structs; independent review verifies renderer bodies have no Theme parameter/capture and no handwritten role hashing. A renderer using Theme directly or a separate manually maintained style identity falsifies this mechanism. | Existing marker theme and updated cache-input tests. | PASS — current workspace compile/tests and ReviewThemeIdentity review; verification.md. |
| C2 | Highlighting keys contain exactly syntax + fallback text, in addition to content/language. | With local injectable cache: change syntax or fallback text and observe distinct entries/appropriate changed output; change only canvas/code/chrome/selection and observe reuse, including no-syntax and forced-error fallback. Different code/language remains distinct. | Extend `highlight.rs` existing local-cache, fallback and theme tests. Preserve no-color equivalence and colored modes. | PASS — current regression plus separately pinned original-base sensitivity; verification.md. |
| C3 | Markdown key includes its full direct/transitive rendering closure and width, but unrelated theme roles do not invalidate it. | Composite Markdown fixture includes heading levels, list, link, quote, code background/fallback, rule and table. Change each concrete input independently, check cache isolation and the affected cell/style; vary unrelated roles and assert one retained local-cache entry plus identical output. Exercise both population orders. | Existing local-cache seam and Markdown shape/theme tests. Replace obsolete all-29-roles-invalidates expectations. | PASS — current workspace regression and review; verification.md. |
| C4 | Palette/mode output and existing lifecycle behavior do not change. | Existing frozen conversation/theme fixtures, bundled palettes/modes, concurrency/poison/eviction tests and root TestBackend rendering remain green. Actual-App consumer theme switching must change committed + live code; cancel-preview must restore prior colors. | Existing render/chat/highlight/Markdown coverage plus hash-pinned owned-peer CLI/PTY consumer. Separate startups cover truecolor/none/truecolor-return; no same-process mode transition or native graphical glyph claim. | PASS — current regression and bounded actual-App consumer; verification.md. |

The original architecture change needed no bespoke oracle/mutation beyond the local-cache behavior and output controls. The executed original-base irrelevant-role reuse RED/GREEN is retained, with its x5xi/zj0m boundaries, in verification.md. F1's later exact allowed-import source-gate repair has its own existing review obligation and actual-Cargo A/B receipt in review-decisions.md; it does not add a rendering/performance requirement.

## Delivery and verification protocol

One atomic issue PR changes theme derives, input types, every internal consumer/key/helper, affected tests and post-smoke documentation in the same boundary. Existing public names remain because they continue to provide the useful Theme-to-input adaptation, not because of a compatibility shim. Before implementation, rerun LSP references for any exported symbols actually changed. Earlier linked-worktree LSP references/definition returned no resolution despite listed symbols; this tool limitation was reported and source callers were mapped as fallback.

Main's executed commands and raw current-source results are retained in verification.md and the portable evidence archive. The assembled workspace, workspace/all-targets clippy, executable build and formatting passed. The actual tested executable rendered committed/live Markdown and syntax code through an owned v2 peer, with palette preview/cancel/commit, resizing, browsing and live append. Separate startup processes cover truecolor, none and truecolor return because mode is startup-only. This is real App/CLI-process output evidence decoded from PTY ANSI, not a live Kiro/model session or native font/glyph qualification.

No new wall-clock bound is required for this identity-only issue. The later render benchmarks measure integrated impact; deterministic cache hits/misses establish this issue's actual performance requirement without a fabricated frame budget.

## Non-goals and future work

No new rendering behavior, theme roles, syntax themes, palettes, history policy, cache capacity, or generic cache redesign. No future deliverable is being deferred from this issue. `cyril-zj0m` and `cyril-c6la` are independently verified existing records, not a reason to omit this issue's own isolation/reuse tests or review.

## Approval

**DG — 2026-10-19, approved by parent Main under delegated routine sign-off.** User delegation reported verbatim by Main: “Delegate routine sign-offs” — “I select and record evidence-backed designs. I still ask before scope changes, relaxed acceptance, architecture changes outside the existing boundaries, or explicit risk acceptance.” Main approved the concrete hash-derived HighlightStyle/MarkdownStyle input identity and structural full Theme identity. No accepted risk, hard-limit change or rendering-fidelity deviation is authorized by this design. Main owns regression/smoke/repository verification, independent review and the issue PR; current completed evidence and the separately requester-accepted Windows baseline limit are indexed in verification.md.
