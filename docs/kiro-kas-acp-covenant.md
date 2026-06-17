# KAS ACP Covenant — authoritative `_kiro/*` wire reference

**Source:** `@kiro/acp-type-covenant` (the types-only "wire covenant" package), shipped inside the extracted KAS bundle at `~/.local/share/kiro-cli/kas/node_modules/@kiro/acp-type-covenant/dist/`. Extracted from v0.3.224 (kiro-cli 2.7.1), 2026-06-16.

**Why this doc exists / how to use it.** This package is the **authoritative contract** for every KAS `_kiro/*` method, notification, and `_meta.kiro` handshake shape — one `.d.ts` per capability. It is a **different package** from `@kiro/agent` (the implementation, whose `dist/tools/`, `dist/services/`, `dist/steering/` `.d.ts` describe internal classes, *not* the wire). **For any KAS wire question, read the covenant first.** Reading only `@kiro/agent` previously produced two wrong hooks conclusions (the enable path and a "server-run" direction that is actually a host-callback) — see [kiro-2.7.1-wire-audit.md](kiro-2.7.1-wire-audit.md). This reference supersedes the hand-reconstructed shapes captured live in that audit's "KAS live wire captures" section.

> Naming: the acp crate cyril uses auto-prefixes `_` outbound / strips inbound, so cyril code refers to these as `kiro/...` while the wire shows `_kiro/...`. Bare-ACP methods (`fs/read_text_file`, `terminal/create`, …) are **not** prefixed.

---

## 1. The complete `_kiro/*` method catalog

Three typed maps in the covenant define the entire surface. Direction is explicit in the type names (`ClientCapabilityTypes` = agent→client; `AgentCapabilityTypes` = client→agent; `AgentNotificationTypes` = agent→client fire-and-forget).

### 1a. Agent → Client **requests** (`client-capabilities/index.d.ts` → `ClientCapabilityTypes`) — the host must implement responders

| Method | Params | Response |
|---|---|---|
| `_kiro/auth/getAccessToken` | `{}` | `GetAccessTokenResponse {accessToken, expiresAt, profileArn?, authMethod?, provider?}` |
| `_kiro/secret/get` | `{key}` | `{value: string\|null}` |
| `_kiro/secret/store` | `{key, value}` | `{success}` |
| `_kiro/secret/delete` | `{key}` | `{success}` |
| `_kiro/openExternalUrl` | `{url}` | `{success}` |
| `_kiro/tool/semantic_rename` | `SemanticRenameCapabilityRequest {path,line,character,oldName,newName}` | `{success, filesChanged, editsApplied, message, fileChanges?}` |
| `_kiro/tool/smart_relocate` | `{sourcePath, destinationPath}` | `{success, message}` |
| `_kiro/tool/get_diagnostics` | `{paths[]}` | `{diagnostics: Record<path,DiagnosticItem[]>, errors, message}` |
| `_kiro/mcp/elicitation` | `{sessionId, toolCallId, elicitation: MCPElicitation}` | `ElicitationResolveResponse {action:'accept'\|'decline'\|'cancel', content?}` |
| `_kiro/userInput` | `UserInputRequest {sessionId, toolCallId, question, options[]}` | `{action:'answered'\|'dismissed', answer?}` |
| `_kiro/hooks/list` | `HooksListParams {trigger, sessionId, toolId?, toolTags?, workspacePaths?}` | `{hooks: AcpContextualHook[]}` |
| `_kiro/hooks/executeHook` | `HookExecuteParams {hookId, hookName, command, sessionId, userPrompt, timeout?, operationId?}` | `{output?, exitCode, cancelled}` |
| `_kiro/hooks/sessionStart` | `{trigger:'sessionStart', sessionId}` | `{results: AcpPrecomputedHookResult[]}` |

Plus the **bare-ACP** host callbacks (negotiated via standard `clientCapabilities.fs`/`.terminal`, not `_meta.kiro`): `fs/read_text_file`, `fs/write_text_file`, `terminal/{create,output,wait_for_exit,release,kill}`; and the **`_kiro/`-prefixed** fs supersets `_kiro/fs/{read_file,write_file,delete,stat,read_directory}` + `_kiro/terminal/shell_type`. See §5.

### 1b. Client → Agent **requests** (`agent-capabilities/index.d.ts` → `AgentCapabilityTypes`) — what cyril can call KAS to do

