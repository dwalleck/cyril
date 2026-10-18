# Design: cyril-8wh0

## Route and inputs

See `route.md` (Structural). Behavior source: the issue acceptance. Wire
evidence: the fixture `crates/cyril-core/tests/fixtures/kas_permission/workflow-plan-2.32.0.json`
(two live captures and one synthetic full-shape plan, provenance inside it)
and the installed KAS 0.66.43 bundle.

## Placement

```text
convert/kas.rs::permission_workflow_plan(args)          // _meta.kiro.workflowPlan → typed
  convert/kas/workflow_plan.rs::parse(value, tool_call_id)
domain_mediator::handle_permission → PermissionRequest.workflow_plan (Box; KAS only)
UiState::show_approval → ApprovalState.workflow_plan
widgets/approval.rs::preview_lines
  plan with steps → widgets/workflow_plan.rs::plan_lines(plan, comparison, width)
  else TrackedToolCall::preview_body(): Diff > Content(Text) > raw text > Absent/Malformed
```

- **Types:** `cyril_core::types::WorkflowPlan` and its nodes model every field
  the bundle emits: plan `{workflowId, nodeId?, kind, workflowRef,
  keptStepIds, steps, omitted?}`. Step nodes add `{agentName, modelId?,
  modelSource?, effortLevel?, effortSource?, task?{text,truncated},
  artifacts?, completion?}`. Sequence `{steps, includedFrom?}`, repeat
  `{steps, maxIterations, onMaxIterations, stopCondition? | stopWhen?}`,
  parallel `{branches, joinPolicy}` and watch `{agentName (handler),
  idleTimeoutSec?, config?}` follow. Closed vocabularies use the existing
  `workflow_enum!`.
- **Unknown versus malformed:** a field the parser does not know is named
  once at debug level (with `tool_call_id`) and ignored. A wrong shape, a
  missing required field or an unknown enum value drops the plan with one
  warning naming the JSON path. The overlay then falls back to the Text.
- **Text versus plan:** the typed plan replaces KAS's Text rendering of the
  same plan, except for the one line only the Text carries: "Compared with
  the replaced steps: …" (what an edit removes). That line is drawn under
  the plan.
- **Untrusted strings:** recipe strings are model output, and
  `workflowPlan` carries them raw. The renderer cleans them as KAS cleans
  its own text (`Ay`/`D_o`): control and bidi formatting characters become
  spaces, whitespace collapses, and values are capped at 160 characters
  (600 for writes and config lines).
- **Size:** `PermissionRequest.workflow_plan` is boxed (clippy
  `large_enum_variant` on `ReplayFrame`).

## Claims / Falsification

| ID | Claim and covered cases | Falsifier / verification | Regression coverage | Status |
|---|---|---|---|---|
| C1 | A Text content block shows in the preview instead of "Preview unavailable". It is bounded to 5 rows and wraps rather than clipping. A diff still wins. Blank Text falls through to raw text or absent. | `traits::preview_body_prefers_text_content` (3 cases), `blank_text_content_falls_through` (2), `approval::text_content_renders_in_the_preview` + insta `8wh0_text_content_preview_80x24`, `text_content_preview_is_bounded` | same | PASS (red on base: `receipts/red.md` §1) |
| C2 | `workflowPlan` converts field for field from the live captures (`launch`, `replace`). The synthetic full shape converts with no unknown fields. Both `omitted` forms convert. | `convert::kas::workflow_plan::tests::{live_launch_plan_converts, live_replace_plan_converts, every_bundle_field_is_modelled, omitted_plans_convert}` | same | PASS |
| C3 | Unknown fields: debug log naming each path, plan kept. Malformed plans: one warning, plan dropped (6 shapes, exact messages). | `unknown_fields_are_ignored_with_a_debug_log`, `malformed_plans_are_dropped_with_a_warning` | same | PASS |
| C4 | Over the real bridge, the plan reaches the App typed on `PermissionRequest`. | `bridge::tests::current_runtime_contract::workflow_plan::kas_workflow_plan_reaches_the_app_typed` | same | PASS |
| C5 | The overlay renders the plan's step list: header, kept steps, each node with agent, model and effort, sources, details, nesting, and the edit comparison. It takes at most 12 rows, markers included, and the options stay on screen. When the tree overflows, the tree is cut and marked; the comparison line keeps reserved rows (up to 4). | `approval::workflow_plan_renders_its_step_list` + insta `8wh0_workflow_plan_80x24`, `workflow_plan_renders_nested_nodes` + insta, `workflow_plan_renders_step_details`, `workflow_plan_shows_what_an_edit_removes`, `overflowing_edit_plan_keeps_the_comparison_line` + insta `8wh0_workflow_plan_overflow_keeps_comparison_90x50`, `workflow_plan_is_bounded`, `workflow_plan::tests::cap_rows_marks_what_it_cuts`, `plan_without_steps_falls_back_to_the_text`, `plan_text_fallback_gets_the_plan_budget` | same | PASS (red before wiring: `receipts/red.md` §2; comparison reservation red: §3) |
| C6 | No control or bidi formatting character from a recipe string reaches the terminal. | `workflow_plan::tests::clean_strips_controls_and_bidi`; `approval::workflow_plan_strips_bidi_and_control_characters` asserts on the rows the widget hands ratatui (before ratatui drops control graphemes itself) and on the space substitution on screen | same | PASS |
| C7 | The #176 path guarantees and the vojx-independent layout still hold, because the plan is ordinary preview body rows. | the existing cyril-rcub sweeps (`option_visible_implies_path_visible_at_every_height`, `option_layout_never_shows_an_option_without_the_path`) pass unchanged | same | PASS |
| C8 | Live (KAS 0.66.43): a client-launched creator run's plan approval shows the plan in cyril's overlay before any step runs. | `receipts/live.md` | — | PASS |

