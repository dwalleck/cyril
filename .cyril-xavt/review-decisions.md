# cyril-xavt shell approval review decisions

## Scope and authorization

This delivery copy retains the original combined-source repair history below. Its actual reconstructed source is on published EJ commit `9400e7c8ddd53776f6fd8b4d28d4c7fc69e45ab4`; delivery-handoff.md owns exact paths and identities. Historical combined-tree checks and reviews are not actual-delivery qualification.

2026-10-19, ReviewShellApproval F1/F2. Initial regression additions were an explicitly authorized tests-only exception to the combined worktree freeze. Main subsequently observed behavioral RED in artifact454 and authorized minimal production repairs and inheritance of repaired un4p/EJ source into the combined tree. The approval-widget owner implements R1/R2 in combined source and the cumulative X snapshot; dependency/App/artifact propagation is separately owned. No test, build, lint, formatter, smoke, commit, push or tracker mutation was performed by this worker.

This record follows Gilfoyle's workflow contract and assessing-review-feedback stage. Governing obligations remain `.cyril-xavt/design.md` C2 (safe command/cwd display without loss) and C3 (actual drawn command and selected option/editor before answering), owned by plan Slice1. The authorized repair preserves behavior, ownership, allocation bounds and permission policy; it does not relax an inspection requirement. Parent owns observed RED, GREEN, gates and independent repair review.

## Findings

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F1 | Fully visible one-character shell commands cannot answer Allow or submit a rejection reason; permit a complete one-character command without treating whitespace, clipping or cwd as context. | ReviewShellApproval | Verified | Pre-fix ShellBody::command_shown required visible>1. Parent artifact454 observes all eight actual App response regressions failing despite visible command context, with82 existing positive cases retained. | Accept | Actual App regressions retained. Initial ShellRows pass now gathers meaningful-command-grapheme metadata capped at two; ShellBody requires positive min(2,total) visible unclipped meaningful graphemes. | Closed: repaired90 App cases pass in542; assembled688 and actual-delivery860 pass; fresh actual-source review correct/confidence0.98. Long commands still need two visible graphemes. |
| F2 | Scalar-width shell wrapping loses the closing quote of FE0F command/cwd boundary cases; count and wrap consistently with rendered graphemes so complete content is reachable. | ReviewShellApproval; ReviewShellApproval.ReviewCoreShellProjection | Verified | Pre-fix shell_rows summed scalar widths although ratatui rendered graphemes. Parent artifact454 observes all four FE0F command/cwd next/end cases seeing p or C instead of the closing quote. | Accept | Actual terminal-buffer/scroll/cwd-refusal regressions retained. Both command and cwd now use borrowed sanitized graphemes consistently for wrapping, counting and visibility. | Closed: correct explicit FE0F filter passes in575; assembled688 and actual-delivery860 pass; fresh actual-source review correct/confidence0.98. Cwd-only refusal retained. |
| F3 | Retained without-cwd fixture expands consent.triggeringResource to the full raw command; restore the exact captured `echo SLEPT` only. Mechanism: source-fixture fidelity. | Actual-X review relayed and repair authorized by Main | Verified | Direct reading of cited `kas-lead-interrupt-sig15-2.32.0.jsonl:56` shows triggeringResource=`echo SLEPT`, distinct from command/resource=`sleep 45 && echo SLEPT`; actual fixture line107 incorrectly repeated the latter. | Accept | Only `without_cwd.permission._meta.kiro.consent.triggeringResource` in shell-preview-2.32.0.json is restored to `echo SLEPT`; canonical eleven-file source manifest refreshed. | Closed before actual-delivery860 gates. Raw command/cwd, production and assertions unchanged; frozen combined source/patch untouched; prior behavioral review remains applicable. |

## Source evidence and fixture geometry

The initial source check used frozen pre-fix production, not a clean base or executed binary; inspected snapshot was approval.rs#709A. The source geometry below explains the subsequently observed parent RED and does not claim worker execution.

