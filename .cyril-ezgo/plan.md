# Plan: cyril-ezgo

## Inputs and partition

- Approved design: [`design.md`](design.md), requester approval “I approve this design”, 2026-08-24; no approved risk acceptances.
- Route/evidence: Empirical; [`evidence.md`](evidence.md) P1 is `PASS`.
- Upstream branch: repository default/upstream discovered by the session-worktree helper; no branch name is hard-coded into an increment contract.

Projected changed lines:

| Slice | Diff estimate |
|---|---:|
| 1. Project and lesson domain | 620 |
| 2. Durable lesson store | 650 |
| 3. Runtime protocol and client | 800 |
| 4. Runtime access lifecycle | 280 |
| 5. Memory command and workspace UX | 560 |
| 6. First-prompt bridge adapter | 430 |
| 7. Transcript/wire integration | 260 |
| **Base sum** | **3,600** |
| **Churn margin (20%)** | **720** |
| **Projected total** | **4,320** |

The 20% margin covers SQLite migration/error mapping, cross-platform path fixtures, and the broad existing `CommandContext`/bridge test-literal migration. Because 4,320 exceeds the 4,000-line review-size gate, use two independently mergeable increments:

1. **lesson-runtime** — Slices 1–5, base 2,910 lines. Mergeable definition: memory-enabled Cyril can resolve one project, teach/replace/list/inspect persistent explicit lessons through the authenticated runtime, and preserve all M0 disabled/status/ownership behavior. Verification uses the real runtime and typed command projection; no later bridge adapter is needed.
2. **first-prompt-adapter** — Slices 6–7, base 690 lines. Mergeable definition: the already-useful lesson runtime becomes a bounded exactly-once wire adapter with exact fail-open and transcript separation. Verification uses the core FakeAgent harness plus App integration and the real runtime from increment 1.

Schema lands in the same increment as its production `/memory` consumer; no retrieval/content schema is merged without a user-facing caller.

## Slice 1: Establish project identity, lesson validation/redaction, and bounded context rendering

**Claim IDs:** C1, C3, C9

**Expected behavior:** A supplied workspace resolves to an opaque stable project ID and separate display path; lesson input normalizes/validates/redacts before any boundary; eligible explicit lessons render in deterministic whole-row order within any character budget.

**Oracle:** Git `rev-parse --git-common-dir` plus `realpath` for C1; a hand-authored detector/redaction table and raw-secret absence scan for C3; a test-only greedy reference over fixture lengths/eligibility for C9.

**Stress fixture:** A linked-worktree layout with relative `gitdir`/`commondir`, Unicode and spaces in paths; 2,000-scalar text containing every supported secret class plus below-threshold lookalikes; context candidates mixing active/inactive, explicit/derived, instruction/non-instruction at exact and one-character-over budget boundaries. Expected: shared Git ID/distinct display, only supported secret values replaced, lookalikes preserved, no partial lesson, exact omitted count.

**Regression fence:** `crates/cyril-memory/src/project.rs::tests::project_identity_matches_git_common_dir_and_keeps_display_paths`; `crates/cyril-memory/src/lesson.rs::tests::supported_secrets_are_redacted_before_every_boundary`; `crates/cyril-memory/src/lesson.rs::tests::context_block_respects_trust_order_and_whole_lesson_budget`.

**Named mutation:** C1 — return the canonical linked workspace instead of following `.git`/`commondir`; C3 — bypass `redact` before hashing/domain construction; C9 — truncate the final rendered string at the budget or change eligibility to a negative exclusion. Each named fence must turn red.

**Complexity/production scale:** Workspace discovery is $O(d)$ ancestor probes with a practical maximum of 64 path components; accept ≤1 ms on the production workstation because it runs once per bound workspace. Normalization/redaction is $O(n \times p)$ for $n \le 2{,}000$ scalars and a fixed detector set $p$; accept ≤1 ms. Rendering is $O(k + 4{,}000)$ and stops after the 4,000-scalar output budget; accept ≤1 ms exclusive of storage.

**Wall budget/phase:** Project resolution is one-off per process-bound workspace: N/A — one-off phase; no wall budget. Validation/redaction is one-off per teach command: N/A — one-off phase; no wall budget. Context rendering is always-on once per fresh session first prompt: ≤5 ms CPU, leaving the local-store operation most of the 250 ms bridge deadline.

**Files:** `Cargo.toml`; `crates/cyril-memory/Cargo.toml`; create `crates/cyril-memory/src/project.rs`; create `crates/cyril-memory/src/lesson.rs`; modify `crates/cyril-memory/src/lib.rs`.

