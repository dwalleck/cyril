# Design: Activate bundled themes and terminal color modes

Issue: **cyril-qaq0**. Route: **Structural** (`.cyril-qaq0/route.md`).
Date: 2026-09-09.

## Route and inputs

- **Route**: Structural (`.cyril-qaq0/route.md` T2 = yes). Downstream:
  `falsifiable-design` → `budgeted-plan` → `checkpointed-build`.
- **Behavior set**: `.cyril-qaq0/spec.md` — the reconciled contract. Its
  behaviors are startup selection, automatic detection precedence, `/theme`
  preview, Enter-accepts-for-the-session, Esc-restore, invalid-value fallback,
  and the pinned unparseable-config posture.
- **Empirical premises**: `N/A — no external premise`. Every premise is
  repository-internal: the six-palette catalog and `resolve` exist (cyril-fkke);
  the picker, command registry, and config seam exist (recon recorded below).
  Automatic detection is a policy over injected values, not a claim about any
  terminal.
- **`evidence.md` / `probe.*`**: `N/A — no Empirical route`. The one probe this
  design runs is the cheapest-falsifier prototype recorded in the run log, and
  it is evidence for this design, not a premise about the outside world.
- **Dependency**: cyril-fkke supplies the six palettes and `ThemeId::ALL`. All
  other declared blockers of cyril-qaq0 are closed (rivets, 2026-09-09).

### Repository facts this design rests on (verified 2026-09-09)

| Fact | Evidence |
|---|---|
| `UiConfig` has exactly `max_messages`, `mouse_capture`; unknown keys are ignored, wrong types are whole-file rejected | `crates/cyril-core/src/types/config.rs:16-34`, `crates/cyril-core/tests/nd4h_legacy_config_compat.rs` |
| A 2-field schema fence pins the serialized `[ui]` key set | `crates/cyril-core/src/types/config.rs:144` |
| `App::new` destructures `&config::UiConfig` exhaustively (no `..`) and is the only config consumption seam | `crates/cyril/src/app.rs:380-385`; fenced by `crates/cyril/tests/nd4h_source_fences.rs` C6 |
| `UiState::new` hardcodes `resolve(CyrilDark, TrueColor)`; no theme setter; production caller is only `app.rs:406` | `crates/cyril-ui/src/state.rs:333` |
| `PickerState { title, options, filter, filtered_indices, selected }`; selection is positional; no `PickerAction` | `crates/cyril-ui/src/traits.rs:480-488` |
| `CommandResultKind::ShowPicker` exists but nothing constructs it; App already handles it | `crates/cyril-core/src/commands/mod.rs:177-180,221-225`; `crates/cyril/src/app.rs:1949-1951` |
| Picker Enter sends `BridgeCommand::ExecuteCommand` using the picker title as the command name | `crates/cyril/src/app.rs:1776-1798`; `crates/cyril-ui/src/state.rs:2073-2079` |
| No `NO_COLOR`/`COLORTERM`/`TERM` handling exists in production anywhere | workspace grep, 2026-09-09 |
| `cyril-ui` contains no `std::env` reads | workspace grep, 2026-09-09 |
| The mode matrix renders the marker theme against `resolve(CyrilDark, mode)` only | `crates/cyril-ui/src/render.rs:1305-1320` |

## Input shapes

| Input | Shapes | Status |
|---|---|---|
| `[ui] theme` | absent; each of 6 accepted ids; unknown string; empty string; wrong TOML type | Covered by C1, C7, C8, C11 |
| `[ui] color_mode` | absent; `automatic`; `truecolor`; `ansi256`; `ansi16`; `none`; unknown string; empty string; wrong TOML type | Covered by C2, C3, C8, C11 |
| `NO_COLOR` | unset; empty; non-empty | Covered by C3 |
| `COLORTERM` | unset; `truecolor`; `24bit`; other | Covered by C3 |
| `TERM` | unset; contains `256color`; `dumb`; other | Covered by C3 |
| host platform | Windows; non-Windows | Covered by C3 |
| picker lifecycle | closed; opened; selection first/middle/last; filter typed; Enter; Esc; Enter with no session id | Covered by C4, C5, C9 |
| config file | absent; present; unreadable; unparseable TOML | C7 (absent), C8 (present), `N/A — pinned existing loader behavior, unchanged by this ticket` |

## Placement

