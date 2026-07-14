# cyril-cc5e — prove-it-prototype findings (2026-07-14)

## Smallest question

> Given a picker with N filtered options and selection at index k, which
> option labels does the render actually draw, and is the selection marker
> (▸) on screen?

## Probe

`probe_cc5e.rs` (run as `crates/cyril-ui/tests/probe_cc5e.rs`, source archived
here) — renders `widgets::picker::render` into a ratatui `TestBackend`,
extracts the character buffer, reports the drawn label set + marker presence.
Navigation reachability is exercised through the real `UiState` methods
(`show_picker` + `picker_select_next`), not synthetic state.

Output: `probe-output.txt`.

## Oracle

`oracle.py` — hand-derived arithmetic of the popup geometry (width/height
clamps, `take(15)` window, border clip, description-line insertion) computed
from reading `picker.rs` constants, **no ratatui involved**. Output:
`oracle-output.txt`.

**Agreement: byte-identical on all four scenarios** (`diff` clean).

```
SCENARIO A-control-80x24  w=80 h=24 n=30 sel=5  marker=true  drawn=opt-00..opt-14
SCENARIO B-deep-sel-80x24 w=80 h=24 n=30 sel=20 marker=false drawn=opt-00..opt-14
SCENARIO C-floor-60x16    w=60 h=16 n=15 sel=14 marker=false drawn=opt-00..opt-07
SCENARIO D-floor-top-60x16 w=60 h=16 n=15 sel=0 marker=true  drawn=opt-00..opt-06
STATE reachable-selected=20 (20 presses, 30 options)
```

## What I learned (that I didn't know before)

**The selection is hidden by two independent clipping mechanisms, not one:**
the `take(15)` hard window (B: invisible even on a full 80×24) *and* the
Paragraph border clip after the height clamp (C: invisible at 60×16 within
the first 15) — and the selected item's inline description line displaces one
option row (D draws 7 rows where C draws 8), so the fix must window over
**variable-height rows**, not a uniform row grid.

Second finding (bounds the design): the state machine is already sound —
`picker_select_next` bounds by `filtered_indices.len()` and
`refilter_picker` clamps `selected` on every filter keystroke. No state-side
fix is needed; the defect is entirely render-side windowing.

## Hard gate

- [x] Probe written, runs against the real codebase (real widget + real UiState)
- [x] Oracle defined, produces output (independent arithmetic, no ratatui)
- [x] Probe and oracle agree on a non-trivial slice (4 scenarios, byte-identical)
- [x] One-sentence learning recorded (above)
