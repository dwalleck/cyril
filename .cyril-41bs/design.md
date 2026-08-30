# Falsifiable design: cyril-41bs

## Route and inputs

- **Route:** Empirical. Source: [`route.md`](route.md), selected because the target topology depended on runtime, wire, process, and Kiro/KAS behavior.
- **Specification:** N/A — `route.md` T4 records the complete explicit behavior set; no `spec.md` is required.
- **Behavior set:** E1 safe `Send + 'static` actor handoff; E2 single/batch/malformed/extension/unknown-update/extreme-number frame behavior; E3 ordered handler claim/fallthrough, dynamic lifetime, cancellation, and nonblocking dispatch; E4 process cwd/diagnostics/group/stall/cancel/EOF parity; E5 direct Kiro v2/KAS parity; E6 zero/no-op/transform conductor parity; E7 useful bidirectional proxy leverage; E8 observer pressure/ownership/replay limits; E9 memory injection and normalized-capture parity; E10 stable-v1/draft-v2 separation. Exact Given/When/Then statements are in `route.md` T4.
- **Empirical premises:** P1–P10 in [`evidence.md`](evidence.md) all pass. Reproducers are under [`probe.sdk2/`](probe.sdk2/).
- **Independent oracles:** standalone `rustc` ownership controls; Python JSON/`Decimal`; direct OS subprocess checks; exact upstream SDK/conductor tests; committed Kiro v2/KAS captures and the KAS covenant; Python queue timing; and Cyril's existing source/memory contract tests. Exact comparisons are in `evidence.md`.
- **Selected architecture:** **Option C — conductor-first production topology.** E5/E6 proved transparent direct/conductor parity for Kiro v2 and KAS, and E7 proved useful bidirectional transformation without moving `Engine`, host-callback, or normalized-observer ownership into a proxy.
- **Route deliverables:** an approved ADR must amend or supersede ADR-0003; blocking upstream gaps receive issue/PR proposals; independently green follow-on work uses verified Rivets records; only reproducible probes remain; no speculative production crate/trait/migration or disposable prototype survives. C14 is discharged by `.cyril-41bs/checkpoints/C14.json`.

## Input shapes