- Pre-fix `crates/cyril-ui/src/widgets/approval.rs:310-320`: the guard required at least two visible positive-width nonwhitespace command scalars. `:`, `w` could provide only one; neither path nor supplied cwd contributed command glyphs.
- The same file at `848-859`, `967-973` and `1138-1144`: both option confirmation and reason submission require the command guard. `crates/cyril-ui/src/state.rs:2523-2535` refuses a decision before popping its responder; `App::handle_approval_key` routes ordinary Enter and feedback Enter into that real state path.
- `approval.rs:96-105`: option desired width is 60, capped by `modal::popup_width`; shell body width subtracts the two borders and four-space `TEXT_INDENT`. At area 80x24 this leaves **54 body cells**.
- `crates/cyril-ui/src/widgets/modal.rs:14-27,43-62`: popup width 60 centers at x=10. `input_top=5` restricts placement to `[1,5)`, giving popup `(10,1,60,4)` and inner height 2. `OptionLayout::new` at `approval.rs:454-458`, called with `preview.has_context()`, reserves exactly one preview/body row and one selected-option row for a typed shell, even without a path. Thus body is y=2, text starts x=15, last body cell is x=68, selected action is y=3, and right border is x=69. The tests assert actual border/action cells and exactly one extracted body row; they do not call a production wrapper to derive expected content.
- Exact command: `format!("printf '{}#\u{fe0f}'", "x".repeat(44))`. Its eight-cell `printf '` prefix plus 44 `x` cells, `#` scalar width 1, FE0F scalar width 0 and final quote total 54 scalar-counted columns. The `#\u{fe0f}` grapheme occupies two rendered cells, making 55. In the first view it starts at `(67,2)` with continuation cell `(68,2)`; the final quote must be reachable at `(15,2)` after offset 1 or the end offset.
- Exact cwd: `format!("/{}#\u{fe0f}'", "x".repeat(46))`, rendered after the literal five-cell `Cwd: ` prefix. Prefix 5 + slash 1 + 46 `x` + scalar `#` 1 + FE0F 0 + final quote 1 again total 54 scalar columns but 55 rendered cells. `echo COMMAND_TOKEN` occupies row 0; offset 1 shows the cwd prefix/FE0F grapheme, and offset 2 or end must expose the cwd's closing quote. Both cwd-only views must remain unable to confirm.
- Pinned dependencies: `Cargo.lock` identifies ratatui 0.30.0, ratatui-core 0.1.0, ratatui-widgets 0.3.0, unicode-segmentation 1.12.0 and unicode-width 0.2.2. Local dependency source under `/home/dwalleck/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`: `ratatui-core-0.1.0/src/text/span.rs:306-315` segments graphemes; `ratatui-widgets-0.3.0/src/paragraph.rs:424-444,460-471` passes them through `LineTruncator` and advances by `symbol.width()`; `unicode-width-0.2.2/src/lib.rs:56-61` explicitly documents emoji-presentation string width differing from summed scalar width. `workflow_plan.rs:144-148` excludes FE0F from bidi-format sanitization, so the sequence is retained.

## R1 — one-character command response regressions

**Delivered:** retained tests and authorized minimal production repair, owned by xavt Slice1. App regressions:

- `xavt_app_whole_one_character_shell_command_can_allow`
- `xavt_app_whole_one_character_shell_command_can_submit_rejection_reason`

Each covers `:` and `w`, pathless and `VOJX_PATH` cases, reusing `xavt_request`, `approval_draw_at` (the actual `App::draw_frame` path), actual `handle_key` routing and the existing request's oneshot receiver. They require the exact whole command and supplied cwd in drawn body cells, visible path when present, actual Allow/Reject selection, and actual `Selected(allow)` or `RejectWithReason(reject,"why")` delivery. Reason entry goes through Down, `r`, and the actual paste event. The chat draft remains owned by chat; the answered request must release approval ownership. No production confirmation predicate is used to compute the expected answer, and the existing pipeline-specific two-character screen classifier is not reused for these whole-command cases.

**Performed checks/results:** parent-reported artifact454 executes90 App tests:82pass and8fail, exactly the colon/w Allow/reason cases with and without a path. These are genuine missing-response failures after visible context, not compile or setup failures. No worker check was run.

**Disposition:** closed by parent GREEN542, assembled gates688, actual-delivery gates860 and fresh actual-source ReviewShellApproval correct/confidence0.98; verification.md owns provenance. The original RED is retained without rerunning it.

## R2 — FE0F closing-quote scroll reachability regressions

