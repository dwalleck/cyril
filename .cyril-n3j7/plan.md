# Plan: cyril-n3j7

## Inputs and partition

- Route: Empirical; `route.md` requires all downstream gates and no `FAIL`.
- Approved design: `design.md`; requester selected "I approve this design" in the design-gate prompt on 2026-08-25; no risk acceptances.
- Evidence: `evidence.md`, `probe.rs`, and `oracle.py`; P1/P2 are `PASS`.
- Review re-entry: N/A — initial approved plan.

| Slice | Projected changed lines |
|---|---:|
| 1. Durable source-turn memory substrate and prompt preparation | 2,800 |
| 2. Authoritative bridge capture and bounded forwarder | 2,500 |
| 3. Storage-backed `/memory` turn inspection | 700 |
| **Slice sum** | **6,000** |
| Churn margin | **1,500 (25%)** |
| **Projected total** | **7,500** |

The 25% margin covers strict wire fixtures, FTS5 shadow-table schema manifests, cross-platform bridge harness updates, and caller migration from the private protocol-v2 cutover. The total exceeds 4,000 lines, so three independently mergeable increments are required:

1. **Memory substrate** — Slice 1. Schema v3, protocol v2, source capture/list/inspect operations, FTS5 retrieval, and query-aware prompt preparation verify with all production callers migrated; lesson behavior stays green without a capture producer.
2. **Capture adapter** — Slice 2, based on Memory substrate. Bridge events persist through the binary adapter and the fake-agent path captures/restarts/recalls without the inspection UI.
3. **Inspection projection** — Slice 3, based on Capture adapter. Typed `/memory` inspection consumes existing durable records; persistence/recall do not depend on it.

Local projected totals with margin: Memory substrate 3,500; Capture adapter 3,125; Inspection projection 875.

## Slice 1: Build durable source-turn storage and query-aware prompt context

**Claim IDs:** C3, C4, C8, C11, C13

**Expected behavior:** Source-event batches stage and commit one immutable project turn; restart preserves partial/completed state; same-ID retries are idempotent/conflict-safe; only completed turns enter deterministic bounded FTS5 recall; lessons and episodes compose behind one opaque query-aware prompt interface; v1/v2 stores migrate atomically to v3; strict authenticated protocol v2 replaces the old context contract.

**Oracle:** Read-only SQLite manifests/counts and independently recomputed versioned SHA-256 for C3/C8; retained Python FTS oracle and hand ID/character expectations for C4; raw JSON/error table for C11; Cargo/source import manifest and compiler visibility for C13.

**Stress fixture:** Two projects; partial turn; 16-event/256-KiB batches; same-ID identical/conflicting replay; 128 bounded tools; stronger foreign match; rank ties; Unicode/quotes/operators/whitespace query; full 4,000-character lessons and three 1,200-character episodes; v1/v2/v3 stores, FTS shadow objects, unknown object, and failed migration. Expected: exact scope/order/bounds, identical replay no-op, typed conflict/audit, and transactional rollback.

**Regression fence:** `source_turn::tests::c3_source_turn_restart_retry_is_exact_once_and_conflict_safe`; `store::tests::c4_episode_recall_is_literal_scoped_deterministic_and_bounded`; `store::tests::c8_memory_v1_and_v2_migrate_atomically_to_v3`; `wire::tests::c11_v2_source_operations_are_strict_and_authenticated`; `architecture_tests::c13_memory_policy_stays_behind_runtime_interface`.

**Named mutation:** C3 `INSERT OR REPLACE`; C4 remove project/status predicate; C8 target v3 without sequential migration; C11 remove strict unknown-field/event bound; C13 put `bm25`/framing in `app.rs`. Each `cN_…` fence must turn red and return green after restoration.

**Complexity/production scale:** Reducer $O(E+B+T)$ for 16 events/256 KiB/128 tools, maximum 100 ms per batch. Query builder $O(Q)$ for 4,096 scalars/64 terms; FTS returns three results and must complete store query/render within 50 ms on a 100,000-turn fixture, preserving 200 ms for IPC/scheduling inside the 250 ms prompt deadline. List $O(100)$; migration $O(N)$ and one-off.

**Wall budget/phase:** Always-on capture batch ≤100 ms and first-prompt store query/render ≤50 ms at production bounds. Migration: N/A — reason: one-off startup phase; no wall budget.

**Files:** Create `crates/cyril-memory/src/source_turn.rs`, `crates/cyril-memory/src/redaction.rs`. Modify `crates/cyril-memory/src/{lib,lesson,protocol,wire,client,runtime,store}.rs`, `crates/cyril/src/{memory_runtime,app}.rs`, `crates/cyril/tests/{memory_runtime,architecture_tests}.rs`, and version assertions in `crates/cyril-core/src/types/memory.rs`.

**Estimate:** 2–3 focused days; signal only.

