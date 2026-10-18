# Route: cyril-vojx

Change: when the approval popup is clamped to inner height 0 it draws only
its border, yet Enter still confirmed `options[selected]`
(`UiState::approval_confirm`). A user could approve or reject a tool call
with nothing on screen describing it. Make every confirm key act only when
the selected option is on screen beside the call's target path.
Date: 2026-10-18

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | None open. Rendering and key handling are fully reproducible through `App::draw_frame` on a `TestBackend` and `App::handle_key` (`receipts/red.md`). | no |
| 2 | Structural module shape | Owners are unchanged. The page geometry stays in `widgets/approval.rs`, which gains `OptionPage`/`ReasonPage` and `trust_popup`/`scope_popup`/`folder_popup`. These are the computations the renderers already did inline, extracted so the renderer and the new `pub(crate) confirm_on_screen` read one source. `render.rs` gains a private `frame_areas` (the `Layout::vertical` that `draw_inner` already built), now shared by `draw_inner` and `input_top`. `UiState::approval_confirm` keeps its signature and gains the guard. No public interface, dependency direction or ownership changes. | no |
| 3 | Production-scale risk | The guard runs once per confirm key: one popup placement and one preview build. Negligible. | no |
| 4 | Explicit behavior | Source: the issue acceptance. (1) At every popup height, Enter has no effect unless the selected option is drawn and, when the call has a target path, the path is drawn too (on its own row, or inline at height 1). (2) Esc still closes and answers Cancel at every height. (3) The reject-with-reason page submits on Enter only while its editor row is drawn and, when the call has a path, the path row is drawn too. Below that height Enter does nothing and the draft survives. (4) The scope, trust and folder pages that follow an "Always allow" pick answer Enter only while their selection (or folder editor) is drawn. These pages never show the path; the operator saw it on the option page that led there. (5) An option-page selection that names no option still answers Cancel, exactly as Esc does. It decides nothing the user needs to see, and existing fences pin it. | yes |

## Design choice: recompute the frame geometry, do not cache the last frame

The guard has to agree with the frame the user is looking at. Two options:

- **Cache what the last render drew** (for example a `Cell<bool>` written by
  the renderer). Rejected: the renderer gets `&dyn TuiState` and must not
  mutate state (the TuiState contract), and UiState would come to depend on a
  side effect of drawing.
- **Recompute from state.** Chosen. `UiState` already records the terminal
  size on every draw (`App::draw_frame`) and every `Event::Resize`, and
  `/powers` already derives its keyboard scroll bound from that size through
  `render::powers_window` (review finding 8, cyril-v19o round 3). The approval
  guard follows the same pattern: `widgets::approval::confirm_on_screen(state)`
  builds the area from `terminal_size()`, takes the input row from
  `render::input_top`, and runs the same `OptionPage`/`ReasonPage`/popup
  computation the renderer draws from. The guard is a pure function of
  UiState fields, with no cached render output.

Building the guard exposed a second defect. `render::input_top` summed the
`FrameRows` budget, but on a frame shorter than the chat floor plus the input
floor the layout solver keeps `Min(CHAT_FLOOR)` and squeezes the input
lower. At heights 6–7 the sum put the input one or two rows above where it is
drawn. A guard built on that sum disagreed with the popup at 80x7: the popup
showed `▸ Allow` and the guard said it was hidden. `input_top` now reads the
same `frame_areas` layout `draw_inner` draws with. `FrameRows::chat`, which
was used only by that sum, is removed.

The reject-with-reason layout is unchanged. At inner heights 1–2 a call with
a path shows only the editor (and the limit line), so Enter is inert there
and Esc backs out to the option page. The option page shows the path at those
heights. Giving the path priority over the editor chrome on the reason page
would make those heights submittable, but it changes the cyril-rcub reason
layout and its snapshot, so it is left as a follow-up question in the PR.

## Selected route

Local. The behavior is explicit, the change stays inside the existing widget
and state owners, and no premise is open.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md, evidence.md, design.md, plan.md | — | N/A — Local route (T1–T4 above) |

## Terminal criterion

- Red: the new fences fail on the base revision (`receipts/red.md`).
- Green: the fences pass with the full workspace suite.
- Every ci.yml lane passes (`receipts/gate.md`), and GitHub CI is green.
- Independent review is satisfied (recorded below).

## Results

- **Red:** PASS as a red (`receipts/red.md`). On base `8108e20d` the four App
  sweeps fail at the first height, and the three later-page sweeps and the
  `input_top` fence fail.
- **Green and gate:** every ci.yml lane passes (`receipts/gate.md`).
- **Independent review:** a rust-code-reviewer subagent did a read-only
  review of `8108e20d..caf2d8b4`. It found nothing blocking: the option and
  reason guards match the drawn frame at every height, and no confirm path
  bypasses `approval_confirm`. Its findings and their dispositions:
  - **should-fix, the folder page wraps its status line.** A folder longer
    than about 7 characters wraps "Allows writes in … for this session only."
    to 2 rows. At inner height 2 that pushed the editor out of the popup while
    the guard said it was drawn. Fixed in `3b933531`: a shared `FolderPage`
    wraps the status itself (no `Paragraph` wrap) and draws it only when all
    of its rows fit above the editor. Fenced by `folder_editor_long_folder`,
    which is red on `caf2d8b4` at height 11.
  - **should-fix, the trust page (v2) wraps a long pattern list**, which
    pushes the selected tier below the popup. Fixed in `3b933531`: label and
    pattern rows are cut to one row each and marked with `…`, so every tier
    takes exactly the 3 rows its window reserves. Fenced by
    `trust_page_long_pattern_list`, which is red on `caf2d8b4` at height 17.
  - **nit, the sweeps did not prove both outcomes occur.** Fixed: every sweep
    now asserts it saw both an answered and an ignored height. The reason
    sweep also asserts that some heights draw the editor without the path
    exactly when the call has one.
  - **nit, a missing blank line.** Fixed.
  - **nit, the guard ignores horizontal clipping.** Kept as a known limit. At
    terminal widths of 5–8 columns, or with a very long option label at
    height 1, the path can show as little as `…`, and the guard counts a
    drawn path row as shown. Requiring a minimum visible path width is a
    separate policy change.
  - **nit, sweeps run at 80 columns only.** Width is covered by the cyril-rcub
    sweeps; the long-row cases above cover the wrap failure class.
  - **nit, Enter is inert on the reason page at inner 1–2 with a path.** This
    is the documented design choice above, and it is flagged in the PR.
