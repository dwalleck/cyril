# kas-core lane: kiro-cli 3.1.0 audit, KAS 0.66.43 (2.32.0) -> 0.66.91 (3.1.0)

Status: COMPLETE (static). Every row is STATIC; nothing from Kiro was executed. Carves under
`~/.local/share/kiro-research/kas-carves/<ver>/node_modules/@kiro/agent/`. KAS classes diffed in order
0.66.43 (2.32.0) -> 0.66.48 (2.33.0) -> 0.66.58 (2.33.1) -> 0.66.62 (2.34.0-2.34.2) -> 0.66.70 (2.35.0-3.0.0) -> 0.66.91 (3.1.0).
"Counts" are occurrences per bundle in that order (`static-kas-core-ctx-3.1.0.py`), so a row's "first version" is the first
carve where the count becomes non-zero (bisected, trap 2). Sibling lanes: kas-workflow (`_kiro/workflow/*`, step credits,
recipes), tui, host. cyril HEAD for every `file:line`: e3a3868d.

Evidence beside this file: `flags/flags-<ver>.txt` (6 registries), `session-info-kinds.txt`, `settings.txt` (AgentSettings zod
blocks + key deltas), `surface/static-kas-surface.json` + `surface/surface-stdout.txt` (literal-set census, 6 trees) +
`surface/log-ids-{added,removed}-<a>-to-<b>.txt`, `literals-delta.txt` (lexer-exact literal multiset OLD->NEW: 23,826 changed,
19,532 new-only, 591 old-only — **inflated by esbuild keepNames in 0.66.91**: every function is followed by `a(fn,"name")`, so
roughly 17k of the new-only literals are function NAMES, not features; see the addendum). Regenerable bulk in `tmp/bundle/` (old/new .raw/.norm, `norm.diff` 195,886 hunk lines,
`hunks.txt` 23.6 MB; statements 344,285 -> 394,431). New scripts: `static-kas-core-ctx-3.1.0.py` (count in all six carves +
context), `static-kas-core-sets-3.1.0.py` (regex vocabulary set diff across six carves), `static-kas-core-catalog-3.1.0.py`
(configuration catalog rows). Reused read-only: `static-kas-flags-2.29.1.py`, `static-kas-settings-2.29.1.py`,
`static-kas-session-info-kinds-2.29.1.py`, `static-kas-surface-2.29.1.py`, `static-kas-bundle-diff-2.31.1.py`,
`static-kas-hunks-2.31.1.py`, `static-kas-literals-2.31.1.py`.

## Summary

- **For a cyril user on KAS the biggest change is silent: model fallback is ON by default in 0.66.91** (`model_fallback_refusal`,
  `model_fallback_availability`, `model_fallback_auto_refusal` default false -> true). KAS may answer on another model after a
  refusal or outage; the `model_fallback` / `model_routed` kinds that announce it have no arm in cyril (dropped at the
  kind-match fallthrough `crates/cyril-core/src/protocol/convert/kas.rs:822`). cyril-aoby covers the kind; it needs a priority re-read (P2).
- **Credits:** every turn now emits `session_info_update{kind:"session_usage", credits:[{usage,unit,unitPlural}]}` after
  `turn_completion`, and `session/load` replays it; this is the "Session credits" source (3.1.0). cyril drops it and sums
  `turn_completion` itself, so a reopened session restarts at zero (`session.rs:291`).
