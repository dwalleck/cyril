# Falsifiable design: cyril-gl5s

## Route and inputs

- **Route:** Structural. `.cyril-gl5s/route.md` selects it because the cutover changes runtime seams, responsibility ownership, dependency direction, and bounded production concurrency without an unverified empirical premise.
- **Specification:** N/A — `route.md` T4 records the complete explicit G1–G11 Given/When/Then behavior set; no `spec.md` is required.
- **Behavior set:** G1 exact ingress/process parity; G2 bounded conductor-only SDK topology; G3 terminal ownership of vendor meaning and host effects; G4 complete App/memory/source turn; G5 App event/error/terminal order; G6 full I10 memory/source matrix; G7 exhaustive command/routing/saturation/disconnect matrix; G8 pre-deletion fences, mutations, and authenticated v2/KAS turns; G9 single-family clean deletion; G10 post-deletion deterministic/authenticated gates; G11 negative-space census. Exact statements are in `route.md` T4.
- **Empirical premise:** N/A — Structural route. Current applicable evidence is `.cyril-41bs/evidence.md` P1–P10 (SDK 2.0.0, conductor 2.0.0, upstream `ce023279824149008659dd8f4b8b70266a7e8210`) and the merged `.cyril-g0dg/` current-runtime oracle. No new probe is required.
- **Independent oracles already available:** standalone `rustc` Send/`Rc` negative controls; Python raw JSON/`Decimal`; OS subprocess checks; exact upstream SDK/conductor tests; authenticated Kiro v2/KAS direct-versus-conductor comparison; hand-authored current-runtime command/routing/App/memory/source tables; and standalone repository censuses. Production fences below reuse those answers but run through the new path.
- **Accepted architecture input:** ADR-0012 and `.cyril-41bs/design.md` select official conductor-first topology, retained `AgentProcess`, a bounded Send-to-serial mediator seam, stable wire v1, unchanged App/memory/source ownership, and no observer interface. This issue-local design supplies the previously missing concrete runtime types, module ledger, protected-parent ledger, and module-shape fence.

## Input shapes

| ID | Production-reachable shape | Status |
|---|---|---|
| I1 | Engine selection: Kiro v2 or KAS; supported, feature-disabled, discovery failure, spawn failure, initialize contradiction. | Covered by C2, C7, C8, C13. |
| I2 | Stage chain: empty; one no-op; one transforming; multiple distinct; repeated same stage type; order significant. | Covered by C2, C3, C10. Production starts empty without a bypass or placeholder stage. |
| I3 | Inbound transport: `Single`; non-empty `Batch` with one/many, duplicate messages, and malformed members; top-level `Malformed`; exact numeric lexemes including `1e400`; clean EOF and I/O error. Empty batch is N/A — SDK `TransportBatch` excludes it. | Covered by C4, C6. |
| I4 | JSON-RPC request, notification, success response, error response; standard and extension methods; known and unknown method/update; metadata absent/present/additive; integer/string IDs; malformed params. | Covered by C4, C5, C7, C13. |
| I5 | Handler path: untyped/typed static handlers, first claim/fallthrough/unmatched, dynamic attach/drop, request forwarding, cancellation before/after forwarding, full mediator queue, slow domain/host work. | Covered by C1, C5, C13. |
| I6 | Process config/lifecycle: empty/multiple args and env, relative/absolute/Unicode/spaced cwd, clean/nonzero exit, bounded stderr with burst/no newline, stall, cancel, stdout EOF, shutdown grace, child/grandchild. Duplicate env keys are N/A — `AgentCommand` represents one final value per key. | Covered by C6. |
| I7 | Every advertised callback family: authentication; standard/Kiro filesystem read/write/stat/read-directory/delete; terminal create/output/wait/release/kill/shell type; permissions with empty/single/multiple/trust options; hooks list/execute/session-start/cancel/change. | Covered by C7, C13. |
| I8 | Routed identity: session absent/main/subagent/unknown; turn absent/present; session create/load/terminate/message/steer/clear; extension params object/array/null/additive unknown. | Covered by C8, C9, C13. |
| I9 | Every current `BridgeCommand` variant and nested sum/option cell, including feature-gated KAS hooks/workflow, disconnected peers, and bounded-channel saturation. | Covered by C13. |
| I10 | First/subsequent prompt; one/multiple original blocks; prepared context absent/present; memory unavailable/starting/ready; UTF-8/tool byte boundaries; completed/interrupted/cancelled/error dispositions; project/session/bridge-turn/source-turn identity reuse/change. | Covered by C8, C9. |
| I11 | Error/termination order: initialize/prompt/callback error; KAS `turn_end` before prompt response; idle/mid-turn/post-turn EOF; crash; deferred disconnect; user shutdown. | Covered by C5, C6, C8, C13. |
| I12 | Protocol negotiation: default stable v1; explicit draft v2; negotiated fallback; v2 rejection; no post-handshake translation. | Covered by C11. Draft v2 production enablement is a permanent non-goal of this cutover. |
| I13 | Dependency/topology lifecycle: final SDK core+conductor only in `cyril-core`, no SDK type in App/UI/memory/voice, and no ACP 0.10/schema 0.11 or `ClientSideConnection`; one pre-deletion feature-branch checkpoint may retain ACP 0.10 under the explicit `agent-client-protocol-legacy` dependency name after every production import/caller has moved to SDK2. The checkpoint has one active conductor runtime, zero legacy source imports, and is never merged independently to main. | Covered by C3, C8, C11, C12, C14. |
| I14 | Runtime inspection: exact three-argument start interface; no observer/tracing/multi-client argument; no synchronous diagnostic work added to forwarding. | Covered by C10, C14. |
| I15 | Source pressure and size: command 32, App notification 256, permission 16, source 32, mediator bounded; exact boundary and boundary+1; no lossy send. | Covered by C1, C5, C9, C13. |
| I16 | Repository/module shape: required/forbidden paths and symbols; protected-parent deltas; dependency direction; handler ownership/order; production-line tripwires; test locations. | Covered by C14. |

