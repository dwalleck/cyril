# v3-coverage lane — standing KAS-default parity inventory (kiro-cli 3.1.0, KAS 0.66.91, TUI 3.1.0, cyril HEAD e3a3868d)

Lane run 2026-11-08. Static only: nothing from Kiro was executed (no kiro-cli, no node acp-server.js). This is the STANDING
parity list (Kiro's reference v3 client × the KAS engine × cyril), not a delta: every version-to-version fact is taken from the
delta lanes and cited as `kas-core`, `kas-workflow`, `tui`, `host` (their `FINDINGS.md` under `audit-3.1.0/`) or `live leg N`
(`audit-3.1.0/live/FINDINGS.md`, progress log; legs 0, 1, 2a done at write time; leg 5 cyril-on-KAS pending). It UPDATES the
nine matrices of `audit-2.32.0/v3-coverage-findings.md` (cyril HEAD 7c7b0d1d); a row marked "= 2.32.0" was re-checked at
HEAD e3a3868d and against the 0.66.91 / TUI 3.1.0 bundles and did not change.

Artifacts: ENGINE `~/.local/share/kiro-research/kas-carves/3.1.0/node_modules/@kiro/agent/dist/server/acp-server.js` (0.66.91,
16,099,406 B; "KAS"); OLD engine 2.32.0 (0.66.43, 12,963,334 B); REFERENCE CLIENT `~/.local/share/kiro-research/tui-bundles/
kiro-tui-3.1.0.js` (14,291,547 B; "TUI"); CYRIL `crates/` at HEAD e3a3868d (`git rev-parse HEAD` = e3a3868df693…). Every
`file:line` below is at that HEAD (trap 17/31); paths are repo-relative. The brief's `crates/cyril/src/domain_mediator/…` does
not exist: the mediator lives at `crates/cyril-core/src/protocol/domain_mediator/` and is cited there.

Verdicts (trap 17): HANDLED / DROPPED (received, ignored) / NOT-SENT / NOT-IMPLEMENTED / BROKEN. Every row is STATIC unless
marked LIVE(live leg N), which cites the live lane's own checked captures; this file deliberately carries no frame citations
(its captures do not resolve from this directory — see Method notes). Class column: **DAY-ONE-GAP** (a cyril user on KAS 0.66.91
hits it without doing anything special; priority in the ranked list), **FOLLOW-UP** (opt-in, cosmetic, or not on the default
path), **OK**.

Counts verified by this lane (`static-v3cov-{ctx,kinds,methods,issues}-3.1.0.py`, outputs `tmp/kinds.json`, `tmp/methods.json`):
`_kiro/*` literals in 0.66.91 = **138** (C2A 65 · A2C-N 43 · A2C-R 16 · prefix/internal 14); cyril names **44** of them in non-test
code (C2A 15 · A2C-N 17 · A2C-R 10 · prefix 2); **43** are called or handled by the TUI and unnamed by cyril; **51** are named by
neither. `session_info_update` kinds = **29**, cyril HANDLED **9**, DROPPED **20** (kas-core's list, re-counted). TUI slash-command
literals **75**, registered definitions **38** (tui lane `tmp/sets/sets-3.1.0.slashcmd*.txt`, re-counted). cyril builtins on KAS:
**23** (24 under `kas_hooks = "kas"`) — `crates/cyril-core/src/commands/mod.rs:511-571`.

## Summary

1. **All eight fixes closed since the 2.32.0 audit are present at HEAD and nothing in 0.66.91 is expected to break them**
   (regression table below): `shell_type` now answers the interpreter basename (`host_shell.rs:216-222`) and 0.66.91's
   classifier `xp()` still maps `bash|zsh|sh|dash|ksh|fish|pwsh|powershell` (quoted in Matrix 5); the `get-kas-token` argv and
   envelope are byte-identical through 3.1.0 (host lane); the Always-allow consent echo (`convert/kas.rs:128-190`) meets an
   unchanged KAS intake (`scope||"session"`, quoted); orphan `tool_call_update`s own a row (`cyril-ui/src/state.rs:555-560`);
   skills/prompts/custom agents dispatch as prompt text (`convert/kas.rs:493-548`); `/model` `/effort` `/mode` ride
   `set_config_option` (`commands/mod.rs:528-534`); `KIRO_HOME` has one validated resolver (`kiro_agent_config.rs:94-152`); the
   repeat-cap wake is labelled (`workflow.rs:638-650`, `turn_labels.rs:23-47`) and the 0.66.91 wake path is unchanged (kas-workflow,
   live leg 2a). Live leg 5 gives the verdicts; this file gives the expectations.
2. **The day-one list is now led by three new 0.66.91 drops on the default engine, not by blockers:** model fallback is ON by
   default (`model_fallback_{refusal,availability,auto_refusal}` → true; kas-core) and cyril drops `model_fallback`/`model_routed`
   at `convert/kas.rs:822`, so the toolbar model (`widgets/toolbar.rs:63-66`) goes stale after a silent swap; the new
   `session_usage` kind (LIVE leg 1, replayed on `session/load`) is dropped, so cyril's local sum (`cyril-core/src/session.rs:291`,
   `types/session.rs:626-663`) restarts at zero on `/load`; workflow step `credits[]` (LIVE leg 2a, gate-off) are dropped by
   `convert/kas/workflow.rs:207-218,259-283,444-520`. Two standing P1s are unchanged by 0.66.91 and still reachable through
   cyril's own `/review`: cyril-oswq (cancel → abort wake → the main agent redoes the review) and cyril-w55l (step Always-allow
   loops 20×; "Permission flow exceeded 20 approval rounds" 2 = 2, kas-workflow).
3. **Handshake drift is one row deeper:** cyril still sends `thinking`, `todoList`, `subagentOrchestration` (`kas/settings.rs:
   77,80,91-96`) — 0.66.91 logs `configuration.intake.unknown-key` for `thinking` (LIVE leg 0, both engines) — and does not send
   the TUI's new `sessionRecap` (3.1.0) nor `memory`/`userMemoryOptIn`, which became load-bearing when the memory rollout flipped
   to `all` in 0.66.58 (kas-core): a user's agent file with `memory.mode` now turns KAS native memory on under cyril.
4. **The TUI's 2.33.0–3.1.0 additions are mostly opt-in or dark for a cyril user** (trap 29): `workflowLaunch` needs
   `chat.enableWorkflows`; session recap needs a setting; Approve-for-me needs `KIRO_TRUST_CLASSIFIER_MODE=enforce` on both sides;
   diagrams/code-index need the `c2s` rollout; `_kiro/modelFallback/*` has no KAS implementation at all (0 hits in six carves,
   tui lane). The one TUI path that lit up on cyril's existing advertisement is `_kiro/mcp/oauthCompleted` (cyril advertises
   `openExternalUrl`, `engine.rs:143`; reads only `url`, `kas/callbacks.rs:190-193`; drops the notification at `convert/kiro.rs:1305`).
5. **Classic retirement does not kill cyril's v2 code yet** (host lane): 3.1.0 still ships the V2 Rust ACP engine and every
   `_kiro.dev/*` literal; the January removal is announced in Kiro's own prompt. What breaks today is the V2 *wrapper* spawn: a
   bare `kiro-cli acp` (`bridge.rs:394`, default argv `crates/cyril/src/main.rs:32-36`) hits the 2.36.0 `clientInfo.name` router
   as an unlisted client (`cyril`, forced on V2 by `identity.rs:22-27`) and, if the 3.x acp default is v3 (HOST-L1), fails at
   `fingerprint.rs:92-105` with a remedy that names the same command. Not the default path → FOLLOW-UP (host P2), Matrix 8.
6. **New cyril surface since 2.32.0 closes two 2.32.0 FOLLOW-UP rows:** `/mcp` is a KAS builtin with a panel
   (`commands/mod.rs:514-516`, `widgets/mcp_panel.rs`; mediator `commands/mcp.rs:34-42` calls `resetServer`/`getPrompt`/`getResource`;
   cyril-nk4o, cyril-tqr8 closed) and `/answer` answers a parked workflow step (`commands/answer.rs:21-29`,
   `domain_mediator/commands/session.rs:606`; cyril-9104). `/steer` rides `_session/steer`, which KAS 0.66.91 implements
   (persistence class `metadata`, quoted below) — it is not v2-only.

## Matrix 1 — Handshake `initialize.clientCapabilities` (`_meta.kiro`, `.settings`, `fs._meta.kiro`)

cyril builder `crates/cyril-core/src/protocol/engine.rs:121-163` (`client_capabilities`): `fs{readTextFile,writeTextFile,
_meta.kiro{readFile,writeFile,stat,readDirectory,delete}}`, `terminal:true` (:134), `_meta.kiro.settings` (:139, from
`kas/settings.rs` marshal), `_meta.kiro.openExternalUrl:true` (:143), `_meta.kiro.hooks` `{enabled:true}` (:149) or
`{enabled,v2}` (:154). The TUI 3.1.0 object is tui lane "Handshake object" (`_Bt(e)` + the seven `type:"other"` host-capability
entries). KAS 0.66.91 resolver: 17 keys, unchanged (kas-core); known-extra settings keys `c2s, verifyFirstWorkflow, sessionRecap,
steeringReminders, memoryEnable, userMemoryOptIn, evalStrategy, evalTreatments` (tui lane, identical list in 0.66.43).

### 1a. Top-level `_meta.kiro` keys

| key | KAS 0.66.91 | TUI 3.1.0 | cyril @HEAD | consequence for a cyril user | issue | class |
|---|---|---|---|---|---|---|
| `settings` | consumed (zod 61 keys, kas-core) | sent (13 rows) | SENT (`engine.rs:139`) — see 1b | see 1b | cyril-53qx | — |
| `hooks{enabled,v2?}` | consumed; v2 = KAS runs file hooks itself | `{enabled:!0,v2:!0}` | SENT `{enabled}` (host mode, default) or `{enabled,v2}` (`engine.rs:149-154`) | parity by design; compaction hooks (3.1.0) exist only on the KAS-side v2 runtime, never on `_kiro/hooks/list` (kas-core) | cyril-497j | OK |
| `openExternalUrl` | gates `_kiro/openExternalUrl` AND (0.66.91) `_kiro/mcp/oauthCompleted` (kas-core) | sent | SENT (`engine.rs:143`) | cyril now receives `_kiro/openExternalUrl{url,serverName,oauthCompletion,oauthFlowId}` and the completion notification — both partially/fully DROPPED (Matrix 4c, 5) | kas-core NEW P4 | FOLLOW-UP P4 |
| `authCallback` | 0 hits (dead; `_kiro/auth/getAccessToken` is unconditional with `--auth=acp-callback`) | sent (`type:"other"`) | NOT-SENT; responder present (`kas/auth.rs:37`) | none | — | OK |
| `userInput` | gates `_kiro/userInput` (`userInputCapability`, quoted Matrix 5) | sent | NOT-SENT | option-less/spec questions take the legacy `request_permission` path | cyril-qo13 (closed) | FOLLOW-UP P3 |
| `secretStorage` | gates `_kiro/secret/{get,store,delete}` ("Client supports ACP secret storage") | sent | NOT-SENT (KAS keeps its own store) | remote-MCP OAuth creds live in KAS's store, not cyril's | — | OK |
| **`mcpElicitation`** (NEW TUI 2.34.0) | **0 hits** — KAS calls `_kiro/mcp/elicitation` unconditionally (tui T8, re-counted: `mcpElicitation` 0 in 0.66.91) | sent | NOT-SENT; responder answers a visible decline (`kas/mcp.rs:50-55,70-82`) | advisory flag; nothing to send | — | OK |
| `telemetryEnabled` / `telemetry{…}` | consumed (`clientTelemetryEnabled`) | sent | NOT-SENT | KAS telemetry at its default | cyril-zjm5 | FOLLOW-UP P3 |
| `knowledge` | consumed | `!0` | NOT-SENT (the settings key is) | = 2.32.0 | — | FOLLOW-UP P4 |
| `requirementsAnalysis`, `specPhaseCheckpoints` | consumed | sent | NOT-SENT | no spec UI (= 2.32.0) | — | OK |
| `streamingShellContent` | consumed (17 = 17, 2.32.0) | `!0` | NOT-SENT | no live shell output chunks | cyril-2mo0 | FOLLOW-UP P3 |
| **`diagrams`** (TUI 2.33.1, `c2s` rollout) | `diagrams:r.diagrams===!0` → `create_diagram` tool, `_kiro/diagram/render`, `diagramRenderFormats` (kas-core) | sent only when the host rollout lists `c2s` (dark) | NOT-SENT | none vs TUI (dark) | cyril-68ny | OK |
| `infrastructureSafety` | consumed | env-gated | NOT-SENT | = 2.32.0 | cyril-3ald | OK |
| `configurationState` | makes KAS push `_kiro/configuration/state` (positive control, trap 28) | not sent | NOT-SENT | 0.66.91 would show the 21 per-model `modelFallback/<id>` chains (LIVE leg 1) | cyril-iq2c | FOLLOW-UP P3 |
| `terminalInput` | consumed; without it the model is told "This client does not support writing to terminal stdin (no terminalInput capability)." (quoted, 0.66.91) | not sent (TUI hosts no terminals) | NOT-SENT while cyril hosts terminals | interactive/background processes get no stdin | cyril-7p7n | FOLLOW-UP P3 |
| `agentHostsForegroundShell` | consumed (`Yrs(...)` host selection) | not sent | NOT-SENT | design alternative only (= 2.32.0) | cyril-7p7n | OK |
| `backgroundProcesses`, `clientTool{SemanticRename,SmartRelocate,GetDiagnostics}`, `c2sViews`, `textSearch`, `findFiles`, `helpDocs`, `specLinks`, `inactiveItems`, `notifications.documentsChanged`, `resolvesSteeringCommands` | consumed | not sent | NOT-SENT | none (TUI parity) | — | OK |
| `fs.{readTextFile,writeTextFile}` + `fs._meta.kiro.{readFile,writeFile,stat,readDirectory,delete}` + `terminal` | adapters swapped per flag (covenant §5) | not advertised | SENT (`engine.rs:121-134`); responders `kas/kiro_fs.rs:85-89`, `kas/terminal_io.rs` | cyril is the fs/terminal host (deliberate divergence) | — | OK |
| `largeToolOutputHandler`, `workflowNotifications` | 0 hits / unknown-key (LIVE leg 0 logs `setting/largeToolOutputHandler`) | TUI sends both (dead) | NOT-SENT | none | — | OK |
| `_meta["terminal-auth"]` (ACP terminal auth, 2.37.0 — host-side) | n/a (host `kiro-cli acp` feature) | — | NOT-SENT; `authMethods` unread | wrapper-only; the free path uses the callback | cyril-cxd2 (host NOTE) | OK |

### 1b. `_meta.kiro.settings` (AgentSettings) — KAS zod 49 → 61 keys (kas-core `settings.txt`)

