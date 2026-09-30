# Plan: cyril-lki9

Inputs: `design.md` (APPROVED 2026-09-29, "yes"; no risk acceptances), `spec.md` (B1–B7), `evidence.md` (P1–P7), `route.md` (Empirical).

## Design verification (step 1)

- Falsification table: 22 rows (C1–C22), every cell filled; no `FAIL`; cheapest falsifier C8 **PASS (model)** with positive control; every other row `PENDING — checkpointed-build`, assigned below.
- Module shape: ledger, two protected parents (`app.rs` ≤ +60 prod, `state.rs` ≤ +70 prod), shape claim C21 with mutations M21/M21b; Length review `N/A — no trigger` (no repository length gate; precedent `.cyril-6bwr`, `.cyril-ell0`).
- Every row is mechanical enough to plan against. Note for build: M8's expected red output is recorded generally in the design ("forwards / busy"); the C8 Rust table test must print the failing scenario name and the observed `{active, forwarded, companion}` (localization rule).

## Module growth ledger

Baseline = production lines before the first `#[cfg(test)]` at merge-base `bfc498b1`.

| Module | Baseline | Projected final | Responsibility change | Interface change | Protected-parent rule |
|---|---:|---:|---|---|---|
| `cyril-core/src/protocol/convert/kas.rs` | 616 | 680–760 | + `turn_start`, `notify-*` classification, cleared filtering, `agent_initiation` helper | `session_info_to_notification` outputs; crate-private `agent_initiation` | N/A |
| `cyril-core/src/types/event.rs` | 719 | 770–830 | + 3 `Notification` variants, `AgentInitiation`, `RoutedNotification.origin` | new variants / field / `with_origin` | N/A |
| `cyril-core/src/protocol/engine.rs` | 352 | 370–400 | + defaulted `turn_origin` | new trait method (defaulted) | N/A |
| `cyril-core/src/protocol/domain_mediator/inbound.rs` | 196 | 225–275 | + origin stamping, `BeginServerTurn` apply, announce emission | none external | N/A |
| `cyril-core/src/protocol/domain_mediator/mod.rs` | 763 | 763–780 | liveness begin for server turns may land here | none | N/A |
| `cyril-core/src/protocol/turn_mediator.rs` | 370 | 450–540 | + server turns, bracket attach, announce-once | `observe(.., main)` / `Disposition::BeginServerTurn` / `announce` | N/A |
| `cyril-core/src/session.rs` | 412 | 415–425 | + `TurnStarted` → Busy | none | N/A |
| `cyril-core/src/workflow.rs` | 1,095 | 1,140–1,190 | + completion log, `take_wake_label` | new method + `WakeLabel` | N/A |
| `cyril-core/src/protocol/convert/kas/workflow.rs` | 1,116 | 1,118–1,130 | + `runLabel` field | snapshot gains `run_label` | N/A |
| `cyril-ui/src/turn_labels.rs` | 0 | 60–120 | create: all header/notice strings | crate-private fns | N/A |
| `cyril-ui/src/subagent_ui.rs` | 244 | 255–275 | + header/notice insert per stream | two methods | N/A |
| `cyril-ui/src/state.rs` | 2,983 | ≤ 3,064 | thin delegation + arms + C6 narrowing | two methods | **protected: ≤ +80 prod (raised from +70, Length review 2026-09-30, reshape → cyril-dgyz); no header/notice literals** |
| `crates/cyril/src/app.rs` | 2,939 | ≤ 2,999 | routing arms only | none | **protected: ≤ +60 prod; no `notify`/`agentInitiated`/header literals** |
| `cyril-core/src/protocol/convert/mod.rs` | 530 | 530 | none | none | shape fence: prod delta 0 |
| `cyril-ui/src/widgets/chat.rs` | 569 | 569 | none (reuses `System`) | none | N/A |

## Partition arithmetic

- Already on the branch vs merge-base (`bfc498b1..c2724e78`): **2,690** added lines, all under `.cyril-lki9/` (1,558 captures/verdicts/logs).
- Remaining workflow artifacts (this plan, `oracles/shape.py`, build logs): ~600.
- Slice diff estimates (code + tests + fixtures): S1 350, S2 290, S3 505, S4 245, S5 170, S6 240, S7 480, S8 70, S9 230, S10 350 = **2,930**.
- Churn margin: **30 %** on code slices (+880) — the mediator/harness work (S3, S4, S10) has the most uncertainty (race table, stall timing harness, replay loader), and prior gilfoyle plans in this repo drifted upward; artifacts get 10 % (+330, captures are fixed, docs grow).
- Total projected: 2,690 + 600 + 2,930 + 880 + 330 = **7,430 > 4,000** → partition required.

| Increment | Slices | Projected (with margin) | Mergeable definition | Verifies without later increments by |
|---|---|---:|---|---|
| **I0 — evidence** | none (artifacts only) | ~3,620 | touches only `.cyril-lki9/**`; no production or test change | `git diff --name-only <default>...` ⊆ `.cyril-lki9/`; oracles runnable (`python3 .cyril-lki9/oracles/mediator_model.py` exit 0) |
| **I1 — core turn model** | S1, S2, S3, S5, S4, S6 | 1,800 + 540 = ~2,340 | wake turns complete (no stuck Streaming), are busy (typing steers, Esc cancels), stall watchdog armed, `AgentInitiatedTurn` / `EngineMessageInjected` / `TurnStarted` routed to no-op UI arms; `notify-*` no longer consumes operator steers at the converter | core table/harness tests + SessionController/App tests; v2 suite unchanged |
| **I2 — labels and notices** | S7, S8, S9, S10 | 1,130 + 340 = ~1,470 | headers/notices rendered; C6 UI narrowing; end-to-end replay | UI/App tests + replay of committed captures |

S3 and S5 must share an increment: S3 alone makes the mediator busy during a wake while `SessionController` is not, so Enter would dispatch `SendPrompt` into the busy guard (error) instead of today's pre-emption.

Scoped shipping authorization (2026-09-29, requester selected "Draft PR per increment"): open a **draft** PR for each increment at its boundary — I0 from `fix/cyril-lki9-workflow-auto-wake`, I1 and I2 on stacked branches; **no merge without requester approval**; tracker records to close on the final merge: cyril-lki9, cyril-5n75.

Branch discovery: the default branch is resolved with `git symbolic-ref refs/remotes/origin/HEAD` at PR time (not hard-coded).

---

## Slice 1: KAS converter recognizes `turn_start` and engine injections; shape fence created

