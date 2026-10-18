# v3-coverage lane — standing KAS-default parity inventory (kiro-cli 2.31.1, KAS 0.66.34, TUI 2.31.1)

Lane started 2026-10-14. Static only: nothing from Kiro was executed (no kiro-cli, node or bun), no
process that authenticates. Every row is **STATIC** unless it cites a frame from the live lane's
captures (read as data, `audit-2.31.1/live/*.jsonl`), which is marked **LIVE(live-lane)**.
This is a standing inventory, not a delta: it asks "what does cyril-on-KAS do today" for every
surface Kiro's engine emits/implements and Kiro's own TUI uses. Cyril HEAD = `5ee8bc07`.

Artifacts (`R` = repo root):
- ENGINE: `~/.local/share/kiro-research/kas-carves/2.31.1/node_modules/@kiro/agent/dist/server/acp-server.js` (0.66.34; "KAS")
- REFERENCE CLIENT: `~/.local/share/kiro-research/tui-bundles/kiro-tui-2.31.1.js` ("TUI")
- CYRIL: `R/crates/` @ 5ee8bc07. File:line below are relative to `crates/cyril-core/src/` unless prefixed.
- Scripts (new, `R/experiments/conductor-spike/`): `static-v3cov-ctx-2.31.1.py` (count+quote any literal in KAS/TUI, `--old` adds 0.66.33/TUI 2.29.1),
  `static-v3cov-kinds-2.31.1.py` (session_info kinds × KAS/TUI/cyril), `static-v3cov-methods-2.31.1.py` (direction-classify all 122 `_kiro/*` literals × TUI × cyril),
  `static-v3cov-issues-2.31.1.py` (rivets grep). Raw outputs: `v3-coverage/raw/{kinds.json,methods.json,handshake-kas-quotes.txt,tui-slash-contexts.txt,tui-slash-handlers.txt}`.
- Counts are `str.count` of the quoted literal in the 0.66.34 / TUI 2.31.1 bundle unless noted (trap 2: where a claim is add/remove, the 0.66.33 count is given too).

Verdict vocabulary (trap 17): **HANDLED**, **DROPPED** (received, ignored — with whether it is logged), **NOT-SENT**
(cyril never sends/advertises), **NOT-IMPLEMENTED** (cyril has no responder/route), **BROKEN** (cyril routes it, but to a
method/shape the engine does not implement). Flip column: **BLOCKS-FLIP** / **FOLLOW-UP** / **NONE**.

## Summary

1. This is a standing inventory of Kiro's engine (KAS 0.66.34), Kiro's own client (TUI 2.31.1) and cyril HEAD 5ee8bc07, covering about 180 matrix rows across 9 matrices. All rows are STATIC except about 10 confirmed from live-lane frames. The KAS core turn (chunks, tool calls, turn_start/turn_end, metering, context %, steering, rate limit, workflows, the fs/terminal/auth/hooks host role) is **HANDLED**. Most of what is missing is UI around the turn.
2. **Flip blockers: three P1 and one P2:**
   - Sessions die ~1 h after the last kiro-cli token refresh, because cyril never renews the token (cyril-taba/zbu7).
   - Every skill, saved-prompt and steering slash command fails: cyril dispatches them as `_kiro.dev/commands/execute`, which KAS rejects with -32603 (cyril-oieu, which is mis-scoped as "future").
   - There is no way to change the model, the agent/mode or the effort. v2 got these from Kiro's agent commands; KAS has no such commands, and nothing in cyril sends `set_config_option` for these options (NEW).
   - (P2) Sub-agent output is mixed into the main transcript (0 `agentSubtaskId` handling; cyril-ebqu, whose premise is stale).
3. **P2 parity gaps:**
   - The rest of the v2 slash surface (`/context`, `/compact`, `/tools`, `/mcp`, `/knowledge`, `/code`, `/rewind`, …) has `_kiro/*` equivalents the TUI already uses, but cyril has none.
   - Compaction, model fallback, MCP status/OAuth and org governance are all received and dropped.
4. `session_info_update`: cyril handles 8 of 25 kinds. The TUI consumes 15. The 17 drops are **unlogged** (`convert/kas.rs:525` → `inbound.rs:145`).
5. Handshake: cyril still sends the dead keys `subagentOrchestration` and `todoList`, plus `toolSearch.enabled`, which nothing consumes since 0.66.34. The **`orchestrate_subagent` tool was removed in KAS 0.66.33**, so CLAUDE.md's pipeline section is stale. The TUI also sends dead keys (`largeToolOutputHandler`, `workflowNotifications`, `authCallback`).
6. Two corrections to repo lore:
   - Cyril no longer filters `ToolKind::Other` (`cyril-ui/src/state.rs:536-558`).
   - `turn_end.stopReason:"error"` is real (LIVE frame 171) and cyril maps it to EndTurn.


## Matrix 1 — Handshake: `initialize.clientCapabilities` (`_meta.kiro`, `_meta.kiro.settings`, `fs._meta.kiro`)

### 1a. KAS-consumed `_meta.kiro` keys (non-settings) vs TUI vs cyril

KAS readers: `QHr(clientCapabilities)` (resolved caps, quote `secretStorage:n.secretStorage===!0,openExternalUrl:n.openExternalUrl===!0,knowledge:n.knowledge===!0,infrastructureSafety:…,c2sViews:…,textSearch:…,findFiles:…,streamingShellContent:n.streamingShellContent===!0`),
`initialize()` (`typeof r?.telemetryEnabled=="boolean"`, `r?.telemetry`, `r?.settings`, `r?.helpDocs`, `r?.hooks?.enabled` / `c.v2===!0`),
agent context (`specLinks:r?.specLinks,specPhaseCheckpoints:r?.specPhaseCheckpoints,requirementsAnalysis:r?.requirementsAnalysis,userInput:r?.userInput`),
`tYr` (`"configurationState"in t&&t.configurationState===!0`), `kMu` (`"inactiveItems"in t&&t.inactiveItems===!0`),
`this.clientMeta?.notifications?.documentsChanged!==!1`, `this.clientMeta?.resolvesSteeringCommands!==!0`, terminal host `{terminalInput:_?.terminalInput===!0}`,
client tools `i5r(...)`: `r[i.metaKey]===!0` over `clientToolGetDiagnostics|clientToolSemanticRename|clientToolSmartRelocate`.
TUI sends `$tt()` = `{telemetryEnabled:t,...t&&{telemetry:uy()},knowledge:!0,hooks:{enabled:!0,v2:!0},requirementsAnalysis:!0,specPhaseCheckpoints:!0,streamingShellContent:!0,...KIRO_INFRA_SAFETY_ROLLOUT_ENABLED&&{infrastructureSafety:!0},...e&&{settings:e}}`
plus capability-registered keys `authCallback`, `userInput`, `openExternalUrl`, `secretStorage` (each `{type:"other",key:…,value:!0}` merged into `_meta.kiro`), and `clientInfo:{name:"kiro-cli",…}`.
Cyril builds `engine::client_capabilities` (`protocol/engine.rs:97-136`): `fs{readTextFile,writeTextFile,_meta.kiro{readFile,writeFile,stat,readDirectory,delete}}`, `terminal:true`, `_meta.kiro.settings`, `_meta.kiro.hooks` (`{enabled}` in default Host mode, `{enabled,v2}` in Kas mode, absent when Off); `clientInfo.name=kiro-cli` (`protocol/bridge.rs:426-431`, cyril-df5l closed).

| key | KAS consumes (static) | TUI 2.31.1 sends | cyril | user-visible consequence in cyril-on-KAS | issue | flip |
|---|---|---|---|---|---|---|
| `settings` | yes (`this.configuration.stateStartup(HB(r?.settings))`) | yes (`oT()`) | HANDLED `protocol/engine.rs:114-116` + `protocol/kas/settings.rs:182-188` | see 1b | cyril-53qx | — |
| `hooks{enabled,v2?}` | yes (`r?.hooks?.enabled`, `c.v2===!0`) | `{enabled:!0,v2:!0}` (KAS runs hooks) | HANDLED `{enabled}` (cyril runs hooks, Host default `types/kas_hooks.rs:49`) / `{enabled,v2}` opt-in `protocol/engine.rs:117-130` | parity by design (cyril is the hooks host) | — | NONE |
| `telemetryEnabled`, `telemetry` | yes | yes (telemetry identity when enabled) | NOT-SENT (`protocol/engine.rs:97-136` has no key) | KAS falls back to its own telemetry default; no client opt-out signal | cyril-zjm5 (2.28.0) | FOLLOW-UP P3 |
| `knowledge` | yes (`knowledge:n.knowledge===!0`) | `!0` | NOT-SENT | knowledge-base capability not advertised (settings `knowledge:{enabled}` IS sent) | — | FOLLOW-UP P4 |
| `requirementsAnalysis`, `specPhaseCheckpoints`, `specLinks` | yes (agent context) | `requirementsAnalysis:!0, specPhaseCheckpoints:!0` | NOT-SENT | spec-mode interactive requirements analysis / phase checkpoints off; cyril has no spec UI anyway | — | NONE (no spec UI) |
| `streamingShellContent` | yes | `!0` | NOT-SENT | no live shell-output streaming (`_kiro/tools/content_chunk` never sent) | cyril-2mo0 | FOLLOW-UP P3 |
| `userInput` (capability key) | yes (`userInputCapability=t.userInput===!0`) | yes (`key:"userInput"` → `_kiro/userInput`) | NOT-SENT | user_input questions WITH options degrade to `session/request_permission` with every option `kind:"allow_once"` (KAS `handleUserInputViaPermissionRequest`); questions with NO options are **auto-skipped** (`"…the client lacks the userInput capability; advancing to next phase…"`) — only reachable in delegated children/spec agents (memory ref user_input reachability) | cyril-qo13 (closed, option choice) | FOLLOW-UP P3 |
| `secretStorage` | yes (`new s$t(this.connection)`) | yes (3 methods) | NOT-SENT | remote-MCP OAuth credentials are not persisted (no `credentialStorage`) → sign-in per session | — | FOLLOW-UP P3 (with MCP OAuth row, Matrix 5) |
| `openExternalUrl` | yes (`onOpenExternalUrl` installed only if advertised) | yes (copies URL to clipboard + notice) | NOT-SENT | remote-MCP OAuth never opens a browser (`this._phase!=="passive"&&this.onOpenExternalUrl`) — the URL only exists in KAS logs | — | FOLLOW-UP P2 (MCP OAuth, Matrix 5) |
| `authCallback` | **0** hits in KAS (`'authCallback': 0`) | yes (`key:"authCallback"`) | NOT-SENT | none — KAS selects callback auth from argv `--auth=acp-callback`, which cyril passes (`protocol/kas/discovery.rs:356`) | — | NONE |
| `infrastructureSafety` | yes | env-gated (`KIRO_INFRA_SAFETY_ROLLOUT_ENABLED==="1"`) | NOT-SENT | `_kiro/safety/*` gate never armed for cyril | cyril-3ald | NONE (dark in TUI too) |
| `configurationState` | yes (push `_kiro/configuration/state`) | **0** in TUI | NOT-SENT | none (TUI parity) | cyril-iq2c | NONE |
| `inactiveItems` | yes | 0 in TUI | NOT-SENT | none | cyril-iq2c | NONE |
| `terminalInput` | yes (client-hosted terminals) | 0 in TUI (TUI does not host terminals) | NOT-SENT while cyril DOES host terminals (`terminal:true`) | interactive/background processes start with `interactive:"no-terminal-input-capability"`; agent cannot write stdin (`"This client does not support writing to terminal stdin (no terminalInput capability)."`) | cyril-7p7n (2.28.0) | FOLLOW-UP P3 |
| `c2sViews`, `textSearch`, `findFiles`, `helpDocs`, `notifications.documentsChanged`, `resolvesSteeringCommands`, `clientTool*` | yes | none | NOT-SENT | none (IDE-only surfaces; TUI parity) | — | NONE |
| `fs._meta.kiro.{readFile,writeFile,stat,readDirectory,delete}` + `fs.{read,write}TextFile` + `terminal` | yes (`kiroFsReadFile:r.readFile===!0`…, `terminal:e?.terminal===!0`) | **none** (TUI lets KAS run fs/shell in-process) | HANDLED `protocol/engine.rs:101-110` | cyril is the fs/terminal host (design choice; KAS-5). Divergence from the reference client is deliberate | — | NONE |

