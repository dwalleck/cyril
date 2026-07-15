# cyril-dij8 — Design: migrate application chrome to semantic colors

## Purpose

Move the application chrome — toolbar, status bar (both in `toolbar.rs`),
crew panel, voice indicator, and the shared chrome frame background — off
hardcoded `Color::` literals and `palette::` color constants onto the
semantic theme contract, following the ghuu/nrnq method: frozen legacy
inventory → canonical ANSI RGB mapping → representability →
zero-normalized-diff equivalence. The probe showed this is the first
**pure re-mapping batch**: all 12 legacy colors are already representable
in the 31-role contract (falsifier passed, 0 missing) — no expansion,
unlike ghuu (+10) and nrnq (+2).

## Architecture

1. **Migrate (3 widget files):** `toolbar::render`,
   `toolbar::render_status_bar` (+ `status_bar_spans`),
   `crew_panel::render`, and `voice::render` each gain `theme: &Theme`
   (the caller `render.rs::draw_inner` already holds `state.theme()`);
   every literal swaps per the canonical mapping:

   | Legacy | Role | Where |
   |---|---|---|
   | `Rgb(30,30,46)` | `chrome` (EXACT) | toolbar + status bar bg |
   | `Color::White` | `text` | session label, crew names |
   | `Color::DarkGray` | `subdued` | secondary text everywhere |
   | `Color::Gray` | `text_secondary` | pending-stage name |
   | `Color::Yellow` | `emphasis` | spinners, effort, steers, warnings, SCROLL, overflow |
   | `Color::Green` | `subdued_positive` | streaming spinner, context OK, ● |
   | `Color::Red` | `subdued_negative` | context critical, Refused |
   | `Color::Cyan` | `accent_quinary` | spinner, mode, code intel, crew title |
   | `Color::Magenta` | `accent_quaternary` | model, loop badge |
   | `palette::USER_BLUE` | `soft_accent` | voice listening |
   | `palette::MUTED_GRAY` | `muted` | voice hint |
   | `palette::SYSTEM_MAUVE` | `accent_alt` | voice transcribing |

   VGA-exact roles (not intent roles like `warning`/`danger`) keep the
   equivalence contract byte-clean — the posture the user approved for
   nrnq; cyril-leiq (verified open, P1) owns role VALUES, so a later
   re-valuation reaches chrome with zero chrome edits. Twin assignments
   (`soft_accent` not `user`, `muted` not `border`, `accent_alt` not
   `system`) follow ghuu's convention: speaker roles are reserved for
   speaker labels; chrome indicators take the non-speaker twin.
   The dead `(White, EndTurn)` arm (probe finding: label empty, never
   rendered) maps inertly to `theme.text`.
2. **Fence (tests):** frozen pre-migration baseline TSV + marker-theme
   wiring tests + no-legacy-source scans, replicating the nrnq fence
   family under a new `chrome_theme.rs` integration test... except fences
   needing `MockTuiState`/`UiState` (cfg(test)-gated) live as in-crate
   test modules; the source-scan fences extend theme.rs's existing family.

## Input shapes (each covered by ≥1 claim)

| Shape | Values | Claim |
|---|---|---|
| `Activity` | Idle/Ready (no spinner) · Sending/Waiting (emphasis) · Streaming (subdued_positive) · ToolRunning (accent_quinary) | C2 scenes; C10 (Idle) |
| Toolbar options | session Some/None; mode/model/effort Some/None; steers 0/1/2; intel on/off; elapsed Some/None | C2 (both toolbar scenes), C10 |
| `context_usage` | None / ≤70 / >70 / >90 / boundary pct=70, pct=90 (strict `>` today) | C2, C10 (boundaries) |
| `StopReason` ×5 | EndTurn (no label; dead White arm) / MaxTokens / MaxTurnRequests / Refusal / Cancelled | C2 (4 scenes), dead arm inert |
| Tokens/credits/scroll/breakdown | each Some/None; cached >0/None; breakdown fits/omitted (width) | C2, C10 |
| Status all-empty | "cyril" fallback | C2 (scene) |
| Crew rows | Working(msg Some/None)/Terminated; loop Some/None; pending deps empty/multi; ≤6 rows / >6 overflow | C2, C10 |
| Crew header | groups [] ("subagents") / [one] ("crew: X") / many ("N crews") — same tuple (accent_quinary) | C2 (one), C10 (others) |
| `VoiceStatus` | Idle (height 0, renders nothing) / Listening (level meter unstyled) / Transcribing | C2, C10 (Idle) |
| `ColorMode` | TrueColor / NoColor scenes; Ansi256/Ansi16 via existing projection suite (all 12 roles pre-covered by theme.rs tests — no new roles) | C2, C6; posture approved in nrnq |
| Theme flow | production single-resolve; marker theme | C3, C5 |

Out-of-scope shapes: Unicode/width in labels (unchanged by color
migration); `ThemeId` beyond CyrilDark (single variant; bundled palettes
are cyril-fkke, verified open); spinner timing/frame selection
(non-color); breakdown width-fitting arithmetic (behavior, untouched).

