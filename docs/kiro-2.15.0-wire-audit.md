# kiro-cli 2.15.0 wire audit (2026-07-27, vs 2.14.1)

**Verdict: SAFE for cyril's current v2 path.** The v2 (Rust) ACP surface cyril consumes is
**frozen** at crate-pin, command/tool, and field-path granularity. The KAS engine took a
4-version `@kiro/agent` jump (**0.22.7 → 0.25.17**), but its `_kiro/*` wire-method surface and
the live host-init handshake are **byte-for-byte stable on cyril's path**. All new substance is
either off-wire (v2 host internals), server-internal (KAS plan→execute graph), or gated behind
capabilities cyril does not advertise (infra-safety, cloud/relay).

This audit spans **two releases** since the last one (2.14.1):

- **2.14.2** — built 2026-07-24T03:12Z, `BUILD_HASH=fcd00b62`. Where the KAS jump landed:
  `@kiro/agent` **0.22.7 → 0.25.17**. Had no dedicated wire audit, so its KAS delta is
  characterized here for the first time. v2 changelog: EU-local telemetry endpoint, refusal
  message suggests specific commands, color-leak + kitty-terminal fixes.
- **2.15.0** — built 2026-07-27T00:53Z, `BUILD_HASH=2b94217b`. The KAS bundle (`acp-server.js`)
  is **`cmp`-identical to 2.14.2's** — all KAS change is attributable to 2.14.2. 2.15.0's own
  deltas are v2/tui-side: a new `chat.showThinkingTips` setting, and V3 plan/spec UX.

Baselines: archived 2.14.1 (`~/.local/share/kiro-research/binaries/2.14.1/`, KAS 0.22.7 tree at
`~/.local/share/kiro-cli/kas/2.14.1-7697bd37.../`). Both new binaries archived to
`~/.local/share/kiro-research/binaries/2.14.{2,15.0}/`, tui.js to `.../tui-bundles/`.

## Embedded changelogs

**2.15.0** (2026-07-27):
- Added: [V3] `/spec new` asks what the spec should cover and drafts requirements from the answer
- Added: `chat.showThinkingTips` setting to show/hide the tip below the thinking indicator
- Added: [V3] Plan mode automatically runs the approved plan instead of requiring a manual switch
- Fixed: typed `@prompt` references occasionally sent as plain text instead of loading the prompt
- Fixed: scope the trust prompt's permission-override warning in `--classic` mode to the newly
  trusted tool

**2.14.2** (2026-07-24):
- Changed: telemetry routed to a region-local endpoint for EU users
- Changed: model refusal message now suggests specific commands (`/model`, `/rewind`, `/chat new`)
- Fixed: color codes no longer leak into tool subprocess output when the TUI forces color detection
- Fixed: issues with some kitty-protocol-compatible terminals after exit

## v2 (Rust) — FROZEN at the wire

- **Crate pins unchanged** across 2.14.1/2.14.2/2.15.0: `agent-client-protocol-0.10.4`,
  `sacp-11.0.0`.
- **Live offline A/B** (`probe-v2-surface-ab-2.11.0.py`, HOME-isolated, `KIRO_TEST_SESSIONS_DIR`):
  init + session/new + all settle notifications, 2.14.1 vs 2.15.0 →
  **24 commands / 14 tools identical, zero field-path delta** across the 5 message types that
  reach cyril (`R:initialize`, `R:session/new`, `_kiro.dev/commands/available`,
  `_kiro.dev/metadata`, `_kiro.dev/subagent/list_update`).
  - Commands (24): agent chat clear code compact context effort feedback goal guide help hooks
    knowledge mcp model paste plan prompts quit reply rewind stats tools usage
  - Tools (14): code glob goal grep introspect knowledge read shell subagent todo_list use_aws
    web_fetch web_search write
  - `tool_search` still absent (14 not 15) — same backend-axis observation as 2.13.0/2.14.1, not
    a binary change.
