# cyril-g9vt — prior art (tracker sweep 2026-08-03)

Most context is same-session (dn91 shipped hours ago). Sweep: mediator,
callback, backpressure, ordered, cancel, shutdown.

## Directly load-bearing

- **cyril-dn91** (closed, PR #82) — the Engine adapter set this mediator
  dispatches to; its probe census (19 handled non-permission variants, in
  `.cyril-dn91/findings.md`) is carried forward as the coverage target for AC5.
- **cyril-b4y4** (closed, PR #81) — TurnMediator extraction: the structural
  precedent for "mediator is a TYPE with a unit-testable entry point, run_loop
  arm delegates" (protocol/turn_mediator.rs). This feature is its host-callback
  sibling, per the ADR-0004 amendment's sequencing note.
- **cyril-3lh8** (closed) — the terminal-registry `Rc` grabbed out of
  KiroClient before the connection takes ownership (bridge.rs:586-ish): one of
  the two ad-hoc escapes the amendment says the mediator replaces.
- **cyril-jiyn/gk17** (closed) — hooks cancel handled inline in
  `ext_notification`: the other ad-hoc escape.
- **cyril-84ca** (closed) — the in-process FakeAgent⇄KiroClient duplex harness
  (bridge.rs tests) this probe reuses; it will also carry the seam scenarios.

## Adjacent, open, NOT this issue's scope

- **cyril-ker1** (P3) — KAS refusal-object rendering (UI surface, not
  mediation).
- **cyril-5db7** (P3) — injectable auth store; the mediator's typed-callback
  seam makes the C13-ordering fence testable without it, but store injection
  itself stays 5db7.
- **cyril-lvok** (P3, filed this session) — ProcessTree kill-on-wait design
  question; terminal lifecycle semantics, not mediation routing.
- **cyril-n809** (P4, filed this session) — setsid drain residual; ditto.
- **cyril-taba** (P2) — auth token refresh; responder internals, not routing.
- **cyril-pnwb** (P3) — turn-end stop_reason fidelity on cancel; TurnMediator
  territory, not host callbacks.

No existing ticket describes the mediator implementation itself besides
cyril-g9vt.
