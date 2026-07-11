# Feature: Semantic colors for conversation surfaces

## What this is

Cyril will migrate chat, Markdown, syntax presentation, message input, and
autocomplete rendering from legacy or hard-coded colors to a resolved semantic
theme. Production continues to select Cyril Dark true-color until theme
configuration is activated, and its canonical rendered appearance remains
unchanged.

## Users

- **Cyril terminal operator**: reads and writes conversations and must retain the
  current Cyril Dark conversation appearance and behavior.
- **Cyril UI contributor**: changes conversation widgets and needs every
  explicitly selected conversation color to come from a semantic theme role or
  its syntax component.

## Behavior

### Render conversation surfaces from the theme

- **Given**: a resolved theme and any conversation state
- **When**: Cyril renders `chat.rs`, `markdown.rs`, `input.rs`, `suggestions.rs`,
  or `highlight.rs`
- **Then**: every explicitly selected displayed color comes from a theme role,
  the selected syntax component, or a diff tint computed from syntax output and
  semantic diff roles

### Preserve Cyril Dark conversation rendering

- **Given**: baseline commit
  `80f3ffa5a7ced20e33c9b98c782c08af704407d5` and the migrated revision
- **When**: both revisions render the four pinned 80×24 scenes in Cyril Dark
  true-color and named baseline ANSI colors are normalized through the
  canonical ANSI-16 RGB table
- **Then**: all 7,680 cells have identical symbols, modifiers, foregrounds, and
  backgrounds

The four pinned scenes are:

1. User-message, agent-message, system-message, thought, steer-status, and
   activity output.
2. Every tool kind and status, diff additions/deletions/context, and tool output.
3. Markdown headings, emphasis, links, lists, tables, quotes, inline code,
   fenced Rust, and unknown-language fallback.
4. Multiline input with cursor and autocomplete suggestions.

### Use true-color in production

- **Given**: theme configuration and automatic detection have not yet been
  activated
- **When**: production rendering resolves its theme
- **Then**: it selects Cyril Dark true-color unconditionally

### Render every explicit mode without hard-coded color

- **Given**: each pinned scene and each of true-color, ANSI-256, ANSI-16, and
  no-color mode
- **When**: all 16 scene-mode combinations render
- **Then**: UI colors come from the resolved roles, syntax tokens come from the
  selected Syntect component, and no-color contains no non-reset foreground or
  background colors

In ANSI-256 and ANSI-16 modes, syntax-token colors remain the RGB values emitted
by Syntect. In no-color mode, `Color::Reset` is allowed; named, indexed, and RGB
colors are forbidden.

### Keep color-bearing caches theme-aware

- **Given**: identical Markdown or source content rendered sequentially under
  different resolved themes or color modes
- **When**: a cached result exists for an earlier theme
- **Then**: each request returns styles for the requested theme rather than
  stale cached colors

Both existing color-bearing caches retain their 256-entry capacities.

### Fall back safely when syntax loading fails

- **Given**: the selected syntax component cannot be loaded despite contract
  validation
- **When**: Cyril renders a code block or highlighted line
- **Then**: rendering continues as plain text using the resolved primary-text
  role and does not panic

### Preserve empty and constrained behavior

- **Given**: empty content, zero-width Markdown, no autocomplete suggestions, or
  a missing/out-of-window autocomplete selection
- **When**: the corresponding conversation surface renders
- **Then**: rendering does not panic, absent suggestions consume 0 rows, and the
  autocomplete panel remains capped at 10 rows

## Expanded semantic contract

This ticket extends the production contract from 19 to exactly 29 UI roles and
retains one syntax component. The ten added generic roles are:

| Added role | Cyril Dark true-color source |
| --- | ---: |
| Emphasis | `#808000` |
| Tertiary accent | `#000080` |
| Quaternary accent | `#800080` |
| Quinary accent | `#008080` |
| Subdued | `#808080` |
| Subdued positive | `#008000` |
| Subdued negative | `#800000` |
| Soft accent | `#8ab4f8` |
| Positive accent | `#81c784` |
| Inset background | `#282c34` |

The historical `.cyril-ixua/spec.md` remains unchanged. Production contract
code and tests become authoritative for the expanded 29-role shape.

### Legacy-to-semantic mapping rules

| Legacy use | Semantic source |
| --- | --- |
| User identity | User-message role |
| Agent identity | Agent-message role |
| System identity | System-message role |
| Unselected autocomplete entry or non-identity user blue | Soft accent |
| Applied steer or non-identity agent green | Positive accent |
| Code-block background | Code background |
| Autocomplete selection background | Inset background |
| Legacy muted gray | Muted text |
| Named dark gray | Subdued |
| Named white | Primary text |
| Named cyan | Quinary accent |
| Named yellow, including pending/warning output | Emphasis |
| Named blue | Tertiary accent |
| Named magenta | Quaternary accent |
| Named green, including successful status and diff addition | Subdued positive |
| Named red, including failed status and diff deletion | Subdued negative |
| Syntax token | Selected syntax component |