| Method | Params | Response |
|---|---|---|
| `_kiro/session/delete` | `{sessionId}` | `{success}` |
| `_kiro/session/rename` | `{sessionId, title}` | `{success}` |
| `_kiro/session/context` | `ContextParams` (subcommand show/add/remove/clear) | `ContextResponse` |
| `_kiro/session/compact` | `{sessionId}` | `{success}` |
| `_kiro/session/export` | `{sessionId}` | `{success, filePath?, error?}` |
| `_kiro/session/history` | `{sessionId, beforeMessageId, limit?}` | `{updates: SessionUpdate[], hasMore, oldestLoadedMessageId?}` |
| `_kiro/checkpoint/revert` | `RevertCheckpointRequest {sessionId, snapshotUri, filePath, toolCallId?}` | `{success}` |
| `_kiro/checkpoint/revertMultiple` | `{sessionId, messageId}` | `{success, affectedFiles, totalFiles, error?}` |
| `_kiro/mcp/resetServer` | `{serverName, startOAuth?}` | `{success}` |
| `_kiro/mcp/getPrompt` | `{serverName, promptName, arguments?}` | `MCPGetPromptResponse` |
| `_kiro/mcp/getResource` | `{serverName, uri}` | `MCPGetResourceResponse` |
| `_kiro/hooks/triggerHook` | `HookTriggerParams` (runCommand\|askAgent) | `{success, code?, error?}` |
| `_kiro/hooks/list` | `HooksListParams` | `{hooks[]}` (bidirectional — also in 1a) |
| `_kiro/spec/invoke` | `SpecInvokeRequest` (executeTask/runAllTasks/generateDocument/analyzeRequirements/createSpec) | `{sessionId, executionId?}` |
| `_kiro/spec/resolveSession` | `{featureName?, strategy:'fresh'\|'reuse', workspacePaths}` | `{sessionId}` |
| `_kiro/spec/getTaskStatuses` | `{tasksFilePath, featureName, workspacePaths}` | `{tasks: SpecTaskStatusItem[]}` |
| `_kiro/permissions/explain` | `{capability?, resource, toolId?}` | `{capability, resource, effect, isExplicitAsk, matchedRule?, scope?, source?}` |
| `_kiro/permissions/list` | `{scope?}` | `{rules: PermissionsListRule[]}` |
| `_kiro/policy/check` | `{capability, paths?, command?, toolId?}` | `{outcome:'allow'\|'deny', reason?}` |
| `_kiro/account/getUsage` | `{}` | `GetUsageResponse` (§8) |
| `_kiro/codeIntelligence` | `{sessionId, subcommand:'status'\|'init'\|'overview'}` | `CodeIntelligenceResponse` |

### 1c. Agent → Client **notifications** (`capabilities/notifications.d.ts` → `AgentNotificationTypes`) — the converter must dispatch all of these

`_kiro/code_references`, `_kiro/mcp/status`, `_kiro/sandbox/status`, `_kiro/mcp/governance_disabled`, `_kiro/policy/changed`, `_kiro/policy/error`, `_kiro/powers/items_changed`, `_kiro/hooks/cancel`, `_kiro/hooks/didChange`, `_kiro/spec/taskStatusChanged`, `_kiro/steering/documents_changed`, `_kiro/progressive_context/items_changed`, `_kiro/customAgent/not_found`, `_kiro/customAgent/config_error`, `_kiro/error/rate_limit`, `_kiro/governance/state`, `_kiro/system/notify`, `_kiro/tools/didChange`.

Standalone client-capability method literals (backing or adjacent to the maps): `_kiro/steering/get_documents`, `_kiro/workspace/active_file`, `_kiro/workspace/currently_open_files`, `_kiro/tasks/get_metadata`, `_kiro/tasks/list`.

---

## 2. `KiroClientMeta` — the `initialize` handshake advertisement surface

`clientCapabilities._meta.kiro` (`client-capabilities/index.d.ts`). This is the authoritative opt-in surface — cyril sets these to turn KAS features on:

