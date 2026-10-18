# TUI-bundle static lane: kiro-cli 2.31.1 -> 2.32.0

Inputs: `~/.local/share/kiro-research/tui-bundles/kiro-tui-2.31.1.js` (sha256 105848c6…6ee2, 13,692,707 B) and `kiro-tui-2.32.0.js` (sha256 eba39f7b…2cd0, 13,723,687 B). The 2.32.0 copy is `cmp`-identical to the installed `~/.local/share/kiro-cli/tui.js`, and both match their `.sha256` sidecars. The KAS carves (`~/.local/share/kiro-research/kas-carves/{2.31.1,2.32.0}`) were grepped read-only, only to cross-reference emitters. Nothing from Kiro was executed.

Legend: **STATIC** = read off the bundle, so it is a lead; **LIVE** = needs a probe. Every row in this file is STATIC. In the cyril column, **H** = HANDLED, **D** = DROPPED, **NS** = NOT-SENT, and **N/A** = TUI-local with no wire part.

---

## Summary

1. **The wire handshake is frozen.** The rename-normalised regions are identical: `initialize` clientMeta (`telemetryEnabled, knowledge, hooks{enabled,v2}, requirementsAnalysis, specPhaseCheckpoints, streamingShellContent, infrastructureSafety?, settings`), the KAS `settings` builder rows, defaults and rollout checks, `session/new`, `session/load` and `session/prompt` `_meta` (`displayText`, `outputStyle`), the 7 parsed agentCapabilities, and the KAS spawn argv and env (`--auth=acp-callback`; the Windows `windows-kas-process`/`windows-kas-stop` subcommands are unchanged). Slash commands: 74 == 74 literals, with no description change.
2. **One new consumed method: `_kiro/mcp/oauthCompleted {serverName, success}`.** It is armed only when `_kiro/openExternalUrl` (a host callback) arrives with the new params `serverName` and `oauthCompletion:"notification"`. **KAS 0.66.43 has 0 hits for `oauthCompleted` and for `oauthCompletion`**, so the method is dark and forward-compat. The visible part of "OAuth links disappear when the server connects" runs on the existing `_kiro/mcp/status` path.
3. **Write approvals with a custom folder (V3) travel on the existing consent echo.** The permission response carries `_meta.kiro.consent{capability:"fs_write", scope:"session"|"workspace"|"user", resource:"<dir>/**", workspaceRoot}`. There is no new option id and no new key; the new part is a folder-glob `resource`. The offer is restricted to KAS, the main session, `capability==="fs_write"` and `askType!=="explicit"`. **cyril sends no `consent` echo at all**, only `rejectionReason` (`convert/kas.rs:67-91`), so it cannot grant a folder at any scope. This extends cyril-gn07.
4. **`/workflow run` plan preview** comes from data the TUI already received: `listRecipes[].plan` is now passed into the inputs form (`panel:{type:"workflow-recipe-inputs", recipe, initialValues, plan}`), and picking a recipe always previews. The plan tree now renders `includedFrom{name,path}` on `sequence` nodes. cyril drops `plan` (`convert/kas/workflow.rs:606-617`) and drops `includedFrom` (`workflow.rs:341-345`).
5. **Output styles (V3) are KAS-supplied.** The TUI already reads the `_kiro/config/template` option `id:"outputStyle"`, `category:"output_style"`, `_meta.kiro.promptScoped:true`, and sends the choice on every prompt as `session/prompt _meta.kiro.outputStyle`. KAS 0.66.43 adds the Proactive, Explanatory and Learning values (kas lane). **cyril has no outputStyle at all** (NS, P2).
6. **New user-visible TUI-only features:** OSC 7501 program status (`ESC]7501;state=<idle|working|blocked|done|error|clear>[:kind=permission|question]:app=kiro-cli BEL`, opt-out `KIRO_NO_PROGRAM_STATUS`); a What's-new memory section (gated on rollouts `memory` AND `memory_controls`); a model-picker sort by last used, cost (`_meta.kiro.rateMultiplier` buckets) or company, persisted in `chat.modelPicker.sortBy` / `chat.modelPicker.lastUsed`; the Always-Allow scope label `global` renamed "always" -> "user"; `s` to cycle scope; `PromptFailedError` turn-failure copy.
7. **Rollout gates:** the same 10 names. `memory` 1 -> 2 call sites (What's new). No new gate, no new `KIRO_*` env except `KIRO_NO_PROGRAM_STATUS`. Dark: `_kiro/mcp/oauthCompleted`. The kas lane found a new KAS `clientCapabilities._meta.kiro.diagrams` flag that the **TUI does not send** (0 relevant hits).
8. **Not in the TUI** (the kas or host lane owns these): creating a workflow from a prompt (the canned `/workflow new` prompt is byte-identical), plain-language changes to running workflows, review loops pausing after 2 rejections (rendered by the generic `pauseReason`/approval path), saved-session refresh, resumed model/effort, `--cloud`/`--repo`/`--resume-id`, `/chat` archive "altered" (host `chat _ import-session`), V2 subagent fallback.

---

## Deltas

Columns: S/L = STATIC/LIVE. Counts are OLD -> NEW occurrences, verified this session with `grep -oF`.

| # | item | version | gate/default | direction | S/L | cyril verdict + file:line | proposed issue |
|---|---|---|---|---|---|---|---|
| T1 | **New consumed ext notification `_kiro/mcp/oauthCompleted`** (literal 0 -> 1; `onExtNotification` set 10 -> 11). Handler: `onExtNotification("_kiro/mcp/oauthCompleted",(o)=>{let i=typeof o.serverName==="string"?o.serverName:void 0,s=i?this.mcpOAuthAttempts.get(i):void 0;if(!i\|\|typeof o.success!=="boolean"\|\|s?.completion!=="notification")return;…` -> `mcp_oauth_completed` (0 -> 4) -> a transient "`${server} connected`" alert | 2.32.0 | armed only by T2; **KAS 0.66.43: 0 hits** (whole carve) | inbound | STATIC | **D** if it arrives: engine.rs:350-369 falls through to `convert/kiro.rs:1284` `_ => Ok(None)` | NOTE on cyril-nk4o (dark; model it when KAS ships it) |
| T2 | `_kiro/openExternalUrl` host-callback params gain `serverName` and `oauthCompletion` (`"notification"` else `"connection-status"`; `oauthCompletion` 0 -> 3). `function Msr(e){return e==="notification"?"notification":"connection-status"}` … `if(a)n({message:i,url:r,serverName:a,oauthCompletion:Msr(e.oauthCompletion)})`. With a `serverName`, the link becomes a per-server `mcpOAuthNotices` entry (`mcp_oauth_notice` 0 -> 4); before 2.32.0 it was a generic `system_notice` | 2.32.0 | KAS 0.66.43 sends neither key | inbound request params | STATIC | **NS**: cyril advertises no `openExternalUrl` capability and has no handler (no hit in `crates/`) | — (fold into the T1 note) |
| T3 | MCP OAuth bookkeeping: `pendingOAuthServerNames` (1 -> 0) replaced by `mcpOAuthAttempts: Map<server,{completion, pendingConnectionStatus}>` (0 -> 6). `_kiro/mcp/status` `failedAuthorization && authorizationUrl` -> attempt. Leaving `connecting` -> `mcp_server_initialized` (connected) or `mcp_oauth_cleared`. A server's OAuth notice clears on connect, with a 4 s "`<server>` connected" toast | 2.32.0 | — | inbound (existing `_kiro/mcp/status`) | STATIC | **D**: `_kiro/mcp/status` unhandled (`convert/kiro.rs:1284`) | NOTE on cyril-nk4o |
| T4 | v2 `kiro.dev/mcp/server_initialized {refreshOnly:true}` -> `mcp_oauth_cleared` (`refreshOnly` 0 -> 1): `type:e.refreshOnly===!0?"mcp_oauth_cleared":"mcp_server_initialized"` | 2.32.0 | — | inbound v2 | STATIC | **D** (v2-only): `convert/kiro.rs:1146-1156` ignores `refreshOnly` | — (v2, demoted per traps 24) |
| T5 | **Write approval: allow a custom folder.** `b2t(e)`: `if(e.agentEngine!=="kas"\|\|!e.ownedBySession)return;let t=e.consentContext;if(t?.capability!==Xir)return;if(t.askType==="explicit")return;` (`Xir="fs_write"`). The prefill is the parent dir of `triggeringResource ?? resource`. Response: `function y2t(e,t){let n=e.endsWith("/")?`${e}**`:`${e}/**`;return{kasScope:t,kasResource:n}}` -> `cw()` -> `_meta.kiro.consent{capability, scope, resource:"<dir>/**", workspaceRoot}`, with optionId = the `allow_always` option. Validation copy (0 -> 1 each): "Type a folder path.", 'The folder cannot step up with "..".', "Spell out the folder instead of using ~.", "Type one folder, not a pattern; everything under it is included.", "Allows writes in ${e} and every folder under it, ${n}." | 2.32.0 | KAS only; main session only; `askType!=="explicit"` | outbound permission-response `_meta` | STATIC | **NS**: cyril's KAS response carries only `_meta.kiro.rejectionReason` (`crates/cyril-core/src/protocol/convert/kas.rs:67-91`). The request `consent` is read (`kas.rs:24-57`) but never echoed | **NOTE cyril-gn07** (P2: folder grants need the consent echo) |
| T6 | Always-Allow scope page: `trustScope` state (0 -> 8) shown as `Scope:  [session]  workspace  user` with `s` cycling `["session","workspace","global"]` (`Ov(e)`; "to cycle scope" 0 -> 2). Label `global:"always"` (2 -> 0) becomes `global:"user"` (0 -> 1). Wire scope stays `global` -> `"user"` (unchanged since 2.31.1: `Ae=b==="global"?"user":b`) | 2.32.0 | — | TUI-local | STATIC | N/A (cyril has no scope page; see T5) | — |
| T7 | **`/workflow run` preview.** Selecting from the picker now calls `X2n`, which re-fetches `listWorkflowRecipes()`, matches by `name`+`source`/`builtIn`, then `vre(…,"preview")`; `a==="preview"` 0 -> 1. The form gets `...s.length>0?{plan:s}:{}` (0 -> 1) with `s=n.plan??[]`, shown as a node tree plus a "N steps · showing …" summary. `/workflow run <name>` with all inputs still launches straight away (`"launch"`). `createWorkflow({source, inputs, modelId?})` unchanged | 2.32.0 | — | inbound (existing `listRecipes` reply) | STATIC | **D**: `WireRecipe` keeps only name/description/source; the doc comment says "`builtIn`, `inputs`, and `plan` are deliberately ignored" (`crates/cyril-core/src/protocol/convert/kas/workflow.rs:606-617`) | **NOTE cyril-4u4a** (item 3: plan[] now feeds a first-party preview) |
| T8 | Workflow node `includedFrom{name,path}` on `type:"sequence"` (0 -> 4). It is validated (`[acp-client] Ignored invalid KAS workflow includedFrom` 0 -> 1), labels the node `name (occurrenceId)`, and sets `sourcePath`. KAS emits it in the recipe zod schema (`x.literal("sequence"),id,steps,includedFrom:Lhe.optional()`; KAS count 8 -> 9) | 2.32.0 | — | inbound (plan / node trees) | STATIC | **D**: serde ignores the unknown key on `WireNodeDescriptor::Sequence` (`crates/cyril-core/src/protocol/convert/kas/workflow.rs:341-345`) | NOTE cyril-inwg (recipe `include`) |
| T9 | Restart-interrupt pause detection: the TUI matches `pauseReason` against a literal list `TFn`, which gains KAS 0.66.43's new wording "Step interrupted (agent shutdown or connection reset); resuming the run retries the step." (TUI 0 -> 1; KAS 0 -> 3, while the old "…; will resume." goes KAS 3 -> 0) | 2.32.0 | — | inbound text | STATIC | **H**: cyril renders `pauseReason` verbatim (`convert/kas/workflow.rs:303`, `:842`) and does no string matching | — |
| T10 | **OSC 7501 program status.** `function V3(e){let t=e.state==="blocked"?`:kind=${e.kind}`:"";return`\x1B]7501;state=${e.state}${t}:app=${Otr}\x07`}` (`Otr="kiro-cli"`). The state is derived by `mlt()`: blocked(question\|permission) / working / error / done / idle. It is written only when `!Kx("KIRO_NO_PROGRAM_STATUS")&&stdout.isTTY`, never in `noInteractive`. It is re-emitted after a screen clear (`reannounce`), and `finish()` writes `state=clear` | 2.32.0 | env opt-out `KIRO_NO_PROGRAM_STATUS` (truthy = set, non-empty, not "0"/"false") | terminal (not ACP) | STATIC | N/A (cyril emits no OSC status) | NEW P4 (TUI parity) |
| T11 | Turn-failure classification: `AS(promptResult)` -> `{result:"success"\|"cancelled"\|"failed", failureReason}`. A failure now **throws** `PromptFailedError` (0 -> 1) with user copy per reason: model_error "The model could not finish this turn", tool_error, timeout, context_limit, execution_limit ("The turn stopped at its execution limit" for `max_tokens`/`max_turn_requests`), internal_error, unknown. `end_turn`+`_meta.kiro.turnFailureReason` = failed. In 2.31.1, `turnFailureReason` fed telemetry only | 2.32.0 | — | inbound `session/prompt` result | STATIC | **H** for the stopReason (`crates/cyril-core/src/protocol/convert/mod.rs:162-164`; `bridge.rs:24-26` maps MaxTokens/MaxTurnRequests/Refusal to Failed). `turnFailureReason` is not read, but KAS has 0 hits, so it is v2-only | — |
| T12 | What's-new memory section: `S5e(engine,cloud,settings)` returns `null` unless `Rn("memory")&&JE("kas",cloud)` (`JE=e==="kas"&&!t&&Rn("memory_controls")`). It returns "launch-v3" (non-KAS: "It runs on the V3 engine: relaunch with kiro-cli --v3 to use it."), "enable" (`memory.mode==="disabled"`: "Memory is off. Turn it on in /memories → config.") or "manage". The release-summary strings "Cloud sessions, specs, …" are removed (1 -> 0 each) | 2.32.0 | rollouts `memory` + `memory_controls` (host `KIRO_ROLLOUT_FEATURES`/`KIRO_ENABLED_FEATURES`); mode default `KIRO_INTERNAL==="1"`?read_write:disabled | TUI-local | STATIC | N/A | NOTE cyril-0na7 (the TUI copy treats `memory.mode` as the user-facing switch) |
| T13 | Model picker arrangements: sort `last-used` (default) \| `cost` \| `company`. Settings `chat.modelPicker.sortBy` (0 -> 1) and `chat.modelPicker.lastUsed` (0 -> 1; up to 20 `modelId -> ts`). `cost` buckets by `_meta.kiro.rateMultiplier` ("4x and up", "2x to 4x", "1x to 2x", "Under 1x", "Rate unknown"); `company` is a prefix map `[["claude-","Anthropic"],["gpt-","OpenAI"]]` else "Other providers". Rows whose description matches `/deprecated/i` sink. New column header "Credit usage" (0 -> 1); cell `${rateMultiplier.toFixed(2)}x credits` | 2.32.0 | — | TUI-local (reads existing `config_option_update` model `_meta.kiro.rateMultiplier`/`rateUnit`) | STATIC | **D** for the rate: `effort_metadata` reads only `hasEffort`/`effortLevels`/`defaultEffortLevel` (`crates/cyril-core/src/protocol/convert/kas.rs:108`), so `rateMultiplier` is dropped | NOTE on cyril-vh69 (P4: credit multiplier in the KAS picker) |
| T14 | V2 `/model` fallback picker: `unset\|disabled\|target` (`kind:"unset"`/`"target"` 0 -> 1). "None" now sends `{targetModelId, disabled:true}`. Copy: "No configured fallback; a refusal still tries the vendor default." (1 -> 0) replaced by "No fallback: a refusal stays on this model instead of trying the vendor default." (0 -> 1) | 2.32.0 | v2 options only (`.fallback`) | outbound v2 | STATIC | v2-only, demoted (cyril-ci3j territory) | — |
| T15 | `/chat save` copy: `return`${t} (zip archive)`` when the extension is not `.zip` (0 -> 1). Save and load still shell out to the host `chat _ export-session` / `import-session`; the load error text comes from the host (`c.message`) | 2.32.0 | — | TUI-local + host IPC | STATIC | N/A (cyril has no `/chat save`) | — |
| T16 | `/code status` formatter reads `status.workspaceRoot` / `status.lspConfigPath` (0 -> 1) instead of `rootPath`/`configPath`. The input is the `{data:{success, status}}` command-response shape; **KAS has 0 `lspConfigPath`**, so this is a v2/host response change | 2.32.0 | — | inbound v2 | STATIC | v2-only, not consumed by cyril | — (host lane) |
| T17 | Approval key chain: the composer is lent to the folder text box (`restrictedEditing` 0 -> 1, `setCommandInput`/`setPromptHint` snapshot and restore) | 2.32.0 | — | TUI-local | STATIC | N/A | — |

### Negative results (load-bearing)

| check | 2.31.1 | 2.32.0 | evidence |
|---|---|---|---|
| `_kiro/*` / `kiro.dev/*` literal sets | 63 / 38 | 64 / 38 (+`_kiro/mcp/oauthCompleted`) | `sets/sets-diff.txt` |
| gates method table | 113 | 114 (same + 1) | `gates-2.31.1-2.32.0.txt` |
| slash literals / registry enum | 74 | 74, symmetric diff empty, no description strings added | `sets/sets-2.32.0.slashcmd.txt`; `vocab/2.31.1__2.32.0.strings.added` |
| hidden `chat _` subcommands | 11 | 11 | `ensure-session, export-session, get-kas-token, get-telemetry-credentials, import-session, midway-expiry, refresh-feed, upgrade-agent, windows-kas-process, windows-kas-stop, windows-sandbox` |
| KAS settings builder rows / defaults / rollout checks | = | = | rows `chat.disableAutoCompaction…workflows->goal`; rollout checks `c2s, memory, memory_controls` |
| clientMeta (`initialize.clientCapabilities._meta.kiro`) | = | = | rename-normalised region diff empty; **no `diagrams`** (the only `diagrams` hit is Mermaid copy "diagrams are not drawn in the terminal yet") |
| agentCapabilities parsed | 7 | 7 | `executionTargets, extensionMethods, replayMarking, sessionListScopes, sessionSources, sourceProviders, strictSessionLoad` |
| prompt `_meta.kiro` keys | displayText, outputStyle | = | |
| `.kiro?.<key>` reads / ext calls | 19 / 6 | 19 / 6 | `consumers-2.31.1-2.32.0.txt` |
| session `_meta` tokens: overrideSessionId / requireExisting / replayMarking / interaction_resolved / strictSessionLoad / promptScoped | 1/6/15/1/5/1 | = | token counts |
| KAS spawn argv/env | `[prefix,"--experimental-wasm-modules",server,"--transport=stdio","--auth=acp-callback",…policy,--endpoint/--control-plane-endpoint/--region]`, `NODE_USE_SYSTEM_CA`, `KIRO_CUSTOM_USER_AGENT` | renames only | `--diff '"--transport=stdio"'` |
| new KAS 0.66.43 fields read by the TUI: `workflowPlan`, `creatorEffort`, `capturePinned`, `bufferOnly`, `callerSessionId`, `diagram(Error)`, `create_diagram`, `max-tokens-recovery`, `plan_changed`, `_kiro/session/notify` | 0 each | 0 each | the TUI consumes none of the new KAS surface the kas lane found |
| `subagentOrchestration` / `todoList` | 0 / 0 | 0 / 0 | cyril still sends both (cyril-53qx) |
| `/workflow new` canned prompt | — | byte-identical after renames | `tmp/wfnew-*.txt` diff: only minified names |

---

## Slash commands

**Set diff: none.** The 74 slash literals are identical in both versions:

`/agent /autonomous /changelog /chat /checkpoint /clear /code /compact /config /context /context-gatherer /copy /custom /disconnect /editor /effort /exit /experiment /feedback /fullscreen /general-task-execution /goal /guide /help /hooks /issue /kiro /knowledge /kvim /lite /load /logdump /mcp /memories /model /paste /plan /powers /prompt /prompts /quit /reasoning /reply /repo /rewind /sandbox /save /session-id /sessions /settings /skill /spawn /spec /stats /steering /switch /tab /tangent /theme /title /todos /tools /transcript /tui /upgrade-agent /usage /verbosity /voice /workflow /workflow-cancel /workflow-resume /workflow-run /workflow-status /workflows`

The `name:"/x"` entries (37) and the command-description strings are unchanged too: no added literal in `vocab/` matches a description.

Changed behaviour of existing commands:

| command | change | class |
|---|---|---|
| `/workflow run` (picker) | always opens the inputs form with a plan preview (T7). Launch is still `_kiro/workflow/new {source, inputs, modelId?}` then `_kiro/workflow/invoke` | **KAS-method-backed**: `_kiro/workflow/listRecipes` (re-fetched; `plan[]`), then `_kiro/workflow/new` / `_kiro/workflow/invoke`. No new method |
| `/workflow new <prompt>` | canned prompt unchanged. "Shows its plan for your approval" is KAS-side: the kas lane found `session/request_permission` with `toolCall.content` plan text and `_meta.kiro.workflowPlan` (toolId `update_workflow`), which the TUI renders through its generic approval path (rawInput / tool_use content; `workflowPlan` 0 == 0) | KAS-method-backed: prompt text `/workflow new …` plus a standard permission request |
| `/model` (V3 picker) | sort arrangements and a credit column (T13) | TUI-local (reads the existing `model` configOption `_meta.kiro.rateMultiplier`) |
| `/model` fallback row | `unset\|disabled\|target`, "None" disables (T14) | **v2-only** |
| `/settings display` → output style | catalog unchanged in code. New values come from KAS (`proactive`, `explanatory`, `learning`) | **KAS-method-backed**: `_kiro/config/template` (`configOptions[id=outputStyle, category=output_style, _meta.kiro.promptScoped:true]`), sent per prompt as `session/prompt _meta.kiro.outputStyle` |
| `/chat save` / `/chat load` | zip-archive copy (T15). "Altered after saving" comes from the host | TUI-local + host `chat _ export-session` / `import-session` |
| `/code status` | field rename (T16) | v2-only (command-response shape) |
| `/memories` | named by the new What's-new memory copy (T12); the command is unchanged | TUI-local + existing `_kiro/memory/*` |

---

## Feature flags and gates

| flag / gate | 2.31.1 | 2.32.0 | default | gates |
|---|---|---|---|---|
| rollout gate function (`Bn` -> `Rn`), names | 10 | 10 (same set) | off unless in `KIRO_ROLLOUT_FEATURES` / `KIRO_ENABLED_FEATURES`; `local_sandbox` = `KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED==="1"` | — |
| `Rn("memory")` call sites | 1 | **2** | off unless the host lists it | + What's-new memory section (T12), together with `memory_controls` |
| other gate call-site counts | background_execution 4, c2s 2, cloud_config 12, local_sandbox 5, managed_settings 5, memory_controls 2, remote_sandbox 4, session_dashboard 3, workflows 2 | = | — | unchanged |
| `KIRO_NO_PROGRAM_STATUS` (env) | 0 | 1 | unset = OSC 7501 on (TTY only) | T10 |
| `KIRO_INTERNAL==="1"` (`ws()`) | existing | + used as the memory-mode default in What's new | unset => `disabled` | T12 |
| settings `chat.modelPicker.sortBy` | 0 | 1 | `last-used` | T13 |
| settings `chat.modelPicker.lastUsed` | 0 | 1 | `{}` | T13 |
| `KIRO_*` env set | 94 | 95 (+`KIRO_NO_PROGRAM_STATUS`) | | `sets/sets-diff.txt` |
| `AB_*` | 0 | 0 | | |

---

## Unreleased / dark candidates

| candidate | gate | default | evidence |
|---|---|---|---|
| **`_kiro/mcp/oauthCompleted` + `openExternalUrl{serverName, oauthCompletion:"notification"}`** (completion-notification OAuth flow) | none in the TUI; needs KAS to send `oauthCompletion:"notification"` | **dark**: KAS 0.66.43 has 0 hits for `oauthCompleted` and for `oauthCompletion` in the whole carve | `if(!i\|\|typeof o.success!=="boolean"\|\|s?.completion!=="notification")return;` |
| What's-new memory introduction | rollouts `memory` AND `memory_controls`, not a cloud session | off unless the host rollout lists both (host lane: registry values) | `function S5e(e,t,n){if(!Rn("memory")\|\|!JE("kas",t))return null;if(e!=="kas")return"launch-v3";…` |
| KAS `diagrams` client capability (kas lane) | `initialize.clientCapabilities._meta.kiro.diagrams` | **the TUI never sends it** (0 relevant hits), so `create_diagram` stays off for Kiro's own TUI | negative result above |
| v2 `refreshOnly` on `kiro.dev/mcp/server_initialized` | — | needs the host to emit it (host lane) | T4 |

---

## Leads for the live lane

Every leg: `HOME=<tmp>` with the real `XDG_DATA_HOME` (traps 7), a direct `node acp-server.js` from the 2.32.0 carve (traps 8), and the full handshake path `initialize.clientCapabilities._meta.kiro.{configurationState:true, settings:{…}}` (traps 5). **Positive control** in every leg: `configurationState:true` must make KAS push `_kiro/configuration/state` (traps 28). No forged backend (traps 13).

**L1 — folder write grant (T5, cyril-gn07).** Make a KAS turn write `<ws>/src/a/x.txt`. Answer `session/request_permission` with `{outcome:{outcome:"selected", optionId:<allow_always option id>}, _meta:{kiro:{consent:{capability:"fs_write", scope:"workspace", resource:"<ws>/src/**", workspaceRoot:"<ws>"}}}}`. Then (a) in the same session, a write to `<ws>/src/b/y.txt` should NOT raise a request; (b) in a fresh session (`session/new`, same workspace) the same should hold if the scope persisted, so record which file changed (agent V3 `permissions.rules`, a workspace `.kiro/…`, or a user file). Arms: `scope:"session"` (expect no persistence) and `scope:"user"`. Control arm: the same reply **without** `_meta.kiro.consent` (cyril today), where a re-ask is expected. Record the request's `consentContext` keys (`askType`, `triggeringResource`) to confirm the offer conditions.

**L2 — output styles (Slash commands: `/settings display`; NEW P2).** Pre-session `_kiro/config/template {}` and post-`session/new`: capture `configOptions[id=outputStyle]`. Expected: `category:"output_style"`, `_meta.kiro.promptScoped:true`, options ⊇ {default, proactive, concise, explanatory, learning}. Then send `session/prompt {…, _meta:{kiro:{outputStyle:"explanatory"}}}` against `"default"` on the same prompt and look for a visible style difference. Also check whether `session/set_config_option configId:"outputStyle"` is accepted (KAS accepts unoffered values silently, so read back `currentValue`).

**L3 — recipe `plan[]` / `includedFrom` (T7, T8).** Seed a workspace recipe with `include` (cyril-inwg) and call `_kiro/workflow/listRecipes {}`. Expected: the row carries `plan[]`, with `type:"sequence"` nodes carrying `includedFrom:{name, path}`. Also record `plan` on the `run_start` node tree.

**L4 — OAuth completion flow (T1, T2).** It is dark (expected NOT to fire). Only if budget remains: advertise `clientCapabilities._meta.kiro.openExternalUrl:true` (the TUI host-capability key `openExternalUrl`) with a remote MCP server that needs OAuth. Record the `_kiro/openExternalUrl` params (do `serverName` and `oauthCompletion` appear?) and any `_kiro/mcp/oauthCompleted`. Static prediction: neither appears on 0.66.43.

**L5 — `diagrams` capability (kas lane cross-ref).** Arm A sends `clientCapabilities._meta.kiro.diagrams:true`, arm B omits it. Compare the tool list for `create_diagram`. The TUI sends nothing, so arm B is the Kiro-TUI baseline.

**L6 — interrupted-step pause text (T9).** Kill KAS mid-step, restart, `session/load`, then `_kiro/workflow/inspect`. Expected `pauseReason` = "Step interrupted (agent shutdown or connection reset); resuming the run retries the step." This is a cheap check that cyril renders it verbatim.

---

## Changelog reconciliation (feed-2.32.0.json, 34 lines)

`L` = located in the TUI bundle.

| # | line | location |
|---|---|---|
| 1 | `--cloud` and `--repo` in non-interactive runs print the cloud session id | **not in the TUI** (`--cloud` 2 == 2, `--repo` 5 == 5, `cloudSessionId` 1 == 1; the +3 `cloudSession` hits are the What's-new `cloudSessionActive` plumbing). Host lane |
| 2 | OSC 7501 program status; opt out `KIRO_NO_PROGRAM_STATUS` | **L, T10** |
| 3 | [V3] Write approvals: custom folder at session/workspace/user scope | **L, T5**: `_meta.kiro.consent{capability:"fs_write", scope, resource:"<dir>/**", workspaceRoot}` on the allow_always option. No new option id |
| 4 | [V3] `/workflow run` confirmation form with plan preview | **L, T7**: no new method; `listRecipes[].plan` goes into the `workflow-recipe-inputs` panel; picker selection always previews |
| 5 | [V3] Proactive, Explanatory, Learning output styles in `/settings display` | **L (path), values KAS**: `_kiro/config/template` option `outputStyle` (`promptScoped`), sent as `session/prompt _meta.kiro.outputStyle`; TUI code unchanged (`outputStyle` 5 == 5), new values in KAS (kas lane) |
| 6 | [V3] Workflow coding/planning/design steps can use Powers | **not in the TUI**. kas lane (wf-* agents `+kiro_powers`) |
| 7 | [V3] Workflow design steps can use configured MCP servers | **not in the TUI**. kas lane (`includeMcpJson:true`) |
| 8 | Startup What's new introduces memory, with next steps for engine and memory setting | **L, T12**: gated on rollouts `memory` + `memory_controls`; setting `memory.mode`; three copies (launch-v3 / enable / manage) |
| 9 | [V3] Always Allow scope page shows the selected scope, `s` cycles | **L, T6** (TUI-local; label `global` "always" -> "user") |
| 10 | Subagents from a V2 session follow the model fallback config | **not in the TUI** (no hunk). Host v2 |
| 11 | [V3] A workflow step's session opens on its task | **not in the TUI**. kas lane |
| 12 | [V3] Creating a workflow from a prompt shows its plan for approval | **not a TUI change**: the canned `/workflow new` prompt is byte-identical; the plan arrives as a KAS permission request (kas lane: `toolCall.content` plus `_meta.kiro.workflowPlan`, which the TUI does not read, 0 == 0) and is rendered by the generic approval view |
| 13 | [V3] sharp/libvips no longer bundled | **not in the TUI** (`sharp` 403 == 403). KAS packaging (RUN-STATE) |
| 14 | [V3] Running workflows accept plain-language changes to remaining steps | **not in the TUI**: no new method or frame. kas lane: `update_workflow` `remainingSteps` -> `workflowPrompt`/`workflowRef` |
| 15 | [V3] Saved-session lists refresh without blocking | **not in the TUI** (no session-list hunk; `listSessions` 17 == 17). kas lane |
| 16 | [V3] Review loops pause after two rejected reviews and ask how to continue | **not in the TUI**: rendered by the existing `pauseReason` / `needs approval` node label. kas lane (feature-pipeline loops 3/abort -> 2/pause) |
| 17 | [V3] `/chat save` to a non-`.zip` name says it is a zip archive | **L, T15** |
| 18 | [V3] `/chat load` explains an altered archive | **not in the TUI**: the message comes from host `chat _ import-session` (`rg(e,c.message,"error")`). Host lane |
| 19 | [V3] Non-interactive `--resume-id` of a cloud session resumes it | **not in the TUI** (`--resume-id` 12 == 12). Host lane |
| 20 | MCP OAuth links disappear when their server connects, server named in the toast | **L, T1–T3**: per-server `mcpOAuthNotices`, "`<server>` connected" toast; dark `_kiro/mcp/oauthCompleted` path |
| 21 | Plain-text non-interactive runs explain why a turn was cancelled or refused | **L partly, T11** (`PromptFailedError` copy); the plain-text printer is host-side |
| 22 | Non-interactive runs exit nonzero when stdout is closed | **not in the TUI** (`EPIPE` 1 == 1). Host lane |
| 23 | Choosing None in the V2 `/model` fallback picker turns fallback off | **L, T14** (v2) |
| 24 | `/model` fallback row names the vendor default when no target is configured | **L, T14** (v2) |
| 25 | V2 crew whose stage cannot start fails with that error | **not in the TUI**. Host v2 |
| 26 | [V3] Paused step in a parallel branch finishes on success/failure | **not in the TUI**. kas lane |
| 27 | [V3] Turn no longer ends early when the model spends its output budget on reasoning | **not in the TUI** (kas lane: prompt origin `max-tokens-recovery`). The related TUI part: `max_tokens` now surfaces "The turn stopped at its execution limit" (T11) |
| 28 | [V3] Buffered workflow messages survive an agent restart | **not in the TUI**. kas lane (`bufferOnly`) |
| 29 | [V3] Command approval checks shell syntax for the running shell incl. fish | **not in the TUI**. kas lane |
| 30 | [V3] Completed workflow steps can send follow-up to their parent | **not in the TUI**. kas lane (`_kiro/session/notify` `callerSessionId`) |
| 31 | [V3] Org MCP registry servers use registry `oauth`/`oauthScopes` | **not in the TUI**. kas lane |
| 32 | [V3] Oversized images retry at the provider's size limit | **not in the TUI**. kas lane |
| 33 | [V3] Resumed sessions show saved model and effort before the model list loads | **not in the TUI** (no model/effort or `session/load` hunk; `effortLevel` 59 == 59). kas lane (likely early `config_option_update` on load) |
| 34 | [V3] Workflow pause messages no longer say a run will resume on its own | **L (consumer), T9**: the TUI pause-text list gains the new KAS string |

Also located, with no changelog line: the model-picker sort/credit column (T13) and the composer-lending approval editor (T17).

---

## Proposed issues/notes (checked against `.rivets/issues.jsonl`)

Priority per fold-and-file.md: P2 = dropped field or missing route on the default engine (KAS is cyril's current path); P4 = cosmetic or TUI-only parity.

- **NOTE cyril-gn07 (raise to P2)**: Kiro's 2.32.0 TUI uses the consent echo for **folder grants**: `allow_always` + `_meta.kiro.consent{capability:"fs_write", scope:"session"|"workspace"|"user", resource:"<dir>/**", workspaceRoot}`. It is offered only for KAS, the main session and non-`explicit` asks. Without the echo, cyril cannot express a folder grant at any scope (`convert/kas.rs:67-91` sends only `rejectionReason`). Live L1 decides whether the echo is required.
- **NEW P2 (kas, v3-parity, ready-for-agent after L2): KAS output styles.** Read `_kiro/config/template` / session `configOptions[id=outputStyle, category=output_style, _meta.kiro.promptScoped:true]`, offer it in a settings picker, persist it locally, and attach `session/prompt _meta.kiro.outputStyle:<id>` per prompt (the TUI's `krt()`). cyril has no outputStyle code (no hit in `crates/`). Related: cyril-lyrx (config/template), parity epic cyril-c6hv.
- **NOTE cyril-4u4a**: `listRecipes[].plan` now feeds a first-party pre-launch preview (T7). The "deliberately ignored" comment at `convert/kas/workflow.rs:606-608` no longer matches Kiro's UX; modelling `plan[]` unblocks a cyril `/workflow run` confirmation.
- **NOTE cyril-inwg**: sequence-node `includedFrom{name,path}` is consumed by the TUI (validated; labels plus `sourcePath`). cyril's `WireNodeDescriptor::Sequence` drops it silently (`workflow.rs:341-345`). Per the full-wire-surface rule: model it or explicit-ignore with a debug log.
- **NOTE cyril-nk4o**: `_kiro/mcp/status`-driven OAuth notice lifecycle (`mcpOAuthAttempts`, cleared on connect), plus the dark `_kiro/mcp/oauthCompleted {serverName, success}` and `openExternalUrl{serverName, oauthCompletion}` (KAS 0 hits).
- **NOTE cyril-vh69** (P4): the KAS model picker in Kiro shows `_meta.kiro.rateMultiplier` as "N.NNx credits" and sorts by last used, cost or company. cyril's `effort_metadata` (`convert/kas.rs:108`) drops `rateMultiplier`/`rateUnit`.
- **NOTE cyril-0na7**: the What's-new memory copy gates on rollouts `memory`+`memory_controls`. It treats `memory.mode==="disabled"` as "Memory is off" and its default is `KIRO_INTERNAL==="1"`?read_write:disabled.
- **NEW P4 (needs-triage, feature): OSC 7501 program status.** Emit `ESC]7501;state=<idle|working|blocked|done|error|clear>[:kind=permission|question]:app=<cyril>BEL` on TTY, honour an opt-out env, `clear` on exit. It is the terminal-native sibling of the Herdr reporter proposed in 2.31.1. Whether the `app=` value matters to terminals is unknown.
- No issue: T4/T14/T16 (v2-only, demoted per traps 24), T9 (handled), T11 (handled for KAS).

---

## Method notes

- **Scripts** (in `experiments/conductor-spike/`; the 2.31.1 originals are untouched):
  - `static-tui-{gates,region,sets,stmtdiff,vocab}-2.32.0.py` are copies of the 2.31.1 tools.
  - `static-tui-consumers-2.32.0.py` is new. It lists `onExtNotification` registrations, kind/case labels, `.kiro?.<key>` reads and ext-call literals, and takes a census of rollout-gate call sites. The census resolves the minified gate function per bundle from its `local_sandbox` body.
- **Outputs** (this directory): `sets/`, `vocab/` (+69/−7 literals, +45/−2 props), `gates-2.31.1-2.32.0.txt`, `consumers-2.31.1-2.32.0.txt`. Under `tmp/` (git-ignored, regenerable): the statement dumps, `d.diff` (853 hunks), `d.real300.txt` (320 real hunks after the 2.31.1 `stmt-filter.py`), and `wfnew-*.txt`.
- **Filter blind spot checked.** `stmt-filter.py` normalises identifiers of 6 characters or fewer, and stmtdiff also blanks string words of 4 characters or fewer. A second pass over the 533 dropped hunks compared their string-literal sets: 0 hunks hid a literal change. The lexer-based vocab diff is the literal safety net (it found, for example, `global:"always"` -> `"user"`).
- **Moved code is not changed code.** The memory-config helpers (`ck`, `Ukn`, `Pkn=["disabled","read_only","read_write"]`) moved earlier in the bundle: one hunk adds them and another removes them. A fixed 9 kB window diff of the settings builder therefore shows spurious adds. The gates extractor rows are the authority, and they are identical.
- **Cross-lane greps** (read-only) of the KAS carves established that `oauthCompleted`/`oauthCompletion`/`lspConfigPath`/`turnFailureReason` are absent from KAS and that `includedFrom` and the new pause wording are present. Rows T1, T2, T9, T11 and T16 rest on these. The kas lane's interim `FINDINGS.md` supplied the KAS-side meaning of changelog lines 12, 14, 16, 27, 28 and 30.
- **Trap compliance.** Every add or remove carries OLD -> NEW counts from `grep -oF` on both bundles (traps 2). Set diffs, not fixed lists (traps 3). All rows are STATIC (traps 4). Every live lead writes the full `initialize.clientCapabilities._meta.kiro` path and uses `configurationState:true` as the positive control (traps 5, 28). No frame citations: this is a static lane.

## Progress log
- [P1] trap-3 sets (`static-tui-sets-2.32.0.py` -> `sets/`): `_kiro/*` 63 -> 64 (+`_kiro/mcp/oauthCompleted`), `kiro.dev/*` 38 == 38, slash literals 74 == 74, `name:"/x"` 37 == 37, `chat _` subcmds 11 == 11, ext-call literals 20 == 20, rollout-check literals 61 == 61, `case"x_y"` 144 == 144, KIRO_* env 94 -> 95 (+`KIRO_NO_PROGRAM_STATUS`), AB_* 0. `kind:` literals +disabled/fold/move/permission/question/target/unfold/unset (all pre-existing as raw text elsewhere in 2.31.1).
- [P2] vocab differ (`static-tui-vocab-2.32.0.py` -> `vocab/`): +69/-7 literals, +45/-2 props, env reads 113 == 113.
- [P3] gates extractor (`static-tui-gates-2.32.0.py` -> `gates-2.31.1-2.32.0.txt`): settings rows/defaults/rollout checks/mentions, agentcaps (7 keys), KAS spawn argv+env, `chat _` subcmds, prompt `_meta.kiro` keys {displayText, outputStyle} — IDENTICAL. Method list 113 -> 114 (+`_kiro/mcp/oauthCompleted`).
- [P4] stmt diff (`static-tui-stmtdiff-2.32.0.py` + audit-2.31.1 `stmt-filter.py`, kept in tmp/): 853 hunks, 320 real (non-rename). Themes: MCP OAuth notices + `_kiro/mcp/oauthCompleted`; write-approval folder offer (fs_write, `kasScope`/`kasResource:"<dir>/**"`); Always-Allow scope page reducer + `s` cycle; workflow run preview form with `recipe.plan`; workflow `includedFrom` (sequence nodes); OSC 7501 program-status reporter; What's-new memory section; prompt failure reasons (`PromptFailedError`); model picker sort/vendor/rate arrangements + `chat.modelPicker.*`; V2 `/model` fallback `unset|disabled|target`; `/chat save` zip suffix; MCP status `workspaceRoot`/`lspConfigPath` rename in code-intelligence panel.
- cyril permission-response baseline: cyril sends only `_meta.kiro.rejectionReason` (`crates/cyril-core/src/protocol/convert/kas.rs:67-91`); no `_meta.kiro.consent{scope,resource}` on any response. Related open issue cyril-gn07.
- [P5] Located changelog leads: folder approvals (`b2t`/`y2t`/`cw`), scope page (`Ov`, `Ol`), workflow preview (`X2n`/`vre`), `includedFrom` (`wLn`), OSC 7501 (`V3`/`IDt`), What's-new memory (`S5e`), `PromptFailedError` (`AS`), model picker (`kS`, `fst`, `pst`), OAuth (`Osr`, `_kiro/mcp/oauthCompleted`). Not in TUI: lines 1, 6, 7, 10–16, 18, 19, 22, 25–33.
- [P6] Cross-ref: KAS 0.66.43 has 0 `oauthCompleted`/`oauthCompletion`/`lspConfigPath`/`turnFailureReason`; it carries `includedFrom` (8 -> 9) and the new pause text (0 -> 3). The TUI reads none of the new KAS fields from the kas lane (`workflowPlan`, `diagrams`, `creatorEffort`, `capturePinned`, `bufferOnly`, …).
- [P7] Wrote `static-tui-consumers-2.32.0.py` -> `consumers-2.31.1-2.32.0.txt`. FINDINGS complete.
