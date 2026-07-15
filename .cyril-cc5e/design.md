# cyril-cc5e — Design: keep picker selection visible

## Purpose

Fix the picker overlay so keyboard navigation always keeps the selected option
visible (probe: today it is hidden by BOTH a `take(15)` window pinned to index
0 and the height-clamp border clip), make overflow visible via a display-only
scrollbar, and extract the centered-popup geometry into a shared helper with
the picker as its first consumer.

## Architecture

All changes live in `cyril-ui` render code. The state machine is untouched —
the probe proved `picker_select_next/prev` and `refilter_picker` already
maintain `selected < filtered_indices.len()`.

1. **`widgets/modal.rs` (new)** — `pub fn centered(area: Rect, desired_width:
   u16, desired_height: u16) -> Rect`: the centering + clamp arithmetic
   currently inlined in `picker.rs:8-14`, behavior-identical. Picker is the
   first consumer; approval/hooks/code panels migrate under cyril-a14l /
   cyril-uw20 (verified open).
2. **`widgets/picker.rs`** — replace `take(visible)` with a selection-centered
   window; add a `Scrollbar` when the list overflows; reserve the description
   row deterministically.

### Core formula (validated by `window-model-check.py`, 57,400 cases, 0 violations)

```
desired_rows = min(n, 15)                     # visible cap kept (status quo)
desc_reserve = 1 if ANY option has a description else 0   # no height jitter
height  = min(desired_rows + desc_reserve + 4, area.h - 4)
inner   = height - 2                          # borders
r_opts  = max(inner - 2 - desc_reserve, 0)    # minus filter + blank lines
rows    = min(n, r_opts)
start   = clamp(selected - rows/2, 0, n - rows)   # saturating in Rust
draw options[start .. start+rows]; desc line after selected if present
```

