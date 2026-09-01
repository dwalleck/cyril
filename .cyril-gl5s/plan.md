# Budgeted plan: cyril-gl5s

## Inputs and partition decision

- **Route:** Structural — `.cyril-gl5s/route.md`.
- **Approved design:** `.cyril-gl5s/design.md`, revised and approved verbatim on 2026-08-30: “I approve the revised design”. No approved risk acceptances.
- **Behavior oracle:** merged `.cyril-g0dg/` current-runtime contract plus `.cyril-41bs/` independent SDK/conductor evidence.
- **Atomicity:** the SDK package/type change reaches every core ACP caller at once; a partial production migration cannot compile or preserve the Bridge contract. Slice 1 is the indivisible production seam: it activates the complete conductor path, migrates every caller, and proves C1–C11/C13/C14. Slice 2 immediately removes the approved named unused legacy dependency and proves C12/final shape. Both are independently green feature-integration review increments; final delivery squashes them into the one atomic main-history cutover commit required by `cyril-gl5s`.
- **Approved pre-deletion lifecycle:** Increment A has exactly one active SDK2/conductor runtime, zero legacy source imports/callers/adapters, and one explicitly named unused core-only `agent-client-protocol-legacy` dependency. Runtime-phase C14 enforces those bounds. Increment B removes it and its schema closure before final delivery.

## Module growth ledger

Counts are pre-change production lines as defined by `design.md`; test-only and documentation paths are budgeted separately. Ranges are drift tripwires, not permission for new responsibility clusters. A single projected value/range applies to both lifecycle phases; `runtime / final` values differ by phase.

| Module | Baseline production lines | Projected runtime / final lines | Responsibility change | Interface change | Protected-parent rule |
|---|---:|---:|---|---|---|
| `Cargo.toml` | 86 | 88 / 87 | replace ACP workspace version; add conductor and named checkpoint-only legacy dependency, then delete legacy | dependency values only | runtime phase permits only named unused core legacy; final one family; no unstable wire-v2 |
| `crates/cyril-core/Cargo.toml` | 67 | 69 / 68 | consume conductor/SDK2 and named checkpoint-only legacy dependency, then delete legacy | private dependencies only | workspace versions/features; final legacy removal |
| `protocol/mod.rs` | 35 | 37 / 37 | declare runtime and mediator | private declarations | declaration-only parent |
| `protocol/bridge.rs` | 2,973 | 300–500 / 300–500 | delete direct connection and serial command/domain bodies; retain public bridge orchestration | public Bridge interfaces unchanged | no SDK builder/handler/conductor body or `match BridgeCommand` |
| `protocol/client.rs` | 669 | 220–350 | replace ACP0.10 client object with ordered SDK2 handler runner | private handler-runner seam | registration/enqueue only; no domain/effect bodies |
| `protocol/transport.rs` | 240 | 240–290 | retain process ownership; provide concrete adapter inputs | AgentProcess interface unchanged or narrowed | no SDK handler/domain responsibility |
| `protocol/sdk_runtime/mod.rs` | 0 | 150–250 | add conductor/stage/lifecycle owner | exact three-argument private start + handle | no observer, runtime trait, direct bypass, Engine/effects |
| `protocol/sdk_runtime/process.rs` | 0 | 120–220 | add concrete official-role adapter, EOF control marker, and test recording point | official `ConnectTo<Client>` only | no spawn policy/domain/observer responsibility |
| `protocol/domain_mediator/mod.rs` | 0 | 350–650 | add serial select loop and bounded actor interface | constructor/channels/run only | no conductor/handler/App responsibility |
| `protocol/domain_mediator/commands/` | 0 | 1,100–1,700 total | move total BridgeCommand mapping by family | private family functions through mediator | no incoming/vendor/rendering responsibility |
| `protocol/domain_mediator/inbound.rs` | 0 | 120–300 | move notification/turn/tool/source application while reusing existing conversion owners | private DomainWork application | no SDK registration or host effects |
| `protocol/domain_mediator/host.rs` | 0 | 180–320 | add pre-initialize host request drain over existing lifecycle/effect adapters | private typed HostWork jobs | effect implementations remain KAS-owned |
| `protocol/engine.rs` | 332 | 320–390 | migrate schema types only | Engine semantic interface unchanged | no runtime topology |
| `protocol/fingerprint.rs` | 111 | 100–140 | migrate initialize schema types only | fingerprint output unchanged | no runtime topology |
| `protocol/convert/mod.rs` | 497 | 470–560 | migrate generic SDK2 v1 schema inputs | domain conversions unchanged | conversion remains core-only |
| `protocol/convert/kiro.rs` | 1,113 | 1,050–1,200 | migrate Kiro extension schema inputs | normalized output unchanged | vendor meaning stays here |
| `protocol/convert/kas.rs` | 581 | 540–650 | migrate KAS schema inputs | normalized output unchanged | vendor meaning stays here |
| `protocol/kas/mod.rs` | 42 | 40–50 | import/re-export migration only | unchanged | no runtime topology |
| `protocol/kas/auth.rs` | 327 | 310–370 | SDK2 callback type migration | auth effect unchanged | terminal-owned |
| `protocol/kas/callbacks.rs` | 469 | 440–540 | SDK2 request/work type migration | callback enum/dispatch unchanged | no proxy ownership |
| `protocol/kas/hooks.rs` | 711 | 670–790 | SDK2 hook type migration | direction/ownership unchanged | no mixed host/KAS ownership |
| `protocol/kas/host_io.rs` | 242 | 225–285 | SDK2 fs callback type migration | host effect unchanged | no runtime topology |
| `protocol/kas/kiro_fs.rs` | 663 | 625–740 | SDK2 Kiro fs type migration | dialect/range semantics unchanged | no runtime topology |
| `protocol/kas/terminal_io.rs` | 714 | 675–800 | SDK2 terminal type migration | lifecycle unchanged | no runtime topology |
| `crates/cyril/src/app.rs` | 2,723 | 2,723 | no production change | App/Bridge/memory/source unchanged | zero production delta; no SDK import |

