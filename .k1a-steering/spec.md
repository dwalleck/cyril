# Feature: K1a — Queue-steering wire + state plumbing (no UX)

## What this is

Cyril gains the protocol and state machinery to *send* mid-turn steering messages to a Kiro 2.7.0+ backend and to *understand* the three `steering_*` echo notifications it gets back. Today cyril cannot send `_session/steer`, and the echo variants arrive on `_kiro.dev/session/update` — a method cyril's converter has no arm for, so they fall to the outer `other =>` arm and are **silently dropped (`Ok(None)` + debug log)**. (The issue's "hit the unknown-variant error arm" is incorrect — see Decision 1.) After K1a the bridge can send steer/clear as awaited requests, the converter turns the echoes into typed `Notification`s, and both state machines track "is steering supported here?" and "how many steers are queued?". **No new user-facing interaction exists yet** — this is the load-bearing foundation that K1b (cyril-bm1j) builds its UX on.

## Users

The word "user" alone is forbidden. Three named roles:

- **Cyril contributor implementing K1b** (primary consumer). Needs: `BridgeCommand::SteerSession`/`ClearSteering`, three `Notification` variants, converter arms that produce them, and queryable state (`steering supported?`, `queued count`). Will see: a compiling, tested cyril-core/cyril-ui surface they can wire Enter-while-busy and `/steer` onto without touching the wire.
- **Kiro 2.7.0+ operator** (eventual beneficiary). At K1a sees: nothing in normal use — there is no trigger for a steer in the TUI yet. The single observable at K1a is the "steering requires kiro-cli 2.7.0+" system message, and only if a steer is somehow issued against a <2.7.0 backend (reachable in K1a only via tests/dev paths).
- **Multi-client observer** (future, ROADMAP K1/Open Tension). A second ACP client sharing the session may originate a steer cyril did not. Cyril receives the `steering_*` echo for a steer it never sent. Needs: the converter to produce the notification unconditionally (never error), independent of whether cyril originated it. This is *why* converter handling is not gated on cyril having sent a steer.

## Behavior

### B1 — Bridge sends a steer as an awaited ext-request
- **Given**: a session whose steering-supported flag is not `false`, and `BridgeCommand::SteerSession { session_id, message }` is enqueued.
- **When**: the bridge processes it.
- **Then**: it sends JSON-RPC **request** (with id) `_session/steer` with params `{ "sessionId": <id>, "message": <message> }` and awaits the response. On result `{ "queued": true }` it logs the ack and emits **no** synthesized success notification — the authoritative success signal is the `steering_queued` echo handled in B4 (avoids double-emit; see Decision 5). It is sent as a request, never a notification (the `commands/execute` lesson: id-less sends are silently dropped).

### B2 — Bridge sends a clear as an awaited ext-request
- **Given**: same precondition, and `BridgeCommand::ClearSteering { session_id }`.
- **When**: the bridge processes it.
- **Then**: it sends request `_session/steer/clear` with params `{ "sessionId": <id> }`, awaits, and on `{ "cleared": true }` logs the ack; the authoritative signal is the `steering_cleared` echo (B6).

### B3 — `-32601 Method not found` marks the session unsupported and surfaces exactly one message
- **Given**: a steer/clear request is sent to a backend that does not implement the method (≤ kiro-cli 2.6.1), and the session's steering-supported flag is currently unset.
- **When**: the response is JSON-RPC error `-32601`.
- **Then**: the bridge emits **one** notification carrying the message `"steering requires kiro-cli 2.7.0+"` through cyril's **existing system-message notification channel** (no new UI surface — Decision 2), and sets the session's steering-supported flag to `false`. No hang (verified clean against 2.6.1).

### B4 — Converter: `steering_queued` → typed notification
- **Given**: an inbound notification with `method == "_kiro.dev/session/update"` (Decision 1, confirmed from captured wire) whose `params.update.sessionUpdate == "steering_queued"`, `params.update.message: string`.
- **When**: `convert/kiro.rs` `to_ext_notification` processes it via a **new outer match arm** `"_kiro.dev/session/update"` (not the existing unprefixed `kiro.dev/session/update` arm).
- **Then**: returns `Ok(Some(Notification::SteeringQueued { message }))` — never `Err`. Routed `RoutedNotification::global` by `ext_notification` (Decision 7). Unknown extra fields (incl. `sessionId`) ignored.

### B5 — Converter: `steering_consumed` → typed notification
- **Given**: `method == "_kiro.dev/session/update"`, `sessionUpdate == "steering_consumed"`, `update.content: string`.
- **When**: converted.
- **Then**: `Ok(Some(Notification::SteeringConsumed { content }))`.