- **Sign-in:** KAS 0.66.91 refuses `session/prompt` with JSON-RPC -32000 `"Sign in to Kiro to send a prompt."`,
  `data:{errorType:"SignInRequiredError",retryErrorType:"CLIENT_ERROR"}` once the identity tracker has seen the credential
  absent (acp-callback: cyril's `_kiro/auth/getAccessToken` responder failed). The prompt is not persisted. The 2.37.0
  terminal sign-in is host-side (KAS only gained `CredentialAbsentError`, 0.66.48).
- **"Approve for me" (0.66.58, surfaced 2.36.0) is a trust classifier gated by env `KIRO_TRUST_CLASSIFIER_MODE=enforce` at KAS
  spawn plus permission rules with the new `effect:"evaluate"`.** cyril's spawn sets no env -> classifier off, evaluate rules
  are skipped (`policy.reference.classifier_off`). Its wire marks (`_meta.kiro.trustClassifierApproved`, a `trustClassifier`
  flag line, `request_permission._meta.kiro.trustClassifier`) are dropped. The permission option builder itself is unchanged.
- **Native memory rollout flipped on in 0.66.58** (`memory_internal_enabled` disabled -> `all`, `memory_external_enabled`
  false -> true); the agent-file `memory.mode` (default `disabled`) still decides per agent. New `_kiro/memory/{sync,
  getSyncStatus}` requests and `_kiro/memory/{syncStatus,changed}` notifications; sub-agents index the dispatcher's memory.
- **Workspace trust is not a KAS wire change:** the acp-server entry hard-codes `workspaceTrusted:!0` for stdio and ws; the
  prompt and `--trust-workspace` live in the host/TUI. cyril's direct spawn keeps loading workspace hooks/agents/MCP exactly
  as on 2.32.0 (no P1). The untrusted-execution `ask` rules and hook/MCP/LSP gates pre-exist in 0.66.43.
- Method set 123 -> 138 (+15, none removed), kinds 25 -> 29 (+`turn_timing`, `suggested_prompts`, `network_configuration`,
  `session_usage`), flags 30 -> 40, AgentSettings keys 49 -> 61, `KIRO_*` 41 -> 52. Hooks gain `PreCompact`/`PostCompact`
  (KAS-side v2 runtime only; the ACP `_kiro/hooks/list` trigger enum is unchanged). `_kiro/mcp/oauthCompleted` is now sent.

## Deltas

Direction legend: A->C agent-to-client notification, A->C req agent-to-client request, C->A client-to-agent request, setting =
handshake/session `_meta`. cyril verdict per trap 17 with the proving `file:line`.

| item | first version (KAS) | gate / default | direction | STATIC | cyril | proposed |
|---|---|---|---|---|---|---|
| **Model fallback default ON**: registry defaults `model_fallback_refusal` false->true, `model_fallback_availability` false->true, `model_fallback_auto_refusal` (added 0.66.58, false) -> true (`flags/flags-3.1.0.txt`); new log ids `modelFallback.{catalog.refresh_failed,chains.listener_failed,preference.written,refresh.failed}`, `reason:autoRefusalDisabled`, `kind:modelFallback`/`kind:fallback`; `_kiro/configuration/set` can write a per-model fallback chain (`HN.modelOf`, `fallbacks.writeSynced`) | 3.1.0 (0.66.91); auto_refusal flag added 2.33.1 (0.66.58) | default on; env `KIRO_FEATURE_MODEL_FALLBACK_*_ENABLED=false` | A->C kinds `model_fallback`, `model_routed` (pre-existing) | STATIC | DROPPED: no arm in `session_info_to_notification` (match `crates/cyril-core/src/protocol/convert/kas.rs:627`, fallthrough `_ => None` kas.rs:822) | NOTE cyril-aoby (raise to P2: default engine, silent model swap) |
| **`session_usage` kind**: `{kind:"session_usage",credits:[{usage,unit?,unitPlural?}]}` = `sumCreditsByUnit` over every `promptTurnSummaries` so far; sent right after each `turn_completion` (`this.usage.recordTurn(R)` then `sessionInfoEmitter.send(I)`), rebuilt from `usage_summary` transcript rows and replayed on `session/load` (`vJe(Zs(D))`, replay-marked) | 3.1.0 (0.66.91) | always | A->C kind | STATIC | DROPPED (kas.rs:822); cyril sums turns itself (`crates/cyril-core/src/session.rs:291` `set_credit_usage`, `session_cost`) and has no load-time total | NEW P3 or NOTE cyril-0o7e: adopt `session_usage` for load-time totals (TUI consumes it: `kiro-tui-3.1.0.js` `type:"session_usage"`) |
| **Sign-in refusal**: `refuseWhileSignedOut()` runs before persistence on every client prompt (`g||await this.refuseWhileSignedOut()`); when `identityTracker.credentialAbsent` and a recheck says `absent`, throws `SignInRequiredError` (-32000, `"Sign in to Kiro to send a prompt."`, `data:{errorType:"SignInRequiredError",retryErrorType:"CLIENT_ERROR"}`). Identity tracker (0.66.48) logs `auth.identity.{signedIn,signedOut,changed,refreshFailed}`; `CredentialAbsentError` (subclass of TokenInvalidError, -32000) is what the acp-callback provider raises before the host ever returned a token | 3.1.0 (0.66.91); tracker 2.33.0 (0.66.48) | always | error on `session/prompt` | STATIC | generic: a failed bridge command surfaces as `Notification::BridgeError` (`crates/cyril-core/src/types/event.rs:332`, rendered `crates/cyril-ui/src/state.rs:1193`); no sign-in-specific handling; the prompt-RPC error path's `operation` label was not located in `protocol/bridge.rs` (live lead KAS-L3) | NOTE cyril-0rn8 (logged-out kiro-cli makes get-kas-token hang): KAS now refuses instead of hanging; cyril should map `errorType:"SignInRequiredError"` to a sign-in hint |
| **Trust classifier ("Approve for me")**: env `KIRO_TRUST_CLASSIFIER_MODE=enforce\|off` (`resolveClassifierMode`, default off); rule effect `evaluate` added to the agent-file schema `effect:enum["allow","deny","ask","evaluate"]` and to `policyDenial.matchedRule.effect`; with mode off, evaluate rules are filtered (`Cfo`), org policies needing it are skipped (`"it needs the trust classifier, which is off"`, `policy.reference.classifier_off`). Wire: tool_call `_meta.kiro.trustClassifierApproved:true` (persisted `classifierApproved`), a pending `tool_call_update` "flag line" with Text content + `_meta.kiro.trustClassifier:{decision:"prompt",reason}`, and `session/request_permission._meta.kiro.trustClassifier`. Logs `acp.policy-eval.classifier-{allow,block,flagged,prompt,begin-failed,strikes-failed}`. The TUI writes `{capability:"all",effect:"evaluate"}` (`pit`) when toggled and reads the same env to show status | 2.33.1 (0.66.58); surfaced 2.36.0 | env off by default; host binary 3.1.0 carries `KIRO_TRUST_CLASSIFIER_MODE` once (host lane: does `kiro-cli acp` set it?) | setting (env) + `_meta.kiro` on tool_call / request_permission | STATIC | NOT-SENT (cyril's KAS spawn env sets no `KIRO_TRUST_CLASSIFIER_MODE`: `crates/cyril-core/src/protocol/kas/discovery.rs`, no match) and DROPPED (`_meta.kiro` on permission requests is not read beyond `consent`/`rejectionReason`, `crates/cyril-core/src/protocol/convert/kas.rs:67-91`) | NEW P3 "Approve for me on KAS: set `KIRO_TRUST_CLASSIFIER_MODE=enforce` and render `trustClassifierApproved` / flag lines" (opt-in; blocked on host-lane confirmation) |
| **Unparseable shell command**: `PolicySession.unparseable()` (new method) returns `{effect:"ask",isExplicitAsk:false}` when no shell rule denies or asks, otherwise `{effect:"deny",basis:{kind:"unparseable",errors}}`; logs `policy.shell.parse_error`; the request still carries `persistableConsent:false` + `persistableConsentReason` (text unchanged) | 2.34.0 (0.66.62) | always | behaviour + `tool_call_update._meta.kiro.policyDenial` | STATIC | HANDLED as an ordinary ask / a denied tool call (kas.rs:147 option kinds); `policyDenial` DROPPED (no reader) | NOTE cyril-58uv |
| Permission option builder `buildPermissionOptions` unchanged: `accept`(allow_once), `always-accept`(allow_always, only when persistable and not explicit), `reject`(reject_once), `always-reject`(reject_always); the 2.34.0 "Allow once and Deny once first" ordering and the 2.36.0 subcommand tier (`git stash *`) are TUI-composed `match` patterns in the consent echo | — | — | — | STATIC | cyril echoes only the exact resource (kas.rs:67-91) | NOTE cyril-7c0s (tiers are a client composition) |
| **Hooks `PreCompact`/`PostCompact`**: trigger table gains both (+camelCase aliases); runtime `triggers.preCompact/postCompact`; gate `enabled && workspaceTrusted && registry.has(trigger)`; hook env `compaction_id`, `reason`, `message_count`, `transcript_path` (+`summary`, `summary_path` for Post); a blocked Pre hook aborts compaction (`hooks.compaction.blocked`, reasons from `appendix\|\|stderr`); Post output becomes an `agent_note`. The ACP hooks provider (`_kiro/hooks/list {trigger}`) uses the unchanged `mae` enum (sessionStart, preToolUse, postToolUse, promptSubmit, agentStop, pre/postTaskExecution) — compaction never reaches `_kiro/hooks/list` | 3.1.0 (0.66.91) | v2 (file/agent-profile) hooks only | A->C `hook_update` kind + hook env | STATIC | `kas_hooks="kas"`: HANDLED via `hook_update` (kas.rs:670 arm); `kas_hooks="host"`: NOT-REACHED (`crates/cyril-core/src/protocol/kas/hooks.rs:636-645` maps file triggers to the camelCase wire set; no compaction trigger exists on the wire) | NOTE cyril-497j (hook types) |
| `PreToolUse` exit 2 with no reason blocks (3.1.0): v2 executor `blocked` flag + `appendix\|\|stderr` fallback; the ACP `executeHook` reply `{exitCode,output,cancelled}` is unchanged | 3.1.0 (0.66.91) | — | — | STATIC | HANDLED (host mode replies `{exitCode, output}` `hooks.rs:818`) | — |
| **`_kiro/mcp/oauthCompleted {serverName, success, oauthFlowId}`** now SENT (`notifyOAuthCompleted`, `onLazyAuthCompleted`) whenever the client advertised `openExternalUrl`; `_kiro/openExternalUrl` params gain `oauthFlowId` beside `serverName`, `oauthCompletion` | 3.1.0 (0.66.91) (TUI consumed it dark since 2.32.0) | `initialize.clientCapabilities._meta.kiro.openExternalUrl:true` | A->C | STATIC | REACHABLE and DROPPED: cyril advertises openExternalUrl (`crates/cyril-core/src/protocol/engine.rs:143`), parses only `url` (`crates/cyril-core/src/protocol/kas/callbacks.rs:189-195`), and `kiro/mcp/oauthCompleted` falls to `_ => Ok(None)` (`crates/cyril-core/src/protocol/convert/kiro.rs:1305`) | NEW P4 "clear the MCP sign-in notice on `_kiro/mcp/oauthCompleted`" (NOTE cyril-tqr8 closed) |
| **Memory rollout**: `resolveMemoryRolloutValue` returns the internal default when both flags are at default -> `"all"` (was `"disabled"`), so `resolveMemoryEnabled` is true for every channel unless the client sends `settings.userMemoryOptIn:false`; actual policy = `deriveMemoryPolicy(rollout, agentMemory{mode,reflection}, permission)`: all-off when `mode==="disabled"` (agent-file default) or capability `context` resource `memory` is denied. `_kiro/memory/{list,get,create,update,delete}` pre-exist (0.66.43); `/memories` delete (2.34.0) = TUI over `_kiro/memory/delete`. Sub-agents: `memory.policy.parentResolved` indexes the dispatcher's memory unless denied | 2.33.1 (0.66.58); parent memory 2.35.0 (0.66.70) | rollout `all`/true; per-agent `memory.mode` default `disabled`; opt-out `settings.userMemoryOptIn:false`; hidden `settings.memoryEnable` | setting | STATIC | NOT-SENT (cyril marshals no `memory`/`userMemoryOptIn`: `crates/cyril-core/src/protocol/kas/settings.rs:77-97`); a user's agent file with `memory.mode` now takes effect under cyril | NOTE cyril-s21o + cyril-0na7 (rollout no longer dark; owner decision whether to send `userMemoryOptIn:false`) |
| **Memory sync**: `_kiro/memory/getSyncStatus {}` -> status; `_kiro/memory/syncStatus` push (per client handshake, periodic) with `state`: `notStarted\|resolving\|off\|unavailable{reason: notSignedIn\|teamKey\|notEntitled\|identityUnknown}\|suspended{reason: forbidden\|backendSignal,lastSyncAt?}\|syncing{lastSyncAt?}\|synced{lastSyncAt,pending}\|failed{reason: sync\|storage,lastSyncAt?,pending}`; `_kiro/memory/sync {}` requests a sync (`memory.sync.requested`); `_kiro/memory/changed {total, counts:{byType,byScope}}` connection-scoped, debounced 250 ms, only while memory is enabled; flags `memory_sync_enabled` false, `memory_sync_{internal,external}_enabled`; env `KIRO_MEMORY_SYNC_ENDPOINT` (0.66.58) replaced by the BFF endpoint from `kiro-session-config` (`readBffEndpoint`, 0.66.91) | getSyncStatus/syncStatus 2.35.0 (0.66.70); sync/changed 3.1.0 (0.66.91) | sync off by default | C->A + A->C | STATIC | DROPPED (kiro.rs:1305) / NOT-SENT | NOTE cyril-s21o |
| **Memory quality**: `memory_quality_enabled` false (0.66.58) -> true (0.66.91), `memory_quality_{internal,external}_enabled`; logs `memory.quality.*`, `memory.getExposure.*` | 2.33.1 / 3.1.0 | flag | internal | STATIC | n/a | — |
| **`turn_timing` kind**: `{kind:"turn_timing", executionId, turnId, ttft:{modelRequestSentAt, firstTokenReceivedAt, agentOutput?{complete, reasons[]}}}`; opt-in through `session/new\|load._meta.kiro.automaticTurnProfiling:{ttft?,perf?,contextBreakdown?}` (`readProfilingOptIns`; `perf` defers `turn_completion`+`session_usage` and adds `throughput`); declared per client (`AutomaticTurnOptIns`) | 2.33.1 (0.66.58) | opt-in | setting + A->C kind | STATIC | NOT-SENT (`crates/cyril/src/domain_mediator/commands/session.rs:475` builds `session/new` without `_meta`) | — (P4 at most) |
| **`suggested_prompts` kind** `{prompts[]}`: side call after a top-level turn when hidden setting `_suggestedPrompts` is on and `KIRO_DISABLE_SUGGESTED_PROMPTS` unset; persisted as session metadata `suggestedPrompts`; `kind:suggested_prompts` sibling of `recap` | 2.34.0 (0.66.62) | hidden, default off | A->C kind | STATIC | NOT-SENT / DROPPED (kas.rs:822) | — |
| **Session recap** (3.1.0 opt-in): catalog item `sessionRecap` (hidden, default off) / AgentSettings `_sessionRecap`; the TUI sends `_sessionRecap` from `chat.enableSessionRecap`; `recap` kind and `KIRO_DISABLE_RECAP` pre-exist (0.66.43) | no KAS delta; gate flipped in the TUI | hidden | A->C kind `recap` | STATIC | NOT-SENT (settings.rs:77-97) / DROPPED (kas.rs:822) | NOTE cyril-53qx |
| **`network_configuration` kind** `{networkConfiguration:{networkAccess, vpcConnection?{appliesTo}}}` pushed after a cloud downlink attaches; `_kiro/environment/get_network_configuration {}` -> same object, `NetworkConfigurationUnavailableError` (-32000) without a remote-sessions endpoint; advertised unconditionally in `extensionMethods` | 3.1.0 (0.66.91) | cloud sessions | A->C kind + C->A | STATIC | n/a (cyril has no cloud sessions); DROPPED if ever sent | — |
| **Jobs**: `_kiro/job/update {sessionId,id,kind,description(<=200 chars),status,startedAt,endedAt?}` (`JobUpdatePublisher`, de-duplicated per status), `_kiro/job/output {sessionId,jobId}\|{sessionId,all:true}` -> `{status,lines[]...}` (tail <=128 KiB), `_kiro/job/cancel`; advertised only when flag `universal_background_tool` (default false) is on; AgentSettings `universalBackgroundTool`; logs `jobs.*` | 3.1.0 (0.66.91) | flag off | A->C + C->A | STATIC | NOT-SENT / DROPPED | NOTE cyril-7p7n (background processes family) |
| **Code intelligence builds**: `_kiro/codeIntelligence/startBuild {sessionId, analysisScope?: structural-only\|structural-and-analysis}` -> `{buildId, workspaceRoot, analysisScope…}`, `cancelBuild`, notification `_kiro/codeIntelligence/indexStatus {buildId,status,reason?,detail?,startedAt?,finishedAt?}` (replayed to late clients); initialize `_meta.kiro.codeIndexAnalysisScopes`; gated by hidden setting `_c2s`; refused for cloud sessions; env `KIRO_C2S_{DEEP_ANALYSIS,MAX_CONCURRENT_INDEX_BUILDS,TOOL_TIMEOUT_MS}` (+`CODEGRAPH_RESOLVER`, `DECLARED_TARGET` 0.66.62); `KIRO_ARCHIFY_CLI_PATH` and the `c2s.archify.*` renderer removed | 3.1.0 (0.66.91) | `_c2s` off | C->A + A->C | STATIC | NOT-SENT / DROPPED | — |
| **`_kiro/configuration/set {krn, value, layer}`** -> `{setting}` (`ConfigurationSetHandler`): writes a catalog setting to layer `user-synced` (account) or `user-local` (`<userConfigDir>` file, `configuration.userLocalSettings.write`, mode 0o700) or a model fallback chain; errors `notGranted`, `locked`, `unavailable` (no signed-in user: `configuration.set.noSignedInUser`); advertised in `extensionMethods`. Catalog rows 41 -> 44: +`_suggestedPrompts` (0.66.62), +`sendMessageDeliveryLimit` (int 1..500, default 50), +`sessionToolsMessaging` (0.66.91) | 2.34.0 (0.66.62); layers 3.1.0 | — | C->A | STATIC | NOT-SENT (cyril never calls it) | NOTE cyril-iq2c |
| **`_kiro/diagram/render {sessionId, diagramId, format: mermaid\|svg}`** -> rendered diagram (mermaid `text/vnd.mermaid`, svg `image/svg+xml`; shapes graph/trace; 2 MiB cap); advertised only with `_meta.kiro.diagrams`; initialize `_meta.kiro.diagramRenderFormats`; `elkjs` 0.11.1 dependency for layout (0.66.62); `_meta.kiro.diagram.id` referenced on fork/export | 2.33.0 (0.66.48) | `initialize.clientCapabilities._meta.kiro.diagrams` (off) | C->A | STATIC | NOT-SENT (engine.rs:137-161 sets no `diagrams`) | NOTE cyril-68ny |
| **Auto-compaction threshold**: AgentSettings `compaction.autoCompactionThreshold` (0..80, default 80); per-session `session/new._meta.kiro.settings` validated (`validateSessionAutoCompactionThreshold`), per-prompt `_meta.kiro.autoCompactionThreshold`; initialize `_meta.kiro.autoCompactionThreshold:{min:0,max:80,default:80}`; relayed sessions `compaction.relayed_threshold_{applied,rejected}` | 2.33.1 (0.66.58) | optional | setting + initialize result | STATIC | NOT-SENT (settings.rs:158-172 marshals only `excludePercent`/`excludeMessages`); initialize `_meta.kiro` not read (`crates/cyril-core/src/protocol/fingerprint.rs:31` reads `agent_capabilities` for identity only) | NOTE cyril-6s21 |
| **`session/prompt._meta.kiro.displayText`** now read by KAS: persisted/replayed user rows show `displayText` instead of the expanded prompt (`Tqu`), and the workflow runner receives it | 2.33.1 (0.66.58) | optional | setting | STATIC | NOT-SENT (`domain_mediator/commands/session.rs:475`) | NOTE cyril-hzkb (open) |
| **`session/prompt._meta.kiro.agentInitiated`** (+`agentInitiatedReason`: `send-message-wake`, `notification-rewake`): marks agent-initiated prompts invisible (`visible:false`), skips the sign-in refusal and the output-style instruction; replayed user rows carry it | 3.1.0 (0.66.91) (reads via `_meta.kiro.agentInitiated`; the flag existed internally earlier) | KAS-internal (workflow wakes) | setting / replay meta | STATIC | DROPPED on replay (user chunks rendered as typed) | NOTE cyril-hzkb / lki9 family (kas-workflow lane owns wake semantics) |
| **`session/new._meta.kiro.shellEnvironment`**: `Record<folder, Record<var, string\|null>>` per-folder env overlay applied to shell commands of that folder (`LiveSessionEnv.applyShellEnvironment`; `session.shell_environment.{applied,malformed}`) | 3.1.0 (0.66.91) | optional | setting | STATIC | NOT-SENT | NOTE cyril-7p7n (terminal family) |
| **`session/set_config_option._meta.kiro.mcpServers`**: client-provided MCP servers on a config change (`applyClientMcpEvent kind:"configOption"`; also `residentJoin`, `runtimeAdopt` for multi-client joins) | 2.34.0 (0.66.62) | optional | setting | STATIC | NOT-SENT (`domain_mediator/commands/kas.rs` sends `configId,value` only) | — |
| **Session tools** (AgentSettings `sessionTools:{awareness,messaging,cloud}`, metadata `sessionAwarenessEnabled`/`sessionMessagingEnabled`/`cloudSessionsEnabled`, `sendMessage.deliveryLimit`): peer messaging between sessions (`PeerMessaging.authorizeTarget` requires workspace trust + messaging on; persisted `_meta.kiro.{peerMessage,peerPromptGeneration,peerReceiverPromptGeneration}`; `delivery_failed`/`notify_failed` kinds) | 3.1.0 (0.66.91) | settings, default off (`sessionToolsMessaging` catalog row) | setting + model tool `send_message` | STATIC | NOT-SENT | — (dark) |
| **Hooks exit-code doc / hook `trigger:"read"`**: hook action help text rewritten (exit-code semantics); `trigger:"read"` is an enterprise-management refresh trigger, not a hook | 3.1.0 | — | — | STATIC | n/a | — |
| **Shell default**: `getDefaultShell` -> `SHELL \|\| (/bin/bash runnable ? /bin/bash : /bin/sh)` for the agent-hosted shell; `_kiro/terminal/shell_type` and the `posix`->`unknown` dialect mapper unchanged (`case"posix"` 1 = 1) | 3.1.0 (0.66.91) | agent-hosted shell only | — | STATIC | n/a: cyril hosts the terminal (`terminal:true`, engine.rs:134) and its `shell_type` reply names the shell (cyril-k8wc closed) | — |
| Shell stdin (3.1.0 "commands that read input get end of input at once"): agent-hosted spawns use `stdio:["ignore",…]` (3 -> 5 sites; count only, the new sites were not read) | 3.1.0 | agent-hosted | — | STATIC | n/a: cyril already nulls stdin (`crates/cyril-core/src/protocol/kas/terminal_io.rs:213`) | — |
| **`web_fetch`** Accept `text/markdown, text/html, application/xhtml+xml;q=0.9, application/xml;q=0.8, */*;q=0.7`, falls back to the html Accept on 406/415 (`web_fetch.accept.fallback`); 10 MB limit pre-exists | 3.1.0 (0.66.91) | — | model tool | STATIC | n/a | — |
| Agent `file://` resource caps: new tool text "` was cut to fit the agent's resource limits. Read the file to see the rest.]`"; `subagent.profileFiles.resolve` / `subagent.profile.resource.unresolved` (sub-agents load their own `file://` resources) | 3.1.0 (0.66.91) | — | prompt content | STATIC | n/a | — |
| Sub-agent turn limit: Subagent Response text "Sub-agent '<id>' stopped at its turn limit before it finished. Its work is NOT done…"; tool refusal "you have reached your turn limit; reply with your final report as text"; `subagent.{iterationLimit.stopped,limitWrapUp.*,response.empty}` | 3.1.0 (0.66.91) | — | orphan `tool_call_update` content | STATIC | HANDLED as content (cyril-1v25 closed) | NOTE cyril-ebqu |
| Stop reasons: `successStopReason` maps model `max_tokens`/refusal -> ACP `max_tokens`/`refusal`, iteration limit -> `max_turn_requests` (pre-existing in 0.66.43; `stopReason:"…"` literal set unchanged) | — | — | `turn_end`/prompt result | STATIC | HANDLED (`kas.rs:1014-1032` via `acp::StopReason`; `crates/cyril-core/src/types/session.rs:762`) | — |
| Empty model reply after a tool call ends the turn (was up to 300 retries): `graphs.empty_end_turn_after_tool_response`; `empty_response_retry` flag default unchanged | 3.1.0 (0.66.91) | — | behaviour | STATIC | n/a | — |
| MCP `structuredContent` extra JSON block (3.1.0 line): **no KAS delta** — `structuredContent` 19 = 19, `.structuredContent` 7 = 7, zero hunks in `norm.diff`; the SDK `projectCallToolResult` helper (appends `JSON.stringify(structuredContent)` as a text block when no text block exists) is in both bundles | — | — | — | STATIC | HANDLED as text content | tui lane |
| MCP elicitation (2.34.0 line): **no KAS delta** — `_kiro/mcp/elicitation {sessionId, toolCallId, elicitation{mode: form\|url,…}}` call site identical (2 = 2); the TUI gained the responder | — | `_kiro/mcp/elicitation` always callable | A->C req | STATIC | cyril answers it (`crates/cyril-core/src/protocol/kas/callbacks.rs:41,439`) | — |
| MCP governance (2.35.0): `mcpEnabled` startup configuration value (`Sr.mcpEnabled==="on"`), `mcp_tools.enabled`/`web_tools.enabled` logs; `_kiro/governance/state` and `_kiro/mcp/governance_disabled` pre-exist; 3.1.0 `DiscoveredProfile` keeps MCP/web on when a later profile check fails | 2.35.0 (0.66.70) / 3.1.0 | org | A->C (pre-existing) | STATIC | `governance_disabled` HANDLED (`crates/cyril-core/src/protocol/convert/kas/mcp.rs:119`); `governance/state` DROPPED | NOTE cyril-0o7e |
| MCP misc: optional power servers no longer block turns (`kind:mcp_unavailable`, 0.66.70), subagents lose inline MCP (`mcp.delegation.*`, `mcp.session.admission_denied`, 0.66.62), same-origin redirects, HTTP 403 probe downgrade (`mcp.era.*`, 0.66.70), registry `oauth` pre-existing | 2.33.1-2.35.0 | — | internal | STATIC | n/a | — |
| Initialize result `_meta.kiro`: +`autoCompactionThreshold` (0.66.58), +`diagramRenderFormats` (with diagrams), +`codeIndexAnalysisScopes` (with c2s); `extensionMethods` += `_kiro/environment/get_network_configuration`, `_kiro/configuration/set`, memory `getSyncStatus`/`sync`, conditional `_kiro/job/{cancel,output}`, `_kiro/diagram/render`, `_kiro/codeIntelligence/{startBuild,cancelBuild}`; `authMethods` unchanged (`aws-builder-id`, `aws-iam-identity-center`) | 2.33.1 / 3.1.0 | — | initialize result | STATIC | DROPPED (fingerprint.rs:31 reads `agentCapabilities` only for engine identity) | — |
| `clientCapabilities._meta.kiro` resolver unchanged (17 keys); `.settings` zod keys 49 -> 61 (`settings.txt`); known-extra keys unchanged | — | — | handshake | STATIC | cyril still sends `thinking`, `todoList`, `subagentOrchestration` -> 3× `configuration.intake.unknown-key` (settings.rs:77-97) | NOTE cyril-53qx |
| Deps/packaging: `tree-sitter-cpp` 0.23.4 external + `cpp` in the c2s parser map `{python,typescript,javascript,java,rust,go,cpp}` (0.66.62); `elkjs` (0.66.62); `ssh2` 1.17.0 declared + external but **zero textual import in acp-server.js** (its optional deps asn1/bcrypt-pbkdf/cpu-features/nan/tweetnacl/safer-buffer/buildcheck are the new node_modules; `_kiro/environment/start_ssh_session` pre-exists and mints credentials from the remote-sessions backend); `@aws-sdk/util-endpoints` + `DiscoveredProfile` (isolated-region routing, `auth.api_key_region.*`); cedar-wasm 4.14.0 | 2.34.0 / 3.1.0 | — | packaging | STATIC | n/a | — |
| Runtime: Node compile cache (`node-compile-cache.d.ts`, `standalone-launcher.d.ts` empty; cache dir `<tmp>/kiro-agent-compile-cache-<uid>`, owner-only, honours `NODE_COMPILE_CACHE`, reads `USERPROFILE` on Windows); `ACP_WS_HOST` for the ws transport (default 127.0.0.1); `KIRO_TEST_FREEZE_CACHED_ROWS`; `client-connections.d.ts` `ClientConnectionEvents.subscribe` (0.66.70) | 3.1.0 / 2.35.0 | — | process | STATIC | n/a | — |
| Workspace trust: `workspaceTrusted:!0` hard-coded in both entry points (`startStdio`, ws); gates pre-exist in 0.66.43 (hooks v2 execution, MCPConfigManager workspace config, LSP/code tool, workflow recipe roots, policy `kiro-scope:untrusted-execution` ask rules for shell/mcp/power, peer messaging); no `--trust*` CLI arg, no `KIRO_*` env, no handshake key; the TUI stores `workspaceTrusted` in its own workspace-roots file and KAS never parses that key (0 hits) | — (refactor only: `for(let m of Agl)` loop) | host/TUI | — | STATIC | unaffected: cyril's spawn is always trusted (same as 2.32.0) | — (method note) |
| Telemetry: `kuts_telemetry` default true unchanged; `otelExport` 1 -> 3, `usageAnalytics` 13 -> 19 (counts include keepNames labels; the 3.1.0 org OTel export gating is NOT verified as new logic from these counts); `telemetry.metricDropped` (0.66.70) | 3.1.0 | org | internal | STATIC | n/a | — |

## Method set / kinds / flags set diffs

- `_kiro/*` literals (`static-kas-core-sets-3.1.0.py`): **123 -> 124 -> 125 -> 126 -> 128 -> 138**. Added by first version:
  0.66.48 `_kiro/diagram/render`; 0.66.58 `_kiro/memory` (an experiment-readiness boundary label, not a method); 0.66.62
  `_kiro/configuration/set`; 0.66.70 `_kiro/memory/getSyncStatus`, `_kiro/memory/syncStatus`; 0.66.91
  `_kiro/codeIntelligence/{startBuild,cancelBuild,indexStatus}`, `_kiro/environment/get_network_configuration`,
  `_kiro/job/{cancel,output,update}`, `_kiro/mcp/oauthCompleted`, `_kiro/memory/{changed,sync}`. Removed: none. Renamed: none.
  Persistence classes (`classifyKnownExtMethod`): jobs `sessionLive`; configuration/set, memory sync, network, codeIntelligence
  builds `localOnly`/`transient`.
- ACP method literals 25 = 25. `sessionUpdate` variants 8 = 8 (`agent_message_chunk, available_commands_update,
  config_option_update, current_mode_update, session_info_update, tool_call, tool_call_update, user_message_chunk`).
  `stopReason:"…"` literals `end_turn`, `cancelled` unchanged.
- `session_info_update` kinds (`session-info-kinds.txt`): **25 -> 25 -> 26 -> 27 -> 27 -> 29**: +`turn_timing` (0.66.58),
  +`suggested_prompts` (0.66.62), +`network_configuration`, +`session_usage` (0.66.91). Removed: none.
- Feature-flag registry (`flags/`): **30 -> 30 -> 35 -> 35 -> 36 -> 40**; env-reachable 15 -> 20. Added: 0.66.58
  `model_fallback_auto_refusal` (false), `memory_quality_enabled` (false), `memory_sync_enabled` (false); 0.66.70
  `consolidate_disclosures` (false); 0.66.91 `memory_quality_{internal,external}_enabled` (disabled/false),
  `memory_sync_{internal,external}_enabled` (disabled/false), `universal_background_tool` (false), `create_response_api_auxiliary`
  (disabled). Defaults changed: 0.66.58 `memory_internal_enabled` disabled -> `all`, `memory_external_enabled` false -> true;
  0.66.91 `model_fallback_refusal`, `model_fallback_availability`, `model_fallback_auto_refusal` false -> true,
  `memory_quality_enabled` false -> true. Removed: none.
- `KIRO_*` quoted literals **41 -> 52** (+`KIRO_FEATURE_MEMORY_QUALITY_ENABLED`, `KIRO_FEATURE_MEMORY_SYNC_ENABLED`,
  `KIRO_FEATURE_MODEL_FALLBACK_AUTO_REFUSAL_ENABLED`, `KIRO_TRUST_CLASSIFIER_MODE` @0.66.58; `KIRO_C2S_CODEGRAPH_RESOLVER`,
  `KIRO_C2S_DECLARED_TARGET`, `KIRO_DISABLE_SUGGESTED_PROMPTS` @0.66.62; `KIRO_FEATURE_CONSOLIDATE_DISCLOSURES_ENABLED` @0.66.70;
  `KIRO_C2S_DEEP_ANALYSIS`, `KIRO_C2S_MAX_CONCURRENT_INDEX_BUILDS`, `KIRO_C2S_TOOL_TIMEOUT_MS`,
  `KIRO_FEATURE_UNIVERSAL_BACKGROUND_TOOL_ENABLED` @0.66.91; -`KIRO_ARCHIFY_CLI_PATH` @0.66.91). `process.env` reads 68 -> 69
  (+`KIRO_MEMORY_SYNC_ENDPOINT` 0.66.58 then removed 0.66.91; +`ACP_WS_HOST`, `KIRO_TEST_FREEZE_CACHED_ROWS`, `USERPROFILE`;
  -`PRINCIPAL_TYPE`, -`SYSTEMROOT`). `AB_*` enum members: `AB_MEMORY*` 6 -> 21 references (quality/sync additions). Host-only
  env in the changelogs (0 hits in every KAS bundle): `KIRO_DEFAULT_SETTINGS`, `KIRO_RUNTIME_DIR`, `KIRO_DISABLE_TELEMETRY`.
- AgentSettings zod keys (`settings.txt`) **49 -> 49 -> 53 -> 54 -> 54 -> 61**: +`compaction.autoCompactionThreshold` (0.66.58),
  +`_suggestedPrompts` (0.66.62), +`sessionTools{awareness,messaging,cloud}`, `universalBackgroundTool`, `sendMessage{deliveryLimit}`
  (0.66.91). Known-extra keys unchanged. Catalog rows (`static-kas-core-catalog-3.1.0.py`) 41 -> 42 -> 44.
- `clientCapabilities._meta.kiro` resolver 17 = 17 keys; `_meta.kiro.X` reads 15 -> 19 (+`displayText` 0.66.58; +`agentInitiated`,
  `diagram`, `peerPromptGeneration` 0.66.91). Hook trigger table +`PreCompact`, `PostCompact` (0.66.91); ACP `mae` trigger enum
  unchanged. CLI `--` literal set 140 -> 141 (+`--deny-w`, `--symlink`, -`--quality`: bwrap/sandbox args; no trust arg).

### Standing coverage: 0.66.91 emissions vs cyril (HEAD e3a3868d)

`session_info_update` kinds (29): HANDLED (9) — `display_error` (only `errorType:"mcp_connection_error"`), `hook_update`,
`turn_completion`, `turn_start`, `turn_end`, `context_usage`, `steering_queued`, `steering_injected`, `steering_cleared`
(`crates/cyril-core/src/protocol/convert/kas.rs:627-760`). DROPPED (20, fall through the `kind` match at kas.rs:627 to `_ => None` at kas.rs:822):
`checkpoint_restore_availability`, `focus_update`, `interaction_resolved`, `model_fallback`, `model_routed`, `network_configuration`,
`pending_interaction`, `queued`, `recap`, `repositories_update`, `session_usage`, `steering_inclusion`, `suggested_prompts`,
`summarization_completed`, `summarization_failed`, `summarization_separator`, `summarization_started`, `summary_message`,
`turn_timing`, `user_message_id_assigned`.

`_kiro/*` agent->client notifications (excluding `_kiro/workflow/*`, kas-workflow lane): HANDLED — `_kiro/error/rate_limit`
(`convert/kiro.rs:1075`), `_kiro/system/notify` (kiro.rs:1086), `_kiro/mcp/status` + `_kiro/mcp/governance_disabled`
(`convert/kas/mcp.rs:119-132`), `_kiro/powers/items_changed` (`engine.rs:887-902`, `convert/kas/powers.rs`), `_kiro/session/notify`
(`convert/kas/notify.rs`), `_kiro/hooks/didChange` + `_kiro/hooks/cancel` (`kas/hooks.rs:35-39`). DROPPED at `convert/kiro.rs:1305`
(`_ => Ok(None)`, silent): `_kiro/c2s/view/changed`, `_kiro/code_references`, `_kiro/codeIntelligence/indexStatus` (new),
`_kiro/configuration/state` (not requested), `_kiro/customAgent/{config_error,not_found}` (cyril-7xxk), `_kiro/governance/state`,
`_kiro/job/update` (new, flag off), `_kiro/knowledge/{indexingStarted,indexingCompleted}`, `_kiro/mcp/oauthCompleted` (new,
REACHABLE), `_kiro/memory/changed` (new), `_kiro/memory/syncStatus` (new), `_kiro/policy/{changed,error}`,
`_kiro/progressive_context/items_changed`, `_kiro/safety/{propertiesChanged,statusChanged}`, `_kiro/sandbox/status`,
`_kiro/sessions/changed`, `_kiro/spec/{phaseCheckpoint,taskStatusChanged}`, `_kiro/steering/documents_changed`,
`_kiro/tools/{content_chunk,didChange}`.

## Unreleased / dark candidates

| candidate | gate | default | evidence |
|---|---|---|---|
| Trust classifier / "Approve for me" wire marks | env `KIRO_TRUST_CLASSIFIER_MODE=enforce` + rule `effect:"evaluate"` | off | `qMs=resolveClassifierMode`; `Wpo` classifierDisplayMetaOf; `acp.policy-eval.classifier-*`; TUI `pit={capability:"all",effect:"evaluate"}` |
| Background jobs protocol (`_kiro/job/*`, `universalBackgroundTool`) | flag `universal_background_tool` | false | `extensionMethods` conditional `...registry.get(UNIVERSAL_BACKGROUND_TOOL)?["_kiro/job/cancel","_kiro/job/output"]:[]`; `JobUpdatePublisher` |
| Code-index builds (`_kiro/codeIntelligence/{startBuild,cancelBuild,indexStatus}`) | hidden setting `_c2s` | off | `_h("c2s")?ZEr:[]`; `codeIndexAnalysisScopes` |
| Session tools: peer messaging / awareness / cloud sessions | `settings.sessionTools.{awareness,messaging,cloud}` | off (`sessionToolsMessaging` row) | `P_={awareness,messaging,cloud}`; `PeerMessaging`; `sendMessageDeliveryLimit` 1..500 default 50 |
| Memory sync to the BFF | flag `memory_sync_enabled` | false | `_kiro/memory/sync`, `syncStatus` union, `readBffEndpoint` |
| Memory quality gate | `memory_quality_enabled` | true (0.66.91) | `memory.quality.gate.resolved` |
| `turn_timing` profiling | `session/new\|load._meta.kiro.automaticTurnProfiling.{ttft,perf,contextBreakdown}` | off | `readProfilingOptIns`, `AutomaticTurnOptIns` |
| Suggested prompts | hidden `_suggestedPrompts`, `KIRO_DISABLE_SUGGESTED_PROMPTS` | off | `startSuggestedPrompts`, `suggestedPrompts.{empty,generated}` |
| Session recap | hidden `sessionRecap` / `_sessionRecap` | off | `startRecap`, `KIRO_DISABLE_RECAP` |
| Diagram render endpoint | `_meta.kiro.diagrams` | off | `handleDiagramRender`, formats mermaid/svg |
| `consolidate_disclosures` | flag | false | `An=t.consolidateDisclosures===!0` in session construction |
| `create_response_api_auxiliary` | experiment flag | disabled | registry row |
| Network configuration / SSH session (cloud) | remote-sessions endpoint (`--remote-sessions-endpoint` / `KIRO_REMOTE_SESSIONS_ENDPOINT`) | unset | `UnavailableNetworkConfigurationSource`, `UnavailableSshCredentialSource` |
| Experiment pinning `KIRO_EVAL_*` / `settings.evalStrategy` (pre-existing) | unset | — | unchanged from 2.32.0 |

## Leads for the live lane

All legs: fresh temp `HOME`, real `XDG_DATA_HOME`, direct `node acp-server.js` from carve 3.1.0 on its own Node
(`binaries/3.1.0/node`), paired with carve 2.32.0 where noted, same day (trap 1). Positive control on every leg:
`initialize.clientCapabilities._meta.kiro.configurationState:true` (expect `_kiro/configuration/state` pushes; traps 5/28;
`largeToolOutputHandler` is NOT a control). KAS debug log: `KIRO_LOG_LEVEL=debug`, `$HOME/.kiro/logs/<ts>/kiro.log`. No
`login`/`logout`; the auth leg below makes the responder fail, never the account (trap 6).

- **KAS-L1 model fallback default-on (passive + flag arm, zero extra turns).** Arm A plain; arm B env
  `KIRO_FEATURE_MODEL_FALLBACK_AVAILABILITY_ENABLED=false`, `KIRO_FEATURE_MODEL_FALLBACK_REFUSAL_ENABLED=false`,
  `KIRO_FEATURE_MODEL_FALLBACK_AUTO_REFUSAL_ENABLED=false`. Expected: kiro.log feature-config resolution lists the three flags
  `true` (A) vs `false` (B) on 0.66.91 and `false` on 0.66.43. Record any `session_info_update{kind:"model_fallback"|"model_routed"}`
  and `_meta.kiro.modelFallback` on chunks during every other leg (cyril-aoby evidence).
- **KAS-L2 `session_usage` (one prompt + one load).** Handshake plain. After `turn_completion` expect
  `session_info_update{_meta.kiro:{kind:"session_usage",credits:[{usage,unit,unitPlural}]}}`; then `session/load` (same
  sessionId, default replay) and expect a replayed `session_usage` (replay-marked `_meta`) before the catch-up burst. Pair:
  0.66.43 emits neither. Also record whether `credits` equals the running sum of the leg's `promptTurnSummaries`.
- **KAS-L3 sign-in refusal (one prompt, cyril-shaped auth).** `--auth=acp-callback`; arm A answers `_kiro/auth/getAccessToken`
  with a JSON-RPC error (never a bad token, never `logout`); arm B (control) answers with the real token. Expected A:
  `session/prompt` fails `{code:-32000, message:"Sign in to Kiro to send a prompt.", data:{errorType:"SignInRequiredError",
  retryErrorType:"CLIENT_ERROR"}}`, no `turn_start`, nothing persisted (`session/load` replays no user message); kiro.log
  `auth.identity.*`. On 0.66.43 arm A should fail differently (TokenExpired/Invalid at the model call). Then run the same arm
  against cyril (debug build at HEAD, record the commit, trap 31) and read what the UI shows and whether the busy state clears.
- **KAS-L4 Approve for me (two arms, one shell turn each).** User `permissions.yaml` with
  `rules:[{capability:"all",effect:"evaluate"}]`. Arm A env `KIRO_TRUST_CLASSIFIER_MODE=enforce`; arm B env unset. Prompt
  "Run exactly: echo AUDITX". Expected A: either the command runs with `tool_call._meta.kiro.trustClassifierApproved:true`, or a
  `tool_call_update{status:"pending", content:[text]}` with `_meta.kiro.trustClassifier:{decision:"prompt",reason}` followed by
  `session/request_permission` carrying `_meta.kiro.trustClassifier`; kiro.log `acp.policy-eval.classifier-*`. Expected B:
  ordinary `request_permission`, kiro.log `policy.reference.classifier_off` or the rule skipped. 0.66.43 ignores both. Ask the
  host lane first whether `kiro-cli acp` (3.1.0 binary carries the env name once) sets the variable on its KAS spawn.
- **KAS-L5 memory rollout (two prompts + RPCs).** Agent file `memory:{mode:"read_write",reflection:true}` (or
  `initialize.clientCapabilities._meta.kiro.settings.memory:{mode:"read_write",reflection:true}`). Expected 0.66.91:
  `_kiro/memory/list {}` returns `{memories:[…]}` (not `"Memory is not enabled."`), kiro.log `memory.placement.decided`,
  `memory.extraction.*` after the second turn, `_kiro/memory/changed {total,counts:{byType,byScope}}` (connection-scoped) after a
  write, `_kiro/memory/getSyncStatus {}` -> `{state:"off"}` (sync flag false). Control: same config on 0.66.43 must answer
  `"Memory is not enabled."` (rollout `disabled`). Second control on 0.66.91: `settings.userMemoryOptIn:false` -> disabled
  (`disabledByOptOut`). Note any hidden extraction model call in `turn_completion.promptTurnSummaries` (billing).
- **KAS-L6 `_kiro/mcp/oauthCompleted` (opportunistic, rides any leg with a remote MCP server).**
  `initialize.clientCapabilities._meta.kiro.openExternalUrl:true`. Record `_kiro/openExternalUrl` params (`url, serverName,
  oauthCompletion, oauthFlowId`) and the later `_kiro/mcp/oauthCompleted {serverName, success, oauthFlowId}`; 0.66.43 sends no
  oauthCompleted.
- **KAS-L7 compaction hooks (one forced compaction).** Agent profile hooks `{trigger:"preCompact", command:"exit 2"}` and
  `{trigger:"postCompact", command:"cat > $HOME/post.json"}`; `_kiro/session/compact {sessionId}` after one turn. Expected:
  pre hook -> compaction refused, kiro.log `hooks.compaction.blocked`, `session_info_update{kind:"hook_update"}` frames; with
  the pre hook removed, post hook runs with stdin JSON carrying `compaction_id, reason, message_count, transcript_path, summary,
  summary_path`. Also assert `_kiro/hooks/list` is never called with a compaction trigger when `_meta.kiro.hooks:{enabled:true}`
  is advertised (kiro.log `[AcpHooksProvider]`).
- **KAS-L8 PreToolUse exit 2 without a reason (host-mode hooks).** Advertise `initialize.clientCapabilities._meta.kiro.hooks:{enabled:true}`;
  answer `_kiro/hooks/list{trigger:"preToolUse"}` with one command hook and `_kiro/hooks/executeHook` with `{exitCode:2,
  output:""}`. Expected 0.66.91: tool blocked, denial text non-empty; compare 0.66.43.
- **KAS-L9 `turn_timing` opt-in (one prompt).** `session/new._meta.kiro.automaticTurnProfiling:{ttft:true}`. Expected
  `session_info_update{kind:"turn_timing", executionId, turnId, ttft:{modelRequestSentAt, firstTokenReceivedAt, …}}` per turn;
  control without the key: none. Arm `perf:true`: `turn_completion` arrives after the stream end with `throughput`.
- **KAS-L10 `_kiro/configuration/set` (zero turns).** From `_kiro/configuration/state` take a user-writable setting krn (e.g.
  `tangentMode`); call `_kiro/configuration/set {krn, value:"on", layer:"user-local"}` -> `{setting}` and a `_kiro/configuration/state`
  push; kiro.log `configuration.userLocalSettings.write`. Control: `layer:"bogus"` -> `Unknown layer`; a locked setting ->
  `reason:"locked"`.
- **KAS-L11 initialize result (zero turns).** Diff `agentCapabilities._meta.kiro` 0.66.43 vs 0.66.91: `autoCompactionThreshold:{min:0,max:80,default:80}`,
  `extensionMethods` additions; with `_meta.kiro.diagrams:true` also `diagramRenderFormats:["mermaid","svg"]`.
- **KAS-L12 `shellEnvironment` (one shell turn, client-hosted terminal).** `session/new._meta.kiro.shellEnvironment:{"<cwd>":{"AUDIT_VAR":"1"}}`;
  prompt "Run exactly: echo $AUDIT_VAR". Expected: kiro.log `session.shell_environment.applied`; record whether `terminal/create`
  carries `env` with `AUDIT_VAR` (client-hosted) or only the agent-hosted shell sees it.
- **KAS-L13 `displayText` (one prompt + load).** `session/prompt._meta.kiro.displayText:"SHOWN"` with prompt text "REAL";
  `session/load` replay: 0.66.58+ shows `SHOWN` as the user chunk, 0.66.43 shows `REAL`.

## Changelog reconciliation

Every [V3]/untagged line of the thirteen feeds, located in the bundle or handed to a lane. "KAS-internal" = behaviour change
without a new wire field.

| release | line | location |
|---|---|---|
| 2.33.0 | [V3] trust a folder before loading hooks/agents/MCP/workflows | host/TUI prompt (`resolveWorkspaceTrust`, `workspaceTrusted` file); KAS entry hard-codes `workspaceTrusted:!0`; gates pre-exist (Deltas) |
| 2.33.0 | [V3] plan approvals compact step tree | kas-workflow / TUI |
| 2.33.0 | faster `grep` parallel scan | KAS-internal (ripgrep concurrency), no wire |
| 2.33.0 | Mermaid flowcharts | TUI |
| 2.33.0 | [V3] long sessions hold less memory | KAS-internal (not located statically; `sessionEviction` pre-exists) |
| 2.33.0 | [V3] GitHub PR watches act on conflicts/checks/threads | kas-workflow (zod +`pullRequest, reviewThreads, baseRef, oid` @0.66.48) |
| 2.33.0 | [V3] terminal title | TUI |
| 2.33.0 | [V3] agent-initiated replies no longer cut off | KAS-internal (agent-initiated prompt path) |
| 2.33.0 | [V3] `/agent` `/model` `/effort` completion | TUI |
| 2.33.0 | [V3] sandbox network rules apply live | KAS-internal (`setNetworkRules`, `updateProxyPolicy`) |
| 2.33.0 | [V3] `/sessions` index freeze | KAS-internal (session index) |
| 2.33.0 | [V3] fallback models settable again | `_kiro/configuration/set` fallback chains (0.66.62) / TUI; flags Deltas |
| 2.33.0 | Mermaid wrap; home-dir agents listed twice; `/sandbox` home dir | TUI / KAS-internal |
| 2.33.0 | [V3] `@agent-sop:` runs the SOP | TUI/host |
| 2.33.0 | [V3] cloud config sync; `/code` cloud; empty cloud config card | cloud / TUI |
| 2.33.0 | [V3] `code` tool slow language servers | KAS-internal (LSP init) |
| 2.33.0 | [V3] throttling vs rate limit | `display_error` variants (`reason:throttled`, `outcome:throttled` @0.66.58) — cyril handles only `mcp_connection_error` display_errors (kas.rs:634-651; cyril-58uv) |
| 2.33.0 | [V3] Windows drive-letter recipes; review loops stop on approval; workflows run when Auto disabled | kas-workflow |
| 2.33.0 | Tab after `/model `; [V3] `/code summary` | TUI / KAS-internal |
| 2.33.0 | [V3] sandbox hostnames case; Windows hook same-named program | KAS-internal (sandbox; hook spawn hardening) |
| 2.33.1 | [V3] workflows start only when asked | kas-workflow (`workflowLaunch` zod @0.66.58) |
| 2.33.1 | `/tools` screen reader; `mcp add --agent` upgrade | TUI / host |
| 2.33.1 | [V3] resumed V2 session approvals notice | TUI |
| 2.33.1 | [V3] compaction retries without images; long image sessions omit images | KAS-internal (`summarization.skip_repeated_compaction` @0.66.58; image strip) |
| 2.33.1 | [V3] MCP OAuth callback port | KAS-internal (MCP OAuth) |
| 2.33.1 | [V3] optional power MCP servers don't block turns | KAS-internal (`resolvePowerServer`, later `kind:mcp_unavailable` @0.66.70) |
| 2.33.1 | [V3] plan approvals first option highlighted | TUI |
| 2.33.1 | HTML doctype colors | TUI |
| 2.33.1 | [V3] subagents search own knowledge bases; load own skills | KAS-internal (`subagent.profileSkills.resolve` @0.66.58) |
| 2.33.1 | [V3] registry MCP package version | KAS-internal |
| 2.33.1 | [V3] messages after spec task run keep updating tasks | KAS-internal (spec) |
| 2.33.1 | [V3] `@builtin` includes web search | KAS-internal (tool tag expansion) |
| 2.33.1 | [V3] user agents keep file rules in home folder | KAS-internal (policy load) |
| 2.34.0 | [V3] MCP elicitation form/link | **no KAS delta**: `_kiro/mcp/elicitation` call site identical (2 = 2), modes form/url pre-exist; TUI responder (Deltas) |
| 2.34.0 | [V3] type suggested changes into plan approval | kas-workflow (`planFeedback`, `appliedPlanFeedback` zod @0.66.58) |
| 2.34.0 | [V3] delete memories from `/memories` | TUI over pre-existing `_kiro/memory/delete` (4 hits in 0.66.43) |
| 2.34.0 | `/model edit` hides models | TUI |
| 2.34.0 | [V3] Workflow use setting | kas-workflow / TUI |
| 2.34.0 | [V3] `--trust-workspace` | host/TUI flag (`trustWorkspace` CLI arg in the TUI); 0 hits in KAS |
| 2.34.0 | [V3] approvals list Allow once/Deny once first | TUI ordering; KAS `buildPermissionOptions` unchanged |
| 2.34.0 | [V3] session dashboard delete confirm | TUI |
| 2.34.0 | [V3] unparseable shell commands denied automatically | KAS `PolicySession.unparseable()` (0.66.62; Deltas) |
| 2.34.0 | [V3] subagents no longer get inline MCP | KAS-internal (`mcp.delegation.*`, `mcp.session.admission_denied` @0.66.62) |
| 2.34.0 | `Working...` when agent goes quiet | TUI |
| 2.34.0 | [V3] headless mode answers; reuses MCP sign-ins | host/TUI; MCP credential store |
| 2.34.0 | [V3] `/sessions` freeze | KAS-internal |
| 2.34.0 | enterprise overage in `/usage` | `_kiro/account/getUsage` consumer (TUI); no schema delta located |
| 2.34.0 | [V3] workflows resume after restart | kas-workflow |
| 2.34.0 | refusal continues on fallback model | model fallback (`model_fallback_auto_refusal` flag @0.66.58; default true @0.66.91) |
| 2.34.0 | `chat.allowIcons`; Mermaid | TUI |
| 2.34.0 | [V3] deny rules save at the scope you pick; rule-blocked call no longer "cancelled" | consent scope (pre-existing echo); status mapping literals unchanged in count; `policyDenial` moved into the tool_call_update builder (`withPolicyDenial`, hunks) — verify live (status `failed` + `_meta.kiro.policyDenial`) |
| 2.34.0 | [V3] `/rewind` forks right | KAS-internal (`session.fork.snapshot_id_refused` @0.66.62) |
| 2.34.0 | [V3] workflow monitor approvals | TUI |
| 2.34.0 | [V3] `kiro-cli acp` no longer shuts down mid-run | host (stdin/teardown); KAS `disposeAllSessionsOnce` unchanged in shape |
| 2.34.0 | pasted images/dictation | TUI |
| 2.34.0 | [V3] `/agent` loads skills | KAS-internal |
| 2.34.0 | [V3] `ssh` in macOS sandbox; delete outside workspace on Windows | sandbox / fs internal |
| 2.34.0 | [V3] HTTP MCP same-origin redirects | KAS-internal (MCP client) |
| 2.34.1 | queued message blank lines | TUI |
| 2.34.2 | offers to switch to V3 | TUI/host |
| 2.35.0 | elevated-privilege warning; `/fullscreen`; ghost | host / TUI |
| 2.35.0 | refused response on Auto retried | `model_fallback_auto_refusal` (flag; backend-resolved until 0.66.91 default true) |
| 2.35.0 | [V3] MCP servers and web tools follow org settings | `Sr.mcpEnabled` startup configuration, `mcp_tools.enabled`/`web_tools.enabled` @0.66.70 (Deltas) |
| 2.35.0 | fallback hang with tool call; dictation | KAS-internal / TUI |
| 2.35.0 | [V3] resuming V2 session planner; Linux reboot resume | host / KAS session liveness |
| 2.35.0 | `/chat` `--resume` listing timeout | host/TUI |
| 2.35.0 | [V3] `/mcp` registry hidden servers; `mcp list` skipped; HTTP 403 probe | KAS-internal (`mcp.era.*` @0.66.70) / TUI |
| 2.35.0 | [V3] refusal with no output after tool result ends turn | KAS-internal (graph) |
| 2.35.0 | [V3] `read_file` offset past end | KAS-internal |
| 2.35.1 / 2.35.2 | performance; autocomplete retirement rollout | host |
| 2.36.0 | [V3] Approve for me | trust classifier (0.66.58; Deltas) + TUI `pit` rule |
| 2.36.0 | [V3] shell rule subcommand tier | TUI-composed `match` pattern in the consent echo; KAS accepts any pattern (pre-existing) |
| 2.36.0 | `agent upgrade` warnings; [V3] V2-vs-V3 permission notice | host / TUI |
| 2.36.0 | `acp.agentEngine` setting | host (0 hits in KAS) |
| 2.36.0 | ghost animations; [V3] spec questions progress | TUI / KAS spec |
| 2.36.0 | [V3] shell tool card titles; rule rows keep file name | TUI |
| 2.36.0 | Mermaid titles; non-ASCII MCP tool names crash; indented command rows | TUI / KAS-internal |
| 2.36.0 | [V3] Node 22.23.4; Bun 1.4.4 | host runtime (RUN-STATE) |
| 2.36.0 | git config env vars ask for approval | shell policy (`GIT_CONFIG_GLOBAL` 0 hits in KAS: either host v2 or a regex table) — not located in KAS |
| 2.37.0 | ACP clients sign in from a terminal when `kiro-cli acp` starts logged out | host; KAS side = `CredentialAbsentError` (0.66.48) lets the agent start without a token |
| 2.37.0 | `KIRO_RUNTIME_DIR` | host (0 hits in KAS) |
| 2.37.0 | fullscreen; ghost colour | TUI |
| 2.37.0 | V2 agents auto-upgraded at launch | host/TUI (`chat.enableAutoAgentUpgrade`) |
| 2.37.0 | [V3] approval prompts first line / `ctrl+o`; rule page scope | TUI |
| 2.37.0 | Mermaid; `kiro-cli update` brew | TUI / host |
| 2.37.0 | `--no-interactive` exit 4; V2-format agent stop | host |
| 2.37.0 | fullscreen clicks; [V3] entire-tool row; tmux wheel; ghost; nested Mermaid; long labels | TUI |
| 2.37.0 | [V3] `/spec` escape sequences | TUI |
| 2.37.1 | welcome screen; pill; [V3] answer cut off at output limit says so | TUI (stop reason `max_tokens` pre-exists on the wire) |
| 2.37.1 | [V3] idle CPU tracking the engine's process tree | host |
| 3.0.0 | V3 default; Classic retired; autocomplete/desktop retired | host |
| 3.1.0 | `PreCompact`/`PostCompact` hooks | KAS 0.66.91 (Deltas) |
| 3.1.0 | `KIRO_DEFAULT_SETTINGS` | host (0 hits in KAS) |
| 3.1.0 | `/usage` + status-line session credits | `session_usage` kind (Deltas) |
| 3.1.0 | workflow step rows show credits | kas-workflow (`credits` on node payloads via `readSessionCredits`/`SessionUsage`) |
| 3.1.0 | session recap opt-in | `sessionRecap` hidden catalog item / `_sessionRecap` (TUI sends from `chat.enableSessionRecap`); `recap` kind pre-exists |
| 3.1.0 | sub-agents read dispatcher's memories | `memory.policy.parentResolved` (0.66.70) + 0.66.91 memory rollout |
| 3.1.0 | prompt refused with sign-in message, not saved | `SignInRequiredError` (Deltas) |
| 3.1.0 | agent `file://` resources capped 32 files / 100,000 chars | new literal "…was cut to fit the agent's resource limits…" (numbers not extracted as literals) |
| 3.1.0 | notifications queue; warnings stay | TUI |
| 3.1.0 | startup cloud config / learnings / steering fetches print no tool cards | hidden-tool set `new Set(["fetch_cloud_config"])` pre-exists (1 = 1); the card suppression for learnings/steering is TUI |
| 3.1.0 | `web_fetch` asks for markdown | Accept header (Deltas) |
| 3.1.0 | MCP `structuredContent` extra JSON block | **no KAS delta** (zero hunks); SDK helper in both |
| 3.1.0 | global skills load from home dir with `kiro-cli acp` | KAS-internal (skill discovery) |
| 3.1.0 | `/agent swap kiro_default`; transcript line missing; queued tray | TUI |
| 3.1.0 | prompt retries unanswered connection 90 s | `KIRO_CONVERSE_CONNECTION_TIMEOUT_MS` pre-exists; KAS-internal |
| 3.1.0 | sub-agent stopped at turn limit reports unfinished work | Subagent Response text (Deltas) |
| 3.1.0 | token-endpoint outage keeps MCP sign-in | `mcp.oauth.token_endpoint.answered` @0.66.91 |
| 3.1.0 | turn cut off by output-token cap / step limit says so | stop reasons `max_tokens`/`max_turn_requests` pre-exist; wording is TUI; `[IterationLimit] Iteration limit reached` log |
| 3.1.0 | `/context show` skill:// unloadable | KAS-internal |
| 3.1.0 | transcripts past 512 MiB | KAS-internal (persistence) |
| 3.1.0 | shell commands run in bash when `SHELL` unset | `getDefaultShell` (Deltas) |
| 3.1.0 | session start no longer deletes another session's logs | KAS-internal (log prune; `logsBasePath` 6 = 6) |
| 3.1.0 | `str_replace` `$$` patterns | KAS-internal |
| 3.1.0 | sub-agents load own `file://` resources | `subagent.profileFiles.resolve` @0.66.91 |
| 3.1.0 | prompt sent as a workflow finishes not dropped; worktree branch | kas-workflow |
| 3.1.0 | spec PBT results without approval; "Run required tasks" finishes | KAS spec internal |
| 3.1.0 | `agent upgrade` warns about V2 tools | host |
| 3.1.0 | `ctrl+o` hint; Mermaid sequence; resize; scrollback; fullscreen rows | TUI |
| 3.1.0 | empty model reply ends the turn (was 300 retries) | `graphs.empty_end_turn_after_tool_response` @0.66.91 |
| 3.1.0 | MCP/web tools stay on when profile check fails | `DiscoveredProfile` / governance (0.66.91) |
| 3.1.0 | MCP sign-in links disappear once connected | `_kiro/mcp/oauthCompleted` now sent (Deltas) |
| 3.1.0 | shell commands that read input get EOF at once | agent-hosted `stdio:["ignore",…]` (3 -> 5 sites); cyril nulls stdin already |
| 3.1.0 | `agent validate` exit code | host |
| 3.1.0 | damaged knowledge listing | `knowledge.listing.failed` @0.66.91 |
| 3.1.0 | requests reach isolated AWS regions | `@aws-sdk/util-endpoints` + API-key region discovery (`DiscoveredProfile`) |
| 3.1.0 | `PreToolUse` exit 2 with no reason blocks | v2 hooks executor (Deltas) |
| 3.1.0 | Windows shell command child processes | `taskkill_failed` kind @0.66.91 |
| 3.1.0 | sub-agent knowledge bases refresh | KAS-internal |
| 3.1.0 | footer session credit total survives reopen | `session_usage` replay on `session/load` |
| 3.1.0 | `--no-interactive` / `kiro-cli acp` no `[INFO]` stderr lines | KAS logger (`[KiroAgent]` info lines) / host; not pinned statically — check cyril's stderr capture live |
| 3.1.0 | `--no-interactive` newline after tool call | TUI/host |
| 3.1.0 | cloud sessions unsupported region | cloud |
| 3.1.0 | crash during save drops next history entry | KAS-internal (persistence) |
| 3.1.0 | `web_fetch` pages over 10 MB reported as too large | `kWe=10*1024*1024` pre-exists; error text change |
| 3.1.0 | zero-result `web_search` shows provider message | `parseSearchBody` + `readMessage` (`message:cJo(s)`) @0.66.91 |
| 3.1.0 | tool-use telemetry under org OTel export | not pinned: `otelExport`/`usageAnalytics` count rises are keepNames-inflated; host governs `telemetry.enabled`/`KIRO_DISABLE_TELEMETRY` |
| 3.1.0 | remote MCP registrations and credential folders owner-only | `configuration.userLocalSettings.write` passes a `mode:` constant on mkdir (`mode:Mi`, minified name; the 0 -> 38 count is a rename artefact and the value was not read) — weak static evidence |
| 3.1.0 | [V2] `GIT_DIR`/`GIT_WORK_TREE` ask | v2 host (out of lane) |

## Proposed issues / notes

Searched `.rivets/issues.jsonl` (454 rows) for: model_fallback/aoby, governance/0o7e, nk4o/tqr8, 58uv, 53qx, j66p, lxuo, iq2c,
1aq0, s21o/0na7, zbu7/0rn8, session_usage/credit, oauthCompleted, elicitation, classifier/evaluate, PreCompact/compaction,
stdin, suggested, recap, profiling, displayText (hzkb), _kiro/job, background/7p7n, memory, sign-in, turn_timing,
extensionMethods, ebqu, 7xxk, 68ny, 0asq, w55l, 4u4a, k8wc, auxt, trust, d7ve.

- **NOTE cyril-aoby (open) — raise to P2:** 0.66.91 flips `model_fallback_{refusal,availability,auto_refusal}` to default true;
  on cyril's default engine the model can change mid-session with no UI trace (`model_fallback`/`model_routed` dropped at
  kas.rs:822). Entry point: every prompt (no opt-in), so P2 by the fold rules; live KAS-L1 confirms the flag table.
- **NEW P3 "Consume `session_usage` for session credit totals (load-time and authoritative per-turn merge)"** — or NOTE on
  cyril-0o7e: kas.rs:822 drops it; session.rs:291 sums turns live; a reopened session shows zero. Labels `kiro-3-1-0`, `kas`.
- **NOTE cyril-0rn8 (open):** KAS 0.66.91 refuses prompts with `SignInRequiredError` (-32000, `data.errorType`) when the
  acp-callback credential is absent instead of hanging at the model call; cyril should map `errorType:"SignInRequiredError"` to a
  sign-in hint and make sure the turn mediator releases the busy state (KAS-L3 against cyril).
- **NEW P3 "Approve for me on KAS"**: set `KIRO_TRUST_CLASSIFIER_MODE=enforce` in cyril's KAS spawn env (opt-in setting) and render
  `tool_call._meta.kiro.trustClassifierApproved` / the `trustClassifier` flag line and `request_permission._meta.kiro.trustClassifier`;
  blocked on the host lane confirming how `kiro-cli acp` sets the env. Labels `kiro-2-33-1`, `kas`.
- **NEW P4 "Clear the MCP sign-in notice on `_kiro/mcp/oauthCompleted`"** (NOTE cyril-tqr8 closed, cyril-nk4o closed): reachable
  today because cyril advertises `openExternalUrl` (engine.rs:143); dropped at kiro.rs:1305.
- **NOTE cyril-s21o + cyril-0na7:** memory rollout GA since 0.66.58 (`memory_internal_enabled:"all"`, `memory_external_enabled:true`);
  policy follows the agent file's `memory.mode`; opt-out `settings.userMemoryOptIn:false`; new `_kiro/memory/{sync,getSyncStatus}`,
  `_kiro/memory/{syncStatus,changed}`; sub-agents index the dispatcher's memory. Owner decision (CLAUDE.md: native memory defaults
  stay unchanged by explicit approval) — this note records that the upstream default itself moved.