## Placement

### SDK runtime and stage chain

- **Owner:** new private `cyril-core::protocol::sdk_runtime` directory module. `mod.rs` owns `SdkRuntime`, `SdkRuntimeHandle`, `StageChain`, `ConductorImpl::new_agent`, ordered `DynConnectTo<Conductor>` stage construction, lifecycle handoff, and termination. `process.rs` owns the concrete `AgentProcess` adapter implementing official `ConnectTo<Client>` over its pipes.
- **New seam:** `async SdkRuntime::start(AgentProcess, DomainChannels, StageChain) -> Result<SdkRuntimeHandle>`. Exactly three arguments. `StageChain` is a private ordered collection of official role adapters and defaults to empty. `SdkRuntimeHandle` carries the official `ConnectionTo<Agent>`, connection completion, bounded shutdown, and retained stderr diagnostics needed by the serial mediator; it adds no Cyril runtime trait.
- **Forbidden:** direct zero-stage bypass; `AgentRuntime`/`AgentEndpoint`; public SDK surface; observer/inspection/tracing registration; `AcpAgent`; draft wire v2; placeholder proxy/stage crate.

### Serial domain mediator

- **Owner:** new private `cyril-core::protocol::domain_mediator` directory module. `mod.rs` owns `DomainMediator`, `DomainConfig`, `DomainChannels`, the single serial select loop, active connection/session state, ingress/turn/tool/source composition, liveness, and terminal event emission. Internal `commands/` modules own outgoing request construction by command family; `inbound.rs` owns ordered converted notifications and permissions; `host.rs` wires existing host lifecycle/effect adapters.
- **New seam:** `DomainMediator::new(DomainConfig, BridgeChannels) -> (DomainMediator, DomainChannels)` and `DomainMediator::run(SdkRuntimeHandle) -> Result<()>`. `DomainConfig` contains the local `Rc<dyn Engine>` and launch-bound domain configuration. Cloneable `DomainChannels` contains bounded Send senders for ordered domain work and a small serial host-request drain. The host drain starts before `initialize`, because KAS can request authentication from inside initialization; callback results re-enter the ordered domain queue. `DomainWork` owns payloads and typed one-shot replies. The seam is intrinsic: SDK handlers must be `Send + 'static`, while Engine/source/turn/host state remains serial and `Rc`-owned.
- **Forbidden:** `Arc<Mutex<dyn Engine>>`; unsafe code; unbounded actor queues; SDK handler awaiting domain/host completion inline; duplicated conversion or host effects; a second command loop in `bridge.rs`.

### SDK client handlers

- **Owner:** existing private `protocol::client`, deepened from an ACP 0.10 `KiroClient` object into SDK 2 client construction and statically ordered inbound handler registration. It owns the untyped unknown-standard fence, typed standard notifications, extension notifications/requests, permission/callback registration, responder completion tasks, and bounded enqueue into `DomainChannels`.
- **New seam:** a private `connect_client(impl ConnectTo<Client>, DomainChannels, lifecycle handoff)` implementation using official `Client::builder()` and `ConnectionTo<Agent>`. Requests enqueue typed work, spawn reply completion through the SDK connection, and return from the handler; notifications enqueue bounded work and return after capacity is acquired.
- **Forbidden:** Engine conversion, tool/source/turn state, filesystem/terminal/hook/auth effects, App notification projection, stage construction, or a typed session handler before the untyped unknown fence.

### Process and exact ingress

