# Design — cyril-bm1j (K1b): queue-steering TUI UX

Status: falsifiable-design. Prove-it agreement: [findings.md](findings.md) (mid-turn probe ↔ wire oracle)
and [idle-steer-wire-capture.log](idle-steer-wire-capture.log) (idle-steer probe ↔ wire oracle, run for this design).
Builds on K1a wire+state (cyril-f2g8), the K1a wire fixes (cyril-c1qe), and the off-loop bridge (cyril-84ca, PR #22,
now on main). Fixes the dropped-message regression **cyril-2vcc**. K1c queue-mode/subagent-steer is **cyril-28z2**.

## Purpose

K1a landed the steering plumbing: a `SteerSession` bridge command (awaited `_session/steer`, -32601 → `SteeringUnsupported`),
the inbound converter that turns `_kiro.dev/session/update` echoes into `SteeringQueued/Consumed/Cleared` notifications,
and reactive counters (`SessionController.steering_depth`, `UiState.steering_queued`). cyril-84ca made the bridge able to
deliver a steer *mid-turn*. **Nothing is user-facing yet**, and worse: pressing Enter while busy currently sends a second
`SendPrompt`, which the post-84ca one-turn guard rejects — the user's message is committed to the transcript but never
sent (cyril-2vcc).

K1b adds the UX on top of working plumbing:

1. **Enter-while-busy steers.** When `session.status() == Busy` and the submitted text is non-slash and non-empty, route
   to `SteerSession` instead of `SendPrompt`. Idle Enter is unchanged. This is also the fix for cyril-2vcc.
2. **Optimistic transcript echo.** On send, append a `SteerEcho{Queued}` carrying the user's own text (always available),
   reconciled in place: → `Applied` on `SteeringConsumed`, → `Cleared` on `SteeringCleared`, → `Unsupported` on
   `SteeringUnsupported`.
3. **Toolbar chip** while `steering_queued() ≥ 1`, cleared as the count returns to 0 (consumed/cleared) and force-reset on
   turn-end so it can't stick.
4. **`/steer <msg>` command** for the explicit path; works idle (backend queues for the next turn — probe-confirmed) and busy.
5. **Advisory copy** — the agent weighs a steer and may decline; an unsupported backend says so once.

## Architecture

- **Pure routing seam (App, `cyril/src/app.rs`).** Two small pure functions make the decisions unit-testable without a
  terminal:
  - `classify_submit(status: &SessionStatus, has_session: bool) -> SubmitRoute` where `SubmitRoute ∈ {Steer, Prompt, NoSession}`.
    Called only for non-empty, non-command text. `!has_session → NoSession`; `Busy → Steer`; else `Prompt`.
  - `steer_gate(unsupported: bool, has_session: bool) -> SteerGate` where `SteerGate ∈ {Send, AdvisoryUnsupported, AdvisoryNoSession}`.
  `submit_input` consults `classify_submit`; the `Steer` arm and the `/steer` command arm both funnel into one async
  `App::dispatch_steer(text)` that applies `steer_gate`, adds the optimistic echo, and sends `BridgeCommand::SteerSession`.
  One code path for both entry points (no drift).
- **Optimistic echo (UiState, `cyril-ui/src/state.rs`).** New `ChatMessageKind::SteerEcho { text: String, status: SteerEchoStatus }`
  with `SteerEchoStatus ∈ {Queued, Applied, Cleared, Unsupported}`. `UiState::add_steer_echo(&str)` pushes a `Queued` entry.
  Reconciliation lives in `apply_notification` (UiState stays the single state-machine owner — App never reaches in):
  `SteeringConsumed` → flip the **oldest** still-`Queued` echo to `Applied` (FIFO; `content` is advisory, not a key);
  `SteeringCleared` → flip **all** `Queued` → `Cleared`; `SteeringUnsupported` → flip **all** `Queued` → `Unsupported`
  (covers the burst case: several steers in flight before the first -32601 returns) **and** keep the existing one
  system message.