| Capability | Owner | New seam | Forbidden |
|---|---|---|---|
| Bundled catalog, id spelling, mode spelling, detection precedence | `crates/cyril-ui/src/theme.rs` | None — extends the existing `resolve`/`ThemeId` seam with `parse_theme_id`, `parse_color_mode`, `detect_color_mode` | No env read, no config read, no rendering |
| Theme picker state and preview | `crates/cyril-ui/src/state.rs` + `PickerState` | None — `PickerState` gains a `kind` discriminator; the preview is `UiState` state | No fs access; no bridge command; no config knowledge |
| `/theme` command | `crates/cyril-core/src/commands/` (builtin) | `CommandResultKind::ShowThemePicker` (semantic, carries no data) | Must not name `ThemeId`, build options, or import `cyril-ui` |
| Config surface | `crates/cyril-core/src/types/config.rs` | None — `UiConfig` gains two `Option<String>` fields | Must not parse or validate theme semantics |
| Env probe + wiring | `crates/cyril/src/main.rs` (probe) and `crates/cyril/src/app.rs` (consume) | `App::new` gains a `ColorEnvironment` parameter | `main.rs` must not read `UiConfig` fields directly (existing C3 fence); `App` must not read `std::env` |

The config value type is `Option<String>`, not an enum, for two reasons: it
distinguishes "absent" (silent default) from "present but unknown" (visible
message), and it keeps the palette registry out of `cyril-core`, which may not
depend on `cyril-ui`.

## Module shape

Current cluster (inventory, from recon):

| Module | Interface | Responsibility clusters | Dependency direction | Change |
|---|---|---|---|---|
| `crates/cyril-ui/src/theme.rs` | `ThemeId`, `ColorMode`, `SyntaxTheme`, `Theme`, `resolve*` | palette data, projection, ANSI-16 semantics, contracts | `ratatui` + `syntect` only | `deepen` |
| `crates/cyril-ui/src/state.rs` | `UiState`, `TuiState` impl | UI state machine, picker lifecycle, theme ownership | `theme`, `traits` | `deepen` |
| `crates/cyril-ui/src/traits.rs` | `TuiState`, `PickerState` | read-only rendering contract, view models | `theme` | `widen` (one field) |
| `crates/cyril-core/src/commands/` | `Command`, `CommandResult` | command parsing and dispatch | no UI crate | `widen` (one local command, one result kind) |
| `crates/cyril-core/src/types/config.rs` | `Config`, `UiConfig` | config schema | serde only | `widen` (two fields) |
| `crates/cyril/src/main.rs` | process startup | env probe, terminal setup, wiring | all crates | `widen` (one probe call) |
| `crates/cyril/src/app.rs` | `App` | event loop, routing, cross-module effects | all crates | `widen` (consume two fields, one result kind, one key branch) |

Seam tests:

- **Deletion test**: deleting `theme.rs` scatters palette data, projection, and
  now id/mode parsing across the binary and the command layer. Not pass-through.
- **Interface test**: callers reach parsing and detection through `theme.rs`
  functions; tests use the same seam.
- **Adapter test**: `N/A — no generic seam`. There is one bundled registry; no
  adapter trait is introduced.
- **Locality test**: catalog, spelling, and detection policy stay in one file
  with one contract test module.

Ownership alternatives for the picker:

1. **Extend `PickerState` with a `PickerKind` discriminator (selected).**
   Reuses viewport, filtering, scrollbar, active-marker rendering, and ~20
   existing tests; Enter's behavior branches in one place in `App`.
2. **A separate `ThemePickerState` overlay.** Duplicates windowing, filtering,
   and scrollbar logic, and adds a fifth overlay to the key-handling chain and
   the mouse-scroll guard. Rejected: parallel modal path for one feature.
3. **Reuse the model-picker wire path** (a fake agent command). Rejected: the
   agent has no theme command; it would need bridge traffic and a fake RPC for
   local state.

Ownership alternatives for the `/theme` command result:

1. **New semantic `CommandResultKind::ShowThemePicker` (selected).** `cyril-core`
   stays palette-ignorant; `App` maps the signal to `UiState::open_theme_picker`.
2. **`ShowPicker { title, options }` with options built in `cyril-core`.**
   Rejected: duplicates the palette catalog and id spelling into `cyril-core`,
   which must not depend on `cyril-ui`.
