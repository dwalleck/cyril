# KAS core lane — 0.66.26 (kiro-cli 2.28.0) → 0.66.33 (2.29.0 + 2.29.1)

Scope: auth providers / `--auth` options, the `_kiro/*` method set, `session_info_update`
kinds, sessionUpdate variants, capabilities, `_meta.kiro` / `.settings` schema keys,
`configOptions`, `KIRO_FEATURE_*`/`AB_*` flags, settings intake, telemetry defaults, memory,
MCP, hooks, terminal/background execution, configuration layers, cascade routing, and standing
coverage of every emitted kind against `crates/cyril-core/src/protocol/convert/kas.rs`.
Out of scope (sibling lanes): kas-workflow, tui, host, live. Binding: nothing from Kiro was
executed — every row is STATIC (carved `acp-server.js` + `.d.ts` only). Every add/remove is an
occurrence count in BOTH bundles plus quoted context; sets were diffed, never recounted.

Artifacts in this directory: `static-kas-surface-2.29.1.py`, `static-kas-session-info-kinds-2.29.1.py`,
`static-kas-settings-2.29.1.py`, `static-kas-flags-2.29.1.py`, `static-kas-identifiers-2.29.1.py`
(copies of the 2.28.0 scripts; the flags script's env-map regex was widened from `[\w]+` to
`[\w$]+` because the minifier renamed the flag-const map `Gn`→`$n` — without that fix it
misreported "0 env-reachable", a pure artefact). Raw outputs: `static-kas-surface.json`,
`raw-surface-diff.txt`, `raw-session-info-kinds.txt`, `raw-flags-2.28.0.txt`,
`raw-flags-2.29.1.txt`, `raw-settings-schema.txt`, `raw-auth-config-snippets.txt`.

## Summary

- **KAS 0.66.33 adds one session_info kind (`model_fallback`), one client→agent method
  (`_kiro/configuration/getState`), one client capability (`_meta.kiro.inactiveItems`), one
  agent capability (`_meta.kiro.modelFallbackReporting`), and five feature flags.** Nothing was
  removed from the `_kiro/*` method set or the sessionUpdate variant set.
- **Auth: no new `--auth` mode and no change to the advertised `authMethods`.** The real change is
  `KIRO_API_KEY` region discovery: `EnvAuthProvider` now carries a `regionSource` and routes the
  key to its home region; the wire contract (`_kiro/auth/getAccessToken`, `AuthIdentity`) is
  unchanged.
- **Model fallback is the headline dark feature**: `model_fallback` kind + `modelFallbackReporting`
  capability + `model_fallback_refusal` / `model_fallback_availability` flags +
  `server_cascade_config` (a server-pushed handoff config) + `vibeToDefaultV2Routing`.
- **MCP is in-lane and changed three ways** (all internal, no wire shape): stdio MCP servers now
  get the FULL parent env (old: a curated allow-list), subagents get their own MCP scopes, and the
  required-server predicate was generalised.
- **`@silvia-odwyer/photon-node` is client-visible**: it backs a new image-fitting pipeline that
  replaces unfittable images with a text placeholder instead of failing the request. This is a
  behaviour change, not a wire-shape change.
- **`api-key-region-probe.d.ts` is type-only** (a `.d.ts` for the new probe); it adds no wire
  field. `EnvAuthProvider.region` becomes a live getter instead of `undefined`.
- **Two AgentSettings keys were Removed (`subagentOrchestration`, `inlineAgents`)** and the
  `orchestrate_subagent` delegation tool is gone; cyril still sends `subagentOrchestration` and
  `todoList` — both now land as `configuration.intake.unknown-key` (DROPPED).
- **`KIRO_DISABLE_RECAP` is NOT removed** — its direct `process.env` read was refactored to a
  generic `process.env[killSwitchEnv]==="true"`; behaviour identical.
- Trap 5 note: `largeToolOutputHandler` does not exist in either bundle (0 hits); use
  `_meta.kiro.configurationState` / `.inactiveItems` or `settings.memory` as the positive control.
- Nothing from the 2.28.1 feed line is in KAS; the 2.29.1 MCP-OAuth-issuer line is host/V2.

## Deltas

"version" = release that first ships it; the OLD/NEW carve boundary is 0.66.26→0.66.33, which
folds 2.29.0 and 2.29.1 (KAS carves byte-identical between them — a KAS change would show in
both). Counts are N in 2.28.0 → 2.29.1. cyril refs are `file:line`; `engine.rs:97` is
`client_capabilities` (the whole outbound `_meta.kiro` surface) and `engine.rs:115` is the
`kiro.insert("settings", …)`, so a NOT-SENT capability/settings row is proven by that function;
`settings.rs:111`/`:116` are the only AgentSettings writers.

