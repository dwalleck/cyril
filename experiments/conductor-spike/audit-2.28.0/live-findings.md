# LIVE lane — kiro-cli 2.26.0 vs 2.28.0, same day (2026-10-08), both engines

Scratch: `scratchpad/audit-2.28/live/` (full captures, `leads/`, `tmp/` homes+workspaces).
Repo (uncommitted): `experiments/conductor-spike/`
- captures: `v2-live-sweep-{2.26.0,2.28.0}.jsonl`, `kas-live-sweep-{2.26.0,2.28.0}.jsonl`,
  `kas-workflow-live-{2.26.0,2.28.0}.jsonl` (gate on+off EXCERPT, ~1 MB each: message chunks, context_usage,
  focus_update, noisy ext-notifs and repeated config/command lists dropped; each record has `gate`). Full
  captures (~2.1 MB gate-on, ~1 MB gate-off) stay in scratch as `kas-workflow-live-gate{on,off}-<ver>.jsonl`.
- probes: `probe-v2-live-sweep-2.28.0.py`, `probe-kas-live-2.28.0.py` (SCENARIO=sweep|workflow|lib),
  `probe-kas-leads-2.28.0.py` (config/history/orch/bg/cascade/telemetry/stepnote/wfextra/always),
  `probe-v2-leads-2.28.0.py` (main/agentnf/dropend/sandbox).
- Record format `{ts, conn, dir, tag, msg}` (+ `probe:` preflight/proc/result records). Auth redacted;
  credential scan of every capture clean.

## 0. What ran — exe proofs

| leg | spawn | proof |
|---|---|---|
| v2 2.26.0 | archive `bin/` PREPENDED to PATH, `kiro-cli acp` | /proc walk: `binaries/2.26.0/.../kiro-cli` (e2070c01) -> `binaries/2.26.0/.../kiro-cli-chat` sha256 **0b0533d0e4ef6dbb** |
| v2 2.28.0 | same | `binaries/2.28.0/.../kiro-cli` (a6292871) -> `kiro-cli-chat` **ae5e172a905bd007** (= installed `~/.local/bin/kiro-cli-chat`) |
| KAS 0.66.15 | DIRECT `~/.local/share/kiro-cli/node --experimental-wasm-modules <kas-carves/2.26.0/.../acp-server.js> --transport=stdio --auth=acp-callback` (cyril's KAS-Free path; same argv the 2.28.0 TUI host uses, observed on the user's live TUI process) | preflight sha256 **9eb39c0e3c63**, package.json 0.66.15, stderr `kas.server.starting "version":"0.66.15"`; node v22.22.2 |
| KAS 0.66.26 | same, `kas-carves/2.28.0` | sha256 **3bc21b1f684c**, 0.66.26 |

No Rust host in the KAS path, so the 2.23.1+ "stale V3 engine" cleanup could not fire. Every spawn:
`HOME=<fresh tmp>`, real `XDG_DATA_HOME`. All runs strictly serial. The only other KAS process on the box was
the user's own interactive `kiro-cli --v3` TUI (started ~22:00Z from fish in the repo cwd; untouched). A few
`proc` records show `argv:[]` (the /proc read raced the exec); those legs' versions are proven by the
preflight sha and the server stderr.