Additional touched non-production paths (`Cargo.lock`, contract tests, issue-local oracles, ROADMAP/AGENTS dependency notes) are included in diff estimates but not production-line tripwires. New responsibility/interface/seam drift returns to `falsifiable-design`; numeric-only drift with unchanged ownership returns to this plan.

## Partition arithmetic

| Slice | Implementation | Tests/fixtures/oracles/docs | Projected changed lines |
|---|---:|---:|---:|
| Slice 1 | 5,800 | 4,300 | 10,100 |
| Slice 2 | 250 | 950 | 1,200 |
| **Sum** | **6,050** | **5,250** | **11,300** |

- **Churn margin:** 30% = 3,390 lines. SDK0.10→2 schema paths and handler idioms touch large callback/converter test modules; compiler-driven migration can expand fixture edits.
- **Budgeted total:** 14,690 changed lines.
- **Review-size result:** above 4,000; two independently green review increments are mandatory.

### Increment A — Active conductor runtime and complete pre-deletion contract

- **Slices:** Slice 1.
- **Mergeable definition:** one active SDK2/conductor runtime, every production caller migrated, all C1–C11/C13/C14 deterministic and authenticated checks green, no direct runtime/bypass/observer; only the approved named unused legacy dependency remains.
- **Verification without Increment B:** complete Bridge/App/memory/source behavior and v2/KAS live turns use the new runtime; runtime-phase shape census proves zero legacy source importers and all approved owners/protected parents. This checkpoint is review-mergeable into the feature integration branch but never delivered independently to main.

### Increment B — Legacy package deletion and final repository contract

- **Slices:** Slice 2.
- **Mergeable definition:** removes the named unused legacy dependency/schema and every obsolete package/symbol/path, reruns post-deletion acceptance, and synchronizes architecture documentation.
- **Verification without later increments:** no later increment exists; final dependency/topology/shape censuses, deterministic suites, and authenticated v2/KAS turns all pass.