cyril marshal `crates/cyril-core/src/protocol/kas/settings.rs`: `BOOL_MAP` :76-86 (`chat.enableThinking→thinking`,
`enableKnowledge→knowledge`, `enableCodeIntelligence→codeIntelligence`, `enableTodoList→todoList`, `enableCheckpoint→checkpoint`,
`enableTangentMode→tangentMode`, `disableAutoCompaction`, `enableSubagent→_subagent`, `enableDelegate→_delegate`), `DEFAULTS_ON`
:91-96 (`codeIntelligence`, `knowledge`, `thinking`, `subagentOrchestration`), `workflows`+`goal` :143-145, `toolSearch` :147-156,
`compaction{excludePercent,excludeMessages}` :159-172, `knowledge{…}` :179-202; source file `<KIRO_HOME>/settings/cli.json`
:63-65. The same object also goes out on `session/new` (`domain_mediator/commands/session.rs:61-77`, cyril-sm36).

| key | KAS 0.66.91 | TUI 3.1.0 sends | cyril sends | consequence | issue | class |
|---|---|---|---|---|---|---|
| `thinking` | **unknown-key** (schema row removed 0.66.43; LIVE leg 0: `configuration.intake.unknown-key setting/thinking` on 0.66.91 AND 0.66.43) | yes (TUI still sends it; same log line) | SENT (`:77`, default-on `:94`) | `chat.enableThinking:false` is ignored at session start; one log line per connection; `/thinking on|off` still works (Matrix 6) | cyril-53qx | DAY-ONE-GAP P2 (standing) |
| `todoList` | 0 hits / unknown-key | no | SENT when `chat.enableTodoList` is boolean (`:80`) | dead key | cyril-53qx | FOLLOW-UP P2 |
| `subagentOrchestration` | 0 hits / unknown-key (tool removed 0.66.33) | no | SENT ALWAYS (`:95`); doc comment `:89-90` still names `orchestrate_subagent` | dead key every handshake | cyril-53qx | FOLLOW-UP P2 |
| **`sessionRecap`** (NEW TUI 3.1.0) | declared extra key (list unchanged since 0.66.43); `_h("sessionRecap")&&startRecap()` at turn end → `kind:"recap"` (tui T2/T3; kas-core) | `{enabled:<chat.enableSessionRecap>}`, default false, always present | NOT-SENT | no recap (same as Kiro's default) | tui NEW P3 | FOLLOW-UP P3 |
| **`memory{mode,reflection}`**, `memoryEnable`, **`userMemoryOptIn`** | rollout `memory_internal_enabled:"all"` since 0.66.58 → policy = agent-file `memory.mode` unless `userMemoryOptIn:false` (kas-core); `userMemoryOptIn` is a visible catalog row (quoted: `userMemoryOptIn:Lce("userMemoryOptIn",{hidden:!1})`) | yes (`PI(e,i)`; `userMemoryOptIn` iff `!Vn("memory_controls")\|\|disabled`) | NOT-SENT (`BOOL_MAP` has no memory row) | **changed meaning since 2.32.0:** a user agent file with `memory.mode:"read_write"` now runs KAS native memory (hidden extraction call per turn, cyril-s21o) under cyril with no opt-out sent | cyril-q8gq, cyril-0na7, cyril-s21o | DAY-ONE-GAP P3 (needs the user's agent file) |
| `workflows{enabled}`, `goal{enabled}` | consumed; `creatorEffort?` accepted | always sent (rollout `available` check gone in 3.1.0, tui lane) | SENT from `chat.enableWorkflows` (`:143-145`) | parity (ADR-0013) | cyril-xehm | OK |
| `workflowNotifications` | 0 hits (dead since ≤2.26.0, kas-workflow) | sent | NOT-SENT (correct) | none | — | OK |
| `largeToolOutputHandler` | unknown-key (LIVE leg 0) | sent | NOT-SENT (correct) | none | — | OK |
| `c2s` | hidden catalog item; gates `_kiro/codeIntelligence/{startBuild,cancelBuild,indexStatus}`, diagrams | `{enabled}` only under rollout `c2s` | NOT-SENT | dark | cyril-1aq0 | OK |
| `infraSafetyMonitor`/`infraSafetyEnforce` | consumed | env-gated | NOT-SENT | dark | cyril-3ald | OK |
| **`compaction.autoCompactionThreshold`** (0.66.58) | 0..80, default 80; initialize result advertises `{min,max,default}` (LIVE leg 0: first in 2.33.1) | not sent (TUI sends `excludePercent`/`excludeMessages` only) | NOT-SENT (`:159-172`) | KAS default 80 (= TUI) | cyril-6s21 | OK |
| **`sessionTools{awareness,messaging,cloud}`**, **`universalBackgroundTool`**, `sendMessage{deliveryLimit}` (0.66.91) | off by default (catalog `sessionToolsMessaging` off; jobs need flag `universal_background_tool`, kas-core addendum) | `universalBackgroundTool` via rollout `background_execution` (0%, host); `sessionTools` not sent | NOT-SENT | dark; a future flip would make `_kiro/session/notify sender:"peer"` reach cyril's adapter, which drops it (`convert/kas/notify.rs:73-77`) | cyril-53qx, cyril-fb1m | OK (dark) |
| `_suggestedPrompts`, `_sessionRecap`, `evalStrategy`, `evalTreatments`, `_c2s`, `_quickSpec`, `_parallelTasks`, `_steeringReminders`, … | hidden / A-B | no | NOT-SENT | none | — | OK |
| `toolSearch{enabled,minPct?,minTokens?}` / `neverDefer` | = 2.32.0 (`enabled` accepted, unconsumed; `neverDefer` consumed) | yes | SENT `:147-156` / NOT-SENT | toggle inert; no eager-MCP control | cyril-53qx, cyril-q8gq | FOLLOW-UP P3 |
| `knowledge{…}`, `codeIntelligence`, `checkpoint`, `tangentMode`, `disableAutoCompaction`, `_subagent`, `_delegate`, `compaction{excludePercent,excludeMessages}` | consumed | yes | SENT | = 2.32.0 | — | OK |
| `semanticReview` (default ON), `fta`/`validation` (default OFF) | consumed | not sent | NOT-SENT | KAS defaults (= TUI) | — | OK |

## Matrix 2 — Agent capabilities read (`initialize` result)

LIVE(live leg 0): 0.66.43 → 0.66.91 result keeps `protocolVersion`, `authMethods` [aws-builder-id, aws-iam-identity-center],
`agentCapabilities.{loadSession,mcpCapabilities,promptCapabilities,sessionCapabilities}`; `_meta.kiro.extensionMethods` 28 → 32
(+`_kiro/configuration/set` 2.34.0, +`_kiro/memory/getSyncStatus` 2.35.0, +`_kiro/memory/sync` and
`+_kiro/environment/get_network_configuration` 3.1.0), `+_meta.kiro.autoCompactionThreshold{min:0,max:80,default:80}` (2.33.1);
`diagramRenderFormats` / `codeIndexAnalysisScopes` only with `diagrams` / `_c2s` (kas-core).
cyril reads exactly one thing: "is `agentCapabilities._meta.kiro` an object" (`crates/cyril-core/src/protocol/fingerprint.rs:30-36`,
engine identity). Every key below has 0 non-test reads in `crates/` (grep `extensionMethods|authMethods|replayMarking|
sessionListScopes|sessionSources|sourceProviders|strictSessionLoad|executionTargets|modelFallbackReporting` → 0 outside tests).

| key | KAS 0.66.91 | TUI 3.1.0 reads | cyril | consequence | issue | class |
|---|---|---|---|---|---|---|
| `extensionMethods` (32) | gate for 10 TUI call families | yes — now also gates `_kiro/memory/*`, `_kiro/configuration/*`, `_kiro/modelFallback/*`, `_kiro/memory/getSyncStatus` (tui lane) | DROPPED | no feature detection; cyril calls `_kiro/workflow/*`, `_kiro/account/getUsage`, `_kiro/hooks/*`, `_kiro/mcp/*` unconditionally and reads the error | cyril-tikf | FOLLOW-UP P3 |
| `executionTargets` (`["local"]`) | cloud gating | yes | DROPPED | no cloud sessions (cyril sends no `executionTarget`) | cyril-tikf | OK |
| `sourceProviders` (`false`) | `--repo` | yes (+ Kiro host) | DROPPED | none until cloud | cyril-tikf | OK |
| `replayMarking` (true) | `_meta.kiro.replay` on replayed frames | yes (`source:"replay"`, T21) | DROPPED | replayed frames indistinguishable on `/load` | cyril-99ds | FOLLOW-UP P3 |
| `sessionSources`, `sessionListScopes`, `strictSessionLoad` | session browser | yes | DROPPED | no session browser | cyril-wigv, cyril-nn85 | OK |
| `modelFallbackReporting` (true) | announces `model_fallback`/`model_routed` kinds (now default-on fallback, kas-core) | 0 reads | DROPPED | none by itself; the kinds are Matrix 4b's problem | cyril-aoby | — |
| **`autoCompactionThreshold{min,max,default}`** (0.66.58) | advertised unconditionally | not read (TUI sends no threshold) | DROPPED | none (KAS default) | cyril-6s21 | OK |
| `diagramRenderFormats`, `codeIndexAnalysisScopes` | only with `diagrams` / `_c2s` | yes under `c2s` | NOT-RECEIVED (not advertised) | dark | cyril-68ny, cyril-1aq0 | OK |
| `authMethods` (+ 2.37.0 `AuthMethod::Terminal` on the wrapper path, host lane) | unchanged on the free path | n/a | DROPPED | wrapper-only sign-in method invisible | cyril-cxd2 | OK |
| `sessionCapabilities.{list,close,delete,fork}`, `loadSession`, `mcpCapabilities`, `promptCapabilities` | = | close/list/fork | DROPPED (SDK parse only) | Matrix 9 (`session/close` never sent) | cyril-wigv, cyril-p9g4 | FOLLOW-UP P4 |

## Matrix 3 — Client → agent methods

Classes: **cyril calls** / **cyril has no route** / **KAS does not implement**. Direction and TUI column from
`tmp/methods.json` (138 literals; `C2A` = KAS `case"_kiro/…"` dispatch). cyril's C2A calls: `_kiro/account/getUsage`
(`domain_mediator/commands/extensions.rs:262`), `_kiro/hooks/list` + `_kiro/hooks/setEnabled` (`kas/hooks.rs:25,29`,
`domain_mediator/commands/kas.rs:56-95`, kas-hooks mode), `_kiro/mcp/{resetServer,getPrompt,getResource}`
(`domain_mediator/commands/mcp.rs:34,37,42`), `_kiro/workflow/{listRecipes,list,inspect,cancel,resume,retry,load,invoke,new}`
(`domain_mediator/commands/kas.rs:308-350`) = **15 of the 65 C2A literals**. The TUI 3.1.0 `extMethod` set (tui lane
`sets-3.1.0.ext_method.txt`, 20 entries + the typed-route literals of tui T6/T7/T9/T17/T18/T19) is the parity bar.

| method | KAS 0.66.91 | TUI 3.1.0 | cyril @HEAD | consequence | issue | class |
|---|---|---|---|---|---|---|
| `initialize`, `session/new`, `session/load`, `session/prompt`, `session/cancel` | — | always | cyril calls (`domain_mediator/commands/session.rs:126,184,282,475`) | `session/new` carries `_meta.kiro.settings` (:61-77); `session/prompt` carries no `_meta` (:510-518) → no `displayText`/`outputStyle` | cyril-j66p, cyril-hzkb | OK / FOLLOW-UP P2 (M6) |
| `session/set_config_option` | ids: `mode, model, memoryReflection, autopilot, contentCollection` (+`workflowLaunch` gate-on); `effortLevel`/`thinking` accepted silently, not echoed as options (LIVE leg 1, both engines) | sets `autopilot:"on"` right after `session/new`, then model, effort, thinking, mode, sandbox ×3, `workflowLaunch`, `memoryReflection` (quoted offsets 11646061, 11016794, 11017015, 11494463, 11524976, 11490308) | cyril calls for `model`/`mode`/`effortLevel` (`commands/mod.rs:528-534`, `types/kas_config.rs:20-48`, mediator `domain_mediator/commands/session.rs:388`) and `thinking` (`extensions.rs:159`); never `autopilot`, `sandbox*`, `workflowLaunch`, `memoryReflection` | Matrix 6 | cyril-vh69 (closed), kas-workflow NEW P2/P3 | OK / FOLLOW-UP |
| `session/set_mode` | accepted (TUI pairs it with `set_config_option mode`) | yes | cyril has `set_mode` (`domain_mediator/commands/session.rs:304`); `/mode` uses the config option | — | — | OK |
| `session/set_model` | **-32603 on both engines** (LIVE leg 1) | not called on KAS | `set_model` exists for v2 (`domain_mediator/commands/session.rs:342`); not used on KAS | — | — | OK |
| `session/fork` (`/rewind`, `createdReason:"rewind"`) | implemented | yes | **no route** | no `/rewind` | cyril-1aq0 | FOLLOW-UP P3 |
| `session/list`, `session/close`, `session/delete`, `_kiro/session/{delete,rename,history}` | implemented | `/chat`, dashboard, `/sessions` | **no route** (`session/close` 0 hits in `crates/`) | no session browser; step sessions never closed after replay | cyril-wigv, cyril-a3th, cyril-p9g4 | FOLLOW-UP P3/P4 |
| `_kiro/session/context`, `_kiro/session/compact` | implemented | `/context`, `/compact` | **no route** | v2 `/context` `/compact` have no KAS twin | cyril-1aq0 | DAY-ONE-GAP P2 (standing) |
| `_kiro/account/getUsage` | implemented (enterprise overage columns 2.34.0 are TUI rendering of the same reply) | `/usage` | cyril calls (`extensions.rs:262`; panel `cyril-ui/src/widgets/usage_panel.rs:245-310`) | session total is cyril's own sum (Matrix 4b `session_usage`) | — | OK |
| `_kiro/hooks/list`, `_kiro/hooks/setEnabled` | implemented | `/hooks` (list only) | cyril calls in kas-hooks mode (`kas/hooks.rs:25,29`) | — | — | OK |
| `_kiro/mcp/resetServer`, `_kiro/mcp/getPrompt`, `_kiro/mcp/getResource` | implemented | `resetServer` (TUI 0 for the other two) | cyril calls (`commands/mcp.rs:34-42`) | `/mcp` panel actions; **closes the 2.32.0 `/mcp missing` row** | cyril-nk4o (closed) | OK |
| `_kiro/workflow/{listRecipes,list,inspect,cancel,resume,retry,load,invoke,new}` | implemented, handlers token-identical (kas-workflow) | 10 (adds `pause`) | cyril calls 9 (`kas.rs:308-350`) | no `/workflow pause` | cyril-4u4a | FOLLOW-UP P4 |
| `_kiro/workflow/pause` | implemented | yes | **no route** | cannot pause a run | cyril-4u4a | FOLLOW-UP P4 |
| `_kiro/workflow/{delete,update,resumeAll,listWatchHandlers}` | implemented | not called | no route | none vs TUI | — | OK |
| `_kiro/workflow/new` with `workflowPrompt` | model-tool only (kas-workflow: `new` takes `workflowPath\|workflow`) | `/workflow new` = canned prompt | not sent | ADR-0013 inline-recipe launch instead; `/workflow new` still unimplemented | cyril-0asq | FOLLOW-UP P2 |
| `_kiro/config/template` | implemented; the only source of the 5 output styles (`id:"outputStyle"`, `promptScoped:!0`, quoted) | `/settings display` | **no route** | no output-style picker | cyril-lyrx, cyril-j66p | FOLLOW-UP P2 |
| `_kiro/codeIntelligence` (+`summary` 2.33.0), `_kiro/codeIntelligence/{startBuild,cancelBuild}` (3.1.0, `_c2s`) | implemented / gated | `/code`, `/code init` | **no route** | no `/code` | cyril-1aq0 | FOLLOW-UP P3 |
| `_kiro/knowledge` | implemented | `/knowledge` | **no route** | no `/knowledge` | cyril-1aq0 | FOLLOW-UP P3 |
| `_kiro/memory/{list,get,delete}` (+`create,update`) | implemented; `list` now returns `total`, `counts{byType,byScope}` (LIVE leg 1) | `/memories` (+ delete 2.34.0) | **no route** (cyril `/memory` = cyril-memory store) | KAS native memories invisible even when the user's agent file enables them (1b) | cyril-0na7 | FOLLOW-UP P3 |
| `_kiro/memory/{getSyncStatus,sync}` (0.66.70/0.66.91) | `{state:"off"}` (LIVE leg 1) | `getSyncStatus` under rollout `memory_cloud_sync` (0%, host) | no route | dark | cyril-0na7, cyril-s21o | OK |
| `_kiro/configuration/getState` / **`_kiro/configuration/set {krn,value,layer}`** (0.66.62) | implemented; `set {}` → -32602 "krn: Required" (LIVE leg 1) | memory cloud sync only (3.1.0) | no route | dark for the TUI too | cyril-iq2c | OK |
| **`_kiro/modelFallback/{get,set}`** (TUI 2.33.0) | **KAS does not implement** (0 hits in six carves; only `modelFallbackReporting`) | `/model fallback` → refused "This agent does not support configuring a model fallback" (tui T6) | no route | parity with the TUI is "nothing"; the chains ARE visible to a `configurationState` client (LIVE leg 1) | cyril-aoby, cyril-lnxg | OK |
| `_kiro/permissions/list` (+ consumed `_kiro/policy/changed`) | implemented | `/permissions` (2.34.0) | **no route**; `_kiro/policy/changed` dropped (`convert/kiro.rs:1305`) | no "what did Always-allow persist" view — the natural follow-up to the auxt echo | cyril-r5pl | FOLLOW-UP P3 |
| `_kiro/diagram/render` | gated on `diagrams` (`DiagramsNotEnabledError` -32602 without it, LIVE leg 1) | `c2s` only | no route | dark | cyril-68ny | OK |
| `_kiro/environment/get_network_configuration` (3.1.0) | -32000 `NetworkConfigurationUnavailableError` without a remote-sessions endpoint (LIVE leg 1) | not called | no route | cloud only | — | OK |
| `_kiro/job/{output,cancel}` (3.1.0) | advertised only with flag `universal_background_tool` (off); -32603 "no persistence classification" when off (LIVE leg 1) | not called | no route | dark | cyril-7p7n | OK |
| `_kiro/sourceProviders/{list,listResources}`, `_kiro/spec/{invoke,resolveSession}`, `_kiro/sandbox/applyConfig`, `_kiro/powers/refresh`, `_kiro/policy/ignore_files_changed` | implemented | yes | no route | no cloud/spec/sandbox UI; powers arrive by push instead (`convert/kas/powers.rs:48`); ignore-file changes not relayed | cyril-tikf, cyril-6vo6, cyril-q159 | FOLLOW-UP P3/P4 |
| `_kiro/checkpoint/{revert,revertMultiple}`, `_kiro/permissions/explain`, `_kiro/policy/check`, `_kiro/session/export`, `_kiro/session/list`, `_kiro/spec/getTaskStatuses`, `_kiro/powers/list`, `_kiro/mcp/toggle`, `_kiro/hooks/triggerHook`, `_kiro/safety/getProperties`, `_kiro/configuration/contribute`, `_kiro/memory/{create,update}`, `_kiro/environment/start_ssh_session` | implemented | not called | no route | none vs TUI | cyril-1aq0 (rewind) | OK |
| `_session/steer`, `_session/steer/clear` (`/steer`) | **implemented** (persistence class `metadata`: `case"_session/steer":case"_session/steer/clear":…return"metadata"`, quoted) | not called | cyril calls (`domain_mediator/commands/subagents.rs:99-117`) | `/steer` works on KAS | — | OK |
| `_session/spawn`, `kiro.dev/session/terminate`, `_message/send` (`/spawn` `/kill` `/msg`) | **0 hits** (v2-only) | not called | cyril calls on every engine (`commands/mod.rs:568-571`, `subagents.rs:30,60,74`) → BridgeError | three commands error on KAS; `/sessions` lists the v2 crew tracker (empty) | cyril-a8se | FOLLOW-UP P3 |
| `_kiro.dev/commands/{execute,options}` | v2-only | — | KAS advertisements never produce this route (`convert/kas.rs:493-548` typed `Prompt`; `drop_v2_command_list` `engine.rs:404-407`) | — | cyril-oieu (closed) | OK |

## Matrix 4 — Agent → client: `sessionUpdate` variants, `session_info_update` kinds, `_kiro/*` notifications

### 4a. `sessionUpdate` variants (KAS literal set 8 = 8, kas-core; cyril `convert/mod.rs:496-578`, KAS override `engine.rs:353-368`)

| variant | KAS 0.66.91 | TUI 3.1.0 | cyril @HEAD | consequence | issue | class |
|---|---|---|---|---|---|---|
| `agent_message_chunk` / `user_message_chunk` | = ; sub-agent chunks tagged `_meta.kiro.agentSubtaskId`; user chunks may carry `_meta.kiro.{replay, agentInitiated, agentInitiatedReason, notification{bufferOnly,callerSessionId}, syntheticUserMessageReason}` (kas-core; LIVE 2.32.0 leg 2a) | routes `agentSubtaskId` chunks to a sub-agent lane; tags `replay` as source `"replay"` (T21) | HANDLED (`convert/mod.rs:496-515`) with **0 `_meta` reads**: `agentSubtaskId` 0 hits in `crates/`; `replay`/`agentInitiated`/`bufferOnly` DROPPED | sub-agent text interleaves with the main agent's; replayed engine-synthesized user rows render as typed | cyril-ebqu, cyril-99ds, cyril-58uv | DAY-ONE-GAP P2 (ebqu) / FOLLOW-UP P3 |
| `agent_thought_chunk` | not in the 0.66.91 `sessionUpdate:"…"` literal set (kas-core: 8 variants, none named thought) | — | HANDLED if ever sent (`:516`) | none | — | OK |
| `tool_call` / `tool_call_update` | = ; `_meta.kiro.preview{…}` on `fs_write` **removed in 0.66.91** (LIVE leg 2a: only `checkpoint{original,modified,local}` + ACP `content[].diff` remain); new `_meta.kiro.{trustClassifierApproved, trustClassifier{decision,reason}}` (dark), `policyDenial` (0.66.62 unparseable commands) | renders `trustClassifier*`, `replay`, `cloudState` | HANDLED generically (`to_tool_call` `:184`, `to_tool_call_update` `:235`; no `_meta` read); approval diff from ACP `content[].diff` (`:207-210`, `cyril-ui/src/traits.rs:539`) → **the `preview` removal is not a cyril regression** (LIVE leg 2a); `policyDenial`/classifier meta DROPPED | child tool calls render as the main agent's (ebqu); a policy-denied call shows as failed with no rule name | cyril-ebqu, cyril-58uv | DAY-ONE-GAP P2 (ebqu) / FOLLOW-UP P3 |
| orphan `tool_call_update` ("Tool Load", "Subagent Response", 3.1.0 turn-limit text) | = (+ "Sub-agent '<id>' stopped at its turn limit…" text, kas-core) | rendered | **HANDLED (FIXED since 2.32.0):** `cyril-ui/src/state.rs:555-560` tracks the first update as a row | the sub-agent's answer row appears | cyril-1v25 (closed) | OK |
| `available_commands_update` | `_meta.kiro.type` ∈ skill / prompt / custom-agent (steering commands dropped since 0.66.33) | split by type | HANDLED: typed `AgentCommandRoute::Prompt` (`convert/kas.rs:493-548`), unknown/missing types suppressed with a log | — | cyril-oieu (closed) | OK |
| `config_option_update` | model option `_meta.kiro{hasEffort,effortLevels,defaultEffortLevel,effortSchemaPath,rateMultiplier,rateUnit,thinkingToggleable}` — same 7 keys OLD/NEW (LIVE leg 1); `workflowLaunch` select appears gate-on (kas-workflow) | model picker sort/credit column; `workflowLaunch` restore | HANDLED wholesale (`cyril-core/src/session.rs:376-389` `KasConfigSnapshot`; toolbar `widgets/toolbar.rs:54-82`); `rateMultiplier`/`rateUnit` DROPPED; `workflowLaunch` kept as a generic select (`convert/mod.rs:109-130`) but not settable (`types/kas_config.rs:20-48`) | no credit column; no Workflow-use control | cyril-lxuo, kas-workflow NEW | FOLLOW-UP P4 / P3 |
| `current_mode_update` | = | ignored | HANDLED (`:551`) | — | — | OK |
| `session_info_update` | 29 kinds | `Ofe` dispatcher + `Tfe` silenced set | 9/29 (4b) | — | cyril-58uv | — |
| `plan`, `usage_update` (schema-only) | no KAS emitter | not mapped | HANDLED if sent (`:529,:578`) | none | — | OK |

### 4b. `session_info_update._meta.kiro.kind` — 29 kinds (`tmp/kinds.json`; counts KAS 0.66.91 / 0.66.43 / TUI 3.1.0)

cyril dispatch `crates/cyril-core/src/protocol/convert/kas.rs:625-822` (`session_info_to_notification`); everything not matched
falls to `_ => None` at **`kas.rs:822` with no log** (cyril-58uv). TUI column = the 3.1.0 `kind===` consumer set
(`sets-3.1.0.kind_eq.txt`) plus zod schemas; "0" = the literal does not occur in the bundle at all (quoted or not).

| kind | KAS 91/43 | TUI 3.1.0 renders? | cyril @HEAD | consequence for a cyril user | issue | class |
|---|---|---|---|---|---|---|
| `turn_start` | 30/23 (+`executionId,turnId,timestamp,boundarySource,userMessageTag`, LIVE leg 1) | yes (14) | HANDLED (`kas.rs:694`); new identity fields DROPPED | none | — | OK |
| `turn_end` | 19/17 | yes (11) | HANDLED (`kas.rs:695`; `turn_end_stop_reason` `:1014-1032` defaults EndTurn with a warn) | `error`/`content_filtered` still read as EndTurn | cyril-pyko, cyril-pnwb | FOLLOW-UP P3 |
| `turn_completion` | 6/6 | yes (3) | HANDLED (`kas.rs:687`) → `TurnMeteringUpdated` → `cyril-core/src/session.rs:369,450` | the only credit source cyril has (see `session_usage`) | — | OK |
| `context_usage` | 8/8 (+`summarizationThreshold` 80 at both levels, LIVE leg 1) | yes (8) | HANDLED (`kas.rs:698-760`); `summarizationThreshold` DROPPED | no "compaction at 80 %" marker on the context bar | cyril-6s21 | FOLLOW-UP P4 |
| `steering_queued` / `steering_injected` / `steering_cleared` | 4/2, 3/2, 4/2 | yes (8 / 1 / 7) | HANDLED (`kas.rs:761,769,800`) | — | — | OK |
| `hook_update` | 4/4 | **0** | HANDLED (`kas.rs:668`) | cyril shows more than Kiro here | — | OK |
| `display_error` | 10/10 (+throttled variants 0.66.58) | **0** (corrects the 2.32.0 "yes") | HANDLED only for `errorType:"mcp_connection_error"` (`kas.rs:628-651`); others DROPPED | throttling / model errors reach the user only via the prompt-response error | cyril-58uv | DAY-ONE-GAP P2 (standing) |
| **`session_usage`** (NEW 0.66.91) | 3/0; LIVE leg 1: `{credits:[{usage:<cumulative>,unit:"credit",unitPlural:"credits"}]}` after every `turn_completion`, replayed with `replay:true` at the end of `session/load` | **yes** (4; footer "Credits: turn X • session Y", `/usage`, `sessionCredits` segment; preferred over the local sum, tui T4) | **DROPPED** (`kas.rs:822`); cyril sums `turn_completion` itself (`cyril-core/src/session.rs:291,450`; `types/session.rs:626-663`) and resets on a new session (`cyril-core/src/session.rs:470`) | `/usage` "Credits" (`widgets/usage_panel.rs:207,219`) and the footer total restart at 0 after `/load` and can drift from Kiro's authoritative total | tui NEW P2 (fold rule P2, RUN-STATE) | **DAY-ONE-GAP P2** |
| **`model_fallback`** / **`model_routed`** | 3/3, 3/3 — **default-on in 0.66.91** (`model_fallback_{refusal,availability,auto_refusal}` false→true, kas-core) | yes (10) / 0 | DROPPED (`kas.rs:822`) | the model can change mid-session with no UI trace; toolbar model (`toolbar.rs:63-66`) stays on the picked id; a refused turn is re-sent on the fallback model unannounced | cyril-aoby (raise to P2, kas-core), cyril-ojum | **DAY-ONE-GAP P2** |
| `summarization_started` / `_completed` / `_failed` / `_separator` / `summary_message` | 3/4/4/3/4 | yes / yes / yes / 0 / 0 | DROPPED | auto-compaction invisible (v2 had `kiro.dev/compaction/status`) | cyril-58uv | DAY-ONE-GAP P2 (standing) |
| `recap` (3.1.0 opt-in via `sessionRecap`) | 6/6 | **yes** (1; primary session only, T3) | DROPPED | none until cyril sends `sessionRecap` (1b) | tui NEW P3 | FOLLOW-UP P3 |
| `pending_interaction` (+`_meta.kiro.workflowPlan`) | 13/12 | zod schema (1) | DROPPED (the plan itself is read from `request_permission`, Matrix 5) | a question/approval resolved elsewhere is not mirrored | cyril-99ds | FOLLOW-UP P3 |
| `interaction_resolved` (+`consentRound`, `replay` 2.34.0) | 17/15 | yes (7) | DROPPED | an approval answered by another client leaves cyril's dialog open | cyril-99ds, cyril-58uv | FOLLOW-UP P3 |
| `user_message_id_assigned` | 3/3 | yes (1; rewind anchor) | DROPPED | blocks `/rewind` | cyril-1aq0 | FOLLOW-UP P3 |
| `queued` | 52/39 | yes (18) | DROPPED (hits in `crates/` are test-only) | none vs TUI for a single client | cyril-58uv | OK |
| `focus_update` | 6/5 | **0** | DROPPED | none vs TUI | cyril-otpi | OK |
| `steering_inclusion` | 9/9 | 1 (schema) | DROPPED | none | cyril-58uv | OK |
| `checkpoint_restore_availability`, `repositories_update` | 2/2, 2/2 | 0 / via `repositories` key | DROPPED | none | cyril-fb1m, cyril-f4tt | OK |
| **`turn_timing`** (0.66.58, opt-in `automaticTurnProfiling`) | 3/0 | **0** | NOT-SENT (no `session/new._meta.kiro.automaticTurnProfiling`, `domain_mediator/commands/session.rs:61-77`) / DROPPED | none | — | OK |
| **`suggested_prompts`** (0.66.62, hidden) | 3/0 | **0** | DROPPED | none | — | OK |
| **`network_configuration`** (0.66.91, cloud) | 2/0 | **0** | DROPPED | none (no cloud sessions) | — | OK |

### 4c. `_kiro/*` agent → client notifications (43 A2C-N literals; kas-core's standing-coverage lists verbatim, TUI column added)

Routing: `engine.rs:375-408` tries `convert/kas/mcp.rs:118-132` (`kiro/mcp/status`, `kiro/mcp/governance_disabled`),
`convert/kas/workflow.rs:527-535` (9 lifecycle events), `convert/kas/powers.rs:48`, `convert/kas/notify.rs:33,60-77`
(`_kiro/session/notify`), then `convert/kiro.rs` (`:1075` rate_limit, `:1086` system/notify, **`:1305 _ => Ok(None)` silent**).
Hooks control frames are claimed earlier by `kas/hooks.rs:35-39`.

| notification | KAS 0.66.91 | TUI 3.1.0 | cyril @HEAD | consequence | issue | class |
|---|---|---|---|---|---|---|
| `_kiro/workflow/{run_start,node_start,node_complete,node_paused,loop_iteration,watch_poll,paused,run_complete,steps_queued}` | = sets; **+`credits[]` on `node_complete`/`run_complete`/snapshot nodes** (0.66.91, LIVE leg 2a gate-off), +`stepsCompleted/stepsTotal/currentNodeId/waitingOnWatch` on every event and list row (0.66.48, LIVE leg 2a), `attention[]` (0.66.43) | yes (step rows show credits, 3.1.0) | HANDLED events (`convert/kas/workflow.rs:527-535`); **`credits` DROPPED** (`WireRunCompleted` :207-218, `WireNodeCompleted` :259-283, `WireNodeSnapshot` :444-520 — kas-workflow); progress fields DROPPED (`WireRunListEntry` :588-618); `attention[]` DROPPED | `/workflow status` shows no per-step credits; run totals come from parent metering (`workflow.rs:487,1319`; `turn_labels.rs:108-121`) | kas-workflow NEW P2, cyril-4u4a | **DAY-ONE-GAP P2** (every cyril-launched run) |
| `_kiro/workflow/recipes_changed`, `node_failed` | = (gated `workflowsEnabled`) | yes | DROPPED | recipe list stale (gate-on only) | cyril-4u4a | FOLLOW-UP P4 |
| `_kiro/session/notify` (`sender` parent/step/**peer** 0.66.91; +`notifyId` LIVE leg 2a, `deliveryCount`) | = + peer | 0 | HANDLED for parent/step (`convert/kas/notify.rs:33`); **`peer` → whole message dropped with a warn** (`notify.rs:73-77`); `notifyId`/`deliveryCount` DROPPED | unreachable until `sessionTools.messaging` is sent | cyril-fb1m | OK (dark) |
| `_kiro/mcp/status`, `_kiro/mcp/governance_disabled` | = (+`kind:mcp_unavailable` 0.66.70) | yes | HANDLED (`convert/kas/mcp.rs:118-132`; `/mcp` panel) — **FIXED since 2.32.0** | — | cyril-nk4o (closed) | OK |
| **`_kiro/mcp/oauthCompleted {serverName,success,oauthFlowId}`** (KAS 0.66.91; TUI consumer since 2.32.0) | sent when the client advertised `openExternalUrl` (kas-core) | yes (clears the per-server sign-in notice, T31) | **REACHABLE and DROPPED** (`convert/kiro.rs:1305`; cyril advertises `openExternalUrl`, `engine.rs:143`) | the OAuth notice is cleared only when `_kiro/mcp/status` flips the server to connected | kas-core NEW P4 | FOLLOW-UP P4 |
| `_kiro/governance/state` | = (+ drives `mcpEnabled`/web tools startup 0.66.70) | yes | DROPPED | org feature flags invisible | cyril-0o7e | FOLLOW-UP P2 |
| `_kiro/customAgent/{not_found,config_error}` | = | not_found yes | DROPPED | silent fallback agent | cyril-7xxk (filed since 2.32.0) | FOLLOW-UP P3 |
| `_kiro/policy/{changed,error}` | = (`changed` now also triggers the TUI's `_kiro/permissions/list` refresh) | `changed` yes | DROPPED | policy rebuild failures / consent warnings unlogged | cyril-r5pl | FOLLOW-UP P3 |
| `_kiro/error/rate_limit`, `_kiro/system/notify`, `_kiro/powers/items_changed`, `_kiro/hooks/{cancel,didChange}` | = | yes / 0 / yes / 0 | HANDLED (`convert/kiro.rs:1075,1086`, `convert/kas/powers.rs:48`, `kas/hooks.rs:35,39`) | — | — | OK |
| `_kiro/tools/didChange`, `_kiro/tools/content_chunk` | = | yes | DROPPED / never sent (no `streamingShellContent`) | no `/tools` panel data; no live shell output | cyril-1aq0, cyril-2mo0 | FOLLOW-UP P3 |
| `_kiro/sandbox/status`, `_kiro/sessions/changed`, `_kiro/spec/{phaseCheckpoint,taskStatusChanged}`, `_kiro/steering/documents_changed` | = | yes | DROPPED | no sandbox/dashboard/spec UI | cyril-6vo6, cyril-mxc7, cyril-as1l | OK |
| **`_kiro/memory/syncStatus {state}`** (0.66.70; pushed at connect and on `session/load`, LIVE leg 1), **`_kiro/memory/changed`** (0.66.91) | sent unconditionally / only while memory is on | `syncStatus` via `/memories` sync | DROPPED | none | cyril-s21o | OK |
| **`_kiro/codeIntelligence/indexStatus`**, **`_kiro/job/update`** (0.66.91) | gated `_c2s` / flag off | yes / 0 | DROPPED | dark | cyril-1aq0, cyril-7p7n | OK |
| `_kiro/configuration/state` | only with `configurationState:true` (3 pushes at init on 0.66.91, LIVE leg 0) | 0 | NOT-REQUESTED | — | cyril-iq2c | OK |
| `_kiro/c2s/view/changed`, `_kiro/code_references`, `_kiro/knowledge/indexing{Started,Completed}`, `_kiro/progressive_context/items_changed`, `_kiro/safety/{propertiesChanged,statusChanged}` | = | 0 | DROPPED | none vs TUI | cyril-45ld, cyril-3ald | OK |

## Matrix 5 — Host callbacks KAS makes into the client

Responders: typed `HostCallback` kinds `crates/cyril-core/src/protocol/kas/callbacks.rs:365-381` (served list `:595-615`);
`_`-requests cyril does not know are answered JSON `null` (`domain_mediator/host.rs:68,248`). Gate words are KAS 0.66.91's.

| callback | KAS 0.66.91 gate | TUI 3.1.0 | cyril @HEAD | consequence | issue | class |
|---|---|---|---|---|---|---|
| `_kiro/auth/getAccessToken {forceRefresh?}` | `--auth=acp-callback` (unconditional; 1× at init, LIVE leg 0) | `["chat","_","get-kas-token"(,"--force-refresh")]` → `{accessToken,expiresAt,profileArn,authMethod?,provider?}` (byte-identical 2.32.0→3.1.0, tui + host lanes) | HANDLED — same argv (`kas/auth.rs:44-46,559-563`), envelope `getKasToken`/`error` (`:859-860`), fields relayed `:895-914` (`authMethod`/`provider` only when present), 30 s deadline (`:49`), cross-process single-flight (`:449-460`), stored-token fallback only when the helper is missing (`:616-647`) | **FIXED since 2.32.0** (cyril-zbu7). 0.66.91 logs `cps.auth_provider.unknown {provider:"none"}` ×2 when the reply has no `provider` (LIVE leg 0) — expect the same line under cyril | cyril-zbu7 (closed), cyril-0rn8 | OK (regression table) |
| `_kiro/terminal/shell_type {}` → `{shellType}` | `terminal:true`; reply classified by `xp()`: `bash\|logbash\|gitbash\|wsl\|zsh\|logzsh\|sh\|dash\|ksh\|ash\|mksh\|pdksh\|oksh\|loksh\|yash\|busybox\|tcsh\|csh → "posix"`, `fish`, `powershell\|pwsh\|pwsh-preview → "powershell"`, `cmd`, else `"unknown"` (quoted @2035112; warn `policy.shell.type.unknown` @2036203); `j4()` then takes the basename (`case"posix"` 1 = 1, @13513415) | not called (TUI hosts no terminals) | **HANDLED, FIXED since 2.32.0:** `terminal_io.rs:283-285` → `host_shell.rs:216-222` `wire_name()` = `bash`/`zsh`/`sh`/`dash`/`ksh` (`PosixShell::name` :29-35), `fish`, `pwsh`, `powershell` — every value is in `xp()`'s list | shell policy sees the real dialect; session-scope Always-allow and V3 rules apply | cyril-k8wc (closed) | OK (regression table) |
| `terminal/{create,output,wait_for_exit,release,kill}` | `terminal:true` | not called | HANDLED (`kas/terminal_io.rs`; stdin null `:213`) | 3.1.0 "stdin EOF at once" is the agent-hosted path; cyril already nulls stdin | — | OK |
| `_kiro/terminal/write` | `terminalInput` (not advertised) | not called | NOT-REACHED | model told stdin is unsupported (1a) | cyril-7p7n | FOLLOW-UP P3 |
| `fs/read_text_file`, `fs/write_text_file`, `_kiro/fs/{read_file,write_file,stat,read_directory,delete}` | `fs.*` + `fs._meta.kiro.*` | not called | HANDLED (`kas/kiro_fs.rs:85-89`, `kas/host_io.rs`) | = 2.32.0 | — | OK |
| `_kiro/hooks/list {trigger,toolId?,toolTags?}`, `_kiro/hooks/executeHook`, `_kiro/hooks/sessionStart` | `hooks.enabled` without `v2`; trigger enum unchanged (sessionStart, preToolUse, postToolUse, promptSubmit, agentStop, pre/postTaskExecution) — **no compaction trigger reaches the wire** (kas-core) | TUI sends `v2:!0` so KAS runs hooks itself | HANDLED in host mode (`kas/hooks.rs:25-42`, trigger map `:636-645` = PascalCase file triggers → camelCase wire; reply `{output,exitCode,cancelled}` `:383-391`; exit 2 blocks) | `PreCompact`/`PostCompact` (3.1.0) never fire for host-mode cyril users; they fire only under `kas_hooks="kas"` | cyril-497j | FOLLOW-UP P4 |
| `_kiro/hooks/cancel`, `_kiro/hooks/didChange` (control notifications) | hooks | — | HANDLED (`kas/hooks.rs:35-39`) | — | — | OK |
| **`_kiro/openExternalUrl {url, serverName, oauthCompletion, oauthFlowId?}`** | `resolvedCapabilities.openExternalUrl` (quoted @15779523); 0.66.91 adds the three keys and pairs the call with `_kiro/mcp/oauthCompleted` | reads `serverName`/`oauthCompletion` (per-server notice) | HANDLED for `url` only (`kas/callbacks.rs:190-193`); `serverName`, `oauthCompletion`, `oauthFlowId` DROPPED | the browser opens; the notice cannot be attributed to a server or cleared by the completion event | kas-core NEW P4 | FOLLOW-UP P4 |
| **`_kiro/mcp/elicitation {sessionId,toolCallId,elicitation{mode:form\|url,…}}`** | unconditional when an MCP tool elicits (`mcpElicitation` 0 hits; failure → `{action:"cancel"}`) | handled: form submit / open-link (T8) | HANDLED with a **visible decline** (`kas/mcp.rs:50-55` notice, `:70-82` `{action:"decline"}`; `callbacks.rs:367,597`) | an MCP tool that elicits gets "declined" and the user sees why | tui NEW P3 (interactive) | FOLLOW-UP P3 |
| `_kiro/userInput {sessionId,toolCallId,question,options}` | `userInputCapability` (quoted @15192582) | handled | NOT-REACHED (flag not sent); the legacy `request_permission` fallback is what cyril sees | option-less questions degrade (cyril-qo13 fixed the first-option drop) | cyril-qo13 (closed) | FOLLOW-UP P3 |
| `_kiro/secret/{get,store,delete}` | `secretStorage` | handled (`.secrets` file) | NOT-REACHED (KAS uses its own store) | none | — | OK |
| `_kiro/workspace/{active_file,currently_open_files}` | `extMethod` from `createActiveFileResolver` (quoted @15582599); consumer under investigation | **0 hits** (TUI does not answer either) | answered `null` (`host.rs:68,248`) if called | none vs TUI | cyril-4cz1 | OK |
| `_kiro/tool/{get_diagnostics,semantic_rename,smart_relocate}`, `_kiro/search/{find_files,text_search}` | `clientTool*` / `findFiles`,`textSearch` (not advertised) | not advertised | NOT-REACHED | none | — | OK |
| `session/request_permission` (request) | `_meta.kiro{toolId,command,consent{capability,resource,askType,triggeringResource,workspaceRoot,…},consentRound,workflowPlan?,trustClassifier?}`; same 4 options in the same order OLD/NEW (LIVE leg 1) | renders tiers/order client-side (T12/T13) | HANDLED: consent read `convert/kas.rs:77-113`; **`workflowPlan` read `kas.rs:28-33` + `convert/kas/workflow_plan.rs` (FIXED since 2.32.0, cyril-8wh0)**; shell command `kas.rs:36-72`; `trustClassifier` DROPPED | plan approvals render; classifier flag line (dark) does not | cyril-8wh0 (closed) | OK |
| `session/request_permission` **response `_meta.kiro`** | intake `scope:v?.scope\|\|"session"`, `resource:v?.resource??triggeringResource??resource`, `workspaceRoot` (quoted @14303817, unchanged); `rejectionReason`; `editedCommand`; unparseable commands carry `persistableConsent:false` (0.66.62) | `Kk`: `{consent{capability?,scope,resource?,workspaceRoot?},rejectionReason?}`; scope default `session` for allow_always / resource-bearing reject_always (3.1.0) | **FIXED since 2.32.0 (cyril-auxt):** `convert/mod.rs:456` → `attach_consent_echo` `convert/kas.rs:128-190` echoes `capability` (verbatim), `scope` (`always_allow.rs:33-47` session/workspace/user, cycled with `s`), `resource` (`:64-96` exact / `<cmd> *` pattern / folder glob / whole tool), `workspaceRoot` (verbatim); `rejectionReason` `kas.rs:195-224`; no agent-JSON write on KAS (`crates/cyril/src/app.rs:2027-2040`, v2 only) | user/workspace grants persist in KAS's own permissions files; the 2.36.0 subcommand tier (`<cmd> <sub> *`) is not offered (`always_allow.rs:163-180`) | cyril-auxt (closed), cyril-7c0s, tui NEW P4 (tier) | OK (regression table) / FOLLOW-UP P4 |

## Matrix 6 — configOptions / modes / models / effort / outputStyle / `workflowLaunch` / `memoryReflection` / `autopilot` / sandbox

KAS offers (LIVE leg 1, OLD = NEW): `session/new` configOptions = `mode` (7) · `model` (auto + 20, `_meta.kiro` 7 keys) ·
`memoryReflection` (off) · `autopilot` (on) · `contentCollection` (disabled); +`workflowLaunch` when `settings.workflows.enabled`
(kas-workflow). `outputStyle` is a `_kiro/config/template` select with `promptScoped:!0` (quoted @9006926), chosen per prompt via
`session/prompt._meta.kiro.outputStyle`. `effortLevel` and `thinking` are NOT echoed as session options on either engine; their
`set_config_option` is accepted silently (LIVE leg 1). cyril: `KasConfigId {Model, Mode, Effort}` (`types/kas_config.rs:20-48`),
pickers registered on KAS only (`commands/mod.rs:528-534`), one `SetConfigOption` per confirm (mediator `domain_mediator/commands/session.rs:388`),
post-ack effort chain (`kas_config.rs:207-234`).

| option | KAS 0.66.91 | TUI 3.1.0 sets | cyril @HEAD | consequence | issue | class |
|---|---|---|---|---|---|---|
| `model` | select, 21 values; same `_meta.kiro` key set as 0.66.43 | `configId:"model"` after `session/new` (default model) | HANDLED: `/model` picker, offered-only values; `rateMultiplier`/`rateUnit` DROPPED | no credit column; **a default-on fallback swap is not reflected** (4b) | cyril-vh69 (closed), cyril-lxuo, cyril-aoby | OK / DAY-ONE-GAP P2 (fallback) |
| `effortLevel` | accepted, not echoed as an option | `configId:"effortLevel"` (quoted @11016794) | HANDLED: `/effort` + toolbar badge (`toolbar.rs:72-75`); levels from the model's `_meta.kiro.effortLevels` | — ; the chain must not wait for an echo (lead V4) | cyril-vh69, cyril-m8kq | OK |
| `mode` | 7 bundled + custom agents | `set_mode` + `configId:"mode"` (quoted @11494463) | HANDLED: `/mode` picker | no `/agent` create/edit, no `/plan` alias | cyril-vh69 | OK |
| `thinking` | accepted silently; AgentSettings `thinking` is an unknown key | `configId:"thinking"` on/off (quoted @11017015) | HANDLED: `/thinking on\|off` (`commands/builtin.rs:549-600` → `SetThinking` → `extensions.rs:159`) | startup `chat.enableThinking` no longer applies (1b) | cyril-53qx | FOLLOW-UP P2 |
| **`autopilot`** | `on` by default | **TUI forces `configId:"autopilot",value:"on"` right after every `session/new`** (quoted @11646061) | DROPPED / NOT-SETTABLE (`KasConfigId::from_wire` → None) | same default as the TUI; cannot turn autopilot off | fold into a vh69 follow-up | FOLLOW-UP P3 |
| **`workflowLaunch`** (0.66.58; gate-on) | select `when-asked \| large-tasks \| delegate-all`, default `when-asked`; a pre-0.66.58 session loads as `legacy` = **delegate-all** (kas-workflow) | `configId:"workflowLaunch"` from `chat.workflowLaunch` after `session/new`; `/settings` "Workflow use" | DROPPED on receipt (generic select kept, not settable); NOT-SENT; `kas/settings.rs:108-116` reads only `chat.enableWorkflows` | with `chat.enableWorkflows=true` the user's Kiro "Workflow use" choice is ignored (engine default `when-asked`) and a loaded pre-0.66.58 session delegates everything | kas-workflow NEW P2 / tui NEW P3 → this lane P3 (trap 29: needs the gate) | FOLLOW-UP P3 (DAY-ONE for gate-on users) |
| `memoryReflection` | off | `/memories` toggle | DROPPED | none (cyril-memory path) | cyril-0na7 | OK |
| `contentCollection` | disabled | governance | DROPPED | none | — | OK |
| `outputStyle` | `_kiro/config/template` only; 5 styles; per-prompt `_meta.kiro.outputStyle` | `/settings display` → prompt meta | NOT-SENT (prompt carries no `_meta`, `domain_mediator/commands/session.rs:514-521`; `_kiro/config/template` never called) | default style only | cyril-j66p (open P2), cyril-lyrx | FOLLOW-UP P2 |
| `sandbox`, `mcpSandboxing`, `sandboxNetworkMode` | local sandbox (GA in the TUI since 3.1.0 gate removal) | `/sandbox` → three `set_config_option`s (quoted @11524976) | NOT-SENT | no sandbox | cyril-6vo6 | FOLLOW-UP P3 |
| cold-catalog model pin on `session/load` | load result carries the saved `model` option on both engines when the catalog is warm (LIVE leg 1, trap 33) | — | HANDLED (`domain_mediator/commands/session.rs:217-219` → `ConfigOptionsUpdated`) | none in practice | — | OK |

## Matrix 7 — Slash commands (TUI 3.1.0: 75 literals, 38 registered definitions; +`/permissions` vs 2.32.0)

Classes: **L** = TUI-local, **K** = KAS-method-backed, **H** = host-backed (`chat _` IPC / launcher), **v2** = only through v2
`kiro.dev/commands/*`, **id** = enum-only telemetry id (no handler). The 3.1.0 definition changes are tui lane "Slash commands"
(`/permissions` new; `/model` +`edit`, `/model fallback` dispatch; `/code` +`summary` and `startBuild`; `/memories` +delete/+sync;
`/usage` +Session credits; `/settings` Features +Workflow use, +Session recap; `feature:` gates removed from 11 commands).
cyril builtins on KAS (`commands/mod.rs:511-571`): `help clear quit new load steer voice usage memory theme thinking sessions spawn
kill msg powers` + `mcp` (:514-516) + `model effort mode` (:528-534) + `workflow review` (:536-540) + `answer` (:543-544) [+ `hooks`
under `kas_hooks="kas"`, :518-520] + every KAS-advertised skill / prompt / custom agent as a prompt-routed agent command.
All 75 literals are placed below (counted).

| TUI command(s) | class / KAS route | cyril on KAS @HEAD | change vs 2.32.0 | gap / issue | class |
|---|---|---|---|---|---|
| skills / saved prompts / custom agents (`/name args`) | K: prompt text | HANDLED (`convert/kas.rs:493-548`; `commands/mod.rs:654` submits the original text) | = (FIXED in 2.32.0 cycle) | — | OK |
| `/model` (+`set-current-as-default`, `edit` 2.34.0, `fallback` 2.33.0) | K `set_config_option model`; `edit` L (`chat.modelPicker.hiddenModels`); `fallback` K `_kiro/modelFallback/*` **unimplemented by KAS** | HANDLED `/model` picker; no `edit`/`fallback` | `edit`, `fallback` new | hidden-models list P4; fallback = nothing to build (3) | OK |
| `/effort` | K `set_config_option effortLevel` | HANDLED | = | — | OK |
| `/agent`, `/plan`, `/autonomous` | K `set_mode` + `set_config_option mode` (+ sandbox ×3 for autonomous; `/agent` also edits agent files, `/agent swap kiro_default` 3.1.0) | `/mode` covers the switch | gate removed on `/autonomous` | no agent create/edit; no `/plan` alias (P4) | OK |
| `/help`, `/quit`, `/exit`, `/clear` | L | builtins (`/clear` local; cyril `/new` = `session/new`) | = | — | OK |
| `/usage` | K `_kiro/account/getUsage` + **Session credits from `session_usage`** (3.1.0) | HANDLED builtin (`commands/builtin.rs:193-204`; panel `widgets/usage_panel.rs`); session total = local sum | TUI total now engine-fed and survives reopen | cyril's total resets on `/load` (4b) | DAY-ONE-GAP P2 |
| `/context` | K `_kiro/session/context` | **missing** | = | cyril-1aq0 | DAY-ONE-GAP P2 (standing) |
| `/compact` | K `_kiro/session/compact` | **missing** | = | cyril-1aq0 | DAY-ONE-GAP P2 (standing) |
| `/tools` | L panel fed by `_kiro/tools/didChange` | missing | = | cyril-1aq0 | FOLLOW-UP P3 |
| `/mcp` | L panel on `_kiro/mcp/status` + K `_kiro/mcp/resetServer` (+ 2.32.0 OAuth notice lifecycle; 3.1.0 `oauthCompleted` toast) | **HANDLED** builtin (`commands/mcp.rs:27-33`; `widgets/mcp_panel.rs`; mediator `commands/mcp.rs:34-42`) — **FIXED since 2.32.0** | — | `oauthCompleted` dropped (4c) | OK |
| `/knowledge`, `/code` (+`summary` 2.33.0; `init` → `startBuild` 3.1.0) | K `_kiro/knowledge`, `_kiro/codeIntelligence[/startBuild]` | missing | `summary`, `startBuild` new | cyril-1aq0 | FOLLOW-UP P3 |
| `/rewind` | K `session/fork{messageId,createdReason:"rewind"}` | missing | = | cyril-1aq0 | FOLLOW-UP P3 |
| `/prompts` | L list of advertised prompts | missing (prompts dispatch; autocomplete lists them) | = | — | FOLLOW-UP P4 |
| `/chat`, `/sessions` | K `session/list\|load\|delete`, `_kiro/session/rename`; dashboard GA (gate removed) | `/load <id>` only; cyril `/sessions` = v2 crew tracker (empty on KAS) | gate removed | cyril-wigv, cyril-a8se | FOLLOW-UP P3 |
| `/spawn` | id (TUI telemetry id only; no KAS route) | cyril builtin → `_session/spawn` (KAS 0 hits) → BridgeError; `/kill` `/msg` likewise (cyril-only names) | = | cyril-a8se | FOLLOW-UP P3 |
| `/hooks` | K `_kiro/hooks/list` | builtin in kas-hooks mode (`_kiro/hooks/list` + `setEnabled`); host mode = cyril's own registry panel (`widgets/hooks_panel.rs`) | = | — | OK |
| `/memories` (+delete 2.34.0, cloud-sync toggle 3.1.0 dark) | K `_kiro/memory/*`, `memoryReflection`, `_kiro/configuration/*` | cyril `/memory` = cyril-memory (different store) | delete, sync new | KAS native memories invisible even when the agent file enables them (1b) | cyril-0na7 | FOLLOW-UP P3 |
| **`/permissions`** (NEW 2.34.0; `policies` → ask / approve-for-me) | L rule write into `permissions.{yaml,json}` + K `_kiro/permissions/list` (+ `_kiro/policy/changed` refresh); Approve-for-me dark (`KIRO_TRUST_CLASSIFIER_MODE=enforce`) | missing | NEW | the KAS-native "what did Always-allow persist" view | cyril-r5pl, tui NEW P4 (dark) | FOLLOW-UP P3 |
| `/workflow`, `/workflows`, `/workflow-run`, `/workflow-resume`, `/workflow-status`, `/workflow-cancel`, `/goal` | K `_kiro/workflow/*` (10 methods); `/workflow run` plan preview; gates removed (GA) | builtin `/workflow recipes\|list\|run\|attach\|status\|cancel\|resume` (`commands/workflow.rs:14,114-143`) + `/review` (`:176-193`) + `/answer`; `/goal` missing; no `pause`, no `new` | gates removed | cyril-wu87/ct2q (`/goal`), cyril-0asq (`/workflow new`), cyril-4u4a (preview, pause) | FOLLOW-UP P3/P4 |
| `/settings` (Features: Workflow use → `workflowLaunch`, Session recap → `sessionRecap`; display → output style), `/config`, `/verbosity`, `/theme` | L (+ K `set_config_option workflowLaunch`, handshake `sessionRecap`, `_kiro/config/template`) | `/theme` builtin only | two Features rows new | `workflowLaunch` (M6), recap (1b), outputStyle (M6) | FOLLOW-UP P3/P2 |
| `/sandbox` | K `set_config_option sandbox\|mcpSandboxing\|sandboxNetworkMode` (GA) | missing | gate removed | cyril-6vo6 | FOLLOW-UP P3 |
| `/spec` | K `_kiro/spec/{resolveSession,invoke}` | missing | = | — | FOLLOW-UP P4 |
| `/tangent` | K (`tangentMode` setting) | missing | gate removed | — | FOLLOW-UP P4 |
| `/load` | L alias (resume) | builtin `/load <id>` → `session/load` | = | — | OK |
| `/powers` | K `_kiro/powers/refresh` + push | builtin panel on `_kiro/powers/items_changed` push only (`convert/kas/powers.rs:29-31` explains the deliberate non-use of `list`/`refresh`) | = | — | OK |
| `/voice` | L dictation (gate removed) | builtin (cyril-voice feature) | gate removed | — | OK |
| `/steering` | id (steering slash commands dropped from `available_commands_update` since 0.66.33) | — | = | cyril-2nz7 | OK |
| `/stats`, `/paste`, `/reply`, `/feedback`, `/upgrade-agent` (H, +V2-tool warning 3.1.0), `/repo`, `/disconnect`, `/switch`, `/kiro`, `/tab`, `/fullscreen` | L / H / cloud | missing | `/upgrade-agent` wording | — | FOLLOW-UP P4 / OK |
| `/changelog /checkpoint /context-gatherer /copy /custom /editor /experiment /general-task-execution /guide /issue /kvim /lite /logdump /prompt /reasoning /save /session-id /skill /title /todos /transcript /tui` | id (enum-only telemetry ids; no handler, no KAS route) | — | = | — | OK |
| cyril-only: `/new`, `/steer` (K `_session/steer`, implemented by KAS), `/memory`, `/review`, `/answer`, `/thinking` (K `set_config_option thinking`), `/kill`, `/msg` | — | builtins | `/answer` new | — | OK |

## Matrix 8 — cyril v2-only code paths with nothing to talk to on 3.x (compact; FOLLOW-UP, low)

Host-lane facts (not re-derived): 3.x ships one binary `kiro-cli` (= the 2.37.1 `kiro-cli-chat`); "Classic retired" = the v1
engine / legacy UI; the **V2 Rust ACP engine is still shipped in 3.1.0** (`chat-cli-v2` 523 paths, all `_kiro.dev/*` literals,
`acp --agent-engine v2` rc=0); Kiro's own prompt: "V2 no longer receives new features and will be removed in January". So these
paths are *deprecated*, not dead — except the wrapper spawn, which the 2.36.0 acp router may already misroute.

| cyril path | file:line | status on 3.x | horizon | issue | class |
|---|---|---|---|---|---|
| V2 wrapper spawn = the user's bare `kiro-cli acp` | `protocol/bridge.rs:394` (`AgentEngine::V2 => Ok((agent_command.clone(), None))`); default argv `crates/cyril/src/main.rs:32-36`; identity forced to `cyril` on V2 (`protocol/identity.rs:22-27`) | **at risk now**: `cli/acp_engine.rs` (2.36.0) routes by `clientInfo.name`; `cyril` is in neither embedded list; if the 3.x acp default is v3 (HOST-L1) the V2Engine receives KAS and `fingerprint.rs:92-105` fails with a remedy naming the same command | immediate | host NEW P2 ("pin `--agent-engine v2`") | FOLLOW-UP P2 (not the default path) |
| `--agent-engine v2`, `[agent] engine = "v2"`, `AgentEngine::V2` | `crates/cyril/src/main.rs:38-44`; `types/agent_engine.rs:13-24` (default `Kas` with the feature) | parses on 3.x; the engine still exists | January | cyril-a7nk | FOLLOW-UP P3 |
| v2 dialect converter (`kiro.dev/*`: metadata, compaction/status, mcp/*, agent/*, model/not_found, session/*, commands/*) | `protocol/convert/kiro.rs` (38 TUI literals / 41 host literals unchanged, host lane) | functional against the shipped V2 engine | January | cyril-a7nk | FOLLOW-UP P3 |
| v2 command bridge (`kiro.dev/commands/{execute,options}`, `settings/list`) | `domain_mediator/commands/extensions.rs:76-213`; suppressed for KAS advertisements (`convert/kas.rs:493-548`, `engine.rs:404-407`) | v2 only | January | cyril-oieu (closed) | OK |
| v2 trust writes (`toolsSettings.execute_bash.allowedCommands`) | `kiro_agent_config.rs:277` via `crates/cyril/src/app.rs:2027-2040` (only for v2 `_meta.trustOptions`; KAS ignores the field) | dead on KAS by design; v2 path intact | January | cyril-auxt (closed) | OK |
| crew model: `/spawn /kill /msg /sessions`, `SubagentTracker`, crew panel | `commands/subagent.rs`; `domain_mediator/commands/subagents.rs:30,60,74`; `crates/cyril-core/src/subagent.rs`; `cyril-ui/src/widgets/crew_panel.rs` | BROKEN on KAS (methods 0 hits); v2 intact | January | cyril-a8se, cyril-kzke | FOLLOW-UP P3 |
| `session/set_model` | `domain_mediator/commands/session.rs:342` | -32603 on KAS; v2 only | January | — | OK |
| `kiro.dev/metadata` metering + `UsageAccount` v2 shape | `convert/kiro.rs`; `session.rs` | v2 only | January | — | OK |
| v2 `PresentAs` discard (`effective_present_as` forces `Cyril` on V2) | `protocol/identity.rs:22-27` | feeds the router problem above | — | — | — |

## Matrix 9 — Session lifecycle (host lane read; its HOST-L1…L6 verdicts pending)

| step | KAS 0.66.91 / host 3.1.0 | TUI 3.1.0 | cyril @HEAD | consequence | issue | class |
|---|---|---|---|---|---|---|
| **spawn (Free, default)** | default data-dir layout unchanged on 3.1.0: `<data dir>/kas/<ver>-<sha256>/node_modules/@kiro/agent/dist/server/acp-server.js`, `<data dir>/{node,bun,tui.js}`, sqlite `extracted_kas_versions`/`pinned_bin_versions` (host, STATIC; HOST-L2 confirms); `KIRO_RUNTIME_DIR` / `kiro-cli _ extract-runtime` (2.37.0) is an opt-in layout | `kiro-cli acp` self-extracts | `kas/discovery.rs:446-449` reads `KIRO_DATA_DIR`, `KIRO_KAS_SERVER_PATH`, `KIRO_AGENT_PATH`; `:208-283` selects `kas/<ver>-<sha>`; node from `KIRO_AGENT_PATH` or PATH (`:355-360`), never the bundled `<data dir>/node`; argv `:363-369` = host's non-path flags (`--experimental-wasm-modules <server> --transport=stdio --auth=acp-callback`) | default installs HANDLED; a `KIRO_RUNTIME_DIR` install gets `KasMissing::Server` whose remedy (`:50-53`) is wrong under a runtime dir; system `node` may be older than @kiro/agent's `engines` (RUN-STATE; lead V6) | host NEW P3 | FOLLOW-UP P3 |
| spawn env | `kiro-cli acp` passes `NODE_USE_SYSTEM_CA`, `KIRO_CUSTOM_USER_AGENT`, `KIRO_OWNED_KAS_*`, `CLOUD_CONFIG_ENDPOINT`, `KIRO_TELEMETRY_IS_INTERNAL_AMAZON`, `KIRO_CLI_IS_CHILD_PROCESS` (host; TUI spawn identical 2.32.0→3.1.0, tui lane); launcher exports `KIRO_TRUST_CLASSIFIER_MODE=enforce` (host env table) | same | cyril's free spawn sets none (no `env(` in `discovery.rs`) | no org cloud config (`fetch_cloud_config` never runs), trust classifier off, generic UA | cyril-0b8t (P2), cyril-be7e, kas-core NEW P3 | DAY-ONE-GAP P2 (0b8t, standing) |
| login gate + auth renewal | `chat _ get-kas-token [--reason] [--current-expires-at] [--force-refresh]`, envelope unchanged 2.32.0→3.1.0 (host STATIC PASS; HOST-L4 live); 3.1.0 host applies newer sqlite migrations on open (host caveat); 2.37.0 terminal sign-in is wrapper-only (`acp --login`, `_meta.terminal-auth`) | handler byte-identical | gate `discovery.rs:488-495` (fetch once at spawn); responder `kas/auth.rs` (Matrix 5) | FIXED (zbu7); logged-out hang → 0rn8 (open); terminal auth method unread (`authMethods` 0 reads) | cyril-0rn8, cyril-cxd2 | OK / FOLLOW-UP |
| `clientInfo.name` | KAS persona by name (`clientAttribution`) | `kiro-cli` | **`kiro-cli` by default on KAS** (`types/present_as.rs:37`, default test `:62-63`, ADR-0008); `cyril` only on V2 (`identity.rs:22-27`) | KAS applies the kiro-cli profile (cyril-po2z's "unrecognized → kiro-ide" description predates ADR-0008) | cyril-po2z (stale description) | OK |
| `initialize` | result read for identity only (`fingerprint.rs:30-36`) | reads 7 `_meta.kiro` keys | Matrix 1/2 | — | — | — |
| `session/new` request | `_meta.kiro{settings, customAgents?, modeId?, overrideSessionId?, executionTarget?, shellEnvironment? (0.66.91), automaticTurnProfiling? (0.66.58), autoCompactionThreshold? (0.66.58)}` | `overrideSessionId`, `settings`, `universalBackgroundTool` (rollout) | `{cwd, mcpServers:[], _meta.kiro.settings}` (`domain_mediator/commands/session.rs:61-77`) | no per-folder shell env, no turn profiling, no custom agents, no cloud target | cyril-7p7n, cyril-jf71, cyril-tikf | OK (opt-ins) |
| `session/new` response | `configOptions` (Matrix 6), `modes`, `_meta{title,agentMode,memoryConfig{disabled,false},semanticReviewEnabled,ftaEnabled,workflowsEnabled,workflowsCreatorEffort?}` (LIVE leg 1 unchanged) | full | `configOptions` HANDLED (`domain_mediator/commands/session.rs:109-110`); `_meta` DROPPED | memory/workflow state not shown | cyril-0na7, cyril-l4e7 | FOLLOW-UP P3 |
| post-`session/new` writes | — | `set_config_option autopilot:"on"`, default model, effort, thinking, `workflowLaunch` | none | KAS defaults (autopilot on, `when-asked`) — differs from the user's Kiro `/settings` only for `workflowLaunch` and the default model | kas-workflow NEW | FOLLOW-UP P3 |
| `session/prompt` | `_meta.kiro{displayText (read since 0.66.58), outputStyle, agentInitiated (0.66.91), autoCompactionThreshold}`; **sign-in refusal** `-32000 "Sign in to Kiro to send a prompt." data{errorType:"SignInRequiredError",retryErrorType:"CLIENT_ERROR"}` before persistence (kas-core; KAS-L3) | `displayText`, `outputStyle` | no `_meta` (`domain_mediator/commands/session.rs:514-521`); **on any prompt error**: `BridgeError{operation:"prompt"}` (`:540-548`) → system message "prompt failed: <message>" (`cyril-ui/src/state.rs:1193-1196`) **and** `TurnCompleted{EndTurn}` as `PromptTerminal` (`:577-586`) → the busy guard IS released (`turn_mediator.rs:256`) | a signed-out cyril shows KAS's own sign-in sentence and stays usable; no `kiro-cli login` hint or re-auth action | cyril-0rn8 (NOTE), cyril-j66p, cyril-hzkb | OK / FOLLOW-UP P3 |
| turn terminals | `turn_end` first, then the prompt response; `turn_*` frames gained `executionId/turnId/timestamp/boundarySource` (LIVE leg 1) | — | HANDLED (`turn_mediator.rs:225-290`; `engine.rs` `emits_wire_turn_end`); identity fields DROPPED | — | — | OK |
| `session/load` | replays with `_meta.kiro.replay`; **replays `session_usage{replay:true}` at the end** (LIVE leg 1); user rows show `displayText` and carry `agentInitiated` (kas-core); result carries the saved `model` option (warm catalog) and `_meta.executionTarget` | buffers replay; tags source `replay`; applies `session_usage` | `load_session_request` carries no `_meta` (`domain_mediator/commands/session.rs:82-87`); `configOptions` HANDLED (`:217-219`); replay unmarked; `session_usage` DROPPED; `agentInitiated`/`bufferOnly` user chunks render as typed | after `/load`: credits total 0, wake prompts look user-typed | tui NEW P2, cyril-99ds | DAY-ONE-GAP P2 (credits) / FOLLOW-UP P3 |
| `session/cancel` | notification | = | HANDLED (`domain_mediator/commands/session.rs:282`) | — | — | OK |
| `session/close` | `sessionCapabilities.close` | step close, dashboard | **NOT-SENT** (0 hits in `crates/`) | loaded workflow-step sessions stay open after replay | cyril-c6hv deferred P4 | FOLLOW-UP P4 |
| step / peer sessions | workflow steps = peer sessions (`_kiro/workflow/*` by `workflowId`; `node_start` names the step session); sub-agents = `agentSubtaskId`-tagged frames on the parent (unchanged); `peer` messaging dark | step sessions in the monitor | steps: `WorkflowTracker` + peer route (cyril-jxfu closed); sub-agents: interleaved (`agentSubtaskId` 0 hits) | — / ebqu | cyril-ebqu, cyril-zd8u | DAY-ONE-GAP P2 (ebqu) |
| auto-wake turns | terminal `run_complete`, repeat-cap pause, step `warning`/`error`, `/review cancel` abort → parent `turn_start` with `agentInitiatedReason`; 0.66.91 `wakeTargetIsBusy` queues a completion as steering when a prompt is in setup (kas-workflow; WF-L4) | same | HANDLED with headers/footers (`turn_labels.rs:23-47,116-134`; `workflow.rs:468-497,638-650`); B5 notice for busy-parent injections (`turn_labels.rs:59-87`) | billed turns the user did not start — `/review cancel` makes the main agent redo the review (cyril-oswq, LIVE 2.26.0) | cyril-oswq (P1), cyril-32sp | DAY-ONE-GAP P1 (oswq, standing) |
| stderr | direct `node acp-server.js` still prints `[INFO]` startup lines (LIVE leg 0); the 3.1.0 "quiet `kiro-cli acp`" is a host-side filter (host lane) | host filter | `protocol/transport.rs:96-116` drains the child's stderr into the debug log and a bounded tail — nothing reaches the TUI | n/a | — | OK |
| shutdown | — | Windows supervisor `windows-kas-process` | no signal handler; SIGTERM/SIGHUP can orphan the node child | — | cyril-qdcs (P2) | DAY-ONE-GAP P2 (standing) |
| V2 wrapper on a 3.x host | acp router (2.36.0) | TUI pins `--agent-engine=v2` since 2.36.0 | bare `kiro-cli acp` (`bridge.rs:394`) | mismatch at `fingerprint.rs:92-105` if the acp default is v3 (HOST-L1) | host NEW P2 | FOLLOW-UP P2 |
| first run after upgrading kiro-cli to 3.x | `install.sh` puts the single binary at `~/.local/bin/kiro-cli` (or `/usr/local/bin`), removes legacy shell integration (host) | — | resolver fallbacks `kas/auth.rs:306,319-337` cover both dirs; nothing references `kiro-cli-chat` (host) | HANDLED | cyril-igrx (closed), cyril-a7nk | OK |

## Day-one gaps (ranked) — what a cyril user on kiro-cli 3.1.0 / KAS 0.66.91 hits without doing anything special

Priority rule (RUN-STATE fold rules + trap 29): P1 = live-proven regression or billed/unsolicited work; P2 = a dropped field or
missing route on the default engine; P3 = reachable only through something the user turns on; P4 = cosmetic/dark. Every row
names the cyril entry point. "new" = first reaches a cyril user with 0.66.91 / 3.1.0; "standing" = unchanged since 2.32.0.

| # | P | gap (what the user experiences) | cyril entry point | evidence | issue | new? |
|---|---|---|---|---|---|---|
| 1 | **P1** | `/review cancel` ends the run, then KAS's abort wake opens a labelled turn on the main session and the model redoes the review (billed sub-agent work outside the run's authorization) | `/review cancel` (`commands/workflow.rs:192`) | LIVE 2.26.0 (issue); wake trigger set unchanged through 0.66.91 (kas-workflow) | cyril-oswq | standing |
| 2 | **P1** | "Always allow" on a workflow step's approval loops 20×, then KAS rejects and the step fails | any `/review` / `/workflow run` approval answered Always-allow | `Permission flow exceeded 20 approval rounds` 2 = 2 in 0.66.91 (kas-workflow); the consent echo (auxt) is new since the live proof → re-check (lead V3) | cyril-w55l | standing (re-check) |
| 3 | **P2** | the model can change mid-session (fallback on refusal/outage, auto re-send of a refused turn) with no trace; the toolbar keeps the picked model | every prompt | `model_fallback_{refusal,availability,auto_refusal}` default true in 0.66.91 (kas-core; KAS-L1); `kas.rs:822` drops `model_fallback`/`model_routed`; `toolbar.rs:63-66` | cyril-aoby (raise to P2), cyril-ojum | **new** |
| 4 | **P2** | `/usage` "Credits" and the footer session total restart at 0 after `/load`, and never match Kiro's authoritative `session_usage` total | `/usage`, `/load` | LIVE leg 1 (`session_usage` after every `turn_completion`, replayed on load); `kas.rs:822`; `cyril-core/src/session.rs:291,450,470`; `usage_panel.rs:207,219` | tui NEW P2 | **new** |
| 5 | **P2** | `/workflow status` rows show no per-step credits; run totals are a parent-metering estimate | `/workflow run`, `/review` (gate-off) | LIVE leg 2a (`credits[]` on `node_complete`/`run_complete`/snapshot nodes); `convert/kas/workflow.rs:207-218,259-283,444-520` | kas-workflow NEW P2 | **new** |
| 6 | P2 | sub-agent (`agentSubtaskId`) text and tool calls interleave with the main agent's | any prompt that spawns a sub-agent | `agentSubtaskId` 0 hits in `crates/`; 1v25 half fixed (`state.rs:555-560`) | cyril-ebqu | standing |
| 7 | P2 | auto-compaction is invisible (`summarization_*`), throttling/model `display_error`s are lost, the new `summarizationThreshold` is not shown | long sessions | `kas.rs:628-651,822`; `kas.rs:698-760` | cyril-58uv, cyril-6s21 | standing (+1 field) |
| 8 | P2 | no `/context`, no `/compact` | typing them | no `_kiro/session/{context,compact}` route (Matrix 3) | cyril-1aq0 | standing |
| 9 | P2 | `chat.enableThinking:false` in cli.json is ignored at session start; three dead keys logged `unknown-key` per connection | every connection | LIVE leg 0 (`setting/thinking` on 0.66.91); `kas/settings.rs:77,80,91-96` | cyril-53qx | standing |
| 10 | P2 | default output style only (5 styles exist) | every prompt | no `_meta.kiro.outputStyle` (`domain_mediator/commands/session.rs:514-521`); `_kiro/config/template` never called | cyril-j66p | standing |
| 11 | P2 | org cloud config never syncs on cyril's spawn (`CLOUD_CONFIG_ENDPOINT` absent) | every spawn (org users) | host env table; `discovery.rs:363-369` | cyril-0b8t | standing |
| 12 | P2 | SIGTERM/SIGHUP can orphan the KAS node child | closing the terminal | issue (LIVE 2.32.0) | cyril-qdcs | standing |
| 13 | P2 | grants recorded for other sessions (steps, sub-agents) are not remembered | workflow approvals in bursts | `always_allow.rs` has no cross-session store (issue) | cyril-7c0s | standing |
| 14 | P3 | with `chat.enableWorkflows=true`, the user's Kiro "Workflow use" choice is ignored (engine default `when-asked`) and a `/load` of a pre-0.66.58 session delegates everything (`legacy`) | `chat.enableWorkflows` + `/load` | kas-workflow (`workflowLaunch`); `kas/settings.rs:108-116`; `kas_config.rs:20-48` | kas-workflow NEW P2 / tui NEW P3 | **new** |
| 15 | P3 | a user agent file with `memory.mode` enables KAS native memory (hidden per-turn extraction) under cyril; no opt-out is sent and `/memory` never shows those memories | user agent file | memory rollout `all` since 0.66.58 (kas-core; KAS-L5); `kas/settings.rs:76-86` has no `memory`/`userMemoryOptIn` | cyril-q8gq, cyril-0na7, cyril-s21o | **new (default flip)** |
| 16 | P3 | a signed-out session shows "prompt failed: Sign in to Kiro to send a prompt." and stays usable, with no `kiro-cli login` hint or re-auth action | token loss mid-session | `SignInRequiredError` -32000 (kas-core; KAS-L3); `domain_mediator/commands/session.rs:540-548,577-586`; `state.rs:1193-1196` | cyril-0rn8 (NOTE) | **new (better than the 2.32.0 hang)** |
| 17 | P3 | an MCP tool that elicits input is declined (visibly) instead of asked | MCP tools that elicit | `kas/mcp.rs:50-55,70-82`; TUI T8 | tui NEW P3 | standing |
| 18 | P3 | no session recap | a setting Kiro has and cyril lacks | `kas/settings.rs` has no `sessionRecap`; `kas.rs:822` drops `recap` | tui NEW P3 | new (opt-in) |
| 19 | P3 | `KIRO_RUNTIME_DIR` / `_ extract-runtime` installs are invisible; the remedy text points the wrong way | packaged / runtime-dir installs | `discovery.rs:446-449,50-53` (host) | host NEW P3 | new (opt-in) |
| 20 | P3 | autopilot cannot be turned off; sandbox cannot be enabled | `/mode` picker has no such rows | `kas_config.rs:20-48` | vh69 follow-up, cyril-6vo6 | standing |
| 21 | P3 | no `/permissions`-style listing of what Always-allow persisted; `_kiro/policy/changed` unlogged | after an Always-allow (user/workspace) | Matrix 3; `convert/kiro.rs:1305` | cyril-r5pl | standing |
| 22 | P3 | `/spawn` `/kill` `/msg` error, `/sessions` is empty | typing them | `commands/mod.rs:568-571`; KAS 0 hits for `_session/spawn` | cyril-a8se | standing |
| 23 | P3 | a missing/broken custom agent falls back silently | `/mode` to a broken agent | `_kiro/customAgent/*` dropped (`kiro.rs:1305`) | cyril-7xxk | standing (filed) |
| 24 | P4 | the remote-MCP OAuth notice is not cleared by `_kiro/mcp/oauthCompleted` (only by the next `_kiro/mcp/status`) | remote MCP with OAuth | `engine.rs:143`; `callbacks.rs:190-193`; `kiro.rs:1305` | kas-core NEW P4 | **new (KAS now sends it)** |
| 25 | P4 | Always-allow offers exact + `<cmd> *` but not Kiro's `<cmd> <sub> *` tier | Always-allow scope page | `always_allow.rs:163-180` | tui NEW P4 | standing |
| 26 | P4 | `KIRO_DEFAULT_SETTINGS` (managed defaults) not consulted; replay marking / `agentInitiated` replay rendered as typed; turn identity fields and `rateMultiplier` dropped; Approve-for-me dark | — | host NEW P4; cyril-99ds; cyril-lxuo; tui NEW P4 | — | new / standing |

Not day-one (checked, no row): workspace trust (KAS hard-codes `workspaceTrusted:!0`; cyril = `--trust-workspace`, kas-core/tui);
`structuredContent` (no KAS delta); `_meta.kiro.preview` removal (cyril never read it, LIVE leg 2a); MCP elicitation flag (advisory);
`PreCompact`/`PostCompact` (host-mode hooks never see them; kas-mode gets `hook_update`); unparseable-command auto-deny (renders as a
denied call); shell default `/bin/bash` when `SHELL` unset (agent-hosted only; cyril hosts the terminal, `host_shell.rs:424-428`).

## Regression checks for the fixes closed since 2.32.0 (expectations for live leg 5; cyril build must be recorded, trap 31)

| fix | what cyril does now (file:line) | what in 0.66.91 / 3.1.0 could break it | expectation for leg 5 |
|---|---|---|---|
| **cyril-k8wc** `shell_type` basename | `terminal_io.rs:283-285` → `host_shell.rs:216-222` `wire_name()`: `bash\|zsh\|sh\|dash\|ksh` (`PosixShell::name` :29-35), `fish`, `pwsh`, `powershell`; default host shell = `$SHELL` else `bash` on PATH (`:424-428`) | 0.66.91 `xp()` classifier (quoted) accepts every value cyril can send; `j4()` basename mapper `case"posix"` 1 = 1; the 3.1.0 `getDefaultShell` change is agent-hosted only; the bound-shell telemetry set `L4a` (quoted @2036808) lacks `ksh`/`dash` variants only for metrics | kiro.log `policy.shell.type.resolved {shellName:"bash"\|"zsh",…}` and **no** `policy.shell.type.unknown`; the second identical command after an Always-allow (session scope) is NOT re-asked; `InterpreterChangedError` never appears |
| **cyril-zbu7** token renewal | argv `kas/auth.rs:44-46,559-563`; envelope `:859-860`; fields `:895-914`; 30 s deadline `:49`; single-flight `:449-460`; stored fallback only on `HelperMissing` `:616-647`; login gate at spawn `discovery.rs:488-495` | nothing in the argv/envelope (host: byte-identical 14/14; TUI handler byte-identical); risks: (a) the 3.1.0 helper applies newer sqlite migrations to `data.sqlite3` (host HOST-L4) — one-way for an installed 2.32.0; (b) `cps.auth_provider.unknown {provider:"none"}` ×2 when the reply lacks `provider` (LIVE leg 0) — cyril relays `provider` only when kiro-cli returns it, so the line appears but is harmless unless 0.66.91 later requires it (LIVE-E/F) | first `_kiro/auth/getAccessToken` answered from `chat _ get-kas-token`; a `forceRefresh:true` request runs `--force-refresh`; no `HelperMissing` fallback on 3.1.0; the session survives past the 1 h boundary |
| **cyril-auxt** consent echo | `convert/mod.rs:456` → `convert/kas.rs:128-190`: `capability`/`workspaceRoot` verbatim from the request, `scope` from `always_allow.rs:33-47` (session/workspace/user; `s` cycles), `resource` from `:64-96` (exact / `<cmd> *` / folder glob / `*`); only for an offered `allow_always`; no agent-JSON write on KAS (`app.rs:2027-2040`) | KAS intake unchanged (`scope\|\|"session"`, quoted @14303817); new since the fix: 0.66.62 unparseable commands carry `persistableConsent:false` (kas-core) — cyril's scope page still offers user/workspace and KAS would refuse to persist silently (lead V2); 2.36.0 subcommand tier is a value KAS accepts (any pattern) but cyril never composes | allow_always reply carries `_meta.kiro.consent{capability:"shell",scope,resource,workspaceRoot}`; with scope `user` the temp HOME gains `.kiro/settings/permissions.yaml` and a fresh `session/new` runs the command unasked; scope `session` suppresses the re-ask within the session |
| **cyril-1v25** orphan `tool_call_update` | `cyril-ui/src/state.rs:555-560` tracks an update whose id never started as a new row (`track_tool_call`) | 0.66.91 adds the turn-limit "Sub-agent '<id>' stopped at its turn limit…" text on the same orphan (kas-core) — content, not shape | "Tool Load" and "Subagent Response" rows appear once each; no "unknown tool call" warning |
| **cyril-oieu** command dispatch | `convert/kas.rs:493-548`: `_meta.kiro.type` ∈ `custom-agent\|skill\|prompt` → `AgentCommandRoute::Prompt`; other/missing types suppressed with a log; `commands/mod.rs:654` submits the original text | KAS 0.66.91 advertises the same three types (steering commands gone since 0.66.33, cyril-2nz7); a new `type` value would be suppressed silently-with-log (watch the debug log) | a skill, a saved prompt and a custom agent each run as `/name args` prompt text; no `_kiro.dev/commands/execute` on the wire |
| **cyril-vh69** `/model` `/effort` `/mode` | `commands/mod.rs:528-534`; `types/kas_config.rs:20-48,207-234,258-275`; one `SetConfigOption` per confirm (`domain_mediator/commands/session.rs:388`); toolbar `toolbar.rs:54-82` | model option `_meta.kiro` keys unchanged (LIVE leg 1); `effortLevel`/`thinking` still accepted-not-echoed on both engines (LIVE leg 1) — the post-ack effort chain must not wait for an echo (lead V4; cyril-m8kq); a `workflowLaunch` select arrives gate-on and is kept as an unsettable generic option | pickers list the 21 models / 7 modes / the model's effort levels; `set_config_option` replies echo `currentValue`; the effort badge updates without a `BridgeError` |
| **cyril-d7ve** `KIRO_HOME` resolver | `kiro_agent_config.rs:94-152` (`resolve_kiro_home`: relative, `/`, home-containing values ignored with a warn); `kas/settings.rs:63-65` reads `<KIRO_HOME>/settings/cli.json` | host: `KIRO_HOME` readers 7 → 9 (secret_storage.rs, 2.34.0) with the same semantics; 3.1.0 adds `KIRO_DEFAULT_SETTINGS` *below* cli.json (host) — cyril reads cli.json only (P4) | with `KIRO_HOME=<tmp>/kh` cyril and KAS agree on agents, hooks, settings and sessions; a relative `KIRO_HOME` is ignored by both |
| **cyril-6lql** repeat-cap wake label | `workflow.rs:468-497` (wake-label queue), `:638-650` (`WakeCause::RepeatCap`), `:693-697`; header `turn_labels.rs:23-47` | 0.66.91 `wakeTargetIsBusy` queues the completion as steering when the parent has a prompt in setup/persistence (kas-workflow; WF-L4) → fewer wakes, more B5 notices (`turn_labels.rs:59-87`); `repeatCapNote` unchanged; LIVE leg 2a: the pause wake is unchanged on NEW (`_kiro/session/notify` then `send-message-wake`) | header `─── ⚙ workflow "<name>" paused · repeat "<node>" reached its iteration limit (N) · agent follow-up ───` on the wake; the residual cyril-32sp case (idle parent, exhausted cap, no note) stays unlabelled |

## Follow-ups (not day-one; mapped to issues)

| # | item | P | issue |
|---|---|---|---|
| F1 | V2 wrapper spawn must pin `--agent-engine v2` (`bridge.rs:394`); reword `fingerprint.rs:92-105` | P2 (if HOST-L1(a) ≠ v2) | host NEW P2, cyril-a7nk |
| F2 | outputStyle: read `_kiro/config/template`, send `session/prompt._meta.kiro.outputStyle` | P2 | cyril-j66p, cyril-lyrx |
| F3 | `_kiro/governance/state`, `focus_update`, `turn_completion` summary | P2 | cyril-0o7e |
| F4 | `workflowLaunch`: read `chat.workflowLaunch`, set after `session/new`, let `KasConfigId` carry it | P3 | kas-workflow NEW |
| F5 | memory: marshal `memory{mode,reflection}` / `userMemoryOptIn` (decide the default), read `_meta.memoryConfig`, `/memories` over `_kiro/memory/*` | P3 | cyril-q8gq, cyril-0na7, cyril-s21o |
| F6 | sign-in refusal: map `errorType:"SignInRequiredError"` to a `kiro-cli login` hint | P3 | cyril-0rn8 |
| F7 | `/permissions` listing (`_kiro/permissions/list` + `_kiro/policy/changed`) | P3 | cyril-r5pl |
| F8 | `KIRO_RUNTIME_DIR` discovery + remedy text | P3 | host NEW P3 |
| F9 | session recap setting + `recap` consumer | P3 | tui NEW P3 |
| F10 | interactive MCP elicitation (form/url) | P3 | tui NEW P3 |
| F11 | `autopilot` toggle, sandbox options, `workflowLaunch` in the picker family | P3 | vh69 follow-up, cyril-6vo6 |
| F12 | `/context`, `/compact`, `/tools`, `/knowledge`, `/code`, `/rewind`, `/workflow pause` | P2/P3/P4 | cyril-1aq0, cyril-4u4a |
| F13 | `_kiro/mcp/oauthCompleted` + `openExternalUrl{serverName,oauthCompletion,oauthFlowId}` | P4 | kas-core NEW P4 |
| F14 | subcommand tier in the Always-allow scope page; hide user/workspace rows when `persistableConsent:false` | P4 | tui NEW P4 (+ this lane's note) |
| F15 | replay marking, `agentInitiated` replay, `consentRound`, `interaction_resolved` | P3 | cyril-99ds |
| F16 | `context_usage.summarizationThreshold`, turn identity fields, `rateMultiplier` | P4 | cyril-6s21, cyril-lxuo |
| F17 | `KIRO_DEFAULT_SETTINGS` layering | P4 | host NEW P4 |
| F18 | `session/close` for loaded step sessions | P4 | cyril-c6hv deferred |
| F19 | `/spawn /kill /msg /sessions` on KAS; crew → `agentSubtaskId` rendering | P3 / P2 | cyril-a8se, cyril-ebqu |
| F20 | trust classifier / Approve-for-me (dark until `KIRO_TRUST_CLASSIFIER_MODE=enforce` is default) | P4 | kas-core NEW P3 (opt-in), tui NEW P4 |

## Proposed issues / notes (rivets grepped with `static-v3cov-issues-3.1.0.py`; nothing filed by this lane)

Already proposed by sibling lanes this cycle (referenced above, not duplicated): tui NEW P2 `session_usage`; kas-workflow NEW P2
step `credits` and NEW P2/P3 `workflowLaunch`; kas-core NOTE aoby→P2, NEW P3 Approve-for-me, NEW P4 `oauthCompleted`, notes on
53qx/58uv/s21o/0na7/hzkb/6s21/iq2c/68ny/7p7n/7c0s/0o7e/ebqu/497j/0rn8; tui NEW P3 recap, NEW P3 elicitation, NEW P4 tier, notes on
r5pl/0na7/iq2c/nk4o-tqr8/68ny/4u4a/1aq0/mq15/99ds/q8gq; host NEW P2 (v2 pin), NEW P3 (`KIRO_RUNTIME_DIR`), NEW P4
(`KIRO_DEFAULT_SETTINGS`), notes on a7nk/cxd2/0rn8/zjm5/s21o/tikf/igrx/d7ve. Rivets check: no open issue matches `session_usage`,
`recap`, `workflowLaunch`, `KIRO_RUNTIME_DIR`, `KIRO_DEFAULT_SETTINGS`, `subcommand tier`, `PreCompact`, `sessionTools`,
`autoCompactionThreshold` — so the NEW proposals above are not duplicates; cyril-7xxk (customAgent) IS filed since the 2.32.0 cycle.

This lane's own proposals (note-vs-issue rule: an existing issue gets a NOTE):
- **NOTE cyril-c6hv (the KAS parity epic):** six children closed since filing (cyril-oieu, cyril-vh69, cyril-zbu7, cyril-auxt,
  cyril-k8wc, cyril-1v25) and cyril-4c4d is CLOSED (KAS default). Remaining P2 children: cyril-ebqu, cyril-58uv, cyril-1aq0,
  cyril-53qx, cyril-j66p, cyril-aoby, cyril-0o7e, cyril-7c0s. Add the three 0.66.91 rows as children once filed (`session_usage`,
  step credits, `workflowLaunch`). Deferred candidates: `_kiro/customAgent/*` is now cyril-7xxk; `session/close` of loaded step
  sessions (P4) is still unfiled. Attach this file as the 3.1.0 parity list.
- **NOTE cyril-0rn8:** statically the turn mediator DOES release on a refused prompt — `domain_mediator/commands/session.rs:540-548` enqueues
  `BridgeError{operation:"prompt"}` and `:577-586` the `TurnCompleted{EndTurn}` terminal; the UI line is "prompt failed: Sign in to
  Kiro to send a prompt." (`state.rs:1193-1196`). The remaining ask is the hint/re-auth mapping of `data.errorType`, not the busy
  state. KAS-L3's cyril arm confirms.
- **NOTE cyril-q8gq (raise P3 → P2 candidate, owner decision):** `userMemoryOptIn` moved from "missing BOOL_MAP row" to
  load-bearing: since 0.66.58 the KAS memory rollout is `all`, so the per-connection veto is the only thing standing between a
  user's `memory.mode:"read_write"` agent file and native memory extraction under cyril. CLAUDE.md records that native memory
  defaults stay unchanged by explicit approval — this note records that the upstream default itself moved (kas-core), so the
  approved stance may need re-confirming.
- **NOTE cyril-auxt (closed) / cyril-7c0s:** KAS 0.66.62+ marks unparseable shell commands `persistableConsent:false` (kas-core);
  cyril's scope page (`always_allow.rs:163-180`) still offers user/workspace for them and KAS will decline persistence silently.
  Read `_meta.kiro.consent.persistableConsent` on the request and hide the non-session rows (fold into tui NEW P4 or 7c0s).
- **NOTE cyril-a8se:** `/steer` is NOT in this family — `_session/steer` and `_session/steer/clear` are implemented by KAS
  0.66.91 (persistence class `metadata`); only `/spawn /kill /msg /sessions` stay v2-only.
- **NOTE cyril-po2z:** its premise (default `PresentAs::Cyril`) is stale: the default is `KiroCli` (`present_as.rs:37`, ADR-0008),
  so cyril's KAS free path presents `kiro-cli`; the `unrecognized → kiro-ide` profile applies only when a user sets `present_as =
  "cyril"`. Also relevant to the host router: on the V2 wrapper cyril is forced to present `cyril` (`identity.rs:22-27`), the name
  the 2.36.0 router cannot find.
- **NOTE cyril-ebqu:** `agentSubtaskId` still has 0 hits in `crates/` at e3a3868d; 0.66.91 adds the turn-limit wrap-up text on the
  orphan "Subagent Response" (now rendered via 1v25).
- **NOTE cyril-6s21 / cyril-58uv:** `context_usage.summarizationThreshold` (80, both levels) and `turn_*.{executionId,turnId,
  timestamp,boundarySource}` arrive on 0.66.91 (LIVE leg 1) and are dropped at `kas.rs:694-697,698-760` — model-or-ignore per the
  full-wire-surface rule (P4).
- **NOTE cyril-m8kq / cyril-vh69 (closed):** LIVE leg 1 says `set_config_option effortLevel|thinking` is accepted and NOT echoed as
  a configOption on either engine; confirm (lead V4) that cyril's post-ack chain (`kas_config.rs:207-234`) and the thinking lever
  do not depend on an echoed option.
- **Correction to the 2.32.0 coverage file (Matrix 4b):** the TUI column said `display_error` is consumed; the 3.1.0 bundle has 0
  occurrences of `display_error` (quoted or not) — Kiro's own client renders MCP failures from `_kiro/mcp/status`, not from this kind.
  `hook_update` and `focus_update` are also 0 in the TUI.
- No new issue from this lane: every gap found maps to an existing issue or a sibling lane's proposal.

## Leads for the live lane (V1…; only what KAS-L1..13, WF-L1..10, TUI-L1..13, HOST-L1..6 did not send)

All legs: fresh temp `HOME`, real `XDG_DATA_HOME`, cyril debug build at HEAD with the commit recorded (trap 31), KAS 0.66.91 on
`binaries/3.1.0/node`; positive control `initialize.clientCapabilities._meta.kiro.configurationState:true` where a driver is used.

- **V1 — leg 5 regression checklist (cyril-on-KAS, 3–4 turns).** Run the eight expectations of the regression table in one
  session: read cyril's real `_kiro/terminal/shell_type` reply (expect the interpreter basename; trap 34), the first
  `getAccessToken` answer (and the `cps.auth_provider.unknown` line), one shell command answered Always-allow with scope `session`
  then `user` (echo shape; `permissions.yaml` under the temp HOME; no re-ask), one sub-agent prompt (orphan rows render), one skill
  invocation, `/model` + `/effort` + `/mode` round trips, `KIRO_HOME=<tmp>/kh`, and one repeat-cap recipe wake (header text).
  Also record `clientInfo.name` (expect `kiro-cli`) and whether `/usage` after `/load` shows 0 while KAS replayed `session_usage`.
- **V2 — Always-allow on an unparseable command (one shell turn).** Prompt a command KAS cannot parse (e.g. an unbalanced quote
  inside a heredoc); expect `request_permission._meta.kiro.consent.persistableConsent:false` + `persistableConsentReason`; answer
  Always-allow with scope `user` from cyril. Expected 0.66.91: no permissions file written, kiro.log consent error; cyril shows the
  grant as saved. Decides the F14 "hide user/workspace rows" note.
- **V3 — cyril-w55l re-check with the consent echo (one `/review` or one-step recipe, gate-off).** Answer a step-session approval
  with Always-allow (scope `session`, cyril's default) and count the approval rounds; OLD proof was pre-auxt. If the loop persists,
  repeat with scope `workspace`.
- **V4 — effort/thinking post-ack on 0.66.91 (zero extra turns, rides V1).** After `/effort <level>` and `/thinking off`, record
  the `set_config_option` replies (LIVE leg 1: no `effortLevel`/`thinking` option echoed) and whether cyril's toolbar badge and
  thinking indicator update without a `BridgeError` (cyril-m8kq).
- **V5 — `/load` credits (rides V1).** After two metered turns, `/new`, then `/load <old id>`: expect KAS to replay
  `session_usage{replay:true}` and `turn_completion` rows; record cyril's `/usage` "Credits" and footer total (expected 0 or the
  replayed turn sum — which one tells whether `turn_completion` replays feed `record_turn`).
- **V6 — node engines (zero turns).** Record `node --version` of the `node` cyril's free spawn resolves (PATH / `KIRO_AGENT_PATH`,
  `discovery.rs:355-360`) against `@kiro/agent` 0.66.91 `package.json engines` (RUN-STATE) and any engine warning on stderr; the
  bundled `<data dir>/node` (v22.23.4) is never preferred by cyril.
- **V7 — `_kiro/workspace/active_file` under cyril (rides any read turn).** kiro.log: does 0.66.91's `createActiveFileResolver` call
  the method on cyril's spawn (answered `null`, `host.rs:68,248`) or skip it for a client without the capability? Feeds cyril-4cz1.

## Method notes

- Scripts (copied from 2.32.0, paths moved to the 3.1.0 carve/TUI with 2.32.0 as OLD): `experiments/conductor-spike/
  static-v3cov-{ctx,kinds,methods,issues}-3.1.0.py`. **Adaptation bug found and fixed:** the methods script indexed
  `surface.json["trees"][1]` (NEW in the two-tree 2.32.0 file, but 0.66.48 in the six-tree 3.1.0 file) and reported 124 literals;
  it now selects the tree whose `version`/`label` is `0.66.91` (138). The kinds list was replaced by kas-core's 29 kinds. Outputs:
  `v3-coverage/tmp/kinds.json`, `v3-coverage/tmp/methods.json` (git-ignored by `experiments/conductor-spike/audit-*/**/tmp/`).
- Counts quoted are `str.count` in the 0.66.91 / TUI 3.1.0 bundles with 0.66.43 / TUI 2.32.0 via `--old` (trap 2); set-level
  claims (138 literals, 29 kinds, 75/38 slash literals, 8 `sessionUpdate` variants) come from the delta lanes' set diffs (trap 3)
  and were re-counted here where a script exists.
- KAS reading this lane did itself (STATIC, quotes reproducible with `static-v3cov-ctx-3.1.0.py --kas -w 400 '<literal>'`):
  `xp()` shell-name classifier (@2035112), `j4()` basename mapper (`case"posix"`, @13513415), `L4a` bound-shell set (@2036808),
  consent intake `scope||"session"` (@14303817), `SignInRequiredError` class (@2693042) and `refuseWhileSignedOut` (@15844960),
  `_kiro/userInput` call under `userInputCapability` (@15192582), `terminalInput` model text (@2039886), `createActiveFileResolver`
  (@15582599), `userMemoryOptIn` catalog row (@2787903), `_kiro/openExternalUrl` gated on `resolvedCapabilities.openExternalUrl`
  (@15779523), `id:"outputStyle"` `promptScoped` (@9006926), `_session/steer` persistence class (@15153621); negatives: `"_kiro/
  modelFallback/` 0, `mcpElicitation` 0, `"_session/spawn"` 0, `id:"<configId>"` 0 for the session options (ids are not literals).
  TUI 3.1.0: `configId:"autopilot|sandbox|mcpSandboxing|sandboxNetworkMode|thinking|mode|model|effortLevel"`, `"workflowLaunch"`,
  `kind==="session_usage"`, `kind==="recap"`, `"_kiro/userInput"`, `"_kiro/secret/get"` quoted at the offsets given; `display_error`,
  `focus_update`, `hook_update`, `turn_timing`, `suggested_prompts`, `network_configuration`, `"_kiro/workspace/active_file"` = 0.
- cyril verification: every `file:line` was re-read at HEAD e3a3868d with `grep -n` / `sed -n` in this session, except the
  `convert/kas/workflow.rs` struct spans (:207-218, :259-283, :444-520, :588-618) and `workflow.rs:487,1319`, which are the
  kas-workflow lane's at the same HEAD. The brief's `crates/cyril/src/domain_mediator/commands/kas.rs` does not exist; the file is
  `crates/cyril-core/src/protocol/domain_mediator/commands/kas.rs`.
- Live evidence is cited by live-lane leg only (legs 0, 1, 2a were complete at write time; leg 5 is the regression verdict). This
  file contains no `<capture>.jsonl#N` citations because the captures live under `audit-3.1.0/live/`, which `check-citations.py`
  does not resolve from this directory; `check-citations.py` was run anyway (expect "0 citations, pass").
- Host lane facts were taken from `audit-3.1.0/host/FINDINGS.md` (complete at write time) and the orchestrator's forwarded summary;
  nothing of the host analysis was redone. HOST-L1 decides whether the V2-wrapper row is P2 or P3.
- Not done: no execution of anything from Kiro, no rivets writes, no edits outside `v3-coverage/` and the four new scripts.
