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

- **Skill resolver** — supplement whatever skill system the underlying agent has (or doesn't have). On KAS there's a **native injection hook** (no proxy rewriting needed): `session/new` `_meta.kiro.customAgents: ClientCustomAgent[]` (`CustomAgentSource.CLIENT_PROVIDED`, highest precedence) — **verified live 2026-06-16** (`probe-kas-client-agent-2.7.1.py`): a client-supplied agent loads with no rejection and runs as a first-class `orchestrate_subagent` role with its injected prompt+tools. So the skill/agent-injection half of the platform vision is wire-proven on KAS, complementing the interception half (hooks + fs/terminal host callbacks, KAS-5/KAS-7).
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
- **Tool-call content beyond text/diff is silently dropped** (v2 wire; surfaced by the 2.7.1 Rust-vs-`.d.ts` type comparison). `convert_tool_call_content` (`convert/mod.rs:131`) keeps only `Diff` and `Content(ContentBlock::Text)` and returns `None` for every other `ContentBlock` (Image / Audio / ResourceLink / EmbeddedResource) and the entire `Terminal` variant; `ToolCallContent` (`types/tool_call.rs:45`) has no variant to hold them either. A tool emitting an image, embedded resource, or terminal embed vanishes from the UI — a silent drop against the project's "no silent failure" rule. Low-frequency on Kiro today (mostly text+diff), so unscheduled, but it is the one genuinely actionable gap on the wire cyril already speaks. Fix: add a `Resource`/`Image` (and/or `Terminal`) `ToolCallContent` variant, or at minimum a text fallback + `debug!` instead of `None`. See the type-coverage section in [`docs/kiro-2.7.1-wire-audit.md`](kiro-2.7.1-wire-audit.md).

## KAS engine integration track

KAS (Kiro Agent Server) is Kiro's TypeScript/LangGraph engine, embedded and self-extracting as of kiro-cli 2.7.1 and reachable today over `kiro-cli acp --agent-engine kas` (the `chat --v3` TUI path is gated by a staged "V3 not supported" check; the ACP path cyril uses is not). KAS is a *different agent on the same wire*: it speaks a `_kiro/*` extension dialect (not v2's `kiro.dev/*` / `_kiro.dev/*`), makes the host supply auth, can call ACP `fs/*` callbacks, and replaces the `agent_crew`/`list_update` subagent model with `agent-subtask` tool calls. This track makes KAS a first-class, opt-in engine in cyril.

**Why its own track (not a K-item):** the K-track is feature-parity within the v2 engine cyril already drives; KAS is a parallel engine with its own dialect and lifecycle. It also intersects the platform vision — KAS is the first Kiro engine that exposes filesystem callbacks, which is a genuine proxy-stage interception point (links to Phase 5).