### 1b. `_meta.kiro.settings` keys — KAS 0.66.34 zod `G2n` (33 keys) + known-extras `bMu` vs TUI `oT()` vs cyril `marshal_agent_settings`

KAS schema quote (`kas/settings.txt` last block): `G2n=k.object({memory:…,_parallelTasks,_steeringReminders,_sessionRecap,_requirementAnalyzer,_c2s,_quickSpec,_subagent,_delegate,thinking,tangentMode,disableAutoCompaction,codeIntelligence,checkpoint,semanticReview,fta,goal,workflows,backgroundExecution,unifiedAgent,vibeToDefaultV2Routing,validation,cascade,specPlan,steeringSupervisor,infraSafetyMonitor,infraSafetyEnforce,_providerPowers,toolSearch:JXs,knowledge:QXs,sessionEviction:tZs,compaction:eZs,terminal:z2n})`;
extras `bMu=["c2s","verifyFirstWorkflow","sessionRecap","steeringReminders","memoryEnable","userMemoryOptIn"]`; anything else → `S.debug("configuration.intake.unknown-key",{key,layer})` (`IMu`, at `KIRO_LOG_LEVEL=debug`).
`JXs=zPe({minPct,minTokens,neverDefer})` where `zPe(e)=k.object({...e,enabled:k.boolean()})` — `enabled` is **schema-required** but its 0.66.33 consumer `B=!q&&cR(oe,"toolSearch")` is gone (kas lane, `"toolSearch"` 2→1); only `Xe=new Set(oe.toolSearch?.neverDefer??[])` reads it.

