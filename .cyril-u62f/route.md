# cyril-u62f — current C6 workspace/all-features diagnostic increment

Date: 2026-10-22. This section is the active route for `diag/cyril-u62f-all-features`; the PR195 record below is retained verbatim as historical evidence, including its then-current labels and outstanding actions.

## Current authorization and composition

The assignment authorizes: **“Implement full settled agent://PlanWindowsAllFeatureCapture plan in newWT.”** Its worker boundary is diagnostic preparation only: no builds, tests, formatter commands, CI launch, authentication/runtime change, Git commit/push/PR action, production fix, timeout increase or tracker change. Main owns scheduled qualification, independent review and any separately authorized publication/capture. After the Main-owned Linux gates, Main authorized this record refresh and the eight minimal portable receipts listed below; no executable source or workflow change is part of that refresh.

- Baseline: current C6/PR197 head `96fae2712888489dccd4cdfbf60e290879e8f6a5`, not the historical d6 or PR195 checkout.
- Diagnostic source: pinned PR195 `dc3fd535c3a78a9332abbdd6cb2cb267d6caa5ee`; path-scoped transplant only, with the exact overlay below.
- Settled scope and reasoning: `agent://PlanWindowsAllFeatureCapture`; failed current lane/provenance: `agent://DiagnoseCurrentWindowsReadiness`.
- Original PR195 worktree, draft PR and records remain untouched. Its package/default capture is non-reproduction, not qualification of this composition.

## Current route tests and outcome contract

| Test | Verdict and evidence |
| --- | --- |
| T1 Empirical premise | **Yes.** U62F-P1 remains UNATTRIBUTED. Current C6 failed initial readiness in Windows workspace/all-features without probes. The missing facts are child initialization/accept/handler progress and the parent's failure remainder, joined to the actual fixture test and launch. Attribution controls any subsequent repair or justified budget. |
| T2 Structural module shape | **No for this instrument.** Existing probe owners, sink, stable identity serialization, caps and hosted finalization are reused. Only fixture test/launch linkage and relocation of the existing workflow envelope are added; no production interface, IPC or lifecycle ownership changes. |
| T3 Production-scale risk | **No production repair.** The disposable opt-in observations may perturb scheduling; they cannot establish the uninstrumented failure's cause by timing alone. No polling-loop writes, retries, waits or timeout increases are added. |
| T4 Explicit behavior | **Yes.** Capture the existing Windows `cargo nextest run --workspace --all-features` step with unchanged command, profile, concurrency, fail-fast, runner label, five-second startup/250ms operation/20ms sleep budgets and assertions. Both fault inputs remain absent. Link actual `NEXTEST_TEST_NAME` → header parent PID → process-local monotonic launch ordinal → child PID → executable at both launch sites. Preserve separate failure-eligible finalization/upload and actual image/checkout/binary provenance. Green is non-reproduction; missing/capped phases remain incomplete and UNATTRIBUTED. |

**Selected route: Empirical**, adopting the existing U62F-P1 premise with current C6 applicability. This is one bounded observation increment, not a selected permanent fix. No new production design/spec/plan is written: the settled diagnostic plan supplies this increment's implementation contract, and a production design remains dependent on attribution.

## Exact current diagnostic overlay

The diagnostic source/workflow/record overlay remains these eleven paths; the final publication allowlist also includes only the eight approved evidence companions below:

1. `.cyril-u62f/probe.rs`
2. `crates/cyril-memory/src/paths.rs`
3. `crates/cyril-memory/src/runtime.rs`
4. `crates/cyril-memory/src/store.rs`
5. `crates/cyril/src/bin/cyril-memory-runtime.rs`
6. `crates/cyril/tests/memory_runtime.rs`
7. `.github/workflows/ci.yml`
8. `.cyril-u62f/source-pins.json`
9. `.cyril-u62f/route.md`
10. `.cyril-u62f/evidence.md`
11. `.cyril-u62f/review-decisions.md`

