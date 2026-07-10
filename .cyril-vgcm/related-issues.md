# cyril-vgcm — related issues (prove-it-prototype step 0)

Tracker searched: `rivets list --all | grep -i "steer\|clear"` (2026-07-09).

## Directly load-bearing

- **cyril-nvmh** (open, P2 bug) — optimistic steer chip can stick >0 when a
  turn-tail steer never drains (paths b/c/d; path a is truthful). `/steer clear`
  gives the user a *manual* drain for exactly these states — but KAS-only, so it
  does not close nvmh (v2 sessions can't clear). Do NOT scope-creep nvmh's
  bounded safety net into this issue; note the interaction in the design.
  nvmh's notes also map the drain points: chip drains ONLY via
  SteeringConsumed/Cleared/Unsupported/SessionCreated (state.rs TurnCompleted no
  longer resets).

## Substrate history (closed, context)

- **cyril-f2g8** (closed, K1a) — wire + state plumbing. Built `BridgeCommand::ClearSteering`,
  `STEER_CLEAR_EXT_METHOD`, `Notification::SteeringCleared`, and the
  `steering_unsupported` per-session set — all committed but with NO user-facing
  trigger for clear (this issue is that trigger).
- **cyril-bm1j** (closed, K1b) — TUI UX: `/steer`, Enter-while-busy, `dispatch_steer`,
  optimistic chip/echo. `/steer <text>` treats ANY non-empty arg as steer text —
  so today `/steer clear` steers the literal word "clear" at the agent
  (compat consideration for the new subcommand).
- **cyril-c1qe** (closed, P1) — the `__session/steer` double-underscore regression;
  fence `steer_methods_are_unprefixed` covers both steer consts. Any new method
  const must follow the unprefixed convention.
- **cyril-7z7u** (closed) — cross-turn steer echo probe; established that a
  backend-deferred steer keeps its chip across turn-end (why TurnCompleted no
  longer resets the chip).
- **cyril-7n1l** (closed, P3) — optimistic chip+echo leak if bridge.send fails
  after add_steer_echo; the clear path must not reintroduce the same shape
  (don't optimistically zero the chip before the send succeeds).
- **cyril-84ca** (closed, P1) — bridge loop blocked on conn.prompt(); mid-turn
  commands (steer/cancel) now work — ClearSteering rides the same fixed loop.

## Not prior art

No existing ticket covers the two candidate defects this probe phase is
examining: (1) ClearSteering's -32601 handler poisoning steer-append via the
shared `steering_unsupported` set on v2, and (2) convert/kas.rs dropping ALL
`session_info_update` steering kinds (queued/consumed/cleared) on KAS. If the
probes confirm, they are design inputs here (and filed if any part is deferred).
