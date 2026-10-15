# LIVE lane — kiro-cli 2.29.1 → 2.31.1 wire audit (KAS 0.66.33 → 0.66.34)

Started 2026-10-14 (UTC 2026-10-15 01:3x). Lane dir: `experiments/conductor-spike/audit-2.31.1/live/`.
All legs strictly serial; direct KAS carve spawns (`<node> --experimental-wasm-modules <acp-server.js>
--transport=stdio --auth=acp-callback`), fresh temp `HOME`, real `XDG_DATA_HOME`.
Driver: `probe-kas-live-2.31.1.py` (copy of 2.29.1 driver + Conn(extra_env/home/argv_extra) + census adds).

Engines
- OLD = `~/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js` (0.66.33)
- NEW = `~/.local/share/kiro-research/kas-carves/2.31.1/node_modules/@kiro/agent/dist/server/acp-server.js` (0.66.34)

## Summary

1. All legs ran serially on 2026-10-15 01:33-02:17 UTC against one backend: direct KAS carves 0.66.33 (2.29.1) and 0.66.34 (2.31.1), plus archived v2 2.29.1/2.31.1. No forging, no login/logout, and no auth renewal events. A user-side renewal landed at 01:35.
2. **The KAS 0.66.34 wire is nearly identical to 0.66.33.** The handshake, sweep census, workflow control plane (17/17 runs match), session/load replay, sandbox, mode switching, command dispatch and MCP deferral are identical. The v2 pair is identical too (320 = 320 paths).
3. The real 0.66.34 deltas, all LIVE:
   - `extensionMethods` gains `_kiro/environment/start_ssh_session` (27 → 28; -32602 / -32000 `SshSessionUnavailableError`).
   - `KIRO_HOME` is honoured: agents, steering, MCP config, logs and sessions all move to an absolute value; relative, `/` and home-containing values are ignored with a WARN.
   - Memory now defaults to `{disabled, reflection:false}` for new sessions only; a loaded 0.66.33 session keeps read_write.
   - `_kiro/workflow/new` accepts and validates `modelId`/`effortLevel` and stamps them on the workflow state.
   - Bundled `feature-pipeline` gains `effortLevel:"high"` on 2 steps.
4. **Static claims corrected:**
   - MCP tools were already deferred behind `tool_load` on 0.66.33 for this account (AB served on), so the "unconditional" change is invisible here.
   - `toolSearch.enabled` has no effect on either engine.
   - `_kiro/governance/state` is pushed on BOTH engines; it is not new.
   - `_kiro/config/template` returns `{modes, configOptions}` with no `agents` key.
   - Availability fallback is already evaluated by default (`modelFallback.declined … notModelAttributable`).
5. **cyril-4c4d flip blockers confirmed live:**
   - (a) Every KAS-advertised skill, prompt and custom-agent command fails the way cyril dispatches it: `_kiro.dev/commands/execute` and `/options` → -32603 PersistenceClassification, 18/18 calls on both engines. Prompt text `/skill args` works.
   - (b) KAS honours only V3 `permissions.rules`. cyril's trust append to V2 `toolsSettings.execute_bash.allowedCommands` is ignored until `agent upgrade` re-runs, and never reaches V3 once the user has edited `permissions`.
   - (c) Sub-agent frames all ride the PARENT sessionId, tagged `agentSubtaskId`. The child's text is not streamed, and `Subagent Response` / `Tool Load` arrive as orphan `tool_call_update`s that cyril's UiState drops silently.
   - (d) `session/set_model` is -32603 on KAS; models go only through `set_config_option`.
6. cyril handshake facts:
   - It sends two dead keys (`subagentOrchestration`, `todoList` → `intake.unknown-key`) and no `memory`, `workflows` or `configurationState`.
   - clientInfo `cyril` is attributed "unrecognized" → KAS infers the `kiro-ide` behavioural client.
   - The TUI reference sends dead keys too (`largeToolOutputHandler`, `workflowNotifications`).
7. KIRO_HOME parity: cyril matches KAS 0.66.34 only for an absolute value, and only for sidecar and settings. Global agents (trust writes) and hooks stay hard-coded to `~/.kiro`, and cyril uses rejected values verbatim.
8. NOT-REACHED: the `model_fallback` frame itself (needs a model-attributable outage or a refusal), the `arrivedDuringLoad` race, `send_message` unsavable → `node_paused`, hooks relocation (v2Hooks off), natural compaction, and the 15 s control-plane timeout. One natural 301 s silent backend stall was seen on 0.66.33.


## Progress log (appended as legs finish)

**REFOCUS (orchestrator relay, received during leg 2 gate-on, ~01:50 UTC):** cyril moves to KAS/v3 as its DEFAULT engine
(v2 backend retiring; rivets cyril-4c4d). Remaining legs re-prioritised for the v3 surface; completed legs kept.
- DROPPED: leg 5 (v2 pair) + lead 11 (v2 acp_agent.rs) unless all v3 items done and token healthy (then last);
  lead 1 v2 counterpart (archived `kiro-cli acp` KIRO_HOME arms). KAS KIRO_HOME arms kept.
- DEMOTED: lead 8 (v3 ease-in on v2 launcher) = cheap no-auth initialize check after v3 items.
- REFRAMED: lead 9 = V2→V3 migration: after `agent upgrade`, does KAS 0.66.34 load the upgraded agent + its permissions,
  and does cyril's trust append survive or freeze the refresh; which file KAS reads permissions from.
- ADDED: A cyril-exact KAS handshake at debug (intake unknown-key list) vs TUI 2.31.1 reference vs 0.66.34 consumption;
  B model_fallback shape on 0.66.34 (2.29.1 F0/F1/F2 recipe, natural backend answers only);
  C memory default under cyril's handshake (0.66.33 vs 0.66.34); D session/load replay 0.66.33 vs 0.66.34.

### Leg 0 — `--help` + initialize-only matrix (DONE 01:33-01:34 UTC)
- `--help`: neither engine has a help surface; both start normally, print nothing on stdout, exit 0 on stdin EOF.
  stderr identical modulo version (`kas.server.starting … 0.66.33|0.66.34`) and timings (`tmp/help-{old,new}.{out,err}`).
- initialize (probe `probe-acp-init-2.31.1.py`, full path `clientCapabilities._meta.kiro.{configurationState,settings,telemetryEnabled}`;
  positive control `configurationState:true` → `_kiro/configuration/state` push arrived in all 4 arms):
  captures `kas-init-{2.29.1,2.31.1}-{callback,none}.jsonl`. callback vs none result IDENTICAL on both engines
  (after normalising log dir); callback mode asks `_kiro/auth/getAccessToken` at init (1×), none-mode 0×.
- OLD→NEW initialize diff = exactly ONE line: `agentCapabilities._meta.kiro.extensionMethods` + `_kiro/environment/start_ssh_session`
  (27 → 28, inserted after `_kiro/codeIntelligence`). `_kiro/config/template` and `_kiro/codeIntelligence` present on BOTH (not new);
  `_kiro/governance/state` on NEITHER (it is an agent→client notification, not a client-callable method). LIVE.
- Full NEW list (28): _kiro/knowledge, _kiro/memory/{list,get,create,update,delete}, _kiro/codeIntelligence,
  _kiro/environment/start_ssh_session, _kiro/session/{context,compact,export,history}, _kiro/config/template,
  _kiro/configuration/getState, _kiro/workflow/{new,invoke,load,inspect,list,pause,cancel,delete,listRecipes,listWatchHandlers,resumeAll,retry,update,resume}.
  OLD list (27) = same minus start_ssh_session. Other `_meta.kiro` caps identical (checkpoints, sessionList, policyNotifications,
  sessionSources[local], sessionListScopes[workspace], executionTargets[local], sourceProviders:false, replayMarking, modelFallbackReporting).

### Leg 1 — paired SCENARIO=sweep (DONE 01:34-01:37 UTC)
- OLD `kas-live-sweep-2.29.1.jsonl` (176 frames, 32.3 s); NEW `kas-live-sweep-2.31.1.jsonl` (176 frames, 33.0 s). Both: 3 turns
  (p1_read end_turn, p2_shell end_turn + 1 permission, p3 bogus model `-32000 InvalidModelError`/`CLIENT_ERROR`), same 8 host callbacks.
  Durations match 2.29.1's sweep (~7 s/turn) — not suspiciously short (trap 21).