**Estimate:** 1 focused implementation slice.

**Diff estimate:** 620 changed lines including tests.

**PR increment:** lesson-runtime

**Commands and expected results:**
- `python .cyril-ezgo/probe.py main /home/dwalleck/repos/cyril linked /home/dwalleck/repos/cyril-wt-feat-cyril-ezgo nongit /tmp` plus the recorded Git/`realpath` oracle → item-for-item identity/display agreement from `evidence.md` P1.
- `cargo test -p cyril-memory project::tests lesson::tests` → every path, validation, detector, trust, order, budget, and omitted-count fixture matches its independent table/reference.
- Apply each three named mutations at checkpoint → its named fence turns red; restore → green.
- `cargo clippy -p cyril-memory -- -D warnings` → no warnings; no suppressed lint or unsafe code.

## Slice 2: Persist scoped active and superseded lessons with append-only audit

**Claim IDs:** C2

**Expected behavior:** Memory schema v1 migrates atomically to v2; create/duplicate/replace operations preserve immutable history, project isolation, deterministic sequence order, and one audit row per teaching attempt across reopen.

**Oracle:** A separate read-only rusqlite connection hand-counts projects, active/invalidated lessons, hashes, supersession links, sequence order, indexes, and audit rows without using production store read methods.

**Stress fixture:** Two projects teach the same normalized text; project A repeats it, replaces it twice in the same millisecond fixture, then queries old/new IDs after reopen. Expected: cross-project IDs distinct; duplicate returns existing active ID; old rows invalidated and inspectable only in project A; sequence order matches insertion; four project-A teaching attempts produce four audits; no raw content in audit.

**Regression fence:** `crates/cyril-memory/src/store.rs::tests::lesson_lifecycle_is_scoped_idempotent_and_append_audited`; `crates/cyril-memory/src/store.rs::tests::memory_v1_migrates_atomically_to_v2`.

**Named mutation:** Remove the partial active-content unique index, or replace the supersession insert transaction with an in-place update; the lifecycle fence must report duplicate active rows or lost history.

**Complexity/production scale:** Teach/replace use indexed $O(\log N)$ lookups and one bounded transaction. List is $O(\min(N,100))$ with ≤100 summaries ×160 characters. Context count is indexed and row iteration stops once 4,000 characters cannot accept another lesson; no active collection is loaded wholesale. Explicit maximum: local store work ≤200 ms per operation, leaving 50 ms of the bridge's 250 ms deadline for IPC/rendering; rationale is the existing 250 ms SQLite busy timeout and local single-user workload.

**Wall budget/phase:** Teach/list/inspect are one-off commands: N/A — one-off phase; no wall budget. Context selection is always-on once per fresh-session first prompt: ≤200 ms local-store wall time.

**Files:** modify `crates/cyril-memory/src/store.rs`; reuse domain types from Slice 1.

**Estimate:** 1 focused implementation slice.

**Diff estimate:** 650 changed lines including migrations and tests.

**PR increment:** lesson-runtime

**Commands and expected results:**
- `cargo test -p cyril-memory store::tests` → v1→v2 migration, duplicate, supersession, scope, order, audit, reopen, lock, corruption, and partial-initialization outcomes match direct SQL oracle counts.
- Apply each named mutation at checkpoint → `lesson_lifecycle_is_scoped_idempotent_and_append_audited` turns red; restore → green.
- `cargo clippy -p cyril-memory -- -D warnings` → no warnings.

## Slice 3: Deepen protocol/runtime/client with strict project-lesson operations

**Claim IDs:** C10, C12

**Expected behavior:** Existing protocol-v1 health/shutdown frames remain valid; typed bind/teach/replace/list/inspect/context operations are authenticated, operation-strict, bounded, and safe; a real runtime restart preserves schema-v2 lessons/audit while knowledge stays v1 and M0 lock/privacy behavior remains.

**Oracle:** Hand-authored raw JSON request/response table for protocol behavior; direct read-only SQLite and filesystem metadata/second-process checks for runtime persistence and ownership.

**Stress fixture:** Legacy null health/shutdown frames; every new valid payload; unknown fields/operations, wrong payload types, wrong auth, non-monotonic IDs, exact-cap and cap+1 frames; real runtime teach/replace/shutdown/restart with a competing owner. Expected: valid legacy/new responses, stable safe codes for every invalid class, preserved lesson/audit rows, memory version 2/knowledge version 1, second owner denied.

**Regression fence:** `crates/cyril-memory/src/wire.rs::tests::v1_operation_payload_matrix_is_strict_and_backward_compatible`; existing cap/auth/id tests; `crates/cyril/tests/memory_runtime.rs::real_runtime_restart_preserves_lessons_and_audit` plus existing runtime M0 fences.

