# Plan — cyril-84ca: drive `conn.prompt()` off the bridge command loop

Design: [design.md](design.md) (claims C1–C7, C9). Prove-it: [findings.md](findings.md).
Slices are sequential; all touch `crates/cyril-core/src/protocol/bridge.rs` (production + colocated
`#[cfg(test)] mod tests`), so they serialize on that file. The fake-agent harness (Slice 2) is the
foundation every regression fence depends on — built before any behavior change.

**No production loop is added by any slice.** The fix is structural (move one `.await` off the loop),
not algorithmic. Every slice's Loop budget is therefore "no new production loop"; the only loops are a
test-only frame pump (Slice 7) bounded by updates-per-turn.

---

## Slice 1: Extract a testable command-loop seam (no behavior change)

**Claim:** (infrastructure for C1–C9) `run_bridge` can be driven against an injected connection, so tests
exercise the loop without spawning a `kiro-cli` subprocess.
**Oracle:** prove-it oracle — `cargo run --example test_bridge -- --agent-command kiro-cli acp` still
completes a real turn; the existing 358 `cyril-core` unit tests still pass (behavior-preservation).
**Stress fixture:** the full existing `cyril-core` test suite + the `test_bridge` real-kiro smoke. Bug class
it must fail under: the refactor silently drops/renames a `BridgeCommand` arm or reorders the handshake →
a previously-passing arm test or the smoke turn breaks.
**Loop budget:** no new loop — the existing `while let Some(cmd) = command_rx.recv()` loop is *moved* into
`run_loop`, unchanged. Cost still O(commands), event-driven.
**Wall budget:** N/A — event-driven (awaits `recv`), not a polling phase.
**Files:** `crates/cyril-core/src/protocol/bridge.rs`.

**Code (advisory):**
```rust
// run_bridge keeps: spawn process -> ClientSideConnection::new -> spawn_local(io_task)
//   let conn = Rc::new(conn);              // Rc now (needed by Slice 3; harmless here)
//   run_loop(conn, channels).await
//
// New private fn = today's handshake + command loop verbatim, only s/conn/&*conn or conn.clone():
async fn run_loop(conn: Rc<acp::ClientSideConnection>, mut channels: BridgeChannels)
    -> crate::Result<()> { /* initialize handshake + the existing `while let Some(cmd)` match */ }
```

**Verification:**
- [ ] Unit tests pass (all 358)
- [ ] `test_bridge` smoke completes a real turn (prove-it oracle agrees)
- [ ] No `BridgeCommand` arm removed (grep arm count unchanged)
- [ ] Loop/wall budgets hold (no new loop)

---

## Slice 2: In-process fake-agent harness + baseline turn

**Claim:** (infrastructure) a `FakeAgent` wired to `run_loop` over a `tokio::io::duplex` can drive a full turn
and the harness observes the bridge's notifications, enabling deterministic CI fences with no `kiro-cli`.
**Oracle:** the baseline turn through the harness reproduces real kiro's shape — exactly one `TurnCompleted`
for one `SendPrompt` — cross-checked against the prove-it capture (one `end_turn` per prompt).
**Stress fixture:** baseline = NewSession → SendPrompt (FakeAgent returns `EndTurn`). Expected: exactly one
`SessionCreated` then exactly one `TurnCompleted{EndTurn}`. Bug class it fails under: duplex mis-wired
(request frames not delivered) → zero notifications / test hangs.
**Loop budget:** test-only. FakeAgent handlers are O(1); no production loop touched.
**Wall budget:** N/A.
**Files:** `crates/cyril-core/src/protocol/bridge.rs` (`#[cfg(test)] mod tests`).

**Code (advisory):**
```rust
// Harness: two duplex ends; ClientSideConnection(KiroClient) <-> AgentSideConnection(FakeAgent).
// let (c_io, a_io) = tokio::io::duplex(64 * 1024);
// let (cr, cw) = tokio::io::split(c_io); let (ar, aw) = tokio::io::split(a_io);
// let (conn, conn_io) = acp::ClientSideConnection::new(KiroClient::new(ntx, ptx), cw.compat_write(), cr.compat(), spawn_local);
// let (_a, a_io_task) = acp::AgentSideConnection::new(FakeAgent::new(script), aw.compat_write(), ar.compat(), spawn_local);
// spawn_local both io tasks; spawn_local(run_loop(Rc::new(conn), channels)); drive via cmd_tx; assert on ntx.
//
// FakeAgent impls Agent: initialize/authenticate/new_session/prompt/cancel (+ ext_method override later).
// Shared Rc<RefCell<Script>> records received requests and holds a gate (Rc<Notify>) so prompt() can block.
struct FakeAgent { st: Rc<RefCell<Script>>, gate: Rc<tokio::sync::Notify> }
// prompt(): record "prompt"; if script.block { gate.notified().await } ; Ok(PromptResponse{ stop_reason })
```