The probe helper and four non-fixture Rust owners are unchanged PR195 copies. The fixture adds only enabled test identity and shared enabled child-launch linkage. The workflow moves the existing capture envelope and Windows-only nextest pin from package/default to workspace/all-features, adapting provenance/artifact scope. The four records describe this composition and retain historical provenance explicitly as historical.

Approved portable evidence companions, all under `.cyril-u62f/receipts/current-c6-linux/`:

12. `canonical.json`
13. `source-manifest.json`
14. `01-fmt-check.log`
15. `02-workspace-check.log`
16. `03-workspace-clippy.log`
17. `04-memory-runtime-nextest.log`
18. `source-review.md`
19. `workflow-parser-results.json`

These are exact copies of the original canonical receipt, hash-only source manifest, four command logs, source review and parser results. The canonical receipt retains its original host paths; `source-pins.json` maps them to these portable companions by SHA256. No binaries, source-tree archive or extra historical/runtime artifacts are added.

`.cyril-u62f/probe.ps1` is excluded: the historical native/VM driver stays at PR195 and is not an active owner or invocation here. Cargo manifests/lock/toolchain/configuration, C6 production changes, general docs/agent instructions and tracker data are excluded from the overlay. A main-targeted draft PR's full diff would also contain its C6 baseline; the diagnostic delta is always measured separately against exact `96fae271…`.

## Current ownership, proof and handoff boundary

- `route.md`: active route/authorization/allowlist; preserved PR195 route below.
- `evidence.md`: current premise, discriminating observations and interpretation limits; preserved PR195 facts below.
- `source-pins.json`: current composition/active source identities and nested historical PR195 provenance; no self-hash.
- `probe.rs`: existing disposable bounded instrument only. `review-decisions.md`: historical repair applicability and required new review.

**PASS — scoped local qualification and source/static review.** Main's independent source/static review found no actionable findings (confidence 0.95); Main loaded YAML and actual PowerShell `Parser.ParseFile` returned zero errors for each relocated body. The sole Linux qualifier then passed format check, full workspace/all-target/all-feature compiler and strict Clippy gates, and App all-feature `memory_runtime` nextest: **8 passed, 0 failed, 0 skipped**. `evidence.md` owns exact commands, portable receipt identities and applicability. Probes and both faults were absent: this is not enabled linkage/phase evidence, Windows qualification, full-workspace runtime execution, attribution, a permanent fix or a failed-CI waiver.

**PENDING — Main, separately authorized hosted capture:** use the existing Windows Test lane once, retain complete original job logs and the narrow phase artifact, actual checkout/head/base/tree/run/attempt/job/image/toolchain, original test outcome and post-test executed-binary hashes. Assess caps, errors, truncation and missing terminal/identity/linkage phases explicitly. Do not force an image, automatically rerun, waive PR197's failed CI, close the issue or infer a repair from a green capture.

The scoped implementation handoff is not empirical/issue completion. The unchanged terminal criterion is valid phase attribution followed by an exercised fix or documented phase budget, with required platform/repository checks and review; unresolved attribution or proof blocks that dependent completion.

---

# Historical PR195 route — preserved from dc3fd535…

# cyril-u62f — Windows memory-runtime phase attribution

Date: 2026-10-22; original host assignment was 2026-10-20 and immutable failure-log timestamps are 2026-10-21.

## Authorization and scope

The requester approved **“Fix both in separate PRs”** for `cyril-peaq` and `cyril-u62f`. This directory belongs only to `cyril-u62f`, on `fix/cyril-u62f-ci`, instrumentation edit base `4ef93cad80a47a5fb49a08794e80de57b13054cb`. Main owns Git/index/commits/push/tracker and Cargo scheduling. The current increment is a **qualified temporary diagnostic prepared for a draft PR and actual hosted Windows phase capture**, not a production repair or issue closure.

Temporary opt-in hooks are authorized in the memory-runtime fixture, runtime configuration/loop, root ACL boundary, store-initialization boundary, and companion entrypoint. No connection timeout, readiness/shutdown budget, assertion, feature-selection policy, IPC implementation, or production lifecycle repair is authorized in this increment.

