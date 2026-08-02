# cyril-dn91 — prior art (tracker sweep 2026-08-02)

Searched `rivets list -n 200` for: adapter, capability, getAccessToken, host callback,
host-io, gating, refuse, hook, auth.

## Directly load-bearing

- **cyril-g9vt** (open, blocked by this) — the ADR-0004 mediator that will *dispatch to*
  the adapter set this issue builds. Its issue text sizes the handled surface at ~19
  non-permission variants; this issue engine-gates that same surface.
- **cyril-evwh** (closed) — KAS-1: built `respond_get_access_token` as a static
  feature-gated handler — the precedent this issue unwinds.
- **cyril-7bdu** (closed) — KAS-5a: fs read/write as `#[cfg(feature="kas")]` KiroClient
  overrides; its design (claim C3, negative-space #3) recorded the deferral that became
  g9vt/dn91.
- **cyril-ufie** (closed) — KAS-5b: terminal responders; introduced the *indirect*
  engine gate (host_shell resolved None for V2 → terminals refuse) that fs/auth/hooks lack.
- **cyril-gk17** (closed) — ADR-0010 bidirectional hooks: the per-direction carve-out
  (AC4). kas_hooks="kas" advertises hooks with NO inbound adapter; empty HookRegistry
  installed at client.rs:57 must not become the "adapter" sentinel.
- **cyril-jiyn** (closed) — KAS-7: hooks host responders + `hooks_mode()` on Engine —
  the one existing engine-consulted gate (construction-time, registry only).

## Adjacent, not in scope

- **cyril-ker1** (open, P3) — KAS refusal *surface* (refusal object dropped) — about
  rendering refusals, not gating them.
- **cyril-taba** (open, P2) — auth token refresh inside the responder; orthogonal to
  who may answer.
- **cyril-5db7** (open, P3) — injectable store wiring for auth tests; a test-seam
  cleanup this issue may partially satisfy but does not own.
- **cyril-mq15** (open, P2) — workspaceTrusted gating of v2 hooks; different gate,
  different layer.
- **cyril-qr6l** (open, P3) — hooks execute command-echo verification against the
  served registry; hardening of a responder, not availability gating. NOTE: the probe
  below shows executeHook runs commands even under V2Engine today — qr6l's concern
  compounds with this issue's finding.
- **cyril-ctnv** (open, P3) — upstream ask re clientInfo keying; not cyril-side gating.

No existing ticket describes the specific defect probed here (V2-bound kas build answers
auth/fs/hooks callbacks); cyril-dn91 itself is that ticket.