- RE-RUN: first NEW run (`kas-live-sweep-2.31.1-run1-authwait.jsonl`) was taken while the token was inside the driver's 200 s
  wait window, so the init-time `_kiro/auth/getAccessToken` reply came 20 s late; session/new blocked behind it and its result
  then lacked the `model` configOption (it arrived 0.23 s later in a `config_option_update`, frame 29). Re-run with a fresh token:
  `model` present in session/new (frame 26) exactly like OLD. ⇒ artifact of auth delay, not a version delta (method note M2).
- Census deltas OLD→NEW (everything else byte-equal modulo ids/timestamps/usage counters):
  - `_kiro/environment/start_ssh_session` pre-session `{}`: OLD `-32603 [PersistenceClassification] Ext method … has no persistence
    classification` → NEW `-32602 Invalid params: _kiro/environment/start_ssh_session requires sessionId as a string of 1-128 characters`.
  - `{sessionId:"bogus-sid-zz9"}` and `{sessionId:<live sid>}`: OLD same -32603 → NEW `-32000 "Starting an SSH session is not available:
    this agent has no remote sessions endpoint configured, so there is no backend to mint credentials."` `data.errorType:"SshSessionUnavailableError"`
    (endpoint check precedes session lookup: a bogus id gets the same -32000). LIVE.
  - `_kiro/config/template {}` pre-session and post-session: IDENTICAL on both, shape `{modes:{availableModes:[…]}, …}` (see lead 2 below).
  - `_kiro/codeIntelligence {subcommand:"status"}`: both `{"success":false,"message":"Code intelligence is not enabled for this session"}`.
- `session/new` result `_meta.memoryConfig`: OLD `{mode:"read_write",reflection:true}` → NEW `{mode:"disabled",reflection:false}`;
  `configOption memoryReflection` OLD `on` → NEW `off` (frame 26 both). No `memoryConfigSource` key on the wire. LIVE (lead 5).
- `_kiro/governance/state` is pushed UNSOLICITED on BOTH engines at session/new (frame 17), identical payload
  `{sessionId,isEnterprise:false,features{mcpEnabled:true,webToolsEnabled:true,usageAnalytics:false,contentCollection:true,
  promptLogging:false,codeReferenceTracker:false,autonomousAgents:true}}` — no `disabledReason`. Not new in 0.66.34.
- `_kiro/mcp/status` `{sessionId,servers:[]}` on both — no `accessMode` key with zero servers.
- session_info kinds/method counts identical in kind set (context_usage, user_message_id_assigned, focus_update, turn_start,
  turn_completion, turn_end, pending_interaction, interaction_resolved, display_error); no `model_fallback`.
- sweep-new-fields (`sweep-new-fields-diff-sweep.txt`): 695 vs 693 paths; +`result.recipes[].plan[].steps[].effortLevel`
  = bundled `feature-pipeline` gains `effortLevel:"high"` on 2 plan entries (frame 136 both; OLD [low,…], NEW [low,…,high,…,high]) — CONFIRMS kas lane;
  +`params.update.rawInput.cwd` = model-chosen optional arg on the shell call (also seen in 2.29.1-era captures) — coverage, not a change.
- CORRECTION to the line above: `_kiro/config/template {}` pre-session is NOT byte-equal — the census summary truncated at 1500 chars.
  Shape on both = `{modes:{currentModeId,availableModes[7]}, configOptions:[mode, memoryReflection, autopilot, contentCollection, outputStyle]}`
  (no `agents` key — the TUI derives agents from the `mode` configOption; no `model` option pre-session; `outputStyle` only here).
  Only delta: `memoryReflection` `on` (OLD) → `off` (NEW) — the memory-default flip again (frame 9 both captures).

### Leg 2a — SCENARIO=workflow GATE=off pair (DONE 01:38-01:43 UTC)
- OLD `kas-workflow-gateoff-2.29.1.jsonl` (1187 rec, 140.6 s), NEW `kas-workflow-gateoff-2.31.1.jsonl` (1367 rec, 160.1 s); both completed
  with the driver's teardown summary. `_meta.workflowsEnabled` false on both.
- Run parity (`wf_parity.py`): all 6 runs MATCH event-for-event — linear (13), loop (23, cancel/retry/cancel2 path), watch (11),
  bgwatch (10), park (10), agenturi (5); every terminal run auto-woke the parent on both. Notification method sets identical
  (counts differ only in model-driven chunk/tool volume). Permission asks 6 vs 9: NEW's model made 3 extra tool calls (`xxd out/s*.txt`)
  in linear — model behaviour, every ask still `consentRound:1`.
- sweep-new-fields (`sweep-new-fields-diff-wfoff.txt`): +`rawInput.cwd` (model-chosen, see leg 1); "removed"
  `permissions[].meta_kiro.workflowWatch.*` = trap-11 artifact (the watch asks sit at permission index 7-8 on NEW; literal count 3 = 3).

### Leg 2b — SCENARIO=workflow GATE=on pair (DONE 01:43-01:56 UTC)
- OLD `kas-workflow-gateon-2.29.1.jsonl` (2307 rec, 534.0 s), NEW `kas-workflow-gateon-2.31.1.jsonl` (2365 rec, 238.3 s). Both
  complete (driver teardown present). `workflowsEnabled` true on both (gate positive control).
- The OLD leg's extra 296 s is ONE silent backend stall: 301 s with zero frames between a step's `tool_call_update completed`
  (frame 140) and its next `agent_message_chunk` (frame 141), linear step s1, no watchdog/notice frame, no stderr WARN.
  Passive natural event on 0.66.33, not version-attributable (lead 10: recorded, not a 15 s control-plane timeout).
- Run parity: all 11 runs MATCH event-for-event (agenturi 5, bgwatch 10, branches 13, fail 5 incl. retry-node → failed,
  launcher (custom agent `run_workflow`) completed, linear 13, loop 23, loopnone 12, park 10, usertier 5, watch 11);
  every terminal run auto-woke the parent. `recipes_changed` 1/1. Notification method set identical.
- sweep-new-fields (`sweep-new-fields-diff-wfon.txt`): 1302 vs 1303 paths; 0 only-NEW; only-OLD `rawInput.cwd` (model coverage).
- ⇒ The `_kiro/workflow/*` control plane is live-identical 0.66.33 → 0.66.34 on this workload (no wire delta from the
  load/invoke/retry concurrency guards; they only change races not exercised here).

### Leg 3 / lead 1 — KAS KIRO_HOME arms (DONE 01:57-02:00 UTC)
Probe `probe-kas-leads-2.31.1.py kirohome`; captures `kas-lead-kirohome-{2.29.1,2.31.1}.jsonl` (+ `-kh-<arm>-stderr.log`), KIRO_LOG_LEVEL=debug,
one `Reply OK` turn per arm (all 10 turns end_turn ~5 s). Per arm a fresh HOME seeded with `home-agent`, `home-steer`, `settings/mcp.json`→`homesrv`,
`hooks/home-hook.json`; the KIRO_HOME root seeded with `kh-agent`, `kh-steer`, `khsrv`, `kh-hook`. Evidence per arm = initialize
`_meta.kiro.logging.logDir`, `session/new` modes, `_kiro/steering/documents_changed`, `_kiro/mcp/status` server name, which MCP child actually
started (its log file), getState(session) resource items, files created after the turn, stderr.

| arm (KIRO_HOME) | 0.66.33 loads / writes | 0.66.34 loads / writes | 0.66.34 log line | cyril `kiro_home_dir()` reads |
|---|---|---|---|---|
| unset | `$HOME/.kiro` (home-agent, home-steer, homesrv; logs + `sessions/<hash>/<sid>/` + `session-index/`) | same | none | `$HOME/.kiro` ✓ |
| absolute `<tmp>/khabs` | **`$HOME/.kiro`** (KIRO_HOME ignored; khabs untouched) | **khabs**: kh-agent, kh-steer, khsrv started, logs `khabs/logs/…`, `khabs/sessions/<hash>/<sid>/…`; `$HOME/.kiro` gets NOTHING | `[INFO] agent.userConfigDir.resolved {"name":"khabs","source":"env"}` | khabs → ✓ on 0.66.34, ✗ on 0.66.33 (sidecar/settings read a dir KAS never wrote) |
| relative `khrel` | `$HOME/.kiro` | `$HOME/.kiro` | `[WARN] agent.userConfigDir.ignored {"name":"khrel","reason":"relative"}` | `khrel` relative to cyril's cwd ✗ |
| root `/` | `$HOME/.kiro` | `$HOME/.kiro` (nothing created under `/`) | `[WARN] agent.userConfigDir.ignored {"name":"","reason":"root"}` | `/` → `/settings/cli.json`, `/sessions` ✗ |
| `=$HOME` | `$HOME/.kiro` | `$HOME/.kiro` | `[WARN] agent.userConfigDir.ignored {"name":"home","reason":"contains-home"}` | `$HOME` (`$HOME/settings/cli.json`, `$HOME/sessions`) ✗ |