## Subtractive sweep (step 2b)

Removes one constraint: "chrome colors are compile-time constants,
independent of `state.theme()`." What it guaranteed: (1) *chrome render
output ignores the theme* — the two full-frame insta snapshots
(`theme_seam_idle`, `theme_seam_tool_diff`) include toolbar/status cells
and WILL change; re-baseline is sanctioned, and C2's normalized
equivalence proves the change is exactly the canonical mapping and
nothing else. (2) *widget tests get stable colors from the mock* —
`MockTuiState.theme` defaults to `marker_theme()`, so existing
toolbar/crew tests (which assert text only, probe-verified) keep passing;
any existing test asserting a chrome color would need the sanctioned
mapping applied (none do). (3) *no caller needs a `Theme`* —
signature change is compile-enforced; sole caller is `render.rs`.
No lock, ordering, or uniqueness property involved.

## Claims

1. **C1 — representability.** The current 31-role Cyril Dark source
   contains every canonical RGB value in the frozen chrome legacy
   inventory (12 values, 0 new).
2. **C2 — equivalence.** Migrated true-color renders of the 13 probe
   scenarios differ from the frozen pre-migration baseline by zero
   normalized cells (normalization: named → canonical RGB per the
   mapping table; nothing else collapses).
3. **C3 — role wiring.** Under the pairwise-distinct marker theme, each
   migrated element renders its MAPPED role's marker color —
   distinguishing the three twin pairs equivalence cannot see
   (`soft_accent`≠`user`, `muted`≠`border`, `accent_alt`≠`system` under
   marker; all equal under Cyril Dark).
4. **C4 — no legacy sources.** After migration, `toolbar.rs`,
   `crew_panel.rs`, `voice.rs` production sections contain zero
   `Color::` literals and zero `palette::` COLOR-constant references
   (`SPINNER_CHARS`/`SPINNER_FRAME_MS` allowlisted — not colors).
5. **C5 — single theme flow.** A full frame under the marker theme
   renders chrome cells from `UiState`'s one resolved theme
   (`state.theme()` called exactly once in `render.rs` production).
6. **C6 — no-color reset.** Under the NoColor projection the scenarios
   carry zero non-Reset colors, with symbols identical to truecolor.
7. **C7 — non-color distinguishability (AC2).** In NoColor renders,
   status meaning is carried by text/symbols alone: stop-reason labels,
   `Context: N%`, crew ●/◆/○ + status words + `+N more`, `⇄ N steers`,
   voice 🎙/⏳ + words.
8. **C8 — scenario completeness.** The baseline scenario set jointly
   exercises all 23 frozen styled tuples (else C2 is vacuous for
   unreached tuples).
9. **C9 — behavior/geometry untouched.** Existing widget/state suites
   pass unmodified (except the two sanctioned snapshot re-baselines);
   the diff touches no layout arithmetic, no `state.rs`, no `traits.rs`
   beyond nothing (mock untouched).
10. **C10 — edge shapes.** Idle/Ready toolbar, status boundaries
    pct=70→OK-color and pct=90→warn-color (strict `>` preserved), crew
    header variants, `Working(None)` message, voice Idle (nothing
    rendered) — all render under the theme param without panic, styled
    per mapping.

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| 1 | C1 | representability script; negative control injects phantom #123456 | ghuu NAMED canon + probe-styles.txt parsed from source text (no rustc) | **2m** | **PASSED** (12 required, missing=0; control fires exactly {#123456}) | `theme::tests::chrome_legacy_colors_are_representable` |
| 2 | C2 | render 13 scenarios post-migration, diff against committed TSV | **frozen pre-migration baseline TSV** (generated from CURRENT code in plan slice 1, before any migration commit) | 45m | pending | `chrome_theme` baseline-equivalence tests, one per surface group (toolbar/status/crew/voice) |
| 3 | C3 | render scenarios under `marker_theme()`; assert per-element marker colors | hand-pinned element→role→marker table written from the mapping, not from render output | 30m | pending | `chrome_theme::marker_wiring_{toolbar,status,crew,voice}` |
| 4 | C4 | scan the 3 files for `Color::` and palette color idents | raw text scan (same mechanism as ghuu/nrnq source fences) | 5m | pending | `theme::tests::chrome_widgets_have_no_legacy_color_sources` |
| 5 | C5 | full-frame render with crew+voice active under marker theme; assert toolbar bg == marker `chrome` | marker-role table applied to frame cells; a widget resolving internally shows Cyril Dark values instead | 10m | pending | `render::tests::chrome_frame_uses_state_theme` |
| 6 | C6 | render scenarios under NoColor resolve | assert every cell fg/bg == Reset; symbols diffed against truecolor scenario symbols | 10m | pending | `chrome_theme::no_color_scenarios_reset` |
| 7 | C7 | NoColor render; strip styles; locate labels/symbols | symbol scan for the label strings (text equality, zero style info used) | 5m | pending | `chrome_theme::no_color_status_distinguishable` |
| 8 | C8 | count distinct styled tuples in the frozen baseline TSV | frozen probe-styles.txt count (23) | 2m | pending | assertion inside the baseline generator + a named completeness test |
| 9 | C9 | run existing suites unmodified; inspect diff scope | pre-existing tests (authored before this design) + `git diff --stat` | 2m | pending | existing widget/state suites + the two re-baselined snapshots |
| 10 | C10 | render edge-shape fixtures | no-panic + tuple subset check vs mapping table (boundaries hand-pinned: 70→OK, 90→warn) | 15m | pending | `chrome_theme::edge_shapes_render` |

