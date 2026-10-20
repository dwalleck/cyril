# cyril-xavt route

Date: 2026-10-19; delivery reconstruction: 2026-10-20. Original design base: `8e5c325e`. Actual delivery base: published EJ commit `9400e7c8ddd53776f6fd8b4d28d4c7fc69e45ab4` (PR190), following un4p PR188. Full Rivets record was read during original implementation, including notes and relationships (none). Scope is shell command/cwd presentation, not permission-grant semantics.

## Route tests

| Test | Verdict and evidence |
|---|---|
| T1 empirical premise | No unresolved design premise. Installed @kiro/agent 0.66.43 and committed live capture `experiments/conductor-spike/audit-2.32.0/live/kas-clientcreator-2.32.0.jsonl` lines 324/328/335–337 establish rawInput.command, optional rawInput.cwd, execute kind, run_command permission metadata, and stub joining. Repaired live UI evidence is still required acceptance, not an unknown payload/design premise. |
| T2 structural module shape | Yes. Add typed ShellCommand data to the core ToolCall interface, classify in convert/kas after the existing permission-ledger join, then consume typed data in UI. Existing owners and core→UI dependency direction stay intact; App remains wiring only. |
| T3 production-scale risk | No new material risk if the display window remains bounded and nonvisible command rows are counted rather than allocated. Full raw command retention already exists. No new process/concurrency or execution responsibility. |
| T4 explicit behavior | Yes. Render the full command (wrapped, bounded and safe from control/bidi display manipulation) plus cwd only when supplied; keep unavailable marker for genuinely absent payloads. Titles are not authoritative payloads. Preserve current modal/input/selection/path guarantees and require actual command context alongside a confirmable choice. Widget test, long-pipe insta snapshot, and actual KAS full-command-before-Allow evidence are mandatory. |

Selected route: **Structural**, because the typed core/UI interface changes. Downstream: falsifiable-design → approved design → budgeted-plan → checkpointed-build.

## Artifact ownership and terminal criterion

- `route.md`: change-workflow.
- `design.md`: falsifiable-design, including primary-source anchors, typed ownership, sanitizer fidelity and bounded window verification.
- `plan.md`: budgeted-plan after approval.
- Separate interrogated spec/probe: unnecessary; requested behavior is explicit and installed source plus retained live frames resolve the payload premise.
- Terminal: all claims and required real KAS rendered smoke, repository gates, independent review, issue commit and own PR have valid evidence. Parent owns verification and publication. No build/test/lint/format command, commit, push, or tracker write by this worker.

## Status

DG approved in design.md. Actual X source is reconstructed on published EJ base9400e7c8ddd53776f6fd8b4d28d4c7fc69e45ab4 with common test-only auth prerequisite6c0f41c74bd94fa49ab1b3a2ae06e3ed70d3de69. The four formatting-context hunks were reconstructed in place and independently reviewed. Typed post-ledger shell projection, borrowed APIs, safe bounded grapheme paging, whole one-grapheme confirmation and cwd-only refusal are delivered; permission authority is unchanged. Private Option<Box<ShellCommand>> is the approved owner-level representation; interim replay boxing is absent.

**Implementation/qualification handoff complete.** Actual-delivery parent gates860 pass; canonical fresh actual-source review is correct/confidence0.98 and explicitly covers formatting-only finalization and the exact F3 reference restoration. The historical combined real-KAS run proves full command/cwd inspected before one AllowOnce and execution-wire exit0/sentinels, but not rendered stdout or a clean model turn. The first-attempt actual-delivery owned-peer supplement preserves the exact captured session/tool/permission payloads, proves full892-byte command/cwd plus refusal/draft/selection controls, then one RejectOnce and clean exit0 without execution. verification.md owns the exact evidence and the post-gate source-freeze manifest; no gate-time tree hash is fabricated.

**Publication remains Main-owned:** stage only the exact publication manifest, commit, update the tracker, and open this issue's own PR. This worker ran only the explicitly authorized bounded owned-peer consumer proof, not repository checks or a second live KAS/account/model operation; no commit, push or tracker write was performed. un4p → ej7y → xavt remains the stack. This record does not declare the parent-owned publication terminal criterion complete.
