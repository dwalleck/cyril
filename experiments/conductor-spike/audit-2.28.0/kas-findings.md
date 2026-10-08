# KAS static lane — 0.66.15 (2.26.x) → 0.66.22 (2.27.x) → 0.66.26 (2.28.0)

Saved by the orchestrator from the lane's hand-back (subagent Write was refused).
Raw evidence: ./out/{surface.txt, identifiers-*.txt, verified.txt, method-defs.txt, describe-diff.txt}.
Scripts: experiments/conductor-spike/static-kas-{surface,identifiers,settings,session-info-kinds,flags}-2.28.0.py

Method: set diffs of string literals/object keys (minified bundle); every add/remove verified by raw
substring count in BOTH bundles (tools/verify.py); every row read in context (tools/ctx.py).

## Delta A — 0.66.15 → 0.66.22 (2.27.x)

- **A1 `_kiro/configuration/state`** (agent→client notif). `await t.recipient.notify("_kiro/configuration/state",s)`.
  Payload `{view, layers[{id,name}], settings[{krn, layer, standing:{kind:"active"}|{kind:"displaced",compose,decidedBy}, readFrom?, value, asserts?}], resources}`.
  Opt-in: `clientCapabilities._meta.kiro.configurationState===true`. Sent: connection view after initialize response;
  per-session view after session/new|load response (`kind:"session-entered"`); re-sent on change (deduped) + catch-up.
  Layers: kiro-agent-defaults, kiro-service, organization, machine, user-synced, user-local, client-connection,
  workspace, agent, session (0.66.26 adds `kiro-agent` after user-local). Display names "Kiro Defaults",
  "Your Organization", "Cloud Config", "Local Settings (~/.kiro)", "This Client" (clientInfo.title||name). cyril: NOT-SENT.
- **A2 `_kiro/configuration/contribute`** (client→agent) `{sessionId?, contributions[{layer, owner, setting?|type?, name?, value,
  asserts?: replace|locked|floor|ceiling|append, readFrom?{krn,uri?}}]}` → `{}`. Dispatched only if contributeHandler exists;
  acp-server.js entry never sets `enableConfigurationContributeMethod`; not in extensionMethods ⇒ UNREACHABLE standalone.
  KRN `krn:<layer>:<owner>:<type>/<name>`; settings `krn:::setting/<name>`. Resource types: steering, agent, skill, power,
  hook, mcp-server, command, rule, workflow. cyril: N/A.
