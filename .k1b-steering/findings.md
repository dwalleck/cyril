# prove-it-prototype — cyril-bm1j (K1b queue-steering TUI UX)

Date: 2026-06-17. Against real `kiro-cli 2.8.0` (logged in), default `acp` v2 engine.

## Smallest question

When a `SteerSession` is enqueued ~1.5s into a live multi-tool turn, does the
steer reach kiro **mid-turn**, or only **after the turn ends**?

## Probe

`probe.rs` (ran as `cargo run --example probe_steer_midturn`). Drives cyril's
REAL `spawn_bridge`; builds nothing from the K1b feature — only the K1a
`SteerSession` primitive + the bridge. NewSession → SendPrompt (3× `sleep 2`
bash steps, `--trust-all-tools`) → at +1.5s enqueue `SteerSession`. Timestamps
every notification relative to prompt-send.

## Oracle (independent)

`wire_shim.py` — a transparent stdio tee spawned in kiro's place. Timestamps
every JSON-RPC frame on the wire, knowing nothing about cyril's notification
channel. Capture in `oracle-wire-capture.log`.

## Agreement (on a non-trivial slice)

| event | probe (cyril notifications) | oracle (wire frames) |
|---|---|---|
| `session/prompt` sent | +0ms | 0.960s |
| `SteerSession` enqueued | +1501ms | — |
| turn ends (`TurnCompleted`/`stopReason:end_turn`) | +11879ms | 12.838s |
| steer crosses the wire / surfaces | +11879ms (`SteeringUnsupported`) | **12.839s** (`__session/steer`, 1ms AFTER the prompt response) |

Both mechanisms agree: **the steer enqueued at 1.5s did not reach kiro until the
turn ended ~11.9s later.** Independent confirmation, not self-confirmation.

## What I learned (3 defects, none caught by K1a's tests — the tests bypass the real wire)

1. **Architectural — bridge serializes the steer behind the in-flight prompt.**
   `bridge.rs` awaits `conn.prompt().await` *inside* the single-consumer command
   loop, so it never `recv()`s the queued `SteerSession` until the turn ends.
   Mid-turn steering is impossible without driving the prompt off the loop
   (e.g. `spawn_local` the prompt future). This is K1b's real scope.

2. **Wire bug — outbound method is double-underscored.** cyril sends
   `ExtRequest::new("_session/steer", …)`; `agent-client-protocol` does
   `format!("_{}", method)` (lib.rs:213), so the wire shows `__session/steer`
   → kiro returns `-32601`. Correct call is `"session/steer"` (cf. `SpawnSession`
   which correctly passes `"session/spawn"`). Same for `_session/steer/clear`.
   (bridge.rs:1060, 1127)

3. **Wire bug — inbound converter arm is dead.** Steering echoes ride wire method
   `_kiro.dev/session/update`; the library STRIPS the leading `_` (lib.rs:294)
   before `ext_notification`, so cyril's converter receives `kiro.dev/session/update`.
   K1a's new arm is keyed `"_kiro.dev/session/update"` (kiro.rs:681) and never
   matches — echoes fall to the existing `"kiro.dev/session/update"` arm (kiro.rs:424)
   which does not handle steering variants → dropped. All of cyril's *working*
   ext arms are keyed without the underscore, confirming the convention K1a broke.

**Net: K1a steering is non-functional end-to-end in BOTH directions, plus blocked
architecturally.** K1a's unit tests pass only because they call `to_ext_notification`
with the raw underscore string and construct `BridgeCommand`s directly — never
crossing `ext_method`'s `format!` or `decode_notification`'s `strip_prefix`.

## Consequence (prove-it-prototype rule)

Substrate is broken → STOP. Do not run falsifiable-design until #2 and #3 are
fixed (mechanical) and #1 is folded into K1b's scope. Bugs filed: see
`related-issues.md` / rivets.
