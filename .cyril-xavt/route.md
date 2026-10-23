# cyril-xavt route

Date: 2026-10-19; historical delivery reconstruction: 2026-10-20. Original design base: `8e5c325e`; reconstruction base: published EJ commit `9400e7c8ddd53776f6fd8b4d28d4c7fc69e45ab4` (PR190), following un4p PR188. Current publication checkpoint: held HEAD `bc6f48a21016320cc6491356f880f37463cf8c6e` plus MERGE_HEAD `af78b1615bd5b97b4c702542fd3ab193151bd4c1`, not HEAD alone. Full Rivets record was read during original implementation, including notes and relationships (none). Scope remains shell command/cwd presentation, not permission-grant semantics.

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

**Approved contract and historical reconstruction.** The original DG, T1–T4 and requirements remain unchanged. Original X source was reconstructed on EJ `9400e7c8ddd53776f6fd8b4d28d4c7fc69e45ab4` with common test-only auth prerequisite `6c0f41c74bd94fa49ab1b3a2ae06e3ed70d3de69`; four formatting-context hunks were reconstructed and independently reviewed. Typed post-ledger shell projection, borrowed APIs, safe bounded grapheme paging, whole one-grapheme confirmation and cwd-only refusal retain unchanged permission authority. The approved private `Option<Box<ShellCommand>>` representation remains; interim ReplayFrame boxing is absent.

**Current held af78 checkpoint.** Main's added modal ownership DG is appended separately in [design.md](design.md#modal-ownership-repair--additional-main-dg); it preserves the original contract while migrating twelve existing clearing callers to whole-painted-span eviction. The current 532-input descriptor is `b6a846bee5f24ab0c2d0b1a48f066dd69ae575abb82bc7033c3f42a56ff48308`, with private App SHA-256 `8e2b7c3a82b360d4ca4f1c1b6410084bf3a5a9e1673612d04bfd6c61739ee7f0`. Current source review is NONE / correct / 0.97, source-only; Main retains its applicability after authoritative rustfmt-only normalization. Both exact-once owned consumer cases passed. [Current verification](verification.md#current-held-af78-checkpoint) owns the initial fmt FAIL, seven later successful command records, retained exact10/Core evidence, original controlled FAIL and current two-case receipts; the [portable index](evidence-current-modal532-b6a846be-index.json) maps immutable originals to bundle members.

**Publication remains Main-owned:** final allowlist, held-merge completion, staging, commit, tracker actions and existing PR191 update. Historical gates860 / review0.98 / private338 and the original combined real-KAS run retain only their recorded historical scope; they are not current qualification. This metadata refresh performs no Git, checks, formatter, runtime, account/model operation or publication. Native glyph/reflow, the separate final-integration MCP thirteenth caller, CI, final assembly and whole-issue/global PASS remain outside this checkpoint. The original un4p → ej7y → xavt dependency order and parent-owned terminal criterion remain intact.