| Flag | Type | Gates |
|---|---|---|
| `secretStorage` | `boolean` | `_kiro/secret/*` |
| `openExternalUrl` | `boolean` | `_kiro/openExternalUrl` |
| `clientToolSemanticRename` | `boolean` | `_kiro/tool/semantic_rename` |
| `clientToolSmartRelocate` | `boolean` | `_kiro/tool/smart_relocate` |
| `clientToolGetDiagnostics` | `boolean` | `_kiro/tool/get_diagnostics` |
| `hooks` | `{ enabled: true }` | `_kiro/hooks/*` — **note: `{enabled:true}`, no `v2`** |
| `knowledge` | `boolean` | knowledge base |
| `specLinks` | `boolean` (def false) | render `kiro-spec://` links |
| `requirementsAnalysis` | `boolean` (def false) | interactive requirements analysis |
| `backgroundProcesses` | `boolean` | `control_process`; IDE-only |
| `userInput` | `boolean` (def false) | `_kiro/userInput` rich input (else legacy requestPermission) |
| `telemetryEnabled` | `boolean` | telemetry opt-out |
| `telemetry` | `{machineId, userId, version, kiroClientVersion, channel, userCohort?, isContentCollectionOptIn?}` | telemetry identity |
| `settings` | `AgentSettings` (§3) | per-session feature flags / experiments |

`fs` and `terminal` are **not** here — they go through standard ACP `clientCapabilities.fs`/`.terminal`. `_kiro/auth/getAccessToken` has no flag (always callable; KAS needs it to authenticate).

---

## 3. `AgentSettings` — `_meta.kiro.settings` feature flags (`settings/index.d.ts`)

Every key optional; each is `{enabled: boolean}` unless noted. `_`-prefixed = internal/experimental.

`_parallelTasks`, `_steeringReminders`, `_sessionRecap`, `_mergeVibeSpec`, `_requirementAnalyzer`, `_c2s` (the c2s_* code-to-spec tools), `_quickSpec`, `_subagent`, `_delegate`, `_providerPowers`, `thinking`, `tangentMode`, `disableAutoCompaction`, `codeIntelligence`, `subagentOrchestration` (true → `orchestrate_subagent`; false/absent → `invoke_sub_agent`), `todoList`, `checkpoint`, `semanticReview` (**default ON** if absent), `fta` (**default OFF**), `toolSearch: {enabled, minPct?, minTokens?, neverDefer?}`, `knowledge: {enabled, includePatterns?, excludePatterns?, maxFiles?, chunkSize?, chunkOverlap?, indexType?}`, `compaction: {enabled, excludePercent?, excludeMessages?}`.

> Hooks is **not** an `AgentSettings` key — it's the top-level `KiroClientMeta.hooks` flag (§2). This is the location trap that cost two wrong attempts.

---

## 4. `session_info_update` — the turn-lifecycle / metadata multiplexer (`session/session-info-update.d.ts`)

One `session/update` variant carries everything v2 split across `kiro.dev/metadata` + the prompt response. `_meta.kiro` is `KiroSessionInfoUpdate`, a **discriminated union on `kind`** — **18 variants** (the live probe only happened to observe 6):

| `kind` | Payload |
|---|---|
| `turn_start` | — |
| `turn_end` | `{stopReason: string}` ← **turn completion signal** |
| `turn_completion` | `{promptTurnSummaries: UsageSummaryEntry[], elapsedTime, status}` ← **metering** (§8) |
| `context_usage` | `{usagePercentage, breakdown?: ContextUsageBreakdown}` |
| `user_message_id_assigned` | `{userMessageId}` |
| `focus_update` | `{title?, description?, status?: 'in_progress'\|'waiting_on_user'\|'completed'\|'idle'}` |
| `steering_inclusion` | `{steeringDocuments: string[], agentSubtaskId?}` |
| `display_error` | `{message, errorType}` |
| `summarization_started` / `summarization_separator` | — |
| `summarization_completed` | `{conversationSummary, truncated}` |
| `summarization_failed` | `{reason: 'error'\|'canceled'}` |
| `summary_message` | `{content}` |
| `recap` | `{text}` |
| `queued` | `{activeSessionId}` |
| `hook_update` | `{hook: HookUpdateMeta}` |
| `pending_interaction` | `{toolCallId, interactionType, question, options[]}` |
| `interaction_resolved` | `{toolCallId, outcome, selectedOption?}` |

`ContextUsageBreakdown` = `{contextFiles, tools, kiroResponses, yourPrompts, sessionFiles}`, each `{percent, tokens, items?: {name, tokens, matched, percent}[]}`.

---

## 5. Host-callback contracts: auth / fs / terminal (KAS-1 / KAS-5)