3. **String-match `/theme` in `App` before the registry.** Rejected: bypasses
   help, aliases, and the single command surface.

Approved module ledger:

| Module/path | Interface | Owns | Hides/reuses | Must not own | Change |
|---|---|---|---|---|---|
| `crates/cyril-ui/src/theme.rs` | `+ parse_theme_id`, `parse_color_mode`, `detect_color_mode`, `ColorEnvironment`, `ColorModeResolution`, `StartupAppearance` | catalog spelling, mode spelling, detection precedence | precedence table, accepted sets | env reads, config, rendering | `deepen` |
| `crates/cyril-ui/src/state.rs` | `+ set_appearance`, `open_theme_picker`; `TuiState::theme` unchanged | committed appearance, preview, picker commit semantics | preview mechanics | fs, bridge, config | `deepen` |
| `crates/cyril-ui/src/traits.rs` | `PickerState + kind: PickerKind` | read-only picker view model | — | theme policy | `widen` |
| `crates/cyril-core/src/commands/builtin.rs` | `+ theme` local command | command surface | — | palette catalog | `widen` |
| `crates/cyril-core/src/commands/mod.rs` | `+ CommandResultKind::ShowThemePicker` | result vocabulary | — | palette catalog | `widen` |
| `crates/cyril-core/src/types/config.rs` | `UiConfig + theme, color_mode` | config schema | — | theme semantics | `widen` |
| `crates/cyril/src/main.rs` | `+ ColorEnvironment` probe | process boundary env read | env names | `UiConfig` field reads | `widen` |
| `crates/cyril/src/app.rs` | `App::new(+ environment)` | UiConfig consumption, result routing, picker Enter branch | — | env reads, palette data | `widen` |

Protected parents:

| Protected parent | Baseline | Allowed change | Forbidden | Exit condition |
|---|---|---|---|---|
| `crates/cyril-ui/src/widgets/` | render from `&Theme` only | none | any `ThemeId`/`ColorMode`/`resolve` reference in production | existing widget source fence passes |
| `crates/cyril-memory/` | memory config/runtime | none | any production delta | `git diff` empty |
| `crates/cyril-core/src/protocol/` | bridge, mediators, convert | none | any production delta | `git diff` empty |

Shape fence: `.cyril-qaq0/oracles/module_shape.py` — production-path check,
`ThemeId`/`ColorMode` confined to `cyril-ui` (+ the `App` consumption seam),
widget ban, protected-parent delta, and "no bridge command from the theme
picker path".

## Claims

- **C1** Every accepted `[ui] theme` string maps to its `ThemeId`; every other
  string maps to `None`.
- **C2** Every accepted `[ui] color_mode` string maps to its `ColorMode`; every
  other string maps to `None`.
- **C3** Automatic detection returns the precedence table's result for every
  combination of the injected environment, with an explicit value overriding
  every other rule.
- **C4** While a theme picker is open, `TuiState::theme()` returns the preview
  for the highlighted row; otherwise it returns the committed theme.
- **C5** Enter commits the preview and closes the picker; Esc discards the
  preview and closes the picker; neither path writes any file.
- **C6** Every bundled theme × 4 color modes renders through that theme's
  resolved roles, with geometry unchanged.
- **C7** With no config file, startup renders byte-identically to today.
- **C8** An unrecognized `theme` or `color_mode` string produces exactly one
  visible system message naming the value and the default, while every other
  config key is honored.
- **C9** `/theme` opens the picker without any bridge traffic.
- **C10** The picker's option list is built only in `cyril-ui` from
  `ThemeId::ALL`; `cyril-core` names no theme.
- **C11** A wrong-typed `theme`/`color_mode` value keeps the pinned whole-file
  default posture rather than producing a per-key message.

