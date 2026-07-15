# cyril-dij8 — prove-it-prototype findings (2026-07-14)

## Smallest question

> What distinct (fg, bg, modifier) style tuples do the chrome surfaces —
> toolbar, status bar, crew panel, voice indicator — actually emit at
> runtime today?

## Probe

`probe_dij8.rs` (ran as `#[cfg(test)] mod probe_dij8` inside cyril-ui —
toolbar/crew/voice take `&dyn TuiState`, and `test_support::MockTuiState`
is cfg(test)-gated, so an in-crate unit test, not an integration test like
nrnq's; source archived here). Branch-maximal fixtures: 3 spinner
activities, session present/absent, mode+model+effort+steers+code-intel,
all 3 context-gauge bands, breakdown bar, all 4 non-EndTurn stop reasons,
tokens+credits, scroll hint, empty fallback, crew working/terminated/
loop-badge/pending/overflow, voice listening/transcribing. Voice fixtures
use the real `UiState` — voice fields live there, NOT on `MockTuiState`
(the trait defaults `voice_status()` to Idle; the mock never overrides
it). Output: `probe-styles.txt` — **23 distinct styled tuples** across 13
scenarios (plus the two unstyled flavors: Reset-on-chrome-bg and full
Reset).

## Oracle

Static source scan of `toolbar.rs` / `crew_panel.rs` / `voice.rs`,
hand-transcribed to per-widget expected tuples BEFORE the probe ran
(`oracle-expected.md`; mechanism: raw source text vs rendered ratatui
buffer). **Agreement: all 23 predicted tuples observed, zero unpredicted
tuples** — including both negative predictions: the `(White, EndTurn)`
arm at toolbar.rs:181 is dead styling (empty label, never pushed; no
White cell in any status-bar scenario), and the Paragraph-level chrome bg
`Rgb(30,30,46)` paints separators and trailing blanks (T1/S1 confirmed).

## Legacy inventory → canonical mapping (ghuu NAMED scheme)

12 distinct legacy colors; every one is representable in the current
31-role contract (verified against theme.rs `EXPECTED_RGB`):

| Legacy literal | Canonical RGB | Role |
|---|---|---|
| `Rgb(30,30,46)` (toolbar+status bg) | `#1e1e2e` | `chrome` — EXACT |
| `Color::White` | `#ffffff` | `text` |
| `Color::DarkGray` | `#808080` | `subdued` |
| `Color::Gray` | `#c0c0c0` | `text_secondary` |
| `Color::Yellow` | `#808000` | `emphasis` |
| `Color::Green` | `#008000` | `subdued_positive` |
| `Color::Red` | `#800000` | `subdued_negative` |
| `Color::Cyan` | `#008080` | `accent_quinary` |
| `Color::Magenta` | `#800080` | `accent_quaternary` |
| `palette::USER_BLUE` | `#8ab4f8` | `soft_accent` / `user` (value twins) |
| `palette::MUTED_GRAY` | `#8c8c8c` | `muted` / `border` (value twins) |
| `palette::SYSTEM_MAUVE` | `#b48ead` | `system` / `accent_alt` (value twins) |

## What I learned (that I didn't know before)

**dij8 is the first PURE re-mapping batch — all 12 chrome legacy colors
are already representable in the 31-role contract, so unlike ghuu (+10
roles) and nrnq (+2), this migration expands nothing; the only genuine
decisions are role-ASSIGNMENT choices among value-identical twins (three
of them: `#8ab4f8`, `#8c8c8c`, `#b48ead`) and semantic-vs-canonical for
the status families.**

Also material for the design:
1. `(White, EndTurn)` at toolbar.rs:181 is dead styling — the migration
   can drop it or map it inertly; equivalence tests won't see it either
   way.
2. Voice state lives on `UiState`, not `MockTuiState` — voice equivalence
   fences must fixture through the real `UiState` (or extend the mock,
   which is scope creep).
3. Toolbar and status bar share one file (`toolbar.rs`) and one bg
   literal; "shared frame styling" from the issue = the chrome bg
   (`Rgb(30,30,46)` = `chrome` role exactly) + crew's unstyled
   `Block::bordered` (border cells render Reset today — mapping them to
   `theme.border` would CHANGE pixels; equivalence demands leaving the
   block unstyled or explicitly styling with a Reset-valued role).
4. After this batch, `palette`'s four color constants have zero
   production consumers (toolbar keeps only `SPINNER_CHARS`/
   `SPINNER_FRAME_MS`) — contraction is cyril-6r3a's job.

## Prior art

See `related-issues.md`: ixua (closed — the seam), ghuu (closed — method +
NAMED canon), nrnq (closed — direct template, PR #53), leiq (OPEN P1 —
role VALUES, orthogonal; canonical mapping keeps it one-touch), 6r3a
(contraction after all batches), qaq0/fkke (activation, downstream).

## Hard gate

- [x] Probe written, runs against the real codebase (all four production chrome surfaces)
- [x] Oracle defined, produces output (static source scan, transcribed pre-run in `oracle-expected.md`)
- [x] Probe and oracle agree (23/23 tuples, both negative predictions held)
- [x] One-sentence learning recorded (above)