Generic roles may be shared across unrelated presentation meanings. Existing
status, diff, and message-identity roles remain reserved and are not reused for
unrelated meanings; legacy named ANSI output uses the generic compatibility
roles where its canonical value differs from those existing roles.

## Success criteria

- **Migration scope**: 5/5 production modules obtain every explicitly selected
  displayed color through the semantic contract, measured by source inspection
  and the regression fence.
- **Contract completeness**: 29/29 UI roles and 1/1 syntax component resolve,
  measured by an exhaustive contract test.
- **Added-role sources**: 10/10 added roles match the pinned RGB values, measured
  by true-color contract inspection.
- **Projection correctness**: 56/56 colored role projections (28 colored roles ×
  2 ANSI modes) select the nearest allowed palette entry with the upstream
  tie-breaking rules, measured by the existing independent projection oracle.
- **True-color compatibility**: 0 symbol, modifier, foreground, or background
  differences across 7,680 normalized cells, measured against the pinned
  baseline revision.
- **Mode coverage**: 16/16 pinned scene-mode combinations render, measured with
  Ratatui `TestBackend` buffers.
- **No-color completeness**: 0 non-reset foreground/background colors across the
  4 no-color scenes, measured by cell inspection.
- **Cache correctness**: 8/8 content-cache combinations (2 caches × 4 modes)
  match an uncached rendering of the requested mode, measured by sequential
  cache tests.
- **Regression fence**: 0 legacy palette imports and 0 hard-coded displayed
  colors in the 5 production modules, measured by a source-level check. The
  only color-construction exceptions are Syntect RGB conversion and diff-tinted
  RGB computation from syntax plus semantic roles.
- **Capacity compatibility**: both 2/2 color-bearing caches retain 256 entries
  and autocomplete retains its 10-row maximum, measured by tests and constant
  inspection.
- **Regression compatibility**: 100% of workspace tests pass, measured by
  `cargo test --workspace`.
- **Quality gate**: formatting, strict workspace Clippy, and workspace tests all
  exit with status 0.

No new latency target is introduced by this ticket.

## Edge cases and decisions

| Edge | Decision | Rationale |
| --- | --- | --- |
| Named ANSI colors vary by terminal | Normalize the baseline through the canonical upstream ANSI-16 RGB table | Compatibility needs deterministic ground truth. |
| One color serves unrelated meanings | Use generic roles rather than one role per meaning | Prevents role proliferation while avoiding identity/status misuse. |
| Existing 19 roles cannot preserve all legacy colors semantically | Add exactly ten roles, for 29 total | Preserves canonical RGB output and the chosen generic-role policy. |
| Existing specific and new generic roles share RGB values | Keep both roles | Identity/status meanings stay separate from generic presentation. |
| ANSI mode renders syntax | Keep Syntect RGB token output | Matches the upstream syntax-component contract. |
| No-color roles resolve to reset | Permit `Color::Reset`, reject every concrete color | Matches the upstream no-color representation. |
| Theme changes while a cached rendering exists | Include resolved theme identity in color-bearing cache keys | Prevents stale colors. |
| Cache lock is unavailable | Preserve uncached rendering behavior rather than panic | Rendering remains available after cache failure. |
| Syntax component lookup fails | Render plain primary-text fallback | A missing optional presentation component must not crash the TUI. |
| Markdown is empty or width is zero | Preserve panic-free existing output | Theme migration does not alter layout semantics. |
| Suggestions are empty | Consume 0 rows and render nothing | Preserves current layout behavior. |
| Selection is missing or outside the visible suggestion window | Render safely without a selected row | Input state can change between completion updates and draws. |
| More than 10 suggestions exist | Keep the selected item in the existing 10-row window | Scope excludes autocomplete layout redesign. |
| Very long chat, diff, or tool output exists | Preserve existing scroll and truncation limits | Scope excludes content and layout changes. |
| Repeated rendering receives the same inputs and theme | Produce identical buffers | Rendering remains deterministic and idempotent. |
| Multiple threads access a color-bearing cache | Retain mutex-protected access | Theme-aware keys must not weaken existing concurrency behavior. |

## Out of scope

This change does **not** include:

- startup configuration, automatic capability detection, or theme selection;
- additional bundled palettes;
- modal or application-chrome migration;
- global removal of the legacy palette;
- layout changes;
- multiline navigation, undo, redo, or other input-editing behavior;
- projecting Syntect token colors into ANSI palettes;
- changing cache capacity or autocomplete row limits;
- editing the historical `.cyril-ixua/spec.md` artifact.

## Constraints