- **Owner:** existing `protocol::transport::{AgentProcess, ProcessGroupGuard, StderrTail}` retains cwd, process pipes, bounded diagnostics, kill-on-drop, Unix group/grandchild cleanup, and process lifetime. `sdk_runtime::process` is the only official-role adapter over those pipes.
- **New seam:** no Cyril trait. The concrete adapter records raw stdout bytes before `BufReader`/official SDK `Lines` parsing in tests; production adds no recording copy. After genuine stdout EOF, it appends one private per-connection lifecycle marker so the ordered SDK handler/domain queue can drain prior frames and preserve the existing typed disconnect contract before runtime shutdown.
- **Forbidden:** post-parse reserialization as C4 evidence; value-only capture; replacement with `AcpAgent`; moving process ownership into a proxy; production raw-capture registration. The private EOF marker is lifecycle control after all agent bytes, never evidence and never an agent-frame rewrite.

### Vendor meaning and host effects

- **Owner:** existing `protocol::engine`, `protocol::convert`, `protocol::host_mediator`, and `protocol::kas::*` adapters retain all Kiro/KAS interpretation, callback lifecycle, filesystem/terminal/hooks/auth effects, tool ledger behavior, and terminal disposition.
- **New seam:** none. SDK 2 schema types replace 0.10 types at the existing core-only conversion seam; domain types remain unchanged.
- **Forbidden:** vendor branching or effects in `sdk_runtime`, a proxy, App, UI, memory, or voice; protocol-default success for an advertised but unadapted callback.

### App-facing bridge

- **Owner:** existing `protocol::bridge` retains `BridgeHandle`, `BridgeSender`, `BridgeChannels`, `SpawnConfig`, channel capacities, engine/spawn selection, fail-stop disconnect delivery, and top-level orchestration. `spawn_bridge` creates process/domain/runtime modules and maps their terminal result; it does not implement their bodies.
- **New seam:** none for callers. Existing `BridgeCommand`, `RoutedNotification`, `PermissionRequest`, source-event, liveness, and shutdown interfaces remain exact.
- **Forbidden:** SDK/schema/raw-frame types in App-facing values; handler registration; conductor construction details; serial command/domain bodies; a runtime selector or compatibility path.

### Source and memory

- **Owner:** existing `protocol::source_observer`, `App::send_prompt`/`dispatch_prompt`, `memory_runtime`, and `capture_forwarder` retain original/prepared placement, exactly-once first-prompt injection, normalized capture, identity, budgets, disposition, and bounded shutdown order.
- **New seam:** none.
- **Forbidden:** wire tap replacing original prompt or normalized capture; prepared blocks recorded as original; retry-time reinjection; SDK types outside core.

### Dependency and documentation direction

- **Owner:** root/core manifests own SDK 2.0.0 and conductor 2.0.0, with default features selected explicitly and `unstable_protocol_v2` absent. ADR-0012 remains the architecture authority; ROADMAP/AGENTS ACP dependency notes are updated where they still describe direct 0.10 or `sacp-proxy`/feature-gated conductor topology.
- **New seam:** none.
- **Forbidden:** direct `agent-client-protocol-schema` dependency, ACP imports outside core, any legacy production source import/caller, simultaneous active runtimes, retained final aliases/shims, or stale documentation claiming a production direct bypass. The only lifecycle exception is the named, unused, core-only `agent-client-protocol-legacy` dependency during the pre-deletion feature-branch checkpoint; it has no source importer and C12 removes it before final delivery.

### Pre-deletion checkpoint lifecycle

- **Purpose:** satisfy the issue's required sequence: prove the complete new conductor path first, then delete the old package family and rerun acceptance.
- **Allowed transient shape:** SDK2/conductor is the sole active runtime; every production source import and caller uses SDK2; ACP 0.10 remains only as the explicitly named, unused `agent-client-protocol-legacy` dependency in `cyril-core` and its lockfile closure.
- **Mechanical limits:** the runtime-phase shape census requires exactly zero legacy Rust imports/references, no `ClientSideConnection`, no direct bypass, and exactly one active runtime. Any second legacy dependency name, source importer, compatibility adapter, or main-history merge is forbidden.
- **Exit:** C12 removes the named dependency and schema closure in the immediately following clean-cutover slice, then the final census and all acceptance gates rerun. Final delivery squashes both green checkpoints into one atomic main-history commit.

## Module shape

### Current cluster inventory

Production-line counts are exact pre-change physical lines up to each file's production/test boundary; `client.rs` excludes its 86 `cfg(test)` helper lines.