| key | KAS 0.66.34 | TUI sends | cyril sends (`protocol/kas/settings.rs`) | consequence | issue | flip |
|---|---|---|---|---|---|---|
| `thinking`,`knowledge`(+ext),`codeIntelligence`,`checkpoint`,`tangentMode`,`disableAutoCompaction`,`_subagent`,`_delegate`,`compaction` | accepted | yes | HANDLED `:78-88`, `:93-98`, `:133-179` | parity | — | NONE |
| `todoList` | **unknown-key** (0 in 0.66.33 and 0.66.34) | no | SENT when `chat.enableTodoList` is boolean (`:82`) | dead key in every such handshake (debug-logged unknown) | cyril-53qx | FOLLOW-UP P2 |
| `subagentOrchestration` | **unknown-key**; 0 hits in 0.66.33/0.66.34 vs 14 in 0.66.26 (2.28.0) — and the `orchestrate_subagent` tool itself is gone (0 vs 21 in 2.28.0: `OrchestrateSubAgent:{name:"orchestrate_subagent",description:"Tracks orchestrate sub-agent (DAG pipeline) too…`) | no | SENT ALWAYS (`DEFAULTS_ON` `:93-98`) | dead key in every handshake; the doc comment (`:90-92` "selects the `orchestrate_subagent` tool") and CLAUDE.md's KAS pipeline section describe a tool that no longer ships | cyril-53qx | FOLLOW-UP P2 |
| `toolSearch.enabled` (+`minPct`,`minTokens`) | accepted, **unconsumed** (`minPct/minTokens` never consumed in either version) | yes | SENT when `toolSearch.enabled` boolean (`:120-131`) | user's toolSearch toggle silently does nothing; MCP deferral is unconditional in 0.66.34 | cyril-53qx, cyril-q8gq, cyril-ea67 | FOLLOW-UP P2 |
| `toolSearch.neverDefer` | consumed | no | NOT-SENT | no way to keep an MCP tool eager (always behind `tool_load`) | cyril-53qx | FOLLOW-UP P3 |
| `memory` `{mode,reflection}` | consumed (`c.memorySettings=…d.memory`); **default flipped to `{mode:"disabled",reflection:false}` in 0.66.34** | yes, always | NOT-SENT | KAS native memory OFF for every cyril session — LIVE(live-lane): `session/new _meta.memoryConfig` OLD `{read_write,true}` → NEW `{disabled,false}` | cyril-0na7 | FOLLOW-UP P3 (cyril has its own cyril-memory; decision needed, not a loss vs v2) |
| `memoryEnable`, `userMemoryOptIn` | known-extras | rollout-gated / computed | NOT-SENT | none beyond the row above | cyril-0na7, cyril-q8gq | NONE |
| `workflows`,`goal` | accepted | when available | NOT-SENT by design (ADR-0011; fenced `protocol/kas/settings.rs:466-487`) | cyril owns `/workflow` natively; KAS workflow commands suppressed | cyril-0asq | NONE |
| `backgroundExecution`, `c2s`, `infraSafety*` | accepted / extra | rollout / env-gated | NOT-SENT | none (dark in TUI) | — | NONE |
| `largeToolOutputHandler`, `workflowNotifications` | **unknown-key** (0 hits in KAS 0.66.33 and 0.66.34) | yes (`largeToolOutputHandler:!0` default; `t.workflowNotifications={enabled,delivery}`) | NOT-SENT | none — the TUI sends dead keys too (trap 5's control example is stale, as the 2.29.1 audit found) | — | NONE |
| `semanticReview`,`fta`,`validation`,`cascade`,`specPlan`,`steeringSupervisor`,`sessionEviction`,`terminal{commandTimeoutMs}`,`_parallelTasks`… | accepted | no | NOT-SENT | KAS defaults (`semanticReviewEnabled:true`, `ftaEnabled:false` on LIVE(live-lane) `session/new _meta`) | cyril-pjw7 (supervisor) | NONE |

## Matrix 2 — Agent capabilities consumed (`initialize` result)

KAS 0.66.34 advertises (LIVE(live-lane) `kas-live-sweep-2.31.1.jsonl` frame 6): `loadSession:true`, `promptCapabilities{image,embeddedContext}`,
`mcpCapabilities{http,sse}`, `sessionCapabilities{list,close,delete,fork{_meta.kiro.messageId}}` (no `resume`), `_meta.kiro{checkpoints, sessionList,
policyNotifications, extensionMethods[28], sessionSources[local], sessionListScopes[workspace], executionTargets[local], sourceProviders:false,
replayMarking:true, modelFallbackReporting:true, logging}`. TUI parser `zrt(...)` reads 7 keys (`…extensionMethods:n(t.extensionMethods),sourceProviders:…,strictSessionLoad:…,replayMarking:…`) plus
`sessionCapabilities?.close` (`supportsSessionClose(){return this.initializeResponse?.agentCapabilities?.sessionCapabilities?.close!=null}`).
Cyril reads only the presence of an `agentCapabilities._meta.kiro` object for engine fingerprinting (`protocol/fingerprint.rs:28-36`); every key below has 0 non-test hits in `crates/`.

| key | KAS | TUI reads (count) | cyril | consequence | issue | flip |
|---|---|---|---|---|---|---|
| `extensionMethods` | 28 methods incl. `_kiro/config/template`, `_kiro/codeIntelligence`, workflow family | yes (9) — gates `/code` status (`if(e.length>0&&!e.includes("_kiro/codeIntelligence"))return`), workflow control (`Xtt(this.kiroCapabilities.extensionMethods)`), pre-session `config/template` | DROPPED (fixture only `protocol/fingerprint.rs:138`) | cannot feature-detect; cyril hard-codes the workflow family | cyril-tikf | FOLLOW-UP P3 |
| `replayMarking` | true | yes (15) — `agentMarksReplay()` gates permission settle on `interaction_resolved` and replay buffering | DROPPED | replayed frames on `session/load` are not distinguishable; no permission settle | cyril-99ds | FOLLOW-UP P3 |
| `modelFallbackReporting` | true | **0** (TUI consumes `model_fallback` unconditionally) | DROPPED | none by itself (the kind is the gap, Matrix 4) | cyril-aoby | NONE |
| `sessionSources`, `sessionListScopes`, `executionTargets`, `sourceProviders` | local-only values | yes (cloud-session gating) | DROPPED | none until cloud sessions | cyril-tikf | NONE |
| `strictSessionLoad` | **0** in KAS | yes (5) | — | none (TUI forward-compat) | — | NONE |
| `checkpoints`, `sessionList`, `policyNotifications`, `logging` | yes | 0 | DROPPED | none (TUI parity) | — | NONE |
| `sessionCapabilities.{list,close,delete,fork}` | yes | close (`supportsSessionClose`), list (`.listSessions(` ×8), fork (`unstable_forkSession`) | DROPPED (never read) | see Matrix 9 | cyril-wigv | FOLLOW-UP (Matrix 9) |
| `promptCapabilities.{image,embeddedContext}` | true/true | SDK default parse | DROPPED | none observed (cyril sends text prompts) | — | NONE |

## Matrix 3 — Client→agent requests

KAS dispatch = 56 literal `case"_kiro/…"` arms (`static-v3cov-methods-2.31.1.py`, dir `C2A`) + ACP methods `initialize, authenticate, session/{new,load,prompt,cancel,close,delete,fork,list,resume,set_mode,set_config_option}, _session/steer, _session/steer/clear` (kas lane `acp_methods`, 25). An unknown `_`-method is **rejected, not -32601**: LIVE(live-lane) frame 146 `-32603 … "[PersistenceClassification] Ext method \"_kiro/session/setWorkflowNotificationDelivery\" has no persistence classification…"`; frames 155/157/159 the same for `session/set_model`.
TUI flow = how the reference client reaches it (slash handler table `Eit`/`aSn` and the KAS `executeCommand` emulator `ort`, Matrix 7).

| method | TUI calls it from | cyril | consequence on default engine | issue | flip |
|---|---|---|---|---|---|
| `initialize`, `session/new`, `session/prompt`, `session/cancel` | always | HANDLED `domain_mediator/commands/session.rs:118,174,256` | — | — | NONE |
| `session/load` | `/chat` load, session dashboard | HANDLED `/load <id>` `commands/builtin.rs:322-332` → `session.rs:174` | no list to pick from (see `session/list`) | cyril-rtrh | — |
| `session/set_config_option` | `/model` (`setConfigOption("model",t)`), `/agent` + `/plan` (`"mode"`), `/effort` (`"effortLevel"`), `/memories` (`"memoryReflection"`), sandbox (`configId:"sandbox"`,`"mcpSandboxing"`), autopilot, thinking | HANDLED wire (`session.rs:349-420`) but only two producers: `/thinking` (`commands/builtin.rs:576`) and the reviewer (`R/crates/cyril-workbench/src/reviewer.rs:184-187`) | **no way to change model, agent/mode or effort on the default engine** | cyril-cxwb, cyril-lxuo, cyril-838u, cyril-lyrx (all kas-4) | **BLOCKS-FLIP** |
| `session/set_mode` | `setSessionMode(` ×8 | producer = reviewer only | as above | — | (folded above) |
| `session/list` | `/chat`, `/sessions` dashboard (`kiroClient.listSessions(a)` with `_meta.kiro{sessionSource,listScope}`) | NOT-SENT | user must know a session id to `/load`; v2 cyril has no list either (parity with v2, gap vs Kiro) | cyril-wigv | FOLLOW-UP P3 |
| `session/fork` | `/rewind` (`sendExtMethod("session/fork",{sessionId,cwd,_meta:{kiro:{messageId,createdReason:"rewind"}}})`) | NOT-SENT | no rewind (v2 had the agent `/rewind` command) | NEW (rewind) | FOLLOW-UP P3 |
| `session/close` | closes finished replayed workflow-step sessions (`this.runtime.closeSession(e)`) | NOT-SENT | step sessions cyril loads stay resident in KAS until exit | — | FOLLOW-UP P4 |
| `session/delete`, `_kiro/session/delete`, `_kiro/session/rename` | session dashboard | NOT-SENT | no session management | — | FOLLOW-UP P4 |
| `_session/steer`, `_session/steer/clear` | composer while busy | HANDLED `domain_mediator/commands/subagents.rs:115,139` | — | — | NONE |
| `_kiro/account/getUsage` | `/usage` | HANDLED `domain_mediator/commands/extensions.rs:262` (`/usage` dashboard) | — | — | NONE |
| `_kiro/session/context` | `/context [show|add|remove|clear]` | NOT-SENT (test-only near-miss) | no `/context` (v2 agent command lost) — the live bar still works from `context_usage` | cyril-1116 | FOLLOW-UP P2 |
| `_kiro/session/compact` | `/compact` | NOT-SENT | no manual compaction on the default engine (v2 `/compact` lost) | NEW | FOLLOW-UP P2 |
| `_kiro/session/history` | transcript paging after load | NOT-SENT | older history of a loaded session not pageable | cyril-a3th | FOLLOW-UP P3 |
| `_kiro/codeIntelligence` | `/code [status|init|overview]` + status indicator | NOT-SENT | v2 `/code` lost | NEW (folded into slash-surface issue) | FOLLOW-UP P3 |
| `_kiro/knowledge` | `/knowledge` | NOT-SENT | v2 `/knowledge` lost | NEW (folded) | FOLLOW-UP P3 |
| `_kiro/hooks/list`, `_kiro/hooks/setEnabled` | `/hooks` (`trigger:"all",includeDisabled:!0`) | HANDLED `domain_mediator/commands/kas.rs:91,131` — registered only in `kas_hooks="kas"` mode (`commands/mod.rs:98`); default Host mode answers hooks itself | — (LIVE(live-lane) frame 122: `"_kiro/hooks/list is not available when v2Hooks is disabled"` confirms the gate) | cyril-5nep | NONE |
| `_kiro/mcp/resetServer` | `/mcp` panel retry / re-auth | NOT-SENT | no MCP retry/re-auth (see Matrix 5 MCP OAuth) | cyril-nk4o | FOLLOW-UP P2 |
| `_kiro/memory/list`, `_kiro/memory/get` | `/memories` | NOT-SENT (cyril has its own `/memory`, cyril-memory) | none vs v2 (KAS memory is default-off in 0.66.34; LIVE frame 108 `"Memory is not enabled."`) | cyril-0na7 | NONE |
| `_kiro/config/template` | pre-session agents/output-style (gated `managed_settings` + `extensionMethods`) | NOT-SENT | no pre-session model/mode pickers | cyril-lyrx | FOLLOW-UP P3 |
| `_kiro/sandbox/applyConfig` | `/sandbox`, local `/autonomous` (`local_sandbox` gate) | NOT-SENT | no sandbox/autonomous | cyril-6vo6, cyril-be7e | FOLLOW-UP P3 |
| `_kiro/spec/resolveSession`, `_kiro/spec/invoke` | `/spec` | NOT-SENT | no spec workflow UI (spec mode still selectable — if mode switching existed) | — | FOLLOW-UP P4 |
| `_kiro/sourceProviders/{list,listResources}` | `/repo` (cloud) | NOT-SENT | cloud only | cyril-tikf | NONE |
| `_kiro/workflow/{new,invoke,load,inspect,list,listRecipes,cancel,resume,retry}` | `/workflow` | HANDLED `domain_mediator/commands/kas.rs:308-350` | — | — | NONE |
| `_kiro/workflow/pause` | `/workflow` | NOT-SENT | cannot pause a run | cyril-4u4a | FOLLOW-UP P3 |
| `_kiro/powers/refresh`, `_kiro/policy/ignore_files_changed` (client→agent **notifications**: `e.sendExtNotification("_kiro/powers/refresh",{})`) | powers panel / `.kiroignore` edit | NOT-SENT (refresh deliberately: `convert/kas/powers.rs:29-31`) | `.kiroignore` edits not hot-applied | — | NONE |
| TUI-0 C2A methods: `_kiro/{checkpoint/*,configuration/{getState,contribute},environment/start_ssh_session,hooks/triggerHook,mcp/{getPrompt,getResource,toggle},memory/{create,update,delete},permissions/*,policy/check,powers/list,safety/getProperties,session/{export,list},spec/getTaskStatuses,terminal/settings_changed,workflow/{delete,update,resumeAll,listWatchHandlers}}` | not called by TUI | NOT-SENT | none (TUI parity) | cyril-iq2c, cyril-tikf | NONE |
| **v2-dialect methods cyril still sends on KAS**: `_kiro.dev/commands/execute`, `_kiro.dev/commands/options` (every advertised command + picker confirm `R/crates/cyril/src/app.rs:2007-2015`), `_session/spawn`, `_kiro.dev/session/terminate`, `_message/send` (`/spawn`,`/kill`,`/msg`, `domain_mediator/commands/subagents.rs:30,60,87`) | — (KAS: `commands/execute` 0 hits, `session/spawn` 0, `session/terminate` 0, `message/send` 0) | **BROKEN** — sent, engine rejects with `-32603 PersistenceClassification` (LIVE shape, frame 146) | see Matrix 7 rows S1/S2 | cyril-oieu | **BLOCKS-FLIP** (S1) |

## Matrix 4 — Agent→client: `sessionUpdate` variants, `session_info_update` kinds, `_kiro/*` notifications

### 4a. `sessionUpdate` variants

KAS emit sites (literal census `sessionUpdate:"x"`): `session_info_update` 3, `tool_call` 2, `tool_call_update` 2, `user_message_chunk` 9, `available_commands_update` 2,
`config_option_update` 4, `agent_message_chunk` 3, `current_mode_update` 1, plus `agent_thought_chunk` via `sessionUpdate:t.streamKind==="say"?"agent_message_chunk":"agent_thought_chunk"`.
`plan`, `plan_update`, `plan_removed`, `usage_update` appear **only** in the bundled ACP SDK schema union (1 hit each: `Gds.and(Re({sessionUpdate:Yt("plan")})),Xds.and(Re({sessionUpdate:Yt("plan_update")}))…sps.and(Re({sessionUpdate:Yt("usage_update")}))`) — no emitter.
TUI `Det` map: `{user_message_chunk:QQn,agent_message_chunk:NQn,agent_thought_chunk:MQn,tool_call:OQn,tool_call_update:LQn,available_commands_update:PQn,session_info_update:jee,config_option_update:Kee,plan:Kee,usage_update:Kee,current_mode_update:UQn}`.
Cyril: `protocol/convert/mod.rs:430-528` + KAS override `protocol/engine.rs:323-343`; unknown tags fenced to a debug drop (`protocol/client.rs:40-52`, `protocol/domain_mediator/mod.rs:465-476`).

| variant | KAS emits | TUI | cyril | consequence | issue | flip |
|---|---|---|---|---|---|---|
| `agent_message_chunk`, `agent_thought_chunk`, `user_message_chunk` | yes | yes | HANDLED `convert/mod.rs:440,450,430` | — but chunks tagged `_meta.kiro.agentSubtaskId` (sub-agent output, `…_meta:{kiro:{...o,...n,...i&&{agentSubtaskId:i}}}`) are rendered **inline in the main transcript**; TUI routes them by subtask (`function GY(e,t,n,r,a){if(n?.agentSubtaskId){…`). cyril: `agentSubtaskId` 0 hits in `crates/` | sub-agent text/thoughts interleave with the main agent's, unattributed | cyril-ebqu, cyril-fjfu | **BLOCKS-FLIP** (with 4a tool_call row) |
| `tool_call`, `tool_call_update` | yes | yes (GY routes `tool_call`/`tool_call_update`/`content`/`thought` carrying `agentSubtaskId` to a sub-agent lane) | HANDLED `convert/mod.rs:459-461`. Sub-agent runs arrive as `kind:"other"` (`title:\`Sub-agent: ${r.subAgentName}\`…_meta:{kiro:{…kind:"agent-subtask",agentSubtaskId:r.subSessionId}}`). **Correction to CLAUDE.md / cyril-ebqu's premise:** cyril no longer filters `ToolKind::Other` — `UiState` pushes every `ToolCallStarted` (`R/crates/cyril-ui/src/state.rs:536-558`, no kind check) and the chat widget renders Other by title (`R/crates/cyril-ui/src/widgets/chat.rs:353`) | the `Sub-agent: X` row shows, but every child tool call and text chunk is rendered as if the main agent did it; no grouping, no crew panel | cyril-ebqu, cyril-fjfu | **BLOCKS-FLIP** |
| `tool_call` for MCP tools under 0.66.34 unconditional deferral | rawInput `{tool_id, arguments}` (TUI 2.31.0 unwraps; tui lane T11) | unwraps | HANDLED generically — shows the wrapper | every MCP call shows the loader wrapper, not the real tool/args (STATIC; live L3) | cyril-2yn8 | FOLLOW-UP P2 |
| `available_commands_update` | yes (skills, saved prompts, steering) | splits by `_meta.kiro.type` into prompts/skills/steering | HANDLED → `CommandsUpdated`, `_meta` dropped (`convert/mod.rs:491-510`) and registered as dispatchable agent commands (`commands/mod.rs:449-469`) | see Matrix 7 S1 (dispatch BROKEN) | cyril-79ox, cyril-oieu | **BLOCKS-FLIP** (S1) |
| `current_mode_update` | yes | ignored (`current_mode_update:UQn` → `null`) | HANDLED `convert/mod.rs:485` (`ModeChanged`) | — | — | NONE |
| `config_option_update` | yes | config tracker | HANDLED `convert/mod.rs:488` — only `key=="model"` read (`session.rs:284-289`, `R/crates/cyril-ui/src/state.rs:1023-1046`) + thinking | effort/mode/memory/autopilot option values not shown | cyril-lxuo, cyril-838u | FOLLOW-UP (Matrix 6) |
| `session_info_update` | yes | `jee` | HANDLED 8/25 kinds (4b) | — | cyril-58uv | — |
| `plan`, `plan_update`, `plan_removed`, `usage_update` | schema only | `Kee` (not mapped) | `plan`/`usage_update` HANDLED; `plan_update`/`plan_removed` would hit the unknown-tag fence (not in `KNOWN_SESSION_UPDATE_TAGS`) | none today (no emitter) | — | NONE |

### 4b. `session_info_update._meta.kiro.kind` — 25 kinds (KAS 0.66.33 = 0.66.34) + TUI-only `model_fallback_promoted`

Source `raw/kinds.json` (`static-v3cov-kinds-2.31.1.py`). Cyril dispatch `protocol/convert/kas.rs:352-526`; anything else → `_ => None` at `convert/kas.rs:525`, then `Ok(false)` in `protocol/domain_mediator/inbound.rs:145-146` with **no log** (the ext-notification path logs at `inbound.rs:208`, this one does not).
TUI consumer `jee` (quoted in method notes); kinds it does not name fall to `z.debug("KAS session update (not yet mapped):",…)`.
LIVE(live-lane) presence on one ordinary 3-turn session (`kas-live-sweep-2.31.1.jsonl`): `context_usage` 11, `focus_update` 11, `user_message_id_assigned` 3, `turn_start/turn_completion/turn_end` 3 each, `pending_interaction` 1, `interaction_resolved` 1, `display_error` 1.

| kind | KAS literal (0.66.33→0.66.34) | TUI consumes | cyril | consequence | issue | flip |
|---|---|---|---|---|---|---|
| `turn_start` | 22→22 | yes (`turn_start` side effect) | HANDLED `convert/kas.rs:397` | — | — | NONE |
| `turn_end` | 17→17 | yes (+`executionId` from `messageId` suffix `-turn-end`) | HANDLED `convert/kas.rs:398` — but `stopReason:"error"` (LIVE frame 171 `{"turnEnd":{"stopReason":"error"},"kind":"turn_end","stopReason":"error",…}`) is not an ACP `StopReason`: warn + **defaults to EndTurn** (`convert/kas.rs:717-734`). The prompt-response error still surfaces as `BridgeError` (`domain_mediator/commands/session.rs:499-510`; LIVE frame 173 `-32000 InvalidModelError`) | failed turn recorded as a normal end_turn in the turn summary | NEW (small) | FOLLOW-UP P3 |
| `turn_completion` | 6→6 | yes (`turn_summary` metering + contextUsagePercentage) | HANDLED `convert/kas.rs:390` | — | cyril-79df | NONE |
| `context_usage` | 8→8 | yes (+`breakdown`) | HANDLED `convert/kas.rs:401` | — | cyril-1116 | NONE |
| `steering_queued` / `steering_injected` / `steering_cleared` | 2/2/2 | yes (`RQn`) | HANDLED `convert/kas.rs:464,472,503` | — | — | NONE |
| `hook_update` | 4→4 | **no** (0) — TUI runs hooks in KAS (`v2:!0`) | HANDLED `convert/kas.rs:371` | — | — | NONE |
| `model_fallback` | 3→3 (`ma({kind:"model_fallback",...tIt(t)})`) | yes since 2.30.0 (`if(fet(r)){for(let a of bet(r))t.emitSideEffect(a)…}`) | DROPPED, unlogged | silent model change; stale model badge | cyril-aoby (raise P3→P2, retriage R3) | FOLLOW-UP P2 |
| `model_fallback_promoted` | **0→0** | yes (forward-compat) | DROPPED | none today | cyril-aoby | NONE |
| `summarization_started` / `_completed` / `_failed` | 3/4/4 | yes → `compaction_status` | DROPPED, unlogged | auto-compaction on the default engine is invisible (v2 `kiro.dev/compaction/status` was HANDLED `convert/kiro.rs:779`) | NEW | FOLLOW-UP P2 |
| `summarization_separator`, `summary_message`, `recap` | 3/4/6 | no | DROPPED | none (TUI parity) | — | NONE |
| `display_error` | 10→10 (`kind:"display_error",message,errorType,…retryErrorType`) incl. MCP connect failures (`` `MCP server "${r}" failed to connect: ${n}` `` broadcast to every session) | yes, as the error text of the next failed tool call (`t.pendingDisplayErrors.set(…)`) | DROPPED, unlogged | in-turn model errors are duplicated by the prompt error (covered); **MCP connection / OAuth-needed errors are lost** | cyril-58uv | FOLLOW-UP P2 (MCP visibility bundle) |
| `user_message_id_assigned` | 3→3 | yes (`kas_message_id_assigned`, rewind anchor) | DROPPED | blocks any `/rewind` (`session/fork` needs `messageId`) | NEW (rewind) | FOLLOW-UP P3 |
| `interaction_resolved` | 13→13 | yes (`gNn`: settles the open permission dialog when not `replay`) | DROPPED | a permission resolved elsewhere (multi-client, cancel) leaves cyril's dialog open | cyril-99ds | FOLLOW-UP P3 |
| `pending_interaction` | 11→11 | no (uses `session/request_permission`) | DROPPED | none (TUI parity) | — | NONE |
| `focus_update` | 5→5 (`title`, `status`, `activity{turnActive,runningWorkflows,pausedWorkflows}`) | no | DROPPED | none vs TUI; session title/activity unavailable | cyril-otpi, cyril-0o7e | NONE |
| `model_routed` | 3→3 | **no** (0) | DROPPED | none vs TUI | cyril-ojum | NONE |
| `checkpoint_restore_availability` | 2→2 | no | DROPPED | none vs TUI | cyril-fb1m | NONE |
| `repositories_update` | 2→2 | yes, via the `repositories` key (`if("repositories"in(r??{}))…session_repositories_update`) | DROPPED | cloud sessions only | — | NONE |
| `steering_inclusion` | 9→9 | no | DROPPED | none | — | NONE |
| `queued` | 37→37 (shared literal) | no | DROPPED | none | — | NONE |

### 4c. `_kiro/*` agent→client notifications (KAS `extNotification("…")` 22 + table-emitted 17)

Source `raw/methods.json`. Cyril: KAS ext notifications go workflow adapter → powers adapter → `convert::kiro::to_ext_notification` (`protocol/engine.rs:345-371`), whose fallback `_ => Ok(None)` (`convert/kiro.rs:1284`) is debug-logged `"extension notification not forwarded"` (`domain_mediator/inbound.rs:207-209`).

| notification | TUI consumer | cyril | consequence | issue | flip |
|---|---|---|---|---|---|
| `_kiro/error/rate_limit` | `handleRateLimitError` | HANDLED `convert/kiro.rs:1074` | — | — | NONE |
| `_kiro/system/notify` | **none** (0) | HANDLED `convert/kiro.rs:1085` | — (cyril ahead of TUI) | cyril-0w3u, cyril-fvht | NONE |
| `_kiro/powers/items_changed` | yes | HANDLED `convert/kas/powers.rs:48` | — | — | NONE |
| `_kiro/hooks/cancel`, `_kiro/hooks/didChange` | didChange yes | HANDLED `protocol/kas/hooks.rs:35,39` | — | — | NONE |
| `_kiro/workflow/{run_start,node_start,node_complete,node_paused,loop_iteration,watch_poll,paused,run_complete,steps_queued}` | yes | HANDLED `convert/kas/workflow.rs:481-489` | — | — | NONE |
| `_kiro/workflow/node_failed` | no | DROPPED (not in KAS's emitter map `Jou`; only in a recognizer set) | none | — | NONE |
| `_kiro/workflow/recipes_changed` | yes | DROPPED | recipe list stale until re-run | cyril-4u4a | FOLLOW-UP P4 |
| `_kiro/mcp/status` | yes (MCP panel + tool list, `handleMcpStatusNotification`) | DROPPED | no MCP server state (connecting/failed/auth-needed) on the default engine; v2 had `kiro.dev/mcp/server_init_failure`/`server_initialized` HANDLED (`convert/kiro.rs:1103,1146`) | cyril-nk4o | FOLLOW-UP P2 |
| `_kiro/mcp/governance_disabled` | yes | DROPPED | no notice when org disables MCP | cyril-nk4o, cyril-bmek | FOLLOW-UP P3 |
| `_kiro/governance/state` | yes (`managed_settings` gate; legacy `X1e` webToolsEnabled) — pushed unsolicited at `session/new` (LIVE frame 17) | DROPPED | enterprise users lose MCP/web tools without explanation | cyril-0o7e (raise P3→P2, retriage R5) | FOLLOW-UP P2 |
| `_kiro/customAgent/not_found` | yes (alert + `agent_switched` to fallback) | DROPPED (v2 twin `kiro.dev/agent/not_found` HANDLED `convert/kiro.rs:1158`) | a missing custom agent silently falls back | NEW (small) | FOLLOW-UP P3 |
| `_kiro/customAgent/config_error` | no (0) | DROPPED (v2 twin HANDLED `convert/kiro.rs:1178`) | none vs TUI; regression vs cyril-v2 | NEW (same) | FOLLOW-UP P3 |
| `_kiro/steering/documents_changed` | yes | DROPPED | no steering catalog | cyril-58uv | FOLLOW-UP P4 |
| `_kiro/tools/didChange` | yes (`tools_update` → `/tools` panel) | DROPPED | no tool list | cyril-58uv | FOLLOW-UP P3 |
| `_kiro/tools/content_chunk` | yes | not received (cyril does not advertise `streamingShellContent`) | no live shell streaming | cyril-2mo0 | FOLLOW-UP P3 |
| `_kiro/sessions/changed` | yes (roster/cloud status) | DROPPED — densest frame (LIVE: 12 in one 3-turn session) | none locally | cyril-58uv | NONE |
| `_kiro/sandbox/status` | yes (`local_sandbox`) | DROPPED | none until sandbox | cyril-6vo6 | NONE |
| `_kiro/spec/taskStatusChanged`, `_kiro/spec/phaseCheckpoint` | yes | DROPPED | none (no spec UI) | cyril-mxc7 | NONE |
| `_kiro/session/notify` | no (0) | DROPPED | none vs TUI (step relay) | cyril-fb1m | NONE |
| `_kiro/progressive_context/items_changed`, `_kiro/policy/{changed,error}`, `_kiro/code_references`, `_kiro/knowledge/indexing{Started,Completed}`, `_kiro/safety/{statusChanged,propertiesChanged}`, `_kiro/c2s/view/changed`, `_kiro/configuration/state` | no (0) | DROPPED / never sent (gated caps) | none (TUI parity) | cyril-3ald, cyril-iq2c, cyril-45ld | NONE |

## Matrix 5 — Host callbacks KAS calls into the client

Cyril responders: typed ACP handlers (`protocol/client.rs:159-272`) + the `_`-request dispatcher (`protocol/domain_mediator/host.rs:45-213`).
**Any other `_`-request is answered with JSON `null`** (`host.rs:188-192`: `"unhandled extension request answered with protocol-default null"`), not an error.
TUI responders: `capabilities:[getAccessToken(), userInput(…), copyUrlToClipboard(…), ...secretStorage()]` (`key:"authCallback"|"userInput"|"openExternalUrl"|"secretStorage"`); the TUI advertises **no** fs/terminal, so KAS runs file I/O and shell in-process for it.

| callback | gate in KAS | KAS calls (static) | TUI implements | cyril | consequence | issue | flip |
|---|---|---|---|---|---|---|---|
| `_kiro/auth/getAccessToken` | argv `--auth=acp-callback` | `Nh(t,"_kiro/auth/getAccessToken",{})` | yes | HANDLED `protocol/kas/auth.rs:24,300` (LIVE frame: 1× at init) | — | — | NONE |
| `fs/read_text_file`, `fs/write_text_file` | `fs.{read,write}TextFile` | yes | no (not advertised) | HANDLED `protocol/client.rs:161-190` (LIVE frame `fs/read_text_file` ×1) | — (deliberate host role) | — | NONE |
| `_kiro/fs/{read_file,write_file,stat,read_directory,delete}` | `fs._meta.kiro.*` | `extMethod("_kiro/fs/…")` | no | HANDLED `protocol/kas/kiro_fs.rs:85-100` | — | — | NONE |
| `terminal/{create,output,wait_for_exit,release,kill}`, `_kiro/terminal/shell_type` | `terminal:true` | yes | no | HANDLED `protocol/client.rs:191-272`, `protocol/kas/terminal_io.rs:612` (LIVE create/wait/output/release ×1) | — | — | NONE |
| `_kiro/terminal/write` | `_meta.kiro.terminalInput` | `Nh(this.connection,"_kiro/terminal/write",{sessionId,terminalId,input})` | no | NOT-IMPLEMENTED (not advertised → never called) | no stdin for background/interactive processes cyril hosts | cyril-7p7n | FOLLOW-UP P3 |
| `_kiro/hooks/{list,executeHook,sessionStart}` + notifications `cancel`/`didChange` | `_meta.kiro.hooks.enabled` (not `v2`) | `Nh(t,"_kiro/hooks/executeHook",…)`, `Nh(e,"_kiro/hooks/sessionStart",…)` | **no** — TUI sends `hooks:{enabled,v2:!0}` so KAS runs hooks itself | HANDLED (Host default) `protocol/kas/hooks.rs:25-42`, `callbacks.rs:545-550` | — (cyril is the hooks host by design) | cyril-5nep | NONE |
| `_kiro/userInput` | `_meta.kiro.userInput` | `Nh(this.connection,"_kiro/userInput",{sessionId,toolCallId,question,options})` | yes | NOT-SENT → KAS falls back to `session/request_permission` (options→`allow_once`) or **auto-advances option-less questions** | delegated children / spec agents cannot ask free-form questions | cyril-qo13 (closed) | FOLLOW-UP P3 |
| `_kiro/secret/{get,store,delete}` | `_meta.kiro.secretStorage` | `Nh(this.connection,"_kiro/secret/get",…)` | yes (`secrets.json`, migrates into KIRO_HOME in 2.31.0) | NOT-SENT | remote-MCP OAuth tokens not persisted across sessions | — | FOLLOW-UP P3 |
| `_kiro/openExternalUrl` | `_meta.kiro.openExternalUrl` | `Nh(this.connection,"_kiro/openExternalUrl",{url:c})` | yes (clipboard + link notice) | NOT-SENT | remote-MCP OAuth (`redirectToAuthorization` → `this.onOpenExternalUrl` only when installed) never reaches the user; with `_kiro/mcp/status` and `display_error` dropped (Matrix 4) an OAuth MCP server just never connects, silently | cyril-nk4o (no dedicated issue) | FOLLOW-UP P2 (NEW bundle) |
| `_kiro/mcp/elicitation` | **ungated** (`acpElicitationHandler:Nr` on every MCP tool wrapper) | `Nh(t,"_kiro/mcp/elicitation",{sessionId,toolCallId,elicitation})` | **no handler** (0 literal hits; TUI `extMethod` throws `No handler registered for method`) | answered `null` → KAS reads `Mi.action` on null → TypeError caught → `{action:"cancel"}` | MCP elicitations always cancel — same outcome as the TUI | — | NONE |
| `_kiro/workspace/{active_file,currently_open_files}` | ungated (context provider) | `e.extMethod("_kiro/workspace/active_file",{sessionId})` | no handler | answered `null` (`j9u(null)` → undefined; `"paths"in null` throws for open files) | `#currentlyOpenFiles` context errors — TUI also errors | — | NONE |
| `_kiro/tool/{get_diagnostics,semantic_rename,smart_relocate}` | `_meta.kiro.clientTool*` | yes | no | NOT-SENT | none (IDE tools) | — | NONE |
| `_kiro/search/{find_files,text_search}` | `_meta.kiro.{findFiles,textSearch}` | `extMethod(...)` | no | NOT-SENT | none (KAS searches in-process) | — | NONE |
| `session/request_permission` | always | yes; `_meta.kiro{toolId,command,consent{capability,resource,askType,…}}`, `consentRound` | yes (+ settles on `interaction_resolved`) | HANDLED (generic path; KAS consent read `convert/kas.rs:24-60`; `rejectionReason` reply `convert/kas.rs:68-95`) | `consentRound` unread (0 hits) — the 20× allow-always loop is fixed engine-side (LIVE(live-lane) leg 2a: every ask `consentRound:1`); outcome `_meta.kiro.{consent.scope,editedCommand}` not sent — TUI sends neither (0 hits) | cyril-w55l, cyril-gn07, cyril-99ds | FOLLOW-UP P3 (re-verify/close w55l) |

## Matrix 6 — configOptions / modes / models / effort

KAS offers (LIVE(live-lane) `kas-live-sweep-2.31.1.jsonl`): `session/new` (frame 26) `configOptions = mode(vibe|spec|quick-spec|bug-fix|plan|autonomous|semantic_reviewer) · model(auto + 20) · memoryReflection(off) · autopilot(on) · contentCollection(disabled)`, `modes.availableModes[7]` with `_meta.kiro.source`, **no `models` key** (`models null`); pre-session `_kiro/config/template` (frame 9) adds `outputStyle`; `effortLevel` / `thinking` appear only for models that support them (TUI `configId:"effortLevel"` ×3, `"thinking"` ×2). `set_config_option model=<bogus>` is **accepted unvalidated** (frames 160-162) and the next prompt fails `InvalidModelError` (frame 173) — the KAS twin of v2's cyril-fj6j.
TUI setters: `setConfigOption("mode",…)` ×8, `("model",…)` ×2, `("effortLevel",…)`, `("memoryReflection",…)`; `configId:` `autopilot` ×2, `mcpSandboxing` ×2, `sandbox`, `sandboxNetworkMode`, `thinking` ×2.
Cyril: stores `ConfigOptionsUpdated`/`ConfigOptionSet`, reads only `key=="model"` for the toolbar (`session.rs:284-289`, `R/crates/cyril-ui/src/state.rs:1023-1046`), mode from `modes`/`current_mode_update` (`convert/mod.rs:485`), thinking lever (`commands/builtin.rs:549-576`, cyril-k3lz closed).

| option | KAS | TUI surface | cyril | consequence on default engine | issue | flip |
|---|---|---|---|---|---|---|
| `model` | select, 21 values | `/model` picker (rate multiplier, effort pane) → `setConfigOption("model",t)` with post-check | DISPLAYED (toolbar) / **NOT-SETTABLE** (no producer of `SetConfigOption{model}` outside the reviewer) | user cannot change model; v2's `/model` agent command is gone on KAS (Matrix 7) | cyril-cxwb, cyril-lxuo, cyril-fj6j (guard) | **BLOCKS-FLIP** |
| `mode` (agent) | select, 7 bundled + custom agents | `/agent`, `/plan`, `/autonomous` → `setConfigOption("mode",r)` | DISPLAYED / NOT-SETTABLE | user cannot switch to spec/plan/bug-fix/custom agents; v2 `/agent` lost | cyril-lyrx | **BLOCKS-FLIP** |
| `effortLevel` | model-dependent | `/effort` → `setConfigOption("effortLevel",t)` | DROPPED (not read) / NOT-SETTABLE | no effort badge, no effort change; v2 `/effort` lost | cyril-838u, cyril-lxuo | FOLLOW-UP P2 |
| `thinking` | model-dependent | `/model` settings pane | HANDLED `/thinking on|off` | — | cyril-k3lz | NONE |
| `memoryReflection` | `off` default in 0.66.34 | `/memories` | DROPPED / NOT-SETTABLE | none (KAS memory off; cyril-memory is cyril's path) | cyril-0na7 | NONE |
| `autopilot` | `on` default | toggle (`configId:"autopilot"`) | DROPPED / NOT-SETTABLE | user cannot turn KAS autopilot off (it stays on, the KAS default) | NEW (fold into the model/mode picker issue) | FOLLOW-UP P3 |
| `contentCollection` | `disabled` | governance-driven | DROPPED | none | — | NONE |
| `outputStyle` | pre-session template only | `/verbosity` + prompt `_meta.kiro.outputStyle` | NOT-SENT | none | — | NONE |
| `sandbox`, `mcpSandboxing`, `sandboxNetworkMode` | only with local sandbox | `/sandbox`, `/autonomous` | NOT-SENT | no sandbox | cyril-6vo6 | NONE (opt-in, P3) |

## Matrix 7 — Slash commands (TUI 2.31.1: 74 literals; 37 registered, routed by `aSn={"/help":"handler",…,"/memories":"handler"}`)

The KAS-mode TUI routes each registered command either to a local handler table `Eit={"/chat":L8,…,"/stats":wit}` or ("backend") through `executeCommand`, which on KAS is the emulator
`ort(e,t){switch(n){case"quit"…case"usage":{…callExtMethod("_kiro/account/getUsage")}case"context":{…contextShow()}…case"compact":{…callExtMethod("_kiro/session/compact",…)}case"rewind":{…this.fork({messageId:a,createdReason:"rewind"})}case"code":{…}default:return{success:!1,message:\`/${n} is not yet supported in KAS mode\`}}`.
The other 37 literals are the telemetry command-id enum `cj` (`nt.Agent="/agent";nt.Autonomous="/autonomous";nt.Changelog="/changelog";…`) plus a few TUI-local intercepts (`/quit|/exit` `lPn`, `/sandbox` for `agentEngine==="kas"`, `/theme`→"moved to /settings theme", `/settings`, `/config`, `/verbosity`, `/switch`, `/kiro`).
Cyril on KAS registers builtins `help clear quit(q,exit) new load steer voice theme usage powers memory thinking sessions spawn kill msg` (`commands/mod.rs:424-439`) + `hooks` (kas_hooks=kas only), `workflow`, `review` (`:406-412`), plus every KAS-advertised skill/prompt/steering name as an `AgentCommand` (`:449-469`).
On v2 cyril additionally had Kiro's 24 agent commands (`agent chat clear code compact context effort feedback goal guide help hooks knowledge mcp model paste plan prompts quit reply rewind stats tools usage`, memory ref KAS command surface, 2.14.2 live A/B) via `kiro.dev/commands/execute`; **KAS advertises none of them**.

### 7a. Cyril breakages on KAS (BROKEN routes)

| id | what | evidence | consequence | issue | flip |
|---|---|---|---|---|---|
| S1 | KAS skills, saved prompts and steering docs arrive in `available_commands_update`, are registered as `AgentCommand` (`is_local=false`, `CommandInfo::new(…,false,false)` `convert/mod.rs:496-503`), and `/name args` dispatches `BridgeCommand::ExecuteCommand` (`commands/mod.rs:534-541`) → `_kiro.dev/commands/execute` (`domain_mediator/commands/extensions.rs:30,190-198`). KAS has `commands/execute` 0 hits and rejects unclassified ext methods (`throw new Error(\`[PersistenceClassification] Ext method "${e}" has no persistence classification…\`)`; LIVE shape frame 146). The TUI instead treats `type` prompt/skill/steering as resources (`oZ=(e)=>e.meta?.type==="prompt"||e.meta?.type==="skill"||e.meta?.type==="steering"`) that KAS expands from the prompt text | STATIC (cyril) + LIVE shape | **every skill / saved-prompt / steering slash command errors** (`BridgeError` "-32603 Internal error") on the default engine; on v2 they worked through `commands/execute` | cyril-oieu (P3, scoped to "future" commands — it is present-day), cyril-79ox | **BLOCKS-FLIP** (P1: regression on the default path) |
| S2 | `/spawn`, `/kill`, `/msg` (registered on every engine) send `_session/spawn`, `_kiro.dev/session/terminate`, `_message/send` (`domain_mediator/commands/subagents.rs:30,60,87`); KAS 0 hits for all three. `/sessions` reads the v2 `SubagentTracker`, which KAS never feeds (`kiro.dev/subagent/list_update` is v2-only) | STATIC | three commands error, `/sessions` always empty | NEW | FOLLOW-UP P3 (hide on KAS) |
| S3 | picker confirm always sends `ExecuteCommand{value}` (`R/crates/cyril/src/app.rs:2007-2015`); on KAS no picker source exists | STATIC | none today (no picker can open on KAS except `/theme`, which is local) | — | NONE |

### 7b. Command-by-command (registered TUI commands + the ones the brief names)

`K:` = KAS method the TUI uses; `L` = TUI-local; `v2` = cyril-v2 had it via `commands/execute`.

| command | TUI route on KAS | cyril on KAS | gap |
|---|---|---|---|
| `/help` | L | builtin | — |
| `/clear` | backend → `executeClear` = **new session** (`frt`: `this.newSession(e)` "Conversation cleared") | builtin "Clear the chat" (local only); `/new` = `session/new` | semantic difference only |
| `/quit`, `/exit` | L | builtin (`q`,`exit` aliases) | — |
| `/model` | handler `rit` → K: `session/set_config_option model` | **missing** (v2) | BLOCKS-FLIP (Matrix 6) |
| `/effort` | handler `sit` → K: `set_config_option effortLevel` | **missing** (v2) | FOLLOW-UP P2 |
| `/agent` | handler `D8` → K: `set_config_option mode` (+ create/edit agent files locally) | **missing** (v2) | BLOCKS-FLIP (Matrix 6) |
| `/plan` | backend → `brt`: `setConfigOption("mode","kiro_planner")` | **missing** (v2) | with `/agent` |
| `/context` | handler `_ot` → K: `_kiro/session/context {show|add|remove|clear}` (breakdown panel) | **missing** (v2); live bar from `context_usage` HANDLED | FOLLOW-UP P2 |
| `/compact` | handler `bot` → K: `_kiro/session/compact` + `compaction_status` | **missing** (v2) | FOLLOW-UP P2 |
| `/usage` | backend → K: `_kiro/account/getUsage` | builtin `/usage` → same method (HANDLED) | — |
| `/tools` | handler `Yot`: local panel fed by `_kiro/tools/didChange`; `trust-all` local | **missing** (v2) | FOLLOW-UP P3 |
| `/mcp` | handler `YWe`: panel from `_kiro/mcp/status` cache + K: `_kiro/mcp/resetServer` | **missing** (v2) | FOLLOW-UP P2 (MCP bundle) |
| `/hooks` | handler `kot` → K: `_kiro/hooks/list {trigger:"all",includeDisabled:!0}` | builtin in `kas_hooks=kas` mode only (`commands/mod.rs:98`); default Host mode: cyril's own registry (no listing command) | FOLLOW-UP P4 |
| `/knowledge` | backend → K: `_kiro/knowledge {subcommand…}` | **missing** (v2) | FOLLOW-UP P3 |
| `/code` | backend → K: `_kiro/codeIntelligence {status|init|overview}` | **missing** (v2) | FOLLOW-UP P3 |
| `/rewind` | handler `Dot` → K: `session/fork {_meta.kiro{messageId,createdReason:"rewind"}}` (needs `user_message_id_assigned`) | **missing** (v2) | FOLLOW-UP P3 |
| `/prompts` | handler `xot`: local list of `prompts_update` (+`/prompts` picker) | **missing** (v2); prompts appear as broken `AgentCommand`s (S1) | with S1 |
| `/stats` | handler `wit`: local perf metrics (hidden) | **missing** (v2 `/stats` was a real command) | FOLLOW-UP P4 |
| `/chat`, `/sessions` | handlers `L8`, `dit`: `session/list`, `session/load`, `session/delete`, `_kiro/session/rename` | `/load <id>`, `/new`; cyril `/sessions` = v2 subagent list (S2) | FOLLOW-UP P3 |
| `/memories` | handler `Sot` → K: `_kiro/memory/list|get`, `memoryReflection` | cyril `/memory` = cyril-memory (different store) | NONE (design) |
| `/workflow` (+`/workflows`,`/workflow-run|resume|status|cancel`, `/goal`) | handler `bit` / backend → K: `_kiro/workflow/*` (`feature:"workflows"`) | builtin `/workflow`, `/review` (HANDLED) | `/goal` missing — FOLLOW-UP P4 |
| `/autonomous`, `/sandbox` | handlers `ait` / local intercept → `set_config_option mode` + `_kiro/sandbox/applyConfig` (gate `local_sandbox`) | missing | FOLLOW-UP P3 (cyril-6vo6) |
| `/spec` | backend → K: `_kiro/spec/*` | missing | FOLLOW-UP P4 |
| `/tangent` | handler `zot` (`feature:"tangent"`, `/switch`) | missing | NONE (feature-gated in TUI) |
| `/paste`, `/reply`, `/feedback`, `/upgrade-agent`, `/voice`, `/repo`, `/disconnect` | L / host subcommand / cloud | `/voice` builtin; others missing (`/paste`,`/reply`,`/feedback` were v2 agent commands) | FOLLOW-UP P4 |
| `/config`, `/settings`, `/verbosity`, `/theme` | L (panels; `/config managed` behind `managed_settings`) | `/theme` builtin | NONE |
| `/steer`, `/new`, `/load`, `/thinking`, `/powers`, `/memory`, `/review` | (TUI: composer steer, `/chat`, `/model` pane, enum-only `/powers`) | cyril builtins, HANDLED on KAS | — |
| enum-only literals: `/changelog /checkpoint /context-gatherer /copy /custom /editor /experiment /fullscreen /general-task-execution /guide /issue /kvim /lite /load /logdump /powers /prompt /reasoning /save /session-id /skill /spawn /steering /title /todos /transcript /tui` | telemetry ids (`cj`), `iM()` maps unknown names to `"/custom"`; no KAS-mode route | — | NONE |

## Matrix 8 — v2-only features a cyril user loses on the flip

Walk of cyril's v2 paths (`protocol/convert/kiro.rs` arms, v2 commands, crew, metering) against the KAS equivalent.

| v2 feature (cyril today) | cyril v2 evidence | KAS equivalent | cyril-on-KAS | consequence | issue | flip |
|---|---|---|---|---|---|---|
| Auth: kiro-cli renews its own token | `kiro-cli acp` owns OIDC | free-path KAS asks the client (`_kiro/auth/getAccessToken`, `expiresAt ≥ now+3min` or rejected) | cyril serves the stored token and **never renews**; stale → `"kiro token expired; …"` (`protocol/kas/auth.rs:210-268`) | default-engine sessions fail ~1 h after the last kiro-cli refresh unless another kiro-cli process refreshes the store (cyril-taba update 2026-08-03: "this now bites routinely") | cyril-taba, cyril-zbu7 | **BLOCKS-FLIP** |
| Agent slash commands (24: model, agent, effort, context, compact, tools, mcp, knowledge, code, rewind, plan, prompts, stats, chat, paste, reply, feedback, guide, goal, usage, hooks, help, clear, quit) | `kiro.dev/commands/available` `convert/kiro.rs:862`; `commands/execute`/`options` `domain_mediator/commands/extensions.rs:30,124-147` | none advertised; TUI maps 14 of them to `_kiro/*`/`set_config_option` (`ort` + `Eit`, Matrix 7) | absent (only `/usage` re-implemented natively) | the whole Kiro command layer disappears | NEW umbrella | **BLOCKS-FLIP** for `/model` `/agent` (Matrix 6), FOLLOW-UP for the rest |
| Skills / prompts as commands | `commands/execute` | `available_commands_update` resources, expanded agent-side | BROKEN (S1) | skills error | cyril-oieu | **BLOCKS-FLIP** |
| Context % + breakdown | `kiro.dev/metadata` `convert/kiro.rs:547` | `session_info_update{context_usage}` | HANDLED `convert/kas.rs:401` | — | — | NONE |
| Credits / metering, turn duration | `kiro.dev/metadata` metering | `turn_completion{promptTurnSummaries,elapsedTime,status}` | HANDLED `convert/kas.rs:390,583-670`; sidecar kind KAS `R/crates/cyril/src/app.rs:524-525` | — | cyril-79df | NONE |
| Token counts | `kiro.dev/metadata` tokens | none (`usage_update` never emitted, schema-only) | — | token columns blank (already blank on v2 — backend strips) | — | NONE |
| Effort badge + `/effort` | `kiro.dev/metadata` effort `convert/kiro.rs:513` | `effortLevel` configOption | DROPPED | no effort display/change | cyril-838u, cyril-lxuo | FOLLOW-UP P2 |
| Refusal detection | metadata refusal (cyril-h8zb) | adapter refusal object | DROPPED | refused turns look normal | cyril-ker1 | FOLLOW-UP P3 |
| Compaction status | `kiro.dev/compaction/status` `convert/kiro.rs:779-818` | `summarization_{started,completed,failed}` | DROPPED | compaction invisible | NEW | FOLLOW-UP P2 |
| Model fallback | v2 `kiro.dev/session/update` model_fallback — **unhandled on v2 too** (`convert/kiro.rs:1057-1062`, cyril-ci3j) | `model_fallback` kind | DROPPED | no regression (both dropped); still a gap | cyril-aoby, cyril-ci3j | FOLLOW-UP P2 |
| Steering echoes | `kiro.dev/session/update` `convert/kiro.rs:957` | `steering_{queued,injected,cleared}` | HANDLED | — | — | NONE |
| Agent switched | `kiro.dev/agent/switched` `:833` | `current_mode_update` | HANDLED | — | — | NONE |
| Agent not found / config error | `kiro.dev/agent/{not_found,config_error}` `:1158,1178` | `_kiro/customAgent/{not_found,config_error}` | DROPPED | silent fallback agent | NEW | FOLLOW-UP P3 |
| Model not found | `kiro.dev/model/not_found` `:1199` | prompt error `-32000 InvalidModelError` (LIVE frame 173) | HANDLED as `BridgeError` (`domain_mediator/commands/session.rs:499-510`) | — | — | NONE |
| Rate limit | `kiro.dev/error/rate_limit` | `_kiro/error/rate_limit` | HANDLED `convert/kiro.rs:1074` | — | — | NONE |
| MCP init failure / initialized / OAuth request | `kiro.dev/mcp/{server_init_failure,server_initialized,oauth_request}` `:1103,1146,1123` | `_kiro/mcp/status`, `display_error{errorType:"mcp_connection_error"}`, `_kiro/openExternalUrl` (cap-gated) | DROPPED / DROPPED / NOT-SENT | MCP failures and OAuth sign-in invisible | cyril-nk4o | FOLLOW-UP P2 |
| Subagent crew panel, `/sessions`,`/spawn`,`/kill`,`/msg`, inbox | `kiro.dev/subagent/list_update` `:1219`, `kiro.dev/session/inbox_notification` `:1241`, `session/spawn` etc. | `agent-subtask` tool_calls + `agentSubtaskId`-tagged chunks; no spawn/kill/msg API | crew panel empty; commands BROKEN (S2); child output interleaves (Matrix 4a) | lose crew visibility and control | cyril-ebqu, cyril-fjfu | **BLOCKS-FLIP** (interleave) |
| Thinking toggle | v2 `reasoning` command | `thinking` configOption | HANDLED (`/thinking`) | — | cyril-k3lz | NONE |
| Clear status | `kiro.dev/clear/status` `:819` | n/a (`/clear` = new session in TUI) | n/a | — | — | NONE |
| Gains on KAS (not losses) | — | `_kiro/workflow/*`, cyril-hosted hooks/fs/terminal, steering clear | HANDLED | — | — | — |

## Matrix 9 — Session lifecycle

| step | KAS | TUI | cyril | consequence | issue | flip |
|---|---|---|---|---|---|---|
| `session/new` request | accepts `_meta.kiro{customAgents,modeId,overrideSessionId,…}` | sends `overrideSessionId` in one wrapper path | sends `{cwd,mcpServers:[]}` only (`domain_mediator/commands/session.rs:49-52`) | none | cyril-jf71 | NONE |
| `session/new` response | `modes`, `configOptions`, `_meta{title,agentMode,memoryConfig,workflowsEnabled,…}` (LIVE frame 26) | full | modes + configOptions HANDLED (`session.rs:78-97`); `_meta` DROPPED | memoryConfig/title not shown | cyril-0na7 | NONE |
| `session/load` | replays history as `session/update` frames, `_meta.kiro.replay` marked (`replayMarking:true`); 0.66.34 suppresses abandoned-prompt replay when messages arrived during the load (`arrivedDuringLoad` 0→4, kas lane) | buffers replay, settles permissions only on non-replay frames | HANDLED `/load <id>` (`session.rs:158-200`); replay frames rendered as live (no `replay` read) | replayed permission/interaction frames are not distinguishable; fewer ghost prompts after 0.66.34 | cyril-99ds, cyril-rtrh | FOLLOW-UP P3 |
| `session/resume` | in ACP method set, **not** in `sessionCapabilities` (LIVE frame 6) | not called (`.resumeSession(` 0) | not sent | none | — | NONE |
| `session/list` | `sessionCapabilities.list`; scopes via `_meta.kiro{sessionSource,listScope}` | `/chat`, dashboard | NOT-SENT | must know the id to `/load` | cyril-wigv | FOLLOW-UP P3 |
| `session/fork` | `fork{_meta.kiro.messageId}` | `/rewind` | NOT-SENT | no rewind | NEW | FOLLOW-UP P3 |
| `session/close` | `sessionCapabilities.close` | closes finished replayed workflow steps | NOT-SENT | loaded step sessions stay resident in KAS until exit | — | FOLLOW-UP P4 |
| `session/delete` | yes | dashboard | NOT-SENT | none | — | NONE |
| `session/cancel` | yes; ends turn with `turn_end{cancelled}` then response `cancelled` | yes | HANDLED `session.rs:256` | — | — | NONE |
| Terminal-of-turn | **both** terminals, `turn_end` first, prompt response 0–1 ms later (memory ref turn-end ordering) | consumes `turn_end` side effect | HANDLED: `emits_wire_turn_end()==true` for KAS (`protocol/engine.rs:294-296`) so the turn mediator owes the companion (cyril-b4y4/upjh); agent-initiated turns (`turn_start` without a prompt, workflow auto-wake) HANDLED (cyril-lki9) | — | — | NONE |
| `turn_end.stopReason` values | `end_turn`, `cancelled`, `error` (LIVE frame 171), `content_filtered` (TUI faulted set) | maps `error`/`content_filtered` to faulted | non-ACP values → warn + `EndTurn` (`convert/kas.rs:717-734`); error text still arrives via the prompt-response `BridgeError` | failed turns summarized as end_turn | NEW (small) | FOLLOW-UP P3 |
| Stream-idle watchdog terminal | `StreamIdleTimeoutError` + `_kiro/system/notify` | — | notify HANDLED; terminal shape open | — | cyril-0w3u | FOLLOW-UP P2 (existing) |
| First run / missing extracted bundle / data dir | free spawn needs `~/.local/share/kiro-cli/kas/<ver>-<hash>/` | host extracts | discovery only (`protocol/kas/discovery.rs`) | already in cyril-4c4d acceptance (and cyril-igrx closed) | cyril-4c4d | (tracked in 4c4d) |

## Flip blockers (what a default-KAS cyril user hits on day one, most severe first)

Priority rule (user, 2026-10-14): KAS is cyril's current path; P1 = regression or billed/unsolicited work on the default path, P2 = dropped field or missing route on the default engine.

1. **Sessions die about an hour after the last kiro-cli token refresh** (P1). Under v2, `kiro-cli acp` renews its own OIDC token. Under the free-path KAS spawn, cyril serves the stored access token and never renews it. Once the token is stale, `_kiro/auth/getAccessToken` fails with "kiro token expired" (`protocol/kas/auth.rs:210-268`) and the session is lost. A user who runs only cyril gets about an hour of work per `kiro-cli login`. Evidence: STATIC (auth.rs) plus the live history recorded in cyril-taba (2026-08-03) and in memory (cyril-ild0 qualification). Fix: **cyril-taba** (refresh trigger) or **cyril-zbu7** (spawn through `kiro-cli acp --agent-engine v3 --auth-method cli`, which keeps the refresh token inside kiro-cli). Both are P2 today; raise to P1.
2. **Every skill, saved-prompt and steering slash command fails** (P1, regression). KAS advertises these entries in `available_commands_update`. Cyril registers each one as an `AgentCommand` and dispatches it as `_kiro.dev/commands/execute`. KAS has no such method (0 hits) and rejects it with `-32603 [PersistenceClassification]` (STATIC cyril path, LIVE error shape in frame 146). On v2 the same commands work. Fix: re-scope **cyril-oieu** (P3 → P1; the gap is present-day, not "future"). Make command dispatch engine-aware: on KAS, send `/name args` as prompt text and keep `_meta.kiro.type` (cyril-79ox). The TUI treats prompt/skill/steering entries as resources (`oZ`). That expansion happens agent-side is live-backed for saved prompts (cyril-79ox: "only ${1} and $ARGUMENTS substitute"). For skills it is [INFERENCE] until live lead V1.
3. **No way to change the model, the agent/mode, or the effort** (P1, regression). v2's `/model`, `/agent` and `/effort` came from Kiro's agent commands, and KAS advertises none of them. Cyril sends `session/set_config_option` correctly (`domain_mediator/commands/session.rs:349-420`), but no UI produces `model`, `mode` or `effortLevel`; the only producers are `/thinking` and the reviewer. NEW: a picker issue (kas-4) built on the existing **cyril-cxwb, cyril-lxuo, cyril-838u, cyril-lyrx**. It must validate the chosen model against the offered options, because KAS accepts a bogus `model` unvalidated (LIVE frames 160-173; the KAS twin of cyril-fj6j).
4. **Sub-agent output is mixed into the main transcript** (P2). KAS tags sub-agent chunks and tool calls with `_meta.kiro.agentSubtaskId` on the parent session's stream, and the TUI routes them into a sub-agent lane (`GY`). Cyril has 0 `agentSubtaskId` hits, so everything a sub-agent says or runs appears as if the main agent did it. Sub-agent rows (`kind:"other"`, "Sub-agent: X") do render, because cyril no longer filters `ToolKind::Other`. That contradicts CLAUDE.md and the premise of cyril-ebqu. The v2 crew panel and the `/sessions` `/spawn` `/kill` `/msg` commands are dead on KAS (S2). Fix: **cyril-ebqu** (re-scope: route by `agentSubtaskId`), cyril-fjfu. STATIC; live lead V2.
5. **Most of the v2 slash-command surface disappears** (P2). `/context` and `/compact` lead the list, followed by `/tools`, `/mcp`, `/knowledge`, `/code`, `/rewind`, `/prompts`, `/plan`, `/chat`, `/stats` and `/paste`/`/reply`/`/feedback`. The TUI maps most of them to `_kiro/*` methods (the emulator `ort` plus the `Eit` handlers, Matrix 7). NEW umbrella with `/context` (`_kiro/session/context`) and `/compact` (`_kiro/session/compact`) as P2 children and the rest as P3/P4.
6. **Compaction is invisible** (P2). `summarization_{started,completed,failed}` are dropped without a log at `convert/kas.rs:525`. v2's `kiro.dev/compaction/status` was handled (`convert/kiro.rs:779`). NEW.
7. **MCP failures and remote-MCP OAuth sign-in are invisible** (P2). Cyril drops `_kiro/mcp/status` and `display_error{errorType:"mcp_connection_error"}`, and does not advertise `openExternalUrl` or `secretStorage`, so KAS never sends the OAuth URL. v2 handled `kiro.dev/mcp/{server_init_failure,server_initialized,oauth_request}`. Fix: **cyril-nk4o** (P3 → P2), plus a NEW issue for the OAuth capability pair.
8. **A model fallback switches the answering model without notice** (P2; not a regression, because v2 cyril drops its fallback too). Fix: **cyril-aoby** (P3 → P2, as the retriage R3 proposed).
9. **Org governance is silent** (P2, enterprise users). `_kiro/governance/state` is pushed unsolicited (LIVE frame 17) and dropped. Fix: **cyril-0o7e** (P3 → P2, retriage R5).

The flip is also gated by items already in cyril-4c4d's acceptance criteria: the first-run/missing-bundle path, the ADR, CI, and macOS (cyril-igrx is closed).

## Follow-ups (not day-one breakage; mapped to issues)

| # | item | P | issue |
|---|---|---|---|
| F1 | Dead settings keys sent on every handshake: `subagentOrchestration` (always), `todoList`, plus `toolSearch.enabled` which has no consumer since 0.66.34; `neverDefer` NOT-SENT. The `orchestrate_subagent` tool itself was removed in 0.66.33 (count 21 in 2.28.0 → 0 in 2.29.0..2.31.1) | P2 | cyril-53qx (note), cyril-q8gq, cyril-ea67 |
| F2 | MCP tool calls show the `tool_load` wrapper `{tool_id, arguments}` | P2 | cyril-2yn8 (note; kas-lane live lead L3) |
| F3 | Effort option dropped (badge, `/effort`) | P2 | cyril-838u, cyril-lxuo (child of the picker issue) |
| F4 | `display_error` dropped (non-MCP cases are covered by the prompt-error path) | P3 | cyril-58uv |
| F5 | `turn_end.stopReason:"error"` (and `content_filtered`) recorded as EndTurn | P3 | NEW |
| F6 | `/spawn` `/kill` `/msg` send v2-only methods on KAS; `/sessions` always empty | P3 | NEW (hide or adapt on KAS) |
| F7 | `_kiro/customAgent/{not_found,config_error}` dropped (v2 twins handled) | P3 | NEW |
| F8 | `/rewind` via `session/fork` + `user_message_id_assigned` | P3 | NEW |
| F9 | Session list/browse (`session/list`), history paging (`_kiro/session/history`) | P3 | cyril-wigv, cyril-a3th |
| F10 | `interaction_resolved` settle + `replayMarking` on load | P3 | cyril-99ds |
| F11 | `userInput` capability (free-form questions auto-skipped in delegated children) | P3 | NEW (or note on cyril-7q8u) |
| F12 | `terminalInput` + `_kiro/terminal/write` for the terminals cyril hosts | P3 | cyril-7p7n |
| F13 | `streamingShellContent` + `_kiro/tools/content_chunk` | P3 | cyril-2mo0 |
| F14 | `_kiro/tools/didChange` (tool list), `/tools` | P3 | cyril-58uv (umbrella) |
| F15 | `extensionMethods` / agent caps parse | P3 | cyril-tikf |
| F16 | KAS memory default off (decide `memory` handshake vs cyril-memory) | P3 | cyril-0na7 |
| F17 | `autopilot` not toggleable (stays on) | P3 | fold into the picker issue |
| F18 | Telemetry policy (`telemetryEnabled` NOT-SENT) | P3 | cyril-zjm5 |
| F19 | Refusal object dropped | P3 | cyril-ker1 |
| F20 | `_kiro/workflow/pause`, `recipes_changed`, launch `modelId`/`effortLevel` | P3/P4 | cyril-4u4a |
| F21 | `session/close` for loaded workflow-step sessions | P4 | NEW (small) |
| F22 | Allow-always 20× loop: fixed engine-side (`consentRound`, every live ask `consentRound:1`); re-verify on cyril and close | P3 | cyril-w55l (note) |
| F23 | Sandbox / autonomous, spec UI, `/goal`, `/stats`, `/paste` `/reply` `/feedback` | P3/P4 | cyril-6vo6, cyril-be7e; umbrella |
| F24 | Docs: CLAUDE.md's KAS-subagent section (OrchestrateSubAgent "LIVE-CONFIRMED", `subagentOrchestration` gate) and the "ToolKind::Other … filtered from display" rule are stale | P4 | NEW (docs) |

## Proposed issues / notes (nothing filed; rivets grep via `static-v3cov-issues-2.31.1.py`)

New issues (labels `kas`, `kiro-2-31-1`, plus a triage label):

- **NEW P1** "KAS default engine: `/model`, `/agent` (mode) and `/effort` pickers driven by session configOptions and `session/set_config_option`". Validate against the offered options, because KAS accepts unknown models unvalidated. Include an `autopilot` toggle. Parent of cyril-cxwb, cyril-lxuo, cyril-838u and cyril-lyrx. Blocks cyril-4c4d.
- **NEW P2** "KAS slash-command parity umbrella: `/context`, `/compact`, `/tools`, `/mcp`, `/knowledge`, `/code`, `/rewind`, `/prompts`, `/plan`, mapped like the TUI's KAS `executeCommand` emulator". Children: `/context` → `_kiro/session/context` (P2); `/compact` → `_kiro/session/compact` (P2); `/rewind` → `session/fork` + `user_message_id_assigned` (P3); the rest P3/P4.
- **NEW P2** "KAS: render compaction lifecycle (`summarization_started/completed/failed`) — v2 compaction/status parity".
- **NEW P2** "KAS remote-MCP OAuth never reaches the user: advertise `openExternalUrl` (and `secretStorage`), surface `display_error{mcp_connection_error}`". Related: cyril-nk4o, cyril-caar.
- **NEW P3** "KAS: `/spawn`, `/kill`, `/msg`, `/sessions` send v2-only methods (`_session/spawn`, `_kiro.dev/session/terminate`, `_message/send`); hide or adapt on KAS".
- **NEW P3** "KAS `turn_end.stopReason` `error`/`content_filtered` default to EndTurn (`convert/kas.rs:717-734`)".
- **NEW P3** "KAS: handle `_kiro/customAgent/{not_found,config_error}` (parity with v2 `kiro.dev/agent/*`)".
- **NEW P3** "KAS: advertise `userInput` and answer `_kiro/userInput` (option-less questions are auto-skipped today)".
- **NEW P4** "KAS: `session/close` loaded workflow-step sessions after replay".
- **NEW P4 (docs)** "CLAUDE.md: KAS subagent section stale. `orchestrate_subagent` and `subagentOrchestration` are gone since KAS 0.66.33 (2.29.0), and `ToolKind::Other` is no longer filtered (`cyril-ui/src/state.rs:536-558`)".

Notes and priority changes on existing issues:

- **cyril-taba** and **cyril-zbu7**: flip blocker #1. Raise one of them to P1 and link it from cyril-4c4d.
- **cyril-oieu**: the gap exists today. Every KAS skill, saved prompt and steering doc is registered and dispatched via `_kiro.dev/commands/execute`, which KAS rejects with -32603. Raise P3 → P1 and re-title.
- **cyril-ebqu**: correct the premise (cyril no longer filters `ToolKind::Other`). The real gap is `agentSubtaskId` routing and grouping. Note that `orchestrate_subagent` and the pipeline meta are gone; only `invoke_sub_agent` remains (30 hits).
- **cyril-aoby** P3 → P2; **cyril-0o7e** P3 → P2 (governance part); **cyril-nk4o** P3 → P2 (both as the retriage proposed, now with parity evidence).
- **cyril-53qx**: `toolSearch.enabled` has no consumer since 0.66.34; the `orchestrate_subagent` tool was removed in 0.66.33, which makes the `subagentOrchestration` doc comment (`settings.rs:90-92`) wrong too.
- **cyril-w55l**: the engine fixed this (`consentRound`). Re-verify on cyril-on-KAS and close.
- **cyril-2yn8**: MCP calls now always pass through `tool_load` `{tool_id, arguments}`.
- **cyril-4c4d**: attach this file as the parity list. The blockers are #1 (taba/zbu7), #2 (oieu), #3 (NEW picker), #4 (ebqu) and #5 (NEW slash umbrella). #6–#9 are the P2 set.

## Leads for the live lane

All legs zero-turn unless marked. Use the full handshake path `initialize.clientCapabilities._meta.kiro[.settings]` (trap 5), a fresh temp HOME with the real XDG_DATA_HOME (trap 7), and a direct `node acp-server.js` from the 2.31.1 carve.
- **V1 skill dispatch (zero turns).** Seed `$HOME/.kiro/skills/probe/SKILL.md`. After `session/new`, record the `available_commands_update` entry (`_meta.kiro.type:"skill"`). Then send cyril's exact frame `_kiro.dev/commands/execute {sessionId, command:{command:"probe", args:{}}}`. Expect `-32603 … [PersistenceClassification]`. Control: `_kiro.dev/commands/options` returns the same error. This converts blocker #2 from STATIC to LIVE.
- **V2 sub-agent interleave (one billed turn, optional).** Prompt the agent to delegate with `invoke_sub_agent`. Record every frame carrying `_meta.kiro.agentSubtaskId`: its `sessionId` (expected: the parent), `sessionUpdate` and `kind`. This converts blocker #4 to LIVE.
- **V3 mode switch (zero turns).** `session/set_config_option {configId:"mode", value:"plan"}` versus `session/set_mode {modeId:"plan"}`. Record which one emits `current_mode_update` and `config_option_update`, so the picker issue knows which to use.
- **V4 compaction frames (passive).** If any leg compacts naturally, record the `summarization_*` payloads (`conversationSummary`, `truncated`, `reason`).
- **V5 MCP OAuth without `openExternalUrl` (zero turns, only with a safe test server).** Record `_kiro/mcp/status` `servers[].status` and any `display_error` text. Never complete an OAuth flow.

## Method notes

- **Direction classification** (`static-v3cov-methods-2.31.1.py`). Client→agent: a literal `case"_kiro/…"` arm (56 methods). Agent→client notification: `extNotification("…")` (22) plus the table emitters (workflow `Jou` map, `c$o`, `JPu`, `p3o`, `UPe`, `KNu`, `LS(…"_kiro/safety/statusChanged"…)`). Agent→client request: `Nh(conn,"…")` or `extMethod("…")`. Four literals are internal multiplex/prefix strings: `_kiro/permission/respond`, `_kiro/userInput/respond`, `_kiro/sessions/`, `_kiro/system/`.
- **KAS client-meta readers:** `QHr`, `initialize()`, the agent-context builder, `tYr`, `kMu`, `clientMeta?.…`, the terminal host and `i5r`. The settings schema is `G2n` plus `bMu`, and the intake log comes from `IMu`. `authCallback`, `largeToolOutputHandler` and `workflowNotifications` all have 0 hits in KAS 0.66.33 and 0.66.34, so the TUI sends them as dead keys.
- **TUI routing tables:** `aSn` (handler vs backend), `Eit` (handler functions), the KAS `executeCommand` emulator `ort`, `Det` (sessionUpdate mapper), `jee` (session_info mapper), `$tt` (clientMeta) and `oT()` (settings). The full handler bodies are in `raw/tui-slash-handlers.txt`.
- **OrchestrateSubAgent removal** was counted in every archived carve with `grep -oE '[oO]rchestrate_?[sS]ub_?[aA]gent'`: 21–22 hits in 2.24.1..2.28.0, then 0 in 2.29.0..2.31.1. `subagentOrchestration` follows the same pattern (5–14 hits, then 0). The 2.28.0 quote is `OrchestrateSubAgent:{name:"orchestrate_subagent",description:"Tracks orchestrate sub-agent (DAG pipeline) too…`.
- **Live frames cited** are read as data from `audit-2.31.1/live/kas-live-sweep-2.31.1.jsonl`: frame 6 (initialize), 9 (config/template), 17 (governance), 26 (session/new), 122 (hooks/list gate), 146 and 155–159 (PersistenceClassification), 160–173 (bogus model, display_error, `turn_end{error}`, InvalidModelError).
- **Not done (out of lane):** no execution, no rivets writes, no edits outside `v3-coverage/` and the new scripts. The cyril line numbers are from HEAD 5ee8bc07.