| item | version | gate/default | direction | STATIC/LIVE | cyril HANDLED/DROPPED/NOT-SENT + file:line | proposed issue? |
|---|---|---|---|---|---|---|
| `_kiro/configuration/getState` (client→agent) | 2.29.0 | advertised unconditionally in `initialize._meta.kiro.extensionMethods`; params `{observer:"connection"}` \| `{observer:"session",sessionId}` \| `{observer:"workspace",roots[]}` | ADD (0→8) | STATIC | NOT-SENT — no client call; unhandled ext method (outbound shape: `domain_mediator/commands/extensions.rs:262`) | new (KAS config layer read) |
| `clientCapabilities._meta.kiro.inactiveItems` | 2.29.0 | `===true` declares the client wants ALL resource items with `standing`; absent ⇒ `resourcesFor` keeps only `items.filter(n=>n.standing.kind==="active")` | ADD (0→2) | STATIC | NOT-SENT — `protocol/engine.rs:97` builds `_meta.kiro` with only `settings`+fs flags; cyril gets the active-only subset | new (capability opt-in) |
| `initialize` result `_meta.kiro.modelFallbackReporting:true` | 2.29.0 | agent capability, unconditional | ADD (0→1) | STATIC | DROPPED — cyril ignores initialize-result `_meta.kiro` extras (no reader in `protocol/`) | new |
| `session_info_update` kind `model_fallback` | 2.29.0 | emitted on fallback; payload `_meta.kiro={kind:"model_fallback", from, to?, scope?, trigger, reason?, agentSubtaskId?}` (+ persisted `abandonedMessageIds`) | ADD (kinds 24→25; token 0→8) | STATIC | DROPPED — `convert/kas.rs:525` `_ => None` | new (model-fallback surfacing) |
| flag `model_fallback_refusal` | 2.29.0 | default `false`; env `KIRO_FEATURE_MODEL_FALLBACK_REFUSAL_ENABLED` | ADD | STATIC | NOT-SENT (`protocol/engine.rs:97`) | new/observe |
| flag `model_fallback_availability` | 2.29.0 | default `false`; env `KIRO_FEATURE_MODEL_FALLBACK_AVAILABILITY_ENABLED` | ADD | STATIC | NOT-SENT (`protocol/engine.rs:97`) | new/observe |
| flag `server_cascade_config` | 2.29.0 | default `""`; no env; `lWc` precedence env > `server_cascade_config` > `defaults` | ADD | STATIC | NOT-SENT | new (cascade routing) |
| flag `vibe_to_default_v2_routing_enabled` | 2.29.0 | default `false`; AB `AB_VIBE_TO_DEFAULT_V2_ROUTING`; no env; hidden setting `vibeToDefaultV2Routing` | ADD | STATIC | NOT-SENT (`settings.rs:116`) | new/observe |
| flag `skill_name_qualifiers` | 2.29.0 | default `false`; env `KIRO_FEATURE_SKILL_NAME_QUALIFIERS_ENABLED`; AB `SKILL_NAME_QUALIFIERS` | ADD | STATIC | NOT-SENT | new/observe |
| `KIRO_API_KEY` home-region discovery | 2.29.0 | applies only on the apiKey path AND NOT (`--region` or (`--endpoint` AND `--control-plane-endpoint`)); probes `[us-east-1, eu-central-1, us-gov-east-1, us-gov-west-1]` via CPS `GetProfileCommand`, 10 s/region, retry cooldown 5 s–60 s | ADD (apiKeyRegion 0→5; regionSource 0→6) | STATIC | NOT-SENT — no wire field; agent-side routing. cyril spawns KAS without `--region`/`--endpoint` (`protocol/kas/discovery.rs`) ⇒ discovery active | new (document routing) |
| `EnvAuthProvider.region` getter + `resolveIdentity()` awaits region | 2.29.0 | `readonly region: string\|undefined` → `get region()`; identity read resolves it before the first request | CHANGE | STATIC | NOT-SENT (`protocol/kas/auth.rs` answers `getAccessToken` only) | fold into above |
| `--auth=machine` `--token-path`/default token path absolutised | 2.29.0 | `isAbsolute(t)?t:resolve(t)` around `DEFAULT_MACHINE_TOKEN_PATH="/tmp/access_token"` | CHANGE | STATIC | NOT-SENT (agent-side) | — |
| `.d.ts` growth: `AuthSelectionParams.apiKeyRegionSource?`, `EnvAuthProviderOptions.regionSource?`, `FileAuthProviderOptions.tokenPath` doc (lineage) | 2.29.0 | new optional fields | ADD | STATIC | NOT-SENT | — |
| advertised `authMethods` `[aws-builder-id, aws-iam-identity-center]` | — | unchanged (1:1) | — | STATIC | HANDLED (`protocol/kas/auth.rs`) | — |
| `--auth` mode set | — | `["user","machine","acp-callback"]`, identical parser both | — | STATIC | HANDLED (`protocol/kas/auth.rs`) | — |
| mux: `interaction_resolved` broadcast to EVERY live client (incl. `selectedOption` answer) + `resendOpenAsks` after `initialize` + `askOpened`/`askResolved`/`markAskResolved`/`sendToEveryLiveClient` | 2.29.0 | every connected client (not just subscribers); replayed updates (`_meta.kiro.replay===true`) exempt | ADD (askResolved 0→2, askOpened 0→4, resendAsks 0→5, `replay===!0` read 0→1) | STATIC | DROPPED — 0 grep hits for `interaction_resolved`/`selectedOption` in cyril; single-stdio client ⇒ broadcast==subscriber set | new (multi-client; cyril-5g2o/8lfs) |
| `pending_interaction` / `interaction_resolved` gain `consentRound` | 2.29.0 | optional field | ADD | STATIC | DROPPED (as above) | fold into above |
| multi-client config-state: `declaresInactiveItems`, `configurationStateFor(id,state)`, `handshakeOf(id)`, `resourcesFor(initialize,resources)`; push guard `this.observes(r,recipient)` | 2.29.0 | per-client handshake | CHANGE | STATIC | NOT-SENT (`protocol/engine.rs:97`) | fold into getState |
| AgentSettings: REMOVE `subagentOrchestration`, `inlineAgents`; ADD `vibeToDefaultV2Routing`; known-outside-schema list drops `subagentOrchestration` | 2.29.0 | schema `catchall(unknown)` | REMOVE (14→0, 11→0) / ADD (0→5) | STATIC | **DROPPED (cyril still sends)** — `protocol/kas/settings.rs:97` DEFAULTS_ON `subagentOrchestration` → `configuration.intake.unknown-key` | **new: prune settings intake** |
| AgentSettings `todoList` (removed upstream in 0.66.22) | ≤2.27 | not in schema, not in known-outside-schema | (already stale) | STATIC | **DROPPED (cyril still sends)** — `settings.rs:82` BOOL_MAP `chat.enableTodoList`→`todoList` | fold into above |
| delegation tool `orchestrate_subagent` (+ selector) removed; only `invoke_sub_agent` | 2.29.0 | `tne="invoke_sub_agent"` only (OLD: `Rrr=invoke_sub_agent, Irr=orchestrate_subagent` + `MUn` selector) | REMOVE (12→0) | STATIC | NOT-SENT (cyril sends no tool name) | fold into settings issue; stale doc `settings.rs:5,92` |
| MCP stdio env: allow-list → full parent env | 2.29.0 | OLD `o3r()` = curated `V9c` allow-list of `i3r.env`; NEW `Tlu()` = all of `process.env` (skip undefined / `"()"`), merged by `xlu` | CHANGE | STATIC | N/A — KAS owns MCP servers; cyril contributes nothing (`convert/kas.rs` has no `mcpServers`) | note (matches 2.29.0 changelog) |
| subagent MCP scopes: `acquireSubagentMcp`, `profileAgentServers`, `runtimeSessionOf`, `restateRuntimeServers`, `subagentMcpScopes` | 2.29.0 | per-subagent registry; OLD had `desiredServersForSession(s)` | ADD (all 0→N) | STATIC | N/A (KAS-internal) | note (matches "dispatched subagents load own mcpServers") |
| `requiredMcpServers(t)` → `requiredMcpServers(t, requiredSet)`; `clientMcpServers.some(waitForReady)` → `runtimeServers.clientRequiresReady(...)` | 2.29.0 | predicate generalised; abort race (`Promise.race([settleModel, mcpWait])`) already present in OLD | CHANGE | STATIC | N/A | note |
| tool-tag map: −`orchestrate_subagent`, −`searchMemories`; +`memory:"context"`, +`validate_spec_format:"fs_read"` | 2.29.0 | map used for hook `toolTags` | CHANGE | STATIC | HANDLED generically (`protocol/kas/hooks.rs`) | note only |
| remote tool alias `searchMemories` → `memory` | 2.29.0 | `$Ao="searchMemories"` → `"memory"` | RENAME (3→0) | STATIC | NOT-SENT | — |
| `ClientCustomAgent` frontmatter field `steeringAfterPromptBody` (boolean) | 2.29.0 | conditional spread; absent ⇒ unchanged | ADD (0→28) | STATIC | NOT-SENT — cyril never sends `customAgents` (0 grep hits in `crates/cyril-core/src`) | new (custom-agent field) |
| image fitting pipeline (external `@silvia-odwyer/photon-node`) | 2.29.0 | worker decode/resize/encode; `maxBase64Bytes` cap; placeholder on `no_vision`/`undecodable`/`unfittable` | ADD (photon 0→12, maxBase64Bytes 0→13, placeholder texts 0→3) | STATIC | NOT-SENT (agent-side; see evidence note) | new (image behaviour) |
| `_meta.kiro.replay===true` read (ask-resolution suppression on replay) | 2.29.0 | marker set in BOTH versions (`replay:!0`), only READ in NEW | ADD (read 0→1) | STATIC | NOT-SENT (cyril sets no such marker) | fold into multi-client |
| config-state zod keys: `configuration`/`envelope`/`manifest`/`stamp`/`versionId`/`hash`/`ino`/`mtimeMs`/`lastReadAt`/`fetchedAt`/`confirmedAt`/`generatedAt`/`identityKind`/`policy`/`roots`/`sweep`/`composeBehavior`/`includedFrom`/`enterpriseManagementStatus` | 2.29.0 | resources + control-plane envelope schema | ADD | STATIC | NOT-SENT (no config-state model; `convert/kas.rs:525` fallback) | fold into getState |
| compaction/summary keys `summaryKind`/`summaryUserRequests`/`noteUserRequests`/`agent_note` | 2.29.0 | persisted-message schema; carries user requests across compaction | ADD (15/28/5/…) | STATIC | DROPPED — no model (`convert/kas.rs:525`) | new (compaction carry) |
| `KIRO_DISABLE_RECAP` | 2.29.0 | direct `process.env.KIRO_DISABLE_RECAP` → generic `process.env[killSwitchEnv]==="true"` | **NOT A REMOVE** (env-read 1→0, literal 0→1; behaviour identical) | STATIC | NOT-SENT | none |
| `display_error.retryErrorType` / retry-message formatting | 2.29.0 | `retryErrorType` 75→75, `display_error` 10→10 | unchanged | STATIC | DROPPED (`convert/kas.rs:525`) | none |
| sessionUpdate variant set | — | literal `sessionUpdate:"…"` 1:1 both: `agent_message_chunk, available_commands_update, config_option_update, current_mode_update, session_info_update, tool_call, tool_call_update, user_message_chunk`; `agent_thought_chunk` bare literal 8→8 | unchanged | STATIC | HANDLED | — |
| `session/set_config_option` / `config_option_update` | — | unchanged (2→2, 5→5) | unchanged | STATIC | HANDLED | — |
| `configOptions` count | — | 21→21 | unchanged | STATIC | HANDLED | — |
| `_kiro/hooks/list` agent→client request params | — | `{trigger, sessionId, toolId?, toolTags?}` — `toolTags` present in BOTH | unchanged | STATIC | HANDLED (`protocol/kas/hooks.rs:587`) | — |
| `_kiro/memory/*` method set + memory schema (`mode: disabled\|read_only\|read_write`, `reflection`) + per-client `settings.memory`→`memorySettings` | — | unchanged (20→20; memorySettings 4→4) | unchanged | STATIC | HANDLED (memory schema unmodelled but not sent by cyril) | — |
| telemetry defaults | — | `telemetryEnabled`, `kutsEnabled`, `standaloneTelemetryConfig`, content-collection unchanged; `KIRO_DISABLE_TELEMETRY`/`telemetry.enabled` 0 hits both | unchanged | STATIC | HANDLED | — |
| background-execution env vars | — | `KIRO_BACKGROUND_STALL_SECONDS`/`KIRO_BACKGROUND_TIMEOUT_DEFAULT_SECONDS`/`KIRO_MONITOR_TIMEOUT_DEFAULT_SECONDS` all 1→1 | unchanged | STATIC | HANDLED/partial | — |
| MCP OAuth server-metadata schema (`issuer`, `authorization_response_iss_parameter_supported`) | — | identical modulo renames | unchanged | STATIC | HANDLED | none |

