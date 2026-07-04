# cyril-l7tw — falsifiable design: make engine death and prompt failure visible

## Purpose

Four mute failure paths in the bridge become visible to the App, per the
issue's acceptance criteria. Grounding: `.cyril-l7tw/findings.md` (probe run 2
reproduced the invisibility on the real bridge; run 1 showed handshake death
is already visible but its reason omits the actionable stderr detail).

## Core design

1. **Prompt-Err surfacing** (issue item 1): the off-loop prompt task's `Err`
   arm emits `BridgeError { operation: "prompt", message: <error> }` through
   the internal channel (ADR-0004) *before* its synthesized
   `TurnCompleted(EndTurn)`. The turn still ends; the user now knows why.
2. **io-pump watcher** (item 2): the io task is no longer detached. A watcher
   task awaits it (Ok **or** Err — probe learned SIGKILL is a *clean EOF*) and
   fires a oneshot into `run_loop` carrying a reason built from the
   `stderr_tail` snapshot. `run_loop` gains a fourth select arm:
   - no turn in flight → emit `BridgeDisconnected { reason: "agent connection
     closed" + tail }` via awaited send, then exit the loop.
   - turn in flight → set a `conn_dead` flag; the prompt task's Err arm
     delivers `BridgeError` + `TurnCompleted` through the inbound channel as
     usual; when the loop observes that `TurnCompleted`, it emits the deferred
     `BridgeDisconnected` and exits. Order seen by the App:
     `BridgeError → TurnCompleted → BridgeDisconnected`.
   Loop exit is safe App-side: `Some(n) = notification_rx.recv()` just
   disables that select arm; the UI keeps running with the disconnect
   rendered. The agent is unrecoverable either way (respawn = cyril-gua0).
3. **Auth-failure surfacing** (item 3): `KiroClient::ext_method` (which has
   `notification_tx` access — the free-function responder does not) observes a
   `getAccessToken` responder `Err` and emits `BridgeError { operation:
   "auth", message: <responder message, incl. the "run kiro-cli login" hint> }`
   before returning the JSON-RPC error to KAS. The KAS-side turn failure then
   *also* surfaces via (1). v2's equivalent (logged-out CLI dies at handshake)
   is covered by (4).