| ID | Production-reachable shape | Status |
|----|----------------------------|--------|
| I1 | Engine selection: Kiro v2 and KAS; selection success and unsupported/failed selection. A later vendor is not assumed by this design. | Successful v2/KAS selection is covered by C2/C7; unsupported/failed selection and App projection are assigned to pending C13/C8. Later vendors are intended future work under `cyril-1gfe`. |
| I2 | Proxy chain collection: empty; one no-op; one transforming; multiple distinct stages; repeated instances of the same stage type. Order is significant; repeated instances are independent ordered entries, not deduplicated sentinels. | Covered by C2/C3. `oracles/e6.py` runs the upstream three-component initialization test; its two proxy instances use the same component type and therefore cover ordered repeated-type instances. |
| I3 | `TransportFrame`: `Single`; non-empty `Batch` with one/many, distinct/duplicate message entries and malformed members; top-level `Malformed`. Empty batch is N/A — the SDK type excludes it. | Semantic behavior is established by E2; production-ingress fidelity is assigned to pending C4. |
| I4 | JSON-RPC request, notification, success response, error response; standard and Kiro extension methods; known and unknown method/update variants; metadata absent, present, additive-unknown; integer/string request IDs; extreme numeric lexemes. | Semantic/handler behavior is covered by C5/C7; exact pre-parse capture is assigned to pending C4. |
| I5 | Handler path: typed/untyped static handlers, first-handler claim, fallthrough, unmatched request/notification, dynamic handler attached/dropped/detached, forwarded response, cancellation before/after forwarding, slow work. | Covered by C5. |
| I6 | Process configuration: program plus empty/multiple arguments, empty/multiple environment entries, cwd relative/absolute/Unicode/embedded spaces; clean exit, nonzero exit, stderr burst/no newline, stall, cancel, stdout EOF, shutdown grace, child plus grandchild. | Covered by C6. Duplicate environment names are N/A — `AgentCommand`/maps represent one final value per key. |
| I7 | Client callbacks: authentication; standard and Kiro fs read/write/stat/read-directory/delete; terminal create/output/wait/release/kill/shell type; permission with empty/single/multiple options and trust options; hooks list/execute/session-start/cancel/change. | Covered by C7. Existing Engine/host validation owns malformed callback payloads. |
| I8 | Routing identity: `RoutedNotification.session_id` absent/main/subagent/unknown; turn ID absent/present; session creation/load/terminate/message/steer/clear; extension params object/array/null/additive unknown; disconnected channels and bounded-channel saturation. | The unchanged pre-migration baseline is the C8 oracle; post-migration App projection is pending C8 and the exhaustive routing matrix is pending C13. |
| I9 | Every `BridgeCommand` family: prompt/new/load/cancel/mode/model; extension; settings list; usage-account query; command options/execute; subagent spawn/terminate/message; steer/clear; KAS hooks list/toggle and workflow; shutdown. Empty/Unicode identifiers and paths remain subject to their existing typed validation. | Assigned to pending C13's exhaustive post-migration command/routing fence. |
| I10 | At the bridge seam: first versus subsequent prompt; one/multiple original blocks; prepared context absent/present; UTF-8 and tool byte-budget boundaries; completed/interrupted/cancelled/error terminal dispositions; source/session/project identity reuse and change. Memory unavailable/starting/ready behavior is established before this seam. | The current seam/baseline is established by E9; exhaustive post-migration behavior is assigned to pending C9. |
| I11 | E8 characterization shapes: a concurrent stream around a permission request; two inline observers including one slow observer; observer disconnect; a fresh attachment requesting replay. | Exercised by claim-local E8 and `oracles/e8.py`: order held, permission stayed on the linear path, slow/disconnected observers affected forwarding, and a fresh channel had no replay. |
| I12 | Protocol negotiation: default stable v1, explicit draft v2, v2-to-v1 negotiation fallback, v2 initialize rejection, and no post-handshake semantic translation. | Covered by C11. Shipping draft v2 is a permanent non-goal of this migration. |
| I13 | Error/termination ordering: initialize error, prompt error, KAS `turn_end` before prompt response, callback error, clean EOF idle/mid-turn/after-turn, crash, deferred disconnect, user shutdown. | Current SDK/process behavior is covered by C5/C6; post-migration App projection is pending C8. |
| I14 | Dependency placement: SDK core/conductor in `cyril-core`; App/UI/memory/voice see only Cyril domain types; old ACP 0.10 and schema 0.11 absent after the migration clean cutover. | Current direction is covered by C3/C11; post-migration App direction/cutover are pending C8/C12. |
| I15 | Production migration observer API: no observer/inspection/tracing registration parameter; no multi-client broadcaster in this migration. | Assigned to pending C10. Future multi-client behavior is intended work in verified `cyril-5g2o`. |
| I16 | Route deliverables: approved ADR-0003 amendment/supersession; blocking upstream proposal disposition; verified green follow-on issues; reproducible-probe retention; no speculative production code or disposable prototype. | Discharged by C14 and `.cyril-41bs/checkpoints/C14.json`. |

### Removed invariants

The migration is subtractive: it removes the old direct `ClientSideConnection` path and its accidental same-thread serialization.

- The old `!Send` connection made it impossible to move `Rc<dyn Engine>` across threads. C1 preserves that fact with a bounded typed actor seam rather than shared locks.
- The direct path guaranteed there was no intermediate request-ID or cancellation hop. C2 and C5 require the official conductor to preserve outer identity and hop-local cancellation.
- Typed deserialization happened before Cyril could see unknown standard updates. C4 and C5 retain a raw/untyped, statically first containment fence.
- One process owner silently guaranteed cwd, stderr-tail, stall, and process-tree behavior. C6 keeps `AgentProcess` as the process owner.
- One client silently avoided fan-out ownership and pressure policy. C10 explicitly keeps multi-client delivery outside the conductor.
- Prompt preparation and normalized observation occupied different sides of the wire seam. C9 keeps both positions unchanged.

