# Feature: Semantic theme seam for Cyril Dark

## What this is

Cyril will gain a complete semantic color contract for its existing Cyril Dark appearance, including explicit projection into true-color, ANSI-256, ANSI-16, and no-color output. This expand step adds the contract beside existing widget styling; it does not migrate widgets, expose theme configuration, or make additional themes selectable.

## Users

- **Cyril UI contributor**: adds or migrates widgets and needs stable roles whose names describe interface meaning rather than a particular color or widget.
- **Cyril terminal operator**: runs Cyril with its existing appearance and must observe no rendered change from this expand step.

## Behavior

### Resolve Cyril Dark in true-color

- **Given**: the built-in Cyril Dark palette containing all 19 semantic roles and the `base16-eighties.dark` syntax component
- **When**: a Cyril UI contributor resolves Cyril Dark in explicit true-color mode
- **Then**: the resolver returns the pinned RGB-or-reset value for every role and the named syntax component, with no terminal-environment lookup

### Project Cyril Dark to ANSI-256

- **Given**: any explicit RGB role in Cyril Dark
- **When**: a Cyril UI contributor resolves the palette in explicit ANSI-256 mode
- **Then**: the role becomes the entry with minimum squared RGB distance among fixed xterm indices 16–255, including the grayscale ramp; ties select the lower index, and `Color::Reset` remains reset

### Project Cyril Dark to ANSI-16

- **Given**: any explicit RGB role in Cyril Dark
- **When**: a Cyril UI contributor resolves the palette in explicit ANSI-16 mode
- **Then**: the role becomes the entry with minimum squared RGB distance in the canonical 16-color table; ties select the lower index, and `Color::Reset` remains reset

The canonical ANSI-16 RGB table is, by index: `#000000`, `#800000`, `#008000`, `#808000`, `#000080`, `#800080`, `#008080`, `#c0c0c0`, `#808080`, `#ff0000`, `#00ff00`, `#ffff00`, `#0000ff`, `#ff00ff`, `#00ffff`, `#ffffff`.

### Disable color explicitly

- **Given**: Cyril Dark with its UI roles and syntax component
- **When**: a Cyril UI contributor resolves the palette in explicit no-color mode
- **Then**: all 19 UI roles resolve to `Color::Reset` and the syntax component emits zero explicit foreground or background colors

### Preserve current rendering

- **Given**: the unmodified revision and the expand-step revision rendering the same default-idle, active-conversation-with-tool-diff, and open-picker states at 80×24
- **When**: the three pairs of `TestBackend` buffers are compared cell by cell
- **Then**: zero cell symbols and zero cell styles differ

## Cyril Dark compatibility mapping

| Semantic role | True-color source |
| --- | ---: |
| Canvas background | `Color::Reset` |
| Chrome background | `#1e1e2e` |
| Code background | `#282c34` |
| Selection background | `#323246` |
| Primary text | `#ffffff` |
| Muted text | `#8c8c8c` |
| Border | `#8c8c8c` |
| Primary accent | `#00ffff` |
| Secondary accent | `#b48ead` |
| User message | `#8ab4f8` |
| Agent message | `#81c784` |
| System message | `#b48ead` |
| Info | `#00ffff` |
| Success | `#00ff00` |
| Warning | `#ffff00` |
| Danger | `#ff0000` |
| Diff addition | `#00ff00` |
| Diff deletion | `#ff0000` |
| Diff context | `#8c8c8c` |

Separate roles may share a value without becoming semantically interchangeable. The syntax component is `base16-eighties.dark` and remains separate from the 19 UI roles.

## Success criteria

- **Role completeness**: 19/19 semantic roles and 1/1 syntax component resolve, measured by an exhaustive theme-contract test.
- **Source determinism**: 18/18 colored roles use explicit RGB values and the canvas is the sole reset source, measured by inspecting the resolved true-color theme.
- **Projection correctness**: 36/36 role projections (18 roles × 2 ANSI modes) have the minimum squared RGB distance in their specified palette with lower-index tie-breaking, measured by an independent brute-force oracle.
- **No-color completeness**: 19/19 UI roles reset and the syntax component emits 0 explicit colors, measured by the theme-contract test.
- **Syntax validity**: 1/1 syntax component identifier exists in Syntect’s loaded theme set, measured by lookup in the default `ThemeSet`.
- **Rendering compatibility**: 0 symbol differences and 0 style differences across all cells in 3 paired 80×24 buffers, measured against the unmodified revision.
- **Regression compatibility**: 100% of pre-existing workspace tests pass, measured by `cargo test --workspace`.

## Edge cases and decisions

