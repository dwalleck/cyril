# Route: cyril-41bs

Change: Prove SDK 2 direct, component, proxy, and conductor behavior and select Cyril's target runtime topology.
Date: 2026-08-29

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | The topology decision depends on SDK 2.0.0 and live/executable system behavior not covered by current applicable evidence. `docs/acp-sdk-2-architecture-research.md` explicitly labels the E1–E10 gates as falsifiable experiments that have not been run (lines 165–178), and leaves `AcpAgent` parity unknown (lines 60–66). The required premises include `Send + 'static` handoff from Cyril's `Rc`/`RefCell` state, lexical/frame preservation, handler and cancellation ordering, process cleanup, Kiro v2/KAS direct and conductor parity, proxy leverage, observer backpressure, memory exactly-once behavior, and stable-v1/draft-v2 routing. No existing `evidence.md` or probe covers them. | yes |
| 2 | Structural boundary | The issue requires a placement decision across `protocol/bridge.rs::spawn_bridge`/`run_bridge`, `client.rs::KiroClient`, `engine.rs::Engine`, `transport.rs::AgentProcess`, `source_observer.rs::SourceObserver`, host mediation, and SDK `Client`/`Agent`/`Proxy`/`Conductor` component boundaries. It also requires a new ADR and preservation or explicit migration of every App-facing bridge contract. | yes |
| 3 | Production-scale risk | The selected topology will own dispatch-loop concurrency, bounded-channel backpressure, slow observers, streaming plus permission ordering, process-group/grandchild cleanup, cancellation, shutdown grace, memory byte budgets, and exactly-once transcript capture. These are production latency, memory, concurrency, and lifecycle risks even though this issue produces experiments rather than a production cutover. | yes |
| 4 | Explicit behavior | **E1:** Given the pinned SDK 2.0.0 component contract and Cyril's `Rc`/`RefCell` mediator, when a bounded Send protocol actor hands work to a serial domain actor, then it runs without unsafe or blanket shared-lock conversion, or records a minimal reproducible blocker. **E2:** Given single, batch, malformed, Kiro-extension, unknown nested-update, and extreme-number frames, when they traverse SDK channels/inspection, then semantic and lexical preservation plus batch/malformed behavior are recorded without weakening forward compatibility. **E3:** Given ordered typed/untyped/dynamic handlers and forwarded requests, when claims, fallthrough, cancellation, guard lifetime, and dispatch-loop blocking are exercised, then observed order is recorded and reversing/removing unknown-first containment fails. **E4:** Given equivalent helper processes, when current `AgentProcess` and SDK `AcpAgent` face cwd, stderr burst, non-zero exit, stall, cancellation, EOF, shutdown grace, and grandchildren, then parity and a keep/replace/upstream decision are recorded. **E5:** Given stable wire-v1 Kiro v2 and KAS flows, when direct SDK topology runs initialize/session/prompt/permission/extension and every advertised host callback, then normalized Engine/SourceObserver output and turn/error order match or exact divergences are recorded. **E6:** Given the same flows, when zero-proxy, no-op-proxy, and transforming-proxy conductor paths run, then initialization, successor envelopes, identity, cancellation, extensions, callbacks, and EOF/crash behavior match or exact divergences are recorded. **E7:** Given one useful bidirectional concern, when implemented as a real proxy, then end-to-end leverage is demonstrated without duplicated Engine conversion or host ownership. **E8:** Given a slow observer, streaming, permission requests, disconnect, and replay, when the topology is stressed, then ordering/backpressure/ownership limits are characterized and any required separate broadcaster is named. **E9:** Given identical prompt and normalized inbound flows, when current memory seams and a proxy/tap are compared, then injection/capture exactly-once behavior, byte budgets, cancellation, identity, and terminal disposition determine a keep/move decision. **E10:** Given SDK 2.0 peers, when stable wire v1 and feature-gated draft wire v2 routing are probed separately, then SDK/wire versions remain distinct and draft v2 stays production-disabled. **Deliverables:** Given all experiment results, when the decision rule is applied, then reproducible experiment artifacts remain, an approved ADR selects and amends/supersedes ADR-0003, blocking upstream gaps have issue/PR proposals, independently green follow-on Rivets work is filed, no speculative production crate/trait or migration lands, and disposable prototype code is removed. | yes |

Unknown tests: none

## Selected route

Empirical — the architecture decision depends on ten unproven runtime, wire, process, and parity premises; Empirical takes precedence over the structural boundary and production-risk verdicts.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior is fully explicit in T4 and the issue acceptance criteria |
| evidence.md, probe.* | prove-it-prototype | required — Empirical route (T1 yes) |
| design.md | falsifiable-design | required |
| plan.md | budgeted-plan | required |

Oracle checkpoint in `checkpointed-build`: required — Empirical route

## Downstream sequence

prove-it-prototype → falsifiable-design → budgeted-plan → checkpointed-build

## Terminal criterion

Empirical — `evidence.md` records `PASS` for every E1–E10 empirical premise, every later artifact satisfies its owning stage's completion criterion, and `checkpointed-build` records no `FAIL`.
