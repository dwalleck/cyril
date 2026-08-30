# Design: current ACP runtime contract oracle

## Route and inputs

- Route: **Structural**, from [`route.md`](route.md). The implementation is test-only, but deliberate child-test-module placement is required to avoid growing `transport.rs`, `bridge.rs`, `source_observer.rs`, and `app.rs`.
- Behavior source: `route.md` T4. It defines five complete given/when/then contracts: pre-parse process bytes/lifecycle; typed App ordering; memory/source I10; every command/routing/saturation/disconnect path; and mutation-localized/no-production-change verification.
- `spec.md`: N/A — `route.md` records fully explicit behavior.
- `evidence.md`, `probe.*`: N/A — no unverified external premise. Current source and current tests are the behavior authority being frozen.
- Existing implementation seams: `AgentProcess::spawn`; `bridge::run_loop` and its in-process `FakeAgent`; `PromptEnvelope`; `SourceObserver`; `App::send_prompt`, `App::handle_notification`, and `App::shutdown_memory_runtime`.
- Empirical premises: N/A — this change characterizes repository behavior; it does not predict SDK 2 or a live agent.

## Input shapes

| Family | Production-reachable shapes | Status |
|---|---|---|
| ACP process source | Exact bytes for single request/notification/response; batch with one/many and duplicate/distinct members; malformed line/member; standard/extension/unknown methods; integer/string IDs; extreme numeric lexeme; LF termination and EOF | Covered by C2 |
| Process configuration | Empty/multiple args; empty/multiple environment entries; relative/absolute cwd; ASCII, Unicode, and embedded spaces | Covered by C13 |
| Process lifecycle | Clean/nonzero exit; stderr empty, line-delimited burst, newline-free burst, and non-UTF-8; retained-tail exact capacity; idle/stalled process; cancellation/drop; stdout EOF; child plus grandchild; bounded graceful shutdown | Covered by C3 and C13 |
| `BridgeCommand` | All 20 variants: prompt, new/load/cancel/mode/model, generic extension, settings/account/options/execute, spawn/terminate/message, steer/clear, KAS hooks list/toggle, workflow, shutdown | Covered by C5 |
| Nested command sums/options | Every `WorkflowOp`; hooks enabled true/false; active session absent/present; successful/malformed/error replies; steering supported/unsupported/already marked | Covered by C5 |
| Command strings/paths/collections | Empty, ASCII, Unicode, embedded spaces; relative/absolute paths; empty/single/multi collections with distinct and duplicate values | Covered by C5 and C13. Invalid values already refused by existing typed constructors are N/A — the oracle consumes only constructible domain values. |
| Extension params | JSON object/array/null/scalar plus additive unknown object keys; exact method spelling without accidental underscore rewriting | Covered by C5 |
| Routed identity | Session scope absent/main/known subagent/unknown/workflow-owned; turn identity absent/present/zero/nonzero; reused and changed session IDs | Covered by C4 and C6 |
| Ordering/lifecycle | Agent event; bridge error; completed/interrupted/failed terminal; idle disconnect; busy error→completion→disconnect; explicit shutdown with no disconnect; KAS duplicate terminal | Covered by C4 and C6 |
| Channel pressure | Command occupancy 0/31/32 plus closed receiver; notifications 0/255/256 then drain plus wedged/dropped receiver; permission 0/15/16; source 0/31/32 and capture overflow | Covered by C6 and C9. Negative occupancy is N/A — capacities and indices are unsigned. |
| Prompt envelope | First/subsequent prompt; one/multiple blocks with distinct/duplicate Unicode text; prepared context absent/present; memory disabled/unbound/unavailable/starting/ready/degraded/failed; no-match and lesson/episode result | Covered by C7. Empty UI prompt is N/A — `submit_input` refuses it before this seam; a directly constructed empty `BridgeCommand` is covered by C5 as current bridge behavior. |
| Source capture | Original versus prepared wire prompt; UTF-8 split boundary and +1; tool id/name/status/input/result exact boundary and +1; event count 0/1/16/+1 and bytes exact 256 KiB/+1; zero/one/128/+1 tools and 256 KiB aggregate; completed/interrupted/failed/abandoned/capture-overflow | Covered by C7 and C8 |
| Identity/scoping | Source ID new per turn; bridge turn ID zero/reused/changed; session same/changed; project same/foreign; sequence contiguous/duplicate/gap | Covered by C8 |
| Shutdown | Bridge completion before capture drain before memory stop; each completes or reaches its current two-second bound; repeated shutdown | Covered by C10 |
| Repository scope | Test modules and oracle artifacts added; manifests and production behavior unchanged; no SDK 2, runtime trait, dormant conductor path, or production capture/observer interface | Covered by C1 and C11 |

## Removed-invariant sweep