## Falsification

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C1 | Accepted theme ids map; others do not | `[ui] theme` × 9 shapes | `theme::tests::parse_theme_id_covers_exactly_the_bundled_ids` asserts all 6 accepted ids round-trip and 3 rejected strings return `None`; a wrong mapping falsifies. Control: a lenient parser passes the accepted half but fails the rejected half | `.cyril-qaq0/oracle-theme-ids.py` computes the accepted set from the spec table, independent of the Rust macro | make `parse_theme_id` return `Some(ThemeId::CyrilDark)` for unknown input | `theme::tests::parse_theme_id_covers_exactly_the_bundled_ids` | seconds | PENDING — checkpointed-build, slice 1 |
| C2 | Accepted mode strings map; others do not | `[ui] color_mode` × 9 shapes | `theme::tests::parse_color_mode_covers_exactly_the_five_values`; a wrong mapping falsifies | `.cyril-qaq0/oracle-theme-ids.py` (same independent table) | make `parse_color_mode` return `Some(ColorMode::TrueColor)` for unknown input | `theme::tests::parse_color_mode_covers_exactly_the_five_values` | seconds | PENDING — checkpointed-build, slice 1 |
| C3 | Detection follows the precedence table | env × 4 vars × explicit/absent | `theme::tests::detection_precedence_matches_every_table_row` drives 12 injected rows (7 rules + 5 combination rows); any row mismatching the spec table falsifies. Control: a detector that ignores `NO_COLOR` fails rows 2/6; one that ignores the explicit value fails row 1 | `.cyril-qaq0/oracle-precedence.py` implements the table independently and prints the expected mode per row | move the `NO_COLOR` check above the explicit-config check | `theme::tests::detection_precedence_matches_every_table_row` | seconds | PENDING — checkpointed-build, slice 1 |
| C4 | Preview drives the rendered theme | picker open/closed × selection | `state::tests::theme_returns_preview_while_theme_picker_is_open` and `theme_returns_committed_after_close`; a preview that leaks past close falsifies | `TuiState::theme()` compared against `resolve(id, mode)` for the expected id, an independent computation | make `TuiState::theme()` ignore the preview | `state::tests::theme_returns_preview_while_theme_picker_is_open` | seconds | PENDING — checkpointed-build, slice 2 |
| C5 | Enter commits, Esc discards, neither writes | picker lifecycle | `app::tests::theme_picker_session_leaves_config_bytes_identical` runs open→move→Esc and open→move→Enter against a temp copy of `.cyril-qaq0/config-nowrite-fixture.toml` and asserts byte equality; `state::tests::enter_commits_and_esc_restores` asserts the committed theme. Positive control: the test asserts the fixture is non-empty and that a planted write changes the bytes | Raw file bytes compared before/after, independent of any config API | write the config file inside the theme-commit path | `app::tests::theme_picker_session_leaves_config_bytes_identical`, `state::tests::enter_commits_and_esc_restores` | seconds | PENDING — checkpointed-build, slice 2 |
| C6 | Every theme × mode renders resolved roles | 6 ids × 4 modes × 4 scenes | Extended `all_scene_theme_mode_combinations_pass` (96 passes) — **cheapest falsifier, run 2026-09-09: PASS**; a theme whose roles are not the ones rendered falsifies | The pinned 7,680-cell Cyril Dark baseline plus per-role projection computed from `resolve(id, mode)`, independent of the renderer | make `resolve` ignore its `id` argument | `render::tests::all_scene_theme_mode_combinations_pass` | seconds | PASS |
| C7 | No-config startup is byte-identical | absent config | `migrated_scenes_match_all_pinned_cells` (7,680 cells, 0 differences) plus `state::tests::new_state_uses_cyril_dark_truecolor`; a changed default falsifies | The committed fixture `crates/cyril-ui/tests/fixtures/conversation-theme-baseline.tsv`, captured before this change | change `UiState::new`'s default to another palette | `migrated_scenes_match_all_pinned_cells` | seconds | PENDING — checkpointed-build, slice 2 |
| C8 | Unknown value → one visible message | unknown theme; unknown mode | `app::tests::unknown_theme_value_renders_one_system_message` and the mode twin assert exactly one message containing the value and the default; zero or two messages falsifies. Positive control: the valid-value twin asserts zero messages | The expected string is composed from the spec's message shape, not from the implementation's constant | drop the diagnostic push | `app::tests::unknown_theme_value_renders_one_system_message` | seconds | PENDING — checkpointed-build, slice 2 |
| C9 | `/theme` sends no bridge command | command dispatch | `commands::tests::theme_command_is_local_and_returns_show_theme_picker` plus `app::tests::theme_command_opens_picker_without_bridge_traffic` (the test bridge records zero sends); a query falsifies. Positive control: `/model` with no args still records one `QueryCommandOptions` | The test bridge's send log, independent of the command implementation | make the theme command send `QueryCommandOptions` | `app::tests::theme_command_opens_picker_without_bridge_traffic` | seconds | PENDING — checkpointed-build, slice 1 |
| C10 | Catalog knowledge stays in `cyril-ui` | placement | `.cyril-qaq0/oracles/module_shape.py` asserts no `ThemeId`/`ColorMode`/theme-id string literal in `crates/cyril-core` production and no palette access in widgets; a violation falsifies. Positive control: the fence reports a planted `ThemeId` reference as a violation | Source census independent of the compiler | build the option list in `cyril-core` from a hardcoded list | `.cyril-qaq0/oracles/module_shape.py` | seconds | PENDING — checkpointed-build, per-slice |
| C11 | Wrong type keeps whole-file defaults | `theme = 3`; `color_mode = true` | `config::tests::wrong_typed_theme_falls_back_to_whole_file_defaults` and the mode twin, with a non-default sibling key (`max_messages = 999`) as the discriminator; per-key survival falsifies | The existing nd4h legacy-compat test shape, independent of the new fields | add `#[serde(deserialize_with)]` that silently defaults the field | `config::tests::wrong_typed_theme_falls_back_to_whole_file_defaults` | seconds | PENDING — checkpointed-build, slice 1 |