## Slice 1: Activate the complete conductor-first SDK2 runtime and preserve C1–C11/C13/C14

**Claim IDs:** C1, C2, C3, C4, C5, C6, C7, C8, C9, C10, C11, C13, C14

**Expected behavior:** Every supported v2/KAS process crosses retained AgentProcess pipes, official zero-or-more-stage conductor, ordered enqueue-and-return SDK handlers, and bounded serial mediator to unchanged Bridge/App/memory/source contracts. Every command, callback, event-order, ingress, pressure, error, and shutdown cell matches the current-runtime oracle. The sole non-final artifact is the approved named unused legacy dependency; no production source imports or executes it.

**Oracle:** `.cyril-41bs` E1–E10 actor/JSON/OS/SDK/conductor/live oracles plus `.cyril-g0dg` hand-authored process, command, routing, App, memory, source, pressure, and shutdown expectations. New harnesses record observations but never compute expected rows.

**Stress fixture:** Deterministic in-process agent and real helper-process matrix: exact segmented single/batch/malformed/unknown/extension/`1e400`; Unicode/spaced cwd; >50 stderr entries and newline-free burst; idle/mid-turn EOF/crash; capacity, capacity+1, closed-peer, delayed-drain cases for command32/notification256/permission16/source32/mediator queues; unknown-first/reversed/removed handlers; slow domain with fast notification; every BridgeCommand/nested option; absent/main/subagent/unknown session and absent/present turn; object/array/null ext params; all callbacks; full I10 prompt/memory/budget/disposition/identity matrix. Expected output is the exact independent byte/member/method/event/cell/disposition/typed result.

**Regression fence:**
- `protocol::sdk_runtime::tests::{zero_stage_runtime_still_has_a_conductor_stage_chain,ordered_stage_chain_preserves_runtime_frame_order,unknown_standard_notification_does_not_enter_domain_queue,c4_recording_reader_preserves_segmented_batch_malformed_and_numeric_bytes,c4_process_adapter_captures_invalid_frames_before_sdk_rejection,process_adapter_preserves_raw_ingress_and_clean_eof}`
- `protocol::bridge::tests::current_runtime_contract::{c1_c5_c6_sdk_handler_backpressure_preserves_every_frame,c5_every_bridge_command_has_an_explicit_current_runtime_outcome,c5_command_failures_preserve_legacy_operation_labels,c5_new_session_rpc_failure_is_fatal,c6_unknown_update_handler_precedes_typed_handler_without_poisoning_connection,c7_malformed_standard_request_is_rejected_before_extension_fallback,c7_unknown_standard_request_returns_method_not_found,c8_host_request_drain_is_live_during_initialize,c11_sdk_runtime_negotiates_stable_wire_v1,c13_extension_params_preserve_array_and_null_shapes,c13_pending_permission_response_does_not_block_shutdown}`
- `protocol::domain_mediator::tests::{domain_work_capacity_is_exact_and_lossless_until_full,host_work_capacity_is_exact_and_fifo,initialization_failure_drains_queued_callback_error,prompt_terminal_is_processed_after_queued_source_frames,kas_runtime_preserves_callbacks_commands_and_wire_terminal_order}` plus the retained transport/source/App current-runtime contracts
- `.cyril-gl5s/oracles/module_shape.py --phase runtime` for C14.

**Named mutation:** Apply each approved mutation singly: capture `Rc<dyn Engine>` in Send task (C1/E0277); direct-connect empty chain/reverse stages (C2); add `AgentEndpoint` (C3); capture after parse/drop batch member (C4); await mediator/host inline (C5); replace AgentProcess/omit cwd (C6); move auth/terminal into proxy/remove callback (C7); reorder terminal/disconnect or leak SDK type (C8); reinject/capture prepared/change budget/disposition/identity (C9); add observer/inspection (C10); enable wire v2/import outside core (C11); remove ListSettings/coerce route/params/use lossy send (C13); add `AgentRuntime`, legacy Rust import, or protected-parent command body (C14). Each fence reports its claim and exact observation, then restoration is green.