## Standing coverage — every emitted kind vs cyril

KAS 0.66.33 emits 25 `session_info_update` kinds (converter anchor: the brace-balanced `switch`
containing `case"pending_interaction":return{pendingInteraction:`). Cyril dispatch is
`session_info_to_notification` (`convert/kas.rs:352`); anything not matched falls to
`_ => None` at `convert/kas.rs:525` (DROPPED).

| kind | cyril | file:line |
|---|---|---|
| `turn_start` | HANDLED | `convert/kas.rs:397` |
| `turn_end` | HANDLED | `convert/kas.rs:398` |
| `turn_completion` | HANDLED | `convert/kas.rs:390` |
| `context_usage` | HANDLED | `convert/kas.rs:401` |
| `hook_update` | HANDLED | `convert/kas.rs:371` |
| `steering_queued` | HANDLED | `convert/kas.rs:464` |
| `steering_injected` | HANDLED | `convert/kas.rs:472` |
| `steering_cleared` | HANDLED | `convert/kas.rs:503` |
| `user_message_id_assigned` | DROPPED | `convert/kas.rs:525` |
| `focus_update` | DROPPED | `convert/kas.rs:525` |
| `steering_inclusion` | DROPPED | `convert/kas.rs:525` |
| `display_error` | DROPPED | `convert/kas.rs:525` |
| `summarization_started` | DROPPED | `convert/kas.rs:525` |
| `summarization_separator` | DROPPED | `convert/kas.rs:525` |
| `summarization_completed` | DROPPED | `convert/kas.rs:525` |
| `summarization_failed` | DROPPED | `convert/kas.rs:525` |
| `summary_message` | DROPPED | `convert/kas.rs:525` |
| `recap` | DROPPED | `convert/kas.rs:525` |
| `queued` | DROPPED | `convert/kas.rs:525` |
| `pending_interaction` | DROPPED | `convert/kas.rs:525` |
| `interaction_resolved` | DROPPED | `convert/kas.rs:525` |
| `checkpoint_restore_availability` | DROPPED | `convert/kas.rs:525` |
| `repositories_update` | DROPPED | `convert/kas.rs:525` |
| `model_routed` | DROPPED | `convert/kas.rs:525` |
| **`model_fallback` (NEW)** | **DROPPED** | **`convert/kas.rs:525`** |

