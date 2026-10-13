# Route: cyril-lki9

Change: Stop cyril-invoked KAS workflow runs from producing an unsolicited, billed model turn in the user's main session (KAS `autoWakeParentOnComplete`), or make any such turn first-class and visible.
Date: 2026-09-29; resumed 2026-10-12.

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | The original route required observations of KAS server-started turns, cancellation, steering, metadata, and injections. `evidence.md` P1–P8 resolves the premises used by the approved option b; the committed 2.26.0 captures remain its reference inputs. Missing-parent and control-session behavior is irrelevant to the rejected options. No new external premise is introduced by the acceptance repair. | **resolved; retain Empirical history** |
| 2 | Structural module shape | The approved change added typed turn/origin notifications, deepened turn ownership in `TurnMediator`, and connected existing session, workflow tracker, UI, and App owners. `design.md` Placement and its approved ledger own the boundaries; `conformance-review-3.md` covers the delivered production shape. The resumed work records the existing decision in ADR-0011 and restores/verifies required acceptance evidence, without changing production ownership or interfaces. | **yes — existing approved change** |
| 3 | Production-scale risk | The resumed acceptance work introduces no production-scale change. Billed follow-up turns remain an explicit approved behavior, not a cost eliminated by the fix. | no |
| 4 | Explicit behavior | `spec.md` B1–B7 and Decisions, approved 2026-09-29 with the 2026-09-30 claim-at-turn-start amendment: retain the parent link, label tagged wakes, busy/watchdog/cancel/steer parity, inline engine notices, and operator-steer isolation. The later give-up correlation is separately tracked by cyril-pd4b, dependent on cyril-fb1m, not part of this repair. | **yes** |

Unknown tests: none

## Selected route

Empirical — adopt the existing route for the unchanged approved request. Its external premises are resolved in `evidence.md`; resume at checkpointed-build's final integration / bounded acceptance repair, not a new specification or design.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | approved; B1–B7 and amended correlation remain unchanged |
| evidence.md, probe.* | prove-it-prototype | retained P1–P8 observations for the committed reference captures |
| design.md | falsifiable-design | approved; claims and ownership remain unchanged |
| plan.md | budgeted-plan | owns final integration and resumed completion evidence |

Verification follows the approved design claims. The existing replay obligation remains required; a converter census or TUI smoke alone does not replace the assembled capture replay.

## Downstream sequence

Original: interrogated-spec → prove-it-prototype → falsifiable-design → budgeted-plan → checkpointed-build. Resumption: checkpointed-build final integration, with its receipt incorporated into `plan.md`.

## Terminal criterion

Empirical — prove-it-prototype records PASS for every empirical premise, every later artifact satisfies its owning stage's completion criterion, and checkpointed-build records no FAIL.

## Completion

PASS — the [resumed completion record](plan.md#resumed-completion-2026-10-12)
owns the delivered ADR amendment, restored capture replays, mutation
sensitivity, consumer smoke, repository checks, independent review, and
source-bound receipts. The approved production behavior is unchanged.
The PR #166 standards round and its repairs are recorded in
[review-decisions.md](review-decisions.md). Branch-local closure of cyril-lki9
and cyril-5n75 and PR publication are authorized by the
[publication record](plan.md#publication-authorization); merging and closure
of any other tracker issue are not authorized.