## Placement

### 1. SDK topology and chain lifecycle

- **Owner:** new private `cyril-core::protocol::sdk_runtime` module, called from `protocol::bridge::run_bridge` at the current connection-construction site. It owns SDK `Client`/`Agent`/`Proxy`/`Conductor` composition, `ConductorImpl::new_agent`, lazy stage construction, frame driving, and connection termination.
- **New seam:** use the proven official role-directed `ConnectTo<R>` interface directly. Production always uses the conductor; an empty stage vector is ordinary zero-proxy operation. Private entrypoint: `SdkRuntime::start(AgentProcess, DomainChannels, StageChain) -> Result<SdkRuntimeHandle>`; it has no observer argument.
- **Competing shape A — rejected:** a Cyril-owned `AgentEndpoint`/`AgentRuntime` trait with direct and conductor adapters. It is shallow, duplicates official routing vocabulary, and retains a dual path after Option C won.
- **Competing shape B — selected:** private official-SDK role adapters plus one conductor path. It gives proxy leverage and locality without a permanent Cyril trait.
- **Competing shape C — rejected:** a raw `TransportFrame` relay as the primary seam. It is useful for inspection but cannot own typed callbacks, domain conversion, or chain lifecycle.
- **Forbidden:** no direct production bypass when the stage vector is empty; no compatibility alias or deprecated ACP 0.10 path; no public SDK runtime interface outside `cyril-core`.

### 2. Send protocol actor to serial domain mediator

- **Owner:** new private `cyril-core::protocol::domain_mediator` module, extracted from `KiroClient`'s domain responsibilities. It owns `Rc<dyn Engine>`, ingress/tool ledgers, normalized conversion, turn/source mediation, and sends existing Cyril domain events.
- **New seam:** bounded typed channels between SDK handlers and the serial mediator. Request work carries a typed reply channel; notification work carries typed/raw ingress required for Engine conversion. This is a real seam because the two sides have incompatible ownership contracts.
- **Competing shape A — rejected:** make `KiroClient`/`Engine` `Arc<Mutex<_>>` and capture them in SDK handlers. It spreads synchronization through the domain and makes ordering implicit.
- **Competing shape B — selected:** SDK handlers remain `Send + 'static`; the mediator remains current-thread and serial; bounded messages cross between them.
- **Forbidden:** unsafe code; blanket shared locks; unbounded actor queues; awaiting host work inline on the dispatch loop.

### 3. Process lifecycle

- **Owner:** existing `protocol::transport::{AgentProcess, ProcessGroupGuard, StderrTail}`.
- **New seam:** no Cyril interface. Add a private adapter implementing SDK `ConnectTo<Client>` over the existing process pipes; the official SDK seam is sufficient.
- **Forbidden:** replacing `AgentProcess` with `AcpAgent`; losing explicit cwd, stall detection, public stderr-tail diagnostics, kill-on-drop, or grandchild cleanup.

### 4. Client handlers, vendor conversion, and host callbacks

- **Owner:** `protocol::client` owns statically ordered SDK handler registration; `protocol::engine` and `protocol::convert` keep vendor meaning; `protocol::host_mediator` and KAS callback modules keep side-effect ownership.
- **New seam:** no vendor abstraction. The SDK client handler sends ingress/request work to the serial mediator; existing Engine adapters remain the vendor seam.
- **Forbidden:** proxy stages may not import/own `Engine`, KAS callback policy, filesystem/terminal/hook effects, tool ledger, or terminal disposition; the typed `SessionNotification` handler may not precede the unknown-update fence.

### 5. App-facing bridge and observable turn contract

- **Owner:** `protocol::bridge` continues to own `BridgeHandle`, `BridgeSender`, command ingress, bounded App notifications/permissions/source events, liveness, and shutdown; `cyril::App` remains an SDK-independent orchestrator.
- **New seam:** none. Existing `BridgeCommand`, `RoutedNotification`, `PermissionRequest`, and source-event interfaces remain the test surface.
- **Forbidden:** no SDK `Dispatch`, `ConnectionTo`, schema types, `serde_json::Value`, proxy envelopes, or raw frames in App/UI/memory/voice interfaces; no second App event-channel rewrite.

