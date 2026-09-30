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
| `cyril-ui/src/state.rs` | 2,983 | ≤ 3,053 | thin delegation + arms + C6 narrowing | two methods | **protected: ≤ +70 prod; no header/notice literals** |
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
