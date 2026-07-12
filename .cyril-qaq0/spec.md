# Feature: Activate bundled themes and terminal color modes

## What this is

Cyril will read a visual theme and a terminal color mode from startup
configuration, resolve them once into the read-only theme UiState already
exposes, and add a session-local `/theme` picker with immediate preview,
accept-with-optional-persist, and Esc-restore. Today both axes exist in code
(29-role contract, four projections) but production hardcodes Cyril Dark
truecolor; this ticket activates selection. It does not add palettes.

## Users

- **Cyril terminal operator**: sets `theme` / `color_mode` in `config.toml`,
  previews and switches themes live with `/theme`, and keeps hand-edited
  config comments intact when persisting a choice.
- **Cyril UI contributor**: consumes the two-axis seam (theme × mode) and the
  detection precedence table; never reads environment variables from widgets.

## Behavior

### Startup selection
- **Given**: `config.toml` with `theme = "<installed id>"` and
  `color_mode = "<truecolor|ansi256|ansi16|none|automatic>"`
- **When**: cyril starts
- **Then**: `UiState` owns `resolve(theme, resolved_mode)`; rendered output
  matches that resolved theme (mode-matrix fence method)

### Automatic mode detection (first match wins)
- **Given**: `color_mode = "automatic"` or absent
- **When**: cyril starts
- **Then**: mode resolves by precedence:
  1. explicit `color_mode` in config → use it, skip detection (overrides NO_COLOR)
  2. `NO_COLOR` set (non-empty) → `none`
  3. `COLORTERM` ∈ {`truecolor`, `24bit`} → `truecolor`
  4. Windows host → `truecolor`
  5. `TERM` contains `256color` → `ansi256`
  6. `TERM` = `dumb` → `none`
  7. otherwise → `truecolor` (today's shipped behavior)

### /theme preview
- **Given**: the `/theme` picker is open
- **When**: the highlight moves to theme T
- **Then**: the next rendered frame uses T projected into the current color
  mode, via the read-only UiState theme (ADR 0005 boundary preserved)

### Accept with optional persistence
- **Given**: the picker with theme T highlighted
- **When**: Enter
- **Then**: the session theme becomes T and a confirmation dialog offers
  "save as default"; **confirm** → the `theme` key in `config.toml` is
  updated (see write fidelity); **decline** → session keeps T, file untouched

### Esc restore
- **Given**: the picker open, any preview active (before or after Enter's dialog)
- **When**: Esc
- **Then**: the session theme reverts to the theme active when the picker
  opened; no dialog, no write

### Invalid value in the new keys
- **Given**: a parseable `config.toml` with `theme = "<unknown>"` (or unknown
  `color_mode`)
- **When**: cyril starts
- **Then**: that key falls back to its default (`cyril-dark` / `automatic`),
  every other config key is honored, and a system message appears in chat:
  `unknown theme "<value>" in config.toml — using cyril-dark`

### Syntactically invalid config (unchanged, cited)
- **Given**: an unreadable or unparseable `config.toml`
- **When**: cyril starts
- **Then**: existing pinned behavior — `tracing::warn!` + whole-file defaults
  (`crates/cyril-core/src/types/config.rs:69-83`); not modified by this ticket

## Success criteria

- **Mode/theme render coverage**: every installed theme × 4 modes renders
  through resolved roles, measured by extending the existing mode-matrix
  fence (render.rs) from 1 theme to N.
  *This method cannot see: perceptual distinctness or real-terminal palette
  rendering — it certifies projection consistency even where ANSI-16
  collapses speaker identity (cyril-q9dx, now blocking) or contrast is
  unreadable (cyril-leiq, related).*
- **Detection precedence**: 8/8 precedence rows (7 rules + default) covered
  by unit tests with injected environment, measured by test names mapping
  1:1 to rows.
  *This method cannot see: terminals that misreport COLORTERM/TERM, or tmux
  RGB-passthrough variance — named accepted risk.*
- **Persist fidelity**: persisting from the dialog changes exactly one line
  (`theme = …`) of a commented fixture config — 0 other diff lines,
  measured by round-trip diff test.
  *This method cannot see: TOML constructs the fixture lacks — fixture must
  include comments, table headers, and at least one inline table; beyond
  that, named accepted risk.*
- **Preview latency**: "immediately" = the next rendered frame after the
  selection-change event (≤ one 50ms fast tick), measured by TestBackend
  render assertion.
- **Invalid-value visibility**: 1 system message rendered in the startup
  buffer for an unknown theme/mode fixture, measured by buffer inspection.
- **Esc restore**: theme-before-picker == theme-after-Esc, measured by state
  test.
- **Quality gate**: `cargo fmt --check`, `cargo clippy -- -D warnings`, and
  workspace tests all exit 0.

## Edge cases and decisions

| Edge | Decision | Source |
|---|---|---|
| Config file absent at startup | silent defaults (cyril-dark, automatic) | config.rs:72, existing |
| Config unreadable/unparseable | warn + whole-file defaults, log-only | config.rs:69-83, existing pinned |
| Unknown `theme` / `color_mode` value | per-key default + visible chat message | decision 6, this session |
| `NO_COLOR` set AND explicit `color_mode` | explicit wins | AC 2 + decision 2 |
| `NO_COLOR` set to empty string | treated as unset | decision 2 (non-empty rule) |
| Config file absent when persist confirmed | create minimal file containing the theme key | recommended, confirm at sign-off |
| Config write fails (permissions/disk) | session theme stays applied; error system message in chat; no crash | recommended, confirm at sign-off |
| Picker with a single installed theme (fkke unlanded) | picker functions with one entry | derived, decision 5 |
| Esc after Enter→decline in same picker session | restores to theme at picker-open (not the declined accept) | Esc behavior, decision 3 |
| Theme swap while agent is streaming | one theme per frame via read-only UiState; theme-keyed caches prevent stale colors | ADR 0005 + ghuu spec edge row (structural gap tracked at cyril-x5xi) |
| ANSI-16 activation quality | blocked until identity collapse is fixed | cyril-q9dx dependency, decision 1 |

## Out of scope

This change does NOT include: additional palettes (cyril-fkke); a live
`/mode` command (mode is config-only); arbitrary user-defined palettes
(ADR 0005); persisting `color_mode` from the picker; wiring or removing the
dead `[ui]` knobs (cyril-nd4h); modal/chrome theming (cyril-nrnq,
cyril-dij8); fixing ANSI-16 projection quality (cyril-q9dx) or Cyril Dark
contrast (cyril-leiq).

## Constraints

| Dimension | Limit | How measured |
|---|---|---|
| New config keys | exactly `theme`, `color_mode` | schema-fence test, updated by whichever of qaq0/nd4h lands second (decision 7) |
| State/renderer boundary | renderer receives `&dyn TuiState` only; no env reads in widgets | source fence + ADR 0005 |
| Persist write | only the `theme` key changes; comments/formatting preserved | round-trip diff test |
| Default with no config | cyril-dark truecolor, byte-identical to today | mode-matrix fence baseline |

## Decisions

| # | Decision | Source | Why |
|---|---|---|---|
| 1 | cyril-q9dx blocks activation; cyril-leiq related-not-blocking | gap Q1 (rivets dep applied) | activating a mode whose identity distinction is erased defeats the activation |
| 2 | Detection precedence table (7 rules, default truecolor) | gap Q2 | each rule observable/testable; inconclusive default preserves shipped behavior |
| 3 | Enter = session apply + confirm dialog for persistence | gap Q3 | requester intent: theme persists; dialog guards the file write |
| 4 | Persist rewrites only the `theme` key, preserving comments/formatting | gap Q4 | operators hand-edit config; full re-serialize would also materialize nd4h's dead knobs |
| 5 | Picker lists themes only; mode is config-only | gap Q5 | ADR 0005 axis separation; persist decision covers only the theme key |
| 6 | Per-key fallback + visible chat message for unknown values | gap Q6 | a one-char typo must not silently reset unrelated config; log-only warnings are invisible in a TUI |
| 7 | No blocker vs cyril-nd4h; second-lander updates the schema fence | gap Q7 | one-line test edit does not justify serializing independent work |
| — | Two-axis model, resolve-once, read-only UiState theme | docs/adr/0005 | accepted 2026-07-10 |
| — | Theme-keyed color-bearing caches | .cyril-ghuu/spec.md edge row | prevents stale preview colors |

## Sign-off

Consequences stated to the requester: see conversation, 2026-07-11
(interrogated-spec audit-mode test run).

The requester replied, verbatim: "Confirmed - themes persist via dialog, tmux shift and edge cases accepted"

Date: 2026-07-11
