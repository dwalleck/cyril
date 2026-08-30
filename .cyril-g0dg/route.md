# Route: cyril-g0dg

Change: Freeze the current direct ACP runtime as a test-only black-box contract oracle before SDK 2
Date: 2026-08-30

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | No external or stale premise is required. Current repository source and tests are the authority being frozen: `protocol::transport::AgentProcess` owns the subprocess stdout/stderr/lifecycle seam; `protocol::bridge::run_loop` plus its in-process `FakeAgent` exercise the direct ACP runtime; `App::handle_notification`, `App::send_prompt`, `PromptEnvelope`, and `SourceObserver` define App/memory/source ordering and identity. The landed `cyril-41bs` evidence proves only why future lexical capture must precede SDK parsing; this ticket characterizes the current pre-parse process boundary without asserting future SDK behavior. | no |
| 2 | Structural boundary | Production interfaces, schemas, and module seams remain unchanged, but the requested additions span process ingress, bridge routing, and App/memory contracts. To avoid further growth in oversized `transport.rs`, `bridge.rs`, and `app.rs`, the new oracle must be placed in dedicated child test modules with private access to their parent implementations. That is a test-module placement decision requiring an explicit structural design. | yes |
| 3 | Production-scale risk | No production path changes. Saturation, byte budgets, subprocess cleanup, and bounded shutdown are exercised only by deterministic bounded fixtures. The oracle adds no runtime allocation, latency, concurrency, or data-volume cost outside tests. | no |
| 4 | Explicit behavior | **Given** an `AgentProcess` fixture emits exact ACP source bytes and exits, stalls, writes diagnostics, or owns descendants, **when** the current stdout/process seam is driven, **then** the fixture observes the exact pre-parse bytes and retained cwd, stderr-tail, EOF, cancellation, process-group, and bounded-shutdown behavior without a production capture hook. **Given** typed main, subagent, unknown, global, error, completion, and disconnect notifications, **when** `App` consumes them in source order, **then** `SessionController` and `UiState` receive only their owned frames and expose the current event/error/completion/disconnect ordering. **Given** first/subsequent prompts, one/multiple original blocks, absent/present prepared context, UTF-8/tool budget edges, all terminal dispositions, memory unavailable/starting/ready states, and reused/changed source/session/project identities, **when** prompt preparation and source capture run, **then** original and wire prompts stay distinct, context is injected exactly once, budgets and identities are preserved, terminal status is exact, and shutdown remains bounded. **Given** every `BridgeCommand`, routing identity shape, extension parameter shape, a full bounded channel, and a disconnected peer, **when** the current direct bridge processes each case, **then** the oracle records the exact request and typed output with no missing, duplicate, reordered, or retyped event. **Given** each claim-local named mutation, **when** its focused fence runs, **then** it fails naming the exact changed byte, matrix cell, event, or route and returns green after restoration. **Given** the final diff, **when** manifests and production modules are inspected, **then** no SDK 2 dependency, runtime trait, dormant conductor path, production observer/capture interface, or production behavior change exists. | yes |

Unknown tests: none

## Selected route

Structural — the behavior is explicit and repository-grounded, but keeping the oracle out of three oversized source files requires deliberate child-test-module placement across existing module seams.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior fully explicit (T4 verdict) |
| evidence.md, probe.* | prove-it-prototype | N/A — no unverified premise (T1 verdict) |
| design.md | falsifiable-design | required |
| plan.md | budgeted-plan | required |

Oracle checkpoint in `checkpointed-build`: required — Structural route

## Downstream sequence

falsifiable-design → budgeted-plan → checkpointed-build

## Terminal criterion

Structural — every downstream artifact satisfies its owning stage's completion criterion, ending with no FAIL in checkpointed-build's recorded gate.
