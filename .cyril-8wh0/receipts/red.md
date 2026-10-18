# cyril-8wh0 red receipts

## 1. Text content preview (slice 1, before `b8a7a01e`'s classifier change)

State: base `8108e20d` with the new tests added and the `PreviewBody::Content`
variant declared, so they compile, but with no producer: `preview_body()` was
unchanged.

Command: `cargo nextest run -p cyril-ui --no-fail-fast -E 'test(text_content) | test(prefers_text_content)'`

Result: 5 run, 0 passed, 5 failed.

- `preview_body_prefers_text_content::over_malformed_raw_input`: got
  `Malformed { raw_input_shape: "object" }`. This is the live symptom: the
  overlay shows "Preview unavailable".
- `preview_body_prefers_text_content::over_raw_input_text`: got `Text("raw")`.
- `preview_body_prefers_text_content::without_raw_input`: got `Absent`.
- `text_content_renders_in_the_preview`: "the Text block must be shown".
- `text_content_preview_is_bounded`: "fifth line missing".

## 2. Typed plan in the overlay (slice 2, before `preview_lines` read the plan)

State: types, parser, `ApprovalState.workflow_plan` and the renderer module
in place, but `preview_lines` did not yet consult the plan.

Command: `cargo nextest run -p cyril-ui --no-fail-fast -E 'test(workflow_plan) | test(plan_without_steps)'`

Result: 4 run, 1 passed, 3 failed. `workflow_plan_renders_its_step_list`,
`workflow_plan_renders_nested_nodes` and `workflow_plan_is_bounded` failed.
`plan_without_steps_falls_back_to_the_text` passed: it pins the fallback,
which held already.

## Wiring

`PermissionRequest.workflow_plan` did not exist on base, so the bridge
contract test (`kas_workflow_plan_reaches_the_app_typed`) and the parser
tests are new-surface fences. They have no runtime red. Their evidence is the
fixture equality (`live_launch_plan_converts`: every field of the live
capture) and the end-to-end live run (`live.md`).

## Review repairs (`3a66e2c3`)

These fences arrived with their repairs and were not run red separately.
Each one pins a counterexample the review demonstrated by reading the code:

- a bidi override in a step id reaches the terminal;
- blank Text gives an empty preview;
- an edit's "Compared with…" line is never shown;
- the Text fallback is cut to 5 rows;
- `wrap_words` exceeds its width.

## 3. Review round 1: the comparison line survives an overflowing tree

State: head `b6bcae11` plus the new test only (no production change).

Command: `cargo nextest run -p cyril-ui --no-fail-fast -E 'test(keeps_the_comparison) | test(strips_bidi)'`

Result: 2 run, 1 passed, 1 failed.

- `overflowing_edit_plan_keeps_the_comparison_line`: FAIL, "the comparison
  line was cut". A 15-step edit plan rendered header, keeps, step-01..step-10
  and `...7 more lines`; "Compared with the replaced steps: removed
  'old-step'" was among the cut rows.
- `workflow_plan_strips_bidi_and_control_characters` (with its strengthened,
  pre-render assertion): PASS on the pre-fix head as well. It is a coverage
  repair, not a defect fix.

After the fix both pass. The rendering is pinned by insta
`8wh0_workflow_plan_overflow_keeps_comparison_90x50`: header, keeps,
step-01..step-07, `...8 more lines`, then the 2-row comparison.
