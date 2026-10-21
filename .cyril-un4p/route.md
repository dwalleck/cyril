# cyril-un4p route

Date: 2026-10-19. Base: supplied worktree at 8e5c325e. Full Rivets record read, including empty notes and prerequisite/dependent lists.

**Released-renderer refresh, 2026-10-20:** Current source/consumer/review receipts are indexed in [verification.md](verification.md#released-renderer-refresh-2026-10-20). The original implementation and artifact476/503 history below is retained, not reused as migrated-state full-workspace or native qualification.

## Routing evidence

| Test | Verdict and evidence |
|---|---|
| T1 empirical premise | No. `widgets/approval.rs` computes and renders both page geometry and its confirmation guard; the existing App `draw_frame` + `handle_key` tests exercise the actual consumer. No external runtime fact determines this fix. |
| T2 structural module shape | No. Preserve existing Preview/OptionPage/ReasonPage/FolderPage owners, UI-only guard, public interfaces, and dependency direction. No split, new owner or schema. Main DG approved the existing UI-owner implementation seam: a direct declaration of already-locked unicode-segmentation1.12.0 for renderer-consistent borrowed grapheme boundaries; no new package/version/framework/public API. Approval and source evidence are in review-decisions.md. |
| T3 production-scale risk | No. A confirmation recomputes a bounded path fit already used by rendering; no new process, async work, retained state, or input-volume contract. |
| T4 explicit behavior | Yes: the issue requires reason-page path priority over Limit/notice, confirmation only with a drawn selection/editor and identifiable path tail, and rejection of zero-inner-width popups. Parent's verified review repair clarifies the boundary: a wholly drawn nonempty target, including a single grapheme or literal ellipsis, is sufficient; an actually middle-elided target still needs more than one visible tail grapheme. Preserve option windowing, path middle-elision/filename retention, input protection, cancellation, reason draft retention, and trust/scope/folder semantics. |

Selected route: **Local**. Other implementation stages remain inapplicable because ownership and approved behavior are unchanged. route.md owns the complete outcome contract; review-decisions.md owns the independent findings, bounded repairs and direct-primitive DG.

## Implementation and verification contract

- Reason pages reserve one editor row, then a path row where one exists and a second row fits; Limit/notice/cosmetic context cannot displace it.
- Both page guards use the fitted path actually drawn, including explicit actual-elision metadata rather than parsing filename text. A fully drawn positive-width single-grapheme target is accepted; an inserted ellipsis or one retained tail grapheme after actual elision is not enough. Grapheme boundaries and display widths match the renderer.
- All confirmable pages require nonzero inner width as well as their existing row/selection checks.
- Permanent regression seam: existing App draw_frame + handle_key, parameterized across width/height and reason-page notice presence. Renderer tests/snapshots capture deliberate reason-page ordering changes.
- Parent verification: red-before/green-after focused regressions, existing approval snapshots and consumer smoke, then required workspace/CI gates. No mid-flight build/test/lint/format command is run by this worker.
- Real rendered TUI smoke: open a path-bearing approval, shrink height through reason inner rows 1–4 with and without validation notice, observe path before Limit/notice at inner 2+, then shrink widths through zero-inner-width and ellipsis/tail boundaries. Enter must preserve the request/draft while context is insufficient and answer only when the drawn option/editor plus tail are present; Esc remains usable.

## Delivery boundary

First issue in the stack, before cyril-ej7y and cyril-xavt. Parent owns independent review, smoke/gates, issue commit and PR publication. No commits or tracker writes by this worker.

## Evidence status

Implementation reconstructed in dedicated delivery worktree from8e5c325e with un4p-only approval.rs/app.rs changes: actual-width/tail context guard, nonzero inner area and reason path priority. RAP1/RAP2 repairs add private fitted-path visibility metadata and borrowed renderer-consistent grapheme fitting/counting. Workspace/member manifests and the existing cyril-ui lock dependency edge declare only the DG-approved, already-locked unicode-segmentation1.12.0 primitive. No ej7y/xavt APIs. Existing roomy/clamped snapshots are retained. The exact two-row assertion in reason_path_precedes_limit_and_notice plus its actual confirmation guard preserve the reason inner2 geometry; Main removed a redundant uncompilable named+inline Insta assertion, mirrored into the combined source and layer inputs.

Original regressions remain unchanged: approval_confirm_requires_visible_path_tail_at_every_size, pathless_approval_cannot_confirm_a_border_only_width and reason_path_precedes_limit_and_notice. Added un4p_review_ actual App/renderer regressions cover fully visible literal-ellipsis/single-character targets and VS16 last-target visibility. Parent artifacts385/386 observed four genuine App behavioral RED cases and six genuine renderer RED cases. The initial App VS16 fixture failed setup geometry, not behavior; its10x9 setup was corrected to10x11 while retaining the six-column/two-inner-row expectation. Parent artifact476 now executes that corrected fixture successfully within five App and six UI repair regressions.

Original-regression sensitivity is separately established by Main's MissingToolResults proof worktree at8e5c325e with test-only transplants: geometry sweep24pass/21intendedfail (artifact401); pathless-width8pass/4intendedfail (artifact403); original reason priority0pass/2intendedfail (artifact412), drawing why█ and Limit instead of target and why█. Artifact405 was an uncompilable redundant Insta assertion, discarded rather than counted as RED. These source-sensitivity results are distinct from the later RAP1/RAP2 review-regression RED in artifacts385/386. No additional baseline run is needed.

PASS — Main's artifact476 establishes all11 focused repair regressions, full workspace tests, all-target Clippy, build and formatting checks on the isolated un4p source. Independent ReviewApprovalPaths repair review reports correct, confidence0.97, no new findings; both RAP1/RAP2 are resolved. Main's artifact503 records successful actual-Cyril-process smoke with an owned KAS-shaped ACP peer:10requests and10matching manual responses, preserved chat draft and exact feedback, including clamped path/editor priority and resize refusal/retention. README guidance was updated after this smoke.

Evidence and limits: [verification.md](verification.md) indexes the portable original harness/receipts/screens, gate output and independent review. The owned peer is not real KAS/backend/account qualification; Pyte decoded ASCII/text geometry is not native-terminal Unicode glyph proof. Exact grapheme boundaries are separately covered by the passing App/TestBackend regressions. The failed initial smoke was a harness-oracle defect, not product RED; corrected-b is the successful10exchange run.

Publication remains Main-owned: commit, issue-specific PR, required publication/CI checks and tracker closure. This worker ran no tests/builds/linters/formatters/smokes, made no production/Cargo edit during this receipt step and performed no commit/close. Child delivery-patches and ej7y/xavt files are excluded from the un4p publication scope.

## Released-renderer refresh — 2026-10-20

- Route remains **Local**: released ratatui-core0.1.2/widgets0.3.2 compatibility within existing UI owners, with no new public API, permission semantics, backend behavior or execution path.
- Preserve `FittedPath`/`ElidedPath`, whole-target versus actual-elision metadata, borrowed grapheme wrapping/counting and exact raw path bytes. Shared text helpers measure complete rows/strings; already-segmented glyphs use borrowed `CellWidth` directly. Control-containing graphemes contribute no painted cells without normalizing their bytes. Actual zero path width stays non-confirmable; `.max(1)` is layout-only.
- Both fitted-row guards use physical columns. Whole visible meaningful targets remain eligible, including standalone halfwidth sound marks; clipped targets and combining-only/zero-cell context remain ineligible. The existing cap2 counts meaningful graphemes, not terminal columns.
- Parent qualified assembled source tree `b4c644051b65b500053dea089a1206d93220d43b`, distinct from both feature HEAD and the preformat staged tree. Current local results are UI+App1263passed/0failed/3ignored, workspace all-target Clippy, default binary build and format checks PASS. ReviewCurrentUn4 found no findings on the actual postformat working source, confidence0.94.
- Exactly one refreshed U10 owned-peer consumer passed against the matching preserved private executable: ten requests/responses, unchanged scenarios/oracles, retained drafts/reasons and narrow Enter refusal. This exercises the production process/converter/manual response wire, not live KAS/backend/auth or native glyph cells.
- **Main-target CI is mandatory.** Local full-workspace/core execution, all-feature/platform and native/auth acceptance are not claimed; inherited auth6c remains unavailable here and unwaived. Publication/commit/push and sequential child authorization remain Main-owned. Historical proof and ambient delivery-patches are preserved.