C6 is the cheapest falsifier and has been run (see the run log). No row is
`FAIL`.

## Non-goals and future work

- **Permanent non-goal** — persisting a theme choice: ADR 0005 plus the
  requester's session-only decision. No tracker issue; the rationale is here and
  in `.cyril-qaq0/spec.md`.
- **Permanent non-goal** — user-defined palettes: ADR 0005.
- **Intended future work** — a live `/mode` command: cyril-qaq0 deliberately
  keeps mode config-only; a runtime mode switch is not conditioned on any
  trigger and is not filed, because the spec's decision 5 settles the axis
  separation for this feature. If a future request asks for it, that request
  files its own issue.
- **Intended future work** — structurally complete cache identities:
  **cyril-x5xi** (verified open 2026-09-09).
- **Intended future work** — wiring or removing the remaining dead `[ui]` knobs:
  **cyril-nd4h** (closed; the seam it built is what this design consumes).

## Falsifier run log

- 2026-09-09 — **cheapest falsifier (C6)**: prototyped the mode-matrix
  extension in the cyril-fkke worktree (the only tree with six palettes) by
  wrapping the existing 4-mode loop in a `ThemeId::ALL` loop. First run
  **FAILED**: `truecolor/markdown foreground cell 122: syntax expected
  Rgb(204,153,204), actual Rgb(180,142,173)`. Root cause: the fence's syntax
  branch assumes the marker theme's syntax component equals the projected
  theme's; the marker was built once from Cyril Dark's syntax. Correction: build
  the marker scenes per theme with `marker.syntax = resolve(id, TrueColor).syntax`.
  Second run **PASSED**: 96 passes, 96 distinct labels, 24 no-color passes with
  zero non-Reset cells. Probe diff saved as `.cyril-qaq0/mode-matrix-probe.diff`;
  the cyril-fkke tree was restored and its original 16-combination test
  re-verified green.
- 2026-09-09 — **independent oracles created and self-checked**:
  `python3 .cyril-qaq0/oracle-theme-ids.py --check` →
  `PASS 6 theme ids, 5 color modes, 10 rejected spellings`;
  `python3 .cyril-qaq0/oracle-precedence.py --check` →
  `PASS 14 precedence cases agree with the spec table`. Both encode the spec's
  tables directly and never read the Rust implementation.
- 2026-09-09 — the correction is a design fact, not a test detail: the
  regression fence must build marker scenes per theme, which is why C6's fence
  description names the per-theme marker.

## Approval

**Approved 2026-09-09.** The requester replied, verbatim:

> Approve

Approved risk acceptances:

- **C12-adjacent detection limitation** — terminals that misreport
  `COLORTERM`/`TERM`, and tmux RGB-passthrough variance, may resolve to a mode
  the operator did not intend. Recorded in `.cyril-qaq0/spec.md` under Success
  criteria; no fence is possible.

Approved decisions presented for this approval: the `[ui]` config surface and
kebab-case value spelling, `Option<String>` config typing, `cyril-ui` ownership
of id/mode parsing and detection policy with the env probe in the binary,
`PickerState` + `PickerKind` reuse instead of a new overlay, the semantic
`ShowThemePicker` command result, and the per-theme marker scenes the C6
falsifier proved necessary.