| Path | Production lines | Interface and caller knowledge | Responsibility clusters | Dependencies / direction | Callers and tests | Adapters | Change |
|---|---:|---|---|---|---|---|---|
| `protocol/bridge.rs` | 2,973 | Public `spawn_bridge`, `BridgeHandle`, `BridgeSender`, `SpawnConfig`; bounded sends and terminal ordering | App channels, engine/spawn selection, direct connection, command loop, turn/liveness wiring, disconnect diagnostics | App → core bridge → ACP/client/transport/domain modules | `cyril::App`; bridge unit/current-runtime contract tests | direct `ClientSideConnection` + `KiroClient` | split |
| `protocol/client.rs` | 669 | Private `KiroClient::new` + ACP client callbacks | inbound protocol callbacks, conversion dispatch, tool joins, callback enqueue | bridge → client → engine/convert/source/host + ACP | `bridge::run_bridge`; client callback/approval tests | ACP 0.10 `Client` implementation | deepen |
| `protocol/transport.rs` | 240 | `AgentProcess::spawn`, `stderr_tail`; cwd/process/lifetime invariants | subprocess pipes, stderr ring, process group | bridge → transport → tokio/nix | bridge; process/current-runtime tests | `AgentProcess` | retain |
| `protocol/engine.rs` | 332 | private `Engine`, `Adapters`, capability derivation | vendor selection, conversion delegation, turn-end authority | bridge/client → engine → convert/ACP schema | bridge/client; engine tests | `V2Engine`, optional `KasEngine` | retain |
| `protocol/host_mediator.rs` | 260 | `accept/cancel/sweep/shutdown`, typed `Job`, ordered finish | callback lifecycle only | bridge/domain → host mediator → terminal KAS adapters | bridge/client; mediator tests | generic callback metadata | retain |
| `protocol/source_observer.rs` | 446 | `new/begin/observe/finish`; identity/budget/disposition invariants | normalized durable source capture | bridge/domain → source observer → source channel | bridge/App capture; source contract tests | `SourceObserver` | retain |
| `protocol/turn_mediator.rs` | 370 | `begin_turn`, notification disposition, cancellation/companion state | serial turn ownership and terminal meaning | bridge/domain → turn mediator → domain events | bridge; pure state tests | `TurnMediator` | retain |
| `protocol/tool_call_ledger.rs` | 59 | merge/snapshot by `(SessionId, ToolCallId)` | permission preview join | client/domain → ledger → domain tool types | client permission path; ledger tests | `ToolCallLedger` | retain |
| `protocol/mod.rs` | 35 | private module declarations/re-exports | protocol namespace wiring | core only | all protocol modules | N/A — namespace module | retain |
| `crates/cyril/src/app.rs` | 2,723 | `App` event loop over SDK-independent core types | orchestration, UI projection, memory/source lifecycle | cyril → core/UI/memory/voice | binary; App current-runtime tests | Bridge and memory handles | retain |
| `protocol/sdk_runtime/` | 0 | proposed exact three-argument start + handle | official conductor/stage/process/lifecycle composition | bridge → sdk runtime → SDK/conductor/transport/client | bridge; C2/C3/C4/C6/C10/C11/C12 tests | official roles + concrete process adapter | create |
| `protocol/domain_mediator/` | 0 | proposed constructor/channels/run | serial domain state, command dispatch, inbound/host/turn/source mediation | bridge/runtime/client → mediator → existing domain modules | bridge; C1/C5/C7/C8/C9/C13 tests | intrinsic Send-to-serial channel seam | create |

### Seam tests

- **Deletion:** removing `sdk_runtime` redistributes conductor/stage/process/lifecycle composition into bridge/client; removing `domain_mediator` redistributes the serial command, conversion, turn, source, and host policy across Send handlers and bridge. Both modules hide real complexity.
- **Interface:** callers and tests exercise runtime behavior through the three-argument start handle and domain behavior through the existing Bridge/App contract; claim-local tests do not reach around those interfaces except the non-production shape census.
- **Adapter:** SDK runtime uses the official `ConnectTo<R>` seam, already implemented by concrete process, channel, proxy, and conductor adapters. DomainChannels needs no second adapter because incompatible Send and serial ownership makes the actor seam intrinsic.
- **Locality:** stage/lifecycle composition has one owner (`sdk_runtime`); handler registration one (`client`); serial domain policy one (`domain_mediator`); process ownership one (`transport`); vendor meaning/effects stay in their current modules.

### Three materially different shapes

#### A — two deep private modules over official roles (selected)

- **Interface:** exact `SdkRuntime::start(process, domain_channels, stages)` plus `DomainMediator::new(...)/run(handle)`.
- **Caller:** `bridge::run_bridge` resolves engine/process, constructs mediator/channels, starts runtime, then runs mediator.
- **Hidden implementation:** conductor chain, process adapter, SDK client lifecycle, bounded actor protocol, commands/inbound/host/turn/source composition.
- **Dependencies/adapters:** official `ConnectTo<R>` roles; concrete `AgentProcess` adapter; intrinsic bounded actor seam.
- **Trade-off:** one additional internal handoff, but maximal locality, no custom runtime vocabulary, small bridge wiring, and independently testable actor/conductor behavior.

#### B — bridge-centric in-place SDK rewrite (rejected)

