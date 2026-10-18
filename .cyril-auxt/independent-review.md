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

## Round 2 — independent cross-model review of 1a7b079d (2026-10-18, relayed by the coordinator)

Verdict: approve-with-nits. The reviewer checked wire placement against KAS 0.66.43's `S8e` reader and re-checked TUI
parity at every offset. Its mutations M1–M3 were all caught. The repairs below are in their own commit, separate from
the queued-approval change (1cea60dd).

| # | Finding | Repair |
|---|---|---|
| R1 | `UiState.always_allow_scope` lasted for the whole process. After one `user` grant, every later scope page opened at the widest scope. The TUI's `k2t` carry-over lives in the approval component, which is mounted only while an approval is pending (byte 13561339), so it resets when the queue drains. | It now resets to `session` when a request arrives to an empty queue (`show_approval`). Red: `scope_resets_to_session_once_the_queue_drains` failed before the repair (`receipts/scope-reset-red.txt`). Carry-over within one queue is pinned by `next_queued_scope_page_opens_at_the_last_scope`. |
| R2 | `approval_cancel` EnterFolder→SelectScope hard-coded `owned: true`. | `EnterFolder` now carries `owned`, which is used by both confirm and cancel. Behaviour is unchanged, because only owned requests are offered a folder. |
| R3 | A foreign option id was logged twice (the converter's foreign-id warn and `attach_consent_echo`'s warn). | `attach_consent_echo` now warns only for an offered option of another kind. A foreign id returns silently, since the converter has already warned. |

## Round 3 — delta review of 4572fc58 (2026-10-18, relayed by the coordinator)

Verdict: APPROVE. No high or medium findings. Coverage is nowhere broader than the TUI, the `D4`/`k8n`/`Clt`
equivalence was verified at every byte offset, and the reviewer's mutations on root and owner were each caught. This
round closes its LOW test gaps in a single commit that changes tests only.

| # | Gap | Repair |
|---|---|---|
| L1a | `D4`'s queued-side `triggeringResource ≠ resource` exclusion (TUI 11403660) was not pinned | `child_queued_triggering_differs` |
| L1b | No case for a main-session Folder grant releasing nothing | `main_folder_releases_nothing` |
| L1c | The no-capability `CapabilityKey::Tool` path was not pinned | `tool_key_same_tool`, `tool_key_other_tool` |
| L2 | `capability_key` falls back to the tool call's title, while the TUI's `E8n` (11406092) tries the tool_use message *name* before the title | Not changed. Recorded in `design.md`, because it is not a one-liner: cyril's approval has no tool_use message name to consult. It is also unreachable today, since KAS always sends `toolId` in `_meta.kiro`, which both sides use first. |

Each new case fails when the code it tests is disabled, and passes again once the file is restored
(`receipts/review3-mutations.txt`):
- (a): only the new case fails.
- (b2): only the new case fails.
- (c): only the new case fails.
- (b), with the whole guard removed: the new case fails along with the two existing exact and pattern cases, because
  one guard covers all three.