- **nm module-path diff** (2.14.1 → 2.15.0, Kiro-internal crates): +14 / −8 modules, all
  **off-wire host internals**, none ACP-facing:
  - Added: `chat_cli::launch::v1`, `chat_cli::telemetry::v1_process_monitor::{V1ProcessMonitor,
    ProcessMonitorState}`, `chat_cli::cli::feed`, `chat_cli::os`, `chat_cli::util::channel`,
    `chat_cli::cli::chat::telemetry_lifecycle::ChatTelemetryLifecycle`,
    `chat_cli::cli::chat::tool_manager::{ManagedMcpClients,McpCleanupTracker}`,
    `chat_cli::cli::chat::tools::use_subagent`, `agent::agent::agent_config::parse`,
    `agent::agent::tui_commands::mcp_types`, `chat_cli::cli::agent::legacy::hooks`,
    `chat_cli::cli::chat::cli::usage::usage_data_provider`.
  - Removed: `chat_cli_v2::agent::acp::orchestration::inbox::InboxStore` (+ its methods),
    `chat_cli::telemetry::core::{Event,TelemetrySender}`, `chat_cli::util::resource_permission`,
    `agent::...::consts::env_var`, `chat_cli::cli::chat::tools::use_aws` (path reshuffle; the
    `use_aws` tool is still advertised), `chat_cli_v2::util::system_info`.
  - **Orchestration refactor (internal):** v2's subagent orchestration dropped `InboxStore` and
    renamed `PermissionStore::can_message`/`check_rate_limit` → `can_interact`. Since the offline
    A/B shows the `subagent` tool and `_kiro.dev/subagent/list_update` are byte-identical, this
    is an internal messaging-model refactor with **no wire impact** — but cyril's subagent
    tracker consumes `list_update`, so note it in case a future release surfaces `can_interact`
    semantics on the wire.

## KAS `@kiro/agent` 0.22.7 → 0.25.17 (landed in 2.14.2; byte-identical in 2.15.0)

A 4-version jump, but the **`_kiro/*` wire surface did not change** and neither did the handshake
cyril sees. The work is server-internal or opt-in.

### Wire surface — unchanged

- **`_kiro/*` method-literal surface: 80 methods, identical** 0.22.7 ↔ 0.25.17 (diff empty).
  `_kiro/spec/*` (`invoke`, `getTaskStatuses`, `resolveSession`, `taskStatusChanged`) and
  `_kiro/config/template` were **already present at 2.14.1** — so 2.15.0's `/spec new` is a
  refinement of the existing spec surface, not a new method.
- **Live host-init A/B** (`probe-kas-hostinit-2.15.0.py`, `--agent-engine kas`, HOME-isolated,
  **no prompt turn → zero credits / zero content collection**), 2.14.1 (0.22.7) vs 2.15.0
  (0.25.17), **completely identical**:
  - `agentCapabilities` identical; `authMethods` identical (`aws-builder-id`,
    `aws-iam-identity-center`).
  - `extensionMethods` (7): `_kiro/knowledge`, `_kiro/codeIntelligence`, `_kiro/session/context`,
    `_kiro/session/compact`, `_kiro/session/export`, `_kiro/session/history`,
    `_kiro/config/template`. `sourceProviders:false`, `executionTargets:[local]`,
    `sessionSources:[local]` — cloud/relay dormant on a local run, as at 2.14.1.
  - `session/new` modes (7): `vibe`, `spec`, `quick-spec`, `bug-fix`, `plan`, `autonomous`,
    `semantic_reviewer` — identical set.
  - `configOptions` (3): `mode`, `autopilot`, `contentCollection` — identical.
  - `session/new._meta` identical, including `semanticReviewEnabled:true`, `ftaEnabled:false`,
    `specPlanEnabled:false`, `specWorkflow:"quick"`, `specSkipClarificationEnabled:true` — all
    of which already existed at 2.14.1.
  - Unsolicited session-start pushes identical set (`_kiro/governance/state`, `_kiro/mcp/status`,
    `_kiro/powers/items_changed`, `_kiro/steering/documents_changed`,
    `_kiro/progressive_context/items_changed`, `_kiro/tools/didChange`, `_kiro/sessions/changed`,
    plus `session/update`). `_kiro/safety/propertiesChanged` did **not** appear — it is gated on
    the client advertising `infrastructureSafety`, which the probe (and cyril) do not.