- **Interface:** keep only `spawn_bridge`; embed Client builder, conductor, process adapter, command conversion, and serial state in `bridge.rs`.
- **Caller:** App remains trivial.
- **Hidden implementation:** almost none beyond the already 2,973-line production parent.
- **Dependencies/adapters:** bridge imports every SDK role and every domain adapter.
- **Trade-off:** least call-site churn, but adds multiple new responsibility clusters to a god file, prevents a mechanical protected-parent fence, and gives tests no shared seam with production composition.

#### C — generic Cyril runtime interface with direct/conductor adapters (rejected)

- **Interface:** `trait AgentRuntime`/`AgentEndpoint`, runtime factory, direct and conductor implementations.
- **Caller:** bridge selects an implementation from configuration.
- **Hidden implementation:** SDK roles behind Cyril aliases and a runtime switch.
- **Dependencies/adapters:** one real production topology plus a retained compatibility adapter.
- **Trade-off:** extension flexibility, but a shallow hypothetical seam, duplicated official vocabulary, dual path, and violation of ADR-0012/C3/C12.

### Module ledger

| Module/path | Interface | Owns | Hides/reuses | Must not own | Adapters | Tests through | Change |
|---|---|---|---|---|---|---|---|
| `protocol/sdk_runtime/mod.rs` | exact `start(process, domain_channels, stages) -> handle`; bounded close/completion | official conductor, ordered stages, connection lifecycle | SDK Client/Agent/Proxy/Conductor, client runner, process adapter | Engine meaning, host effects, App projection, observer API, direct bypass | official `ConnectTo<R>`, `DynConnectTo<Conductor>` | `SdkRuntime`/Bridge contract | create |
| `protocol/sdk_runtime/process.rs` | concrete `ConnectTo<Client>` adapter | process-pipe-to-SDK frame driving and test-only pre-parse recording point | `AgentProcess`, SDK ByteStreams/Lines | spawn policy, cwd selection, stderr ownership, domain conversion, production observer | real AgentProcess; fixture pipes in tests | official role interface | create |
| `protocol/domain_mediator/mod.rs` | constructor returning bounded `DomainChannels`; `run(handle)` | serial select loop, local Engine state, connection/session/turn/liveness composition | command/inbound/host submodules and existing mediators | conductor/stage construction, SDK handlers, App/UI/memory logic | intrinsic actor seam | existing Bridge/App types | create |
| `protocol/domain_mediator/commands/mod.rs` + family children | private total dispatch over `BridgeCommand` | outgoing command mapping, typed SDK requests, exactly-one outcome | session, extensions/settings/steering, subagent, KAS workflow/hooks families | incoming handlers, vendor conversion, App rendering | official `ConnectionTo<Agent>` | C13 table through Bridge | create |
| `protocol/domain_mediator/inbound.rs` | private `DomainWork` application | ordered notification/permission projection, turn/tool/source state | Engine/convert/TurnMediator/ToolCallLedger/SourceObserver | SDK registration, host effects, App state | bounded DomainChannels | C5/C8/C9 through Bridge | create |
| `protocol/domain_mediator/host.rs` | private callback acceptance/cancel/shutdown wiring | serial host lifecycle orchestration | HostMediator + KAS effect adapters | effect implementations or capability policy | typed host jobs | C7 callback matrix | create |
| `protocol/client.rs` | private SDK client runner over official agent component + DomainChannels | ordered untyped/typed handlers, bounded enqueue, responder tasks | SDK Client builder and callback schemas | Engine/turn/source/tool state, host effects, stage construction | official Client role | client/Bridge callback tests | deepen |
| `protocol/transport.rs` | unchanged AgentProcess interface | spawn/cwd/pipes/stderr/process tree | tokio/nix | SDK handler/domain/proxy responsibilities | AgentProcess | process tests | retain |
| `protocol/engine.rs`, `convert/`, `kas/`, `host_mediator.rs`, `source_observer.rs`, `turn_mediator.rs`, `tool_call_ledger.rs` | existing private domain interfaces | existing vendor/effect/turn/source/tool responsibilities | SDK 2 schema only at core conversion sites | runtime topology or App responsibilities | V2/KAS/domain adapters | existing tests + new path fences | retain |
| `protocol/bridge.rs` | existing public Bridge interfaces | App channels, engine/spawn choice, fail-stop mapping, top-level wiring | new runtime/mediator | SDK handler/conductor/serial command responsibility bodies | runtime and mediator concrete modules | Bridge/App contract | split |
| App/UI/memory/voice | unchanged SDK-independent interfaces | current orchestration/rendering/persistence/voice responsibilities | core domain types | any SDK/schema/raw-frame type | existing Bridge/memory adapters | existing crate tests | retain |

### Protected parents