**Claim IDs:** C1, C4, C5, C21 (fence creation; runs at every later checkpoint)
**Expected behavior:** `turn_start` → `Notification::TurnStarted`; `steering_injected` with `notify-*` id → `EngineMessageInjected{message_id, content, severity}`; `steering_cleared` strips `notify-*` ids and emits nothing when none remain; App/UiState/SessionController accept the new variants as no-ops (explicit arms, no behavior).
**Oracle:** C1: `grep -c '"kind": "turn_start"'` per capture vs converted `TurnStarted` count. C4/C5: expected variants hand-derived from P6 frame ids and the spec B6 table. C21: `git diff --numstat` + `git grep` computed outside `shape.py`.
**Stress fixture:** a `steer-` id whose content is `"[notification/success] fake"` (content must not drive classification) → `SteeringConsumed`; `steering_cleared{["notify-a","notify-wf-b"]}` → `None` (not `Some([])`); `steering_cleared{[]}` → `SteeringCleared{[]}`; `messageId: "notify"` (bare prefix without dash) → treated as operator steer (prefix is `notify-`).
**Regression fence:** unit tests in `crates/cyril-core/src/protocol/convert/kas.rs` (`turn_start_converts_to_turn_started`, `notify_injection_is_engine_message`, `cleared_strips_notify_ids`); `.cyril-lki9/oracles/shape.py` (created here).
**Named mutation:** M1 (delete `turn_start` arm), M4 (route `notify-` to `SteeringConsumed`), M5 (forward filtered empty list), M21 (`"notify-"` literal in `app.rs`).
**Complexity/production scale:** `notify-` filter over `messageIds`: O(k), k ≤ ids per frame (observed 1); no cap needed — frame size bounded by KAS.
**Wall budget/phase:** always-on (per frame); deterministic, no latency obligation: `N/A — reason: constant-time classification, fenced by unit tests`.
**Module shape:** adds KAS literals to `convert/kas.rs` (owner); `event.rs` gains variants; protected parents: `app.rs` +≤6 (no-op arm), `state.rs` +≤6 (no-op arm); `python3 .cyril-lki9/oracles/shape.py` → `C21 PASS`.
**Files:** `crates/cyril-core/src/protocol/convert/kas.rs`, `crates/cyril-core/src/types/event.rs`, `crates/cyril-core/src/session.rs` (arm only if the match is exhaustive), `crates/cyril-ui/src/state.rs` (no-op arm), `crates/cyril/src/app.rs` (no-op arm), `.cyril-lki9/oracles/shape.py`.
**Estimate:** 1.5 h
**Diff estimate:** 350
**PR increment:** I1
**Commands and expected results:**
- `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core -E 'test(/turn_start|notify|cleared_strips/)'` → the three fences pass; the stress-fixture rows produce the stated variants.
- apply M1 / M4 / M5 → the matching fence fails naming its row; restore → green.
- `python3 .cyril-lki9/oracles/shape.py` → C21 PASS; apply M21 → `C21 FAIL crates/cyril/src/app.rs:<n> forbidden literal "notify-"`; restore → PASS.
- `env -u CARGO_TARGET_DIR cargo clippy --all-targets --all-features -- -D warnings` → clean.

### Checkpoint record — Slice 1 (2026-09-29, branch `fix/cyril-lki9-i1-core-turns`)

**Impact analysis (step 1).** `session_info_to_notification` — sole caller `KasEngine::convert_session_update` (`protocol/engine.rs:315`); signature unchanged, outputs widened. New `Notification` variants: exhaustive matches found by `cargo check --workspace --all-targets --all-features` — `crates/cyril-ui/src/state.rs` `apply_notification` (explicit no-op arm added) and `crates/cyril/examples/test_bridge.rs` printer (arms added); `SessionController` and App route through existing documented catch-alls (no change). Semantic change for `SteeringConsumed` consumers (`UiState::flip_consumed_steer_echo`, App): they no longer receive `notify-*` ids — intended (C4, cyril-5n75).

**Gate.**
1. Affected unit tests — PASS: `cargo nextest run --workspace --all-features` → 2087 passed, 13 skipped (incl. the existing `steering_kinds_degrade_never_drop`).
2. Falsifiers C1/C4/C5 — PASS: `turn_start_converts_to_turn_started`, `notify_injection_is_engine_message`, `cleared_strips_notify_ids`, `lki9_traces_match_raw_frame_census`. C21 — PASS (`shape.py`).
3. Stress fixture — PASS: `steer-` id with `[notification/success]` content → `SteeringConsumed{steer-1}`; bare `notify` id → `SteeringConsumed`; empty id → `SteeringConsumed{None}`; empty severity → `None`; `[notify-a, notify-wf-b]` → `None`; `[]` → drain-all kept.
4. Implementation vs oracle — PASS: converter counts over the 5 committed traces equal the independent `grep` census exactly — (TurnStarted, EngineMessageInjected, SteeringConsumed, SteeringCleared) = tail (3,2,0,0), gate-on (4,1,0,0), busy (2,1,0,0), cancel (2,1,0,0), steer (2,0,1,1). Shape: script deltas (app.rs 0, state.rs +5) = `git diff --numstat`; `git grep` literal hits outside `kas.rs` are a doc comment and `hook.rs` test code only.
5. Module shape — PASS: `python3 .cyril-lki9/oracles/shape.py` → `C21 PASS (base bfc498b1 on origin/main; protected-parent prod deltas {app.rs: 0, state.rs: 5})`.
6. Budget — N/A — reason: plan records no production-scale or wall budget for this slice (O(k) id filter, k ≤ ids per frame).
7. Regression fence — PASS (same runs as item 2).
8. Named mutation — PASS (red): M1 (delete `turn_start` arm) → `turn_start_converts_to_turn_started` + census FAIL; M4 (`if false &&` on the notify branch) → `notify_injection_is_engine_message` + census FAIL; M5 (forward filtered empty list) → `cleared_strips_notify_ids` + census FAIL; M21 (`"notify-"` const in `app.rs`) → `C21 FAIL crates/cyril/src/app.rs:32 R1`; M21b (+80 lines `state.rs`) → `C21 FAIL … R3: prod delta 85 > 70`.
9. Fence restored — PASS: files restored from scratch copies (`cmp` identical, never `git checkout`); all four tests + shape fence green.
10. Parity and reuse — PASS. Search: `grep` over `crates/` + manifests. Reused: `steering_text` / `steering_message_id` / `steering_message_ids` (`convert/kiro.rs`, shared degrade discipline) for the new branch; `crate::test_support::must_succeed`; new `test_support::kas_trace_to_routed` is a thin sibling of `kas_recording_to_routed` delegating to the production `kas_capture_to_routed` (envelope differs: `{ts,dir,msg}` vs recorder `parsed`) — `capture_frames` (private to `convert/kas/workflow.rs` tests, `parsed` unwrap only) not reused because it does not convert. Severity read uses `.filter(|s| !s.is_empty())`, the same empty-means-absent rule `steering_message_id` applies. Symmetry audit (new engine-injection branch beside `SteeringConsumed`): error handling — same `steering_text` degrade for content; logging — the notify-only `steering_cleared` drop logs at `debug` (an expected frame, not drift), while the helper's existing drift warns are unchanged; fallback — id absent/empty keeps the legacy operator path; caller observability — distinct variant; guard parity — classification on id only, content never consulted (stress row). Acceptance change: `steering_cleared` naming only `notify-*` ids was previously accepted as an id-scoped clear (whose unknown ids fell back to id-less chips); now it is dropped — control rows `[notify-a, steer-b]` and `[]` fence both neighbours.
11. Preserved enforcement — N/A — reason: no gate, fence, validator, oracle or policy file repointed, relaxed or deleted; the existing steering tests run unchanged.

