# kiro-cli 2.14.0 + 2.14.1 wire audit (2026-07-23, vs 2.13.0)

**Verdict: SAFE for cyril's current v2 path.** The v2 (Rust) ACP surface is frozen at
symbol *and* field-path granularity across both releases. All substance is either off-wire
(v2 Rust internals) or KAS-side. Two release captures because they landed a day apart:

- **2.14.0** — built 2026-07-22T20:02Z, `BUILD_HASH=75b9a41b`. The heavy release: KAS
  `@kiro/agent` **0.18.2 → 0.22.7** (4-minor jump), telemetry + cloud-session maturation.
- **2.14.1** — built 2026-07-23T04:42Z (hours before this audit), `BUILD_HASH=a190cd43`. A
  v2-only patch: KAS bundle is **byte-identical** to 2.14.0's; the deltas are the `/model`+
  `/effort` session-only flip, `set-current-as-default`, a startup spinner, and an MCP
  `structuredContent` fix.

Baselines: archived 2.13.0 (`~/.local/share/kiro-research/binaries/2.13.0/`); KAS 0.18.2 tree
self-extracted at `~/.local/share/kiro-cli/kas/2.13.0-6b915ae.../`. Both new binaries archived
to `~/.local/share/kiro-research/binaries/2.14.{0,1}/`, tui.js to `.../tui-bundles/`.

## Embedded changelogs

**2.14.1** (2026-07-23):
- Added: `/effort set-current-as-default` — save current effort as the model's default
- Added: startup spinner so the terminal isn't blank while the CLI launches
- Changed: **`/model` and `/effort` selections are now session-only**; use
  `set-current-as-default` to save a default
- Fixed: MCP tool calls failing with "Improperly formed request" when the server returns
  `structuredContent`
- Fixed: terminal modes (focus reporting, bracketed paste) restored on all exit paths
  (`/quit`, `/exit`, double Ctrl-C)

**2.14.0** (2026-07-22):
- Added: [V3] automatic retry on empty/truncated model response streams
- Added: feature tip below the thinking indicator
- Added: [V3] `/upgrade-agent` — migrate V2 agent configs to the universal V2+V3 format
- Changed: `/model`+`/effort` confirmation messages show how to disable auto-save
- Fixed: garbled control keys after Ctrl+Z suspend
- Fixed: [V3] model/effort options refresh after switching to a custom agent
- Fixed: [V3] `/plan` no longer deadlocks when the model asks 2+ clarifying questions
- Fixed: [V3] active agent no longer appears in its own sub-agent delegation list
- Fixed: [V3] supervised-mode turn approvals persist across session reopen
- Fixed: [V3] `web_fetch` collapsed to single retry on transient failure
- Fixed: [V3] raw schema validation errors no longer leak to users on invalid tool input

## v2 (Rust) — frozen at the wire

- **crate pins unchanged**: `agent-client-protocol-0.10.4`, `sacp-11.0.0` across 2.13.0 /
  2.14.0 / 2.14.1.
- **Live A/B surface capture** (offline: `initialize` + `session/new` + `commands/available`;
  the host was logged out, so no turn/backend fields — same methodology as prior local
  captures): **24 commands / 14 tools identical**, **75 structural field paths, zero delta**
  between 2.13.0 and 2.14.1. Artifacts: `scratchpad/v2-surface-{2.13.0,2.14.1}.jsonl`.
  - Commands (24): agent chat clear code compact context effort feedback goal guide help hooks
    knowledge mcp model paste plan prompts quit reply rewind stats tools usage
  - Tools (14): code glob goal grep introspect knowledge read shell subagent todo_list use_aws
    web_fetch web_search write
- **`tool_search` still absent** (14 tools, not 15) — unchanged from the 2.13.0 backend-axis
  observation. Still attributable to a backend/`toolSearch.*`-settings rollout, not a binary
  change. Re-check next audit.

### Off-wire v2 Rust deltas (nm module-path diff; kiro-cli-chat 694.8 → 695.5/695.3 MB)