| Protected parent | Baseline responsibilities | Allowed change | Forbidden change | Exit condition |
|---|---|---|---|---|
| `protocol/bridge.rs` (2,973 production lines) | public bridge channels/config, spawn/engine choice, fail-stop mapping, current direct runtime and serial loop | delete direct runtime/serial bodies; add declarations and thin construction/delegation to runtime+mediator | SDK handler registration, conductor/process adapter implementation, new command family body, observer/runtime switch | no `ClientSideConnection`; no SDK builder/handler/conductor symbol; no `match BridgeCommand` body; production lines decrease and remain within plan ledger |
| `protocol/mod.rs` (35 production lines) | namespace wiring | declare `sdk_runtime` and `domain_mediator` | implementation body or public re-export outside core | declaration-only diff; growth within plan ledger |
| `crates/cyril/src/app.rs` (2,723 production lines) | SDK-independent orchestration, UI projection, memory/source lifecycle | no production change; current-runtime tests may be reused/renamed outside the parent test body | SDK import/type, event-order rewrite, memory/source relocation, new production responsibility | zero production-line delta and no SDK import |
| root and non-core member manifests | workspace dependency versions and crate-specific dependencies | replace workspace ACP version; add conductor only to core; pre-deletion checkpoint may retain the one named unused core-only legacy dependency | ACP/conductor dependency in cyril/UI/memory/voice; any legacy source importer/caller, second runtime, compatibility adapter, or final old+new coexistence | runtime phase: SDK2 is sole active path and only the named unused legacy dependency remains; final phase: one SDK family and only cyril-core consumes ACP/conductor |

### Mechanical shape claim

C14 is enforced by `.cyril-gl5s/oracles/module_shape.py` with an issue-local ledger manifest. It discovers the upstream/default branch, compares committed and working-tree changes, and reports `claim_ids: ["C14"]` plus the exact path/symbol/delta. Runtime phase checks required/forbidden paths, SDK dependency/import direction, exactly zero legacy source importers, sole conductor runtime, `SdkRuntime::start` arity, forbidden observer/runtime-trait symbols, unknown-first registration ownership/order, protected-parent deltas, required test locations, and growth tripwires while permitting only the named unused legacy dependency. Final phase additionally requires C12's one-family package graph. It never runs from production code.

Named mutation: add `pub(crate) trait AgentRuntime {}` to `protocol/sdk_runtime/mod.rs`. The shape fence must fail with C14 and that exact symbol/path; restoration must return green. Adding a legacy Rust import during runtime phase and adding the old dependency during final phase are separate positive controls.

## Claims

- **C1.** SDK `Send + 'static` handlers communicate with serial `Rc` domain state only through bounded typed `DomainChannels`.
- **C2.** Every production connection uses `ConductorImpl` with zero or more ordered stages, including the empty chain.
- **C3.** Official `ConnectTo<R>` roles are the only runtime interface; Cyril adds no endpoint/runtime trait or direct path.
- **C4.** The process adapter preserves exact pre-parse ingress bytes, frame shape/order, malformed evidence, and unknown/extension semantics.
- **C5.** The unknown-standard fence precedes typed handlers, forwarding preserves identity/cancellation/lifetime, and handlers enqueue then return before slow work.
- **C6.** Retained `AgentProcess` behavior preserves cwd, bounded diagnostics, stall, process-tree, EOF, cancellation, and shutdown grace.
- **C7.** Engine/conversion and terminal host adapters remain the sole owners of Kiro/KAS meaning and every advertised callback effect.
- **C8.** App-facing types and notification/error/completion/disconnect order remain SDK-independent and unchanged.
- **C9.** First-prompt memory and normalized source semantics preserve every I10 placement, identity, budget, exactly-once, pressure, disposition, and shutdown cell.
- **C10.** Runtime start has exactly three arguments and exposes no observer, inspection, tracing, or multi-client registration.
- **C11.** Production defaults to stable wire v1 and confines all SDK dependencies/imports to core.
- **C12.** After the approved pre-deletion checkpoint proves the sole active SDK2/conductor path, the clean-cutover slice removes the one named unused legacy dependency and leaves one SDK family with no old connection, bypass, alias, or shim.
- **C13.** Every BridgeCommand, engine outcome, routed identity, extension shape, saturation, and disconnect cell produces the exact current typed outcome once without hanging.
- **C14.** The module tree matches the approved ledger in both lifecycle phases: each responsibility has one owner, protected parents gain no responsibility body, the runtime phase has no legacy source importer or second runtime, and the final phase has one package family.

