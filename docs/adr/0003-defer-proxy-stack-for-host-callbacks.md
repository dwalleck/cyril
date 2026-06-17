# Defer the sacp-proxy/conductor stack; KAS host callbacks are the near-term interception mechanism

Status: accepted (2026-06-17)

## Context

The platform vision (Mission, Phase 2, Phase 5) frames cyril's differentiating value as **composable proxy stages** built on `sacp-proxy`/`sacp-conductor` — a separate process in the JSON-RPC path that observes/rewrites the wire. But the KAS integration work surfaced a second interception mechanism: KAS delegates file I/O, shell execution, and hooks to the **host** via ACP callbacks, making cyril itself the executor and therefore the natural audit/gate/transform point — with no `sacp` dependency and structured (typed) requests instead of parsed-from-stream messages.

The side-effect concerns that originally justified proxy stages (transcript audit of file ops, org write/exec policy, Windows/WSL path translation) are exactly what KAS-5 (fs/terminal) and KAS-7 (hooks) deliver via host callbacks.

## Decision

**Host-callback support for KAS is the near-term interception path.** The `sacp-proxy`/conductor stack is deferred until KAS is fully implemented. Conductor's surviving, non-subsumed justification is narrowed to **stable workflow orchestration** (the session-level workflow engine) and **multi-client fan-out** — neither of which host callbacks address — to be revisited post-KAS.

## Considered options

- **Keep `sacp-proxy` as the primary interception mechanism now (original Phase 2)** — rejected for the near term: for KAS it duplicates, with more moving parts and a single-maintainer dependency (Open Tensions #4/#5), what host callbacks do natively.
- **Drop the proxy stack entirely** — rejected: it remains the *only* mechanism for (a) interception over agents that run side effects in-process and advertise no callbacks (e.g. v2 Kiro), (b) message-stream concerns that aren't side effects (context injection, fan-out/observer), and (c) third-party, language-agnostic composable stages. The long-term mission still wants it.

## Consequences

- Near-term platform value is delivered through the `cyril-stages` **host-callback layer** (responders, no `sacp`), not `sacp-proxy` wire stages. These are distinct shapes and should be named distinctly.
- Open Tensions #4 (framework ahead of the curve) and #5 (single-maintainer `sacp` risk) are partially discharged for side-effect interception — cyril does not depend on that stack to ship KAS audit/gate/policy.
- Phase 2 (a `sacp-proxy` transcript-recorder) is on hold; when the stack is revisited it should lead with fan-out/observer or workflow orchestration — whichever genuinely needs the wire.
- Vendor-neutral side-effect interception over in-process agents is explicitly **not** a near-term goal; if it becomes one, the proxy stack returns to the critical path.