Auth: `_kiro/auth/getAccessToken` answered from the sqlite store (profileArn from `state`). The single-flight
post-expiry renewer was armed but **never fired** (`auth_events: []` in every run). The store rolled
22:52Z -> 23:49Z on its own (the user's TUI host renewed it). Token row present at end (exp 23:49:33Z). No
login/logout run.

## 1. v2 engine (2.26.0 vs 2.28.0, identical workload)

Workload: initialize -> session/new -> settle -> p1 file read -> p2 shell (permission, allow_once) ->
`_kiro.dev/commands/options` model|agent|context -> `commands/execute` context|tools|usage ->
session/list, set_config_option -> set_model valid/restore/bogus -> p3 -> session/close.

The method inventory is identical on both: `_kiro.dev/{commands/available x1, metadata x14, session/update x2,
subagent/list_update x1}`, `session/request_permission x1`, session/update {agent_message_chunk 2, tool_call 2,
tool_call_update 3}. The initialize result is identical except `agentInfo.version`. The session/new keys are
`models, modes, sessionId` on both, with identical models (21) and modes.
`session/list`, `session/set_config_option` and `session/close` return `-32601` on both (`sessionCapabilities:{}`).

**Field diff: 3 new paths, 0 removed:**
- `result.options[].fallback{eligible, vendorDefault[]}` on `_kiro.dev/commands/options {command:"model"}`
  (2.28.0 only). Examples: `claude-opus-5.5 -> {"vendorDefault":["claude-opus-4.8"],"eligible":true}`,
  `claude-sonnet-5.5 -> ["claude-sonnet-5"]`, `claude-opus-5 -> ["claude-opus-4.8"]`, all others `[]`.
  After `/model fallback` is set, the block gains `configured:"claude-haiku-4.5"`. `readOnly`/`configuredUnreadable`
  (TUI-lane names) were NOT observed. **DROPPED** by cyril (`kiro::parse_options_response` reads only
  value/label/name/description/group/current).

**Model fallback (T-L2/H-L1/H-L2):**
- `commands/execute {command:"fallback", args:{targetModelId}}` on 2.28.0 returns `{"success":true,"message":
  "Cleared the configured fallback for claude-sonnet-5."}`. With `{targetModelId, fallbackModelId}` it returns
  `"Fallback for claude-sonnet-5 set to claude-haiku-4.5."`. A bogus target returns `{"success":false,"message":"Unknown model:
  bogus-model-zz9. ..."}`, and `args:{}` returns `-32700 Parse error data.error "missing field targetModelId"`.
  On 2.26.0 every call returns `{"success":false,"message":"Model fallback is not available."}` and the options carry no
  `fallback` block (dark there). The setting is session-scoped: the isolated `~/.kiro/settings/cli.json` stayed `{}`.
- `session/set_model` with a valid id returns `{}`. **A bogus id also returns `{}` on BOTH** (still no validation). The next
  prompt fails with `-32603 "Encountered an error in the response stream: The model 'bogus-model-zz9' is not
  available..."` and zero session/update frames. **This is identical on 2.26.0 and 2.28.0: no fallback rescue and no `model_fallback` frame.**
  A bogus id cannot carry a configured fallback, and "unavailable/refused" for a real model could not be provoked.
  The `model_fallback` wire shape is still UNVERIFIED live.

**Other v2 leads (2.28.0, with 2.26.0 control where run):**
- H-L3 `acp --agent nonexistent-agent-zz9` emits `_kiro.dev/agent/not_found {sessionId, requestedAgent,
  fallbackAgent:"kiro_default"}`. **The keys are identical on 2.26.0 and 2.28.0**: sessionId is already there and no
  4th field was observed. A session-level switch via `commands/execute {command:"agent", args:{value:<missing>}}`
  returns `{"success":false,"message":"Unknown agent: ... Run /agent ..."}` with NO not_found notification (both).
- T-L3/H-L4 `KIRO_TRUST_CLASSIFIER_SHADOW=1` + `_LOG=<file>`: **no log file was created** and there was no wire
  delta on either version. Answering the permission with result `_meta:{provenance:"human"}` versus no meta made no
  observable difference. This is likely gated by the `trust_classifier_shadow` rollout (internal-nightly), so the env
  var alone is not enough. Permission requests carry `_meta.trustOptions[{label, display, setting_key:"allowedCommands",
  patterns[]}]` on both versions (pre-existing).
- H-L5 `KIRO_TEST_DROP_FIRST_END_TURN=1`: both prompts returned `end_turn` normally (~6 s) with no stall and no
  wire delta. The env var is not honored on the `acp` path (or only in test builds).
- H-L6 `.kiro/sandbox.json {"enabled":true}` in both the workspace and the fake HOME: init and session/new were
  clean. stderr was empty, nothing reached the wire and nothing appeared in `kiro-chat.log`. The warning did not
  reproduce, but the schema was guessed, so this is UNVERIFIED.

## 2. KAS baseline sweep (0.66.15 vs 0.66.26, identical workload)

Workload: initialize (caps fs+terminal, `_meta.kiro.configurationState:true`) -> session/new -> settle ->
p1 read -> p2 shell -> 22-method read-only census -> set_model x3, set_config_option model bogus -> p3 ->
session/close. On both versions: init ok, turns end_turn, sessionCapabilities `{list, close, delete,
fork{_meta.kiro.messageId}}`, and the same 26 extensionMethods (all 14 `_kiro/workflow/*` control methods ARE advertised).

New on 0.66.26 (field diff: 32 new, 3 "removed"):
- **`_kiro/configuration/state`** notification (opt-in via `clientCapabilities._meta.kiro.configurationState`).
  The live shape corrects the static lane's `standing/decidedBy`:
  `{view:{observer:"connection"|"session", sessionId?}, layers[{id,name}], settings[{krn, value, compose,
  assertions[{layer,compose}], contributions[{layer, stated{value,used}}]}], resources[]}`.
  - The connection view arrives right after initialize: 8 layers, `settings:[]`. Initialize-level settings are
    NOT represented.
  - The session view arrives after session/new or session/load: 11 layers (adds workspace, agent and session),
    with settings such as `krn:::setting/workflows off`, `krn:::setting/fta off`, `krn:::setting/thinking on`.
  - The `client-connection` layer name is clientInfo.name.

  **NOT-SENT/DROPPED** by cyril (no opt-in, no handler).
- **configOption `memoryReflection`** (select on/off, `description:"Allow this session to reflect on new
  memory"`) is inserted between model and autopilot. It is HANDLED generically by `convert::to_config_options`
  (key/label/value/options); `configOptions[].description` is DROPPED.
- **Watch handler `background-process`** appears in `_kiro/workflow/listWatchHandlers` (configSchema
  `{command, cwd, pollIntervalSec, commandTimeoutSec, waitSec<=3600, outputTailLines<=1000}`, defaultPollIntervalSec 1).
  cyril never calls listWatchHandlers (N/A).
- `_kiro/configuration/contribute` returns `-32603 [PersistenceClassification] ... no persistence classification`
  on 0.66.15 and `-32603 "Unknown ext method: _kiro/configuration/contribute"` on 0.66.26. It is classified but no
  handler is wired standalone, which matches static A2.
- The "removed" `configOptions[]._meta.kiro.promptScoped` is a **differ artifact**. `sweep-new-fields.py` walks
  only the first 5 array elements, and inserting memoryReflection pushed `outputStyle` (which carries promptScoped)
  to index 5. It is present in both captures. This is a tooling caveat for future audits.
- `rawInput.depth/explanation` comes from a List Directory call the model chose to make (coverage, not a change).
- Model fallback on KAS:
  - `session/set_model` returns `-32603 [PersistenceClassification]` on both versions; KAS does not implement it.
  - `set_config_option model=bogus` is accepted silently on both.
  - The next prompt fails with `-32000 "The model 'bogus-model-zz9' is not available..."`
    (`data{errorType:"InvalidModelError", retryErrorType:"CLIENT_ERROR"}`), plus a session_info_update `display_error`
    and `turn_end stopReason:"error"`.

  All of this is identical on both versions. No fallback.

## 3. WORKFLOW lane (both versions x gate on/off, identical recipes + control script)

Gate ON is the TUI 2.28.0 settings set, sent at BOTH initialize and session/new `_meta.kiro.settings`:
- `workflows` and `goal` set to `{enabled:true}`.
- `workflowNotifications {enabled:true, delivery:"steer"}`. This is the TUI's real key, but KAS 0.66.x has 0
  `workflowNotifications` hits, so it is TUI-only.
- The TUI defaults: codeIntelligence, knowledge, thinking, largeToolOutputHandler.

The `_meta.workflowsEnabled` echo was true/false as expected.

Recipes (in the workspace `.kiro/workflows/` unless noted):
- linear: 2 steps, `{{s1.output}}`, runLabel.
- loop: repeat max 1, onMaxIterations pause, unreachable stopCondition.
- loopnone: repeat max 2, no stop condition, continue.
- branches: a **YAML** recipe; parallel joinPolicy all, then a join step.
- watch: repeat + `command` watch handler, `stopWhen w.terminal`.
- bgwatch: `background-process` watch.
- park: step with `completion.containsText`.
- fail: step with a bogus `modelId`.
- broken: missing prompt, so it gets a validationError.
- late: written and then edited mid-session.
- usertier: user-tier recipe at `~/.kiro/workflows/usertier.workflow.yaml` (isolated HOME).
- `agent://wf-coder`.
- A custom agent at `.kiro/agents/wf-launcher.json` (`tools:[run_workflow, inspect_workflow, read_file]`, with the
  permissions marker).

Per-run results (events identical across versions unless noted):

| run | ops exercised | 0.66.15 | 0.66.26 |
|---|---|---|---|
| linear | new(runLabel), invoke, `_session/steer` on s1, pause, inspect, load, list, resume, complete, wake | completed; steer `{queued:true,messageId}`; pause `{paused:true}` lands as node_paused+paused+run_complete:paused | identical |
| loop | cap pause -> inspect -> `update replace_remaining` `{updated:true,queued:false,"Applied: the remaining steps were replaced."}` -> resume `extendRepeat{loop,+1}` -> paused again -> cancel(initiator/reason) -> aborted+wake -> retry `{retriedNodeIds:["loop","after2"]}` -> paused -> cancel -> aborted+wake -> delete `{ok:true}` -> inspect `-32603 not registered...` | as described | identical |
| watch (command) | 1 watch_poll terminal-state -> responder | completed | identical |
| bgwatch | background-process watch | **new -> `-32603 "Watch handler 'background-process' is not registered."`** | completed; 3 watch_poll (idle, idle, terminal-state); capturedOutput JSON `{terminalId, status:"exited", exitCode:0, signal:null, startedAt, outputFile:"$TMPDIR/kiro-bg-<uid>/workflow-watch/<term>.log", outputTail:"BGDONE"}`. Runs IN KAS, with no client terminal/create |
| park | interactive step parks (node_paused+paused+run_complete:paused) -> `session/prompt` on the STEP session "FINISHED" -> completed | ok | identical |
| loopnone / branches(YAML) / usertier / agent:// | | completed | identical |
| fail | step bogus model -> failed -> retry nodeId -> failed | failed (no model fallback) | identical |
| launcher (gate on) | parent prompt `_meta.kiro.modeId:"wf-launcher"` -> model calls run_workflow -> run -> wake | completed (runLabel null) | identical |
| reload | 2nd connection: session/load parent (replays 100+ user_message_chunk, turn_completion `replay:true`) -> workflow/list -> resumeAll (all skipped "terminal status") -> load(linear) | ok | ok, plus `_kiro/configuration/state` x2 |

**Auto-wake (cyril-lki9):** EVERY terminal run_complete (completed, failed, or aborted by cancel) triggers an
unsolicited parent turn:
- Frame sequence: `focus_update x2 -> turn_start -> ... -> turn_completion{promptTurnSummaries[{unit:"credit",
  usage~0.10-0.38, usedTools[...]}], requestIds[], elapsedTime, status} -> turn_end`. There is NO
  user_message_chunk and no session/prompt.
- Ordering is identical on both versions: `node_complete -> run_complete -> (<=40 ms) parent focus_update, turn_start`.
- Gate OFF wakes too; the turns are shorter because there are no workflow tools.
- Wake usage accounting is on the wire on **both** 0.66.15 and 0.66.26. There is no KAS-side change for the 2.27.1
  "wake-turn usage accounting" item; that change is in the host/TUI.
- A paused run_complete does not wake.

**Permission routing:**
- Approvals for watch and background-process commands arrive on the **PARENT session**. They carry
  `_meta.kiro {toolId:"execute_bash", command, workflowWatch{workflowId,nodeId}, consent{capability:"shell",
  resource,...}}` and toolCallId `workflow-watch-<wf>-<node>-<n>`.
- Step tool approvals arrive on the step session.

**Recipes:**
- `recipes_changed {sessionId, workspacePaths, recipes[]}` fired x2 (add + edit) with gate ON only, 0 times with gate
  OFF, on both versions.
- listRecipes row keys: `_meta, builtIn, description, inputs, name, plan, source, validationError`. The broken
  recipe gets `"Invalid workflow: steps.0.prompt: Required"` on both.
- YAML and user-tier recipes are listed and runnable on both.
- **`_kiro/workflow/new` REQUIRES `inputs`**, even `{}`. Omitting it returns
  `-32602 {'inputs': {'_errors': ['Required']}}` on both (my first attempt tripped it).

**Workflow field diff:** the gate-on and gate-off pairs give the SAME 46 non-meta new paths.
- `finalState|initialState|state.backgroundExecution` (bool) and `.memoryConfigSource` ("legacy"): **DROPPED**
  (cyril workflow.rs keeps a fixed WorkflowState shape).
- `finalState.root.children[].watchCursor{terminalId,startedAt,outputFile}`, `.watchTerminal` and
  `capturedOutputs.bg`: HANDLED (opaque `serde_json::Value` passthrough in workflow.rs).
- `result.sessions[]._meta.kiro.settings.backgroundExecution.enabled` on session/list: DROPPED.
- Plus the config-state, memoryReflection and handler-schema items from section 2.
- Removed: only the promptScoped artifact.

**Method discovery (all -32603, never -32601):** the following all return
`[PersistenceClassification] ... no persistence classification` on both versions. The control-method set is unchanged.
- `_kiro/workflow/{steer,message,status,attach,validate,notifications}`
- `_kiro/session/setWorkflowNotificationDelivery` (the TUI calls it only if advertised, and KAS does not advertise it)
- `_kiro/terminal/{moveToBackground,output,stopBackground,stopAllBackground}`

## 4. KAS leads (0.66.26 unless noted)

- **L1/L4 config state:** see above.
  - With init `settings.thinking:off` and session `thinking:on`, the session view shows only
    `krn:::setting/thinking value on, contributions[{layer:"session"}]`; the connection view has `settings:[]`.
  - `KIRO_DUMP_CONFIGURATION=1` + `_DIR` writes `<dir>/connection/stdio/0000-<ms>.json` and
    `<dir>/<sessionId>/NNNN-<ms>.json`. Each file is `{view, layers[ids], writtenAt, resolution:{...same as wire...}}`.
    Copies are in `leads/confdump-0.66.26/`.
- **L2 contribute:** `-32603 data.details "Unknown ext method: _kiro/configuration/contribute"`, both with and
  without sessionId.
- **T-L6 memory:**
  - Init `settings.memory {mode:"read_only", reflection:false}` is IGNORED (rollout/flag-gated): session/new
    still returns `_meta.memoryConfig {mode:"read_write", reflection:true}`.
  - There is no `memoryConfigSource` in session/new `_meta`.
  - `set_config_option memoryReflection on` returns the full configOptions and pushes config_option_update, but
    no configuration/state.
- **T-L7:** agentCapabilities `_meta.kiro` has NO `strictSessionLoad`. `session/load {sessionId:<random>,
  _meta.kiro.requireExisting:true}` **creates the missing session** and returns a new empty record (`title "New Session"`).
- **T-L4 `_kiro/session/history`:**
  - `{sessionId, limit}` without `beforeMessageId` returns `{updates:[], hasMore:false}`, i.e. EMPTY.
  - With `beforeMessageId:<userMessageId>`, updates are full replay frames: user_message_chunk
    `_meta.kiro{userMessageTag, messageId, timestamp}`, turn_start, agent_message_chunk, context_usage,
    turn_completion, turn_end.
  - `limit:1` returns one turn plus `hasMore:true, oldestLoadedMessageId:"<id>-turn-start"`.
  - An unknown beforeMessageId returns empty.
- **T-L5 displayText:**
  - `session/prompt _meta.kiro.displayText:"short label A"` is persisted. On session/load replay, the
    user_message_chunk TEXT is the label and `_meta.kiro.displayText` is set.
  - The session title became "Short Label A".
  - The saved-prompt invocation `/echoall alpha beta` was also persisted with `displayText`.
- **L5 contextBreakdown:** `session/prompt _meta.kiro.contextBreakdown:"detailed"|"summary"` adds
  `turn_completion.contextBreakdown` with this schema:
  `{schema:1, modelCalls, totalChars, window{sizeTokens:1000000, usagePercentage}, categories{kiroInstructions,
  toolSpecs, mcpTools, agentsIndex{count}, skillsIndex, powersIndex, steering, skills, toolIO{inputChars,outputChars},
  history{user/assistant/thinking/compactionSummaryChars}, workspace{fileTree/openFiles/environment/repositoriesChars},
  memory{learnings, knowledge}, attachedFiles, other}, media{images,imageBytes,documents,documentBytes}}`.
  "detailed" produced the same output as "summary" here (no `items[]`). The control turn has no breakdown.
  cyril: NOT-SENT/DROPPED.
- **L9 saved prompts:**
  - `.kiro/prompts/*.md` files become available_commands entries `{name, description:"(file prompt)",
    input{hint}, _meta.kiro{type:"prompt", resource{resourceType:"prompt", source{origin:"workspace", root}},
    arguments[], originalName}}`.
  - The argument grammar is `$ARGUMENTS | ${@} | ${1}..${10}` ONLY. A file using bare `$1 $2` gets
    `arguments:[]` and hint `""`, and `/hello world BANANA` is passed RAW to the model with no expansion.
  - With `${1} ${2}`: hint `"<arg1> <arg2>"`, arguments `[{name:"arg1",required}, ...]`, and expansion worked
    ("Hello, world! BANANA").
  - cyril: entries HANDLED as commands, `_meta.kiro` DROPPED.
- **T-L1 subagentOrchestration** (tool list from model self-report plus toolSpecs chars from contextBreakdown):
  - A (omitted, workflows off) and B (`{enabled:true}`, workflows off) are identical: `invoke_sub_agent` is
    present, there is no OrchestrateSubAgent, and toolSpecs is 27951 chars.
  - C (omitted, workflows on) and D (`{enabled:true}`, workflows on) are identical: **`invoke_sub_agent` is GONE**,
    and `run_workflow, inspect_workflow, update_workflow, send_message` are added (43756 chars).
  - So `subagentOrchestration` has **no effect** on 0.66.26 either way, and turning workflows on suppresses the
    chat delegation tool regardless. cyril still sends the setting; it is harmless but dead.
  - `_kiro/tools/didChange` carries only `{sessionId, tags[{source,tag,description}]}`; tool names are not on the wire.
- **validate_workflow / save_workflow_definition (gate on, vibe parent):**
  - On 0.66.15, `validate_workflow` is present and works (`{"valid":false,"errors":["Schema error at
    steps.0.prompt: Required"],"warnings":[]}`).
  - **On 0.66.26 it is absent**, confirming static B14.
  - `save_workflow_definition` is absent from the parent's tools in BOTH versions (it is not a vibe-parent tool).
- **L3 `_kiro/terminal/write`:** NOT elicited.
  - Setup: `backgroundExecution:{enabled:true}` (plus `KIRO_FEATURE_BACKGROUND_EXECUTION_ENABLED=true` in a second
    run), with terminalInput on and off.
  - The model called `Control Process {action:"start"}`, which produced a client `terminal/create {sessionId,
    command, cwd, env[KIRO_SESSION_ID, AWS_SDK_UA_APP_ID]}` with **no outputByteLimit**. The KAS log shows
    `terminal.background.started {interactive:false}`.
  - It then called `Get Process Output` and `List Processes`, and finally `Control Process {action:"stop"}`, which
    produced `terminal/kill`.
  - The model chose non-interactive mode, so the terminalInput leg matched the control leg. Nothing beyond that was tested.
- **L6 cascade:**
  - availableModels has only `auto` (no auto-fast/balanced/smart).
  - Setup: session `cascade:{enabled}` plus `KIRO_FEATURE_CASCADE_CONFIG` (probeStartTurn 1, notifications on).
  - The KAS log shows `model_routing.session.enabled {initialCategory:"auto-balanced",...}` and a **hidden
    `model-classifier` call on EVERY turn** (`q.converse.dispatch {modelId:"auto-balanced",
    agentMode:"model-classifier"}`, ~1.7-2.3 s).
  - None of it reaches the wire: turn_completion still lists 1 requestId, and there is no `model_routed` and no
    model config_option_update (the category never changed). Whether the classifier call is billed is not visible.
- **L8 KUTS telemetry:** run through a CONNECT-logging proxy. The proxy was honored (`management.us-east-1.kiro.dev`
  and `runtime.us-east-1.kiro.dev` appeared). There were **zero `telemetry*.kiro.dev` connects** in every variant:
  - default
  - `KIRO_FEATURE_KUTS_TELEMETRY_ENABLED=false`
  - initialize `_meta.kiro.telemetryEnabled:false`
  - a 240 s idle run with a graceful stdin close
  - the 0.66.15 control

  KUTS export was not observed on a direct cyril-style spawn (clientInfo `cyril-probe`).
- **L7 step parent note (fault-injected):**
  - Setup: a proxy that RESETS sockets while the step streams, plus `KIRO_WORKFLOW_TRANSIENT_RETRY_DELAYS_SEC=3,3,3,3,3,3`.
  - Sequence:
    1. `_kiro/system/notify {level:"warning", message:"The connection to the model was interrupted. Retrying..."}`.
    2. Six retries, each a `node_paused {kind:"retry-wait", reason:"Connection error (ECONNRESET); retrying in 3s
       (attempt n/7)."}` plus a re-emitted `node_start`.
    3. A give-up `node_paused` (no kind): "Transient connection error (ECONNRESET); the run is paused and can be resumed."
    4. `paused`, then `run_complete:paused`.
    5. **`_kiro/session/notify {sessionId:<parent>, callerSessionId:<step>, severity:"info", sender:"step",
       workflowId, nodeId, agentName, message:"Step "n1" had connection errors (ECONNRESET) from ... so the
       workflow stopped retrying it and paused the run. When the connection is back, resume the run with
       update_workflow (action update_status, status "running"), or tell the user."}`**
    6. An **unsolicited parent turn**: turn_start -> display_error "Your connection was interrupted..."
       (ConnectionResetError) -> turn_completion -> turn_end.
    7. Resume -> completed -> normal wake.
  - My first attempt used an HTTP 502 fault instead. The SDK classified that as a server fault, so the step failed
    outright (`failureReason "HTTP 502"`) with no retry-wait and no note. The run_complete auto-wake itself then
    FAILED, and the parent got a failed turn with `display_error` (KAS log `workflow.emitNotification.auto_wake_failed`).
  - cyril: session/notify DROPPED; `_kiro/system/notify` HANDLED (kiro.rs); turn_completion
    `recoveries:["streamErrorRetry"]` (pre-existing in 0.66.15) DROPPED.
- **Child-session allow_always (coordinator question):**
  - Answering `allow_always` ("always-accept") on a **workflow STEP** shell approval makes KAS re-ask for the SAME
    toolCallId immediately, with `_meta.kiro.consentRound` counting 1->20. It then rejects with "Permission flow
    exceeded 20 approval rounds" (~0.7 s total).
  - The step then fails (0.66.26) or asks need_input and pauses (0.66.15). **The loop itself is the same on both versions.**
  - On the MAIN session, allow_always works: 1 prompt, and the identical command next turn is not re-asked.
  - So KAS does not honor step-session "always" grants itself, and the TUI's client-side child-consent grant table
    is load-bearing. **cyril risk: an "Always" click on a workflow-step approval produces 19 more prompts and then a failed step.**

## 5. HANDLED / DROPPED summary (2.28.0-only items)

| item | engine | cyril |
|---|---|---|
| commands/options `fallback{eligible,vendorDefault,configured}` | v2 | DROPPED |
| `commands/execute fallback` | v2 | NOT-SENT |
| `_kiro/configuration/state` | KAS | NOT-SENT (opt-in) / no handler |
| configOption `memoryReflection` | KAS | HANDLED generically; `description` DROPPED |
| workflow state `backgroundExecution`, `memoryConfigSource` | KAS | DROPPED |
| watch `background-process` (watchCursor/watchTerminal/capturedOutput) | KAS | HANDLED (opaque) |
| session/list `_meta.kiro.settings.backgroundExecution` | KAS | DROPPED |
| `turn_completion.contextBreakdown` | KAS | NOT-SENT |
| available_commands `_meta.kiro{type:"prompt",arguments,...}` | KAS | DROPPED (entry handled) |
| `_kiro/session/notify` (step notes incl. network give-up) | KAS | DROPPED |
| `validate_workflow` tool removed | KAS | N/A (cyril-0asq design input) |

## 6. Surprises / leads for the static lanes

1. The step-session allow_always loop (both versions) is the biggest practical cyril hazard found.
2. `_kiro/session/history` silently returns empty without `beforeMessageId`.
3. A saved-prompt `$1` is not a placeholder; only `${1}` is.
4. The `subagentOrchestration` setting is dead, and turning workflows on removes `invoke_sub_agent`.
5. The cascade classifier is a hidden per-turn model call with no wire trace.
6. KAS memory init settings are ignored on a standalone spawn (memoryConfig stays read_write/true).
7. `requireExisting` is ignored, so session/load creates the missing session.
8. `sweep-new-fields.py` walks only 5 array elements, so it reports false "removed" paths when an array grows.
9. The workflow ext methods ARE in `agentCapabilities._meta.kiro.extensionMethods`. The authoring skill says they
   are not advertised; that is stale.

## 7. Correction (re-probe with clientCapabilities placement)

**What was wrong.** `probe-kas-leads-2.28.0.py`'s `init()` helper put initialize settings at the request's
top-level `_meta.kiro.settings`. KAS 0.66.26 reads them ONLY from `initialize.clientCapabilities._meta.kiro`
(`let r=t.clientCapabilities?._meta?.kiro … this.clientMeta=r`; `subagentOrchestrationActive` and
`captureMemoryConfig` both read `this.clientMeta?.settings`). cyril sends them there too
(`crates/cyril-core/src/protocol/engine.rs` `client_capabilities`). So every "init setting" in §4 T-L1, T-L6 and
L1/L4 was a no-op. The §4 bullets for those three items and §6 items 4 and 6 are **void**; the results below replace them.

**Re-probe.** `probe-kas-initsettings-2.28.0.py` is a copy of the leads probe with only the `init()` fix
(settings merged into `clientCapabilities._meta.kiro` next to `caps_meta`), plus three new probes:
`orch2`, `memory` and `confstate`. The spawn is unchanged: the same 0.66.26 carve (direct
`node --experimental-wasm-modules acp-server.js --transport=stdio --auth=acp-callback`), `HOME=<tmp>` with the
real `XDG_DATA_HOME`, one KAS process at a time. The initialize frames in the capture show the corrected
placement. Capture: `kas-initsettings-2.28.0.jsonl` (931 lines: the `all` run plus a `confstate` re-run that adds
arm S3). Tokens are redacted; grep found no live token prefix, no JWT, no ARN and no Bearer header.

Auth: the token was already expired at start (-1235 s). The single-flight post-expiry renewer fired exactly once
(`kiro-cli user whoami`, rc 0, +3599 s). No other kiro process was running, and the token was valid at the end
(2768 s left). Neither run used login or logout.

### 7.1 T-L1 `subagentOrchestration` x workflows

Tool lists are the model's self-report; toolSpecs chars come from `contextBreakdown`. Init settings for every arm
are `base` (`codeIntelligence`, `knowledge`, `thinking` and `largeToolOutputHandler`, each `{enabled:true}`),
plus `subagentOrchestration` where noted. `workflows`+`goal` go in the session/new `_meta.kiro.settings`, as
in the original probe.

| arm | orch in init | workflows (session) | delegation tool | workflow tools | toolSpecs chars | tools |
|---|---|---|---|---|---|---|
| A | – | off | `invoke_sub_agent` | – | 38285 | 17 |
| B | on | off | **`orchestrate_subagent`** | – | 41279 | 17 |
| C | – | on | **none** (suppressed) | run/inspect/update_workflow, send_message | 54090 | 20 |
| D | on | on | **`orchestrate_subagent`** (survives) | same 4 | 60453 | 21 |
| E | on (+wf/goal also in init, cyril-like) | on | `orchestrate_subagent` | same 4 | 60453 | 21 |
| F | – (orch only in session/new) | on | none | same 4 | 54090 | 20 |

- The code's prediction is confirmed:
  - Init-level `subagentOrchestration:{enabled:true}` swaps the builder from `invoke_sub_agent` to
    `orchestrate_subagent` (tool title "Orchestrate Sub-agent", +2994 chars).
  - With workflows on, it **keeps** a chat delegation tool. `suppressChatDelegationTool` is
    `workflowsEnabled && !subagentOrchestrationActive`.
- Without orchestration (C), workflows-on still removes the delegation tool entirely, as originally reported.
  That part of the old finding stands for the no-orch case only.
- A session/new `subagentOrchestration` (F) is **ignored**. The setting is connection-scoped
  (`clientMeta.settings`).
- Putting workflows+goal in the init settings as well (E) does not change the tool list relative to D.
- So the setting is **not dead**. cyril's default-on `subagentOrchestration` (`DEFAULTS_ON` in
  `kas/settings.rs`) is what gives cyril sessions `orchestrate_subagent`, and keeps delegation alive with
  workflows on.
