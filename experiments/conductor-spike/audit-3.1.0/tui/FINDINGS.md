# TUI-bundle static lane: kiro-cli 2.32.0 -> 3.1.0

Inputs: `~/.local/share/kiro-research/tui-bundles/kiro-tui-<ver>.js` for the ten distinct bundle classes 2.32.0 (sha256 eba39f7b…, 13,723,687 B), 2.33.0, 2.33.1, 2.34.0, 2.34.1 (=2.34.2), 2.35.0 (=2.35.1=2.35.2), 2.36.0, 2.37.0, 2.37.1 (=3.0.0) and 3.1.0 (sha256 de0a81d6…, 14,291,547 B); every copy matches its `.sha256` sidecar. KAS carves `~/.local/share/kiro-research/kas-carves/<ver>/node_modules/@kiro/agent/dist/server/acp-server.js` (0.66.43, 0.66.48, 0.66.58, 0.66.62, 0.66.70, 0.66.91) were grepped read-only, only to confirm a receiver or emitter. Nothing from Kiro was executed. cyril verdicts are at HEAD e3a3868d.

Legend: **STATIC** = read off the bundle, so it is a lead; **LIVE** = needs a probe. Every row in this file is STATIC. cyril column: **SENT / NS** (NOT-SENT) for client->agent surface, **H** (HANDLED) / **D** (DROPPED) for agent->client surface, **N/A** = TUI-local with no wire part. "first" = the first bundle in which the literal appears (counts in that bundle, its predecessor and 3.1.0 are in the row).

---

## Summary

1. **The `initialize` handshake changed twice.** `clientCapabilities._meta.kiro` gains `...Vn("c2s")&&{diagrams:!0}` (2.33.1; dark unless the host rollout lists `c2s`), and `.settings` gains **`sessionRecap:{enabled}`** (3.1.0; `chat.enableSessionRecap`, default `false`, forced `false` in `--no-interactive`). KAS 0.66.91 reads `sessionRecap` as a declared client-settings wire key and runs `startRecap()` at turn end, pushing **`session_info_update kind:"recap" {text}`**, which the TUI now consumes (3.1.0). Session recap is **engine-fed**, not TUI-local. cyril: NS + D.
2. **Session credits are now engine-fed.** New `session_info_update kind:"session_usage" {credits:[{usage,unit,unitPlural}], replay?}` (3.1.0; KAS 0.66.91 `SessionUsage.recordTurn/report`, replayed on load) feeds the `Session credits` status segment and `/usage`; the TUI prefers it over its own `turn_summary` sum (`sessionCreditsReported`). Workflow step credits are a `credits[]` array validated on workflow node descriptors and `run_complete` (3.1.0). cyril: D (local sum only; resets on `session/load`).
3. **Thirteen new `_kiro/*` client routes, zero removed.** 2.33.0 `_kiro/modelFallback/{get,set}` (the KAS `/model fallback` path, **dark: KAS 0.66.91 has no such method; it advertises only `modelFallbackReporting`**); 2.33.1 `_kiro/diagram/render` (c2s-gated); 2.34.0 `_kiro/mcp/elicitation` (handled host callback, advertised as `_meta.kiro.mcpElicitation:true`), `_kiro/memory/delete`, `_kiro/permissions/list` (re-fetched on the new consumed notification `_kiro/policy/changed`); 3.1.0 `_kiro/codeIntelligence/{startBuild,cancelBuild,indexStatus}`, `_kiro/configuration/{getState,set}` and `_kiro/memory/getSyncStatus` (memory cloud sync, gate `memory_cloud_sync`). KAS 0.66.91 also **lit up the TUI's 2.32.0 OAuth-completion path**: `_kiro/openExternalUrl` now carries `{url, serverName, oauthCompletion, oauthFlowId?}` and KAS emits `_kiro/mcp/oauthCompleted {serverName, success, oauthFlowId}` (0 -> 2 at 0.66.91) — the 3.1.0 "MCP sign-in links disappear" line.
4. **Permission responses are unchanged on the wire; the TUI only changed what it puts in them.** Typed plan suggestions (2.34.0) ride `reject_once` + `_meta.kiro.rejectionReason`; the subcommand tier (2.36.0) is a `consent.resource` of the form `"git stash *"`; option ORDER (2.34.0) is a TUI sort `{allow_once:0,reject_once:1,allow_always:2,reject_always:3}`. **Approve for me** (2.36.0) is a permissions-file rule `{capability:"all",effect:"evaluate"}` the TUI writes itself (`permissions.yaml|json`), then `_kiro/permissions/list`; it is **dark**: both the TUI (`xC`) and KAS (`resolveClassifierMode`) require `KIRO_TRUST_CLASSIFIER_MODE=enforce` after the `trust_classifier_enforce` rollout gate was removed in 3.1.0. Inbound classifier meta: `tool_call_update._meta.kiro.trustClassifier{decision:"prompt",reason}` and `trustClassifierApproved:true`.
5. **Workspace trust (2.33.0) never reaches KAS.** The prompt gates `session/new` client-side (`pM(e)=agentEngine!=="kas"||workspaceTrust==="trusted"`); the decision persists in a TUI-owned file (`workspaceTrusted`/`workspaceTrustCase`); `--trust-workspace` (2.34.0) only skips the check for one launch. KAS's standalone entry hardcodes `workspaceTrusted:!0`. cyril therefore behaves like `--trust-workspace`.
6. **MCP elicitation (2.34.0) is a host callback the TUI answers** with `{action:"accept",content:{…}}` (form) / `{action:"accept"|"decline"|"cancel"}` (url); `KIRO_DISABLE_MCP_ELICITATION=1` removes the handler. KAS calls it unconditionally when an MCP tool elicits and does not check the `mcpElicitation` flag (0 hits). cyril already answers with a visible decline (H).
7. **KAS spawn and auth are byte-identical after renames**: argv `[prefix,"--experimental-wasm-modules",server,"--transport=stdio","--auth=acp-callback",…]`, env `NODE_USE_SYSTEM_CA`, `KIRO_CUSTOM_USER_AGENT`, `KIRO_OWNED_KAS_{PARENT,CONTROL}`, `clientInfo.name:"kiro-cli"`; `_kiro/auth/getAccessToken` -> `["chat","_","get-kas-token"(,"--force-refresh")]` -> `{kind:"getKasToken",data:{accessToken,expiresAt,profileArn,authMethod?,provider?}}` / `{kind:"error"}`. The 2.37.0 "sign in from a terminal" and 3.1.0 "prompt refused with a sign-in message" lines are host/KAS-side (0 relevant TUI hunks). `KIRO_RUNTIME_DIR` and `acp.agentEngine` have 0 TUI hits (host lane).
8. **V2 is gone from the TUI's own surface but not from its parser.** `--v2`, `--classic`, `--legacy-ui`, `--v3`, `--tui` are accepted-and-ignored (`type:"skip"`, 2.37.1 = 3.0.0); `--agent-engine <v>` is still parsed and forwarded. The 2.34.2 "offers to switch you to V3" bundle is byte-identical to 2.34.1, so that offer lives in the Rust host.
9. **Rollout gates: 10 -> 7 names.** Removed in 3.1.0: `local_sandbox`, `remote_sandbox`, `session_dashboard`, `workflows` (sandbox, dashboard and workflows are GA in the TUI; `/sessions`, `/goal`, `/workflow*`, `/autonomous`, `/tangent`, `/voice` lost their `feature:` gate) and `trust_classifier_enforce` (added 2.34.0). Added: `memory_cloud_sync` (3.1.0). `c2s` call sites 2 -> 14. `KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED` is gone.
10. **Slash commands 74 -> 75** (+`/permissions` @2.34.0, `policies` subcommand); `/model` gains `edit` (2.34.0, TUI-local `chat.modelPicker.hiddenModels`); `/code` gains `summary` (2.33.0); the 3.1.0 `structuredContent` JSON block is a KAS-side change (TUI 0 hits, KAS literal 19 == 19).

---

## Handshake object (what the 3.1.0 TUI sends)

### `initialize.clientCapabilities._meta.kiro` (`_Bt(e)` in 3.1.0; `Vnt(e)` in 2.32.0; `specPhaseCheckpoints` 1 == 1)

```js
{ telemetryEnabled: <telemetry on>,            // 2.7.x
  ...t && { telemetry: QE() },                  // identity object when telemetry is on
  knowledge: true, hooks: { enabled: true, v2: true },
  requirementsAnalysis: true, specPhaseCheckpoints: true, streamingShellContent: true,
  ...process.env.KIRO_INFRA_SAFETY_ROLLOUT_ENABLED === "1" && { infrastructureSafety: true },
  ...Vn("c2s") && { diagrams: true },           // NEW 2.33.1 ({diagrams:!0}: 2.33.0=0, 2.33.1=1, 3.1.0=1); rollout c2s
  ...e && { settings: e } }                     // the AgentSettings object below
```

Plus the **host-capability keys** built from `{type:"other",key,value:!0,method,handler}` entries (`type:"other",key:"` 6 -> 7 literals, the 7th is `mcpElicitation`), which the covenant lists as `KiroClientMeta` flags: `authCallback` (`_kiro/auth/getAccessToken`), `userInput` (`_kiro/userInput`), `openExternalUrl` (`_kiro/openExternalUrl`), `secretStorage` x3 (`_kiro/secret/{get,store,delete}`), and **`mcpElicitation` (`_kiro/mcp/elicitation`, NEW 2.34.0: `mcpElicitation` 2.33.1=0, 2.34.0=4, 3.1.0=4)** — omitted when `KIRO_DISABLE_MCP_ELICITATION==="1"` (`mcpElicitation:(e)=>Wln()?{type:"other"}:Kln(e)`). The 7 parsed `agentCapabilities._meta.kiro` keys are unchanged (`executionTargets, extensionMethods, replayMarking, sessionListScopes, sessionSources, sourceProviders, strictSessionLoad`); `extensionMethods` now gates `_kiro/memory/*`, `_kiro/configuration/*`, `_kiro/modelFallback/*` and `_kiro/memory/getSyncStatus` calls (`this.kiroCapabilities.extensionMethods?.includes(...)`).

cyril (`crates/cyril-core/src/protocol/engine.rs:120-163` `client_capabilities`): sends `settings`, `openExternalUrl` (when the adapter exists), `hooks:{enabled:true[,v2:true]}`. **NS**: `telemetryEnabled`, `knowledge`, `requirementsAnalysis`, `specPhaseCheckpoints`, `streamingShellContent`, `diagrams`, `mcpElicitation`, `userInput`, `secretStorage`, `authCallback` (cyril answers `_kiro/auth/getAccessToken` without the flag, which KAS calls regardless).

### `.settings` (`oL()` in 3.1.0, `B1()` in 2.32.0; anchor `[kas-settings] Built settings for initialize:` 1 == 1)