### 6. Source observation and memory

- **Owner:** `protocol::source_observer` remains the normalized domain observer; `App::send_prompt`/`dispatch_prompt`, `memory_runtime`, and `capture_forwarder` retain first-prompt injection and durable capture orchestration.
- **New seam:** none.
- **Forbidden:** SDK inspection or a proxy may not replace original-prompt capture, normalized inbound capture, project/session/turn identity, terminal disposition, byte budgets, or exactly-once injection.

### 7. Wire inspection and observer fan-out

- **Owner:** the private `ConnectTo<Client>` process adapter captures exact inbound bytes at the `AgentProcess` stdout boundary before SDK/`serde_json` parsing, then hands semantic frames to `sdk_runtime`. E2 proves why SDK inspection cannot provide this guarantee; C4 remains pending until the production-ingress fixture passes.
- **New seam:** a private lexical capture hook at process ingress, used only for containment/audit evidence; no user-facing tracing registration. No broadcaster is added. Verified issue `cyril-5g2o` owns the future multi-client interface; verified issue `cyril-1ixa` records current unbounded-pressure risk.
- **Forbidden:** do not add an inline optional observer, call the linear conductor a fan-out bus, or let diagnostic work join the forwarding critical path.

### 8. Dependency direction and wire version

- **Owner:** workspace and `cyril-core` manifests.
- **New seam:** none.
- **Forbidden:** no ACP dependency outside `cyril-core`; no simultaneous ACP 0.10/SDK 2 production families; no default `unstable_protocol_v2` feature; no semantic v1↔v2 translation.

## Claims

- **C1.** SDK `Send + 'static` handlers and Cyril's `!Send` domain mediator communicate only through bounded typed channels, so `Rc` domain state never crosses the actor seam.
- **C2.** Every production ACP connection runs through `ConductorImpl` with zero or more ordered proxy stages, and zero-proxy behavior is observably equivalent to the direct SDK path.
- **C3.** The official `ConnectTo<R>` role interface is the only runtime interface; private Cyril structs implement that interface, but Cyril adds no endpoint/runtime trait and retains no conductor-bypass production path.
- **C4.** The process adapter captures exact inbound bytes before SDK parsing, while SDK transport handling preserves frame shape, source order, malformed evidence, and extension/unknown semantic behavior.
- **C5.** Unknown-standard containment is statically ordered before typed handlers; forwarding preserves identity/cancellation/dynamic lifetime; because a slow SDK handler blocks later dispatch, Cyril handlers enqueue bounded mediator work and return before slow domain work.
- **C6.** The terminal process adapter retains Cyril's cwd, stderr-tail, stall, process-group, EOF, cancellation, and shutdown contracts by continuing to use `AgentProcess`.
- **C7.** `Engine`, convert modules, and the terminal host mediator remain the sole owners of Kiro/KAS meaning and side effects; proxies transform wire concerns only.
- **C8.** App-facing domain types, permission/source interfaces, and turn/error ordering remain SDK-independent and behaviorally unchanged.
- **C9.** First-prompt memory injection and normalized `SourceObserver` capture remain at their current seams; the migration must preserve original/prepared prompt placement, budgets, identity, exactly-once behavior, and terminal disposition across I10.
- **C10.** Production `SdkRuntime::start` has no observer/inspection/tracing parameter; E8 characterizes why future multi-client fan-out requires a separate bounded broadcaster.
- **C11.** SDK 2 uses stable ACP wire v1 by default, draft wire v2 remains disabled, and SDK types remain confined to `cyril-core`.
- **C12.** The production migration cleanly removes ACP 0.10/schema 0.11 and the old direct connection path without aliases, compatibility shims, or a dual runtime.
- **C13.** Every existing engine-selection result, `BridgeCommand` variant, routed identity shape, extension-param shape, disconnect, and bounded-saturation path crosses the SDK/conductor migration without loss or SDK-type leakage.
- **C14.** The architecture spike lands its route deliverables atomically: an approved ADR amends/supersedes ADR-0003, every blocking upstream gap has an explicit proposal disposition, verified follow-on work is linked, reproducible probes remain, and speculative/disposable code does not.