- LIVE verdict: 0.66.34 relocates agents, steering, `settings/mcp.json` (MCP servers), logs, sessions + session-index wholesale to an
  absolute KIRO_HOME, and rejects relative / root / home-containing values with a WARN (name only, no full path). 0.66.33 ignores
  KIRO_HOME entirely (0 `userConfigDir` lines). No "ignoring KIRO_HOME" string on the KAS side — the literal is `agent.userConfigDir.ignored`.
- Hooks: NOT-REACHED — `_kiro/hooks/list` = `-32603 … not available when v2Hooks is disabled` in every arm on both engines (cyril-like
  handshake without `hooks.v2`); hook dir relocation stays STATIC (`homeGlobalHooksDir=join(userConfigDir,…)`).
- cyril divergence (absolute arm, 0.66.34): KAS reads global agents from `<KIRO_HOME>/agents` but cyril's trust append writes
  `~/.kiro/agents/<name>.json` (`crates/cyril-core/src/kiro_agent_config.rs:102-104`); cyril's inbound hook registry loads `~/.kiro`
  (`crates/cyril-core/src/protocol/domain_mediator/mod.rs:262-266`). Sidecar (`usage/kiro_sidecar.rs:512-547`, walks
  `sessions/<hash>/<sid>`) and settings (`protocol/kas/settings.rs:50-59`) follow KIRO_HOME → CORRECT on 0.66.34 for absolute values,
  wrong for the three rejected forms (cyril uses the value verbatim, `kiro_agent_config.rs:93-98`).

### Leg 3 / A+C — cyril-exact KAS handshake vs TUI 2.31.1 reference (DONE 02:00-02:01 UTC)
Probe `probe-kas-v3default-2.31.1.py handshake`; captures `kas-v3d-handshake-{2.29.1,2.31.1}.jsonl`; intake lines read from the KAS
debug log file (`$HOME/.kiro/logs/<ts>/kiro.log`, NOT stderr — the 2.29.1 lane did the same). Zero turns.
Arms (full path `initialize.clientCapabilities._meta.kiro…`):
- `cyril_user` = exactly what `engine.rs:97-136 client_capabilities()` + `kas/settings.rs:107-180 marshal_agent_settings()` produce from this
  user's real `~/.kiro/settings/cli.json`: `fs{readTextFile,writeTextFile,_meta.kiro{readFile,writeFile,stat,readDirectory,delete}}`,
  `terminal`, `_meta.kiro{settings{thinking,todoList,codeIntelligence,knowledge,subagentOrchestration,toolSearch{enabled:true,minPct:0,minTokens:0}},
  hooks{enabled:true}}`, clientInfo `{name:"cyril",version:"0.2.0-alpha.1",title:"Cyril"}`; session/new `{cwd,mcpServers:[]}` (commands/session.rs:50-52).
- `cyril_empty_clijson` = DEFAULTS_ON only (codeIntelligence, knowledge, thinking, subagentOrchestration).
- `tui_ref` = TUI 2.31.1 `$tt(oT())` clientMeta reconstructed from the bundle for the same cli.json (external user):
  `_meta.kiro{telemetryEnabled:false,knowledge:true,hooks{enabled,v2},requirementsAnalysis,specPhaseCheckpoints,streamingShellContent,
  settings{workflows,goal,thinking,codeIntelligence,knowledge,largeToolOutputHandler,workflowNotifications{enabled,delivery:"steer"},
  memory{mode:"disabled",reflection:true},userMemoryOptIn{enabled:false},toolSearch{…}}}`, clientInfo `kiro-cli`. (Limitation: the TUI
  ALSO sends `settings` on `session/new._meta.kiro`; this arm sends them only at initialize.)
- `control_cfgstate` = `configurationState:true` + `settings{thinking}` → positive control: `_kiro/configuration/state` pushed 2× (0× in every
  arm without the flag — cyril never sends `configurationState`).

Intake (debug log), IDENTICAL on 0.66.33 and 0.66.34:

| key (client-connection layer) | cyril sends? | TUI sends? | 0.66.33 / 0.66.34 intake | verdict |
|---|---|---|---|---|
| `subagentOrchestration` | YES (DEFAULTS_ON, settings.rs:93-98) | no | `configuration.intake.unknown-key` | cyril sends a dead key (unchanged since 2.29.1, cyril-53qx) |
| `todoList` | YES when `chat.enableTodoList` (settings.rs:82) | no | `unknown-key` | dead key (cyril-53qx) |
| `thinking` | yes | yes | `configuration.intake.unread-key` (both clients, every arm) | accepted, not read at connection layer |
| `codeIntelligence`, `knowledge` | yes | yes | no line (accepted) | consumed |
| `toolSearch{enabled,minPct,minTokens}` | yes | yes | no line (schema key) | intake-accepted on both; `enabled` has no consumer in 0.66.34 (static; see lead 3) |
| `memory` | NO | yes (`{mode:"disabled",reflection:true}`) | no line; session/new `memoryConfig` = `{disabled, reflection:true}` on both | consumed (TUI arm is the positive control) |
| `userMemoryOptIn` | no | yes | no line | accepted |
| `workflows` / `goal` | no (ADR-0011, settings.rs `workflows_gate_never_marshaled`) | yes | `workflows` → `unread-key`; `goal` no line; `workflowsEnabled` stays false when sent only at initialize | gate is read from session/new, not initialize |
| `workflowNotifications` | no | yes | `unknown-key` | dead at connection layer |
| `largeToolOutputHandler` | no | yes (TUI default true) | `unknown-key` | the TUI itself sends a dead key; confirms 2.29.1 T-ltoh (not a valid positive control) |

- clientInfo: `cyril` → `clientInfo.name {"reportedClientName":"cyril","clientAttribution":"unrecognized","client":"kiro-ide"}` + warn
  `behavioral client inferred from environment {inferredClient:"kiro-ide"}`; `kiro-cli` → `clientAttribution:"kiro-cli"`. Both engines.
  cyril (default PresentAs::Cyril, `types/present_as.rs:43-47`) is therefore run with KAS's *kiro-ide* behavioural profile.
- **C (memory default) LIVE:** with cyril's handshake (no `memory` key) `session/new._meta.memoryConfig` = 0.66.33 `{mode:"read_write",reflection:true}`
  / 0.66.34 `{mode:"disabled",reflection:false}`; `configOption memoryReflection` `on` → `off`; `_kiro/memory/list` `-32602 Memory is not enabled`
  on both (AB-gated). Same in cyril_empty and control arms. Surfaces: session/new `_meta.memoryConfig`, the `memoryReflection` configOption
  (session/new + every `config_option_update`), and pre-session `_kiro/config/template`. No `memoryConfigSource` on the wire.

### Leg 3 / B — model_fallback shape (DONE 02:01-02:02 UTC)
Probe `probe-kas-v3default-2.31.1.py fallback` (2.29.1 recipe), captures `kas-v3d-fallback-{2.29.1,2.31.1}.jsonl`, KIRO_LOG_LEVEL=debug.
| arm | env | model | outcome (both engines) | session_info kinds | model_fallback / _promoted |
|---|---|---|---|---|---|
| F0 | – | `bogus-model-zz9` | `-32000 InvalidModelError` / `CLIENT_ERROR` | user_message_id_assigned, focus_update, turn_start, display_error, turn_completion, turn_end | none / none |
| F1 | both `KIRO_FEATURE_MODEL_FALLBACK_*_ENABLED=true` | bogus | identical to F0 | identical | none / none |
| F2 | both flags | auto | end_turn 5 s | normal | none / none |
- NEW evidence (debug log, `$HOME/.kiro/logs/<ts>/kiro.log`, both engines, F0 AND F1):
  `modelFallback.declined {"trigger":"availability","reason":"notModelAttributable","from":"bogus-model-zz9","errorType":"InvalidModelError","executionId":…}`.
  The fallback decision IS evaluated for the availability trigger even in F0 with no flag env — so `availabilityFallback` is already on for
  this account (AB/backend-served; the static flag default is `false`) — and an unknown model id is refused as not model-attributable
  (bundle `sCc`: `!i||!oCc(i)` → `eligibility:"notModelAttributable"`). [INFERENCE from code path + log; the AB value itself is not on the wire.]