ACP handler module set unchanged. Kiro-internal deltas, none observed on the ACP wire:

- **`chat_cli_v2::agent::acp::commands::context::{ContextBreakdown, CategoryBreakdown,
  BreakdownItem}`** gain `Serialize` impls (new in 2.14.0) — the v2 `/context` command now
  returns a **structured category breakdown** where it previously returned an aggregate. This
  is the `/context` command-execute *response body*, not a new notification; cyril's
  notification pipeline is unaffected. (Prior state: "v2 only via /context, aggregate only" —
  see [[reference_kiro_context_usage_breakdown]].)
- **`/effort set-current-as-default`** (2.14.1): `chat_cli_v2::database::settings::Settings::
  update<...effort::set_current_as_default...>` replaces the old `Settings::merge` path;
  `SessionManagerHandle::update_setting` removed. This is the session-only flip below.
- **`chat_cli::util::launch_spinner::{start_launch_spinner, SpinnerGuard, spawn_spinner}`**
  (2.14.1) — the startup spinner.
- **MCP `structuredContent` fix** (2.14.1): `structuredContent` string count 7 → 9;
  `agent::agent::mcp::tool_result_to_model_json` added. Relevant to cyril only if it proxies
  MCP tool results — the fix is server→agent side.
- `chat_cli::cli::chat::cli::code::LogEntry` serialize (2.14.0, `/code` panel logging);
  `agent_config::migration::*` continues (the `/upgrade-agent` universal V2+V3 migration
  engine); `tools::mkdir`/`glob`/`McpTool` free-function entries continue shuffling in the
  internal universal-agent crate (never ACP-advertised — the live A/B confirms the advertised
  set is unchanged).

### Behavioral flip: `/model` + `/effort` are session-only on 2.14.1

The ≥2.11.0 "sticky" auto-save-as-default behavior (opt-out keys `chat.disableAutoDefault
{Model,Effort}` added in 2.12.3) is **reversed**. In 2.14.1 a `/model` or `/effort` selection
applies to the current session only; saving a default now requires the explicit
`set-current-as-default` subcommand. Evidence converges: changelog; `disableAutoDefault*`
string count 6 → 4 → **0** across 2.13.0/2.14.0/2.14.1 (settings deleted from code, not just
docs); `set-current-as-default` count 11 → 11 → 15; the two `settings/disable-auto-default-*`
doc-manifest nodes removed in 2.14.1; `Settings::merge` → `Settings::update` in nm.