**Diff estimate:** 2,800 changed lines including implementation, inline tests, raw-wire/migration fixtures, and caller cutover.

**PR increment:** Memory substrate

**Commands and expected results:**
- `cargo test -p cyril-memory c3_source_turn_restart_retry_is_exact_once_and_conflict_safe` → partial/completed reopen; same ID/content one row; changed content typed conflict preserving bytes/hash plus audit. C3 mutation red; restored green.
- `cargo test -p cyril-memory c4_episode_recall_is_literal_scoped_deterministic_and_bounded` → IDs/order match `oracle.py`; foreign/ineligible absent; 3×1,200/3,600 scalar bounds. C4 mutation red; restored green.
- `cargo test -p cyril-memory c8_memory_v1_and_v2_migrate_atomically_to_v3` → v1/v2 preserve data and exact v3 objects; failed v3 rolls back; reopen idempotent. C8 mutation red; restored green.
- `cargo test -p cyril-memory c11_v2_source_operations_are_strict_and_authenticated` → raw-frame matrix matches error table; malformed/oversized/version/auth/id remain distinct. C11 mutation red; restored green.
- `cargo test -p cyril --test memory_runtime c3_` → real runtime restart preserves partial/completed rows and retry semantics.
- `cargo test -p cyril --test architecture_tests c13_memory_policy_stays_behind_runtime_interface` → no policy escapes cyril-memory. App `bm25` mutation red; restored green.

## Slice 2: Capture bridge turns without blocking ACP or losing shutdown ordering

**Claim IDs:** C1, C2, C5, C6, C9, C10, C12

**Expected behavior:** Every accepted main prompt yields bounded original-only events; all normalized callbacks begun before response quiescence assemble, thoughts stay excluded, terminals are truthful, loss never completes, capture ignores UI retention, a later same-project first prompt receives lessons then episodes exactly once, numeric ID reuse cannot collide, and bridge stop plus forwarder drain precede memory shutdown.

**Oracle:** Hand source/wire/UI vectors for C1/C5; terminal truth table C2; independent reducer and secret/thought scanner C6; Tokio clock, queue counter, bridge signal, runtime log C9; Cargo metadata C10; accepted-prompt list/direct durable keys C12.

**Stress fixture:** Enriched first prompt; Unicode/multi-block source; partial tool updates; callback held until after UI completion but before barrier; AgentThought; error/Cancelled/Refusal/MaxTokens/KAS dual terminal/death; replay history; two bridge instances with numeric zero; 33 maximum fragments against paused runtime; quit mid-turn. Expected: full source, no thought/context leak, correct dispositions, queue ≤32, loss non-completed, bridge completion before runtime stop, distinct durable IDs.

**Regression fence:** `bridge::tests::c1_accepted_prompt_capture_precedes_ui_and_excludes_context`; `bridge::tests::c2_terminal_disposition_never_false_completes`; `c5_first_prompt_is_ordered_exactly_once_and_source_clean`; `c6_stream_tool_tail_assembles_without_thoughts_or_secrets`; `c9_slow_capture_is_bounded_and_shutdown_drains_in_order`; `c10_core_and_ui_remain_persistence_free`; `c12_source_identity_survives_numeric_reuse_and_ignores_history`.

**Named mutation:** C1 capture wire blocks; C2 false-complete prompt error; C5 prefix source; C6 close at UI terminal/accept thought; C9 unbounded channel or wrong shutdown order; C10 add memory dependency to core; C12 key numeric ID/seed UserMessage. Each fence turns red then green after restoration.

**Complexity/production scale:** Fragmentation $O(B)$ with 64-KiB pieces. Enqueue $O(1)$ into 32 slots, about two MiB queued maximum. Forwarder $O(E+B)$ for 16 events/256 KiB and one in-flight request; bridge synchronous work ≤10 ms per fragment, async batch ≤100 ms. Quiescence is atomic-count based, no collection scan, ≤50 ms at the 32-event stress bound. Shutdown drain hard-capped two seconds.

**Wall budget/phase:** Always-on bridge capture ≤10 ms per 64-KiB event, barrier ≤50 ms per turn, forwarder ≤100 ms per batch. Shutdown: N/A — reason: one-off process phase with two-second hard bound.

**Files:** Create `crates/cyril-core/src/types/source_turn.rs`, `crates/cyril/src/capture_forwarder.rs`, `crates/cyril/tests/memory_capture.rs`. Modify root/Core Cargo manifests, `crates/cyril-core/src/types/{mod,event}.rs`, `crates/cyril-core/src/protocol/{mod,bridge,client,turn_mediator}.rs`, `crates/cyril/src/{main,app,memory_runtime}.rs`, and architecture tests.

**Estimate:** 2–3 focused days; signal only.

**Diff estimate:** 2,500 changed lines including bridge interface/barrier/queue, adapter, fake-agent and integration fixtures.

**PR increment:** Capture adapter

