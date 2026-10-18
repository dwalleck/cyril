# independent-review.md — cyril-auxt

Reviewer: independent `rust-code-reviewer` agent, 2026-10-18. Scope: `git diff 10e38e3c..50dcc3b3 -- crates`
against CLAUDE.md and `design.md`. It re-read the TUI 2.32.0 bundle functions in the provenance table. Verdict:
**no blocking findings**. It verified that every terminal path answers the responder exactly once, that queue order is
kept, that Esc unwinds correctly, and that key and paste routing works. It confirmed the rows (`isr`/`Ub`), folder offer
(`b2t`/`A2t`), validation (`vde`), echo (`cw`), and child rule (`_lt`/`C8n`) are faithful to the TUI, and found no
discipline violations.

| # | Finding | Decision |
|---|---|---|
| S1 | TUI attributes sub-agent approvals to their subtask (`prepareKasPermissionRequest`); cyril treats them as main-session | Out of scope: needs KAS subtask tracking (cyril-ebqu). KAS's permission `_meta.kiro` carries no `agentSubtaskId`. Recorded as a known divergence in `design.md` Non-goals and raised to the requester. |
| S2 | Scope-cycle test could not catch a row-tracking regression | Fixed: `scope_cycle_follows_the_highlighted_row_when_rows_change`, `scope_cycle_from_the_folder_row_returns_to_the_first_row` |
| N1 | TUI carries the scope to the next approval (`k2t`) | Fixed: `UiState.always_allow_scope`; test `next_scope_page_opens_at_the_last_scope` |
| N2 | Design wording vs the `consent.is_some()` gate | Fixed the wording in `design.md` (the gate matches the TUI's `De`) |
| N3 | "captured reply" test is partly self-authored | Renamed to `…_matches_the_audit_echo`, with a doc comment pointing at the live receipt |
| N4 | `oo` trims; cyril used `!is_empty()` | Fixed: the echo `verbatim` and `non_empty` trim-check |
| N4b | `triggeringResource: ""` with `??` | Accepted: KAS never sends an empty string. Treating it as absent is the safer reading. |
| N5 | Folder editor accepts newline keys (then rejects as unprintable) | Accepted: the validation message names it. The shared editor is reused rather than forked. |
| N6 | `unwrap_or(0)` cursor restores | Accepted: these mirror the existing SelectTrust restore and the TUI's `osr` fallback. Not an error path. |

The repairs were re-run in the affected tests (81 passed) and in the full gate (`receipts/gate.txt`).