## Falsification

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C1 | Bounded actor handoff | I1, I5, I15 | Run new runtime with capacity-one work and local `Rc` state; falsified by cross-thread state, unbounded queue, loss, or deadlock. Closed-channel and full-channel controls distinguish actor wiring from domain logic. | `.cyril-41bs` standalone `rustc` negative `Rc` move plus bounded actor oracle. | Capture `Rc<dyn Engine>` in the SDK `tokio::spawn`; compile must fail `E0277`, localized to C1. | `c1_bounded_handoff_keeps_domain_state_local` + compile Send bounds | <5 s | PENDING — checkpointed-build runtime spine slice |
| C2 | Conductor-only ordered topology | I1, I2, I7, I11 | Compare zero/no-op/transform/distinct/repeated SDK harnesses; falsified by factory/path/order/lifecycle divergence. Direct-agent and reversed-stage controls distinguish conductor composition from vendor behavior. | Upstream conductor tests + `.cyril-41bs` E6/E7 deterministic/live comparators. | Add empty-stage direct connect or reverse StageChain iteration; C2 fence names path/order. | `c2_zero_proxy_and_ordered_chain_match_direct_contract` | <30 s deterministic; authenticated matrix later | PENDING — checkpointed-build runtime spine slice |
| C3 | Official role interface only | I2, I13 | Source/type census plus compile harness; falsified by custom runtime trait, bypass, or role alias. Positive control injects a forbidden trait so absence is decisive. | SDK `ConnectTo<R>` source/API audit and independent manifest/source census. | Add `pub(crate) trait AgentEndpoint {}` in sdk_runtime; fence names symbol. | `c3_sdk_topology_uses_official_roles_only` + C14 shape oracle | <5 s | PENDING — checkpointed-build runtime spine slice |
| C4 | Exact ingress/frame semantics | I3, I4 | Feed segmented real-process fixture with batch malformed member, unknown update, extension, and `1e400`; compare recording-reader bytes and semantic frames. A post-parse recorder and first-member-only mutation are controls. | Independently assembled bytes/SHA-256 + Python JSON/Decimal oracle. | Move recording point after parse or forward first batch member only; fence reports exact byte/member. | `c4_process_ingress_captures_before_parse_and_preserves_frames` | <10 s | PENDING — checkpointed-build transport slice |
| C5 | Ordered nonblocking handlers | I4, I5, I11 | Unknown-first/reversed/removed and slow-domain/fast-notification matrices; falsified by drop, wrong claim, changed ID/cancel, or delayed fast method. Queue-full is a separate bounded-pressure control. | Exact upstream handler/dynamic/cancellation tests + `.cyril-41bs` E3/E6. | Await mediator or host reply inside an SDK handler; fence reports delayed fast method. | `c5_unknown_update_fence_precedes_typed_handler`; `c5_forwarding_preserves_outer_identity_and_cancellation`; `c5_slow_handler_does_not_block_dispatch` | <10 s | PENDING — checkpointed-build handler slice |
| C6 | Process parity | I3, I6, I11 | Run cwd/diagnostic/group/stall/cancel/EOF/grace matrix through ProcessAdapter; falsified by any changed cell. Direct OS fixture separates process policy from SDK errors. | `.cyril-g0dg` current process oracle + `.cyril-41bs` direct OS E4. | Replace AgentProcess with AcpAgent or omit `.current_dir(cwd)`; fence fails `cwd_unicode_spaces`. | `c6_process_component_preserves_agent_process_contract` + existing transport tests | 5–15 s | PENDING — checkpointed-build transport slice |
| C7 | Terminal vendor/effect ownership | I1, I4, I7 | Run all callback families through direct/conductor deterministic matrix and authenticated KAS/v2 turns; falsified by missing family, proxy-owned effect, or normalized-order drift. Unsupported-family and v2 host-callback N/A controls prevent default-filled success. | KAS covenant, committed captures, current Engine/host tests, independent live comparator. | Move auth/terminal effect into a proxy or remove one registered callback; fence names owner/method. | `c7_terminal_client_owns_all_callback_families` + existing callback tests | <30 s deterministic; authenticated later | PENDING — checkpointed-build callback slice |
| C8 | App contract/order parity | I1, I8, I10, I11, I13 | Compare typed event traces through conductor for success/error/EOF/crash/shutdown; falsified by type leak or first changed event. A pure SessionController/UiState projection distinguishes bridge order from UI rendering. | `.cyril-g0dg` App ordering table and independent state projections. | Emit BridgeDisconnected before terminal completion or add SDK raw type to RoutedNotification; fence names event/type. | `c8_app_contract_and_terminal_order_are_unchanged` | <10 s | PENDING — checkpointed-build vertical App turn slice |
| C9 | Memory/source parity | I8, I10, I15 | Run complete I10 table through App→conductor→source persistence; falsified by any original/prepared, injection, budget, identity, pressure, disposition, or shutdown cell. Static expected rows and persisted reload separate source behavior from wire capture. | `.cyril-g0dg` memory/source matrix + `.cyril-41bs` E9 tap comparison. | Inject every prompt, capture prepared as original, change byte bound, map Interrupted→Completed, or constant identity; fence names exact I10 cell. | `c9_memory_and_source_matrix_survives_sdk_topology` | <20 s | PENDING — checkpointed-build vertical App turn slice |
| C10 | No observer runtime interface | I2, I14 | Shape/signature census and E8 pressure characterization; falsified by fourth parameter or inline observer work. Positive mutation proves the absence check. | Python observer model + SDK channel source audit + compile interface use. | Add observer parameter and `bridge_with_inspection`; fence names argument/symbol. | `c10_sdk_runtime_has_no_observer_parameter` + C14 shape oracle | <5 s | PENDING — checkpointed-build runtime spine slice |
| C11 | Stable v1/core-only SDK | I12, I13 | Run official negotiation tests and workspace census; falsified by default v2, wrong fallback, or non-core importer. Explicit-v2 positive test proves version discrimination. | Exact upstream negotiation tests + independently parsed manifests/source. | Enable `unstable_protocol_v2` or import ACP in cyril-ui; fence names feature/path. | `c11_sdk_runtime_defaults_to_v1`; `c11_only_core_imports_acp` | <5 s | PASS — `python3 .cyril-41bs/probe.sdk2/oracles/e10.py` on 2026-08-30 |
| C12 | One-family clean cutover | I13 | After the new path passes complete acceptance, remove the named legacy dependency and run lock/manifest/source/topology census; falsified by 0.10/schema 0.11, ClientSideConnection, direct bypass, alias, or shim in final phase. The runtime-phase positive control proves SDK2 is active before deletion; required SDK2 presence prevents vacuous success. | Independent package graph and source census against pinned SDK2 lock plus recorded pre-deletion acceptance. | Retain/re-add the named old dependency or ClientSideConnection after cleanup; fence names package/symbol. | `c12_sdk2_cutover_has_one_transport_family_and_runtime` | <10 s | PENDING — checkpointed-build clean-cutover slice |
| C13 | Complete command/routing contract | I1, I4, I8, I9, I11, I15 | Table-drive every command/nested cell, engine failure, routed identity, params shape, full queue, closed peer; falsified by missing/duplicate/retyped output or deadline. Indexed expectations separate fake recording from expected results. | `.cyril-g0dg` hand-authored command/routing/saturation tables + independent App projections. | Drop ListSettings, coerce None session, coerce params to object, or use lossy try_send; fence names exact cell. | `c13_all_bridge_commands_and_routing_shapes_cross_sdk_topology` | <30 s | PENDING — checkpointed-build command/routing slice |
| C14 | Approved phased module shape | I13, I14, I16 | Run runtime/final path/dependency/symbol/diff/line censuses; falsified by missing owner, wider interface, forbidden parent body, legacy source importer/second runtime in runtime phase, or any old package in final phase. Positive forbidden-symbol, legacy-import, and protected-parent mutations make absence checks decisive. | Standalone Python manifest/diff census, independent of Rust behavior tests. | Add `pub(crate) trait AgentRuntime {}` or a legacy Rust import during runtime phase; fence reports C14 and exact path/symbol. | `.cyril-gl5s/oracles/module_shape.py --phase runtime|final` | <5 s | PENDING — checkpointed-build every slice and final fresh-context conformance review |

