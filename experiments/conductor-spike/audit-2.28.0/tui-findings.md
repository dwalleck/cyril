# TUI-bundle static lane — kiro-cli 2.26.0 → 2.26.1 → 2.27.0 → 2.27.1 → 2.28.0

Inputs: `~/.local/share/kiro-research/tui-bundles/kiro-tui-{2.26.0,2.26.1,2.27.0,2.27.1,2.28.0}.js`
(sha256 df778987… / 8106c422… / 7f92dc0a… / 3fe9ded8… / 6733ecdb…). Cross-checks: KAS carves
`kas-carves/<ver>/node_modules/@kiro/agent/dist/server/acp-server.js` (0.66.15 / 0.66.15 / 0.66.22 / 0.66.22 / 0.66.26),
v2 `kiro-cli-chat` strings, host rollout registry (`static-rollout-2.24.0.py`).

Method:
- `experiments/conductor-spike/static-tui-vocab-2.28.0.py` — real JS lexer (strings, nested templates, regex
  literals, comments) → string-literal and property-name sets per bundle; a delta member counts only if the
  raw text is absent from the other bundle (defeats re-minification). Deltas: 2.26.1 +18/-0 literals;
  2.27.0 +179/-13; 2.27.1 +138/-6; 2.28.0 +127/-30. Outputs in `vocab/`.
- `experiments/conductor-spike/static-tui-gates-2.28.0.py` — anchored extraction of what the TUI SENDS
  (KAS settings builder rows/defaults/rollout checks, hidden host subcommands, method literals,
  agentCapabilities parser, KAS spawn argv/env, session/prompt `_meta.kiro` keys). Output `gates.txt`.
- Wire-method literal set across all 5 bundles: only ONE addition (`_kiro/session/history`, 2.27.1),
  zero removals. Most 2.27/2.28 TUI growth is UI (shells monitor, /kvim, Windows sandbox setup, memory panels).

