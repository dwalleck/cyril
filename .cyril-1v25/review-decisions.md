# cyril-1v25 review decisions

## Findings

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
| --- | --- | --- | --- | --- | --- | --- | --- |
| F1 | Retaining an ID-only tool index across SessionCreated lets a legal reused ID in a new session mutate the old session's preserved row; retire current-session lookup/live state at the transition. | ReviewMissingKas | Verified | Pre-fix RED artifact 119; isolated repaired workspace GREEN artifact 188, including all six KAS regressions; real two-session TUI smoke PASS. | Accept | Clear current-session tool lookup/live tracking only when main session identity changes; preserve history and same-session deduplication. | Resolved. Independent scoped re-review found no blocking findings (confidence 0.98). |
| F2 | A Started frame for a retained ID after commit does not repopulate the active list; proposed fix distinguishes Started from Updated. | Aggregated advisory; Main disposition | Verified | Direct control-flow check establishes that a known row plus an empty live list does not repopulate that list. No real consumer regression or required behavior violation was demonstrated. The existing pre-commit later-start test is green in Main's artifact 92. | Reject | N/A — no shown product regression; preserve the approved update/live behavior rather than expanding the repair. | non-blocking — speculative post-commit restart semantics are outside the demonstrated missing-result defect. No intended deferred feature is created. |

## Review errors

The aggregated advisory also claimed the existing `missing_start_update_commits_preceding_streams_at_arrival` active-count assertion must fail. That claim is refuted: its orphan insertion already creates the active entry, and Main's artifact 92 records this test passing. The verified F2 control-flow observation applies only after commit clears that entry, not to the cited test sequence.

## F1 atomic repair record

- Governing obligation: tool IDs are session-local; preserve chronological history without merging distinct sessions' calls. This is an approved-contract repair within the existing Local UiState owner.
- Before production repair: added the regression only, covering old-session history, SessionCreatedB, both first Started/Updated forms of a reused ID, active retirement, front pruning, and a subsequent partial completion.
- Pre-fix sensitivity: Main ran the exact named integration test `reused_tool_id_in_a_new_session_preserves_the_old_session_row` before production repair. Artifact 119 records RED: one reused row instead of two; its executable came from `/home/dwalleck/repos/cyril/target`. The checked source was the uncommitted shared repair atop base 8e5c325e, plus the regression, without the SessionCreated reset.
- Delivered F1 repair: `crates/cyril-ui/src/state.rs` now retires `tool_call_index` and `active_tool_calls` when `SessionCreated` changes `main_session_id`, preserving all message history. Same-session acknowledgments retain their deduplication state; commit/prune behavior is unchanged. Main acknowledged RED and authorized this minimal approved-contract repair before it was applied.
- GREEN — Main's isolated `fix/cyril-1v25` delivery tree passed all six KAS integration cases, the workspace suite, workspace/all-targets clippy with `-D warnings`, executable build, and formatting check (artifact 188). It excludes the separate fiv1 implementation. The actual binary's two-session smoke also passed: both sessions reused the captured IDs and retained two chronological pairs of Tool Load/Subagent Response rows.
- Current checked state for earlier proof: base 8e5c325e plus the uncommitted shared `state.rs`/error `traits.rs` repairs and named KAS/v2 helper/tests/fixtures. Artifact 92 proves the prior seven tests and assembled gates; the new F1 test and repair are not covered by that receipt.

## Independent review state

ReviewMissingKas independently re-reviewed F1 after the focused GREEN result: no blocking findings, confidence 0.98. Same-session acknowledgments, pruning and history clearing preserve their intended semantics. The earlier unaffected capture/rendering conclusions remain valid. F2 remains rejected rather than silently implemented.