## Falsification

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|-------|-------------|-----------|--------|----------------|------------------|------|--------|
| C1 | Bounded actor handoff keeps `Rc` state local. | I1, I5, I8; removed same-thread invariant. | Run claim-local `e1`; falsified if the SDK component cannot exchange bounded work without moving/sharing `Rc` state. | Claim-local `oracles/e1.py` independently compiles a forbidden `Rc` thread move and a valid bounded handoff. | In `protocol/sdk_runtime.rs`, change the actor launch to capture `Rc<dyn Engine>` inside `tokio::spawn`; `cargo check -p cyril-core` must report `E0277` and the fence cannot compile. | Required migration test `c1_bounded_handoff_keeps_domain_state_local` plus compile-time `Send` bounds. | <1 second cached | PASS |
| C2 | Conductor-first zero-or-more topology preserves direct behavior. | I1, I2, I7, I13. | Run claim-local `e6`, `oracles/e6_live_parity.py`, and `oracles/e6.py`; falsified if any direct/conductor engine-topology cell loses protocol/session/end-turn/required-method parity or offline distinct/repeated stages lose asserted order. | Claim-local `oracles/e7.py` independently composes ordered envelopes, including two labeled instances of the same transform type; committed captures supply vendor method expectations. | In `sdk_runtime.rs`, add `if stages.is_empty() { client.connect_to(terminal) }` or reverse the stage vector; `c2_zero_proxy_and_ordered_chain_match_direct_contract` must report the factory/order divergence. | Required migration integration test `c2_zero_proxy_and_ordered_chain_match_direct_contract`; retained offline/live parity probes. | 2–3 minutes authenticated | PASS |
| C3 | Official roles are the only runtime interface. | I2, I14; placement. | Run claim-local `e6` and compile the SDK component/conductor probe with workspace clippy; falsified if private structs cannot implement `ConnectTo<R>` without a Cyril-owned runtime trait or bypass. | Claim-local `oracles/e6.py`, SDK source/API audit of `ConnectTo`/`ConductorImpl`, and workspace manifest census. | Add unused `pub(crate) trait AgentEndpoint` in `protocol/sdk_runtime.rs`; `cargo clippy -- -D warnings` must fail on dead code, while the architecture census rejects a second runtime interface. | Required architecture test `c3_sdk_topology_uses_official_roles_only` plus clippy `-D warnings`. | <2 seconds cached | PASS |
| C4 | Frame semantics and exact ingress evidence survive the SDK seam. | I3, I4, I13. | In the production migration, send a fixture containing single/batch/malformed/unknown frames and the exact `1e400` source line through the process adapter; falsified if semantic order/evidence changes or the capture bytes differ before SDK parsing. | Claim-local E2 proves SDK semantic inspection preserves frame order but not lexical bytes; `oracles/e2.py` independently supplies the exact raw fixture and semantic expectations. | Move the lexical capture call after `serde_json`/SDK frame construction or forward only the first batch member; `c4_process_ingress_captures_before_parse_and_preserves_frames` must emit `claim_ids: ["C4"]` and identify the changed byte/member. | Required migration integration test `c4_process_ingress_captures_before_parse_and_preserves_frames`; retained E2 remains the semantic/necessity probe. | <2 seconds cached | PENDING — owner `cyril-1gfe`; budgeted-plan slice `Transport ingress`; checkpointed-build output `.cyril-1gfe/checkpoints/C4.json` |
| C5 | Static ordering/forwarding are preserved and slow work stays outside handlers. | I4, I5, I13. | Run claim-local `e3` and `e6`; falsified if reversed/removed unknown-first handling is not detected, the E3 slow-handler negative control does not block the fast notification, cancellation misses the terminal component, IDs change externally, or dynamic lifetime fails. | Claim-local `oracles/e3.py` runs exact upstream ordering/dynamic/out-of-order tests; `oracles/e6.py` runs exact cancellation tests. | In `protocol/client.rs`, await a mediator reply/host operation before returning from an SDK notification handler; `c5_slow_handler_does_not_block_dispatch` must emit `claim_ids: ["C5"]` and report the delayed fast method. | Required tests `c5_unknown_update_fence_precedes_typed_handler`, `c5_forwarding_preserves_outer_identity_and_cancellation`, and `c5_slow_handler_does_not_block_dispatch`; retained E3/E6. | 5 seconds cached | PASS |
| C6 | Existing process behavior survives behind an SDK component. | I6, I13. | Run claim-local `e4`; falsified if cwd, bounded diagnostics, group cleanup, stall/cancel, EOF, or grace behavior differs from the retained contract. | Claim-local `oracles/e4.py` combines direct OS subprocess behavior with independent SDK/current-source audit. | Replace the private process component with `AcpAgent`, or omit `.current_dir(cwd)`; `c6_process_component_preserves_agent_process_contract` and existing transport tests must fail. | Existing transport tests plus required parity test `c6_process_component_preserves_agent_process_contract`. | 5–10 seconds | PASS |
| C7 | Vendor conversion and host effects remain terminal-owned. | I1, I4, I7. | Run claim-local direct/conductor live matrices plus callback matrix; falsified if any advertised family is unanswered, a proxy owns effects, or v2/KAS normalized ordering diverges. | KAS covenant and committed wire captures plus existing `client.rs`/`host_mediator.rs` tests. | Move `_kiro/auth/getAccessToken` or terminal execution into a proxy, or drop one callback handler; `c7_terminal_client_owns_all_callback_families` must report the wrong owner/missing method. | Existing mediator/callback/refusal/hook/HostMediator tests plus required `c7_terminal_client_owns_all_callback_families`. | 10–30 seconds plus authenticated Kiro | PASS |
| C8 | SDK-independent App interfaces and turn/error ordering remain unchanged. | I1, I8, I13, I14. | In the production migration, run the Cyril bridge harness through conductor topology and compare typed commands, notifications, completion, error, and disconnect order to the current harness. | Unchanged pre-migration bridge tests plus independent SessionController/UiState projections define expected domain events without SDK types. | In `protocol/bridge.rs`, emit `BridgeDisconnected` before terminal `TurnCompleted`, or add an SDK `RawJsonRpcMessage` field to `RoutedNotification`; `c8_app_contract_and_terminal_order_are_unchanged` must emit `claim_ids: ["C8"]` and name the changed event/type. | Required migration integration test `c8_app_contract_and_terminal_order_are_unchanged`; existing prompt-error, death/disconnect, and callback-concurrency baselines. | 20–60 seconds | PENDING — owner `cyril-1gfe`; budgeted-plan slice `App contract`; checkpointed-build output `.cyril-1gfe/checkpoints/C8.json` |
| C9 | Memory and normalized source semantics stay at current seams. | I8, I10, I13. | In the production migration, run the table-driven I10 matrix; falsified by changed original/prepared prompt placement, exactly-once count, UTF-8/tool budget, identity, disposition, or unavailable/starting/ready behavior. | Claim-local E9 plus `oracles/e9.py` run current core/App/runtime behavioral contracts for every I10 category. | Mutation suite: in `crates/cyril/src/app.rs`, inject prepared context on every prompt; in `source_observer.rs`, capture `prepared_blocks`, remove truncation, map `Interrupted` to `Completed`, or replace scoped identity with a constant. `c9_memory_and_source_matrix_survives_sdk_topology` must emit `claim_ids: ["C9"]` and the exact red I10 cell. | Required migration test `c9_memory_and_source_matrix_survives_sdk_topology`; retain the named source/App/runtime baseline tests. | 1–3 minutes | PENDING — owner `cyril-1gfe`; budgeted-plan slice `Memory and source contract`; checkpointed-build output `.cyril-1gfe/checkpoints/C9.json` |
| C10 | Production exposes no inline observer or tracing registration. | I11, I15; removed single-client invariant. | Run claim-local E8 characterization and assert the exact three-argument `SdkRuntime::start(AgentProcess, DomainChannels, StageChain)` API; falsified if production accepts observer work or treats synchronous inspection as isolated/replay-capable delivery. | `oracles/e8.py` independently streams around permission, models two inline observers, disconnect, and fresh-attachment replay, and audits the SDK's linear successor/unbounded channel. | Add an observer argument to `SdkRuntime::start` and invoke it from `bridge_with_inspection`; `c10_sdk_runtime_has_no_observer_parameter` must fail its function-signature assertion and emit `claim_ids: ["C10"]` plus the added parameter. | Required compile-time test `c10_sdk_runtime_has_no_observer_parameter`; `cyril-5g2o` owns broadcaster behavior/fences. | <1 second | PENDING — owner `cyril-1gfe`; budgeted-plan slice `Runtime API`; checkpointed-build output `.cyril-1gfe/checkpoints/C10.json` |
| C11 | Stable-v1 and core-only dependency direction are enforced. | I12, I14. | Run claim-local `e10` and `oracles/e10.py`; falsified if default builders select v2, fallback retries incorrectly, or the workspace manifest/source census finds an ACP importer outside core. | Exact upstream negotiation tests plus independently parsed member manifests and Rust imports. | Enable `unstable_protocol_v2` by default or add ACP to `cyril-ui`; `c11_sdk_runtime_defaults_to_v1` or `c11_only_core_imports_acp` must fail. | Required tests `c11_sdk_runtime_defaults_to_v1` and `c11_only_core_imports_acp`. | <5 seconds | PASS |
| C12 | Migration is a clean cutover with one SDK family and one runtime path. | I14; placement and clean-cutover invariant. | After the migration slice, run a claim-local dependency/topology census; falsified if ACP 0.10/schema 0.11, old `ClientSideConnection`, a direct bypass, alias, or shim remains. | Independent lockfile/manifest/source census compared with SDK 2.0.0's pinned package graph. | In root `Cargo.toml`, retain `agent-client-protocol = 0.10.2`, or in `protocol/bridge.rs` retain construction of old `ClientSideConnection`; `c12_sdk2_cutover_has_one_transport_family_and_runtime` must emit `claim_ids: ["C12"]` and the exact obsolete package/symbol path. | Required migration test `c12_sdk2_cutover_has_one_transport_family_and_runtime` plus workspace `cargo tree` gate. | <10 seconds after build | PENDING — owner `cyril-1gfe`; budgeted-plan slice `Clean cutover`; checkpointed-build output `.cyril-1gfe/checkpoints/C12.json` |
| C13 | The complete bridge command/routing contract survives migration. | I1, I5, I8, I9. | In the production migration, run a table-driven SDK/conductor harness over every `BridgeCommand`, engine-selection failure, routed ID/turn presence cell, extension-param shape, slow-work/saturation, and channel disconnect; falsified by any missing/duplicated/retyped output or hang. | The unchanged pre-migration bridge harness plus independent SessionController/UiState projections compute expected domain events without SDK types. | In `protocol/bridge.rs`, separately delete the `ListSettings` arm, coerce `session_id: None` to main, coerce non-object `ExtMethod.params` to `{}`, and replace bounded awaited send with lossy `try_send`; `c13_all_bridge_commands_and_routing_shapes_cross_sdk_topology` must emit `claim_ids: ["C13"]` and the exact red matrix cell for each mutation. | Required migration test `c13_all_bridge_commands_and_routing_shapes_cross_sdk_topology`. | 1–3 minutes | PENDING — owner `cyril-1gfe`; budgeted-plan slice `Command and routing matrix`; checkpointed-build output `.cyril-1gfe/checkpoints/C13.json` |
| C14 | Route deliverables are complete and non-speculative. | I16. | At the spike documentation/cleanup checkpoint, verify an approved ADR amends/supersedes ADR-0003, proposal dispositions and verified follow-ons are linked, reproducible probes remain, and no production migration/disposable prototype exists. | `route.md` T4 plus primary Rivets records and repository path census. | Omit the ADR supersession link or leave a disposable prototype/build artifact; `c14_route_deliverables_are_complete` must emit `claim_ids: ["C14"]` and the missing/stale path. | Required checkpoint `c14_route_deliverables_are_complete`. | <5 seconds | PASS — `.cyril-41bs/checkpoints/C14.json` records both named mutations red, restored fence green, verified follow-on owners, and a clean reproducible-only census. |