**Commands and expected results:**
- `cargo test -p cyril-core c1_accepted_prompt_capture_precedes_ui_and_excludes_context` → source equals original, wire alone prefixed, observer precedes UI. Mutation red; restored green.
- `cargo test -p cyril-core c2_terminal_disposition_never_false_completes` → truth table exact; only EndTurn eligible. Mutation red; restored green.
- `cargo test -p cyril --test memory_capture c5_first_prompt_is_ordered_exactly_once_and_source_clean` → first later prompt has lessons then episode; second does not; UI/source original. Mutation red; restored green.
- `cargo test -p cyril --test memory_capture c6_stream_tool_tail_assembles_without_thoughts_or_secrets` → tail retained, tool merge equals oracle, no thought/secret. Mutation red; restored green.
- `cargo test -p cyril --test memory_capture c9_slow_capture_is_bounded_and_shutdown_drains_in_order` → UI completes, queue ≤32, overflow non-completed, bridge signal before final capture/runtime stop. Mutation red/hangs within virtual bound; restored green.
- `cargo test -p cyril --test architecture_tests c10_core_and_ui_remain_persistence_free` → no forbidden dependency. Manifest mutation red; restored green.
- `cargo test -p cyril --test memory_capture c12_source_identity_survives_numeric_reuse_and_ignores_history` → numeric-zero turns have distinct durable IDs; UserMessage creates none; same-ID retry one row. Mutation red; restored green.

## Slice 3: Expose durable turns through typed `/memory` inspection

**Claim IDs:** C7

**Expected behavior:** `/memory turns` returns bounded newest-first rows; `/memory inspect-turn <id>` renders scoped identity, state, timestamps, hash, truncation/provenance, original prompt, assistant, and tools after UI clear/eviction; malformed/missing/foreign IDs share safe not-found.

**Oracle:** Direct runtime fixture records and hand exact strings/counts, independent of UI history and formatter helpers.

**Stress fixture:** Zero/one/100/101 turns; all states; malformed/foreign IDs; Unicode at bounds; UiState cleared/overflowed. Expected: 100 plus omitted/corrupt counts, 4,000 prompt/8,000 assistant/4,000 tool characters (16,000 total content), scalar-safe markers, safe not-found, identical result after eviction.

**Regression fence:** App/UI `c7_turn_inspection_survives_ui_retention_and_is_scoped`, plus core parser and UI formatter C7 matrices.

**Named mutation:** `app.rs` derives from `UiState::messages`; C7 fence red then green after restoration.

**Complexity/production scale:** List $O(R+C)$ for 100 bounded previews; inspect $O(C)$ for 16,000 content characters. Store/query/format maximum 50 ms per invocation.

**Wall budget/phase:** Always-on per `/memory turns|inspect-turn` invocation ≤50 ms; remains off App event loop.

**Files:** Modify `crates/cyril-core/src/commands/{mod,builtin}.rs`, `crates/cyril-core/src/types/memory.rs`, `crates/cyril-ui/src/{lib,memory_format}.rs`, `crates/cyril/src/{app,memory_runtime}.rs`, and `crates/cyril/tests/memory_capture.rs`.

**Estimate:** 1 focused day; signal only.

**Diff estimate:** 700 changed lines including typed projections/actions, parser/formatter/App wiring, fixtures.

**PR increment:** Inspection projection

**Commands and expected results:**
- `cargo test -p cyril-core memory_commands_emit_typed_actions` → exact syntax adds turns/inspect-turn, keeps lessons, rejects missing/malformed without bridge dispatch.
- `cargo test -p cyril-ui c7_` → exact list/state/provenance/truncation strings and scalar bounds match hand fixtures.
- `cargo test -p cyril --test memory_capture c7_turn_inspection_survives_ui_retention_and_is_scoped` → store fixture/output agree before/after clear/eviction; foreign/missing safe. Mutation red with `C7:`; restored green.

## Tracker taxonomy

Approved design exclusions cite verified `cyril-xajq`, `cyril-nxq5`, `cyril-s7gn`, `cyril-y91y`, `cyril-3dqf`, `cyril-39xn`, and `cyril-ct0y`. Permanent exclusions are private-thought persistence and a caller-visible backend selector, with rationale in `design.md`. No new deferral is introduced.

## Self-review

- [x] Claims C1–C13 are each assigned once; every PENDING falsifier is in its owning slice; C10 cheapest falsifier remains PASS.
- [x] Every slice has all thirteen fields and explicit `N/A — reason` where applicable.
- [x] Every claim creates its `cN_…` fence with the approved mutation; no fence-less risks.
- [x] Every loop has complexity/input/cost bounds; every always-on phase has a wall budget.
- [x] 6,000 + 25%/1,500 = 7,500 projected lines; three increments are independently green and below 4,000 with local margin.
- [x] Tracker taxonomy is applied.
- [x] No slice is declared complete; checkpointed-build exclusively judges completion.
