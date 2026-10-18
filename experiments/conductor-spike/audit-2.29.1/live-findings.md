# LIVE lane — kiro-cli 2.28.0 → 2.29.1 wire audit (KAS 0.66.26 → 0.66.33)

Started 2026-10-11. Lane dir: `experiments/conductor-spike/audit-2.29.1/live/`
All legs strictly serial; every spawn is the direct KAS carve
(`<node> --experimental-wasm-modules <acp-server.js> --transport=stdio --auth=acp-callback`),
fresh temp `HOME`, real `XDG_DATA_HOME`, one Kiro process at a time. Auth is answered from
`~/.local/share/kiro-cli/data.sqlite3`; the single-flight renewer fired only when the token
crossed its grace window. No `login`/`logout` was run. Captures are redacted.

Engines
- OLD 2.28.0 = `/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js` (0.66.26, sha256 `3bc21b1f684c…`)
- NEW 2.29.1 = `…/kas-carves/2.29.1/…/acp-server.js` (0.66.33)

## Summary

1. Same-day, strictly-serial, paired runs of both KAS carves (0.66.26 → 0.66.33): smoke/`--help`,
   initialize auth matrix, `SCENARIO=sweep`, `SCENARIO=workflow` gate off/on, W-series, T-series,
   lead probes. Every spawn proved by `/proc/<pid>/exe` + KAS `kas.server.starting` version.
2. The workflow control plane is **live-identical** (all 16 run sequences match run-for-run); the
   memory handshake, `overrideSessionId`, census methods and initialize result are unchanged.
3. Real wire ADDs: `_kiro/configuration/getState` implemented; `consentRound` on
   `pending_interaction`/`interaction_resolved`; `tool_call._meta.kiro.resources[]`;
   `agentCapabilities._meta.kiro.modelFallbackReporting`; populated `config resources[].items[]`.
4. Real behaviour FIX: a workflow-step interactive approval re-asked 20× then failed on 0.66.26;
   on 0.66.33 it resolves in one round with `consentRound`.
5. Unreleased (no changelog line): recipe `include` nodes + include-cycle detection, startup
   recipe-catalog reads for the `user`/`cloud` tiers, `getState`, `tool_call` repo `resources[]`.
6. REGRESSION: the three bundled steering slash commands vanish from `available_commands_update`
   on 0.66.33 (5 commands → 2), while the steering docs still load and list `active`;
   **re-confirmed on a fresh clean pair** (both update frames, `_meta.kiro` on the survivors
   unchanged).
7. cyril gaps: `getState` NOT-SENT (0 call sites); interaction-kind `_meta.kiro` + tool-call
   `resources` DROPPED (`convert/kas.rs:525`, `kas.rs:540`); cyril SENDS two keys KAS now rejects
   (`subagentOrchestration`, `todoList` → `settings.rs:82,93-97`).
8. Confirmed negatives: `tool_tags` is not on the ACP wire; `largeToolOutputHandler` is unknown on
   both engines; api-key/region, `model_fallback`, multi-client fanout and the image-placeholder
   surface remain UNPROVEN (see Leads).
9. All findings are LIVE (each row names its capture + frame); STATIC-lane leads were confirmed or
   corrected in Leg 3 — see "static rows confirmed/corrected".
10. Leg 5 complete: **v2 host pair = IDENTICAL** (313/313 field paths, archived binaries);
   **H1** (`--auth-method` ownership) and **H3** (`agent upgrade`) CONFIRMED live; **H4**'s
   `cleanup-autocomplete-artifacts` exists **in `kiro-cli-chat`** (new in 2.29.1, dry run removes
   nothing — my first attempt used the launcher, which does not carry it); telemetry pair shows
   0 `telemetry*.kiro.dev` dials in all arms on both engines.


## Method notes

- Driver = `probe-kas-live-2.29.1.py` (copy of `probe-kas-live-2.28.0.py` with exactly two edits:
  the `initialize` settings are written under `clientCapabilities._meta.kiro.settings` per trap 5,
  and `_kiro/configuration/getState` + `_kiro/memory/get` added to the sweep census). Both OLD and
  NEW legs use this same copy, so the pair is comparable.
- The 2.28.0 baseline driver put init settings at the request-level `_meta` (trap 5 violation); the
  corrected placement is used here for every leg.
- Frame index = 1-based line number in the named `.jsonl`.
- Convention for handshake probes below: the full path
  `initialize.clientCapabilities._meta.kiro.*` is written out; a positive control is named per probe.

## Leg 0 — smoke and `--help` (both engines)

LIVE, captures `tmp/help-2.28.0-*.{out,err}`, `tmp/help-2.29.1-*.{out,err}`.

- `--help` and `-h` are **not** flags of `acp-server.js`. Both engines ignore argv, start the server
  normally (`kas.server.starting {"product":"KAS (Kiro Agent Server)","version":"0.66.26"|"0.66.33"}`),
  print no usage and exit 0 on EOF of stdin. There is no help surface to diff.
- NEW logs recipe-catalog work at startup that OLD does not:
  `fs.workflow.recipes.readDir.ok {"system":"fs","tier":"user"}`, `{"tier":"cloud"}`, and
  `fs.workflow.recipeCatalog.refreshShared.ok {"trigger":"startup"}` (stderr of every NEW run).
  OLD 0.66.26 emits none of these three lines in any run. → recipe catalog is read at boot for the
  `user` and `cloud` tiers; the `cloud` tier read is new work on the startup path.
- OLD emitted `[WARN] [ConfigFileWatcher] Sentinel watcher error: ENOSPC: … file watchers reached`
  in this environment; NEW did not in the same conditions. Recorded, not attributed (host wal state
  is shared and racy).

## Leg 0b — initialize-only, auth-mode matrix (OLD/NEW × `--auth=acp-callback` / no `--auth`)

Probe `probe-acp-init-2.29.1.py`; captures `kas-init-{2.28.0,2.29.1}-{callback,none}.jsonl`.
Handshake sent with the full path `initialize.clientCapabilities._meta.kiro = {configurationState:true,
settings:{codeIntelligence,knowledge,thinking,largeToolOutputHandler,memory}}`. Positive control:
`configurationState:true` → the `_kiro/configuration/state` push arrives (frame 5 callback captures,
frame 4 no-auth captures) on both engines.

- **The `initialize` result is byte-identical between `--auth=acp-callback` and no `--auth`** on both
  engines (only `_meta.kiro.logging.logDir`/`filePath` differ, i.e. the temp HOME). auth-mode diff
  output: `IDENTICAL` for 2.28.0 and 2.29.1.
- auth mode only changes (a) the stderr line — `Auth: --auth=acp-callback (host-mediated refresh via
  _kiro/auth/getAccessToken)` vs `Auth: default token file` — and (b) whether the host is asked for a
  token during `initialize`: in callback mode `_kiro/auth/getAccessToken` is invoked **at initialize
  time, before any prompt** (frame 4 of both callback captures); in no-auth mode it is not called at
  all during init. LIVE.
- **initialize result diff OLD → NEW** (frame 6 in `kas-init-2.28.0-callback.jsonl` vs
  `kas-init-2.29.1-callback.jsonl`), only two real lines:
  - `agentCapabilities._meta.kiro.modelFallbackReporting: true` — ADD (absent in OLD). LIVE.
  - `agentCapabilities._meta.kiro.extensionMethods` + `"_kiro/configuration/getState"` (26 → 27). LIVE.
  Everything else (`sessionCapabilities {list,close,delete,fork{_meta.kiro.messageId}}`,
  `mcpCapabilities {http,sse}`, `promptCapabilities {image,embeddedContext}`, `authMethods
  [{aws-builder-id},{aws-iam-identity-center}]`, `checkpoints/sessionList/sessionListScopes
  ["workspace"]/sessionSources ["local"]/sourceProviders:false/policyNotifications/replayMarking/
  executionTargets ["local"]`) is identical. There is no `agentInfo`/`protocolVersion` change.

## Leg 1 — paired `SCENARIO=sweep` (LIVE)

Captures `kas-live-sweep-{2.28.0,2.29.1}.jsonl` (+ `-main-stderr.log`). Identical workload, both
engines spawned by the same driver copy the same day. Coverage is equal and healthy: OLD 165
frames / 181 KB, NEW 183 frames / 214 KB; both ran 3 turns (p1_read, p2_shell = `end_turn`; p3 on a
bogus model = `-32000 InvalidModelError`), exercised the same 30-method census + 8 host callbacks,
and the initialize frame shows the corrected handshake
(`initialize.clientCapabilities._meta.kiro = {configurationState}` → positive control: the
`_kiro/configuration/state` push arrived 2× in both). Both legs are valid; no leg was short.

