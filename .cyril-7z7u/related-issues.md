# cyril-7z7u — prior art (prove-it-prototype step 0)

Searched rivets (`steer`/`k1`) + the `.k1b-steering/` artifacts.

| Issue | Relationship |
|---|---|
| cyril-bm1j (closed) | K1b UX — **where this gap was found** (xhigh code-review of the K1b diff). The chip resets at `TurnCompleted` but a still-Queued `SteerEcho` is left in scrollback. |
| cyril-f2g8 (closed) | K1a wire + state plumbing — the `SteerSession` primitive + the three `steering_*` variants. |
| cyril-c1qe (closed) | K1a wire fix — `_session/steer` double-underscore + dead inbound arm. Establishes the working wire form. |
| cyril-84ca (closed) | Bridge serialization — drove the off-loop prompt dispatch that makes mid-turn steer reach the wire only at turn-end. |

**Wire form established (from `.k1b-steering/idle-steer-wire-capture.log`, kiro 2.8.0 v2):**
- Client→agent **wire** method is `_session/steer` (single underscore on the wire; cyril sends `session/steer`, the acp lib prepends `_`). Response `{queued: true}`.
- Echoes arrive as notifications `_kiro.dev/session/update` with `update.sessionUpdate ∈ {steering_queued, steering_consumed, steering_cleared}`.

**Key correction to the issue's premise:** cyril-7z7u says the idle-steer capture "shows steers survive the idle→turn boundary, suggesting it does [drain on next turn]." **It does NOT** — `idle-steer-wire-capture.log` ends right after `steering_queued`; it never sends a subsequent `session/prompt`, so it shows the steer *queued* but never *consumed*. There is **no existing evidence** that any queued steer (idle or mid-turn) is ever drained vs dropped. The turn→turn boundary is fully unverified — probe required.