- **Old baseline is wrong too.** Arm A (base in init) has `code` and `knowledge`. The no-init-settings control
  (memory M0: 15 tools, 27951 chars, the same number the old probe reported for all of A/B) lacks them.
  Non-memory init keys override the model-config feature flags (`hPe(Crr(...l.has(h)?Tw(u,h):...))`), so the old
  27951/43756 figures describe a client that sends no init settings, not cyril.
- `_kiro/tools/didChange` still carries tags only. The KAS log never names the tool ids (0 hits in every arm),
  so the tool list rests on the model's self-report plus the toolSpecs deltas.

### 7.2 T-L6 memory (`clientCapabilities._meta.kiro.settings.memory`)

| arm | init memory | session/new memory | session/new `_meta.memoryConfig` | configOption `memoryReflection` |
|---|---|---|---|---|
| M0 control | – | – | `{read_write, reflection:true}` | on |
| M1 | `{mode:"disabled"}` | – | **`{disabled, reflection:true}`** | on |
| M2 | `{mode:"read_only", reflection:false}` | – | **`{read_only, reflection:false}`** | **off** |
| M3 (session control) | – | `{mode:"disabled"}` | `{disabled, reflection:true}` | on |
| M4 | `{read_only, reflection:false}` | `{mode:"read_write"}` | `{read_write, reflection:false}` | off |