- **`_kiro/configuration/getState` is now implemented** (this is the only census method that
  changed status). OLD 0.66.26: `-32603 [PersistenceClassification] Ext method
  "_kiro/configuration/getState" has no persistence classification`. NEW 0.66.33:
  `{observer:"workspace",roots:[…]}` returns a full configuration state
  `{view:{observer,roots}, layers[{id,name}], settings[], resources[]}`; `{}` returns
  `-32602 Invalid params: observer must be "connection", "session" with a sessionId, or
  "workspace" with roots`. Frames: OLD `kas-live-sweep-2.28.0.jsonl` census (tag
  `m:_kiro/configuration/getState`), NEW same tag. LIVE. cyril NOT-SENT (`crates/cyril-core/src`:
  no `getState` call site).
- **`consentRound` now rides the two `session_info_update` interaction kinds.** OLD: `consentRound:1`
  only on `session/request_permission.params._meta.kiro` (frame 63). NEW: on
  `session/request_permission.params._meta.kiro` (frame 77) **and** on the `pending_interaction`
  session_info_update `_meta.kiro` (frame 76) **and** on `interaction_resolved` (frame 83). LIVE.
  cyril DROPPED: no `consentRound`/`pendingInteraction`/`interactionResolved` reader
  (0 grep hits in `crates/cyril-core/src`; the kind switch falls through at
  `protocol/convert/kas.rs:525`).
- **`tool_call`/`tool_call_update` gain `_meta.kiro.resources[]`.** `toolOrigin` is present in BOTH
  (`"default"`/`"acp"`); NEW adds `resources:[{provider:"github",owner:"dwalleck",name:"cyril"}]`
  on the frames for the shell tool call (the workspace is a subdir of the cyril git repo, so KAS
  now resolves the enclosing repository and stamps it on the tool call). NEW frames 47-49 of
  `kas-live-sweep-2.29.1.jsonl`; no `resources` key anywhere in the OLD capture. LIVE. cyril
  DROPPED — the tool_call `_meta` is read only for the `agentInitiated` boolean tag
  (`protocol/convert/kas.rs:531-540`); no `resources`/`toolOrigin` field is modelled.
- **Bundled steering slash commands are NOT in `available_commands_update` on NEW.** OLD emits 5
  commands in both frames (`architecture-selection`, `quick-spec`, `bug-fix` — the three
  `_meta.kiro.type:"steering"` ones carrying `contextQuery` — plus `general-task-execution`,
  `context-gatherer`); NEW emits only the 2 custom-agent commands in both frames (frames 15 and 23
  of each capture). Frame-consistent within each run. The `_kiro/steering/documents_changed`
  notification still carries the same `architecture-selection` bundled doc on both, so the docs are
  loaded; only the command advertisement differs. Confirmed by the dedicated re-probe (leg 3
  `commands`), **re-run on a fresh clean pair and reproduced** — treated below. The `sweep-new-fields` "removed"
  `availableCommands[]._meta.kiro.contextQuery` is a differ artifact of this difference (the field
  did not disappear; the 3 commands carrying it did).
- `rawInput.depth`/`rawInput.explanation` in the diff are model-chosen coverage (NEW's list-dir call
  carried them), not a schema change.
- Everything else in the census is unchanged: `_kiro/hooks/list` (`v2Hooks is disabled` on both),
  `_kiro/memory/{list,get}` (`Memory is not enabled.` on both), `_kiro/workflow/listWatchHandlers`
  (same `background-process` handler), `listRecipes`, `permissions/list`, `account/getUsage`,
  `session/{list,rename,history,context}`, `config/template`, `tools (31)`, `authMethods`,
  `sessionCapabilities`. `session/list` `_meta.kiro` keys identical (`agentMode,createdAt,source,
  executionTarget,status`).
- Model fallback on the sweep path: identical on both — `set_config_option model=bogus` is accepted
  silently, the next prompt fails `-32000 InvalidModelError` / `retryErrorType:"CLIENT_ERROR"` with a
  `display_error` + `turn_end`, no `model_fallback` frame. (The flag-gated fallback probe is leg 3
  `fallback`.)

## Leg 2 — paired `SCENARIO=workflow` gate off/on (LIVE)

Captures `kas-workflow-gate{off,on}-{2.28.0,2.29.1}.jsonl` (+ `-main-stderr.log`), the same
driver copy for all four legs, gate settings sent at BOTH the corrected
`initialize.clientCapabilities._meta.kiro.settings` path and `session/new._meta.kiro.settings`.

