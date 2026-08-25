# Kiro CLI 2.19.2 wire audit

**Audited:** 2026-08-25 (release date 2026-08-25). Patch release; binary `BUILD_HASH=80d5aeb3` (built 2026-08-25T02:46Z). Origin tarball (`kirocli-x86_64-linux.tar.xz`, 677 MB, manifest sha `263eb01a…`) verified and byte-identical to the installed binaries; archived at `~/.local/share/kiro-research/binaries/2.19.2/` (+ `BUILD-INFO`, `SHA256SUMS`). tui.js carved from `kiro-cli-chat` and sha-verified against the embedded digest (`kiro-tui-2.19.2.js`, 13,128,610 B, +13 KB). **KAS 0.48.0 → 0.52.1** — four minors riding a patch (bundle 23.13 → 23.33 MB; order-insensitive line diff ≈ +15.5k / −14.6k).

**Cyril verdict: SAFE.** Nothing cyril sends or parses today breaks on either engine. The `_kiro/*` / `session/*` method vocabulary and the `sessionUpdate` kind set are unchanged on KAS; v2 notification field-paths are identical in a same-day A/B. The action is (a) one backend-side behavior change that makes workflow `{{step.output}}` work *today*, (b) KAS finally honoring `disableAutoCompaction`, (c) a v2 dispatcher refactor that fixes a crash and tightens `_message/send`, and (d) a handful of new-but-dark wire fields. Four probes and ten captures committed; three issues filed.

Method note: every "new vs. old" claim below was isolated on one axis — same-day **binary** A/B (2.19.1 archive vs 2.19.2 installed for v2; `KIRO_KAS_SERVER_PATH` pinning KAS 0.48.0 for v3) — per `reference_kiro_wire_audit_methodology`. Two headline findings flipped from "binary" to "backend" only because of that pin.

## 1. Workflows (`_kiro/workflow/*`) — LIVE, both KAS builds same-day

Two-step `wf-coder` recipe (`{{token}}` → `{{s1.output}}-BETA`), gate-off `new` → `invoke` → `run_complete`, on 0.52.1 and on 0.48.0 pinned. Probe: `probe-kas-workflow-live-2.19.2.py`; captures `kas-workflow-live-{0.52.1,0.48.0pin}-2.19.2.jsonl`.

**Event contract: unchanged.** `run_start` → `node_start` (×2, second carries `sessionId`) → `node_complete` → … → `run_complete`; payload key sets per kind identical to the 0.46.1 capture; step sessions titled `«workflow» · «nodeId»`; step `session/list` rows carry `_meta.kiro.workflow{workflowId,workflowName,nodeId,nodePath,type}` + `modeId` + `settings.workflows.enabled=true` (all pre-existing). `_kiro/workflow/list` rows carry `parentSessionId` (also on 0.48.0 — not new).

### `capturedOutput` is NON-EMPTY today — and it is the BACKEND, not the binary

```
0.52.1  node_complete s1 capturedOutput="ALPHA"   s2 capturedOutput="ALPHA-BETA"   ← {{s1.output}} interpolated
0.48.0  node_complete s1 capturedOutput="ALPHA"   s2 capturedOutput="ALPHA-BETA"   ← SAME BUILD that gave "" on 08-20
```