- **A3 env `KIRO_DUMP_CONFIGURATION(_DIR)`** — dumps resolved layers (default tmpdir()/kiro-agent-configuration).
- **A4 multi-client .d.ts** — SharedInitialization ("Answers every client's initialize with the agent's first successful
  initialize result. Each request is still checked against the ACP schema", replaces cachedInitializeResult);
  ClientConnections (per-client handshake, repeat initialize replaces; session-entered/departed; ConnectionId stdio|ws);
  withClientConnections(stream) wraps stdio as one client; MultiplexStream +hasClientEntered(sessionId, clientId), +dispose().
  Per-client facts (configurationState opt-in, clientInfo) from each client's handshake; capabilities resolved once from first.
  No wire change for single stdio client. Relevant cyril-5g2o, cyril-8lfs.
- **A5 `settings.backgroundExecution {enabled}`** — setting → persisted → flag `background_execution` (default false;
  env KIRO_FEATURE_BACKGROUND_EXECUTION_ENABLED). `tuo(e,t){return e?"background-execution":t?.backgroundProcesses===!0?"control-process":"none"}`;
  `M$r(e,t){return e.terminal&&!t?"client":"agent"}` ⇒ with terminal:true (cyril advertises) bg processes run in cyril-hosted
  terminals. Separate system prompt `session_start_<agentType>_bg`. cyril: NOT-SENT.
- **A6 `ftaVibe` → `validation` (breaking rename)** — `KZe(e,t){return e.data.validation?.enabled??t}`; flag fta_vibe→validation;
  env KIRO_FEATURE_FTA_VIBE_ENABLED→KIRO_FEATURE_VALIDATION_ENABLED; persisted fallback `ftaVibeSettingEnabled`. cyril: N/A.
- **A7 `todoList` setting + `todo_list` tool REMOVED** (5→0). cyril `crates/cyril-core/src/protocol/kas/settings.rs` still maps
  chat.enableTodoList→todoList (harmless catch-all, stale). cyril-5onw moot; cyril-q8gq/ea67 should drop mapping.
- **A8 workflow step parent notes** — `sendParentNote(…,"network-waiting"|"network-give-up")` → deliverRuntimeNoteToParent →
  `deliverSendMessage(parent,msg,"info",{role:"step",workflowId,nodeId,agentName?,sessionId?},true)` ⇒ CAN AUTO-WAKE idle parent
  (billed) + `extNotification("_kiro/session/notify",…)`. New (`workflow.step.parent_note` 0 in 0.66.15). cyril: session/notify DROPPED.
  Touches cyril-lki9, 5n75, oswq.
- **A9 saved prompts as slash commands** — `.kiro/prompts/*.md` (ws + ~), MCP prompts → available_commands entries with
  `_meta.kiro.{type:"prompt", resource, arguments}`, input hint "<req> [opt]"; expanded agent-side (prompt.resolve.complete).
  cyril: HANDLED generically, _meta DROPPED. (Memory "KAS advertises ONLY skills" is stale.)
- **A10 steering context references** `#[[file:…]]` / `#[[folder:…]]` expanded in steering, gated by file-read policy. Agent-side.
- **A11 permission engine fail-closed** — "Tool call denied: Kiro's permission engine (cedar-wasm) could not be loaded…".
- **A12 memory tool** — param docs rewritten; persisted `memoryConfigSource: "legacy"|"explicit"`; {mode, reflection} unchanged.
- Removed in A: fta_vibe/ftaVibe*, todo_list, CScoreAccessDenied, unsupportedOnCreateResponse ("Document attachments are not yet supported on this model path").

## Delta B — 0.66.22 → 0.66.26 (2.28.0)

- **B1/B2 `_kiro/terminal/write {sessionId, terminalId, input}`** (agent→client request), gated on
  `initialize.clientCapabilities._meta.kiro.terminalInput===true`. Without: "This client does not support writing to terminal
  stdin (no terminalInput capability)." / `interactive:"no-terminal-input-capability"`. cyril: NOT-SENT.
- **B3 model routing "cascade"** — `cascade:{enabled}` (modelRoutingSettingEnabled on session/new+load) | JSON env
  KIRO_FEATURE_CASCADE_CONFIG | AB `cascade_config` (default '{"cascadeEnabled":false}'). Only for model `auto` or category
  auto-fast/auto-balanced/auto-smart. Defaults {initialCategory:"auto-balanced", availableCategories[...], probeStartTurn:10,
  probeInterval:5, handoffHistory:"full", compactionMaxChars:2000, clientNotificationEnabled:false,
  clientNotificationMessage:"Found a better model for this task. Switching to it now."}. Hidden "model-classifier" +
  "model-handoff" calls (extra billing) → overrideModelForRouting. No config_option_update seen ⇒ cyril model badge stale.
  Persisted `cascadeState`. cyril: NOT-SENT.
- **B4 session_info_update kind `model_routed {message}`** (23→24 kinds), only if clientNotificationEnabled; replayed from
  persisted `model_route`. cyril: DROPPED (convert/kas.rs `_ => None`).
- **B5 interrupted tool calls** — shape unchanged (tool_call_update failed + rawOutput, then turn_end). Text now
  "This tool was interrupted before it reported a result, so it may or may not have taken effect." / new
  "This tool call was cancelled before it ran." (actionState Canceled → failed). Persisted ⇒ same on reload. cyril HANDLED; partly cyril-fjfu.
- **B6 background execution v2** — lifecycle started/exited/consumed/output/stalled/stop-unconfirmed; notifyOnOutput
  (monitor mode, 0→22); interactive (1→22); adopt running foreground cmd; stall detection on prompt-looking output;
  bg terminal/create now carries outputByteLimit. Env KIRO_BACKGROUND_STALL_SECONDS, KIRO_BACKGROUND_TIMEOUT_DEFAULT_SECONDS,
  KIRO_MONITOR_TIMEOUT_DEFAULT_SECONDS (300). cyril partial; outputByteLimit ignored (cyril-1rpv).
- **B7 workflow watch handler `background-process`** {command, cwd?, waitSec≤3600, outputTailLines≤1000} → payload
  {status exited|stopped|timed_out, exitCode, signal, outputTail, outputFile?}; permission via run spawnGate; listed by
  `_kiro/workflow/listWatchHandlers`.
- **B8 `turn_completion.contextBreakdown`** — request `session/prompt _meta.kiro.contextBreakdown:"summary"|"detailed"`.
  Schema 1 {modelCalls, totalChars, window{usagePercentage?, sizeTokens}, compacted?, categories{kiroInstructions, toolSpecs,
  mcpTools, agentsIndex, skillsIndex, powersIndex, steering, skills, toolIO, history, workspace, memory, attachedFiles, other:
  {chars, percent, items?[{name, uri?, chars, percent, count?, inclusion?}], omitted?}}, media{images, imageBytes, documents,
  documentBytes}}. cyril NOT-SENT; fits cyril-1116, cyril-0s9x.
- **B9 settings intake rework** — declarative table: session/new|load `_meta.kiro.settings` keys → session-layer statements:
  workflows, validation, fta, thinking, tangentMode, checkpoint, _parallelTasks, _requirementAnalyzer, _quickSpec, _subagent,
  _delegate, compaction.excludePercent, compaction.excludeMessages. Initialize `settings.knowledge.*` → new `kiro-agent` layer.
  Logs configuration.intake.unknown-key / unread-key. Recognised outside schema: c2s, verifyFirstWorkflow, sessionRecap,
  steeringReminders, memoryEnable, userMemoryOptIn, subagentOrchestration.
- **B10 `_kiro/hooks/list` toolTags** — client→agent registry destructure gains `toolTags`; match via new toolPattern vs {id,tags}.
  Warning hooks.trigger.sessionEndSemanticsChanged ("Use Stop for hooks that should run after every turn").
  Agent→client `_kiro/hooks/list` (cyril serves) has carried toolTags since ≤0.66.15; cyril kas/callbacks.rs parses only
  {trigger, toolId?} ⇒ standing gap vs 2.28.0 TUI tag matchers (shell, read, @builtin, @mcp). cyril-7q8u, qr6l.
- **B11 dynamic delegation** — AB `dynamic_delegation_config` (default '{"delegationEnabled":false}'), JSON env
  KIRO_FEATURE_DYNAMIC_DELEGATION_CONFIG; categories [auto-balanced, auto-smart]; invoke_sub_agent +`modelCategory`.
- **B12 KUTS telemetry** — flag kuts_telemetry default TRUE (env KIRO_FEATURE_KUTS_TELEMETRY_ENABLED); standaloneTelemetryConfig()
  kutsEnabled; endpoints telemetry.{us-east-1,eu-central-1}.kiro.dev, beta.us-east-1.telemetry-v2.kiro.dev;
  ClientType "acp-other" for clientInfo ∉ {kas-standalone, acp-client}. cyril sends no telemetryEnabled.
- **B13 `KIRO_LEGACY_RESPONSES_REPLAY`** — `_kiroLegacyResponsesReplay`, persisted legacy_reasoning_replay (GPT Responses
  reasoning replay); excluded from wire replay.
- **B14 `validate_workflow` tool REMOVED** (6→0) — folded into save_workflow_definition. run_workflow with workflowPrompt
  refuses when wf-workflow-creator is user-overridden (`creator_override_refused`). cyril-0asq.
- Removed in B: validate_workflow; knowledgeChunkSize/Overlap catalog keys → knowledgeChunking{size,overlap} (wire
  settings.knowledge.chunkSize/chunkOverlap still read); configuration.contribute.* logs → configuration.intake.*.
  (~50 control-plane *Command names 5→4 = bundler artefact.)

## Flags
0.66.22: +validation, +background_execution (env-reachable), −fta_vibe. 0.66.26: +kuts_telemetry (default true),
+dynamic_delegation_config, +cascade_config (JSON defaults + JSON env). Totals 26 flags, 13 boolean env-reachable + 2 JSON env.
(Old extractor misreports JSON flags; -2.28.0 copy fixes.)

## Unchanged
initialize result & agentCapabilities; ACP sessionUpdate variants; workflow event zod literals; session/set_config_option (2→2).
Covenant `@kiro/acp-type-covenant` NOT shipped (version pin 0.66.26 + erased type import only; zod inlined ~2 MB).
Changelog items with no KAS wire: /tangent merge, model fallback (control-plane SDK type names only), model-unavailable notices,
non-interactive workflows, Classic→3.0.

## Leads (live)
- L1 configurationState payload (initialize `_meta.kiro.configurationState:true` → session/new; opt KIRO_DUMP_CONFIGURATION).
- L2 contribute → expect Unknown ext method.
- L3 `_kiro/terminal/write` (terminal:true + terminalInput:true + backgroundExecution) with `python3 -c 'input();print(42)'`; control without.
- L4 session settings override initialize settings (thinking off@init, on@session/new; read standing.decidedBy).
- L5 contextBreakdown:"detailed"; control without.
- L6 auto* models in availableModels; cascade run with KIRO_FEATURE_CASCADE_CONFIG probeStartTurn:1 + clientNotificationEnabled.
- L7 parent-note wake via KIRO_WORKFLOW_TRANSIENT_RETRY_DELAYS_SEC + blocked egress.
- L8 KUTS egress under logging HTTPS_PROXY; kill switches env vs initialize telemetryEnabled:false.
- L9 saved prompt `.kiro/prompts/hello.md` → available_commands _meta.kiro.type:"prompt"; `/hello x` expansion.

## Existing issues touched
cyril-0o7e, 1116, 0s9x, lki9, 5n75, oswq, fjfu, 7q8u, qr6l, 0asq, 5g2o, 8lfs, 1rpv, 5onw, q8gq, ea67.