All retained probes/oracles emit structured `claim_ids`. Matrix fences C9/C13 must name the red cell; C4 must name the byte/member; C8 the event/type; C10 the signature parameter; C12 the package/symbol path; C14 the missing/stale path. C4/C8/C9/C10/C12/C13 remain pending at the exact owner, budgeted-plan slice, checkpoint, and output above; C14 is discharged by `.cyril-41bs/checkpoints/C14.json`.

## Non-goals and future work

### Permanent non-goals

- **No production cutover in `cyril-41bs`.** This issue selects and documents the architecture; it does not change the shipping bridge.
- **No draft ACP wire v2 in the SDK2 migration.** SDK package version and wire version remain separate; the selected contract is stable wire v1.
- **No App/UI/domain rewrite.** The empirical actor seam preserves current typed interfaces, so such a rewrite adds risk without leverage.
- **No `AgentProcess` replacement.** The selected migration retains its cwd, diagnostics, liveness, and cleanup contracts rather than partially recreating them around `AcpAgent`.
- **No empty stages crate, placeholder proxy, compatibility alias, or dual production path.** Conductor zero-proxy operation is the initial concrete topology.
- **No claim that conductor tracing/inspection provides normalized capture or multi-client fan-out.** Those are different interfaces with different ownership.
- **No new tracing or MCP behavior in the migration.** Existing behavior is preserved; topology capability alone does not justify an unrequested consumer.