## Slices

1. `b8a7a01e`: the Text content preview (C1).
2. `a6cb02d2` + `03465a72`: the typed plan, wiring and renderer (C2–C5, C7),
   then boxing the field.
3. `3a66e2c3`: the review repairs (C6, the comparison line, blank Text, the
   plan budget for the Text fallback, the word-wrap width, the doc and log
   context).

## Non-goals and future work

- `pending_interaction` `session_info_update` also carries the plan. It
  still falls through the KAS catch-all, tracked by cyril-58uv; the approval
  itself is a `request_permission`, which this change renders.
- Enabling the workflow gate and `/workflow new` are cyril-sm36 and
  cyril-0asq. This change is their prerequisite.

## Review round 1 (cross-model review of #180 at `b6bcae11`)

Verdict: APPROVE, with one Medium to fix before merge. Dispositions:

- **Medium, approval completeness:** the plan was wrapped and then cut at 12
  rows, so for an edit or replacement whose tree overflowed, the comparison
  line (the only line naming removed steps) disappeared behind "...N more
  lines". KAS reserves room for it (`R_o`). Fixed: `plan_lines` reserves the
  comparison's rows (at most `MAX_COMPARISON_ROWS` = 4) and cuts only the
  tree, marking the cut (`cap_rows`). The plan body now never exceeds 12
  rows including markers; before, it was 12 plus a marker. Fenced by
  `overflowing_edit_plan_keeps_the_comparison_line`, red before the fix
  (`receipts/red.md` §3). `workflow_plan_is_bounded` was updated
  deliberately for the marker now taking a row of the budget. New snapshot:
  `8wh0_workflow_plan_overflow_keeps_comparison_90x50`; the existing plan
  snapshots are unchanged.
- **Low, fixture provenance:** `replace_live` cited an untracked capture.
  Fixed by committing `experiments/conductor-spike/audit-2.32.0/live/kas-clientcreator-2.32.0.jsonl`
  (sha256 `edccfbd68221e1521e57b5b189177e350f220faf747b1d95c8a77afb92c1be81`)
  after a secret scan:
  - no JWT-like strings (`eyJ…`);
  - the only token-bearing frame, the `_kiro/auth/getAccessToken` reply
    (line 5), already reads `<REDACTED>` for `accessToken`, `expiresAt` and
    `profileArn`;
  - no Authorization or Bearer headers, refresh tokens, AWS keys, passwords,
    secrets, emails or user ids;
  - the only long opaque string is the KAS bundle's sha256.

  The fixture's provenance now names the committed path, the 0-based frame
  and the hash.
- **Nit, `joinPolicy`:** documented on the type. It is required on the wire
  (zod `Dir`), but modelled as `Option` deliberately, so a future KAS that
  drops it still shows the plan.
- **Nit, vacuous assertion:** `!screen.contains('\u{202E}')` could not fail,
  because ratatui 0.30 already drops those graphemes. It is replaced by an
  assertion on the rows `plan_lines` hands ratatui, and the on-screen check
  keeps the space-substitution form (`evil txt.exe: wf [31mcoder`), which an
  uncleaned row would render as `eviltxt.exe: wf[31mcoder`.

## Approval

The design follows the issue's own fix direction and acceptance. It changes
no approved behavior beyond them. Requester approval is pending: it is
requested on the PR.
