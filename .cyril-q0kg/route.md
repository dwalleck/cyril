# Route: cyril-q0kg

Change: the approval overlay logged `WARN approval preview raw_input has no
string text/content field` from inside the render path, so it fired on every
frame while the overlay was open (2.32.0 audit: ~14k lines / 3.9 MB of
`cyril.log` for one approval). Keep the cyril-j1b3 intent — log the malformed
shape, never coerce the value, render "Preview unavailable" — but log once per
permission request, at the state transition that installs the approval.
Date: 2026-10-18

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | No external fact is open. The per-frame emission is reproduced in-repo by the red fence (`receipts/red.md`: 6 frames → 6 WARN events). The approval's tool call is never mutated after install: every `self.approvals` access in `crates/cyril-ui/src/state.rs` touches only `selected`/`phase`, and `show_approval` is the only production constructor of `ApprovalState` (`grep -rn "ApprovalState {"` → only test fixtures otherwise). So a classification computed at install time stays valid while the overlay is open. | no |
| 2 | Structural module shape | Owners unchanged. The raw-input classification moves out of `widgets/approval.rs::preview_lines` into a new additive display accessor `TrackedToolCall::preview_body()` returning `PreviewBody` (`crates/cyril-ui/src/traits.rs`) — `TrackedToolCall` is CLAUDE.md's documented home for display interpretation (`primary_path`, `command_text`). The warning moves to `UiState::show_approval` (`state.rs`), which already owns approval install. No seam, dependency direction, or existing signature changes; nothing crosses a crate boundary. No length gate in the repository. | no |
| 3 | Production-scale risk | The fix removes a per-frame log write (the measured 3.9 MB/session). The classification is a constant-time JSON key lookup, as before. | no |
| 4 | Explicit behavior | Behavior source: the issue's acceptance criteria. (1) Installing an approval whose tool call has no diff and a raw input without a string `text`/`content` logs exactly one WARN naming `tool_call_id` and `raw_input_shape` (shape only, never the value); rendering it any number of frames, in the option phase or the reject-with-reason phase, logs nothing more. (2) A payload with a diff, a string `text`/`content`, or no raw input logs nothing. (3) The "Preview unavailable" marker still renders for absent and malformed payloads; diff and text bodies render unchanged. | yes |

## Selected route

Local — explicit behavior inside existing owners; one additive display
accessor, no interface, seam, or scale change, no open premise.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md, evidence.md, design.md, plan.md | — | N/A — Local route (T1–T4 above) |

## Downstream sequence

none — red fence first, minimal fix, fence stays as the regression fence.

## Terminal criterion

- Red: `malformed_preview_warning_is_not_repeated_per_frame` fails on the
  unmodified renderer for both malformed cases (`receipts/red.md`).
- Green: the same fence passes after the fix, with the cyril-ui suite.
- Every ci.yml lane passes (`receipts/gate.md`).
- Independent review satisfied (recorded below).

## Results

See `receipts/red.md` and `receipts/gate.md`.
