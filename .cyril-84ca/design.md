# Design — cyril-84ca: drive `conn.prompt()` off the bridge command loop

Status: falsifiable-design. Prove-it agreement: [findings.md](findings.md) (probe ↔ wire oracle).
Builds on PR #20 (steering wire) and unblocks K1b (**cyril-bm1j**); K1c queue-mode is **cyril-28z2**.

## Purpose

`run_bridge` is a single-consumer loop: `while let Some(cmd) = command_rx.recv().await { match cmd {...} }`.
The `SendPrompt` arm awaits `conn.prompt(request).await` **inline**, so for the whole turn the loop never
returns to `recv()`. Every other command — `SteerSession`, `CancelRequest`, … — sits unread in the channel
until the turn ends. Result: mid-turn steering and mid-turn cancel are impossible (proven in `.k1b-steering/`).

Fix: dispatch `SendPrompt` **off the loop** via `tokio::task::spawn_local`, on an `Rc`-shared connection, so
the loop immediately returns to `recv()` and can process steer/cancel/etc. while the turn runs. The spawned
task owns the turn's terminal notification. The probe proved the ACP `ClientSideConnection` multiplexes
concurrent requests, so **no connection-layer change is needed** — this is purely a bridge-structure change.

## Architecture

- `conn: ClientSideConnection` → `let conn = Rc::new(conn)`. The loop holds one `Rc`; each prompt task gets a
  clone. All ACP methods take `&self`, so this is sound (compile-proven in the probe).
- New loop-local state: `prompt_task: Option<tokio::task::JoinHandle<()>>`. "Turn active" ⇔
  `prompt_task.as_ref().is_some_and(|h| !h.is_finished())` — synchronous, self-clearing (no never-reset bool).
- `SendPrompt` handler (no turn active): clone `conn` + `notification_tx`, `spawn_local` a task that awaits
  `conn.prompt()` and emits the turn's terminal `TurnCompleted` (Ok → mapped stop_reason; transport Err →
  `TurnCompleted{EndTurn}`, preserving today's two-path behavior). Store the `JoinHandle`.
- `SendPrompt` handler (turn active): do **not** start a second prompt; emit `BridgeError{operation:"prompt", …}`.
- All other arms: **unchanged** and still inline — they are short request/responses (`new_session`, `set_mode`,
  `ext_method`, `_session/steer`, `session/cancel`, …). The loop being free is exactly what lets them run mid-turn.
- `Shutdown`: if `prompt_task` is unfinished, `.abort()` it, then `break`.
- **Test seam**: extract the loop body into a form drivable against an in-process `AgentSideConnection` fake
  agent (inject reader/writer instead of always spawning `AgentProcess`), so regression fences run
  deterministically in CI with no `kiro-cli` and no subprocess flakiness.

## Input shapes (command stream × in-flight prompt state)

The feature's input is the `BridgeCommand` stream (16 variants); the fix adds the dimension *prompt in flight?*.
Production-reachable shapes the design must cover:

1. `SendPrompt` | no turn active → spawn turn.
2. `SendPrompt` | turn active → guarded (no 2nd `conn.prompt()`, `BridgeError`).
3. `SendPrompt` | prior task finished-but-not-cleared (`is_finished()==true`) → allowed to start (not falsely blocked).
4. `SteerSession` | turn active → `_session/steer` sent mid-turn.
5. `CancelRequest` | turn active → `session/cancel` sent; turn resolves `Cancelled`.
6. Quick command (`NewSession`/`LoadSession`/`SetMode`/`SetModel`/`ExtMethod`/`ListSettings`/`QueryCommandOptions`/`ExecuteCommand`/`SpawnSession`/`TerminateSession`/`SendMessage`/`ClearSteering`) | turn active → processed without waiting.
7. `Shutdown` | turn active → abort task + return.
8. `SteerSession`/`CancelRequest` | no turn active (idle) → unchanged: these arms are untouched and there is no in-flight task to interact with, so the idle path is byte-identical pre/post fix (covered by rationale, not a new claim).
9. Prompt resolves Ok → `TurnCompleted{stop_reason}`. 10. Prompt resolves transport-Err → `TurnCompleted{EndTurn}`.