### Intended future work

- `cyril-1gfe` (verified open epic, 2026-08-29 via `rivets show`) owns the production SDK2/conductor migration, clean removal of ACP 0.10, and ROADMAP/document updates.
- `cyril-5g2o` (verified open task, 2026-08-29 via `rivets show`) owns multi-client observer topology and the separate bounded broadcaster interface.
- `cyril-1ixa` (verified open task, 2026-08-29 via `rivets show`) records current unbounded notification pressure and the trigger for backpressure work.

### Tracker verification

The authoritative primary checkout is `/home/dwalleck/repos/cyril`; the feature-worktree tracker snapshot predates these records. On 2026-08-29, `rivets show` in the primary checkout returned:

- `cyril-1gfe` — open P2 epic, **“Adopt the official ACP Rust SDK and canonical component/proxy architecture”**; its goal and acceptance criteria own the SDK2/conductor production cutover and clean removal of ACP 0.10/schema 0.11.
- `cyril-5g2o` — open P3 task, **“Steal KAS multi-client observer pattern for cyril's multi-client-observers proxy-stage”**; its description owns the separate mux/broadcaster design.
- `cyril-1ixa` — open P4 task, **“acp rpc layer buffers notifications unboundedly under UI stall”**; its description owns the current unbounded-pressure trigger.

## Falsifier run log
- **C1 cheapest falsifier:** PASS on 2026-08-29. Claim-local output from `cargo run --quiet --bin e1` reported distinct protocol/domain threads, bounded capacity 1, `domain_state_crossed_send_boundary: false`, and no unsafe/shared domain lock. `python3 oracles/e1.py` emitted `claim_ids: ["C1"]`, independently rejected the `Rc` cross-thread mutation, and compiled/ran the bounded handoff.

## Approval

- **Status:** APPROVED.
- **Requester words:** “Approve conductor-first design”
- **Date:** 2026-08-30.
- **Risk acceptances:** None proposed.
