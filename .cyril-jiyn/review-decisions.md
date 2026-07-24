# cyril-jiyn — pre-PR review decisions (2026-07-19)

Two-axis review (standards + spec) via parallel agents. Each finding verified
before applying.

## Standards axis (clean — zero hard violations)

| # | Finding | Category | Verified? | Decision | Note |
|---|---|---|---|---|---|
| S1 | 3× repeated serialize→RawValue→ExtResponse in the responders | Duplicated Code | Yes (read all 3) | Accept | `json_ext_response` helper collapses it |
| S2 | params-parse `unwrap_or(Null)` swallows error without log | silent-failure nit | Yes (unreachable in practice; RawValue pre-validated) | Accept (light) | `parse_ext_params` with a `debug!` — restores log-before-fallback consistency |
| S3 | signal death → 137 always (SIGTERM should be 143) | precision nit | Yes | Reject | `.code()` gives no signal number cross-platform; 137="killed", documented, nonzero — the point (not-a-0-sentinel) holds |
| S4 | `file_stem().unwrap_or("hooks")` no log on non-UTF8 name | silent-failure nit | Yes | Reject | Pathological input; names still differ so ids stay distinct; negligible |
| S5 | wire triggers as `&'static str` vs enum | Primitive Obsession | Yes | Reject | Deliberate wire pass-through; the PascalCase→camelCase map is the typed layer |

## Spec axis (three real findings)

