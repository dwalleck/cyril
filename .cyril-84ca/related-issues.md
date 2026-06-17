# prove-it-prototype — cyril-84ca — Step 0: prior art

Searched the rivets tracker (keywords: steer, prompt, bridge, cancel, loop, mid-turn).

## Related issues
- **cyril-84ca** (this, in_progress) — Bridge command loop blocks on `conn.prompt()` for the whole turn → no mid-turn steer/cancel.
- **cyril-bm1j** (in_progress) — K1b Queue-steering TUI UX. Blocked by 84ca. The K1b prove-it run is what *filed* 84ca.
- **cyril-c1qe** (closed, PR #20) — K1a steering wire defects. Fixed; out of scope here.
- **cyril-f2g8** (closed) — K1a wire+state plumbing.
- **cyril-28z2** (open) — K1c polish; blocked by bm1j.

## Existing prove-it artifacts (substantial prior art)
`.k1b-steering/` already proved the **bug** end-to-end against real kiro 2.8.0:
- `probe.rs` drives cyril's REAL `spawn_bridge`; enqueues a `SteerSession` 1.5s into a 3×`sleep 2` turn.
- `wire_shim.py` = transparent stdio tee → `/tmp/k1b_wire.log` (independent oracle).
- `oracle-wire-capture.log`: steer crossed the wire at 12.839s, **1ms after** the prompt response at 12.838s → blocked until turn end.

## What is NOT yet proven (the gap this probe must close)
The bug is proven; the **fix mechanism** is not. The proposed fix is "drive `conn.prompt()` off the
command loop (`spawn_local`) so the loop stays free to send the steer mid-turn." That assumes the ACP
`ClientSideConnection` can carry a second request while a `prompt()` is still pending. If the connection
serialized requests, `spawn_local` would not help and the design is a fantasy.

**Smallest question:** With `conn.prompt()` driven off-loop via `spawn_local`, does an `ext_method("session/steer")`
issued on the *same* (Rc-shared) connection ~1.5s later cross the wire **before** the prompt's response?

Source-level evidence (vibes, to be confirmed by probe): `agent-client-protocol` 0.10.2 `rpc.rs` sends every
request via `outgoing_tx.unbounded_send()` (non-blocking) keyed by an atomic `RequestId`, awaiting a per-id
`oneshot`, with a background pump routing responses — i.e. multiplexed by construction. The probe must turn
that reading into evidence: probe output vs the independent `wire_shim.py` frame timestamps.
