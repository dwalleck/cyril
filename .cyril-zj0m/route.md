# cyril-zj0m route

Initially routed 2026-10-19 on base `8e5c325e`; finalized documentation/evidence 2026-10-20 on the exact actual delivery identified in [verification.md](verification.md). Original Rivets records were fetched with `rivets show cyril-c6la cyril-zj0m cyril-x5xi --json`. Main owns source integration, Cargo qualification, review commissioning and publication. This documentation worker changed no source, fixtures, tracker, commits or pushes; the one later authorized direct benchmark CLI rejection is recorded separately.

## Requested behavior

Completed Write diff derivations must be reused: one derivation supplies summary and rendered lines, unchanged historical diffs are not recomputed per frame, and coherence tests plus a repeated-render benchmark pass. Preserve pending approval preview behavior and source merge semantics. This issue must have its own independent code review and substantive PR.

## Route tests

| Test | Verdict and evidence |
|---|---|
| T1 Empirical premise | **No.** Source determines all required inputs/lifecycles. `widgets/chat.rs:377-387,393-486` computes Myers twice for an ordinary completed Write. `TrackedToolCall::{new,update}` is the centralized UI-owned mutation seam (`traits.rs:356-403`), and domain content merge behavior is explicit (`cyril-core/src/types/tool_call.rs:148-188`). No external/backend observation is necessary. |
| T2 Structural module shape | **Yes.** A retained, theme-independent derivation becomes responsibility of `TrackedToolCall`, backed by one private UI derivation module; chat and approval migrate to that interface. The core data type must not own presentation context/row-limit policy. |
| T3 Production-scale risk | **Yes.** Large Myers inputs, duplicate active/history snapshots, cache detachment on content updates, and storing copied source lines can create avoidable compute/memory cost. Demand-driven initialization and shared immutable compact rows address that risk; behavior/work tests and production-shaped repeated renders qualify it. |
| T4 Explicit behavior | **Yes; approved.** Preserve status/header colors, line-number gutters, one context row, 20-row body limit plus ellipsis, old-text absence as empty source, and approval Diff previews regardless of completion. The explicit user decision in design.md Approval matches summary to the **first Diff**, preserving body selection and hiding misleading later-Diff counts when the first is unchanged. |

**Selected route: Structural.** The behavior and lazy tracked-source ownership approvals are recorded in design.md; implementation is delivered without broadening to multi-file display or changing the source merge contract.

## Evidence and affected consumers

- Chat `render_tool_call` is the sole historical tool renderer; main and focused child both dispatch to it (`widgets/chat.rs:175-203,311-391`).
- Approval preview invokes `render_diff_lines` without a completed-Write gate (`widgets/approval.rs:466-519`). Completion-only reuse must not remove that path.
- Main starts create both active and historical tracked snapshots, and updates merge both (`state.rs:551-580`). Child uses the same wrapper (`subagent_ui.rs:118-133`); approval creates an immutable tracked snapshot (`state.rs:1799-1825`).
- Empty content preserves prior content; nonempty content replaces it. Metadata-only updates must not invalidate a text-only derivation (`types/tool_call.rs:148-188`).
- The current summary fixture does not assert the exact counts its comment names (`chat.rs:1701-1735`); body/truncation fixtures exist at `:1631-1698,1758-1800`.

## Artifacts and downstream gate

`route.md` belongs to change-workflow. `design.md` belongs to falsifiable-design; its Approval section records **DG approval of lazy tracked-source derivation** and the **explicit user first-Diff-for-both approval**, relayed by Main on 2026-10-19. Production work may proceed. Budgeted-plan owns `plan.md`, then checkpointed-build records delivered proof. No separate spec/probe/evidence artifact is needed: behavior is stated and source resolves premises.

Recommended delivery order: `cyril-x5xi` → `cyril-zj0m` → `cyril-c6la`, one independently verifiable commit/PR boundary per issue. This PR adds coherent reuse and the repeated-Write benchmark even before viewport virtualization; the following PR adds long-history/visible-window behavior rather than an empty duplicate fix.

Terminal criterion: all issue outcomes, relevant tests, benchmark and real consumer smoke, repository gates and independent review have valid evidence; no blocking defect; then parent opens this issue's PR. Preparation is not issue completion. No universal feature budget or hard line target is imposed.

## Delivered implementation and checked boundary

The seven issue-owned source files are `tool_diff.rs`, `traits.rs`, `widgets/chat.rs`, `widgets/approval.rs`, `lib.rs`, the UI Cargo manifest and `benches/conversation_render.rs`, all under `crates/cyril-ui/`. [publication-manifest.json](publication-manifest.json) is the exact parent publication allowlist, including their qualified hashes and this issue's README/workflow/evidence files. No other issue directory, fixture or source is authorized by that allowlist.

One lazy theme-independent derivation supplies first-Diff counts, bounded source-range rows and truncation state. Initialized clones share immutable data; equivalent source/status/metadata updates retain it and source replacement detaches it. Chat and pending approval borrow the selected source and apply current colors. No copied old/new/row text is retained; core source merge semantics and historical ordering are unchanged.

The exact checked actual tree is `6241931fecb663e9bb17e5cc0e76c35c12833828`, HEAD `6ab6d77e03b73a3a45d6d526fbc19641dace5513`, based on the old PR #189 stack plus the common fixture prerequisite. This is not the later output-alias refresh and does not prequalify any future merge/rebase. Post-qualification edits are only README and `.cyril-zj0m` documentation/evidence. [verification.md](verification.md) owns the source pins, gates, raw benchmark, separate CLI rejection, reviews and actual-App consumer receipts.

## Verification and review disposition

The full actual-tree workspace tests, all-targets workspace Clippy, CLI build and format check passed. The completed-write benchmark exercised sparse-2k/dense-20k/pending controls with cold and warm samples; timings are report-only, with no unmeasured baseline or acceptance threshold. The owned actual-App PTY consumer passed first attempt, including pending RejectOnly comparison and turn completion. README now documents the delivered first-Diff and reuse behavior.

The F1 missing `PermissionOption.is_destructive` regression fixture repair is source-reviewed and executed successfully in the actual-tree workspace run. The first qualification attempt's benchmark lint failure remains **historical FAIL**, not a waived gate; the owner repaired it before fresh actual-tree qualification. [review-decisions.md](review-decisions.md) and [verification.md](verification.md) separate those repairs and their evidence.

The original pinned `.cyril-zj0m/production.patch` remains an immutable historical sensitivity boundary, not the current assembled diff. Its original-base executed RED/GREEN is retained with raw logs/manifests in the portable archive. Independent review of that patch is not silently relabeled actual-tree review; the current review conclusion and scope are in the verification index.

## Qualification limits and publication ownership

Actual-App ANSI/pyte cell observations are not native graphical-terminal font/glyph proof, a live Kiro/model turn, a permission grant, native 50,000-message qualification or a later c6la/output-alias qualification. TestBackend timing excludes terminal I/O. Ignored workspace tests are not exercised coverage.

The inherited requester-accepted `cyril-u62f` Windows readiness baseline exception remains bounded by [the public acceptance record](https://github.com/dwalleck/cyril/pull/183#issuecomment-6284441811) and `limits/windows-baseline-exception.md` in the portable archive. Cancelled coverage is not green; the exception authorizes no retries, skips, runtime changes or new failures.

Main owns the issue-specific commit/PR, CI adjudication, tracker closure and later stack refresh. Those publication actions have not been performed by this worker. The portable handoff and manifest are the delivery boundary, not a claim that publication or future-source qualification has happened.
