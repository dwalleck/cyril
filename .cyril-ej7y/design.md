# cyril-ej7y design

## Route and inputs

Structural route; issue behavior in route.md T4. Installed KAS 0.66.43 `dist/server/acp-server.js:12207,12212` applies cleaning and a 600-code-point cap to the entire workflow comparison, including its prefix, then appends `...` if cut. The existing `workflow_plan::clean` matches: controls/bidi formatting become spaces, whitespace collapses and ends are trimmed before the cap. “Full comparison” means this complete retained workflow `comparisonText`, not a full file Diff/Myers result or expanded diff cache. A 600-code-point comparison can exceed 12 wrapped rows even at ordinary popup width because continuations are indented; navigation reconciles retention with the unchanged visible-row bound.

## Approved behavior disposition

Parent relay of the requester's explicit choice, 2026-10-19:

> User explicitly selected via ask: "Scrollable bounded preview" — "Keep the 12-row layout limit and make the full comparison reachable by scrolling. Adds preview navigation."

This approves bounded scrolling and retention of the full reference comparison. It does not authorize changing the12-row limit, shortening the reference text, or permission-grant semantics.

## Placement / module shape

| Owner | Decision and interface | Forbidden |
|---|---|---|
| widgets/workflow_plan.rs | Keep existing typed tree/cleaning/wrapping owner. Clean comparison to600 chars and wrap without a row cap; shrink the tree budget to12 minus comparison rows, retaining at least one row for the tree's omission marker when a nonempty tree cannot fit. Fix cap_rows(...,0) to yield no rows for an actually zero budget. Comparison may exceed12 projected rows but never12 visible body rows. | No mutable scroll state, no keys, no reference-shortening substitute. |
| traits.rs ApprovalState | One request-local preview_scroll_offset: usize, initial0. Preserve it between option/reason phases; queue promotion starts at its own0. | No global/shared offset, no rendering cache, no core scroll state. |
| widgets/approval.rs | Existing Preview/OptionPage/ReasonPage own body projection and bounded viewport. Pin path rows. Cap body demand at12, independent of total projected rows. One shared body-window computation gives effective offset, actual content capacity and one-row navigation status when needed. Expose crate-local preview_window(&dyn TuiState) for input using recorded frame dimensions and actual page layout, like confirm_on_screen. | No independent geometry arithmetic in state/App; never scroll selection, path or editor. |
| state.rs UiState | New approval_preview_key(KeyEvent)->bool consumes only dedicated navigation keys on option/reason phases and mutates the front approval offset using widget-provided current bounds. Clamp stale offset before movement. | No wrapping/placement rules or bridge access. |
| App handle_approval_key | One delegation before editor branches, then unchanged current routing. PgUp/PgDn NONE page the preview; exact Shift+Up/Down move one row; exact Shift+Home/End jump first/last window. | Do not steal unmodified Up/Down selection or reason movement, unmodified Home/End editor movement, Ctrl/Alt combinations, or later scope/trust/folder keys. |

Retain existing owners and dependency direction; no new generic module/framework or TuiState method. Compiler visibility/dependency checks plus independent review cover placement. LSP was attempted for exported ApprovalState/ToolCall references and hover, but linked-worktree project intelligence returned no results despite known callsites; use explicit constructor/caller inventory and report limitation rather than claiming successful semantic coverage.

## Window contract

- Workflow body layout demand=min(total projected rows,12). Popup placement/windowed selection/input protection remain unchanged.
- Actual body rows subtract pinned path rows from that page's assigned preview rows. The offset applies only to body text.
- If body fits, render it unchanged and no hint. If it overflows and at least2 body rows fit, reserve one of those rows for `↑ start–end/total ↓ PgUp/PgDn`; at most11 content rows +1 hint. If only1 body row fits, retain that content row and put a compact navigation indication on the popup border, never an extra body row. Zero body rows mean no scroll target.
- Clamp rendering to current max offset. Clamp before every key action as well, so resizing from a narrow viewport cannot leave dead offsets. Page step equals actual visible content rows, not a hard-coded12.
- The complete cleaned comparison remains in the projection. Ordinary tree truncation remains explicit: reserve at least one projected row for its omission marker rather than silently deleting the tree when the comparison needs12 or more rows. This marker and full comparison are navigated through the same12-row viewport. No silent four-row comparison cap survives.
- Comparison continuation indent is comparison-only `min(4,width.saturating_sub(2))`: leave two cells for wide glyphs where possible, or the sole cell for ASCII at width1. Existing tree continuation indent stays4. Zero actual text cells have no scroll target; navigation cannot reveal text in nonexistent cells.
- Ordinary text retains its established five content rows plus marker. Plan Text-content and raw-text fallbacks use12 total rows including marker:12 rows fit intact;13 rows show11 plus `...2 more lines`. No discarded row is omitted from marker accounting.
- The same viewport may later serve typed shell command bodies in cyril-xavt without introducing a second scrolling implementation. That issue must add visible-command confirmation checks; mere typed-field presence is not on-screen context.