This change is purely additive: it adds characterization fixtures and test-only adapters. It removes no serialization point, guard, validation, uniqueness rule, ordering guarantee, or production precondition.

## Placement

### Scope and module-size fence

- **Owner:** `.cyril-g0dg/oracles/scope.py` owns the deterministic repository-scope census because no runtime module should know about the migration ticket or inspect repository source.
- **New seam:** a test-only command-line oracle; no production interface. It reports exact forbidden path/package/marker findings.
- **Forbidden:** adding the census to a Rust production module, reading source text from a behavioral Rust test, or permitting SDK 2/runtime/conductor/capture production code.

### Process ingress and lifecycle

- **Owner:** `protocol::transport::tests::current_runtime_contract`, loaded from `crates/cyril-core/src/protocol/transport/tests/current_runtime_contract.rs`. `AgentProcess` already owns cwd, process pipes, stderr retention, and process-tree lifetime.
- **New seam:** no production seam. A private `RecordingReader` test adapter wraps `AgentProcess.stdout` and records only bytes accepted by `poll_read` before forwarding them into the current ACP reader. The raw expected byte vector is computed independently from fixture segments.
- **Forbidden:** a production capture hook, observer parameter, proxy process, duplicated production process implementation, SDK dependency, or integration test that widens `AgentProcess` visibility.

### Direct bridge command, identity, and pressure contracts

- **Owner:** `protocol::bridge::tests::current_runtime_contract`, split into `commands.rs`, `routing.rs`, and `saturation.rs` beneath `crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/`. The existing in-process duplex `FakeAgent` remains the single direct-runtime adapter.
- **New seam:** no production seam. The child module uses parent-private harness state. Small test-only recording fields may be added to `Script`; exact expectations live in independent table rows, not in the fake's mapping.
- **Forbidden:** a second bridge implementation, public test support, production capture/observer interface, SDK 2 types, or adding the contract body to the already oversized `bridge.rs`.

### Prompt/source contract

- **Owner:** `protocol::source_observer::tests::current_runtime_contract` owns event/budget/identity fixtures; `App::tests::current_runtime_contract::memory` owns first-prompt preparation because those modules already own the respective behavior.
- **New seam:** no production seam. Tests use `PromptEnvelope`, `SourceObserver`, the existing in-process memory runtime, and bridge command receiver.
- **Forbidden:** moving memory into core, exposing prepared context to source capture/UI, duplicating budget validation, or appending these matrices to `source_observer.rs`/`app.rs`.

### App event and shutdown contract

- **Owner:** `App::tests::current_runtime_contract`, split into `ordering.rs`, `memory.rs`, and `shutdown.rs` beneath `crates/cyril/src/app/tests/current_runtime_contract/`.
- **New seam:** no production seam. The child module drives private typed handlers and existing test bridge/memory adapters. A test-only trace may record handler milestones; it must not compile outside `cfg(test)`.
- **Forbidden:** a production event observer, direct ACP/JSON parsing in `cyril`, changing SessionController-before-UiState application, or adding the contract body to `app.rs`.

The dedicated child modules are deep test modules: each exposes no production interface and concentrates one contract family behind existing module interfaces. Deleting them removes only oracle coverage, not a pass-through layer that callers must learn.

## Claims

- **C1.** All new Rust oracle code compiles only under `cfg(test)` in dedicated child modules, and no oversized production file receives contract-test bodies.
- **C2.** A test adapter at `AgentProcess.stdout` observes the exact bytes the current ACP parser receives, including lexical forms that semantic JSON parsing normalizes or rejects.
- **C3.** The current stderr tail retains exactly the last 50 entries in arrival order, including exact oldest/newest values after overflow.
- **C4.** Typed main, subagent, workflow, unknown, global, completion, error, and disconnect frames reach only their current App owners and preserve current source order.
- **C5.** Every constructible `BridgeCommand` and nested operation produces its current exact wire method/params and current typed success/error output, with no silent command path.
- **C6.** Turn/session identity, bounded saturation, disconnect, and explicit shutdown preserve every accepted item exactly once and in order; full or closed paths return their current typed failure instead of dropping data silently.
- **C7.** The complete first/subsequent-prompt and memory-state matrix preserves original blocks, prepends prepared context only on wire and at most once, and retains current retry behavior.
- **C8.** Source capture preserves source/session/project identity and terminal disposition while enforcing every UTF-8, event, byte, tool-count, field, and aggregate budget boundary.
- **C9.** Source and permission pressure paths remain bounded and report overflow or backpressure through current typed contracts.
- **C10.** App shutdown remains ordered bridge completion → capture drain → memory stop, bounded at each current deadline, and idempotent.
- **C11.** The final repository contains no SDK 2 dependency, new runtime trait, dormant conductor path, production capture/observer interface, or production behavior change.
- **C12.** Every claim-local named mutation turns exactly its assigned fence red with the claim ID and changed byte, cell, event, or route in the failure, then returns green after restoration.
- **C13.** The process fixture pins current cwd, argument/environment, EOF, exit, stall/cancel, process-group, and bounded-shutdown behavior without changing `AgentProcess`.