| key | source | 2.32.0 | 3.1.0 | first |
|---|---|---|---|---|
| `thinking, knowledge, codeIntelligence, checkpoint, tangentMode, disableAutoCompaction, _subagent, _delegate, largeToolOutputHandler` | `chat.enable*` / `chat.disableAutoCompaction` booleans -> `{enabled}` | = | = | — |
| defaults when unset | `{codeIntelligence:!0,knowledge:!0,thinking:!0,largeToolOutputHandler:!0}` | = | = + `sessionRecap:!1` (interactive only) | 3.1.0 |
| **`sessionRecap`** | `if(nz().noInteractive)t.sessionRecap={enabled:!1};else r.sessionRecap=!1,o.push([Ve.CHAT_ENABLE_SESSION_RECAP,"sessionRecap"])` — `chat.enableSessionRecap` (`CHAT_ENABLE_SESSION_RECAP` 2.37.1=0, 3.1.0=8) | absent | `{enabled:<bool>}` always present | **3.1.0** |
| `infraSafetyMonitor`, `infraSafetyEnforce` | only when `KIRO_INFRA_SAFETY_ROLLOUT_ENABLED==="1"` | = | = | — |
| `c2s` | `Vn("c2s")` -> `{enabled: chat.enableC2s===true}` | = | = | — |
| `workflows`, `goal` | 2.32.0 `ZOn(e,t){if(t.available)e.workflows=…}`; 3.1.0 `k_r(e,t){e.workflows={enabled:t.enabled},e.goal={enabled:t.enabled}}` with `o_(){return{enabled:Bo("chat.enableWorkflows",!1)}}` — the rollout `available` check is gone, the keys are always sent | gated | always | 3.1.0 |
| `workflowNotifications` | `{enabled:workflows.enabled, delivery:<chat.defaultInterruptBehavior>}` | = | = | — |
| rollout-gated pairs `C_r` (`E_r` table) | `memory` -> `memoryEnable` etc. | = | = | — |
| `memory` | `PI(e,i)` -> `{mode, reflection}` (`memory.mode`, `memory.reflection`); `memoryEnable:{enabled:!0}` iff `Vn("memory")&&KIRO_INTERNAL&&mode!=="disabled"`; `userMemoryOptIn` iff `!Vn("memory_controls")||disabled` | = | = | — |
| `toolSearch {enabled,minPct?,minTokens?}`, `compaction {enabled,excludePercent?,excludeMessages?}`, `knowledge {enabled,includePatterns?,excludePatterns?,maxFiles?,chunkSize?,chunkOverlap?,indexType?}` | = | = | = | — |

KAS 0.66.91 intake of `sessionRecap`: declared client-settings wire key (`enc=["c2s","verifyFirstWorkflow","sessionRecap","steeringReminders","memoryEnable","userMemoryOptIn","evalStrategy","evalTreatments"]`, identical list in 0.66.43) and `_h("sessionRecap")&&this.startRecap()` at turn end; the zod `AgentSettings` schema still names `_sessionRecap`, so the TUI deliberately uses the extra-key path.

cyril (`crates/cyril-core/src/protocol/kas/settings.rs:77-95,144-202`): sends `thinking, knowledge, codeIntelligence, todoList, checkpoint, tangentMode, disableAutoCompaction, _subagent, _delegate, subagentOrchestration, workflows, goal, toolSearch, compaction, knowledge{…}`. **NS**: `sessionRecap`, `largeToolOutputHandler`, `c2s`, `memory`, `memoryEnable`, `userMemoryOptIn`, `workflowNotifications`, `infraSafety*`. Still sent though absent from the TUI and the KAS schema: `todoList`, `subagentOrchestration` (cyril-53qx, open).

### Per-request `_meta` the TUI attaches

| request | `_meta.kiro` | 2.32.0 -> 3.1.0 |
|---|---|---|
| `session/prompt` | `displayText` (slash-command echo), `outputStyle` (`chat.outputStyle`, prompt-scoped) — `T0t(e,t)` / `B0t(e)` | unchanged (`displayText` 3 == 3; `promptScoped` 1 == 1). cyril **NS** both (cyril-j66p) |
| `session/new` | `KiroNewSessionRequestMeta` as before; region diff around `.newSession({` shows only the `clientInfo.name` literal -> constant `m5="kiro-cli"` | unchanged |
| `session/load` | unchanged (`.loadSession({` 3 == 3, normalised diff empty) | unchanged |
| `session/request_permission` response | `Kk(e,t,n)`: `{kiro:{consent:{...capability?, scope, ...resource?, ...workspaceRoot?}, ...rejectionReason?}}` (`kiro:{consent` 1 -> 2). 3.1.0 scope default: `t==="allow_always"||kxt(t)&&(n?.kasResource!==void 0||n?.kasWholeCapability===!0)?"session":"invocation"` (a `reject_always` with a resource now also defaults to `session`); `resource` may now be an exact command, a **subcommand tier `"<cmd> <sub> *"`** (2.36.0, `z_()`), a pattern `"<cmd> *"`, a folder glob `"<dir>/**"` or `"*"` (whole capability) | shape unchanged; new values. cyril **SENT** for allow_always grants (`crates/cyril-core/src/protocol/convert/kas.rs:150-191`: capability, scope, resource, workspaceRoot) and `rejectionReason` for reject_once (`kas.rs:200-224`) |
| `session/set_config_option` | no `_meta`. configIds the TUI sets: literals `autopilot, effortLevel, mcpSandboxing, mode, model, sandbox, sandboxNetworkMode, thinking` (= 2.32.0) plus the variable ids `workflowLaunch` (`ew`, **2.34.0**: `"workflowLaunch"` 2.33.1=0, 2.34.0=2) and `memoryReflection` (4 == 4) | +`workflowLaunch`. cyril sets `model, thinking, mode, effortLevel` (`crates/cyril-core/src/commands/mod.rs:1596,2023,2327,2690`); **NS** `workflowLaunch`, `memoryReflection` |

---

## Deltas

Counts are raw occurrences from `static-tui-ctx-3.1.0.py` (OLD = 2.32.0, NEW = 3.1.0, plus the first bundle). KAS counts are from the carve named in the row.