## Claims / falsification

| ID | Claim/cases | Verification and falsifier | Regression coverage | Status |
|---|---|---|---|---|
| C1 | KAS600-code-point comparison retained, including added/changed ids after removed ids;599/600/601 boundaries, Unicode/control/bidi cleaning, multiple popup widths. | Render the actual plan approval; page to end. Missing any independently expected retained id/character or reaching text beyond reference cap falsifies. Exact expected cap derives from KAS rule, not current implementation. | Widget rstests + long-comparison insta snapshot; preserve existing plan snapshots. | PASS — verification.md G1/S1; ASCII/wide boundary pages and retained/excluded tails recorded. |
| C2 | Body <=12 rows including status, path pinned, selected option/editor remains visible and input protected across height/width. | Real App draw_frame/handle_key traversal and TestBackend render assertions. Page traversal failing to reach tail, stealing choice/editor movement,13th body row, hidden target confirmation or input overlap falsifies. | Extend current approval App seams; rstest notice/phase/size/navigation cases. | PASS — verification.md G1/S1; actual geometry observations and captures retained, including one-row border hint. |
| C3 | Offset request-local, resize/phase changes clamp, key modifiers preserve editing/selection, later pages untouched. | UI state + actual App modal-key tests, queue promotion and shrink/grow/page-boundary cases. Stale offset/queue leakage, stolen editor Home/End or non-preview phase mutation falsifies. | Existing state/App behavior seams. | PASS — verification.md G1/S1; actual modifier, first-key resize normalization, reason/draft and queue-reset receipts; later pages covered by G1. |
| C4 | Plan fallbacks total budget12 includes marker and displaced row; ordinary generic budgets unchanged. | Renderer rstests with11/12/13/25 rows, raw text and Text-content fallback, widths with wrapping.13 visible rows, wrong omitted count, capping exact12, or changed ordinaryfive+marker falsifies. | Focused fallback render tests + existing generic bounds tests. | PASS — verification.md G1/S1; plan Text/raw fallback cases plus distinct ordinary Text five-plus-marker and Diff twenty-plus-marker controls. |
| C5 | Actual rendered TUI exposes comparison tail via documented navigation while selection/input remain usable. | Run built Cyril in safe owned fixture using realistic workflow approval payload; capture initial and final PgDn/Shift-End views plus resize and unchanged Up/Down selection/reason-editor behavior. | Mandatory real rendered smoke receipt; not replaced by isolated helper checks. | PASS — verification.md S1; first actual launch,24/24 requests/responses, exit0; PTY decoded-text/ASCII geometry only, not native glyph or live upstream qualification. |

Executed sensitivity is recorded in verification.md S2: one identical actual workflow fixture and key/viewport sequence leaves retained late comparison `chg11` unreachable on the preserved old-parent binary, then reaches it on the G1-qualified EJ binary. This bounded comparison was obtained after G1/S1; it is not a fabricated historical test-first RED or merely a status-text difference. G1/S1 retain the broader current-state regression/consumer coverage. No bespoke mutation framework or 24-case smoke rerun was used.

## Non-goals

No permission grants, workflow execution changes, scrollable entire step tree, mouse-policy changes, new overlay framework, or changed600-char/12-row bound. Truncated tree rows remain explicitly marked; the newly reachable information is the comparison required by this issue.

## Design approval

**DG approved, 2026-10-19 — Main**, after reading the complete design, including the explicit tree omission-marker reservation. Governing requester delegation relayed verbatim: **"Delegate routine sign-offs" — "I select and record evidence-backed designs. I still ask before scope changes, relaxed acceptance, architecture changes outside the existing boundaries, or explicit risk acceptance."** This is delegated design approval, not a requester verbatim design sign-off. Approved concrete request-local offset, bounded shared viewport and exact dedicated keys as specified. The earlier bounded-scroll behavior choice retains its single owning record above. No risk or limit relaxation authorized.

**DG addendum approved by Main, 2026-10-19:** comparison-only adaptive continuation indent above, after code inspection identified four-column continuation padding clipping otherwise usable narrow comparison rows. This follows the already-approved reachable full comparison at usable widths; no text/reference, row-budget, public interface or existing tree-indent relaxation. Required additional actual-render regressions cover narrow ASCII, wide glyphs in two cells, and no-text-width no-target behavior.
