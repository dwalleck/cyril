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

## Amendment: all handled host callbacks cross the mediator (2026-07-30)

**This restores the Decision above rather than changing it.** That Decision
already routed "permission today; KAS-5 fs/terminal, KAS-1 auth later" through
the loop. [cyril-g9vt](../../.rivets) deferred the *implementation*; what
follows records why that deferral has expired, and adds the four decisions the
original did not make.

### Context

The original mediator was implemented for permission requests, but later KAS
auth, file I/O, terminal, shell-type, and hooks callbacks executed directly in
`KiroClient`. Hook cancel/change control notifications also stayed there. The
bridge therefore mediated one request kind while the host-callback traffic that
ADR-0003 intended to audit, gate, or transform bypassed the seam.

`CONTEXT.md` names a **Host callback** as a server-to-client ACP request or
control notification through which the running agent asks Cyril, acting as the
host, to provide a decision or capability.

The three recorded reasons for the cyril-g9vt deferral, reassessed:

1. *"the loop seam has no consumer yet"* — **expired.** The consumer is
   lifecycle unification, already visible as two ad-hoc escapes: the terminal
   registry `Rc` grabbed out of `KiroClient` before the ACP connection takes
   ownership (cyril-3lh8), and hook cancellation handled inline in
   `ext_notification`. Two adapters, two bespoke routes; a third capability
   would invent a third.
2. *"routing through `run_loop` forces a `#[cfg]`'d `run_loop` parameter"* —
   **dissolved.** `InternalChannels` already threads a `#[cfg(feature = "kas")]`
   field through a single parameter, destructured with `..`.
3. *"the non-blocking invariant is already satisfied without the loop arm"* —
   **correct, and conceded.** The ACP connection spawns each inbound request as
   its own task, so direct resolution was always off-loop. Non-blocking is a
   constraint this amendment must **preserve**, not a benefit it delivers.

Likewise, shutdown-abort is not a leak being fixed: `kill_on_drop(true)` plus
`LocalSet` teardown already reaps callback children. Making that guarantee
explicit and testable is worth doing, but it carries no weight in the
cost/benefit.

### Decision

New decisions this amendment makes:

- **Typed and exhaustive at the seam.** `KiroClient` is the protocol adapter: it
  parses each known ACP payload into an exhaustive typed internal callback
  before mediation and converts the typed outcome back to the exact ACP
  response/error. Raw JSON and opaque executable jobs do not cross. Unknown
  extension traffic keeps ACP's default handling and does not become a loose
  catch-all callback.
- **The mediator is a type, and stays thin.** Not `select!` arms: an
  `accept()`-style entry point, unit-testable without the async harness, owning
  ordered acceptance, correlation, callback-task lifetime, cancellation routing,
  shutdown, and user-visible failure ordering for one bridge lifetime.
  `run_loop`'s arm delegates and holds no callback policy inline. Depth accretes
  on the Engine-selected adapter set ([ADR-0001](0001-kiro-engine-trait.md)
  amendment), so adding a capability touches the mediator zero times.
- **Ordered acceptance, concurrent resolution.** Ingress is bounded and
  lossless; producers await capacity. Callbacks are accepted in channel order,
  lifecycle state is registered *before* resolution begins — so a later control
  notification can target already-accepted work — and resolution runs
  concurrently, off the loop. `run_loop` never awaits callback resolution. The
  concurrency primitive is an implementation choice, not part of this decision.
- **Failure ordering.** When a callback failure has both a user-visible
  notification and an ACP error, enqueue the App notification first, then
  resolve the agent response. Only human decisions and existing user-visible
  failures enter `App`; bridge-internal callback execution does not become UI
  orchestration.

Constraints carried forward, unchanged in force:

- Auth, Host I/O, and Hooks adapters retain concrete execution and
  resource-specific registries.
- Cancellation stays callback-specific: session cancel reaches Host I/O for
  terminal reaping, hook cancel remains operation-scoped, a dropped ACP
  responder cancels its resolution, and bridge shutdown aborts every outstanding
  callback task.
- **[ADR-0002](0002-kas-cargo-feature-gate.md) survives**: the callback payload
  type is uninhabited in a default build (`enum HostCallback {}`), so the arm
  compiles, its match is exhaustively empty, and no KAS code links. The channel
  field threads through `InternalChannels` exactly as `terminals` already does.
  "The default build cannot read your token" stays a compile-time fact.
- The cutover is strict behavior parity, **sliced per adapter** (Auth, then
  Host I/O, then Hooks). A temporary two-path state is permitted *within* a
  slice, never across the milestone; each slice deletes the direct paths it
  replaces. Do not add an audit/policy adapter, feature flag, or compatibility
  shim until a real varying policy exists.

### Verification contract

- Every handled callback variant has an exhaustive mediation-wiring case.
- Existing KAS adapter tests continue to own concrete auth, file, terminal, and
  hooks execution behavior.
- Focused seam scenarios prove non-blocking mediation, ordered acceptance with
  concurrent resolution, bounded backpressure, callback-specific cancellation,
  responder drop, shutdown, and parity-preserving error/notification ordering.
- Advertisement and callback reachability are verified together through the
  Engine-selected adapter set.

### Consequences

- The bridge becomes the real interception seam ADR-0003 anticipated without
  moving slow work or human waits onto its command loop.
- Future audit/gate/transform behavior has one insertion point after typed
  adaptation and before resolution dispatch.
- The mediator's interface, not direct KAS implementation methods, is the
  primary lifecycle test surface.
- **Size the cutover from the variant count, not the family count.** The four
  families named here are ~19 handled non-permission variants: 7 typed
  `acp::Client` overrides, 5 ext requests, the 5 `_kiro/fs/*` ops added by
  [ADR-0009](0009-kiro-fs-dialect.md), and 2 control notifications.
- **Hazard, to confirm live rather than assume** — the same posture as the
  `select!`-interleave note above. Control notifications share the bounded,
  order-preserving ingress with the requests they cancel, so a full channel
  makes `_kiro/hooks/cancel` wait behind the hook executions it targets.
  Acceptance is cheap (register + spawn), so the window should be small.
- Candidate 02 of the same review — turn lifecycle, cyril-b4y4 — amends *this
  ADR's* turn-completion section. Landing it first keeps ADR-0004 to one
  coherent amendment instead of two on overlapping ground, and lands this
  mediator in a `run_loop` that has already shed its inline turn policy.