| # | item | first | gate / default | direction | S/L | cyril verdict + file:line | issue / note |
|---|---|---|---|---|---|---|---|
| T1 | **`clientCapabilities._meta.kiro.diagrams:true`** — `...Vn("c2s")&&{diagrams:!0}` in the clientMeta builder (`{diagrams:!0}` 2.33.0=0, 2.33.1=1, 3.1.0=1). KAS 0.66.91 reads it (`diagrams:r.diagrams===!0`) and then advertises `_kiro/diagram/render` + `diagramRenderFormats` | 2.33.1 | rollout `c2s` (off unless the host lists it) | outbound initialize | STATIC | **NS** — `crates/cyril-core/src/protocol/engine.rs:120-163` builds `settings`/`openExternalUrl`/`hooks` only | NOTE cyril-68ny |
| T2 | **`settings.sessionRecap:{enabled}`** — `if(nz().noInteractive)t.sessionRecap={enabled:!1};else r.sessionRecap=!1,o.push([Ve.CHAT_ENABLE_SESSION_RECAP,"sessionRecap"])` (`CHAT_ENABLE_SESSION_RECAP` 2.37.1=0, 3.1.0=8). KAS 0.66.91: declared wire key (`enc=[…,"sessionRecap",…]`, same list in 0.66.43) → `_h("sessionRecap")&&this.startRecap()` | 3.1.0 | `chat.enableSessionRecap`, default **false**; `/settings` Features "Session recap · one model call per turn" (available only on kas) | outbound initialize settings | STATIC | **NS** — `crates/cyril-core/src/protocol/kas/settings.rs:77-95` has no `sessionRecap` row | NEW P3 (with T3) |
| T3 | **consumed `session_info_update kind:"recap" {text}`** → `{type:"session_recap",text}` only for the primary session (`if(r?.kind==="recap"){if(!t.isPrimarySession)return null;let o=Wxt(r.text);…}`; `kind==="recap"` 0 -> 1); recap panel states incl. `dwelling` (0 -> 3). KAS emitter `handleRecap` → `send({kind:"recap",text})` exists in every carve (2 == 2) | 3.1.0 | needs T2 on | inbound kind | STATIC | **D** — `crates/cyril-core/src/protocol/convert/kas.rs:626-700` dispatches 6 kinds; `kas.rs:822 _ => None` drops `recap` silently | NEW P3 (with T2) |
| T4 | **consumed `kind:"session_usage" {credits:[{usage,unit?,unitPlural?}], replay?}`** → `{type:"session_usage",credits:[{value,unit,unitPlural}],replay}` (`z5` allows ≤16 entries; `"session_usage"` 0 -> 4). Feeds `applySessionUsage` → footer `Credits: turn X • session Y` with `sessionCreditsReported` taking precedence over the local `turn_summary` sum, and the new status segment `sessionCredits:{label:"Session credits",description:"Credits this session has used"}` (0 -> 1). KAS 0.66.91: `SessionUsage.recordTurn(promptTurnSummaries)`/`report()` (`kind:"session_usage"` 0.66.70=0, 0.66.91=2), replayed on load (3.1.0 "footer total no longer resets") | 3.1.0 | none | inbound kind | STATIC | **D** — `kas.rs:822`; cyril keeps a local sum (`crates/cyril-core/src/types/session.rs:627-662` `SessionCost.total_credits`) that starts at 0 on `session/load` | **NEW P2**; NOTE cyril-8alk |
| T5 | workflow **`credits[]`** on node descriptors and `run_complete` (`kbe(e)`: `[acp-client] Ignored invalid KAS workflow credits` 0 -> 1; same `z5` validator) → "Workflow step rows show the credits each step used" | 3.1.0 | none | inbound `_kiro/workflow/*` field | STATIC | **D** — `crates/cyril-core/src/protocol/convert/kas/workflow.rs` node/run structs have no `credits` field (grep 0; serde ignores) | NOTE cyril-4u4a (kas-workflow lane owns the emitter) |
| T6 | **`_kiro/modelFallback/get {sessionId}` / `set {sessionId,targetModelId,fallbackModelId?,disabled?}`** (0 -> 1 each) behind `/model fallback` on KAS; refused with `"This agent does not support configuring a model fallback. Update the agent and try again."` unless `extensionMethods` lists it. **KAS 0.66.43…0.66.91: 0 hits for `"_kiro/modelFallback/`**; KAS has only the config krn prefix `modelFallback/` and the capability `modelFallbackReporting:!0` | 2.33.0 | extensionMethods (never advertised) → **dark** | outbound request | STATIC | **NS** (no hit in `crates/`) | NOTE cyril-aoby, cyril-lnxg |
| T7 | **`_kiro/diagram/render {sessionId,diagramId,format}`** → `{resource:{text}}`, refused over 32,768 B (`source_limit`); `if(!Vn("c2s"))return{ok:!1,reason:"missing"}` (0 -> 1; KAS 0.66.48+ = 4) | 2.33.1 | rollout `c2s` + T1 | outbound request | STATIC | **NS** | NOTE cyril-68ny |
| T8 | **`_kiro/mcp/elicitation` handled** — `{type:"other",key:"mcpElicitation",value:!0,method:"_kiro/mcp/elicitation",handler}`; validator `H_r`: `mode` defaults `"form"` (needs `requestedSchema`), `"url"` needs `url`, else `{action:"decline"}`; malformed → `{action:"cancel"}`. Replies: form submit `a({action:"accept",content:D.content})`; url `Open in browser`/`I'll open it myself` → `{action:"accept"}`, `Decline`, `Cancel`. `KIRO_DISABLE_MCP_ELICITATION==="1"` drops the handler. KAS 0.66.91 calls `Pm(e,"_kiro/mcp/elicitation",{sessionId,toolCallId:operationId,elicitation})` unconditionally and maps a failure to `{action:"cancel"}`; `mcpElicitation` 0 hits in KAS | 2.34.0 (`mcpElicitation` 2.33.1=0, 2.34.0=4) | env opt-out only | inbound request | STATIC | **H** (visible default decline) — `crates/cyril-core/src/protocol/kas/callbacks.rs:597`, `crates/cyril-core/src/protocol/kas/mcp.rs:1-60` | NEW P3 (interactive form/url) |
| T9 | **`_kiro/permissions/list {sessionId}`** → `{rules:[{capability,effect,scope,source,match?,exclude?}]}` → `permission_rules_update {rules, unreadable?}` (0 -> 3); refreshed on the **new consumed notification `_kiro/policy/changed`** (`onExtNotification(rEr,…)` through a variable; `isForeignSessionPush` filter). KAS: list 3 -> 4, changed 2 == 2 | 2.34.0 | none | outbound request + inbound notification | STATIC | **NS** / **D** — `engine.rs:375-408` → `crates/cyril-core/src/protocol/convert/kiro.rs:1305 _ => Ok(None)` | NOTE cyril-r5pl |
| T10 | **Approve for me** — `/permissions policies approve-for-me` writes rule `pit={capability:"all",effect:"evaluate"}` into the workspace or user permissions file (`Uy=["permissions.yaml","permissions.json"]`; user dir `join(kiroHome,"settings")`), detected back as `preset:approve-for-me in <file>` or an equal rule; then `refreshPermissionRules()` (T9). Gate `xC(e)=e==="kas"&&KIRO_TRUST_CLASSIFIER_MODE?.trim().toLowerCase()==="enforce"`; 2.34.0–2.37.1 also had one `trust_classifier_enforce` rollout call site, removed in 3.1.0. KAS 0.66.91 `resolveClassifierMode` is env-only too (`KIRO_TRUST_CLASSIFIER_MODE` 1) | 2.34.0 (`Approve for me` 2.33.1=0, 2.34.0=2, 2.36.0=5) | env `enforce` on both sides → **dark** | TUI-local file write; KAS file watcher → `_kiro/policy/changed` | STATIC | **NS** (cyril writes no permissions files; grep 0) | NEW P4 (dark) |
| T11 | inbound classifier meta: `request_permission._meta.kiro.trustClassifier {decision:"prompt",reason}` (`Gmt`) → flag line under the approval; `tool_call(_update)._meta.kiro.trustClassifierApproved:true` (0 -> 4) → "approved by Kiro". KAS emits `_meta:{kiro:{trustClassifier:R.meta}}` on a pending `tool_call_update` (0.66.58+) | 2.34.0 | T10 | inbound meta | STATIC | **D** — permission meta reader `convert/kas.rs:80-110` reads only `consent`; tool-call meta not read | fold into T10 |
| T12 | **subcommand tier** — `z_()`: `patternResource="<argv0> *"`, `subcommandResource="<argv0> <argv1> *"` when argv1 matches `/^[A-Za-z][A-Za-z0-9_-]*$/`, argv0 ∉ `{sudo,doas,env}` and ≠ `*`; scope page rows `exact match | subcommand | pattern | entire tool`, each `meta:{kasScope,kasResource}` → `consent.resource` (`subcommandResource` 2.35.0=0, 2.36.0=5) | 2.36.0 | KAS only | outbound permission-response value | STATIC | SENT-capable (`convert/kas.rs:150-191` echoes `grant.resource().wire()`), but the tier is never offered: `crates/cyril-core/src/always_allow.rs:624-643` derives exact + base pattern only → **NS** | NEW P4 |
| T13 | approval **option order** — `w$r={allow_once:0,reject_once:1,allow_always:2,reject_always:3}` (`reject_once:1` 2.33.1=0, 2.34.0=1); the response is unchanged | 2.34.0 | — | TUI-local | STATIC | N/A | — |
| T14 | **typed plan suggestions** — a workflow-plan approval answered with text: `reject_once` + `_meta.kiro.rejectionReason:<text>`; TUI-local `planReply:{sessionId, reply:{kind:"edits-requested",request:c}|{kind:"denied"}}` recorded on the tool_use row (`"edits-requested"` 2.33.1=0, 2.34.0=1; `.kiro?.rejectionReason` read 2.33.0) | 2.34.0 | — | outbound (existing frame) | STATIC | **SENT-capable** — `convert/kas.rs:200-224` `attach_rejection_metadata` (RejectWithReason) | — |
| T15 | `request_permission._meta.kiro.workflowPlan` read (0 -> 3) for the compact step tree with expandable details | 2.33.0 | — | inbound request meta | STATIC | **D** — `convert/kas.rs:80-110` | NOTE cyril-4u4a |
| T16 | **workspace trust** — `wgt()` gates startup: `if(agentEngine!=="kas")return!0; if(noInteractive)skip; if(cliArgs.trustWorkspace)skip(one launch)`; `resolveWorkspaceTrust(cwd)` reads a TUI-owned file (`wue="workspaceTrusted"`, `r6="workspaceTrustCase"`, written mode 0700/`AAt`); `pM(e)=e.agentEngine!=="kas"||e.workspaceTrust==="trusted"` blocks prompting. Decline → exit "Not trusted: … Kiro exited without loading this folder." (`workspaceTrust` 0 -> 70 @2.33.0; `trustWorkspace` 0 -> 2 @2.34.0). **No frame to KAS**: KAS's standalone entry passes `workspaceTrusted:!0` (2 hits) and only reads it for protected-write pre-approval / ignore files | 2.33.0 (`--trust-workspace` 2.34.0) | KAS engine only | none (TUI-local) | STATIC | N/A — cyril spawns KAS and calls `session/new` directly, i.e. always `--trust-workspace` | NOTE cyril-mq15 |
| T17 | **`_kiro/memory/delete {sessionId?, id}`** (`Nbe(this)` adds `sessionId` when known; gated `extensionMethods`; `no memory with id` → false) for `/memories` delete (0 -> 2; KAS 4 == 4 then 5) | 2.34.0 | memory on | outbound request | STATIC | **NS** (grep `_kiro/memory` 0) | NOTE cyril-0na7 |
| T18 | **memory cloud sync** — `_kiro/configuration/getState {observer:"connection"}` → `{settings,layers}`; `_kiro/configuration/set {krn:"krn:::setting/memorySync", value:"on"|"off", layer:"user-synced"|"user-local"}` (error `data:{reason:"locked",layer}` → `MemorySyncLockedError`); `_kiro/memory/getSyncStatus {}`; memory rows read `_meta.kiro.cloudState`; `/memories` sync toggle. TUI 0 -> 1 each; KAS getState 8 -> 9, set 0 -> 4 (0.66.62), getSyncStatus 0 -> 6 (0.66.70) | 3.1.0 | rollout `memory_cloud_sync` (1 call site) | outbound requests + inbound meta | STATIC | **NS** / **D** | NOTE cyril-0na7, cyril-iq2c |
| T19 | **`_kiro/codeIntelligence/startBuild {sessionId}`** → `{buildId,status("joined"…),workspaceRoot?}`, `cancelBuild`, `indexStatus`; `code_index_status` UI events (0 -> 1 each; KAS 0 -> 5, advertised under `..._h("c2s")?ZEr:[]`) | 3.1.0 | KAS `c2s` | outbound requests | STATIC | **NS** | NOTE cyril-1aq0 |
| T20 | **`session/set_config_option {configId:"workflowLaunch", value}`**, values `when-asked|large-tasks|delegate-all` (`ew="workflowLaunch"`, `"workflowLaunch"` 2.33.1=0, 2.34.0=2); restored after `session/new` from `chat.workflowLaunch`; `workflow_launch_update` event; `/settings` Features "Workflow use" | 2.34.0 | workflows enabled | outbound request | STATIC | **NS** — cyril sets `model, thinking, mode, effortLevel` only (`commands/mod.rs:1596,2023,2327,2690`) | NEW P3 |
| T21 | `content`/`tool_call` **`_meta.kiro.replay===true`** → source `"replay"` (`.kiro?.replay` added 2.34.1) | 2.34.1 | `replayMarking` | inbound meta | STATIC | **D** | NOTE cyril-99ds |
| T22 | `interaction_resolved` reads `consentRound` and `replay` (`consentRound` 0 -> 14 @2.34.0; KAS 35 -> 40) | 2.34.0 | — | inbound kind | STATIC | **D** — `kas.rs:822` | NOTE cyril-58uv |
| T23 | `/model edit` → `chat.modelPicker.hiddenModels` (0 -> 1), picker hides listed ids | 2.34.0 | — | TUI-local | STATIC | N/A | — |
| T24 | `/code summary` subcommand (`summary:"Generate codebase documentation from the overview"` 0 -> 1) on the existing `_kiro/codeIntelligence` method | 2.33.0 | — | outbound (existing) | STATIC | **NS** | NOTE cyril-1aq0 |
| T25 | **`KIRO_DEFAULT_SETTINGS`** (+`KIRO_DEFAULT_SETTINGS_SKIP_OWNER_CHECK`): root-owned defaults file layered under `cli.json`; Linux/macOS only, absolute path, regular file, root-owned, not group/other-writable, ≤1 MiB (`too_large`, `malformed_json`, …) | 3.1.0 | env | TUI-local settings layer | STATIC | N/A (cyril's marshal reads the user `cli.json` only) | NOTE cyril-q8gq |
| T26 | env/banners: `KIRO_ELEVATED_PRIVILEGES` (2.35.0; root/Administrator warning, `app.warnOnElevatedPrivileges`), `KIRO_AGENT_UPGRADE_NOTICE` (2.37.0; host banner), `KIRO_LAUNCH_3_0` (2.37.0; 3.0 welcome variant), `KIRO_STANDALONE_NOTICE` (2.33.1, removed 3.1.0), `KIRO_DISABLE_OBSERVER_RESUME` (2.33.0; `e==="kas"&&…!=="1"`), `KIRO_TEST_POLICY_HOME` (2.33.0; tests) | as listed | env | TUI-local | STATIC | N/A | — |
| T27 | host IPC `chat _ upgrade-agent --action conflicts --cwd <dir> --agent <name>` → `{kind:"permissionConflicts",data:{agents}}` (0 -> 1 @2.36.0) for the per-session "V3 rules block what V2 allowed" notice | 2.36.0 | KAS | host IPC | STATIC | N/A (host lane) | — |
| T28 | CLI flags `{type:"skip",flags:["--tui","--v3","--v2","--legacy-ui","--classic"]}` (`"--legacy-ui","--classic"` 2.37.0=0, 2.37.1=1); `{type:"string",key:"agentEngine",flags:["--agent-engine"],acp:"--agent-engine"}` kept; `==="v2"` 12 == 12 (session-list/telemetry classification only) | 2.37.1 (=3.0.0) | — | TUI-local | STATIC | N/A | — |
| T29 | `structuredContent` extra JSON block: **TUI 0 hits in every bundle**; KAS literal 19 == 19 → engine-side rendering change | 3.1.0 | — | — | STATIC | — (kas-core lane) | — |
| T31 | **OAuth completion path lit up (KAS side).** TUI unchanged since 2.32.0 (`oauthCompletion` 3 == 3, `"_kiro/mcp/oauthCompleted"` 1 == 1; handler requires `serverName` string, `success` boolean and a recorded attempt with `completion:"notification"`; `oauthFlowId` 0 — not read). KAS 0.66.91: `openExternalUrl({url, serverName, oauthCompletion, ...oauthFlowId})` and `notifyOAuthCompleted` → `extNotification("_kiro/mcp/oauthCompleted",{serverName,success,oauthFlowId})`, only when the client advertised `openExternalUrl` (`oauthCompleted` 0.66.43…0.66.70=0, 0.66.91=2) | KAS 0.66.91 (TUI 2.32.0) | client `openExternalUrl:true` | inbound request params + inbound notification | STATIC | cyril advertises `openExternalUrl` (`engine.rs:143`) and handles `_kiro/openExternalUrl` reading only `url` (`crates/cyril-core/src/protocol/kas/callbacks.rs:193`) → `serverName`/`oauthCompletion` **D**; `_kiro/mcp/oauthCompleted` **D** (`convert/kiro.rs:1305`) | NOTE cyril-nk4o / cyril-tqr8 (closed) → reopen-or-new P3 |
| T30 | TUI-only, unchanged since 2.32.0: OSC 7501 (`KIRO_NO_PROGRAM_STATUS` 1 == 1), `chat.terminalTitle` 1 == 1, `chat.allowIcons` 1 == 1, output-style picker (`"output_style"` 1 == 1, `promptScoped` 1 == 1, `chat.outputStyle` 1 == 1), memory controls (`memory.mode` 2 == 2, `memory.reflection` 3 == 3, `userMemoryOptIn` 1 == 1, `memoryReflection` configId 4 == 4) | — | — | — | STATIC | cyril-j9r4, cyril-j66p, cyril-0na7 unchanged | — |

### Negative results (load-bearing)

| check | 2.32.0 | 3.1.0 | evidence |
|---|---|---|---|
| `_kiro/*` literal set | 64 | 77 (+13, −0) | `tmp/sets/sets-diff.txt` |
| `kiro.dev/*` literal set | 38 | 38 | same |
| `onExtNotification("<literal>")` | 11 | 11 (`_kiro/policy/changed` registers through a variable; the census misses it) | `consumers-2.32.0-3.1.0.txt` |
| slash literals / `name:"/x"` defs | 74 / 37 | 75 / 38 (+`/permissions`) | `tmp/sets/sets-3.1.0.slashcmd*.txt` |
| hidden `chat _` subcommands | 11 | 11 (`ensure-session, export-session, get-kas-token, get-telemetry-credentials, import-session, midway-expiry, refresh-feed, upgrade-agent, windows-kas-process, windows-kas-stop, windows-sandbox`) | gates file |
| settings builder rows | 12 | 12 (+ the `sessionRecap` row the fixed pattern misses) | gates file + hand-read region |
| prompt `_meta.kiro` keys | displayText, outputStyle | = | gates file |
| agentCapabilities parsed | 7 | 7 | gates file |
| KAS spawn argv / env | see §KAS spawn | renames only | `--diff '"--transport=stdio"'` |
| `getKasToken` handler | `Rsr` | `$Wr`, byte-identical logic | ctx dump |
| `configId:"…"` literals | 8 | 8 (+ variables `workflowLaunch`, `memoryReflection`) | inline python |
| `subagentOrchestration` / `todoList` in the TUI | 0 / 0 | 0 / 0 | cyril still sends both (cyril-53qx) |
| `KIRO_RUNTIME_DIR`, `acp.agentEngine`, `structuredContent`, `PreCompact` | 0 | 0 | host / kas lanes |
| `sessionCredits` token | 8 | 32 | T4 |

---

## Methods and kinds (set diffs)

### `_kiro/*` literals the TUI calls or handles — 64 -> 77

| literal | first | direction | notes |
|---|---|---|---|
| `_kiro/modelFallback/get`, `_kiro/modelFallback/set` | 2.33.0 | client→agent request | T6; **no KAS receiver through 0.66.91** |
| `_kiro/diagram/render` | 2.33.1 | client→agent request | T7; c2s |
| `_kiro/mcp/elicitation` | 2.34.0 | agent→client request (handled) | T8 |
| `_kiro/memory/delete` | 2.34.0 | client→agent request | T17 |
| `_kiro/permissions/list` | 2.34.0 | client→agent request | T9 |
| `_kiro/policy/changed` | 2.34.0 | agent→client notification (consumed) | T9 |
| `_kiro/codeIntelligence/startBuild`, `/cancelBuild`, `/indexStatus` | 3.1.0 | client→agent request | T19 |
| `_kiro/configuration/getState`, `_kiro/configuration/set` | 3.1.0 | client→agent request | T18 (first TUI use; KAS had getState since ≤0.66.43, set since 0.66.62) |
| `_kiro/memory/getSyncStatus` | 3.1.0 | client→agent request | T18 |

Removed: none. `kiro.dev/*`: 38 == 38 (v2 surface frozen). The consumed ext-notification set by literal is unchanged (`_kiro/customAgent/not_found, _kiro/error/rate_limit, _kiro/governance/state, _kiro/mcp/governance_disabled, _kiro/mcp/oauthCompleted, _kiro/mcp/status, _kiro/sandbox/status, _kiro/sessions/changed, _kiro/spec/phaseCheckpoint, _kiro/spec/taskStatusChanged, _kiro/tools/content_chunk`) plus `_kiro/policy/changed` via variable.

### `session_info_update` kinds the TUI consumes

Unchanged dispatcher `Ofe` plus two new branches: `kind==="session_usage"` (T4) and `kind==="recap"` (T3). `interaction_resolved` now also reads `consentRound` and `replay` (T22). The unmapped-kind set `Tfe` (silenced) is unchanged in shape; anything else logs `KAS session update (not yet mapped)`. `kind===` literal set 81 -> 142 overall, but the only wire-kind additions are `recap`, `session_usage` (and the TUI-local `dwelling`).

### `sessionUpdate` variants

No new `sessionUpdate:"…"` literal (set diff empty in every pair). `_meta.kiro` reads on existing variants: `+rejectionReason` (2.33.0, own response), `+trustClassifier` (2.34.0, T11), `+replay` (2.34.1, T21), `+cloudState` (3.1.0, T18); `workflowPlan` (2.33.0, T15) and `trustClassifierApproved` (2.34.0, T11) are read through validators rather than `?.` chains.

### Stream-event types (TUI-internal, for orientation)

New `type===` literals: `mcp_elicitation_request`, `workflow_launch_update` (2.34.0); new `case"…"`: `permission_rules_update` (2.34.0), `session_recap`, `session_usage`, `code_index_status`, `cloud_session_unsupported`, `too_large`, `source_limit` (3.1.0), `invalid_ir`, `unsupported_shape` (2.33.1, diagram IR).

---

## Slash commands

**Set diff: +1.** 74 -> 75 literals, 37 -> 38 `name:"/x"` definitions; only `/permissions` is new:

`{name:"/permissions",description:"Manage how Kiro handles permissions",meta:{subcommands:["policies"],subcommandDescriptions:{policies:"Choose whether Kiro asks or approves for you"},subcommandsOptional:!0}}` (2.34.0).

Definition changes (rename-normalised diff of every `{name:"/…"}` object):

| command | change | first | class |
|---|---|---|---|
| `/permissions` | new; `policies` → picker `ask for approval` / `approve for me` (T10) | 2.34.0 | TUI-local file write + `_kiro/permissions/list` / `_kiro/policy/changed` (KAS-method-backed listing) |
| `/model` | `subcommands:["set-current-as-default","edit"]`, `edit:"Choose which models the picker lists"` (T23); `/model fallback [<id>|clear]` dispatch (`o==="fallback"`) → on KAS `_kiro/modelFallback/*` (T6, dark), on v2 the `fallback{}` options | 2.34.0 (edit), 2.33.0 (fallback) | TUI-local (edit); KAS-method-backed but unreachable (fallback) |
| `/code` | description `…codebase overview, and documentation`; `subcommands:[...z6]` + `summary:"Generate codebase documentation from the overview"` (T24); `/code init` now drives `_kiro/codeIntelligence/startBuild` with `code_index_status` progress when KAS advertises it (T19) | 2.33.0 (summary), 3.1.0 (startBuild) | KAS-method-backed |
| `/autonomous`, `/goal`, `/sessions`, `/tangent`, `/voice`, `/workflow`, `/workflow-cancel`, `/workflow-resume`, `/workflow-run`, `/workflow-status`, `/workflows` | `feature:"remote_sandbox" | "workflows" | "session_dashboard" | "tangent" | "voice"` gate keys removed (GA) | 3.1.0 | unchanged routes |
| `/memories` | + delete (`_kiro/memory/delete`, T17); + cloud-sync toggle and `cloudState` badges under `memory_cloud_sync` (T18) | 2.34.0 / 3.1.0 | KAS-method-backed |
| `/usage` | + "Session credits" from `session_usage` (T4); enterprise overage/consumed-credits columns (2.34.0, `overage` 91 -> 103) from the existing `_kiro/account/getUsage` reply | 3.1.0 / 2.34.0 | KAS-method-backed |
| `/settings` Features | + "Workflow use" (`chat.workflowLaunch` → `set_config_option configId:"workflowLaunch"`, T20); + "Session recap" (`chat.enableSessionRecap`, T2; available only when `agentEngine==="kas"`, applied at the next `initialize`) | 2.34.0 / 3.1.0 | KAS-method-backed (workflowLaunch) / handshake setting (recap) |
| `/trust` | **does not exist** (0 hits); trust is the startup prompt + `--trust-workspace` (T16) | — | TUI-local |
| `/sandbox`, `/spec`, `/upgrade-agent`, `/workflow new` | definitions unchanged; `/upgrade-agent` + `kiro-cli agent upgrade` warnings are host-side (T27) | — | unchanged |

Status-line segments (`ron` map): `approveForMe:"local-only"` and `sessionCredits` are new segment ids (3.1.0 for `sessionCredits`; `approveForMe` 2.34.0).

---

## KAS spawn and auth (OLD vs NEW)

| item | 2.32.0 | 3.1.0 |
|---|---|---|
| server path | `process.env.KIRO_KAS_SERVER_PATH` else walk up to `node_modules/@kiro/agent/dist/server/acp-server.js` (10 levels); node `process.env.KIRO_KAS_NODE_PATH||"node"` | identical |
| argv | `[...prefix,"--experimental-wasm-modules",server,"--transport=stdio","--auth=acp-callback",...policyArgs,...krsEndpoint?["--endpoint=…"],...cpsEndpoint?["--control-plane-endpoint=…"],...region?["--region=…"]]` | identical (`--auth=acp-callback` 1 == 1) |
| env | `{...process.env, NODE_USE_SYSTEM_CA: env??"1", NODE_CHANNEL_FD: undefined, NODE_CHANNEL_SERIALIZATION_MODE: undefined, ...(ownsProcess?{KIRO_OWNED_KAS_PARENT:String(pid), KIRO_OWNED_KAS_CONTROL:<token>}:{}), KIRO_CUSTOM_USER_AGENT:"KiroCLI/<ver>…"}` (`KIRO_OWNED_KAS_CONTROL` 1 == 1) | identical; the factory option `localSandboxAvailable` is gone (sandbox GA) |
| Windows supervisor | `chat _ windows-kas-process` / `chat _ windows-kas-stop <token>` (10 s timeout) | identical |
| `clientInfo` to KAS | `{name:"kiro-cli",version,_meta}` | `{name:m5="kiro-cli",version,_meta}` (literal hoisted to a constant); host-side ACP connection `name:"kiro-tui"` likewise `p5` |
| `_kiro/auth/getAccessToken` responder | `Rsr`: `["chat","_","get-kas-token"]` + `"--force-refresh"` iff `forceRefresh`; `kind:"error"` → throw "Failed to verify authentication. Please log in again to continue."; `kind!=="getKasToken"` → throw; returns `{accessToken,expiresAt,profileArn,...authMethod?,...provider?}` | `$Wr`: **byte-identical logic** (`getKasToken` 1 == 1, `force-refresh` 3 == 3 — one is the telemetry-credentials helper, one an Emacs keyword table) |
| `acp.agentEngine`, `KIRO_RUNTIME_DIR` | 0 | 0 — host lane (the launcher's own `kiro-cli acp` path) |
| 2.37.0 "ACP clients can sign in from a terminal when `kiro-cli acp` starts logged out" | — | no TUI hunk (`kiro-cli login` 16 == 16, `"auth_error"` 4 == 4); the only sign-in copy added in 3.1.0 is memory cloud sync ("Sign in to sync your memories.", "Cloud sync is not available with an API key sign-in.") |
| 3.1.0 "prompt refused with a sign-in message when Kiro cannot get a sign-in token" | — | not in the TUI (`auth_error` consumer unchanged); KAS copy `Sign in` 1 -> 2 (kas-core lane) |

cyril (`crates/cyril-core/src/protocol/kas/auth.rs:44-46,557-560,859,872-907`): argv `["chat","_","get-kas-token"(,"--force-refresh")]`, envelope `getKasToken`/`error`, fields `accessToken, expiresAt, profileArn, authMethod?, provider?` — **matches the 3.1.0 TUI exactly**; no change needed for cyril-zbu7 (closed). cyril-0rn8 (open: logged-out `get-kas-token` hang) is unaffected by the TUI; the live lane should re-check the logged-out behaviour on the 3.1.0 launcher (TUI-L7).

---

## Rollout gates and dark features

Gate function: `Rn` (2.32.0) → `Ln`/`En`/`Bn`/`vn`/`In`/`Tn` → **`Vn(e){return uXn().has(e)}`** (3.1.0). The `local_sandbox` special case (`process.env.KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED==="1"`) and the `e!=="voice"||…` tail are gone; `Xa(e){return e==="kas"&&Rn("local_sandbox")}` became `Ha(e){return e==="kas"}`.

| gate | 2.32.0 call sites | 3.1.0 call sites | change |
|---|---|---|---|
| `background_execution` | 4 | 4 | — |
| `c2s` | 2 | **14** (10 @2.33.1, 11 @2.34.1) | gates `diagrams:!0` (T1), `_kiro/diagram/render` (T7), code-index analysis scopes, `c2s` setting |
| `cloud_config` | 12 | 12 | — |
| `managed_settings` | 5 | 4 (−1 @2.35.0) | — |
| `memory` | 2 | 2 | — |
| `memory_controls` | 2 | 2 | — |
| `memory_cloud_sync` | 0 | **1** | NEW 3.1.0 (T18) |
| `local_sandbox` | 5 | **0** | removed 3.1.0 (sandbox GA on KAS) |
| `remote_sandbox` | 4 | **0** | removed 3.1.0 |
| `session_dashboard` | 3 | **0** | removed 3.1.0 (`/sessions` GA) |
| `workflows` | 2 | **0** | removed 3.1.0 (workflows GA; `chat.enableWorkflows` default false still decides `settings.workflows.enabled`) |
| `trust_classifier_enforce` | 0 | 0 | added 2.34.0 (1 site), removed 3.1.0 (T10 now env-only) |

`KIRO_*` env set 95 -> 103: +`KIRO_DISABLE_OBSERVER_RESUME`, `KIRO_TEST_POLICY_HOME` (2.33.0), +`KIRO_STANDALONE_NOTICE` (2.33.1, −3.1.0), +`KIRO_DISABLE_MCP_ELICITATION`, `KIRO_TRUST_CLASSIFIER_MODE` (2.34.0), +`KIRO_ELEVATED_PRIVILEGES` (2.35.0), +`KIRO_AGENT_UPGRADE_NOTICE`, `KIRO_LAUNCH_3_0` (2.37.0), +`KIRO_DEFAULT_SETTINGS`, `KIRO_DEFAULT_SETTINGS_SKIP_OWNER_CHECK` (3.1.0), −`KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED` (3.1.0). `AB_*` 0 == 0.

| dark / gated feature | gate | default | evidence |
|---|---|---|---|
| **`/model fallback` on KAS** (`_kiro/modelFallback/*`) | `agentCapabilities._meta.kiro.extensionMethods` must list it; KAS 0.66.91 lists `_kiro/knowledge, _kiro/memory/*, _kiro/codeIntelligence, _kiro/environment/*, _kiro/session/*, _kiro/job/*?, _kiro/config/template, _kiro/configuration/{getState,set}, _kiro/workflow/*, _kiro/sourceProviders/*?, _kiro/diagram/render?, _kiro/codeIntelligence/{startBuild,cancelBuild}?` — never modelFallback | **dark** (TUI ahead of KAS) | KAS `"_kiro/modelFallback/` 0 in six carves; TUI copy "This agent does not support configuring a model fallback" |
| **Approve for me** (trust classifier) | `KIRO_TRUST_CLASSIFIER_MODE=enforce` on TUI (`xC`) and KAS (`resolveClassifierMode`) | dark | T10/T11 |
| `diagrams:true` + `_kiro/diagram/render` + diagram IR viewer (`invalid_ir`, `unsupported_shape`, `source_limit`) | rollout `c2s` | off | T1/T7 |
| memory cloud sync | rollout `memory_cloud_sync` + `extensionMethods` ⊇ {getState,set} | off | T18 |
| MCP elicitation | on; `KIRO_DISABLE_MCP_ELICITATION=1` opt-out | on | T8 |
| session recap | `chat.enableSessionRecap` | off | T2/T3 |
| infrastructure safety | `KIRO_INFRA_SAFETY_ROLLOUT_ENABLED=1` | off | unchanged |
| `KIRO_DEFAULT_SETTINGS` | env | unset | T25 |
| `_kiro/mcp/oauthCompleted` (TUI 2.32.0) | **no longer dark**: KAS 0.66.91 sends `oauthCompletion` on `_kiro/openExternalUrl` and emits `_kiro/mcp/oauthCompleted` when the client advertised `openExternalUrl` (`oauthCompleted` 0 -> 2 at 0.66.91) | live when `openExternalUrl:true` | T31 |

---

## Leads for the live lane

Every leg: fresh `HOME=<tmp>` with the real `XDG_DATA_HOME` (trap 7); direct `node acp-server.js` from the 3.1.0 carve on the carved 3.1.0 node (trap 8; RUN-STATE); the full path `initialize.clientCapabilities._meta.kiro.{configurationState:true, settings:{…}}` (trap 5) with `configurationState:true` as the positive control that must produce `_kiro/configuration/state` (trap 28); OLD arm = 0.66.43 carve same day (trap 1). Frames below are what the 3.1.0 TUI sends, so the live lane can replay them verbatim.

- **TUI-L1 — session recap (T2/T3).** Arm A `initialize` with `clientCapabilities._meta.kiro.settings:{sessionRecap:{enabled:true}, …TUI defaults…}`; arm B `{enabled:false}`; arm C key absent. One short prompt, then wait ≥10 s idle. Expect in A only, after `turn_end`: `session/update {sessionUpdate:"session_info_update", _meta:{kiro:{kind:"recap", text:"<string>"}}}`. Record `$HOME/.kiro/logs/*/kiro.log` for `configuration.intake.unknown-key` naming `sessionRecap` (trap 23: the TUI uses the non-underscore key while the zod schema names `_sessionRecap`). Also note whether a `kind:"suggested_prompts"` arrives (KAS `_suggestedPrompts`, not sent by the TUI).
- **TUI-L2 — session credits (T4).** Any metered turn. Expect `session_info_update {kind:"session_usage", credits:[{usage:<n>, unit:"credit", unitPlural:"credits"}], …}` beside `turn_completion`; record ordering relative to `turn_end`. Then `session/load` the same session on a fresh connection and expect `kind:"session_usage"` with `replay:true` (3.1.0 "footer total no longer resets"). OLD arm: 0.66.43 must emit nothing (`kind:"session_usage"` 0 hits).
- **TUI-L3 — workflow credits (T5).** On a two-step recipe, capture `_kiro/workflow/node_complete`, `run_complete` and `_kiro/workflow/inspect`; expect `credits:[{usage,unit,unitPlural}]` on node descriptors and the run. Cross-ref the kas-workflow lane before spending the run.
- **TUI-L4 — `/model fallback` on KAS is dark (T6).** Read `initialize` result `agentCapabilities._meta.kiro.extensionMethods` (expect no `_kiro/modelFallback/*`, and `modelFallbackReporting:true`), then send `{"method":"_kiro/modelFallback/get","params":{"sessionId":"<sid>"}}` and expect `-32601`. Zero-turn leg.
- **TUI-L5 — permissions list + policy change (T9/T10).** `{"method":"_kiro/permissions/list","params":{"sessionId":"<sid>"}}` → expect `{rules:[{capability,effect,scope,source,match?,exclude?}]}`. Then write the TUI's Approve-for-me rule (`capability: all`, `effect: evaluate`) into the workspace `permissions.yaml` the TUI uses (`Hct("workspace")` → under the workspace `.kiro/`; confirm the path from KAS's `_kiro/policy/changed` or `source`) and expect `_kiro/policy/changed {sessionId}` then a re-list showing `source` = that file. Optional arm (one billed turn, ask first): spawn KAS with `KIRO_TRUST_CLASSIFIER_MODE=enforce`, run a shell command and expect either `tool_call_update._meta.kiro.trustClassifier{decision:"prompt",reason}` before `session/request_permission`, or `trustClassifierApproved:true` with no permission request.
- **TUI-L6 — MCP elicitation (T8), optional.** Only with an MCP server that elicits: expect `_kiro/mcp/elicitation {sessionId, toolCallId, elicitation:{mode:"form"|"url", message, requestedSchema?|url?}}` with **no** `mcpElicitation` flag advertised (KAS does not check it). cyril-shaped arm: cyril's decline reply (`crates/cyril-core/src/protocol/kas/mcp.rs`) and the resulting tool result text.
- **TUI-L7 — auth envelope (observe only; never log out, trap 6).** With the 3.1.0 launcher on PATH and the user logged in, record one `kiro-cli chat _ get-kas-token` stdout line (redact the token): expect `{"kind":"getKasToken","data":{"accessToken","expiresAt","profileArn","authMethod"?,"provider"?}}` — the TUI handler is unchanged, so any extra key is the host lane's finding (cyril-0rn8 context).
- **TUI-L8 — Workflow use (T20).** After `session/new` with workflows enabled, send `{"method":"session/set_config_option","params":{"sessionId":"<sid>","configId":"workflowLaunch","value":"delegate-all"}}`; expect `configOptions[id=workflowLaunch]` in the reply with `currentValue:"delegate-all"` and options `when-asked|large-tasks|delegate-all`. Also check whether `config_option_update` already offers `workflowLaunch` after `session/new` (the TUI only sets it when offered). OLD arm 0.66.43: expect the id absent.
- **TUI-L9 — memory cloud sync (T18), read-only arms only.** `{"method":"_kiro/configuration/getState","params":{"observer":"connection"}}` → `{settings,layers}`; `{"method":"_kiro/memory/getSyncStatus","params":{}}` → record the shape. **Do not** send `_kiro/configuration/set {krn:"krn:::setting/memorySync", value:"on", …}` (creates cloud state, trap 32); `value:"off", layer:"user-local"` is acceptable if the status says it is already off.
- **TUI-L10 — code index build (T19).** `{"method":"_kiro/codeIntelligence/startBuild","params":{"sessionId":"<sid>"}}` → expect `{buildId, status, workspaceRoot?}` when `extensionMethods` lists it (`_h("c2s")`), else `-32601`; then `indexStatus`/`cancelBuild`. Zero-turn except the index itself.
- **TUI-L11 — subcommand tier (T12).** On a `git stash list` shell approval reply `{outcome:{outcome:"selected",optionId:<allow_always id>}, _meta:{kiro:{consent:{capability:"shell", scope:"session", resource:"git stash *", workspaceRoot:"<ws>"}}}}`; then `git stash show` must not re-ask and `git status` must. Control arm: `resource:"git *"`.
- **TUI-L13 — OAuth completion (T31).** Advertise `clientCapabilities._meta.kiro.openExternalUrl:true` (cyril does) with a remote MCP server that needs OAuth. Expect `_kiro/openExternalUrl {sessionId?, url, serverName, oauthCompletion:"notification"|"connection-status", oauthFlowId?}` and, after the browser flow, `_kiro/mcp/oauthCompleted {serverName, success, oauthFlowId}`. OLD arm 0.66.43: neither key (static prediction). cyril-shaped arm: cyril's `openExternalUrl` responder (reads only `url`) — record whether KAS still sends `_kiro/mcp/status` so the link clears without the notification.
- **TUI-L12 — diagrams (T1/T7).** Arm A `clientCapabilities._meta.kiro.diagrams:true`, arm B omit; compare `extensionMethods` for `_kiro/diagram/render` and `diagramRenderFormats` in the `initialize` result (cyril-68ny). Zero-turn.

---

## Changelog reconciliation (all thirteen feeds, 188 lines)

`L` = located in the TUI bundle (row); `TUI-local` = TUI change with no wire part (literal count where one exists); `not in my lane` = kas-core / kas-workflow / host.

### 2.33.0 (28)
| # | line | location |
|---|---|---|
| 1 | [V3] trust a folder before loading hooks/agents/MCP/workflows | **L T16** — TUI-local gate, no frame to KAS (`workspaceTrust` 0 -> 70) |
| 2 | [V3] plan approvals compact step tree | **L T15** (`workflowPlan` 0 -> 3) |
| 3 | faster `grep` parallel | not in my lane (KAS) |
| 4 | Mermaid flowcharts with subgraphs | TUI-local (renderer) |
| 5 | [V3] long sessions hold less memory | no distinctive literal; not in my lane |
| 6 | [V3] PR watches act on conflicts/checks/threads | not in my lane (kas-workflow) |
| 7 | [V3] terminal title with `chat.terminalTitle` | TUI-local (`chat.terminalTitle` 1 == 1) |
| 8 | [V3] self-started replies no longer cut off | not in my lane (KAS) |
| 9 | [V3] `/agent`, `/model`, `/effort` inline completion | TUI-local |
| 10 | [V3] sandbox network rules in `permissions.yaml` apply live | not in my lane (KAS); the `permissions.yaml` literal enters the TUI here for T10's reader |
| 11 | [V3] `/sessions` no longer freezes while indexing | TUI-local |
| 12 | [V3] fallback models from `/model` pane and `/model fallback` | **L T6** — `_kiro/modelFallback/{get,set}` added; **dark on KAS** |
| 13 | Mermaid node descriptions wrap | TUI-local |
| 14 | home dir no longer lists global agents twice | no wire part; not located to a literal |
| 15 | [V3] `/sandbox` in home dir | TUI-local |
| 16 | [V3] `--agent <a> @agent-sop:<name>` runs the SOP | TUI/host argument preprocessing; no wire literal located |
| 17 | [V3] cloud config keeps syncing | not in my lane (KAS) |
| 18 | [V3] `/code` in cloud sessions shows workspace root and `lsp.json` | TUI-local (2.32.0 T16 fields) |
| 19 | [V3] no cloud-config sync card when empty | TUI-local |
| 20 | [V3] `code` tool diagnostics from slow LSPs | not in my lane (KAS) |
| 21 | [V3] throttling vs rate-limit copy | not in my lane (KAS `display_error`) |
| 22 | [V3] Windows recipe drive-letter case | not in my lane (KAS) |
| 23 | [V3] review loops stop on approve | not in my lane (kas-workflow) |
| 24 | [V3] workflows run when org disables Auto | not in my lane (kas-workflow) |
| 25 | Tab after `/model ` shows `set-current-as-default` | TUI-local (`set-current-as-default` 6 == 6) |
| 26 | [V3] `/code summary` generates documentation | **L T24** (`summary:` 0 -> 1) |
| 27 | security: sandbox host names case-insensitive | not in my lane (KAS) |
| 28 | security: Windows hooks no same-named program | not in my lane (KAS) |

### 2.33.1 (16)
| # | line | location |
|---|---|---|
| 1 | [V3] workflows start only when explicitly asked | not in my lane (KAS); the TUI control arrives at 2.34.0 (T20) |
| 2 | `/tools` screen-reader lines | TUI-local (`screen reader` 3 == 3) |
| 3 | `kiro-cli mcp add --agent` | not in my lane (host) |
| 4 | [V3] resuming a V2 session says approvals not carried over | not located in the TUI (2.33.1 adds no such copy); KAS/host |
| 5 | [V3] compaction retries without images | not in my lane (KAS) |
| 6 | [V3] MCP sign-in frees the OAuth port | not in my lane (KAS) |
| 7 | [V3] unavailable optional power MCP servers | not in my lane (KAS) |
| 8 | [V3] long image sessions omit older images | not in my lane (KAS) |
| 9 | [V3] plan approvals open with the first option highlighted | TUI-local |
| 10 | HTML doctypes theme colours | TUI-local |
| 11 | [V3] subagent custom agents search own KBs | not in my lane (KAS) |
| 12 | [V3] registry servers launch catalog version | not in my lane (KAS) |
| 13 | [V3] messages after spec task run keep updating tasks | not in my lane (KAS) |
| 14 | [V3] `@builtin` includes web search | not in my lane (KAS) |
| 15 | [V3] dispatched subagents load own skills | not in my lane (KAS) |
| 16 | [V3] user agents keep file rules from home | not in my lane (KAS) |

Also located in 2.33.1 with no changelog line: `_kiro/diagram/render` + `diagrams:!0` (T1/T7), `KIRO_STANDALONE_NOTICE` banner (T26), `c2s` gate 2 -> 10.

### 2.34.0 (28)
| # | line | location |
|---|---|---|
| 1 | [V3] MCP elicitation (form or link) | **L T8** |
| 2 | [V3] type suggested changes into a plan approval | **L T14** — `reject_once` + `_meta.kiro.rejectionReason` |
| 3 | [V3] delete memories from `/memories` | **L T17** |
| 4 | `/model edit` | **L T23** |
| 5 | [V3] Workflow use under Features | **L T20** — `set_config_option configId:"workflowLaunch"` |
| 6 | [V3] `--trust-workspace` | **L T16** (one-launch skip, no persistence) |
| 7 | [V3] approvals list Allow once / Deny once first | **L T13** |
| 8 | [V3] dashboard asks `Do you want to delete …?` | TUI-local (0 -> 1) |
| 9 | [V3] unparseable shell commands auto-denied | not in my lane (KAS) |
| 10 | [V3] subagents lose inline MCP servers | not in my lane (KAS) |
| 11 | `Working...` when the agent goes quiet | TUI-local (0 -> 1) |
| 12 | [V3] headless no longer answers questions / approves | TUI `noInteractive` guards; no wire change |
| 13 | [V3] headless reuses MCP sign-ins | not in my lane (KAS) |
| 14 | [V3] `/sessions` opens without freezing | TUI-local |
| 15 | enterprise overage status and consumed credits in `/usage` | **L** — TUI renders new fields of the existing `_kiro/account/getUsage` reply (`overage` 91 -> 103); field names are the kas-core lane's |
| 16 | [V3] workflows resume after restart without replanning | not in my lane (kas-workflow) |
| 17 | refusal partway continues on the fallback model | not in my lane (KAS / v2 host) |
| 18 | `chat.allowIcons` off drops box drawing | TUI-local (1 == 1) |
| 19 | Mermaid renders in more cases | TUI-local |
| 20 | [V3] deny rules save at the picked scope; blocked call not "cancelled" | **L** — consent echo now applies to `reject_always` (`Kk`: scope default `session` when a resource is chosen; `kiro:{consent` 1 -> 2) + KAS |
| 21 | [V3] `/rewind` forks at the right point | not in my lane (KAS) |
| 22 | [V3] approvals in the workflow monitor open on the current call, `q` closes | TUI-local |
| 23 | [V3] `kiro-cli acp` no longer shuts down mid-run | not in my lane (host/KAS) |
| 24 | pasted images / dictated text in filters | TUI-local |
| 25 | [V3] `/agent` loads the new agent's skills | not in my lane (KAS) |
| 26 | [V3] `ssh` in the macOS sandbox | not in my lane (KAS) |
| 27 | [V3] deleting files outside workspace on Windows | not in my lane (KAS) |
| 28 | security: remote MCP same-origin redirects | not in my lane (KAS) |

Also located in 2.34.0 with no changelog line: `_kiro/permissions/list` + `_kiro/policy/changed` (T9), the Approve-for-me rule writer and `trustClassifier` meta (T10/T11, one release before the 2.36.0 announcement), `consentRound` on `interaction_resolved` (T22).

### 2.34.1 (1) · 2.34.2 (1)
| # | line | location |
|---|---|---|
| 2.34.1-1 | queued message with blank lines | TUI-local |
| 2.34.2-1 | offers to switch you to V3 at chat start | **not in the TUI** — 2.34.2's bundle is byte-identical to 2.34.1; host lane |

Also located in 2.34.1 with no line: `_meta.kiro.replay` tagging (T21).

### 2.35.0 (15) · 2.35.1 (1) · 2.35.2 (1)
| # | line | location |
|---|---|---|
| 1 | root/Administrator warning, `app.warnOnElevatedPrivileges` | **L T26** (`warnOnElevatedPrivileges` 0 -> 2, `KIRO_ELEVATED_PRIVILEGES`) |
| 2 | `/fullscreen` pins the prompt | TUI-local |
| 3 | Kiro ghost greets on startup | TUI-local |
| 4 | refused response on Auto retried | not in my lane (KAS) |
| 5 | [V3] MCP servers and web tools follow org settings | not in my lane (KAS) |
| 6 | model fallback no longer hangs | not in my lane (KAS/v2) |
| 7 | dictation "scratch that" | TUI-local |
| 8 | [V3] resuming a V2 session no `default` warning | not in my lane (KAS) |
| 9 | [V3] Linux resume after reboot not "open elsewhere" | TUI/host session lock; no wire |
| 10 | `/chat` and `--resume` no timeout on large history | host IPC; no wire |
| 11 | [V3] `/mcp` explains registry-hidden servers | TUI render of existing `_kiro/mcp/status` data |
| 12 | [V3] `kiro-cli mcp list` | not in my lane (host) |
| 13 | [V3] HTTP MCP 403 probe | not in my lane (KAS) |
| 14 | [V3] refusal with no output ends the turn | not in my lane (KAS) |
| 15 | [V3] `read_file` offset past EOF | not in my lane (KAS) |
| 2.35.1-1 | performance improvements | — |
| 2.35.2-1 | retiring autocomplete desktop app (rollout) | not in my lane (host) |

### 2.36.0 (15)
| # | line | location |
|---|---|---|
| 1 | [V3] Approve for me in `/permissions` | **L T10** (dark: `KIRO_TRUST_CLASSIFIER_MODE=enforce`) |
| 2 | [V3] shell rules offer a subcommand tier | **L T12** |
| 3 | `kiro-cli agent upgrade` warns | not in my lane (host) |
| 4 | [V3] once-per-session notice for agents whose V3 rules block V2 | **L T27** (host IPC `permissionConflicts`) |
| 5 | `acp.agentEngine` setting | **not in the TUI** (0 hits); host lane |
| 6 | click the ghost for animations | TUI-local (`hop nod shake wink`) |
| 7 | [V3] clearer `/spec` requirements questions | TUI render + KAS; no new wire literal |
| 8 | [V3] shell tool cards title themselves | TUI-local |
| 9 | [V3] rule rows keep the file name | TUI-local |
| 10 | Mermaid titles/notes | TUI-local |
| 11 | crash on non-ASCII MCP tool names | not located; host/KAS |
| 12 | indented wrapped tool command rows | TUI-local |
| 13 | security: Node 22.23.4 | not in my lane (host runtime) |
| 14 | security: `GIT_CONFIG_GLOBAL` env prefixes ask approval | not in my lane (KAS) |
| 15 | security: Bun 1.4.4 | not in my lane (host) |

### 2.37.0 (18)
| # | line | location |
|---|---|---|
| 1 | ACP clients can sign in from a terminal when `kiro-cli acp` starts logged out | **not in the TUI** (handler unchanged; `kiro-cli login` 16 == 16); host lane |
| 2 | `KIRO_RUNTIME_DIR` | **not in the TUI** (0 hits); host lane |
| 3 | fullscreen keeps the prompt on the bottom rows | TUI-local |
| 4 | ghost keeps full colour | TUI-local |
| 5 | V2 agents auto-upgraded at launch; `chat.enableAutoAgentUpgrade` | host (setting 0 hits in the TUI); the TUI part is the `KIRO_AGENT_UPGRADE_NOTICE` banner (T26) |
| 6 | [V3] approval prompts show a multi-line command's first line until `ctrl+o` | TUI-local |
| 7 | [V3] approval rule page highlights scope choices, `s` hint | TUI-local |
| 8 | Mermaid node shapes | TUI-local |
| 9 | `kiro-cli update` suggests brew | not in my lane (host) |
| 10 | `--no-interactive` says `stopping (exit 4)` | host/TUI non-interactive; no wire |
| 11 | [V3] `--no-interactive` stopped by a V2 agent names the upgrade command | not in my lane (host) |
| 12 | fullscreen clicks land on the right row | TUI-local |
| 13 | [V3] entire-tool row names the tool | TUI-local (`entire tool${…}`) |
| 14 | mouse wheel on old tmux | TUI-local |
| 15 | ghost stays on the home screen | TUI-local |
| 16 | Mermaid nested containers | TUI-local |
| 17 | Mermaid long labels | TUI-local |
| 18 | security: `/spec` strips escape sequences | TUI-local |

Also located in 2.37.0 with no line: `KIRO_LAUNCH_3_0` welcome variant (T26).

### 2.37.1 (4) · 3.0.0 (3)
| # | line | location |
|---|---|---|
| 2.37.1-1 | refreshed welcome / What's new | TUI-local |
| 2.37.1-2 | fullscreen scroll pill | TUI-local |
| 2.37.1-3 | [V3] answer cut off at the output limit says so | TUI-local copy on the existing stopReason (`output limit` 0 -> 1, `cut off` 0 -> 1) |
| 2.37.1-4 | [V3] idle sessions use less CPU tracking the process tree | TUI-local (supervision) |
| 3.0.0-1 | V3 is the default | host; TUI side = `--v2/--classic/--legacy-ui` accepted-and-ignored since 2.37.1 (T28) |
| 3.0.0-2 | Classic retired | host; the TUI keeps its v2 code paths (`==="v2"` 12 == 12) |
| 3.0.0-3 | inline autocomplete + desktop app retired | not in my lane (host) |

### 3.1.0 (57)
| # | line | location |
|---|---|---|
| 1 | `PreCompact`/`PostCompact` hooks | not in my lane (KAS: `PreCompact` 0 -> 23; TUI 0) |
| 2 | `KIRO_DEFAULT_SETTINGS` | **L T25** |
| 3 | `/usage` + `Session credits` segment | **L T4** (`session_usage`) |
| 4 | workflow step rows show credits | **L T5** |
| 5 | session recap (`chat.enableSessionRecap`) | **L T2/T3** — engine-fed |
| 6 | sub-agents read parent memories | not in my lane (KAS) |
| 7 | prompt refused with a sign-in message when no token | not in my lane (KAS); TUI `auth_error` consumer unchanged |
| 8 | agent `file://` resources capped | not in my lane (KAS) |
| 9 | notifications queue into one row, `+N waiting` | TUI-local (` waiting` 5 -> 6) |
| 10 | warnings/errors stay until a keypress | TUI-local |
| 11 | startup cloud config/learnings/steering fetches print no tool cards | TUI-local filter (`fetch_cloud_config` 0 -> 7) |
| 12 | `web_fetch` asks for markdown | not in my lane (KAS) |
| 13 | MCP `structuredContent` extra JSON block | **not in the TUI** (0 hits); KAS (T29) |
| 14 | global skills load from the home dir in `kiro-cli acp` | not in my lane (KAS/host) |
| 15 | `/agent swap kiro_default` | not located to a TUI literal; KAS agent switch |
| 16 | missing/repeated transcript line | TUI-local |
| 17 | message sent while a sub-agent runs leaves the queued tray | TUI-local |
| 18 | prompt retries an unanswered connection for 90 s | not in my lane (KAS) |
| 19 | sub-agent stopped at its turn limit reports | not in my lane (KAS) |
| 20 | token-endpoint outage keeps MCP sign-in | not in my lane (KAS) |
| 21 | turn cut off by output cap / step limit says it stopped early | not in the TUI (`stopped early` 0); KAS copy |
| 22 | `/context show` lists unloadable `skill://` | not in my lane (KAS) |
| 23 | sessions past 512 MiB open again | TUI-local transcript store / host |
| 24 | shell runs in `bash` when `SHELL` unset | not in my lane (KAS) |
| 25 | starting a session no longer deletes another's logs | not in my lane (KAS) |
| 26 | `str_replace` `$$` patterns | not in my lane (KAS) |
| 27 | sub-agents load their own `file://` resources | not in my lane (KAS) |
| 28 | prompt sent as a workflow finishes is not dropped | not in my lane (kas-workflow) |
| 29 | spec task runs record property tests without approval | not in my lane (KAS) |
| 30 | workflow runs land on the checked-out branch | not in my lane (kas-workflow) |
| 31 | `agent upgrade` / `/upgrade-agent` warn about V2 tools like `todo_list` | host IPC; TUI renders the warning (`no-v3-tool` literal added 3.1.0) |
| 32 | tool cards drop the stale `ctrl+o to toggle` hint | TUI-local (`ctrl+o to toggle` 2 == 2; behavioural) |
| 33 | empty model reply ends the turn | not in my lane (KAS) |
| 34 | MCP/web tools stay on for org users | not in my lane (KAS) |
| 35 | MCP sign-in links disappear once the server connects, server named in the toast | **L T31** — the TUI's 2.32.0 `_kiro/mcp/oauthCompleted` consumer is now fed: KAS 0.66.91 emits it (`oauthCompleted` 0 -> 2) and passes `serverName`/`oauthCompletion` on `_kiro/openExternalUrl` |
| 36 | "Run required tasks" finishes every task | not in my lane (KAS) |
| 37 | shell commands reading stdin get EOF | not in my lane (KAS) |
| 38 | Mermaid sequence diagrams | TUI-local |
| 39 | `kiro-cli agent validate` exit code | not in my lane (host) |
| 40 | damaged knowledge listing | not in my lane (KAS) |
| 41 | isolated AWS regions | not in my lane (KAS) |
| 42 | `PreToolUse` exit 2 blocks without a reason | not in my lane (KAS) |
| 43 | Windows shell child processes | not in my lane (KAS) |
| 44 | sub-agent KB refresh | not in my lane (KAS) |
| 45 | resize no longer blanks the chat | TUI-local |
| 46 | scrolled-back chat keeps the line | TUI-local |
| 47 | footer session credit total no longer resets on reopen | **L T4** (`session_usage` with `replay:true`) |
| 48 | `--no-interactive` and `kiro-cli acp` print no `[INFO]` on stderr | not in my lane (KAS/host) |
| 49 | `--no-interactive` newline after a tool call | host plain-text printer |
| 50 | cloud sessions in an unsupported region show as unavailable | **L** — TUI maps the KAS error (`cloud_session_unsupported`, `CodeIntelligenceCloudSessionUnsupportedError` added 3.1.0) |
| 51 | reopening after a crash during a save | TUI-local / host transcript |
| 52 | `web_fetch` pages over 10 MB reported as too large | not in my lane (KAS; the TUI's `too_large` is T25's) |
| 53 | zero-result `web_search` message | not in my lane (KAS) |
| 54 | tool output stays on its row in fullscreen | TUI-local |
| 55 | security: tool-use telemetry with OTel-only export | not in my lane (host/KAS) |
| 56 | security: MCP registrations owner-only | not in my lane (KAS) |
| 57 | security: [V2] `GIT_DIR`/`GIT_WORK_TREE` prefixes | not in my lane (v2 host) |

Also located in 3.1.0 with no changelog line: `_kiro/codeIntelligence/{startBuild,cancelBuild,indexStatus}` (T19), memory cloud sync (`_kiro/configuration/{getState,set}`, `_kiro/memory/getSyncStatus`, `cloudState`, gate `memory_cloud_sync`; T18), the five removed rollout gates and `KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED` (sandbox/dashboard/workflows GA), the `feature:` gate removal on 11 slash commands, `KIRO_STANDALONE_NOTICE` removed.

---

## Proposed issues/notes (checked against `.rivets/issues.jsonl`; entry point named per trap 29)

Priority rule: P2 = a dropped field or missing route that KAS pushes by default and cyril surfaces (or would surface) today; P3 = opt-in flow reachable only through a setting/command the user turns on; P4 = cosmetic / dark.

- **NEW P2 — KAS `session_usage` kind (T4).** Entry point: cyril's toolbar/session cost (`crates/cyril-core/src/types/session.rs:627-662`, fed by `TurnMeteringUpdated`), which KAS 0.66.91 now supersedes with `session_info_update {kind:"session_usage", credits:[{usage,unit,unitPlural}], replay?}` — authoritative, and replayed on `session/load` so the total survives reopen. cyril drops it at `convert/kas.rs:822` and its local sum restarts at 0 after a load (the exact bug Kiro's 3.1.0 line 47 fixes). Model the kind (`Notification::SessionUsage`), prefer it over the local sum when present (`sessionCreditsReported` semantics), keep the local sum for 0.66.43. Related: cyril-8alk (shared credit-unit predicate; NOTE there), cyril-2lh8 (closed).
- **NEW P3 — session recap (T2/T3).** Entry point: a cyril setting (no slash command yet) → `initialize.clientCapabilities._meta.kiro.settings.sessionRecap:{enabled:true}` and a consumer for `kind:"recap" {text}` (primary session only, shown above the input after idle). Default off in Kiro too ("one model call per turn" = billed). NS `settings.rs:77-95`; D `kas.rs:822`. NOTE on cyril-53qx: the TUI's settings table now has 13 rows (`+sessionRecap`), still no `todoList`/`subagentOrchestration`; `workflows`/`goal` are always sent (the rollout `available` check is gone).
- **NEW P3 — Workflow use (`workflowLaunch`, T20).** Entry point: cyril's `/workflow` surface (ADR-0013 keeps workflows on) → `session/set_config_option {configId:"workflowLaunch", value:"when-asked"|"large-tasks"|"delegate-all"}`, offered as a configOption after `session/new`; cyril sets only `model|thinking|mode|effortLevel` (`commands/mod.rs:1596,2023,2327,2690`). Decides whether the model may start workflows on its own — a billing-relevant control cyril users cannot reach. Pending TUI-L8.
- **NEW P3 — interactive MCP elicitation (T8).** Entry point: cyril's existing `_kiro/mcp/elicitation` responder (`kas/callbacks.rs:597`, `kas/mcp.rs`) which declines visibly. Kiro answers forms (`{action:"accept",content:{…}}`, `requestedSchema`-driven) and links (open/copy → `{action:"accept"}`). Opt-in flow (only MCP tools that elicit) → P3. NOTE cyril-dn91 (closed): KAS calls the method without any `mcpElicitation` flag; the TUI's flag is advisory.
- **NEW P4 — subcommand tier in Always-allow (T12).** Entry point: `crates/cyril-core/src/always_allow.rs:624-643` (exact + base pattern). Add the `"<cmd> <sub> *"` tier with the TUI's `z_()` rules (argv1 `^[A-Za-z][A-Za-z0-9_-]*$`, argv0 ∉ sudo/doas/env). The echo path already carries any resource (`convert/kas.rs:150-191`). Pending TUI-L11.
- **NEW P4 (dark) — trust classifier / Approve for me (T10/T11).** Entry point: none until `KIRO_TRUST_CLASSIFIER_MODE=enforce` is default on KAS. Record the frames: rule `{capability:"all",effect:"evaluate"}` in `permissions.yaml|json`; `tool_call_update._meta.kiro.trustClassifier{decision:"prompt",reason}`; `trustClassifierApproved:true`. Model-or-log per the full-wire-surface rule when it lights up. Related cyril-8q8p (v2 provenance).
- **NOTE cyril-aoby / cyril-lnxg (T6):** the 3.1.0 TUI has a KAS `/model fallback` route (`_kiro/modelFallback/get {sessionId}` / `set {sessionId,targetModelId,fallbackModelId?,disabled?}`), but KAS 0.66.48–0.66.91 never advertises or implements it (0 literal hits; only `modelFallbackReporting:true`). Do not build it until a KAS carve carries the literal. Pending TUI-L4.
- **NOTE cyril-r5pl (T9):** `_kiro/policy/changed` is now consumed by the TUI as a trigger to re-run `_kiro/permissions/list {sessionId}` → `{rules:[{capability,effect,scope,source,match?,exclude?}]}`; cyril drops the notification at `convert/kiro.rs:1305` and never lists rules. A `/permissions`-style listing is the KAS-native route for "what did my Always-allow persist" (cyril-auxt follow-up).
- **NOTE cyril-0na7 (T17/T18):** memory controls grew `_kiro/memory/delete {sessionId?, id}` (2.34.0) and, under rollout `memory_cloud_sync`, `_kiro/configuration/getState {observer:"connection"}`, `_kiro/configuration/set {krn:"krn:::setting/memorySync", value:"on"|"off", layer:"user-synced"|"user-local"}` (locked → `data:{reason:"locked",layer}`), `_kiro/memory/getSyncStatus {}`, and `_meta.kiro.cloudState` on records. `memory.mode`/`memory.reflection`/`memoryReflection` configId are unchanged. Also NOTE cyril-iq2c (`_kiro/configuration/getState` now has a first-party consumer).
- **NOTE cyril-nk4o / cyril-tqr8 (both closed) → reopen or NEW P3 (T31):** KAS 0.66.91 now completes the MCP OAuth loop the TUI wired in 2.32.0: `_kiro/openExternalUrl` params `{url, serverName, oauthCompletion, oauthFlowId?}` and the notification `_kiro/mcp/oauthCompleted {serverName, success, oauthFlowId}`. Entry point: cyril's existing `openExternalUrl` responder (`kas/callbacks.rs:193`, advertised at `engine.rs:143`) — read the two new params, model the notification (`convert/kiro.rs:1305` drops it) and clear the OAuth notice per server. Opt-in flow (remote MCP with OAuth) → P3. Pending TUI-L13.
- **NOTE cyril-68ny (T1/T7):** Kiro's TUI sends `diagrams:true` only when the host rollout lists `c2s`, and KAS then advertises `_kiro/diagram/render {sessionId,diagramId,format}` (32 KiB source cap, `diagramRenderFormats`). Still dark by default.
- **NOTE cyril-4u4a (T5/T15):** workflow node descriptors and `run_complete` carry `credits[]`; the TUI reads `request_permission._meta.kiro.workflowPlan` (since 2.33.0) for the step tree. Both dropped by `convert/kas/workflow.rs` / `convert/kas.rs:80-110`.
- **NOTE cyril-1aq0 (T19/T24):** `/code` gained `summary` (2.33.0) and an async index build (`_kiro/codeIntelligence/startBuild {sessionId}` → `{buildId,status,workspaceRoot?}`, `cancelBuild`, `indexStatus`; advertised under KAS `c2s`).
- **NOTE cyril-mq15 (T16):** Kiro's workspace-trust prompt is TUI-only and KAS's standalone entry hardcodes `workspaceTrusted:!0`; there is no frame to wire. cyril's "always trusted" behaviour equals Kiro's `--trust-workspace`; a cyril gate would have to sit before `session/new`, like `wgt()`.
- **NOTE cyril-99ds (T21), cyril-58uv (T22):** the TUI tags `content`/`tool_call` with `_meta.kiro.replay===true` as source `"replay"` (2.34.1) and reads `consentRound`+`replay` on `interaction_resolved` (2.34.0).
- **NOTE cyril-q8gq (T25):** Kiro's settings now layer a root-owned `KIRO_DEFAULT_SETTINGS` file under `cli.json`; cyril's marshal reads only the user file.
- **NOTE cyril-j66p, cyril-j9r4, cyril-lxuo:** output-style picker, OSC 7501 and the model-picker credit column are unchanged in 3.1.0 (counts 1 == 1).
- **NOTE cyril-zbu7 (closed) / cyril-0rn8:** the `getAccessToken` responder is byte-identical through 3.1.0; the 2.37.0 sign-in flow is host-side. No TUI-side change for the token lead.
- No issue: T13 (TUI sort), T23, T26–T30 (TUI-local / host / kas lanes), T14 (cyril already sends `rejectionReason`).

