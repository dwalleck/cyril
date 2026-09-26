# Route: cyril-y628

Change: Add optional tool-rejection feedback to Cyril's approval overlay and send it as KAS rejection metadata.
Date: 2026-09-26
Repository state: main merge cede8f5f (PR #130), tracker closure dc1462b6, local claim commit 83b1ee65 on feat/cyril-y628.
ROADMAP ownership: KAS engine integration, KAS-2a tool-approval path; not auto-approval policy.

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | Existing retained evidence: docs/kiro-2.24.0-wire-audit.md §7.4; experiments/conductor-spike/probe-kas-new-surface-2.24.0.py:188-199 constructs response-level _meta.kiro.rejectionReason; paired kas-new-surface-turns-0668-2.24.0.jsonl and kas-new-surface-turns-0660-2.24.0.jsonl show a nonempty reject_once reason reaches failed tool output/model on KAS 0.66.8 and is ignored on 0.66.0. This change uses that same wire contract without changing the external behavior assumption. No assumption that v2 consumes feedback, that reject_always consumes it, or that empty feedback has special server semantics. | no |
| 2 | Structural module shape | Public PermissionResponse in crates/cyril-core/src/types/event.rs lacks rejection feedback; ApprovalPhase/ApprovalState in crates/cyril-ui/src/traits.rs lacks text-entry state. Core conversion owns ACP encoding; KAS-specific metadata belongs to protocol/convert/kas.rs (the approved design refines the initial candidate kiro.rs to the existing KAS owner). UiState owns approval transitions/draft, widgets/approval.rs owns rendering, App owns input routing. Extend these existing seams while preserving owners and dependency direction. PermissionRequest.responder oneshot—not BridgeCommand—carries decisions through DomainMediator::handle_permission. Protected parents: App, UiState, generic convert/mod.rs; do not add wire parsing or UI business logic to App, or KAS-specific parsing to generic conversion. Candidate owners remain those existing modules. | yes |
| 3 | Production-scale risk | Human-entered feedback on one active queued approval; no new background work, algorithmic load, concurrency owner, or bulk-data path. Preserve nonblocking permission task and FIFO queue. Input/render bounds are a UI design obligation, not a production-scale pipeline change. | no |
| 4 | Explicit behavior | Ticket explicitly names optional rejection feedback on reject_once and the top-level wire field, but does not settle entry affordance, preservation of one-action plain rejection, cancel/back behavior, input/paste semantics, or whether unsupported engines expose the affordance. These affect observable behavior and need a specification approval. | no |

Unknown tests: none. T4's unresolved behavior is identified, not assumed.

## Selected route

Structural — public domain/UI interfaces change and observable approval UX needs specification; existing paired live evidence covers the external premise.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | required — unresolved approval UX (T4 no) |
| evidence.md, probe.* | prove-it-prototype | N/A — existing applicable paired evidence covers the wire premise (T1 no) |
| design.md | falsifiable-design | required — public permission and approval-state interfaces change |
| plan.md | budgeted-plan | required — Structural route |

Oracle checkpoint in checkpointed-build: required — Structural route.

## Downstream sequence

interrogated-spec → falsifiable-design → budgeted-plan → checkpointed-build

## Terminal criterion

Structural — satisfied. Specification/design approvals are recorded in their owning artifacts. plan.md's final checkpoint records all eleven gates PASS (with explicit inapplicable budget classes); conformance.md records the isolated structural PASS. No unresolved FAIL or deferred acceptance obligation remains. Publication/tracker closure is outside this local implementation authorization.