Coverage (all four legs complete, none short — each log ends with the driver's teardown summary):
gate-off OLD 1184 rec / NEW 1297; gate-on OLD 2148 / NEW 2316. `_meta.workflowsEnabled` echoes
`false` gate-off / `true` gate-on on both engines (positive control for the gate).

**The `_kiro/workflow/*` control plane is live-identical OLD → NEW.** Full run-by-run parity table in
`sweep-new-fields-diff-wfon.txt`'s sibling `wf-run-parity.txt`: gate-on **all 10 workflow names
(`linear, loop, loopnone, park, watch, bgwatch, branches, usertier, fail, agent-wf-coder`) MATCH
event-for-event**, and gate-off all 6 names MATCH.

| run | terminal (OLD / NEW) | events |
|---|---|---|
| linear (steer/pause/inspect/resume) | completed / completed | `run_start, node_start×2, node_paused, paused, run_complete:paused, run_start, node_start, node_complete:completed, node_start×2, node_complete:completed, run_complete:completed` (run again to completion → 19-frame sequence, identical) |
| loop (cap pause → update/extendRepeat → cancel → retry → delete) | aborted / aborted (+ `run_complete:paused`) | identical 22-frame sequence incl. `loop_iteration`, `run_complete:aborted` |
| loopnone | completed / completed | identical incl. two `loop_iteration` |
| watch (command handler) | completed / completed | identical incl. `watch_poll`, `loop_iteration` |
| bgwatch (`background-process` handler) | completed / completed | `watch_poll×3` then two completed nodes (handler present in 0.66.26 too) |
| branches (YAML) | completed / completed | identical 13-frame sequence |
| park (interactive step) | completed / completed | `node_paused, paused, run_complete:paused, run_start, node_complete:completed, run_complete:completed` |
| usertier | completed / completed | identical |
| fail (bogus `modelId`) | failed / failed | `node_complete:failed, run_complete:failed` ×2 (retry), no fallback on either |
| agent-wf-coder (custom agent calls `run_workflow`) | completed / completed | gate-on only |

- `recipes_changed` total: gate-off 0 / 0; gate-on 1 / 1. `listRecipes` returns 18 recipes on all four
  legs; `listWatchHandlers` includes `background-process` on all four.
- `params.update.rawInput.depth` (live) / `.query` (pinned) are model-chosen-coverage rows (different
  tool calls), not schema changes.
- `permissions[].meta_kiro.workflowWatch.*` appears "only under live" in the differ but is a **trap-11
  artifact**: the literal `workflowWatch` occurs 3× in BOTH gate-on captures (in the driver's summary
  and in 2 approval `_meta.kiro` objects each); the two watch approvals simply land at different
  `permissions[]` indices because NEW's approval array is longer (consentRound re-asks).
- NEW-only rows visible in this lane (all cross-cutting, all already established in leg 1): the
  `configuration/state` push now carries populated `resources[].items[]`
  (`standing.kind`, `value.{id,name,uri,description,inclusion}`, `compose`, `pureAssertions`, `type`);
  `params.update._meta.kiro.consentRound`; `result.agentCapabilities._meta.kiro.modelFallbackReporting`.
  There is **no workflow-specific wire delta** — matching the kas-workflow static result.
- Diff artifacts: `sweep-new-fields-diff-wfoff.txt` (1118 vs 1100 paths, 19 new / 1 removed) and
  `sweep-new-fields-diff-wfon.txt` (1305 vs 1284, 23 new / 2 removed). The "removed" path in both is
  `availableCommands[]._meta.kiro.contextQuery`, the steering-command artifact (leg 3).
- **Process note (trap 11/void-leg):** the gate-on NEW leg was still writing when an earlier diff was
  taken; both diffs and the parity table above were re-run against the final, complete captures. The
  path sets did not change, but the coverage prose there did — recorded so the void numbers do not
  resurface.

## Leg 2b — W-series (kas-workflow W1-W3) (LIVE)

Captures `kas-wseries-{2.28.0,2.29.1}.jsonl` (results read from the `{"probe":"result"}` record, not
the truncated log), same driver copy, `SCENARIO=wseries`, gate off. Both legs completed (rc=0).

### W1 — recipe `include` nodes are NEW in 0.66.33 (LIVE, real ADD)

The same `parent.workflow.json` (`steps:[{type:"include",id:"review",path:inc}, {type:"step",id:"after",
prompt:"…{{review/s.output}}…"}]`):

| engine | `_kiro/workflow/new` | run |
|---|---|---|
| 0.66.26 | **rejected**: `'parent.workflow.json' is not a valid workflow: [{"code": "invalid_union_discriminator", "options": ["step","sequence","repeat","parallel","watch"], "path": ["steps", …` | never started (`include_events: null`, `include_completed: null`) |
| 0.66.33 | accepted | `run_start, node_start×3, node_complete:completed ×2, node_start×2, node_complete:completed, run_complete:completed` → `completed`; `capturedOutputs {"review/s":"OK","after":"AFTER"}` |

Counts: `invalid_union_discriminator` OLD 4 / NEW 0; `includedFrom` OLD 0 / NEW 3. The NEW
`run_start.nodeTree` carries the expansion and a **new `includedFrom` field**:
`{"nodeId":"review","type":"sequence","steps":[{"nodeId":"review/s","type":"step","agentName":"wf-coder"}],
"includedFrom":{"name":"inc","path":"…/inc.workflow.json",…}}` and the included step nests its
`nodePath` (`[wf,"review","review/s"]`, frames 29-47 of `kas-wseries-2.29.1.jsonl`). cyril: workflow
events are relayed opaquely (cyril does not author recipes) → HANDLED generically; the `nodeTree`
shape is not modelled. **Not in any 2.29.x changelog line** → a dark/unreleased feature.

### W2 — include negatives

| probe | 0.66.26 | 0.66.33 |
|---|---|---|
| path outside workspace roots | `-32603 … is outside the allowed workspace roots (…)` | same |
| include cycle | `-32603 … is not a valid workflow: [{"code":"invalid_union_discriminator","options":[step,…]}]` (include not recognised at all) | `-32603 … has invalid include nodes: include 'loop' (…)` — dedicated cycle detector |
| `inputs` omitted | `-32602 Invalid params {'inputs': {'_errors': ['Required']}}` | identical |

Counts: `has invalid include nodes` OLD 0 / NEW 2. The cycle guard is new with the include feature.

### W3 — step approval: consentRound + a 20× re-ask bug fixed (LIVE)

Same single-node recipe `ap.workflow.json` (one shell step `echo WFAPPROVED > wfapproved.txt`),
`perms_pick="always-accept"` on both legs (identical client answer
`{"outcome":{"outcome":"selected","optionId":"always-accept"}}`):

| engine | `session/request_permission` asks | `pending_interaction` / `interaction_resolved` consentRound | node/run |
|---|---|---|---|
| 0.66.26 | **20×** (frames 67,73,78,…,163 — same `toolId:run_command`, same command each time) | `[null]×20` / `[null]×20` | node FAILED (frame 218: `capturedOutput:"…blocked by the approval flow and never executed…"`, `failureReason:"Step signaled error via send_message."`), `run_complete:failed` |
| 0.66.33 | **1×** (frame 222) | `[1]` / `[1]` | node completed (frame 258, `capturedOutput:"DONE"`), `run_complete:completed` |

So on 0.66.26 an approved workflow shell step re-asks consent 20 times and then fails; on 0.66.33 the
same client answer resolves in one round, with `consentRound` present on the interaction kinds
(the leg-1 finding reproduced on the workflow path). The 20×/fail-to-1×/complete change is a
LIVE behaviour delta; attributing it *specifically* to the consentRound addition is my
[INFERENCE] — the captures prove the correlation, not the mechanism.



## Leg 3 — static-lane leads (kas-core L1-L8, kas-workflow W1-W3)

Handshake discipline: every probe below writes the FULL path
`initialize.clientCapabilities._meta.kiro.*`; the per-probe positive control is named.

Probe files (my lane): `probe-kas-leads-2.29.1.py` (L1-L8), `probe-kas-workflow-2.29.1.py`
(SCENARIO=wseries, W1-W3). Captures `kas-lead-*.jsonl`, `kas-wseries-*.jsonl`.

### L2 `_kiro/configuration/getState` — LIVE, CONFIRMED, extended

Capture `kas-lead-getstate-2.29.1.jsonl`; pull responses frames 24/26/28 (G0), 53/55/57 (G1),
82/84/86 (G2); pushed `_kiro/configuration/state` at frames 34/47 (G1) and 63/76 (G2).

- The pull is **ungated**: G0 sends NO `configurationState` and still gets a full
  `{view,layers,settings,resources}` for `{observer:"connection"}`, `{observer:"session",sessionId}` and
  `{observer:"workspace",roots}`. `configurationState:true` only turns on the **push**
  (G0 push_after_init 0 / push_after_new 0 vs G1 and G2 1/1) — the positive control behaves.
- Layer sets: connection view 9 layers (`kiro-agent-defaults, client-defaults, kiro-service,
  organization, machine, user-synced, user-local, kiro-agent, client-connection`); session and
  workspace views 12. The last connection layer's `name` is the client `clientInfo.name`
  (`cyril-probe`).
- Session view is the only one with settings: `[{krn:"krn:::setting/workflows", value:"off",
  contributions:[{layer:"session", stated:{value:"off"}}]}]`; connection/workspace `settings:[]`.
- `resources[]` = 3 typed bundles (`steering`, `agent`, `workflow`), 22 items total; each item is
  `{krn, layer, standing:{kind:"active"}, value:{…}}`, each resource `{type, name, compose:"append",
  assertions:[{layer,compose}], pureAssertions:[], items[]}`. Steering items: `architecture-selection`,
  `quick-spec`, `bug-fix` (`inclusion:"manual"`, `uri:"builtin:…"`); agent items include the `wf-*`
  agents; workflow items: `autoresearch, feature-pipeline, goal, investigate, publish-pr, ralph`.
- **`inactiveItems` produced no observable difference here**: G1 and G2 return identical item sets
  and every item is `standing.kind:"active"` (22/22) — this workspace has no shadowed/non-active
  items to reveal, so the capability's filter is untestable in this configuration. Marked UNPROVEN
  (not contradicted).
- cyril: **NOT-SENT** — no `getState` call site (`crates/cyril-core/src` has 0 occurrences).

### L7 settings intake unknown-key — LIVE, CONFIRMED (debug log)

Capture `kas-lead-intake-2.29.1.jsonl` (info level: 0 hits) and `kas-lead-intake-debug-2.29.1.jsonl`
(`KIRO_LOG_LEVEL=debug`). At debug level the cyril-like arm logs **exactly two** lines:

```
configuration.intake.unknown-key {"key":"setting/subagentOrchestration","layer":"client-connection"}
configuration.intake.unknown-key {"key":"setting/todoList","layer":"client-connection"}
```

counts `{"configuration.intake.unknown-key": 2, "subagentOrchestration": 1, "todoList": 1}`; the
control arm (`{"thinking":{"enabled":true}}`) logs 0. So kas-core's static claim is live-corroborated:
cyril's `DEFAULTS_ON` `subagentOrchestration` (`protocol/kas/settings.rs:93-97`) and its
`chat.enableTodoList → todoList` mapping (`settings.rs:82`) are keys KAS 0.66.33 no longer recognises;
they land in the `client-connection` configuration layer as unknown keys. The earlier info-level leg
was a log-level artifact (trap 16 in spirit). This is a **live-verified cyril send defect**.

### L1 `model_fallback` notification — UNPROVEN (cannot be provoked)

Capture `kas-lead-fallback-2.29.1.jsonl`; `turn_end` frames 38/79/123.

| arm | env flags | model | outcome | `model_fallback` |
|---|---|---|---|---|
| F0 control | – | `bogus-model-zz9` | `-32000 InvalidModelError`, kinds `…turn_start, display_error, turn_completion, turn_end` | none |
| F1 | both fallback flags `true` | `bogus-model-zz9` | identical to F0 (same kinds) | none |
| F2 | both fallback flags `true` | `auto` | `end_turn`, normal kinds | none |

No `model_fallback` frame on 0.66.33 in any arm; the two flags alone do not manufacture a chain.
Positive control `turn_start`+`turn_end` present in every arm (1/1). Re-run with
`KIRO_LOG_LEVEL=debug` (`kas-lead-fallback-debug-{2.28.0,2.29.1}.jsonl`, `tmp/lead-fallback-debug-*.log`):
still `model_fallback_frames: []`, and the refused turn reports
`{'errorType':'InvalidModelError','retryErrorType':'CLIENT_ERROR'}` — the SDK classifies an unknown
model as a **client** error, so it neither retries nor falls back. The kind needs a served fallback
(a real over-quota/refused primary with a configured chain, or a server-pushed `server_cascade_config`)
which cannot be produced without backend fault injection (trap 13). The initialize capability
`_meta.kiro.modelFallbackReporting:true` **is** advertised unconditionally (leg 0b/leg 1). cyril DROPPED
(`convert/kas.rs:525`).

### L5 image placeholder — PARTIAL (control now valid)

Capture `kas-lead-image-2.29.1.jsonl` (re-run with a valid 8×8 PNG).

| arm | model | outcome | text |
|---|---|---|---|
| text-only | `deepseek-3.2` | `end_turn`, no error | prose: "I can't read images directly because the current model doesn't have vision capabilities…" |
| vision (control) | `claude-sonnet-4.5` | `end_turn`, no error | described the image ("a simple solid blue rectangle…") — control valid |

So on 0.66.33 a text-only model given an image does **not** fail the request (`end_turn`), which is
the fix's observable behaviour, and a vision model passes the same image through unchanged. The
compiled placeholder string `[image omitted: the selected model does not read images]` was **not**
observed on the client wire in either arm (0 hits for "image omitted" in the capture), and the KAS
log carries only `fs.loadImageCodec.ok {"system":"fs","durationMs":43}` — no placeholder/fitting log
line. Which surface carries the placeholder (assistant text vs model-side input) is therefore
**UNPROVEN**; it cannot be seen from the client side without backend instrumentation.

Note: the earlier arm in this same probe (1×1 PNG) produced `-32000 ImageValidationError "Could not
process image"` on the vision model — that was an invalid control input (a 1×1 PNG), since corrected;
it incidentally shows KAS does validate image input.

### L6 cascade/handoff config precedence — LIVE, CONFIRMED (debug log)

Captures `kas-lead-cascade-2.29.1.jsonl` (info: 0 lines) and `kas-lead-cascade-debug-2.29.1.jsonl`
(`KIRO_LOG_LEVEL=debug`). The resolver line is named **`model_handoff.config.resolved`**:

```
model_handoff.config.resolved {"source":"defaults","defaulted":["prompt","template","tag"]}   (default arm)
model_handoff.config.resolved {"source":"env","defaulted":["prompt","template","tag"]}        (KIRO_FEATURE_CASCADE_CONFIG arm)
```

So the env-var source wins over defaults, confirming the first half of `lWc` precedence
(env > server_cascade_config > defaults). The `server_cascade_config` tier cannot be exercised
without a backend push (trap 13), and it is not distinguished here — the env arm never reports
`source:"server_cascade_config"`. Counts: `handoff.config.resolved` 1 (default) / 2 (env arm, which
spawned twice).

### L8 stdio MCP servers inherit the full parent env — LIVE, CONFIRMED

Capture `kas-lead-mcpenv-2.29.1.jsonl` (proc frame 2). A workspace-stdio MCP server
(`.kiro/settings/mcp.json` → `mcp-env.py`) wrote its own environment on start; the file shows
`{"MY_MARKER":"1", "HOME":…, "PATH":…}`. `MY_MARKER` is not on any curated allow-list, so the
0.66.33 change to inherit the full `process.env` is corroborated live. This matches the 2.29.0
changelog line ("Stdio MCP servers now inherit the full agent environment"), which kas-core rated
STATIC.

### L3 multi-client ask fanout — UNPROVEN (harness limit)

`acp-server.js` is spawned per Connection over its own stdio pipe, so two `Conn` objects are two
KAS processes, not two clients of one server. Static confirmation of the mechanism:
`server/single-client-stream.d.ts:3` ("a multiplexed stream as it is, any other stream wrapped as
one client") and `server/multiplex-stream.d.ts` (`transportFor(clientId)`), i.e. fanout needs the
multiplex/ws transport the Rust host owns. Next probe is in "Leads for the live lane".

### L4 API-key region discovery — UNRUN

No `KIRO_API_KEY` is available in this environment and this lane does not obtain credentials
(trap 13 / brief). Not attempted.

### `available_commands` steering slash commands — LIVE, CONFIRMED on a FRESH clean pair (NEW regression)

**Re-run deliberately on 2026-10-11 17:29-17:30** (nothing else writing, one clean run per engine, same
driver, same `SCENARIO=wseries`-free `commands` probe, same settle windows 8 s / 25 s / end):
captures `kas-lead-commands-{2.28.0,2.29.1}.jsonl` (overwritten by the clean run).

| engine | `available_commands_update` frames | names (count) | `_meta.kiro.type` |
|---|---|---|---|
| 0.66.26 | frame 15 **and** 23 | `architecture-selection, quick-spec, bug-fix, general-task-execution, context-gatherer` (**5**) | `steering, steering, steering, custom-agent, custom-agent` |
| 0.66.33 | frame 15 **and** 23 | `general-task-execution, context-gatherer` (**2**) | `custom-agent, custom-agent` |

**Verdict: the three bundled steering slash commands (`_meta.kiro.type:"steering"`) really are absent
from `available_commands_update` on 0.66.33 — it reproduces on a clean pair and is NOT a capture
artifact.** They are uniformly absent across both update frames (and in every other 0.66.33 capture in
this lane: sweep, workflow off/on, leads), so it is not a timing/coverage artifact either.

It is a **command-registration** loss, not a feature loss:
- `_kiro/steering/documents_changed` (frame 14, both engines) still delivers
  `[("architecture-selection","manual"), ("quick-spec","manual"), ("bug-fix","manual")]` — byte-identical
  doc set, `type:"steering"`, `uri:"builtin:…"` → the steering docs still load.
- `getState` still lists the same three as `standing.kind:"active"` resource items (leg 3 L2).
- The `_meta.kiro` on the **surviving** commands did **not** change: `general-task-execution` is
  byte-identical on both engines (`{"type":"custom-agent","commandId":"general-task-execution",
  "resource":{"resourceType":"agent",…}}`).
- Not a tier/workspace artifact: the `user`/`cloud` recipe-catalog reads are a *different* subsystem
  (workflow recipes, leg 0) and the same workspace/handshake was used for both engines.

**What cyril loses:** cyril HANDLES the update generically (`convert/mod.rs:491` maps
name/description/input) but drops `_meta.kiro`, so it will faithfully show 2 commands where 2.28.0 showed
5 — the `/quick-spec`, `/bug-fix` and `/architecture-selection` slash commands disappear for cyril users
on 2.29.1. **Proposed issue: yes** (cyril-visible regression; may be worth an upstream report since the
docs still load — the registration filter looks like a bug, and it sits next to the 2.29.1 line
"manual steering files now load even when their slash command appears mid-prompt").
Same-day, different binaries → attributed to the binary (trap 1).

### T-series (memory + telemetry handshakes, T1/T2/T3 analogs) — LIVE

Captures `kas-t-{ltoh,memory,override,interaction}-{2.28.0,2.29.1}.jsonl`, same driver, `KIRO_LOG_LEVEL=debug`.

**T-ltoh — `largeToolOutputHandler` is an unknown setting on BOTH engines (refutes the 2.28.0 L8
hypothesis).** Arm `{"largeToolOutputHandler":{"enabled":true}}` at
`initialize.clientCapabilities._meta.kiro.settings` → KAS logs
`configuration.intake.unknown-key {"key":"setting/largeToolOutputHandler","layer":"client-connection"}`
on 0.66.26 **and** 0.66.33 (counts `largeToolOutputHandler:1, unknown-key:1, intake.unknown:1` on each);
the control arm `{"thinking":{"enabled":true}}` produces 0 unknown-key on both. So KAS never reads this
key at the initialize handshake, in either version — a **confirmed negative**, not a delta. It is also
**not a cyril key** (`largeToolOutputHandler` = 0 hits in `crates/**/*.rs`); it was a probe arm from the
2.28.0 audit, so there is no cyril defect here. (Both legs also log the warn
`Unrecognized clientInfo.name; behavioral client inferred from environment {reportedClientName:"cyril-probe", inferredClient:"kiro-ide"}` — a KAS-side behavioural inference, unchanged.)

**T-memory — the memory handshake is wire-identical OLD→NEW (LIVE, `kas-t-memory-*.jsonl`).** Five arms,
normalized comparison `IDENTICAL`:

| arm | `_meta` sent | resulting `session/new` `_meta.memoryConfig` | `configOption memoryReflection` |
|---|---|---|---|
| a_none (control) | – | `{mode:"read_write", reflection:true}` | `on` |
| b_readonly | `memory:{mode:"read_only"}` | `{mode:"read_only", reflection:true}` | `on` |
| c_disabled_optout | `memory:{mode:"disabled"}, userMemoryOptIn:{enabled:false}` | `{mode:"disabled", reflection:true}` | `on` |
| d_memoryEnable | `memoryEnable:{enabled:true}` | `{mode:"read_write", reflection:true}` | `on` |
| e_memoryEnable_internal | `memoryEnable:{enabled:true}` + `KIRO_INTERNAL=1` | `{mode:"read_write", reflection:true}` | `on` |

`memoryConfig` is echoed on the `session/new` result `_meta` (`memoryConfig_kiro` absent), and
`memoryReflection` is a `configOption` id on both. The 2.29.0 `/memories` changelog line therefore
does not change this handshake.

**T-override — `overrideSessionId` honoured on both (LIVE).** `session/new._meta.kiro.overrideSessionId=<id>`
→ the session is created with exactly that id (`honored:true`), a second `session/new` with the same id
does not mint a duplicate (`repeat_same:null, repeat_equal_first:false`), and the no-field control mints
a fresh id (`control_is_minted:true`). Normalized comparison `IDENTICAL`.

**T-interaction — `pending_interaction` / `interaction_resolved` on an unanswered ask (LIVE).**
With the permission ask left unanswered (`perms_hold`) and then `session/cancel`, **both** engines emit
`interaction_resolved` with `outcome:"cancelled"`, and both expose the `pending_interaction` +
`interaction_resolved` `session_info_update` kinds. The only difference is `consentRound`:

| engine | `interaction_resolved` `_meta.kiro` |
|---|---|
| 0.66.26 | `{"interactionResolved":{"toolCallId":"run_command_toolu_…","outcome":"cancelled"},"kind":"interaction_resolved","toolCallId":"run_command_toolu_…","outcome":"cancelled"}` |
| 0.66.33 | same **plus** `"consentRound":1` (both inside `interactionResolved` and at the top level) |

So the frame exists on both; the delta is exactly the `consentRound` field (independent confirmation of
the leg-1 row). cyril drops the whole `_meta.kiro` kind here (`convert/kas.rs:525`), which on 2.28.0 and
2.29.1 alike would leave an unanswered ask unresolved in the UI — a pre-existing cyril gap, sharpened on
2.29.1 by the unavailable round counter.



## Leg 4 — `sweep-new-fields.py --diff` per pair

Raw artifact: `sweep-new-fields-diff-sweep.txt` (sweep pair). Workflow pairs: `sweep-new-fields-diff-wf{off,on}.txt`.

**sweep pair** (`--diff kas-live-sweep-2.29.1.jsonl kas-live-sweep-2.28.0.jsonl`): 679 paths (NEW) vs
618 (OLD); 62 "only NEW", 1 "only OLD". Mining the new paths:
- 46 of them are the whole `_kiro/configuration/getState` result shape (`result.{view,layers,settings,
  resources}[…]`) — the method answered on NEW and errored on OLD, so its entire payload is new.
  These are NOT handshake fields; they are the getState body (see leg 1 / leg 3 `getstate`). Note
  `resources[].items[].standing.kind` and `resources[].items[].value.{id,name,uri,description,
  inclusion}`, `pureAssertions`, `type` — the resource-item model, present because the getState
  probe sent `inactiveItems:true` (leg 3 `getstate` G2).
- `result.agentCapabilities._meta.kiro.modelFallbackReporting` — real initialize ADD (leg 0b/leg 1).
- `params.update._meta.kiro.consentRound` — real ADD (interaction kinds, leg 1).
- `params.update._meta.kiro.resources[].{name,owner,provider}` — real ADD on tool_call frames (leg 1).
- `params.update.rawInput.{depth,explanation}` — model-chosen coverage (list-dir call), not a change.
- "only OLD" `params.update.availableCommands[]._meta.kiro.contextQuery` — differ artifact of the
  steering-command advertisement difference (trap 11: array-element walk); the field is not removed.


## Static-lane rows — CONFIRMED / CORRECTED

Trap 4: a static finding is a lead, a live capture is a verdict. Rows the static lanes handed me:

**CONFIRMED live**
- kas-core **L2** (`_kiro/configuration/getState` implemented) → leg 1 + leg 3 `getstate`: CONFIRMED,
  extended (pull ungated, push gated, 9/12 layers, 3 typed resource bundles).
- kas-core **L7** (cyril settings keys land as unknown-key) → leg 3 `intake` at debug: CONFIRMED
  exactly two lines (`subagentOrchestration`, `todoList`).
- kas-core **L8** (stdio MCP servers inherit the full env) → leg 3 `mcpenv`: CONFIRMED
  (`MY_MARKER` from an unlisted var reached the child).
- kas-core **L6** (cascade config precedence) → leg 3 `cascade` at debug: CONFIRMED for
  env > defaults (`model_handoff.config.resolved {"source":"env"|"defaults"}`); the
  `server_cascade_config` tier stays UNPROVEN (server-pushed, trap 13).
- kas-workflow **W1** (recipe `include` expansion) → leg 2b: CONFIRMED, and stronger than a
  behaviour change — `include` is a **new step type** (0.66.26 rejects it with
  `invalid_union_discriminator`).
- kas-workflow **W2** (include negatives) → leg 2b: CONFIRMED; plus a **new** dedicated cycle
  detector (`has invalid include nodes`), absent on 0.66.26.
- kas-workflow **W3** (`consentRound` on a step approval) → leg 2b: CONFIRMED (`1` on all three
  surfaces), with the additional 20×-re-ask/`failed` → 1×-ask/`completed` behaviour delta.
- host/tui **H7** (`tool_tags` not on the ACP wire) → CONFIRMED NEGATIVE: 0 hits in every capture.

**CORRECTED**
- kas-core **L1** (`model_fallback` notification): the row as stated is not live-provable. Corrected
  verdict: what IS live is `agentCapabilities._meta.kiro.modelFallbackReporting:true` (ADD); the
  notification needs a served chain and cannot be provoked (and an unknown model is a
  `CLIENT_ERROR`, so the SDK does not even retry). Status: UNPROVEN, not contradicted.
- kas-core **L5** (image placeholder): corrected — the compiled placeholder string does **not**
  reach the client wire; only `fs.loadImageCodec.ok` is logged, and a text-only model returns
  `end_turn` prose. The fix's *observable* behaviour is confirmed; the placeholder *surface* is not.
- 2.28.0 audit's `largeToolOutputHandler` init-setting hypothesis: CORRECTED to a confirmed
  **negative** (unknown key on both 0.66.26 and 0.66.33), and it is not a cyril key.
- The 2.28.0-audit-era "one sweep leg finished suspiciously fast" note: honoured — every leg in this
  run was re-run to completion and each log carries the driver's teardown; a gate-on NEW capture that
  was still writing when first diffed was re-diffed (`Leg 2` process note).

## Deltas

Every row is STATIC or LIVE; LIVE rows name the capture. Direction is OLD(2.28.0/0.66.26) → NEW
(2.29.1/0.66.33). cyril verdict is HANDLED / DROPPED / NOT-SENT with a `file:line`, or `n/a` when the
field never reaches cyril.

| item | version | gate/default | direction | STATIC/LIVE | cyril verdict + file:line | issue? |
|---|---|---|---|---|---|---|
| `_kiro/configuration/getState` implemented (was `-32603 no persistence classification`) | 2.29.0 / 0.66.33 | pull = ungated; push gated by `configurationState:true` | ADD | LIVE (`kas-live-sweep-*`, `kas-lead-getstate-2.29.1`) | **NOT-SENT** — 0 call sites in `crates/cyril-core/src` | yes |
| `extensionMethods` += `_kiro/configuration/getState` (26→27) | 2.29.0 | default | ADD | LIVE (`kas-init-*-callback`, frame 6) | n/a (cyril never reads the list) | no |
| `agentCapabilities._meta.kiro.modelFallbackReporting: true` | 2.29.0 | default | ADD | LIVE (`kas-init-2.29.1-callback`, OLD 0 / NEW 2 hits) | **NOT-SENT** — 0 hits `crates/**/*.rs` | maybe |
| `consentRound` on `session_info_update` `pending_interaction`/`interaction_resolved` `_meta.kiro` | 2.29.0 | default | ADD | LIVE (`kas-live-sweep-*`, `kas-wseries-*`) | **DROPPED** — `protocol/convert/kas.rs:525` (`_ => None`) | yes |
| workflow-step interactive approval: 20× re-ask + `node_complete:failed` → 1× ask + `completed` | 2.29.0 | default | FIX | LIVE (`kas-wseries-*` frames 67-219 vs 222-259) | engine-side; cyril relays verbs opaquely | note |
| `tool_call`/`tool_call_update` `_meta.kiro.resources[]` (`{provider,owner,name}`) | 2.29.0 | default | ADD | LIVE (`kas-live-sweep-2.29.1` frames 47-49; OLD 0 hits) | **DROPPED** — only `agent_initiation` reads the tool `_meta` (`kas.rs:540`), for `agentInitiated` | maybe |
| `configuration/state` push + `getState` result `resources[].{type,compose,assertions[],pureAssertions,items[]}` populated (`standing.kind`, `value.{id,name,uri,description,inclusion}`) | 2.29.0 | push opt-in | ADD | LIVE (`kas-live-sweep-*`, `kas-workflow-*`) | **NOT-SENT** — cyril never pulls and has no resource model | yes |
| workflow recipe `include` nodes accepted (`{"type":"include"}`), plus `includedFrom` on `nodeTree` and nested `nodePath` | 0.66.33 (**no changelog line**) | default | ADD | LIVE (`kas-wseries-*`; OLD `invalid_union_discriminator` ×4) | relayed opaquely (no recipe/nodeTree model) | no |
| include-cycle negative `has invalid include nodes` | 0.66.33 | default | ADD | LIVE (`kas-wseries-2.29.1`, OLD 0 / NEW 2 hits) | n/a | no |
| bundled steering slash commands dropped from `available_commands_update` | 2.29.0 | default | REMOVE (**regression**) | LIVE, **re-confirmed on a fresh clean pair** (`kas-lead-commands-*`, re-run 17:29-17:30: OLD 5 cmds `[steering×3, custom-agent×2]` / NEW 2 cmds `[custom-agent×2]`, both frames; `documents_changed` doc set identical in both) | HANDLED generically `convert/mod.rs:491`; user-visible command loss | yes |
| v2 host ACP surface (Rust `kiro-cli acp` sweep) | 2.29.0 | default | UNCHANGED | LIVE (`v2-live-sweep-{2.28.0,2.29.1}.jsonl`, archived bins: 313 = 313 paths, 0 added / 0 removed; same 3 turns) | n/a (v2 engine) | no |
| `configuration.intake.unknown-key` for `setting/subagentOrchestration` + `setting/todoList` | 0.66.33 | debug log | ADD (diagnostic) | LIVE (`kas-lead-intake-debug-2.29.1`) | cyril **SENDS** both: `protocol/kas/settings.rs:82` (`enableTodoList→todoList`), `settings.rs:93-97` (`subagentOrchestration` default-on) | yes |
| startup recipe-catalog reads for `user` + `cloud` tiers | 0.66.33 | always-on boot | ADD | LIVE (stderr of every NEW leg) | n/a | no |
| stdio MCP child inherits the full parent env | 2.29.0 | default | ADD | LIVE (`kas-lead-mcpenv-2.29.1`) | n/a (KAS spawns MCP) | no |
| memory handshake (`session/new._meta.memoryConfig`, `configOption memoryReflection`) | – | default | UNCHANGED | LIVE (`kas-t-memory-*`, normalized IDENTICAL) | HANDLED | no |
| `session/new._meta.kiro.overrideSessionId` | – | default | UNCHANGED | LIVE (`kas-t-override-*`, IDENTICAL) | n/a | no |
| `largeToolOutputHandler` at `initialize..._meta.kiro.settings` | – | – | NEGATIVE (unknown on both) | LIVE (`kas-t-ltoh-*`) | not a cyril key (0 hits) | no |

_(Leg 5 rows — H1/H3/H4, v2 host pair, telemetry — appended below.)_

| item | version | gate/default | direction | STATIC/LIVE | cyril verdict + file:line | issue? |
|---|---|---|---|---|---|---|
| `kiro-cli acp --auth-method <cli\|client>` (v3 engine auth owner) | 2.29.0 | default `cli` | ADD | LIVE (`v2-acpmethod-{client,cli}.jsonl`, archived 2.29.1 host bin) | n/a for the ACP wire; cyril *is* the client that must answer when `client` is chosen (`protocol/kas/auth.rs:24,300`) | yes (document the flag) |
| `kiro-cli agent upgrade <dir>` (V2/V3 agent-config migration, `fs_read → read`) | 2.28.1 (folded) | default | ADD | LIVE (`tmp/h3-*.log`, dry-run + real run) | n/a (host CLI) | no |
| `kiro-cli-chat _ cleanup-autocomplete-artifacts` (legacy-artifact cleanup) | **2.29.1** (absent in 2.28.0) | default | ADD (**no changelog line**) | LIVE+STATIC (`tmp/h4-dryrun.log`; `kiro-cli-chat` strings 2.29.1 = 4 hits, 2.28.0 = 0; launcher `kiro-cli` = 0 in both) | n/a (host CLI) | no |
| telemetry dials for `telemetry.enabled` / `KIRO_DISABLE_TELEMETRY` | – | default | UNCHANGED | LIVE (`kas-telemetry-{2.28.0,2.29.1}.jsonl`, 0 `telemetry*.kiro.dev` connects in all 6 arms) | n/a for KAS (v3 host surface) | no |


## Unreleased / dark candidates

Features present in KAS 0.66.33 with **no corresponding 2.29.x changelog line** — the "unreleased" set
the brief asked for. Each is LIVE with a capture; gate/default noted.

1. **Workflow recipe `include` nodes** — `0.66.33` accepts `{"type":"include","path":…}` step nodes and
   expands them (`includedFrom` on the `nodeTree`, nested `nodePath`), with a dedicated cycle detector
   (`has invalid include nodes`). `0.66.26` rejects the same file (`invalid_union_discriminator`).
   Gate: none (default). Evidence: `kas-wseries-2.29.1.jsonl` frames 29-109; `kas-wseries-2.28.0.jsonl`
   (4× `invalid_union_discriminator`, 0 runs started).
2. **Startup recipe-catalog reads for the `user` and `cloud` tiers** — new boot work
   (`fs.workflow.recipes.readDir.ok {"tier":"user"|"cloud"}`, `recipeCatalog.refreshShared.ok
   {"trigger":"startup"}`) absent in 0.66.26. Gate: boot/always. Evidence: stderr of every NEW leg.
3. **`_kiro/configuration/getState` is implemented** — was `-32603 [PersistenceClassification] … has
   no persistence classification`. Gate: pull ungated, push via `configurationState:true`. Evidence:
   `kas-live-sweep-*`, `kas-lead-getstate-2.29.1.jsonl`.
4. **`tool_call._meta.kiro.resources[]` repository stamping** (`{provider,owner,name}` resolved from the
   enclosing git repo). Gate: default. Evidence: `kas-live-sweep-2.29.1.jsonl` frames 47-49.
5. **Populated `resources[].items[]` in `configuration/state`** (standing/value/assertions/pureAssertions/
   type) — same method as (3); the push payload is new content. Evidence: leg 2 diffs.
6. **`kiro-cli-chat _ cleanup-autocomplete-artifacts`** — new `internal` subcommand in the CHAT binary
   only (2.29.1 `kiro-cli-chat` 4 string hits; 2.28.0 and the launcher 0), no changelog line.
   Gate: explicit invocation. Evidence: `tmp/h4-dryrun.log` + the strings table in Leg 5 H4.

Not dark (present in a changelog or already known): `consentRound` (interaction kinds) tracks the V3
"accurate outcomes" fix family; `modelFallbackReporting` tracks the "model fallback" line;
stdin-MCP env inheritance is the "Stdio MCP servers now inherit the full agent environment" line.

## Leads for the live lane

What is still **unproven** and the exact next probe. None of these blocks the deltas above.

1. **L1 `model_fallback` notification (KAS).** Unproven: two flags alone do not manufacture a chain; the
   kind needs a *served* fallback (`server_cascade_config` is server-pushed — forging it is trap 13).
   Next probe: run with both `KIRO_FEATURE_*_FALLBACK*` flags AND a primary model that returns a real
   over-quota/refusal, then read `_meta.kiro.modelFallbackReporting` frames.
2. **L3 multi-client `interaction_resolved` fanout.** Unproven: `acp-server.js` is one process per stdio
   pipe, so this needs the multiplex/ws transport the Rust host owns (`server/multiplex-stream.d.ts`).
   Next probe: two clients over one multiplexed stream (host-lane territory).
3. **L4 `KIRO_API_KEY` home-region auth.** Unrun: no API key in this environment.
   Next probe: mint a key in a non-`us-east-1` region, run `SCENARIO=sweep` with `KIRO_API_KEY` set.
4. **L5 image-placeholder surface.** Unproven: the compiled placeholder string never reaches the client
   wire; only `fs.loadImageCodec.ok` is logged. Next probe: backend-side instrumentation (not client-visible).
5. **`inactiveItems` filter.** Unproven: the workspace had no shadowed/non-active resource items
   (22/22 `standing.kind:"active"`). Next probe: create a user-level override of a bundled steering doc,
   then re-run the `getstate` G2 arm and confirm the item flips to a non-active standing.
6. **available_commands regression causality.** RESOLVED as a fact (re-confirmed on a clean pair: OLD 5
   / NEW 2; the docs still load `active`; the survivors' `_meta.kiro` is unchanged) — what remains open
   is *why* the three steering commands are filtered out of the advertisement. Next probe: diff the
   `_kiro/steering/*` registry content against the advertisement in a v3 TUI session.
7. **H-series — all probed this round (Leg 5).** H1 `--auth-method` ownership CONFIRMED (client-mode
   handshake end-to-end still unproven); H3 `agent upgrade` CONFIRMED; H4 `cleanup-autocomplete-artifacts`
   confirmed present in `kiro-cli-chat`, new in 2.29.1; H5 v2 mid-session fallback UNPROVEN (needs a
   served chain); H6 MCP OAuth URL not run (no OAuth-requiring MCP server available).


## Leg 5 — H-series (host/CLI) + v2 host pair + telemetry

### H1 — `kiro-cli acp --auth-method` (LIVE, CONFIRMED at the wire level)

Archived 2.29.1 host bin (`…/binaries/2.29.1/extracted/kirocli/bin/kiro-cli`, `--version` = `kiro-cli
2.29.1`), which resolves the bundled self-extracted KAS (`…/kiro-cli/kas/2.29.1-32dbef42…/acp-server.js`)
— proved by the capture's `proc_tree`. `acp --help` (zero auth) documents
`--auth-method <METHOD>`: `cli` (default; "Resolve access tokens for the v3 engine from the Kiro CLI
credential store") vs `client` ("Forward the v3 engine's access-token requests to the ACP client").

Live pair (`v2-acpmethod-{client,cli}.jsonl`, same driver copy adapted to take `ACP_ARGS`):

| leg | `_kiro/auth/getAccessToken` frames to the ACP client | note |
|---|---|---|
| `--auth-method client` | **32 requests** (0 recorded token replies from my driver) | the turn then failed `-32000 "Kiro could not load the available models because you are not signed in."` |
| `--auth-method cli` | **0** | the engine resolved the token itself |

So the changelog line is CONFIRMED at the ownership level: `client` routes token acquisition to the
ACP client (requests observed on the wire), `cli` does not (zero). The *end-to-end* success of
client-mode auth is **UNPROVEN** here — my driver observed 32 requests but no reply was recorded for
them and the client-mode prompt still reported "not signed in", i.e. the probe did not complete the
token handshake (probe fidelity, not an engine defect; the requests also carried empty `params`, unlike
KAS's own `_kiro/auth/getAccessToken` which my leg 0b probe answers at init). cyril's responder shape is
`{accessToken, expiresAt, profileArn}` (`crates/cyril-core/src/protocol/kas/auth.rs:24,300`).

### H3 — `kiro-cli agent upgrade <dir>` (LIVE, CONFIRMED; proves the folded 2.28.1 line)

Zero auth, zero turns, archived 2.29.1 bin, fixture = legacy hook config + universal config + symlink +
non-agent JSON (`tmp/h3-*.log`):

- `--dry-run` → "Dry run — no files written"; prints per-file decisions (`✓ would upgrade`, `↷ skipped`,
  `= unchanged`); writes nothing.
- real run → `✓ upgraded legacy.json (1 warning)` with
  `⚠ widened-hook-matcher  hooks: Hook 'preToolUse-0' matcher fs_read -> read now also matches grep,
  glob, code, introspect, tool_search`, `↷ skipped link.json (symlink to a file outside the directory)`,
  `↷ skipped notes.json (not an agent config)`, `= unchanged universal.json (already universal)`;
  `1 upgraded (1 with warnings) · 3 unchanged/skipped · 0 errors`; originals backed up as
  `legacy.json.bak`.

The `fs_read -> read` widening matches the 2.29.1 line "`[V3] /upgrade-agent` rewrites legacy hook
matchers such as `fs_read` so they fire on V3" exactly — this is the *only* live proof of that line.

### H4 — `cleanup-autocomplete-artifacts`: WRONG BINARY, subcommand IS present (LIVE + STATIC)

Seeded temp HOME (`~/.local/bin/{q,qchat,kiro-cli-term}` symlinks + shell-integration lines).
I ran **the launcher** `kiro-cli _ cleanup-autocomplete-artifacts --dry-run` →
`error: unrecognized subcommand 'cleanup-autocomplete-artifacts'` / `Usage: kiro-cli internal [OPTIONS]
<COMMAND>` (so `_` aliases `internal`), and nothing was removed. **That negative was my binary choice,
not an absent feature.** Static check of the archived 2.29.1 bin dir
(`strings -n 8 <bin> | grep -c cleanup-autocomplete-artifacts`):

| binary | 2.29.1 hits | 2.28.0 hits |
|---|---|---|
| `kiro-cli` (launcher) | **0** | 0 |
| `kiro-cli-chat` (chat/engine binary) | **4** | 0 |
| `kiro-cli-term`, `q`, `qchat` (legacy shims) | 0 | – |

So the subcommand lives in **`kiro-cli-chat`** and is **new in 2.29.1** (0 hits in the whole 2.28.0
bin dir) — it agrees with the host lane's static finding. Live re-run on the correct binary
(`tmp/h4b-kiro-cli-chat-cleanup.log`): `kiro-cli-chat _ cleanup-autocomplete-artifacts --dry-run` →
`rc=0`, `would remove: /home/dwalleck/.local/share/kiro-cli/ssh_inner` / `Dry run; nothing was removed.`
— **the subcommand exists and its dry run removes nothing**. (`kiro-cli-chat internal --help` → rc=2
"unrecognized subcommand 'internal'": the chat binary reaches `internal` only via the `_` alias path,
not as a listed subcommand.) The launcher's `kiro-cli internal --help` lists the legacy Amazon-Q set only
(`pre-cmd, local-state, callback, install, … inline-shell-completion-accept, multiplexer, help`) — no
`cleanup-autocomplete-artifacts`. Caveat: my seeded temp HOME did not take effect for the XDG data root
(the script expanded `$HOME` at write time), so the dry run targeted the REAL
`~/.local/share/kiro-cli/` — harmless because it is a dry run and it removed nothing.
This is an **unreleased/dark candidate** (no changelog line mentions it).

### v2 host pair (leg 5 core) — LIVE, IDENTICAL (first attempt voided, re-run clean)

First attempt failed: `run-final.sh` invoked `./probe-v2-live-sweep-2.28.0.py` from the lane dir, but
that shared probe lives in `experiments/conductor-spike/` — `python3: can't open file
…/live/./probe-v2-live-sweep-2.28.0.py: No such file or directory` (both OLD and NEW legs; empty
captures). Voided, fixed by copying it into the lane as `probe-v2-live-sweep-2.29.1.py` (copy, original
untouched) and re-run via `run-final2.sh`.

Clean re-run: `v2-live-sweep-{2.28.0,2.29.1}.jsonl`, archive `bin/` prepended to PATH — the capture's
`proc_tree` proves both ran the ARCHIVED binaries (`…/binaries/2.28.0/extracted/kirocli/bin/…` and
`…/2.29.1/…`), `--version` = `kiro-cli 2.28.0` / `kiro-cli 2.29.1`.

- **`sweep-new-fields.py --diff`: 313 paths (NEW) vs 313 (OLD), 0 only-NEW / 0 only-OLD — IDENTICAL
  field sets, no shape change on this workload** (artifact `sweep-new-fields-diff-v2.txt`).
- Both legs completed the same 3 turns: `p1_read` `end_turn` 7.8 s, `p2_shell` `end_turn` 7.6 s,
  `p3_after_bogus_model` `-32603 Encountered an error in the response stream` — identical outcomes.
- H5 (mid-session model fallback on v2): the probe sets a bogus model mid-session and prompts; **no
  fallback on either engine** (same error shape) → the 2.29.1 line "model fallback changes take effect in
  the current session without restarting" is **not demonstrated** here; it needs a served fallback chain,
  exactly like the KAS-side L1. Recorded as UNPROVEN, not contradicted.
- H6 (MCP OAuth URL over SSH/tmux): not run — no OAuth-requiring MCP server was available in this lane.

### telemetry handshake (dial-vs-suppression) — LIVE, NO CHANGE

`probe-kas-initsettings-2.29.1.py telemetry` with the in-process CONNECT-logging proxy, both engines,
three arms each (`default`, `KIRO_FEATURE_KUTS_TELEMETRY_ENABLED=false`,
`init_telemetryEnabled:false`), `TELEMETRY_IDLE=25`; captures `kas-telemetry-{2.28.0,2.29.1}.jsonl`.

| arm | 0.66.26 connects (n) | 0.66.33 connects (n) |
|---|---|---|
| default | `management.us-east-1.kiro.dev:443`, `runtime.us-east-1.kiro.dev:443` (6) | same (6) |
| `KIRO_FEATURE_KUTS_TELEMETRY_ENABLED=false` | same two, **`telemetry_connects: []`** (6) | same two, **`[]`** (6) |
| `init_telemetryEnabled:false` | same two, **`[]`** (7) | same two, **`[]`** (6) |

**Zero `telemetry*.kiro.dev` dials in every arm on BOTH engines** (positive control: the proxy does see
`management`/`runtime` dials in the same arms, so it is not blind). The handshake key
`clientCapabilities._meta.kiro.telemetryEnabled` is on the wire in every initialize on both engines.
So the 2.29.1 line "`[V3]` non-interactive runs respect `telemetry.enabled` and `KIRO_DISABLE_TELEMETRY`"
does **not** manifest as a change in KAS dials — consistent with the 2.28.0 audit's identical result;
the line's surface is the v3 host/TUI, not the standalone KAS process.


## Changelog reconciliation

`feed-2.29.1.json` (23 lines; the feed-only 2.28.1 entry folds in) and `feed-2.28.1.json` (12 lines).
Location = the live evidence in this lane, else the owning static lane. "not in my lane" is a claim
about the *lane*, not about whether the line is real.

### 2.29.1 (incl. the folded 2.28.1 line)

| # | type | line | location |
|---|---|---|---|
| 1 | fixed | MCP OAuth sign-in (classic + Rust V2) works with issuer-requiring servers | not in my lane (classic/V2 host); wire twin = H6 (`_kiro.dev/mcp/oauth_request`), v2 host pair — see Leg 5 |
| 2 | added | `[V3] /memories` configures memory access + auto updates for local sessions | memory handshake measured: `_meta.memoryConfig` + `configOption memoryReflection` — **wire-identical** OLD→NEW (Leg 3 T-series). The `/memories` surface itself is the TUI lane |
| 3 | changed | V2 TUI hook matchers (`read`) cover more tools | not in my lane (TUI hook payload) |
| 4 | changed | V2 TUI tool hooks receive `tool_tags` | not in my lane — confirmed NEGATIVE on the ACP wire (0 `tool_tags` hits in every capture; H7) |
| 5 | changed | `[V3] kiro-cli acp` uses CLI login unless `--auth-method=client` | Leg 5 H1 (v2 host pair) |
| 6 | changed | `[V3]` custom agents use their tool settings to decide which `mcp.json` servers to start | not in my lane (kas-core/kas-workflow static); adjacent to L8 |
| 7 | changed | `[V3]` stdio MCP servers inherit the full agent environment | **L8 — LIVE CONFIRMED** (`MY_MARKER` from an unlisted env var reached the server) |
| 8 | fixed | Classic `fs_write` no longer auto-approves symlink writes outside `allowedPaths` | not in my lane (classic engine) |
| 9 | fixed | Model fallback changes take effect in the current session without restarting | Leg 5 H5 (v2 host pair); KAS side L1 UNPROVEN (needs a served fallback chain) |
| 10 | fixed | MCP OAuth shows the authorization URL (SSH/tmux) | H6 (v2) — not run; cyril drops the frame when `oauthUrl` is empty (`convert/kiro.rs:1134`) |
| 11 | fixed | `[V3]` non-interactive runs respect `telemetry.enabled` and `KIRO_DISABLE_TELEMETRY` | **measured — no KAS dial change**: 0 `telemetry*.kiro.dev` connects in all 6 arms on both engines (`kas-telemetry-*`); the line's surface is the v3 host/TUI, not the KAS process (Leg 5) |
| 12 | fixed | `[V3] /upgrade-agent` rewrites legacy hook matchers (`fs_read`) | **Leg 5 H3 — LIVE CONFIRMED** (`agent upgrade <dir>` dry-run + real run, `tmp/h3-*.log`) |
| 13 | fixed | Knowledge search limits across bases | not in my lane (kas-core static) |
| 14 | fixed | `[V3]` sessions no longer unexpectedly close under heavy load | no wire surface isolated; not probed (needs load) |
| 15 | fixed | `[V3]` custom agents as subagents load their own `mcpServers` | not in my lane (kas-workflow static) |
| 16 | fixed | `[V3]` effort selector offered at session start | not probed; T-series did not cover effort |
| 17 | fixed | `[V3]` images are resized to fit the selected model instead of failing | **L5 — LIVE PARTIAL** (no request failure on a text-only model; placeholders not client-visible) |
| 18 | fixed | `[V3] KIRO_API_KEY` keys minted outside `us-east-1` authenticate in their home region | L4 UNRUN (no API key available) |
| 19 | fixed | `[V3]` readable retry message for a non-JSON error page | not in my lane (kas-core static) |
| 20 | fixed | `[V3]` resumed sessions keep `UserPromptSubmit` hook output | not in my lane (hooks; kas-core/TUI) |
| 21 | fixed | `[V3]` stop cancels a prompt sent while MCP servers are still starting | not probed |
| 22 | fixed | `[V3]` a prompt sent as a turn ends now includes that turn in the model's history | not probed |
| 23 | fixed | `[V3]` manual steering files load even when their slash command appears mid-prompt | **tension with Leg 3 `commands`**: on 0.66.33 the three steering slash commands vanish from `available_commands_update`. The line is about *loading*; the advertisement regressed. Flagged, not resolved |

### 2.28.1 (feed-only, folded)

| # | type | line | location |
|---|---|---|---|
| 1 | added | `kiro-cli agent upgrade <dir>` upgrades a directory of agent configs for V2 and V3 | Leg 5 H3 (live CLI run, archived 2.29.1 bin) |
| 2 | added | `/settings` Display > Full Screen, copy-on-select | not in my lane (TUI) |
| 3 | added | Classic sessions offer to switch to 3.0 and upgrade agent configs | not in my lane (classic/TUI) |
| 4 | added | `[V3]` hook matchers accept tool tags (`shell`, `read`, `@builtin`, `@mcp`) | not in my lane (hooks); note `tool_tags` is not on the ACP wire (row 4 above) |
| 5 | changed | `/model` and `/effort` pickers open mid-turn | not in my lane (TUI) |
| 6 | fixed | conversations no longer stop responding after a sub-agent finishes (V2 TUI) | not in my lane (TUI) |
| 7 | fixed | expanded activity trays no longer limit task lists to six rows | not in my lane (TUI) |
| 8 | fixed | Windows V2 stdio MCP argument handling | not in my lane (host/Windows) |
| 9 | fixed | voice recording hints point to `kiro-cli settings voice.*` | not in my lane (TUI) |
| 10 | fixed | returning from the agent monitor / session dashboard no longer misplaces the chat view | not in my lane (TUI) |
| 11 | fixed | `[V3]` interrupted tool calls report accurate outcomes | not in my lane (kas-core static) |
| 12 | fixed | `[V3]` reduced memory growth during very long sessions | no wire surface; not probed |