`_kiro/*` ext methods/notifications (all 122 `_kiro/` literals in the 2.29.1 bundle checked
against `crates/cyril-core/src`; the acp crate strips the leading `_`, so cyril matches the
`kiro/` form). Fallback for unhandled notifications is `to_ext_notification` `_ => Ok(None)` at
`convert/kiro.rs:1284`. HANDLED rows cite the arm; incidental/comment-only hits are corrected.

**Agent→client notifications / pushes**

| method | cyril | file:line |
|---|---|---|
| `_kiro/error/rate_limit` | HANDLED | `convert/kiro.rs:1074` |
| `_kiro/system/notify` | HANDLED | `convert/kiro.rs:1085` |
| `_kiro.dev/metadata` (v2 family, shared arm) | HANDLED | `convert/kiro.rs:547` |
| `_kiro.dev/session/update` (steering echoes) | HANDLED | `convert/kiro.rs:957` |
| `_kiro.dev/mcp/server_init_failure` | HANDLED | `convert/kiro.rs:1103` |
| `_kiro.dev/mcp/oauth_request` | HANDLED | `convert/kiro.rs:1123` |
| `_kiro.dev/session/inbox_notification` | HANDLED | `convert/kiro.rs:1241` |
| `_kiro/powers/items_changed` | HANDLED | `convert/kas/powers.rs:48` |
| `_kiro/workflow/*` (run_start/run_complete/node_*/paused/steps_queued/watch_poll/loop_iteration) | HANDLED | `convert/kas/workflow.rs` |
| `_kiro/configuration/state` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/session/notify` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/sessions/changed` | DROPPED (test-only mention) | `domain_mediator/tests/serial.rs:38` |
| `_kiro/mcp/status`, `_kiro/mcp/governance_disabled` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/sandbox/status` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/policy/{changed,error}`, `_kiro/policy/ignore_files_changed` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/hooks/{cancel,didChange}` | HANDLED | `protocol/kas/hooks.rs:35,39` |
| `_kiro/spec/taskStatusChanged` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/steering/documents_changed` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/progressive_context/items_changed` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/customAgent/{not_found,config_error}` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/governance/state` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/safety/statusChanged`, `_kiro/safety/propertiesChanged` (Infrastructure Safety gate — can block tool calls) | DROPPED — 0 grep hits in `crates/cyril-core/src` (`kiro/safety` unhandled) | `convert/kiro.rs:1284` |
| `_kiro/tools/didChange`, `_kiro/code_references` | DROPPED | `convert/kiro.rs:1284` |
| `_kiro/c2s/view/changed`, `_kiro/knowledge/indexing*`, `_kiro/terminal/settings_changed`, `_kiro/terminal/write`, `_kiro/tools/content_chunk` | DROPPED | `convert/kiro.rs:1284` |

