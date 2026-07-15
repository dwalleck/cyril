# cyril-nrnq — prove-it-prototype findings (2026-07-14)

## Smallest question

> What distinct (fg, bg, modifier) style tuples do the four modal overlays
> (approval both phases, picker, hooks, code panel) actually emit at runtime
> today?

## Probe

`probe_nrnq.rs` (run as `crates/cyril-ui/tests/probe_nrnq.rs`, source archived
here) — renders each modal into a `TestBackend` with branch-maximal fixtures
(both approval phases, trust options, all four LSP statuses, matcher and
matcher-less hooks, selected/unselected/description picker rows) and dumps the
distinct style tuples. Output: `probe-styles.txt` — **30 styled tuples** (plus
one `Reset|Reset|NONE` per widget: unstyled cells; `Cell::style()` returns
concrete `Reset` where `Style::default()` holds `None`s — same normalization
reality ghuu's design recorded).

## Oracle

Static source scan — `grep -n "Color::|Modifier::"` over the four widget
files, hand-transcribed to expected per-widget tuples BEFORE the probe ran
(different mechanism: raw source text vs rendered ratatui buffer).
**Agreement: every one of the 30 runtime tuples is predicted by the source
scan and every source literal was reached by a fixture** — including the
phase asymmetry (approval selected row is BOLD, picker's is not) and the
hooks matcher purple.

## Legacy inventory → canonical mapping (ghuu method, `.cyril-ghuu/cheapest-falsifier.py` NAMED table)

| Legacy literal | Canonical RGB | Existing role with that value |
|---|---|---|
| `Rgb(50,50,70)` (selection bg) | `#323246` | `selection` — EXACT |
| `Color::White` | `#ffffff` | `text` |
| `Color::Cyan` | `#008080` | `accent_quinary` |
| `Color::DarkGray` | `#808080` | `subdued` |
| `Color::Yellow` | `#808000` | `emphasis` |
| `Color::Green` | `#008000` | `subdued_positive` |
| `Color::Red` | `#800000` | `subdued_negative` |
| `Color::Gray` | `#c0c0c0` | **NONE — missing from the 29 roles** |
| `Rgb(176,141,255)` (hooks matcher) | `#b08dff` | **NONE — missing from the 29 roles** |

(`Color::Gray` is absent from ghuu's NAMED canon because conversation
surfaces never used it; `#c0c0c0` is the VGA value consistent with that
table's scheme.)

## What I learned (that I didn't know before)

**The modal batch introduces two legacy colors (`Color::Gray` → `#c0c0c0`
and matcher purple `#b08dff`) that the 29-role contract cannot represent —
nrnq is an expand-AND-migrate batch like ghuu (whose 26-role draft was
rejected for exactly this failure mode), not a pure re-mapping.**

Also material for the design: (1) the leiq P1 (dim VGA values unreadable) is
a role-VALUE problem, orthogonal to nrnq's role-ASSIGNMENT problem — the
ghuu-consistent canonical mapping keeps the equivalence contract intact and
lets leiq re-value roles later without re-touching modals; (2) approval and
picker share the selection style except for BOLD (approval has it, picker
doesn't) — an existing asymmetry to preserve, and both selections already
carry the ▸ prefix, satisfying "distinguishable without color alone" today.

## Prior art

See `related-issues.md`: ixua (closed — the 29-role seam), ghuu (closed —
the method + equivalence fences to replicate), leiq (open P1 — role values,
orthogonal but adjacent), dij8 (sibling batch: chrome), 6r3a (cleanup after
all batches), a14l/uw20 (own the modal GEOMETRY; nrnq is colors only).

## Hard gate

- [x] Probe written, runs against the real codebase (all four production widgets)
- [x] Oracle defined, produces output (static source scan, transcribed pre-run)
- [x] Probe and oracle agree (30/30 tuples, all fixtures reach all branches)
- [x] One-sentence learning recorded (above)
