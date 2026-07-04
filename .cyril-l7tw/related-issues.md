# cyril-l7tw — related issues (tracker prior art)

Searched `rivets list` for bridge/disconnect/silent/TurnCompleted. Relevant:

- **cyril-0gke** (closed, PR #41) — stderr drain + `stderr_tail()` ring buffer on
  `AgentProcess`. Its handoff note is explicit: threading the tail into
  user-facing `BridgeDisconnected` reasons is *this* issue's scope; the
  bridge-exit emission at `bridge.rs:~163` lives outside `run_bridge`, so the
  tail handle must be passed up.
- **cyril-84ca** (closed, P1) — created the off-loop prompt task whose `Err` arm
  (bridge.rs:566-576) is item (1)'s collapse site. The `turn_in_flight` gate is
  safe (Err still synthesizes TurnCompleted); the failure is mute, not stuck.
- **cyril-dcc6** (closed, PR #39) — C14b live evidence that the collapse is
  dangerous: un-hardened KAS smokes green-lit on `TurnCompleted` alone, which
  would also pass on an auth-failed turn. Smokes now assert a `KAS_SMOKE_OK`
  sentinel as interim mitigation.
- **cyril-9akh** (open, P3) — notification-vs-response ordering race around
  TurnCompleted. Adjacent: any new error notification emitted from the prompt
  task must go through the same internal channel (ADR-0004 single mediator) to
  avoid widening that race.
- **cyril-a71q** (open, P3) — turn-seq dedup for stale TurnCompleted; same
  single-terminal-marker invariant the fix must preserve.
- **cyril-1ixa** (open, P4) — agent-side rpc buffering under UI stall. Boundary:
  item (4) here is the *bridge→App* channel (capacity 256), not the rpc layer.
- **cyril-cx27** (open, P4) — SessionController doesn't reset context_usage on
  BridgeDisconnected; more BridgeDisconnected traffic makes this slightly more
  visible but doesn't change its scope.