- `model_fallback` frame shape: NOT-REACHED (needs a model-attributable availability error or a refusal stop; neither occurs naturally
  without forging, trap 13). `model_fallback_promoted`: 0 hits in every 2.31.1-lane capture (sweep, workflow ×4, fallback ×2).
  Static shape stays `{kind:"model_fallback", from, to?, scope?, trigger, reason?, agentSubtaskId?}` (TUI T1). cyril DROPPED at
  `crates/cyril-core/src/protocol/convert/kas.rs:525` (cyril-aoby).
- `agentCapabilities._meta.kiro.modelFallbackReporting:true` advertised on both (leg 0).

### Leg 3 / D — session/load replay (DONE 02:03-02:05 UTC)
Probe `probe-kas-v3default-2.31.1.py sessionload`; captures `kas-v3d-sessionload-2.29.1.jsonl` (0.66.33 loads its own leg-1 sweep session),
`kas-v3d-sessionload-2.31.1.jsonl` (0.66.34 loads its own), `kas-v3d-sessionload-cross-0.66.33to34.jsonl` (0.66.34 loads a COPY of the
0.66.33-written HOME = the upgrade path). Each run also: abandoned-prompt arm (turn 1 `FIRST`, turn 2 streaming → SIGKILL the KAS process
group 2 s into streaming, respawn same HOME, `session/load`).
- Sweep-session replay: 42 frames on all three, identical sequence — `user_message_chunk` / `turn_start` / tool_call+update /
  `agent_message_chunk` / `turn_completion` / `turn_end` per turn, plus `pending_interaction`+`interaction_resolved` (with `consentRound`)
  and `display_error` for the failed bogus-model turn; every replayed update carries `_meta.kiro.replay:true`; ~32-33 frames precede the
  load response. Replay `_meta.kiro` key set identical OLD/NEW.
- Abandoned prompt (both engines identical, 26 frames): the killed turn replays as `user_message_chunk` + `turn_start` + a SYNTHETIC
  `turn_end {stopReason:"cancelled", messageId:"<sid>-<turnId>-turn-end-synthetic", replay:true}` (frame 154 both); the partial streamed
  answer is not replayed. ⇒ the 0.66.34 `arrivedDuringLoad` suppression (kas lane) did not change this path; its race (messages arriving
  DURING the load) was not exercised — NOT-REACHED.
- Cross-version load: the 0.66.33 session loads on 0.66.34 with its persisted `memoryConfig {read_write, reflection:true}` — the new
  disabled default applies to NEW sessions only (load result `_meta.memoryConfig`, cross capture).
- cyril: `turn_end` → `Notification::TurnCompleted{stop_reason}` (convert/kas.rs:398-400) — the synthetic cancelled closure maps through the
  normal path; `replay` flag is not read in convert/kas.rs (load-window handling lives in the mediator, not verified here).

### Leg 3 / lead 3 — MCP on-demand (DONE 02:05-02:08 UTC)
Probe `probe-kas-leads-2.31.1.py mcp`; captures `kas-lead-mcp-{2.29.1,2.31.1}.jsonl`; offline stdio server
`audit-2.31.1/live/fixtures/mcp_probe_server.py` as workspace `.kiro/settings/mcp.json` server `probesrv` (tools probe_echo, probe_add);
one turn per arm ("use probe_echo … ZEBRA"); all 8 turns end_turn ~8 s with `ECHO:ZEBRA (server probesrv)`.

| arm (`initialize…settings`) | 0.66.33 | 0.66.34 |
|---|---|---|
| none | DEFERRED: model calls `tool_load` first, then the tool | identical |
| `toolSearch{enabled:false}` | DEFERRED (setting has no effect) | DEFERRED (no effect) |
| `toolSearch{enabled:true}` | DEFERRED, no `tool_search` tool used | identical |
| `toolSearch{neverDefer:["probesrv::probe_echo","probe_echo","@probesrv/probe_echo"]}` | NOT deferred: direct call | identical |

- **CORRECTS the kas static lane**: 0.66.33 already defers MCP tools behind `tool_load` for this account (`AB_TOOL_LOAD` was served ON
  by the backend). The 0.66.34 code change (deferral unconditional) is invisible on the wire for this account; it matters only for accounts
  whose AB was off. `toolSearch.enabled` is inert on BOTH engines (live), `neverDefer` works on both.
- Wire shapes (identical both engines; NEW frames 43-58 of `kas-lead-mcp-2.31.1.jsonl`, conn `mcp-none`):
  - `tool_load` emits ONLY a `tool_call_update` (no preceding `tool_call`): `{status:"completed", title:"Tool Load", rawInput:{tool_ids:["probesrv::probe_echo"]},
    rawOutput:{message:"{\"loaded\":[{tool_id,description,input_schema}],\"not_found\":[],\"note\":…}"}, _meta.kiro{toolOrigin:"default"}}`, no `kind`.
  - deferred MCP call: `tool_call {title:"@probesrv/probe_echo", kind:"other", rawInput:{tool_id:"probesrv::probe_echo", arguments:{text:"ZEBRA"}},
    _meta.kiro{serverName:"probesrv", toolOrigin:"default"}}` → update → completed with `rawOutput{response,imageBase64Urls,message}` + content.
  - neverDefer'd (direct) call: `rawInput:{text:"ZEBRA", _meta:{_isValid,_activePath,_completedPaths}}`, `toolOrigin:"client"`.
  - neverDefer value form: one of the three supplied matched (static: `server::tool` or tool id) — not isolated.
- cyril: the orphan `Tool Load` update is DROPPED SILENTLY — `UiState` ToolCallUpdated only updates a tracked id
  (`crates/cyril-ui/src/state.rs:560-575`, no log); the deferred call's args sit under `rawInput.arguments` with `tool_id` instead of the
  tool's own fields (TUI 2.31.0 unwraps `{tool_id, arguments}`, T11; cyril NOT-HANDLED); MCP calls are `kind:"other"`.
- `_kiro/mcp/status` (lead 7 / T4): `{sessionId, servers:[{name,status,tools[{name,description,disabled,inputSchema}],_meta.kiro.resource…}]}` —
  NO `accessMode` / `accessModeFilteredServers` key on either engine for this personal account.

### Leg 3 / lead 4 — `_kiro/workflow/new` launch defaults (DONE 02:08 UTC)
Probe `probe-kas-leads-2.31.1.py wfdefaults`; captures `kas-lead-wfdefaults-{2.29.1,2.31.1}.jsonl`; gate on (`workflowsEnabled:true` both).
| call | 0.66.33 | 0.66.34 |
|---|---|---|
| `/nonexistent/x.workflow.json` + `modelId:5` | `-32603 … workflowPath … is outside the allowed workspace roots (…)` (param ignored) | `-32603 Invalid workflow shape: [{code:"invalid_type",expected:"string",received:"number",path:["modelId"]}]` (validated BEFORE path) |
| same, no modelId (control) | outside-roots error | outside-roots error |
| `effortLevel:7` | outside-roots | `Invalid workflow shape … path:["effortLevel"]` |
| `modelId:"claude-sonnet-5"` (string) | outside-roots | outside-roots (string passes shape check) |
| valid one-step recipe + `modelId:"claude-haiku-4.5", effortLevel:"low"` | run completes on `model auto` (step session `config_option_update` model=`auto`); params silently ignored | run completes; step session `config_option_update` model=`claude-haiku-4.5`; `new` result `initialState.root.children[0]` and `inspect.state` carry `modelId:"claude-haiku-4.5", effortLevel:"low"` (state top level AND the step node; `nodePlan` too) |
- LIVE CONFIRMED (K5). Wire ADD on 0.66.34: workflow state / nodeTree gain `modelId` + `effortLevel` (top-level + per step node).
  The 0.66.33 `inspect.state` already carried `memoryConfig` + `memoryConfigSource:"legacy"`; on 0.66.34 the run's
  `memoryConfig` is `{disabled, reflection:false}` (memory flip reaches workflow state, source still `legacy`).
- cyril NOT-SENT (`crates/cyril-core/src/protocol/domain_mediator/commands/kas.rs:349-356` sends workflowPath/inputs/parentSessionId/workspacePaths only).
- Side note (model behaviour, not version): on haiku/low the step answered "I'm ready to receive the concrete instruction…" instead of `ONE`
  — captured output stays model-controlled (cyril-srp6 context).
- send_message unsavable-signal → `node_paused`: NOT-REACHED (needs a persistence failure; not producible without forging).