- Init memory settings **are honored**. The old "IGNORED (rollout/flag-gated)" claim is void.
- Resolution is per-field (`j1n`): the session value wins, then the init value, then the default
  (`read_write`/`true`). That is why M1's `disabled` leaves `reflection:true`, and M4 mixes the two.
- `memoryReflection` mirrors the resolved `reflection`.
- `memoryConfigSource` is never on the wire (persisted-record schema only).
- No `memory` tool appears in any arm: the toolSpecs count is identical (27951) and `contextBreakdown.memory`
  is all 0. The memory tool stays dark regardless of mode on a standalone spawn.
- The session record echoes the session/new `_meta` under `_meta._meta`.

### 7.3 L1/L4 `_kiro/configuration/state`

All arms set `clientCapabilities._meta.kiro.configurationState:true`.

- **S0 control** (no init settings; session/new `thinking:on`):
  - The connection view has 8 layers and `settings:[]`.
  - The session view has 11 layers and contains `workflows off`, `fta off` and `thinking on`. Each of those
    rows has a single `session` contribution.
- **S1** (init `thinking:{enabled:false}` + `memory{read_only,false}`; session/new `thinking:on`): **identical to S0.**
  - The connection view is still `settings:[]`.
  - `thinking` resolves `on` with ONLY a `session` contribution. The init `off` is not a contribution in any
    layer, so nothing is standing or displaced.
  - memory does not appear in configuration/state. It took effect anyway: memoryConfig `{read_only, false}`.
