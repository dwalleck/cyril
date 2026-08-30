# Use the official SDK 2 conductor as Cyril's only production ACP topology

Status: accepted (2026-08-30)

Supersedes: [ADR-0003](0003-defer-proxy-stack-for-host-callbacks.md)

Decision approval: “Approve conductor-first design” (2026-08-30)

## Context

Cyril's current bridge predates the official Rust SDK 2 component model. It uses
`agent-client-protocol` 0.10.2/schema 0.11.2 and a direct
`ClientSideConnection`; transport, dispatch, Kiro/KAS conversion, serial domain
state, host callbacks, and App projection meet around that connection.

SDK 2.0.0 supplies first-party `Client`, `Agent`, `Proxy`, and `Conductor`
roles, ordered handlers, frame-aware channels, and the role-directed
`ConnectTo<R>` component interface. Adopting only its callback signatures would
preserve the wrong topology and require another bridge rewrite when proxy stages
arrive. Selecting conductor-first without executable parity evidence would risk
moving vendor meaning, host effects, memory semantics, or error ordering into
the wrong layer.

The `cyril-41bs` empirical spike tested ten premises against pinned SDK 2.0.0
sources and live Kiro v2/KAS processes. Reproducers, independent oracles, and the
approved falsifiable design live under [`.cyril-41bs/`](../../.cyril-41bs/).

## Decision

Choose **Option C: conductor-first production topology** for the SDK 2
migration.

Every production ACP connection will run through the official
`ConductorImpl`, including the zero-proxy case. An empty ordered stage chain is
ordinary operation, not a reason to select a direct bypass. Cyril will not add
an `AgentRuntime`, `AgentEndpoint`, or equivalent interface that renames the
official component vocabulary.

The migration keeps stable ACP wire v1. SDK major version 2 is not wire version
2; the `unstable_protocol_v2` feature remains disabled in production.

## Placement and ownership

### SDK runtime

A new private `cyril-core::protocol::sdk_runtime` module owns SDK
`Client`/`Agent`/`Proxy`/`Conductor` composition, ordered stage construction,
frame driving, and connection termination. Its production entrypoint is:

```text
SdkRuntime::start(AgentProcess, DomainChannels, StageChain)
    -> Result<SdkRuntimeHandle>
```

The API has three arguments and **no observer**, inspection, tracing, or
multi-client registration parameter.

Private Cyril structs implement the official `ConnectTo<R>` interface directly.
There is one conductor path and no direct compatibility path.

### Serial domain mediator

A new private `cyril-core::protocol::domain_mediator` module owns the existing
`Rc<dyn Engine>` domain state, ingress/tool ledgers, normalized conversion,
turn/source mediation, and Cyril domain-event emission.

SDK handlers are `Send + 'static`. They send bounded typed work to the serial
mediator and return before slow domain or host work. Request work carries a typed
reply channel. The `Rc` state never crosses the actor boundary; Cyril does not
convert it to `Arc<Mutex<_>>`, use unsafe code, or introduce unbounded actor
queues.

The unknown-standard-update handler remains statically registered before typed
session handlers. Handler order, request identity, cancellation, and dynamic
handler lifetime remain SDK-owned protocol behavior.

### Process and exact ingress

Cyril retains `protocol::transport::AgentProcess`, `ProcessGroupGuard`, and
`StderrTail`. A private adapter implements `ConnectTo<Client>` over the existing
process pipes.

This keeps explicit cwd, bounded stderr diagnostics, stall detection,
kill-on-drop, process-group/grandchild cleanup, EOF handling, and shutdown grace.
Exact inbound bytes are captured at `AgentProcess` stdout before SDK or
`serde_json` parsing; semantic frames then enter `sdk_runtime`.

`AcpAgent` is not used as the production process owner because the spike showed
that it lacks Cyril's launch-cwd field, stall policy, and public always-on stderr
tail.

### Vendor meaning and host effects

`protocol::engine`, `protocol::convert`, and terminal host mediators remain the
sole owners of Kiro/KAS meaning and side effects. Proxy stages may transform
wire concerns, but they do not own `Engine`, filesystem/terminal/hooks effects,
KAS authentication, tool ledgers, or terminal disposition.

### App, source, and memory contracts

`protocol::bridge` continues to expose the existing `BridgeCommand`,
`RoutedNotification`, permission, source-event, liveness, and shutdown
contracts. SDK dispatch/schema/raw-frame types stay inside `cyril-core`; App,
UI, memory, and voice remain SDK-independent.

First-prompt memory injection stays at `App::send_prompt`/`dispatch_prompt`.
`SourceObserver` remains the normalized domain observer for original prompts,
project/session/turn identity, byte budgets, tool lifecycle, and terminal
disposition. Wire inspection may supplement audit evidence but does not replace
these contracts.

### Observer topology

The official conductor is a linear successor chain, not a broadcaster. Inline
inspection is synchronous: a slow or disconnected observer affects forwarding,
and a fresh attachment has no replay.

This migration exposes no observer API. A separate bounded broadcaster with
explicit pressure, replay, and ownership policy is future work in
`cyril-5g2o`. The existing unbounded-pressure trigger remains tracked by
`cyril-1ixa`.