**Agent→client requests (host callbacks cyril must answer)**

| method | cyril | file:line |
|---|---|---|
| `_kiro/auth/getAccessToken` | HANDLED | `protocol/kas/auth.rs:24` |
| `_kiro/hooks/list` | HANDLED | `protocol/kas/hooks.rs:25` |
| `_kiro/hooks/executeHook` | HANDLED | `protocol/kas/hooks.rs:32` |
| `_kiro/hooks/sessionStart` | HANDLED | `protocol/kas/hooks.rs:42` |
| `_kiro/fs/{read_file,write_file,stat,read_directory,delete}` | HANDLED | `protocol/kas/kiro_fs.rs:96-100` |
| `_kiro/terminal/shell_type` | HANDLED | `protocol/kas/terminal_io.rs:612` |
| `_kiro/{userInput,mcp/elicitation,secret/*,openExternalUrl,tool/semantic_rename,tool/smart_relocate,tool/get_diagnostics,search/*}` | NOT-SENT — cyril never advertises these capabilities, so KAS never calls them (`protocol/engine.rs:97` inserts only `settings`+fs flags) | `protocol/engine.rs:97` |

**Client→agent requests (cyril callable, advertised in `extensionMethods`)**

| method | cyril | file:line |
|---|---|---|
| `_kiro/account/getUsage` | HANDLED | `domain_mediator/commands/extensions.rs:262` |
| `_kiro/session/context` | HANDLED | `convert/kas.rs` (context arm) |
| `_kiro/hooks/setEnabled` | HANDLED | `protocol/kas/hooks.rs:29` |
| `_kiro/workflow/*` (invoke/list/listRecipes/new/inspect/cancel/resume/load/retry/pause) | HANDLED | `commands/workflow.rs` + `convert/kas/workflow.rs` |
| `_kiro/configuration/getState` (NEW) | NOT-SENT | unhandled (outbound shape `domain_mediator/commands/extensions.rs:262`) |
| `_kiro/{session/delete,rename,compact,export,history,checkpoint/*,spec/*,policy/check,permissions/*,codeIntelligence,sourceProviders/*,mcp/resetServer,mcp/getPrompt,mcp/getResource,mcp/toggle,memory/*,sandbox/applyConfig,terminal/...}` | NOT-SENT | no call sites |

## Unreleased / dark candidates

Gate, default, evidence:

1. **Model fallback chain** — `model_fallback` session_info kind (0→8 tokens; converter arm
   `case"model_fallback":case"model_routed":return{}`), `_meta.kiro.modelFallbackReporting:true`
   capability, flags `model_fallback_refusal`/`model_fallback_availability` (both default `false`,
   env-reachable), `server_cascade_config` (default `""`), and reason literals
   `chainExhausted`/`noChainConfigured`/`winnerChanged`/`explicitPick`/`explicitPickDuringWrite`/
   `sameLayer`/`notEnabled`/`noServedTarget`/`staleActivityStatus`/`refusalFallback` with
   `source:server_cascade_config|defaults|env`. Evidence: `raw-surface-diff.txt`,
   `raw-auth-config-snippets.txt` (`VAt` payload, `lWc` source resolver).
2. **Server-pushed cascade/handoff config** — `lWc` resolves `envConfigJson` (env) >
   `abServerConfigJson` (`server_cascade_config`) > defaults. Default `""` ⇒ dormant unless the
   backend pushes it.
