# Feature: Activate bundled themes and terminal color modes

Issue: **cyril-qaq0** (Rivets). Route: **Structural** (`.cyril-qaq0/route.md`).

## Scope reconciliation — 2026-09-09

Two requester decisions supersede the 2026-07-11 sign-off and the persistence
behavior that sign-off approved.

| Question | Decision | Rationale | Effect on this spec |
|---|---|---|---|
| Does Enter offer to persist the choice? | **No. Enter accepts for this session only; Cyril never writes configuration.** | Requester decision, 2026-09-09. | The persistence dialog, write-fidelity criteria, persistence failure paths, and every persistence edge row are removed. Configuration is read-only at startup. |
| Implement the palette prerequisite first, or activate the one-theme catalog? | **Implement cyril-fkke first.** | Requester decision, 2026-09-09. | This spec activates the completed six-palette catalog (cyril-fkke, PR #120). Palette definitions are not part of this change. |

Repository reconciliation (2026-09-09). The July text named a loader and a theme
seam that no longer exist; these facts replace it.

- Startup configuration is loaded by `cyril_memory::load_config_report`
  (`crates/cyril-memory/src/config.rs`), which returns the ordinary `Config`
  plus an independent memory-section outcome. `Config::load_from_path` is
  historical and must not be reintroduced.
- `App::new` receives `&config::UiConfig` and destructures it **exhaustively**
  (`crates/cyril/src/app.rs`); a new `UiConfig` field must be consumed there to
  compile. That seam is the cyril-nd4h guarantee this change relies on.
- `UiState::new` hardcodes `resolve(ThemeId::CyrilDark, ColorMode::TrueColor)`;
  no theme setter exists today.
- `ThemeId` has six variants; `ThemeId::ALL` and `ThemeId::name()` exist
  (cyril-fkke). `ColorMode` has `TrueColor`, `Ansi256`, `Ansi16`, `None`.
- The `[ui]` section currently holds `max_messages` and `mouse_capture`.

## What this is

Cyril reads a visual theme and a terminal color mode from `config.toml`,
resolves them once into the read-only theme `UiState` already exposes, and adds
a session-local `/theme` picker with immediate preview and Esc-restore. Today
both axes exist in code — six palettes, four projections — but production
hardcodes Cyril Dark truecolor; this ticket activates selection. It adds no
palettes and writes no configuration.

## Users

- **Cyril terminal operator**: sets `[ui] theme` and `[ui] color_mode` in
  `config.toml`, previews and switches themes live with `/theme`, and can rely
  on Cyril never editing their config file.
- **Cyril UI contributor**: consumes the two-axis seam (theme × mode) and the
  detection precedence table; never reads environment variables or config from
  widgets.

## Behavior

### Startup selection

- **Given** a `config.toml` with `[ui] theme = "<id>"` and
  `[ui] color_mode = "<mode>"` (either key absent or present)
- **When** cyril starts
- **Then** `UiState` owns `resolve(theme, resolved_mode)` and rendered output
  matches that resolved theme.

Accepted `theme` values, exactly: `cyril-dark`, `cyril-light`,
`high-contrast-dark`, `high-contrast-light`, `catppuccin-mocha`,
`gruvbox-dark`.

Accepted `color_mode` values, exactly: `automatic`, `truecolor`, `ansi256`,
`ansi16`, `none`.

Absent keys take their defaults: `theme = "cyril-dark"`, `color_mode =
"automatic"`. With no `config.toml` at all, rendered output is byte-identical to
today's shipped Cyril Dark truecolor.

### Automatic mode detection (first match wins)

- **Given** `color_mode = "automatic"` or absent
- **When** cyril starts
- **Then** the mode resolves by this precedence:

| # | Condition | Result |
|---|---|---|
| 1 | explicit `color_mode` in config | that mode; detection is skipped entirely (overrides `NO_COLOR`) |
| 2 | `NO_COLOR` set to a non-empty value | `none` |
| 3 | `COLORTERM` is `truecolor` or `24bit` | `truecolor` |
| 4 | host is Windows | `truecolor` |
| 5 | `TERM` contains `256color` | `ansi256` |
| 6 | `TERM` is exactly `dumb` | `none` |
| 7 | none of the above | `truecolor` (today's shipped behavior) |

Detection reads environment values only at the process boundary; the precedence
itself is a pure function of injected values.

### `/theme` preview

- **Given** the `/theme` picker is open
- **When** the highlight moves to theme T
- **Then** the next rendered frame uses T projected into the current color mode,
  through the read-only `UiState` theme. The committed theme is unchanged until
  Enter.

The picker lists every bundled theme in `ThemeId::ALL` order (Cyril Dark first)
with a human-readable label.

### Enter accepts for the session

- **Given** the picker with theme T highlighted
- **When** Enter
- **Then** the session theme becomes T, the picker closes, and **no
  configuration file is created, opened for writing, or modified**. No dialog
  is shown.

### Esc restore

- **Given** the picker open with any preview active
- **When** Esc
- **Then** the session theme reverts to the theme active when the picker
  opened, and the picker closes. No write occurs.

### Invalid value in a new key

- **Given** a parseable `config.toml` with an unrecognized `[ui] theme` or
  `[ui] color_mode` value
- **When** cyril starts
- **Then** that key alone falls back to its default, every other config key is
  honored, and one system message appears in chat naming the offending value
  and the default, e.g.
  `unknown theme "cyrl-light" in config.toml — using cyril-dark`.

### Syntactically invalid config (unchanged, pinned)

- **Given** an unreadable or unparseable `config.toml`
- **When** cyril starts
- **Then** existing pinned behavior holds: whole-file defaults plus a log-only
  warning, as implemented by the current loader. This ticket does not change it.

## Success criteria

- **Render coverage**: every bundled theme × 4 color modes renders through
  resolved roles, measured by extending the existing mode-matrix render fence
  from 1 theme to all 6.
  *This method cannot see perceptual distinctness or real-terminal palette
  rendering — it certifies projection consistency even where ANSI-16 collapses
  speaker identity or contrast is unreadable (cyril-q9dx closed; cyril-leiq
  closed).*
- **Detection precedence**: 8/8 rows (7 rules + default) covered by unit tests
  with injected environment, measured by test names mapping 1:1 to rows.
  *This method cannot see terminals that misreport `COLORTERM`/`TERM`, or tmux
  RGB-passthrough variance — named accepted risk.*
- **No configuration writes**: after a simulated picker session (open → move →
  Esc; open → move → Enter), the config file is byte-identical to its pre-run
  bytes, measured by a before/after byte comparison over
  `.cyril-qaq0/config-nowrite-fixture.toml` (comments, an inline table, and
  multiple tables). When no config file exists, none is created.
- **Preview latency**: "immediately" means the next rendered frame after the
  selection-change event, measured by a `TestBackend` render assertion.
- **Invalid-value visibility**: exactly 1 system message rendered for an
  unknown-theme fixture and 1 for an unknown-mode fixture, measured by buffer
  inspection.
- **Esc restore**: theme-before-picker equals theme-after-Esc, measured by a
  state test.
- **Startup default**: with no config file, the resolved theme equals
  `resolve(CyrilDark, TrueColor)`, measured by a state test.
- **Quality gate**: `cargo fmt --check`, `cargo clippy -- -D warnings`, and the
  workspace test suite all exit 0.

## Edge cases and decisions

| Edge | Decision | Source |
|---|---|---|
| Config file absent at startup | silent defaults (`cyril-dark`, `automatic`); no file is created | existing loader + no-writes decision |
| Config unreadable/unparseable | whole-file defaults, log-only warning | current pinned loader behavior |
| Unknown `theme` / `color_mode` value | that key's default + one visible chat message | decision 6, this session |
| `NO_COLOR` set AND explicit `color_mode` | explicit wins | precedence row 1 |
| `NO_COLOR` set to empty string | treated as unset | precedence row 2 (non-empty rule) |
| `NO_COLOR` set AND `color_mode = "automatic"` | `none` | precedence row 2 |
| Picker with only the default theme installed | not reachable: cyril-fkke ships six | fkke landed |
| Esc after a preview, before Enter | restores theme at picker-open | Esc behavior |
| Esc pressed when no picker is open | unchanged existing behavior | not this ticket |
| Theme swap while the agent is streaming | one theme per frame via read-only `UiState`; theme-keyed caches prevent stale colors | ADR 0005 + cyril-ghuu spec; structural cache-identity gap tracked at cyril-x5xi |
| `theme`/`color_mode` keys present but not strings (e.g. `theme = 3`) | whole-file defaults via the pinned serde posture; no per-key message | decision 6 scope limit |
| `color_mode` changed while running | not offered: mode is config-only, no `/mode` command | decision 5 |

## Out of scope

This change does NOT include: additional palettes (cyril-fkke); a live `/mode`
command (mode is config-only); arbitrary user-defined palettes (ADR 0005);
**any configuration write, persistence dialog, or config-mutation helper**;
wiring or removing the dead `[ui]` knobs (cyril-nd4h); modal/chrome theming
(cyril-nrnq, cyril-dij8); fixing ANSI-16 projection quality (cyril-q9dx) or
Cyril Dark contrast (cyril-leiq); making theme-keyed cache identities
structurally complete (cyril-x5xi).

## Constraints

| Dimension | Limit | How measured |
|---|---|---|
| New config keys | exactly `theme`, `color_mode`, both under `[ui]` | schema-fence test; `UiConfig` exhaustive destructure in `App::new` forces consumption |
| Configuration writes | zero, ever, from this feature | before/after byte comparison + source fence forbidding write APIs on the config path |
| State/renderer boundary | renderer receives `&dyn TuiState` only; no env or config reads in widgets | existing source fences (ADR 0005) |
| Environment reads | only at the process boundary, in the binary | source fence: no `std::env` in `cyril-ui`/`cyril-core` production |
| Default with no config | `cyril-dark` truecolor, byte-identical to today | mode-matrix fence baseline |

## Decisions

| # | Decision | Source | Why |
|---|---|---|---|
| 1 | cyril-q9dx no longer blocks; all declared blockers except cyril-fkke are closed | rivets, 2026-09-09 | activation can proceed on the fixed projection |
| 2 | Detection precedence table (7 rules, default `truecolor`) | gap Q2, July | each rule observable/testable; inconclusive default preserves shipped behavior |
| 3 | Enter applies for the session only; no persistence dialog, no write | requester, 2026-09-09 | supersedes the July dialog decision |
| 4 | No config-mutation code path exists at all | requester, 2026-09-09 | a path that cannot run cannot corrupt a hand-edited file |
| 5 | Picker lists themes only; mode is config-only | gap Q5, July | ADR 0005 axis separation |
| 6 | Per-key fallback + visible chat message for unknown *string* values | gap Q6, July | a one-char typo must not silently reset unrelated config; log-only warnings are invisible in a TUI |
| 7 | No blocker vs cyril-nd4h; the second lander updates the schema fence | gap Q7, July | one-line test edit does not justify serializing independent work |
| 8 | Config values are kebab-case ids, independent of Rust variant names | `.cyril-qaq0/config-nowrite-fixture.toml` (the July prototype already probed `theme = "cyril-dark"` / `"gruvbox-dark"`) | the config file is a public contract; a Rust rename must not break it. `ThemeId::name()` is PascalCase and stays a Rust-facing identity, not the operator spelling |
| — | Two-axis model, resolve-once, read-only `UiState` theme | docs/adr/0005 | accepted 2026-07-10 |
| — | Theme-keyed color-bearing caches | .cyril-ghuu/spec.md | prevents stale preview colors |

## Sign-off

The 2026-07-11 sign-off is **superseded** where it conflicts with this spec: it
approved persistence via a confirmation dialog, which decision 3 removes.

Current authority: the requester's 2026-09-09 decisions recorded at the top of
this file. Design approval follows `falsifiable-design`.