**Complexity/production scale:**
- Process adapter is $O(n)$ over input bytes with no production recording copy; stress one 1 MiB frame and 1,000 frames. Maximum added adapter/conductor overhead: 5 ms p99 per 1 MiB frame and 50 ms total for 1,000 small zero-stage frames, below one 50 ms UI tick.
- Stage forwarding is $O(s)$ per frame; production `s=0`, stress `s=4`, no dedup/replay.
- Domain enqueue/dequeue is $O(1)$ with fixed capacities; available-capacity overhead maximum 1 ms p99. Full-capacity wait must resolve on one drain, never allocate unboundedly or drop.
- Command/callback dispatch remains $O(1)$ plus existing payload conversion; no new history scan.
- Shape oracle is $O(p+d)$; maximum 2 seconds for 1,000 paths/20,000 changed lines.

**Wall budget/phase:** Frame forwarding/domain enqueue are always-on with 5 ms/1 ms budgets. Stage construction, spawn, initialize, shutdown, census, authenticated acceptance are one-off. Intentional saturation waiting is bounded by receiver progress/deadline and measured separately.

**Module shape:** Creates approved `sdk_runtime`/`domain_mediator`; deepens client; retains transport/engine/convert/KAS; splits runtime/serial bodies from protected bridge. Numeric-only implementation evidence tightened the approved ranges without changing responsibilities or interfaces: `bridge.rs` 300–500, `client.rs` 220–350, `sdk_runtime/mod.rs` 150–250, `sdk_runtime/process.rs` 120–220, `domain_mediator/mod.rs` 350–650, `inbound.rs` 120–300, and `host.rs` 180–320. The original 550-line lower bounds would reward padding after the clean split; these tighter bounds fence the actual deeper modules instead. Protected-parent deltas remain: `protocol/mod.rs` exactly +2 production lines (37 final); `crates/cyril/src/app.rs` exactly 0 production lines (2,723 final); root `Cargo.toml` exactly +2 lines (88 runtime, 87 final); `crates/cyril-core/Cargo.toml` exactly +2 lines (69 runtime, 68 final); all other member manifests exactly 0 lines. `module_shape.py` is authoritative for the executable bounds.