**Verification:**
- [ ] Baseline test: 1 `SendPrompt` → exactly 1 `TurnCompleted{EndTurn}`
- [ ] Harness delivers both directions (FakeAgent records the `session/prompt` it received)
- [ ] prove-it oracle still agrees (real-kiro smoke unaffected — harness is test-only)
- [ ] Loop/wall budgets hold

---

## Slice 3: Off-loop prompt dispatch (the core change)

**Claim:** C1 (loop frees) + C6 (exactly one `TurnCompleted` per turn, success and transport-error paths).
**Oracle:** harness — FakeAgent records request arrival; the bridge's notification stream is asserted by the
test (independent of the loop's internals). For C6 transport-error: drop the agent end of the duplex so
`conn.prompt()` returns `Err`.
**Stress fixture (two, adversarial):**
- C1: FakeAgent `prompt` blocks on the gate. Sequence: SendPrompt, then `ListSettings`. Expected: the
  `ListSettings` response notification is observed **before** `TurnCompleted`; then release the gate.
  **Fails under:** leaving `conn.prompt().await` inline (today's bug) → `ListSettings` blocked → arrives
  after `TurnCompleted` (or test times out).
- C6: (a) normal turn → exactly 1 `TurnCompleted`. (b) FakeAgent drops the connection mid-prompt →
  exactly 1 `TurnCompleted{EndTurn}`. **Fails under:** the spawned task's `Err` arm `return`s without
  notifying → 0 `TurnCompleted` → UI stuck busy.
**Loop budget:** no new loop. `spawn_local` is O(1) per turn; the prompt task is one-shot.
**Wall budget:** N/A.
**Files:** `crates/cyril-core/src/protocol/bridge.rs`.
**Doc-comment contract:** the spawned task "MUST emit exactly one `TurnCompleted`" is **load-bearing for
correctness** (zero → UI stuck busy forever). Enforced structurally: both Ok and Err arms call
`notify_or_closed(TurnCompleted)`; fenced by C6. (Not a `debug_assert` — a missing notify is a release bug.)

**Code (advisory):**
```rust
// Replace the inline SendPrompt arm body with:
let conn2 = conn.clone();
let ntx = channels.notification_tx.clone();
let handle = tokio::task::spawn_local(async move {
    let note = match conn2.prompt(request).await {
        Ok(r)  => Notification::TurnCompleted { stop_reason: convert::to_stop_reason(r.stop_reason) },
        Err(e) => { tracing::error!(error=%e, "prompt failed");
                    Notification::TurnCompleted { stop_reason: StopReason::EndTurn } }
    };
    let _ = ntx.send(note.into()).await;   // App-closed handled by the loop's own recv ending
});
prompt_task = Some(handle);                 // stored for Slices 4 (guard) & 5 (abort)
// loop returns to recv() immediately
```

**Verification:**
- [ ] C1 fixture: quick command observed before `TurnCompleted`
- [ ] C6 fixture: exactly one `TurnCompleted` on both success and transport-error
- [ ] prove-it oracle: real-kiro turn still completes with one `TurnCompleted`
- [ ] Loop/wall budgets hold (no new loop)

---

## Slice 4: One prompt in flight — guard + self-clearing

**Claim:** C4 (a `SendPrompt` during a turn does not start a second `conn.prompt()`; emits `BridgeError`) +
C9 (after a turn finishes, the guard reports not-busy so the next `SendPrompt` starts).
**Oracle:** harness — FakeAgent counts `session/prompt` requests received.
**Stress fixture:** FakeAgent `prompt` blocks. Sequence: SendPrompt #1; SendPrompt #2 (while #1 blocked) →
expect FakeAgent has received **exactly 1** `session/prompt` and a `BridgeError` was emitted; release gate,
await `TurnCompleted`; SendPrompt #3 → expect FakeAgent now receives a **2nd** `session/prompt`.
**Fails under (two bugs, distinct asserts):** (a) no guard → FakeAgent sees 2 prompts during the turn;
(b) a `bool busy` set-true-never-reset → #3 rejected (FakeAgent never sees the 2nd prompt).
**Loop budget:** O(1) `JoinHandle::is_finished()` check per `SendPrompt`. No loop.
**Wall budget:** N/A.
**Files:** `crates/cyril-core/src/protocol/bridge.rs`.
**Doc-comment contract:** "at most one prompt in flight" is **load-bearing for correctness** (two concurrent
`conn.prompt()` on one session is undefined) → enforced by a **runtime check** that survives release (the
guard returns a `BridgeError`, not a `debug_assert!`).

**Code (advisory):**
```rust
// At the top of the SendPrompt arm:
if prompt_task.as_ref().is_some_and(|h| !h.is_finished()) {
    if notify_or_closed(&channels.notification_tx, Notification::BridgeError {
        operation: "prompt".into(),
        message: "a turn is already in progress".into(),
    }).await { break; }
    continue;
}
// else fall through to Slice 3's spawn block
```

**Verification:**
- [ ] C4: 2nd mid-turn `SendPrompt` → FakeAgent saw exactly 1 prompt + `BridgeError` emitted
- [ ] C9: after completion, a fresh `SendPrompt` starts a 2nd turn (FakeAgent sees 2nd prompt)
- [ ] prove-it oracle: real-kiro single turn unaffected
- [ ] Loop/wall budgets hold

---

## Slice 5: Shutdown aborts an in-flight turn

**Claim:** C7 — `Shutdown` during a turn aborts the in-flight prompt task and `run_loop` returns without deadlock.
**Oracle:** test driver — `run_loop` future completes within a bounded `tokio::time::timeout`.
**Stress fixture:** FakeAgent `prompt` blocks forever (gate never released). Sequence: SendPrompt; Shutdown.
Expected: `run_loop` returns within 1s and the prompt task is aborted. **Fails under:** not storing/aborting
the handle → the task leaks past `run_loop` return, or (if anyone re-introduced an inline await) the loop
never sees Shutdown → `timeout` fires.
**Loop budget:** O(1) `handle.abort()`. No loop.
**Wall budget:** N/A.
**Files:** `crates/cyril-core/src/protocol/bridge.rs`.

**Code (advisory):**
```rust
BridgeCommand::Shutdown => {
    if let Some(h) = prompt_task.take() { h.abort(); }
    break;
}
```

**Verification:**
- [ ] C7: Shutdown mid-turn → `run_loop` returns within the timeout; task aborted
- [ ] prove-it oracle: clean shutdown after a real turn still works (`test_bridge` exits 0)
- [ ] Loop/wall budgets hold

---

## Slice 6: Mid-turn steer + cancel acceptance fences (the headline)

**Claim:** C2 (steer reaches the agent before `TurnCompleted`) + C3 (cancel resolves the in-flight prompt to
`Cancelled`, no hang). No new production code — these behaviors are enabled by Slice 3; this slice adds the
acceptance fences (overriding `FakeAgent::ext_method` + `cancel` to record/gate).
**Oracle:** harness — FakeAgent records the arrival order of `_session/steer` / `session/cancel` vs its own
`prompt` return; empirical cross-check = the prove-it probes (`probe/src/main.rs`, `probe/src/bin/cancel.rs`).
**Stress fixture:**
- C2: FakeAgent `prompt` blocks; SendPrompt then SteerSession. Expected: FakeAgent's `ext_method("session/steer")`
  is invoked **before** its `prompt` returns. **Fails under:** inline prompt → steer dequeued only post-turn.
- C3: FakeAgent `prompt` blocks until `cancel` arrives, then returns `Cancelled`; SendPrompt then CancelRequest.
  Expected: FakeAgent's `cancel` invoked mid-turn AND bridge emits `TurnCompleted{Cancelled}` within a bound.
  **Fails under:** inline prompt → cancel never dequeued → timeout, no `TurnCompleted{Cancelled}`.
**Loop budget:** test-only; no production loop.
**Wall budget:** N/A.
**Files:** `crates/cyril-core/src/protocol/bridge.rs` (test module; FakeAgent gains `ext_method`/`cancel` recording).

**Verification:**
- [ ] C2: FakeAgent receives `_session/steer` before `prompt` returns
- [ ] C3: FakeAgent receives `session/cancel` mid-turn; bridge emits exactly one `TurnCompleted{Cancelled}`
- [ ] prove-it oracle: `probe` (steer) + `cancel.rs` probes still agree against real kiro
- [ ] Loop/wall budgets hold

---

## Slice 7: Streaming-before-completion ordering fence (C5)

**Claim:** C5 — every streaming notification of a turn is emitted before that turn's `TurnCompleted`.
**Oracle:** harness — FakeAgent emits N `session/update` (`agent_message_chunk`) via the agent→client
notification path, then returns; the test asserts the bridge's notification order. Empirical cross-check: the
prove-it wire capture (all `session/update` precede the `end_turn` result at 6.086s — already passed).
**Stress fixture:** FakeAgent emits 3 chunks then `EndTurn`. Expected notification order:
`[chunk, chunk, chunk, TurnCompleted]`. **Fails under:** the spawned prompt task emits `TurnCompleted` before
the io_task drains the streamed chunks (e.g. notifying before awaiting the prompt result) → a chunk lands
after `TurnCompleted`.
**Loop budget:** the agent→client **pump** is a `while let Some(u) = upd_rx.recv()` loop, **test-only**,
bounded by updates-per-turn (fixture: 3; realistic test ceiling: tens). O(updates), well under 10^6.
**Wall budget:** N/A (test-only).
**Files:** `crates/cyril-core/src/protocol/bridge.rs` (test module: add the notification pump to the harness).
**Output stream:** notifications travel the `notification_tx` mpsc channel (not stdout/stderr) — the
data/diagnostic-fd rule is N/A; `tracing` diagnostics go to stderr; no `println!` added to production.

**Code (advisory):**
```rust
// FakeAgent can't hold AgentSideConnection (moved into ::new), so emit via a channel + pump task:
// let (upd_tx, mut upd_rx) = tokio::sync::mpsc::unbounded_channel();   // FakeAgent holds upd_tx
// let a = Rc::new(agent_conn);
// spawn_local(async move { while let Some(n) = upd_rx.recv().await { let _ = a.session_notification(n).await; } });
// FakeAgent::prompt(): for chunk in &script.chunks { upd_tx.send(make_chunk(chunk))?; } gate.notified().await; Ok(EndTurn)
```

**Verification:**
- [ ] C5: notification order is `[chunks…, TurnCompleted]` (no chunk after `TurnCompleted`)
- [ ] prove-it oracle: wire capture ordering still holds (re-runnable)
- [ ] Loop budget: pump loop is test-only, O(updates ≤ tens)
- [ ] Wall budget holds

---

## Plan Self-Review

**1. Every loop — complexity + production-scale budget:**
- Command loop (`while recv()`): pre-existing, *moved* not added (Slice 1). O(commands), event-driven. ✔ within budget.
- Slices 3,4,5,6: **no new loop** (spawn_local/is_finished/abort are O(1)). ✔
- Slice 7 pump (`while upd_rx.recv()`): **test-only**, O(updates ≤ tens per turn). ✔ far under 10^6.
- No production loop is introduced by this feature. No gaps.

**2. Every fixture — bug class it fails under (not happy-path):**
- S1: dropped/renamed arm or reordered handshake. S2: duplex mis-wired (no delivery). S3-C1: inline-await
  regression (quick cmd blocked); S3-C6: Err path skips notify (0 completions). S4: (a) no guard → 2 prompts,
  (b) busy-flag-never-reset → next prompt rejected. S5: handle not aborted → leak/deadlock. S6-C2: steer
  dequeued post-turn; S6-C3: cancel never dequeued → hang. S7: TurnCompleted emitted before streaming drains.
  Every fixture targets a named bug; none is happy-path-only. No gaps.

**3. Every doc-comment precondition — classified + enforced:**
- "exactly one `TurnCompleted` per turn" (S3): load-bearing-correctness → structural enforcement (both arms
  notify) + C6 fence. ✔
- "at most one prompt in flight" (S4): load-bearing-correctness → **runtime check** (guard → `BridgeError`,
  survives release), not `debug_assert!`. ✔
- No documented precondition left unenforced. No gaps.

**4. Every write target — data vs diagnostic:**
- Notifications → `notification_tx` mpsc channel (neither stdout nor stderr; consumed by the App). Not an fd;
  rule N/A. `tracing::error!/warn!` → stderr (diagnostic). No new `println!`/stdout writes in production. No gaps.

**5. Every tracker reference — resolves to a covering issue:**
- `cyril-bm1j` (K1b, App-side busy routing) — verified present (in_progress). `cyril-28z2` (K1c, queue-mode) —
  verified present (open). Both cited only as out-of-scope boundaries (Negative space), not deferred work from
  this plan. The fake-agent harness is in-scope (Slices 2/7), not deferred. No untracked deferrals. No gaps.

## Hard gate
- [x] Every slice has all mandatory fields
- [x] Every loop has a complexity statement (all "no new production loop"; pump is test-only bounded)
- [x] Every slice has an adversarial stress fixture
- [x] Claim coverage matches the design: C1,C6→S3; C4,C9→S4; C7→S5; C2,C3→S6; C5→S7 (all of C1–C7,C9 covered)
- [x] Every tracker reference resolves to an existing covering issue (cyril-bm1j, cyril-28z2 verified)