Sweep (step 7): `session_info_to_notification` doc list updated (`turn_start`, engine injections); no forward references; tracker phrases in code cite cyril-lki9/cyril-5n75 only via design claim ids. New intra-doc links resolve (`cargo doc … | grep TurnStarted|EngineMessageInjected` empty; the crate's pre-existing broken links elsewhere are unrelated).

## Slice 2: agent-initiated tag carried as envelope metadata

**Claim IDs:** C2, C3, C19
**Expected behavior:** `Engine::turn_origin` (default `None`; KAS parses `_meta.kiro.agentInitiated/agentInitiatedReason` on chunk/thought/tool_call/tool_call_update); `inbound.rs` stamps `RoutedNotification.origin`; v2 always `None`.
**Oracle:** P1/P2 capture counts (tail 57/2/4; gate-on 292/6/8) vs `turn_origin` Some-counts; harness script's own sent-frame list for C3.
**Stress fixture:** `agentInitiated: "true"` (string, not bool) → `None`; `_meta.kiro` = array → `None`; reason present but `agentInitiated` absent → `None`; tagged `AgentThoughtChunk` → `Some`; v2 engine on a tagged frame → `None`.
**Regression fence:** `convert/kas.rs` table test + capture-count test (reads the two committed captures via the existing `capture_frames()` helper pattern); bridge harness test `origin_is_stamped` in `domain_mediator/tests` / `bridge/tests/current_runtime_contract/routing.rs`; `engine.rs` v2 unit test.
**Named mutation:** M2 (ignore `ToolCallUpdate`), M3 (remove `.with_origin`), M19 (default `turn_origin` parses `_meta`).
**Complexity/production scale:** O(1) per frame (fixed-path JSON lookups).
**Wall budget/phase:** always-on; deterministic: `N/A — reason: constant-time lookup fenced by unit tests`.
**Module shape:** `engine.rs` gains defaulted method (owner of per-engine dispatch); `convert/kas.rs` owns parsing; `event.rs` envelope field; `app.rs` destructure update (+≤2 lines, `origin: _`); `shape.py` → PASS (no `agentInitiated` literal outside `convert/kas.rs`).
**Files:** `crates/cyril-core/src/protocol/engine.rs`, `crates/cyril-core/src/protocol/convert/kas.rs`, `crates/cyril-core/src/types/event.rs`, `crates/cyril-core/src/protocol/domain_mediator/inbound.rs`, `crates/cyril/src/app.rs`, harness test file.
**Estimate:** 1.5 h
**Diff estimate:** 290
**PR increment:** I1
**Commands and expected results:**
- `cargo nextest run -p cyril-core -E 'test(/turn_origin|origin_is_stamped/)'` → table rows as specified; capture counts equal 57/2/4 and 292/6/8 exactly.
- M2 → capture-count row red (tool_call_update 4 ≠ 0); M3 → harness red; M19 → v2 row red; each restored → green.

### Checkpoint record — Slice 2 (2026-09-29, branch `fix/cyril-lki9-i1-core-turns`)

**Impact analysis (step 1).** New trait method `Engine::turn_origin` (defaulted): implementors `V2Engine` (inherits `None`), `KasEngine` (override) — `grep 'impl Engine for'` finds only these two. `RoutedNotification` gained a `pub origin` field: struct-literal / exhaustive-pattern sites found by `cargo check --workspace --all-targets --all-features` — `crates/cyril/src/app.rs:1221` (destructure; bound explicitly per the file's no-`..` convention) and `:1363` (pending-buffer re-queue; origin preserved); `domain_mediator/mod.rs:586` uses a `..`-free match on `DomainWork::Routed` and compiled unchanged; constructors `global`/`scoped` set `None`. New stamp site: `domain_mediator/inbound.rs::handle_session` (sole session/update conversion site).

**Gate.**
1. Affected unit tests — PASS: `cargo nextest run --workspace --all-features` → 2091 passed, 13 skipped.
2. Falsifiers C2/C3/C19 — PASS: `turn_origin_table`, `turn_origin_matches_tagged_frame_census`, `origin_is_stamped_on_the_frame_it_came_on`, `v2_turn_origin_is_always_none`.
3. Stress fixture — PASS: string `"true"` → None; `kiro` array → None; reason without flag → None; empty reason → `Some(reason None)`; tagged `agent_thought_chunk` → Some; tagged `session_info_update` → None; v2 on a tagged frame → None (positive control: KAS → Some).
4. Implementation vs oracle — PASS: `turn_origin` over every agent->client `session/update` in the two committed captures equals the P1/P2 grep census exactly — tail (57 chunk, 2 tool_call, 4 tool_call_update; workflow-complete-wake 63), gate-on (292, 6, 8; send-message-wake 105, workflow-complete-wake 201).
5. Module shape — PASS: `python3 .cyril-lki9/oracles/shape.py` → `C21 PASS (… app.rs: 5, state.rs: 5)`; `agentInitiated` literals only in `convert/kas.rs` production code.
6. Budget — N/A — reason: O(1) fixed-path lookups; plan records no budget.
7. Regression fence — PASS (same runs as item 2).
8. Named mutation — PASS (red): M2 (drop `ToolCallUpdate` arm) → `turn_origin_table` + census FAIL; M3 (`let _ = origin` instead of `with_origin`) → `origin_is_stamped_on_the_frame_it_came_on` FAIL; M19 (default `turn_origin` parses `_meta`) → `v2_turn_origin_is_always_none` FAIL.
9. Fence restored — PASS: three files restored from scratch copies (`cmp` identical); four fences green.
10. Parity and reuse — PASS. Search: `grep` for `_meta`/`agentInitiated` readers in `convert/` (only `SessionMode.welcomeMessage`, trust options — none reusable) and `test_support` loaders. `AgentInitiation::new` applies the empty-means-absent rule shared with `steering_message_id`; the census test deserializes at the acp layer exactly as `kas_capture_to_routed` does (no origin is exposed by that helper, so it could not be reused for the origin tally — divergence justified by C2's need to observe `turn_origin`, not the converted content). Harness: `Script.chunk_meta` extends the existing chunk emitter rather than adding a second emitter. Symmetry: the new `turn_origin` hook parallels `convert_session_update` (same `&acp::SessionNotification` input, same engine dispatch); the defaulted-vs-undefaulted divergence from `emits_wire_turn_end` is justified in the trait doc (a wrong inherited `None` loses only a label).
11. Preserved enforcement — N/A — reason: no gate/fence/policy repointed, relaxed or removed.

Sweep (step 7): three forward-looking statements rewritten to cite their design claims (event.rs `origin` doc → C10/Slice 4; engine.rs default rationale → C7; app.rs binding comment → C10) — comment-only, evidence retained.

## Slice 3: mediator owns server-started turns (fixes the dropped wake completion)

**Claim IDs:** C7, C8, C9, C12, C22
**Expected behavior:** idle + main `TurnStarted` → server turn (busy, liveness begun); its unstamped `turn_end` → one `TurnCompleted`; cyril turn attaches the first wire bracket; foreign `TurnStarted` → `Forward`; `CancelRequest` targets the server turn's session; stall watchdog fires during a silent server turn.
**Oracle:** `.cyril-lki9/oracles/mediator_model.py` (proposed rules, 10 scenarios); P4 live leg (cancel); P5 prompt leg (silent wake).
**Stress fixture:** the 10 model scenarios as a Rust table (incl. `race D@0 tail=SRE`, `race D@2 tail=SER`, `wake-then-dispatch-rejected`, `two-wakes`); a foreign `TurnStarted` while a main server turn is active; a duplicate `turn_end` after release → `DropUnowned`.
**Regression fence:** table test `server_turn_scenarios` in `crates/cyril-core/src/protocol/turn_mediator.rs` (prints scenario name + `{active, forwarded, companion}` on failure); bridge harness tests `cancel_targets_server_turn` and `stall_fires_during_server_turn` (`stall.rs` pattern, short `stall_threshold`).
**Named mutation:** M7 (idle `TurnStarted` → `Forward`), M8 (drop the attach rule), M9 (drop main-session check), M12 (`cancel_active` ignores server turns), M22 (skip `turn_liveness.begin` on `BeginServerTurn`).
**Complexity/production scale:** O(1) per frame; state = one active turn + one companion (unchanged bound).
**Wall budget/phase:** always-on; deterministic (fenced by the table test); stall test uses a virtual/short threshold, `N/A — reason: no latency obligation`.
**Module shape:** `turn_mediator.rs` deepened (owner of turn ownership); `inbound.rs` applies the new disposition and begins liveness; `domain_mediator/mod.rs` passes the main session (`active_session_id`) to `observe`; no protected-parent change; `shape.py` → PASS.
**Files:** `crates/cyril-core/src/protocol/turn_mediator.rs`, `crates/cyril-core/src/protocol/domain_mediator/inbound.rs`, `crates/cyril-core/src/protocol/domain_mediator/mod.rs`, `crates/cyril-core/src/protocol/domain_mediator/commands/session.rs` (only if `cancel_active` needs the server-turn session), bridge harness test files.
**Estimate:** 3 h
**Diff estimate:** 505
**PR increment:** I1
**Commands and expected results:**
- `cargo nextest run -p cyril-core -E 'test(/server_turn|cancel_targets_server_turn|stall_fires_during_server_turn/)'` → all 10 scenarios end idle, forwards = brackets, no companion; cancel reaches `session/cancel{sessionId: main}`; `TurnStalled` scoped to main.
- `python3 .cyril-lki9/oracles/mediator_model.py` → 10/10 (oracle agreement: each Rust scenario's disposition log matches the model's `log` column item by item).
- M7/M8/M9/M12/M22 each → the named test red with the scenario/state printed; restore → green.

### Checkpoint record — Slice 3 (2026-09-29, branch `fix/cyril-lki9-i1-core-turns`)

**Impact analysis (step 1).** `TurnMediator::observe` signature gained `main: Option<&SessionId>` — callers: `domain_mediator/inbound.rs::handle_routed_with_source_disposition` (sole production caller; passes `self.active_session_id.as_ref()`) and 28 unit-test calls in `turn_mediator.rs` (migrated to `main = None`, behavior-neutral: `main` only affects `TurnStarted`). `Disposition` gained `BeginServerTurn`: exhaustive matches — `inbound.rs` (the only one; arms liveness). `ActiveTurn` gained `origin`/`bracket_open`; the companion registration on unstamped release now depends on origin. `active_turn_session()` callers (cancel target `commands/session.rs:249`, liveness stamp `inbound.rs:41`) now see server turns too — intended (C12/C22).

**Qualification finding (resolved in-slice).** The first run of `server_turn_scenarios_match_the_model` disagreed with the independent model on `race D@0 tail=SER`: the server-turn release ASSIGNED `companion = None`, wiping the pre-empted dispatch's owed synthesized twin, so its prompt response became `DropStale` instead of `Absorb`. Cause: implementation wrong (model rule: a server release registers nothing and leaves the ledger untouched). Fixed; the comment at the release arm records why.

**Gate.**
1. Affected unit tests — PASS: `cargo nextest run --workspace --all-features` → 2096 passed, 13 skipped (all 51 pre-existing mediator + bridge-contract tests unchanged and green).
2. Falsifiers C7/C8/C9/C12/C22 — PASS: `server_turn_scenarios_match_the_model`, `server_turn_owes_no_companion`, `foreign_turn_start_is_forwarded`, `cancel_targets_server_turn`, `stall_fires_during_server_turn`.
3. Stress fixture — PASS: 10 model scenarios incl. both race orderings and `wake-then-dispatch-rejected`; duplicate `turn_end` after a server release → `DropUnowned`; foreign `TurnStarted` idle and during a main server turn → `Forward`, main untouched; `TurnStarted` with no main known → `Forward`.
4. Implementation vs oracle — PASS: every scenario's disposition log equals `.cyril-lki9/oracles/mediator_model.py`'s log item by item (`cyril#N` ids normalized; the model allocates ids only for dispatches); model rerun 10/10.
5. Module shape — PASS: `C21 PASS (… app.rs: 5, state.rs: 5)`. R1 initially FAILED on four `tracing` messages containing `turn_start` in `turn_mediator.rs`; fixed by rewording to the internal variant name `TurnStarted` (the fence was NOT relaxed).
6. Budget — N/A — reason: O(1) per frame; state bounded to one active turn + one companion (unchanged).
7. Regression fence — PASS (item 2 runs).
8. Named mutation — PASS (red): M7 (idle `TurnStarted` → `Forward`) → all five FAIL; M9 (`main.is_some()` instead of `main == Some(session)`) → `foreign_turn_start_is_forwarded` FAIL; M12 (`active_turn_session` hides server turns) → `cancel_targets_server_turn` (+3) FAIL; M22 (skip `turn_liveness.begin`) → `stall_fires_during_server_turn` FAIL. **M8 (drop the attach rule) initially stayed GREEN — blind fence**: the table inferred `ATTACH` from the pre-state. Repaired (log `ATTACH` only when the bracket actually opened on that frame) — the asserted behavior is unchanged and stricter; M8 then red (`got [BEGIN cyril, FORWARD nested-start, …] want [BEGIN cyril, ATTACH, …]`), and M7 re-proved red against the repaired fence.
9. Fence restored — PASS: all files restored from scratch copies (`cmp` identical); five fences green.
10. Parity and reuse — PASS. Search: `grep`/reading of `turn_mediator.rs`, `harness.rs`. Server-turn allocation reuses `TurnAllocator::allocate` and mirrors `BeginTurn::Exhausted`'s refusal (warn + not tracked). Harness: `cancelled_sessions` is a separate ledger (the exact-order `received` ledger asserted by `commands.rs` stays untouched); `inject_and_see` reuses `InboundProbe::send` + `recv_notif`; the stall fence copies `stall.rs`'s paused-time pattern. Symmetry audit — server turn beside dispatched turn: error handling — same exhaustion refusal; logging — `debug` for begin/attach, `warn` only for the unexpected nested start (KAS pre-empts, never nests); fallback — unknown/foreign sessions forward untouched, as foreign terminals already did; observability — distinct `BeginServerTurn` disposition; guard parity — the busy guard (`begin_turn` → `Busy`) now also covers server turns (a dispatch during a wake is refused; design C8 scenario `wake-then-dispatch-rejected`). Acceptance change: a terminal with no dispatched turn but an open server turn on that session is now forwarded (was `DropUnowned`) — controls: foreign-session and post-release duplicate rows.
11. Preserved enforcement — PASS: no gate relaxed. The shape fence R1 failure was fixed in the code, not the fence; the mediator's pre-existing drop/absorb dispositions keep their fences (all pre-existing tests green, unchanged apart from the mechanical `main = None` argument).

Sweep (step 7): `ActiveTurn` doc updated; CONTEXT.md "Turn owner" / "Companion terminal" / "Turn mediation" updated (agent-initiated turns); ADR-0004 amended (dated) for the `observe` signature and server turns; the release-arm comment rewritten (the old "every turn has a prompt RPC" invariant is gone).

## Slice 5: session is Busy during a server turn (Enter steers, Esc cancels)

**Claim IDs:** C11
**Expected behavior:** `SessionController` Busy on `TurnStarted`, Active on `TurnCompleted`; App: Enter with text after `TurnStarted` dispatches `SteerSession`; Esc dispatches `CancelRequest`.
**Oracle:** P5 live evidence (steer accepted during a wake; prompt would pre-empt it).
**Stress fixture:** `TurnStarted` while already Busy (cyril-owned turn) → stays Busy, no double transition; `TurnCompleted` without a prior `TurnStarted` (v2 path) → Active as today.
**Regression fence:** `session.rs` unit tests; App test in `crates/cyril/src/app/tests/current_runtime_contract/` asserting the dispatched `BridgeCommand` kind.
**Named mutation:** M11 (remove the `TurnStarted` arm).
**Complexity/production scale:** `N/A — reason: no loop`.
**Wall budget/phase:** `N/A — reason: state transition, no latency obligation`.
**Module shape:** `session.rs` one arm (owner); `app.rs` unchanged in production (classify_submit already keys on status); `shape.py` → PASS.
**Files:** `crates/cyril-core/src/session.rs`, App test file.
**Estimate:** 1 h
**Diff estimate:** 170
**PR increment:** I1 (must ship with Slice 3)
**Commands and expected results:**
- `cargo nextest run -p cyril-core -p cyril -E 'test(/turn_started_busy|enter_steers_during_server_turn/)'` → Busy/Active transitions; `SteerSession` dispatched, no `SendPrompt`.
- M11 → App test red (`SendPrompt` observed); restore → green.

### Checkpoint record — Slice 5 (2026-09-29, branch `fix/cyril-lki9-i1-core-turns`)

**Impact analysis (step 1).** `SessionController::apply_notification` gained a `TurnStarted` arm (before: the catch-all). Consumers of `SessionStatus::Busy`: `classify_submit` (`app.rs:2321`, Enter → steer vs prompt), the Esc arm (`app.rs:1729`, cancel), and the App's own `set_status` calls at dispatch (unchanged). No signature change.

**Gate.**
1. Affected unit tests — PASS: `cargo nextest run --workspace --all-features` → 2098 passed, 13 skipped.
2. Falsifier C11 — PASS: `session::tests::turn_started_busy_transitions`, `app::tests::enter_steers_during_server_turn`.
3. Stress fixture — PASS: `TurnStarted` while Busy → no transition (`false`); `TurnCompleted` without a prior `TurnStarted` (v2 path) → Active as before; Error → Busy (a new turn clears a prior failure); Disconnected / Initializing / Compacting not overridden.
4. Implementation vs oracle — PASS: evidence P5 (live, `.cyril-lki9/lki9-live-{steer,prompt}-06615-2.26.0.jsonl`) — `_session/steer` during a wake is accepted and honored, `session/prompt` pre-empts it; after this slice the App dispatches `SteerSession` during a server turn (the accepted path), never `SendPrompt` (the pre-empting one); control run dispatches `SendPrompt` when idle.
5. Module shape — PASS: `C21 PASS (… app.rs: 5, state.rs: 5)` — `app.rs` production unchanged by this slice (the test is in `mod tests`).
6. Budget — N/A — reason: no loop; state transition only.
7. Regression fence — PASS (item 2).
8. Named mutation — PASS (red): M11 (`if false && …` on the `TurnStarted` arm) → both fences FAIL (the App fence observes a non-steer dispatch).
9. Fence restored — PASS: `session.rs` restored from scratch copy (`cmp` identical); both green.
10. Parity and reuse — PASS. Search: `grep` for `SessionStatus::Busy` setters. The new arm mirrors `TurnCompleted`'s unconditional `Active` with a guarded `Busy` (divergence justified in the arm comment: connection/compaction phases have their own exit paths). App test reuses `test_app_with_command_rx`, `session_created_frame`, `key`, `insert_text` (the `esc_marks_cancel_sent_during_stall` pattern). Symmetry: Enter/Esc paths are the existing `classify_submit` / Esc arm, now reachable during server turns — no new branch.
11. Preserved enforcement — N/A — reason: nothing repointed or relaxed.

Sweep (step 7): no prose describes "Busy only after dispatch"; the new arm's comment states the current rule. Placement note: the unit test was first appended into `thinking_tests`, then `kas_hook_tests`, by an end-of-file insertion; relocated to `session::tests` before commit.

## Slice 4: announce the agent-initiated turn once

**Claim IDs:** C10
**Expected behavior:** per session per wire turn, exactly one `AgentInitiatedTurn{reason}` forwarded immediately before the first tagged frame; re-armed at `turn_start`/`turn_end`; untagged turns produce none; App routes it to a no-op arm (labels arrive in I2).
**Oracle:** `probe_p1_p7.py` per-turn table (which turns on which sessions are tagged, and their reasons).
**Stress fixture:** 57 tagged frames in one turn → 1 announce; two sessions interleaving tagged frames → one each, in order; tagged frames on a step session with no main turn → announced for that session; a turn whose tag appears only on its 3rd frame → announce lands before frame 3, after frames 1–2.
**Regression fence:** bridge harness test `announce_once_per_turn` (+ `turn_mediator.rs` unit for the flag).
**Named mutation:** M10 (never reset the announced flag at `turn_end`).
**Complexity/production scale:** per-session flag map: O(1) per frame; entries = sessions with an open turn (bounded by live sessions, observed ≤ 3); cleared at `turn_end`.
**Wall budget/phase:** always-on; deterministic: `N/A — reason: constant-time, fenced`.
**Module shape:** `turn_mediator.rs` (flag), `inbound.rs` (emission); `app.rs` +≤4 (no-op arm); `shape.py` → PASS.
**Files:** `crates/cyril-core/src/protocol/turn_mediator.rs`, `crates/cyril-core/src/protocol/domain_mediator/inbound.rs`, `crates/cyril-core/src/types/event.rs`, `crates/cyril/src/app.rs`, harness test file.
**Estimate:** 1.5 h
**Diff estimate:** 245
**PR increment:** I1
**Commands and expected results:**
- `cargo nextest run -p cyril-core -E 'test(/announce_once/)'` → counts and positions as in the stress fixture; agreement with the probe's per-turn table for both captures.
- M10 → `two-wakes` row red (1 ≠ 2); restore → green.

### Checkpoint record — Slice 4 (2026-09-29, branch `fix/cyril-lki9-i1-core-turns`)

**Impact analysis (step 1).** New `Notification::AgentInitiatedTurn(AgentInitiation)`: exhaustive matches — `UiState::apply_notification` (joined the explicit lki9 no-op group) and the `test_bridge` printer; App/SessionController/subagent routing use catch-alls (routing by scope unchanged). `TurnMediator` gained `announced` + `announce()`; `observe` now clears the set on `TurnStarted`/`TurnCompleted` (no disposition change). `inbound.rs` emits the announcement before forwarding a frame. `test_support`: `kas_capture_to_routed` / `kas_trace_to_routed` now map over new routed cores `kas_capture_routed` / `kas_trace_routed` (origin kept); signatures unchanged; 40 existing capture-replay consumers re-run green.

**Gate.**
1. Affected unit tests — PASS: `cargo nextest run --workspace --all-features` → 2100 passed, 13 skipped.
2. Falsifier C10 — PASS: `announce_once_per_turn` (bridge harness, real inbound path), `announce_matches_tagged_turn_census`.
3. Stress fixture — PASS: tag first on a turn's 3rd frame → ANNOUNCE lands after p1/p2, before t0; 57 tagged frames → one ANNOUNCE; next turn re-armed; start-less turn re-armed by the previous END (its own END is unowned → dropped, as before); a turn after a missed END re-armed by its START; untagged turn → none; main and `child-7` interleaved → one each, scoped to its own session, in order.
4. Implementation vs oracle — PASS: announcements replayed from both committed captures equal the `probe_p1_p7.py` per-turn table — tail `[(sess_dd72baef…, workflow-complete-wake)]`; gate-on `[(sess_f6407f82… main, workflow-complete-wake), (sess_f7800342… step, send-message-wake)]`.
5. Module shape — PASS: `C21 PASS (… app.rs: 5, state.rs: 7)`.
6. Budget — PASS: per-session `HashSet`, O(1) per frame; entries = sessions with an announced turn in flight (≤ 2 in every capture); cleared at turn start/end.
7. Regression fence — PASS (item 2).
8. Named mutation — PASS (red): M10 (drop the END reset) → `announce_once_per_turn` FAIL; **M10b (added; drop the START reset)** → FAIL. First M10 run stayed green on a fully bracketed fixture (START reset masked it) — blind fixture repaired with two isolating rows; recorded as a technical proof correction in design.md. An intermediate fixture expectation (x1's END forwarded) was wrong — the mediator correctly drops an unowned END; expectation corrected with a comment, code unchanged.
9. Fence restored — PASS: `turn_mediator.rs` restored from scratch copy (`cmp` identical); both fences green.
10. Parity and reuse — PASS. Search: `grep` in `bridge/tests/`, `test_support.rs`. Reused `routing::message` (widened to `pub(super)`, not copied); `InboundProbe`, `start_session`, `recv`; `test_support` refactor makes one routed core serve both pair-returning helpers (no duplicated JSON-RPC walk). Symmetry: the announcement is scoped exactly like the frame it precedes (same session routing as content); no new error path (`notify` failure propagates like the frame's own `notify`).
11. Preserved enforcement — N/A — reason: nothing repointed or relaxed; `routing::message` visibility widened within the test module only.

Sweep (step 7): `RoutedNotification.origin` doc now present tense (announcement implemented); App binding comment already cites C10.

## Slice 6: tracker yields the oldest unconsumed wake label per parent session

**Claim IDs:** C13
**Expected behavior:** `take_wake_label(s)` returns `{name: runLabel → workflowName → workflowId, status}` for terminal completions parented to `s`, oldest first, each once; `None` otherwise. `runLabel` parsed from snapshots.
**Oracle:** hand table from spec FIFO row + KAS precedence (`runLabel||workflowName||workflowId`, bundle static read).
**Stress fixture:** three completions for `s` (completed, failed, aborted) + one for another parent + one non-terminal (paused) → take ×4 = A, B, C, None; a run re-completing (duplicate `run_complete`) is logged once; `runLabel: ""` → falls through to `workflowName` (empty = absent per CLAUDE.md partial-update rule).
**Regression fence:** unit tests in `crates/cyril-core/src/workflow.rs`; snapshot parse test in `convert/kas/workflow.rs`.
**Named mutation:** M13 (newest instead of oldest).
**Complexity/production scale:** per-parent `VecDeque`: O(1) push/pop; size = completions awaiting a wake per session (observed 1; KAS wakes once per terminal run) — max accepted: unbounded growth is prevented by consumption; if a session never wakes, entries remain until the session's runs are evicted with the tracker's existing lifecycle (no new retention beyond the run map).
**Wall budget/phase:** one-off per run completion: `N/A — reason: one-off phase; no wall budget`.
**Module shape:** `workflow.rs` deepened (owner), `convert/kas/workflow.rs` one optional field; `shape.py` → PASS.
**Files:** `crates/cyril-core/src/workflow.rs`, `crates/cyril-core/src/protocol/convert/kas/workflow.rs`.
**Estimate:** 1.5 h
**Diff estimate:** 240
**PR increment:** I1
**Commands and expected results:**
- `cargo nextest run -p cyril-core -E 'test(/wake_label|run_label/)'` → A, B, C, None; precedence rows as specified.
- M13 → red (B ≠ A); restore → green.

### Checkpoint record — Slice 6 (2026-09-29, branch `fix/cyril-lki9-i1-core-turns`)

**Impact analysis (step 1).** `runLabel` plumbed wire → `WireSnapshot.run_label` → `WorkflowSnapshotMetadata::with_run_label` / `run_label()` → `WorkflowSnapshotParts.run_label` → `WorkflowRun.run_label` (+ `WorkflowSnapshot::run_label()` flat accessor, matching `parent_session_id`/`workspace_path`). `WorkflowRun` struct literals: `canonicalize_snapshot`, `sparse_opening_run`, test `seed` — all updated (compiler-enumerated). `WorkflowRun` equality now includes `run_label` (terminal duplicate detection compares whole runs — a label change on a repeat completion is a conflict, consistent with every other field). `apply_completion` records `(parent, workflow_id)` on a first terminal transition; new `WorkflowTracker::take_wake_label` + `WakeLabel`. No existing caller changed.

**Gate.**
1. Affected unit tests — PASS: `cargo nextest run --workspace --all-features` → 2102 passed, 13 skipped (all workflow tracker/converter tests unchanged and green).
2. Falsifier C13 — PASS: `wake_labels_are_fifo_per_parent`, `run_label_parsed_from_final_state`.
3. Stress fixture — PASS: completions for `s` (Completed, Failed w/ label, Aborted w/ empty label) + another parent + a Paused "completion" + an exact duplicate → take(s) = recipe-A/Completed, labelled-b/Failed, recipe-E/Aborted (empty label falls back), None; take(other) = recipe-C; take(never) = None.
4. Implementation vs oracle — PASS: FIFO/precedence expectations hand-derived from spec (FIFO row) and KAS's static wake policy (`runLabel || workflowName || workflowId`); the live gate-on capture's `finalState.runLabel` = `create-notes-summary-recipe` parses; the cyril-style tail capture has none.
5. Module shape — PASS: `C21 PASS (… app.rs: 5, state.rs: 7)`.
6. Budget — PASS: per-parent `VecDeque`, O(1) push/pop; entries = completions awaiting a header (one per KAS wake); an entry for an evicted run is skipped with a debug log.
7. Regression fence — PASS (item 2).
8. Named mutation — PASS (red): M13 (`pop_back` instead of `pop_front`) → `left: Some(("recipe-E", Aborted)) right: Some(("recipe-A", Completed))`.
9. Fence restored — PASS: `workflow.rs` restored from scratch copy (`cmp` identical); green.
10. Parity and reuse — PASS. Search: `grep` in `workflow.rs` / `types/workflow.rs` tests and accessors. `run_label` follows the existing optional-metadata builder pattern exactly (`with_*` / accessor / parts field / flat snapshot accessor); empty-means-absent applied at the wire (like the other lki9 reads) and again at naming. Test builder `parented_completion` reuses `snapshot_with_status` + `completion` and only rebuilds the metadata (the existing builders cannot set parent/label) — justified divergence. Symmetry: wake labels are fed ONLY from `run_complete` events, not fetched snapshots (`apply_snapshot`) — deliberate: KAS auto-wakes only on `run_complete` (evidence P3/P7); a terminal status learned from a fetched snapshot has no wake to name.
11. Preserved enforcement — N/A — reason: no gate relaxed; terminal-duplicate absorption unchanged.

Sweep (step 7): nothing falsified; the new fields' docs cite cyril-lki9.

## Slice 7: header and notice text; insertion into transcripts; activity on TurnStarted

**Claim IDs:** C15, C16, C17, C18
**Expected behavior:** `turn_labels` builds the approved strings; `UiState::{begin_agent_initiated_turn, show_engine_injection}` and the subagent-stream equivalents insert them as `System` lines after flushing streaming text; `TurnStarted` → activity `Waiting`; approvals unaffected.
**Oracle:** spec text (approved literals, copied by hand into expected values); P6 busy trace order for positioning.
**Stress fixture:** workflow name with Unicode and embedded quotes (`«review» "x"`), empty content, content without the `[notification/…] ` prefix, severity absent, reason absent (`unspecified`), notice arriving before any streamed text (no empty AgentText committed).
**Regression fence:** unit tests in `crates/cyril-ui/src/turn_labels.rs`; UiState tests in `state.rs` (order, activity, approval); `subagent_ui.rs` tests.
**Named mutation:** M15 (no prefix strip), M16 (append without flush), M17 (remove UiState `TurnStarted` arm), M18 (gate approvals on `Waiting`).
**Complexity/production scale:** string formatting O(len(content)); content bounded by KAS injection size (observed ≤ 250 chars).
**Wall budget/phase:** event-driven, deterministic: `N/A — reason: formatting fenced by unit tests`.
**Module shape:** creates `turn_labels.rs` (owner of strings); `state.rs` +≤60 (two thin methods + arm); `subagent_ui.rs` +≤25; `shape.py` → PASS (no header literals in `state.rs`/`app.rs`).
**Files:** `crates/cyril-ui/src/turn_labels.rs`, `crates/cyril-ui/src/lib.rs` (mod decl), `crates/cyril-ui/src/state.rs`, `crates/cyril-ui/src/subagent_ui.rs`.
**Estimate:** 2 h
**Diff estimate:** 480
**PR increment:** I2
**Commands and expected results:**
- `cargo nextest run -p cyril-ui -E 'test(/turn_labels|agent_initiated|engine_injection|turn_started_activity/)'` → exact strings; order `[AgentText abc, System notice, AgentText def]`; `Waiting` then `Ready`; approval overlay topmost.
- M15/M16/M17/M18 → the named test red; restore → green.
- `python3 .cyril-lki9/oracles/shape.py` → PASS; M21b (+80 lines to `state.rs`) → `C21 FAIL crates/cyril-ui/src/state.rs prod delta 150 > 70`.

### Checkpoint record — Slice 7 (2026-09-29, branch `fix/cyril-lki9-i2-labels`)

**Impact analysis (step 1).** New module `cyril-ui/src/turn_labels.rs` (no callers before this slice). New `UiState::{begin_agent_initiated_turn, show_engine_injection}` (callers arrive in Slice 9). New `Transcript` enum in `traits.rs`; `SubagentStream::push_system`, `SubagentUiState::add_system_message`, `WorkflowUiState::add_system_message`. `UiState::apply_notification`: `TurnStarted` now has its own arm (was a no-op group member). **Core classification added (placement already approved: KAS literals only in `convert/kas.rs`):** `AgentInitiation::new(reason, workflow_completion)` + `is_workflow_completion()`; `EngineMessageInjected.workflow_completion` — callers: `convert/kas.rs` (producer), `agent_initiated.rs` test helper, `test_bridge` printer (`..`), kas.rs tests. `WakeLabel::new` made public (plain value type; tracker uses it).

**Gate.**
1. Affected unit tests — PASS: `cargo nextest run --workspace --all-features` → 2107 passed, 13 skipped.
2. Falsifiers C15/C16/C17/C18 — PASS: `turn_labels::tests::{header_strings, notice_strings}`, `state::tests::{lki9_notice_lands_in_arrival_order, lki9_turn_started_sets_busy_indicator, lki9_approval_during_server_turn}`; classification fenced in `turn_origin_table` / `turn_origin_matches_tagged_frame_census` / `notify_injection_is_engine_message` (workflow-completion flag agrees with the raw reason / `notify-wf-` id on every row and every captured frame).
3. Stress fixture — PASS: Unicode + quotes in a run name shown as given; empty content adds no blank line; content without the `[notification/…] ` prefix shown verbatim; absent/empty severity → `info`; absent reason → `unspecified`; a label never renames a non-workflow wake; header before any streamed text commits no empty text block; a step session's header never lands in the main transcript.
4. Implementation vs oracle — PASS: every expected string transcribed by hand from spec B1/B3/B4/B5 (approved literals) and the P6 busy-trace wake text; arrival order matches the P6 busy trace (injection mid-turn).
5. Module shape — PASS: `C21 PASS (… app.rs: 5, state.rs: 62)` — every header/notice literal is in `turn_labels.rs` (R2); `state.rs` +62 of its +70 cap (Slice 8's remaining steer change is planned to net ≈ +6 via a shared helper — see Slice 8).
6. Budget — N/A — reason: string formatting O(len(content)); no loop over collections added.
7. Regression fence — PASS (item 2).
8. Named mutation — PASS (red): M15 (no prefix strip) → `notice_strings` + `lki9_notice_lands_in_arrival_order`; M16 (append without flush) → `lki9_notice_lands_in_arrival_order`; M17 (disable the `TurnStarted` arm) → `lki9_turn_started_sets_busy_indicator` + `lki9_approval_during_server_turn`; M18 (drop approvals while `Waiting`) → `lki9_approval_during_server_turn`.
9. Fence restored — PASS: `turn_labels.rs`, `state.rs` restored from scratch copies (`cmp` identical); all green.
10. Parity and reuse — PASS. Search: `grep` in `cyril-ui/src`. `turn_labels` follows the `workflow_format` pure-formatter pattern; status words reuse `WorkflowRunStatus::as_str` (cyril's existing spelling); main-transcript insertion reuses `UiState::add_system_message` (flush discipline); stream insertion mirrors it in `SubagentStream::push_system`; both stream owners reuse the existing `entry().or_insert_with(SubagentStream::new)` first-contact pattern. Symmetry: main vs stream insertion — same flush-then-push order, same `ChatMessage::system` kind; streams do not get the `TurnStarted` activity arm (C17 is scoped to the main indicator by the design).
11. Preserved enforcement — N/A — reason: nothing relaxed.

Sweep (step 7): the `apply_notification` no-op comment rewritten (the App now renders header/notices via the two methods); `turn_labels` module doc names its fence rule.

## Slice 8: unknown steer ids never flip a bound operator chip

**Claim IDs:** C6
**Expected behavior:** `flip_consumed_steer_echo` falls back only to the oldest id-less Queued echo; counter changes only when an echo flips.
**Oracle:** cyril-5n75 acceptance (hand-written expected states).
**Stress fixture:** bound `steer-A` Queued + id-less echo Queued + `SteeringConsumed{notify-X}` → id-less echo flips, `steer-A` stays Queued, counter −1; only `steer-A` Queued → nothing flips, counter unchanged; `SteeringConsumed{None}` (legacy) → oldest Queued flips as today.
**Regression fence:** UiState tests in `crates/cyril-ui/src/state.rs`.
**Named mutation:** M6 (restore unconditional FIFO fallback).
**Complexity/production scale:** existing linear scan over messages (unchanged).
**Wall budget/phase:** `N/A — reason: existing path, no new phase`.
**Module shape:** existing steer responsibility in `state.rs` (+≤10 prod, within the protected-parent cap); `shape.py` → PASS.
**Files:** `crates/cyril-ui/src/state.rs`.
**Estimate:** 0.5 h
**Diff estimate:** 70
**PR increment:** I2
**Commands and expected results:**
- `cargo nextest run -p cyril-ui -E 'test(/steer_fallback/)'` → the three stress rows as specified.
- M6 → red (`Applied` ≠ `Queued`); restore → green.

### Checkpoint record — Slice 8 (2026-09-30, branch `fix/cyril-lki9-i2-labels`)

**Impact analysis (step 1).** `UiState::flip_consumed_steer_echo` (sole caller: the `SteeringConsumed` arm) — semantics narrowed; `flip_cleared_steer_echoes` (sole caller: the `SteeringCleared` arm) — inline id-less fallback extracted into the new `flip_oldest_idless_queued`, behavior unchanged. No signature change.

**Qualification stop → Length review (resolved).** The first gate run failed C21 R3: `state.rs` +75 > +70. Stopped per module-shape; requester approved **retain and raise to +80** (2026-09-30); reshape deferred to **cyril-dgyz**; recorded in design.md (Module shape → Length review), plan growth ledger, and `shape.py` (cap 70 → 80, with a comment citing the approval).

**Gate.**
1. Affected unit tests — PASS: `cargo nextest run --workspace --all-features` → all passed (22 steer-related UI tests incl. the pre-existing cyril-vgcm/7z7u/nvmh fences unchanged).
2. Falsifier C6 — PASS: `lki9_steer_fallback_never_drains_a_bound_chip`.
3. Stress fixture — PASS: bound `steer-A` + id-less chip + `Consumed{notify-X}` → id-less flips, `steer-A` stays Queued, counter 2 → 1; only a bound chip → nothing flips, counter unchanged; legacy `Consumed{None}` → FIFO flip as before.
4. Implementation vs oracle — PASS: expected states hand-transcribed from the cyril-5n75 acceptance criterion and the existing doc intent (fallback covers dropped/deferred Queued echoes = id-less chips).
5. Module shape — PASS: `C21 PASS (… app.rs: 5, state.rs: 75)` under the approved +80 cap.
6. Budget — N/A — reason: existing linear scan over messages, unchanged bound.
7. Regression fence — PASS (item 2).
8. Named mutation — PASS (red): M6 (drop the id-less narrowing; old unconditional FIFO) → `left: [("first", Applied, Some("steer-A")), ("second", Queued, None)]` — the cyril-5n75 bug reproduced. **Changed fence (C21 cap) re-proved:** M21b (+6 lines → +81) → `C21 FAIL … prod delta 81 > 80`.
9. Fence restored — PASS: `state.rs` restored from scratch copy (`cmp` identical); fences green.
10. Parity and reuse — PASS. Search: `grep` for SteerEcho flip helpers in `state.rs`. The consumed fallback now REUSES the cleared path's id-less rule through one extracted helper (`flip_oldest_idless_queued`) instead of a second copy. Symmetry: Cleared and Consumed now share the same unknown-id fallback (id-less only); Consumed's id-less legacy branch keeps FIFO over all Queued (unchanged) — the divergence is the old-dialect convention both functions already documented. Acceptance change: a Consumed id bound to no chip previously flipped the oldest Queued chip even if bound to another id; now it flips only an id-less chip or nothing — control rows cover both neighbours.
11. Preserved enforcement — PASS: the C21 R3 cap was RAISED (70 → 80) — authorized by the requester's Length-review approval (design.md, 2026-09-30), deferral cyril-dgyz verified; detection re-proved at the new boundary (M21b +81 red). No other gate touched.

Sweep (step 7): `flip_consumed_steer_echo` doc rewritten (the old "else the OLDEST Queued chip" sentence was the bug); `flip_cleared_steer_echoes` doc still accurate (behavior unchanged).

## Slice 9: App routes announcements and injections to tracker + UI

**Claim IDs:** C14
**Expected behavior:** `AgentInitiatedTurn{workflow-complete-wake}` → `take_wake_label` + B1/B4 header; other reasons → B3 header, no label taken; `EngineMessageInjected{notify-wf-…}` → label + workflow notice; other `notify-` → step notice; main session → `UiState`, other sessions → their stream.
**Oracle:** spec B1/B3/B4/B5 strings (hand-written); tracker state inspected independently in the test.
**Stress fixture:** two completions pending + one wake → header names the older, the newer remains; `send-message-wake` on a step session while a completion is pending for main → generic header in the step stream, main label untouched.
**Regression fence:** App tests in `crates/cyril/src/app/tests/current_runtime_contract/`.
**Named mutation:** M14 (take a label for every reason).
**Complexity/production scale:** `N/A — reason: O(1) routing`.
**Wall budget/phase:** `N/A — reason: event-driven routing`.
**Module shape:** `app.rs` protected parent: routing arms only, cumulative ≤ +60 prod; `shape.py` → PASS.
**Files:** `crates/cyril/src/app.rs`, App test file.
**Estimate:** 1.5 h
**Diff estimate:** 230
**PR increment:** I2
**Commands and expected results:**
- `cargo nextest run -p cyril -E 'test(/wake_header|engine_injection_routing/)'` → headers/notices and tracker state as specified.
- M14 → red (label consumed on `send-message-wake`); restore → green.
- `python3 .cyril-lki9/oracles/shape.py` → PASS (app.rs delta ≤ 60).

## Slice 10: end-to-end replay of the committed captures

**Claim IDs:** C20
**Expected behavior:** replaying the tail, gate-on, busy, cancel and prompt captures through KAS conversion + `TurnMediator` + `SessionController` + `UiState` yields the spec success-criteria observables (B1 header once before the wake's first text; B2 busy span + one `TurnCompleted`; B5 notices; B7 silent wake busy, no header).
**Oracle:** P1–P7 probe/oracle outputs (independent Python/grep).
**Stress fixture:** the gate-on capture (two chained wakes across two sessions, 292 tagged frames, a permission mid-wake) and the prompt capture (silent wake pre-empted by a prompt).
**Regression fence:** replay tests reading `.cyril-lki9/lki9-live-*.jsonl` and `experiments/conductor-spike/kas-workflow-*-2.26.0.jsonl` (cyril-core test for conversion+mediator; cyril-ui/cyril test for state).
**Named mutation:** M20 (remove the `BeginServerTurn` application in `inbound.rs`).
**Complexity/production scale:** test-only; captures ≤ 700 frames.
**Wall budget/phase:** `N/A — reason: test-only`.
**Module shape:** tests only; `shape.py` → PASS.
**Files:** replay test files + a small capture loader (test-only).
**Estimate:** 2 h
**Diff estimate:** 350
**PR increment:** I2
**Commands and expected results:**
- `cargo nextest run --workspace -E 'test(/lki9_replay/)'` → every spec success-criterion observable holds; counts agree with `probe_p1_p7.py`.
- M20 → tail replay red (no `TurnCompleted`, activity stuck); restore → green.
- full gate: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo nextest run --workspace --all-features` → clean/green.

---

## Self-review

1. Every design row C1–C22 is assigned to exactly one slice (C21 created in S1, rerun every slice); every PENDING falsifier is discharged by its implementing slice. ✓
2. Every slice has the fourteen fields; conditional cells carry `N/A — reason`. ✓
3. Every fence is created in the slice implementing its claim, with the design's named mutation; no fence-less claims. ✓
4. New loops (S1 filter, S4 flag map, S6 deque, S7 formatting) state cost and bound; no real-latency phases. ✓
5. Module shape field + growth ledger cover every touched module and both protected parents; Length review `N/A — no trigger` cited. ✓
6. Partition applied with a documented margin (30 % code, 10 % artifacts); every slice names its increment; each increment has a mergeable definition. ✓
7. Tracker taxonomy: deferrals cite cyril-fb1m, cyril-99ds, cyril-zd8u, cyril-4u4a (verified, open). ✓
8. Fence assertions assert the observable their mutation changes (variant, count, order, dispatched command, disposition). ✓
9. No slice is declared complete. ✓