## Non-goals and future work

### Permanent non-goals of this cutover

- No draft ACP wire v2 enablement: SDK package major and wire version are independent; stable wire v1 is the accepted production contract.
- No App/UI/domain rewrite: existing typed contracts already provide the migration oracle and moving them would erase the equivalence boundary.
- No `AcpAgent` process ownership: it lacks Cyril's cwd, stall, and diagnostic interface.
- No compatibility source alias, runtime switch, direct bypass, or rollback flag: the named transient legacy dependency has no source importer or adapter, C12 removes it before final delivery, and rollback is reverting the one atomic cutover commit.
- No placeholder stage crate or speculative proxy: an empty official StageChain is concrete zero-stage operation.
- No production capture/observer registration: exact-ingress verification uses the private process-adapter test seam, not a new user-facing capability.

### Intended future work

- `cyril-5g2o` (verified open 2026-08-30) owns multi-client observer/multiplex topology, its separate bounded broadcaster, replay, pressure, and ownership policy.
- `cyril-1ixa` (verified open 2026-08-30) owns the trigger-conditioned unbounded notification-pressure risk. This cutover preserves current bounded App/mediator contracts but does not claim a new agent-side backpressure design.
- Later vendor-neutral agent selection remains under verified parent epic `cyril-1gfe`; this cutover preserves only the explicitly supported Kiro v2/KAS selection matrix.

## Falsifier run log

- **2026-08-30 — C11 cheapest falsifier:** `python3 .cyril-41bs/probe.sdk2/oracles/e10.py` → PASS. Output named `claim_ids: ["C11"]`; all three official v1/v2 negotiation tests passed; explicit unstable-v2 separation passed; independent manifest/source census found only `cyril-core` imports ACP.

## Approval

- **Status:** APPROVED.
- **Requester words:** “I approve the revised design”
- **Date:** 2026-08-30.
- **Risk acceptances:** None.