### B6 — Converter: `steering_cleared` → typed notification
- **Given**: `method == "_kiro.dev/session/update"`, `sessionUpdate == "steering_cleared"` (payload carries no `message`/`content` — confirmed by probe: frame is `{"sessionUpdate":"steering_cleared"}` only).
- **When**: converted.
- **Then**: `Ok(Some(Notification::SteeringCleared))`. Always valid — no field to be missing.

### B7 — State: SessionController tracks support + queue depth
- **Given**: a `SessionController`.
- **When**: it applies `SteeringQueued` (depth += 1), `SteeringConsumed` (depth -= 1, floor 0), `SteeringCleared` (depth = 0), or the B3 unsupported-marking.
- **Then**: getters report the current queued-steer depth and whether steering is supported for the session. `apply_notification` returns `true` when a field changed. No async, no bridge access, no UI knowledge (existing SessionController contract).

### B8 — State: UiState mirrors what K1b will render
- **Given**: `UiState`.
- **When**: it applies the same three notifications via `apply_notification`.
- **Then**: it updates whatever minimal field K1b's toolbar chip / local echo will read (e.g. queued-steer presence). No rendering happens at K1a. `cyril-ui` imports no `acp::`/`protocol::` — it consumes the cyril-core `Notification` enum only (existing contract).

### B9 — Subsequent steers after unsupported-marking are short-circuited
- **Given**: a session whose steering-supported flag is already `false`.
- **When**: another `SteerSession`/`ClearSteering` is enqueued.
- **Then**: the bridge sends **zero** `_session/steer[/clear]` requests for it and emits **no** further system message (Decision 3: short-circuit, single message). Optionally debug-logged.

## Success criteria

Each has a number, a unit, and a method:

- **All three variants on `_kiro.dev/session/update` produce a notification**: 3/3 convert-layer unit tests assert `Ok(Some(SteeringQueued|Consumed|Cleared))` for the captured payloads. Measured by `cargo test -p cyril-core` (convert module). One regression test asserts the pre-existing `kiro.dev/session/update` (unprefixed) arm and the outer `other =>` drop are otherwise unchanged.
- **Exactly one system message per session on repeated -32601**: with N=2 sequential steers against a -32601 mock transport, the bridge emits exactly 1 system-message notification. Measured by the bridge error-path test counting emitted notifications.
- **Zero re-sends after unsupported-marking**: with N=2 steers and the flag flipped after the first -32601, the recording mock transport shows exactly 1 outbound `_session/steer` request. Measured by the bridge error-path test asserting send count == 1.
- **Missing payload field drops the notification**: a `steering_queued` with no `message` (and `steering_consumed` with no `content`) each yield `Ok(None)` and 1 `warn!` log, 0 panics. Measured by convert-layer unit tests (Decision 4).
- **State transitions are correct**: SessionController + UiState tests apply `queued → queued → consumed → cleared` and assert depth sequence `1 → 2 → 1 → 0`, plus an unsupported-flag flip test. Measured by `cargo test`.
- **Gates green**: `cargo test` passes; `cargo clippy -- -D warnings` is clean; new/changed files pass `cargo fmt --check`. (Pre-existing fmt failures in `bridge.rs`/`app.rs`/`main.rs` are out of scope — do not reformat them.)

## Edge cases and decisions

| Edge | Decision | Rationale |
|---|---|---|
| Echo arrives for a session cyril did not originate a steer for (multi-client observer) | Converter still produces the typed notification (never errors); delivered `RoutedNotification::global` | ROADMAP K1: a future observer setup receives steering echoes cyril didn't send; converter must not error. Global is acceptable at K1a — main session is the default target |
| Echo for a subagent / non-main session id | At K1a the notification is global (not scoped to the subagent) because `ext_notification` only promotes `ToolCallChunk` to scoped (Decision 7) | Subagent steering is K1c, explicitly out of scope; scoped promotion is deferred until then |
| `steering_queued` missing `message` / `steering_consumed` missing `content` | `Ok(None)` + `warn!`; do **not** fabricate empty string | CLAUDE.md: no sentinel defaults; missing ≠ empty |
| `steering_cleared` carries extra/unknown fields | Ignore extras, still produce `SteeringCleared` | Defensive unknown-field tolerance |
| Steer against an idle (not-busy) session | Bridge sends it; backend holds for next turn (wire-valid). No busy-gating at K1a | Busy-gating / Enter-while-busy is K1b; K1a sends whatever command it's handed |
| Two steers queued before any consume | Depth increments to 2; each `steering_consumed` decrements | State tracks depth, not a boolean, so K1b's chip count is accurate |
| `steering_consumed` when depth is already 0 (e.g. observed mid-stream) | Depth floors at 0, does not go negative | Defensive; avoids underflow on partial observation |
| -32601 on a session already marked unsupported | Short-circuited before send; no second message (B9) | Decision 3 |
| Non-`-32601` error from steer/clear (timeout, transport, other JSON-RPC error) | Bridge emits a distinct failure notification (`SteeringFailed`-style or generic error notification); does **not** set the unsupported flag | A transient failure isn't "method absent"; only -32601 means unsupported |
| Variants currently silently dropped, not errored | Today `_kiro.dev/session/update` hits the outer `other =>` arm → `Ok(None)`. The fix adds a new outer arm; it is NOT new cases under the existing `kiro.dev/session/update` arm | Captured wire (log lines 25/37/120) + converter line 674. Corrects the issue's "hit the unknown-variant error arm" premise |
| A future binary moves steering to `_kiro/steering/session_update` (tui.js already uses that string) | Out of scope for K1a — spec to the proven `_kiro.dev/session/update` wire. Optionally tolerate the alternate string later if a capture shows the backend emitting it | The binary/backend wire (captured 2.7.0) still uses `_kiro.dev/*`; tui.js migration is a client-display string, not the served method |

