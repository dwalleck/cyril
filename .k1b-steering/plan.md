# Plan — cyril-bm1j (K1b): queue-steering TUI UX

Budgeted-plan over the approved [design](design.md) (12 claims C1–C12, cheapest falsifier C12 passed).
Branch `feat/k1b-queue-steering-ux` (off merged main; cyril-84ca substrate present). Each slice ≤2 files,
≤~50 LOC, ≤30 min, build-green after each. C12 is backend behavior already proven — no code slice (see end).

**Shared facts (all slices):**
- Message list is bounded by `max_messages` (config `ui.max_messages`, default 500). Reconciliation scans are `O(messages) ≤ 500` per steering notification; steering notifications are a handful per turn ⇒ well under 10^6 ops / 10^3 syscalls. No always-on phase ⇒ no wall budget except where noted.
- Output streams: user-facing text → `UiState` transcript (in-TUI data); diagnostics → `tracing` (→ `cyril.log`). **No new `stdout`/`stderr`/`println!`** is introduced in any slice.
- `BridgeSender::from_sender(mpsc::Sender<BridgeCommand>)` (bridge.rs:73) lets every bridge-touching slice assert the emitted command on a test receiver — deterministic, no `kiro-cli`.

---

## Slice 1: `SteerEcho` message type + transcript render