| Edge | Decision | Rationale |
| --- | --- | --- |
| Main canvas currently inherits terminal background | Keep `Color::Reset` in every mode | Forcing RGB would redesign the existing canvas. |
| Two roles currently share one color | Keep separate roles with equal values | Semantic independence allows later themes to diverge safely. |
| Equal RGB distance to two palette entries | Select the lower palette index | Makes output deterministic. |
| ANSI-256 indices 0–15 are terminal-customizable | Search only indices 16–255 | Keeps ANSI-256 projection based on fixed xterm values. |
| ANSI-256 grayscale is closer than the 6×6×6 cube | Select the grayscale entry | Minimum distance, not cube rounding, defines correctness. |
| No-color requested | Reset every UI role and disable syntax colors | No hidden foreground or background colors remain. |
| A source palette uses a named ANSI color | Reject it in contract validation | Named source colors do not have terminal-independent RGB values. |
| Syntax component name is absent from Syntect defaults | Fail contract validation | Silent fallback would hide a broken bundled theme. |
| Environment contains `NO_COLOR`, `TERM=dumb`, or color hints | Ignore it | This ticket supports explicit modes only; automatic detection belongs to `cyril-qaq0`. |
| Existing partial work exposes configuration fields or additional palettes | Remove or defer those portions | Configuration and selection belong to `cyril-qaq0`; five additional palettes belong to `cyril-fkke`. |
| Terminal customizes its base ANSI palette | Continue using the canonical table as the projection oracle | Projection must be reproducible even though physical terminal colors can differ. |

## Out of scope

This change does **not** include:

- Migrating conversation, modal, or chrome widgets to semantic roles; tracked by
  `cyril-ghuu`, `cyril-nrnq`, and `cyril-dij8`.
- Adding Cyril Light, high-contrast, Catppuccin, or Gruvbox palette values;
  tracked by `cyril-fkke`.
- Startup configuration, automatic terminal-capability detection, a `/theme`
  picker, live preview, or runtime theme switching; tracked by `cyril-qaq0`.
- Arbitrary operator-defined palettes, which ADR-0005 rejects for this theme
  model.
- Changing syntax highlighting output or diff rendering; syntax presentation is
  tracked by `cyril-ghuu`, and diff presentation by the applicable widget
  migration tickets.
- Removing legacy palette access after migration; tracked by `cyril-6r3a`.

<!-- markdownlint-disable MD013 -->

## Constraints

| Dimension | Limit | How measured |
| --- | --- | --- |
| Semantic completeness | Exactly 19 UI roles plus 1 syntax component | Exhaustive contract test |
| Source colors | 18 RGB values plus 1 reset canvas | Resolved-theme inspection |
| Rendering change | 0 changed symbols and 0 changed styles in 3 × 80×24 scenes | Paired buffer comparison |
| ANSI-256 search range | Exactly indices 16–255 | Projection oracle |
| ANSI-16 search range | Exactly 16 canonical entries | Projection oracle |
| Public configuration growth | 0 new fields | Configuration schema and diff inspection |
| Production widget migration | 0 widgets | Production diff inspection |

## Decisions log

| # | Question | Decision | Why |
| ---: | --- | --- | --- |
| 1 | Who directly uses this seam? | A Cyril UI contributor; a Cyril terminal operator is the compatibility audience. | The expand step is contributor-facing but must remain invisible to operators. |
| 2 | What does Cyril Dark equivalence mean? | Zero changed rendered cells or styles for this ticket. | No widget migration occurs in the expand step. |
| 3 | Where is visual leeway allowed? | Only in later migration tickets with explicit snapshot review. | An unused seam has no reason to change output. |
| 4 | Are message identity and status severity the same roles? | No; user, agent, and system identity are separate from info, success, warning, and danger. | Identity is not status. |
| 5 | Are syntax-token colors UI roles? | No; syntax highlighting is a separate theme component. | Token palettes are dynamic and language-specific. |
| 6 | Are diff meanings UI roles? | Yes; addition, deletion, and context are separate roles. | Diff state has meaning independent of syntax color. |
| 7 | What does no-color do? | Reset all UI and syntax colors. | Color-disabled output must contain no hidden theme colors. |
| 8 | How is ANSI correctness measured? | Minimum squared RGB distance with lower-index tie-breaking. | “Looks close” is not falsifiable. |
| 9 | What is the initial semantic role set? | The 19 roles listed in this artifact. | The set covers backgrounds, text, structure, emphasis, identity, status, and diff meaning. |
| 10 | Does this ticket expose configuration? | No. | Configuration must activate atomically with selection. |
| 11 | Which visual themes does this ticket implement? | Cyril Dark only. | Additional palettes and selection have separate tickets. |
| 12 | Does Cyril Dark force a canvas background? | No; canvas remains terminal-default. | Preserves current rendering. |
| 13 | What are Cyril Dark’s exact source values? | The compatibility mapping in this artifact. | The seam is not a palette redesign. |
| 14 | How should automatic mode treat `TERM=dumb`? | As no-color, in downstream ticket `cyril-qaq0`. | Minimal terminals should not receive color escapes automatically. |
| 15 | May ANSI-256 projection choose indices 0–15? | No. | Those entries are terminal-customizable. |
| 16 | Does this ticket perform automatic environment detection? | No. | That is activation/configuration behavior. |
| 17 | Which frames measure visual compatibility? | Default idle, active conversation with tool diff, and open picker at 80×24. | They sample the main frame, rich content, and a modal surface. |
| 18 | Which source color forms are valid? | Explicit RGB plus reset only. | Named ANSI source colors are terminal-dependent. |
| 19 | What happens when the syntax component is missing? | Contract validation fails. | Silent fallback would certify an incomplete theme. |

## Sign-off

The requester typed, verbatim: "Yes, it defines cyril dark and it's 4 projections. Migrating widgets, configuration, pickers, and other themes are out of scope"

Date: 2026-07-10

<!-- markdownlint-enable MD013 -->