**Files:**
- `Cargo.toml`
- `Cargo.lock`
- `crates/cyril-core/Cargo.toml`
- `crates/cyril-core/src/protocol/mod.rs`
- `crates/cyril-core/src/protocol/bridge.rs`
- `crates/cyril-core/src/protocol/client.rs`
- `crates/cyril-core/src/protocol/transport.rs`
- `crates/cyril-core/src/protocol/engine.rs`
- `crates/cyril-core/src/protocol/fingerprint.rs`
- `crates/cyril-core/src/protocol/host_mediator.rs`
- `crates/cyril-core/src/protocol/source_observer.rs`
- `crates/cyril-core/src/protocol/turn_mediator.rs`
- `crates/cyril-core/src/protocol/tool_call_ledger.rs`
- `crates/cyril-core/src/protocol/sdk_runtime/mod.rs`
- `crates/cyril-core/src/protocol/sdk_runtime/process.rs`
- `crates/cyril-core/src/protocol/sdk_runtime/tests/mod.rs`
- `crates/cyril-core/src/protocol/sdk_runtime/tests/topology.rs`
- `crates/cyril-core/src/protocol/sdk_runtime/tests/process.rs`
- `crates/cyril-core/src/protocol/sdk_runtime/tests/handlers.rs`
- `crates/cyril-core/src/protocol/domain_mediator/mod.rs`
- `crates/cyril-core/src/protocol/domain_mediator/inbound.rs`
- `crates/cyril-core/src/protocol/domain_mediator/host.rs`
- `crates/cyril-core/src/protocol/domain_mediator/commands/mod.rs`
- `crates/cyril-core/src/protocol/domain_mediator/commands/session.rs`
- `crates/cyril-core/src/protocol/domain_mediator/commands/extensions.rs`
- `crates/cyril-core/src/protocol/domain_mediator/commands/subagents.rs`
- `crates/cyril-core/src/protocol/domain_mediator/commands/kas.rs`
- `crates/cyril-core/src/protocol/domain_mediator/tests/mod.rs`
- `crates/cyril-core/src/protocol/domain_mediator/tests/callbacks.rs`
- `crates/cyril-core/src/protocol/domain_mediator/tests/app_contract.rs`
- `crates/cyril-core/src/protocol/domain_mediator/tests/commands.rs`
- `crates/cyril-core/src/protocol/convert/mod.rs`
- `crates/cyril-core/src/protocol/convert/kiro.rs`
- `crates/cyril-core/src/protocol/convert/kas.rs`
- `crates/cyril-core/src/protocol/convert/probe_j1b3.rs`
- `crates/cyril-core/src/protocol/convert/probe_qo13.rs`
- `crates/cyril-core/src/protocol/kas/mod.rs`
- `crates/cyril-core/src/protocol/kas/auth.rs`
- `crates/cyril-core/src/protocol/kas/callbacks.rs`
- `crates/cyril-core/src/protocol/kas/hooks.rs`
- `crates/cyril-core/src/protocol/kas/host_io.rs`
- `crates/cyril-core/src/protocol/kas/kiro_fs.rs`
- `crates/cyril-core/src/protocol/kas/settings.rs`
- `crates/cyril-core/src/protocol/kas/terminal_io.rs`
- `crates/cyril-core/src/protocol/probe_dn91.rs`
- `crates/cyril-core/src/protocol/transport/tests/current_runtime_contract.rs`
- `crates/cyril-core/src/test_support.rs`
- `crates/cyril-core/src/usage.rs`
- `.cyril-gl5s/oracles/module_shape.py`
- `.cyril-gl5s/oracles/module-shape.json`
- `.cyril-gl5s/oracles/run_contract.py`
- `.cyril-gl5s/oracles/run_mutations.py`

**Estimate:** 4–8 focused implementation days; signal only. SDK type migration and exhaustive mutation checkpoint dominate uncertainty.

**Diff estimate:** 10,100 changed lines: 5,800 implementation, 3,500 behavioral tests/fixtures, 800 shape oracle/manifest.

**PR increment:** Increment A — Active conductor runtime and complete pre-deletion contract.

**Commands and expected results:**
- `python3 .cyril-gl5s/oracles/run_contract.py --phase runtime` → exit 0 and JSON `passed_claims` contains exactly `C1,C2,C3,C4,C5,C6,C7,C8,C9,C10,C11,C13,C14`; every named test filter ran through the conductor topology.
- `python3 .cyril-gl5s/oracles/module_shape.py --phase runtime` → exit 0 and C14 PASS with the exact protected-parent deltas above, three-argument start, zero legacy source importers/second runtime, and only the named unused legacy dependency.
- `python3 .cyril-gl5s/oracles/run_mutations.py --phase runtime` → exit 0 after applying each approved C1–C11/C13/C14 mutation singly, observing only its named fence fail with the expected mismatch, restoring the file, and observing green.
- `cargo fmt --all -- --check` → exit 0.
- `cargo clippy --all-targets -- -D warnings` → exit 0.
- `cargo clippy --all-targets --all-features -- -D warnings` → exit 0.
- `cargo test --all-targets` → exit 0.
- `cargo test --all-targets --all-features` → exit 0.
- `cargo run -p cyril --example test_bridge -- --agent-command kiro-cli acp` → exit 0 after authenticated v2 stable-wire-v1 streaming/tool/terminal order and bounded conductor shutdown.
- `bash -lc 'set -eu; repo=$(pwd); tmp=$(mktemp -d); trap \"rm -rf \\\"$tmp\\\"\" EXIT; printf \"SDK2 KAS fixture\\n\" >\"$tmp/README\"; cd \"$tmp\"; cargo run --manifest-path \"$repo/Cargo.toml\" -p cyril --features kas --example test_bridge -- --agent-engine kas --prompt \"Read README and run printf SDK2_HOST_CALLBACK, then summarize.\" --agent-command kiro-cli acp'` → exit 0 after authenticated KAS auth/fs/terminal/permission/hooks, structural `turn_end` order, and bounded conductor shutdown without repository mutation.