- **S2** (same init, no session settings): `thinking` has **no row at all**, and the session view has only
  `workflows`/`fta`.
- **S3** (init `knowledge:{enabled:false, maxFiles:123}` + `thinking off`; session/new `thinking on`):
  - The connection view now has `krn:::setting/knowledge off` and `knowledgeMaxFiles 123`, both
    `assertions/contributions[{layer:"kiro-agent", stated{value, used:true}}]`.
  - The session view carries them forward and adds the session rows: `thinking on`, with a `session`
    contribution only.
- Why: the intake tables are `XAt = {startup:[], kiroAgent:Puu, session:[…, Nuu]}`.
  - Initialize settings feed only the **`kiro-agent`** layer, and only for the knowledge* family (Puu).
  - The **`client-connection`** layer gets nothing from initialize, because the `startup` table is empty on
    0.66.26.
  - `thinking` and the other Nuu keys are session-table keys, read from session/new or per message.
- The row shape is `{krn, value, compose, assertions[{layer, compose}], contributions[{layer, stated{value,
  used}}]}` and has no other keys. There is no `standing`, `displaced` or `decidedBy`.
- Whether init `thinking:off` affects the model is untested: 0 `agent_thought_chunk` in every arm, including
  the thinking-on control.
- `KIRO_DUMP_CONFIGURATION` dumps match the wire (in private scratch, not committed).

**Revised §6 items.**
- Item 4: `subagentOrchestration` is live and connection-scoped. It selects `orchestrate_subagent`, and it
  prevents workflows-on from suppressing chat delegation.
- Item 6: KAS memory init settings are honored per-field. The memory tool itself stays absent.
- Item 2 (§2's "Initialize-level settings are NOT represented") needs this qualifier: they are not represented
  except for knowledge*, which lands in the `kiro-agent` layer.
- Same misplacement, not re-measured: §4 L8's `init_telemetryEnabled_false` variant also sent
  `telemetryEnabled` at top-level `_meta.kiro`. KAS reads it from `clientCapabilities._meta.kiro`, so that leg
  is void too.