**Estimate:** ~7–10 weeks across milestones KAS-0…7 (KAS-2 split into 2a–2d during the 2026-06-16 grilling). The **walking skeleton is KAS-0 → KAS-1 → KAS-2a**.
**Depends on:** spawn-level engine selection needs **no new transport work** — `main.rs` already takes `--agent-command <Vec<String>>` → `AgentCommand::try_from_argv`, so appending `--agent-engine kas` is free today. Phase 1's refactor is still the tidy long-term home for the arg, but does not gate this track. Otherwise orthogonal to Phases 2–5 and the K-track. Requires kiro-cli ≥ 2.7.1 at runtime.
**Sequencing (decided 2026-06-16):** **K1 (queue steering) ships before this track.** K1 is higher-certainty value (real v2 users, today) and only weakly coupled — it adds three `sessionUpdate` variants to `convert/kiro.rs`, which KAS-0's `Engine`-trait port then absorbs at negligible cost. Honors Open Tension #6 and keeps the credential-custodian surface (KAS-1) off the critical path until concrete v2 value has shipped.
**Architecture (see [ADR-0001](adr/0001-kiro-engine-trait.md), [ADR-0002](adr/0002-kas-cargo-feature-gate.md)):** engine is bound at **agent-subprocess spawn** (the bridge runs one `kiro-cli acp [--agent-engine kas]` process and holds one `Box<dyn Engine>` for its life — every session on it shares the engine, so there is no per-session engine lookup); **startup-only selection in v1**, switching means restarting the subprocess. The two engines sit behind a **small Kiro-scoped `Engine` trait** (convert notification → internal `Notification`; declare `client_capabilities`; detect turn-end) **plus optional capability sub-traits** (`AuthResponder`≈KAS-1, `HostIo`≈KAS-5, `HooksHost`≈KAS-7, `GovernanceSource`≈KAS-2c) that KAS implements and v2 does not. Engine nests *under* the Kiro vendor — **not** the same mechanism as the Phase-1/4 vendor seam (Claude does not implement `Engine`). KAS code, especially the credential-reading `AuthResponder`, lives behind a default-off **`kas` cargo feature**; a default build cannot read the kiro token. **"KAS-2" in milestones below now means KAS-2a (the walking skeleton)** unless noted.
**Wire reference:** [`docs/kiro-2.7.1-wire-audit.md`](kiro-2.7.1-wire-audit.md) — the full audit, plus the reproducible probes in [`experiments/conductor-spike/`](../experiments/conductor-spike/) (`probe-kas-subagent`, `probe-kas-fs`, `probe-kas-orchestrate`, all 2.7.1).
**Prerequisite — ACP crate coverage (KAS-0 verification spike, *not* a blocker).** *Corrected 2026-06-16 (grilling).* The earlier "cyril pins **0.9**, KAS ships **^0.19**, bump before KAS-2" note was stale and conflated two version lines. Facts: cyril is **already on `agent-client-protocol` 0.10.2 / schema 0.11.2** (Cargo.lock), and **0.11.2 already carries the typed `SessionInfoUpdate` variant** that KAS's `turn_end` rides — so the walking skeleton (KAS-2a) is **not** gated on an SDK migration. The "0.19" was the *TypeScript* `@agentclientprotocol/sdk` version, a different numbering line from the Rust crate (apples to oranges). Residual risk is narrower and operational: `SessionUpdate` is `#[serde(tag = "sessionUpdate")]` with **no `#[serde(other)]` catch-all** (`#[non_exhaustive]` is a Rust API attribute, not serde tolerance), so a *future* KAS-emitted typed `session/update` variant absent from 0.11.2 would hard-fail at the acp-crate deserialization layer **before** `convert/` runs — and, unlike the raw `_kiro/*` ext path (which is JSON-tolerant), standard `session/update` offers no raw fallback hook. Mitigation chosen during grilling: a **KAS-0 verification spike** confirms 0.11.2 deserializes every typed `session/update` variant KAS emits live (`agent_message_chunk`, `tool_call`/`tool_call_update`, `available_commands_update`, `config_option_update`, `session_info_update`); the no-catch-all is a documented upgrade-trigger, not a code defense (forking the crate or intercepting beneath the acp Client trait is deferred until a live unknown variant actually bites).

### KAS-0 — `Engine` trait + v2 port + gating (invisible foundation)

**Depends on:** nothing. Ships **no** user-visible change — its acceptance criterion is strict v2 parity.

Stand up the seam everything else hangs off, without changing v2 behavior:

- Introduce the Kiro-scoped **`Engine` trait** (core surface) + the first **capability sub-trait** stubs, and **port today's `convert/kiro.rs` behind a `V2Engine` impl** with zero behavior change. This is a pure refactor of the most load-bearing working code — its oracle is **behavioral**: every existing v2 test passes *and* a live `kiro-cli acp` session streams / tool-calls / renders subagents / mode-picks identically (CLAUDE.md "verify functional wiring end-to-end after any refactor").
- Wire the **engine gate at bridge spawn**: an `AgentEngine` enum derived from the startup `agent_command` (default v2); `spawn_bridge` picks the one `Box<dyn Engine>` the bridge uses for its life. **Startup-only in v1** (`--agent-engine` / config) — `session/new` cannot change engine (it reuses the existing subprocess), so a live `/engine`-as-respawn (tear down + re-spawn the bridge) is deferred. Selecting a non-v2 engine before KAS-1 cleanly reports "not available yet".
- Establish the **`kas` cargo feature** (default off, empty for now) — the project's first feature flag; add a CI lane that builds + lints + tests `--features kas` so it cannot bitrot under "warnings are errors."
- **ACP-coverage verification spike** (see Prerequisite above): confirm schema 0.11.2 deserializes every typed `session/update` variant KAS emits live; the stale 0.9 version facts were fixed 2026-06-16 in this file and CLAUDE.md.

### KAS-1 — Engine selection + auth responder (the entry gate)