**Named mutation:** C10 — decode new payloads through permissive `serde_json::Value` or ignore unknown fields; strict matrix turns red. C12 — omit v2 migration/reopen state or release ownership before serving; real runtime fence turns red.

**Complexity/production scale:** Codec work is $O(F)$ for frame size $F \le 1$ MiB, rejected before allocating above the cap. Each client method performs one local IPC request and delegates bounded store loops from Slice 2. Maximum accepted end-to-end runtime operation ≤200 ms under uncontended local conditions; bridge supplies a stricter 250 ms outer deadline.

**Wall budget/phase:** Health and command operations are one-off: N/A — discrete requests; no recurring wall budget. First-prompt context request is always-on once per fresh session: ≤200 ms runtime wall time.

**Files:** modify `crates/cyril-memory/src/protocol.rs`, `wire.rs`, `runtime.rs`, `client.rs`, `lib.rs`; modify `crates/cyril/tests/memory_runtime.rs`.

**Estimate:** 1 focused implementation slice.

**Diff estimate:** 800 changed lines including raw-wire and process tests.

**PR increment:** lesson-runtime

**Commands and expected results:**
- `cargo test -p cyril-memory wire::tests protocol::tests client::tests` → legacy/new valid frames and every invalid matrix cell match the hand-authored oracle; cap/auth/id fences remain green.
- `cargo test -p cyril --test memory_runtime` → real runtime reports memory schema 2/knowledge schema 1, persists lesson/supersession/audit across restart, denies second owner, preserves private permissions.
- Apply both named mutations at checkpoint → corresponding strict-wire/runtime fence turns red; restore → green.
- `cargo clippy -p cyril-memory -p cyril -- -D warnings` → no warnings.

## Slice 4: Publish credential-hiding project access through runtime lifecycle

**Claim IDs:** C4

**Expected behavior:** App/bridge callers can hold only a bound `ProjectMemory`; it returns typed unavailable outcomes across disabled/starting/degraded/failed/runtime-loss states, while core/UI dependency graphs remain persistence-free and admin shutdown privilege remains inaccessible.

**Oracle:** Cargo dependency graph and manifest parser for crate direction; compiler visibility plus state-table expectations for credential/client access.

**Stress fixture:** Construct absent/off/starting/ready/degraded/failed handles and close the readiness channel after ready. Expected: only ready can execute project operations; every other state returns a distinct safe unavailable outcome; no endpoint, credential, SQL, or admin shutdown method is exposed on the bound handle.

**Regression fence:** create `crates/cyril/tests/architecture_tests.rs::core_and_ui_remain_persistence_free`; `crates/cyril/src/memory_runtime.rs::tests::project_access_tracks_runtime_lifecycle_without_exposing_admin`.

**Named mutation:** Add `cyril-memory` to `cyril-core/Cargo.toml`, or expose `AdminCredential`/`AdminClient` from the project handle; dependency/visibility fence must fail to compile or report the forbidden edge/surface.

**Complexity/production scale:** N/A — no new collection loop; watch-state lookup and client clone are $O(1)$.

**Wall budget/phase:** Runtime status lookup is always-on at each memory operation: ≤100 µs; it is an in-process watch snapshot with no I/O.

**Files:** modify `crates/cyril/src/memory_runtime.rs`; create `crates/cyril/tests/architecture_tests.rs`; update test-only constructors in `crates/cyril/src/app.rs` as required.

**Estimate:** 1 focused implementation slice.

**Diff estimate:** 280 changed lines including tests.

**PR increment:** lesson-runtime

**Commands and expected results:**
- `cargo test -p cyril --test architecture_tests` and focused `cargo test -p cyril memory_runtime::tests::project_access_tracks_runtime_lifecycle_without_exposing_admin` → dependency graph contains no core/UI→memory edge and lifecycle matrix matches expected typed outcomes.
- Apply forbidden-edge mutation at checkpoint → architecture fence turns red; restore → green.
- `cargo clippy -p cyril -- -D warnings` → no warnings.

## Slice 5: Add typed `/memory` teaching UX and bind `/new` to startup workspace

**Claim IDs:** C5, C11

**Expected behavior:** `/memory status|teach|teach --replace|list|inspect` parse into typed actions, App executes them against the concrete bound project, and UI renders bounded typed projections; initial and `/new` sessions always use the canonical startup-bound workspace rather than process cwd.

