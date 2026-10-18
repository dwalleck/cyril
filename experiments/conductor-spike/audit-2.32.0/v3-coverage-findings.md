# v3-coverage lane — standing KAS-default parity inventory (kiro-cli 2.32.0, KAS 0.66.43, TUI 2.32.0)

Lane run 2026-10-18. Static only: nothing from Kiro was executed (no kiro-cli, no node acp-server.js).
This file UPDATES the nine matrices of the 2.31.1 coverage write-up (`audit-2.31.1/v3-coverage-findings.md`,
cyril HEAD 5ee8bc07); it does not rebuild them. Rows marked "= 2.31.1" were re-checked against HEAD and the
0.66.43 / TUI 2.32.0 bundles and did not change (same KAS count, same TUI consumer, same cyril verdict).

Artifacts: ENGINE `~/.local/share/kiro-research/kas-carves/2.32.0/node_modules/@kiro/agent/dist/server/acp-server.js`
(0.66.43, "KAS"); REFERENCE CLIENT `~/.local/share/kiro-research/tui-bundles/kiro-tui-2.32.0.js` ("TUI");
CYRIL `crates/` at HEAD 7c7b0d1d (crates last touched by ff878634 #169 cyril-oieu, b9b6e95d #168 cyril-5nep,
bb5c03c0 #171 cyril-vh69). File:line are relative to `crates/cyril-core/src/` unless prefixed with a crate.
Verdicts (trap 17): HANDLED / DROPPED (received, ignored; logged or not) / NOT-SENT / NOT-IMPLEMENTED / BROKEN.
STATIC unless marked LIVE(live lane leg N) — those cite the live lane's own checked citations in
`audit-2.32.0/live/FINDINGS.md`; this lane cites no capture frames itself (its captures do not resolve from this dir).
Flip column: BLOCKS-FLIP / FOLLOW-UP Pn / NONE.

## Summary

1. Two of the four 2.31.1 flip blockers are **fixed at HEAD**: cyril-oieu (#169, KAS skills, saved prompts and custom agents dispatch as prompt text; LIVE re-qualified on 2.32.0 per the issue note) and cyril-vh69 (#171, closed: `/model` `/effort` `/mode` KAS builtins over `session/set_config_option`). Two are **still open and unchanged**: cyril-zbu7 (token never renewed) and cyril-auxt ("Always allow" not persisted).
2. **New day-one blocker (P1, STATIC, filed as cyril-k8wc):** cyril answers `_kiro/terminal/shell_type` with `"posix"`, which KAS's `jh()` maps to `"unknown"`. Every bash/zsh shell command then asks, every time, and even in-session "Always allow" cannot apply (`kas/host_shell.rs:166-171`). It pre-dates 0.66.43 and blocks any cyril-auxt fix. Live KAS-L1 decides.
3. **cyril-auxt has a KAS-native route (trap 26).** KAS persists an allow_always grant itself when the permission response echoes `_meta.kiro.consent{capability,scope,resource,workspaceRoot}`. Scope `user` writes `<userConfigDir>/settings/permissions.{yaml,json}`, scope `workspace` writes the workspace permissions file, and a response without the echo defaults to `session`. Kiro's TUI has sent this echo since at least 2.31.1, which corrects the 2.31.1 coverage row that said it sends none. cyril sends only `rejectionReason` (`protocol/convert/kas.rs:68-92`). Re-scope auxt onto cyril-gn07's echo instead of writing agent JSON.
4. 0.66.43 adds no `_kiro/*` method, no session_info kind and no sessionUpdate variant (123 = 123 methods, 25 = 25 kinds). cyril's coverage is unchanged: 8 of 25 kinds HANDLED, and the other 17 drop unlogged at `protocol/convert/kas.rs:660`.
5. New 2.32.0 surface, by reachability:
   - Reachable on cyril's ADR-0011 gate-off path: workflow `attention[]`, `inspect.pendingSteps` and `bufferOnly` are DROPPED (P3). The repeat-cap pause wake is HANDLED as a generic agent-initiated turn with no run label; it is a billed turn, and the live lane owns that P1 candidate.
   - NOT-SENT: `settings.thinking` is now an unknown key that cyril still sends (`kas/settings.rs:79,96`); outputStyle (prompt `_meta.kiro.outputStyle`) is never sent.
   - DROPPED: model `rateMultiplier`.
   - Opt-in only, NOT-SENT and NONE for parity: the `diagrams` and `agentHostsForegroundShell` handshake keys.
   - Not reachable in cyril today (cyril-8wh0, P3): the creator-as-step workflow plan approval.
6. Ranked flip blockers at HEAD: cyril-zbu7 (P1), cyril-k8wc (P1, new), cyril-auxt (P1, re-scope), then cyril-ebqu with cyril-1v25 (P2: sub-agent frames interleave into the main transcript, and the sub-agent's answer is dropped as an orphan update).

## Matrix 1 — Handshake `initialize.clientCapabilities` (`_meta.kiro`, `.settings`, `fs._meta.kiro`)

cyril builder unchanged since 2.31.1: `protocol/engine.rs:97-136` emits `fs{readTextFile,writeTextFile,_meta.kiro{readFile,writeFile,stat,readDirectory,delete}}`,
`terminal:true`, `_meta.kiro.settings` (`protocol/kas/settings.rs` marshal) and `_meta.kiro.hooks` (`{enabled}` Inbound / `{enabled,v2}` Outbound).
KAS 0.66.43 capability resolver `fGr` adds `diagrams:n.diagrams===!0`; clientMeta adds `agentHostsForegroundShell` (kas lane; counts `diagrams:n.diagrams` 0 -> 1,
`agentHostsForegroundShell` 0 -> 4). TUI 2.32.0 clientMeta region rename-identical to 2.31.1 (tui lane): it sends neither new key (`agentHostsForegroundShell` 0; the one `diagrams` hit is Mermaid copy).

### 1a. Top-level `_meta.kiro` keys

| key | KAS 0.66.43 | TUI 2.32.0 | cyril @HEAD | consequence on cyril-on-KAS | issue | flip |
|---|---|---|---|---|---|---|
| `diagrams` **(NEW 0.66.43)** | registers tool `create_diagram`; tool_call `_meta.kiro.{toolId,diagram{id,sessionId,schemaVersion},diagramError}` | **not sent** | NOT-SENT (`protocol/engine.rs:97-136`); tool `_meta` would be DROPPED anyway (`protocol/convert/mod.rs:182-265` reads no `_meta`) | none vs TUI (the tool stays off for Kiro's own client too) | kas lane NEW P3 (dark surface) | NONE |
| `agentHostsForegroundShell` **(NEW 0.66.43)** | `Sjr(...)` returns host `"agent"` (`client_override`): KAS runs foreground commands itself even with `terminal:true` | not sent (TUI hosts no terminals) | NOT-SENT | none. Note: opting in would also sidestep the shell_type gap (KAS would use its own detected dialect), at the cost of cyril's host role for foreground shell — a design option for cyril-k8wc, not a fix | cyril-7p7n (note, kas lane) | NONE |
| `settings` | consumed (schema `m6n`, see 1b) | sent | HANDLED (`protocol/engine.rs:113-116`) | see 1b | cyril-53qx | — |
| `hooks{enabled,v2?}` | consumed | `{enabled:!0,v2:!0}` | HANDLED (Inbound default = cyril hosts hooks; #168 added tag selectors to that host, `protocol/kas/hooks.rs`) | parity by design | cyril-5nep | NONE |
| `telemetryEnabled`/`telemetry` | = 2.31.1 | sent | NOT-SENT | KAS telemetry default | cyril-zjm5 | FOLLOW-UP P3 |
| `knowledge` | = | `!0` | NOT-SENT | capability not advertised (settings key is) | — | FOLLOW-UP P4 |
| `requirementsAnalysis`, `specPhaseCheckpoints`, `specLinks` | = | sent (2) | NOT-SENT | no spec UI | — | NONE |
| `streamingShellContent` | = (17 = 17) | `!0` | NOT-SENT | no `_kiro/tools/content_chunk` live shell output | cyril-2mo0 | FOLLOW-UP P3 |
| `userInput` | = (`userInputCapability` 6 = 6) | registered capability | NOT-SENT | option-less questions auto-skipped in delegated children | cyril-qo13 (closed) | FOLLOW-UP P3 |
| `secretStorage`, `openExternalUrl` | = | registered | NOT-SENT | remote-MCP OAuth URL never reaches the user; TUI 2.32.0 now also arms `_kiro/mcp/oauthCompleted` off `openExternalUrl{serverName,oauthCompletion}` (KAS 0 hits — dark) | cyril-tqr8, cyril-nk4o | FOLLOW-UP P2 |
| `authCallback`, `largeToolOutputHandler`, `workflowNotifications` | 0 hits (dead) | TUI sends dead keys | NOT-SENT | none | — | NONE |
| `infrastructureSafety`, `configurationState`, `inactiveItems`, `c2sViews`, `textSearch`, `findFiles`, `helpDocs`, `notifications.documentsChanged`, `resolvesSteeringCommands`, `clientTool*` | = | not sent (env-gated infraSafety) | NOT-SENT | none (TUI parity); `configurationState` would surface the new "session-reasoning" config rows | cyril-iq2c, cyril-3ald | NONE |
| `terminalInput` | = (7 = 7) | not sent (TUI hosts no terminals) | NOT-SENT while cyril hosts terminals | no stdin for interactive/background processes | cyril-7p7n | FOLLOW-UP P3 |
| `fs.*` + `fs._meta.kiro.*` + `terminal` | = | not advertised | HANDLED (`protocol/engine.rs:101-110`) | cyril is the fs/terminal host (deliberate divergence) | — | NONE |

### 1b. `_meta.kiro.settings` (AgentSettings) — KAS schema `G2n` (0.66.34) -> `m6n` (0.66.43)

| key | KAS 0.66.43 | TUI sends | cyril sends (`protocol/kas/settings.rs`) | consequence | issue | flip |
|---|---|---|---|---|---|---|
| `thinking` **(REMOVED 0.66.43)** | schema and intake row gone; client value logs `configuration.intake.unknown-key` (trap 23). Thinking/effort moved to the "session-reasoning" config source; `thinking` row lost its `"off"` default, so the model's `defaultThinkingEnabled` decides | TUI settings builder rows unchanged (tui lane) | SENT: `chat.enableThinking -> thinking` (`:79`) and default-on `thinking` (`:96`) | `chat.enableThinking:false` in cli.json no longer turns thinking off at session start; the always-on default no longer forces it on. `/thinking on|off` still works (configId `thinking`, `commands/builtin.rs:509-576`) | cyril-53qx (kas lane note) | FOLLOW-UP P2 |
| `workflows.creatorEffort` **(NEW)** | optional, inside `workflows{enabled}` | when available | NOT-SENT by design (ADR-0011; `workflows` never marshaled, `protocol/kas/settings.rs` test `workflows_gate_never_marshaled`) | none (creator step not reachable gate-off) | cyril-0asq | NONE |
| `evalStrategy`/`evalTreatments` **(NEW, hidden known-extras)** | pin a backend A/B treatment (`x-kiro-eval-*` headers) | no | NOT-SENT | none | — | NONE |
| `subagentOrchestration` | 0 hits (0.66.34 and 0.66.43) | no | SENT ALWAYS (`:97`) | dead key every handshake; the doc comment above `DEFAULTS_ON` still says it "selects the `orchestrate_subagent` tool" (tool gone since 0.66.33) | cyril-53qx | FOLLOW-UP P2 |
| `todoList` | 0 hits | no | SENT when `chat.enableTodoList` is boolean (`:82`) | dead key | cyril-53qx | FOLLOW-UP P2 |
| `toolSearch.enabled` (+`minPct`,`minTokens`) / `neverDefer` | = 2.31.1 (accepted, `enabled` unconsumed; `neverDefer` consumed) | yes | SENT `:120-131` / NOT-SENT | toggle does nothing; no eager-MCP control | cyril-53qx, cyril-q8gq | FOLLOW-UP P2/P3 |
| `memory{mode,reflection}` | = (default `{disabled,false}`, unchanged 1 -> 1) | yes | NOT-SENT | KAS native memory off; LIVE(live lane leg 1): `memoryConfig {disabled,false}` on both engines | cyril-0na7 | FOLLOW-UP P3 |
| other accepted keys (`knowledge`, `codeIntelligence`, `checkpoint`, `tangentMode`, `disableAutoCompaction`, `_subagent`, `_delegate`, `compaction`, `semanticReview`, `fta`, …) | = | partly | HANDLED or NOT-SENT as in 2.31.1 | = | — | NONE |

## Matrix 2 — Agent capabilities read (`initialize` result)

LIVE(live lane leg 0): the 0.66.34 -> 0.66.43 initialize result is IDENTICAL (normalised): `extensionMethods` 28 = 28, `authMethods`, all `_meta.kiro` caps.
cyril reads only "is `agentCapabilities._meta.kiro` an object" (`protocol/fingerprint.rs:30-36`); every key below has 0 non-test reads.
New this cycle: **Kiro's own host binary is now a second consumer** of `executionTargets` / `sourceProviders` (host lane: non-interactive `chat --cloud` gate, `--repo` -> `_kiro/sourceProviders/list`).

| key | KAS | TUI reads | Kiro host reads (NEW 2.32.0) | cyril | consequence | issue | flip |
|---|---|---|---|---|---|---|---|
| `executionTargets` | `["local"]` (standalone) | yes (cloud gating) | **yes**: "--cloud is not available: the agent did not advertise cloud sandbox sessions…" | DROPPED | no cloud sessions (cyril sends no `session/new _meta.kiro.executionTarget`) | cyril-tikf (host lane note) | NONE |
| `sourceProviders` | `false` | yes | **yes** (`--repo` gate + `_kiro/sourceProviders/list`) | DROPPED | none until cloud | cyril-tikf | NONE |
| `extensionMethods` | 28 (unchanged) | yes (gates `/code`, workflow control, config/template) | — | DROPPED | no feature detection | cyril-tikf | FOLLOW-UP P3 |
| `replayMarking` | true | yes | — | DROPPED | replayed frames indistinguishable on `session/load` | cyril-99ds | FOLLOW-UP P3 |
| `modelFallbackReporting` | true | 0 | — | DROPPED | none by itself | cyril-aoby | NONE |
| `sessionSources`, `sessionListScopes`, `strictSessionLoad`, `checkpoints`, `sessionList`, `policyNotifications`, `logging` | = | as 2.31.1 | — | DROPPED | none | — | NONE |
| `sessionCapabilities.{list,close,delete,fork}` | = | close/list/fork | — | DROPPED | Matrix 9 | cyril-wigv | FOLLOW-UP |
| `promptCapabilities` | = | SDK parse | — | DROPPED | none | — | NONE |

## Matrix 3 — Client -> agent methods

KAS dispatch set unchanged (`static-v3cov-methods-2.32.0.py`: 123 `_kiro/*` literals, identical direction classes; kas lane: handlers token-identical except
workflow internals). cyril's non-test coverage of the 123 literals is byte-for-byte the 2.31.1 set (tmp/methods.json). TUI 2.32.0 adds no client->agent
call (its one new method is the inbound `_kiro/mcp/oauthCompleted`).

| method | TUI route | cyril @HEAD | consequence | issue | flip |
|---|---|---|---|---|---|
| `initialize`, `session/new`, `session/prompt`, `session/cancel`, `session/load` | always | HANDLED (`protocol/domain_mediator/commands/session.rs:50-60,174-195,475`) | prompt carries no `_meta` (`session.rs:475`): no `outputStyle`, no `displayText` | — / see M6 | NONE |
| `session/set_config_option` | `/model`, `/agent`, `/plan`, `/effort`, `/memories`, autopilot, sandbox, thinking | **HANDLED for model/mode/effortLevel/thinking** (CHANGED: #171 `commands/mod.rs:497-506` registers `/model` `/effort` `/mode` on KAS; App confirm sends one `SetConfigOption`, `crates/cyril/src/app.rs:2051-2060`; values revalidated against the catalog, `types/kas_config.rs:267`) | 2.31.1 blocker #3 closed; `autopilot`, `memoryReflection`, `outputStyle` still have no producer | cyril-vh69 (closed) | NONE (residuals FOLLOW-UP, M6) |
| advertised skill / prompt / custom-agent commands | TUI sends `/name args` as prompt text | **HANDLED** (CHANGED: #169 `protocol/convert/kas.rs:357-421` classifies `_meta.kiro.type` ∈ {custom-agent, skill, prompt} as `AgentCommandRoute::Prompt`; `commands/mod.rs:624` submits the original text as a prompt; unknown/missing types suppressed with a log). LIVE per cyril-oieu note 2026-10-18 (real cyril TUI on installed 2.32.0: skill, file prompt and custom agent all ran) | 2.31.1 blocker #2 closed | cyril-oieu (in_progress, awaiting closure) | NONE |
| `_kiro.dev/commands/execute`/`options` on KAS | — | no longer reachable from KAS advertisements (KAS v2 command lists dropped, `protocol/convert/kas.rs:427-436`); the `BridgeExecute` route survives for v2 only | none | cyril-oieu | NONE |
| `_session/spawn`, `_kiro.dev/session/terminate`, `_message/send` (`/spawn` `/kill` `/msg`) | — (KAS 0 hits) | BROKEN on KAS = 2.31.1: still registered on every engine (`commands/mod.rs:488-491`, `commands/subagent.rs`) | three commands error; `/sessions` always empty | cyril-a8se | FOLLOW-UP P3 |
| `session/list`, `session/fork`, `session/close`, `session/delete`, `_kiro/session/{delete,rename,history}` | `/chat`, `/rewind`, dashboard | NOT-SENT = 2.31.1 | no session browser, no rewind | cyril-wigv, cyril-a3th, cyril-1aq0 | FOLLOW-UP P3/P4 |
| `_kiro/session/context`, `_kiro/session/compact` | `/context`, `/compact` | NOT-SENT = 2.31.1 | v2 `/context` `/compact` lost | cyril-1aq0 | FOLLOW-UP P2 |
| `_kiro/codeIntelligence`, `_kiro/knowledge`, `_kiro/mcp/resetServer`, `_kiro/config/template`, `_kiro/sandbox/applyConfig`, `_kiro/spec/*`, `_kiro/workflow/pause` | as 2.31.1 | NOT-SENT | as 2.31.1; `config/template` is now also the only source of the 5 output styles (M6) | cyril-1aq0, cyril-nk4o, cyril-lyrx, cyril-6vo6, cyril-4u4a | FOLLOW-UP P3/P4 |
| `_kiro/account/getUsage`, `_session/steer[/clear]`, `_kiro/hooks/{list,setEnabled}` (kas-hooks mode), `_kiro/workflow/{new,invoke,load,inspect,list,listRecipes,cancel,resume,retry}` | as 2.31.1 | HANDLED (`protocol/domain_mediator/commands/extensions.rs:262`, `protocol/domain_mediator/commands/kas.rs:308-355`) | — | — | NONE |
| `_kiro/workflow/new` with `workflowPrompt` | TUI `/workflow new` = canned prompt (byte-identical) | not sent; cyril launches only `{workflowPath | inline workflow}` (ADR-0011) | the 0.66.43 creator-as-step flow is unreachable in cyril (see M4c) | cyril-0asq, cyril-8wh0 | NONE |
| TUI-0 C2A methods (`_kiro/checkpoint/*`, `configuration/*`, `memory/{create,update,delete}`, `permissions/*`, `policy/check`, `workflow/{delete,update,resumeAll,listWatchHandlers}`, …) | not called | NOT-SENT | none | cyril-iq2c | NONE |

## Matrix 4 — Agent -> client: sessionUpdate variants, session_info_update kinds, `_kiro/*` notifications

### 4a. `sessionUpdate` variants (cyril `protocol/convert/mod.rs:460-545`, KAS override `protocol/engine.rs:330-345`)

KAS variant set unchanged (kas lane). TUI `Det` mapper unchanged (tui lane).

| variant | KAS | TUI | cyril @HEAD | consequence | issue | flip |
|---|---|---|---|---|---|---|
| `agent_message_chunk` / `agent_thought_chunk` / `user_message_chunk` | = (`agentSubtaskId` 20 = 20) | routes `agentSubtaskId`-tagged chunks to a sub-agent lane (TUI `agentSubtaskId` 41 = 41) | HANDLED `:460-488`, but `agentSubtaskId` has 0 hits in `crates/` | sub-agent text interleaves with the main agent's | cyril-ebqu | **BLOCKS-FLIP (P2)** |
| `user_message_chunk` with `_meta.kiro.notification.{bufferOnly,callerSessionId}` **(NEW 0.66.43)** | buffered step-verdict messages persisted and replayed as user chunks | 0 reads | DROPPED (keys unread); the text renders as a user message on replay | LIVE(live lane leg 2a): present on replayed step-verdict chunks; a buffered engine message shows as if the user typed it | cyril-58uv (note) | FOLLOW-UP P3 |
| `user_message_chunk` with `_meta.kiro.syntheticUserMessageReason:"max-tokens-recovery"` **(NEW enum value)** | 1 automatic re-invoke when reasoning eats the output budget | not read | DROPPED; on `session/load` replay the nudge would render as a user message | cosmetic, replay only | — (live lead KAS-L7) | FOLLOW-UP P4 |
| `tool_call` / `tool_call_update` | = ; new `_meta.kiro.{toolId,diagram,diagramError}` only behind `diagrams` | GY sub-agent routing | HANDLED generically (`to_tool_call` `protocol/convert/mod.rs:182`, `to_tool_call_update` `:233`, no `_meta` read) | child tool calls render as the main agent's; MCP calls show the `tool_load` wrapper | cyril-ebqu, cyril-2yn8 | BLOCKS-FLIP (with row 1) / FOLLOW-UP P2 |
| orphan `tool_call_update` ("Tool Load", "Subagent Response") | = | rendered | **DROPPED silently**: `crates/cyril-ui/src/state.rs:560-575` updates only ids seen in a start, returns `true` | the sub-agent's answer row never appears | cyril-1v25 | **BLOCKS-FLIP (P2, with ebqu)** |
| `available_commands_update` | = | split by `_meta.kiro.type` | **HANDLED** (CHANGED, #169: typed route, `protocol/convert/kas.rs:361-421`; registry replaced per advertisement) | — | cyril-oieu | NONE |
| `config_option_update` | = (model options `_meta.kiro.{hasEffort,effortLevels,defaultEffortLevel,effortSchemaPath,rateMultiplier,rateUnit,thinkingToggleable}`) | config tracker + model picker sort/credit column (NEW TUI 2.32.0) | **HANDLED** (CHANGED, #171): wholesale `KasConfigSnapshot` (`session.rs:363-376`), toolbar model + mode + effort (`crates/cyril-ui/src/state.rs:1063-1091`, `crates/cyril-ui/src/widgets/toolbar.rs:54-75`); `rateMultiplier`/`rateUnit` DROPPED (`protocol/convert/kas.rs:108` reads effort keys only) | no credit multiplier in cyril's picker | cyril-lxuo | FOLLOW-UP P4 |
| `current_mode_update` | = | ignored | HANDLED `:515` | — | — | NONE |
| `session_info_update` | = | `jee` | 8/25 (4b) | — | cyril-58uv | — |
| `plan`, `usage_update` (schema-only), `plan_update`, `plan_removed` | no emitter | not mapped | as 2.31.1 | none | — | NONE |

### 4b. `session_info_update._meta.kiro.kind` (`static-v3cov-kinds-2.32.0.py` -> tmp/kinds.json)

Kind set 25 = 25 (0.66.34 -> 0.66.43). Literal counts moved only for `turn_start` 22 -> 23, `queued` 37 -> 39, `pending_interaction` 11 -> 12,
`interaction_resolved` 13 -> 15 (new emit sites; no new kind). cyril dispatch `protocol/convert/kas.rs:487-661`; everything else falls to
`_ => None` at `protocol/convert/kas.rs:660` **with no log** (cyril-58uv, still open P1).

| kind | TUI consumes | cyril @HEAD | consequence | issue | flip |
|---|---|---|---|---|---|
| `turn_start`, `turn_end`, `turn_completion`, `context_usage`, `steering_queued/injected/cleared`, `hook_update` | yes (hook_update: no) | HANDLED (unchanged) | `turn_end.stopReason` `error`/`content_filtered` still default to EndTurn (`protocol/convert/kas.rs:852-875`) | cyril-pyko | FOLLOW-UP P3 |
| `pending_interaction` | no | DROPPED. **New payload**: may carry `_meta.kiro.workflowPlan` (creator plan approval) | unreachable in cyril (ADR-0011: creator step needs gate-on `run_workflow`/`update_workflow`) | cyril-8wh0 (P3, blocked on cyril-0asq) | NONE |
| `model_fallback` / `model_fallback_promoted` | yes | DROPPED unlogged | silent model change | cyril-aoby | FOLLOW-UP P2 |
| `summarization_started/completed/failed` | yes (`compaction_status`) | DROPPED unlogged | auto-compaction invisible (v2 handled `kiro.dev/compaction/status`) | cyril-58uv (umbrella; no dedicated issue) | FOLLOW-UP P2 |
| `display_error` | yes | DROPPED unlogged | MCP connect / OAuth-needed errors lost | cyril-tqr8, cyril-58uv | FOLLOW-UP P2 |
| `user_message_id_assigned` | yes (rewind anchor) | DROPPED | blocks `/rewind` | cyril-1aq0 | FOLLOW-UP P3 |
| `interaction_resolved` | yes | DROPPED | permission resolved elsewhere leaves the dialog open | cyril-99ds | FOLLOW-UP P3 |
| `focus_update`, `model_routed`, `checkpoint_restore_availability`, `repositories_update`, `steering_inclusion`, `queued`, `summarization_separator`, `summary_message`, `recap` | no (repositories via key) | DROPPED | none vs TUI | cyril-otpi, cyril-0o7e, cyril-ojum, cyril-fb1m | NONE |

### 4c. `_kiro/*` agent -> client notifications

Fallback = debug-logged "extension notification not forwarded" (`protocol/convert/kiro.rs:1284` -> `protocol/domain_mediator/inbound.rs`).
Workflow reachability per ADR-0011 (orchestrator note): cyril never sets `settings.workflows.enabled`, so KAS registers no model-facing workflow
tools; cyril-launched runs (`_kiro/workflow/new {workflowPath | inline workflow}`) still emit every lifecycle event below.

| notification | TUI | cyril @HEAD | reachable on cyril (gate-off)? | consequence | issue | flip |
|---|---|---|---|---|---|---|
| `_kiro/workflow/{run_start,node_start,node_complete,node_paused,loop_iteration,watch_poll,paused,run_complete,steps_queued}` | yes | HANDLED (`protocol/convert/kas/workflow.rs:481-489`) | yes | — | — | NONE |
| **`attention[]` on every lifecycle event, run state, list rows, `finalState`, node level (NEW 0.66.43)** | 0 reads (TUI ignores it too) | DROPPED: no field on `WireRunCompleted` (`protocol/convert/kas/workflow.rs:202`), `WireSnapshot` (`:377`), `WireRunListEntry` (`:542`), `WorkflowRun` (`workflow.rs:294-316`) | **yes** — LIVE(live lane legs 2a/2b): present on every event, `[]` when nothing waits; populated `{kind:"question",since,nodeId,nodePath,sessionId}` on a parked step | `/workflow` cannot say which step waits on a permission or question — the 20x-prompt workflow UX (cyril-w55l/gn07 history) could key on it | NEW-candidate: kas lane proposed NEW P2 or NOTE cyril-4u4a — reference theirs; this lane rates it P3 (TUI parity: TUI also ignores it) | FOLLOW-UP P3 |
| `_kiro/workflow/inspect` reply `pendingSteps` (NEW) | 0 | DROPPED (`WireStateReply` `:216`) | yes | pending work already arrives via `steps_queued` (HANDLED) | cyril-4u4a (note) | FOLLOW-UP P4 |
| `steps_queued` re-emitted on restore (NEW emit sites) | yes | HANDLED (resolution-free frame replaces pending steps, `workflow.rs:746-760`) | yes | — | — | NONE |
| paused `run_complete` -> `_kiro/session/notify{severity:"info"}` -> **parent auto-wake turn `send-message-wake` (NEW 0.66.43, repeat cap)** | TUI gets the same wake (engine behaviour) | HANDLED as a generic agent-initiated turn: header "agent-initiated · send-message-wake" (`crates/cyril-ui/src/turn_labels.rs`, `protocol/turn_mediator.rs:1297-1323`); no run label, since wake labels queue only for terminal statuses (`workflow.rs:836-849`); `_kiro/session/notify` itself DROPPED | **yes** — any cyril-launched recipe whose repeat has `onMaxIterations:"pause"`; bundled `feature-pipeline` loops became `maxIterations:2,onMaxIterations:"pause"` in 0.66.43. LIVE(live lane leg 2a, 2b): billed parent turn with a `read_file` tool after the pause | an unsolicited billed turn the user did not start, asking about a run cyril cannot name | live lane (P1 candidate, extends cyril-lki9) — reference | FOLLOW-UP (P1-candidate owned by live lane) |
| plan preview `listRecipes[].plan`, sequence `includedFrom` | TUI 2.32.0 renders a pre-launch plan preview | DROPPED (`WireRecipe` `:611`, `Sequence` `:341`) | yes (data is in replies cyril already gets) | no `/workflow run` confirmation preview | cyril-4u4a, cyril-inwg (tui lane notes) | FOLLOW-UP P3 |
| pause texts reworded ("…; resuming the run retries the step.") | string-matched | HANDLED verbatim (no string matching) | yes | — | — | NONE |
| `_kiro/workflow/{node_failed,recipes_changed}` | = | DROPPED | yes | recipe list stale | cyril-4u4a | FOLLOW-UP P4 |
| `_kiro/mcp/status`, `_kiro/mcp/governance_disabled`, `_kiro/governance/state` | yes; TUI 2.32.0 adds per-server OAuth notice lifecycle on `mcp/status` | DROPPED | yes | MCP state/OAuth/governance invisible | cyril-nk4o, cyril-tqr8, cyril-0o7e | FOLLOW-UP P2 |
| `_kiro/mcp/oauthCompleted {serverName,success}` **(NEW in TUI 2.32.0; KAS 0 hits — dark)** | yes | would be DROPPED (`protocol/convert/kiro.rs:1284`) | no (KAS never sends it) | none today | cyril-nk4o (tui lane note) | NONE |
| `_kiro/customAgent/{not_found,config_error}` | not_found yes | DROPPED (v2 twins handled) | yes | silent fallback agent | (2.31.1 proposed NEW P3; not filed — grep found none) | FOLLOW-UP P3 |
| `_kiro/error/rate_limit`, `_kiro/system/notify`, `_kiro/powers/items_changed`, `_kiro/hooks/{cancel,didChange}` | as 2.31.1 | HANDLED (`protocol/convert/kiro.rs:1075,1086`, `protocol/convert/kas/powers.rs:48`, `protocol/kas/hooks.rs:35,39`) | yes | — | — | NONE |
| `_kiro/session/notify` (now also `severity:"success"` handoff-restored, finished-step `info`) | 0 | DROPPED | yes | the wake text is visible only through the woken turn | cyril-58uv | FOLLOW-UP P3 |
| `_kiro/tools/didChange`, `_kiro/tools/content_chunk`, `_kiro/steering/documents_changed`, `_kiro/sessions/changed`, `_kiro/sandbox/status`, `_kiro/spec/*`, remaining TUI-0 notifications | as 2.31.1 | DROPPED / never sent | — | as 2.31.1 | cyril-58uv, cyril-2mo0, cyril-6vo6, cyril-mxc7 | FOLLOW-UP P3/P4 / NONE |

## Matrix 5 — Host callbacks KAS calls into the client

Responders: typed ACP handlers + `_`-request dispatcher `protocol/domain_mediator/host.rs:44-190`; any other `_`-request is answered JSON `null`
(`protocol/domain_mediator/host.rs:182`). Callback set unchanged in 0.66.43 (kas lane).

| callback | KAS gate | TUI | cyril @HEAD | consequence | issue | flip |
|---|---|---|---|---|---|---|
| `_kiro/terminal/shell_type` | `terminal:true` | not called (no terminals) | HANDLED but **wrong value**: `{"shellType":"posix"}` for every bash/zsh/sh/dash/ksh host (`protocol/kas/terminal_io.rs:283-286` -> `protocol/kas/host_shell.rs:166-171`; classifier `protocol/kas/host_shell.rs:459-464` folds them into `ShellKind::Posix`). KAS `jh()` accepts only concrete names (`bash`,`zsh`,`sh`,`dash`,`ksh`,`fish`,`pwsh`,`powershell`,`cmd`,…; quote `t==="bash"||…||t==="csh"?"posix":"unknown"`), so `"posix"` -> `"unknown"` (warn `policy.shell.type.unknown`); `"fish"` and `"powershell"` map correctly. 0.66.43 now also feeds this dialect into execute_bash approvals (`InterpreterChangedError` on mismatch) | on POSIX hosts (Linux/macOS default) every shell command not denied asks; neither in-session "Always allow" nor V3 allow rules can apply. Probe drivers answered `"bash"`, so no capture shows cyril's value; live KAS-L1 decides | **cyril-k8wc (P1, filed)** — reference | **BLOCKS-FLIP** |
| `terminal/{create,output,wait_for_exit,release,kill}`, `fs/{read,write}_text_file`, `_kiro/fs/*` | advertised caps | no | HANDLED = 2.31.1 (`protocol/kas/kiro_fs.rs:85-89`, `protocol/kas/terminal_io.rs`) | — | — | NONE |
| `_kiro/auth/getAccessToken` | argv `--auth=acp-callback` | yes | HANDLED but never renews (`protocol/kas/auth.rs:210-291`) | session dies ~1 h after the last kiro-cli refresh | cyril-zbu7, cyril-taba | **BLOCKS-FLIP** (M8) |
| `_kiro/hooks/{list,executeHook,sessionStart}` | `hooks.enabled` (no v2) | no (TUI lets KAS run hooks) | HANDLED (Inbound default; #168 enforces tag selectors, `protocol/kas/hooks.rs`) | — | cyril-5nep | NONE |
| `_kiro/openExternalUrl` — **params gain `serverName`, `oauthCompletion` in the TUI 2.32.0 responder** (KAS 0.66.43: 0 hits for either, it still sends `{url}`) | `openExternalUrl` cap | yes (per-server OAuth notice) | NOT-SENT (cap not advertised -> never called) | remote-MCP OAuth never reaches the user | cyril-tqr8 | FOLLOW-UP P2 |
| `_kiro/userInput`, `_kiro/secret/*`, `_kiro/terminal/write` | caps | yes / yes / no | NOT-SENT | as 2.31.1 | cyril-qo13, cyril-tqr8, cyril-7p7n | FOLLOW-UP P3 |
| `_kiro/mcp/elicitation`, `_kiro/workspace/{active_file,currently_open_files}` | ungated | no handler | answered `null` -> KAS cancels / errors (TUI equivalent) | none vs TUI | — | NONE |
| `session/request_permission` request | always | yes | HANDLED; consent read (`protocol/convert/kas.rs:24-57`). **New 0.66.43 payload**: `toolCall.content` Text + `_meta.kiro.workflowPlan` for creator plan approvals; approval preview renders only Diff or `raw_input.text/content`, else "Preview unavailable" (`crates/cyril-ui/src/widgets/approval.rs:80-131`) | plan approvals unreachable in cyril (ADR-0011) | cyril-8wh0 (P3, blocked on cyril-0asq) | NONE |
| `session/request_permission` **response `_meta.kiro`** | KAS reads `consent{scope,resource,workspaceRoot}` (`S8e`: `scope:w?.scope||"session"`, `resource:w?.resource??n?.triggeringResource??n?.resource`), `rejectionReason`, `editedCommand` | **CORRECTION to the 2.31.1 row:** the TUI sends `{kiro:{consent:{capability, scope, resource?, workspaceRoot?}, rejectionReason?}}` on every consent-bearing response (2.31.1 `ew()`, 2.32.0 `cw()`; scope defaults `"session"` for allow_always, `"invocation"` otherwise). 2.32.0 adds the folder-glob `resource:"<dir>/**"` for fs_write grants. `editedCommand`: TUI 0 hits | sends `rejectionReason` only (`protocol/convert/kas.rs:68-92`, built at `protocol/convert/mod.rs:431`) | without the echo every allow_always lands at **session scope** (KAS default) and is never persisted. With `scope:"user"`/`"workspace"`, KAS `persistConsent` writes the rule itself into `<userConfigDir>/settings/permissions.{yaml,json}` or the workspace permissions file and rebuilds policy. This is the KAS-native fix for cyril-auxt (trap 26). Folder grants (2.32.0) need the same echo | cyril-auxt, cyril-gn07, cyril-k8wc | **BLOCKS-FLIP** (via auxt) |

## Matrix 6 — configOptions / modes / models / effort / outputStyle

KAS offers (LIVE(live lane leg 1), NEW engine): `session/new` configOptions = mode(7) · model(auto + 20) · memoryReflection(off) · autopilot(on) ·
contentCollection(disabled). `outputStyle` is **not** a session configOption on either engine: it appears only in `_kiro/config/template` and is
chosen per prompt with `session/prompt _meta.kiro.outputStyle` (`promptScoped:true`). `set_config_option outputStyle` is accepted silently and not echoed.
cyril (#171): `KasConfigId {Model, Mode, Effort}` (`types/kas_config.rs:20-58`); pickers `commands/builtin.rs:588-720`; catalog validation
`types/kas_config.rs:267`; App sends exactly one `SetConfigOption` on confirm (`crates/cyril/src/app.rs:2051-2060`).

| option | KAS 0.66.43 | TUI 2.32.0 | cyril @HEAD | consequence | issue | flip |
|---|---|---|---|---|---|---|
| `model` | select, 21 values; `_meta.kiro` per value incl. `rateMultiplier`/`rateUnit` | `/model` picker; NEW sort last-used/cost/company + "N.NNx credits" column (`chat.modelPicker.*`) | **HANDLED** (CHANGED): `/model` picker, offered-only values (blocks the KAS "unoffered model accepted" trap); `rateMultiplier` DROPPED | no credit column | cyril-vh69 (closed), cyril-lxuo | NONE / FOLLOW-UP P4 |
| `effortLevel` | model-dependent (levels from the model value's `_meta.kiro.effortLevels`) | `/effort` | **HANDLED** (CHANGED): `/effort` + toolbar effort badge from the `effortLevel` currentValue (`crates/cyril-ui/src/state.rs:1063-1091`) | — | cyril-vh69 | NONE |
| `mode` | 7 bundled + custom agents | `/agent`, `/plan`, `/autonomous` -> `setConfigOption("mode",…)` | **HANDLED** (CHANGED): `/mode` picker (offered-only; KAS coerces unknown modes to vibe) | TUI's `/agent` create/edit agent files has no cyril twin | cyril-vh69 | NONE |
| `thinking` | model-dependent; AgentSettings `thinking` removed (M1b) | model settings pane | HANDLED `/thinking on|off` (configId `thinking`) | startup cli.json `chat.enableThinking` no longer applies | cyril-53qx | FOLLOW-UP P2 |
| cold-catalog pin on resume (NEW) | `session/load` result carries the saved model option before the catalog loads; one-option select only when the catalog is cold | — | HANDLED: load-result `config_options` -> `ConfigOptionsUpdated` (`protocol/domain_mediator/commands/session.rs:182-195`). LIVE(live lane leg 1): NEW load result already carries the full 21-value model option; OLD sent it later in a `config_option_update`. A truly cold one-option pin would offer one row until the full update (STATIC, `types/kas_config.rs:190-262`) | none in practice | — (live lead KAS-L4) | NONE |
| `outputStyle` | `_kiro/config/template` only; options `[default, concise]` -> `[default, proactive, concise, explanatory, learning]` (LIVE leg 1) | `/settings display` -> per-prompt `_meta.kiro.outputStyle` | NOT-SENT: no `outputStyle` in `crates/`; prompt has no `_meta` (`protocol/domain_mediator/commands/session.rs:475`); cyril never calls `_kiro/config/template` | cyril users get the default style only; not a v2 loss (v2 had no styles) | tui lane NEW P2 (reference; this lane would rate P3 opt-in) + cyril-lyrx | FOLLOW-UP P3 |
| `autopilot` | `on` default | toggle | DROPPED / NOT-SETTABLE (`KasConfigId::from_wire` ignores it) | cannot turn autopilot off | (fold into a vh69 follow-up) | FOLLOW-UP P3 |
| `memoryReflection`, `contentCollection` | off / disabled | `/memories` / governance | DROPPED | none (cyril-memory path) | cyril-0na7 | NONE |
| `sandbox`, `mcpSandboxing`, `sandboxNetworkMode` | local sandbox only | `/sandbox`, `/autonomous` | NOT-SENT | no sandbox | cyril-6vo6 | NONE (P3 opt-in) |

## Matrix 7 — Slash commands (TUI 2.32.0: 74 literals, set-identical to 2.31.1; 37 registered)

TUI routing unchanged (tui lane: no slash literal or description change; the KAS `executeCommand` emulator still ends
`/${n} is not yet supported in KAS mode` and still maps `usage`, `context`, `compact`, `rewind`, `code`). Classes: **L** = TUI-local, **K** = KAS-method-backed,
**v2** = only through v2 `kiro.dev/commands/*`. cyril builtins @HEAD: `help clear quit new load steer voice usage memory theme thinking sessions spawn kill msg powers`
+ on KAS `model effort mode workflow review` (+ `hooks` in kas-hooks mode) (`commands/mod.rs:482-540`) + every KAS-advertised skill/prompt/custom-agent as a
prompt-routed agent command (#169).

| command | TUI class / route on KAS | cyril on KAS @HEAD | change vs 2.31.1 | gap / issue | flip |
|---|---|---|---|---|---|
| skills / saved prompts / custom agents (`/name args`) | K: prompt text | **HANDLED** (prompt route) | **FIXED** (was BROKEN S1) | — cyril-oieu | NONE |
| `/model` | K: `set_config_option model` | **HANDLED** builtin | **FIXED** | credit column P4 (cyril-lxuo) | NONE |
| `/effort` | K: `set_config_option effortLevel` | **HANDLED** builtin | **FIXED** | — | NONE |
| `/agent`, `/plan`, `/autonomous` | K: `set_config_option mode` (+ agent file editor for `/agent`) | `/mode` builtin covers the switch | **FIXED** (switch) | no `/agent` create/edit, no `/plan` shortcut alias (P4) | NONE |
| `/help`, `/quit`, `/exit`, `/clear` | L / `/clear` = new session in TUI | builtins (`/clear` local only; `/new` = session/new) | = | semantic difference only | NONE |
| `/usage` | K: `_kiro/account/getUsage` | HANDLED builtin | = | — | NONE |
| `/context` | K: `_kiro/session/context` | missing | = | cyril-1aq0 | FOLLOW-UP P2 |
| `/compact` | K: `_kiro/session/compact` | missing | = | cyril-1aq0 | FOLLOW-UP P2 |
| `/tools` | L panel fed by `_kiro/tools/didChange` | missing | = | cyril-1aq0 / cyril-58uv | FOLLOW-UP P3 |
| `/mcp` | L panel on `_kiro/mcp/status` + K `_kiro/mcp/resetServer`; 2.32.0 OAuth notice lifecycle | missing | = | cyril-nk4o, cyril-tqr8 | FOLLOW-UP P2 |
| `/knowledge`, `/code` | K: `_kiro/knowledge`, `_kiro/codeIntelligence` | missing | = | cyril-1aq0 | FOLLOW-UP P3 |
| `/rewind` | K: `session/fork{messageId,createdReason:"rewind"}` | missing | = | cyril-1aq0 | FOLLOW-UP P3 |
| `/prompts` | L list of advertised prompts | missing (prompts are dispatchable, not listable) | partly fixed | autocomplete lists them | FOLLOW-UP P4 |
| `/chat`, `/sessions` | K: `session/list|load|delete`, `_kiro/session/rename`; 2.32.0 `/chat save` zip copy via host helper | `/load <id>` only; cyril `/sessions` = v2 subagent list (empty on KAS) | = | cyril-wigv, cyril-a8se | FOLLOW-UP P3 |
| `/spawn`, `/kill`, `/msg` | v2-only (enum ids) | BROKEN on KAS (v2-only methods) | = | cyril-a8se | FOLLOW-UP P3 |
| `/hooks` | K: `_kiro/hooks/list` | builtin in kas-hooks mode; Inbound default = cyril's own registry (no list command) | = | — | FOLLOW-UP P4 |
| `/memories` | K: `_kiro/memory/*` + `memoryReflection` | cyril `/memory` (cyril-memory, different store) | = | design | NONE |
| `/workflow` family (`/workflows`, `/workflow-run|resume|status|cancel`), `/goal` | K: `_kiro/workflow/*`; 2.32.0 `/workflow run` plan preview from `listRecipes[].plan` | builtin `/workflow`, `/review` HANDLED; `/goal` missing | preview NEW in TUI | cyril-4u4a (plan preview), `/goal` P4 | FOLLOW-UP P3/P4 |
| `/settings` (display -> output style), `/config`, `/verbosity`, `/theme` | L (+ K `_kiro/config/template` for output styles) | `/theme` builtin only | output styles grew to 5 | outputStyle (M6) | FOLLOW-UP P3 |
| `/sandbox`, `/spec`, `/tangent`, `/stats`, `/paste`, `/reply`, `/feedback`, `/upgrade-agent`, `/repo`, `/disconnect`, `/switch`, `/kiro`, `/tab` | L / K (gated) / cloud | `/voice` builtin; rest missing | = | cyril-6vo6, cyril-be7e | FOLLOW-UP P3/P4 / NONE |
| enum-only telemetry ids (`/changelog /checkpoint /context-gatherer /copy /custom /editor /experiment /fullscreen /general-task-execution /guide /issue /kvim /lite /logdump /prompt /reasoning /save /session-id /skill /steering /title /todos /transcript /tui`) | no KAS route | — | = | — | NONE |

## Matrix 8 — v2 features a cyril user loses on the flip

| v2 feature | KAS equivalent | cyril-on-KAS @HEAD | change vs 2.31.1 | issue | flip |
|---|---|---|---|---|---|
| kiro-cli renews its own OIDC token | client answers `_kiro/auth/getAccessToken` (`expiresAt ≥ now+3min`) | serves the stored token, never renews (`protocol/kas/auth.rs:210-291`) | = (open) | cyril-zbu7 (P1), cyril-taba | **BLOCKS-FLIP** |
| "Always allow" persisted (v2 reads `toolsSettings.execute_bash.allowedCommands`) | KAS persists consent itself from the response echo (M5); KAS ignores V2 `allowedCommands` | still writes V2 `allowedCommands` (`kiro_agent_config.rs:156`, `persist_trust_grant`); no echo -> session scope only; on POSIX hosts even session scope is inert (k8wc) | = (open), but **re-scope** to the echo; now blocked by cyril-k8wc | cyril-auxt (P1), cyril-gn07, cyril-k8wc | **BLOCKS-FLIP** |
| skills / prompts / agent commands | prompt text | HANDLED | **FIXED** | cyril-oieu | NONE |
| `/model`, `/agent`, `/effort` | config selects | HANDLED (`/model` `/mode` `/effort`) | **FIXED** | cyril-vh69 | NONE |
| remaining v2 slash surface (`/context /compact /tools /mcp /knowledge /code /rewind /chat /stats /paste /reply /feedback`) | `_kiro/*` per M7 | missing | = | cyril-1aq0 | FOLLOW-UP P2 (`/context`, `/compact`) / P3-P4 |
| crew panel + `/sessions /spawn /kill /msg` | `agentSubtaskId` frames on the parent stream; orphan "Subagent Response" | child frames interleave; answer row dropped; commands BROKEN | = | cyril-ebqu, cyril-1v25, cyril-a8se | **BLOCKS-FLIP (P2)** |
| compaction status | `summarization_*` | DROPPED | = | cyril-58uv | FOLLOW-UP P2 |
| MCP init failure / OAuth request | `_kiro/mcp/status`, `display_error`, `openExternalUrl` | DROPPED / NOT-SENT | = | cyril-nk4o, cyril-tqr8 | FOLLOW-UP P2 |
| effort badge | `effortLevel` option | HANDLED | **FIXED** | cyril-vh69 | NONE |
| startup thinking toggle via cli.json `chat.enableThinking` | v2 settings / KAS `settings.thinking` (removed in 0.66.43) | sent but now an unknown key | **NEW regression (engine-side)** | cyril-53qx | FOLLOW-UP P2 |
| agent not found / config error | `_kiro/customAgent/*` | DROPPED | = | (unfiled) | FOLLOW-UP P3 |
| model fallback | `model_fallback` | DROPPED (v2 also drops) | = | cyril-aoby | FOLLOW-UP P2 |
| refusal detection | refusal object | DROPPED | = | cyril-ker1 | FOLLOW-UP P3 |
| context %, metering, steering echoes, mode switched, rate limit, model-not-found, thinking toggle | KAS kinds / ext | HANDLED | = | — | NONE |

## Matrix 9 — Session lifecycle

| step | KAS 0.66.43 | TUI | cyril @HEAD | consequence | issue | flip |
|---|---|---|---|---|---|---|
| `session/new` request | `_meta.kiro{customAgents,modeId,overrideSessionId,executionTarget,…}` (host 2.32.0 now sends `executionTarget` for `--cloud`) | `overrideSessionId` | `{cwd, mcpServers:[]}` only (`protocol/domain_mediator/commands/session.rs:50-52`) | no cloud target | cyril-tikf | NONE |
| `session/new` response | modes, configOptions, `_meta{title,agentMode,memoryConfig,workflowsEnabled,…}` | full | configOptions HANDLED (#171 snapshot); `_meta` DROPPED | = | cyril-0na7 | NONE |
| `session/load` | replay with `_meta.kiro.replay`; result now carries the saved model option (LIVE leg 1); `_meta.executionTarget{kind:"local"}` (both engines) | buffers replay | HANDLED (`session.rs:174-195`); replay unmarked; `bufferOnly` / `max-tokens-recovery` user chunks replay as user text | = + new replay noise | cyril-99ds | FOLLOW-UP P3 |
| `session/list`, `fork`, `close`, `delete` | = | `/chat`, `/rewind`, step close | NOT-SENT | = | cyril-wigv, cyril-1aq0 | FOLLOW-UP P3/P4 |
| `session/cancel`; turn terminals (`turn_end` first, then prompt response) | = | = | HANDLED (`protocol/engine.rs` `emits_wire_turn_end`) | — | — | NONE |
| `turn_end.stopReason` `error`/`content_filtered`; max-tokens recovery adds no stop reason (`end_turn` 15, `cancelled` 4 unchanged) | = | `PromptFailedError` copy per failure reason (TUI 2.32.0) | non-ACP values -> EndTurn (`protocol/convert/kas.rs:852-875`); error text still reaches the user via the prompt-response error | failed turns summarized as end_turn | cyril-pyko | FOLLOW-UP P3 |
| agent-initiated turns (workflow auto-wake terminal; NEW repeat-cap pause wake) | `turn_start` without a prompt, chunks `agentInitiatedReason` | — | HANDLED (cyril-lki9); pause wake unlabelled (M4c) | billed turn on pause | live lane P1-candidate | FOLLOW-UP |
| **permission response** (allow_always / folder grant) | consent echo -> `persistConsent` (session/user/workspace); default `session` | TUI echoes `consent{capability,scope,resource,workspaceRoot}` (since ≤2.31.1; 2.32.0 adds folder `resource:"<dir>/**"`, main-session, `fs_write`, non-`explicit` only) | `rejectionReason` only (`protocol/convert/kas.rs:68-92`) | grants never outlive the session; no folder grants | cyril-auxt, cyril-gn07 | **BLOCKS-FLIP** (auxt) |
| shell dialect for approvals | `shell_type` reply -> `jh()`; mismatch -> `InterpreterChangedError` | n/a | `"posix"` -> `"unknown"` | every POSIX shell command asks | cyril-k8wc | **BLOCKS-FLIP** |
| first run / extracted bundle / data dir | free spawn needs `kas/<ver>-<hash>/` | host extracts | discovery only (`protocol/kas/discovery.rs`) | tracked in cyril-4c4d acceptance | cyril-4c4d | (in 4c4d) |

## Flip blockers (cyril-4c4d) — what a KAS-by-default cyril user hits on day one

### Status of the 2.31.1 blockers at HEAD 7c7b0d1d

| 2.31.1 # | issue | status at HEAD | evidence |
|---|---|---|---|
| 1 | cyril-zbu7 (token renewal; cyril-taba stopgap) | **OPEN, unchanged** | `protocol/kas/auth.rs:210-291` still refuses a stale token ("kiro token expired; run `kiro-cli login`"); spawn still `--auth=acp-callback` (`protocol/kas/discovery.rs:356`); no commit touched auth since the audit |
| 2 | cyril-oieu (skills/prompts dispatched as `_kiro.dev/commands/execute`) | **FIXED** (ff878634, #169); issue in_progress awaiting explicit closure | `protocol/convert/kas.rs:357-421` (typed `Prompt` route), `commands/mod.rs:624`; LIVE re-qualification on installed 2.32.0 recorded in the issue (skill, file prompt, custom agent all ran) |
| 3 | cyril-vh69 (no model/mode/effort change) | **FIXED** (bb5c03c0, #171; closed 2026-10-16) | `commands/mod.rs:497-506`, `types/kas_config.rs:20-58,267`, `crates/cyril/src/app.rs:2051-2060`; residuals: `rateMultiplier`, `autopilot`, outputStyle (FOLLOW-UP) |
| (P1 list) | cyril-auxt ("Always allow" written to V2 `allowedCommands`) | **OPEN, unchanged code; CHANGED scope**: now blocked by cyril-k8wc, and KAS persists grants itself from the consent echo | `kiro_agent_config.rs:156`; M5 response row; KAS `S8e` / `persistConsent` quotes |
| 4 | cyril-ebqu (+ cyril-1v25) sub-agent interleave / answer dropped | **OPEN, unchanged** | `agentSubtaskId` 0 hits in `crates/`; `crates/cyril-ui/src/state.rs:560-575` |

### Ranked blockers at HEAD (most severe first)

1. **Sessions die about an hour after the last kiro-cli token refresh** (P1, unchanged). Fix: **cyril-zbu7** (spawn via `kiro-cli acp --agent-engine v3 --auth-method cli`) or the cyril-taba stopgap.
2. **Every shell command on a bash/zsh host asks for permission, every time** (P1, NEW to the list; pre-existing defect that 0.66.43 makes more load-bearing). cyril's `shell_type` reply `"posix"` makes KAS classify the shell `"unknown"`, so its shell policy returns `ask` for anything not denied. In-session "Always allow" (KAS default scope `session`) and any V3 allow rule are inert (STATIC; quote of KAS `jh()` in M5). v2 users never saw this: v2 honoured cyril's `allowedCommands`. Fix: **cyril-k8wc** (filed): report the concrete shell basename (`bash`, `zsh`, …). The resolved executable is already in `HostShell`, so this is a small change. Opting into `agentHostsForegroundShell` is a heavier alternative. Live KAS-L1 decides P1.
3. **"Always allow" never survives the session** (P1, cyril-auxt). Re-scope (trap 26): echo `_meta.kiro.consent{capability, scope:"user"|"workspace", resource, workspaceRoot}` on allow_always responses, as Kiro's TUI does. KAS then persists the rule into its own permissions file (`<userConfigDir>/settings/permissions.{yaml,json}` / workspace file) and rebuilds policy, with no agent-JSON writes. That makes cyril-gn07's "optional passthrough" the fix mechanism. Order: k8wc first (otherwise shell grants stay inert), then auxt via the echo. The live lead TUI-L1 (folder grant) tests the persistence leg.
4. **Sub-agent output is mixed into the main transcript, and its answer is dropped** (P2, unchanged). Fix: **cyril-ebqu** (route by `agentSubtaskId`) + **cyril-1v25** (orphan `tool_call_update`). The v2 crew commands are dead on KAS (**cyril-a8se**, P3).

New 2.32.0 items that are NOT day-one blockers:
- The workflow plan approval (cyril-8wh0) is unreachable under ADR-0011.
- Workflow `attention[]` is DROPPED, but Kiro's TUI ignores it too.
- The repeat-cap pause wake is a billed turn only on cyril-launched pause recipes; the live lane owns that P1 candidate. cyril already labels it "agent-initiated · send-message-wake".
- outputStyle has no v2 equivalent.
- `settings.thinking` removal only affects the startup default; `/thinking` works.

The P2 set behind the blockers is unchanged from 2.31.1:
- `/context` and `/compact` (cyril-1aq0).
- Compaction visibility (cyril-58uv).
- MCP status/OAuth (cyril-nk4o, cyril-tqr8).
- Model fallback (cyril-aoby).
- Governance (cyril-0o7e).

The 4c4d acceptance items (first-run bundle, ADR, CI, macOS) still apply.

## Follow-ups (not day-one; mapped to issues)

| # | item | P | issue |
|---|---|---|---|
| F1 | `settings.thinking` removed in 0.66.43 — `chat.enableThinking` (`kas/settings.rs:79`) and default-on `thinking` (`:96`) are now unknown keys; `subagentOrchestration` (`:97`), `todoList` (`:82`) still dead; doc comment on `DEFAULTS_ON` names a removed tool | P2 | cyril-53qx (kas lane note) |
| F2 | outputStyle: read `_kiro/config/template` `outputStyle` (5 styles), send `session/prompt _meta.kiro.outputStyle` | P3 (tui lane proposes P2) | tui lane NEW + cyril-lyrx |
| F3 | Workflow `attention[]` (events, state, list rows, `finalState`, node level) + `inspect.pendingSteps` — would let `/workflow` show which step waits on a permission/question | P3 | kas lane NEW-or-NOTE cyril-4u4a (reference) |
| F4 | Repeat-cap pause wake: label the woken turn with the paused run (`workflow.rs:836-849` queues labels for terminal statuses only); decide billing stance | P1-candidate | live lane proposal (extends cyril-lki9) |
| F5 | `bufferOnly` / `max-tokens-recovery` engine-synthesized user chunks replay as user text on `session/load` | P3/P4 | cyril-58uv, cyril-99ds (note) |
| F6 | Recipe `plan[]` preview + sequence `includedFrom` dropped | P3 | cyril-4u4a, cyril-inwg (tui lane notes) |
| F7 | Model `rateMultiplier`/`rateUnit` (credit column) dropped | P4 | cyril-lxuo |
| F8 | `autopilot` not settable | P3 | fold into a vh69 follow-up |
| F9 | `diagrams` capability + `create_diagram` rendering (dark; TUI does not opt in) | P3 | kas lane NEW P3 |
| F10 | `executionTargets` / `sourceProviders` now consumed by Kiro's host; cyril reads neither | P3 | cyril-tikf (host lane note) |
| F11 | Workflow plan approval rendering (`toolCall.content` Text + `workflowPlan`) — reachable only once `/workflow new` authoring lands | P3 | cyril-8wh0 (blocked on cyril-0asq) |
| F12 | `_kiro/customAgent/{not_found,config_error}` still unfiled since 2.31.1 | P3 | propose (below) |
| F13 | `/spawn /kill /msg /sessions` on KAS | P3 | cyril-a8se |
| F14 | unchanged 2.31.1 follow-ups: `turn_end` stopReason (cyril-pyko), `interaction_resolved`/`replayMarking` (cyril-99ds), `userInput`, `terminalInput` (cyril-7p7n), `streamingShellContent` (cyril-2mo0), session list/rewind (cyril-wigv, cyril-1aq0), telemetry (cyril-zjm5), refusal (cyril-ker1), memory (cyril-0na7) | P3/P4 | as listed |

## Proposed issues / notes (rivets grepped with `static-v3cov-issues-2.32.0.py`; nothing filed by this lane)

Already filed this cycle (reference, do not re-propose): **cyril-k8wc** (P1, shell_type), **cyril-8wh0** (P3, plan approvals, blocked on cyril-0asq).
Lane proposals referenced, not duplicated: kas lane (NOTE cyril-53qx thinking; attention[] NEW-or-NOTE cyril-4u4a; diagrams NEW P3; notes on cyril-iq2c,
cyril-lyrx, cyril-0asq, cyril-inwg, cyril-58uv, cyril-7p7n), tui lane (NOTE cyril-gn07 raise; outputStyle NEW; notes cyril-4u4a, cyril-inwg, cyril-nk4o,
cyril-vh69 -> this lane redirects that one to **cyril-lxuo**; OSC 7501 NEW P4), host lane (notes cyril-tikf, cyril-a7nk, cyril-ci3j), live lane (repeat-cap wake P1-candidate).

This lane's own proposals:
- **NOTE cyril-auxt (P1, re-scope):** KAS persists allow_always grants itself from the permission-response echo `_meta.kiro.consent{capability, scope, resource, workspaceRoot}`. In KAS `S8e`, a missing echo defaults to `scope:"session"`. `persistConsent` writes scope `user` to `<userConfigDir>/settings/permissions.{yaml,json}` and scope `workspace` to the workspace permissions file, then rebuilds policy. Kiro's TUI has sent this echo since at least 2.31.1 (`ew()`/`cw()`). Replace the agent-JSON write (`kiro_agent_config.rs:156`) with the echo, and keep cyril-k8wc as a hard prerequisite.
- **NOTE cyril-gn07:** merge it into auxt's fix as the mechanism and raise it out of `needs-info`. The allow_always half of its question is answered statically: without the echo the grant is session-only, and with it KAS persists the grant. The 2.32.0 folder grant (`resource:"<dir>/**"`, fs_write) uses the same echo. Live TUI-L1 confirms.
- **NOTE cyril-4c4d:** at HEAD the blockers are cyril-zbu7 and cyril-auxt (blocked by cyril-k8wc). cyril-oieu and cyril-vh69 are fixed; close oieu. Consider linking cyril-k8wc as a direct `blocks` (it is only transitive today via auxt). Attach this file as the 2.32.0 parity list.
- **NOTE cyril-c6hv:** add cyril-k8wc as a child. Mark the vh69 residuals (rateMultiplier -> cyril-lxuo; autopilot; outputStyle) as P3/P4 follow-ups.
- **NEW P3** "KAS: handle `_kiro/customAgent/{not_found,config_error}` (parity with v2 `kiro.dev/agent/*`)". Proposed in the 2.31.1 coverage but not found in rivets. KAS counts 3 / 2; TUI consumes not_found.
- **NOTE cyril-53qx:** `kas/settings.rs` doc comment above `DEFAULTS_ON` still says `subagentOrchestration` "selects the `orchestrate_subagent` tool". The tool was removed in 0.66.33, and `thinking` was removed from AgentSettings in 0.66.43.
- **Correction for the 2.31.1 audit doc / coverage file:** the Matrix 5 permission row said the TUI sends no consent echo ("0 hits"). That is wrong: the TUI builds `{kiro:{consent:{capability,scope,resource?,workspaceRoot?},rejectionReason?}}` in 2.31.1 (`ew()`) and 2.32.0 (`cw()`).

## Leads for the live lane (only rows not already sent as KAS-L*, TUI-L*, HOST-L*)

All legs: fresh temp `HOME`, real `XDG_DATA_HOME`, direct `node acp-server.js` from the 2.32.0 carve, positive control
`initialize.clientCapabilities._meta.kiro.configurationState:true` (traps 5/7/28).
- **V1: the cyril-shaped arm of KAS-L1 (one or two turns).** Answer `_kiro/terminal/shell_type` with `{"shellType":"posix"}`, with no V3 rule. Prompt "Run exactly: echo AUDITV1". Answer the first `session/request_permission` with optionId `always-accept` and **no** `_meta`, which is exactly cyril's response. Then prompt the same command again in the same session. Expected on `"posix"`: a second permission request, because the session-scope grant is inert under the `"unknown"` dialect. Control arm `"bash"`: no second request (the default session scope applies). Optional third arm: `"bash"` with `_meta.kiro.consent{capability:"shell", scope:"user", resource:"echo AUDITV1", workspaceRoot:<ws>}`. Record which permissions file appears under the temp HOME (`.kiro/settings/permissions.yaml` expected), and confirm a fresh `session/new` runs the command unasked. The result decides the cyril-auxt re-scope and cyril-k8wc together.
- **V2 (zero turns, opportunistic): cyril's actual reply.** If the cyril-on-KAS leg runs, record cyril's real `_kiro/terminal/shell_type` reply. Every probe driver so far replied `"bash"`, so no capture shows cyril's `"posix"`.

## Method notes

- Scripts (copied from 2.31.1, paths moved to the 2.32.0 carve/TUI with 2.31.1 as OLD; logic unchanged): `experiments/conductor-spike/static-v3cov-{ctx,kinds,methods,issues}-2.32.0.py`.
  Outputs: `v3-coverage/tmp/kinds.json`, `v3-coverage/tmp/methods.json` (regenerable, git-ignored). The methods script took the kas lane's `kas/surface/static-kas-surface.json`.
- Counts quoted are `str.count` in 0.66.43 / TUI 2.32.0 with OLD (0.66.34 / TUI 2.31.1) via `--old` (trap 2). Set-level claims (123 = 123 methods, 25 = 25 kinds, 74 = 74 slash literals) come from the set diffs of this lane and the kas/tui lanes (trap 3).
- New KAS reading this lane did itself (STATIC): `jh()`/`DMe()` shell-name mapper, `S8e`/`rSu` consent intake, `persistConsent` + `eHo`/`G4u`/`j4u`/`tHo`/`nHo` permission-file writers, `editedCommand` intake; TUI `cw()`/`ew()` consent echo and `_lt()`/`Wst()` trust record. Quotes are reproducible with `static-v3cov-ctx-2.32.0.py --kas|--tui -w 1400 '<literal>'`.
- cyril re-verification: every 2.31.1 row whose code moved since 5ee8bc07 (25 files under `crates/` changed by #168/#169/#171) was re-read at HEAD. Rows outside those files keep their verdicts, but their line numbers were refreshed where they are cited.
- Live evidence is cited by live-lane leg only. This lane's captures do not resolve for `check-citations.py` from `v3-coverage/`, so the file deliberately contains no frame citations.
- The probe drivers answer `shell_type` with `"bash"` (sweep and workflow captures), so no live capture yet exercises cyril's `"posix"` value. That is why lead V1 exists.
- Not done: no execution, no rivets writes, no edits outside `v3-coverage/` and the four new scripts.
