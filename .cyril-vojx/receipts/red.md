# cyril-vojx red receipts

Base revision: `8108e20d` (origin/main at branch point). Production code
unmodified; only the new tests were applied (the test files of `caf2d8b4`
checked out onto a scratch branch at `8108e20d`).

## App key path (crates/cyril/src/app.rs)

Command: `cargo nextest run -p cyril --no-fail-fast -E 'test(vojx) | test(acts_only_with)'`

Result: 5 run, 1 passed, 4 failed.

- `approval_enter_acts_only_with_selection_and_path_on_screen::{with_path,without_path}`:
  FAIL, `Enter answered a hidden approval: height=4 … inner=None`. The frame
  draws no popup at all (only `No session` and blank rows), yet Enter
  answered `allow`.
- `reason_enter_acts_only_with_editor_and_path_on_screen::{with_path,without_path}`:
  FAIL, `Enter submitted a hidden reason: height=4 … inner=None`.
- `vojx_sweep_covers_every_cramped_popup_height`: PASS. It is the coverage
  check that the sweep reaches no popup, inner 0 and inner 1..=8, so it holds
  on base too.

## UiState and frame layout (crates/cyril-ui)

Command: `cargo nextest run -p cyril-ui --no-fail-fast -E 'test(later_pages) | test(input_top_is_the_drawn)'`

Result: 4 run, 0 passed, 4 failed.

- `later_pages_confirm_only_what_is_drawn::{scope_page,folder_editor,trust_page}`:
  FAIL at `height=4`. Enter answered with no popup drawn.
- `input_top_is_the_drawn_input_row_at_every_height`: FAIL,
  `height=6: input_top disagrees with the drawn input box`, `left: 2`. The
  summed row budget put the input two rows above the drawn box.

## Found during green

With the guard in place but `input_top` still summing `FrameRows`, the App
sweeps failed at `height=7 inner=Some(1)`: `Enter ignored an approval shown
with its path`. The popup drew `▸ Allow`, but the guard placed the input one
row higher than the frame did. That is the defect the `input_top` fence above
pins. It is fixed by `frame_areas`.