Cyril status legend: HANDLED / DROPPED (arrives, ignored) / NOT-SENT (cyril could send, doesn't) / N/A.

---------------------------------------------------------------------------------------------------
## CONFIRMED — wire-relevant (ranked)

| # | rel | item | kind | quoted context (2.28.0 unless noted) | cyril | significance |
|---|---|---|---|---|---|---|
| 1 | 2.27.1 (GA) | **v2 model fallback**: inbound `kiro.dev/session/update` `{sessionUpdate:"model_fallback", from, to:string\|null, cause:"refused"\|"unavailable", promoted?:bool}` | inbound v2 ext update | `case"model_fallback":return T0n(e.sessionId,t)` … `T0n: typeof t.from!=="string"\|\|!y0n(t.to)\|\|!z2(t.cause,h0n)…h0n=["refused","unavailable"]`; renderer `ast`: `if(n.promoted){…setCurrentModel({id:r…})}` else retry banner "`…; retrying on <model>...`" | **DROPPED** (convert/kiro.rs handles `AgentExecutionUserMessageQueued` on the same channel, not `model_fallback`/`retry_warning`/`stream_discarded`) | Parser existed since ≤2.26.0 but host rollout `model_fallback` (0 % dark in 2.26.0) is **gone from the 2.28.0 registry** = GA ("a refused or over-capacity turn now retries on another model by default"). With `promoted:true` the session's model silently changes → cyril's model badge/`/model` state goes stale. No rivets issue (cyril-svi2 covers stall/retry only). |
| 2 | 2.27.1 | **v2 `/model fallback`**: options from `kiro.dev/commands/options {command:"model"}` carry per-model `fallback:{eligible, readOnly, configured, configuredUnreadable, vendorDefault[]}`; set via `kiro.dev/commands/execute {command:"fallback", args:{targetModelId, fallbackModelId?}}` (omit fallbackModelId = None) | outbound v2 command + option field | `NWn: executeCommand({command:"fallback",args:r})`; `i({targetModelId:ke,...qe!==null&&{fallbackModelId:qe}})`; guard `s==="model"&&t.trim()==="fallback"&&n.agentEngine!=="kas"` | NOT-SENT; `fallback` option field DROPPED | Same TUI code in 2.26.0 (dark); now user-facing. v2-only (TUI explicitly skips on KAS). |
| 3 | 2.28.0 | **v2 permission response `_meta.provenance:"human"`** | outbound v2 | `let Q=l==="kas"?I_(d,g,h):{...h,provenance:"human"};d.resolve({outcome:"selected",optionId:b,...Q?{_meta:Q}:{}})` | NOT-SENT | Pairs with v2 2.27.1 `trust_shadow_host.rs` "permission-classifier" shadow mode (rollout `trust_classifier_shadow` 100 % internal-nightly, env `KIRO_TRUST_CLASSIFIER_SHADOW`, `_LOG`). Engine records classifier verdict beside the human decision. Telemetry-grade for cyril; low priority but cheap to send. |
| 4 | 2.28.0 | **TUI stopped sending `subagentOrchestration`** in `initialize._meta.kiro.settings`; `chat.enableMainAgentSubagentTool` + `KIRO_TEST_DISABLE_SUBAGENT_ORCHESTRATION` removed | outbound settings change | 2.26.x defaults `{…,largeToolOutputHandler:!0,subagentOrchestration:process.env.KIRO_TEST_DISABLE_SUBAGENT_ORCHESTRATION!=="1"}`; 2.27.x `subagentOrchestration={enabled:(!workflowsEnabled\|\|Fa(CHAT_ENABLE_MAIN_AGENT_SUBAGENT_TOOL,!0))&&…}`; 2.28.0 defaults `{codeIntelligence,knowledge,thinking,largeToolOutputHandler}` only | cyril still sends `subagentOrchestration:{enabled:true}` (kas/settings.rs `DEFAULTS_ON`) | **Reference-client divergence.** KAS 0.66.26 defaults it `kp("subagentOrchestration","off")` and, when workflows are on, `suppressChatDelegationTool = workflowsEnabled && !subagentOrchestrationActive` ("Only workflows use sub-agents; chat delegates through them"). So 2.28.0 TUI sessions likely get NO OrchestrateSubAgent; cyril still does. Decide deliberately (cyril-ucii/ebqu). See LEAD L1. |
| 5 | 2.27.0 | **KAS memory controls**: `initialize._meta.kiro.settings.memory = {mode:"disabled"\|"read_only"\|"read_write", reflection:bool}` (from cli.json `memory.mode`/`memory.reflection`) when rollout `memory_controls`; else legacy `memory.enabled→userMemoryOptIn`. Live toggle: `session/set_config_option {configId:"memoryReflection", value:"on"\|"off"}`; reads `session/new` `_meta.memoryConfig` (or `_meta.kiro.memoryConfig`) | outbound settings + config option | `o=Pn("memory_controls");if(!o)a.push(["memory.enabled","userMemoryOptIn"])…if(…,o)t.memory=s9(e)`; `await e.setConfigOption("memoryReflection",s?"on":"off")`; `NX: sb(e,(r)=>r.id==="memoryReflection")` | NOT-SENT (`memory`, `userMemoryOptIn` — cyril-q8gq); memoryReflection rides cyril's generic config_options (no memory UI); `memoryConfig` DROPPED (cyril-l4e7) | KAS confirms: 0.66.22 schema `W1n=k.object({memory:Zkn.optional(),_parallelTasks:…})`, `ZLt="memoryReflection"`. Rollout `memory_controls` 0 %→**100 % internal all channels** (2.28.0); `memory` → internal all channels. 2.28.0 UI copy: access = next session, auto-updates = immediate. Relevant to cyril-s21o (KAS native memory vs cyril-memory): `memory:{mode:"disabled"}` is the clean veto. |
| 6 | 2.27.1 | **`_kiro/session/history {sessionId, beforeMessageId, limit:100}` → `{updates[], hasMore, oldestLoadedMessageId}`** | new outbound method (TUI) | `sendExtMethod("_kiro/session/history",{sessionId:t,beforeMessageId:n,limit:r})`; keeps `user_message_chunk` text where `_meta.kiro.source!=="steer"`, with `_meta.kiro.timestamp` | NOT-SENT | KAS dispatch has it (`case"_kiro/session/history"` → "sessionLive") since ≤0.66.15. TUI uses it for prompt-history recall and to find a tangent's branch point (`/tangent merge`). Cheap paged-history read for cyril resume/up-arrow. Listed in cyril-saf4 only as a swept name. |
| 7 | 2.27.1 | **`session/prompt _meta.kiro.displayText`** | outbound prompt meta | `dDn(e,t){…_meta:{…n,kiro:{…w9(n.kiro),displayText:t}}}`; used with `{persistDisplayText:!0}` for tangent-merge summary prompts | NOT-SENT | KAS persists it on the user row (`persistStepUserMessage(t,…,{kiro:{displayText:…}})`) so replay shows the short label instead of the long synthetic prompt — fixes "reopening a tangent shows the full prompt". Useful for any cyril-synthesized prompt (e.g. /workflow new, review prompts). `outputStyle` prompt meta pre-exists (cyril-lyrx). |
| 8 | 2.26.1 | **`session/load _meta.kiro.requireExisting:true`**, gated on new `agentCapabilities._meta.kiro.strictSessionLoad:true` | outbound load meta + capability | `…t?.requireExisting&&this.kiroCapabilities.strictSessionLoad===!0&&{requireExisting:!0}`; else log "strict session load unavailable; using ordinary load" | N/A (cyril has no session/load) | **TUI-ahead**: `strictSessionLoad`/`requireExisting` have 0 hits in KAS 0.66.15–0.66.26. Used by `--resume-id` (herdr). Capability parser now: executionTargets, sessionSources, sessionListScopes, extensionMethods, sourceProviders, **strictSessionLoad**, replayMarking (cyril-tikf, cyril-99ds). |
| 9 | 2.27.1 | **`session/close` on finished workflow-step sessions** after the TUI loaded them for transcript/credit replay | outbound (existing ACP method) | `closeFinishedStep(e,t){…this.runtime.closeSession(e).then((a)=>{if(a\|\|…)return;…"Agent cannot close sessions; replayed workflow steps stay loaded"` | N/A today; becomes relevant with cyril-jxw3 (attach via load) / cyril-zd8u | Each loaded step stays resident in KAS until closed. `sessionCapabilities.close` noted in cyril-l4e7. |
| 10 | 2.28.0 | **`/workflow new` canned prompt changed**: no longer tells the model to author+validate itself when `wf-workflow-creator` is unavailable | outbound prompt text | 2.27.1: "Only if `wf-workflow-creator` is unavailable: author the definition yourself … validate it with the validate_workflow tool"; 2.28.0: "If `wf-workflow-creator` is unavailable, do not author the definition yourself: nothing else in this session can validate it. Tell me workflows cannot be created in this session and stop." | N/A (cyril-0asq design) | Exactly the gate-off failure from the 2.26.0 audit §8b (unvalidated recipe + false success). Update `tui-workflow-new-prompt-*.txt` reference and cyril-0asq. |
| 11 | 2.27.0 | **KAS child-consent grants**: `allow_always` on a child (subagent/workflow-step) approval with `consentContext.capability` is recorded as a TUI-side grant keyed `capability\0workspaceRoot\0ownerSessionId`; matching queued approvals auto-resolve `allow_once` | client policy over existing wire (`_meta.kiro.consent{capability,scope,resource?,workspaceRoot?}`) | `kasChildConsentGrants:_?mot(T.kasChildConsentGrants,_):…`; log `kas.child_consent.recorded {capability,match,scope,ownerSessionId}`; `got(): [db(a)?"shell":a, workspaceRoot, originSessionId??sessionId].join("\x00")` | cyril: not implemented (approves one at a time) | Directly addresses the "37 permission prompts per workflow run" cost (memory: custom-agent run_workflow). Reference behaviour for cyril's approval queue. |
| 12 | 2.28.0 | **Windows KAS supervisor**: on win32 with local sandbox available, KAS is spawned as `kiro-cli chat _ windows-kas-process -- <node> --experimental-wasm-modules acp-server.js --transport=stdio --auth=acp-callback --sandbox=auto --sandbox-network-mode=…` with env `KIRO_OWNED_KAS_PARENT=<pid>`, `KIRO_OWNED_KAS_CONTROL=<uuid>`; shutdown via `kiro-cli chat _ windows-kas-stop <uuid>`; setup via `kiro-cli chat _ windows-sandbox [--register [--maintenance-confirmed]]` → `{kind:"windowsSandbox",data:{status}}` (`registrationRequired`/`repairNeeded`/`observationUnknown`/ready) | host spawn/IPC (not ACP) | `Ctt: t==="win32"&&n ? {executable:r(),prefix:["chat","_","windows-kas-process","--",e],ownsProcess:a}`; `ktt: {KIRO_OWNED_KAS_PARENT:String(process.pid),KIRO_OWNED_KAS_CONTROL:e}` | NOT-SENT (cyril spawns node directly, discovery.rs) | Cyril is native on Windows; KAS sandboxing there now assumes the owned supervisor. Subcommands absent from the Linux `kiro-cli-chat` (Windows MSI build). Also `--sandbox=${backend}` → fixed `--sandbox=auto` ("Global sandbox isolation setting is no longer supported"). cyril-6vo6 adjacent. |
| 13 | 2.26.1 | **KAS spawn env `NODE_USE_SYSTEM_CA=1`** (default unless set) | host spawn env | `env:{...process.env,NODE_USE_SYSTEM_CA:process.env.NODE_USE_SYSTEM_CA??"1",…,KIRO_CUSTOM_USER_AGENT:…}` | NOT-SENT (also KIRO_CUSTOM_USER_AGENT never sent — standing gap) | Corporate TLS-intercepting proxies: TUI-spawned KAS trusts the OS store; cyril-spawned KAS doesn't. `KIRO_CUSTOM_USER_AGENT = "KiroCLI/<v> KAS/<kas> os/<p> md/appVersion-<v> app/AmazonQ-For-CLI"` is read by KAS (backend UA). |
| 14 | 2.27.1 | **`todoList` setting dropped** (`chat.enableTodoList→todoList` row removed) | outbound settings | 2.27.0 rows include `["chat.enableTodoList","todoList"]`; 2.27.1 not | cyril still maps it (kas/settings.rs BOOL_MAP) | KAS has 0 `todoList` hits in 0.66.22+ → dead key; harmless, prune with cyril-q8gq. |
| 15 | 2.27.1 | **shell rawOutput `agent_notes`** now rendered alongside stdout/stderr | inbound field render | `[me.stdout,me.stderr,me.agent_notes].filter(…)` | DROPPED in cyril-ui traits.rs (stdout/stderr only) | v2 field = `$AGENT_CONTEXT_OUT` FIFO side channel ("instructions or context sent to you by the command"). Low. |

### Pure-TUI / host-only deltas (no cyril wire action)
- 2.26.1 **herdr** integration (`HERDR_ENV/HERDR_BIN_PATH/HERDR_PANE_ID/HERDR_SOCKET_PATH`): TUI reports its session to a herdr pane; resume = `kiro-cli chat --agent-engine=v3 [--cloud] --resume-id <id>`. Unannounced.
- 2.27.0 telemetry exporter in-TUI: hidden `kiro-cli chat _ get-telemetry-credentials [--expected-generation G] [--force-refresh]` → `{accessToken, profileArn, tokenType, generation}`; Bearer + `x-kiro-profile-arn`/`x-kiro-token-type` to `telemetry.{us-east-1,eu-central-1}.kiro.dev`. `KIRO_CONTENT_COLLECTION_ENABLED`/`KIRO_TELEMETRY_IS_ENTERPRISE` → `isContentCollectionOptIn` in initialize `_meta.kiro.telemetry` (KAS reads it into telemetry attrs only). **Do not call `--force-refresh` from parallel tooling** (single-flight OIDC refresh — logout risk).
- 2.27.0 shells monitor / move-to-background UI (`chat.keybindings.moveToBackground`, Tab monitor). Wire methods (`_kiro/terminal/{moveToBackground,output,stopBackground,stopAllBackground}`) were already in 2.26.0 and are **still unserved by KAS 0.66.26** (0 literal hits; TUI treats -32601/"unknown ext method" as `not-enabled`). Rollout `background_execution` still 0 %.
- 2.27.0 output-style catalog refactor (`_kiro/config/template` option `outputStyle`, category `output_style`) — pre-existing method (cyril-lyrx).
- 2.27.1 **`/kvim`** embedded Neovim pane (msgpack-RPC client) — rollout `kvim` (new, 100 % internal), **not in changelog**.
- 2.27.1 `/tangent merge [dest]` — composition of existing wire: summary `session/prompt` in the tangent (with `displayText`), switch, `session/prompt` "Merged findings from …" in dest; branch point via `_kiro/session/history`.
- 2.27.1 model-not-served notices: reads `session/new` result `_meta.modelId` + served ids from `configOptions` model list; log "[kas] initial model not served". Pure TUI logic (v2 analog: cyril-fj6j).
- 2.28.0 `/model` & `/effort` pickers open mid-turn: TUI queues an internal `/reasoning <model> effort=X|thinking=on|off` command (`reasoning:{effect:"applyReasoning",drainOrdered:!0,local:{internal:!0}}`) and drains it after the turn → existing `commands/execute {command:"reasoning",args:{targetModelId,effort|thinkingEnabled,setAsDefault:true}}` (v2) / `set_config_option effortLevel|thinking` (KAS). Already HANDLED in cyril.
- 2.28.0 `chat.fullscreenCopyOnSelect`, voice hint text, memory telemetry counters.
- 2.28.0 "Classic sessions offer to switch you to 3.0 and upgrade your agent configs" is **not in tui.js** — it is Rust launcher code (`chat-cli/src/launch/classic_nudge.rs`, `auto_migrate.rs`; strings "Classic is being deprecated with the Kiro CLI 3.0 release in October", `KIRO_CLASSIC_NUDGE_RELAUNCHED`). Hand to the binary lane. "Hook matchers accept tool tags" also has no TUI footprint (KAS-side).
- Host rollout registry 2.26.0→2.28.0 (still 21 rows): +`kvim` (100 internal), +`trust_classifier_shadow` (100 internal nightly), −`model_fallback` (GA), −`v2_non_interactive`; `memory_controls` 0→100 internal; `memory` nightly→all channels; `v3_prompt` 25→35.

### Coordinator follow-up (KAS-lane surfaces): does the 2.28.0 TUI wire them?
| surface | TUI 2.26.0/2.27.1/2.28.0 | verdict |
|---|---|---|
| initialize `clientCapabilities._meta.kiro.configurationState` | 0/0/0 literal hits | NOT wired by TUI |
| `…_meta.kiro.terminalInput` | 0/0/0 | NOT wired |
| `…_meta.kiro.telemetryEnabled` | 1/1/1 | **wired, unchanged**: clientMeta `{telemetryEnabled:Lx(), ...telemetryEnabled&&{telemetry:Hh()}, knowledge:!0, hooks:{enabled:!0,v2:!0}, requirementsAnalysis:!0, specPhaseCheckpoints:!0, streamingShellContent:!0, ...KIRO_INFRA_SAFETY_ROLLOUT_ENABLED==="1"&&{infrastructureSafety:!0}, settings}`; `Lx()` false when `KIRO_DISABLE_TELEMETRY` set |
| settings `backgroundExecution` | 1/1/1 | **wired, rollout-gated**: `[["memory","memoryEnable"],["background_execution","backgroundExecution"]]` → `{enabled:true}` iff host rollout `background_execution` (0 %, or `KIRO_ENABLED_FEATURES`) |
| settings `cascade` | only SQL-grammar hits | NOT wired |
| settings `validation` / `ftaVibe` | 0/0/0 | NOT wired |
| session/prompt `_meta.kiro.contextBreakdown` | only i18n keys `components.contextBreakdown.*` | NOT sent; TUI consumes breakdown from `session_info_update` (`kind:"context_usage"` + `breakdown` → `context_breakdown_update`) |
| `_kiro/configuration/state` | 0/0/0 | NOT handled |
| `_kiro/terminal/write` | 0/0/0 | NOT handled (TUI never advertises terminalInput) |
| session_info_update `model_routed` | 0/0/0 | NOT handled (falls to `"KAS session update (not yet mapped)"` debug) |
| available_commands `_meta.kiro.type:"prompt"` | present all 3 | **handled, pre-existing**: `u1n` switch on `_meta.kiro.type` → `prompt` (→ `prompts_update`, keeps `commandId`, `arguments` from `_meta.arguments ?? kiro.arguments`, source), `skill`/`steering` (scope global|workspace + path), drops `agent`/`mode`/`custom-agent`, rest → `commands_update` |
Session_info_update kinds the 2.28.0 TUI maps (`BX`): `displayError`, `repositories`, `turn_completion`, `summarization_{started,completed,failed}`, `context_usage`(+`breakdown`), `user_message_id_assigned`, `turn_start`, `turn_end`, plus `r1n` handlers; unchanged set across the range.

---------------------------------------------------------------------------------------------------
## LEADS (need a live probe) — each with the exact probe

L1. **Does KAS 0.66.26 expose OrchestrateSubAgent when the client omits `subagentOrchestration`?** (item 4)
    Probe (FREE, handshake only, `HOME=<tmp>`): KAS `initialize` with `_meta.kiro.settings` = 2.28.0 TUI set
    `{codeIntelligence,knowledge,thinking,largeToolOutputHandler:{enabled:true}, workflowNotifications:{enabled:false}}`
    vs same + `subagentOrchestration:{enabled:true}`; then `session/new`; diff advertised tool list (`_kiro/tools/didChange`
    / available tools in `commands/available` analog) for `OrchestrateSubAgent`/delegation tool. Third leg: add
    `workflows:{enabled:true}` to confirm `suppressChatDelegationTool`.
L2. **v2 `model_fallback` frame shape live** (item 1). Cannot force a refusal; instead: `kiro-cli acp` (v2), pick a model
    from `session/new models.availableModels` that the backend marks over-capacity, or `session/set_model` to a
    served-but-entitlement-blocked id; record `KIRO_ACP_RECORD_PATH`; look for `_kiro.dev/session/update`
    `sessionUpdate:"model_fallback"`. Also call `kiro.dev/commands/options {sessionId, command:"model", partial:""}`
    (free) to confirm the per-option `fallback{eligible,readOnly,configured,configuredUnreadable,vendorDefault[]}` block,
    and `kiro.dev/commands/execute {command:{command:"fallback",args:{targetModelId:<cur>}}}` (as a REQUEST) to see the ack.
L3. **v2 trust-classifier shadow** (item 3): v2 with env `KIRO_TRUST_CLASSIFIER_SHADOW=1`
    `KIRO_TRUST_CLASSIFIER_SHADOW_LOG=/abs/tmp/shadow.jsonl`; trigger one shell permission; answer with
    `_meta:{provenance:"human"}` vs none; inspect the log for the verdict + provenance pairing. (Costs one classifier call.)
L4. **`_kiro/session/history` shape** (item 6): KAS session with 2 prompts, then
    `_kiro/session/history {sessionId, beforeMessageId:<omit or last id>, limit:100}`; confirm `{updates[],hasMore,oldestLoadedMessageId}`
    and that `user_message_chunk._meta.kiro` carries `messageId`, `timestamp`, `source`, `displayText`.
L5. **`displayText` persistence** (item 7): `session/prompt {…, _meta:{kiro:{displayText:"short label"}}}` then
    `session/load` (or L4 history) — check the replayed user row shows `_meta.kiro.displayText`.
L6. **memory controls** (item 5): KAS initialize with `settings.memory={mode:"read_only",reflection:false}`; check
    `session/new` result `_meta.memoryConfig` echoes it and `configOptions` contains `memoryReflection`; then
    `session/set_config_option {configId:"memoryReflection",value:"on"}` → expect `config_option_update` push.
    Control: `settings.userMemoryOptIn={enabled:false}` legacy form.
L7. **strictSessionLoad** (item 8): read `initialize` result `agentCapabilities._meta.kiro` on 0.66.26 (expect no
    `strictSessionLoad`); `session/load {sessionId:<random uuid>, cwd, mcpServers:[], _meta:{kiro:{requireExisting:true}}}`
    → does KAS create-on-miss or error? (TUI fallback suggests create-on-miss is the old behaviour.)
L8. **Background control methods** (2.27.0 UI): KAS with `settings.backgroundExecution={enabled:true}`, run a shell tool
    with `run_in_background:true`, then `_kiro/terminal/moveToBackground {sessionId, toolCallId}` and
    `_kiro/terminal/output {sessionId, terminalId, lines:50}` — expect -32601/unknown on 0.66.26 (static: 0 hits); KAS
    has `control_process` + background process manager, so the control may live under another name.
L9. **Windows KAS supervisor** (item 12, Windows VM only): from an installed 2.28.0 MSI run
    `kiro-cli chat _ windows-sandbox` (no `--register`; read-only status) and record the JSON
    `{kind:"windowsSandbox",data:{status}}`; do NOT run `--register` (admin/UAC, shared profiles).

## Files
- FINDINGS.md (this), `vocab/` (per-version literal/prop sets + pairwise deltas + buckets + context),
  `gates.txt` (anchored handshake extraction), `settings-builder.<v>.txt`, `wfnew-prompt.{2.27.1,2.28.0}.txt`.
- Durable: `experiments/conductor-spike/static-tui-vocab-2.28.0.py`, `experiments/conductor-spike/static-tui-gates-2.28.0.py`.
