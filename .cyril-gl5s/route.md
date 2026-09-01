# Route: cyril-gl5s

Change: Replace the direct ACP 0.10 runtime with the approved conductor-first SDK 2 production topology as one clean cutover.
Date: 2026-08-30

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | No unverified premise remains. `.cyril-41bs/evidence.md` P1–P10 is current for the pinned SDK 2.0.0/conductor 2.0.0 topology and covers the Send-to-serial handoff, pre-parse capture placement, handler ordering, retained process adapter, Kiro v2/KAS parity, zero/no-op/transform conductor chains, proxy ownership, observer limits, memory/source seams, and stable wire v1. `.cyril-g0dg/` supplies the merged current-runtime C1–C13 contract oracle. Neither the SDK version, evidence target, nor current bridge contract changed after those same-day artifacts were recorded and merged. | no |
| 2 | Structural module shape | The change replaces `protocol::bridge`'s direct ACP `ClientSideConnection` construction with a private `protocol::sdk_runtime` owner of official `ConnectTo<R>`/`ConductorImpl` composition and moves `KiroClient`'s serial domain responsibilities into a private `protocol::domain_mediator`. The retained `protocol::transport::AgentProcess` remains process owner through a private SDK role adapter; `protocol::engine`, `protocol::convert`, `protocol::host_mediator`, App-facing bridge types, source observation, memory, UI, and voice retain their interfaces and responsibility ownership. Candidate owners are the approved `sdk_runtime` and `domain_mediator` modules. Protected parents are `protocol::bridge` (orchestration only), `protocol::client` (ordered SDK handler registration only), `crates/cyril/src/app.rs` (SDK-independent orchestration), and the manifests outside `cyril-core` (no SDK dependency). The old direct runtime and ACP 0.10/schema 0.11 are deleted without aliases or shims. | yes |
| 3 | Production-scale risk | Bounded Send-to-serial queues, enqueue-and-return SDK handlers, source-order preservation, saturation behavior, process stall/cancellation/shutdown, and full-duplex callback traffic carry concurrency, throughput, memory, and liveness risk. The C1/C4/C5/C6/C10/C13 fences and explicit queue/process budgets are required. | yes |
| 4 | Explicit behavior | **G1:** Given single, batch, malformed, unknown, extension, and extreme-number ingress, when bytes cross `AgentProcess` into SDK 2, then exact pre-parse bytes, semantic frame order/shape, cwd, bounded diagnostics, stall/EOF/cancellation/process-tree/shutdown behavior match C4/C6. **G2:** Given either supported engine and zero or more ordered stages, when the runtime starts, then every connection uses official `ConnectTo<R>` roles through `ConductorImpl`, handlers perform bounded enqueue-and-return handoff to the serial mediator, the private start API has exactly `(AgentProcess, DomainChannels, StageChain)`, stable wire v1 is used, and SDK types stay in core (C1/C2/C3/C5/C10/C11). **G3:** Given every advertised Kiro/KAS callback family, when callbacks traverse the chain, then Engine/conversion retain vendor meaning and terminal host mediators retain filesystem, terminal, hook, auth, tool-ledger, and terminal-disposition effects (C7). **G4:** Given a complete first and subsequent Cyril turn, when it traverses App, memory preparation, conductor, streaming/tools, normalized source capture, terminal disposition, and shutdown, then first-prompt context is injected exactly once and all observable ordering/identity/budget contracts hold. **G5:** Given initialize, prompt, callback, EOF, crash, disconnect, and shutdown outcomes, when projected to App, then SDK-independent notifications, errors, completion, and disconnect retain the current C8 order. **G6:** Given every I10 memory/source matrix cell, when a turn runs, then original versus prepared placement, exactly-once injection, project/session/turn identity, UTF-8/tool budgets, terminal disposition, and bounded capture shutdown match C9. **G7:** Given every `BridgeCommand`, engine-selection result, routed identity, extension-param shape, queue saturation, and disconnect cell, when dispatched through the new runtime, then exactly one correctly typed output path occurs without loss, duplication, coercion, or hang (C13). **G8:** Given the completed new path before deletion, when every C1–C13 fence and named mutation is run and authenticated Kiro v2/KAS turns execute, then all are green after each mutation is restored. **G9:** Given accepted deterministic and authenticated evidence, when the cutover completes, then ACP 0.10/schema 0.11, `ClientSideConnection`, direct bypasses, aliases, shims, and the old runtime are absent and one SDK family/runtime path remains (C12). **G10:** Given the post-deletion tree, when formatting, clippy with warnings denied, all-target tests, the deterministic contract matrix, topology census, and authenticated Kiro v2/KAS acceptance run again, then all pass. **G11:** Given the final diff, when scope is censused, then it contains no observer API, Cyril-owned endpoint/runtime trait, draft wire v2 enablement, SDK type outside core, proxy-owned vendor effect, production runtime switch, or cleanup follow-up. | yes |

Unknown tests: none

## Selected route

Structural — the accepted behavior is explicit and empirically grounded, but the cutover changes runtime seams, responsibility owners, dependency direction, and production concurrency/liveness behavior.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior is fully explicit in T4 and the issue acceptance criteria |
| evidence.md, probe.* | prove-it-prototype | N/A — no unverified premise; current evidence is `.cyril-41bs/evidence.md` and the merged baseline oracle is `.cyril-g0dg/` |
| design.md | falsifiable-design | required |
| plan.md | budgeted-plan | required |

Oracle checkpoint in `checkpointed-build`: required — Structural route.

## Downstream sequence

falsifiable-design → budgeted-plan → checkpointed-build

## Terminal criterion

Structural — every downstream artifact satisfies its owning stage's completion criterion, ending with no `FAIL` in checkpointed-build's recorded gate.