## Slice 2: Remove the named legacy ACP family and prove the final clean repository contract

**Claim IDs:** C12

**Expected behavior:** ACP0.10/schema0.11, named legacy dependency, `ClientSideConnection`, direct-runtime symbols, aliases/shims, and stale direct/`sacp-proxy` documentation are absent. Exactly SDK2/schema1.5/conductor2 and one runtime remain; all Slice1 behavior passes again.

**Oracle:** Independently parsed manifests, lock graph, source import/symbol/path census, upstream/default diff, ADR-0012, and recorded Slice1 outputs. SDK2/conductor presence is a positive control against vacuous no-ACP success.

**Stress fixture:** Independently inject legacy dependency/schema, `ClientSideConnection`, direct bypass, runtime trait, observer arg, non-core import, protected-parent command body. Expected C12/C14 failure names exact package/symbol/path/delta; restoration returns final censuses green.

**Regression fence:** `c12_sdk2_cutover_has_one_transport_family_and_runtime` plus `.cyril-gl5s/oracles/module_shape.py --phase final`.

**Named mutation:** Re-add named old dependency; C12 names package/version. Separately add `ClientSideConnection` sentinel/reference; C12 names exact path/symbol. Restore and rerun green.

**Complexity/production scale:** No production loop. Census is $O(p+l)$ over at most 5,000 packages/250,000 Rust lines; maximum one-off cost 5 seconds, above current scale but bounded against accidental expensive scans.

**Wall budget/phase:** One-off cleanup/census/docs/acceptance rerun; no always-on phase or production wall budget.

**Module shape:** Completes the manifest/protected-parent exit. Final protected-parent deltas from the approved baseline are: `protocol/bridge.rs` −2,673..−2,473 production lines (300–500 final); `protocol/mod.rs` exactly +2 production lines (37 final); `crates/cyril/src/app.rs` exactly 0 production lines (2,723 final); root `Cargo.toml` exactly +1 line (87 final); `crates/cyril-core/Cargo.toml` exactly +1 line (68 final); `crates/cyril/Cargo.toml`, `crates/cyril-ui/Cargo.toml`, `crates/cyril-memory/Cargo.toml`, and `crates/cyril-voice/Cargo.toml` exactly 0 lines. `module_shape.py --phase final` must PASS with one core-only SDK family, no old symbol/path, exact start arity/owners, and those deltas; legacy dependency/protected-parent mutations red then restoration green.

**Files:**
- `Cargo.toml`
- `Cargo.lock`
- `crates/cyril-core/Cargo.toml`
- `.cyril-gl5s/oracles/module_shape.py`
- `.cyril-gl5s/oracles/module-shape.json`
- `.cyril-gl5s/oracles/run_contract.py`
- `.cyril-gl5s/oracles/run_mutations.py`
- `docs/ROADMAP.md`
- `AGENTS.md`

**Estimate:** 1 focused day; signal only.

**Diff estimate:** 1,200 changed lines: 250 manifest/lock/obsolete deletion, 500 final census/mutations, 450 docs.

**PR increment:** Increment B — Legacy package deletion and final repository contract.

