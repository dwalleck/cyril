# Plan: current ACP runtime contract oracle

## Inputs and partition arithmetic

Approved design: [`design.md`](design.md), requester approval “Approve design” on 2026-08-30, no risk acceptances.

Discovered upstream branch: `origin/main` (from the feature worktree's tracked upstream).

| Slice | Diff estimate |
|---|---:|
| 1. Process and source contract | 700 |
| 2. Bridge command and pressure contract | 950 |
| 3. App, memory, and shutdown contract | 800 |
| 4. Scope and mutation oracle | 300 |
| **Estimated changed lines** | **2,750** |
| **25% churn margin** | **688** |
| **Projected total** | **3,438** |

The 25% margin covers fixture-shape expansion and test-harness recording fields discovered while wiring all 20 command variants. The projected total is at or below the 4,000-line review-size gate, so the plan has one PR increment.

### PR increment: Current-runtime contract oracle

Slices 1–4 form one independently mergeable test-only increment. It adds only child test modules and standalone oracle artifacts, leaves all production interfaces/manifests/behavior unchanged, and verifies against the current direct ACP runtime without SDK 2. Each slice is independently green; the final increment proves the no-production-change boundary and every named mutation before merge. No later increment is required for this issue.

## Slice 1: Pin process ingress and source behavior outside oversized modules

**Claim IDs:** C2, C3, C8, C9, C13

**Expected behavior:** A real `AgentProcess` exposes the exact pre-parser stdout bytes; process configuration/lifecycle remains current; prompt/source events preserve original blocks, identities, dispositions, and every byte/count budget; source and permission pressure remains bounded and typed.

**Oracle:** Fixed fixture segments independently assemble the expected raw byte vector and digest; OS exit status/tempfile/portable Unix `ps` establish lifecycle; independent byte/character counters and an expected event ledger reconstruct source output; monotonic indexed events establish pressure counts.

**Stress fixture:** One raw stream combines standard and extension frames, batch members, malformed evidence, integer/string IDs, duplicate members, and the exact `1e400` lexeme. Process paths contain Unicode and spaces, stderr exceeds both line and ring capacities, and a child owns a grandchild. Source cases hit every exact boundary and +1, split multi-byte UTF-8 at the boundary, reuse numeric turn IDs, change session/project identity, and fill source/permission channels before draining. Expected: exact byte equality, no split code point, correct truncation metadata/disposition, no scope leak, and bounded completion.

**Regression fence:**
- `protocol::transport::tests::current_runtime_contract::{c2_exact_preparse_bytes,c13_process_contract_matrix}`
- existing `protocol::transport::tests::stderr_tail_keeps_only_last_n_lines` for C3
- `protocol::source_observer::tests::current_runtime_contract::{c8_source_contract_matrix,c9_bounded_pressure_contract}`
- `crates/cyril-memory/tests/current_runtime_contract.rs` for persisted identity/budget/disposition round trips

**Named mutation:**
- C2: change `1e400` to `1e40`; C2 reports the exact byte index.
- C3: change `STDERR_TAIL_CAPACITY` from 50 to 49; the retained count/first value fails.
- C8: change `TOOL_INPUT_BYTES` by one; the exact-boundary or +1 matrix cell fails by name.
- C9: map source overflow to `Failed`; `source.full.disposition` fails by name.
- C13: omit `.current_dir(cwd)`; `cwd_unicode_spaces` fails by name.

**Complexity/production scale:** Test-only loops: raw compare $O(B)$ with $B \le 64$ KiB; event reconciliation $O(E)$ with $E \le 32$ core events and memory batches capped at 16/256 KiB; tool matrix $O(T)$ with $T \le 129$ to exercise the 128/+1 boundary. No production loop changes. Maximum accepted focused-suite wall cost: 30 seconds on CI, set above the existing five-second subprocess deadlines plus compile-free execution overhead.

**Wall budget/phase:** N/A — one-off test phases; no production phase or wall budget.

**Files:**
- Modify `crates/cyril-core/src/protocol/transport.rs` only to declare the child test module.
- Create `crates/cyril-core/src/protocol/transport/tests/current_runtime_contract.rs`.
- Modify `crates/cyril-core/src/protocol/source_observer.rs` only to declare the child test module.
- Create `crates/cyril-core/src/protocol/source_observer/tests/current_runtime_contract.rs`.
- Create `crates/cyril-memory/tests/current_runtime_contract.rs`.
- Create the Slice 1 checkpoint under `.cyril-g0dg/checkpoints/` during checkpointed-build.

**Estimate:** 3–5 focused implementation hours.

**Diff estimate:** 700 changed lines.

**PR increment:** Current-runtime contract oracle.

**Commands and expected results:**
- `cargo test -p cyril-core protocol::transport::tests::current_runtime_contract -- --nocapture` → exact raw bytes and every process matrix cell agree with independent expectations.
- `cargo test -p cyril-core protocol::source_observer::tests::current_runtime_contract -- --nocapture` → each identity, disposition, UTF-8, tool, event, byte, and pressure row agrees item-by-item.
- `cargo test -p cyril-memory --test current_runtime_contract -- --nocapture` → persisted/reloaded source records preserve scope and bounds; recall eligibility matches terminal disposition.
- Apply each Slice 1 named mutation and rerun only its focused fence → exact C2/C3/C8/C9/C13 expected-red text; restore and rerun → green.
- `cargo fmt --all -- --check && cargo test --all-targets && cargo clippy --all-targets -- -D warnings` → workspace remains formatted, behaviorally green, and warning-free after the slice.

## Slice 2: Freeze every bridge command, route, saturation, and disconnect path

**Claim IDs:** C5, C6

**Expected behavior:** Every one of the 20 `BridgeCommand` variants and each nested operation crosses the current direct `run_loop` with exact wire method/params and exact typed output/error; session/turn identities, capacity edges, agent death, and explicit shutdown preserve accepted items exactly once and in source order.

**Oracle:** A hand-authored expectation table keyed by case name computes expected method, params, notification sequence, scope, and turn identity independently. The FakeAgent records observations only; it does not generate expected values. Distinct monotonic IDs independently reconcile saturation order/count.

**Stress fixture:** Cases include no active session versus active session; empty/ASCII/Unicode/space-bearing values; relative/absolute and duplicate workspace paths; extension params object/array/null/scalar/additive keys; every `WorkflowOp`; hooks true/false; success/malformed/error replies; steering supported/unsupported/already marked; command channel 31/32/closed and notification channel 255/256/drain/wedged/dropped. Expected: no silent arm, exact payloads, current asymmetric error mapping, one terminal/disconnect sequence, and bounded failure.

**Regression fence:**
- `protocol::bridge::tests::current_runtime_contract::commands::c5_bridge_command_contract_matrix` under default and `kas` features.
- `protocol::bridge::tests::current_runtime_contract::routing::c6_routed_identity_contract`.
- `protocol::bridge::tests::current_runtime_contract::saturation::c6_pressure_disconnect_contract`.

**Named mutation:**
- C5: remove `partial: ""` from command-options params; `query_options.params.partial` fails.
- C6: replace the fail-stop bounded send with `try_send`; `notification.full.disconnect` fails.

**Complexity/production scale:** Test-only table traversal $O(C + W + N)$ where $C=20$ command variants, $W$ is the finite `WorkflowOp` set, and $N \le 257$ notification entries. No production loop changes. Maximum accepted focused-suite wall cost: 45 seconds default and 60 seconds with `kas`, allowing bounded five-second death-path controls while rejecting hangs.

**Wall budget/phase:** N/A — one-off test phases; no production phase or wall budget.

**Files:**
- Modify `crates/cyril-core/src/protocol/bridge.rs` only for a child-module declaration and minimal `cfg(test)` recording fields/helpers reused by the existing FakeAgent.
- Create `crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/mod.rs`.
- Create `crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/commands.rs`.
- Create `crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/routing.rs`.
- Create `crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/saturation.rs`.
- Create the Slice 2 checkpoint under `.cyril-g0dg/checkpoints/` during checkpointed-build.

**Estimate:** 5–8 focused implementation hours.

**Diff estimate:** 950 changed lines.

**PR increment:** Current-runtime contract oracle.

**Commands and expected results:**
- `cargo test -p cyril-core protocol::bridge::tests::current_runtime_contract -- --nocapture` → all default-runtime command, identity, saturation, disconnect, and shutdown rows match exact expectations.
- `cargo test -p cyril-core --features kas protocol::bridge::tests::current_runtime_contract -- --nocapture` → KAS hooks/workflow rows and shared rows match the same typed contract.
- Apply C5 and C6 mutations singly and rerun their focused fences → exact case names red; restore and rerun → green.
- `cargo fmt --all -- --check && cargo test --all-targets && cargo clippy --all-targets -- -D warnings` → workspace remains formatted, behaviorally green, and warning-free after the slice.

## Slice 3: Freeze App ordering, prompt memory, and bounded shutdown

**Claim IDs:** C4, C7, C10

**Expected behavior:** Typed notifications reach only the current App owner in current order; every first/subsequent-prompt and memory-state row preserves original/wire separation and exactly-once preparation/retry; shutdown remains bridge completion → capture drain → memory stop, bounded and idempotent.

**Oracle:** Declarative expected event traces and prompt matrix rows are separate from App/state implementations. Test-controlled oneshots and a monotonic clock independently establish shutdown order and bounds.

**Stress fixture:** Main, known subagent, unknown, workflow-owned, and global routes receive agent/error/completion/disconnect sequences with absent/zero/nonzero turn identity. Prompt rows cover first/subsequent, one/multiple duplicate/distinct Unicode blocks, context absent/present, no-match/lesson/episode, and disabled/unbound/unavailable/starting/ready/degraded/failed memory. Shutdown delays each collaborator below and beyond its bound and invokes shutdown twice. Expected: first changed trace cell is named, context appears once on wire and never in source/UI, only Starting re-arms, and shutdown order never inverts.

**Regression fence:**
- `App::tests::current_runtime_contract::ordering::c4_app_event_order_contract`.
- `App::tests::current_runtime_contract::memory::c7_prompt_memory_contract_matrix`.
- `App::tests::current_runtime_contract::shutdown::c10_shutdown_contract`.

**Named mutation:**
- C4: swap SessionController/UiState application; the first changed event fails by name.
- C7: prepare an already prepared result again; `ready.context_present.exactly_once` fails.
- C10: stop memory before capture drain; `C10 order: memory_stop before capture_drain` fails.

**Complexity/production scale:** Test-only traversal $O(R + M + S)$ over a fixed routing table, at most 30 prompt rows, and a fixed shutdown permutation set. No production loop changes. Maximum accepted focused-suite wall cost: 45 seconds, dominated by explicit two-second timeout cells.

**Wall budget/phase:** N/A — one-off test phases; no production phase or wall budget.

**Files:**
- Modify `crates/cyril/src/app.rs` only for a child-module declaration and minimal `cfg(test)` milestone trace if state snapshots cannot prove order.
- Create `crates/cyril/src/app/tests/current_runtime_contract/mod.rs`.
- Create `crates/cyril/src/app/tests/current_runtime_contract/ordering.rs`.
- Create `crates/cyril/src/app/tests/current_runtime_contract/memory.rs`.
- Create `crates/cyril/src/app/tests/current_runtime_contract/shutdown.rs`.
- Create the Slice 3 checkpoint under `.cyril-g0dg/checkpoints/` during checkpointed-build.

**Estimate:** 4–6 focused implementation hours.

**Diff estimate:** 800 changed lines.

**PR increment:** Current-runtime contract oracle.

**Commands and expected results:**
- `cargo test -p cyril current_runtime_contract -- --nocapture` → every route/order, prompt/memory, and shutdown row agrees item-by-item with its independent table/trace.
- Apply C4, C7, and C10 mutations singly and rerun focused fences → exact event/cell/order text red; restore and rerun → green.
- `cargo fmt --all -- --check && cargo test --all-targets && cargo clippy --all-targets -- -D warnings` → workspace remains formatted, behaviorally green, and warning-free after the slice.

## Slice 4: Enforce test-only scope and mutation localization

**Claim IDs:** C1, C11, C12

**Expected behavior:** The final tree keeps contract bodies in dedicated child test modules, contains no SDK 2/runtime/conductor/production-capture change, and records a mechanically complete expected-red mutation manifest whose entries localize every C1–C11/C13 failure.

**Oracle:** `scope.py` combines Cargo metadata with an explicit allowed-path/forbidden-marker census; `mutations.py` validates claim coverage, unique fence ownership, exact expected-red text, and restoration records independently from the Rust tests.

**Stress fixture:** Add one forbidden SDK 2 manifest entry, expose one test module outside `cfg(test)`, and replace one exact expected-red string with generic text, each singly. Expected: C11 names exact manifest/version, C1 names exact declaration/path, C12 rejects the non-local manifest entry; restoration returns both oracles green.

**Regression fence:**
- `.cyril-g0dg/oracles/scope.py --claim C1`.
- `.cyril-g0dg/oracles/scope.py --claim C11`.
- `.cyril-g0dg/oracles/mutations.py --verify-manifest`.

**Named mutation:**
- C1: remove `#[cfg(test)]` from one child module declaration.
- C11: add `agent-client-protocol = "2.0.0"` to a production manifest.
- C12: replace C5's exact expected-red manifest text with a generic substring.

**Complexity/production scale:** One-off file/metadata scans $O(F + K)$ over fewer than 500 tracked source/manifest paths and 13 claim entries. Maximum accepted cost: two seconds per Python oracle on the repository tree, matching the small bounded census and preventing accidental dependency on builds or network access.

**Wall budget/phase:** N/A — one-off local/CI oracle commands; no production phase or wall budget.

**Files:**
- Create `.cyril-g0dg/oracles/scope.py`.
- Create `.cyril-g0dg/oracles/mutations.py`.
- Create `.cyril-g0dg/oracles/mutations.json`.
- Create the Slice 4/final checkpoint under `.cyril-g0dg/checkpoints/` during checkpointed-build.

**Estimate:** 2–3 focused implementation hours.

**Diff estimate:** 300 changed lines.

**PR increment:** Current-runtime contract oracle.

**Commands and expected results:**
- `python3 .cyril-g0dg/oracles/scope.py --claim C1 && python3 .cyril-g0dg/oracles/scope.py --claim C11` → child modules are test-only; no forbidden dependency/marker/path appears.
- `python3 .cyril-g0dg/oracles/mutations.py --verify-manifest` → every C1–C11/C13 claim has one unique fence, mutation, and exact expected-red result plus a restored-green record.
- Apply C1/C11/C12 mutations singly → exact claim-local oracle failure; restore → green.
- `cargo fmt --all -- --check` → all Rust additions are formatted.
- `cargo test --all-targets` → complete default-feature workspace contract passes.
- `cargo test -p cyril-core --all-targets --features kas` → KAS-only command contract passes.
- `cargo clippy --all-targets -- -D warnings` → default workspace is warning-free.
- `cargo clippy -p cyril-core --all-targets --features kas -- -D warnings` → KAS contract path is warning-free.

## Tracker taxonomy

- Permanent non-goal: SDK 2 and the conductor-first production cutover are excluded because this issue must freeze the old runtime before it changes; verified `cyril-gl5s` owns the intended cutover.
- Permanent non-goal: a production raw-capture/observer interface and multi-client broadcaster are excluded because private test adapters suffice; verified `cyril-5g2o` owns intended multi-client observation.
- Permanent non-goal: broad refactors of existing oversized modules are excluded because unrelated movement would obscure the baseline. New additions are isolated in child test modules instead.
- Intended future work: verified `cyril-gl5s` consumes this oracle for the atomic SDK 2 migration.
- Intended future work: verified `cyril-1ixa` owns the current unbounded RPC buffering trigger; this plan only freezes existing bounded seams.

## Self-review

1. **Claim assignment:** PASS. C1–C13 are assigned exactly once; C3's pre-design PASS is retained and rerun in Slice 1; every PENDING falsifier is discharged by its owning slice.
2. **Mandatory fields:** PASS. Every slice records all thirteen fields; conditional fields use `N/A — reason`.
3. **Fence ownership and mutations:** PASS. Each implementing slice creates its fences and applies the approved named mutations; no fence-less risk exists.
4. **Complexity and wall budgets:** PASS. Every new loop is test-only with explicit asymptotic/input/cost bounds; all phases are one-off, so no production wall budget applies.
5. **Partition:** PASS. 2,750 estimated lines + 688 (25%) churn = 3,438, under 4,000; one independently mergeable PR increment.
6. **Tracker taxonomy:** PASS. All future work cites verified `cyril-gl5s`, `cyril-5g2o`, or `cyril-1ixa`; permanent non-goals carry rationale.
7. **Completion ownership:** PASS. No slice is declared complete; checkpointed-build exclusively judges completion.