---

## Method notes

- **Scripts** (`experiments/conductor-spike/`): `static-tui-{sets,vocab,gates,consumers,region,stmtdiff}-3.1.0.py` are copies of the 2.32.0 tools (docstring updated); **new `static-tui-ctx-3.1.0.py`** replaces `grep -oE '.{0,N}needle.{0,N}'`, which the system `grep` (ugrep) rejects ("exceeds complexity limits") on 14 MB single-line bundles. zsh note: `R="python3 -I x.py"; $R` is one word — call scripts directly.
- **Outputs** (this directory): `consumers-2.32.0-3.1.0.txt`, `gates-2.32.0-3.1.0.txt`; under `tmp/` (git-ignored): `sets/`, `vocab/` (per-pair `*.buckets.txt`, `*.strings.added`, `*.props.added`), `stmt-{2.32.0,3.1.0}.txt`, `d.diff`, `chunk*.md`, `FINDINGS.progress.bak`.
- **Blind spots found and closed this cycle.** (1) The consumers census only sees `onExtNotification("<literal>")`; `_kiro/policy/changed` registers through a variable (`onExtNotification(rEr,…)`), so ext-notification censuses must resolve `var X="_kiro/…"` aliases. (2) The gates extractor's fixed `settings.rows` pattern (`["a.b","c"]`) misses rows pushed conditionally (`o.push([Ve.CHAT_ENABLE_SESSION_RECAP,"sessionRecap"])`) — hand-read the builder each cycle. (3) The gate census resolves the gate function from the `local_sandbox` env body, which 3.1.0 removed; resolve it from `("c2s")&&{diagrams` instead (done inline). (4) `type:"other",key:"…"` entries are the spread-built host-capability flags (trap 30); counting that pattern (6 -> 7) is how `mcpElicitation` was found.
- **Minified-name traps.** `KIRO_OWNED_KAS_{PARENT,CONTROL}` looked "added" in the statement diff because the spawn region shifted; both exist in 2.32.0 (1 == 1). `rollout_check` in the sets script also matches zod `er("snake")` discriminators (`legacy_reasoning_replay`, `sub_agent_start`, …) and `ds("workspace_trust_resolved")` perf marks — those are not gates; the per-function census is the authority.
- **Cross-lane greps** (read-only, six KAS carves) established: `sessionRecap` is a declared client-settings wire key in 0.66.43 and 0.66.91; `kind:"session_usage"` 0 -> 2 at 0.66.91; `kind:"recap"` 2 == 2 everywhere; `_kiro/modelFallback/*` 0 everywhere; `mcpElicitation` 0 everywhere (KAS calls elicitation unconditionally); `workspaceTrusted:!0` hardcoded in the standalone entry; `KIRO_TRUST_CLASSIFIER_MODE` env-only on KAS; `structuredContent` 19 == 19; `oauthCompleted` 0 -> 2 at 0.66.91 (the 2.32.0 lane's "dark" verdict is now stale — T31).
- **Trap compliance.** Every add/remove carries OLD -> NEW counts plus the first bundle (trap 2); families were set-diffed across all ten bundles (trap 3); every row is STATIC (trap 4); every live lead writes the full `initialize.clientCapabilities._meta.kiro` path with `configurationState:true` as control (traps 5, 28) and names the OLD arm (trap 1); no Kiro binary was executed (trap 6); no frame citations (static lane), `check-citations.py` run anyway.

## Progress log
- [P1] All ten distinct bundles (2.32.0 2.33.0 2.33.1 2.34.0 2.34.1 2.35.0 2.36.0 2.37.0 2.37.1 3.1.0) verified against their `.sha256` sidecars; 2.34.2==2.34.1, 2.35.1==2.35.2==2.35.0, 3.0.0==2.37.1 (RUN-STATE classes). Scripts copied as `static-tui-{sets,vocab,gates,consumers,region,stmtdiff}-3.1.0.py`; new `static-tui-ctx-3.1.0.py`.
- [P2] trap-3 sets (`tmp/sets/sets-diff.txt`): `_kiro/*` 64 -> 77 (+13), `kiro.dev/*` 38 == 38, slash literals 74 -> 75 (+`/permissions` @2.34.0), `chat _` subcmds 11 == 11, KIRO_* env 95 -> 103 (+9/−1), `kind===` +dwelling/recap/session_usage @3.1.0, `type===` +mcp_elicitation_request/workflow_launch_update @2.34.0.
- [P3] consumers census (`consumers-2.32.0-3.1.0.txt`): `onExtNotification("<literal>")` 11 == 11 (variable-registered `_kiro/policy/changed` missed); `.kiro?.<key>` reads +rejectionReason @2.33.0, +trustClassifier @2.34.0, +replay @2.34.1, +cloudState @3.1.0; gate fn Rn -> Vn; 3.1.0 gates {background_execution 4, c2s 14, cloud_config 12, managed_settings 4, memory 2, memory_cloud_sync 1, memory_controls 2}.
- [P4] gates extractor (`gates-2.32.0-3.1.0.txt`): fixed-pattern rows unchanged; hand-read regions found `+...Vn("c2s")&&{diagrams:!0}` (@2.33.1) and settings `+sessionRecap` (@3.1.0).
- [P5] vocab differ (`tmp/vocab/*.buckets.txt`): +551/−11 (2.33.0), +28 (2.33.1), +217/−26 (2.34.0), +52/−26 (2.34.1), +25/−14 (2.35.0), +37/−1 (2.36.0), +20/−8 (2.37.0), +6/−6 (2.37.1), +259/−19 (3.1.0) literals; wire-relevant buckets folded into Deltas.
- [P6] Hand-read regions: clientMeta, settings builder, consent echo (`Kk`), elicitation (`H_r`, `PBt`, form/url responders), approve-for-me (`xC`, `pit`, `/permissions` handlers), trust (`wgt`, `resolveWorkspaceTrust`, `AAt`), subcommand tier (`z_`), option order (`w$r`), planReply, modelFallback descriptors, `session_usage`/`recap` consumers, spawn region, auth handler, memory sync, code-index build, workflowLaunch.
- [P7] KAS carve cross-reference (six versions, plus 2.34.1/2.36.0/2.37.0/2.37.1/3.0.0 for `oauthCompleted`) for every new literal; cyril verdicts at HEAD e3a3868d with file:line. Correction folded in: the 2.32.0 lane's dark `_kiro/mcp/oauthCompleted` is live on 0.66.91 (T31).
- [P8] FINDINGS complete; `check-citations.py` run; no background jobs left.