**Commands and expected results:**
- `cargo tree -p cyril-core --edges normal` → dependency graph contains `agent-client-protocol v2.0.0`, `agent-client-protocol-schema v1.5.0`, and `agent-client-protocol-conductor v2.0.0`, and contains no ACP 0.10/schema 0.11 package.
- `python3 .cyril-gl5s/oracles/run_contract.py --phase final` → exit 0 and JSON `passed_claims` contains exactly `C1,C2,C3,C4,C5,C6,C7,C8,C9,C10,C11,C12,C13,C14`; outputs for C1–C11/C13 are byte-for-byte equal to the recorded runtime-phase results.
- `python3 .cyril-gl5s/oracles/module_shape.py --phase final` → exit 0 with C12/C14 PASS, the exact final protected-parent deltas above, one package/runtime family, no old source symbol/path, and no stale live topology statement in `docs/ROADMAP.md` or `AGENTS.md`.
- `python3 .cyril-gl5s/oracles/run_mutations.py --phase final` → exit 0 after independently re-adding the named legacy dependency and `ClientSideConnection` sentinel, observing only C12/C14 fail with exact package/symbol/path, restoring both, and observing green.
- `cargo fmt --all -- --check` → exit 0.
- `cargo clippy --all-targets -- -D warnings` → exit 0.
- `cargo clippy --all-targets --all-features -- -D warnings` → exit 0.
- `cargo test --all-targets` → exit 0.
- `cargo test --all-targets --all-features` → exit 0.
- `cargo run -p cyril --example test_bridge -- --agent-command kiro-cli acp` → exit 0 after authenticated v2 stable-wire-v1 conductor turn and bounded shutdown.
- `bash -lc 'set -eu; repo=$(pwd); tmp=$(mktemp -d); trap \"rm -rf \\\"$tmp\\\"\" EXIT; printf \"SDK2 KAS fixture\\n\" >\"$tmp/README\"; cd \"$tmp\"; cargo run --manifest-path \"$repo/Cargo.toml\" -p cyril --features kas --example test_bridge -- --agent-engine kas --prompt \"Read README and run printf SDK2_HOST_CALLBACK, then summarize.\" --agent-command kiro-cli acp'` → exit 0 after authenticated KAS auth/fs/terminal/permission/hooks, structural `turn_end` order, and bounded conductor shutdown.
- `test \"$(git rev-list --count \"$(git merge-base HEAD main)\"..HEAD)\" -eq 1 && git diff --check \"$(git merge-base HEAD main)\"..HEAD` → exit 0 after checkpoint commits are squashed, proving one atomic feature commit and a whitespace-clean diff.

## Tracker taxonomy

- **Permanent non-goals:** draft wire v2, App/UI/domain rewrite, AcpAgent ownership, runtime switch/direct bypass, compatibility source aliases, placeholder stage, and production observer remain excluded for approved rationales.
- **Approved lifecycle state:** named unused legacy dependency is not future work; it is a mechanically bounded feature-branch checkpoint removed by Slice2 before delivery.
- **Intended future work:** verified `cyril-5g2o` owns multi-client broadcaster; verified `cyril-1ixa` owns trigger-conditioned agent-side pressure; parent `cyril-1gfe` owns later vendor-neutral selection. No cyril-gl5s behavior is deferred to them.
- **Cleanup follow-up:** N/A — Slice2 owns deletion/docs/final mutations/post-deletion reruns.

## Self-review

1. **Claim assignment:** PASS — Slice1 owns C1–C11/C13/C14 exactly once; Slice2 owns C12 exactly once. Every PENDING falsifier has an owning slice/command.
2. **Mandatory fields:** PASS — both slices carry all fourteen fields and reasoned conditionals.
3. **Fence locality:** PASS — Slice1 creates runtime/behavior/shape fences with approved mutations; Slice2 creates C12 final fence/mutations.
4. **Complexity/wall budgets:** PASS — every new always-on path has scale/measurable bounds; one-off phases classified.
5. **Module shape:** PASS — every touched owner is ledgered; bridge/mod/App rules and runtime/final shape commands explicit.
6. **Review-size partition:** PASS — 11,300 + 3,390 = 14,690; two independently green feature-integration increments; final squash atomic.
7. **Tracker taxonomy:** PASS — lifecycle/non-goals/future items classified with verified owners.
8. **Completion ownership:** PASS — no slice declared complete; checkpointed-build alone records gates.
