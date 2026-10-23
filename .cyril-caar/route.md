# cyril-caar route

## Current draft publication — 2026-10-22

**Native acceptance BLOCKED by authentication; draft only.** The reachable publication step is an explicitly pending draft, not the terminal acceptance criterion. Main authorizes documentation/proof publication while native authentication is blocked; the issue stays open.

The [current proof index](current-main-proof/index.json) owns source/binary identities, the sealed local qualification, independent-review applicability and pending gates. See [local qualification](current-main-proof/qualification.json), [review applicability](current-main-proof/review-applicability.json), [native attempts and blocker](current-main-proof/native-consumer.json), and the [draft PR body](pr-body.md). No native/CI/final-conformance/merge-ready or issue-closure claim follows from this publication. Logical delivery remains Tqr #193 → NK #196 → Caar; the draft targets main for existing CI and explicitly depends on #196.

## Retained approved contract and canonical history

Imported from the owning canonical Caar record. Approved requirements remain governing; historical source pins, patch recipes and check statuses below remain attributed to their named earlier frames, not to the current qualified source. Archival patches/private receipts referenced below are not implicitly shipped or authorized for reapplication. Current status is the proof index above.

Date: 2026-10-19. Base `8e5c325e`; linked worktree only. Full `rivets show cyril-caar --json` read, including acceptance and empty notes/relationships.

## Route tests

| Test | Verdict and evidence |
|---|---|
| T1 — empirical premise | P1a is resolved by the isolated 2.32.0 v2 observation: `_kiro.dev/mcp/startup_status` is a request requiring `sessionId`; result includes `allStarted`, named `pending`, `failed` and `determinable`. **P1b remains unproven:** timely complete inventory when session/new itself stalls; the observed query answered alongside new completion. Spec D1 now explicitly approves naming known facts and reporting that the agent supplied no complete inventory when absent. No source/timing guarantee is invented. See `evidence.md` and `startup-status-2.32.0.json`. |
| T2 — module shape | Yes for the complete proposed correction: a private startup-attempt owner gathers inventories/progress and enriches the serial timeout outcome; KAS MCP snapshot ownership comes from `nk4o`. No duplicate Kiro filesystem/config resolver is permitted. Public seams need not grow solely for the deadline. |
| T3 — production-scale risk | No new performance feature. A dedicated >=90-second new-session wait intentionally permits slow server/OAuth initialization while ingress and host callbacks continue. Preserve existing load/control/prompt budgets, single serial domain ownership and fail-stop cleanup; no polling/retry framework. |
| T4 — explicit behavior | Yes, including requester-approved spec D1. Keep >=90s separate new budget, full early-event retention and truthful named timeout diagnostics; when no authoritative complete inventory exists, explicitly report the agent supplied none rather than infer silent names or report `none`. |

**Selected route: Empirical**, taking precedence over Structural. This is not a constant-only Local fix. Raising a timeout alone would leave required diagnostics/proof unresolved and is not implemented as a pretend complete slice.

## Artifacts and owning stages

- `route.md` — change-workflow.
- `evidence.md`, `probe-startup.py` and `startup-status-2.32.0.json` — prove-it-prototype; P1a observed, P1b explicitly pending. Existing source establishes early KAS snapshots and the nonblocking domain path.
- `spec.md` — interrogated-spec owns the requester-approved absent-inventory decision D1.
- `design.md` — falsifiable-design proposal, incorporating D1 while preserving P1b's factual limitation.
- `plan.md` — budgeted-plan after applicable approval/premise gates.
- checkpointed-build, independent code review, distinct `caar` PR — parent-owned; no commit/push/publication by worker.

No supplied feature budget/hard size cap. Existing lints/ownership requirements remain unchanged.

## Terminal criterion / current gate

Dedicated session/new budget >=90s, attributable pre-response MCP events and full approved reported/silent/OAuth/unknown diagnostics. Main approved stack and implementation. **Full caar source and clean post-repaired-NK patch boundary delivered; PENDING — parent actual RED/GREEN, real consumer/repository proof, post-smoke docs, independent review and distinct PR.** NK owns the sole admission guard; caar retains timer/generation/fact responsibilities. Raw premise and standalone fixture receipts are not Cyril verification.
