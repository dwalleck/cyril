# cyril-7z7u — prove-it-prototype findings

Date: 2026-06-21. Against real `kiro-cli 2.8.1` (logged in), default `acp` v2 engine. Run in the cyril-7z7u worktree.

## Smallest question

When a steer is queued during turn N and turn N completes **without consuming it** (no tool boundary in turn N), is it **drained on turn N+1** or **dropped** at turn-end?

## Probe

`.cyril-7z7u/probe_cross_turn_steer.py` — raw ACP over stdio against `kiro-cli acp` (v2), deliberately bypassing cyril's bridge to isolate kiro's own backend behavior. Builds nothing from the K1b feature; sends raw `_session/steer` (wire form) and records the `steering_*` `_kiro.dev/session/update` notifications + agent text, per turn. Two timings via the `TURN1` env var:
- **early** (default essay prompt): steer fired on turn-1's first chunk (runway remains).
- **late** (`TURN1="Reply with exactly one word: alpha. Do not use any tools."`): steer fired on the only chunk ≈ turn's tail.

## Oracle (independent)

`$XDG_RUNTIME_DIR/kiro-log/kiro-chat.log` (Kiro's internal `chat_cli_v2::agent::acp::acp_agent` event log) — a *different layer* than the ACP wire the probe reads (the agent's internal event, upstream of wire serialization). It logs `Received agent event: SteeringConsumed { content: "STEER-PROBE-MARKER..." }` independently. Corroborating second oracle: the model's behavior — the turn that consumed the steer appended **ZEBRA** (the steer's instruction), independent of the protocol notification.

## Agreement (non-trivial slice)

Early case: probe sees wire `steering_consumed` in turn 1 ↔ oracle logs `SteeringConsumed` ↔ ZEBRA appears. Three independent mechanisms agree the steer was real, consumed, and effective — not self-confirmation.

## Results

| Steer timing | turn-1 events | turn-2 events | ZEBRA |
|---|---|---|---|
| early (runway) | `steering_queued`, `steering_consumed` | — | yes |
| late (tail) | *(none)* | `steering_queued`, `steering_consumed` | yes |

## What I learned (not obvious before the probe)

**`steering_queued` and `steering_consumed` always arrive as a PAIR within a single turn — the turn that actively processes the steer — never split across turns.** A steer with runway pairs in the same turn; a steer sent at a turn's tail defers the *whole pair* to the next turn (turn 1 emitted zero steering events). Consumption is NOT gated on a tool boundary (the early no-tool turn still consumed it). Once consumed, the steer's instruction persists as context (ZEBRA in later turns).

Corollary I did not predict: my (and the issue's) "queued in turn N, consumed in turn N+1" model is wrong at the **wire** level — the wire never leaves a steer `steering_queued`-without-`steering_consumed` across a turn boundary.

## Design implication (for falsifiable-design — not built here)

The issue's option **(a) is confirmed**, but the mechanism is client-side, not wire-side:

- The **wire** pairs queued+consumed in one turn, so there's no wire-level "outstanding queued steer at TurnCompleted."
- But cyril adds an **optimistic** `SteerEcho{Queued}` + increments the chip at **user-send time** (K1b `add_steer_echo`), which can be mid-turn-N. When the backend **defers a late steer to turn N+1**, that optimistic echo/chip outlives `TurnCompleted(N)`.
- Therefore the current K1b reset-chip-at-`TurnCompleted` **under-counts a still-pending steer** (the steer is consumed in N+1, not N). → issue's fix **option (1): stop resetting the chip at turn-end; drive chip/echo state off the wire `steering_queued`/`steering_consumed`/`steering_cleared` events, letting it drain naturally.**
- Open sub-question for the design: when the deferred wire `steering_queued` arrives in turn N+1, does cyril's converter create a *second* echo (duplicate) alongside the optimistic one? The reconciliation (optimistic echo ↔ wire queued ↔ wire consumed) needs a falsifier. Backend behavior is now pinned; the remaining risk is cyril's client-side reconciliation.

## Substrate status

Not broken — well-defined and now mapped. Safe to proceed to falsifiable-design.