### Leg 3 / lead 6 — sandbox (DONE 02:09-02:10 UTC)
Probe `probe-kas-leads-2.31.1.py sandbox`; captures `kas-lead-sandbox-{2.29.1,2.31.1}.jsonl`. Zero turns. IDENTICAL on both engines:
| arm | `_kiro/sandbox/status` push |
|---|---|
| control | none |
| `session/set_config_option {configId:"sandbox",value:"enabled"}` then `{configId:"mcpSandboxing",value:"enabled"}` (no argv) | `{status:"active",backend:"bubblewrap",networkMode:"default_blocked"}` then `{…,mcpSandboxed:true}` |
| argv `--sandbox=auto --sandbox-network-mode=default_blocked` | `{status:"active",backend:"bubblewrap",networkMode:"default_blocked"}` at session/new |
| argv + `KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED=1` | same as argv |
| `KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED=1` only | none |
- `sandbox` / `mcpSandboxing` are accepted as hidden configIds: neither appears in `configOptions` before or after (the set returns the
  same 5 options mode/model/memoryReflection/autopilot/contentCollection). The env gate is TUI-only; a direct KAS spawn ignores it.
  The 2.31.0 "/sandbox GA" line has no KAS-side delta on the wire (the GA is the host/TUI gate, as the static lanes said).
- cyril: NOT-SENT (no sandbox configId, no `--sandbox` argv; `_kiro/sandbox/status` unhandled → `convert/kiro.rs:1284` `_ => Ok(None)`). NOTE cyril-6vo6/cyril-be7e.
- One NEW `session/new` result again lacked `model` (timing — models fetched async; arrived in a later `config_option_update`). Second
  occurrence of method note M2; not version-attributable (also seen on OLD tui_ref arm in A).

### Leg 3 / V1 — cyril agent-command dispatch on KAS (DONE 02:11 UTC) — FLIP BLOCKER CONFIRMED LIVE
Probe `probe-kas-v3cover-2.31.1.py v1`; captures `kas-v3c-v1-{2.31.1,2.29.1}.jsonl`. Seeded `$HOME/.kiro/skills/probe-skill/SKILL.md`,
`<ws>/.kiro/skills/ws-skill/SKILL.md`, `$HOME/.kiro/prompts/probe-prompt.md`, `<ws>/.kiro/prompts/ws-prompt.md`.
- `available_commands_update` (2 frames, both engines identical) advertises 6 commands: `general-task-execution`, `context-gatherer`
  (`_meta.kiro.type:"custom-agent"`, `input.hint:"task to delegate"`), `probe-skill`/`ws-skill` (`type:"skill"`, `originalName`,
  `resource.source.origin user|workspace`, `input.hint:"optional context"`), `ws-prompt`/`probe-prompt` (`type:"prompt"`,
  description `"(file prompt)"`, `input.hint:"[args]"`, `arguments:[{name:"args",description:"All arguments",required:false}]`).
- cyril's exact frame (`commands/mod.rs:496-541 AgentCommand::execute` → `domain_mediator/commands/extensions.rs:13-47`:
  wire method `_kiro.dev/commands/execute` — `send_extension` prefixes `_` — params `{sessionId, command:{command:<name>, args:{}|{value:"hello"}}}`)
  and the selection path `_kiro.dev/commands/options {command, sessionId, partial:""}`: **every one of the 6 commands × 3 calls = 18/18
  fail on BOTH engines** with `-32603 Internal error`, `data.details: '[PersistenceClassification] Ext method "_kiro.dev/commands/execute"
  has no persistence classification. Add it to KnownExtMethod in persistence-classification.ts.'` (same for `…/options`). Unprefixed
  `kiro.dev/commands/execute` and `_kiro/commands/execute` fail identically. Not a 0.66.34 regression — the method family does not exist in KAS.
- Control (one billed turn, NEW): `session/prompt "/probe-skill hello"` → end_turn 7.1 s; KAS model emits `tool_call "Load skill: probe-skill"
  rawInput{name:"probe-skill"}` → `tool_call_update "Loaded skill: probe-skill"` and answers `SKILLRAN hello`. ⇒ On KAS a skill/prompt
  command works when sent as prompt TEXT; cyril's ext-method dispatch breaks every KAS-advertised command (cyril-4c4d blocker #2 = REAL).

### Leg 3 / V3 — mode/model switching on KAS (DONE 02:12 UTC)
Probe `probe-kas-v3cover-2.31.1.py v3`; captures `kas-v3c-v3-{2.31.1,2.29.1}.jsonl`. Zero turns. IDENTICAL on both engines (modulo memoryReflection).
| call | response | notifications |
|---|---|---|
| `session/set_config_option {configId:"mode",value:"plan"}` | `{configOptions:[mode=plan, model, memoryReflection, autopilot, contentCollection]}` | `available_commands_update` + `config_option_update` (mode=plan); NO `current_mode_update` |
| `session/set_mode {modeId:"vibe"}` / `{modeId:"plan"}` | `{}` | `available_commands_update` + `config_option_update` (mode switches); NO `current_mode_update` |
| `set_config_option mode="bogus-mode-zz9"` | success, returns configOptions with mode=`vibe` (silently coerced to default) | acu + config_option_update(mode=vibe) |
| `set_mode {modeId:"bogus-mode-zz9"}` | `{}` success | config_option_update(mode=vibe) — silent fallback |
| `set_config_option model="bogus-model-zz9"` | success, configOptions model=`bogus-model-zz9` (accepted UNVALIDATED; fails at next prompt, leg 1 p3) | config_option_update(model=bogus) |
| `session/set_model {modelId:"bogus-model-zz9"}` | `-32603 [PersistenceClassification] Ext method "session/set_model" has no persistence classification` (also for a valid id — sweep census) | none |
- Both mode paths work and both report via `config_option_update` only (KAS never sends `current_mode_update`). Neither validates mode ids.
- `session/set_model` is NOT implemented by KAS (unlike v2 where it works); cyril's `BridgeCommand::SetModel` → `session/set_model`
  (`crates/cyril-core/src/protocol/domain_mediator/commands/session.rs:303-321`) fails on KAS; models must go through `set_config_option model`.

### Leg 3 / V5 — `_kiro/mcp/status` without openExternalUrl/secretStorage (from lead-3 captures, no extra spawn)
The lead-3 handshake advertised NO `openExternalUrl`/`secretStorage` (only `configurationState` + settings). `_kiro/mcp/status` on both engines:
`{sessionId, servers:[{name:"probesrv", status:"connecting"→"connected", tools:[{name,description,disabled,inputSchema}], _meta.kiro.resource{resourceType:"mcpServer",source{origin},krn,layer}}]}`;
no display_error. OAuth-requiring server: NOT-RUN (no safe test server; never complete an OAuth flow).

### Leg 3 / V2 — sub-agent interleave (DONE 02:13 UTC, one billed turn, NEW only)
Probe `probe-kas-v3cover-2.31.1.py v2`; capture `kas-v3c-v2-2.31.1.jsonl` (end_turn 11.4 s). 7 frames carry `agentSubtaskId`;
**all 7 arrive on the PARENT sessionId** (no child session id ever appears on the wire; 36 parent frames + 7 session-less):
- frame 37 `tool_call {toolCallId:"invoke_subagent_toolu_…", title:"Sub-agent: general-task-execution", kind:"other",
  rawInput{name, prompt, explanation, contextFiles[]}, _meta.kiro{kind:"agent-subtask", agentSubtaskId}}` + updates (same meta) closing it;