- **NOTE cyril-58uv:** kinds 25 -> 29 (+`turn_timing`, `suggested_prompts`, `network_configuration`, `session_usage`); 9/29 handled;
  `_kiro/*` notification set +`codeIntelligence/indexStatus`, `job/update`, `mcp/oauthCompleted`, `memory/changed`, `memory/syncStatus`
  all dropped at kiro.rs:1305; `display_error` variants for throttling still dropped.
- **NOTE cyril-53qx:** AgentSettings 61 keys in 0.66.91 (`settings.txt`); cyril still marshals `thinking`, `todoList`,
  `subagentOrchestration` (settings.rs:77-97) -> three `configuration.intake.unknown-key`; new keys `compaction.autoCompactionThreshold`,
  `_suggestedPrompts`, `_sessionRecap` (recap gate), `sessionTools.*`, `universalBackgroundTool`, `sendMessage.deliveryLimit`.
- **NOTE cyril-hzkb (open):** KAS reads `session/prompt._meta.kiro.displayText` since 0.66.58 (persisted/replayed as the shown user
  text) and `_meta.kiro.agentInitiated` since 0.66.91 (replayed agent-initiated prompts are marked).
- **NOTE cyril-6s21:** `compaction.autoCompactionThreshold` (0..80, default 80) in AgentSettings, per-prompt `_meta.kiro.autoCompactionThreshold`,
  initialize `_meta.kiro.autoCompactionThreshold:{min,max,default}` (0.66.58).