Non-vacuity (a buggy implementation each catches): C1 — a contract
missing `accent_quaternary` (control run PROVES the mechanism fires);
C2 — mapping Cyan→`accent` (`#00ffff`) instead of `accent_quinary`
(`#008080`): normalized diff ≠ 0 on every mode/title cell; C3 — swapping
`muted`↔`border` or `soft_accent`↔`user` (both invisible to C2 — values
coincide under Cyril Dark; marker theme separates them); C4 — one
leftover `Color::Yellow` in `status_bar_spans`; C5 — `toolbar::render`
calling `resolve_truecolor` internally (frame shows Cyril Dark values
under marker); C6 — carrying a concrete `Rgb` through the NoColor path;
C7 — dropping the "Cancelled" label and signaling by color alone; C8 —
a scenario set that skips `voice_transcribing` (count 22 < 23); C9 —
"improving" the crew border to `theme.border` mid-migration (pixel
change; snapshot + C2 fire); C10 — flipping strict `>` to `>=` at the
70/90 boundaries (boundary fixture pins today's Green at exactly 70).

Distinctness: every fence is a separately named test scoped to one
surface; the falsifier script prints labeled counts.

## Negative space (deliberately NOT in this change)

1. **No role VALUES change** — dim-VGA readability is cyril-leiq (open,
   P1); the canonical mapping keeps chrome one-touch for leiq's future
   re-valuation.
2. **No intent re-bucketing** — the context gauge's Green/Yellow/Red map
   to VGA-exact `subdued_positive`/`emphasis`/`subdued_negative`, not
   `success`/`warning`/`danger` (which hold bright values `#00ff00`/
   `#ffff00`/`#ff0000` that would visibly change chrome NOW and break the
   equivalence AC). The issue AC's "success, warning, danger" wording is
   satisfied at the STATE level (those states get semantic roles); intent
   re-bucketing is the theme-activation era's call (cyril-qaq0, verified
   open) or leiq's re-valuation.
3. **No styling of currently-unstyled elements** — crew's `Block` border
   stays default-styled (renders Reset today; probe finding #3), the
   voice meter bar stays unstyled, separators stay `Span::raw`.
4. **No palette contraction** — `palette.rs` keeps its color constants
   until cyril-6r3a (verified open, blocked by this issue); only chrome's
   REFERENCES to them are removed. Spinner constants stay in `palette`.
5. **No new roles, no theme switching, no activation** — CyrilDark
   remains the only `ThemeId` (cyril-qaq0 / cyril-fkke, verified open).
6. **No behavior changes** — breakdown width-fitting, overflow capacity,
   spinner frame timing, singular/plural steer text, `height_for`
   contracts all byte-identical.
7. **No `MockTuiState` extension** — voice fences fixture through the
   real `UiState` (probe finding #2); adding voice fields to the mock is
   out of scope.

## Open decisions (for design approval)

1. **Twin-role assignment for voice** — recommended: `soft_accent`
   (listening), `muted` (hint), `accent_alt` (transcribing), following
   ghuu's "speaker roles only for speaker labels" convention.
   Alternative: `user`/`muted`/`system` — reads semantically ("user is
   speaking / system is working") but couples chrome to speaker-label
   re-values; visually identical today either way.
2. **Named-color posture (reconfirm)** — recommended: VGA-exact roles,
   the posture approved for nrnq; keeps zero-diff equivalence and leiq
   orthogonality. The AC's "success, warning, danger" wording is
   satisfied at the state level (see Negative space #2). Alternative:
   intent roles — brightens the status gauge/warnings NOW and fails the
   visual-equivalence AC as written.
3. **Mode coverage depth (inherited)** — TrueColor + NoColor scenario
   fences with ANSI modes covered by the existing projection unit suite
   (approved for nrnq; chrome adds zero new roles, so the projection
   suite already covers every role this batch touches).

## Approval

Approved by dwalleck 2026-07-14 (in-session): voice twin-roles =
`soft_accent` (listening) / `muted` (hint) / `accent_alt` (transcribing);
mapping posture = VGA-exact roles (reconfirmed from nrnq); mode coverage =
TrueColor + NoColor scenarios with the existing projection unit suite
covering ANSI modes (inherited from nrnq approval). Cheapest falsifier
passed (12 values, 0 missing; phantom control fires). Proceed to
budgeted-plan.