- child's own tool calls ride the parent stream: frame 41 `tool_call "Read File" kind:"read" _meta.kiro{agentSubtaskId, toolOrigin:"default"}`;
- child's result: frame 46 `tool_call_update {title:"Subagent Response", status:"completed", rawInput{response:"ALPHA",files:[]},
  rawOutput{message:"Response recorded."}, _meta.kiro{agentSubtaskId, toolOrigin:"acp"}}` (no standalone tool_call precedes it — orphan update, like Tool Load);
- the child streams NO `agent_message_chunk` to the client — its answer surfaces only as the `Subagent Response` rawInput.
- cyril: `agentSubtaskId`/`kind:"agent-subtask"` not read (convert/kas.rs agent_initiation reads only `agentInitiated`, :531-545);
  child tool calls interleave into the parent transcript; `invoke_subagent` call is `kind:"other"`; the orphan `Subagent Response` update is
  dropped by UiState (state.rs:560-575). Blocker #4 (KAS crew rendering, ROADMAP KAS-3) = REAL.
V4 compaction: no natural compaction in any leg (all sessions ≤ ~3 % context) — NOT-REACHED.

### Leg 3 / lead 9 (reframed) — V2→V3 agent migration vs cyril's trust append (DONE 02:14-02:16 UTC)
Part 1 zero-auth: `probe-host-agent-upgrade-2.31.1.py` (archived 2.31.1 `kiro-cli-chat agent upgrade`, temp HOME + temp XDG_DATA_HOME),
result `h5-agent-upgrade-2.31.1.json`, fixture dir `tmp/h5-ouix0k0e`. Fixture = V2 agent `probe.json` with
`toolsSettings.execute_bash.allowedCommands:["echo V2TRUST"]`, `fs_read.allowedPaths:["./docs"]`.
- Upgrade writes the universal format IN PLACE (+ `probe.json.bak`): tools `fs_read/execute_bash`→`read/shell`, V2 `toolsSettings` KEPT, new V3
  `permissions.rules[{capability:"shell",match:["echo V2TRUST"],effect:"allow"}, {fs_read ./docs allow}, {fs_read allow}, {skill allow}]`,
  and `kiroUpgrade{revision:2, fingerprint:"sha256:…", source:"sha256:…"}`.
| arm (after upgrade 1) | edit | upgrade 2 | V3 `permissions` shell rule after |
|---|---|---|---|
| A control | none | `= unchanged … already universal` | [echo V2TRUST] |
| B raw V2 edit (key order kept) | append "echo CYRILTRUST" to V2 allowedCommands | `✓ upgraded` (fingerprint+source re-stamped) | [V2TRUST, CYRILTRUST] |
| C **cyril emulation** (`json.loads` → append → `json.dumps(indent=2, sort_keys=True)` = serde_json Map default, kiro_agent_config.rs:156-251) | same append, keys re-sorted | `✓ upgraded` (identical fingerprint to B) | [V2TRUST, CYRILTRUST] |
| D cyril reformat only (sorted keys, no change) | none semantic | `= unchanged` | [V2TRUST] |
| E user edited V3 `permissions` (added a deny), then cyril append | — | `↷ skipped probe.json out of sync (not rebuilt)` | [V2TRUST] + deny — CYRILTRUST never reaches V3 |
- Fingerprint is SEMANTIC (key-order-insensitive): cyril's re-serialisation does NOT itself mark the file user-edited (D). The refresh
  freezes only when the V3 section diverges (E) — then every later cyril grant is stranded in the V2 section.
Part 2 (2 billed+1 turns, NEW): `probe-kas-v3cover-2.31.1.py l9` with `AGENT_FILE` = arm C's file right after cyril's append (before any
refresh: V2 has V2TRUST+CYRILTRUST, V3 has V2TRUST only), installed as `$HOME/.kiro/agents/probe.json`; capture `kas-v3c-l9-2.31.1.jsonl`.
- KAS 0.66.34 loads the upgraded agent (`probe` appears in modes; `set_config_option mode=probe` OK).
- `echo V2TRUST` → NO permission ask (V3 rule honoured); `echo CYRILTRUST` → permission ASKED; `echo UNTRUSTED` → asked.
  ⇒ **KAS reads permissions ONLY from the V3 `permissions.rules` section; the V2 `toolsSettings.<tool>.allowedCommands` that cyril writes
  (kiro_agent_config.rs:7-15, 212-251) is ignored by KAS.** cyril's "Always allow" persistence is therefore inert on the v3 default engine
  until someone re-runs `agent upgrade` (and permanently inert once the user has edited `permissions`). Flip blocker — proposed issue.

### Leg 3 / lead 8 — V3 ease-in on the v2 launcher (DONE 02:17 UTC, zero auth, demoted)
`probe-v2-easein-2.31.1.py` on archived 2.31.1 `kiro-cli acp`, temp HOME + temp XDG_DATA_HOME; capture `v2-easein-2.31.1.jsonl`.
Arms control / `KIRO_ENABLED_FEATURES='["v3_prompt"]'` / `KIRO_ROLLOUT_FORCE_INTERNAL=1`: all three exit rc=1 within 1.5 s with stderr
`error: You are not logged in, please log in with kiro-cli login` before reading stdin (initialize never answered). 0 bytes of
"V3"/"Switch"/"You're using Kiro CLI V2" on stdout or stderr; the only writes are `home/.kiro/settings/cli.json` = `{}` and
`xdg/kiro-cli/data.sqlite3` (identical in all arms; no `chat.agentEngine`). ⇒ No ease-in surface on `acp` up to the auth gate;
an authenticated `acp` initialize with these env arms was not run (would need the real data dir; demoted per refocus). PARTIAL.

## Legs

| leg | command / probe | engine(s) | arms | capture (lane dir unless noted) | duration | outcome |
|---|---|---|---|---|---|---|
| 0 | `node … acp-server.js --help` | 0.66.33, 0.66.34 | – | `tmp/help-{old,new}.{out,err}` | <5 s | no help surface; identical but for version |
| 0b | `probe-acp-init-2.31.1.py` | both | callback / none | `kas-init-{2.29.1,2.31.1}-{callback,none}.jsonl` | ~10 s each | one `extensionMethods` add |
| 1 | `probe-kas-live-2.31.1.py SCENARIO=sweep` | OLD then NEW | – | `kas-live-sweep-{2.29.1,2.31.1}.jsonl` | 32.3 s / 33.0 s | 3 turns each; **NEW re-run** (first run void: auth-wait, `…-run1-authwait.jsonl`) |
| 2a | same, `SCENARIO=workflow GATE=off` | OLD then NEW | – | `kas-workflow-gateoff-*.jsonl` | 140.6 / 160.1 s | 6/6 runs MATCH |
| 2b | same, `GATE=on` | OLD then NEW | – | `kas-workflow-gateon-*.jsonl` | 534.0 / 238.3 s | 11/11 MATCH; OLD +296 s = one natural backend stall |
| 3.1 | `probe-kas-leads-2.31.1.py kirohome` | both | unset/abs/rel/root/home | `kas-lead-kirohome-*.jsonl` | 76/77 s | relocation on NEW only |
| 3.A+C | `probe-kas-v3default-2.31.1.py handshake` | both | cyril_user/cyril_empty/tui_ref/control | `kas-v3d-handshake-*.jsonl` + debug kiro.log | 24 s | intake identical; memory flip |
| 3.B | `… fallback` | both | F0/F1/F2 | `kas-v3d-fallback-*.jsonl` | 18 s | no model_fallback frame; `modelFallback.declined` in debug log |
| 3.D | `… sessionload` | both + cross | sweep session / abandoned prompt | `kas-v3d-sessionload-{2.29.1,2.31.1,cross-0.66.33to34}.jsonl` | 29 s each | replay identical |
| 3.3 | `probe-kas-leads-2.31.1.py mcp` | both | none/ts_disabled/ts_enabled/never_defer | `kas-lead-mcp-*.jsonl` | 80/77 s | deferral identical on both |
| 3.4 | `… wfdefaults` | both | 5 calls + 1 run | `kas-lead-wfdefaults-*.jsonl` | 12 s | launch defaults NEW-only |
| 3.6 | `… sandbox` | both | 5 arms | `kas-lead-sandbox-*.jsonl` | 24 s | identical |
| 3.V1 | `probe-kas-v3cover-2.31.1.py v1` | NEW (+control turn), OLD | 6 commands × 3 calls | `kas-v3c-v1-*.jsonl` | 16 / 9 s | 18/18 fail on both |
| 3.V3 | `… v3` | both | 7 calls | `kas-v3c-v3-*.jsonl` | ~20 s | identical |
| 3.V2 | `… v2` | NEW | 1 turn | `kas-v3c-v2-2.31.1.jsonl` | 12.5 s | all tagged frames on parent |
| 3.9 | `probe-host-agent-upgrade-2.31.1.py` + manual arm E + `… l9` | archived 2.31.1 chat (no auth), NEW KAS | A-E, 3 turns | `h5-agent-upgrade-2.31.1.json`, `tmp/h5-ouix0k0e/E_upgrade.log`, `kas-v3c-l9-2.31.1.jsonl` | <1 min | V3-only permissions; freeze on V3 edit. **First run voided** (D-arm truncation bug + PROBE_SCRATCH unset), fixed and re-run |
| 3.8 | `probe-v2-easein-2.31.1.py` | archived 2.31.1 launcher (no auth) | control / v3_prompt / force_internal | `v2-easein-2.31.1.jsonl` | 5 s | auth gate exits first; partial. **First run crashed** (BrokenPipe), fixed and re-run |
| 4 | `sweep-new-fields.py --diff` | per pair | – | `sweep-new-fields-diff-{sweep,wfoff,wfon,v2}.txt` | – | see below |
| 5 | `probe-v2-live-sweep-2.31.1.py` | archived v2 2.29.1 / 2.31.1 (bin/ first on PATH; proc_tree exe = archive kiro-cli + kiro-cli-chat) | – | `v2-live-sweep-*.jsonl` | ~35 s each | IDENTICAL (run last, token healthy) |

Bundled, trimmed copies are in `experiments/conductor-spike/<topic>-2.31.1.jsonl` (built by `live/trim_captures.py`; each part has a header record). Frame indices in this file refer to the lane-dir captures. The v2 counterpart of lead 1 was dropped per the refocus. No orphan processes remained after any leg; the one `acp-server.js` left running is the user's own installed-tree KAS.

## Deltas

| item | version | gate/default | direction | LIVE | cyril HANDLED/DROPPED/NOT-SENT + file:line | proposed issue? |
|---|---|---|---|---|---|---|
| `agentCapabilities._meta.kiro.extensionMethods` + `_kiro/environment/start_ssh_session` (27 → 28) | 0.66.34 | default | ADD | LIVE `kas-init-2.31.1-callback.jsonl` init_result | NOT-SENT/not parsed (`protocol/fingerprint.rs:138` fixture only) | NOTE cyril-tikf |
| `_kiro/environment/start_ssh_session`: `{}` → -32602 "requires sessionId as a string of 1-128 characters"; any sessionId → -32000 `SshSessionUnavailableError` (OLD: -32603 PersistenceClassification) | 0.66.34 | needs `KIRO_REMOTE_SESSIONS_ENDPOINT` (unset) | ADD (dark) | LIVE sweep pre/census | NOT-SENT (0 hits in crates/) | NOTE cyril-tikf |
| KIRO_HOME → userConfigDir (agents, steering, settings/mcp.json, logs, sessions, session-index); relative/root/home ignored with `agent.userConfigDir.ignored{reason}` | 0.66.34 | env | CHANGE | LIVE `kas-lead-kirohome-*` | sidecar `usage/kiro_sidecar.rs:512-547` + settings `protocol/kas/settings.rs:50-59` HANDLED for absolute; DIVERGES: global agents `kiro_agent_config.rs:102-104`, hooks `domain_mediator/mod.rs:262-266`, verbatim value `kiro_agent_config.rs:93-98` | **yes** (KIRO_HOME parity, one issue) |
| memory default `{read_write,true}` → `{disabled,false}` (session/new `_meta.memoryConfig`, `memoryReflection` on → off, config/template, workflow state) for NEW sessions; loaded old sessions keep theirs | 0.66.34 | no `memory` key sent | CHANGE | LIVE `kas-live-sweep-*` f26, `kas-v3d-handshake-*`, cross-load | NOT-SENT (cyril sends no `memory`, settings.rs BOOL_MAP/DEFAULTS_ON) | NOTE cyril-0na7/cyril-s21o |
| `_kiro/workflow/new` `modelId`/`effortLevel` validated before path; step session runs on the given model; state + nodes carry `modelId`/`effortLevel` | 0.66.34 | optional | ADD | LIVE `kas-lead-wfdefaults-*` | NOT-SENT `domain_mediator/commands/kas.rs:349-356` | NOTE cyril-4u4a |
| bundled `feature-pipeline` steps `effortLevel:"high"` ×2 | 0.66.34 | bundled | CHANGE | LIVE sweep f136 | n/a | no |
| MCP deferral via `tool_load` (orphan `tool_call_update` "Tool Load" `{tool_ids}`; call rawInput `{tool_id, arguments}`) | **both** (0.66.33 already, AB on) | AB/default | UNCHANGED for this account | LIVE `kas-lead-mcp-*` f43-58 | DROPPED silently: orphan update ignored `crates/cyril-ui/src/state.rs:560-575`; `{tool_id,arguments}` not unwrapped | **yes** (render tool_load + unwrap; ties cyril-2yn8) |
| `toolSearch.enabled` (true/false) no effect; `neverDefer` un-defers | both | – | NEGATIVE | LIVE `kas-lead-mcp-*` | cyril SENDS `toolSearch{enabled,minPct,minTokens}` (`settings.rs:120-131`), inert; `neverDefer` NOT-SENT | NOTE cyril-q8gq/ea67 |
| `_kiro.dev/commands/execute` / `/options` → -32603 PersistenceClassification for every KAS command (skill, prompt, custom-agent) | both | – | GAP | LIVE `kas-v3c-v1-*` | BROKEN: `commands/mod.rs:496-541` + `domain_mediator/commands/extensions.rs:13-47` | **yes** (flip blocker) |
| KAS reads only V3 `permissions.rules`; V2 `toolsSettings.execute_bash.allowedCommands` ignored | 0.66.34 | – | GAP | LIVE `kas-v3c-l9-2.31.1.jsonl` + `h5-agent-upgrade-2.31.1.json` | BROKEN: trust persistence writes V2 only, `kiro_agent_config.rs:146-251` | **yes** (flip blocker) |
| `agent upgrade` semantic fingerprint: key reorder = unchanged; V2 edit → refresh; V3 edit → "out of sync (not rebuilt)" | 2.30.0+ host | explicit / auto-upgrade | ADD | LIVE (no auth) | cyril's sorted re-serialisation is safe; stranded grants after V3 edit | same issue |
| sub-agent: `agentSubtaskId` frames on PARENT sessionId; child tool calls interleave; `Subagent Response` orphan update; no child text stream | 0.66.34 (shape) | – | – | LIVE `kas-v3c-v2-2.31.1.jsonl` f37-46 | DROPPED: `agentSubtaskId`/`kind:"agent-subtask"` unread (`convert/kas.rs:525,531-545`); orphan update dropped (`state.rs:560-575`) | NOTE ROADMAP KAS-3 (flip blocker) |
| `session/set_model` → -32603 on KAS; `set_config_option model` accepts any value unvalidated | both | – | GAP | LIVE `kas-v3c-v3-*`, sweep | cyril `SetModel` → `session/set_model` (`commands/session.rs:303-321`) fails on KAS | maybe (route via config option on KAS) |
| mode switch: `set_config_option mode` and `set_mode` both work and both report via `config_option_update` only (no `current_mode_update`); bogus mode silently → `vibe` | both | – | – | LIVE `kas-v3c-v3-*` | (cyril set_mode `commands/session.rs:282`) — UI must listen to config_option_update | NOTE picker issue |
| intake: `subagentOrchestration`, `todoList` unknown-key; `thinking` unread-key; TUI's `largeToolOutputHandler`, `workflowNotifications` unknown; `workflows` unread at initialize | both | – | – | LIVE debug kiro.log (handshake) | cyril SENDS dead keys `settings.rs:82,93-98` | NOTE cyril-53qx |
| clientInfo `cyril` → `clientAttribution:"unrecognized"`, inferred client `kiro-ide` | both | – | – | LIVE debug log | `types/present_as.rs:43-47` (default Cyril) | maybe (decide PresentAs for KAS) |
| `_kiro/governance/state` pushed at session/new `{sessionId,isEnterprise,features{7 bools}}`, no `disabledReason` | both | – | UNCHANGED (static said new parser only) | LIVE sweep f17 | DROPPED `convert/kiro.rs:1284` | NOTE cyril-0o7e |
| `_kiro/config/template {}` pre-session → `{modes, configOptions[mode,memoryReflection,autopilot,contentCollection,outputStyle]}` | both | – | – | LIVE sweep f9 | NOT-SENT | NOTE cyril-lyrx |
| `_kiro/mcp/status` has no `accessMode` for a personal account | both | – | – | LIVE `kas-lead-mcp-*` | DROPPED `convert/kiro.rs:1284` | NOTE cyril-nk4o |
| sandbox via hidden configIds `sandbox`/`mcpSandboxing` or argv `--sandbox=auto` → `_kiro/sandbox/status{status:"active",backend:"bubblewrap",networkMode,mcpSandboxed?}`; rollout env ignored | both | – | UNCHANGED | LIVE `kas-lead-sandbox-*` | NOT-SENT; status DROPPED `convert/kiro.rs:1284` | NOTE cyril-6vo6/be7e |
| abandoned turn replays as synthetic `turn_end{stopReason:"cancelled", messageId:"…-turn-end-synthetic", replay:true}` | both | – | UNCHANGED | LIVE `kas-v3d-sessionload-*` f154 | HANDLED as TurnCompleted `convert/kas.rs:398-400` | no |
| `modelFallback.declined {trigger:"availability",reason:"notModelAttributable"}` without flag env (availability fallback already on) | both | AB/backend | – | LIVE debug kiro.log (fallback) | n/a (log only); frame DROPPED `convert/kas.rs:525` | NOTE cyril-aoby |
| v2 `kiro-cli acp` sweep | 2.29.1 → 2.31.1 | default | UNCHANGED | LIVE `v2-live-sweep-*` (320 = 320) | n/a | no |

## Lead verdicts

1. **KIRO_HOME: CONFIRMED** (KAS). NEW relocates everything and logs `agent.userConfigDir.resolved/ignored{reason}`; OLD ignores it.
   - Correction: there is no "ignoring KIRO_HOME" text on the KAS side.
   - Hooks relocation stays STATIC: `_kiro/hooks/list` is v2Hooks-gated.
   - The v2 counterpart was DROPPED per the refocus.
   - cyril resolves the same dir only for an absolute value, and only for sidecar and settings.
2. **extensionMethods / new methods: CONFIRMED with corrections.**
   - `start_ssh_session` was added; -32602 and -32000 were exactly as predicted.
   - `_kiro/config/template` and `_kiro/codeIntelligence` were already listed on 0.66.33 (not new).
   - `_kiro/governance/state` is not in the list (it is a notification) and is pushed unsolicited on both engines (not new).
   - Corrects the TUI lead: `config/template` has no `agents` key.
3. **MCP on-demand: REFUTED as a version delta.** Deferral is identical on both engines for this account.
   - `toolSearch.enabled:false` has no effect on either engine (CONFIRMED); `neverDefer` un-defers (CONFIRMED).
   - rawInput `{tool_id, arguments}` is CONFIRMED.
   - New: `tool_load` is an orphan `tool_call_update`.
4. **Workflow launch defaults: CONFIRMED.**
   - `modelId:5` → NEW "Invalid workflow shape"; OLD returns a path error.
   - The NEW step ran on `claude-haiku-4.5`.
   - The unsavable `send_message` → `node_paused` path is NOT-REACHED.
5. **Memory default: CONFIRMED.**
   - Surfaces: session/new `_meta.memoryConfig`, `memoryReflection`, config/template, workflow state.
   - Correction: `memoryConfigSource` exists only in workflow state (`"legacy"`), not on session/new. Existing sessions keep their memory setting.
6. **Sandbox: CONFIRMED unchanged.**
   - `set_config_option sandbox` works without argv; argv works without the env; `KIRO_LOCAL_SANDBOX_ROLLOUT_ENABLED` has no KAS effect.
   - configOptions never advertise sandbox.
7. **mcp/status `accessMode`: NOT PRESENT** for a personal account on either engine.
8. **v3 ease-in: PARTIAL.** `acp` exits at the auth gate with a temp data dir, with no prompt bytes and no `agentEngine` write. The authenticated arm was not run (demoted).
9. **Agent-upgrade (reframed): CONFIRMED, and it is a blocker.**
   - The fingerprint is semantic, so cyril's sorted serialisation does not freeze the refresh.
   - KAS honours only V3 `permissions.rules`, so cyril's V2-only grants are inert until a refresh.
   - A user edit to V3 freezes the refresh ("out of sync").
10. **Passive:**
    - No model_fallback, refusal or compaction occurred.
    - One 301 s silent backend stall occurred on 0.66.33 (gate-on linear step).
    - No 15 s timeout was observed.
11. **v2 acp growth: no wire change.** The v2 pair is IDENTICAL (2.29.1 vs 2.31.1). The same-day 2.30.0 vs 2.31.0 pair was not run.
- **A (cyril-exact handshake):** delivered as a key-by-key table in Leg 3/A+C. Intake is identical on 0.66.33 and 0.66.34. `toolSearch` is intake-accepted (no unknown-key) but inert.
- **B (model_fallback):** the frame is NOT-REACHED. `model_fallback_promoted` has 0 hits anywhere. The decline path is LIVE in the debug log.
- **C (memory with cyril's handshake):** CONFIRMED disabled on 0.66.34 and read_write on 0.66.33.
- **D (session/load):** replay identical. The abandoned-turn closure shape is LIVE. The `arrivedDuringLoad` race is NOT-REACHED.
- **V1 / V2 / V3 / V5:** CONFIRMED as above. V4 (compaction) is NOT-REACHED.
- **V3b effortLevel validation (zero turns, NEW only; `probe-kas-v3cover-2.31.1.py effort`, capture `kas-v3c-effort-2.31.1.jsonl`): VALIDATED BY SILENT IGNORE.**
  - At session/new (model `auto`) there is NO `effortLevel` configOption. `model` options carry `_meta.kiro{rateMultiplier, rateUnit, hasEffort, effortSchemaPath, effortLevels, defaultEffortLevel, thinkingToggleable}`; `auto` has `hasEffort:false`, Claude models have `effortLevels:["low","medium","high","xhigh","max"]`.
  - `set_config_option model=claude-opus-5.5` adds `effortLevel` (currentValue = the model's `defaultEffortLevel` `medium`, options = its effortLevels) to the response and to `config_option_update`.
  - `effortLevel="none"` and `"bogus-zz9"` both return SUCCESS with effortLevel still `medium`, plus a `config_option_update` repeating `medium`. There is no error: the invalid value is dropped silently.
  - `high`/`xhigh`/`max` apply.
  - ⇒ A client must read back `currentValue`; a success response does not mean the level was applied. (Contrast `model`, which is accepted unvalidated — V3.)
- **V1 KAS-native invocation (static, TUI 2.31.1):** `oZ` (prompt/skill/steering) only filters and sorts the slash menu. On submit, the TUI sends the line as plain prompt text: `session/prompt {sessionId, prompt:[{type:"text",text:"/name args"}], _meta.kiro{outputStyle?, displayText?}}` (`hnt`/`bnt`/`KMn`; no `resource_link`, no `_meta.kiro.type`). KAS expands it agent-side. LIVE: the V1 control turn (`/probe-skill hello` as text) produced `tool_call "Load skill: probe-skill"` → `SKILLRAN hello`. No further turn was spent (orchestrator).

## sweep-new-fields results per pair

- sweep (`sweep-new-fields-diff-sweep.txt`): NEW 695 / OLD 693 paths.
  - Only-NEW: `result.recipes[].plan[].steps[].effortLevel` (real: feature-pipeline) and `params.update.rawInput.cwd` (model coverage).
  - Only-OLD: none.
- workflow gate-off (`…-wfoff.txt`): 1115 / 1117.
  - Only-NEW: `rawInput.cwd` (coverage).
  - Only-OLD: `permissions[].meta_kiro.workflowWatch.*` — a trap-11 artifact; the literal count is 3 = 3.
- workflow gate-on (`…-wfon.txt`): 1302 / 1303. Only-OLD: `rawInput.cwd` (coverage).
- v2 (`…-v2.txt`): 320 = 320, IDENTICAL.
- The lead captures are not field-diffed; their deltas are read directly above.

## Method notes

- **M1:** `largeToolOutputHandler` is NOT a valid positive control. It is an unknown key on both engines (2.29.1 T-ltoh, re-confirmed: `intake.unknown-key`). Use `configurationState:true` → `_kiro/configuration/state` push. Every probe here did.
- **M2:** if the init-time `_kiro/auth/getAccessToken` reply is delayed (driver waiting out the <200 s window), session/new blocks. Separately, `session/new` can return WITHOUT the `model` configOption because models load asynchronously and arrive in a later `config_option_update`. This was seen 3× and is timing, not a version delta.
  - Re-run any leg whose session/new waited on auth.
  - cyril must not treat a missing `model` option in session/new as "no models".
- **M3:** KAS debug output (`KIRO_LOG_LEVEL=debug`) goes to `$HOME/.kiro/logs/<ts>/kiro.log` (or `<KIRO_HOME>/logs`), NOT to stderr. Probes that grep stderr for `intake.unknown-key` or `modelFallback.*` see nothing.
- **M4:** "orphan `tool_call_update`" is a recurring KAS shape (`Tool Load`, `Subagent Response`): an update with a `toolCallId` never announced by a `tool_call`. Differs and renderers keyed on tool_call creation miss them.
- **M5:** set `PROBE_SCRATCH` for every helper. `tempfile` otherwise lands in the harness tmp outside the lane; one fixture went to `~/.claude/tmp` and was removed.
- **M6:** do not commit `live/tmp/` (191 MB of temp HOMEs/workspaces). Commit the lane captures, FINDINGS, `wf_parity.py`, `trim_captures.py`, `fixtures/`, `h5-agent-upgrade-2.31.1.json` and the bundled `<topic>-2.31.1.jsonl` copies.
- Scripts added in `experiments/conductor-spike/`: `probe-kas-live-2.31.1.py`, `probe-acp-init-2.31.1.py`, `probe-kas-leads-2.31.1.py`, `probe-kas-v3default-2.31.1.py`, `probe-kas-v3cover-2.31.1.py`, `probe-v2-live-sweep-2.31.1.py`, `probe-v2-easein-2.31.1.py`, `probe-host-agent-upgrade-2.31.1.py`.