- **Chip counter (UiState).** K1a's reactive `steering_queued` is unchanged on `SteeringQueued/Consumed/Cleared`; K1b
  adds one transition: `TurnCompleted` resets it to 0 (so an un-consumed mid-turn steer can't leave a stuck chip).
- **Chip render (toolbar, `cyril-ui/src/widgets/toolbar.rs`).** Add `fn steering_queued(&self) -> usize` to the `TuiState`
  trait (and `MockTuiState`); the renderer reads only the trait. Render a chip in `toolbar::render` when `≥ 1`.
- **`/steer` command (`cyril-core/src/commands/`).** A registry `SteerCommand` (so it shows in help/autocomplete) that
  validates a non-empty message and returns a new `CommandResultKind::Steer { text }`; empty → `SystemMessage` usage. The
  command does **not** touch the bridge or UI — `submit_input` routes the `Steer` result into `App::dispatch_steer`,
  keeping the async send + UI echo in the App (the established `ShowPicker`-style split).

## Input shapes (submit/command × session status × steering notifications)

Enter submit (non-empty), by status:
1. `Busy` + non-slash text → **steer** (C1) — also the cyril-2vcc fix.
2. `Active` (idle) + non-slash text → **prompt**, unchanged (C2).
3. No active session (`Disconnected`/`Initializing`/`Error`) + non-slash → advisory "no active session", unchanged (rationale; same `session.id()==None` branch as today).
4. Slash text, any status → registry dispatch, unchanged (commands already run mid-turn post-84ca). `/steer` is the new member (C10/C11).
5. Empty text → no-op, unchanged (rationale; `take_input().is_empty()` early return).

`/steer`:
6. `/steer <msg>`, idle → dispatch_steer; backend **accepts and queues for next turn** (C10 routing + C12 backend).
7. `/steer <msg>`, busy → dispatch_steer mid-turn (C10).
8. `/steer` (no arg) → usage system message, no send, no echo (C11).

Steering notifications (reconciliation), by echo-state shape — 0, 1, N queued echoes:
9. `SteeringQueued{Some|None}` → chip counter +1 (C8); no transcript echo added here (it was added optimistically on send).
10. `SteeringConsumed{Some|None}` → oldest `Queued` echo → `Applied`, counter −1 (C4; both payload shapes behave identically — `content` is not a correlation key).
11. `SteeringCleared` → all `Queued` → `Cleared`, counter 0 (C5).
12. `SteeringUnsupported` → all `Queued` → `Unsupported` + one system message (C6).
13. `TurnCompleted` (counter 0 / >0) → counter 0 (C9).

Dispatch gate shapes:
14. `dispatch_steer` with `steering_unsupported()==true` → no send, no echo, advisory (C7) — the keystone that keeps the optimistic echo reconcilable (see below).

**Out of scope (justified):**
- *Queue-mode buffering of follow-up **prompts** while busy* (Kiro Ctrl+S parity) — **cyril-28z2** (K1c). K1b's busy-Enter is a steer, not a deferred prompt.
- *Subagent-scoped steering* (`/steer @name`, per-session routing) — **cyril-28z2** (K1c). K1b steers the main session only.
- *@-file expansion inside steer text.* Steers send raw text; `@`-expansion stays prompt-only (settled rationale — a steer is a short instruction, not a context load).
- *Multi-client transcript echo.* A `SteeringQueued` echo originated by another client bumps the chip but adds no `SteerEcho` (this client never sent it). Single-client today; the counter degrades gracefully (settled rationale, not deferred work).

## Claims

1. **C1 — busy Enter steers:** with `status==Busy`, a non-empty non-slash submit routes to `SteerSession` for the active session, never `SendPrompt`.
2. **C2 — idle Enter prompts:** with `status==Active`, a non-empty non-slash submit still routes to `SendPrompt` (byte-identical to today).
3. **C3 — optimistic echo on send:** `dispatch_steer("X")` appends a `SteerEcho{Queued, "X"}` to the transcript before any wire round-trip.
4. **C4 — consumed flips oldest queued (FIFO):** one `SteeringConsumed` flips exactly the oldest still-`Queued` echo to `Applied` and decrements the chip by 1 — newer queued echoes are untouched.
5. **C5 — cleared flips all queued:** `SteeringCleared` flips every still-`Queued` echo to `Cleared` and resets the chip to 0.
6. **C6 — unsupported flips all queued + one notice:** `SteeringUnsupported` flips every still-`Queued` echo to `Unsupported` and adds exactly one system message (no echo left stuck on the burst path).
7. **C7 — unsupported gate suppresses send+echo:** `dispatch_steer` when `steering_unsupported()` is true sends no `SteerSession` and adds no `Queued` echo (only an advisory) — so every optimistic echo is guaranteed a reconciling notification.
8. **C8 — chip tracks queued count:** the toolbar renders a steer chip iff `steering_queued() ≥ 1`, showing the count.
9. **C9 — chip clears on turn-end:** `TurnCompleted` resets `steering_queued()` to 0.
10. **C10 — /steer dispatches a steer:** `/steer fix tests` yields a steer of "fix tests" routed through `dispatch_steer`, idle or busy.
11. **C11 — /steer no-arg is advisory:** `/steer` with empty args adds a usage message and sends nothing / echoes nothing.
12. **C12 — backend accepts an idle steer:** `_session/steer` sent to an active-but-idle session is accepted by kiro 2.7.0+ (`{queued:true}` + `steering_queued` echo), so `/steer` idle is meaningful, not a silent no-op. **(Empirical; PASSED.)**

## Falsification

| # | Claim | Falsifier (input → falsifying result) | Oracle (independent) | Cost | Status | Regression fence |
|---|-------|----------------------------------------|----------------------|------|--------|------------------|
| C1 | busy Enter steers | `classify_submit(Busy, true)`; if ≠ `Steer`, false. **Buggy impl:** today's `submit_input` (no busy branch) → `SendPrompt` → 84ca guard rejects → message lost (cyril-2vcc). | pure-fn return value asserted in test | 10m | pending | unit `app::classify_submit_busy_routes_to_steer` |
| C2 | idle Enter prompts | `classify_submit(Active, true)`; if ≠ `Prompt`, false. **Buggy impl:** over-broad busy check routes idle to steer. | pure-fn return value | 10m | pending | unit `app::classify_submit_idle_routes_to_prompt` |
| C3 | optimistic echo | `add_steer_echo("X")`, inspect `messages()` immediately; if no `SteerEcho{Queued,"X"}`, false. **Buggy impl:** reactive echo added only on `SteeringQueued` (1-RTT late, lost on unsupported). | UiState `messages()` snapshot | 15m | pending | unit `state::add_steer_echo_appends_queued` |
| C4 | consumed flips oldest | add 2 echoes; apply 1 `SteeringConsumed`; if the newer flips, both flip, or none flip, false. **Buggy impl:** flip newest / flip all / flip none. | UiState `messages()` + `steering_queued()` | 20m | pending | unit `state::consumed_flips_oldest_queued_echo` |
| C5 | cleared flips all | add 3 echoes; apply `SteeringCleared`; if any stays `Queued` or chip≠0, false. **Buggy impl:** decrement-by-1 instead of clear-all. | UiState `messages()` + count | 15m | pending | unit `state::cleared_flips_all_queued_echoes` |
| C6 | unsupported flips all + 1 msg | add 2 echoes; apply `SteeringUnsupported`; if any stays `Queued`, or 0/2 system messages, false. **Buggy impl:** flips only one → burst leaves a stuck "queued" echo. | UiState `messages()` count by kind | 20m | pending | unit `state::unsupported_flips_all_echoes_and_one_message` |
| C7 | unsupported gate | `steer_gate(unsupported=true, has_session=true)`; if `Send`, false. (And `dispatch_steer` under that gate sends nothing / echoes nothing.) **Buggy impl:** no gate → optimistic echo stuck forever (bridge drops a known-unsupported steer silently — bridge.rs:1089). | pure-fn return value | 15m | pending | unit `app::steer_gate_blocks_when_unsupported` |
| C8 | chip render | render `toolbar` with `steering_queued()`=0 (no chip) and =2 (chip shows "2"); if chip at 0 or absent at 2, false. **Buggy impl:** chip keyed off `is_busy` not the count. | `TestBackend` buffer scrape | 20m | pending | render `toolbar::renders_steer_chip_when_queued` |
| C9 | chip clears on turn-end | set counter via 2× `SteeringQueued`; apply `TurnCompleted`; if `steering_queued()≠0`, false. **Buggy impl:** today's `TurnCompleted` arm (state.rs:399) leaves the counter → stuck chip. | UiState `steering_queued()` | 10m | pending | unit `state::turn_completed_resets_steering_queued` |
| C10 | /steer dispatches | `SteerCommand.execute(ctx, "fix tests")`; if result ≠ `Steer{"fix tests"}`, false. **Buggy impl:** /steer maps to `SendPrompt` / returns `SystemMessage`. | `CommandResult` value | 15m | pending | unit `commands::steer_parses_message` |
| C11 | /steer no-arg advisory | `SteerCommand.execute(ctx, "")`; if it returns `Steer{..}` (would send/echo), false. **Buggy impl:** empty steer sent → backend gets an empty steer. | `CommandResult` value | 10m | pending | unit `commands::steer_empty_is_usage` |
| C12 | backend accepts idle steer | send `SteerSession` to an idle session via the wire tee; if wire shows `-32601` or no `steering_queued` echo, false. **Buggy impl:** N/A — backend behavior; the probe *is* the falsifier. | `wire_shim.py` log (independent of cyril's notif channel) | **3m** | **passed** | **manual** — external backend, not CI-able; permanent artifact `.k1b-steering/probe_idle_steer.rs` + `idle-steer-wire-capture.log`; cyril-side send covered by C10 |

**Cheapest falsifier run before approval:** C12 (3m). The design's one empirical unknown was whether `/steer` on an idle
session does anything. The probe drove cyril's real bridge → wire tee → kiro-cli 2.8.0 and the **oracle** (`/tmp/k1b_wire.log`,
saved to `idle-steer-wire-capture.log`) shows `C2A _session/steer` (single underscore), `A2C steering_queued` echo, and
`A2C {"queued":true}` — **accepted**. The design's claim (idle /steer queues for next turn) survived; had it shown -32601,
the `/steer` idle path would have collapsed to advisory-only. **Passed.**

**Regression-fence note:** every claim except C12 has a named deterministic CI test (unit/render). C12 is external-backend
behavior with no CI-able fence; per the skill it is marked `manual` with its re-runnable probe + captured oracle as the
permanent audit artifact, and the cyril-side behavior it depends on (idle `/steer` actually sends a `SteerSession`) is
fenced by C10. All non-C12 fences are introduced as part of this issue, not deferred.

## Negative space (what this deliberately does NOT do)

1. **No queue-mode buffering of follow-up prompts** while busy (Kiro's Ctrl+S toggle that flushes to `session/prompt` on
   turn-end). Busy-Enter is a *steer*, not a deferred prompt. Tracked: **cyril-28z2** (K1c).
2. **No subagent-scoped steering** (`/steer @name`, per-session routing). K1b steers the main session only. Tracked: **cyril-28z2**.
3. **No @-file expansion in steer text** — steers send raw text; `@`-references are expanded only on the prompt path.
4. **No multi-client transcript echo** — a `SteeringQueued` echo this client didn't originate bumps the chip but adds no
   `SteerEcho`. Single-client is the only production shape today.
5. **Chip is wire-reactive while the echo is optimistic** — a ≤1-RTT window where the transcript shows "queued" before the
   chip increments. Accepted: chip = live HUD (wire truth), echo = instant feedback (user's own text).
6. **No persistence of an idle steer past its turn in the chip** — `TurnCompleted` resets the chip even though the backend
   may still hold an un-consumed idle steer for a later turn. Accepted divergence (chip is best-effort live state, not a
   queue mirror); the `SteerEcho` in scrollback keeps its last-known status.

## Hard gate checklist

- [x] Every production-reachable input shape (1–14 above) is covered by a claim or noted out-of-scope with justification.
- [x] Every claim has a falsifier in the table.
- [x] Every falsifier names an independent oracle (pure-fn value / `TestBackend` buffer / wire tee — never the code under test asserting itself).
- [x] Every falsifier names a specific buggy implementation that would make it fail (non-vacuity column); C12's is the probe itself.
- [x] Every claim has a distinct verifiable output (per-claim named test + distinct signal).
- [x] Every measurement-based claim has a deterministic CI fence, except C12 (external backend) which is `manual` with a documented re-runnable probe + captured oracle, and its cyril-side dependency fenced by C10.
- [x] Deferrals cite verified tracker IDs (cyril-28z2 confirmed present in rivets; cyril-2vcc the fixed regression, confirmed present).
- [x] The cheapest falsifier (C12) has been run and passed.
- [x] Negative space has ≥3 entries (6 listed).