| # | Finding | Category | Verified? | Decision | Note |
|---|---|---|---|---|---|
| P1 | Claim 12 (didChange) named fence `hooks_did_change_consumed` absent | Missing test | Yes (grep: no such test) | Accept | Added `hooks_did_change_consumed` — injects the notification, asserts Ok |
| P2 | Claim 13 (non-blocking) substituted a weaker oracle | Partial fence | Yes — AND my substitute test had a bug (join! elapsed always ≥ hook time) | Accept | Wrote the real `slow_hook_does_not_block_loop`: shell_type resolves <2s while a 3s hook runs, timing captured at resolution not after join. The bug the reviewer predicted was real — caught it because the first version FAILED |
| P3 | **sessionStart: pause approved "EXECUTE"; impl only ACKNOWLEDGES `{results:[]}`, defers exec to cyril-tpfd** | Wrong vs signed-off decision | Yes | **Surface to approver** | Deferral is honestly tracked (cyril-tpfd filed) and evidence-driven (AcpPrecomputedHookResult shape not verifiable — don't-guess-a-wire-shape). BUT it reverses a HARD-PAUSE decision; the reviewer is right that this should go back to the approver, not be decided at build time. Raised at the merge pause. |
| P4 | Covenant hooks section not updated (design item 5) | Docs gap | Yes | Reject (tracked) | Covenant re-sync is cyril-mfkg (pre-existing); ROADMAP + conductor-spike README done |

## Outcome

Applied: S1, S2, P1, P2 (4 commits or one fixup commit). Rejected with
evidence: S3, S4, S5, P4. **Surfaced, not decided: P3** — the sessionStart
scope reduction reverses a pause-approved decision and is the approver's call
at merge time.

Notable: P2's verification found a genuine bug in my own substitute test
(`join!` measures both futures' completion, so elapsed-after-join always
included the hook's full runtime). The corrected fence captures the timing at
shell_type resolution and now actually proves non-blocking.

# cyril-jiyn — post-PR two-axis review decisions (2026-07-23, PR #62)

Second two-axis review, run against `main...HEAD` after the PR opened.
Findings verified per gilfoyle/assessing-review-feedback; three re-litigated
points from the pre-PR review above were rejected by citing the prior
decision. Applied fixes: commit `a63a4ed` (N1–N3) and `19c0d25` (N10).

## Standards axis

| # | Finding (one line) | Category | Verified? | Decision | Note |
|---|---|---|---|---|---|
| N1 | `entries.flatten()` drops per-entry read_dir errors | Bug | Yes — contradicts `load()`'s own "every per-entry problem is a warn + skip" doc contract | Accept | `a63a4ed`: match + warn + skip |
| N2 | `respond_list` trigger `unwrap_or("")` + `executeHook` userPrompt fallback, both silent | Standards | Yes | Modify | `a63a4ed`: the `""` trigger was also a sentinel — replaced with logged let-else early empty reply + `respond_list_missing_trigger_replies_empty` fence; userPrompt fallback now warns |
| N3 | `code().unwrap_or(137)` silent on signal death | Standards | Yes | Modify | `a63a4ed`: warn added; the 137 mapping itself stays per prior S3 rejection (no cross-platform signal number worth forking for) |
| N4 | `parse_ext_params` logs `debug!`, CLAUDE.md says warn | Style | Yes | Reject | Already decided: prior S2 chose `debug!` deliberately (unreachable in practice — RawValue pre-validated) |
| N5 | Reply JSON hand-built in 5 spots; `wire_trigger` as `&'static str` | Design | Yes | Reject | `json!` is the established kas-responder idiom (`json_ext_response` pins the plumbing); enum half already rejected as prior S5 (wire pass-through; the PascalCase→camelCase map is the typed layer) |
| N6 | Method if-cascade grows in both `handle_ext_request` and `ext_notification` | Design | Yes | Reject | Matches the existing client.rs dispatch idiom (auth/shell_type); revisit as a dispatch table if it grows again |
| N7 | `respond_execute` runs wire-supplied command verbatim; registry never consulted | Design/security | Yes — covenant makes executeHook an echo of list-served commands, and says the client "handles approval" | Reject (defer) | Filed **cyril-qr6l** (discovered-from cyril-jiyn): probe whether KAS echoes ids/commands unchanged, then cross-check + warn/deny on mismatch — the org-gate integrity property |
| N8 | hooks.rs (705 lines) is two halves; split | Polish | Yes | Reject | Registry + executor are two cohesive halves of one covenant surface; split when a third concern arrives |

## Spec axis

| # | Finding (one line) | Category | Verified? | Decision | Note |
|---|---|---|---|---|---|
| N9 | sessionStart stub reverses pause-approved EXECUTE | Spec deviation | Yes | **Surface to approver** (again) | Same as prior P3 — deferral to cyril-tpfd is honest + evidence-driven but must be ratified by the approver at merge; still open |
| N10 | Wire-audit doc lacks the decided default + no-composition result | Docs gap | Yes — narrower than reported: the hooks section exists, the decision record was absent | Modify (absorb) | `19c0d25`: decided-default bullet added to the wire-audit hooks section; covenant half stays tracked at cyril-mfkg |
| N11 | A/B capture partial (`prompt_completed: false` both arms; preToolUse never fired on 2.13.0) | Evidence quality | Yes | Reject | Honestly caveated in findings.md; exit-2 block rests on the 2.7.1 end-to-end capture + source continuity; per-release both-arms fence stands |
| N12 | cyril-jmjb absorbed by slice 0 but still open | Process | Yes (rivets: open) | Accept (merge action) | Close cyril-jmjb alongside cyril-jiyn when PR #62 merges |
| N13 | Windows `cmd /C` path + tests were unrequested | Scope | Yes | Accept (keep) | CI-forced platform fix; Windows is a supported platform (CLAUDE.md Platform Constraints) |

## Outcome

Applied: N1, N2, N3 (`a63a4ed`), N10 (`19c0d25`). Rejected with evidence:
N4, N5, N6, N8, N11 (three by citing pre-PR decisions). Deferred with
tracker: N7 → cyril-qr6l. Merge actions: N12 (close cyril-jmjb).

**N9 RATIFIED by the approver at merge (2026-07-23):** the sessionStart
execute→stub deferral stands; execution ships with cyril-tpfd once the
`AcpPrecomputedHookResult` shape is verifiable. This closes the last open
deviation from the pause-approved design.