**Oracle:** Hand-authored command/result/output matrix; direct equality between emitted `BridgeCommand::NewSession.cwd` and the constructor fixture path.

**Stress fixture:** All syntax/error shapes from design Input shapes; 101 lesson summaries to force 100+omitted output; invalidated/foreign/malformed inspect IDs; registry/App constructed with Unicode-space workspace deliberately distinct from actual process cwd. Expected: typed actions, safe validation/not-found output, provenance/trust/status visible, bounded list, exact fixture workspace on every new-session command.

**Regression fence:** `crates/cyril-core/src/commands/mod.rs::tests::memory_commands_emit_typed_actions`; `new_command_uses_bound_workspace`; `crates/cyril-ui/src/memory_format.rs::tests::lesson_command_matrix_is_bounded_and_typed`; App command integration in `crates/cyril/src/app.rs`.

**Named mutation:** C5 — return an untyped `SystemMessage` from `MemoryCommand` or format raw rows in App; typed action/projection fence turns red. C11 — restore `std::env::current_dir()` in `NewCommand::execute`; fixture-workspace fence turns red.

**Complexity/production scale:** Command parsing is $O(n)$ for ≤2,000-scalar teach input. List formatting is $O(\min(N,100) \times 160)$ with ≤100 rows and bounded omitted count; accepted maximum rendered output 20 KiB, sufficient for IDs/metadata/previews while preventing TUI flooding.

**Wall budget/phase:** Slash commands and session creation are one-off discrete actions: N/A — one-off phase; no recurring wall budget.

**Files:** modify `crates/cyril-core/src/commands/builtin.rs`, `commands/mod.rs`, core memory projection types and exports; modify `crates/cyril-ui/src/memory_format.rs`; modify `crates/cyril/src/app.rs`, `main.rs`; migrate `CommandContext`/registry constructors in core tests, `crates/cyril/tests/event_routing.rs`, and other compiler-reported callers.

**Estimate:** 1 cross-crate atomic slice because the public command/registry signatures and every caller must migrate together.

**Diff estimate:** 560 changed lines including tests/callers.

**PR increment:** lesson-runtime

**Commands and expected results:**
- `cargo test -p cyril-core commands::tests` → every syntax row maps to the expected typed action; `/new` emits the bound fixture workspace.
- `cargo test -p cyril-ui memory_format::tests` → typed status/lesson outputs match exact bounded matrix.
- Focused App/event-routing tests → teach/list/inspect concrete calls produce projected command output and no bridge command; `/new` emits bound cwd.
- Apply both named mutations at checkpoint → corresponding command/workspace fences turn red; restore → green.
- `cargo test` and `cargo clippy -- -D warnings` → increment is independently green across the workspace.

## Slice 6: Prepend first-prompt context exactly once with exact fail-open

**Claim IDs:** C6, C8

**Expected behavior:** Bridge calls a narrow first-prompt context adapter once per accepted fresh session, prepends one complete returned block before unchanged originals, never calls it on later prompts, and sends originals unchanged on every unavailable/error/closed/timeout shape within 250 ms virtual time.

**Oracle:** A separately cloned original `Vec<String>` and hand-built expected ACP vectors; paused Tokio clock plus FakeAgent completion notification.

**Stress fixture:** Empty, one, multi, duplicate, and Unicode original vectors; reused session ID; adapters returning block/None/error/closed/forever-pending; first send failure and later prompt. Expected: exact prefix only on successful first nonempty prompt, empty consumes attempt unchanged, every failure unchanged, later never calls adapter, all turns complete.

**Regression fence:** `crates/cyril-core/src/protocol/bridge.rs::tests::first_prompt_memory_is_ordered_and_exactly_once`; `empty_prompt_consumes_attempt_without_mutation`; `memory_failure_and_timeout_are_exact_fail_open` with `start_paused = true`.

**Named mutation:** C6 — set attempted only after await or overwrite original block 0; exactly-once/order fence turns red. C8 — propagate adapter error with `?` or remove `tokio::time::timeout`; fail-open fence turns red or fails the virtual-time completion bound.

**Complexity/production scale:** Prefix construction is $O(B)$ for original block count/bytes, bounded by the existing 1 MiB downstream frame cap; no original block is copied except the unavoidable ACP conversion already present, and insertion preallocates exactly one extra slot. Adapter call occurs once per fresh session.

**Wall budget/phase:** Always-on for the first accepted prompt of each fresh session only: hard outer deadline 250 ms; disabled/no-adapter path ≤100 µs and performs no allocation beyond existing prompt conversion.