The 2.19.0 audit (§ 14) found `capturedOutput: ""` in every run (0.46.1 live, 0.38.7 pinned, 2.16.x captures). Today both builds capture. The KAS-side code is **byte-identical** across 0.48.0 → 0.52.1: `extractCapturedOutput` (last `role:"bot"` message's `entries[].type:"text"`), the transcript fallback, and the bundled `wf-coder` agent. What changed is the **model's turn shape**: the step session now emits `agent_message_chunk "ALPHA"` → `Send Message` tool (severity `success`) → **a second `agent_message_chunk "ALPHA"`** (distinct `-say` replayId, ~2.4 s later) → `turn_end`. On 08-20 the turn ended on the tool call with no trailing text, so the extractor found nothing. Same probe, same recipe, same `auto` model id — the trailing restatement is model/backend behavior.

Consequences:

- `{{id.output}}` / `{{previous.output}}` are **usable today but model-roulette** — they depend on the model restating after `send_message`. `artifacts` (path registry) remains the only deterministic data-flow path. The `kiro-workflow-authoring` skill was corrected from "always empty" to this nuance.
- **Renderer note (cyril-zd8u):** step streams now show the answer twice. Both chunks are legitimate, separately-replayId'd assistant messages, so a dedup would be wrong; the panel should just expect a post-tool restatement.
- Nothing for `WorkflowTracker` — `capturedOutput` was always modelled as optional text.

### `rootConversationId` — binary-new (0.52.1), workflow-visible

`RootConversationIdSchema = string().min(1).max(128)`; 0 → 97 hits. A session-tree root id stamped on every session record (`session.json`), on step sessions (inherited from the run), on log lines, and — the only client-visible spot — on `_kiro/workflow/new`'s `initialState` and `run_complete.finalState` (absent on 0.48.0-pin, present on 0.52.1). It is deliberately **not** exposed through the public `_meta.kiro` (bundle comment: "not the public `_meta.kiro`, which any ACP client could use to forge tree membership").

`_kiro/workflow/new` parent-root policy, live-probed:

| `parentSessionId` | `host.getSessionRootConversationId` | Result |
|---|---|---|
| live session | `found` | root = parent's root (`sess_…` of the caller) |
| **bogus id** | `absent` | **accepted**; root = the bogus id (`workflow-state.json` persists it) |
| omitted | — | accepted; root = a fresh synthesized uuid |
| unreadable record | `error` | refused: "cannot resolve the conversation root of parent session … Refusing to create the run with a fabricated root; retry once the parent session record is readable, or omit 'parentSessionId'" (not trippable live) |

So the "refuse to fabricate" guard only covers I/O failure; a wrong `parentSessionId` still creates a run with a fabricated root. Cyril's `/workflow new` passes its real session id — no impact.

## 2. `disableAutoCompaction` is WIRED in KAS 0.52.1 (was accept-but-ignore through 0.48.0) — LIVE

`disableAutoCompaction` occurrences 2 → 30. Resolution (`resolveDisableAutoCompaction`): `session/new _meta.kiro.settings.disableAutoCompaction.enabled` → `initialize._meta.kiro.settings.…` → persisted session metadata → `false`. Enforcement lives on the execution path (`state.execution.sessionServices.disableAutoCompaction`), with the user-facing refusal "Context limit reached because the disableAutoCompaction setting is enabled. Compact the conversation manually, then try again."

Live (`probe-kas-surface-2.19.2.py`, second session with the setting): the `session/new` result's `_meta` now carries **`disableAutoCompaction: true`** (new field — 0.48.0's `_meta` has no such key), `session/list` echoes `_meta.kiro.settings.disableAutoCompaction.enabled: true` (also on 0.48.0), and **0.52.1 persists it** to `session.json` (0.48.0 did not — grep of the pinned run's fake HOME found nothing). Enforcement itself (refusing to auto-compact at overflow) was not triggered — it needs a genuine context overflow.

This corrects the 2.19.1 static finding ("KAS accepts-but-ignores all three compaction settings"): `disableAutoCompaction` is now honored per session; `compaction.excludePercent/excludeMessages` are still consumed by nothing (0 further hits). For cyril this is the first per-session compaction knob on KAS — filed as **cyril-6s21** (P4).

## 3. `_meta.kiro.outputTransformation` — plumbed, DARK on the wire; the wire cap predates it

New stamp assembled in `assembleEventFields` and attached to the terminal `tool_call_update` `_meta.kiro` and the persisted `tool_result`:

```
{kind: "offloaded", absFilePath, totalChars}   // ≥ LARGE_OUTPUT_CONFIG.CHAR_THRESHOLD (30,000 chars): full output → file, model gets head/tail 500 preview
{kind: "clipped",   originalChars}
```

Gate: `isFeatureEnabled("largeToolOutputHandler")`, resolved from the **connection-level** settings (`initialize._meta.kiro.settings.largeToolOutputHandler.enabled`) — the same key on `session/new` did **not** activate it. Handled tools: `execute_bash`, `get_process_output`, `web_fetch`, `remote_web_search`, `mcp_*`.

Live, three configurations (`seq 1 8000` ≈ 39 KB; control `seq 1 300`):

| Config | Offload file written | `outputTransformation` on wire |
|---|---|---|
| default | no | no |
| session-level setting | no | no |
| initialize-level setting | **yes** — `<session>/tool-outputs/execute_bash-<id>.txt` (30,076 B) | **no** |

So the stamp is plumbed but never reached the ACP wire in any configuration — the emitter's `event.outputTransformation` was unset on the path the terminal update takes. Treat as dark; re-probe next release.

**Retroactive finding:** the 500/500 preview cyril sees on big tool results is `capToolResultForWire` — present since **KAS 0.46.1 (2.19.0)**, absent in 0.38.7. Terminal `tool_call_update` content ≥ 30,000 chars is rewritten to `head(500) + "\n...[truncated N chars]...\n" + tail(500)` (surrogate-safe), while `rawOutput` on the same frame still carries the full tool-truncated output (`[ExecuteBash] Output truncated {"toChars":30074}` in the KAS log). Cyril renders the marker verbatim today, which is acceptable; a first-class treatment (and the future stamp) is **cyril-7cnh** (P3).

## 4. v2 (Rust) engine — same-day A/B 2.19.1 vs 2.19.2

New modules: `chat_cli_v2::agent::acp::extension_request::{ExtensionRequestKind::{parse,recognize}, ProductionExecutor, respond}` (a typed dispatcher whose string table spells out `_kiro.dev/commands/{execute,options}`, `_kiro.dev/session/{list,terminate}`, `_kiro.dev/settings/{list,set}`, `_session/steer{,/clear}`, `_session/spawn`, `_message/send` — names 2.19.1 built by concatenation, hence 0 static hits) and `chat_cli_v2::telemetry::acp_method::{AcpConnectionContext, AcpMethodTelemetry, RequestObservation, ResolvedRequest}` (per-ACP-method telemetry — cyril's traffic is now observed per method, continuing the 2.19.0 `acp_client` dimension).

Live A/B (`probe-v2-ext-methods-ab-2.19.2.py`, `v2-ext-ab-{2.19.1,2.19.2}-2.19.2.jsonl`):

| Call | 2.19.1 | 2.19.2 |
|---|---|---|
| `_kiro.dev/commands/options`, `settings/list`, `session/list`, `session/terminate`(bogus), `_session/steer/clear`, unknown method | same | same (`-32601` for unknown) |
| `_kiro.dev/settings/set` unknown key | **agent process exits** (exit 0, stdout closes — a crash) | `-32602 Invalid params` "`probe.nonexistent.key` is not a valid setting" |
| `_message/send {sessionId, message}` | `{ok:true}` — even for a bogus session | `-32700 Parse error {error:"missing field `content`", json, phase:"deserialize"}` |
| `_message/send {sessionId, content:"str"}` | `{ok:true}` | real session `{ok:true}`; **bogus session `-32602 "Unknown session id"`** |
| `_message/send content:[blocks]` / `{block}` | `{ok:true}` | `-32700` "invalid type: sequence/map, expected a string" |

So 2.19.1 validated nothing on `_message/send` (silently accepted any body and any target); 2.19.2 requires `content: String` and a known session. **Cyril is compatible** — the bridge sends `{"sessionId", "content": String}` (`bridge.rs:2319`, `subagent.rs:194`) — and `/msg` to a dead target now surfaces an error instead of a silent no-op. Structured `-32700` data (`{error, json, phase}`) is new.

Everything else is quiet: notification field-paths identical (`commands/available` 23 paths, `_kiro.dev/metadata` 6, `agent_message_chunk` 4); watchdog/retry/compaction strings intact (5→4 counts are LTO re-glue); `chat.disableTrustAllConfirmation` present in both; `api.*` settings set unchanged. Module removals: `agent::agent::goal`, v1 `chat_cli::cli::chat::tools::web_fetch`, `chat_cli_v2::database::settings::SettingsData`, `kiro_telemetry::otel::OtelMetricsSink`. New: `agent_client_protocol_schema::tool_call::ToolCallUpdate{,Fields}` instantiated (the "correct file names for Windows / mixed-separator paths" fix), `usage_renderer::{format_limited_usage,format_usage_percentage}` (the `/usage` fix), `prompt::PasteState`, `sacp::role::RemoteStyle`, `kiro_telemetry::identity_epochs`.

## 5. `--output-format stream-json` (non-interactive) — LIVE on v2 and v3

`kiro-cli chat --no-interactive --output-format stream-json [--agent-engine v2|v3] "<prompt>"` emits JSON Lines `{type, data}`; `stream-json` implies `--no-interactive`; v1 refuses (`runError {stage:"engine", message:"…not supported on the v1 engine. Pass --agent-engine v2 (or v3)."}`). Captures: `stream-json-{v2,v3}-2.19.2.jsonl`.

```
runStarted    {payloadSchema:"acp", acpProtocolVersion:1, engine:"v2"|"v3"}
sessionUpdate {sessionId, update:<the ACP session/update payload, verbatim, incl. _meta.kiro>}
metadata      {sessionId, contextUsagePercentage, meteringUsage[], turnDurationMs}     (v2 only — the _kiro.dev/metadata params)
runFinished   {sessionId, status:"success", stopReason:"end_turn", finalText, finalTextTruncated}
runError      {sessionId, stage, message}
```

`_kiro/*` extension notifications are **not** forwarded (v3 run: 13 `sessionUpdate`s, nothing else). Tool permissions: "[denied] tool permission approval is not supported in non-interactive mode. Use --trust-all-tools to auto-approve." Relevance: `payloadSchema:"acp"` makes ACP the canonical event schema across all three Kiro surfaces; for cyril this is a zero-ACP smoke oracle and fixture source, nothing to adopt.

## 6. Spec mode — explicit only; `_kiro/spec/taskStatusChanged` gets a host relay + TUI consumer

- KAS 0.52.1 **removed intent classification** (LLM + local `chat/do/spec` intent, `intentClassificationOverride`, "Intent classification determined spec mode", `agentModeOverride:"spec"`) — spec mode is entered only via the `mode` config option (list unchanged: `vibe, spec, quick-spec, bug-fix, plan, autonomous, semantic_reviewer`). c2s spec generation now writes `properties.md` instead of `behavior.md`.
- `_kiro/spec/taskStatusChanged {sessionId, tasksFilePath, changes}` (emitted by `SpecTaskStatusTracker` when it flips a `- [ ]`/`[x]`/`[-]`/`[~]` checkbox in `tasks.md`) **already existed in 0.48.0**; what is new is the **host relay** (`spec_task_status_changed` stream event in `kiro-cli-chat`) and the **tui.js consumer** (0 → 1). Cyril can subscribe directly — **cyril-mxc7** (P4).

## 7. Capability drift that is BACKEND, not binary (same-day pin)

Versus the 08-21 capture, `initialize.agentCapabilities._meta.kiro` now shows `sessionSources: [local, remote]`, `sessionListScopes: [workspace, user]`, `executionTargets: [local, cloud-sandbox]`, and `extensionMethods` + `_kiro/sourceProviders/{list,listResources}` (21 → 23). **KAS 0.48.0 pinned today advertises the identical list** (stderr: `acp.remote_sessions.enabled {endpoint: app.kiro.dev}`) — a feature-flag rollout between 08-21 and 08-25, not 0.52.1. Likewise the `model` configOption cold-registry race is still visible and version-independent (present cold on the 0.48.0-pin run, absent on the 0.52.1 run, same hour). Turn-end ordering re-verified: `turn_end` → `session/prompt` response, 4–5 ms.

## 8. Other KAS 0.52.1 deltas (static)

- **Feature flags**: `AB_MEMORY_EXTERNAL` (`memory_external_enabled`, env `KIRO_FEATURE_MEMORY_EXTERNAL_ENABLED`, paired with the pre-existing `AB_MEMORY_INTERNAL` — an agent-memory experiment; worth watching against cyril-memory's positioning) and `AB_FTA_VIBE` (`KIRO_FEATURE_FTA_VIBE_ENABLED`; Functional-Task-Alignment validator in vibe mode; new `ftaVibe` setting + persisted `ftaVibeSettingEnabled`; per-turn `ftaVibeArm` attribution). `AgentSettings` key set is 31 = 31 (no new keys).
- **Hooks**: matcher validation at load — `[hooks] Hook has no matcher and will run on every tool call` / `Hook matcher has no effect for this trigger` (logger only, no wire).
- **Interrupted tools**: `recoverInterruptedTool: true` — a retryable transient error mid-tool pairs the interrupted `toolUse` with a retry-instruction failure result so the model re-issues it (bounded `MAX_TRANSIENT_RECOVERIES`); TLS classes `TlsCertificateError`/`TlsProtocolError`, `DROPPED_CONNECTION_ERROR_CODES=[ECONNRESET, EPIPE]` with user-facing messages ("A TLS protocol error interrupted the connection…"). Not on the ACP wire.
- **MCP**: `structuredContent` validation against output schemas, `resolveMcpTimeout` → "MCP server '<name>' connection timed out after <N>ms" (the "no longer waits indefinitely" fix), `updateServersAwaitingReady`, OAuth CIMD (`clientMetadataUrl`, `client_id_metadata_document_supported`, error `invalid-client-metadata-url`), SEP-2243 `Mcp-Param-*` header validation, string→boolean/number coercion counters (`stringToBooleanCoercions`).
- **Images**: `ImageValidationError` (backend `IMAGE_{COUNT,DIMENSION,FORMAT,MIME,SIZE}_*` reasons, user-attributable) + `imageBase64Urls` on tool messages — tool results can now carry images to the model.
- **Host env**: `KIRO_KAS_CONTROL_PLANE_ENDPOINT` → `--control-plane-endpoint=` on the KAS spawn (loopback-only, validated in the host); `KIRO_TUI_TELEMETRY_KEY{,_FILE}` (TUI hands the `acp` child a per-spawn telemetry key file and scrubs the env); new host ext `_kiro.dev/telemetry/identityChanged` (TUI→host, `kiro_telemetry::identity_epochs`).
- **tui.js**: +13 KB; new consumers for `_kiro/spec/taskStatusChanged` and `identityChanged`; content kinds `document`/`image`, `type:"blob"`, `kind:"unclassified"`; **no new slash commands or settings keys**.
- **Doc manifest**: zero drift — 2.19.2 embeds the same 2026-08-17 generation as 2.19.1 (103/138 nodes; only `session-management.md` `validated` flip). No baselines committed.
- Announced items with no wire footprint: subagents inheriting `/tools trust-all` (v2 crew, `trust_all_tools` string re-glue only), streamed-code-fence and escape-sequence sanitizing (TUI), Windows `cmd` fail-closed parsing (`splitOnOperators` single-quote tracking), `grep_search`/`file_search` read-allow and separator fixes (tool-internal).

## 9. Follow-ups filed

- **cyril-7cnh** (P3) — render the KAS tool-result wire cap marker (since 0.46.1) and consume `_meta.kiro.outputTransformation` when it lights up.
- **cyril-mxc7** (P4) — subscribe to `_kiro/spec/taskStatusChanged`.
- **cyril-6s21** (P4) — expose KAS's per-session `disableAutoCompaction` (first honored compaction knob on v3).
- Notes added to **cyril-zd8u** (capturedOutput is model-dependent-non-empty; expect the post-tool restatement in step streams).

## 10. Artifacts

- Probes: `experiments/conductor-spike/probe-kas-surface-2.19.2.py` (surface + `disableAutoCompaction` leg; run twice, once with `KIRO_KAS_SERVER_PATH` pinning 0.48.0), `probe-kas-workflow-live-2.19.2.py` (two-step run + bogus/parentless `workflow/new` legs; run on both builds), `probe-kas-output-transform-{,lth-,lth-init-}2.19.2.py` (default / session-level / initialize-level `largeToolOutputHandler`), `probe-v2-ext-methods-ab-2.19.2.py` (crash-resilient same-day v2 A/B), `probe-v2-message-send-shape-ab-2.19.2.py`, `probe-stream-json-2.19.2.sh`.
- Captures (token-scrubbed): `kas-surface-{2.19.2,0.48.0pin-2.19.2}.jsonl`, `kas-workflow-live-{0.52.1,0.48.0pin}-2.19.2.jsonl`, `kas-output-transform-{,lth-init-}2.19.2.jsonl`, `v2-ext-ab-{2.19.1,2.19.2}-2.19.2.jsonl`, `stream-json-{v2,v3}-2.19.2.jsonl`.
- Archive: binaries + `BUILD-INFO` + `SHA256SUMS` at `~/.local/share/kiro-research/binaries/2.19.2/`; `tui-bundles/kiro-tui-2.19.2.js` (+ `.sha256`); KAS bundles self-extracted at `~/.local/share/kiro-cli/kas/2.19.{1,2}-*/`.