`GetAccessTokenResponse {accessToken, expiresAt (≥ now+3min or rejected), profileArn?, authMethod?, provider?}`.

**Method-name split** (the host dispatcher must handle both forms): base read/write + all terminal lifecycle use **bare ACP**; Kiro supersets/extras use **`_kiro/`**.

| Method | Params → Response |
|---|---|
| `fs/read_text_file` (bare) | ACP `ReadTextFileRequest` → `ReadTextFileResponse` |
| `fs/write_text_file` (bare) | ACP `WriteTextFileRequest` → `WriteTextFileResponse` |
| `_kiro/fs/read_file` | (Kiro superset of read_text_file) |
| `_kiro/fs/write_file` | + optional `_meta.kiro.range {start?,end?: {line,character}}` |
| `_kiro/fs/delete` | `{path, recursive?}` → `{}` |
| `_kiro/fs/stat` | `{path}` → `{type: FileType, size}` |
| `_kiro/fs/read_directory` | `{path}` → `{entries: DirectoryEntry[]}` |
| `terminal/{create,output,wait_for_exit,release,kill}` (bare) | ACP terminal lifecycle |
| `_kiro/terminal/shell_type` | `{}` → `{shellType: string}` |

All carry `BaseCapabilityRequest {sessionId, _meta?}` / `BaseCapabilityResponse {message?, _meta?}` (`capabilities/common.d.ts`).

---

## 6. Client-injected custom agents (`session/types.d.ts`)

`session/new` accepts `_meta.kiro` = `KiroNewSessionRequestMeta {introspectArtifactsPath?, customAgents?: ClientCustomAgent[]}`. This is the native skill/agent-injection hook (highest-precedence agent source). `ClientCustomAgent`:

```ts
{ id, description?, prompt, tools?: '*'|string[], excludedTools?, model?,
  includeMcpJson?, includePowers?, mcpServers?: Record<string, ClientAgentMcpServer>,
  resources?: string[], permissions?: { rules: ClientAgentPermissionRule[] }, welcomeMessage? }
```
`ClientAgentPermissionRule {capability, match?, exclude?, effect: 'allow'|'deny'|'ask'}`.

---

## 7. Trust v2 — the permission model on the wire (`capabilities/trust/types.d.ts`)

Layered onto standard ACP `requestPermission` / `selectedPermissionOutcome` / `tool_call_update` via `_meta.kiro`:

- `tool_call_update._meta.kiro.policyDenial: PolicyDenialInfo {capability, resource, triggeringResource?, effect:'deny', matchedRule: PolicyRuleInfo, scope, source}` — surfaces a policy denial.
- `requestPermission._meta.kiro`: `KiroPermissionRequestMeta {toolId, command?, agentManagesTrust?, consent?: ConsentRequestContext}` where `ConsentRequestContext {capability, resource, askType:'explicit'|'implicit', matchedRule?, scope?, source?, workspaceRoot?}`.
- `selectedPermissionOutcome._meta.kiro`: `KiroPermissionOutcomeMeta {consent?: ClientConsentContext {scope:'invocation'|'session'|'user'|'workspace', resource?, workspaceRoot?}, editedCommand?}` — lets the client persist a consent scope and supply a user-edited replacement command.

`PolicyScope = 'kiro'|'administration'|'user'|'workspace'|'agent'|'session'`. The agent-side evaluation path is exposed to clients via `_kiro/policy/check`, `_kiro/permissions/{list,explain}` (§1b).

---

## 8. Usage / credits (`capabilities/usage/types.d.ts`, `metering/index.d.ts`)

`_kiro/account/getUsage {}` → `GetUsageResponse {success, message, data?: UsageData}`:
```ts
UsageData { planName, billingCycleReset, overagesEnabled, isEnterprise,
  usageBreakdowns: UsageBreakdownEntry[], bonusCredits: BonusCreditEntry[] }
UsageBreakdownEntry { resourceType, displayName, used, limit, percentage,
  currentOverages, overageRate, overageCharges: number|undefined, currency }
BonusCreditEntry { name, used, total, daysUntilExpiry }
```
`UsageSummaryEntry {usedTools?, usage?: number /*credits*/, unit?, unitPlural?}` — the per-invocation credit primitive embedded in `turn_completion` (§4).

---

## 9. Governance, MCP, sandbox, tools, slash-commands, message payloads (selected)