## Evidence

The retained probes establish:

- **E1:** bounded `Send` protocol work can reach a serial `Rc` domain actor
  without moving/sharing the domain state.
- **E2:** SDK inspection preserves semantic frame order and malformed evidence,
  but parse/reserialize cannot guarantee lexical bytes; capture must precede
  parsing.
- **E3:** handler priority/fallthrough, cancellation, and dynamic lifetime hold;
  a slow handler blocks later dispatch, requiring enqueue-and-return handlers.
- **E4:** `AgentProcess` owns required cwd, diagnostics, stall, and lifecycle
  behavior that `AcpAgent` does not expose.
- **E5/E6:** authenticated Kiro v2 and KAS sessions used stable wire v1 and
  matched direct/conductor lifecycle milestones across all six
  zero/no-op/transform cells. KAS exercised auth, filesystem, terminal,
  permission, hooks, and structural turn-end ordering; v2 host callbacks are a
  named live N/A. Separate deterministic matrices proved exhaustive callbacks,
  actual response IDs, typed errors, cancellation/failure, and transformed
  payloads rather than filling unexercised live cells with defaults.
- **E7:** bidirectional transforms changed requests, responses, and
  notifications while terminal components retained vendor conversion and host
  ownership.
- **E8:** two composed inline observers demonstrated slow-observer coupling,
  permission-path coupling, disconnect failure, and no replay.
- **E9:** a wire tap sees the prepared prompt and lacks Cyril's normalized
  identity/disposition contract, so memory/source seams stay in place.
- **E10:** stable wire v1 is the default; draft v2 is explicit and
  production-disabled; ACP imports remain confined to `cyril-core`.

The direct/conductor live comparator reports `contract_matches_direct: true` for
all six v2/KAS lifecycle cells, with live typed-error, outer-ID, and
cancellation evidence explicitly marked not exercised. Deterministic matrices
own those contracts. Independent source/capture/oracle checks are recorded in
[evidence.md](../../.cyril-41bs/evidence.md).

## Upstream proposal disposition

**No blocking upstream gap remains.** The official `ConnectTo<Client>`,
conductor, handler, channel, and forwarding APIs support the selected topology.
Cyril's retained `AgentProcess` adapter deliberately owns the cwd, diagnostics,
stall, and process-tree semantics that `AcpAgent` does not expose. Because that
ownership is Cyril-specific and does not require an SDK API change, no upstream
issue or PR is justified by this spike.

If production work discovers a blocker that cannot remain in the private
adapter, `cyril-gl5s` must stop at its checkpoint and file the minimal upstream
proposal before changing this decision.

## Consequences

- The SDK 2 migration is a clean cutover: ACP 0.10/schema 0.11, the old direct
  `ClientSideConnection`, aliases, shims, and dual runtimes leave together.
- Zero proxy stages still pay conductor composition cost. Live parity and the
  deterministic probes found no observable contract divergence; this buys one
  topology and removes a future bridge rewrite.
- SDK handlers cannot perform slow domain/host work inline. Bounded mediator
  channels make pressure and request replies explicit.
- Exact lexical capture remains a process-ingress concern; SDK semantic
  inspection is insufficient.
- Host callbacks remain the side-effect boundary described by ADR-0003.
  ADR-0003's deferral of the proxy/conductor stack is superseded; its separation
  between wire transformation and host effects is retained.
- ADR-0003's promise to move persistent-memory adapters when a proxy stack is
  activated is also superseded. Under this decision, activation alone moves
  neither first-prompt injection nor normalized source capture; both remain at
  their current seams for the SDK 2 migration.
- Multi-client observation is not smuggled into the runtime API.

## Rejected alternatives

### Direct-only SDK adoption

Rejected. It would pass through fewer components today but preserve a second
future bridge rewrite and a zero-stage special case. The live conductor matrix
found no parity blocker.

### Cyril-owned direct/conductor runtime trait

Rejected. A custom endpoint/runtime trait is shallow: it duplicates
`ConnectTo<R>`, retains two production paths, and has no second domain-level
implementation.

### Raw frame relay as the primary seam

Rejected. It can inspect bytes but cannot own typed callbacks, Engine
conversion, host effects, or conductor lifecycle.

### `AcpAgent` as process owner

Rejected. It does not satisfy Cyril's cwd, stall-watchdog, and public bounded
stderr-tail contracts.

### Conductor inspection as observer fan-out

Rejected. The measured path is synchronous and linear, with coupled pressure,
disconnect, and no replay.

## Implementation and follow-on ownership

- `cyril-gl5s` (child of `cyril-1gfe`) owns the production conductor-first SDK
  2 clean cutover and C1–C13 checkpoints. Its dependency-ordered internal slices
  are Transport ingress; Runtime API and actor topology; App contract; Memory
  and source contract; Command and routing matrix; Clean cutover.
- `cyril-5g2o` owns a separate bounded multi-client broadcaster.
- `cyril-1ixa` owns the current unbounded-pressure trigger.

No production migration, placeholder stage crate, public runtime trait, or draft
wire-v2 enablement lands with this ADR.