## Out of scope

This change does NOT include:

- Enter-while-busy routing to a steer (K1b).
- `/steer <msg>` and `/steer clear` slash commands (K1b).
- Toolbar "⏎ steering queued" chip (K1b).
- Local-echo transcript entry for a queued steer and its update-on-consumed (K1b).
- Queue-mode client-side buffering / Ctrl+S parity (K1c).
- Subagent steering (`/steer @<name>`) — unprobed (K1c).
- Any KAS-engine (`--agent-engine kas`) steering path (KAS track).
- Generalizing steering as a vendor-neutral abstraction (Open Tension #2 — it stays a Kiro extension).
- Clearing the queue on `TurnCompleted` as a UX behavior (that drives K1b's chip; K1a only plumbs the state).

## Constraints

| Dimension | Limit | How measured |
|---|---|---|
| New UX surface | Zero new keybind, overlay, mouse-scroll-guard entry, or input mode | Code review against the key-handling-layers chain in CLAUDE.md |
| Crate boundaries | `cyril-ui` imports no `acp::`/`protocol::`; only `agent-client-protocol` importer remains `cyril-core` | `cargo build` + grep |
| Bridge invariant | Every steer/clear command emits a notification on its error path (success path covered by the echo) | bridge error-path test |
| Lints | No `unwrap`, no `let _ =`, no `#[allow]`, no `unsafe`, no sentinel defaults | `cargo clippy -- -D warnings` |
| Backward compat | A steer to ≤2.6.1 errors cleanly (-32601), is remembered, no hang | manual probe / mock transport test |
| Toolchain | Rust 2024, pinned 1.94.0 | `rust-toolchain.toml` |

## Decisions log

| # | Question | Decision | Why |
|---|---|---|---|
| 1 | Which JSON-RPC method do the three `steering_*` variants arrive on — `kiro.dev/session/update`, `_kiro.dev/session/update`, or `_kiro/steering/session_update`? | **PINNED from captured wire: `_kiro.dev/session/update`.** Source: `experiments/conductor-spike/logs/probe-steer-goal-2.7.0.log` lines 25 (`steering_queued {message}`), 37 (`steering_consumed {content}`), 120 (`steering_cleared {}`). Request acks: `{queued:true}` / `{cleared:true}`. **This corrects two project errors:** (a) the issue/ROADMAP say `kiro.dev/session/update` (unprefixed) — wrong; (b) the issue says the variants "hit the unknown-variant error arm" — wrong, they hit the outer `other =>` arm and are silently dropped (`Ok(None)`). The wire-audit's "returns a Protocol Err" (line 29) is also wrong for the same reason. **Action item: correct rivets cyril-f2g8, ROADMAP K1a, and the wire audit.** | Requester asked to prove the wire; the captured log already does. No live re-probe needed for the wire shape, though prove-it-prototype may confirm against the user's current binary. |
| 2 | K1a is "no UX change" yet must "surface one system message" on -32601 — how does it surface? | Emit through cyril's **existing system-message notification channel**; "no UX change" means no new keybind/overlay/input-mode. Treated as foundational plumbing for K1b. | Requester: "it's a foundational step to prepare for the next issues." System messages are an existing channel, so reusing it adds no new UI surface. |
| 3 | After the first -32601, what happens to later steers in that session? | **Short-circuit, single message**: no further wire send, message fires only on the first -32601. | Requester selection; matches AC "surfaces one system message" and "remembered per session." |
| 4 | Missing payload field / unknown extra fields on a `steering_*` notification? | **Missing required field → drop + `warn!` (`Ok(None)`); unknown extras ignored.** | Requester selection; CLAUDE.md forbids sentinel empty-string defaults. |
| 5 | On a *successful* steer, does the bridge synthesize a notification or rely on the backend echo? | **Rely on the echo.** The converter-produced `SteeringQueued`/`Consumed`/`Cleared` is the single source of truth (it must exist anyway for the observer case in Decision-table row 1). The bridge does not synthesize a duplicate on `{queued:true}`/`{cleared:true}`. | Avoids double-counting when cyril originates the steer; the echo is verified reliable (wire audit). The literal "notify on success" from the issue is satisfied by the echo, not a second bridge emit — flagged here so the implementer doesn't double-emit. |
| 6 | Where does queued-steer state live? | Both layers, per existing pattern: SessionController holds the session fact (depth + supported flag); UiState mirrors the minimal field K1b renders. | AC names "SessionController/UiState state tests"; CLAUDE.md assigns session facts to SessionController, render-facing state to UiState. |
| 7 | How are steering notifications routed to the App (scoped vs global)? | **Global at K1a.** `ext_notification` (`client.rs:134-139`) promotes only `ToolCallChunk` to `RoutedNotification::scoped`; all other variants go `global`. Steering variants therefore carry **no `session_id` field** (matching ROADMAP's `SteeringQueued { message }`). Scoped promotion is deferred to K1c (subagent steering). | Discovered by reading the dispatch during prove-it-prototype — corrected the spec's original "routed by session_id" claim. Global is correct for the main session, the only K1a/K1b target. |

## Oracle (prove-it-prototype)

**Probe** (`.k1a-steering/probe.rs`, run as a throwaway `#[test]` in `convert/kiro.rs`, since `to_ext_notification` is `pub(crate)`): fed the three captured `_kiro.dev/session/update` steering frames through the **real** `to_ext_notification` and asserted the current return value.

Probe output (all three frames):
```
[probe] queued:   to_ext_notification(_kiro.dev/session/update) = Ok(None)   variant=steering_queued   payload=Some("STEERING UPDATE: stop now.")  sessionId=Some(...)
[probe] consumed: to_ext_notification(_kiro.dev/session/update) = Ok(None)   variant=steering_consumed payload=Some("STEERING UPDATE: stop now.")  sessionId=Some(...)
[probe] cleared:  to_ext_notification(_kiro.dev/session/update) = Ok(None)   variant=steering_cleared  payload=None                              sessionId=Some(...)
```

**Oracle 1 (independent — static grep, not runtime):** `grep -c '"_kiro.dev/session/update" =>'` → **0** match-arms (vs **1** for the unprefixed `kiro.dev/session/update`). Zero arms ⇒ the method falls to the outer `other =>` arm (`kiro.rs:674`) ⇒ `Ok(None)`. **Agrees with the probe.**

**Oracle 2 (independent — call-graph):** `to_ext_notification` is called from exactly one place: `ext_notification` (`client.rs:129`), the raw-JSON ext path. The unprefixed `kiro.dev/session/update` arm already works in production (`tool_call_chunk`), proving non-literal `*/session/update` methods reach `ext_notification` rather than the typed `session_notification` handler. `_kiro.dev/session/update` is likewise non-literal ⇒ reaches `to_ext_notification`. **Confirms reachability** (the genuine unknown).

The captured wire log (`experiments/conductor-spike/logs/probe-steer-goal-2.7.0.log`, lines 25/37/120) is the input ground truth — frames copied verbatim, not hand-built.

## What I learned (that wasn't obvious before the probe)

1. **The variants are silently DROPPED (`Ok(None)`), not errored** — falsifying the issue/wire-audit's "returns a Protocol Err" claim against the running code. The fix is a new *outer* arm, and the K1b behavior of a stray pre-fix steer echo is "nothing happens," not "an error toast."
2. **Only `ToolCallChunk` is routed session-scoped; everything else is global.** This corrected the spec (Decision 7) — steering variants need no `session_id` field, and subagent-scoped steering can't work until the dispatch promotion is extended (K1c).

## Sign-off

The requester typed, verbatim:

> "This feature adds the ability to send steering messages to agents, the converter-produced echo is the source of truth. The wire methods still need to be proven. Also, if you look in @experiments/ , you might find some of the work is done for you"

This restatement matches the artifact: steering-send capability (B1/B2), converter echo as the single source of truth (Decision 5, B4–B6), wire methods to be proven (Decision 1 — subsequently pinned from the captured log the requester pointed to). K1b UX confirmed out of scope by omission.

Date: 2026-06-17