**Out of scope (justified):**
- *Concurrent prompts on different sessions.* The bridge owns one `conn` and one `active_session_id`; subagent
  turns run agent-side and surface as `session/update`, never as bridge `conn.prompt()` calls. One in-flight
  prompt is the production invariant (settled rationale, not deferred work).
- *`command_rx` closed / App died.* Existing `notify_or_closed`/`break` handling is unchanged by an off-loop prompt.
- *Permission-request cancellation ACP dance.* Permission handling is unchanged; see Negative space.

## Claims

1. **C1 — loop frees:** with no turn active, `SendPrompt` spawns `conn.prompt()` and the loop returns to `recv()` before turn end (a command sent after `SendPrompt` is processed mid-turn).
2. **C2 — steer mid-turn:** a `SteerSession` dequeued during a turn sends `_session/steer` to the agent **before** that turn's `TurnCompleted`.
3. **C3 — cancel resolves:** a `CancelRequest` during a turn sends `session/cancel`, the in-flight `conn.prompt()` resolves `Cancelled`, and the task emits exactly one `TurnCompleted{Cancelled}` (no hang).
4. **C4 — single turn:** a `SendPrompt` received during a turn does not call `conn.prompt()` again; it emits a `BridgeError` and the original turn's lone `TurnCompleted` is unaffected.
5. **C5 — ordering:** every streaming notification of a turn is emitted before that turn's `TurnCompleted`.
6. **C6 — always notifies:** each turn emits exactly one terminal `TurnCompleted` (success and transport-error paths), so the UI never stays stuck busy.
7. **C7 — shutdown aborts:** `Shutdown` during a turn aborts the in-flight task and returns from `run_bridge` without deadlock.
8. **C9 — guard self-clears:** after a turn's task finishes, the "turn active" guard reports not-busy, so the next `SendPrompt` starts a new turn.

## Falsification

| # | Claim | Falsifier (input → falsifying result) | Oracle (independent) | Cost | Status | Regression fence |
|---|-------|----------------------------------------|----------------------|------|--------|------------------|
| C1 | loop frees | Slow-prompt fake agent; send `SendPrompt` then a quick cmd (`ListSettings`). If its response arrives only *after* `TurnCompleted`, false. **Buggy impl that fails it:** inline `conn.prompt().await` (today). | Notification order recorded by the test harness (separate from bridge) | 1h (needs harness) | pending | integ test `bridge::loop_frees_during_turn` |
| C2 | steer mid-turn | Same harness; after `SendPrompt`, send `SteerSession`. If the fake agent receives `_session/steer` *after* it returns the prompt, false. **Buggy impl:** inline prompt → steer dequeued post-turn. | Fake agent records request arrival order | 1h | pending | integ test `bridge::steer_reaches_agent_before_turn_end` |
| C3 | cancel resolves | Fake agent sleeps on prompt until cancel; send `CancelRequest` mid-turn. If `prompt()` doesn't resolve within a bound (hang) or no `TurnCompleted`, false. **Buggy impl:** inline prompt → cancel never dequeued → timeout. | (a) wire tee `_/k1b_wire.log`; (b) bound on `TurnCompleted` arrival | **3m** (empirical) + 1h (CI) | **passed (empirical)** | integ test `bridge::cancel_resolves_busy_turn` |
| C4 | single turn | Send two `SendPrompt`s back-to-back into one turn. If the fake agent sees two `session/prompt` requests, false. **Buggy impl:** no `is_finished` guard → second `spawn_local(prompt)`. | Fake agent counts `session/prompt` requests (==1) + `BridgeError` observed | 1h | pending | integ test `bridge::second_prompt_rejected_midturn` |
| C5 | ordering | Fake agent emits N `session/update` then returns. If notifications arrive as `[…, TurnCompleted, update]` (any update after TurnCompleted), false. **Buggy impl:** task emits `TurnCompleted` before the io_task drains updates. | (a) wire tee shows all `session/update` precede result; (b) harness notification order | **1m** (empirical) + 1h (CI) | **passed (empirical)** | integ test `bridge::streaming_precedes_turn_completed` |
| C6 | always notifies | Run a turn to Ok, and a turn whose agent drops the connection (transport Err). If a path emits 0 or 2 `TurnCompleted`, false. **Buggy impl:** spawn task `return`s on Err without notifying → UI stuck busy. | Harness counts `TurnCompleted` per turn (==1) | 1h | pending | integ test `bridge::turn_emits_exactly_one_completion` |
| C7 | shutdown aborts | Start a slow turn, send `Shutdown`. If `run_bridge` doesn't return within a bound, false. **Buggy impl:** handle not stored/aborted → loop waits on turn or task leaks. | `run_bridge` return observed within timeout (test driver) | 1h | pending | integ test `bridge::shutdown_aborts_inflight_prompt` |
| C9 | guard self-clears | Run a turn to completion, then send `SendPrompt` again. If the second is rejected as "turn in progress", false. **Buggy impl:** a `bool busy` set true, never reset. | Fake agent sees a second `session/prompt` after the first completed | 30m | pending | integ test `bridge::new_turn_after_completion` |