3. **Configuration layers / getState + inactiveItems** — `_kiro/configuration/getState` advertised
   in `extensionMethods`; `declaresInactiveItems` requires a literal `true`; `resourcesFor` filters
   to `standing.kind==="active"` otherwise; opt-in is per-client from that client's handshake.
4. **Vibe→V2-default routing** — hidden setting `vibeToDefaultV2Routing` / AB
   `vibe_to_default_v2_routing_enabled` (default false, **no env**):
   `Q0t(e,t){return e==="vibe"&&t.vibeToDefaultV2Routing?{modeId:RO,routedFrom:e}:{modeId:e}}`.
5. **Skill-name qualifiers** — `skill_name_qualifiers` (default false, env
   `KIRO_FEATURE_SKILL_NAME_QUALIFIERS_ENABLED`, AB `SKILL_NAME_QUALIFIERS`), wired to the session
   setting `skillNameQualifiers` alongside `validation`; no client-set path.
6. **Image fitting** — dark in the sense that it changes the failure mode silently; three
   placeholder texts are compiled in.

## Leads for the live lane

Each lead writes the full handshake path `initialize.clientCapabilities._meta.kiro.*` and pairs
a positive control.

- **L1 — `model_fallback` notification.** Method: `session/update` with
  `sessionUpdate:"session_info_update"`. Setup: any `session/prompt` that triggers a fallback
  (force by an unavailable/over-quota primary model, or flip
  `KIRO_FEATURE_MODEL_FALLBACK_AVAILABILITY_ENABLED=true` and
  `KIRO_FEATURE_MODEL_FALLBACK_REFUSAL_ENABLED=true`). Handshake: nothing (agent capability is
  advertised, not opted into). Expected: `_meta.kiro =
  {kind:"model_fallback", from, to?, scope?, trigger, reason?, agentSubtaskId?}`. Positive control:
  `turn_start`/`turn_end` in the same capture. cyril drops it (`convert/kas.rs:525`).
- **L2 — `_kiro/configuration/getState`.** Client→agent ext request
  `_kiro/configuration/getState`, params `{"observer":"connection"}`, or
  `{"observer":"session","sessionId":"<id>"}` after `session/new`. Handshake control: also send
  `initialize.clientCapabilities._meta.kiro.configurationState=true` to receive
  `_kiro/configuration/state` notifications and compare with the pull. Expected: a
  configuration-state object `{view, layers[], settings[], resources[]}`; with
  `_meta.kiro.inactiveItems=true` the `resources[].items[]` include non-`active` standing,
  otherwise only `standing.kind==="active"`. Control: without `configurationState` the push is
  absent while the request still answers.
- **L3 — multi-client interaction resolution.** Two stdio clients (or stdio + ws). Client A opens
  a `session/request_permission` (or `_kiro/userInput`); client B connects *after* the ask is
  raised. Expected: B receives the open ask right after its `initialize` (`resendOpenAsks`), and
  `interaction_resolved` (with `selectedOption`) reaches BOTH A and B regardless of subscription.
  Control: a plain `session_update` is NOT fanned to the non-subscribed client B. Requires
  `terminal`/fs advertisement to exercise a tool ask.
- **L4 — API-key region discovery.** Spawn with `KIRO_API_KEY` and NOTHING else. Expected log
  `auth.api_key_region.discovered {region:"…"}` (or `auth.api_key_region.not_found
  {probed:[…],inconclusive,cooldownMs}`). Control: re-spawn with `--region=us-east-1` and confirm
  discovery is skipped (`apiKeyRegionDiscoveryApplies` false). Purpose: prove a non-us-east-1 key
  authenticates.
- **L5 — image placeholder.** `session/prompt` with an image content block against a text-only
  `modelId`. Expected: NO request failure; instead a placeholder. Probe WHICH surface carries
  `[image omitted: the selected model does not read images]` (assistant text vs `_meta`), since
  the static lane could not prove whether it is streamed to the client or only replaces the
  model-side part. Control: same prompt against a vision model → `outcome:"unchanged"`.
- **L6 — cascade/handoff config precedence.** Flip `KIRO_FEATURE_CASCADE_CONFIG` (env JSON) vs a
  server-pushed `server_cascade_config`; confirm `handoff.config.resolved` logs
  `source` in `env` > `server_cascade_config` > `defaults`. Control: default run logs
  `source:"defaults"`.
- **L7 — settings intake unknown-key.** Send
  `initialize.clientCapabilities._meta.kiro.settings =
  {"subagentOrchestration":{"enabled":true},"todoList":{"enabled":true}}` (what cyril sends
  today) → expect two `configuration.intake.unknown-key` lines. Control: a recognized key
  (`{"thinking":{"enabled":true}}`) logs nothing.
- **L8 — MCP full-env inheritance.** Spawn KAS with a marker env var that is NOT on the old
  allow-list (e.g. `MY_MARKER=1`) and a stdio MCP server whose tool echoes `process.env`. Expected:
  the server sees `MY_MARKER` (NEW `Tlu()` = full `process.env`). Control: a variable on the old
  allow-list still behaves as before.

## Changelog reconciliation

`feed-2.29.1.json` (2.29.1 then 2.29.0) and `feed-2.28.1.json` (2.28.1 then 2.28.0). The 2.28.0
lines belong to the prior audit; listed for completeness.