`desc_reserve` keys off *any option has a description* (not "selected has
one") so popup height is stable while navigating — selection-dependent
reservation would make the popup jitter and the window math self-referential.

## Input shapes (each covered by ≥1 claim)

| Shape | Values covered | Claim |
|---|---|---|
| `filtered_indices.len()` n | 0 / 1 / < rows / = rows / > rows / > 15 | C7, C6, C1, C2 |
| `selected` k | 0 / mid / len−1 / inside first window / far beyond | C1, C2, C5 |
| `description` | all-None / Some on selected / Some on non-selected | C3, C6 |
| `group` | Some / None | cosmetic — covered incidentally by C6 fixture |
| terminal | 60×16 floor / 80×24 / large (200×50) / below floor (20×8, 5×5) | C5, C1, C6, C10 |
| filter | empty / narrowing / no-match | C7 + C1 (refilter clamps k, probe-proven) |

Out of scope shapes: Unicode/width-overflow in labels and descriptions —
single-line width-clipping is status quo and unchanged by this design
(settled rationale, not deferred work; row *content* evolves under
cyril-lxuo, verified open).

## Subtractive sweep (step 2b)

The change is render-internal and additive-in-effect: the only "constraint"
removed is "options past index 15 are never drawn," which no code observes
(rendering is a leaf; no state reads the drawn set). No serialization point,
guard, ordering, or uniqueness property is removed. One sentence, per skill:
**no removed-invariant claims required.** The state machine is deliberately
untouched (C9 fences that).

## Claims

1. **C1 — visibility invariant.** For every (n≥1, k, terminal ≥ 60×16), the
   selected option's label and ▸ marker are drawn inside the popup.
2. **C2 — window fill.** Drawn options are a contiguous slice of
   `filtered_indices` containing `selected`, of length exactly `min(n,
   r_opts)` — no blank list rows while options overflow, no overdraw.
3. **C3 — description containment + height stability.** When the selected
   option has a description it is drawn inside the popup consuming exactly
   the reserved row, and popup height is constant across all k for a fixed
   option set.
4. **C4 — scrollbar iff overflow.** A scrollbar is rendered iff n > rows;
   it consumes no key events (display-only).
5. **C5 — floor navigability.** At exactly 60×16 with 15 described options,
   stepping k = 0→14 through real `UiState` key methods keeps the selection
   visible at every step, and each step changes the highlighted label.
6. **C6 — no-overflow stability.** When all options fit, the window starts at
   0, no scrollbar renders, and the popup shrinks to content height (today's
   look preserved).
7. **C7 — empty result.** n = 0 renders filter line + frame with no marker
   and no panic.
8. **C8 — geometry parity.** `modal::centered` returns byte-identical `Rect`s
   to the current inline picker arithmetic for all (area, w, h) in a sweep.
9. **C9 — state untouched.** `PickerState` and all `picker_*` state methods
   are behaviorally unchanged (existing state tests pass unmodified; diff
   scope is render-side only).
10. **C10 — degenerate safety.** Below-floor terminals (20×8, 5×5, 0×0)
    render without panic; all arithmetic saturates.

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| 1 | C1 visibility | render sweep (n,k,size), assert ▸ + label drawn | oracle.py-style independent arithmetic of expected window, diffed line-by-line | 30m | pending (formula core **passed** via model check) | `picker_viewport::selection_always_visible` |
| 2 | C2 window fill | same sweep, assert drawn set == expected contiguous slice | same independent arithmetic | incl. | pending (model check **passed**) | `picker_viewport::window_contiguous_fill` |
| 3 | C3 desc + stable height | render all k for fixed set, record height + desc row | height must equal formula constant; desc text inside popup rect | 15m | pending | `picker_viewport::description_contained_height_stable` |
| 4 | C4 scrollbar iff | render n≤rows and n>rows, grep buffer for scrollbar glyphs | glyph presence vs arithmetic `n > rows`; key path: app.rs picker arm diff = empty | 10m | pending | `picker_viewport::scrollbar_iff_overflow` |
| 5 | C5 floor nav | 60×16, drive UiState 14 next-presses, render each | per-step marker row must exist; labels advance per arithmetic | 15m | pending | `picker_viewport::floor_60x16_full_walk` |
| 6 | C6 no-overflow | render n=5 at 80×24, diff drawn set + height vs today's probe A-analog | pre-change probe output (committed) as ground truth | 10m | pending | `picker_viewport::small_list_layout_unchanged` |
| 7 | C7 empty | render n=0 | process exit (no panic) + no ▸ in buffer | 5m | pending | `picker_viewport::empty_filter_no_panic` |
| 8 | C8 parity | sweep (area,w,h), compare helper Rect to old formula reimplemented in the test | old arithmetic transcribed verbatim from git blame of picker.rs:8-14 | 10m | pending | `modal::centered_parity_sweep` |
| 9 | C9 state | run existing cyril-ui state tests unmodified | pre-existing test suite (written before this design) | 2m | pending | existing `state::tests` picker suite |
| 10 | C10 degenerate | render at 20×8, 5×5, 0×0 | no panic (process-level) | 5m | pending | `picker_viewport::degenerate_sizes_no_panic` |
| — | core formula | **exhaustive model check, 57,400 cases** | independent Python arithmetic (no ratatui, no Rust) | **5m** | **PASSED** | superseded by fences 1/2/5 at build time |

Non-vacuity (buggy implementation each fence catches): C1/C2 — today's
`take(15)` code fails both (probe B/C are the witnesses); C2 — unclamped
`start = k - rows/2` leaves blank tail rows near the end of the list; C3 —
reserving the desc row only when the *selected* option has one makes height
jitter across k; C4 — unconditional scrollbar fails the ≤rows case; C5 —
center-only-no-clamp math hides k=14 at the floor; C6 — fixed 15-row popup
fails shrink-to-content; C7 — `n-1` on usize 0 panics; C8 — any rounding
change in centering (e.g. `(w-width+1)/2`) shifts the Rect; C9 — "fixing"
visibility by clamping `selected` in render mutates confirm semantics and
fails existing state tests; C10 — non-saturating `h-4` underflows.

Distinctness: every fence is a separately named test; the model check prints
V1/V2/V3 per violation with full parameters.

## Negative space (deliberately NOT in this change)

1. **No mouse support / clickable scrollbar** — scrollbar is display-only per
   the AC; cyril input remains keyboard-first.
2. **No color/style changes** — hardcoded `Color::Rgb(50,50,70)` etc. stay;
   semantic-color migration of modals is cyril-nrnq (verified open).
3. **No other modal migrates** — approval/hooks/code-panel geometry is
   untouched; they adopt `modal::centered` under cyril-a14l / cyril-uw20
   (both verified open). Picker is deliberately the *only* consumer here.
4. **No description wrapping** — descriptions stay single-line and
   width-clipped (status quo; settled rationale — nothing in the AC or
   tracker demands wrapping; picker row content evolves under cyril-lxuo).
5. **No state-machine changes** — probe proved selection/filter invariants
   already hold; `PickerState` fields are unchanged.
6. **No visible-cap change** — the 15-row maximum on large terminals stays
   (status quo look; growing the cap is a product decision nobody has asked
   for — settled rationale, no trigger condition).

## Open decisions (for design approval)

1. **Scroll posture: centered-follow (recommended).** Selection stays
   centered once past mid-window (`start = clamp(k - rows/2, …)`), stateless
   — no `scroll_offset` field to maintain across filter churn. Alternative:
   minimal-scroll (selection walks to the window edge before the window
   moves), which needs persistent offset state in `PickerState` and
   filter-change reconciliation. Centered-follow is what the model check
   validated.
2. **Description placement: inline under the selected row (recommended),**
   preserving today's look, with the row reserved whenever any option has a
   description. Alternative: fixed footer line at the popup bottom.
3. **Helper location/name: `cyril-ui/src/widgets/modal.rs`,
   `modal::centered(area, w, h) -> Rect` (recommended).**

## Approval

Approved by dwalleck 2026-07-14 (in-session): scroll posture = centered-follow, description = inline under selection, helper = widgets/modal.rs. Cheapest falsifier passed (57,400 cases). Proceed to budgeted-plan.
