# Design: Semantic theme seam for Cyril Dark

Status: approved (2026-07-10)

## Purpose

Add a complete, pure semantic-theme contract for Cyril Dark without changing
widget rendering or public configuration. The design is grounded by the signed
specification and by the projection probe whose independent oracle agrees on
18/18 role values, 18/18 ANSI-256 projections, and 18/18 ANSI-16 projections.

## Probe ground truth

The agreed projection output is `.cyril-ixua/probe-output.tsv`. The critical
finding is that the unverified partial cube projector disagrees with the oracle
on 8/18 ANSI-256 roles because it does not search the xterm grayscale ramp. The
design therefore uses exhaustive nearest-entry selection, not cube rounding or
color-family heuristics.

<!-- markdownlint-disable MD013 -->

## Input shapes

| Input | Production-reachable shapes | Coverage |
| --- | --- | --- |
| Theme identifier | `CyrilDark` | Claims 1, 3, 7, 8 |
| Explicit color mode | True-color, ANSI-256, ANSI-16, no-color | Claims 3–7 |
| Source color | RGB at 0 boundary, RGB at 255 boundary, mid-range RGB, duplicate RGB values across roles, reset canvas | Claims 1–7 |
| Distance result | Unique nearest entry, exact palette match, equal-distance tie | Claims 4–6 |
| Semantic role collection | Fixed 19-role structure; 18 RGB roles and one reset role | Claims 1, 2, 7 |
| Syntax component | Present and valid; missing name is validation-only failure input | Claims 7, 8 |
| Render comparison | Default idle, active conversation with tool diff, open picker; each at 80×24 | Claim 9 |
| Public configuration | Existing four-field UI configuration with no theme fields | Claim 10 |

An empty or variable-length role collection is not production-reachable because
the source and resolved themes use a fixed-field type. Named ANSI source colors
are not production-reachable because the source type represents only RGB and
reset. Additional theme identifiers are owned by `cyril-fkke`; automatic mode
and environment strings are owned by `cyril-qaq0`.

## Change classification

This is purely additive relative to the committed code: it adds an unused UI
module and removes no guard, lock, ordering rule, uniqueness property, or
validation rule. The dirty-worktree theme/config draft is not an existing
invariant; implementation must reconcile it to this design before acceptance.
No removed-invariant sweep is required.

## Architecture

### Ownership boundary

The semantic-theme contract belongs to `cyril-ui`. Core protocol and
configuration types do not depend on it, and this ticket adds no public
configuration fields. This prevents visual policy from leaking into
`cyril-core` before configuration activation in `cyril-qaq0`.

### Source representation

A private fixed-field source theme contains all 19 semantic roles. Each role is
a private `SourceColor` with exactly two variants: explicit RGB and reset. This
makes terminal-defined named colors unrepresentable at the source boundary and
makes a missing semantic role a compile-time construction failure.

The private source theme carries a typed syntax identifier for
`base16-eighties.dark`; it does not carry an arbitrary string. Contract
validation resolves that identifier against Syntect's loaded default theme set.

### Resolved representation

The public read-only `Theme` contains the same 19 fixed roles as Ratatui
`Color` values and an optional typed syntax component. Resolution accepts the
single `ThemeId::CyrilDark` and one of four explicit `ColorMode` variants. It is
pure: it reads no process environment and mutates no cache or application state.

True-color maps RGB to `Color::Rgb` and preserves reset. No-color maps every
role to `Color::Reset` and removes the syntax component.

### ANSI-256 projection

Projection enumerates xterm indices 16–255. Indices 16–231 use levels
`[0, 95, 135, 175, 215, 255]`; indices 232–255 use grayscale levels 8 through
238 in steps of 10. It minimizes squared RGB distance using a key of
`(distance, index)`, which makes lower-index tie-breaking explicit. The output
is `Color::Indexed(index)`.

### ANSI-16 projection

Projection searches the 16 canonical RGB entries pinned in the spec with the
same `(distance, index)` key. The selected index maps to Ratatui's named base or
bright color variant so terminals receive ANSI-16 SGR output rather than an
ANSI-256 indexed escape.

### Dirty-worktree reconciliation

The existing unverified draft is replaced rather than extended: theme and color
identifiers leave public core configuration, automatic environment detection is
removed, the five non-Cyril-Dark palettes move to `cyril-fkke`, the 10-role
shape expands to the signed 19-role contract, and both heuristic projectors are
replaced by nearest-entry search. The module is exported by `cyril-ui` but no
widget consumes it.

## Claims

1. Cyril Dark resolves exactly the 19 pinned semantic role values and the one pinned syntax identifier.
2. Every source role is either explicit RGB or reset, and canvas is the only reset source role.
3. True-color resolution preserves all 18 RGB values exactly and preserves the reset canvas.
4. ANSI-256 resolution chooses the minimum-distance fixed xterm entry in indices 16–255 for every RGB role.
5. ANSI-16 resolution chooses the minimum-distance canonical entry for every RGB role and emits the corresponding Ratatui named color.
6. Equal-distance candidates resolve to the lower palette index in both ANSI modes.
7. No-color resolution resets all 19 roles and removes the syntax component.
8. The Cyril Dark syntax identifier exists in Syntect's loaded default theme set.
9. Adding and exporting the unused seam changes zero symbols and zero styles in the three pinned render buffers.
10. The public UI configuration schema remains the same four fields and no production widget consumes the new seam.

## Falsification

