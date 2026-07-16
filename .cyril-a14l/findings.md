# cyril-a14l — prove-it-prototype findings

**Feature:** keep chat and input usable at the supported 60×16 floor.
**Probe:** `crates/cyril-ui/src/probe_a14l.rs` (temporary `#[cfg(test)]`
module) — renders the production `render::draw()` at 60×16 via `TestBackend`
across six states; output pinned in `probe-testbackend-output.txt`.
**Oracle:** `oracle-pty.py` — the REAL `cyril` binary (inert
`--agent-command sleep 300`) on a real 60×16 pty, screen reconstructed by
`pyte` (independent VT100 emulator); bracketed-paste for the draft, real
keystrokes for `@` completion. Outputs in `oracle-draft-output.txt`,
`oracle-at-output.txt`. A third mechanism — hand arithmetic from the widget
sources — oracles the overlay geometry.

## Probe ↔ oracle agreement

| Slice | Probe (TestBackend+Mock) | Oracle (real binary, pty+pyte) | Agree |
|---|---|---|---|
| S1 big draft | input `┌`=row 6, `└`=row 14; draft-1..7 visible; cursor row clipped; status row 15 | identical rows, identical visible lines | ✅ exact |
| S2 `@` autocomplete | input rows 6–10; 4 of 10 suggestion rows (11–14); status row 15 | identical | ✅ exact |
| S4 approval overlay | popup rows 3–11, covers input rows 10–11 | hand arithmetic: w=min(60,56)=56, h=3+6=9, y=(16−9)/2=3 → rows 3–11 | ✅ exact |
| S5 picker overlay | popup rows 4–11, covers input top border + first content row | `centered()` arithmetic (cc5e-pinned) | ✅ exact |

First `at` oracle run DISAGREED (no suggestions at all): root cause was the
oracle environment, not the app — `FileCompleter::load` is `git ls-files`
only, and the scratch dir wasn't a git repo. Filed **cyril-2mfa**; re-run in
a git dir agreed exactly.

## Facts established (the design must stand on these)

1. **The solver already protects chat and the bars.** At 60×16 the root
   `Layout::vertical` honors chat `Min(5)`, toolbar `Length(1)`, status
   `Length(1)` in every probed state. AC-"three chat rows" is not currently
   at risk from the solver — the *casualties are the input and suggestions*.
2. **The input is bottom-clipped with no cursor-follow scroll.** With a
   10-line draft the input gets 9 of its requested 12 rows and shows
   draft-1..7; the cursor (end of draft) is invisible — the user types
   blind. `input.rs`'s doc comment ("content beyond this scrolls within the
   box") is fiction: `render()` never calls `.scroll()`. By the same
   mechanism a >10-line draft hides the cursor at ANY terminal size
   (height_for clamps at 12 total rows).
3. **Suggestions render a fixed 10-item window into whatever they get.**
   `suggestions::render` computes its window from `MAX_VISIBLE=10` with no
   knowledge of the actual area (4 rows at 60×16); with selection index ≥ 4
   the `▸` marker is off-screen (probe S2b) — keyboard operation goes blind.
4. **Deficit distribution among `Length` constraints is roughly
   proportional.** S3 (input demands 12, suggestions 10, only 9 rows free):
   input→5, suggestions→4. Nothing guarantees the input's 2 border rows +
   1 content row survive at more extreme deficits — allocation must become
   explicit, not solver-emergent.
5. **Centered modals cover the input.** Approval (rows 3–11) and picker
   (rows 4–11) both overlap input rows 10–11 at 60×16, hiding the draft's
   first row and top border. `approval.rs` duplicates the legacy geometry
   inline instead of using `modal::centered()`.
6. **Suggestions are in-flow, below the input.** While open they reflow the
   frame (chat 9→5 rows). AC-2 wants them anchored above the input as a
   non-permanent surface under height pressure.

## What I learned (one sentence)

At 60×16 the constraint solver quietly sacrifices exactly the two surfaces
the user is interacting with — the input (bottom-clipped, cursor invisible,
its claimed scrolling never implemented) and the suggestion list (fixed
10-row window into 4 rows, selection marker walks off-screen) — while
centered modals cover what's left of the input; none of this is visible at
80×24, and the real-binary pty oracle reproduced every row of it.

## Cleanup note

`probe_a14l.rs` is probe scaffolding, not the feature; the checkpointed
build replaces it with real regression fences and removes the module.