**Cyril impact:** the standing assumption that cyril's `/model` (`commands/execute {model}`)
rewrites the user-global default no longer holds on 2.14.1 — the selection is session-scoped.
This is arguably better UX (no surprise clobber of the user's global default). Any cyril code
or UX copy assuming persistence, and any handling of `chat.disableAutoDefault*`, is now stale.
If cyril wants to offer "save as default" it must surface the `set-current-as-default`
subcommand. Corrects [[reference_kiro_2_12_3_diff]].

## tui.js (carve verified vs embedded sha; 12,660,250 → 12,660,327 → 12,667,766 bytes)

Archived `kiro-tui-2.14.{0,1}.js` (+`.sha256`). Command registry, settings keys, and stop
reasons otherwise stable except as noted.

### `_kiro/workflow/*` — client-side workflow-progress protocol (NEW, forward-looking)

2.14.1 adds a full **client-side parser + renderer for a DAG workflow-progress protocol**, but
**neither shipped engine emits it**: KAS 0.22.7 has zero `workflow-progress`/`wf-progress`
occurrences, and tui.js registers no client-initiated `_kiro/workflow/start|control` method —
only inbound handling. It is scaffolding for a future workflow-orchestration engine (cloud or a
later KAS). Shape:

- Method prefix `_kiro/workflow/`; notification `kind: "workflow-progress"`; messageId prefix
  `wf-progress-`.
- Run events: `run_start, node_start, node_complete, node_paused, need_input, loop_iteration,
  watch_poll, paused, run_complete, steps_queued` (+ `run_failed→failed`, `run_aborted→aborted`).
- Node types: `step, sequence, repeat, parallel, watch`.
- Node status: `pending, running, paused, completed, failed, aborted, skipped`;
  run status: `running, paused, completed, failed, aborted`.
- Node `completionSignal`: `success, need_input, error`. Nodes carry `nodeTree`/`root`
  (recursive `children`), `inputs`, `artifacts`, `capturedOutputs`, `pauseReason`,
  `additionalDirectories`, `workspacePath`, `parentSessionId`, `branchId`, loop `iteration`.

**Transport (important for cyril):** workflow-progress is **not a new `session/update`
variant** — it piggybacks the existing **`user_message_chunk`**. The TUI's
`convertAcpUpdateToEvent` inspects each `user_message_chunk` for `_meta.kiro.notification.kind
== "workflow-progress"` (or a `_meta.kiro.messageId`/`notifyId` starting `wf-progress-`); when
matched, it JSON-parses the chunk's **text content** as the event envelope
(`{method: "_kiro/workflow/<event>", workflowId, ...}`) and emits a `workflow_progress` UI event
instead of a user message. Consequences for cyril: (a) its `SessionUpdate` deserialization will
**not** hard-fail on these (they're plain `user_message_chunk`s — see the serde-tagged-enum note
in CLAUDE.md); (b) but cyril will **render them as ordinary user messages** unless it adds the
same `_meta.kiro` sniff and routes them to a workflow renderer.

This is a rich orchestration model — loops (`repeat`+`loop_iteration`), fan-out (`parallel`),
polling (`watch`+`watch_poll`), and human-in-the-loop (`need_input`, `node_paused`). It
**overlaps cyril's own session-level-workflow ambition** ([[project_cyril_session_level_workflows]])
and is a strong signal that Kiro is heading toward orchestrated multi-node workflows on the ACP
wire. Watch item — model the notification shape now so cyril can render it when an emitter ships.

### `_kiro/frontendToolCall` client handler REMOVED in 2.14.0

The client-side handler added in **2.13.0** (registered cap `frontendToolCall:true`, declined
every call with `{outcome:"cancelled"}` — the reference implementation `cyril-qc00` was modeled
on) was **withdrawn in 2.14.0**. In 2.13.0 the `openExternalUrl` handler was immediately
followed by the `_kiro/frontendToolCall` `TL()` block; in 2.14.0 it leads straight into
`secrets.json`. String count `frontendToolCall`: 2 → **0** → 0 in tui.js. The **KAS agent side
retains it** (15 refs in 0.22.7, was 19). Since the agent only emits `_kiro/frontendToolCall`
when a client advertises the cap, no shipped client triggers it today. **Impact on cyril-qc00:**
the reference client posture is now "don't advertise the cap at all." cyril declining remains
safe but no longer mirrors any shipped client — the issue can drop in priority; see below.

### Other tui.js deltas

- `/effort set-current-as-default` + `/model set-current-as-default` subcommands; session-only
  model/effort messaging (matches the Rust flip above).
- **SSH / remote detection**: a helper flags a "remote" environment when any of
  `KIRO_FAKE_IS_REMOTE`, `Q_FAKE_IS_REMOTE`, `SSH_CLIENT`, `SSH_CONNECTION`, `SSH_TTY` is set —
  wiring for cloud/remote-session UX (the `KIRO_FAKE_IS_REMOTE` env is a test override).
- Cloud-session UI maturation (all off cyril's v2 default path): token growth in
  `agent_connect` (3→15), `session_create` (3→10), `/sessions` (0→7), `/disconnect` (0→4),
  `/repo` (4→5), `remote_sandbox` (0→3), `question_request` (0→4), `context_breakdown_update`
  (0→4), roster statuses `waiting_on_user`/`in_progress`/`provision_failed`. Consistent with
  the telemetry below.

## KAS `@kiro/agent` 0.18.2 → 0.22.7 (landed in 2.14.0; 2.14.1 KAS byte-identical)

`acp-server.js` is **byte-identical between 2.14.0 and 2.14.1** — the entire KAS jump is a
2.14.0 event. Carved login-free from the embedded `kas-bundle.tar` gzip (offset ~7.19 MB);
549,939,200-byte tar; unminified `acp-server.js` (479,623 lines). Despite the 4-version jump,
the **`_kiro/*` wire-method surface gained only two methods**:

- **`_kiro/config/template`** (NEW) — a **session-less "pre-session compose surface"**: a client
  queries the answering agent's own mode/model registries to build config-options *before*
  creating a session. Classified `localOnly` + `transient` (never forwarded to a relayed
  session, never persisted) and **advertised unconditionally** in `extensionMethods`. Directly
  useful for cyril's **KAS-4 configOptions** track — lets cyril populate mode/model pickers with
  no session in flight.
- **`_kiro/userInput/respond`** (NEW, multiplex layer) — in `multiplex-stream.ts`. A client
  (notably an **AFM observer window**) forwards a user's answer to a pending `_kiro/userInput`
  prompt by `toolCallId` instead of a raw JSON-RPC response (it doesn't know the agent's request
  id). Params `{toolCallId, action: 'answered'|'dismissed', answer?}`; a bare `{toolCallId}` is
  rejected `-32602`. Acked fire-and-forget `{success:true}`. **AFM** is a net-new token
  (0→3) — an observer/standalone multiplexing mode where a session-less client browses/forwards.

**Host-init leg (live, `acp --agent-engine kas`, logged out):** the advertised capability set
is identical to 2.13.0 except `extensionMethods` gains `_kiro/config/template`. All cloud gating
is dormant on a local run: `executionTargets:[local]`, `sessionSources:[local]`,
`sessionListScopes:[workspace]`, `sourceProviders:false`. Our fresh 2.13.0 re-capture is
byte-identical to the committed `logs/kas-host-init-2.13.0-20260716.json` baseline (probe
reproducibility confirmed). Capabilities `checkpoints`, `sessionList`, `policyNotifications`,
`fork{messageId}`, `loadSession`, `mcpCapabilities{http,sse}` unchanged.

### KAS internals of note (directly verified)

- **AFM observer/standalone mode** (net-new). A `MultiplexStream` fans **multiple concurrent
  WebSocket clients into one `KiroAgent`**, with a new `ClientRole = 'primary' | 'observer'`
  (`multiplex-stream.d.ts:54`). A *primary* answers agent-initiated requests via normal JSON-RPC
  responses and receives fs/terminal callbacks; an *observer* gets a synchronous protocol-level
  ack, has its raw responses discarded by the mux, and resolves prompts asynchronously by
  `toolCallId` (`_kiro/permission/respond` pre-existing; `_kiro/userInput/respond` new). Default
  role is **observer** for backward compat with "standalone WS server scenarios where all
  connections are external portals." The "AFM window" is such an observer, browsing sessions
  without creating one (which is why a new `refreshGovernance()` was added — a fresh agent
  serving only a session-less observer would otherwise never resolve governance). The acronym is
  **never expanded** in the bundle (0→3 comment-only occurrences); "Agent Frontend Multiplexer"
  is plausible but unconfirmed. **This mux is the WebSocket transport — the stdio single-client
  path cyril spawns is unaffected by it.**
- **Ext-method persistence classification** is now explicit: KAS tags every `_kiro/*` method as
  `transient` / `sessionForwarded` / `localOnly` / `localOnlyUntilScoped`, with a hard
  `assertExtMethodClassified` that throws if a method is unclassified — a good enumeration of the
  full KAS ext surface (knowledge, codeIntelligence, session/{context,compact,export,history},
  hooks/{list,setEnabled,triggerHook}, permissions/{list,explain}, policy/check, mcp/{toggle,
  reset,getPrompt,getResource}, powers/refresh, safety/getProperties, sandbox/applyConfig,
  account/getUsage, sourceProviders/{list,listResources}, spec/{invoke,resolveSession,
  getTaskStatuses}, session/{list,delete}, config/template).
- **Cloud gating model** (dormant locally): capability advertisement is gated by
  `relayConfigured` (→ `executionTargets:["local","cloud-sandbox"]`), `remoteConfigured` (→
  `sessionSources:["local","remote"]`, `sessionListScopes:["workspace","user"]`), and
  `providersConfigured` (→ `sourceProviders` + the two `sourceProviders/*` methods). None active
  on a local logged-out run, so the wire stays local-only.

- **Cloud/relay execution is now WIRED, not just measured** — the real substance of the KAS jump.
  0.22.7 swaps the fat `@amzn/kiro-web-portal-service-typescript-client` (~100 Smithy commands:
  threads, billing, automations, learnings, secrets…) for a slim
  **`@amzn/kiro-web-portal-service-bearer-typescript-client`** (`KiroWebBearerService`) with ~30
  agent-plane commands including a **new `SendAcpMessage` op** (ext methods/notifications forward
  over it as `relayed.acp_message.forward`), `StreamSendMessage`, `LoadSession`, `CancelSession`,
  `RespondToPermission`. Sessions now carry residency `kind: "relayed" | local`; a relayed
  prompt goes to the sandbox via `remoteAgent.submitPrompt` (renamed from 0.18.2's `submitTurn`)
  and a per-session durable **"downlink"** pump projects `content/callback/turnEnded/
  historyComplete` frames to subscribed clients; `session/load` returns once the downlink opens;
  new `TurnWaiter` parks handlers on turn boundaries. Comment: "a relayed session is
  indistinguishable from a local one to the client." Refusals surface as
  `RemoteSessionUnsupportedError`. So the cloud stack moved from **dormant scaffolding (2.12.3) →
  wired-but-gated (2.14.0)**; it is still inert on a local/logged-out run, but the execution
  plumbing now exists. Notably `RespondToFrontendToolCall` did **not** survive the BFF swap —
  reinforcing the client-side `frontendToolCall` withdrawal above. Residency routing lives in a
  new `src/acp/ext-method-routing.ts` (the transient/localOnly/sessionForwarded/
  localOnlyUntilScoped table above).

- **Unsolicited safety/governance pushes at session start** (client-relevant, but gated). New
  `bootstrapSafetyProperties` fire-and-forgets a `_kiro/safety/propertiesChanged` push at *both*
  `session/new` and `session/load` (gated on the client advertising the safety cap +
  monitor/enforce flag); a `_kiro/governance/state` push goes to every WS client at connect with
  an **empty `sessionId: ""`**. cyril won't receive these unless it advertises `infrastructureSafety`,
  but the pattern to remember is: **KAS can push session-less/unsolicited state at session start.**
  Paired with a new KAS-side infra-deploy detector (classifies `cloudformation/cdk/sam/terraform/
  tofu/pulumi/kubectl/helm…` bash invocations into interactive/alwaysAuto/bypassed by flags) — the
  KAS analogue of the v2 Rust permission detector.

- **Stream-recovery** (off-wire, matches the 2.14.0 "[V3] automatic retry" changelog item): a
  shared-budget retry loop silently re-issues empty (`EMPTY_RESPONSE_RETRY`) and truncation-
  suspected (`TRUNCATED_RESPONSE_RETRY`) model streams, plus a Q-multiblock reasoningContent
  strip-and-retry; on exhaustion the turn fails with `NoResponseError` instead of silently ending.

- **Frozen** (occurrence-count-confirmed): no new tools, subagents, agent definitions, modes, or
  `session_info_update` variants; `buildConfigOptions` identical; checkpoints/`kiro-snapshot-v2`,
  steering, `searchMemories`/`memoryEnable`, KiroClientMeta inbound flags — all flat.

## Telemetry metric catalog (2.14.0-specific; 2.14.1 byte-identical here)

+2 metrics, both cloud-agent, both live-wired (not dormant spec):

- **`kiro_cli_cloud_session_ready_seconds`** (histogram, `[engine]`, buckets
  `[1,2,5,10,20,30,60,120,300]s`) — time-to-ready for cloud/remote session provisioning.
- **`kiro_cli_cloud_repo_attach_total`** (counter, `[repo_attach_event, repo_count_bucket,
  engine]`) — a cloud-session repo-attachment flow (`opened`→`submitted`, repo-count buckets
  `none/1/2/3_5/6_plus`), matching the dormant GITHUB/GITLAB/MIDWAY `sourceProviders`.
- `cloud_event` allowed-values gain `ready` + `provision_failed` (completes the cloud-session
  lifecycle state machine on the existing `kiro_cli_cloud_session_total`).

These metrics measure the cloud/relay execution path that 0.22.7 **wired** (see the KAS section)
— the stack progressed from dormant-scaffolding (2.12.3) to wired-and-instrumented (2.14.0),
still gated off on a local run and with zero doc-manifest footprint, but no longer just a spec.

## Doc-manifest delta — SAFE

No new methods/tools/agent-types leaked. Small manifest: 86 → 87 → 85 nodes; large 118-node
manifest frozen. Only meaningful add: `features/rate-limit-errors.md` (`category:feature`,
2.14.0) documenting throttle/overload/monthly-limit error surfacing — mechanism is **not new**
(v2 `RateLimitErrorNotification` ACP ext since ≥2.12.3; KAS `rate_limit_error` stream event).
The two `settings/disable-auto-default-*` nodes were removed and `/model`//`effort`//
`default-model` nodes had `"sticky"`→`"set-current-as-default"` keyword swaps (the session-only
flip). `/upgrade-agent` + "universal V2+V3 agent format" (the most net-new *capability* in this
window) has no manifest node — track it via the KAS/agent-schema axis.

## Actionable items for cyril

1. **`cyril-qc00` (frontendToolCall, KAS-2, open):** the v2 TUI reference client dropped the cap
   in 2.14.0. Recommended posture: cyril does not advertise `frontendToolCall` (matches every
   shipped client); if it ever does, decline `{outcome:"cancelled"}`. Priority can drop — no
   shipped client exercises it. Agent side is retained, so it can resurface.
2. **`_kiro/config/template` → KAS-4 configOptions:** new session-less way to fetch mode/model
   registries for pre-session pickers. Fold into the configOptions track.
3. **`_kiro/workflow/*` client protocol (watch):** model the `workflow-progress` DAG
   notification now; convergence signal with [[project_cyril_session_level_workflows]]. No
   emitter ships yet — do not build rendering until one does, but capture the shape.
4. **Memory correction:** `/model`+`/effort` are session-only on 2.14.1 — cyril's `/model` no
   longer clobbers the user-global default.
5. **Rate-limit/refusal ext coverage — verified good, no action.** The `rate-limit-errors.md`
   doc node prompted a check: cyril already maps **both** dialects' rate-limit wire methods
   (`_kiro.dev/error/rate_limit` v2 and `_kiro/error/rate_limit` KAS) to
   `Notification::RateLimited` in `convert/kiro.rs:647`, with regression tests in `kiro.rs` and
   `engine.rs`. `StopReason::Refusal` is likewise handled. Not a `cyril-h8zb`-class drop.

## Methodology notes

- KAS carved login-free from `kas-bundle.tar` gzip (`scratchpad/carve_kas.py`); tui.js carved
  and sha-verified against the embedded integrity hash (`scratchpad/carve_tui.py`).
- 2.14.0 vs 2.14.1 KAS `acp-server.js` `cmp`-identical → attribute all KAS change to 2.14.0.
- Host was **logged out** — live captures cover the offline surface (init/session-new/
  commands-available, KAS host-init handshake) only; turn/backend-emitted fields (metadata,
  metering, `/context` response body) were not re-captured this cycle. Those are backend-axis
  anyway.