- **`_kiro/governance/state`** `{sessionId, isEnterprise, features: {mcpEnabled, webToolsEnabled, usageAnalytics, contentCollection, promptLogging, codeReferenceTracker, autonomousAgents}, disabledReason?}` — org-policy feature flags; cyril should gate UI on these.
- **`_kiro/mcp/status`** `{sessionId, servers: MCPServerStatus[], registryServers?, accessMode?, ...}`; `MCPServerStatus` is a `status`-discriminated union (`connecting|connected|failed|disabled`) with per-server tools/prompts/resources.
- **`_kiro/sandbox/status`** `{status:'active'|'disabled'|'unavailable', backend?, networkMode?, mcpSandboxed?, reason?}` — `unavailable` = commands run **without** isolation (security warning).
- **Tool advertisement** (`capabilities/tools/list.d.ts`): `_kiro/tools/didChange {sessionId, tags: SessionToolTag[]}`; `SessionToolTag {source:'builtin'|'mcp', tag, description}` — built-ins are 4 category tags (read/write/shell/web), MCP is one `@server/tool` tag each. No granular built-in ids on the wire.
- **Slash commands** (`slash-commands/types.d.ts`): ACP `AvailableCommand` + `_meta.kiro: {type:'steering'|'custom-agent'|'skill', originalName?}`.
- **Persisted/streamed messages** (`session/schemas/index.d.ts`): `MessagePayload` is a 22-variant union. `ToolKind = read|edit|execute|search|delete|move|fetch|think|switch_mode|other`; `ToolCallStatus = pending|awaiting_approval|approved|denied|executing|completed|failed` (7 states). `SessionMetadata` carries `modelId, agentMode, autopilot, effortLevel, semanticReviewEnabled, ftaEnabled, status, ...`.

---

## 10. Cyril implications (deltas vs cyril's current model)

- **Engine-select + the converter arm (KAS-2)** must dispatch the full §1c notification set and the §4 `session_info_update` 18-kind union — not just message/tool chunks. Turn-end = `kind:"turn_end"`; metering = `kind:"turn_completion"`.
- **Host-callback responders (KAS-1/KAS-5):** auth (§5), fs + terminal (§5), and — newly clarified — **hooks** (`_kiro/hooks/{list,executeHook,sessionStart}`, §1a) and optionally `userInput`, `secret/*`, `openExternalUrl`, the client-tool callbacks. Each is gated by a `KiroClientMeta` flag (§2). These are the real proxy-stage interception points.
  - **Hooks verified end-to-end (2026-06-16, `experiments/conductor-spike/probe-kas-hooks-host-2.7.1.py`).** With `_meta.kiro.hooks={enabled:true}`, one shell-tool turn drove `_kiro/hooks/list` at four ordered triggers (`promptSubmit`→`preToolUse`→`postToolUse`→`agentStop`; preToolUse/postToolUse carry `toolId`+`toolTags`), then `_kiro/hooks/executeHook` per returned runCommand hook (host runs it). `userPrompt` per trigger: promptSubmit=prompt text, preToolUse=JSON tool args, postToolUse=JSON `{toolName,toolArgs,toolResult,toolSuccess}`, agentStop=empty. **A `preToolUse` `executeHook` returning `{exitCode:2, output:"DENY…"}` BLOCKS the tool** (no postToolUse fires) and the agent surfaces the host's `output` as the denial reason — the wire mechanism for an org write/exec-policy stage.
- **Type-coverage:** cyril's `ToolCallStatus` (4 states) vs KAS's 7 (adds `awaiting_approval`/`approved`/`denied`/`executing`); cyril folds `ToolKind` delete/move into Write while KAS keeps them distinct. cyril has no type for the §6 `ClientCustomAgent`, §3 `AgentSettings`, §7 Trust-v2 `_meta.kiro` permission extensions, or the §8 `UsageData` — all needed for the KAS engine.
- **Client→agent surface cyril gains under KAS (§1b):** real `session/{list,fork,history(paginated),export,compact,delete,rename}`, `context`, `codeIntelligence`, `permissions/explain`, `policy/check`, `account/getUsage`, `checkpoint/revert`, spec/* — far beyond the v2 engine.

**The covenant package is the source of truth; the per-capability `.d.ts` carry every field. This doc is the curated index — open the matching `.d.ts` for exhaustive detail.**
