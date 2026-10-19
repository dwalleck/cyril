# Route: cyril-8wh0

Change: KAS 0.66.43 asks the user to approve a workflow's plan through an
ordinary `session/request_permission` (`update_workflow`). The plan arrives
as a Text content block and as `_meta.kiro.workflowPlan`. cyril's overlay
showed "Preview unavailable". Show the Text (bounded), type the plan in
`convert/kas`, and render its step list in the overlay. This is a
prerequisite of cyril-0asq and cyril-sm36.
Date: 2026-10-18

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | Resolved from existing evidence, so no probe was needed. The wire shape comes from the 2.32.0 live captures (`kas-lead-wfplan-2.32.0.jsonl` frame 93 for kind `launch`; `kas-clientcreator-2.32.0.jsonl` frame 67 for kind `replace`). The full emitter comes from the installed 0.66.43 bundle (`P0o`, `M0o`, `Ydu`, `nxe`, `wHr`, the `Xk` recipe schema, `Ohe`/`Fir`/`Lhe`/`Oir`/`Dir`, and the `Ay`/`D_o` string cleaning). The covenant doc predates `workflowPlan` (FINDINGS.md says so). Kiro's 2.32.0 TUI reads no `workflowPlan` (0 occurrences) and shows the Text through its generic approval view. | no (resolved) |
| 2 | Structural module shape | Yes. New public domain types (`cyril_core::types::WorkflowPlan` and its node types, in `types/workflow/plan.rs`). A new field on the public `PermissionRequest` (`workflow_plan: Option<Box<WorkflowPlan>>`) and on `ApprovalState`. A new converter submodule `convert/kas/workflow_plan.rs`, and a new widget module `widgets/workflow_plan.rs`. A new `PreviewBody::Content` variant. Ownership follows CLAUDE.md: `_meta` parsing stays in `convert/kas`, the UI sees only domain types, and the App is untouched apart from test literals. | yes |
| 3 | Production-scale risk | No. The plan is parsed once per request. The plan render is bounded (12 rows plus a marker). Text past the row cap is counted without building strings. | no |
| 4 | Explicit behavior | Source: the issue acceptance. (1) A Text content block shows in the overlay instead of "Preview unavailable", bounded. (2) `workflowPlan` is typed, every field is modelled, and unknown fields get a debug log. The overlay renders the step list for `update_workflow`. (3) Live: a plan approval shows the plan before any step runs. | yes |

## Selected route

Structural (T2). The behavior is explicit (T4) and the premise is resolved
(T1), so the next stage is design.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| design.md | falsifiable-design | `design.md` |
| plan.md | budgeted-plan | N/A: three slices, recorded in design.md's Slices section. No feature budget was supplied, and the PR is about 2.4k lines, mostly tests and fixtures. |
| spec.md, evidence.md | — | N/A: behavior explicit (T4); premise resolved from captures and bundle (T1) |

## Terminal criterion

The design claims pass with evidence. Every ci.yml lane passes, GitHub CI
is green, and the live check passes. Independent review is satisfied.