### What the jump actually built (server-internal / opt-in — off cyril's path)

Module-set diff of the unminified `acp-server.js`: **+31 modules, 0 removed.** Highlights:

- **Plan → Execute orchestration** (`src/graphs/plan-execute-graph.ts`,
  `src/execution/definitions/plan-execute/plan-execute.ts`, `src/tools/switch-to-execution.ts`)
  = the 2.15.0 "[V3] plan mode auto-runs the approved plan" feature. A server-side LangGraph
  state machine: the plan agent calls the internal `switch_to_execution` tool (schema: one
  `plan` string ≤100k), which sets `state.execution.requestedExecutionPlan`; the graph then
  transitions (`EXECUTE_SETUP` node) into a fresh executor context. **Emits zero
  `workflow-progress`/`nodeTree`/DAG events and no bespoke notifications** — the executor's work
  reaches a client as ordinary `session/update` streams. `switch_to_execution` is plan-agent-
  internal, not in the default tool registry. `PLAN_ITERATION_LIMIT=300`.
- **Infra-safety override permission** (`src/acp/acp-safety-override-permission.ts`,
  `acp-permission-utils.ts`). A new permission shape: emits a `tool_call` (`kind:"other"`,
  `_meta.kiro.safetyOverride={kind:"infra-safety",toolName,reason,blockedProperties}`, status
  `pending`), then a standard `session/request_permission` with options **`reject_once` ("Keep
  blocked", leading/safe default)** and `allow_once` ("Allow anyway"); the decision rides a
  `tool_call` update as `_meta.kiro.safetyOverride.decision`. **Gated on
  `kiroMeta.infrastructureSafety === true`** (default false) → cyril, which does not advertise
  it, never receives it. It rides standard tool_call / request_permission — no new
  `session/update` variant, so it cannot hard-fail cyril's strict deserializer.
- **Refusal-message** (`src/acp/refusal-message.ts`) = 2.14.2's "refusal suggests /model,
  /rewind, /chat new," per-client (`kiro-cli`/`kiro-ide`/`kiro-web`).
- **Steering supervisor / prefilter** (`src/steering/steering-supervisor.ts`,
  `steering-prefilter.ts`) = the `KIRO_SUPERVISOR_DEBUG` var found at 2.14.2. Default-off behind
  `steeringSupervisor.enabled`; evaluation-only shadow mode. Costs credits when enabled.
- **Cloud-config buildout** (`src/cloud-config/*`, 17 modules: sync, cache store, BFF/HTTP/inert
  sources, pack-downloads). Dormant on local — `KIRO_CLOUD_CONFIG_ENDPOINT` gives an
  `InertCloudConfigSource` and there is no live HTTP path unless a cloud endpoint is configured.
- **Activity dialect** (`src/session/activity-dialect.ts`) — a key normalizer
  (`field_meta → _meta`, snake→camel) for an "activity" shape; `client-reach.ts` adds
  `SOLE_CLIENT_ID` (AFM multi-client scaffolding). Both internal.

## tui.js — `showThinkingTips` added; workflow DAG parser byte-stable

- Carved + sha-verified against the embedded integrity hash: 2.14.2
  (`6cdd4f22…`, 12,665,691 B), 2.15.0 (`a99b8f76…`, 12,713,413 B).
- New `chat.showThinkingTips` marker present in 2.15.0 (absent 2.14.1) — matches the changelog
  and the new `settings/show-thinking-tips.md` doc node.
- **`_kiro/workflow/*` DAG progress protocol: byte-stable across 2.14.1/2.14.2/2.15.0** — same
  counts for `workflow-progress` (4), `wf-progress-` (1), `run_start`/`node_start` (1),
  `_kiro/workflow` (1), `loop_iteration` (1). Still parser/converter-only, no renderer, no
  emitter. See the workflow section below.

## Workflow ACP route — still no emitter (cyril-6beh)

The user asked whether the anticipation infrastructure around the workflow ACP route has been
built out. **It has not** — the specific `_kiro/workflow/*` DAG progress protocol cyril-6beh
tracks remains a facade with no producer on any engine:

| Engine | `workflow-progress` / `wf-progress` / DAG event names | Verdict |
|---|---|---|
| tui.js 2.15.0 | client parser byte-identical to 2.14.1 | parse-and-drop scaffolding, no renderer |
| KAS 0.25.17 | **0** `workflow-progress`, **0** `wf-progress`, **0** `run_complete`/`node_paused`/`loop_iteration`/`steps_queued` | no emitter |
| v2 Rust 2.15.0 | no `workflow`/DAG symbols (its `orchestration::*` is the existing subagent-crew substrate) | no emitter |

**Important distinction.** KAS *did* ship a concrete orchestration path in this window — the
plan → execute graph (above) — but it is a **different mechanism** from the watched wire route.
It is a server-internal LangGraph state machine that emits normal `session/update` streams and
does **not** touch `_kiro/workflow/*`, `nodeTree`, or the `workflow-progress` notification kind.
The engine's own orchestration vocabulary (`stages`/`repeat`/graph nodes) and this client
protocol's vocabulary (`nodeTree`/node-types/`workflow-progress`) remain two unrelated designs.
Keep cyril-6beh **deferred**: no shipped engine emits the protocol, so there is still nothing to
model against.

Also note the KAS internal "workflow" vocabulary (`workflowType`, `agentWorkflowType`,
`createBugfixWorkflowDefinition`, `createDesignFirstWorkflowDefinition`, etc.) is the
**agent-selection / mode** concept (spec / bug-fix / fast-task / requirements-first /
design-first / verify-first), **not** the DAG progress protocol. Don't conflate the two.

## Doc-manifest — SAFE

Small manifest 86 nodes, large 118 (frozen). Diff vs the committed 2.13.0 baseline (spanning the
2.14.x changes): **no unannounced feature leaked.**
- Added: `features/rate-limit-errors.md` (known since 2.14.1), `settings/show-thinking-tips.md`
  (the new 2.15.0 setting — corresponds to a shipped changelog item).
- Removed: `settings/disable-auto-default-{effort,model}.md` (the /model+/effort session-only
  flip from 2.14.1).
- Validated-date bumps only: `commands/update.md`, `features/model-refusal-alerts.md`,
  `slash-commands/effort.md`, `tools/session-management.md`.

## Methodology / reproducibility

- Binaries downloaded from the versioned S3 origin, `cmp`-verified against the installed
  `~/.local/bin/kiro-cli-chat` (2.15.0 match), archived with BUILD-INFO.
- KAS carved login-free from the embedded `kas-bundle.tar` gzip stream (`carve_kas.py`, offset
  ~7.19 MB); tui.js carved + sha-verified (`carve_tui.py`). KAS `acp-server.js` is unminified —
  plain `diff` is the high-signal lane (module-marker set diff over `// src/...` headers).
- 2.14.2 vs 2.15.0 KAS `cmp`-identical ⇒ all KAS change attributed to 2.14.2.
- Live probes HOME-isolated (`HOME=<tmp>` + real `XDG_DATA_HOME`) per
  `feedback_isolate_kiro_probes_with_home`; `~/.kiro/logs` verified 10 dirs before and after each
  KAS spawn. SocialGitHub login stores its token under `kirocli:social:token` (not
  `kirocli:odic:token`) — same `{access_token, expires_at, profile_arn}` shape.
- No live turn captured, so turn/backend fields (metadata/metering/effort) not re-verified — a
  backend-axis concern, out of scope for a binary-isolation audit.

Artifacts: `experiments/conductor-spike/{probe-kas-hostinit-2.15.0.py, kas-hostinit-2.14.1.jsonl,
kas-hostinit-2.15.0.jsonl, v2-surface-2.14.1.jsonl, v2-surface-2.15.0.jsonl}`;
`docs/kiro-docs-index-2.15.0-*.json`.