**Claim:** Display foundation — a `SteerEcho{status}` renders one distinct line per status (`Queued`/`Applied`/`Cleared`/`Unsupported`), visible support for C3–C6.
**Oracle:** `TestBackend` buffer scrape (independent of the type's own logic).
**Stress fixture:** render four messages, one per status, text `"café→ stop"` (Unicode + arrow). Expected: buffer contains four *distinct* suffixes — `queued`, `applied`, `cleared`, `not supported`; `Applied`'s suffix ≠ `Queued`'s (tie-break wired); no panic on the Unicode text.
**Loop budget:** no new loop (render reuses the existing per-message loop in `chat::render`, `O(messages)` unchanged).
**Wall budget:** n/a.
**Files:** `crates/cyril-ui/src/traits.rs`, `crates/cyril-ui/src/widgets/chat.rs`.

**Code (advisory):**
```rust
// traits.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteerEchoStatus { Queued, Applied, Cleared, Unsupported }
// in ChatMessageKind:
SteerEcho { text: String, status: SteerEchoStatus },
// constructor:
pub fn steer_echo(text: String) -> Self { Self { kind: ChatMessageKind::SteerEcho { text, status: SteerEchoStatus::Queued }, timestamp: std::time::Instant::now() } }

// chat.rs render arm:
ChatMessageKind::SteerEcho { text, status } => {
    let suffix = match status {
        SteerEchoStatus::Queued => "queued",
        SteerEchoStatus::Applied => "applied",
        SteerEchoStatus::Cleared => "cleared",
        SteerEchoStatus::Unsupported => "not supported",
    };
    // dim "  ↳ steer: <text> — <suffix>"
}
```
**Doc-comment contracts:** none (pure data + presentation).
**Verification:**
- [ ] Unit/render tests pass
- [ ] Stress fixture: four distinct suffixes present; Unicode renders
- [ ] prove-it oracle (idle-steer) still agrees with binary
- [ ] No new loop; render cost unchanged

---

## Slice 2: `UiState::add_steer_echo` (C3)

**Claim (C3):** `add_steer_echo("X")` appends a `SteerEcho{Queued,"X"}` immediately, before any wire round-trip.
**Oracle:** `UiState::messages()` snapshot (public observable, not the method's internals).
**Stress fixture:** (a) `add_steer_echo("fix tests")` → last message is `SteerEcho{Queued,"fix tests"}`; (b) two calls `"a"` then `"b"` → two echoes in insertion order (FIFO precondition for Slice 3); (c) empty `""` still appends a `Queued{""}` (degenerate-but-valid; no special-casing that would drop it).
**Loop budget:** none (`Vec::push` `O(1)`; `enforce_message_limit` is the existing `O(1)` amortized drain).
**Files:** `crates/cyril-ui/src/state.rs`.

**Code (advisory):**
```rust
pub fn add_steer_echo(&mut self, text: &str) {
    self.flush_streaming_agent_text();
    self.flush_streaming_thought();
    self.messages.push(ChatMessage::steer_echo(text.to_string()));
    self.messages_version += 1;
    self.enforce_message_limit();
}
```
**Doc-comment contracts:** none load-bearing (empty text is valid, not a precondition).
**Verification:**
- [ ] Unit tests pass (3 fixture cases)
- [ ] Insertion order preserved (FIFO)
- [ ] prove-it oracle still agrees
- [ ] No new loop

---

## Slice 3: `SteeringConsumed` flips oldest queued echo (C4)

**Claim (C4):** One `SteeringConsumed` flips exactly the oldest still-`Queued` echo to `Applied` and decrements the chip by 1; newer queued echoes untouched.
**Oracle:** `messages()` statuses + `steering_queued()` (observables).
**Stress fixture (designed to fail FIFO/scope bugs):** add E1,E2,E3 (all `Queued`); apply `SteeringConsumed{None}` → **E1=Applied, E2=Queued, E3=Queued**, chip −1. Apply `SteeringConsumed{Some("x")}` → **E2=Applied** (not E3; content is *not* a key), E3=Queued. Adversarial: a 4th `SteeringConsumed` with **no** `Queued` echo remaining → no panic, no status change, chip `saturating_sub` floors at 0.
**Loop budget:** find-first-`Queued` scan = `O(messages) ≤ 500` per notification; a few per turn. Within budget.
**Files:** `crates/cyril-ui/src/state.rs`.

**Code (advisory):**
```rust
Notification::SteeringConsumed { .. } => {
    self.steering_queued = self.steering_queued.saturating_sub(1);
    if let Some(m) = self.messages.iter_mut().find(|m|
        matches!(m.kind(), ChatMessageKind::SteerEcho { status: SteerEchoStatus::Queued, .. }))
    { m.set_steer_status(SteerEchoStatus::Applied); self.messages_version += 1; }
    true
}
```
**Doc-comment contracts:** if `set_steer_status` documents "only call on a `SteerEcho`," that is a **sanity-hint** (programmer error) ⇒ `debug_assert!` on the kind; release tolerates (no-op). Not load-bearing for output correctness.
**Verification:**
- [ ] Unit tests pass (FIFO across interleaving)
- [ ] Stress fixture: E1 then E2 flip in order; content ignored; empty-queue no-op
- [ ] prove-it oracle still agrees
- [ ] Scan `O(≤500)` confirmed

---

## Slice 4: `SteeringCleared` flips all queued echoes (C5)

**Claim (C5):** `SteeringCleared` flips every still-`Queued` echo to `Cleared` and resets the chip to 0.
**Oracle:** `messages()` statuses + `steering_queued()`.
**Stress fixture (designed to fail clobber bugs):** state with E1=`Applied` (from a prior consume), E2,E3=`Queued`; apply `SteeringCleared` → **E2,E3=Cleared, E1 stays Applied** (terminal states are NOT re-flipped), chip=0. Adversarial: empty transcript + `SteeringCleared` → no panic, chip=0.
**Loop budget:** flip-all scan = `O(messages) ≤ 500`. Within budget.
**Files:** `crates/cyril-ui/src/state.rs`.

**Code (advisory):**
```rust
Notification::SteeringCleared => {
    self.steering_queued = 0;
    for m in self.messages.iter_mut() {
        if matches!(m.kind(), ChatMessageKind::SteerEcho { status: SteerEchoStatus::Queued, .. }) {
            m.set_steer_status(SteerEchoStatus::Cleared);
        }
    }
    self.messages_version += 1;
    true
}
```
**Doc-comment contracts:** none new (reuses `set_steer_status`).
**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture: only `Queued`→`Cleared`; `Applied` preserved
- [ ] prove-it oracle still agrees
- [ ] Scan `O(≤500)` confirmed

---

## Slice 5: `SteeringUnsupported` flips all queued + one system message (C6)

**Claim (C6):** `SteeringUnsupported` flips every still-`Queued` echo to `Unsupported` AND adds exactly one system message (K1a behavior preserved).
**Oracle:** `messages()` counted by kind.
**Stress fixture (burst bug):** E1,E2,E3 all `Queued`; **one** `SteeringUnsupported` → all three `Unsupported` (not just the first — the bridge dedups the *notification* once/session, so a single notice must reconcile every in-flight echo), **exactly one** `System` message added. Adversarial: an `Applied` echo present is NOT flipped to `Unsupported`.
**Loop budget:** flip-all scan = `O(messages) ≤ 500`.
**Files:** `crates/cyril-ui/src/state.rs`.

**Code (advisory):** extend the existing `SteeringUnsupported` arm (state.rs:471) — keep `add_system_message(message)`, add the flip-all loop (same shape as Slice 4 but → `Unsupported`).
**Doc-comment contracts:** none new.
**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture: burst → all flipped, exactly one message
- [ ] prove-it oracle still agrees
- [ ] Scan `O(≤500)` confirmed

---

## Slice 6: `TurnCompleted` resets the chip counter (C9)

**Claim (C9):** `TurnCompleted` resets `steering_queued()` to 0.
**Oracle:** `steering_queued()`.
**Stress fixture:** 2× `SteeringQueued` (chip=2) + one `Queued` echo present; apply `TurnCompleted` → `steering_queued()==0`. Adversarial: (a) `TurnCompleted` at chip 0 → stays 0 (no underflow); (b) the `Queued` **echo status is unchanged** by turn-end (intended chip/echo divergence, design negative-space #6) — assert the echo is still `Queued`.
**Loop budget:** none (single assignment in the existing arm).
**Files:** `crates/cyril-ui/src/state.rs`.

**Code (advisory):** in the `TurnCompleted` arm (state.rs:399) add `self.steering_queued = 0;`.
**Doc-comment contracts:** none.
**Verification:**
- [ ] Unit test passes (today's code FAILS this — non-vacuous)
- [ ] Stress fixture: counter→0, echo status untouched
- [ ] prove-it oracle still agrees

---

## Slice 7: `steering_queued()` on the `TuiState` trait (+ UiState/Mock impls)

**Claim:** the render layer can read the queued count through `&dyn TuiState` (enables C8).
**Oracle:** trait-object call return value.
**Stress fixture:** `UiState` after 2× `SteeringQueued`, called as `(&state as &dyn TuiState).steering_queued()` → 2; after a `SteeringConsumed` → 1 (reads live, not cached). `MockTuiState{steering_queued:3}` → 3.
**Loop budget:** none (accessor).
**Files:** `crates/cyril-ui/src/traits.rs` (trait method + `MockTuiState` field), `crates/cyril-ui/src/state.rs` (`impl TuiState for UiState` at :93 — delegate to the inherent getter).

**Code (advisory):**
```rust
// traits.rs trait:
fn steering_queued(&self) -> usize;
// MockTuiState field + impl: returns self.steering_queued
// state.rs impl TuiState for UiState:
fn steering_queued(&self) -> usize { self.steering_queued }
```
**Doc-comment contracts:** none.
**Verification:**
- [ ] Unit test passes (trait-object reads live count)
- [ ] Build green (exhaustive impls satisfied)
- [ ] prove-it oracle still agrees

---

## Slice 8: Toolbar steer chip (C8)

**Claim (C8):** the toolbar renders a steer chip iff `steering_queued() ≥ 1`, showing the count.
**Oracle:** `TestBackend` buffer scrape with `MockTuiState`.
**Stress fixture:** `steering_queued=0` → no chip glyph in buffer; `=2` → buffer contains `"2"` adjacent to a steer marker (e.g. `⇄`/`steer`); `=1` → contains `"1"`; adversarial `=999` on an 80-wide terminal → renders without panic/overflow.
**Loop budget:** none (one conditional `parts.push`).
**Files:** `crates/cyril-ui/src/widgets/toolbar.rs`.

**Code (advisory):** in `toolbar::render`, after the activity/model spans, `if state.steering_queued() >= 1 { parts.push(Span::raw(" · ")); parts.push(Span::styled(format!("⇄ {} steer", n), yellow)); }`.
**Doc-comment contracts:** none.
**Verification:**
- [ ] Render tests pass (0/1/2/999)
- [ ] Stress fixture: absent at 0, present at ≥1, no overflow
- [ ] prove-it oracle still agrees

---

## Slice 9: `classify_submit` pure routing fn (C1, C2)

**Claim (C1+C2):** non-empty non-command submit routes `Busy→Steer`, `Active→Prompt`, no-session→`NoSession`.
**Oracle:** pure-fn return value against the spec truth table (external to the App machinery).
**Stress fixture (truth table, incl. adversarial cells):** `(Busy,true)→Steer`; `(Active,true)→Prompt`; `(Disconnected,false)→NoSession`; **adversarial** `(Busy,false)→NoSession` (no session beats busy — can't steer a nonexistent session); `(Compacting,true)→Prompt` and `(Error,true)→Prompt` (only `Busy` steers; every other present-session non-idle state falls to the unchanged prompt path).
**Loop budget:** none.
**Files:** `crates/cyril/src/app.rs`.

**Code (advisory):**
```rust
enum SubmitRoute { Steer, Prompt, NoSession }
fn classify_submit(status: &SessionStatus, has_session: bool) -> SubmitRoute {
    if !has_session { SubmitRoute::NoSession }
    else if matches!(status, SessionStatus::Busy) { SubmitRoute::Steer }
    else { SubmitRoute::Prompt }
}
```
**Doc-comment contracts:** "call only for non-empty, non-command text" — **sanity-hint** (caller `submit_input` already early-returns empty and dispatches commands first). The fn doesn't read text, so a violation produces a correct route anyway; no enforcement needed beyond the doc note.
**Verification:**
- [ ] Unit tests pass (all truth-table cells)
- [ ] Adversarial cells: `(Busy,false)→NoSession`
- [ ] prove-it oracle still agrees

---

## Slice 10: `steer_gate` pure gate fn (C7)

**Claim (C7):** `dispatch_steer` sends + echoes only when supported and a session exists; otherwise advisory, nothing sent/echoed.
**Oracle:** pure-fn return value (spec truth table).
**Stress fixture:** `(unsupported=false, has_session=true)→Send`; `(true,true)→AdvisoryUnsupported`; `(false,false)→AdvisoryNoSession`; **adversarial** `(true,false)→AdvisoryNoSession` (session check precedes the unsupported check — order is load-bearing for the message shown).
**Loop budget:** none.
**Files:** `crates/cyril/src/app.rs`.

**Code (advisory):**
```rust
enum SteerGate { Send, AdvisoryUnsupported, AdvisoryNoSession }
fn steer_gate(unsupported: bool, has_session: bool) -> SteerGate {
    if !has_session { SteerGate::AdvisoryNoSession }
    else if unsupported { SteerGate::AdvisoryUnsupported }
    else { SteerGate::Send }
}
```
**Doc-comment contracts:** the ordering (session before unsupported) is encoded, not a caller precondition.
**Verification:**
- [ ] Unit tests pass (incl. `(true,false)→AdvisoryNoSession`)
- [ ] prove-it oracle still agrees

---

## Slice 11: `dispatch_steer` + busy-Enter wiring (C1/C3/C7 integration; fixes cyril-2vcc)

**Claim:** busy non-command Enter, and the `/steer` path, both funnel through one `dispatch_steer` that applies `steer_gate`, adds the optimistic echo, and emits `SteerSession` (or an advisory) — never a second `SendPrompt` (the cyril-2vcc fix).
**Oracle:** a test `BridgeSender::from_sender(tx)`; assert the command landing on `rx` (independent of `dispatch_steer`'s internals).
**Stress fixture (designed to fail the cyril-2vcc regression + gate bugs):**
- `dispatch_steer(ui, session=Busy+active, supported, "halt")` → `rx` receives **`SteerSession{message:"halt"}`** (NOT `SendPrompt`), `ui.messages` last is `SteerEcho{Queued,"halt"}`.
- gate=unsupported → `rx` receives **nothing** (drain-empty), `ui` has an advisory `System` message, **no** `Queued` echo (so the optimistic echo can never stick — the keystone).
- gate=no-session → nothing on `rx`, advisory message, no echo.
- `submit_input` with `status=Busy` + text `"x"` → routes via `classify_submit`→`Steer`→`dispatch_steer` (assert `SteerSession`, assert **no** `SendPrompt` and **no** lost message).
**Loop budget:** none (drains a test channel of bounded depth in the fixture only).
**Wall budget:** n/a (event-driven, one send per call).
**Files:** `crates/cyril/src/app.rs`.

**Code (advisory):**
```rust
async fn dispatch_steer(
    ui: &mut UiState, session: &SessionController, bridge: &BridgeSender, text: String,
) -> cyril_core::Result<()> {
    debug_assert!(!text.is_empty(), "callers guarantee non-empty steer text");
    match steer_gate(session.steering_unsupported(), session.id().is_some()) {
        SteerGate::Send => {
            let sid = session.id().expect("has_session checked by gate").clone();
            ui.add_steer_echo(&text);
            bridge.send(BridgeCommand::SteerSession { session_id: sid, message: text }).await?;
        }
        SteerGate::AdvisoryUnsupported =>
            ui.add_system_message("Steering isn't supported by this backend (needs kiro-cli 2.7.0+).".into()),
        SteerGate::AdvisoryNoSession =>
            ui.add_system_message("No active session — nothing to steer.".into()),
    }
    Ok(())
}
// submit_input: after the slash-command branch, before the SendPrompt block:
//   match classify_submit(self.session.status(), self.session.id().is_some()) {
//       SubmitRoute::Steer => return dispatch_steer(&mut self.ui_state, &self.session, &self.bridge_sender, text).await,
//       SubmitRoute::NoSession => { advisory; return } SubmitRoute::Prompt => { /* existing block */ } }
```
**Doc-comment contracts:** `dispatch_steer` "callers guarantee non-empty text" — **sanity-hint**: in release an empty steer would be a backend no-op, not wrong cyril output ⇒ `debug_assert!` (not a runtime refusal). The `.expect("has_session checked by gate")` is a **compile-time invariant** guarded by the immediately-preceding gate (allowed `expect`, mirrors the codebase).
**Verification:**
- [ ] Unit tests pass (4 fixture cases via test channel)
- [ ] cyril-2vcc fixture: busy Enter → `SteerSession`, no `SendPrompt`, message not lost
- [ ] Unsupported gate: nothing sent, no `Queued` echo (keystone)
- [ ] prove-it oracle still agrees with the **binary** (run the idle-steer probe again)
- [ ] No always-on loop introduced

---

## Slice 12: `/steer` command + result routing (C10, C11)

**Claim (C10):** `/steer fix tests` yields a steer of `"fix tests"` routed to `dispatch_steer` (idle or busy). **(C11):** `/steer` with empty args is a usage message — nothing sent/echoed.
**Oracle:** `CommandResult` value (parse layer) + the test `BridgeSender` channel (dispatch layer).
**Stress fixture:** `SteerCommand.execute(ctx, "fix tests")` → `CommandResultKind::Steer{text:"fix tests"}`; `execute(ctx, "")` and `execute(ctx, "   ")` → `SystemMessage(usage)` (whitespace-only is empty after trim); **adversarial** `execute(ctx, "  hi  ")` → `Steer{text:"hi"}` (trimmed, but inner spaces in `"a b"` preserved → assert `execute(ctx,"a b")==Steer{"a b"}`). Routing: `submit_input` on `"/steer go"` → `dispatch_steer` → test channel gets `SteerSession{message:"go"}`.
**Loop budget:** none.
**Files:** `crates/cyril-core/src/commands/builtin.rs` (or a new `steer.rs`) + register in `commands/mod.rs`; **and** `crates/cyril/src/app.rs` (new `CommandResultKind::Steer` arm in the `submit_input` command branch → `dispatch_steer`). 2 files in cyril-core touched (`builtin.rs` + `mod.rs` registration) — registration is a one-line addition; if that trips the 2-file rule, register in the same file as the command. App routing is the cyril-crate file.

> Note: `CommandResultKind::Steer{text}` is a new enum variant in `commands/mod.rs`. Adding it forces the `handle_command_result`/`submit_input` match to handle it (build-green pressure) — that match lives in `app.rs`, so the variant + the App arm land together. Keep the command itself (`builtin.rs`) and the enum+registration (`mod.rs`) within cyril-core; the App arm is the cyril-crate edit. This slice therefore spans `commands/mod.rs` (+`builtin.rs`) and `app.rs`. If that exceeds 2 files in practice, split into 12a (enum variant + `SteerCommand` + registration, cyril-core) and 12b (App routing arm, cyril crate).

**Code (advisory):**
```rust
// commands: SteerCommand { name()="steer" }
async fn execute(&self, _ctx, args) -> Result<CommandResult> {
    let msg = args.trim();
    if msg.is_empty() { Ok(CommandResult::system("Usage: /steer <message>".into())) }
    else { Ok(CommandResult::steer(msg.to_string())) }  // new CommandResultKind::Steer
}
// app.rs submit_input command branch:
if let CommandResultKind::Steer { text } = result.kind {
    self.dispatch_steer_self(text).await?;   // thin wrapper over the free fn
} else { self.handle_command_result(result); }
```
**Doc-comment contracts:** `SteerCommand` "empty args → usage, never an empty steer" is **load-bearing for correctness** (an empty steer would reach the backend) ⇒ enforced by the runtime `if msg.is_empty()` branch (survives release), not a `debug_assert`.
**Verification:**
- [ ] Unit tests pass (parse: full/empty/whitespace/trim/inner-space)
- [ ] Routing test: `/steer go` → `SteerSession{"go"}` on the test channel
- [ ] prove-it oracle still agrees
- [ ] No new loop

---

## Slice 12-empirical (C12): backend accepts idle steer — DONE, no code

**Claim (C12):** `_session/steer` to an idle session is accepted by kiro 2.7.0+. **Already proven** (cheapest falsifier, design step 6): probe `.k1b-steering/probe_idle_steer.rs` ↔ oracle `idle-steer-wire-capture.log` (`{queued:true}` + `steering_queued` echo). No implementation; the cyril-side send is fenced by Slice 11/12 routing tests; C12's permanent artifact is the re-runnable probe (regression fence = `manual`, per design).

---

## Plan Self-Review (step 7 — five lists, all gaps resolved)

**1. Every loop — complexity + production-scale budget:**
- Slice 3 find-first-`Queued`: `O(messages) ≤ 500`, ~few/turn. ✅ under 10^6.
- Slice 4 flip-all-`Queued`: `O(messages) ≤ 500`. ✅
- Slice 5 flip-all-`Queued`: `O(messages) ≤ 500`. ✅
- All other slices introduce **no new loop** (push/assignment/accessor/conditional). Render arms (Slices 1, 8) reuse the existing `O(messages)` render loop — not new.
- No always-on phase ⇒ no wall-budget violations.

**2. Every fixture — bug class it fails under (not happy-path):**
- S1: Unicode-assumption + suffix-distinctness (Applied≠Queued). 
- S2: empty-string drop + insertion-order.
- S3: FIFO/oldest (newer flips), content-as-false-key, empty-queue underflow.
- S4: terminal-state clobber (Applied re-flipped), empty transcript.
- S5: burst (one notice must flip all in-flight echoes), Applied not clobbered.
- S6: counter underflow at 0, chip/echo divergence (echo must NOT change).
- S7: stale-vs-live count.
- S8: absent-at-0, overflow at 999 on 80-wide.
- S9: `(Busy,false)→NoSession` (no-session beats busy).
- S10: `(true,false)→AdvisoryNoSession` (check order).
- S11: **cyril-2vcc regression** (busy Enter must not emit `SendPrompt`/lose the message) + unsupported keystone (no stuck echo).
- S12: whitespace-only empty, trim with preserved inner spaces.
All adversarial, none happy-path-only. ✅

**3. Every doc-comment precondition — classification + enforcement:**
- S3 `set_steer_status` "only on a SteerEcho": **sanity-hint** → `debug_assert!` (release no-op is harmless).
- S9 `classify_submit` "non-empty, non-command text": **sanity-hint** → doc note only (fn ignores text; violation still yields a correct route).
- S11 `dispatch_steer` "non-empty text": **sanity-hint** → `debug_assert!` (empty = backend no-op, not wrong cyril output). `expect("has_session checked by gate")` = compile-time invariant guarded one line above.
- S12 `SteerCommand` "empty args → usage, never empty steer": **load-bearing for correctness** → runtime `if msg.is_empty()` branch (survives release). ✅
No documented precondition is left unenforced.

**4. Every write target — data vs diagnostic:**
- User-facing steer echoes / advisories / chip → `UiState` transcript + toolbar = **data**, in-TUI (correct sink).
- Failure/skip notes → `tracing` (→ `cyril.log`) = **diagnostic** (correct sink).
- **No new `println!`/`stdout`/`stderr`** introduced. ✅

**5. Every tracker reference — resolves to an existing issue covering the work:**
- cyril-2vcc (Enter-while-busy dropped message) — fixed by Slice 11; **verified present** in rivets.
- cyril-28z2 (K1c queue-mode/subagent-steer) — design out-of-scope items; **verified present**.
- No new deferral phrase ("deferred"/"follow-up"/"future"/"revisit if") appears in this plan beyond those two verified citations. ✅

**Claim coverage vs design (C1–C12):** C1→S9+S11 · C2→S9 · C3→S2+S11 · C4→S3 · C5→S4 · C6→S5 · C7→S10+S11 · C8→S8 (+S7 accessor) · C9→S6 · C10→S12 · C11→S12 · C12→done. All 12 covered. ✅

No gaps. Plan ready for `checkpointed-build`.
