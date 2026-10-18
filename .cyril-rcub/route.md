# Route: cyril-rcub

Change: the approval overlay wrote a tool call's target path as one line
(`format!("  {path}")`) and clipped it at the popup border. On a long write
path the file name disappeared (live 2.32.0:
`/home/dwalleck/repos/cyril/experiments/conductor-spike/a`). Make the path
identifiable. Never hide the file name, at any width, including with Unicode
or wide glyphs.
Date: 2026-10-18

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | None open. Rendering is fully reproducible on `TestBackend` (`receipts/red.md`). For the wide-glyph premise, ratatui draws a wide glyph as the symbol plus a continuation cell. The test reader `screen_rows` advances by the symbol's display width, and the wide fences pass and fail as expected. Column widths come from `unicode-width`, which `text.rs` and ratatui already use. | no |
| 2 | Structural module shape | Owners are unchanged. All path layout stays in `widgets/approval.rs` (a private `Preview` plus private helpers). `widgets/modal.rs` gains `popup_width`, which is the width clamp `centered` already applied, now extracted so it has one source of truth. `centered`/`place` behaviour is unchanged, and the parity sweep in `modal.rs` still passes. No public interface or dependency direction changes. The repository has no length gate. | no |
| 3 | Production-scale risk | The work per frame is wrapping one path, plus a bounded shrink loop when clamped (at most `width × rows` iterations, in practice one or two). This is negligible. | no |
| 4 | Explicit behavior | Behavior source: the issue acceptance ("a path longer than the overlay width keeps its final component visible: wrapped or middle-ellipsized; insta snapshot at 80x24 and a narrow width") and the requester's brief. (1) With rows to spare, the whole path is on screen, wrapped at display-column boundaries, and the rows read back as the exact path. (2) When the popup is clamped and the path has fewer rows than it needs, the path is middle-elided into those rows. The elided text keeps the file name, plus its parent when that fits, and gives every remaining column to the leading context. The elision is marked with `…`. (3) A file name wider than the whole budget keeps its end, so the extension stays. (4) A wide glyph is never split and never pushes a row past the border. (5) Both the option page and the reject-with-reason page use this layout. (6) A clamped option page keeps the selected option visible. Ownership was decided as follows. The blank separator after the preview was not counted in the row budget, which clipped the selection (cyril-a14l C8). It is fixed here because the clamped fence cannot be true without it. | yes |

## Design choice: wrap first, elide only when clamped

- **Wrap alone is not enough.** The option page drops preview rows from the bottom when it is clamped. A wrapped path's file name sits on its last row, so the file name is exactly what would be lost.
- **Middle-ellipsis alone is not enough.** It always hides directories in the middle. For a write outside the workspace, those directories are what the user needs to see, and this is the safety reason the issue exists.
- **Combined.** `Preview` lays out the path first, before any body rows. It gets every row it needs when the popup has them, and is elided into its share only when the popup is clamped. The file name and the leading context are always kept, which is the brief's "never hide the file name".
- **Wrap granularity.** The path wraps at the character level (by display column), not at separators. The rows join back to the exact path, and each row carries as much of the path as fits.

## Selected route

Local. The behavior is explicit, the change stays inside the existing widget
owner, there is one extracted modal helper, and no premise is open.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md, evidence.md, design.md, plan.md | — | N/A — Local route (T1–T4 above) |

## Terminal criterion

- Red: the render fences fail on the base revision, and mutations M1 (old
  single-line path) and M2 (old separator budget) are each detected by the
  final fences (`receipts/red.md`).
- Green: the fences, the unit and invariant tests, and four insta snapshots
  (80x24, 40x24, clamped 80x16, clamped 44x16) pass with the cyril-ui suite.
- Every ci.yml lane passes (`receipts/gate.md`).
- Independent review is satisfied (recorded below).

## Results

Recorded after the gate and review.
