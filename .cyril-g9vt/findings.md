# cyril-g9vt — prove-it findings (2026-08-03)

## Smallest question

Through the REAL ACP connection (not direct trait calls), do two concurrently
issued server→client host callbacks resolve concurrently — the ADR-0004
amendment's non-blocking premise the mediator must PRESERVE?

## Probe

`probe_g9vt_concurrent_callback_resolution` (bridge.rs test module, on the
cyril-84ca FakeAgent⇄KiroClient duplex harness): the fake agent's `prompt`
fires `kiro/hooks/executeHook {command: "sleep 1"}` and
`kiro/terminal/shell_type` concurrently via `tokio::join!`, timing each, with
**non-null response bodies required** (`exitCode` / `shellType` keys — an
`Ok(null)` is the unhandled protocol default, not an answer).

Result: `slow = 1.004s, fast = 334µs` — the fast callback resolved ~3000×
faster while the slow one was in flight. Concurrent resolution holds at the
connection layer.

## Oracle

`.cyril-g9vt/oracle-acp-rpc.txt` — an independent source census of the acp
crate (0.10.2): `rpc.rs:264-286` wraps EVERY incoming request in
`spawn(async { handler.handle_request(request).await ... })` (our wiring
passes `tokio::task::spawn_local`), so per-request tasking — and therefore
concurrent resolution — is the crate's dispatch mechanism, agreeing with the
runtime timing. Different mechanism (source reading vs wall-clock), same
conclusion.

## Probe failures that taught (both cause-3, probe-wrong — model corrected)

1. **Underscore mechanics**: first run sent `_kiro/...` from the agent side —
   both callbacks resolved in ~110µs with `Ok(null)`. The acp crate's
   `ext_method` PREPENDS the `_` escape on the wire (`lib.rs`:
   `format!("_{}", args.method)`) and the receiver strips exactly one
   (`strip_prefix('_')`), so my `_kiro/...` became `__kiro/...` on the wire and
   `_kiro/...` at dispatch — matching no arm and falling to the protocol
   default. Design consequence: the client-side dispatch sees SINGLE-stripped
   names, and a malformed double-prefixed method lands as `_kiro/*`-with-
   underscore and silently nulls — the typed-callback parse (ADR "typed and
   exhaustive at the seam") should keep unknown-method traffic on the default
   path by NAME, never by prefix heuristics.
2. **Mid-turn ordering**: second run stored no timings because the fixture
   emitted `turn_end` BEFORE the callbacks — the turn completed, the test read
   too early. Corrected by firing callbacks before turn_end (the live shape).
   Design consequence: callbacks can still be IN FLIGHT when a turn's terminal
   frames arrive; the mediator's lifecycle state must tolerate
   resolution-after-turn-end rather than assuming callbacks nest inside turns.

## Facts carried forward (already oracled elsewhere, not re-probed)

- **19 handled non-permission variants** — dn91's probe census
  (`.cyril-dn91/findings.md`, runtime probe + source census agreed), now
  dispatch-gated through `Engine::adapters()` (PR #82). AC5's coverage target.
- **Notification-before-error ordering** exists today only on the auth path
  and is CODE-ORDERED at the client (`client.rs::ext_method`: the
  `notify_if_auth_failure(...).await` completes before the result returns) —
  a static fact; it becomes fence-able once the mediator owns a typed failure
  path (injectable failure without the real store, currently blocked on
  cyril-5db7's store seam).
- **Channel bounds**: `create_channel_pair` uses bounded mpsc
  (`COMMAND_CAPACITY`/`NOTIFICATION_CAPACITY`/`PERMISSION_CAPACITY`,
  bridge.rs:134-137) and every producer `send(...).await`s — bounded-lossless
  ingress ("producers await capacity") is the standing convention the
  mediator's queue must match.

## What I learned (that I didn't know before)

The acp crate's underscore escape is SYMMETRIC — senders prepend, receivers
strip exactly one — so extension-method identity is positional, and a
double-prefix lands silently on the protocol-default null; and callbacks do
not nest inside turns: a `turn_end` can arrive while host callbacks are still
resolving, which the mediator's lifecycle model must treat as normal, not as
an edge case.