| Dimension | Limit | How measured |
| --- | --- | --- |
| Production module scope | Exactly 5 modules | Source fence |
| Semantic contract | Exactly 29 UI roles plus 1 syntax component | Exhaustive contract test |
| Production default | Cyril Dark true-color only | Render-state test |
| Compatibility baseline | Commit `80f3ffa5a7ced20e33c9b98c782c08af704407d5` | Paired renderer |
| Compatibility scenes | 4 scenes at exactly 80×24 | Paired buffer comparison |
| Normalized rendering change | 0 changed symbols, modifiers, foregrounds, or backgrounds | 7,680-cell comparison |
| Explicit-mode coverage | 4 modes × 4 scenes | 16 buffer renders |
| No-color output | 0 non-reset colors | Cell inspection |
| Legacy imports | 0 across scoped production modules | Source fence |
| Hard-coded displayed colors | 0 across scoped production modules | Source fence |
| Markdown cache | 256 entries | Constant inspection |
| Highlight cache | 256 entries | Constant inspection |
| Autocomplete height | At most 10 rows | Widget test |

## Decisions log

| # | Question | Decision | Why |
| ---: | --- | --- | --- |
| 1 | What is visual equivalence? | Zero differences after canonical ANSI-16 RGB normalization. | Raw named and RGB Ratatui forms differ despite representing the same canonical color. |
| 2 | May the contract gain roles? | Yes, only where the audit finds missing meanings or values. | The original 19 roles cannot preserve every scoped legacy color cleanly. |
| 3 | What is the production scope? | Five modules: chat, Markdown, input, suggestions, and highlight. | These contain the conversation presentation named by the ticket. |
| 4 | Which mode is active before configuration? | Cyril Dark true-color. | Configuration activation belongs downstream. |
| 5 | Must caches understand themes? | Yes. | Color-bearing cached output must not become stale. |
| 6 | Which scenes establish compatibility? | The four pinned 80×24 scenes. | Together they exercise every conversation surface and major color branch. |
| 7 | Are all explicit modes exercised? | Yes, all four modes across all four scenes. | Hard-coded colors must be observable before runtime selection ships. |
| 8 | Are unrelated meanings separate roles? | No; generic roles may be reused. | The requester chose bounded role growth over meaning-specific proliferation. |
| 9 | Which initial generic roles were required? | Emphasis, tertiary accent, quaternary accent, and subdued. | They preserve named yellow, blue, magenta, and dark-gray output. |
| 10 | Who are the audiences? | Cyril terminal operator and Cyril UI contributor. | One observes compatibility; the other consumes the semantic seam. |
| 11 | What adjacent work is excluded? | Configuration, palettes, other widget batches, contract removal, layout, and editing behavior. | Each is independently tracked. |
| 12 | What baseline revision is authoritative? | `80f3ffa5a7ced20e33c9b98c782c08af704407d5`. | It is the ticket-start `HEAD`. |
| 13 | What happens if syntax loading fails? | Plain primary-text fallback. | Presentation failure must not crash rendering. |
| 14 | What does the source fence enforce? | Zero palette imports and hard-coded displayed colors, with two construction exceptions. | Static evidence complements scene coverage. |
| 15 | Does this ticket rewrite the upstream signed artifact? | No; it extends production code/tests to 29 roles. | The upstream artifact remains historical evidence. |
| 16 | Are ANSI syntax colors projected? | No; Syntect continues to emit RGB. | Matches the upstream syntax-component contract. |
| 17 | Is there a new performance target? | No; preserve both 256-entry caches and the 10-row suggestion cap. | This is a presentation migration, not a performance redesign. |
| 18 | What empty/constrained behavior changes? | None. | Scope excludes layout and content behavior. |
| 19 | How are user blue and agent green reused outside identity? | Add soft-accent and positive-accent roles. | Preserves output without identity-role misuse. |
| 20 | How is code background reused by autocomplete? | Add an inset-background role. | Preserves output without code-role misuse. |
| 21 | May no-color styles contain reset? | Yes; reset is not a concrete color. | Matches the upstream resolved theme API. |
| 22 | What did the first design falsifier reveal? | Named ANSI red, green, yellow, and cyan use canonical base values absent from the 26-role draft. | The contract must cover actual Ratatui named-color values, not assumed bright RGB values. |
| 23 | How is the failed coverage falsifier resolved? | Correct emphasis and add subdued-positive, subdued-negative, and quinary-accent roles, producing 29 roles. | Covers all fixed legacy values while retaining the generic-role policy. |

<!-- markdownlint-disable MD013 -->

## Sign-off

The requester typed, verbatim: "The 5 conversation modules use resolved sematic colors, 4 of the 80x24 scenes should have no differences from the baseline, all four color modes are verified. Configuration, extra palettes, removal legacy palates, migration of modals and chrome, layout, and editing changes are out of scope. The semantic contract has 29 roles"

Date: 2026-07-11

<!-- markdownlint-enable MD013 -->