**Depends on:** KAS-0 (the engine gate + `kas` feature exist; KAS-1 fills in the `AuthResponder` and the live KAS spawn).

Without a host-supplied token, every KAS turn dies immediately (`[TokenInvalidError] … Host refresh callback returned no access token`). This milestone is the precondition for everything below.

- Make the KAS spawn real: the `AgentEngine` gate from KAS-0 now resolves `kas` to a live `kiro-cli acp --agent-engine kas` process (default stays v2). All new KAS code lands behind the `kas` cargo feature established in KAS-0.
- Implement an **`_kiro/auth/getAccessToken`** server→client request responder: reply `{ accessToken, expiresAt, profileArn }`. KAS validates `expiresAt` is > now + ~3 min and **requires `profileArn`** (backend 400s "profileArn is required" without it).
- Token sourcing — **mirror Kiro's own responder, don't just read the sqlite row.** Kiro implements this in `crates/chat-cli-v2/src/auth/{kas_token.rs, refresh_coordinator.rs, social.rs, builder_id.rs, external_idp.rs}`: it (1) resolves the active token across **three types** — social (GitHub → `kirocli:social:token`, carries `profile_arn`), AWS Builder ID, and external IdP (`kirocli:external-idp:token`); (2) **proactively OIDC-refreshes** (`create_token`, `grant_type=refresh_token`, `oidc.*.amazonaws.com`) before the ~3-min pre-expiry buffer, through a **lock-guarded `refresh_coordinator`** that serializes concurrent refreshes; (3) answers `{accessToken, expiresAt, profileArn}`. A naive "read `kirocli:social:token` and return it" responder breaks on the refresh-buffer (the failure our probes hit) and on Builder-ID/external-IdP users. Cleanest for cyril: **delegate to kiro-cli's own auth** rather than reimplement the OIDC refresh + multi-type resolution. **This makes cyril a custodian of a kiro credential** — handle it as such (no logging, read-only, minimal lifetime). See new Open Tension #7.
- Tests: auth-responder unit test (shape + expired-token rejection), engine-select plumbing, a gated end-to-end smoke against a live KAS session.

### KAS-2a — Walking skeleton: a plain KAS turn renders **and ends**

**Depends on:** KAS-1. **This is the milestone that earns a demo** — select KAS, authenticate, prompt, watch text + tool calls stream, turn completes, input is live again.

The *minimum* for a plain turn to render and end cleanly. The other `_kiro/*` surfaces (governance, mcp/status, powers, progressive_context) and the other `session_info_update` kinds all fire on a plain turn too, but cyril's existing **unknown-variant `debug!` arm drops them gracefully** — so they are deferred to 2b–2d without hanging anything.

- Converter arm (engine-keyed, in the `Engine`-trait structure from KAS-0) for **`agent_message_chunk` / `agent_thought_chunk` / `tool_call` / `tool_call_update`** only.
- **`session_info_update` → `kind: "turn_end"`** mapped to cyril's turn-completion / busy-clear. **This — not the `session/prompt` response — is the turn-completion signal under KAS** (audit §"session_info_update"). **Hang-proofing acceptance criterion:** confirm cyril's prompt-response await is **non-blocking** under KAS — drive completion off `turn_end` and treat the prompt response as secondary/whenever-it-arrives, so a non-returning response cannot freeze the skeleton. Verify with the existing `probe-kas-*` scripts (no new probe needed).
- Answer **`_kiro/terminal/shell_type`** (`{sessionId}` → `{shellType: "bash"|"zsh"|"fish"|"powershell"|"sh"}`) — the lightest host callback, called once at session setup; a missing reply yields `Shell: undefined` in the system prompt.
- Handle **`sess_…`-prefixed session ids** and per-run log dirs under `~/.kiro/logs/<ts>/`.
- Defensive unknown-variant tolerance for everything else (one `debug!` line noting `_kiro/*` surfaces are intentionally dormant pre-2b).

### KAS-2b — Metering + context-usage breakdown

**Depends on:** KAS-2a. The remaining `session_info_update` kinds — additive data whose absence loses information but hangs nothing:

- `turn_completion` → `promptTurnSummaries: [{unit:"credit", usage:<f64>, usedTools:[...]}]` + `elapsedTime` (ms) + `status` — the metering analog (maps to cyril's `TurnMetering`).
- `context_usage` → `usagePercentage` + a **per-category** `breakdown.{contextFiles,tools,kiroResponses,yourPrompts,sessionFiles}` (each `{tokens, percent, items?}`) — richer than cyril's flat `TokenCounts`; either map to `ContextUsage` + extend tokens, or model the breakdown.
- `focus_update` (turn title), `user_message_id_assigned`, `turn_start` — lifecycle/metadata, surface or ignore as needed.

See the "KAS live wire captures" section in [`docs/kiro-2.7.1-wire-audit.md`](kiro-2.7.1-wire-audit.md).

### KAS-2c — Governance gating (`GovernanceSource` sub-trait)

**Depends on:** KAS-2a.

- **`_kiro/governance/state`** `{isEnterprise, features:{mcpEnabled, webToolsEnabled, usageAnalytics, contentCollection, promptLogging, codeReferenceTracker, autonomousAgents}}` — org-policy feature flags (Cedar-derived). Gate UI/affordances on them (hide web tools when `webToolsEnabled:false`, surface `autonomousAgents`/`promptLogging` posture) rather than assuming everything is on. v2 has no equivalent; implement as the `GovernanceSource` capability sub-trait.

### KAS-2d — Agent-config migration notice

**Depends on:** KAS-2a.

- **Agent-config migration trap.** KAS's loader **silently skips** any `.kiro/agents/` profile that uses the v1/v2 fields (`allowedTools`/`toolsSettings`) and lacks a `permissions` block — it's treated as "CLI-only" and dropped (debug-logged, not loaded). So a user's existing agent library is *invisible* under KAS until migrated (add a `permissions` block; move `allowedTools`→`tools`/`permissions`). Cyril should **detect CLI-only profiles when running the KAS engine and surface them** (a one-time "N agents won't load under KAS — migrate?" notice), rather than letting them vanish silently. Format is irrelevant (`.json`/`.md`/`.yaml` all load); the field set is the gate. See the "User agent files" note in [`docs/kiro-2.7.1-wire-audit.md`](kiro-2.7.1-wire-audit.md).

### KAS-3 — Subagent / crew rendering for the `agent-subtask` model

**Depends on:** KAS-2a.

KAS **never sends `kiro.dev/subagent/list_update`** — cyril's `SubagentTracker` + `crew_panel`, which key off `list_update`, see nothing. KAS subagents are ordinary `tool_call`s tagged `_meta.kiro.kind: "agent-subtask"`:

- Group child tool calls by **`_meta.kiro.agentSubtaskId`** (rotates from the spawn `toolCallId` to the child-execution UUID at `in_progress`); recognize the `title: "Subagent Response"` child returns (`rawInput.{response,files}`) and the parent `rawOutput.{response,subExecutionId}`.
- For the DAG orchestrator (`orchestrate_subagent`), render **`_meta.kiro.pipeline.stages[]`** — it projects the whole graph upfront (`name`, `role` = registered agent id, `dependsOn`, `status`, per-stage `agentSubtaskId`). This is the KAS analog of the v2 `agent_crew` `pendingStages` the `crew_panel` already understands; adapt that renderer rather than rebuild.
- Note the orchestrator is gated behind the `subagentOrchestration` setting, enabled by the host at **`initialize` → `clientCapabilities._meta.kiro.settings.subagentOrchestration = {enabled:true}}`** (not `session/new`); decide whether cyril asserts it by default.
- Keep the existing `_meta.kiro.agentSubtaskId` grouping behind the engine flag so v2 `list_update` rendering is untouched.

### KAS-4 — Config options + modes UX

**Depends on:** KAS-2.

Unlike v2 (where `configOptions` was always `null`), KAS populates it:

- Surface `configOptions`: `mode` (vibe / spec / quick-spec / bug-fix / plan / autonomous / semantic_reviewer), `autopilot` (on / Supervised), `contentCollection`. The existing mode picker generalizes to these; `autopilot` is a session-level permission posture cyril can expose directly instead of mediating per-tool approvals.
- **Read the *initial* `configOptions` from the `session/new` response.** `session_created_from_response` (`convert/mod.rs:61`) currently reads only `modes`+`models` and ignores `config_options` (correct for v2, where it's always `null`). Under KAS it's populated, so the initial set must be lifted here; today cyril would only pick up config state from a later `config_option_update` — and on KAS the *set* path fires no echo (see below), so the initial read is the only way to learn the starting `mode`/`autopilot`. Surfaced by the 2.7.1 Rust-vs-`.d.ts` type comparison.
- Wire `session/set_config_option` — **verified working** (2026-06-16): request `{sessionId, configId, value}`, returns the rebuilt `configOptions` (the source of truth — **no `config_option_update` notification fires on set**, so read the response). Caveat: invalid values are silently coerced, not rejected (`autopilot="bogus"` → `"off"`), so cyril should constrain `value` to the advertised option ids client-side.
- **`/code` panel under KAS** — `_kiro/codeIntelligence {subcommand:'status'}` (gated by `settings.codeIntelligence`) returns `{initialized, languages[], lspServers:[{name, languages[], status, isAvailable, initDurationMs?}]}` — verified live 2026-06-16, and it **maps directly onto cyril's existing `CodePanelData`/`LspServerInfo` types**, so the `/code` panel works against KAS with a thin converter arm. `init`/`overview` subcommands do the heavier work.
- **`/usage` panel under KAS** — `_kiro/account/getUsage` (client→server request) returns the billing/usage data v2 only exposed via the `/usage` slash command. Shape (verified live 2026-06-16): `{success, message, data:{planName, billingCycleReset, overagesEnabled, isEnterprise, usageBreakdowns:[{resourceType, displayName, used, limit, percentage, currentOverages, overageRate, overageCharges, currency}], bonusCredits[]}}`. cyril can render a real usage/credits panel from this instead of parsing a slash-command response. (Carries account/billing data — keep it out of logs/telemetry.)

### KAS-5 — Host I/O callback responders: fs **and** terminal (first real proxy-stage hook)

**Depends on:** KAS-1; converges with Phase 2 / Phase 5 (stages).
**Wire reference:** the host-responsibility callback map in [`docs/kiro-2.7.1-wire-audit.md`](kiro-2.7.1-wire-audit.md), reproduced by `experiments/conductor-spike/probe-kas-callbacks-2.7.1.py`.

KAS is the first Kiro engine to delegate **both file I/O and shell execution** to the host via ACP callbacks, each **capability-negotiated**: what cyril advertises in `initialize` decides whether KAS calls back or runs in-process (v1/v2 behavior). Opt-in, not mandatory — but the two together are the platform's first real interception point over a Kiro agent's side effects.

> **Verified end-to-end 2026-06-16** (`experiments/conductor-spike/probe-kas-fs-terminal-host-2.7.1.py`): advertising `fs={readTextFile,writeTextFile}` + `terminal:true` and implementing real responders, KAS routed **every** read/write/exec of a tool-using turn through the client (bare-ACP names; agent does read-before-write + verify-after-write; terminal lifecycle `create→wait_for_exit→output→release`). A file written only via our `fs/write_text_file` responder landed on disk → the delegation is real. The platform thesis is proven; this milestone is now implementation, not investigation.

- **Filesystem** — advertise `clientCapabilities.fs = {readTextFile, writeTextFile}` → implement `fs/read_text_file` (`{sessionId, path}` → `{content}`, no permission) and `fs/write_text_file` (`{sessionId, path, content}` → `{}`, fires `session/request_permission`). Public ACP method names, not `_kiro/fs/*`. (`_kiro/fs/{delete,stat}` exist in the bundle but did not fire in the verified turn — add only if a probe shows them used.)
- **Shell/terminal** — advertise `clientCapabilities.terminal = true` → implement the lifecycle `terminal/create` (`{sessionId, command, args[], cwd}` → `{terminalId}`) → `terminal/wait_for_exit` (`{terminalId}` → `{exitStatus}`) → `terminal/output` (`{terminalId}` → `{output, exitStatus}`) → `terminal/release` (+ `terminal/kill`). With `terminal:true` advertised, **every command the agent runs flows through these on the host**; omit the capability and KAS runs shell in-process. Pairs with `_kiro/terminal/shell_type` from KAS-2.
- cyril implements **none** of these today (empty `clientCapabilities`), so this is purely additive. Owning `fs/*` + `terminal/*` lets cyril audit/gate/translate every file op and command KAS performs — the natural home for transcript audit, org write/exec policy, and Windows/WSL path translation as a **stage** (`crates/cyril-stages/`, Phase 2) rather than ad-hoc bridge code.

**Sequencing:** ship fs first (simpler, no lifecycle), then terminal. Each can land independently behind its capability flag.

### KAS-6 — Open-file context (lights up conditional steering + spec activeFile)

**Depends on:** KAS-1; pairs with KAS-2.

KAS implements IDE-grade conditional features that key off the session's **open files** — but they sit dormant if the client never supplies that context (verified: steering `inclusion: fileMatch` glob-matches `fileMatchPattern` via `minimatch` against `openFiles`, and the fileMatch lookup is skipped entirely when `openFiles` is empty; spec mode similarly keys off `activeFile`/`openFiles`). cyril is a chat TUI with no editor "open files," so against KAS today these features never trigger — you'd get only `inclusion: always` steering, i.e. the old v1/v2 behavior.

- Synthesize an `openFiles`/`activeFile` set and feed it to KAS (via the `_meta.kiro`/document channel the engine reads into its graph state) — sourced from files the user `@`-attaches or references, files the agent recently touched, and/or the cwd.
- This is the smallest change that turns on a whole class of IDE-parity behavior (conditional steering, spec activeFile logic) without cyril implementing those features itself — the engine already does, it just needs the input.
- Vendor-note: this is Kiro-specific plumbing (`_meta.kiro` open-file state), but the *concept* (telling an agent "these files are in play") is generalizable if other ACP agents grow similar context hooks.

### KAS-7 — Hooks host (org write/exec-policy interception point)

**Depends on:** KAS-1; converges with Phase 2 / Phase 5 (stages). Fully de-risked — fired end-to-end 2026-06-16.

KAS hooks are a **host-callback** model, and cyril is the host. Verified live (`experiments/conductor-spike/probe-kas-hooks-host-2.7.1.py`, contract in [`docs/kiro-kas-acp-covenant.md`](kiro-kas-acp-covenant.md) §1a + the 2.7.1 audit hooks section):

- Advertise `clientCapabilities._meta.kiro.hooks = {enabled: true}` at `initialize` (sibling of `settings`; **no `v2`**).
- Implement two responders: **`_kiro/hooks/list`** (`{trigger, sessionId, toolId?, toolTags?, workspacePaths?}` → `{hooks: AcpContextualHook[]}`) and **`_kiro/hooks/executeHook`** (`{hookId, hookName, command, sessionId, userPrompt, timeout?}` → `{output?, exitCode, cancelled}`); optionally `_kiro/hooks/sessionStart`. The agent queries `list` at `promptSubmit`/`preToolUse`/`postToolUse`/`agentStop` (preToolUse/postToolUse carry `toolId`+`toolTags`); cyril **owns the hook registry** (it returns the hooks) and **runs runCommand hooks** (askAgent hooks are agent-side prompt injection, never crossing to executeHook).
- **This is the org write/exec-policy stage's wire mechanism:** a `preToolUse` `executeHook` returning `{exitCode: 2, output: "<reason>"}` **blocks the tool** and feeds the reason to the agent. cyril can audit/gate/deny every tool call with an explanation — without a regex engine of its own, and composably as a stage.
- Handle the `_kiro/hooks/{cancel,didChange}` agent→client notifications (cancel in-flight on `session/cancel`; refresh the registry view on didChange).
- **Complementary read/evaluate API (verified live 2026-06-16):** hooks are the *enforcement* path; KAS also exposes the *policy* path as client→agent requests — `_kiro/permissions/list` (returns the resolved Cedar/TrustV2 ruleset: kiro-scope guardrails + the agent-profile allowlist), `_kiro/permissions/explain` (`{capability, resource}` → effect + matched rule + isExplicitAsk), and `_kiro/policy/check` (`{capability, paths?|command?}` → allow/deny, resolving `ask` via `session/request_permission`). cyril can *display* the active policy and *gate its own* client-executed tools against the same engine instead of duplicating policy logic — the natural data source for an org-policy stage UI. See the 2.7.1 audit "Client→agent methods".

**Responder note — cyril is the executor, but it runs its own registry, not model output.** The flow is a full circle through cyril: the agent calls `_kiro/hooks/list` → cyril returns the hooks **it owns** (loaded from the user's `.kiro/hooks/*.json`, with the `runCommand` command string) → the agent calls `_kiro/hooks/executeHook` with **that same command** → cyril spawns it. So the command cyril executes is **user/cyril-authored, never model-generated** — the agent only drives *iteration* ("a preToolUse fired, run your hooks and give me the verdict"). Concretely the `executeHook` responder must: spawn a subprocess for the `command`, set the `USER_PROMPT` env var to the request's `userPrompt`, honor `timeout` (default 60s; `0` = no timeout), capture combined stdout/stderr into `output`, return the real `exitCode`, and register an abort handle under `operationId` (fallback `${sessionId}:${hookId}`) so `_kiro/hooks/cancel` can kill it. `cancelled: true` reports a user-rejected/aborted command. This is the **same host-executor shape as the fs/terminal callbacks (KAS-5)** — so it shares their home: process spawning, Windows/WSL path translation, and the org write/exec-policy gate all belong in one `cyril-stages` host-callback layer, not scattered. `askAgent` hooks never reach this responder (agent-side prompt injection). It is opt-in: omit the `hooks` capability flag and KAS runs hooks in-process itself (the `@kiro/agent` `processRunner` fallback) — cyril takes the executor role deliberately, to own the audit/gate point.

**Non-goals:** replicating KAS's spec/quick-spec workflow UIs verbatim; exposing `--v3`/the gated `chat` TUI; treating `_kiro/*` as a vendor-neutral abstraction (it's Kiro-specific — generalize only if ACP standardizes equivalents, per Open Tension #2). The `_kiro/session/{compact,export,fork,list}` methods are advertised but unprobed — out of scope until a concrete UX needs them.

> **Spec workflow (full arc verified live 2026-06-16, but out of scope as a built UI):** the whole `_kiro/spec/resolveSession` → `invoke createSpec` (requirements) → `invoke generateDocument design` → `invoke generateDocument tasks` → `getTaskStatuses` chain works. Each `invoke` is async (returns `{sessionId}`, then streams a full turn), self-scaffolding `.kiro/specs/<feature>/{requirements,design,tasks}.md` (EARS requirements; mermaid-architecture design; checkbox tasks with `_Requirements: X.Y_` traceability) via bundled subagents (`feature-requirements-first-workflow`, `requirement-detailer`) + interactive questions; `getTaskStatuses` returns a hierarchical `{taskId, markdownStatus, isLeaf, isOptional, subTasks[]}` tree. **`executeTask` also verified** — it returns `{sessionId, executionId}`, delegates to the bundled `spec-task-execution` subagent, writes real source, and flips the task to `completed`/`succeed`. If cyril ever surfaces spec, the whole lifecycle rides the **KAS-3 agent-subtask rendering** + the `pending_interaction`/`userInput` path — no new primitive. (Only `runAllTasks`, a looped `executeTask`, is unexercised.) See the 2.7.1 audit "spec workflow" capture and `experiments/conductor-spike/spec-sample-2.7.1/`.

## Vendor-neutral client features (candidates)

Small client-side UX features that key off **standard ACP** (not a vendor extension), so they work across every registered agent. Independent of the platform phases and the vendor tracks.

### CN1 — Notify on pending approval

**Estimate:** small (~2–4 days).
**Depends on:** nothing — `session/request_permission` already drives cyril's approval overlay.

Fire a user-attention signal (terminal bell / OS notification, configurable) when a `session/request_permission` is pending and the TUI isn't focused (or after a short idle delay), and clear it on response. Lets the user walk away from a long turn and get pulled back exactly when the agent is blocked on them.

- **Why it's a cyril feature, not a hook:** verified 2026-06-16 that KAS's `HookTrigger` enum has exactly 11 values with **no `Notification`/permission/`WaitingForApproval` trigger** — you can *gate* a permission decision with a `PreToolUse` hook but cannot get a hook that fires *when the agent pauses for you*. Kiro handles that notification at the protocol/client layer instead (`session/request_permission` + the agent's own `_kiro/system/notify`), and the agent-side notification is v2-TUI-only anyway (the Rust TUI's BEL/OSC-9, which cyril never receives — see the notifications research). So cyril is the right place to own this, and doing it generically covers every agent.
- **Vendor-neutral:** `session/request_permission` is core ACP — this works for Claude, Codex, Kiro (any engine), etc., with no extension dependency.
- **Scope:** a notifier keyed off the existing `PermissionRequest` → `UiState::show_approval()` path; settings for method (bell / OS notification / none) and trigger (always / only-when-unfocused / after-Ns-idle); clears on approve/deny/cancel. No new wire surface, no overlay/key-chain changes. Could later generalize to a turn-completion notification (the other thing Kiro's v2-TUI-only BEL does).

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