| # | Claim | Falsifier | Independent oracle | Cost | Status | Regression fence |
| ---: | --- | --- | --- | ---: | --- | --- |
| 4 | ANSI-256 is nearest across indices 16–255. | Run the Rust projection probe for all 18 RGB roles; any index differing from brute force falsifies the claim, and the known cube-only implementation fails muted `#8c8c8c`. | Python independently parses the signed mapping and generates cube plus grayscale entries. | 5s | passed: 18/18 | Unit test `theme::tests::ansi256_uses_nearest_fixed_xterm_entry` with muted `#8c8c8c → 245` and all pinned roles. |
| 5 | ANSI-16 is nearest in the canonical table and emits matching named colors. | Run all 18 RGB roles; any wrong index or index-to-Ratatui mapping falsifies the claim, and the current threshold-based color-family heuristic is the known bad implementation. | Python brute-forces the canonical table; a separate expected index-to-variant table checks Ratatui output. | 5s | passed: 18/18 indexes; variant mapping pending | Unit test `theme::tests::ansi16_uses_nearest_canonical_entry`. |
| 6 | Ties choose the lower index. | Project an RGB midpoint equidistant from two entries in each palette; choosing the higher entry falsifies the claim. | Python enumerates all equal minima and reports the minimum index. | 10s | pending | Unit test `theme::tests::ties_choose_lower_palette_index`; reversing candidate order must make a buggy last-wins implementation fail. |
| 3 | True-color is identity and reset stays reset. | Resolve all source roles in true-color; any changed RGB or black canvas falsifies the claim. | The signed hex table and terminal-default canvas in the committed renderer. | 15s | pending | Unit test `theme::tests::truecolor_preserves_source_values_and_reset`. |
| 7 | No-color resets every role and removes syntax. | Resolve no-color and count explicit colors; any count above 0 or retained syntax identifier falsifies the claim. | A serialized list independently checked for 19 resets plus `None`. | 20s | pending | Unit test `theme::tests::no_color_resets_roles_and_disables_syntax`; an implementation that only resets accents must fail. |
| 1 | The resolved contract matches all pinned roles and syntax. | Serialize the resolved theme; any missing, extra, or unequal role/name falsifies the claim. | Parser over the signed compatibility table plus literal syntax identifier. | 30s | pending | Unit test `theme::tests::cyril_dark_matches_signed_contract`; omitting diff context must fail distinctly. |
| 2 | Source values are RGB/reset only and only canvas resets. | Inspect source construction; any named ANSI color or second reset role falsifies the claim. | AST search for Ratatui named colors in source definitions plus resolved-value count. | 30s | pending | Unit test `theme::tests::source_has_eighteen_rgb_roles_and_one_reset`; replacing RGB cyan with `Color::Cyan` must be unrepresentable. |
| 8 | The syntax identifier exists. | Look up the typed identifier in loaded Syntect defaults; `None` falsifies the claim. | Syntect's packaged default-theme catalog, independently searchable for the exact identifier. | 30s | pending | Unit test `theme::tests::cyril_dark_syntax_theme_exists`; a one-character typo must fail. |
| 10 | Configuration and widget consumption remain unchanged. | Compare serialized default configuration keys and production references against committed `HEAD`; a new key or widget reference falsifies the claim. | Committed configuration serialization and AST reference search. | 1m | pending | Tests `config::tests::default_ui_config_schema` and `theme::tests::seam_has_no_widget_references`; the current dirty draft must fail both before reconciliation. |
| 9 | Three rendered states are byte-for-byte style equivalent. | Render each state from committed `HEAD` and the ticket revision; any symbol/style diff falsifies the claim. | Paired Ratatui buffers produced from an isolated clean worktree at `HEAD`. | 3m | pending | Snapshot tests `render_equivalence::{idle,tool_diff,picker}`; wiring the theme into default rendering must fail at least one snapshot. |

### Cheapest falsifier result

The projection probe and independent oracle were rerun after this design was
written. The oracle produced distinct passing outputs for role values,
ANSI-256, and ANSI-16; the exact command and output are recorded in
`.cyril-ixua/design-falsifier-output.txt`.

<!-- markdownlint-enable MD013 -->

## Negative space

- Widget migration is not included here: conversation, modal, and chrome
  migrations are owned by `cyril-ghuu`, `cyril-nrnq`, and `cyril-dij8`.
- The five additional bundled palettes are owned by `cyril-fkke`.
- Configuration, automatic capability detection, and live selection are owned
  by `cyril-qaq0`.
- Removing legacy palette access after migrations is owned by `cyril-6r3a`.
- Arbitrary operator-defined palettes are rejected by ADR-0005 rather than
  treated as untracked future work.

## Self-review

- **Claim count**: 10; the scope remains one seam and four explicit outputs.
- **Input coverage**: every production-reachable theme, mode, source-color,
  distance, syntax, rendering, and configuration shape maps to a claim.
- **Removed invariants**: none; the committed-code change is additive.
- **Falsifier independence**: source/spec parsing, brute-force Python, Syntect's
  catalog, committed buffers, and AST/config inspection are outside the theme
  implementation.
- **Non-vacuity**: every row names a concrete bad implementation that its fence
  rejects, including cube rounding, threshold color families, higher-index ties,
  retained syntax colors, missing diff context, named source colors, syntax
  typos, dirty config fields, and accidental widget wiring.
- **Distinctness**: each claim has a named output or test; ANSI-256 and ANSI-16
  report separately.
- **Cost distribution**: every claim has a falsifier costing three minutes or
  less.
- **Tracker references**: all six cited IDs were verified in Rivets while this
  design was written.

## Approval

The requester approved this design on 2026-07-10.