**2.29.1** (KAS carve == 2.29.0, so a KAS-side fix would be identical in both):
- [fixed] MCP OAuth sign-in in classic and Rust V2 … issuer → **not in my lane.** The KAS MCP
  OAuth server-metadata schema (`issuer`, `authorization_response_iss_parameter_supported`) is
  1:1 between 2.28.0 and 2.29.1 modulo minifier renames (34/34, 3/3). The line names classic +
  Rust V2.

**2.29.0:**
- [added] [V3] `/memories` configures memory access and automatic updates for local sessions →
  **in my scope, no KAS wire change.** Memory schema (`mode`, `reflection`),
  `_kiro/memory/{list,get,create,update,delete}` (20→20), and per-client `settings.memory` →
  `memorySettings` (4→4) are all unchanged. `<memories>` in the bundle is only a prompt tag.
  `/memories` is a CLI/TUI command writing the same `settings.memory`.
- [changed] V2 TUI hook matchers such as `read` cover more tools → **not in my lane** (V2 TUI).
- [changed] V2 TUI tool hooks receive `tool_tags` → **not in my lane** (V2 TUI). KAS's
  `_kiro/hooks/list` already carried `toolTags` in 2.28.0.
- [changed] [V3] `kiro-cli acp` uses your CLI login unless you pass `--auth-method=client` →
  **not in my lane** (Rust launcher/host). KAS's `--auth` parser and `authMethods` are unchanged.
- [changed] [V3] Custom agents use their tool settings to decide which `mcp.json` servers to start
  → **in my lane, located.** `requiredMcpServers` gained a per-server required set
  (`requiredMcpServers(t,r)`), `clientMcpServers.some(waitForReady)` became
  `runtimeServers.clientRequiresReady(...)`, and `poolServersFor`/`restateRuntimeServers` replaced
  `desiredServersForSession(s)`. `includeMcpJson` gating (`Wgo`/`Ggo`) filters MCP tools by the
  agent's tool settings.
- [changed] [V3] Stdio MCP servers now inherit the full agent environment → **in my lane,
  located.** OLD `constructStdioTransport` built `env` from `o3r()` (a curated allow-list `V9c`
  read out of `i3r.env`); NEW uses `Tlu()` = all of `process.env`, merged by the new
  `xlu(...)`. (See L8.)
- [fixed] Classic `fs_write` no longer auto-approves symlink writes outside `allowedPaths` →
  **not in my lane** (Classic/V2).
- [fixed] Model fallback changes take effect in the current session without restarting → **in my
  lane, located** (L1).
- [fixed] MCP OAuth now shows the authorization URL … SSH or tmux → **in scope (MCP), no KAS
  delta found.** The `_kiro.dev/mcp/oauth_request` arm and its `oauthUrl` field are unchanged;
  the changelog points at the V2/TUI surface.
- [fixed] [V3] Non-interactive runs respect `telemetry.enabled` and `KIRO_DISABLE_TELEMETRY` →
  **not in my lane.** Both identifiers are 0 hits in the KAS bundle (2.28.0 and 2.29.1); KAS
  telemetry (`telemetryEnabled`, `kutsEnabled`, `standaloneTelemetryConfig`) is unchanged.
- [fixed] [V3] `/upgrade-agent` rewrites legacy hook matchers such as `fs_read` → **not in my
  lane** (TUI slash command; `agent upgrade`/`upgrade-agent`/`upgradeAgent` are 0 hits in KAS).
- [fixed] Knowledge search limits apply across all bases, with combined pagination and file-type
  filtering → **not in my lane** (knowledge index; the `knowledge` AgentSettings schema and
  `_kiro/knowledge` are unchanged).
- [fixed] [V3] Sessions no longer unexpectedly close under heavy load → **in my lane, no wire
  delta.** Candidate locus is the mux ask-lifecycle rework (`askOpened`/`askResolved`/
  `resendOpenAsks`); no method/field shape changed.
- [fixed] [V3] Custom agents dispatched as subagents now load their own `mcpServers` → **in my
  lane, located.** New `acquireSubagentMcp`, `profileAgentServers`, `runtimeSessionOf`,
  `restateRuntimeServers`, `subagentMcpScopes` (all 0 in 2.28.0).
- [fixed] [V3] Effort selector is offered at session start for an effort-capable default model →
  **not in my lane** (TUI).
- [fixed] [V3] Images are resized to fit the selected model instead of failing the request →
  **in my lane, located** (photon-node note below).
- [fixed] [V3] `KIRO_API_KEY` keys minted outside `us-east-1` now authenticate in their home
  region → **in my lane, located** (L4).
- [fixed] [V3] Readable retry message when the backend returns a non-JSON error page → **no KAS
  token delta found** (`retryErrorType` 75→75, `display_error` 10→10, `non-JSON` 2→2); the fix is
  in error formatting/SDK, not a wire shape.
- [fixed] [V3] Resumed sessions keep `UserPromptSubmit` hook output in the conversation → **in my
  lane, partial.** `_kiro/hooks/*` shapes are unchanged; the new compaction carry fields
  (`summaryKind`/`summaryUserRequests`/`noteUserRequests`/`agent_note`) are the likely mechanism.
- [fixed] [V3] Stop now cancels a prompt sent while MCP servers are still starting → **in my
  lane, no wire delta.** The prompt path already raced `settleSessionModel` against the MCP wait
  in 2.28.0; only the required-server predicate changed (`clientRequiresReady`).