Historical native preflight, exact-version prerequisite provisioning, native preparation and individually authorized diagnostic attempts are retained in `evidence.md` and immutable receipts. Main subsequently authorized the hosted instrument, bounded invariant-lint repair, source review and Linux/parser qualification. Each GO applies only to its recorded action; publication and hosted execution remain Main-owned.

## Route tests

| Test | Verdict and evidence |
| --- | --- |
| T1 Empirical premise | **Yes.** U62F-P1: which named phase accounts for the approximately 7–9 seconds outside the existing readiness message's elapsed measurement? The historical/current failures preserve whole-test and readiness-message durations but not setup, child progress, error/backtrace construction, root cleanup, or executor completion. This answer determines the repair or justified phase budget. See `evidence.md`. |
| T2 Structural module shape | **No for the instrument.** Temporary observation stays at existing owners. The shared `probe.rs` is an Empirical instrument included by path, not a new production API. The final repair route must be reconsidered after attribution if its ownership/interface changes. |
| T3 Production-scale risk | **No production repair introduced.** Instrumentation is opt-in and disposable. Instrument I/O and observer sampling can perturb scheduling; those are explicitly recorded limitations, not evidence that observed CI latency is instrumentation overhead. |
| T4 Explicit behavior | **Yes.** Existing issue acceptance requires a reproducing run or per-phase timings identifying the stall's phase, followed by either a fix or a documented budget. Preserve original assertions, deadlines, package defaults and fail-fast/concurrency semantics. Never infer a cause from whole-test durations, unchanged source, or a green retry. |

**Selected route: Empirical.** `gilfoyle` contract → `prove-it-prototype` → `falsifiable-design` after factual attribution → plan/checkpoint/qualification/review. No production design or implementation choice is made before the consequential phase is observed.

## Artifact ownership

- `route.md`: route owner; authorization and terminal conditions.
- `evidence.md`: Empirical owner; immutable facts, probe mechanism/commands, source pins, native prerequisites, unresolved premises.
- `probe.rs`, `probe.ps1`, `source-pins.json`: disposable instrument, bounded driver, and source provenance.
- Design/plan: not yet applicable; they follow the observed answer rather than codifying an unproven cause.

## Current scoped handoff

The current temporary instrument has ordinary source review with F7/F8 repaired, YAML loading and two actual PowerShell `Parser.ParseFile` results with zero errors, plus Main-owned repaired-source format check, strict App all-target Clippy and the full Linux `memory_runtime` target: **8 passed, 0 failed, 0 ignored**. The prior App all-target check is retained with the qualification receipt's stated applicability. Canonical results: `local://u62f-hosted-capture-repaired-Linux-qualified-handoff.json`; syntax-only proof: `local://u62f-hosted-workflow-parser-proof.json`. One Linux parent helper reached its 256-record cap, so later parent cleanup/drop phases are incomplete. Neither parser success nor Linux PASS proves Windows behavior, complete phase capture or the original cause.

**PENDING — Main:** commit the qualified diagnostic, merge Main038 normally, then compare the **actual published head** against historical d6 across the relevant source/manifests/lock/toolchain/configuration/workflow scope, separating diagnostic from ambient changes; do not claim 4ef=d6 or qualify an unobserved future merge checkout. Publish only a temporary **draft** PR, retain the actual checkout/head/base/run/attempt/ImageVersion, original Windows package command, narrow phase artifact and complete job log. Hosted actual phase capture is next; green means non-reproduction, cap/missing phases mean missing evidence, and neither authorizes automatic rerun, issue closure or a guessed budget/fix. Publication inputs and the exact 12-path allowlist/pins are in `local://u62f-hosted-capture-publication-handoff.json`. U62F-P1 remains **UNATTRIBUTED**.

## Terminal criterion

The issue is complete only after valid phase attribution and either an exercised fix or a documented budget for that phase, with required readiness/schema/replay/restart/reopen coverage and mandatory repository/platform checks/review. A failed gate remains failed; no red-baseline waiver is created by this route.
