# Cyril Roadmap

> *Cyril is the polished TUI for the Agent Client Protocol ecosystem.*

This document captures the phased path from cyril's current Kiro-focused implementation toward the vendor-neutral platform vision. It is the source of truth for the project's direction; every phase below should produce concrete commits, and finishing one phase unblocks the next.

## Mission

Cyril is a polished terminal interface for the Agent Client Protocol ecosystem. Run any of 37+ registered agents — Claude, Cursor, Codex, Cline, Goose, Kiro, and more — through a single interface. Beneath the TUI, composable proxy stages add behaviors no agent ships natively: skill systems, transcript audit, organizational permission policies, persistent memory across sessions, multi-client observers. Vendor neutrality is a feature, not a roadmap; stages are how cyril compounds value over time.

## Why this direction

The original mission ("cross-platform TUI client for Kiro CLI, with WSL bridging on Windows") is obsolete: Kiro CLI now ships a native Windows binary, eliminating the WSL gymnastics that originally motivated the project. At the same time, the broader [ACP ecosystem](https://github.com/agentclientprotocol) has matured into a multi-vendor standard with formal governance, language SDKs, and a curated agent registry. Cyril's strategic position shifts from "Kiro client" to "polished TUI for the ACP ecosystem with composable proxy-stage behaviors."

The competitive landscape has a real gap to fill:

| Tool | Vendor strategy | Agentic features | Composability |
| --- | --- | --- | --- |
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

**Status note (2026-06-17):** **on hold, deprioritized behind the KAS host-callback track** (Open Tension #8, ADR-0003). KAS-5/KAS-7 deliver the side-effect interception this phase was meant to validate — via host callbacks, not a wire proxy — and single-client transcript recording is achievable cyril-internally without a proxy at all. The `sacp-proxy`/conductor stack's first uniquely-justified jobs are **fan-out/observer** and **stable workflow orchestration**, both deferred until KAS is implemented. Phase 2 as written (a `sacp-proxy` transcript-recorder) is not the next proxy work; when the stack is revisited it should lead with whichever of those two actually needs the wire.

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

The presentation side of that generalization starts with the semantic-theme track: `cyril-ixua` (complete) defines Cyril Dark's 19-role contract and explicit true-color, ANSI-256, ANSI-16, and no-color projections. Widget migrations (`cyril-ghuu`, `cyril-nrnq`, `cyril-dij8`), additional palettes (`cyril-fkke`), and configuration/selection (`cyril-qaq0`) activate the seam without coupling visual policy to an agent vendor.

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
**Wire reference:** [`docs/kiro-2.7.0-wire-audit.md`](kiro-2.7.0-wire-audit.md) — `_session/steer` / `_session/steer/clear` requests, `steering_queued` / `steering_consumed` / `steering_cleared` variants on **`_kiro.dev/session/update`** (underscore-dot prefix; confirmed by captured wire `experiments/conductor-spike/logs/probe-steer-goal-2.7.0.log`, NOT the unprefixed `kiro.dev/session/update`). **Correction (cyril-c1qe, 2026-06-17, probed vs kiro 2.8.0):** that underscore-dot prefix is the **wire** form only. The `agent-client-protocol` library adds one leading `_` on send (`format!("_{}")`) and strips one on receive (`strip_prefix('_')`), so **code uses the unprefixed form** — outbound `ExtRequest::new("session/steer")` (→ wire `_session/steer`); inbound the converter receives `kiro.dev/session/update` and handles steering in that existing arm.

Steering lets the user redirect the agent mid-turn without cancelling: the message is queued, injected at the next tool boundary, and the model arbitrates (advisory, not imperative). This also fixes a latent cyril bug: today Enter-while-busy submits a second `session/prompt` mid-turn (`app.rs` Layer-4 Enter → `submit_input()` with no busy guard), which has no defined semantics.

**Milestone K1a — Wire + state plumbing (no UX change)**

- `BridgeCommand::SteerSession { session_id, message }` and `BridgeCommand::ClearSteering { session_id }`; bridge sends `_session/steer[/clear]` as awaited ExtRequests (the commands/execute lesson applies: requests with ids, never notifications). Bridge MUST emit a notification on both success and error paths (existing invariant).
- `Notification::SteeringQueued { message }`, `SteeringConsumed { content }`, `SteeringCleared` variants; handle the three `sessionUpdate` variants in `convert/kiro.rs`. **Correction (cyril-c1qe):** these are NOT a new `_kiro.dev/session/update` arm — the ACP lib strips the leading `_`, so steering echoes arrive as `kiro.dev/session/update` and are handled in that **existing** arm (alongside `tool_call_chunk`), NOT a prefixed arm and NOT the `other =>` catch-all. K1a's prefixed arm was dead code (probed vs kiro 2.8.0); fixed by folding steering into the `kiro.dev/session/update` arm. (The original "falls to `other =>`, silently dropped" claim was wrong — post-strip they hit the existing arm.) Do this defensively regardless of UX timing: a future multi-client observer setup could receive steering echoes cyril didn't originate.
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

**Framing — KAS turns cyril from a *client* into a *host*.** v2 is receive-and-render: cyril parses ~15 `kiro.dev/*` notifications and implements **zero** host callbacks (empty `clientCapabilities`). KAS expands the surface ~3–4× in notifications *and* adds a categorically new dimension — **server→client requests cyril must answer** (`auth/getAccessToken`, `fs/*`, `terminal/*`, `hooks/*`, `shell_type`). So the back half of this track (KAS-5 fs/terminal, KAS-7 hooks) is **host-responsibility work that converges with the stages layer (`crates/cyril-stages/`, Phase 2/5), not dialect parsing** — size and test it as such. In particular it makes `platform/path.rs` load-bearing and demands **cross-platform Windows/Linux testing** (path translation `C:\`↔`/mnt/c`, native-vs-WSL command execution) that the receive-only v2 path never required.

> **Update 2026-07-02 — 2.11.0 audit + first live-session evidence base.** The 2.11.0 audit + reading two real captured sessions (KAS and v2, `KIRO_ACP_RECORD_PATH`) sharpen this track. See the committed **reference corpora**: `experiments/conductor-spike/{kas,v2}-live-session-trace-2.11.0.jsonl` + the shared `kas-live-session-trace-2.11.0.md` (full agent→client inventory, v2/KAS side-by-side, concrete tool-lifecycle field diff). Wire status: **`@kiro/agent` jumped 0.3.299→0.8.0 in 2.11.0 but is wire-stable** (a version renumber; covenant package still ships) — the only additive surface is 4 methods: `_kiro/knowledge/indexing{Started,Completed}`, `_kiro/sandbox/applyConfig`, `_session/steer/clear`. Milestone-mapped findings (all live-probed):
>
> - **Permission is ENGINE-CONDITIONAL — sharpens KAS-2a's tool-approval path (rivets cyril-qo13).** (a) KAS `user_input` clarifying questions carry N options **all `kind: allow_once`**; cyril's kind-keyed `PermissionResponse` + `find_option_id(kind)` can only return the *first* option → it silently answers every spec/quick-spec question with choice 0. (b) The trust models have **zero field overlap**: v2 `_meta.trustOptions[].{patterns,setting_key}` (command-pattern, persisted to a setting) vs KAS `_meta.kiro.consent{capability,scope}` (Cedar grant). So `PermissionResponse` must become engine-conditional: carry the selected `optionId` **and** a KAS `consent{capability,scope}` passthrough, while keeping the v2 `trustOption` path.
> - **KAS-2b — three dropped `session_info_update` kinds worth consuming (rivets cyril-0o7e):** `turn_completion {elapsedTime, status, promptTurnSummaries:[{usage, usedTools[]}]}` (per-turn cost + duration + tool list — richer than the flat metering; `usedTools[]` is KAS-only, cost+duration also on v2 `metadata`), and `focus_update {focus.title}` (status line). Per-file `context_usage.items[]` carries **`progressivelyLoaded`** (rivets cyril-1116).
> - **KAS-2c — `_kiro/governance/state` live-confirmed** `{isEnterprise, features:{mcpEnabled, webToolsEnabled, usageAnalytics, contentCollection, promptLogging, codeReferenceTracker, autonomousAgents}}` — the `GovernanceSource` feed exists exactly as scoped.
> - **KAS-4 — model options carry per-model `_meta.kiro.{rateMultiplier, hasEffort, effortSchemaPath}}` cyril drops (rivets cyril-lxuo):** surface an effort badge + credit-rate hint; gate `/effort` on `hasEffort`. **KAS-only** — v2 encodes the rate as a `"N.NNx credits"` group string and has no `hasEffort`.
> - **KAS-8 — `_session/steer/clear`** (KAS-only steering-clear companion, rivets cyril-vgcm, K1 track); **`_kiro/knowledge/indexing*`** (rivets cyril-45ld, optional progress display — see the crew-panel-style sketch `experiments/knowledge-panel-render/`); **`kiro-snapshot-v2://<sess>:<hash>` checkpoint URIs** on completed edits → the `/rewind` substrate (diff is also inline as `content[].{type:"diff",newText,oldText}`, so basic rendering needs no URI resolution).
> - **KAS-0/2a — turn-end detection design is well-founded (rivets cyril-9akh evidence):** across both completed turns in the KAS trace, the only `session/update` trailing `turn_end` was a benign `context_usage` (~10ms later); **no `agent_message_chunk`/`tool_call` after `turn_end`**. Also confirms the busy-guard note below (turn-end ≠ prompt-response on KAS).
> - **KAS-1 — auth contract confirmed live:** the callback reply is `{accessToken, expiresAt, profileArn, provider}` with `profileArn` **required** (present in the reference client); the `--auth=machine` / `KIRO_API_KEY` file-token modes bypass the callback entirely (a simpler entry gate). **Note:** a live KAS session ran with `contentCollection: false` but `provider: "Enterprise"` — prior audits assumed a non-enterprise token.
> - **KAS-5 — host callbacks only fire when advertised:** the live KAS session did all file I/O via `tool_call` + `session/request_permission` (never `fs/*`) because the client advertised no fs capability — reinforcing that KAS-5 is genuinely opt-in (omit the capability → the callbacks never fire).

**Why its own track (not a K-item):** the K-track is feature-parity within the v2 engine cyril already drives; KAS is a parallel engine with its own dialect and lifecycle. It also intersects the platform vision — KAS is the first Kiro engine that exposes filesystem callbacks, which is a genuine proxy-stage interception point (links to Phase 5).

**Estimate:** ~7–10 weeks across milestones KAS-0…7 (KAS-2 split into 2a–2d during the 2026-06-16 grilling). The **walking skeleton is KAS-0 → KAS-1 → KAS-2a**.
**Depends on:** spawn-level engine selection needs **no new transport work** — `main.rs` already takes `--agent-command <Vec<String>>` → `AgentCommand::try_from_argv`, so appending `--agent-engine kas` is free today. Phase 1's refactor is still the tidy long-term home for the arg, but does not gate this track. Otherwise orthogonal to Phases 2–5 and the K-track. Requires kiro-cli ≥ 2.7.1 at runtime.
**Sequencing (decided 2026-06-16):** **K1 (queue steering) ships before this track.** K1 is higher-certainty value (real v2 users, today) and only weakly coupled — it adds three `sessionUpdate` variants to `convert/kiro.rs`, which KAS-0's `Engine`-trait port then absorbs at negligible cost. Honors Open Tension #6 and keeps the credential-custodian surface (KAS-1) off the critical path until concrete v2 value has shipped.
**Milestone order (revised 2026-06-16):** after the walking skeleton (KAS-0 → KAS-1 → KAS-2a), **KAS-5 (host I/O callbacks, fs first) jumps ahead of KAS-3/KAS-4.** It is both the heaviest and the most platform-aligned milestone — the first real `cyril-stages` interception point — so proving it early converges the KAS track with Phase 2 instead of leaving it to the tail. KAS-2b/2c/2d (metering / governance / agent-config) are small additive polish that slot in opportunistically. Net order: **0 → 1 → 2a → 5 → (2b/2c/2d, 3, 4, 6, 7)**.
**Architecture (see [ADR-0001](adr/0001-kiro-engine-trait.md), [ADR-0002](adr/0002-kas-cargo-feature-gate.md)):** engine is bound at **agent-subprocess spawn** (the bridge runs one `kiro-cli acp [--agent-engine kas]` process and holds one `Box<dyn Engine>` for its life — every session on it shares the engine, so there is no per-session engine lookup); **startup-only selection in v1**, switching means restarting the subprocess. The two engines sit behind a **small Kiro-scoped `Engine` trait** (convert notification → internal `Notification`; declare `client_capabilities`; detect turn-end) **plus optional capability sub-traits** (`AuthResponder`≈KAS-1, `HostIo`≈KAS-5, `HooksHost`≈KAS-7, `GovernanceSource`≈KAS-2c) that KAS implements and v2 does not. Engine nests *under* the Kiro vendor — **not** the same mechanism as the Phase-1/4 vendor seam (Claude does not implement `Engine`). KAS code, especially the credential-reading `AuthResponder`, lives behind a default-off **`kas` cargo feature**; a default build cannot read the kiro token. **"KAS-2" in milestones below now means KAS-2a (the walking skeleton)** unless noted.
**Wire reference:** [`docs/kiro-2.7.1-wire-audit.md`](kiro-2.7.1-wire-audit.md) — the full audit, plus the reproducible probes in [`experiments/conductor-spike/`](../experiments/conductor-spike/) (`probe-kas-subagent`, `probe-kas-fs`, `probe-kas-orchestrate`, all 2.7.1).
**Prerequisite — ACP crate coverage (KAS-0 verification spike, *not* a blocker).** *Corrected 2026-06-16 (grilling).* The earlier "cyril pins **0.9**, KAS ships **^0.19**, bump before KAS-2" note was stale and conflated two version lines. Facts: cyril is **already on `agent-client-protocol` 0.10.2 / schema 0.11.2** (Cargo.lock), and **0.11.2 already carries the typed `SessionInfoUpdate` variant** that KAS's `turn_end` rides — so the walking skeleton (KAS-2a) is **not** gated on an SDK migration. The "0.19" was the *TypeScript* `@agentclientprotocol/sdk` version, a different numbering line from the Rust crate (apples to oranges). Residual risk is narrower and operational: `SessionUpdate` is `#[serde(tag = "sessionUpdate")]` with **no `#[serde(other)]` catch-all** (`#[non_exhaustive]` is a Rust API attribute, not serde tolerance), so a *future* KAS-emitted typed `session/update` variant absent from 0.11.2 would hard-fail at the acp-crate deserialization layer **before** `convert/` runs — and, unlike the raw `_kiro/*` ext path (which is JSON-tolerant), standard `session/update` offers no raw fallback hook. Mitigation chosen during grilling: a **KAS-0 verification spike** confirms 0.11.2 deserializes every typed `session/update` variant KAS emits live (`agent_message_chunk`, `tool_call`/`tool_call_update`, `available_commands_update`, `config_option_update`, `session_info_update`); the no-catch-all is a documented upgrade-trigger, not a code defense (forking the crate or intercepting beneath the acp Client trait is deferred until a live unknown variant actually bites).

**Protocol coverage matrix.** The milestones below were built bottom-up from probes; this matrix is the top-down completeness check against the covenant's full surface ([`kiro-kas-acp-covenant.md`](kiro-kas-acp-covenant.md) §1 — the authoritative method catalog). Legend: ✅ has a milestone · ⚠ consciously deferred, *or* tolerated by KAS-2a's unknown-variant `debug!` arm (invisible, not broken) · ❌ no milestone → **KAS-8**.

*Agent→client requests (host callbacks — cyril answers these only when it advertises the gating `_meta.kiro` flag):*

| Surface | Coverage |
| --- | --- |
| `auth/getAccessToken` | ✅ KAS-1 |
| `fs/*` · `_kiro/fs/*` | ✅ KAS-5a |
| `terminal/*` · `_kiro/terminal/shell_type` | ✅ KAS-5b / 2a |
| `hooks/{list,executeHook,sessionStart}` | ✅ KAS-7 |
| `userInput` | ✅ CN1 (optional) |
| `secret/*` · `openExternalUrl` · `tool/{semantic_rename,smart_relocate,get_diagnostics}` · `mcp/elicitation` · `search/{find_files,text_search}` | ❌ KAS-8 — opt-in (omit the flag → never called) |

*Client→agent requests (optional — cyril chooses to call these):*

| Surface | Coverage |
| --- | --- |
| `permissions/{list,explain}` · `policy/check` | ✅ KAS-7 |
| `account/getUsage` · `codeIntelligence` | ✅ KAS-4 |
| `session/{compact,export,history,context,delete,rename}` · `spec/*` | ⚠ KAS-7 non-goals |
| `checkpoint/{revert,revertMultiple}` · `mcp/{resetServer,getPrompt,getResource}` · `hooks/{triggerHook,setEnabled}` · `tasks/*` · `knowledge` | ❌ KAS-8 |

*Agent→client notifications (the converter receives all; unhandled = invisible, not protocol-breaking):*

| Surface | Coverage |
| --- | --- |
| `session_info_update` (18-kind union) | ✅ KAS-2a (turn_end) + KAS-2b |
| `governance/state` | ✅ KAS-2c |
| `hooks/{cancel,didChange}` | ✅ KAS-7 |
| `steering/documents_changed` (catalog; the *injection* signal is `session_info_update:steering_inclusion`, KAS-2b) | ✅ KAS-6 |
| `powers/items_changed` · `progressive_context/items_changed` · `system/notify` · `sessions/changed` · `tools/didChange` | ⚠ tolerated; surface opportunistically |
| `safety/{statusChanged,propertiesChanged}` | ❌ KAS-8 — **can block tool calls** (enforcement, not display) |
| `error/rate_limit` | ❌ KAS-8 — UX-important |
| `mcp/{status,governance_disabled}` · `policy/{changed,error}` · `code_references` · `customAgent/{not_found,config_error}` | ❌ KAS-8 |

**Currency caveat:** the covenant *doc* catalog is synced at `@kiro/agent` 0.3.224; the installed package is 0.3.257 (2.8.1). Clean method-level diff = exactly 6 uncataloged methods, all mapped above: `safety/{statusChanged,propertiesChanged}`, `sessions/changed`, `hooks/setEnabled`, `search/{find_files,text_search}` (the last are **host callbacks** in the fs/terminal family). Re-sync the covenant per release (README step 6) so this matrix stays exhaustive.

### KAS-0 — `Engine` trait + v2 port + gating (invisible foundation)

**Depends on:** nothing. Ships **no** user-visible change — its acceptance criterion is strict v2 parity.

Stand up the seam everything else hangs off, without changing v2 behavior:

- Introduce the Kiro-scoped **`Engine` trait** (core surface — convert + `client_capabilities`) and **port today's `convert/kiro.rs` behind a `V2Engine` impl** with zero behavior change. (The first **capability sub-trait** stub was originally slated here but moved to KAS-1 — a consumer-less stub is dead code under `-D warnings`; see ADR-0001.) This is a pure refactor of the most load-bearing working code — its oracle is **behavioral**: every existing v2 test passes *and* a live `kiro-cli acp` session streams / tool-calls / renders subagents / mode-picks identically (CLAUDE.md "verify functional wiring end-to-end after any refactor").
- Wire the **engine gate at bridge spawn**: an `AgentEngine` enum derived from the startup `agent_command` (default v2); `spawn_bridge` picks the one `Box<dyn Engine>` the bridge uses for its life. **Startup-only in v1** (`--agent-engine` / config) — `session/new` cannot change engine (it reuses the existing subprocess), so a live `/engine`-as-respawn (tear down + re-spawn the bridge) is deferred. Selecting a non-v2 engine before KAS-1 cleanly reports "not available yet".
- Establish the **`kas` cargo feature** (default off, empty for now) — the project's first feature flag; add a CI lane that builds + lints + tests `--features kas` so it cannot bitrot under "warnings are errors."
- **ACP-coverage verification spike** (see Prerequisite above): confirm schema 0.11.2 deserializes every typed `session/update` variant KAS emits live; the stale 0.9 version facts were fixed 2026-06-16 in this file and CLAUDE.md.

### KAS-1 — Engine selection + auth responder (the entry gate)

**Depends on:** KAS-0 (the engine gate + `kas` feature exist; KAS-1 fills in the `AuthResponder` and the live KAS spawn).

Without a host-supplied token, every KAS turn dies immediately (`[TokenInvalidError] … Host refresh callback returned no access token`) — **when spawned the way the v3 TUI does.** But there are two spawn shapes, and only one needs the responder (see the free-path note next).

**Free-path de-risking (verified live 2026-06-21).** A first live KAS turn is reachable with **zero credential-custodian code**: spawning `acp-server.js` **directly** over stdio with no `--auth` flag uses KAS's default file-auth (reads the SSO cache, self-refreshes), so `_kiro/auth/getAccessToken` never fires — only the `kiro-cli` *wrapper* spawn injects `--auth=acp-callback` and makes the responder mandatory. This moves Open Tension #7 **off the critical path for the walking-skeleton demo**: ship the free path first (the responder below is for the blessed wrapper lifecycle + Builder-ID/external-IdP users); the trade-off is that direct spawn makes cyril own server-entry + node-runtime discovery. **Full auth-mode + flag contract, 5-tier priority, and the probe:** [`kiro-2.8.1-wire-audit.md` § KAS runtime behavior](kiro-2.8.1-wire-audit.md#kas-runtime-behavior--live-capture-2026-06-21-addendum) (+ memory `reference_kiro_kas_launch_contract`).

- Make the KAS spawn real: the `AgentEngine` gate from KAS-0 now resolves the KAS engine to a live `kiro-cli acp --agent-engine <flag>` process (default stays v2). All new KAS code lands behind the `kas` cargo feature established in KAS-0. **The CLI engine flag is version-dependent:** kiro-cli **2.8.0 renamed `--agent-engine kas` → `v3`** (probe-verified 2026-06-19: `kas` is rejected; the accepted values are `v1`/`v2`/`v3`), whereas 2.7.1 accepted `kas`. Resolve the flag from the installed version rather than hardcoding `kas`. (The `kas` cargo feature is cyril's own gate and is unrelated to this CLI flag.)
- Implement an **`_kiro/auth/getAccessToken`** server→client request responder: reply `{ accessToken, expiresAt, profileArn }`. KAS validates `expiresAt` is > now + ~3 min and **requires `profileArn`** (backend 400s "profileArn is required" without it).
- Token sourcing — **mirror Kiro's own responder, don't just read the sqlite row.** Kiro implements this in `crates/chat-cli-v2/src/auth/{kas_token.rs, refresh_coordinator.rs, social.rs, builder_id.rs, external_idp.rs}`: it (1) resolves the active token across **three types** — social (GitHub → `kirocli:social:token`, carries `profile_arn`), AWS Builder ID, and external IdP (`kirocli:external-idp:token`); (2) **proactively OIDC-refreshes** (`create_token`, `grant_type=refresh_token`, `oidc.*.amazonaws.com`) before the ~3-min pre-expiry buffer, through a **lock-guarded `refresh_coordinator`** that serializes concurrent refreshes; (3) answers `{accessToken, expiresAt, profileArn}`. A naive "read `kirocli:social:token` and return it" responder breaks on the refresh-buffer (the failure our probes hit) and on Builder-ID/external-IdP users. Cleanest for cyril: **delegate to kiro-cli's own auth** rather than reimplement the OIDC refresh + multi-type resolution. **This makes cyril a custodian of a kiro credential** — handle it as such (no logging, read-only, minimal lifetime). See new Open Tension #7.
- **The auth responder is only the credential half of the `initialize` → `_meta.kiro` (`KiroClientMeta`) handshake.** The other half is `_meta.kiro.settings` (`AgentSettings`) — marshaling the user's `kiro-cli settings` into the handshake so KAS runs with `thinking` / `toolSearch` (MCP token savings) / `knowledge` / `compaction` / `subagentOrchestration` etc. enabled, instead of the degraded all-flags-off default cyril gets by sending only `{cwd, mcpServers}`. This is the v2-vs-KAS asymmetry: the v2 Rust backend reads these settings straight from the settings file across the ACP boundary (free for cyril), but KAS is a separate process and must be told over the wire. Filed separately as **cyril-nhzw** (blocked by this milestone) since it warrants its own `AgentSettings` type + tests; see covenant §3 (`docs/kiro-kas-acp-covenant.md`).
- Tests: auth-responder unit test (shape + expired-token rejection), engine-select plumbing, a gated end-to-end smoke against a live KAS session.

### KAS-2a — Walking skeleton: a plain KAS turn renders **and ends**

**Depends on:** KAS-1. **This is the milestone that earns a demo** — select KAS, authenticate, prompt, watch text + tool calls stream, turn completes, input is live again.

The *minimum* for a plain turn to render and end cleanly. The other `_kiro/*` surfaces (governance, mcp/status, powers, progressive_context) and the other `session_info_update` kinds all fire on a plain turn too, but cyril's existing **unknown-variant `debug!` arm drops them gracefully** — so they are deferred to 2b–2d without hanging anything.

- Converter arm (engine-keyed, in the `Engine`-trait structure from KAS-0) for **`agent_message_chunk` / `agent_thought_chunk` / `tool_call` / `tool_call_update`** only.
- **`session_info_update` → `kind: "turn_end"`** mapped to cyril's turn-completion / busy-clear. **This — not the `session/prompt` response — is the turn-completion signal under KAS** (audit §"session_info_update"). **Hang-proofing acceptance criterion:** confirm cyril's prompt-response await is **non-blocking** under KAS — drive completion off `turn_end` and treat the prompt response as secondary/whenever-it-arrives, so a non-returning response cannot freeze the skeleton. Verify with the existing `probe-kas-*` scripts (no new probe needed).
- Answer **`_kiro/terminal/shell_type`** (`{sessionId}` → `{shellType: "bash"|"zsh"|"fish"|"powershell"|"sh"}`) — the lightest host callback, called once at session setup; a missing reply yields `Shell: undefined` in the system prompt.
- Handle **`sess_…`-prefixed session ids** and per-run log dirs under `~/.kiro/logs/<ts>/`.
- Defensive unknown-variant tolerance for everything else (one `debug!` line noting `_kiro/*` surfaces are intentionally dormant pre-2b).

> **Note — the off-loop busy-guard + `TurnCompleted` emission from PR #22 (cyril-84ca) must be reworked here.** That PR drives the v2 turn off the command loop: `TurnCompleted` is emitted when `conn.prompt().await` resolves, and the one-turn busy-guard keys off `prompt_task.is_finished()`. Both encode **"prompt response == turn end"** — correct for v2, **false for KAS**, where turn-end is `session_info_update → turn_end` and the prompt response is secondary/late. So under KAS, `is_finished()` stays false after a logical turn-end (the next legitimate prompt is wrongly rejected with "a turn is already in progress") and `TurnCompleted` fires late/never — the exact hang this milestone's non-blocking acceptance criterion guards against. Fix: route **both** the busy-guard and `TurnCompleted` emission through the engine's turn-end signal (KAS-0's turn-end detection in the `Engine` trait), not `prompt_task.is_finished()` / `prompt().await`. The concrete code is the `SendPrompt` guard + the spawned `prompt_task` in `bridge.rs`; tracked on **cyril-atjw (KAS-0)**.

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

- Surface `configOptions`: `mode` (vibe / spec / quick-spec / bug-fix / plan / autonomous / semantic_reviewer), `autopilot` (on / Supervised), `contentCollection`. The existing mode picker generalizes to these; `autopilot` is a session-level permission posture cyril can expose directly instead of mediating per-tool approvals. **Option entries are `{value, name}` — key the picker on `options[].value`, not `.id`** (2.10.0 probe: keying on `.id` yields all-`None`). Live behavioral contracts for `plan` / `bug-fix` / `quick-spec` (read-only enforcement; docs-first turn 1; quick-spec's `userInput` clarify + plan-approval gate): [`kiro-kas-modes-2.10.0.md`](kiro-kas-modes-2.10.0.md).
- **Read the *initial* `configOptions` from the `session/new` response.** `session_created_from_response` (`convert/mod.rs:61`) currently reads only `modes`+`models` and ignores `config_options` (correct for v2, where it's always `null`). Under KAS it's populated, so the initial set must be lifted here; today cyril would only pick up config state from a later `config_option_update` — and on KAS the *set* path fires no echo (see below), so the initial read is the only way to learn the starting `mode`/`autopilot`. Surfaced by the 2.7.1 Rust-vs-`.d.ts` type comparison.
- Wire `session/set_config_option` — **verified working** (2026-06-16): request `{sessionId, configId, value}`, returns the rebuilt `configOptions` (**the source of truth — always read the response**). Caveat: invalid values are silently coerced, not rejected (`autopilot="bogus"` → `"off"`), so cyril should constrain `value` to the advertised option ids client-side. **Echo behavior is version-drifty — do not depend on it either way:** the 2.7.1 probe (0.3.257) saw *no* `config_option_update` after an explicit set, but the 2026-07-02 modes probe (2.10.0 / 0.3.299) observed a post-set rebuild broadcast (`available_commands_update` + `config_option_update` + `_kiro/tools/didChange`; set-echo vs turn-start not disambiguated). Treat any echo as an idempotent state refresh, never as the confirmation signal. See [`kiro-kas-modes-2.10.0.md`](kiro-kas-modes-2.10.0.md) wire delta #2.
- **`/code` panel under KAS** — `_kiro/codeIntelligence {subcommand:'status'}` (gated by `settings.codeIntelligence`) returns `{initialized, languages[], lspServers:[{name, languages[], status, isAvailable, initDurationMs?}]}` — verified live 2026-06-16, and it **maps directly onto cyril's existing `CodePanelData`/`LspServerInfo` types**, so the `/code` panel works against KAS with a thin converter arm. `init`/`overview` subcommands do the heavier work.
- **`/usage` panel under KAS** — `_kiro/account/getUsage` (client→server request) returns the billing/usage data v2 only exposed via the `/usage` slash command. Shape (verified live 2026-06-16): `{success, message, data:{planName, billingCycleReset, overagesEnabled, isEnterprise, usageBreakdowns:[{resourceType, displayName, used, limit, percentage, currentOverages, overageRate, overageCharges, currency}], bonusCredits[]}}`. cyril can render a real usage/credits panel from this instead of parsing a slash-command response. (Carries account/billing data — keep it out of logs/telemetry.)

### KAS-5 — Host I/O callback responders: fs **and** terminal (first real proxy-stage hook)

**Depends on:** KAS-2a (the walking skeleton must render a turn first). **Pulled ahead of KAS-3/KAS-4** per the revised milestone order — this is the **first `cyril-stages` deliverable**, converging the KAS track with Phase 2.
**Wire reference:** the host-responsibility callback map in [`docs/kiro-2.7.1-wire-audit.md`](kiro-2.7.1-wire-audit.md), reproduced by `experiments/conductor-spike/probe-kas-callbacks-2.7.1.py` and (the three-mode fs gating + a working real-disk responder) `probe-kas-fs-callbacks-2.8.1.py`.

KAS is the first Kiro engine to delegate **both file I/O and shell execution** to the host via ACP callbacks, each **capability-negotiated**: what cyril advertises in `initialize` decides whether KAS calls back or runs in-process (v1/v2 behavior). Opt-in, not mandatory — but the two together are the platform's first real interception point over a Kiro agent's side effects.

> **Verified end-to-end 2026-06-16** (`experiments/conductor-spike/probe-kas-fs-terminal-host-2.7.1.py`): advertising `fs={readTextFile,writeTextFile}` + `terminal:true` and implementing real responders, KAS routed **every** read/write/exec of a tool-using turn through the client (bare-ACP names; agent does read-before-write + verify-after-write; terminal lifecycle `create→wait_for_exit→output→release`). A file written only via our `fs/write_text_file` responder landed on disk → the delegation is real. The platform thesis is proven; this milestone is now implementation, not investigation.

- **Filesystem — three advertisement-selected modes** (mapped live 2026-06-21): advertise **nothing** → KAS does fs **in-process**; advertise `fs={readTextFile,writeTextFile}` → **base ACP** `fs/read_text_file`+`fs/write_text_file` (portable, public names); advertise `fs._meta.kiro={readFile,writeFile,stat,readDirectory,delete}` → the **Kiro superset** `_kiro/fs/*` (range-aware writes, plus stat/read_directory/delete). **The superset wins when both are advertised**, so this is a real cyril choice — base for vendor-portability vs. the richer kiro tier (which must implement all five, since the agent stats + lists the dir before writing). KAS sends **absolute** paths, which is why `platform/path.rs` is load-bearing. Full shapes, gating, and the working responder reference: [`kiro-2.8.1-wire-audit.md` § KAS runtime behavior](kiro-2.8.1-wire-audit.md#kas-runtime-behavior--live-capture-2026-06-21-addendum).
- **Shell/terminal** — advertise `clientCapabilities.terminal = true` → implement the lifecycle `terminal/create` (`{sessionId, command, args[], cwd}` → `{terminalId}`) → `terminal/wait_for_exit` (`{terminalId}` → `{exitStatus}`) → `terminal/output` (`{terminalId}` → `{output, exitStatus}`) → `terminal/release` (+ `terminal/kill`). With `terminal:true` advertised, **every command the agent runs flows through these on the host**; omit the capability and KAS runs shell in-process. Pairs with `_kiro/terminal/shell_type` from KAS-2.
- cyril implements **none** of these today (empty `clientCapabilities`), so this is purely additive. Owning `fs/*` + `terminal/*` lets cyril audit/gate/translate every file op and command KAS performs — the natural home for transcript audit, org write/exec policy, and Windows/WSL path translation as a **stage** (`crates/cyril-stages/`, Phase 2) rather than ad-hoc bridge code.

**Sub-milestones — ship fs first (simpler, no lifecycle), then terminal; each lands independently behind its capability flag:**

- **KAS-5a — fs callbacks.** Pick a tier (per the three-mode finding above): **base** `fs={readTextFile,writeTextFile}` (portable, public ACP names — recommended default) or the **kiro superset** `fs._meta.kiro={readFile,writeFile,stat,readDirectory,delete}` (range-aware, but requires implementing stat + read_directory + delete too). Implement the chosen tier in the `cyril-stages` host-callback layer (not the bridge). Write fires `session/request_permission`. **Cross-platform acceptance:** `platform/path.rs` translation is now load-bearing — KAS sends absolute paths, so verify on **Linux** (no-op) *and* **Windows** (`C:\`↔`/mnt/c` both directions) that paths in/out of the responder are correct, a file written only via the responder lands at the right host path, and the agent's read-before-write/verify-after-write loop sees consistent paths. The Python responder in `probe-kas-fs-callbacks-2.8.1.py` is a line-for-line reference for the Rust implementation.
- **KAS-5b — terminal callbacks.** Advertise `terminal:true`; implement `terminal/create→wait_for_exit→output→release` (+ `kill`). Pairs with `_kiro/terminal/shell_type` (KAS-2a). **Cross-platform acceptance:** verify command execution on **Linux** (native) and **Windows** (decide + test native-host vs WSL execution, per the original mission's terminal concern), exit-status/output capture, and that `shell_type` matches the shell commands actually run on.

Both sub-milestones are the same host-executor shape as KAS-7's hook responder, so they share the `cyril-stages` home: process spawning, path translation, and the org write/exec-policy gate belong in one layer, not scattered across the bridge.

### KAS-6 — Conversation-file context (lights up conditional `fileMatch` steering + spec activeFile)

**Depends on:** KAS-1; pairs with KAS-2.

> **Corrected 2026-06-21** — this milestone's original premise was wrong. It assumed `fileMatch` steering keys off an IDE-supplied `openFiles` set (editor tabs) fed via a callback, and is therefore *dormant* for a chat client. It is not. **fileMatch matches files already in the *conversation*** — file content blocks attached to a prompt + paths the agent reads via `read_file` (`getWorkspaceFiles()` in `@kiro/agent`) — re-checked at every tool boundary, and the injected doc stays resident for the session. Verified live (`experiments/conductor-spike/probe-kas-filematch-steering{,-v2,-v3-persistence}.py`); see the [2.7.1 audit steering section](kiro-2.7.1-wire-audit.md#steering-inclusion-under-kas--filematch-matches-conversation-files-not-ide-open-tabs).

What this actually means:

- **The common case already works, no client work required:** any file the agent reads via `read_file` triggers its matching `fileMatch` steering for the rest of the session (steering is sticky once injected — deduped, never evicted). KAS does **not** call `_kiro/workspace/{currently_open_files,active_file}` in a bare-ACP session, so there's nothing to "fill in" there.
- **The real client lever — let the user trigger it deliberately:** map `@`-attached / referenced files to ACP **file content blocks** on `session/prompt`. KAS records those as `document.type:"file"` workspace files, so attaching `Foo.tsx` activates `fileMatchPattern:"**/*.tsx"` steering. This is the small, high-leverage change — not "synthesize an editor open-files set."
- **Surface what's active (UI):** build an "active rules/steering" view from `session_info_update:steering_inclusion` (the injection events, by URI) cross-referenced with `_kiro/steering/documents_changed` (the catalog, each doc tagged `always|fileMatch|manual`). **Do not** read `/context` for this — it lists only user-attached files and token buckets, never steering.
- **Separate, smaller lever (don't conflate):** the wire `openFiles`/`activeFile` graph-state fields *do* exist on `_meta.kiro`, but a code comment ties them to **intent-detection weighting** (and possibly spec `activeFile`) — *not* steering. Supplying them is optional and independent of the steering win above; treat it as a nice-to-have, not the point of this milestone.
- Vendor-note: file content blocks are standard ACP; the steering activation they cause is Kiro-specific, but the *concept* (telling an agent "these files are in play") generalizes if other ACP agents grow similar context hooks.

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

### KAS-8 — Remaining protocol surfaces (completeness sweep)

**Depends on:** KAS-2a (the converter + host-callback layer exist). Mostly small/additive; several items are conscious **decline** decisions, not implementation work. This milestone exists so every surface the coverage matrix marks ❌ is *decided*, not silently missing — it closes the "is the KAS protocol fully wired?" question.

**Functional gaps — promote from silently-tolerated to tracked (each filed as a rivets issue):**

- **`_kiro/safety/{statusChanged,propertiesChanged}` — Infrastructure Safety gate (kiro-cli 2.8.0+).** Cedar/AWS-governance signals that **can block a tool call**. Unlike the other notification gaps this is *enforcement*, not display: dropping it means cyril presents a tool as runnable that KAS will refuse, or misses a posture change. Surface the safety status and reflect a safety-blocked tool in the UI (pairs with the KAS-2c `GovernanceSource` sub-trait). Tracked on **cyril-3ald**.
- **`_kiro/error/rate_limit`.** Currently dropped by the unknown-variant arm — the user sees a stalled turn with no reason. Surface as a system message (and clear on next turn). Tracked on **cyril-3zy4**.
- **`_kiro/mcp/*` — MCP-management subsystem** (`status` + `governance_disabled` notifications; `resetServer` / `getPrompt` / `getResource` / `elicitation` requests). v2 has a `/mcp` command; under KAS the MCP surface is this `_kiro/*` dialect. A `/mcp` panel + OAuth-reset flow. Tracked on **cyril-nk4o**.

**Opt-in host callbacks — decide advertise-or-not (default: don't advertise; KAS falls back in-process):**

- `secret/{get,store,delete}` (gated `secretStorage`) — **purpose pinned (source 2026-06-21):** persistence backend for **remote-MCP OAuth state only** — OAuth client-registration info, access/refresh tokens, and PKCE verifier, keyed `kiro.mcp.<sha256(url+headers)>` per connection (`AcpSecretStorage`/`CredentialStorageManager`). Not a general secret API. Declining only means OAuth'd remote MCP servers re-auth each session (KAS keeps its own backend); stdio/non-OAuth MCP unaffected. Advertise only if cyril hosts KAS with remote OAuth MCP **and** backs it with real secure storage (never plaintext — cyril becomes custodian of MCP OAuth tokens). Default **decline**. See memory `reference_kiro_kas_secret_storage`.
- `openExternalUrl` (gated `openExternalUrl`) — open auth/doc URLs from the host; cheap, decide per UX.
- `tool/{semantic_rename,smart_relocate,get_diagnostics}` (gated `clientTool*`) — client-side **LSP** callbacks; a chat TUI has no language server, so **decline** (KAS uses its own).
- `mcp/elicitation` — MCP structured prompts; bundle with the `/mcp` work above.
- `search/{find_files,text_search}` — **host callbacks** (fs/terminal family): `find_files {pattern,exclude?,maxResults}`→`{files[]}`, `text_search {pattern,caseSensitive,includePattern?,excludePattern?}`→`{matches[]}`. The agent delegates file/grep search to the host — a natural **decline** for now (KAS searches in-process), but it's the same audit/gate interception surface as fs/terminal if cyril ever wants to own search too. Characterized 2026-06-21; not yet in the covenant doc catalog.

**Optional client→agent calls — add when a UX needs them:**

- `checkpoint/{revert,revertMultiple}` — the KAS analog of v2 `/rewind`; wire when cyril surfaces rewind under KAS.
- `hooks/{triggerHook,setEnabled}` — `setEnabled` (persist a hook's enabled state, 2.8.1) pairs with a KAS-7 hooks tree UI (enable/disable from the view); `triggerHook` is the manual-fire path. Wire with the KAS-7 hooks work.
- `tasks/{list,get_metadata}`, `_kiro/knowledge` — feature-specific; out of scope until their UX lands.

**Tolerated notifications — surface opportunistically (the `debug!` arm keeps them non-fatal):** `powers/items_changed`, `progressive_context/items_changed`, `system/notify`, `_kiro/sessions/changed` (the multi-client observer roster — feeds the session-level-workflow direction), `tools/didChange`.

**Conscious exclusions (restated):** `session/{compact,export,history,context,delete,rename}` and the full `spec/*` workflow stay KAS-7 non-goals; `customAgent/{not_found,config_error}` ride the Phase-5 client-agent-injection work, not this track.

## Vendor-neutral client features (candidates)

Small client-side UX features that key off **standard ACP** (not a vendor extension), so they work across every registered agent. Independent of the platform phases and the vendor tracks.

### CN1 — Notify on pending approval

**Estimate:** small (~2–4 days).
**Depends on:** nothing — `session/request_permission` already drives cyril's approval overlay.

Fire a user-attention signal (terminal bell / OS notification, configurable) when a `session/request_permission` is pending and the TUI isn't focused (or after a short idle delay), and clear it on response. Lets the user walk away from a long turn and get pulled back exactly when the agent is blocked on them.

- **Why it's a cyril feature, not a hook:** verified 2026-06-16 that KAS's `HookTrigger` enum has exactly 11 values with **no `Notification`/permission/`WaitingForApproval` trigger** — you can *gate* a permission decision with a `PreToolUse` hook but cannot get a hook that fires *when the agent pauses for you*. Kiro handles that notification at the protocol/client layer instead (`session/request_permission` + the agent's own `_kiro/system/notify`), and the agent-side notification is v2-TUI-only anyway (the Rust TUI's BEL/OSC-9, which cyril never receives — see the notifications research). So cyril is the right place to own this, and doing it generically covers every agent.
- **Vendor-neutral:** `session/request_permission` is core ACP — this works for Claude, Codex, Kiro (any engine), etc., with no extension dependency.
- **Scope:** a notifier keyed off the existing `PermissionRequest` → `UiState::show_approval()` path; settings for method (bell / OS notification / none) and trigger (always / only-when-unfocused / after-Ns-idle); clears on approve/deny/cancel. No new wire surface, no overlay/key-chain changes. Could later generalize to a turn-completion notification (the other thing Kiro's v2-TUI-only BEL does).
- **KAS richer-question channel (optional, additive):** verified 2026-06-16 that KAS also exposes `_kiro/userInput` (gated by `_meta.kiro.userInput:true`) — a *structured-question* callback distinct from `session/request_permission`, carrying multi-option `{title, description, recommended, subOptionsLabel?, subOptions?}` (nested choices). It coexists with the permission channel (questions vs tool-approvals) and is triggered by the `spec`-tagged `user_input` tool (spec flows). If cyril builds a richer approval/question overlay, advertising `userInput` upgrades KAS clarifying questions from a plain chat message to a real multi-choice picker — but it's KAS-specific, so keep the core notifier vendor-neutral and treat this as a capability-gated enhancement. See the 2.7.1 audit "userInput + client-side LSP tool callbacks".

### CN2 — Voice input (speech-to-text prompts)

**Estimate:** medium. V1a walking skeleton ~2–3 days; V1b remote transcription ~1 week; V1c polish ~3–5 days; V2 local engine ~1–2 weeks.
**Depends on:** nothing. Fully vendor-neutral and orthogonal to the agent/engine — speech→text happens entirely client-side and the result enters as an ordinary `session/prompt`, so it works against **every** ACP agent with zero wire surface.

Hold-to-talk (or toggle) speech-to-text that drops the transcript into the input buffer. Modeled on Kiro's own v2-TUI voice feature, which is **client-local and never crosses ACP** (audit 2026-06-19): Kiro captures audio and runs **OpenAI Whisper locally via ONNX Runtime** (`ort` crate, `base|small` models from its own S3), *or* streams to a remote server gated by the `KIRO_VOICE_SERVER_URL` env var. Notably Kiro **registers a `voice.serverUrl` setting that is wired to nothing** (no reader, no env bridge) — cyril should do the thing Kiro left half-built: a real `[voice]` config block whose `endpoint` is actually consumed.

**Architecture (decided 2026-06-19).** Mirror the bridge: a dedicated `cyril-voice` thread spawned like `spawn_bridge` (`protocol/bridge.rs`), talking to the App over mpsc channels (`VoiceCommand` in, `VoiceEvent` out), surfaced as a 5th `tokio::select!` arm in `app.rs`. Control-plane types (`VoiceCommand`/`VoiceEvent`/`VoiceStatus`/`VoiceHandle`) live in **cyril-core** (lightweight, always compiled) so the App's field and select arm are cfg-free; the **engine** (heavy audio/ML deps) lives in **`cyril-voice` behind a default-off `voice` cargo feature** (ADR-0002 `kas` precedent). Engine init never fails at spawn — errors arrive as `VoiceEvent::Error`. STT is a swappable `Transcriber` trait; the first real impl is **remote-batch** (`POST /v1/audio/transcriptions`, OpenAI-compatible — works with OpenAI, Groq, self-hosted whisper.cpp server) so v1 needs no bespoke server and no bundled model. Display state (`voice_status`, `voice_level`) lives in `cyril-ui`; transcript inserts via the existing `UiState::insert_text`.

**Milestone V1a — Walking skeleton (no audio, no network).** `cyril-voice` crate + feature flag; `VoiceCommand`/`VoiceEvent`/`VoiceHandle` in cyril-core; `/voice` slash command toggles via `CommandResultKind::ToggleVoice`; 5th select arm routes events; a **stub transcriber** emits `Status(Listening)` + oscillating `Level` ticks on start and a fixed `Transcript` on stop. Proves the full wiring end-to-end — press `/voice`, watch a stub line appear in the input. Voice meter row (`widgets/voice.rs`, `height_for` like `crew_panel`) + `any_voice_active()` fast-tick.

**Milestone V1b — Remote transcription.** `cpal` mic capture → 16 kHz mono PCM + RMS level; `RemoteTranscriber` (multipart POST to the configured endpoint); `[voice]` config (`endpoint`, `model`, `api_key_env`, `silence_timeout`, `auto_submit`); silence-timeout auto-stop; "transcribing…" status during the POST. No partials (batch endpoint) — the level meter + spinner carry the UX honestly.

**Milestone V1c — Polish.** `auto_submit` (Enter the transcript automatically); Esc-cancels-recording folded into the key-layer chain (`app.rs`); true push-to-talk (hold-key) where the terminal supports the **Kitty keyboard protocol** (key-release events), toggle/VAD fallback elsewhere.

**Milestone V2 — Local engine (later, behind the feature).** `LocalTranscriber` via `whisper-rs` (whisper.cpp) or `candle`, with model download + cache. The offline/privacy story (audio never leaves the box), matching Kiro's default path.

- **Layering / safety:** `unsafe_code = "forbid"` is per-crate and does **not** propagate to deps, so `cpal`/`whisper-rs`/`ort` (all `unsafe` internally) are fine as long as cyril's own code stays safe. Heavy deps stay behind the `voice` feature; a default `cargo build` pulls in none of them.
- **Non-goals:** replicating Kiro's exact on-device ONNX stack (V2 picks whatever Rust STT is cleanest); streaming partial transcripts in v1 (batch endpoint); voice *output*/TTS.

## Open tensions

1. **Kiro is conspicuously absent from the ACP registry.** Cyril's most-tested agent is the one outside the curated ecosystem. Either ignore (use Kiro by direct path, others via registry) or advocate to AWS for registry membership.
2. **Some current cyril UI is Kiro-specific** (mode picker, welcomeMessage rendering, `kiro.dev/*` command parsing). Vendor-neutrality means generalizing, gating on detected capabilities, or accepting they only show for Kiro.
3. **Auth flows differ per vendor.** Each registered agent has its own auth method. Real UX work to handle showing/redirecting auth flows per agent.
4. **Stages framework is ahead of the curve.** No standardized stage registry exists yet; `sacp-conductor` and `sacp-proxy` are one project's framework. If that stack doesn't become canonical, cyril's stages story may need to migrate.
5. **Single-maintainer risk on the Rust tooling tier.** `sacp`, `sacp-tokio`, `sacp-conductor`, `sacp-proxy`, and `acpr` are all published by Niko Matsakis. The protocol itself is multi-stakeholder; the Rust tooling is one-person-led. Mitigated by clean exit ramps (all MIT/Apache), but worth knowing.
6. **Mission drift.** "Vendor-neutral platform" is more ambitious than "Kiro client." Need to keep shipping a strictly-better-than-status-quo Kiro experience while building the platform underneath.
7. **KAS makes cyril a credential custodian** (added 2026-06-16, KAS-1). Driving the KAS engine requires cyril to read kiro's bearer token from its on-disk auth store and hand it to the KAS subprocess via `_kiro/auth/getAccessToken` — a responsibility the v2 engine never imposed (it self-authenticates). This is read-only access to a token cyril doesn't own, with refresh-on-expiry, and it widens cyril's security surface. Acceptable for a local Kiro engine, but a real consideration before KAS is default-on, and it does not generalize to other vendors (each has its own auth — Open Tension #3).
8. **The proxy/conductor stack is deferred in favor of host callbacks** (decided 2026-06-17; see [ADR-0003](adr/0003-defer-proxy-stack-for-host-callbacks.md)). KAS's host-callback model (fs/terminal/hooks — KAS-5/KAS-7) subsumes the *side-effect interception* that originally justified `sacp-proxy` stages (transcript audit, org write/exec policy, path translation), and does it without the `sacp` dependency — partially discharging #4 and #5 for those use cases. Decision: **host-callback support for KAS is the near-term interception path; the `sacp-proxy`/conductor stack waits until KAS is fully implemented.** Conductor's surviving justification is **stable workflow orchestration** (the session-level workflow engine), *not* side-effect interception — revisit post-KAS. Vendor-neutral interception over *in-process* agents (e.g. v2 Kiro, which advertises no callbacks) remains the proxy's irreducible job if/when that becomes a goal.

## Reference / further reading

- [Agent Client Protocol](https://agentclientprotocol.com) — protocol spec
- [agentclientprotocol/registry](https://github.com/agentclientprotocol/registry) — curated agent registry (37+ agents as of 2026-05-03)
- [agentclientprotocol/acpr](https://github.com/agentclientprotocol/acpr) — registry runner CLI
- [agentclientprotocol/symposium-acp](https://github.com/agentclientprotocol/symposium-acp) — `sacp` / `sacp-conductor` / `sacp-proxy` / `sacp-tokio`
- [agentclientprotocol/claude-agent-acp](https://github.com/agentclientprotocol/claude-agent-acp) — Claude Code as an ACP server
- [`experiments/conductor-spike/`](../experiments/conductor-spike/README.md) — the 2026-05-03 spike that empirically validated conductor passthrough and produced the binary-isolation findings

## How to update this document

When a phase is completed, mark its status and add a link to the merge commit / PR that finished it. When tensions are resolved or new ones surface, update the Open Tensions section with dated notes. This document supersedes itself; older versions live in git history.