- [fixed] [V3] A prompt sent just as a turn ends now includes that turn in the model's history →
  **no wire delta found** (turn lifecycle kinds unchanged).
- [fixed] [V3] Manual steering files now load even when their slash command appears mid-prompt →
  **no wire delta found** (steering kinds unchanged).

**2.28.1** (feed-only; one line, folds into 2.29.x binaries):
- [added] `kiro-cli agent upgrade <dir>` upgrades a directory of agent configs for V2 and V3 →
  **not in my lane.** `agent upgrade`/`upgrade-agent`/`upgradeAgent`/`agentUpgrade` are 0 hits in
  both KAS bundles; this is the host binary's subcommand (host lane).

## Method notes

- **Trap 3 (sets, not lists):** every category was extracted as a *set* of string/object-key
  literals from both bundles and diffed (`static-kas-surface-2.29.1.py`, 10 categories). The
  `_kiro/*` method set (121→122) and the `sessionUpdate` variant set are unchanged except
  `_kiro/configuration/getState`.
- **Trap 2 (count in both before add/remove):** every claimed add/remove has a count in both
  bundles and context in `raw-*.txt`. Three near-misses were falsified this way:
  `KIRO_DISABLE_RECAP` (env-read 1→0 but behaviour preserved via `killSwitchEnv`),
  `_meta.kiro.replay` (the *field* exists in both; only the *read* is new), and the flags
  script's false "0 env-reachable" (regex vs the renamed `$n` const map).
- **Minifier-rename artefacts caught:** the flag-const map `Gn`→`$n` broke the copied flags
  script's env-map regex (`[\w+\.\w+]` → `[\w$]+`), producing a false "0 env-reachable"; fixed
  and re-run (16 env-reachable in 2.29.1). `authMethods`, OAuth metadata, and tool ids compared
  after normalising renamed identifiers — all 1:1.
- **Trap 17 (HANDLED/DROPPED/NOT-SENT with a cyril line):** every row cites a cyril `file:line`.
  The provable fallbacks are `convert/kas.rs:525` (`session_info_to_notification` `_ => None`) and
  `convert/kiro.rs:1284` (`to_ext_notification` `_ => Ok(None)`); the "not sent" proof is
  `protocol/engine.rs:97` (`client_capabilities` — the complete outbound `_meta.kiro` surface)
  and `protocol/kas/settings.rs:111,116` (the only AgentSettings writers). Confirmed by grep:
  cyril has **0** hits for `interaction_resolved`, `pending_interaction`, `selectedOption`,
  `modelFallbackReporting`, `configurationState`, `customAgents`.
- **Trap 5 (handshake path + positive control):** all leads write
  `initialize.clientCapabilities._meta.kiro.*` and pair a control. `largeToolOutputHandler` is
  0 hits in both bundles; the connection-level controls that exist are `configurationState`,
  `inactiveItems`, and `settings.memory`.
- **Trap 4:** every row is STATIC; the live lane owns verdicts. L2/L3/L5/L8 have open questions
  the static lane could not settle (push vs pull; whether the image placeholder is streamed;
  which env vars a spawn actually inherits).

## Explicit asks

**Does `@silvia-odwyer/photon-node` change client-visible behaviour? — YES.**
Evidence: absent in 2.28.0, present in 2.29.1 (`photon` 0→12, `@silvia-odwyer` 0→1,
`maxBase64Bytes` 0→13, `"image omitted"` 0→3). `dist/server/acp-server.externals.json` gains
`"@silvia-odwyer/photon-node"`, and `package.json` moves it to a runtime dependency
(`"@silvia-odwyer/photon-node": "0.3.4"`). The worker (`D9c="@silvia-odwyer/photon-node"`,
`require(workerData.photonPath)`) is driven by `Kuo`/`tlo`, which runs over the model request's
images and returns
`{kind:"placeholder"|"image", outcome:"unchanged"|"resized"|"decoder_unavailable"|"placeholder_no_vision"|"placeholder_undecodable"|"placeholder_unfittable"}`.
Compiled-in placeholder strings:
`[image omitted: the selected model does not read images]`,
`[image omitted: could not be reduced to the selected model's size limit]`,
`[image omitted: could not be decoded]`. This is the changelog's "Images are resized to fit the
selected model instead of failing the request": the *failure mode* is replaced, so a prompt that
previously errored now succeeds with an omitted-image note. It is not a new wire method or field
(session/prompt's image content block is unchanged); it is an agent-side behaviour change whose
text may surface in the conversation. `sharp` moves to a devDependency (`^0.35.5`), so it is no
longer a runtime image path.

**Does `dist/server/api-key-region-probe.d.ts` change client-visible behaviour? — NO
(type-only), but it documents a behaviour change elsewhere.**
Evidence: it is a `.d.ts` (declaration-only, no runtime code) declaring
`GetProfileRegionProbeOptions`, `PROBE_TIMEOUT_MS = 10000`, `RoutingOverrides`,
`apiKeyRegionDiscoveryApplies(overrides)`, and `createGetProfileRegionProbe(options)`. There is no
corresponding change to any wire method, field, or capability — `_kiro/auth/getAccessToken`, its
response, and `authMethods` are untouched. The behaviour it describes (home-region routing for
`KIRO_API_KEY`) is real and observable only in that authentication succeeds; the wire surface is
unchanged.
