# cyril-a14l design — height-aware layout at the 60×16 floor

Grounded in `.cyril-a14l/findings.md` (probe ↔ pty-oracle agreement on four
slices). The design contradicts no probe fact; it replaces the two
solver-emergent casualties (input, suggestions) with explicit allocation and
teaches modals to respect the input.

## Core rules

**R1 — Explicit vertical budget** (in `render::draw_inner`):

```
avail    = height − toolbar(1) − status(1) − crew − voice        (saturating)
input_h  = input_demand.min(avail − CHAT_FLOOR)   floored at min(3, avail)
chat     = Constraint::Min(CHAT_FLOOR)            CHAT_FLOOR = 3
```

Crew/voice keep their `height_for` (crew collapse is cyril-lme2's scope).
With surplus, chat absorbs the remainder exactly as today — `Min(3)` and
`Min(5)` solve identically when space is free, so roomy frames don't move.

**R2 — Input owns its rows: char-wrap + cursor-follow window.**
`input::render` builds its own unicode-width-aware char-wrapped visual rows
(replacing `Paragraph::Wrap`), computes the cursor's visual row exactly, and
slides a window so the cursor row is always inside the visible content rows.
Falsifier F-A (run, passed) showed `Paragraph::scroll` offsets by post-wrap
visual rows, and ratatui's word-wrap cannot be replicated exactly from
outside for mid-word cursors — so the widget stops delegating wrap. Visual
change: long lines break mid-word (char-wrap) instead of at word boundaries;
no existing fixture pins word-wrap (verified: baseline TSV input scene never
wraps).

**R3 — Suggestions: in-flow when roomy, overlay when constrained.**
Placement predicate: in-flow (today's row below the input) iff
`avail − input_h − s_demand ≥ 5` (today's chat comfort). Otherwise the
in-flow row gets `Length(0)` and suggestions paint as an overlay anchored
directly above the input's top border, height `min(s_demand, input_top − 1)`,
never covering the toolbar. In both modes the render window becomes
`visible = min(total, MAX_VISIBLE, area.height)` with center-scroll over
*visible* — the selected `▸` row can no longer leave the screen.

**R4 — Modals: shared placement, input-protected, selection-windowed.**
One helper (extending `widgets/modal.rs`): region = rows `[1, input_top)`;
width clamp unchanged (`min(desired, width−4)`); height
`min(desired, region)`; y = legacy full-frame centering **when the legacy
rect doesn't overlap the input**, else shifted/clamped into the region.
Approval (both phases — trust items are 3 rows each), picker, hooks, and
code panels all route through it (approval/hooks/code drop their inline
copies). Approval gains a selection viewport (picker already has one from
cyril-cc5e) so a clamped popup keeps `▸` visible.

**R5 — Degenerate sizes stay panic-free.** All arithmetic saturating;
falsifier F-B (run, passed) pinned 0 fallbacks / 400 adversarial renders on
main as the baseline to preserve.

## Input shapes (step 2)

- **Frame height:** <16 (best-effort), 16 (floor), 17–23, ≥24 (roomy) —
  claims C1, C2, C6, C9, C11. **Width:** <60 best-effort (C11), 60 (C1), ≥80
  (C6/C9). Width-axis reflow beyond this: out of scope (cyril-mdbp fixed the
  known width bug; no width behavior changes here).
- **Draft:** empty / 1 line / = max rows / > max rows / one long wrapping
  line / wide Unicode (`世界`) / cursor at start·middle·end — C2, C3.
- **Suggestions:** 0 / 1 / < MAX / = MAX / > MAX × selected None·0·mid·last
  — C4; placement fits/doesn't-fit — C5, C6.
- **Overlays:** none / approval SelectOption (1..4 options) / approval
  SelectTrust (3-row items) / picker (with filter row) / hooks / code — C7,
  C8, C9.
- **Crew/voice rows:** 0 / >0 — C1 matrix includes crew>0.
- **Chat scroll:** follow (None) / browse (Some(back), incl. back≫len) — C10.

## Removed-invariant sweep (step 2b — the change is subtractive)

1. *"A widget's area equals its `height_for` whenever the solver can satisfy
   it."* Removed by R1. Only `render.rs` consumes `height_for` (grepped);
   widgets already receive arbitrary `area`s. Safe.
2. *"Suggestions always occupy in-flow rows below the input."* Removed by
   R3. Key dispatch operates on autocomplete *state*, never on geometry;
   suggestions consume no mouse events (guard chain unchanged). Covered by
   C5's operability assertion anyway.
3. *"Modals are vertically centered in the full frame."* Removed by R4. No
   code computes popup hit-regions (no mouse interaction with popups);
   visual parity where it matters is claim C9.
4. *"Input wraps at word boundaries."* Removed by R2. Property was never
   asserted (no wrapping fixture exists); accepted visual change, flagged
   for design approval (decision D1).

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| F-A | `Paragraph::scroll`+`Wrap` offsets by post-wrap visual rows (mechanism for R2's "build rows ourselves" decision) | 30-char line at width 10, scroll(1): row0 = chars 10–19 ⇒ visual; "END" ⇒ logical | rendered buffer vs the two mutually-exclusive predictions | 5m | **passed** | none needed — decision record; R2 no longer depends on Paragraph wrap |
| F-B | Adversarial state×size matrix (400 renders incl. 1×1) reaches no panic fallback on main | sweep sizes {1..200}×{1..100} × 4 states; any "Render error" text falsifies | fallback banner text (rendered only via `catch_unwind` path) | 5m | **passed** | `probe → no_fallback_size_sweep` promoted to permanent test (C11) |
| C1 | At every size ≥60×16, toolbar, status, ≥3 chat rows, and an input ≥3 rows with both borders are all present, for every state shape incl. crew>0 | render adversarial matrix; parse rows | buffer parse: `┌`/`└` rows, status text, chat row count — vs hand-computed budget arithmetic in the test comment | 30m | pending | `layout_floors_hold_across_adversarial_matrix` |
| C2 | Exactly one cursor block is visible inside the input content rect for every draft×cursor shape at 60×16 and 80×24 | S1 state (10-line draft, cursor at end) — main shows 0 cursor cells in input; new code must show 1 at the oracle-predicted cell | independent Python char-wrap re-computation (`oracle-input-wrap.py`) predicts (row, col) | 30m | pending — **fails on main by construction** | `input_cursor_always_visible` |
| C3 | The visible input window is exactly the char-wrap window containing the cursor, rows in order, no duplication | cursor at start/middle/end of a 30-line unicode draft; compare full visible content | same Python oracle emits the expected row strings | 30m | pending | `input_scroll_window_matches_oracle` |
| C4 | The selected `▸` suggestion row is inside the rendered area for every (total, selected, height) | S2b shape (10 items, selected=7, 4 rows) — main renders no `▸` | expected window from the hand formula over *effective* visible (transcribed into test as independent arithmetic) | 15m | pending — **fails on main** | `suggestion_selection_always_visible` |
| C5 | When in-flow placement would drop chat below 5 rows, the input box does not move when suggestions open, and suggestions paint above it | 60×16, 10 suggestions: input `┌` row identical open vs closed; suggestion text present in rows < input_top; status intact — main moves input from row 10 to 6 | frame-diff between open/closed renders | 20m | pending — **fails on main** | `suggestions_overlay_under_pressure` |
| C6 | When in-flow fits (80×24, ≤10 suggestions, small draft), suggestion placement and content are unchanged from today | run today's pinned fixtures + full-frame 80×24 compare against a buffer captured on main pre-change | pinned TSV baseline (commit-stamped) + saved main-frame fixture | 10m | pending (pinned tests pass on main today) | existing `suggestion_shape_matches_pinned_baseline` + new `roomy_frame_matches_main_fixture` |
| C7 | No overlay-painted cell intersects the input area at ≥60×16, for approval (both phases), picker, hooks, code × input 1-line and max-draft | render overlay vs no-overlay, diff cells, assert all diffs above input top — main fails (S4/S5 cover rows 10–11) | frame-diff + input rect from `┌`/`└` parse | 45m | pending — **fails on main** | `modals_never_cover_input` |
| C8 | With the popup clamped, the selected approval option / trust item stays visible | approval, 3 options, region 5 rows, selected=last: `▸`+label in buffer — main clips it | buffer scan; expected window arithmetic inline | 30m | pending — **fails on main** | `modal_selection_visible_when_clamped` |
| C9 | Wherever the legacy centered rect would NOT overlap the input, the new placement equals it exactly | grid sweep (area × desired × input_top) comparing new placement to transcribed legacy arithmetic (cc5e-style parity oracle) | verbatim transcription of today's `centered()`/approval inline math | 20m | pending | `modal_placement_parity_when_unconstrained` |
| C10 | Browse mode at 60×16: scroll-back reaches the oldest message and the scrollbar renders, with chat ≥3 rows | `chat_scroll_back = Some(large)` at 60×16: first message text visible in chat rows | expected top line computed from the message list independently of chat.rs math | 15m | pending (expected to pass on main — pins the floor) | `browse_mode_usable_at_floor` |
| C11 | No state×size (down to 1×1) reaches the panic fallback after the change | F-B sweep re-run on new code | fallback banner text | 5m | pending (baseline passed as F-B) | `no_fallback_size_sweep` |

Cheapest falsifiers (F-A, F-B) ran before this document was presented; both
passed. Claims marked **fails on main** double as bug-class-embedding
fences: pre-change code fails them, post-change code must pass.

## Negative space (what this deliberately does NOT do)

1. No crew-panel responsive collapse (cyril-lme2), no hooks-table column
   work (cyril-uw20), no gauge redesign (cyril-9ode), no shortcut overlay
   (cyril-91iu) — this issue only guarantees their *allocation* can't break
   the floors.
2. No input editing features: no logical-line navigation, no undo, no key
   rebinding — rendering and placement only (cyril-4vvw owns input editing;
   input.rs churn kept minimal for it).
3. No width-axis reflow changes; no wrapping changes in chat/markdown.
4. Below 60×16 the only guarantee is no-panic/no-fallback (C11) — floors
   are best-effort there by issue definition (settled rationale: the issue
   names 60×16 as the supported floor).
5. No mouse interaction with overlays, no layout persistence, no config
   surface for floors.

## Open decisions — RESOLVED (user approval 2026-07-15)

All five decided as recommended: D1 char-wrap, D2 all four overlays through
the shared helper, D3 clamp+window the modal (input never covered), D4
overlay predicate at chat<5, D5 chat floor 3 under pressure. Design approved
verbatim ("Approve, proceed").

- **D1 (input wrap):** switch input to char-wrap (recommended; exact cursor
  math, no fixture pins word-wrap) vs re-derive word-wrap (rejected:
  falsifier F-A + mid-word cursor makes exact replication fragile).
- **D2 (modal scope):** route hooks/code panels through the same placement
  helper (recommended: one rule, no special cases) vs AC-minimum
  (approval+picker only).
- **D3 (modal vs grown input):** clamp+window the modal in the region above
  the input (recommended) vs collapsing the input to its floor while a modal
  is open. Corner: max draft at 60×16 leaves a 3-row popup — operable via
  the selection window, but tight.
- **D4 (overlay predicate):** overlay-above kicks in whenever chat would
  drop below 5 (recommended). Note: at 80×24 the max-draft+10-suggestions
  combo also switches to overlay — a behavior change above the floor, and
  an improvement (today the solver crushes both).
- **D5 (chat floor):** under pressure chat floor is 3 (AC minimum,
  recommended — gives the draft more rows at 60×16) vs keeping 5.