**Delivered:** retained tests and authorized minimal production repair, owned by xavt Slice1. Widget regressions:

- `xavt_shell_fe0f_command_closing_quote_is_scroll_reachable` — next-row offset 1 and end offset `usize::MAX`.
- `xavt_shell_fe0f_cwd_closing_quote_is_scroll_reachable` — next-row offset 2 and end offset `usize::MAX`, preceded by the actual cwd-only first row at offset 1.

Both reuse `shell_approval(&command, cwd)` and `render_at(&state,80,24,5)`. Initial-view assertions read actual grapheme symbols and continuation/border cells, and verify one actual body row. Scrolled assertions require the literal closing quote at the actual first text cell `(15,2)`, as well as the selected Allow row. Existing `shell_body_rows`/`screen_rows` only extract already rendered cells; no replacement wrapper or independent width oracle was added. The cwd case preserves explicit negative `confirm_is_on_screen` assertions for both its first and final cwd-only views. Existing cwd-only App/widget regressions are untouched.

**Performed checks/results:** parent-reported artifact454 executes all four FE0F command/cwd next/end cases; all fail at the required quote cell with p or C instead. This is genuine unreachable-tail RED, not a compile failure. Earlier exact first-view grapheme checks retain FE0F and the reviewed geometry. No worker execution is claimed.

**Disposition:** closed by the correct explicit four-case FE0F GREEN575, assembled gates688, actual-delivery gates860 and fresh actual-source ReviewShellApproval correct/confidence0.98. The unrelated zero-match filter in542 is never counted as focused GREEN.

## Parent focused commands — not run by worker

Parent's pre-fix behavioral RED is recorded above; do not repeat it to reconfirm the reported defect. After the authorized repair is handed off ready, run the focused commands for GREEN:

```sh
env -u CARGO_TARGET_DIR cargo test --manifest-path /home/dwalleck/repos/cyril-wt-fix-cyril-xavt/Cargo.toml -p cyril xavt_app_whole_one_character_shell_command -- --nocapture
env -u CARGO_TARGET_DIR cargo test --manifest-path /home/dwalleck/repos/cyril-wt-fix-cyril-xavt/Cargo.toml -p cyril-ui xavt_shell_fe0f_ -- --nocapture
```

These are historical focused command references, not outstanding rerun instructions. verification.md owns the completed assembled and actual-delivery results and exact source boundaries; this finalization worker did not run repository checks.

## Review errors and repair review

The previous stack-handoff.md two-glyph requirement was too restrictive for a complete valid one-character command under design C3. Its text now matches the initial-pass threshold, without weakening response assertions. The older xavt_command_drawn helper remains a multi-character pipeline-screen classifier, not a whole-command oracle, so it is intentionally unchanged and not used by R1.

No other reviewer error was established by the source check. Fresh actual-source ReviewShellApproval reported correct/confidence0.98 with no actionable behavioral findings, specifically retaining the one-character and FE0F repairs and their regressions. F3 subsequently restores one reference field without changing the reviewed runtime behavior. Canonical receipt and applicability are recorded in verification.md.

## Production authorization

Main after artifact454: “Authorize minimal R1wholeonecharacceptance + R2grapheme-consistent wrap/rowcount forcommand ANDcwd, sameborrowedsegmentationedge approved. You maynowpropagate repairedun4p/EJ snapshots into combined andfinishxavt production repairs; retainall82positive behaviors.” The shared primitive is the already-locked unicode-segmentation1.12.0 with explicit default-features=false, approved in the owning un4p repair record; combined manifests and UI lock edge now match that foundation. No new package/version or public API is introduced.

## Delivered production repair receipt

Both authorized approval.rs files received identical repair edits (tool snapshot3C97) after the cumulative repaired X widget was propagated into combined source. Private ShellRows returns row count and total meaningful command graphemes capped at two during the existing initial count pass; ShellBody reuses that count rather than rescanning the entire command for a threshold. Current-view guards count only positive-width nonwhitespace, fully fitting command graphemes. Zero meaningful content, clipping and cwd-only windows remain refused.

