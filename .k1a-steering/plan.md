# Plan: K1a — Queue-steering wire + state plumbing

Upstream: `.k1a-steering/design.md` (approved, cheapest falsifier #7 passed). 5 slices cover all 13 design claims. Order is dependency-safe: each slice leaves `cargo test`/`clippy -D warnings` green.

**Workspace invariants (apply to every slice):** no `unwrap`/`let _ =`/`#[allow]`/`unsafe`/sentinel; `cargo test` + `cargo clippy -- -D warnings` + `cargo fmt --check` on touched files (do NOT reformat the pre-existing fmt failures in `bridge.rs`/`app.rs`/`main.rs`). Diagnostics via `tracing` (stderr/`cyril.log`); no `println!`; system messages are UI transcript data, not a pipe.

---

## Slice 1: Converter arm + echo Notification variants

**Claim:** Design claims 1–6 (queued/consumed/cleared → typed notifications; missing-field → `Ok(None)`; unknown sub-variant → `Ok(None)`; unprefixed arm unchanged).
**Oracle:** captured wire log `experiments/conductor-spike/logs/probe-steer-goal-2.7.0.log` lines 25/37/120 (input + intended echo); CLAUDE.md no-sentinel rule; existing `to_ext_notification_session_update_tool_call_chunk` test (mod.rs:938) for the regression guard.
**Stress fixture:** (each expected output written before code)
- `steering_queued` frame that *also* carries a stray `content:"WRONG"` → must read `message`, produce `SteeringQueued{message:"X"}` (NOT content). [catches read-wrong-key]
- `steering_cleared` with an extra unexpected field `{"sessionUpdate":"steering_cleared","foo":1}` → `Ok(Some(SteeringCleared))` (NOT dropped). [catches require-payload bug]
- `steering_paused` (unknown sub-variant) → `Ok(None)`, NOT `Err`. [catches copying the unprefixed arm's `Some(other)=>Err`]
- `steering_queued` with no `message` → `Ok(None)` + `warn!`, NOT `SteeringQueued{message:""}`. [catches `unwrap_or("")`]
- unprefixed `kiro.dev/session/update` + `tool_call_chunk` → still `ToolCallChunk`. [regression]
**Loop budget:** No new loops. The new arm is a single `match` on `sessionUpdate` (O(1)); field reads are O(1).
**Wall budget:** n/a (not an always-on phase).
**Files:** `crates/cyril-core/src/types/event.rs` (add `SteeringQueued{message:String}`, `SteeringConsumed{content:String}`, `SteeringCleared`); `crates/cyril-core/src/protocol/convert/kiro.rs` (new outer arm + tests).

**Code (advisory):**
```rust
// kiro.rs, new outer arm BEFORE the `other =>` fallthrough:
"_kiro.dev/session/update" => {
    let u = params.get("update");
    match u.and_then(|u| u.get("sessionUpdate")).and_then(|s| s.as_str()) {
        Some("steering_queued") => match u.and_then(|u| u.get("message")).and_then(|v| v.as_str()) {
            Some(m) => Ok(Some(Notification::SteeringQueued { message: m.to_string() })),
            None => { tracing::warn!("steering_queued missing message, dropping"); Ok(None) }
        },
        Some("steering_consumed") => match u.and_then(|u| u.get("content")).and_then(|v| v.as_str()) {
            Some(c) => Ok(Some(Notification::SteeringConsumed { content: c.to_string() })),
            None => { tracing::warn!("steering_consumed missing content, dropping"); Ok(None) }
        },
        Some("steering_cleared") => Ok(Some(Notification::SteeringCleared)),
        Some(other) => { tracing::debug!(variant = other, "unhandled _kiro.dev steering variant"); Ok(None) }
        None => { tracing::debug!("_kiro.dev/session/update missing sessionUpdate"); Ok(None) }
    }
}
```
Note: unknown sub-variant returns `Ok(None)` (tolerant), deliberately UNLIKE the unprefixed arm's `Err` — the `_kiro.dev/*` dialect must tolerate future variants (design claim 5).

**Verification:**
- [ ] Unit tests pass (6: one per claim 1–6, distinct names per design fence column)
- [ ] Stress fixtures produce the expected outcomes above
- [ ] prove-it-prototype oracle still agrees: the original `probe.rs` premise inverts cleanly (was `Ok(None)`, now typed) — re-running it should now FAIL its old `Ok(None)` assert for the 3 valid frames, confirming the arm is wired; keep the probe's data, flip its assertions into the 3 unit tests.
- [ ] No new loops; O(1) holds.

---

## Slice 2a: Pure `steer_error_action` + `SteeringUnsupported` variant

**Claim:** Design claims 7, 8, 9 (detect -32601; MethodNotFound→MarkAndNotify once; other code→BridgeError, no mark).
**Oracle:** JSON-RPC spec (`MethodNotFound` == -32601, from `agent-client-protocol-schema`) + the design decision table — independent of the bridge I/O loop.
**Stress fixture:**
- `steer_error_action(ErrorCode::InternalError /* -32603 */, already=false)` → `BridgeError` (NOT `MarkAndNotify`). [catches "any error ⇒ unsupported", which would permanently disable steering on a transient backend hiccup]
- `steer_error_action(ErrorCode::MethodNotFound, already=true)` → `AlreadyUnsupported` (NOT a second notify). [catches missing emit-once guard]
- `steer_error_action(ErrorCode::MethodNotFound, already=false)` → `MarkAndNotify`. [happy]
**Loop budget:** No new loops. Pure `match` over `(code, bool)`, O(1).
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/bridge.rs` (pure fn + `SteerAction` enum + `#[cfg(test)]` unit tests); `crates/cyril-core/src/types/event.rs` (add `Notification::SteeringUnsupported{message:String}`).

**Code (advisory):**
```rust
#[derive(Debug, PartialEq, Eq)]
enum SteerErrorAction { MarkAndNotify, AlreadyUnsupported, BridgeError }

fn steer_error_action(code: acp::ErrorCode, already_unsupported: bool) -> SteerErrorAction {
    match (code == acp::ErrorCode::MethodNotFound, already_unsupported) {
        (true, false) => SteerErrorAction::MarkAndNotify,
        (true, true)  => SteerErrorAction::AlreadyUnsupported,
        (false, _)    => SteerErrorAction::BridgeError,
    }
}
```
(`e.code == acp::ErrorCode::MethodNotFound` proven to compile — design falsifier #7.)

**Verification:**
- [ ] Unit tests pass (3 cases above, distinct asserts)
- [ ] Stress fixtures produce expected outcomes
- [ ] prove-it-prototype oracle unaffected (no converter/binary change)
- [ ] No new loops; O(1).

---

## Slice 2b: BridgeCommand variants + handler wiring + pre-send gate

**Claim:** Design claim 10 (already-unsupported session → zero requests, zero notifications), and wires Slice 2a into the live bridge.
**Oracle:** the per-session unsupported-set semantics (independent: `HashSet` contract); the bridge-channel test harness (bridge.rs:1034-style BridgeCommand round-trip).
**Stress fixture:** (pure-predicate level — see budget note)
- set = `{sid_A}`; `should_skip_steer(&set, &sid_A)` → `true` (skip). [happy]
- set = `{sid_A}`; `should_skip_steer(&set, &sid_B)` → `false` (a DIFFERENT session still steers). [catches global-vs-per-session bug: one unsupported session must not mute others]
- set = `{}`; any sid → `false`.
**Loop budget:** No new loops. `unsupported_sessions: HashSet<SessionId>` — `contains`/`insert` are O(1) amortized; |set| ≤ #live sessions (main + a handful of subagents) ≪ 10^3. The bridge's existing command loop is unchanged (one steer handler = O(1) per command).
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/types/event.rs` (add `BridgeCommand::SteerSession{session_id,message}`, `ClearSteering{session_id}`); `crates/cyril-core/src/protocol/bridge.rs` (two handler arms — exhaustive match forces them here — + the local `HashSet` declared in the loop scope + `should_skip_steer` predicate).

**Code (advisory):**
```rust
// in the bridge loop scope, alongside other long-lived locals:
let mut steering_unsupported: std::collections::HashSet<SessionId> = Default::default();

// handler (SteerSession shown; ClearSteering mirrors with the /clear method + no message):
BridgeCommand::SteerSession { session_id, message } => {
    if steering_unsupported.contains(&session_id) {
        tracing::debug!(%session_id, "steering unsupported for session, skipping send");
        continue; // pre-send gate: no request, no notification
    }
    let params = serde_json::json!({ "sessionId": session_id.as_str(), "message": message });
    // ... to_raw_arc + ext_method("_session/steer", ...).await ...
    if let Err(e) = result {
        match steer_error_action(e.code, steering_unsupported.contains(&session_id)) {
            SteerErrorAction::MarkAndNotify => {
                steering_unsupported.insert(session_id.clone());
                // emit Notification::SteeringUnsupported { message: "steering requires kiro-cli 2.7.0+" }
            }
            SteerErrorAction::AlreadyUnsupported => {} // emit-once
            SteerErrorAction::BridgeError => { /* emit Notification::BridgeError {…} */ }
        }
    }
    // Ok({queued:true}) => do NOTHING (echo is the source of truth — design claim, Decision 5)
}
```

**Budget/risk note (honest):** the *predicate* `should_skip_steer` is unit-tested cheaply; the *wiring* ("handler returns before `ext_method`") is a 2-line `contains → continue` guard with no transport seam to assert "zero bytes on the wire" — the bridge talks to a live `ClientSideConnection`, and building a fake transport is out of proportion for K1a. The guard's correctness is local and covered by the predicate test + code review. End-to-end "zero send" verification arrives with K1b's dispatch site (`cyril-bm1j`). This is the design's claim-10 cost (30m) reduced to the predicate; the residual is explicitly accepted here, not silently dropped.

**Verification:**
- [ ] Unit tests pass (`should_skip_steer` cases) + existing bridge channel tests still green
- [ ] Stress fixtures produce expected outcomes
- [ ] `cargo check` green (exhaustive `BridgeCommand` match now covers both new variants)
- [ ] prove-it-prototype oracle unaffected
- [ ] No new loops; HashSet O(1) holds, |set| ≪ 10^3

---

## Slice 3: SessionController steering state

**Claim:** Design claim 11 (depth `[1,2,1,0]` floored; unsupported flag set; reset on new session).
**Oracle:** hand-computed arithmetic sequence (independent of the impl).
**Stress fixture:**
- apply `[Queued, Queued, Consumed, Cleared]` → depths observed `[1, 2, 1, 0]`. [catches `Cleared` decrementing-by-1 instead of zeroing]
- apply `Consumed` when depth already 0 → stays 0 (saturating). [catches underflow panic / wraparound]
- apply `SteeringUnsupported` (flag true), then `set_session(new_id, …)` → flag `false` AND depth `0`. [catches cross-session leak: a 2.6.1 session's "unsupported" must not persist onto a fresh 2.7.0 session]
**Loop budget:** No new loops. Each arm is O(1) field arithmetic (`+= 1`, `saturating_sub(1)`, `= 0`, `= true`).
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/session.rs` only (fields `steering_depth: usize`, `steering_unsupported: bool`; getters `steering_depth()`, `steering_unsupported()`; four `apply_notification` arms replacing what currently falls into `_ => false`; reset in `set_session` / wherever a new session zeroes state).

**Doc-comment contract:** none introduced (no "callers must" preconditions; the floor is `saturating_sub`, a defensive total operation, not a precondition).

**Verification:**
- [ ] Unit tests pass (`steering_state_transitions_and_reset`, distinct asserts per sub-case)
- [ ] Stress fixtures produce expected outcomes
- [ ] `apply_notification` returns `true` on each steering change (existing contract)
- [ ] prove-it-prototype oracle unaffected
- [ ] No new loops; O(1).

---

## Slice 4: UiState steering mirror + system message

**Claim:** Design claims 12 (SteeringUnsupported → one system message), 13 (queue mirror queued→1, consumed→0).
**Oracle:** message-list count before/after (independent); hand-computed mirror sequence.
**Stress fixture:**
- apply one `SteeringUnsupported{message:"steering requires kiro-cli 2.7.0+"}` → exactly +1 system message, and it contains the text. [catches no-arm = 0 messages]
- apply `Queued` then `Consumed` → mirror back to 0; `Consumed` at 0 → floor 0. [catches consumed-not-decrementing]
**Loop budget:** No new loops. `add_system_message` (existing) + O(1) mirror arithmetic.
**Wall budget:** n/a.
**Files:** `crates/cyril-ui/src/state.rs` only (a `steering_queued: usize` mirror field + getter; `apply_notification` arms: `SteeringUnsupported → add_system_message(message)`, `SteeringQueued → += 1`, `SteeringConsumed → saturating_sub(1)`, `SteeringCleared → = 0`).

**Output stream:** `add_system_message` writes to the in-memory transcript (UI data rendered by the TUI), not stdout/stderr — correct; no pipe consumer.

**Verification:**
- [ ] Unit tests pass (`steering_unsupported_adds_one_message`, `steering_queue_mirror`)
- [ ] Stress fixtures produce expected outcomes
- [ ] cyril-ui still imports no `acp::`/`protocol::` (consumes only the cyril-core `Notification` enum)
- [ ] prove-it-prototype oracle unaffected
- [ ] No new loops; O(1).

---

## Plan Self-Review

**1. Every loop — complexity stated & within budget?**
- Slices 1, 2a, 3, 4: **no new loops** (single `match` arms, O(1) arithmetic). ✓
- Slice 2b: `HashSet<SessionId>` `contains`/`insert` O(1) amortized; |set| ≤ #live sessions ≪ 10^3; bridge command loop unchanged. ✓
- No loop annotated `O(?)`. ✓

**2. Every fixture — designed to fail under a named bug class?**
- S1: read-wrong-key, require-payload, copy-the-Err-arm, `unwrap_or("")`, regression. ✓
- S2a: "any error ⇒ unsupported", missing emit-once. ✓
- S2b: global-vs-per-session muting. ✓
- S3: cleared-decrements-by-1, underflow, cross-session leak. ✓
- S4: no-arm (0 messages), consumed-not-decrementing. ✓
- None are happy-path-only. ✓

**3. Every doc-comment precondition — classified & enforced?**
- No load-bearing "callers must" preconditions are introduced in any slice. Floors use `saturating_sub` (defensive total ops, not preconditions). Missing-field handling is explicit `Ok(None)` (not a precondition). No `debug_assert!`-vs-runtime-check decision arises. ✓

**4. Every write target — data or diagnostic?**
- `tracing::warn!`/`debug!` (S1, S2b) → diagnostic → stderr/`cyril.log`. ✓
- `add_system_message` (S4) → UI transcript data (not a stdout pipe). ✓
- No new `println!` / stdout writes. ✓

**5. Every tracker reference — resolves to a covering issue?**
- K1b `cyril-bm1j` (verified, open) — the dispatch site / end-to-end steer-send test deferred from Slice 2b. ✓
- K1c `cyril-28z2` (created + verified this session, ref `ROADMAP:K1c`) — subagent-scoped routing / queue-mode. ✓
- No uncited deferrals. ✓

**Claim coverage vs design:** S1→{1,2,3,4,5,6}, S2a→{7,8,9}, S2b→{10}, S3→{11}, S4→{12,13}. All 13 design claims covered, none duplicated. ✓

No gaps in any of the five lists.
