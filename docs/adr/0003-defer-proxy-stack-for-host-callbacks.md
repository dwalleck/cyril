# Defer the sacp-proxy/conductor stack; KAS host callbacks are the near-term interception mechanism

Status: accepted (2026-06-17); extended by
[ADR-0009](0009-kiro-fs-dialect.md) and
[ADR-0010](0010-kas-hook-registry-direction.md) (2026-08-01)

**Scope note added 2026-08-01.** The Context below lists "org write/exec policy"
among the concerns host callbacks deliver. As implemented, the fs responders
deliver the **audit** half only — every mutation is logged, nothing is refused;
the central write/exec gate seam is still deferred to its first consumer
(cyril-g9vt). Read that claim as "host callbacks are where the gate will go",
not "the gate exists". ADR-0009 records which callback *dialect* cyril answers
(the choice this ADR left open), and ADR-0010 records that hooks are
bidirectional — under `kas_hooks = "kas"` the agent, not cyril, executes them,
which inverts this ADR's assumption for that one family.

**Revisit fired and resolved 2026-08-08 — the deferral holds, and is stronger.**
This ADR named **stable workflow orchestration** as one of two surviving,
non-subsumed justifications for the proxy stack, to be revisited post-KAS.
kiro-cli 2.16.0 shipped a workflow engine in KAS (`_kiro/workflow/*`: a real DAG
scheduler with `step`/`sequence`/`repeat`/`parallel`/`watch` nodes, persisted
runs, resume). Probed live 2026-08-08
(`experiments/conductor-spike/probe-kas-workflow-gateoff-2.16.0.py`): cyril can
create **and execute** a client-authored DAG, and receive its full lifecycle
event stream, over its **existing single stdio ACP link** — no wire
interposition, no `sacp` dependency, no conductor. So the justification is not
triggered; it is largely **discharged by the vendor** for the single-vendor case.
What survives of it is narrower: **cross-vendor** orchestration (stage 1 on Kiro,
stage 2 on Claude), where `_message/send` is unavailable and cyril must relay —
and that is blocked on Phases 3/4 regardless. See [ADR-0011](0011-ungated-client-driven-workflow-control-plane.md)
for how cyril drives that engine, and the ROADMAP **W track**.

**Persistent-memory exception accepted 2026-08-22 (`cyril-ct0y`).** Automatic
memory recall is a message-stream concern, but waiting for the deferred proxy
stack would make every other part of the memory runtime depend on Phase 2.
Cyril may therefore inject the bounded first-prompt memory block at the
existing bridge prompt seam as an explicit interim adapter. Durable capture is
a separate bridge observer: it records original outbound prompts and normalized
inbound turn/tool lifecycle before UI projection. Neither bridge adapter owns
storage, ranking, consolidation, or UI policy; those remain behind the shared
memory interface. When the proxy stack is activated, automatic capture and
injection move to a proxy-stage adapter and both interim bridge adapters are
removed. The memory runtime, capability-bound MCP tools, stores, and TUI control
plane remain unchanged.

## Context

The platform vision (Mission, Phase 2, Phase 5) frames cyril's differentiating value as **composable proxy stages** built on `sacp-proxy`/`sacp-conductor` — a separate process in the JSON-RPC path that observes/rewrites the wire. But the KAS integration work surfaced a second interception mechanism: KAS delegates file I/O, shell execution, and hooks to the **host** via ACP callbacks, making cyril itself the executor and therefore the natural audit/gate/transform point — with no `sacp` dependency and structured (typed) requests instead of parsed-from-stream messages.

The side-effect concerns that originally justified proxy stages (transcript audit of file ops, org write/exec policy, Windows/WSL path translation) are exactly what KAS-5 (fs/terminal) and KAS-7 (hooks) deliver via host callbacks.

## Decision

**Host-callback support for KAS is the near-term interception path.** The `sacp-proxy`/conductor stack is deferred until KAS is fully implemented. Conductor's surviving, non-subsumed justification is narrowed to **stable workflow orchestration** (the session-level workflow engine) and **multi-client fan-out** — neither of which host callbacks address — to be revisited post-KAS.

## Considered options

- **Keep `sacp-proxy` as the primary interception mechanism now (original Phase 2)** — rejected for the near term: for KAS it duplicates, with more moving parts and a single-maintainer dependency (Open Tensions #4/#5), what host callbacks do natively.
- **Drop the proxy stack entirely** — rejected: it remains the only general mechanism for (a) interception over agents that run side effects in-process and advertise no callbacks (e.g. v2 Kiro), (b) message-stream concerns that must operate outside Cyril's own bridge (context injection, fan-out/observer), and (c) third-party, language-agnostic composable stages. The bounded persistent-memory exception above is deliberately temporary and bridge-local; it does not replace that general mechanism. The long-term mission still wants it.

## Consequences

- Near-term platform value is delivered through the `cyril-stages` **host-callback layer** (responders, no `sacp`), not `sacp-proxy` wire stages. These are distinct shapes and should be named distinctly.
- Open Tensions #4 (framework ahead of the curve) and #5 (single-maintainer `sacp` risk) are partially discharged for side-effect interception — cyril does not depend on that stack to ship KAS audit/gate/policy.
- Phase 2 (a `sacp-proxy` transcript-recorder) is on hold; when the stack is revisited it should lead with fan-out/observer or workflow orchestration — whichever genuinely needs the wire.
- Until that revisit, `cyril-ct0y` may inject the first-prompt memory block at
  the bridge prompt seam and capture original outbound plus normalized inbound
  lifecycle through a separate pre-UI bridge observer. Its clean cutover removes
  both bridge adapters only after the proxy-stage adapter passes the same
  adapter-neutral suite: identical original-prompt preservation, exactly-once
  first-prompt injection bytes/budgets, session/project identity, normalized
  assistant/tool capture and terminal dispositions, fail-open behavior, and no
  double capture while both implementations are present for verification.
- Vendor-neutral side-effect interception over in-process agents is explicitly **not** a near-term goal; if it becomes one, the proxy stack returns to the critical path.