**Files:** modify `crates/cyril-core/src/protocol/bridge.rs`; add narrow adapter interface near bridge spawn; modify bridge spawn callsites/test harness configuration; extend FakeAgent `Script` to capture prompt payloads.

**Estimate:** 1 bridge-local atomic slice.

**Diff estimate:** 430 changed lines including harness/fences.

**PR increment:** first-prompt-adapter

**Commands and expected results:**
- Focused `cargo test -p cyril-core protocol::bridge::tests::first_prompt_memory` plus fail-open/empty tests → captured vectors exactly match hand-built oracle and virtual time reaches completion at 250 ms without mutation/retry.
- Apply both named mutations at checkpoint → corresponding order/exactly-once/fail-open fence turns red; restore → green.
- `cargo clippy -p cyril-core -- -D warnings` → no warnings.

## Slice 7: Wire bound project memory end to end without transcript contamination

**Claim IDs:** C7

**Expected behavior:** Main supplies the bound concrete project adapter to bridge, interactive and startup prompts show only original text in UiState while FakeAgent receives the memory prefix, and the injected block is never passed back as lesson/source input.

**Oracle:** Original UI input fixture compared directly to UiState messages; independently built prefixed vector compared to FakeAgent capture; raw source/request capture checked for absence of `CYRIL_LESSONS` outside the outbound adapter block.

**Stress fixture:** Interactive prompt with file attachment and startup `--prompt`, both containing a marker also present in one stored lesson. Expected: UI UserText equals only original typed text, attachment stays an original subsequent wire block, memory prefix appears once and first on wire, no persisted/source field contains the prefix.

**Regression fence:** App/integration `injected_context_is_wire_only_for_interactive_and_startup_prompts`; extend real-runtime lesson process test only where needed to create the production adapter fixture.

**Named mutation:** Change `App::add_user_message` or startup-prompt commit to use prepared wire text; the integration fence exposes `CYRIL_LESSONS` in UserText and turns red.

**Complexity/production scale:** N/A — no new loop beyond Slice 6 bridge prefix and existing App message projection.

**Wall budget/phase:** N/A — wiring adds no phase; Slice 6's 250 ms deadline remains the complete always-on first-prompt budget.

**Files:** modify `crates/cyril/src/main.rs`, `app.rs`, `memory_runtime.rs`; add/extend integration harness under `crates/cyril/tests/` or App tests; update bridge construction callsites.

**Estimate:** 1 end-to-end wiring slice.

**Diff estimate:** 260 changed lines including integration fence.

**PR increment:** first-prompt-adapter

**Commands and expected results:**
- Focused App/integration command → UiState original text and FakeAgent prefixed wire vector agree item-by-item with separate oracle; prefix absent from UI/source capture.
- Apply named App-message mutation at checkpoint → transcript fence turns red; restore → green.
- `cargo test` and `cargo clippy -- -D warnings` → final two-increment stack is workspace-green.

## Tracker taxonomy

- No generic backend selector, new tuning surface, delete/restore/promotion command, or persistence in core/UI: permanent non-goals for cyril-ezgo with rationale in approved `design.md`.
- Transcript-derived episodes are intended future work in verified `cyril-n3j7`.
- Loaded-session capability binding and agent-facing MCP recall are intended future work in verified `cyril-3dqf`.
- Cited knowledge/embedding retrieval and proxy-stage replacement are intended future work under verified epic `cyril-ct0y` and ADR-0003.

No new tracker issue is required by this plan.

## Self-review

- [x] Every design claim C1–C12 is assigned exactly once; each `PENDING` falsifier is discharged in its owning slice.
- [x] Every slice has all thirteen mandatory fields; conditional fields use `N/A — reason`.
- [x] Every claim's deterministic fence and named mutation land in the implementing slice; there are no approved-risk fence omissions.
- [x] Every new loop records asymptotic cost, production-scale bound, and explicit accepted maximum; every always-on phase has a wall budget.
- [x] Base 3,600 + 20% churn 720 = 4,320, so two independently mergeable increments are defined and every slice names one.
- [x] All future-work phrases cite verified tracker coverage; permanent non-goals carry rationale.
- [x] No slice is declared complete here; checkpointed-build owns completion.
- [x] Cross-platform path behavior is exercised with filesystem-shaped relative `gitdir`/`commondir` fixtures on each native test host; platform-specific hashing uses `cfg`-appropriate standard-library encodings. Local Linux proof is not misreported as native Windows proof; CI is the native Windows/macOS behavioral gate.
- [x] Error branches enumerated in the input/stress matrices distinguish missing, unreadable, invalid, unavailable, not-found, auth, timeout, corrupt, and unsupported states; errors preserve typed sources at module seams.