- **NOTE cyril-iq2c:** `_kiro/configuration/set {krn,value,layer}` (0.66.62; layers `user-synced`/`user-local` + fallback chains 0.66.91);
  catalog rows 41 -> 44.
- **NOTE cyril-68ny:** `_kiro/diagram/render {sessionId, diagramId, format: mermaid|svg}` and initialize `diagramRenderFormats`.
- **NOTE cyril-7p7n:** `session/new._meta.kiro.shellEnvironment` per-folder env overlay; `_kiro/job/*` background-job protocol behind
  `universal_background_tool`; agent-hosted `SHELL` fallback to `/bin/bash`.
- **NOTE cyril-7c0s:** the 2.36.0 subcommand tier (`git stash *`) is a client-composed `match` pattern on the consent echo; cyril
  echoes the exact resource only (kas.rs:67-91).
- **NOTE cyril-0o7e:** `_kiro/governance/state` now also drives `mcpEnabled`/web tools startup configuration (0.66.70) and the
  `DiscoveredProfile` keeps them on when a later profile check fails (0.66.91).
- **NOTE cyril-ebqu:** sub-agent turn-limit wrap-up text rides the Subagent Response (0.66.91).
- **NOTE cyril-497j:** hook triggers `PreCompact`/`PostCompact` exist only on the KAS-side v2 runtime; the ACP `_kiro/hooks/list`
  trigger enum is unchanged.