4. **stderr tail in fail-stop reasons** (items 2+3, 0gke handoff):
   `run_bridge` appends the `stderr_tail` snapshot (last 5 lines in the
   user-facing reason; full tail stays in the tracing log) to any `Err` it
   propagates from `run_loop` — so the handshake-failure `BridgeDisconnected`
   for a logged-out kiro-cli finally shows `error: You are not logged in,
   please log in with kiro-cli login` (probe run 1's stderr).
5. **Undroppable fail-stop** (item 4): the thread-exit emission in
   `spawn_bridge` replaces `try_send` with `rt.block_on(timeout(5s, send))` —
   a full channel with a live App delivers after drain; a dropped receiver
   errs immediately; a wedged App gives up after 5s with the existing warn.

## Input shapes (step 2)

Death timing × turn state:
- **mid-turn** (turn_in_flight = Some) → C1, C2, C4
- **idle** (turn_in_flight = None) → C3
- **during handshake** (initialize Err) → C7
- **at spawn** (binary missing; no process, empty tail) → C8

io-pump completion mode: **clean EOF (`Ok`)** → C5 (the probe-proven common
mode; fences use duplex drop = clean EOF); **io `Err`** → same watcher awaits
the JoinHandle either way — covered by construction, noted not separately
fenced (no deterministic way to force a mid-frame io error through a duplex).

prompt-Err kind: transport death (C1) vs agent-level error text (KAS
TokenInvalidError etc.) — same arm, message passed through verbatim (C1's
message assert covers pass-through).

Dead-conn follow-up: SendPrompt racing the watcher (conn dead, loop alive) →
C6; commands after loop exit → channel closed, App-side `send_or_log`
(existing pattern, C6 asserts sender errors).

Fail-stop channel state: empty (existing tests) / **full, live App** → C9 /
**receiver dropped** → C10.

stderr tail: non-empty → C7; empty (spawn failure, no process output) → C8
(reason must still be well-formed, no "…stderr:" stub).

Auth callback result: Err → C11; Ok → C12 (no noise).

Engine: v2 (C1-C10 harness) ; KAS dual turn-end → C13.

## Removed-invariant sweep (step 2b)

The change is **subtractive in one place**: today `run_loop` *never exits* on
connection death (only on App-gone / Shutdown). Adding the io-death exit
removes "the bridge command channel outlives any agent failure":

- "App can always send commands after agent death" → now sends fail
  channel-closed. Safe: every App-side send already goes through
  `send_or_log` (CLAUDE.md invariant), and `BridgeDisconnected` has already
  told the user the bridge is gone. Fenced by C6's sender-errors assert.
- "notification channel never closes while the UI runs" → App select arm
  `Some(n) = recv()` disables cleanly on close (verified in app.rs:151);
  UiState already handles `BridgeDisconnected` (state.rs:465). Noted safe.
- "exactly one terminal TurnCompleted per turn" (cyril-a71q invariant) → the
  deferred-disconnect path *waits* for the observation that clears
  `turn_in_flight`; `BridgeError` is not a terminal marker. Fenced by C2.
- "Shutdown aborts the prompt task" → unchanged arm; the io_done arm is
  guarded (`, if conn_dead.is_none()`) so a resolved oneshot is never
  re-polled. Fenced by existing shutdown tests staying green.

KAS wrinkle (accepted, noted): if a KAS turn's `turn_end` cleared
`turn_in_flight` before death, io_done takes the idle path and a late
prompt-task `BridgeError` may miss the closed channel — the disconnect
already tells the story; the duplicate TurnCompleted was being dropped by
dedup anyway (C13 keeps that green).

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| C-mech | Agent drop resolves pending prompt as Err AND completes the io task (EOF = clean Ok) | raw duplex + KiroClient, drop agent side mid-prompt | acp rpc layer behavior itself (no bridge code under test); cross-checked live by probe run 2 | 5m | **passed** (`Ok(Ok(()))`) | unit `l7tw_agent_drop_resolves_prompt_err_and_completes_io` (in tree now) |
| C1 | Mid-turn death emits BridgeError{op="prompt"} with the transport error text before TurnCompleted | harness: park a prompt, kill the agent side; record channel order | channel transcript ordering (App's view), not bridge internals; buggy impl that fails it: today's code (no BridgeError at all) — verified failing pre-fix | 15m | pending | unit `death_mid_turn_emits_bridge_error_before_turn_completed` |
| C2 | The killed turn still ends with exactly ONE TurnCompleted(EndTurn) | same fixture, count TurnCompleted | count over full drain; buggy impl: emitting error INSTEAD of completion (busy sticks), or double-completion | 0 (same fixture, distinct assert) | pending | unit `death_mid_turn_single_turn_completed` |
| C3 | Idle death emits BridgeDisconnected (reason contains "agent connection closed" + tail marker) and run_loop exits | harness: no turn in flight, kill agent side; await loop handle | loop JoinHandle completion + reason substring; buggy impl: today's detached io pump (nothing emitted, loop runs forever — test times out) | 15m | pending | unit `death_while_idle_emits_disconnected_and_exits` |
| C4 | Mid-turn death delivers BridgeError → TurnCompleted → BridgeDisconnected in that order | same fixture as C1, assert full sequence incl. disconnect after completion | ordered transcript; buggy impl: disconnect emitted immediately on io_done (order inverted → assert fires) | 0 (same fixture, distinct assert) | pending | unit `death_mid_turn_disconnect_after_completion` |
| C5 | Detection works on CLEAN EOF (not only io Err) | C1/C3 fixtures use duplex drop = clean EOF (C-mech proved Ok(())) | C-mech's recorded completion mode | 0 | pending (by construction of C1/C3) | same fences as C1/C3 |
| C6 | SendPrompt racing the death window (conn dead, loop alive) yields BridgeError + TurnCompleted, not silence; after loop exit the sender errors | harness: kill agent, immediately SendPrompt before io_done handled; then assert sender.send errs post-exit | transcript + `BridgeSender::send` Result | 20m | pending | unit `dead_conn_prompt_errors_not_silent` |
| C7 | Handshake failure reason includes the stderr tail (v2 "not logged in" becomes visible) | unit: append_tail(reason, tail) shape test; LIVE: rerun probe run 1 (logged-OUT kiro-cli 2.11.0 — no login needed) post-fix, expect "not logged in" in BridgeDisconnected | live: kiro-cli's own stderr text vs the notification the App receives (independent: kiro wrote it, channel must carry it) | unit 10m; live 2m | pending | unit `handshake_failure_reason_includes_stderr_tail`; live run recorded in findings.md |
| C8 | Spawn failure (missing binary) still emits BridgeDisconnected; empty tail keeps the reason well-formed | spawn_bridge with a bogus binary name; drain notification | notification presence + reason has no dangling tail stub; buggy impl: tail-append that panics/garbles on empty tail | 10m | pending | unit `spawn_failure_disconnect_reason_wellformed` |
| C9 | Fail-stop BridgeDisconnected survives a FULL channel with a live (slow) App | fill notification channel to capacity 256, run emission, then drain; assert disconnect present | receiver-side drain content; buggy impl: current `try_send` (drops on Full — test fails against today's code by construction) | 10m | pending | unit `failstop_disconnect_survives_full_channel` |
| C10 | Fail-stop emission with a dropped receiver returns promptly (no hang) | drop receiver, run emission under a 6s watchdog | wall-clock bound; buggy impl: unbounded blocking_send (hangs forever → watchdog fires) | 0 (same test file) | pending | unit `failstop_disconnect_no_hang_on_dropped_receiver` |
| C11 | getAccessToken responder Err ⇒ BridgeError{op="auth"} with the responder's actionable message, AND the JSON-RPC error still goes to KAS | unit on the new pure mapping (method+Err → Notification); harness: fake agent sends the ext request in a build where the store read fails deterministically — store injection is cyril-5db7's scope, so the wire-level fence uses the mapping fn + ext_method seam test with a forced-Err method stub | mapping output + harness transcript; buggy impl: swallowing the error into the JSON-RPC reply only (today's behavior) | 20m | pending | unit `auth_callback_err_emits_bridge_error` (kas feature) |
| C12 | No BridgeError noise on successful turns / successful auth callback | existing happy-path harness turn test extended with a zero-BridgeError assert | transcript scan; buggy impl: emitting BridgeError unconditionally in ext_method | 5m | pending | extended `harness_drives_one_turn` assert |
| C13 | KAS dual turn-end dedup is undisturbed (one TurnCompleted; late duplicate dropped) | existing KAS-2a idempotent-completion tests + C2's count | existing test suite | 0 | pending | existing KAS-2a tests stay green |

Live validation (post-build, before merge): C7-live (logged-out kiro-cli
2.11.0 — runnable NOW without login) and a repeat of probe run 2 via
`l7tw_death_probe` with the replay agent, expecting the full C4 sequence on
the transcript. A logged-IN mid-turn kill against real kiro-cli 2.11.0 is
desirable but **user-gated** (needs `kiro-cli login`); the deterministic
fences are the CI-permanent form (dcc6 C14b pattern).

## Negative space (deliberately not doing)

1. **No agent respawn/reconnect** — visibility only; recovery is cyril-gua0.
2. **No new StopReason variant** — the turn still ends `EndTurn`;
   `BridgeError` carries the why. UiState's activity machine is untouched.
3. **No queuing/retry of commands that raced the death** — dropped;
   `BridgeDisconnected` supersedes them.
4. **No notification-channel backpressure redesign** — cyril-1ixa.
5. **No per-turn identity/dedup strengthening** — cyril-a71q.
6. **No KAS rate-limit surfacing** (`_kiro/error/rate_limit`) — cyril-3zy4.
7. **No injectable auth-store wiring** — the C11 fence works at the
   ext_method seam; store injectability is cyril-5db7.

## Post-build addendum (pre-PR review, 2026-07-04)

- **Slice-4 stress fixture (b) not built as specified** (duplicate
  TurnCompleted injected via an `inbound_tx` clone): the state it guards —
  deferred disconnect armed ∧ no turn in flight ∧ a completion arriving — is
  unreachable by construction: the flag is only set while a turn is in
  flight, and the first observed completion both clears the flag and fires
  the disconnect+exit. The reachable adversarial neighbor (KAS dual
  completion racing the death) is fenced by
  `death_after_turn_end_single_disconnect`, which caught a real drain-dedup
  bug during the build.
- **C10's fence is named** `failstop_disconnect_no_hang_on_wedged_or_dropped_receiver`
  (a superset of the design's dropped-receiver falsifier: it adds the
  wedged-App bound).
- **C9/C10 carve-out**: when the bridge *runtime itself* fails to construct,
  the fail-stop emission falls back to `try_send` — nothing ever ran, so the
  channel is empty; a bounded send is impossible without a runtime.
- `conn_dead` was renamed `deferred_disconnect` during review (it holds the
  deferred reason, not a boolean).

## Open decisions flagged for approval

- **Loop exit on death** (vs staying alive against a dead conn): design says
  exit — App-side is verified safe, and a dead conn serves nothing. The old
  probe phase-2 behavior (silent accept) becomes structurally impossible.
- **5s timeout** on the fail-stop send (vs unbounded blocking_send).
- **Tail excerpt size** in user-facing reasons: last 5 lines.
- **BridgeError before TurnCompleted** ordering (UI shows the error while the
  turn is still visually active, then it ends).
