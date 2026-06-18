# prove-it-prototype — cyril-84ca (bridge command loop blocks mid-turn)

Date: 2026-06-17. Against real `kiro-cli 2.8.0` (logged in), default `acp` v2 engine.
Prior art: see [related-issues.md](related-issues.md). The **bug** was already proven by
`.k1b-steering/`; this probe proves the **fix mechanism** is not a fantasy.

## Smallest question

The proposed fix drives `conn.prompt()` off the single-consumer command loop (`spawn_local`)
so the loop can send a steer mid-turn. That only works if the ACP `ClientSideConnection` can
carry a second request while a `prompt()` is still pending. **Can it?** — i.e. does an
`ext_method("session/steer")` issued ~1.5s into a live prompt cross the wire and get answered
*before* the prompt's turn-end response?

## Probe

`probe/` — a standalone throwaway crate (its own `[workspace]`) using the REAL
`agent-client-protocol` 0.10.x library. It builds NOTHING from cyril's bridge: it sets up a bare
`ClientSideConnection`, `spawn_local`s `conn.prompt()` (a 3×`sleep 2` turn), then 1.5s later calls
`conn.ext_method("session/steer")` on the **same `Rc`-shared connection**. The compile itself is
part of the evidence: the borrow checker accepts two concurrent in-flight `&self` calls on one
`Rc<ClientSideConnection>`.

## Oracle (independent)

`.k1b-steering/wire_shim.py` — a transparent stdio tee spawned in kiro's place. It timestamps every
JSON-RPC frame on the wire, knowing nothing about the probe's clock or its `Client` handler. Capture:
[oracle-wire-capture.log](oracle-wire-capture.log) (key frames; full run in `/tmp/k1b_wire.log`).

## Agreement (on a non-trivial slice)

| event | probe (own `Instant`) | oracle (wire tee) |
|---|---|---|
| `session/prompt` sent | +0ms | 0.591s |
| `_session/steer` crosses wire | +1500ms | **2.092s** (+1.501s after prompt) |
| kiro `steering_queued` echo | +1502ms (`Ok`) | 2.093s (+1ms) |
| kiro `steering_consumed` | — | **4.499s (still mid-turn)** |
| prompt result `end_turn` | +5495ms | 6.086s (+5.495s) |

Probe and oracle agree to the millisecond (1502≈1501ms; 5495=5495ms). The steer round-tripped ~4s
**before** turn end — the exact inverse of the bug capture (`oracle-wire-capture.log` in
`.k1b-steering/`: steer at 12.839s, 1ms *after* the turn response).

## What I learned (one sentence)

The ACP `ClientSideConnection` multiplexes concurrent in-flight requests (a steer issued mid-prompt
is answered in ~1ms), so the fix is **purely a cyril bridge-structure problem** — driving `prompt()`
off the single-consumer command loop is sufficient and viable, with **no connection-layer change
needed**.

## Bonus findings (cheap, fell out of the probe)

1. Kiro doesn't just *queue* a mid-turn steer — it **consumes** it during the turn
   (`steering_consumed` at 4.499s, before `end_turn` at 6.086s). Mid-turn steering genuinely affects
   the running turn; K1b's UX is worth building, not just a queue-for-next-turn.
2. Wire cross-checks for the just-merged PR #20: outbound shows single-underscore `_session/steer`
   (c1qe outbound fix correct), and the echo rides `_kiro.dev/session/update` → stripped to
   `kiro.dev/session/update` (c1qe inbound fold correct).

## Caveat

Probe resolved `agent-client-protocol` 0.10.4; cyril locks 0.10.2. The verified behavior
(request multiplexing via `unbounded_send` + per-id `oneshot`; single-`_` ext prefix) is identical
across the 0.10.x line — the relevant `rpc.rs`/`lib.rs` code is unchanged.

## Hard gate (satisfied — `falsifiable-design` may proceed)

- [x] Probe written, runs against the real system (real kiro 2.8.0 + real ACP library)
- [x] Oracle defined, produces output (`wire_shim.py` → wire-frame timestamps)
- [x] Probe and oracle agree on a non-trivial slice (mid-turn steer ordering, matched to the ms)
- [x] Learned something non-obvious: the connection multiplexes; the fix is bridge-only