Private shell_graphemes preserves safe VS16/ZWJ clusters and borrows sanitized pieces. Controls/bidi become spaces, raw LF remains a newline, and CRLF preserves its prior space-then-newline display semantics. Both command and cwd use these same grapheme widths for row count, viewport rendering and visibility. Only requested viewport rows allocate strings; count/guard passes allocate none, range-end early stopping remains, and no complete sanitized copy is made. Raw typed command/cwd and grant semantics are unchanged.

Historical parent artifact542 executes all90 xavt App cases successfully and full workspace3117pass with13existing ignored tests. The attempted UI filter xavt_review_ matched zero cases: it is NOT four focused FE0F GREEN results, although the full suite executed UI tests. All-target Clippy then blocked on the replay helper's large enum variant; build did not run. The later correct FE0F filter575, approved owner-level representation repair, full combined gates688 and actual-delivery gates860 supersede that intermediate gate failure. No behavioral RED is hidden and no new check is claimed for the historical worker.

## Interim ReplayFrame gate repair — superseded

Parent artifact542 reported ReplayFrame::Permission616 versus routed Notification376 under large_enum_variant/-Dwarnings. Both helper payloads were temporarily boxed rather than shifting the imbalance from Permission to Notification. Parent575 then passed the correct four FE0F cases and workspace3117, but exposed ChatMessageKind::ToolCall296 versus SteerEcho73. Build still did not run. This established that the new inline optional shell context, not each containing enum, was the proper representation owner.

Main approved private Option<Box<ShellCommand>> at the existing ToolCall owner, preserving borrowed APIs and allocating only for an attached shell projection. The three source lines are applied in core types/tool_call.rs. Both interim ReplayFrame boxes, both construction wrappers and all four App consuming dereferences have been retired together. The helper/App edit receipts restore their pre-box snapshots3918/FD79; no net test-support API/heap delta remains, and every assertion is unchanged.

LSP references for the exported helper/variants returned inconsistent empty results before both interim change and retirement; full Rust-source fallback identified two constructors and two consuming helpers. Primary-worker references for ShellCommand/builder/getter also falsely returned empty; source fallback inventories seven builder, nine getter and eighteen domain request sites, all API-compatible. Tool defects were reported. Static source-derived layout bounds and Box-versus-Arc rationale are in design.md; actual final Clippy confirmation remains parent-owned. The obsolete03b-replay-support patch in frozen un4p outputs must not be applied; the updated X feature patch supersedes it.

## Current qualification boundary

Actual X source is reconstructed from the pinned latest feature delta with the approved private optional-context box, no per-ToolCall/chat heap layer and no ReplayFrame workaround. Combined gates688 remain historical; actual repository qualification is parent bg128/artifact860 after F3 and final formatting. Source was frozen after860; the later eleven-file freeze digest is not a contemporaneous gate-time tree hash. The qualified binary's SHA-256 is33891699d8cfa71f5ce8e16fae3675b23c4e104d384a7f7d69416880ffebfb84. Fresh actual-source review and the exact captured-wire actual-delivery RejectOnce proof complement, but do not relabel, the one historical real-KAS run. verification.md owns all final result/provenance boundaries. No source repair, second native execution, account/model contact, commit or tracker write occurred in finalization.

## Actual-delivery fixture-fidelity repair

F3 belongs to Slice1's retained live-derived fixture under design C1 and C5. Main reports fresh actual-X independent review correct/confidence0.98 with no behavioral findings, then authorizes this exact one-field correction before gates. Reading the named capture line and fixture field establishes the mismatch directly; this is reference restoration, not an execution/permission behavior change or a new oracle.

Delivered: fixture triggeringResource matches `echo SLEPT`; actual full shell command remains `sleep 45 && echo SLEPT`, and cwd/production/assertions are unchanged. delivery-handoff.md retains the corrected pre-format reconstruction identity; verification.md owns the post-gate source freeze. ApprovalVisibility acknowledged retaining the correction in future authorized exports; frozen inputs remain untouched.

Evidence: direct source/fixture reading established reference restoration before final parent860 qualification. The prior source-only behavioral review is unchanged in scope by this one-field correction. Artifact860 then passes the actual-delivery full suite/all-target Clippy/build/format chain; the owned consumer proof passes with exact captured payloads, only one RejectOnce, no execution, no completed-frame replay, and a clean exit0. This closes the bounded repair and consumer documentation handoff; Main owns publication.