## Falsification

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C1 | Test-only modular placement | Repository scope; child modules | Build library/non-test targets and inspect changed Rust line locations; falsified if contract helpers compile into the library or any new test body lands in the four oversized parent files. | `scope.py` computes changed paths/line classes independently from Rust compilation. | Remove `#[cfg(test)]` from one child-module declaration; non-test build must fail on test-only dependencies and C1 must name the declaration. | `scope.py --claim C1` plus `cargo check --all-targets` | <1 min | PENDING — checkpointed-build, scope slice |
| C2 | Exact pre-parse bytes | All ACP source lexical shapes | Emit the segmented fixture through a real `AgentProcess`, record stdout bytes before ACP parsing, and compare byte-for-byte; falsified by any differing index/length. | An independently assembled byte vector and SHA-256 digest, not serde or the recording adapter. | Change `1e400` to `1e40` in one fixture segment; `c2_exact_preparse_bytes` must fail with C2 and the exact byte index. | `protocol::transport::tests::current_runtime_contract::c2_exact_preparse_bytes` | <10 s | PENDING — checkpointed-build, process slice |
| C3 | Exact retained stderr tail | More than 50 line-delimited stderr entries | Push 55 distinct entries and compare retained count, first, and last values; falsified by any capacity or ordering mismatch. | An independently generated `line 0`…`line 54` sequence and fixed expected retained range 5…54. | Change `STDERR_TAIL_CAPACITY` from 50 to 49; `stderr_tail_keeps_only_last_n_lines` must fail on count/first value. | `protocol::transport::tests::stderr_tail_keeps_only_last_n_lines` | <5 s | PASS — 2026-08-30 focused falsifier |
| C13 | Retained process behavior | Process config/lifecycle matrix excluding the separately pinned stderr ring | Drive real portable shell fixtures with bounded deadlines; falsified by any matrix cell differing from expected cwd/status/liveness/deadline. | OS exit status, tempfile path contents, and portable Unix `ps` are independent of `AgentProcess` implementation. | Omit `.current_dir(cwd)`; `c13_process_contract_matrix` must fail at `cwd_unicode_spaces`. | `protocol::transport::tests::current_runtime_contract::c13_process_contract_matrix` plus existing transport lifecycle tests | <15 s | PENDING — checkpointed-build, process slice |
| C4 | App ownership and order | Routed identities and lifecycle sequences | Feed typed sequences and compare owner/state trace after every frame; falsified by a missing, duplicate, retyped, or reordered milestone. | A declarative expected trace table separate from `handle_notification` and state implementations. | Swap SessionController/UiState application in `App::handle_notification`; `c4_app_event_order_contract` must fail at the first changed event. | `App::tests::current_runtime_contract::ordering::c4_app_event_order_contract` | <5 s | PENDING — checkpointed-build, App slice |
| C5 | Exhaustive command contract | All 20 commands, nested sums/options, params/replies | Send each case through `run_loop`/FakeAgent and compare recorded method/params and typed outputs; falsified by any absent/extra/retyped result or payload difference. | A hand-authored expectation table keyed by command case; the fake records observations but does not compute expectations. | Remove `partial: ""` from options params; `c5_bridge_command_contract_matrix` must fail at `query_options.params.partial`. | `protocol::bridge::tests::current_runtime_contract::commands::c5_bridge_command_contract_matrix` under default and `--features kas` | <20 s | PENDING — checkpointed-build, bridge slice |
| C6 | Identity, saturation, disconnect | Routed/turn shapes; capacities and closed peers | Fill each bounded channel with distinct IDs, exercise the next send/close/death/shutdown, drain, and reconcile order/count/typed errors; falsified by loss, duplication, reordered identity, unbounded wait, or wrong terminal order. | Monotonic IDs and an independent expected sequence/count computed before sends. | Replace fail-stop bounded `send` with `try_send`; `c6_pressure_disconnect_contract` must fail at `notification.full.disconnect`. | `protocol::bridge::tests::current_runtime_contract::saturation::c6_pressure_disconnect_contract` | <10 s | PENDING — checkpointed-build, bridge slice |
| C7 | Prompt/memory matrix | First/subsequent, blocks, context, memory states | Run every matrix row through App prompt dispatch and compare original blocks, wire blocks, pending/retry state, and command count; falsified by any cell mismatch or double injection. | Static expected rows built from original inputs and explicit context, independent from `PromptEnvelope::into_wire_blocks`. | Call `PromptEnvelope::prepared` again for an already prepared result; `c7_prompt_memory_contract_matrix` must fail at `ready.context_present.exactly_once`. | `App::tests::current_runtime_contract::memory::c7_prompt_memory_contract_matrix` | <30 s | PENDING — checkpointed-build, App slice |
| C8 | Source budgets/identity/dispositions | UTF-8/tool/event/identity matrix | Feed boundary and +1 cases, persist/reload, reconstruct fragments, and compare identity/status/truncation metadata; falsified by a split code point, wrong limit, scope leak, or recall-ineligible terminal accepted. | Independent byte/character counters and expected event ledger, plus memory-store reload. | Change `TOOL_INPUT_BYTES` by one; `c8_source_contract_matrix` must fail at `tool_input.exact_boundary` or `tool_input.plus_one`. | `protocol::source_observer::tests::current_runtime_contract::c8_source_contract_matrix` and memory boundary tests | <20 s | PENDING — checkpointed-build, source slice |
| C9 | Bounded source/permission pressure | 32-source, 16-permission, capture bounds | Hold receivers, fill to capacity, release/drain, and compare typed overflow/backpressure outcomes; falsified by silent loss or an unbounded wait. | Distinct indexed events and independent capacity constants read from public typed limits where available or fixed contract expectations. | Change SourceObserver overflow disposition to `Failed`; `c9_bounded_pressure_contract` must fail at `source.full.disposition`. | source-observer and bridge saturation contract tests | <10 s | PENDING — checkpointed-build, source/bridge slices |
| C10 | Ordered bounded shutdown | Completion/drain/runtime permutations | Delay each collaborator below/above its bound and record milestones; falsified by reordered stop, leaked task, or exceeding the documented bound. | Test-controlled oneshots and monotonic clock deadlines independent from shutdown implementation. | Shut memory runtime before capture drain; `c10_shutdown_contract` must fail with `C10 order: memory_stop before capture_drain`. | `App::tests::current_runtime_contract::shutdown::c10_shutdown_contract` | <10 s | PENDING — checkpointed-build, App slice |
| C11 | No production/SDK change | Manifests and production paths | Run the scope census over the final tree; falsified by SDK 2, conductor/runtime/capture production markers, production file behavior changes, or forbidden dependencies. | Cargo metadata plus a path/marker allowlist in the standalone oracle. | Add `agent-client-protocol = "2.0.0"` to a production manifest; `scope.py --claim C11` must name the manifest and version. | `.cyril-g0dg/oracles/scope.py --claim C11` | <5 s | PENDING — checkpointed-build, scope slice |
| C12 | Mutation localization | Every C1–C11 and C13 mutation | Apply each named mutation singly and run only its fence; falsified if the fence stays green or reports only a generic suite failure. | The mutation patch and expected-red manifest are independent from each behavior test and record exact output before restoration. | Change C5's expected-red manifest entry from `query_options.params.partial` to a generic substring; `.cyril-g0dg/oracles/mutations.py --verify-manifest` must reject it as non-local. | `.cyril-g0dg/oracles/mutations.py --verify-manifest` plus per-slice mutation results recorded by checkpointed-build | Per mutation <1 min | PENDING — checkpointed-build, every slice |

