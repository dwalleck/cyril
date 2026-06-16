# Cyril Roadmap

> *Cyril is the polished TUI for the Agent Client Protocol ecosystem.*

This document captures the phased path from cyril's current Kiro-focused implementation toward the vendor-neutral platform vision. It is the source of truth for the project's direction; every phase below should produce concrete commits, and finishing one phase unblocks the next.

## Mission

Cyril is a polished terminal interface for the Agent Client Protocol ecosystem. Run any of 37+ registered agents — Claude, Cursor, Codex, Cline, Goose, Kiro, and more — through a single interface. Beneath the TUI, composable proxy stages add behaviors no agent ships natively: skill systems, transcript audit, organizational permission policies, persistent memory across sessions, multi-client observers. Vendor neutrality is a feature, not a roadmap; stages are how cyril compounds value over time.

## Why this direction

The original mission ("cross-platform TUI client for Kiro CLI, with WSL bridging on Windows") is obsolete: Kiro CLI now ships a native Windows binary, eliminating the WSL gymnastics that originally motivated the project. At the same time, the broader [ACP ecosystem](https://github.com/agentclientprotocol) has matured into a multi-vendor standard with formal governance, language SDKs, and a curated agent registry. Cyril's strategic position shifts from "Kiro client" to "polished TUI for the ACP ecosystem with composable proxy-stage behaviors."

The competitive landscape has a real gap to fill:

| Tool | Vendor strategy | Agentic features | Composability |
|---|---|---|---|
| aider, cline, opencode | OpenAI-API-compat across providers | Reimplements its own tool-call/file-edit | None |
| Claude Code, Kiro CLI native, Cursor | Vendor-locked | Native | Vendor-controlled plugins only |
| `acpr` (registry runner) | Vendor-neutral | Native via ACP | None — just a process spawner |
| **cyril (envisioned)** | **Vendor-neutral via ACP registry** | **Native via ACP** | **Stages composable across all vendors** |

That last row is currently empty. The polished-TUI-for-the-ACP-registry niche has no incumbent.

## Phases

### Phase 0 — Re-anchor mission ✅

**Status:** complete (PR #8).

Update `README.md` and `CLAUDE.md` to use the canonical tagline and elaboration paragraph above. Update the GitHub repository description to match. Establishes the framing every subsequent decision defers to.

### Phase 1 — Transport refactor + Kiro extension boundary

**Estimate:** 1–2 weeks.
**Depends on:** Phase 0.

- Refactor `crates/cyril-core/src/protocol/transport.rs` to take a `Vec<String>` for the agent command instead of hardcoding `<binary> acp` as the second arg. The conductor passthrough spike already identified this as the gating wart for any non-Kiro agent integration.
- Move Kiro-specific extension parsing (the `_kiro.dev/*` handling currently in generic `convert.rs`) into a clearly named module. A module boundary is sufficient; a separate crate can wait for Phase 3+. The TUI side starts treating Kiro as one supported vendor, not the universe.
- Update README's Prerequisites section: drop the WSL-only line now that Kiro ships a native Windows binary.

### Phase 2 — First proxy stage (stages-first)

**Estimate:** 2–4 weeks.
**Depends on:** Phase 1.

Build `transcript-recorder` as cyril's first proxy stage in a new workspace crate (`crates/cyril-stages/`). Use the `sacp-proxy` framework. Replaces the existing `experiments/kiro-proxy-rs/` POC with a properly architected stage. Wire conductor invocation through cyril's bridge, gated on a feature flag (default = direct spawn, today's behavior).

**Why stages-first** (over registry-first): ships immediate value to today's Kiro user base, lower risk because the user-facing UX doesn't shift much, and validates the stage architecture before committing to building several more stages. The vendor-neutral story is real but doesn't require registry integration to validate empirically — Phase 4 can use a hardcoded `claude-acp` invocation if needed.

### Phase 3 — Registry-aware agent selection

**Estimate:** 2–4 weeks.
**Depends on:** Phase 1.

- Fetch `https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json`. Cache locally with sensible refresh (the registry's own auto-update cadence is hourly).
- Present an agent picker at startup or via `/agent <name>` command.
- Use `acpr` as a runtime helper (cyril spawns it instead of the agent binary directly, getting npm/PyPI/binary handling for free).
- Per-vendor auth flow handling (each registered agent has its own auth method per `AUTHENTICATION.md`); design the UX surface for redirect/login flows.

### Phase 4 — First non-Kiro agent end-to-end

**Estimate:** 1–2 weeks.
**Depends on:** Phase 3 (or Phase 2 if you prefer to validate vendor-neutrality with hardcoded invocation first).

Pick `claude-acp` as the first non-Kiro target (largest user base, well-supported, ~1,800 stars on the wrapper). Verify cyril's TUI works end-to-end. Generalize Kiro-specific UI affordances (mode picker, `_meta.welcomeMessage` rendering, `kiro.dev/*` commands) as capability-gated rather than always-on. This is the moment "vendor-neutral cyril" becomes demonstrable rather than aspirational.

### Phase 5+ — Stages catalog growth

**Estimate:** ongoing.
**Depends on:** Phase 2.

Each new stage is reusable across all supported vendors. Candidates in rough priority order:

- **Skill resolver** — supplement whatever skill system the underlying agent has (or doesn't have)
- **Context injector** — auto-attach project context, steering files, environment metadata per turn
- **Auto-approval policy** — bypass permission prompts for whitelisted tools according to org rules
- **Persistent memory** — cross-session memory synthesis the underlying agent doesn't ship natively
- **Fan-out observer** — broadcast notifications to additional ACP clients for editor integrations (e.g. nvim plugin reading what cyril sees)

Each stage is its own subprocess, written in any language that speaks `sacp-proxy`'s protocol. Stages can be authored by third parties and dropped into a user's chain config.

## Kiro feature-parity track

Parallel to the platform phases above (addresses Open Tension #6: keep shipping a strictly-better-than-status-quo Kiro experience while the platform is built underneath). Track items are independent of Phases 1–5 unless noted; they touch `convert/kiro.rs`, which is already the designated home for Kiro deviations, so they move cleanly if Phase 1 relocates that module.

### K1 — Queue steering (Kiro 2.7.0+)

**Estimate:** 1.5–2.5 weeks across three milestones.
**Depends on:** nothing (orthogonal to Phases 1–5). Requires kiro-cli ≥ 2.7.0 at runtime; degrades gracefully on older binaries.
**Wire reference:** [`docs/kiro-2.7.0-wire-audit.md`](kiro-2.7.0-wire-audit.md) — `_session/steer` / `_session/steer/clear` requests, `steering_queued` / `steering_consumed` / `steering_cleared` variants on `kiro.dev/session/update`.

Steering lets the user redirect the agent mid-turn without cancelling: the message is queued, injected at the next tool boundary, and the model arbitrates (advisory, not imperative). This also fixes a latent cyril bug: today Enter-while-busy submits a second `session/prompt` mid-turn (`app.rs` Layer-4 Enter → `submit_input()` with no busy guard), which has no defined semantics.

**Milestone K1a — Wire + state plumbing (no UX change)**

- `BridgeCommand::SteerSession { session_id, message }` and `BridgeCommand::ClearSteering { session_id }`; bridge sends `_session/steer[/clear]` as awaited ExtRequests (the commands/execute lesson applies: requests with ids, never notifications). Bridge MUST emit a notification on both success and error paths (existing invariant).
- `Notification::SteeringQueued { message }`, `SteeringConsumed { content }`, `SteeringCleared` variants; handle the three `sessionUpdate` variants in `convert/kiro.rs` — today they fall into the unknown-variant `Err` arm. Do this defensively regardless of UX timing: a future multi-client observer setup could receive steering echoes cyril didn't originate.
- Support gating: there is no capability flag and no `commands/available` entry for steering — the only gate is optimistic send. On `-32601 Method not found` (clean, no hang — verified against 2.6.1), surface one system message ("steering requires kiro-cli 2.7.0+") and remember unsupported for the session. Optionally retain `agentInfo.version` from `initialize` in `SessionController` (currently discarded) for a nicer preflight message.
- Tests: convert-layer tests for all three variants (including unknown-field tolerance), `SessionController`/`UiState` state tests, bridge error-path test.

**Milestone K1b — TUI UX**

- **Enter-while-busy steers.** When `session.status() == Busy` and the input is non-slash text, submit routes to `SteerSession` instead of `session/prompt`. Local echo in the transcript as a visually distinct entry (e.g. `↪ steer: <msg> — queued`), updated when `steering_consumed` arrives. When idle, behavior is unchanged (steer-on-idle is wire-valid but `session/prompt` is the honest default).
- Toolbar chip while ≥1 steer is queued (`⏎ steering queued`); cleared on consumed/cleared/turn end.
- `/steer <msg>` and `/steer clear` slash commands as the explicit path (also the only path for queuing a steer against an idle session, which Kiro holds for the next turn).
- Esc stays cancel — steer-clear is explicit only. No new overlay, so no key-chain or mouse-guard changes.
- UI copy must frame steering as advisory ("suggests a course correction; the agent weighs it against the current task") — live testing showed the model can and does decline.

**Milestone K1c — Polish + parity (optional, evaluate after K1b)**

- Queue-mode parity with Kiro's Ctrl+S toggle: a client-side buffer that flushes to `session/prompt` on `TurnCompleted` instead of steering mid-turn. Pure cyril-side feature; no new wire surface.
- Subagent steering: `_session/steer` against subagent session ids is unprobed — probe first, then wire `/steer @<name> <msg>` through the existing `SubagentTracker::find_by_name` lookup if it works.
- Document in `docs/kiro-acp-protocol.md` and the CLAUDE.md notification table.

**Non-goals:** replicating Kiro's TUI mode-toggle UI verbatim; steering as a vendor-neutral abstraction (it's a Kiro extension today — generalize only if/when ACP standardizes an equivalent, per Open Tension #2).

### K2 — Candidate backlog (unscheduled)

- `/goal` status UI — blocked on upstream: the iterative-loop notifications (`kiro.dev/goal/status`) never fired on the bare ACP path in 2.7.0 probes, and `subcommand:"status"` has an upstream misparse bug. The command itself already works in cyril via dynamic registration. Revisit on the next Kiro release.
- `goal` tool-call visibility: the agent's `goal {command:"complete"}` calls arrive with `kind:"other"`, which cyril filters from display as "planning" steps — a completed goal is currently invisible. Small convert/display exception once `/goal` usage is real.

## KAS engine integration track

KAS (Kiro Agent Server) is Kiro's TypeScript/LangGraph engine, embedded and self-extracting as of kiro-cli 2.7.1 and reachable today over `kiro-cli acp --agent-engine kas` (the `chat --v3` TUI path is gated by a staged "V3 not supported" check; the ACP path cyril uses is not). KAS is a *different agent on the same wire*: it speaks a `_kiro/*` extension dialect (not v2's `kiro.dev/*` / `_kiro.dev/*`), makes the host supply auth, can call ACP `fs/*` callbacks, and replaces the `agent_crew`/`list_update` subagent model with `agent-subtask` tool calls. This track makes KAS a first-class, opt-in engine in cyril.

**Why its own track (not a K-item):** the K-track is feature-parity within the v2 engine cyril already drives; KAS is a parallel engine with its own dialect and lifecycle. It also intersects the platform vision — KAS is the first Kiro engine that exposes filesystem callbacks, which is a genuine proxy-stage interception point (links to Phase 5).

**Estimate:** ~5–8 weeks across five milestones.
**Depends on:** Phase 1's transport refactor is the clean home for the `--agent-engine kas` arg (`Vec<String>` agent command); KAS-1 can land before it by appending the flag, but prefers it. Otherwise orthogonal to Phases 2–5 and the K-track. Requires kiro-cli ≥ 2.7.1 at runtime.
**Wire reference:** [`docs/kiro-2.7.1-wire-audit.md`](kiro-2.7.1-wire-audit.md) — the full audit, plus the reproducible probes in [`experiments/conductor-spike/`](../experiments/conductor-spike/) (`probe-kas-subagent`, `probe-kas-fs`, `probe-kas-orchestrate`, all 2.7.1).

### KAS-1 — Engine selection + auth responder (the entry gate)

**Depends on:** nothing hard (prefers Phase 1).

Without a host-supplied token, every KAS turn dies immediately (`[TokenInvalidError] … Host refresh callback returned no access token`). This milestone is the precondition for everything below.

- Engine selection: spawn `kiro-cli acp --agent-engine kas` (config/flag-gated; default stays v2). A new `AgentEngine` enum on the bridge/session, surfaced as a startup option or `/engine kas`.
- Implement an **`_kiro/auth/getAccessToken`** server→client request responder: reply `{ accessToken, expiresAt, profileArn }`. KAS validates `expiresAt` is > now + ~3 min and **requires `profileArn`** (backend 400s "profileArn is required" without it).
- Token sourcing: read kiro's own store (`~/.local/share/kiro-cli/data.sqlite3`, table `auth_kv`, key `kirocli:social:token` → `{access_token, expires_at, refresh_token, profile_arn}`). Re-read on each callback (KAS re-requests near expiry); refresh when stale. **This makes cyril a custodian of a kiro credential** — handle it as such (no logging, read-only, minimal lifetime). See new Open Tension #7.
- Tests: auth-responder unit test (shape + expired-token rejection), engine-select plumbing, a gated end-to-end smoke against a live KAS session.

### KAS-2 — `_kiro/*` dialect + turn lifecycle parity

**Depends on:** KAS-1.

Make a plain KAS prompt turn render correctly. KAS's surface differs from v2 on several load-bearing points:

- A parallel converter arm (in `convert/kiro.rs`, or its Phase-1 successor module) keyed on the active engine: KAS emits `_kiro/*` notifications — `_kiro/tools/didChange`, `_kiro/mcp/status`, `_kiro/progressive_context/items_changed`, `_kiro/governance/state`, `_kiro/powers/items_changed`, `_kiro/steering/documents_changed` — plus `session/update` variants `session_info_update`, `available_commands_update`, `config_option_update`.
- **Turn completion is signaled in a notification**, not the prompt response: `session_info_update` with `_meta.kiro.turnEnd.stopReason: "end_turn"`. Cyril's busy-state/turn-end logic must recognize this path (today it keys off the `session/prompt` response).
- `sess_…`-prefixed session ids; non-empty `sessionCapabilities {list, fork}`; per-run log dirs under `~/.kiro/logs/<ts>/`.
- Defensive unknown-variant tolerance, as with the steering variants in K1a.

### KAS-3 — Subagent / crew rendering for the `agent-subtask` model

**Depends on:** KAS-2.

KAS **never sends `kiro.dev/subagent/list_update`** — cyril's `SubagentTracker` + `crew_panel`, which key off `list_update`, see nothing. KAS subagents are ordinary `tool_call`s tagged `_meta.kiro.kind: "agent-subtask"`:

- Group child tool calls by **`_meta.kiro.agentSubtaskId`** (rotates from the spawn `toolCallId` to the child-execution UUID at `in_progress`); recognize the `title: "Subagent Response"` child returns (`rawInput.{response,files}`) and the parent `rawOutput.{response,subExecutionId}`.
- For the DAG orchestrator (`orchestrate_subagent`), render **`_meta.kiro.pipeline.stages[]`** — it projects the whole graph upfront (`name`, `role` = registered agent id, `dependsOn`, `status`, per-stage `agentSubtaskId`). This is the KAS analog of the v2 `agent_crew` `pendingStages` the `crew_panel` already understands; adapt that renderer rather than rebuild.
- Note the orchestrator is gated behind the `subagentOrchestration` setting, enabled by the host at **`initialize` → `clientCapabilities._meta.kiro.settings.subagentOrchestration = {enabled:true}}`** (not `session/new`); decide whether cyril asserts it by default.
- Keep the existing `_meta.kiro.agentSubtaskId` grouping behind the engine flag so v2 `list_update` rendering is untouched.

### KAS-4 — Config options + modes UX

**Depends on:** KAS-2.

Unlike v2 (where `configOptions` was always `null`), KAS populates it:

- Surface `configOptions`: `mode` (vibe / spec / quick-spec / bug-fix / plan / autonomous / semantic_reviewer), `autopilot` (on / Supervised), `contentCollection`. The existing mode picker generalizes to these; `autopilot` is a session-level permission posture cyril can expose directly instead of mediating per-tool approvals.
- Probe and wire `session/set_config_option` (the *set* direction is unverified; `config_option_update` is emitted but round-trip is untested — gate behind the probe result).

### KAS-5 — Filesystem-callback responder (first real proxy-stage hook)

**Depends on:** KAS-1; converges with Phase 2 / Phase 5 (stages).

KAS is the first Kiro engine to call ACP `fs/*` callbacks, and it is **capability-negotiated**: advertise `clientCapabilities.fs = {readTextFile, writeTextFile}` and KAS routes all file I/O through `fs/read_text_file` / `fs/write_text_file` to the host; advertise nothing and it does in-process I/O (v1/v2 behavior). Opt-in, not mandatory.

- Implement the two responders (public ACP method names, not `_kiro/fs/*`). Reads need no permission; writes already fire `session/request_permission`.
- This is the first place cyril can interpose on a Kiro agent's file operations — the natural home for audit, org write-policy, and Windows/WSL path translation as a **stage** rather than ad-hoc TUI code. Build it as/with `crates/cyril-stages/` (Phase 2) rather than inline in the bridge.

**Non-goals:** replicating KAS's spec/quick-spec workflow UIs verbatim; exposing `--v3`/the gated `chat` TUI; treating `_kiro/*` as a vendor-neutral abstraction (it's Kiro-specific — generalize only if ACP standardizes equivalents, per Open Tension #2). The `_kiro/session/{context,compact,export,history,fork,list}` methods are advertised but unprobed — out of scope until a concrete UX needs them.

## Open tensions

1. **Kiro is conspicuously absent from the ACP registry.** Cyril's most-tested agent is the one outside the curated ecosystem. Either ignore (use Kiro by direct path, others via registry) or advocate to AWS for registry membership.
2. **Some current cyril UI is Kiro-specific** (mode picker, welcomeMessage rendering, `kiro.dev/*` command parsing). Vendor-neutrality means generalizing, gating on detected capabilities, or accepting they only show for Kiro.
3. **Auth flows differ per vendor.** Each registered agent has its own auth method. Real UX work to handle showing/redirecting auth flows per agent.
4. **Stages framework is ahead of the curve.** No standardized stage registry exists yet; `sacp-conductor` and `sacp-proxy` are one project's framework. If that stack doesn't become canonical, cyril's stages story may need to migrate.
5. **Single-maintainer risk on the Rust tooling tier.** `sacp`, `sacp-tokio`, `sacp-conductor`, `sacp-proxy`, and `acpr` are all published by Niko Matsakis. The protocol itself is multi-stakeholder; the Rust tooling is one-person-led. Mitigated by clean exit ramps (all MIT/Apache), but worth knowing.
6. **Mission drift.** "Vendor-neutral platform" is more ambitious than "Kiro client." Need to keep shipping a strictly-better-than-status-quo Kiro experience while building the platform underneath.
7. **KAS makes cyril a credential custodian** (added 2026-06-16, KAS-1). Driving the KAS engine requires cyril to read kiro's bearer token from its on-disk auth store and hand it to the KAS subprocess via `_kiro/auth/getAccessToken` — a responsibility the v2 engine never imposed (it self-authenticates). This is read-only access to a token cyril doesn't own, with refresh-on-expiry, and it widens cyril's security surface. Acceptable for a local Kiro engine, but a real consideration before KAS is default-on, and it does not generalize to other vendors (each has its own auth — Open Tension #3).

## Reference / further reading

- [Agent Client Protocol](https://agentclientprotocol.com) — protocol spec
- [agentclientprotocol/registry](https://github.com/agentclientprotocol/registry) — curated agent registry (37+ agents as of 2026-05-03)
- [agentclientprotocol/acpr](https://github.com/agentclientprotocol/acpr) — registry runner CLI
- [agentclientprotocol/symposium-acp](https://github.com/agentclientprotocol/symposium-acp) — `sacp` / `sacp-conductor` / `sacp-proxy` / `sacp-tokio`
- [agentclientprotocol/claude-agent-acp](https://github.com/agentclientprotocol/claude-agent-acp) — Claude Code as an ACP server
- [`experiments/conductor-spike/`](../experiments/conductor-spike/README.md) — the 2026-05-03 spike that empirically validated conductor passthrough and produced the binary-isolation findings

## How to update this document

When a phase is completed, mark its status and add a link to the merge commit / PR that finished it. When tensions are resolved or new ones surface, update the Open Tensions section with dated notes. This document supersedes itself; older versions live in git history.