- No issue: workspace trust (hard-coded trusted on cyril's spawn), structuredContent (no KAS delta), MCP elicitation (no KAS delta).

## Method notes (traps hit)

- **Hard-coded flags live at the entry point, not where they are consumed.** `workspaceTrusted` has 89 consumers and three
  plausible wire sources; the answer was `workspaceTrusted:!0` in `startStdio`/ws bootstrap. For any "how does KAS learn X" question,
  grep `X:!0|X:!1` and the `process.env`/`g0("--arg")` reads at the bootstrap before reading consumers.
- **A changelog line can be true with zero KAS delta.** 2.34.0 "MCP elicitation" and 3.1.0 "structuredContent extra JSON block"
  both resolve to the TUI/SDK side: `_kiro/mcp/elicitation` 2 = 2 and zero `structuredContent` hunks in `norm.diff`. Check the
  lexer diff (`hunks.txt`) for the keyword before attributing a line to the engine.
- **Flag defaults move without a method or schema change** (trap 3 variant). The biggest user-facing change of this cycle (model
  fallback on) is three `default:!1 -> !0` edits visible only in the registry extraction; the memory rollout flip was a string
  default (`"disabled" -> "all"`). Run the flags script on every carve, not only OLD/NEW, and bisect defaults, not just keys.
- **Minified handler maps gain `a(fn,"name")` wrappers between versions**, so a regex anchored on `"method":(` misses OLD or NEW.
  Anchor on the quoted method literal alone and read both contexts (`_kiro/configuration/set` handler exists since 0.66.62 even
  though `"…":a(` matched only 0.66.91).
- **The six-carve count row is the bisect.** `static-kas-core-ctx-3.1.0.py` prints one count per carve; a first non-zero column
  attributes the change to a release without diffing intermediate bundles. Keep it for the next cycle (five KAS deltas is the new
  normal).
- **zsh does not word-split `$B`.** Build multi-path argument lists as arrays (`B=(); B+=(…); "${B[@]}"`); the first kinds/settings
  runs failed on a single space-joined argument.
- **Static could not pin two 2.34.0 lines** ("rule-blocked call no longer cancelled", "kiro-cli acp no longer shuts down mid-run")
  and the 2.36.0 git-config env rule (`GIT_CONFIG_GLOBAL` 0 hits in KAS): the status-mapping literal set is unchanged in count and
  the hunks only show `policyDenial` moving into the tool_call_update builder. These are live leads, not findings.
- Covenant doc (docs/kiro-kas-acp-covenant.md) lacks: `_kiro/configuration/set`, `_kiro/diagram/render`, `_kiro/memory/{sync,
  getSyncStatus,syncStatus,changed}`, `_kiro/job/*`, `_kiro/codeIntelligence/{startBuild,cancelBuild,indexStatus}`,
  `_kiro/environment/get_network_configuration`, `_kiro/mcp/oauthCompleted`, the four new kinds, `effect:"evaluate"`,
  `automaticTurnProfiling`, `shellEnvironment`, `autoCompactionThreshold`, `sessionTools`. Sync when the covenant package updates.

## Addendum (orchestrator heads-up, 2026-11-08): keepNames and lane split

- **0.66.91 is built with esbuild keepNames**: every function carries `a(fn,"name")`, so every method/kind/function-name literal
  gains occurrences without a new call site, and ~17k of the 19,532 "new-only" literals in `literals-delta.txt` are function names.
  Re-check of this lane's 0.66.91-only rows against that rule (trap 2): every row above that attributes a change to 0.66.91 was
  verified from surrounding code, not from a count — `trigger:"PreCompact"` dispatch + hook env builder (`Qir`), `SessionUsage`
  class + emit sites, `SignInRequiredError` class + `refuseWhileSignedOut()` throw site, `notifyOAuthCompleted` emit,
  `getDefaultShell` `/bin/bash` fallback, `ConfigurationSetHandler.set`, `JobUpdatePublisher`, `handleGetNetworkConfiguration`,
  `machineTokenType` assignments, the `shellEnvironment` zod key, `_meta.kiro.agentInitiated` read, `readProfilingOptIns`.
  Rows that rested on bare counts are now hedged in place: telemetry (`otelExport`/`usageAnalytics`), the owner-only `mode:`
  constant, and the `stdio:["ignore"]` site count. Label-only sightings already called out as such: `"mapToolCallStatus"`,
  `"_kiro/configuration/set":a(`, `handleDiagramRender(e){`, the camelCase `"preCompact"` strings (both are `a(…,"preCompact")`
  labels on the v2 triggers object; the real evidence is the `preCompact:` property and `trigger:"PreCompact"`).
- Conversely the keepNames labels make 0.66.91 navigable by intent: `a(X,"runCompleteCreditFields")`, `a(X,"sumCreditsByUnit")`,
  `a(X,"resolveClassifierMode")`, `a(X,"buildPermissionOptions")`, `a(X,"readProfilingOptIns")`, `a(X,"jobUpdateOf")`,
  `a(X,"classifyKnownExtMethod")` were the anchors used above; the next audit should grep the label, not the minified name.
- **Lane split confirmed with kas-workflow:** this lane owns the intake/gate semantics of the 0.66.91 AgentSettings keys
  `sessionTools{awareness,messaging,cloud}` and `universalBackgroundTool`; the `_kiro/session/notify` sender `"peer"` and the
  `workflowLaunch` configOption (0.66.58) are the workflow lane's. Intake detail for the two keys: each `sessionTools.*` toggle is
  a `NUe(wireKey, catalogSetting, recordField)` triple — `awareness` -> catalog `sessionToolsAwareness` -> session metadata
  `sessionAwarenessEnabled`; `messaging` -> `sessionToolsMessaging` -> `sessionMessagingEnabled`; `cloud` -> `sessionToolsCloud` ->
  `cloudSessionsEnabled`; `enabledIn(view)` is `catalog value === "on"`, recorded into the session record after load
  (`recordAfterLoad`). `universalBackgroundTool` is recorded as `Ien(jobs.enabled, universalBackgroundToolSetting)` (flag AND
  setting), and the `_kiro/job/*` methods are advertised only when the feature flag `universal_background_tool` (default false)
  resolves true — the setting alone does not expose the protocol. The TUI derives the setting from the `background_execution`
  rollout flag (`E_r=[["background_execution","universalBackgroundTool"]]` in `kiro-tui-3.1.0.js`), so on Kiro's own client it
  follows a backend gate; cyril marshals neither key (`crates/cyril-core/src/protocol/kas/settings.rs:77-97`). Both default off
  (catalog `sessionToolsMessaging` off; `universalBackgroundTool` absent -> `jobTools` undefined).