**Cheapest falsifiers run before approval:** C5 (1m, free against the existing oracle capture — all `session/update`
frames precede the `end_turn` result at 6.086s) and C3 (3m — probe `cancel.rs`: `session/cancel` at 1.930s →
`stopReason:"cancelled"` at 1.931s, prompt resolved 2ms after cancel). Both **passed**.

**Regression-fence note:** C3 and C5 above have empirical (one-shot) falsifiers; per skill rule they each get a
deterministic CI test (named) as the permanent fence. All fences depend on the in-process fake-agent harness
(an `AgentSideConnection` whose `prompt` is gated on a test-controlled signal) — building that harness is part
of this issue, not deferred.

## Negative space (what this deliberately does NOT do)

1. **No client-side command/steer queue or batching.** Each command is sent the instant it's dequeued; no
   buffering or reordering. Queue-mode parity (Kiro's Ctrl+S) is **cyril-28z2** (K1c).
2. **No change to App-side routing policy.** Deciding *when* to send `SteerSession` vs `SendPrompt`
   (Enter-while-busy) is **cyril-bm1j** (K1b); this issue only makes the bridge able to deliver mid-turn.
3. **No concurrent turns.** Exactly one in-flight prompt is enforced (C4); the fix enables mid-turn *commands*,
   not parallel *prompts*.
4. **No change to permission handling** or the ACP permission-cancellation dance (responding to pending
   `request_permission` with `Cancelled` on cancel). Unchanged from today.
5. **No change to subagent turn execution** (agent-side; surfaces only as `session/update`).

## Hard gate checklist

- [x] Every production-reachable input shape (1–10 above) is covered by a claim or noted out-of-scope w/ justification.
- [x] Every claim has a falsifier in the table.
- [x] Every falsifier names an independent oracle (wire tee / fake-agent harness order, not the bridge itself).
- [x] Every falsifier names a specific buggy implementation that would make it fail (non-vacuity column).
- [x] Every claim has a distinct verifiable output (per-claim named test + distinct oracle signal).
- [x] Every measurement-based claim (C3, C5) has a named deterministic CI regression fence.
- [x] Deferrals cite verified tracker IDs (cyril-bm1j, cyril-28z2 — both confirmed present in rivets).
- [x] The cheapest falsifier (C5, also C3) has been run and passed.
- [x] Negative space has ≥3 entries (5 listed).