## Non-goals and future work

- Permanent non-goal: no SDK 2 dependency or production cutover. This ticket freezes the old runtime so `cyril-gl5s` can later prove an atomic cutover equivalent.
- Permanent non-goal: no production raw-capture/observer interface or multi-client broadcaster. The oracle needs only private test adapters; production observation belongs to verified future issue `cyril-5g2o`.
- Permanent non-goal: no new Cyril runtime trait or dormant conductor path. The approved cutover owns the official `ConnectTo`/conductor topology in verified issue `cyril-gl5s`.
- Intended future work: `cyril-gl5s` performs the SDK 2 conductor-first clean cutover and consumes this oracle.
- Intended future work: `cyril-1ixa` owns the existing unbounded-pressure trigger; this ticket characterizes current bounded seams and does not redesign pressure policy.
- Existing oversized modules are not broadly refactored here. That is a permanent scope choice for this test-only ticket: additions are isolated now, while unrelated code movement would obscure the contract baseline it is meant to freeze.

Tracker IDs `cyril-gl5s`, `cyril-5g2o`, and `cyril-1ixa` were verified in the accepted `cyril-41bs` design/plan and remain current follow-on owners.

## Falsifier run log

- 2026-08-30 — `cargo test -p cyril-core protocol::transport::tests::stderr_tail_keeps_only_last_n_lines -- --exact` — **PASS**: 1 passed. Cheapest design falsifier confirms C3's current 50-entry ring and exact oldest/newest values.

The cheapest design falsifier is C3 and is PASS. C13 remains PENDING until the complete remaining process matrix and its named mutation pass.

## Approval

- Requester approval: “Approve design”
- Date: 2026-08-30
- Approved risk acceptances: None
