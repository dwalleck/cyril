# cyril-4c4d — KAS as the default engine

Date: 2026-10-12. Route owner: Gilfoyle change-workflow.

## Inputs and source state

Requester: “Claim and implement cyril-4c4d”. The authoritative behavior and acceptance contract is `rivets show cyril-4c4d` (issue updated 2026-10-08). Implementation worktree: `feat/cyril-4c4d-default-kas`, based on fetched `origin/main` at `99ed7ce52a4ae7c422ebd62dd98e5f73b707901e`.

The primary checkout has unrelated local audit/tracker commits and is not rebased or published by this change. The initial claim was correctly refused while cyril-igrx was open. PR #167 subsequently merged as `b9c33f026b75ec3d229fe7bb3b8433f8cb218c60`; this session preserved local main history with merge `bc5d00aa` and claimed/started cyril-4c4d in `5ee8bc07`. The implementation branch integrated only published upstream in `808d0bc7` and the claim in `1a610946`, excluding unrelated unpublished tracker relationships. No blocking edge was removed and no other session's work was closed.

## Routing evidence

| Test | Verdict | Evidence |
|---|---|---|
| T1: consequential unresolved external premise | No | Existing discovery supplies the actionable first-run policy. Exact hook selector grammar and KAS slash-prompt parsing were resolved from versioned bundles and existing captures, with provenance in `design.md` D3–D4. Real runtime verification remains an implementation obligation. The remaining local lesson/context choice is a product boundary, not an unknown upstream fact. |
| T2: structural interface/ownership/security boundary | Yes | `crates/cyril/Cargo.toml:17-24` disables KAS by default; `AgentEngine::default()` selects V2 (`agent_engine.rs:11-24`). Enabling credential-reading code by default explicitly changes ADR-0002's compile-time guarantee and the library default-selection contract. Existing owners stay in place; no module split or new abstraction is proposed. |
| T3: introduced production-scale risk | No | This switches to an existing engine and spawn path. No new queues, concurrency, storage, or algorithms are introduced. Different engine startup prerequisites and credential custody are compatibility/safety decisions covered by the design, not invented performance budgets. |
| T4: explicit requested behavior | Yes | `spec.md` owns the approved prerequisite repairs and native-memory policy; `design.md` Approval resolves D4's exact command-text/local-context boundary. Default-switch outcomes, compatibility, important errors, safety and scope are now explicit. |

## Selected route

**Structural.** The feature and runtime defaults amend a documented credential-custody boundary. No empirical prototype is required: the proposed first-run behavior preserves existing discovery and error routing rather than inventing fallback semantics.

## Required artifacts and sequence

- `route.md` — change-workflow: routing, authoritative scope, dependencies.
- `spec.md` — interrogated-spec: requested behavior, parity evidence, prerequisite/scope and memory-policy decisions, requester approval.
- `design.md` — falsifiable-design, after specification approval: defaults, credential policy, placement, verification, requester approval.
- `plan.md` — budgeted-plan, after design approval: independently green implementation slices.
- checkpointed-build — implementation, consumer and repository checks, independent review and final assembled proof.

The parity audit required specification decisions; `spec.md` now records the requester’s answers. No probe or independent reimplementation is justified for the existing first-run policy. Resolve the command-routing and hook-matcher contracts from existing source before design, reassessing T1 if a consequential premise remains unknown. No feature budget or hard size limit was supplied.

## Pending dependencies and terminal criterion

- SATISFIED — prerequisite owner landed PR #167 / cyril-igrx; its closed state and platform source are integrated. cyril-4c4d is claimed/in progress.
- SATISFIED — specification/design approvals are recorded; all three implementation slices completed under the approved contracts.
- SATISFIED at `a813eed9` — every ticket outcome, explicit V2/no-default compatibility, affected callers/docs, real consumer checks, all-platform repository gates and independent review are recorded in `plan.md`. The earlier intermittent Windows readiness observation is retained without claiming an unproven fix. Delivery remains the authorized draft PRs.

Routing completion is not implementation completion. The change is complete only when those artifacts satisfy their owning criteria and no blocking defect or required proof remains unresolved. Publication/merge and issue closure require the workflow's scoped authorization; they are not implied by preparation of this route.

## Publication authorization

Requester selected **“Publish draft PRs”** when asked to push the two prerequisite repairs and the default-engine branch for required Linux/macOS/Windows CI. This authorizes those draft PRs only; no merge, issue closure, or publication of unrelated main commits.
