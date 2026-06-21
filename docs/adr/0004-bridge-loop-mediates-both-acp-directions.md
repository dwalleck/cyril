# The bridge command loop mediates both ACP directions; it forwards server→client requests but never awaits their resolution

Status: accepted (2026-06-21)

## Context

KAS-0 ([cyril-atjw](../../.rivets)) introduces the Kiro-scoped `Engine` trait ([ADR-0001](0001-kiro-engine-trait.md)) and ports v2 behind it with strict behavioral parity. The trait's third responsibility — *detect turn-end* — forces a structural change in the bridge.

Today (cyril-84ca / PR #22) notifications **bypass** the bridge command loop: `KiroClient` and the off-loop `prompt_task` send straight to the App's channels, and turn-state keys off `prompt_task.is_finished()`. That couples "turn-end" to "the `prompt()` RPC resolved" — true for v2, **false for KAS**, where turn-end is a streamed `session_info_update → turn_end` notification and the prompt response is late/secondary. Under KAS the busy-guard would never clear (`is_finished()` stays false after a logical turn-end) and the next prompt is wrongly rejected.

Separately, KAS delegates file I/O, shell execution, and (for the blessed lifecycle) auth to the **host** via server→client ACP *requests* — the platform's near-term interception point ([ADR-0003](0003-defer-proxy-stack-for-host-callbacks.md), KAS-1/KAS-5). Cyril must be able to audit/gate/transform those. The maintainer chose to build that mediation seam **now**, in KAS-0, rather than have KAS-2a rewire turn-end and KAS-5 build request mediation from scratch. But a server→client request carries a *response*, and a permission response is a **human decision** that can take many seconds.

## Decision

The bridge command loop (`run_loop`) becomes the **single mediator of the inbound ACP stream in both directions**.

- **Notifications** — including the off-loop prompt task's synthesized `TurnCompleted` — flow through an internal channel the loop `select!`s on and forwards to the App. `Notification::TurnCompleted` is the **engine-agnostic universal turn-end marker**: v2 synthesizes it from the `prompt()` response, KAS's convert arm maps `session_info_update → turn_end` to it. The loop observes that marker to clear a **loop-local `turn_in_flight: Option<SessionId>`** flag, which replaces `prompt_task.is_finished()` for the busy-guard and cancel-target. The `JoinHandle` is retained **only** for `Shutdown`'s `abort()`.

- **Server→client requests** (permission today; KAS-5 fs/terminal, KAS-1 auth later) also route through the loop, so the engine's optional capability sub-traits can gate/transform them. **The loop forwards each request and never awaits its resolution.** The response continues to flow App→client via the request's embedded `responder` oneshot, **bypassing the loop**. Cyril-side resolution that is slow (KAS-5 file read / shell exec) spawns off-loop, the same way the turn prompt does (cyril-84ca).

`convert` stays in `KiroClient`: the loop forwards already-converted internal types (`Notification`, `PermissionRequest`), not raw `acp::*`. The engine is shared as `Rc<dyn Engine>` (single-threaded `LocalSet`) — used by `KiroClient` for convert and by the loop for `client_capabilities` at init.

## Considered options

- **Keep notifications/requests bypassing the loop; clear turn-state from the producer via a shared `Rc<RefCell>` flag** — rejected: smaller in KAS-0, but leaves KAS-2a to rewire turn-end and KAS-5 to build request mediation from scratch — the redo the maintainer chose to avoid.
- **Loop owns the full request round-trip (awaits the App's response and returns it)** — rejected: a permission response is a human decision; awaiting it inside the `select!` arm freezes notification and command processing for the whole dialog. The non-blocking forward rule exists precisely to prevent this.
- **Move `convert` into the loop (`KiroClient` becomes a raw pipe)** — rejected for KAS-0: it either splits convert (notifications in the loop, permission in the client) or forces the whole permission round-trip into the loop — a parity risk for no turn-end benefit. Convert stays consolidated in `KiroClient`.

## Consequences

- KAS-0 ships the seam with **zero v2 behavior change**: notifications and permission requests gain one internal hop but are forwarded unchanged. Acceptance is behavioral — every v2 test plus a live `kiro-cli acp` session streaming / tool-calling / approving / cancelling identically.
- The **non-blocking forward invariant** — *the loop forwards a request and never awaits its resolution; slow resolution spawns off-loop* — is load-bearing and governs KAS-5. Reintroducing a blocking await there is a regression, not a convenience. (Capture it at the `request` arm in code.)
- `turn_in_flight` and `prompt_task` are two turn-state fields that move in lockstep under v2 but **diverge intentionally under KAS** (the flag clears at the streamed `turn_end`; the prompt future resolves later). Hazard handed to KAS-2a: after `turn_end` clears the flag, a still-running prompt task must **not** emit a competing late `TurnCompleted` — that is KAS-2a's "treat the prompt response as secondary."
- The loop's `select!` interleaves command handling with notification/request forwarding; while a short inline command RPC (`new_session`/`set_mode`/`cancel`/`steer`) awaits, inbound items briefly buffer in the internal channel. The long await (`prompt`) is already off-loop, so the window is small — a documented parity item to confirm live, not assume.
