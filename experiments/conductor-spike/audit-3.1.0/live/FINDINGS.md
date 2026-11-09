# LIVE lane — kiro-cli 2.32.0 → 3.1.0 wire audit (KAS 0.66.43 → 0.66.91)

Started 2026-11-08 21:05 UTC. Lane dir: `experiments/conductor-spike/audit-3.1.0/live/`.
All legs strictly serial; direct KAS carve spawns (`<node> --experimental-wasm-modules <acp-server.js>
--transport=stdio --auth=acp-callback`), fresh temp `HOME` per spawn under `live/tmp/`, real `XDG_DATA_HOME`,
`KIRO_LOG_LEVEL=debug` (kiro.log lines copied beside each capture as `*-kirolog.txt`).

Engines
- OLD = `~/.local/share/kiro-research/kas-carves/2.32.0/node_modules/@kiro/agent/dist/server/acp-server.js` (0.66.43, 12,963,334 B) on node_OLD `~/.local/share/kiro-cli/node` = **v22.22.2**
- NEW = `~/.local/share/kiro-research/kas-carves/3.1.0/node_modules/@kiro/agent/dist/server/acp-server.js` (0.66.91, 16,099,406 B) on node_NEW `~/.local/share/kiro-research/binaries/3.1.0/node` = **v22.23.4**
- intermediates 2.33.0 (0.66.48), 2.33.1 (0.66.58), 2.34.0 (0.66.62), 2.35.0 (0.66.70) on node_OLD
- auth: token row present at start (expires 21:47Z); renewals single-flight via the installed 2.32.0 launcher `kiro-cli user whoami` only

## Summary
- **SAFE for cyril's KAS path.** 2.32.0 → 3.1.0 (KAS 0.66.43 → 0.66.91, node v22.22.2 → v22.23.4) removes no `_kiro/*` method and no `session_info_update` kind; initialize / session/new / prompt / permission / terminal / client-hooks frames keep their shape; 0 × -32601 on anything cyril sends; cyril-on-KAS S1–S3 + V4/V5 pass on build `e3a3868d`.
- **Additive wire:** `session_usage` kind (cumulative credits, replayed on `session/load`) — DROPPED by cyril (P2); turn frames gain `executionId/turnId/timestamp/boundarySource`; `summarizationThreshold`; workflow `credits[]` + `stepsCompleted/stepsTotal/currentNodeId/waitingOnWatch`; `notifyId`; `workflowLaunch` option; hooks v2 runtime behind `hooks:{enabled,v2}` with PreCompact/PostCompact; `turn_timing` (replay-only); `recap` (gated — also on OLD).
- **Behaviour changes:** sub-agent child text now STREAMS on the parent session tagged `agentSubtaskId` and the orphan "Subagent Response" is gone (P2: cyril-1v25 handling is dead on 0.66.91); unparseable shell commands are auto-denied with no ask; `SignInRequiredError` when the auth callback errors; "Approve for me" exists behind `KIRO_TRUST_CLASSIFIER_MODE=enforce`; `_kiro/policy/changed.errors[]`; `_meta.kiro.preview` removed (not a cyril regression).
- **Host 3.1.0:** the runtime extracts to `$KIRO_RUNTIME_DIR`, default `<bin>/../lib/kiro-cli/<ver>-<hash>/{node,bun,tui.js,kas/…}` — nothing under `<data_dir>/kas`, which is all `discovery.rs:208` searches → **P1 candidate** (KAS Free spawn after a 3.1.0 install); logged-out `kiro-cli acp` answers `authMethods kiro-login` and refuses session/new; an unlisted client is routed to **v3** → the explicit-v2 wrapper must pin `--agent-engine v2` (P2).
- **REFUTED / not reproduced:** workspace trust prompt (TUI-only; bare ACP loads everything), WF-L4 wake race, WF-L9 recipe trust gating, MCP `structuredContent` extra block.
- **STOP-AUTH:** the archived 3.1.0 host, opened on a COPY of the auth store, rewrote the copy's token row in 6.7 s; the real store was never touched (expires 23:50:59Z); all later legs ran with no token-visible archived spawn. S4–S8 NOT-RUN (budget); seeds + launch lines are ready.

## Legs
| leg | command / probe | capture(s) | duration | status |
|---|---|---|---|---|
| 0 help | `<node> --experimental-wasm-modules <acp-server.js> --help` OLD (node v22.22.2) / NEW (node v22.23.4) | `tmp/help-{old,new}.{out,err}` | 2×<1 s | completed (no help surface on either) |
| 0 init | `probe-acp-init-3.1.0.py` AUTH=callback NEW/OLD/2.33.0/2.33.1/2.34.0/2.35.0 + AUTH=none NEW | `kas-init-{3.1.0-callback,3.1.0-none,2.32.0,2.33.0,2.33.1,2.34.0,2.35.0}-callback.jsonl` | 7×4 s | completed |
| 1 sweep | `probe-kas-live-3.1.0.py` SCENARIO=sweep OLD→NEW | `kas-live-sweep-{2.32.0,3.1.0}.jsonl` (+`-kirolog.txt`) | 57.7 / 60.4 s | completed |
| 2a workflow gate off | SCENARIO=workflow GATE=off OLD→NEW | `kas-workflow-gateoff-{2.32.0,3.1.0}.jsonl` | 226.8 / 231.1 s | completed (6/6 runs MATCH) |
| 2b workflow gate on | GATE=on OLD→NEW | `kas-workflow-gateon-{2.32.0,3.1.0}.jsonl` | 329.9 / 443.6 s | completed (11/11 MATCH; NEW +114 s = one in-driver token renewal) |
| 3 LIVE-A trust (+WF-L9) | `probe-kas-leads-3.1.0.py trust` ARM=bare NEW/OLD | `kas-lead-trust-{3.1.0,2.32.0}.jsonl` | 45.5 / 45.6 s | completed |
| 3 LIVE-B elicitation (TUI-L6) | `… elicit` ARM=plain NEW/OLD | `kas-lead-elicit-{3.1.0,2.32.0}.jsonl` | 124.9 / 124.6 s | completed |
| 3 LIVE-C(i) unparse | `… unparse` NEW/OLD | `kas-lead-unparse-{3.1.0,2.32.0}.jsonl` | 19.7 / 18.8 s | completed |
| 3 HOST-L1/L3/L5 engine routing | `probe-kas-host31-3.1.0.py engine` 3.1.0 arms a–m; 2.36.0/2.35.2 arms a,b | `host-engine-{3.1.0,2.36.0,2.35.2}.jsonl` | 56 / 3 / 2 s | completed (logged-out gate answers before routing) |
| 3 HOST-L2 extraction | `… extract` | `host-extract-3.1.0.jsonl` | 3 s | completed |
| 3 KAS-L3 sign-in (LIVE-E i) | `… signin` NEW error/empty/nulltoken, OLD error/empty | `kas-lead-signin-{3.1.0,2.32.0}.jsonl` | 42.7 / 28.2 s | completed |
| 3 KAS-L4/TUI-L5 Approve for me | `… policyrule` NEW rule/enforce/evalonly, OLD rule | `kas-lead-policyrule-{3.1.0,2.32.0}.jsonl` | 64.1 / 33.1 s | completed |
| 3 hooks31 (KAS-L7/L8, LIVE-D) | `… hooks31` arms | `kas-lead-hooks31-*-{3.1.0,2.32.0}.jsonl` | — | batch-2 arms VOID (runner overwrote same-named captures; v2 gate not advertised) → re-run in batch 3 with `hooks:{enabled,v2}` (row below) |
| 3 TUI-L11/LIVE-C(ii) tier | `… tier` NEW 4 arms, OLD 2 arms | `kas-lead-tier-{3.1.0,2.32.0}.jsonl` | 169.7 / 85.9 s | completed |
| 3 KAS-L5 memory | `… memory` NEW on/optout/plain, OLD on | `kas-lead-memory-{3.1.0,2.32.0}.jsonl` | 107.5 / 36.7 s | completed |
| 3 WF-L2/TUI-L8 workflowLaunch | `… wflaunch` NEW on/off/crossload, OLD on | `kas-lead-wflaunch-{3.1.0,2.32.0}.jsonl` | 44.9 / 11 s | completed (crossload = OLD-created session loaded by NEW in the same HOME) |
| 3 WF-L3 plan feedback | `… wfplan31` NEW/OLD | `kas-lead-wfplan31-{3.1.0,2.32.0}.jsonl` | 67.0 / 54.9 s | completed |
| 3 LIVE-H sub-agent | `… subagent` NEW/OLD | `kas-lead-subagent-{3.1.0,2.32.0}.jsonl` | 31.5 / 31.9 s | completed |
| 3 zero-turn census (TUI-L4/L9/L10/L12, KAS-L10) | `… zeroturn` NEW plain/diagrams, OLD plain | `kas-lead-zeroturn-{3.1.0,2.32.0}.jsonl` | 10.5 / ~6 s | completed |
| 3 KAS-L1 fallback flags | `… fallbackflags` NEW/OLD plain+off | `kas-lead-fallbackflags-{3.1.0,2.32.0}.jsonl` | 12.3 / 12.5 s | completed (flag values not logged; NOT-REACHED) |
| 3 TUI-L1 recap | `… recap` NEW on/off/absent/underscore, OLD on | `kas-lead-recap-{3.1.0,2.32.0}.jsonl` | 148.8 / 36.9 s | completed |
| 3 KAS-L9 turn_timing | `… turntiming` NEW ttft/perf/control, OLD ttft (+ NEW prompt arm in batch 3) | `kas-lead-turntiming-{3.1.0,2.32.0}.jsonl` | 30.1 / — | session-level arms NOT-REACHED; prompt arm completed in batch 3 (`kas-lead-turntiming-prompt-3.1.0.jsonl`) |
| 3 wfresume (2.34.0 resume) | `… wfresume` NEW/OLD (SIGKILL mid-step, respawn same HOME, load/inspect/resume) | `kas-lead-wfresume-{3.1.0,2.32.0}.jsonl` | 88.3 / — | NEW completed |
| 3 batch 2/3 leads | `probe-kas-leads-3.1.0.py` denyrule / wakerace / jsonpath / peer / shellenv / displaytext (NEW+OLD) | `kas-lead-{denyrule,wakerace,jsonpath,peer,shellenv,displaytext}-{3.1.0,2.32.0}.jsonl` | 22:25–22:33Z | completed |
| 3 hooks31 re-run (LIVE-D) | `run-leads2.sh` arms v2 / v2block / hookscap / agentfile / preblock NEW; v2 / hookscap / agentfile OLD | `kas-lead-hooks31-{v2,v2block,hookscap,agentfile,preblock}-3.1.0.jsonl`, `kas-lead-hooks31-{v2,hookscap}-2.32.0.jsonl` | 22:34–22:50Z | completed (OLD `agentfile` VOID: 701 s stall across token expiry, no renewal) |
| 3 KAS-L9 prompt arm | `… turntiming ARMS=ttft,perf,control` (prompt-level `_meta`) NEW | `kas-lead-turntiming-prompt-3.1.0.jsonl` | 22:50–22:51Z | completed (session-level NEW capture overwritten by the runner; re-run not done — budget) |
| 3 host chain 1 | `probe-kas-host31-3.1.0.py help` + `token` (LIVE-F on a store COPY) + `extract-default` | `host-help-3.1.0.jsonl`, `host-token-3.1.0.jsonl`, `host-extract-default-3.1.0.jsonl` | 22:52–22:53Z | STOPPED at the `token-engine` arm (STOP-AUTH, see Method notes); get-kas-token + default-dir arms completed |
| 3 host chain 2 | `… engine` arms a2,b2,c2,d2,f2,g2,k2 with `--auth-method client` on EMPTY stores | `host-engine-authclient-3.1.0.jsonl` | 22:58–22:59Z | completed |
| 4 file-only | `sweep-new-fields.py --diff` per pair + `unread-fields.py` | `sweep-new-fields-diff-*.txt`, `unread-fields-3.1.0.txt` (70 UNREAD, all triaged) | 22:54–22:57Z | completed |
| 5 cyril-on-KAS | `target/debug/cyril --agent-engine kas` (build e3a3868d) in hex1b, seeded HOME `tmp/cyril-S1-home-p73pf3fz`, `KIRO_KAS_SERVER_PATH`=NEW carve, `KIRO_AGENT_PATH`=node v22.23.4 | `cyril-kas/*.txt`, `cyril-kas/S1-startup.cast`, `cyril-kas/S1-S3-cyril.log` | 23:00–23:16Z | S1–S3, V4, V5(partial), V6, V7 done; S4–S8 NOT-RUN (budget), seeds + launch lines ready in `tmp/leg5-launch.txt` |
| 6 v2 pair | — | — | — | DROPPED per brief |

## Deltas
| item | first version | LIVE | capture#frame"expect" | cyril HANDLED/DROPPED/NOT-SENT + file:line | proposed issue/note |
|---|---|---|---|---|---|
| `session_info_update kind:"session_usage"` `{credits:[{usage,unit,unitPlural}]}` after every `turn_completion`, replayed on `session/load` (`replay:true`) — cumulative session credits | 3.1.0 (0.66.91) | yes | kas-live-sweep-3.1.0.jsonl#53"session_usage", kas-live-sweep-3.1.0.jsonl#388"session_usage" | DROPPED — exact-kind match falls to `_ => None` (`crates/cyril-core/src/protocol/convert/kas.rs:822`); per-turn `turn_completion` still HANDLED (`kas.rs:687`) | P2 NEW: read `session_usage` for the toolbar's session total (replaces summing turns; survives reopen) |
| `turn_start`/`turn_completion`/`turn_end` gain `executionId`, `turnId`, `timestamp`, `boundarySource:"observed"` | 2.33.1 (0.66.58) | yes | kas-live-sweep-3.1.0.jsonl#55"boundarySource" | DROPPED (0 hits) | allowlist: `session/update.update._meta.kiro.{executionId,turnId,boundarySource}` # turn identity, not rendered |
| `context_usage` gains `summarizationThreshold` (top level + under `contextUsage`); initialize `_meta.kiro.autoCompactionThreshold {min,max,default}` | 2.33.1 | yes | kas-live-sweep-3.1.0.jsonl#30"summarizationThreshold", kas-init-2.33.1-callback.jsonl#3"autoCompactionThreshold" | DROPPED (0 hits) | allowlist both; note on cyril-5et2 (context bar could draw the threshold) |
| `extensionMethods` +`_kiro/configuration/set` (2.34.0), +`_kiro/memory/getSyncStatus` (2.35.0), +`_kiro/memory/sync`, +`_kiro/environment/get_network_configuration` (3.1.0); notification `_kiro/memory/syncStatus {state}` (2.35.0) | 2.34.0–3.1.0 | yes | kas-init-3.1.0-callback.jsonl#8"_kiro/memory/sync", kas-live-sweep-3.1.0.jsonl#8"_kiro/memory/syncStatus" | NOT-SENT (methods); `_kiro/memory/syncStatus` DROPPED (unnamed notification) | allowlist `_kiro/memory/syncStatus` # cloud memory sync state; tracked: cyril-0na7 |
| `_kiro/configuration/state` pushes 21 `modelFallback/<model>` connection-layer settings at session/new | 2.33.0–3.1.0 (not pinned) | yes | kas-live-sweep-3.1.0.jsonl#18"modelFallback/claude-opus-5.5" | DROPPED (configurationState is a probe control; cyril does not advertise it) | note on cyril-aoby/cyril-iq2c |
| `_kiro/memory/list` adds `total`, `counts{byType,byScope}` | 3.1.0 | yes | kas-live-sweep-3.1.0.jsonl#150"byScope" | NOT-SENT (cyril never calls `_kiro/memory/list`) | cyril-0na7 note |
| workflow events: `credits[]` on `node_complete`/`run_complete` + snapshot nodes (3.1.0); `stepsCompleted/stepsTotal/currentNodeId/waitingOnWatch` on every event and on live `workflow/list` rows (2.33.0) | 2.33.0 / 3.1.0 | yes | kas-workflow-gateoff-3.1.0.jsonl#193"credits", kas-workflow-gateoff-3.1.0.jsonl#64"stepsTotal", kas-workflow-gateoff-3.1.0.jsonl#99"waitingOnWatch" | DROPPED (0 hits for all five; `WorkflowRun` has no fields) | P3 NEW: step credits + progress in the workflow panel (cyril-4u4a extension) |
| `_kiro/session/notify` gains `notifyId` | 3.1.0 | yes | kas-workflow-gateoff-3.1.0.jsonl#165"notifyId" | DROPPED (0 hits) | allowlist `_kiro/session/notify.notifyId` # correlation id; the `notify-` prefix is already stripped from steering ids (cyril-lki9 P6) |
| `_meta.kiro.preview{file,modifiedContent,originalContent,…}` on fs_write tool calls REMOVED | 3.1.0 | yes (removal) | kas-workflow-gateoff-2.32.0.jsonl#108"modifiedContent" vs kas-workflow-gateoff-3.1.0.jsonl#111"toolOrigin" | never read; diff comes from ACP `content[].diff` (`crates/cyril-core/src/protocol/convert/mod.rs:207`) | allowlist line `session/update.update._meta.kiro.preview.*` is now dead — remove at next edit |
| `workflowLaunch` session config option (when-asked / large-tasks / delegate-all) when `settings.workflows.enabled` | 2.33.1 (0.66.58) | yes | kas-workflow-gateon-3.1.0.jsonl#21"Delegate all work" | DROPPED — pickers model only model/mode/effortLevel (`crates/cyril-core/src/types/kas_config.rs:20-48`) | P3 NEW: expose `workflowLaunch` in `/mode`-class picker when workflows are enabled (ADR-0013 follow-up) |
| `session/prompt` refused with -32000 `SignInRequiredError` "Sign in to Kiro to send a prompt." when the auth callback errors; prompt not saved | 3.1.0 | yes | kas-lead-signin-3.1.0.jsonl#36"SignInRequiredError" | HANDLED generically as a prompt error (busy clears on `TurnCompleted` error path); message text shown | note on cyril-0rn8: show the sign-in text verbatim; leg 5 checks the UI |
| Approve for me: `tool_call_update._meta.kiro.trustClassifierApproved:true`, second `permission-classifier` model call, no `request_permission`; gated by env `KIRO_TRUST_CLASSIFIER_MODE=enforce` + yaml `{capability:all, effect:evaluate}` | 2.36.0 (0.66.70) | yes | kas-lead-policyrule-3.1.0.jsonl#157"trustClassifierApproved", kas-lead-policyrule-3.1.0.jsonl#321"permission-classifier" | DROPPED (0 hits); cyril never sets the env | P3 NEW: opt-in `[agent] approve_for_me` → set env on the KAS child + badge classifier-approved tool rows |
| `_kiro/policy/changed` gains `errors[]`; an unsupported effect is skipped with a warning instead of failing the whole policy (OLD: `status:"failed"` + `_kiro/policy/error` + every tool denied) | 3.1.0 | yes | kas-lead-policyrule-3.1.0.jsonl#28"Skipping rule 0", kas-lead-policyrule-2.32.0.jsonl#43"failed to parse" | DROPPED (`_kiro/policy/changed`, `_kiro/policy/error` unnamed) | P3 NEW: surface `_kiro/policy/error`/`changed.errors[]` as a notice (a bad permissions.yaml silently denies every tool) |
| unparseable shell command auto-denied: `tool_call_update status:"failed"` with "Tool call denied: Kiro could not parse…" and NO permission request (OLD: 2-option ask with `persistableConsent:false`) | 2.34.0 | yes | kas-lead-unparse-3.1.0.jsonl#39"Tool call denied", kas-lead-unparse-2.32.0.jsonl#37"persistableConsentReason" | HANDLED generically (failed tool call) | note on cyril-k8wc: the `persistableConsent:false` ask is gone for this case |
| `_kiro/mcp/elicitation {sessionId,toolCallId,elicitation{mode:form|url,…}}` + `tool_call_update status:pending _meta.kiro.elicitation` mirror | ≤2.32.0 (both engines) | yes | kas-lead-elicit-3.1.0.jsonl#130"requestedSchema", kas-lead-elicit-3.1.0.jsonl#129"elicitation" | HANDLED: visible decline (`crates/cyril-core/src/protocol/kas/mcp.rs`) | note: the pending-update mirror could render the question; leg 5 S6 |
| MCP `structuredContent` → "extra JSON block" | 3.1.0 (claimed) | NOT REPRODUCED (text block only on both) | kas-lead-elicit-3.1.0.jsonl#59"The square of 7 is 49" | NOT-SENT | lead for kas-core; no cyril change |
| workspace trust prompt | 2.33.0 (TUI) | no KAS frame; workspace tier loads | kas-lead-trust-3.1.0.jsonl#26"wsagent", kas-lead-trust-3.1.0.jsonl#188"completed" | n/a | none — P1 candidate REFUTED for ACP clients |
| logged-out `kiro-cli acp` (3.1.0 host): answers initialize with `authMethods kiro-login`/terminal `--login`, refuses session/new -32000 "Authentication required" | 2.37.0 (host) | yes | host-engine-3.1.0.jsonl#1"kiro-login", host-engine-3.1.0.jsonl#8"kiro-login-terminal" | NOT-SENT (cyril's KAS path spawns node directly; the V2 wrapper path would see it) | note on cyril-0rn8 / v2 wrapper: handle `authMethods` + the -32000 refusal |
| 3.1.0 runtime extraction moves to `$KIRO_RUNTIME_DIR/<host-ver>-<hash>/{node,bun,tui.js,kas/node_modules/@kiro/agent/…}`; data dir has no `kas/` | 2.37.0 (`KIRO_RUNTIME_DIR`) / 3.x layout | yes | host-extract-3.1.0.jsonl#1"3.1.0-a53631e2096ad811" | cyril discovery reads `<data_dir>/kas/<kas-ver>-<sha>` (`crates/cyril-core/src/protocol/kas/discovery.rs:208`) | **P1 candidate** (resolved by HOST-L2 default-dir arm): with `KIRO_RUNTIME_DIR` unset the 3.1.0 host extracts to `<bin>/../lib/kiro-cli/3.1.0-<hash>/` (host-extract-default-3.1.0.jsonl#1"lib/kiro-cli/3.1.0-a53631e2096ad811"), i.e. OUTSIDE `<data_dir>/kas` — cyril's KAS Free discovery cannot find the bundle after a 3.1.0 upgrade; needs one confirmation on a real installed 3.1.0 before filing P1 |
| hooks v2 runtime: `initialize.clientCapabilities._meta.kiro.hooks:{enabled:true,v2:true}` → `_kiro/hooks/didChange`, standalone `.kiro/hooks/*.json` (`{version:"v1",hooks:[{name,trigger,matcher?,action:{type:"command",command}}]}`), PreCompact/PostCompact triggers (PostCompact stdin JSON with `compaction_id,reason,summary,…`), KAS-side PreToolUse exit 2 blocks | 2.33.x–3.1.0 | yes | kas-lead-hooks31-v2-3.1.0.jsonl#15"_kiro/hooks/didChange", kas-lead-hooks31-v2-3.1.0.jsonl#81"audit-precompact", kas-lead-hooks31-v2-3.1.0.jsonl#83"audit-postcompact", kas-lead-hooks31-v2-3.1.0.jsonl#85"blocked by a PreToolUse hook", kas-lead-hooks31-agentfile-3.1.0.jsonl#36"v2Hooks is disabled" | NOT-SENT: cyril advertises `hooks` without `v2` (`crates/cyril-core/src/protocol/kas/hooks.rs:383`); client-executed hooks via `_kiro/hooks/executeHook` still HANDLED (`hooks.rs:777-781`) | P2 NEW: a cyril-hosted hook that exits 2 without output does not block (both engines) — cyril must print a reason or the exit code to block; P3: opt into `v2:true` for PreCompact/PostCompact |
| `turn_timing` replay-only kind + prompt-level `_meta` opt-in puts `firstTokenReceivedAt`/perf fields on the last chunk | 3.1.0 (kind) | yes | kas-lead-turntiming-prompt-3.1.0.jsonl#86"turn_timing", kas-lead-turntiming-prompt-3.1.0.jsonl#54"firstTokenReceivedAt" | DROPPED (`kas.rs:822`; cyril never sends the opt-in) | allowlist `session/update.update._meta.kiro.firstTokenReceivedAt`; cyril's own TTFT already shows in `/usage` (leg 5) |
| `recap` kind behind `settings.sessionRecap` | ≤2.32.0 (exists on 0.66.43) | yes | kas-lead-recap-3.1.0.jsonl#57"recap", kas-lead-recap-2.32.0.jsonl#55"recap" | DROPPED (`kas.rs:822`); cyril does not set the gate | P3 note: not a 3.1.0 delta |
| workflow `fileCheck.jsonPath` grammar tightened: `a[0]` rejected at `_kiro/workflow/new` ("unsupported '['"); `$.done` honoured | 3.1.0 | yes | kas-lead-jsonpath-3.1.0.jsonl#31"unsupported", kas-lead-jsonpath-3.1.0.jsonl#100"run_complete" | n/a (recipe authoring) | note for `kiro-workflow-authoring` skill + cyril-xehm epic |
| plan feedback: `appliedPlanFeedback[]` on the revised plan event | 3.1.0 | yes | kas-lead-wfplan31-3.1.0.jsonl#445"appliedPlanFeedback" | DROPPED (0 hits) | P3: show applied feedback in the plan approval overlay (cyril-xehm) |
| `_kiro/configuration/set` refuses client writes to connection-layer settings (`ConfigurationSetRefusedError` / `notGranted`); `_kiro/codeIndex/startBuild` → `CodeIntelligenceNotEnabledError` | 2.34.0 / 3.1.0 | yes | kas-lead-zeroturn-3.1.0.jsonl#119"ConfigurationSetRefusedError", kas-lead-zeroturn-3.1.0.jsonl#119"CodeIntelligenceNotEnabledError" | NOT-SENT | none |
| 3.1.0 host `--auth-method client` (routes only with v3) + `acp.agentEngine` setting; unlisted client → v3 | 2.37.0 / 3.1.0 host | yes | host-help-3.1.0.jsonl#7"--auth-method", host-help-3.1.0.jsonl#7"acp.agentEngine setting", host-engine-authclient-3.1.0.jsonl#2"only supported with --agent-engine=v3" | NOT-SENT (KAS Free spawns node directly); the explicit-v2 wrapper path sends `kiro-cli acp` verbatim | P2 NEW: v2 wrapper must pin `--agent-engine v2` or a 3.1.0 host answers in the `_kiro/*` dialect |
| `kiro-cli chat _ get-kas-token` envelope unchanged; `--force-refresh` still advertised | 3.1.0 host | yes | host-token-3.1.0.jsonl#4"getKasToken", host-help-3.1.0.jsonl#6"force-refresh" | HANDLED (`crates/cyril-core/src/protocol/kas/auth.rs:896-914`) | none |

## Lead verdicts
| lead | verdict | evidence |
|---|---|---|
| LIVE-A workspace trust (P1 candidate) | REFUTED for ACP clients: no trust frame on either engine; workspace agents/MCP/recipes/steering load with no answer | kas-lead-trust-3.1.0.jsonl#26"wsagent", kas-lead-trust-3.1.0.jsonl#77"ECHO:TRUST", kas-lead-trust-3.1.0.jsonl#188"completed" |
| LIVE-B / TUI-L6 MCP elicitation | CONFIRMED on BOTH engines (form + url), unconditional host callback; `structuredContent` JSON block NOT REPRODUCED on either | kas-lead-elicit-3.1.0.jsonl#130"requestedSchema", kas-lead-elicit-2.32.0.jsonl#114"requestedSchema", kas-lead-elicit-3.1.0.jsonl#59"The square of 7 is 49" |
| LIVE-C (i) unparseable command auto-deny (2.34.0) | CONFIRMED NEW (no ask, failed tool update); OLD 2-option ask with `persistableConsent:false` | kas-lead-unparse-3.1.0.jsonl#39"Tool call denied", kas-lead-unparse-2.32.0.jsonl#37"persistableConsentReason" |
| LIVE-C (ii) / TUI-L11 subcommand tier | CHANGED: the tier is a TUI-composed glob; `git stash *` consent works identically on OLD and NEW; user scope persists to permissions.yaml on both | kas-lead-tier-3.1.0.jsonl#111"terminal/create", kas-lead-tier-2.32.0.jsonl#106"terminal/create", kas-lead-tier-3.1.0.jsonl#905"matchedRule" |
| LIVE-C (iii) / KAS-L4 / TUI-L5 Approve for me | CONFIRMED NEW: `trustClassifierApproved:true`, `permission-classifier` model call, gated by env `KIRO_TRUST_CLASSIFIER_MODE=enforce`; `decision:"prompt"` branch NOT-REACHED; OLD: `evaluate` effect is fatal (all tools denied) | kas-lead-policyrule-3.1.0.jsonl#157"trustClassifierApproved", kas-lead-policyrule-3.1.0.jsonl#321"permission-classifier", kas-lead-policyrule-2.32.0.jsonl#43"failed to parse" |
| LIVE-C extra deny rule | CHANGED only in text: `status:failed` + `policyDenial` on both, never "cancelled" | kas-lead-denyrule-3.1.0.jsonl#45"policyDenial", kas-lead-denyrule-2.32.0.jsonl#44"policyDenial" |
| LIVE-D / KAS-L7 / KAS-L8 hooks | CONFIRMED (batch 3): the v2 hook runtime is gated by `initialize.clientCapabilities._meta.kiro.hooks:{enabled:true,v2:true}` (without it `v2Hooks is disabled`); with it KAS emits `_kiro/hooks/didChange`, runs standalone `.kiro/hooks/*.json`, fires PreCompact + PostCompact, and a KAS-executed PreToolUse exit 2 blocks the tool ("blocked by a PreToolUse hook"). Client-executed path (`_kiro/hooks/executeHook`, cyril's shape): exit 2 WITHOUT output does NOT block on either engine (tool completes) | kas-lead-hooks31-agentfile-3.1.0.jsonl#36"v2Hooks is disabled", kas-lead-hooks31-v2-3.1.0.jsonl#15"_kiro/hooks/didChange", kas-lead-hooks31-v2-3.1.0.jsonl#81"audit-precompact", kas-lead-hooks31-v2-3.1.0.jsonl#83"audit-postcompact", kas-lead-hooks31-v2-3.1.0.jsonl#85"blocked by a PreToolUse hook", kas-lead-hooks31-hookscap-3.1.0.jsonl#72"_kiro/hooks/executeHook", kas-lead-hooks31-hookscap-3.1.0.jsonl#74"completed", kas-lead-hooks31-hookscap-2.32.0.jsonl#67"_kiro/hooks/executeHook" |
| LIVE-E (i) / KAS-L3 sign-in refusal | CONFIRMED NEW (error-arm): `SignInRequiredError`, nothing persisted; empty/null token → `ModelRegistryUnauthenticatedError` on both and the prompt is saved | kas-lead-signin-3.1.0.jsonl#36"SignInRequiredError", kas-lead-signin-3.1.0.jsonl#112"ModelRegistryUnauthenticatedError", kas-lead-signin-2.32.0.jsonl#40"TokenExpiredError" |
| LIVE-E (ii) / HOST-L3 logged-out `kiro-cli acp` | CONFIRMED (2.37.0): initialize answered by the host with `authMethods kiro-login` / terminal `--login` variant; `session/new` refused -32000; 2.36.0/2.35.2 launchers exit rc 1 instead | host-engine-3.1.0.jsonl#1"kiro-login", host-engine-3.1.0.jsonl#8"kiro-login-terminal", host-engine-2.36.0.jsonl#1"not logged in" |
| LIVE-F / HOST-L4 / TUI-L7 get-kas-token | CONFIRMED: 3.1.0 `kiro-cli chat _ get-kas-token` envelope unchanged `{kind:"getKasToken",data:{accessToken,expiresAt,profileArn,provider}}`, `expiresAt` equals the store row, same stdout on repeat, and the call itself left the store copy's token row unchanged (cyril's parser `crates/cyril-core/src/protocol/kas/auth.rs:896-914` still fits); `--force-refresh` is still advertised | host-token-3.1.0.jsonl#4"getKasToken", host-token-3.1.0.jsonl#4"expiresAt_equals_store_expires_at", host-token-3.1.0.jsonl#5"same_stdout", host-help-3.1.0.jsonl#6"force-refresh" |
| LIVE-G / KAS-L2 / TUI-L2 session credits | CONFIRMED: `kind:"session_usage"` cumulative credits after each turn + replay on load; OLD 0 hits; equals the running sum of promptTurnSummaries | kas-live-sweep-3.1.0.jsonl#53"session_usage", kas-live-sweep-3.1.0.jsonl#388"session_usage", kas-live-sweep-3.1.0.jsonl#282"0.4589933797678275" |
| LIVE-H sub-agent (+memory) | CHANGED: child text now streams as parent-session `agent_message_chunk` tagged `agentSubtaskId`; "Subagent Response" orphan gone; memory half NOT-REACHED | kas-lead-subagent-3.1.0.jsonl#92"agentSubtaskId", kas-lead-subagent-2.32.0.jsonl#89"Subagent Response" |
| WF-L1 / TUI-L3 step credits | CONFIRMED NEW; node credits == step session_usage; run = sum | kas-workflow-gateoff-3.1.0.jsonl#193"credits", kas-workflow-gateoff-3.1.0.jsonl#291"0.43817720348258704" |
| WF-L2 / TUI-L8 workflowLaunch | CONFIRMED (option, round trip, legacy load = delegate-all with no option; OLD "always delegate") | kas-lead-wflaunch-3.1.0.jsonl#21"when-asked", kas-lead-wflaunch-3.1.0.jsonl#27"delegate-all", kas-lead-wflaunch-3.1.0.jsonl#179"contentCollection" |
| WF-L3 plan feedback | CONFIRMED: revised plan on `rejectionReason` on BOTH; `appliedPlanFeedback[]` NEW only; `<approved_plan_feedback>` not on the wire | kas-lead-wfplan31-3.1.0.jsonl#117"write-done", kas-lead-wfplan31-3.1.0.jsonl#445"appliedPlanFeedback", kas-lead-wfplan31-2.32.0.jsonl#118"write-done" |
| WF-L4 wake race | REFUTED for OLD / same on both: the prompt at `node_complete` runs at once, completion arrives as `steering_injected` (parent_busy_queued), no unsolicited wake turn, 3/3 rounds each | kas-lead-wakerace-3.1.0.jsonl#116"end_turn", kas-lead-wakerace-3.1.0.jsonl#104"steering_injected", kas-lead-wakerace-2.32.0.jsonl#99"steering_injected" |
| WF-L5 jsonPath grammar | CONFIRMED: NEW rejects `a[0]` at workflow/new ("unsupported '['") and `$.done` stops after 1 iteration; OLD accepts `a[0]` and runs BOTH to the cap (its `$.` fileCheck never matched) | kas-lead-jsonpath-3.1.0.jsonl#31"unsupported", kas-lead-jsonpath-3.1.0.jsonl#100"run_complete", kas-lead-jsonpath-2.32.0.jsonl#489"run_complete" |
| WF-L6 list progress | CONFIRMED (live rows only; absent after completion) | kas-workflow-gateoff-3.1.0.jsonl#99"stepsCompleted", kas-workflow-gateoff-3.1.0.jsonl#1152"completed" |
| WF-L7 wake reply length | NOT-REACHED: no truncated wake reply reproduced on OLD today (lengths 37…9790 chars, all complete) | kas-workflow-gateoff-2.32.0.jsonl#1128"review the workflow results" |
| WF-L8 peer notify | NOT-REACHED: `settings.sessionTools` is a known-but-unread key on NEW (unknown on OLD); no `send_message` tool, no peer notify | kas-lead-peer-3.1.0.jsonl#218"unread-key", kas-lead-peer-2.32.0.jsonl#107"unknown-key" |
| WF-L9 recipe trust gating | REFUTED (no gating): workspace recipe runs via `_kiro/workflow/new` on both | kas-lead-trust-3.1.0.jsonl#188"completed" |
| WF-L10 branch landing | NOT RUN (optional, budget) | — |
| TUI-L1 session recap | CONFIRMED gate `settings.sessionRecap`; CHANGED: the kind exists on 0.66.43 too | kas-lead-recap-3.1.0.jsonl#57"recap", kas-lead-recap-2.32.0.jsonl#55"recap" |
| TUI-L4 modelFallback dark | CONFIRMED (-32603 no-classification on both; `modelFallbackReporting:true`) | kas-lead-zeroturn-3.1.0.jsonl#119"_kiro/modelFallback/get" |
| TUI-L9 connection getState / memory sync | CONFIRMED: 21 `modelFallback/*` settings NEW (0 OLD); `getSyncStatus {state:"off"}` NEW | kas-lead-zeroturn-3.1.0.jsonl#119"modelFallback/claude-sonnet-5" |
| TUI-L10 code index build | CHANGED: `startBuild`/`cancelBuild` → `CodeIntelligenceNotEnabledError` (needs the setting); indexStatus not advertised | kas-lead-zeroturn-3.1.0.jsonl#119"CodeIntelligenceNotEnabledError" |
| TUI-L12 diagrams | CONFIRMED: `_kiro/diagram/render` + `diagramRenderFormats ["mermaid","svg"]` only with the flag | kas-lead-zeroturn-3.1.0.jsonl#119"diagramRenderFormats" |
| TUI-L13 / KAS-L6 OAuth | NOT-REACHED (no OAuth MCP fixture) | — |
| KAS-L1 fallback flag defaults | NOT-REACHED (values not logged; 0 fallback kinds in any capture) | kas-lead-fallbackflags-3.1.0.jsonl#55"modelFallback.chains.read" |
| KAS-L5 memory rollout | CONFIRMED NEW: `_kiro/memory/changed`, list `total/counts`, `getSyncStatus`; `userMemoryOptIn:false` = hard off; default still disabled on a bare spawn | kas-lead-memory-3.1.0.jsonl#45"_kiro/memory/changed", kas-lead-memory-3.1.0.jsonl#109"Memory is not enabled" |
| KAS-L9 turn_timing | CONFIRMED with the prompt-level opt-in: live data rides the last chunk (`firstTokenReceivedAt` etc.) and `kind:"turn_timing"` is a REPLAY-ONLY kind on `session/load`; the session-level key is NOT-REACHED | kas-lead-turntiming-prompt-3.1.0.jsonl#54"firstTokenReceivedAt", kas-lead-turntiming-prompt-3.1.0.jsonl#86"turn_timing" |
| KAS-L10 configuration/set | CONFIRMED method shape (`krn`, `layer` required); client writes REFUSED `notGranted` for tangentMode/thinking at user-local | kas-lead-zeroturn-3.1.0.jsonl#119"ConfigurationSetRefusedError" |
| KAS-L11 initialize diff | CONFIRMED (leg 0 + diagrams arm) | kas-init-3.1.0-callback.jsonl#8"_kiro/memory/sync", kas-lead-zeroturn-3.1.0.jsonl#119"diagramRenderFormats" |
| KAS-L12 shellEnvironment | CHANGED: applied by KAS (`session.shell_environment.applied`) but NOT forwarded to client `terminal/create` env on either engine | kas-lead-shellenv-3.1.0.jsonl#71"shell_environment.applied", kas-lead-shellenv-3.1.0.jsonl#49"AWS_SDK_UA_APP_ID" |
| KAS-L13 displayText | CHANGED: replay shows `displayText` as the user chunk on BOTH engines (not 0.66.58+) | kas-lead-displaytext-3.1.0.jsonl#64"SHOWN", kas-lead-displaytext-2.32.0.jsonl#58"SHOWN" |
| HOST-L1 engine routing | CONFIRMED on EMPTY stores via the `--auth-method client` discriminator (accepted only when the resolved engine is v3): an unlisted client (cyril's clientInfo) → v3 and the host asks `_kiro/auth/getAccessToken`; snapshot client names / `acp.agentEngine:v2` / `--agent-engine v2` → "only supported with --agent-engine=v3" ⇒ v2. Token arms on a store copy were STOPPED (STOP-AUTH) | host-engine-authclient-3.1.0.jsonl#3"_kiro/auth/getAccessToken", host-engine-authclient-3.1.0.jsonl#2"only supported with --agent-engine=v3", host-engine-authclient-3.1.0.jsonl#5"only supported with --agent-engine=v3", host-help-3.1.0.jsonl#7"--auth-method" |
| HOST-L2 extraction layout | CONFIRMED `$KIRO_RUNTIME_DIR/<ver>-<hash>/{node,bun,tui.js,kas/…}`; default (env unset) = `<bin>/../lib/kiro-cli/<ver>-<hash>`, nothing under `<data_dir>/kas` — P1 candidate for `crates/cyril-core/src/protocol/kas/discovery.rs:208` | host-extract-3.1.0.jsonl#1"3.1.0-a53631e2096ad811", host-extract-default-3.1.0.jsonl#1"lib/kiro-cli/3.1.0-a53631e2096ad811" |
| HOST-L5 `--agent-engine v1` | CHANGED: accepted, same logged-out answer, no panic | host-engine-3.1.0.jsonl#10"kiro-login" |
| HOST-L6 cyril v2 wrapper vs 3.1.0 `kiro-cli acp` | NOT-RUN — would expose a token to the archived host (orchestrator ruling after STOP-AUTH). Static expectation from HOST-L1: cyril's wrapper spawn `kiro-cli acp` with no `--agent-engine v2` would be routed to v3 on a 3.1.0 host | host-engine-authclient-3.1.0.jsonl#2"only supported with --agent-engine=v3" |
| V1 regression checklist (leg 5) | PASS on build e3a3868d for the reached items: toolbar in ~2 s; first `getAccessToken` served from `chat _ get-kas-token` with `force_refresh:false` (S1-cyril.log); `shell_type` answered in 2 ms; `/probe-skill hello` ran as prompt text (`✓ Loaded skill`, `SKILLRAN hello`); `/model` `/effort` `/mode` round trips OK. NOT-REACHED: Always-allow scopes (S4), sub-agent rendering (S5), `KIRO_HOME` (S7), repeat-cap wake header | `cyril-kas/S1-startup-toolbar.txt`, `cyril-kas/S2-commands-skill-run.txt`, `cyril-kas/S3-settings-effort-set.txt`, `cyril-kas/S3-settings-mode-picker.txt` |
| V2 Always-allow on an unparseable command | CHANGED (moot): on 0.66.91 the ask is gone (LIVE-C i), so cyril shows no overlay and nothing is saved | kas-lead-unparse-3.1.0.jsonl#39"Tool call denied" |
| V3 w55l step-session Always-allow rounds | NOT-RUN (optional; budget) | — |
| V4 effort/thinking post-ack | CONFIRMED: `/effort` picker low/medium/high✓/xhigh/max, pick `low` → "Effort set to low." and toolbar badge `◇ high` → `◇ low`; `/thinking` on `auto` correctly reports "Thinking can't be toggled on the current model" (the toggle on claude-sonnet-5 was not re-tried — budget) | `cyril-kas/S3-settings-effort-set.txt`, `cyril-kas/S3-settings-thinking.txt` |
| V5 `/load` credits vs footer | PARTIAL: after one billed turn `/usage` shows `Credits 0.1352 credits` from per-turn `turn_completion` (HANDLED `kas.rs:687`); the cumulative `session_usage` kind is DROPPED (`kas.rs:822`), so after `/load` the footer would restart from the replayed turns, not the session total; `/new` + `/load` steps NOT-RUN (budget) | `cyril-kas/S5V-usage-overlay.txt`, kas-live-sweep-3.1.0.jsonl#388"session_usage" |
| V6 node engine | CONFIRMED: cyril ran the NEW bundle on `KIRO_AGENT_PATH` = archive node v22.23.4 (≥22.13.0); the 2.32.0 data-dir node is v22.22.2 — both satisfy `engines` | `cyril-kas/S1-cyril.log` |
| V7 `_kiro/workspace/active_file` under cyril | NOT-REACHED: 0 `active_file` lines in the S1 HOME KAS log (`tmp/cyril-S1-home-p73pf3fz/.kiro/logs/*/kiro.log`); cyril does not send the resolver and no agent read triggered it in S1–S3 | — |

## sweep-new-fields + unread-fields
Done in leg 4 (22:54–22:57Z), file-only: `sweep-new-fields.py --diff` for every OLD/NEW pair (`sweep-new-fields-diff-*.txt`) and `unread-fields.py` over all NEW captures against `docs/wire-ignored-fields.txt` → `unread-fields-3.1.0.txt` (70 UNREAD lines, every one triaged as allowlist / P2 / P3 / note). The full triage table, with the `file:line` HANDLED/DROPPED/NOT-SENT proof per line, is the "Leg 4" section of the progress log below; the resulting allowlist proposals are listed under Proposed issues/notes.

## cyril-on-KAS
All rows: cyril build `e3a3868df693f975232250a694bfd2d3552ee084` (HEAD unchanged all session; `tmp/leg5-build-commit.txt`, build log `tmp/cargo-build-leg5.log`), `target/debug/cyril --agent-engine kas`, `KIRO_KAS_SERVER_PATH` = NEW carve (0.66.91), `KIRO_AGENT_PATH` = archive node v22.23.4, temp HOME `tmp/cyril-S1-home-p73pf3fz`, real `XDG_DATA_HOME`, driven through hex1b (recording `cyril-kas/S1-startup.cast`). cyril.log stayed at INFO (`cyril-kas/S1-S3-cyril.log`, 13 lines); the KAS debug log of the run is under the temp HOME `.kiro/logs/`.

| scenario | result | evidence (`live/cyril-kas/`) |
|---|---|---|
| S1 startup + handshake | PASS — launched 23:00:50.457Z, toolbar `sess_000207c9… · vibe · auto` within ~2 s; one `getAccessToken` served via `chat _ get-kas-token` (`force_refresh:false`, KAS logs it as `acp-peer.refreshToken.ok durationMs:809` = the round trip to cyril, NOT an OIDC refresh); `shell_type` answered in 2 ms; no `force_refresh:true`, no token failure in the whole run | `S1-launch-time.txt`, `S1-startup-toolbar.txt`, `S1-cyril.log` |
| S2 advertised commands | PASS — `/` autocomplete lists the KAS skills; `/probe-skill hello` goes out as prompt text, KAS answers `✓ Loaded skill: probe-skill` then `SKILLRAN hello`; the skill-load tool call raised a permission overlay (cyril WARN "approval preview raw_input has no string text/content field", P3 display nit) | `S2-commands-slash.txt`, `S2-commands-skill-run.txt`, `S1-S3-cyril.log` |
| S3 settings | PASS — `/thinking` on `auto`: "Thinking can't be toggled on the current model"; `/model` picker = full catalog, filter `claude-sonnet-5` + Enter → "Model set to claude-sonnet-5." and toolbar `claude-sonnet-5 ◇ high · think`; `/effort` → `low` → badge `◇ low` (V4); `/mode` picker Default✓/Spec/Quick Spec/Bug Fix/Plan/Autonomous/semantic_reviewer — no `workflowLaunch` row (DROPPED, `types/kas_config.rs:20-48`; workflows were not enabled in this HOME; the seeded `cyril-S3-home-vfnzbflf` with `chat.enableWorkflows=true` was NOT-RUN) | `S3-settings-thinking.txt`, `S3-settings-model.txt`, `S3-settings-model-picked-effort-picker.txt`, `S3-settings-effort-set.txt`, `S3-settings-mode-picker.txt` |
| V5 `/usage` | PARTIAL — Overview: Turns 1, Provider requests 2, Retries 1, TTFT 1784 ms, duration 84 s, `Credits 0.1352 credits`, token columns `n/a (backend-gated)`; credits come from per-turn `turn_completion`, not from the new cumulative `session_usage` (DROPPED); `/new` + `/load` NOT-RUN | `S5V-usage-overlay.txt` |
| S4 trust (git stash list, Always allow, permissions.yaml) | NOT-RUN (budget after STOP-AUTH); seed `tmp/cyril-S4-*` + launch line ready | `tmp/leg5-launch.txt` |
| S5 sub-agent delegation (LIVE-H in the UI) | NOT-RUN; expectation from the wire: child text arrives as parent `agent_message_chunk` tagged `agentSubtaskId`, so cyril renders it inline as parent text with no sub-agent attribution | kas-lead-subagent-3.1.0.jsonl#92"agentSubtaskId" |
| S6 MCP fixture (`probe_echo` Tool Load, `probe_elicit` decline) | NOT-RUN; seed with `fixtures/mcp_probe_server.py` ready | `tmp/leg5-launch.txt` |
| S7 `KIRO_HOME` | NOT-RUN; seed ready | `tmp/leg5-launch.txt` |
| S8 workspace tier | NOT-RUN; seed ready | `tmp/leg5-launch.txt` |
| V6 node | PASS — v22.23.4 archive node ran the 0.66.91 bundle under cyril; cyril's own discovery was bypassed via `KIRO_AGENT_PATH` (the discovery P1 candidate is therefore untested in the UI) | `S1-cyril.log` |
| V7 `_kiro/workspace/active_file` | NOT-REACHED — 0 lines in the run's KAS log | `tmp/cyril-S1-home-p73pf3fz/.kiro/logs/` |
| shutdown observation | Ctrl+Q / Ctrl+C sent through hex1b did not reach cyril as control bytes (hex1b key encoding, not a cyril bug); SIGTERM to cyril left the KAS node child alive until killed by hand — cyril's SIGTERM path does not reap the child (P3 to verify with a real terminal) | process table 23:15Z |

## Proposed issues/notes
P1 candidate
- **KAS Free discovery vs 3.1.0 runtime layout** — the 3.1.0 host extracts to `$KIRO_RUNTIME_DIR` (default `<bin>/../lib/kiro-cli/<ver>-<hash>/kas/node_modules/@kiro/agent/…`), nothing under `<data_dir>/kas`; `crates/cyril-core/src/protocol/kas/discovery.rs:208` searches `<data_dir>/kas` only and fails with "KAS bundle not found" (`:51`). Confirm once on a real installed 3.1.0 (the archived host in a scratch `KIRO_RUNTIME_DIR` is the only evidence), then file P1: search `KIRO_RUNTIME_DIR`, then `<bin>/../lib/kiro-cli/<ver>-*`, then the legacy `<data_dir>/kas`.

P2
- `session_usage` DROPPED (`kas.rs:822`) — read it for the toolbar/`/usage` session total (survives `/load`; equals the running sum of `promptTurnSummaries`).
- Sub-agent child text on the parent session (`_meta.kiro.agentSubtaskId`) — route tagged chunks to a sub-agent stream instead of inline parent text; retire the orphan "Subagent Response" path (cyril-1v25) for 0.66.91.
- Explicit-v2 wrapper must pin `--agent-engine v2`: a 3.1.0 host routes an unlisted client to v3 and answers a logged-out initialize with `authMethods kiro-login` (handle the -32000 "Authentication required" refusal in the wrapper path; cyril-0rn8).
- cyril-hosted hook exit 2 WITHOUT output does not block the tool on either engine — cyril's `executeHook` reply must carry a reason/exit code that KAS honours, or document that hooks must print to block.

P3
- `workflowLaunch` config option in a picker when `settings.workflows.enabled` (ADR-0013 follow-up).
- Approve-for-me opt-in: set `KIRO_TRUST_CLASSIFIER_MODE=enforce` on the KAS child + badge `trustClassifierApproved` rows.
- Surface `_kiro/policy/error` and `_kiro/policy/changed.errors[]` (a bad permissions.yaml silently denies every tool on OLD; NEW skips the rule with a warning).
- Workflow panel: step `credits[]` + `stepsCompleted/stepsTotal/currentNodeId/waitingOnWatch`; `appliedPlanFeedback[]` in the plan overlay (cyril-4u4a / cyril-xehm).
- `/thinking` + `/effort` after a model switch: re-check the toggle on a thinking-capable model (left on `auto` in S3).
- Approval overlay: "approval preview raw_input has no string text/content field" for skill-load tool calls — render the skill name instead of a blank preview.
- Usage tracker: "usage context update has no observed turn identity, ignoring" on the pre-turn `context_usage` push — decide whether the initial context fill should seed the bar.
- SIGTERM to cyril leaves the KAS child running (verify outside hex1b; if real, reap the child in the bridge shutdown path).

Allowlist proposals (`docs/wire-ignored-fields.txt`): `session/update.update._meta.kiro.{executionId,turnId,timestamp,boundarySource}`, `context_usage.summarizationThreshold` (+ `contextUsage.summarizationThreshold`), `initialize._meta.kiro.autoCompactionThreshold.*`, `_kiro/memory/syncStatus`, `_kiro/session/notify.notifyId`, `_meta.kiro.firstTokenReceivedAt`; REMOVE the dead `session/update.update._meta.kiro.preview.*` line. Full triage of the 70 UNREAD lines is in the Leg 4 section of the progress log.

Notes for other lanes / docs: `structuredContent` extra JSON block NOT REPRODUCED (kas-core lead); `recap` kind predates 3.1.0; `displayText` replay and `shellEnvironment` apply on BOTH engines; the `kiro-workflow-authoring` skill should document the `jsonPath` grammar (`[` unsupported) and `appliedPlanFeedback`.

## Method notes
Traps hit (all recorded as they happened in the progress log):
- **STOP-AUTH incident (22:53:20Z).** Host chain 1 ran LIVE-F on a byte COPY of `~/.local/share/kiro-cli/data.sqlite3` in a scratch `XDG_DATA_HOME`. `get-kas-token` on the copy was clean (row unchanged, host-token-3.1.0.jsonl#1"token_row_changed"). The next arm, archived 3.1.0 `kiro-cli acp` on the same copy, changed the copy's `auth_kv` token row within 6.7 s (host-token-3.1.0.jsonl#7"copy token row changed"; KAS log `acp-peer.refreshToken.ok durationMs:8`; `state` +1 row, `extracted_kas_versions` 4→2, schema unchanged host-token-3.1.0.jsonl#8"copy_schema_changed"). Whether that was a real OIDC refresh (which would have invalidated the single-use refresh token in the REAL store) is unprovable from the client side; the copy was deleted (host-token-3.1.0.jsonl#9"copy_deleted"), the real store's `expires_at` stayed 23:50:59Z, and the orchestrator was paged. Ruling: no `whoami`, no `--force-refresh`, no archived spawn that can see a token; HOST-L6 NOT-RUN; HOST-L1 only on EMPTY stores (done, `--auth-method client` discriminator). Later observation: KAS labels the ordinary `getAccessToken` round trip to its client `acp-peer.refreshToken.ok` as well (cyril S1 run: 809 ms = the `get-kas-token` subprocess), so that log line alone is not evidence of an OIDC refresh — the row rewrite is.
- **Trap (new, own words):** a copy of the real auth store must only ever be opened by `kiro-cli chat _ get-kas-token` WITHOUT `--force-refresh`; `kiro-cli acp` on ANY store that holds a token writes the token row within seconds, and the client cannot tell a re-write from a refresh. Archived-host legs run on EMPTY stores only; anything needing a token runs through the installed launcher that owns the user's session.
- **Handshake placement (trap 5)** honoured: every probe writes `initialize.clientCapabilities._meta.kiro.{settings.*, configurationState:true, hooks:{enabled,v2}}`; positive control `_kiro/configuration/state` on every NEW capture. The hooks v2 gate was found by reading the bundle after the first hooks pass produced "v2Hooks is disabled".
- **Runner overwrote same-named arm captures** (`run-leads.sh` wrote every arm of one probe to one file): batch-2 hooks arms VOID and re-run via `run-leads2.sh` (suffix by `ARM=`); `ARMS=` is still unsuffixed and overwrote the session-level `turntiming` NEW capture (renamed to `-prompt-`; the session-level re-run was not done — budget). Any citation to `kas-lead-turntiming-session-3.1.0.jsonl` in the progress log is therefore a pending re-run, not a file.
- **hex1b input:** a literal newline in `send_terminal_input` arrives as Ctrl+J and inserts `j`; `\uXXXX` escapes are sent literally; Ctrl+Q/Ctrl+C via `send_terminal_key` did not reach cyril as control bytes. Use `send_terminal_key Enter` for submits and kill the process for shutdown.
- **check-citations discipline:** expectations are simple substrings without escaped quotes, frame numbers looked up (0-based, lane-dir captures), re-run after every section. The trimmed bundles under `experiments/conductor-spike/kas-*-3.1.0.jsonl` carry a header record (frame N there = N+1).
- **OLD `agentfile` hooks arm VOID:** a 701 s stall across the 21:47Z token expiry with no in-driver renewal; the turn timed out. Low value, not re-run.
- **MCP fixture threads** died at EOF until joined; `unread-fields.py` swallows captures placed after `--src` (put captures first).
- Billing: all legs used the pre-approved billed KAS sessions; the leg-2b NEW run (+114 s) contains one in-driver single-flight renewal via `kiro-cli user whoami` BEFORE the STOP-AUTH ruling; none after.
- Processes: every probe node, the cyril process and its KAS child were stopped before hand-back (SIGTERM to cyril left the node child alive; killed by hand). hex1b session stopped and removed.

## Progress log (appended as legs finish)

### Leg 0 — `--help` + initialize-only matrix (DONE 21:12-21:16 UTC)
- `--help`: neither engine has a help surface; both start, print nothing on stdout, exit 0 on stdin EOF (`tmp/help-{old,new}.{out,err}`,
  rc=0, 0 B stdout, 666 B stderr each). stderr identical modulo version (`kas.server.starting … 0.66.43|0.66.91`) and pid.
  NEW names its log directory `<ts>-<pid>` (`.kiro/logs/20261108T211255970-24843/`), OLD `<ts>` only — matches the 3.1.0 line
  "Starting a session no longer deletes the logs of another session that is still running". Both still print `[INFO]` startup lines on
  stderr from a direct spawn; the 3.1.0 "`kiro-cli acp` no longer print the agent's `[INFO]` startup lines" is therefore a HOST-side filter
  (lead for the host lane; direct `node acp-server.js` spawns — cyril's path — still see them).
- initialize-only (`probe-acp-init-3.1.0.py`, full path `clientCapabilities._meta.kiro.{configurationState,settings,telemetryEnabled}`,
  positive control `configurationState:true` → `_kiro/configuration/state` pushed in every arm): captures `kas-init-{3.1.0-callback,
  3.1.0-none,2.32.0-callback,2.33.0-callback,2.33.1-callback,2.34.0-callback,2.35.0-callback}.jsonl`, 4 s each, node versions recorded in
  frame #0 (kas-init-3.1.0-callback.jsonl#0"v22.23.4", kas-init-2.32.0-callback.jsonl#0"v22.22.2").
- callback vs none on NEW: initialize result IDENTICAL; callback asks `_kiro/auth/getAccessToken` 1× at init
  (kas-init-3.1.0-callback.jsonl#2"_kiro/auth/getAccessToken"), none 0× (kas-init-3.1.0-none.jsonl#5"callbacks").
- OLD→NEW initialize result, LIVE: `protocolVersion` 1 = 1, `authMethods` [aws-builder-id, aws-iam-identity-center] unchanged, no `agentInfo`
  on either; `agentCapabilities.{loadSession,mcpCapabilities,promptCapabilities,sessionCapabilities}` unchanged.
  `_meta.kiro` deltas, attributed per carve:
  - `extensionMethods` 28 → 32: `+_kiro/configuration/set` first in **2.34.0** (kas-init-2.34.0-callback.jsonl#3"_kiro/configuration/set"),
    `+_kiro/memory/getSyncStatus` first in **2.35.0** (kas-init-2.35.0-callback.jsonl#3"_kiro/memory/getSyncStatus"),
    `+_kiro/memory/sync` and `+_kiro/environment/get_network_configuration` first in **3.1.0** (kas-init-3.1.0-callback.jsonl#8"_kiro/memory/sync",
    kas-init-3.1.0-callback.jsonl#8"_kiro/environment/get_network_configuration"). Nothing removed.
  - `+_meta.kiro.autoCompactionThreshold {min:0, max:80, default:80}` first in **2.33.1** (kas-init-2.33.1-callback.jsonl#3"autoCompactionThreshold");
    absent in 2.33.0 (kas-init-2.33.0-callback.jsonl#3"modelFallbackReporting").
  - NEW notification at init: `_kiro/memory/syncStatus {state:"off"}` first in **2.35.0** (kas-init-2.35.0-callback.jsonl#5"_kiro/memory/syncStatus"),
    also on NEW (kas-init-3.1.0-callback.jsonl#5"_kiro/memory/syncStatus") — pushed BEFORE the initialize result is answered? No: after (frame #3 is the result).
  - NEW pushes `_kiro/configuration/state` THREE times at init (kas-init-3.1.0-callback.jsonl#4"_kiro/configuration/state",
    kas-init-3.1.0-callback.jsonl#6"_kiro/configuration/state", kas-init-3.1.0-callback.jsonl#7"_kiro/configuration/state"; settings 1 → 2 → 22) where OLD
    pushes once (kas-init-2.32.0-callback.jsonl#4"_kiro/configuration/state"). The first push is byte-equivalent OLD vs NEW (same 9 layers, 3 resources,
    1 setting). The later pushes carry the resolved connection-layer settings (analysed in leg 1).
- Intake log (trap 28): both engines log `configuration.intake.unknown-key` for `setting/thinking` and `setting/largeToolOutputHandler` at the
  `client-connection` layer (kas-init-3.1.0-callback.jsonl#10"setting/thinking", kas-init-2.32.0-callback.jsonl#7"setting/thinking") — the
  2.32.0 verdict on `settings.thinking` (cyril-53qx) stands on 0.66.91. NEW additionally logs `cps.auth_provider.unknown {provider:"none"}` twice
  (kas-init-3.1.0-callback.jsonl#10"cps.auth_provider.unknown") — the callback reply carries no `authMethod`/`provider`; lead: does 3.1.0 want them
  (LIVE-E/F).
- STATIC side-note from the bundles (lead, not verdict): `workspaceTrusted:!0` is hard-coded in BOTH engines' stdio and ws bootstraps
  (`new R7e(e,{…workspaceTrusted:!0…})` in 0.66.43; `new lrt(t,{…workspaceTrusted:!0…})` in 0.66.91), and no `trust`-named handshake key or
  `_kiro/*` method exists in either bundle → the 2.33.0 workspace-trust prompt is a HOST/TUI feature; a direct ACP spawn is always trusted.
  LIVE-A tests this. New trust identifiers in 0.66.91 are all `trustClassifier*` (= 2.36.0 "Approve for me"): the approval request grows
  `_meta.kiro.trustClassifier` and tool_call_update grows `classifierApproved` / `classifierFlag {decision:"prompt", reason}` (LIVE-C iii).

### Leg 1 — paired SCENARIO=sweep (DONE 21:19-21:23 UTC; same backend, 2 min apart)
- Driver `probe-kas-live-3.1.0.py` (copy of 2.32.0 + `git stash list` p2, old echo command as p2b, extended census, generalized config census,
  kiro.log copy). OLD `kas-live-sweep-2.32.0.jsonl` (391 frames, 57.7 s, node v22.22.2 kas-live-sweep-2.32.0.jsonl#0"v22.22.2"), NEW
  `kas-live-sweep-3.1.0.jsonl` (403 frames, 60.4 s, node v22.23.4 kas-live-sweep-3.1.0.jsonl#0"v22.23.4"). Both: init ok, 5 turns
  (p1 read, p2 `git stash list`, p2b echo, p3 bogus model → -32000 InvalidModelError on both, p4 write outside), no auth renewal, ~7 s per
  turn (not suspiciously short, trap 21). `_kiro/terminal/shell_type` answered `bash` once per connection on both
  (kas-live-sweep-3.1.0.jsonl#15"_kiro/terminal/shell_type"; trap 34 — cyril's own reply is read in leg 5).
- **Session credits (3.1.0), LIVE — the carrier is a NEW `session_info_update` kind `session_usage`**: after every `turn_completion` NEW emits
  `{kind:"session_usage", credits:[{usage:<cumulative credits>, unit:"credit", unitPlural:"credits"}]}` — kas-live-sweep-3.1.0.jsonl#53"session_usage"
  (0.1319 after turn 1), kas-live-sweep-3.1.0.jsonl#92"0.23706273830845773" (cumulative after turn 2), kas-live-sweep-3.1.0.jsonl#282"0.4589933797678275"
  (after turn 5); also replayed once at the end of `session/load` with `replay:true` (kas-live-sweep-3.1.0.jsonl#388"session_usage"), which is the
  3.1.0 "footer's session credit total no longer resets to zero when a session is reopened". OLD never sends it (0 hits; OLD kinds:
  kas-live-sweep-2.32.0.jsonl#390"info_kinds"). The per-turn `turn_completion.promptTurnSummaries[].usage` is unchanged on both
  (kas-live-sweep-2.32.0.jsonl#49"promptTurnSummaries", kas-live-sweep-3.1.0.jsonl#52"promptTurnSummaries"). No `_kiro/session/getUsage`-class
  method exists (`_kiro/session/getUsage` and `_kiro/usage/get` are -32601-class on both — census); `_kiro/account/getUsage` is the only pull
  method and is account-level (kas-live-sweep-3.1.0.jsonl#148"KIRO PRO MAX"). cyril: see leg 4 triage (expected DROPPED — no `session_usage` reader).
- **Turn-lifecycle frames grew identity fields (NEW)**: `turn_start`/`turn_completion`/`turn_end` now carry `executionId`, `turnId`, `timestamp`
  and `boundarySource:"observed"` (kas-live-sweep-3.1.0.jsonl#39"boundarySource", kas-live-sweep-3.1.0.jsonl#55"turnId"); OLD `turn_end` has only
  `{turnEnd, kind, stopReason, messageId}` (kas-live-sweep-2.32.0.jsonl#51"turn_end"). `context_usage` gained `summarizationThreshold` (80) at both the
  top level and under `contextUsage` (kas-live-sweep-3.1.0.jsonl#30"summarizationThreshold"), matching the new `_meta.kiro.autoCompactionThreshold`
  capability (leg 0, 2.33.1).
- **Approval request shape (2.34.0 reorder, 2.36.0 subcommand tier): NOT on the wire.** Both engines send the same four options in the same
  order `accept/Allow (allow_once)`, `always-accept/Always allow (allow_always)`, `reject/Deny (reject_once)`, `always-reject/Always deny (reject_always)`
  for `git stash list` — kas-live-sweep-2.32.0.jsonl#64"always-accept", kas-live-sweep-3.1.0.jsonl#68"always-accept" — with identical
  `_meta.kiro {toolId:"run_command", command, consent{capability:"shell", resource, askType:"implicit", triggeringResource, workspaceRoot}, consentRound:1}`.
  No `git stash *` tier, no rule options, no `trustClassifier` key on a plain spawn → the reorder and the subcommand tier are TUI-side renderings of
  the consent echo (`_meta.kiro.consent.scope/resource` on the reply), not new options. cyril's overlay therefore cannot "surface" them from the
  request alone; the tier exists only if the client composes the `resource` glob itself (LIVE-C ii). The write approval (p4, NEW model wrote after
  one ask) is the same 4-option shape with `toolId:"fs_write"` (kas-live-sweep-3.1.0.jsonl#262"fs_write"); OLD's model declined p4 without a tool call
  this time (model variance, as in the 2.32.0 audit, reversed).
- **New methods behave (NEW) / fail with the persistence-classification internal error (OLD)**: `_kiro/configuration/set {}` → NEW -32602
  "Invalid params: krn: Required" (kas-live-sweep-3.1.0.jsonl#197"krn: Required") — it takes a `krn` (e.g. `krn:::setting/workflows`), lead for a
  write probe; `_kiro/memory/getSyncStatus {}` and `_kiro/memory/sync {}` → NEW `{state:"off"}` (kas-live-sweep-3.1.0.jsonl#201"state");
  `_kiro/memory/list` → NEW adds `total` and `counts{byType,byScope}` (kas-live-sweep-3.1.0.jsonl#150"byScope"); `_kiro/environment/get_network_configuration`
  → NEW -32000 `NetworkConfigurationUnavailableError` "no remote sessions endpoint configured" (kas-live-sweep-3.1.0.jsonl#205"NetworkConfigurationUnavailableError");
  `_kiro/diagram/render` → NEW -32602 `DiagramsNotEnabledError` (kas-live-sweep-3.1.0.jsonl#213"DiagramsNotEnabledError"; the `diagrams` client cap gates it);
  `_kiro/codeIntelligence/indexStatus`, `_kiro/job/output` → -32603 "no persistence classification" on BOTH (not advertised; kas-live-sweep-3.1.0.jsonl#209"indexStatus").
  OLD answers every new method with the -32603 persistence-classification error (kas-live-sweep-2.32.0.jsonl#182"has no persistence classification").
  `session/set_model` is still -32603 on both (kas-live-sweep-3.1.0.jsonl#231"session/set_model").
- **Connection-layer model-fallback settings pushed at session/new (NEW only)**: NEW's `_kiro/configuration/state` after `session/new` carries 21
  `krn:::setting/modelFallback/<modelId>` entries (e.g. `claude-sonnet-5 → ["claude-haiku-4.5"]`, `claude-opus-5.5 → ["claude-opus-4.8"]`, most `[]`)
  — kas-live-sweep-3.1.0.jsonl#18"modelFallback/claude-opus-5.5" — then the 12-layer session view with the same 21 + `workflows:"off"`
  (kas-live-sweep-3.1.0.jsonl#29"setting/workflows"). OLD pushes only `workflows:"off"` (kas-live-sweep-2.32.0.jsonl#26"setting/workflows"). This is the
  2.33.0 "Fallback models can be set again from `/model fallback`" state surfacing; the per-model fallback list is now visible to a configurationState client.
- **`_kiro/memory/syncStatus {state:"off"}` notification** fires on connect and on `session/load` (kas-live-sweep-3.1.0.jsonl#8"_kiro/memory/syncStatus",
  kas-live-sweep-3.1.0.jsonl#334"_kiro/memory/syncStatus"); first in 2.35.0 (leg 0). cyril: notification method never named (leg 4 triage).
- Unchanged OLD→NEW: session/new result key set and `_meta` (`memoryConfig {disabled,false}`, `semanticReviewEnabled`, `ftaEnabled:false`,
  `workflowsEnabled:false`), configOptions [mode, model, memoryReflection, autopilot, contentCollection] (no `outputStyle`/`effortLevel`/`thinking`
  session option on either; `set_config_option` for those is still silently accepted and not echoed), 7 modes, `_kiro/config/template` outputStyle
  options [default, proactive, concise, explanatory, learning], `session/load` result `[_meta, configOptions, modes]` with the saved `model` on both
  (warm catalog on both — trap 33), `_kiro/account/getUsage` shape (plan KIRO PRO MAX), read-only census answers for every curated method.
- Model catalog (same backend, same day): 21 options on both, same `_meta.kiro` key set per option (`hasEffort, effortLevels, defaultEffortLevel,
  effortSchemaPath, rateMultiplier, rateUnit, thinkingToggleable`) — no model-option shape change.
- sweep-new-fields (`sweep-new-fields-diff-sweep.txt`): 921 vs 872 paths; 50 only-NEW. Version deltas: `session_usage`/`credits[]`, `executionId`,
  `turnId`, `boundarySource`, `summarizationThreshold`, `autoCompactionThreshold`, memory `total/counts`, `syncStatus.state`, `result._meta` of
  session/new… (sessionId-level `_meta` unchanged in content — `result._meta._meta` is an artifact of the diff key `_meta.kiro` nesting). Coverage,
  not version: the p4 write family (`fs/write_text_file`, `checkpoint.{local,modified,original}`, `content[].{oldText,newText,path}`, `resources[]`,
  `rawInput.text`) because OLD's model declined the write this run; `rawInput.cwd` (model-chosen optional arg; 12 NEW frames, 0 OLD — the NEW
  model passes `cwd` to run_command every time, kas-live-sweep-3.1.0.jsonl#64"cwd" — model/system-prompt variance, lead only).

### Leg 2a — SCENARIO=workflow GATE=off pair (DONE 21:23-21:33 UTC)
- OLD `kas-workflow-gateoff-2.32.0.jsonl` (1267 frames, 226.8 s), NEW `kas-workflow-gateoff-3.1.0.jsonl` (1323 frames, 231.1 s); both completed
  with the driver summary, `workflowsEnabled:false` on both, 7 = 7 permission asks, no auth renewal. Run parity: all 6 runs MATCH event-for-event
  (linear 13, loop 23, watch 11, bgwatch 10, park 10, agenturi 5 — kas-workflow-gateoff-3.1.0.jsonl#1322"\"events\"" vs kas-workflow-gateoff-2.32.0.jsonl#1266"\"events\"");
  every terminal run auto-woke the parent on both (`agentInitiatedReason` values `send-message-wake` + `workflow-complete-wake` on both); the repeat-cap
  pause wake (2.32.0 finding, cyril-6lql) is unchanged: NEW `_kiro/session/notify` on the loop pause followed by a `send-message-wake` parent turn
  (kas-workflow-gateoff-3.1.0.jsonl#514"send-message-wake"). `recipes_changed` 0 = 0 this run on both (the 2.32.0 lane saw 2 with the gate on — re-checked in 2b).
- **Workflow step credits (3.1.0), LIVE**: `node_complete` and `run_complete` carry `credits:[{usage, unit:"credit", unitPlural:"credits"}]` per step
  (kas-workflow-gateoff-3.1.0.jsonl#193"\"credits\""; s1 = 0.19 credits), and every node in `finalState.root.children[]` (nested repeat/parallel children too)
  carries its own `credits[]` (kas-workflow-gateoff-3.1.0.jsonl#91"finalState"). OLD has none (kas-workflow-gateoff-2.32.0.jsonl#172"node_complete", no `credits`).
  Static bisect: `"session_usage"` 0 → 3 only in 0.66.91 → first in **3.1.0**.
- **Run progress fields on EVERY `_kiro/workflow/*` event and on `workflow/list` rows (first in 2.33.0)**: `stepsCompleted`, `stepsTotal`, `currentNodeId`,
  `waitingOnWatch` — on run_start/node_start/node_complete/node_paused/paused/loop_iteration/watch_poll/run_complete
  (kas-workflow-gateoff-3.1.0.jsonl#64"stepsTotal", kas-workflow-gateoff-3.1.0.jsonl#193"currentNodeId") and on `_kiro/workflow/list` `runs[]`
  (kas-workflow-gateoff-3.1.0.jsonl#99"waitingOnWatch"); absent on OLD (kas-workflow-gateoff-2.32.0.jsonl#96"parentSessionId"). Static: `stepsCompleted`
  0 → 3 at 0.66.48 → **2.33.0** ("compact step tree" plan approvals). cyril: 0 hits for all four (DROPPED; leg 4 triage).
- **`_kiro/session/notify` gained `notifyId`** (`notify-<uuid>`; kas-workflow-gateoff-3.1.0.jsonl#165"notifyId"); OLD notify has `{sessionId, callerSessionId,
  message, severity, sender, workflowId, nodeId, agentName}` only (kas-workflow-gateoff-2.32.0.jsonl#145"callerSessionId"). The same id is what cyril already
  strips from `steering_cleared` as a `notify-` prefix (`crates/cyril-core/src/protocol/convert/kas.rs` doc, cyril-lki9 P6) — now it is also on the notify frame.
- **`_meta.kiro.preview` on write tool calls is GONE in 0.66.91**: OLD `tool_call`/`tool_call_update` for `fs_write` carried
  `preview{file, modifiedContent, originalContent, local, original, modified}` (kas-workflow-gateoff-2.32.0.jsonl#108"modifiedContent",
  kas-workflow-gateoff-2.32.0.jsonl#126"originalContent"); NEW carries only `checkpoint{original, modified, local}` at completion
  (kas-workflow-gateoff-3.1.0.jsonl#129"checkpoint") and the standard ACP `content[]` diff block, nothing on the pending `tool_call`
  (kas-workflow-gateoff-3.1.0.jsonl#111"toolOrigin"). Static: `modifiedContent` 16 → 13 only at 0.66.91 → **3.1.0**. cyril: never read `preview`
  (0 production hits; the approval overlay derives the diff from ACP `content[].diff` via `crates/cyril-core/src/protocol/convert/mod.rs:207-210` and
  `TrackedToolCall::preview_body` `crates/cyril-ui/src/traits.rs:539`) → NOT a cyril regression; a client that rendered the pending-write preview from
  `_meta.kiro.preview.modifiedContent` (the 2.32.0 TUI did) now has no pre-approval content until the `content[]` diff arrives with the completed update.
- Turn-lifecycle identity fields (`executionId`, `turnId`, `boundarySource`, `userMessageTag` on the parent's wake `turn_start`
  kas-workflow-gateoff-3.1.0.jsonl#1034"userMessageTag") and `session_usage` (16 per capture) appear on every step session as in leg 1; static:
  `boundarySource` 0 → 15 at 0.66.58 → **2.33.1**. `rawInput.sessionId` on a tool call = model-chosen arg (coverage).
- sweep-new-fields (`sweep-new-fields-diff-wfoff.txt`): 1197 vs 1155 paths; 50 only-NEW = the families above; 8 only-OLD = the removed
  `preview.*` family (the one REAL removal of the audit so far) + `documents[].content` (coverage).
- steering_injected 10 vs 11 and steering_cleared 5 vs 6 (one fewer engine injection on NEW) — count noise from the pause-wake timing, not a shape change.

### Forwarded leads received (orchestrator, during leg 2b ~21:45 UTC) — kas-workflow lane WF-L1…L10
Plan: WF-L1/L6/L7 ride the leg 2a gate-off pair (already captured; analysed below in 2b); WF-L3 = my `wfplan31` arm (+ `appliedPlanFeedback`
inspection and the step session's `<approved_plan_feedback>` opening message); WF-L2 (workflowLaunch option, gate on + cross-engine
session/load), WF-L4 (wake race, 3× per arm), WF-L5 (jsonPath grammar), WF-L8 (peer notify via `settings.sessionTools.messaging.enabled`),
WF-L9 (recipe in an untrusted workspace; pairs with LIVE-A) are new arms in `probe-kas-leads-3.1.0.py`; WF-L10 only if budget remains.
Leg-5 S3 extra: with `chat.enableWorkflows=true` in the temp HOME's cli.json, record whether `workflowLaunch` shows anywhere in cyril.

### Forwarded leads received (orchestrator, during leg 2b ~21:48 UTC) — tui lane TUI-L1…L13 + two corrections
- Corrections accepted: LIVE-A runs as a CONFIRMATION leg (static: trust prompt is TUI-only, KAS hardcodes `workspaceTrusted:!0`; a trust frame
  would REFUTE); LIVE-B: `_kiro/mcp/elicitation` is an unconditional host callback present on both engines; the TUI advertises
  `_meta.kiro.mcpElicitation:true` (my `capflag` arm) and cyril declines (`crates/cyril-core/src/protocol/kas/mcp.rs`).
- Plan: TUI-L1 → `recap` arm (on/off/absent/_sessionRecap); TUI-L2 → DONE in leg 1 (session_usage both directions, replay on load; OLD 0 hits);
  TUI-L3 = WF-L1 (leg 2a); TUI-L4/L9/L10/L12 → `zeroturn` arm (plain + diagrams); TUI-L5 → `policyrule` arm (rule + `KIRO_TRUST_CLASSIFIER_MODE=enforce`);
  TUI-L6 = LIVE-B; TUI-L7 = LIVE-F; TUI-L8 = WF-L2; TUI-L11 → `tier` arm matrix (tier/star/exact × session/user); TUI-L13 NOT-REACHED (no
  OAuth-capable remote MCP fixture; static KAS 0→2).

### Forwarded leads received (orchestrator, during leg 3 batch 1 ~21:53 UTC) — kas-core lane KAS-L1…L13 + LIVE-C additions
- KAS-L2 = TUI-L2: DONE in leg 1; the `session_usage.credits[].usage` IS the running sum of the leg's `promptTurnSummaries` (0.1319 → +0.1052 =
  0.2371 → +0.1090 = 0.3460 → +0 (failed turn) = 0.3460 → +0.1130 = 0.4590: kas-live-sweep-3.1.0.jsonl#92"0.23706273830845773",
  kas-live-sweep-3.1.0.jsonl#131"0.34603577810945274", kas-live-sweep-3.1.0.jsonl#246"0.34603577810945274", kas-live-sweep-3.1.0.jsonl#282"0.4589933797678275").
- KAS-L11 = leg 0 (+ `zeroturn` diagrams arm for `diagramRenderFormats`). KAS-L3 = `signin` arm (+ post-failure `session/load` replay check).
  KAS-L4 = `policyrule` arms (rule / enforce / evalonly). KAS-L7 = `hooks31` arms (agentfile / preblock / hookscap; object-form and array-form
  agent hooks). KAS-L8 = `hooks31 hookscap` (client exit-2 with empty output). New arms: `fallbackflags` (L1), `memory` (L5: on/optout/plain),
  `turntiming` (L9), `shellenv` (L12), `displaytext` (L13), `denyrule` (LIVE-C deny-rule status). KAS-L6 NOT-REACHED (no OAuth fixture).
  KAS-L10 folded into `zeroturn` (`_kiro/configuration/set` krn/layer matrix, never memorySync). Passive: every capture is scanned for
  `model_fallback`/`model_routed` kinds and `_meta.kiro.modelFallback`.

### Leg 3 / LIVE-A (+WF-L9) — workspace trust on bare ACP (DONE 21:51-21:53 UTC) — CONFIRMED: no trust surface on KAS, everything loads
- `probe-kas-leads-3.1.0.py trust` ARM=bare on NEW (`kas-lead-trust-3.1.0.jsonl`, 234 frames, 45.5 s) and OLD (`kas-lead-trust-2.32.0.jsonl`, 227 frames,
  45.6 s): a workspace seeded with `.kiro/agents/wsagent.json`, `.kiro/settings/mcp.json` (fixture `wsprobe`), `.kiro/workflows/wsrecipe.workflow.json`,
  `.kiro/hooks/*.kiro.hook`, `.kiro/steering/ws-steer.md`, and the same shapes under the temp `$HOME/.kiro/` (`useragent`, `userprobe`, `userrecipe`);
  identical handshakes (`configurationState:true`, no hooks cap). 3 billed turns + 1 workflow run per engine; identical results on both.
- **No trust-related frame of any kind** from either engine: the only client-facing request in the whole leg is the MCP tool approval
  (kas-lead-trust-3.1.0.jsonl#69"mcp_wsprobe_probe_echo"); no `_kiro/*` trust method, no `_kiro/userInput`, no `session/request_permission` about the folder,
  no `trust` key in session/new `_meta` or in any notification (every "trust" substring in the capture is the lane's own temp-dir name). kiro.log has no
  trust decision line. This CONFIRMS the static claim (TUI-only gate; KAS `workspaceTrusted:!0`).
- **Workspace tier loads without any trust answer, on both engines**: `wsagent` is in session/new `modes.availableModes` and `available_commands_update`
  (kas-lead-trust-3.1.0.jsonl#26"wsagent"; OLD kas-lead-trust-2.32.0.jsonl#21"wsagent"); `wsprobe` is in `_kiro/mcp/status` with `source.origin:"workspace"`
  (kas-lead-trust-3.1.0.jsonl#15"wsprobe") and its tool ran after an on-demand "Tool Load" (kas-lead-trust-3.1.0.jsonl#64"Tool Load", reply `ECHO:TRUST`
  kas-lead-trust-3.1.0.jsonl#77"ECHO:TRUST"; OLD kas-lead-trust-2.32.0.jsonl#76"ECHO:TRUST"); the workspace steering file is applied (every reply ends `WSSTEER`,
  kas-lead-trust-3.1.0.jsonl#131"WSSTEER"); `wsrecipe` is listed (kas-lead-trust-3.1.0.jsonl#39"wsrecipe.workflow.json") and **WF-L9**: `_kiro/workflow/new` on
  the workspace recipe is accepted and the run completes (kas-lead-trust-3.1.0.jsonl#188"completed"; OLD kas-lead-trust-2.32.0.jsonl#183"completed") — no
  "not available in this session's configuration" text and no trust refusal on either engine. Switching mode to `wsagent`/`useragent` works on both.
- Hooks: `_kiro/hooks/list` (client→agent) answers -32603 "`_kiro/hooks/list is not available when v2Hooks is disabled`" on BOTH engines when the client does
  not advertise `hooks:{enabled:true}` (kas-lead-trust-3.1.0.jsonl#35"v2Hooks is disabled"; OLD kas-lead-trust-2.32.0.jsonl#32"v2Hooks is disabled"), and the
  agent-file `userPromptSubmit` hooks did NOT run on either engine in this bare arm (hook log file never written) — the hooks model is exercised in LIVE-D.
- cyril impact: NONE from the trust feature — a cyril user upgrading to 3.1.0 keeps workspace agents/MCP/recipes/steering on the KAS path with no new prompt.
  (The P1 candidate in the brief is REFUTED for the ACP path; it only concerns the Kiro TUI.)

### Leg 2b — SCENARIO=workflow GATE=on pair (DONE 21:36-21:49 UTC)
- OLD `kas-workflow-gateon-2.32.0.jsonl` (2522 frames, 329.9 s), NEW `kas-workflow-gateon-3.1.0.jsonl` (2514 frames, 443.6 s). Both complete with the
  driver summary, `workflowsEnabled:true` on both (gate positive control), 12 vs 13 permission asks, `recipes_changed` 2 = 2
  (kas-workflow-gateon-3.1.0.jsonl#60"recipes_changed", kas-workflow-gateon-2.32.0.jsonl#57"recipes_changed"; same key set). NEW's +114 s is the lane's
  **single in-driver token renewal** at 425 s (kas-workflow-gateon-3.1.0.jsonl#2234"auth_renew": rc 0, 3599 s left afterwards) during the `reload`
  connection's init — the 90 s post-expiry grace + `kiro-cli user whoami`; no login/logout. Run parity (`wf_parity.py`): all 11 runs MATCH event-for-event
  (agenturi 5, bgwatch 10, branches 13, fail 5 + retry-node, launcher completed, linear 13, loop 23, loopnone 12, park 10, usertier 5, watch 11); every
  terminal run auto-woke the parent on both; the repeat-cap pause wake is unchanged.
- **WF-L2 / TUI-L8 CONFIRMED — `workflowLaunch` session config option (first in 2.33.1 per static)**: with `settings.workflows.enabled:true` NEW's
  session/new result configOptions = [mode, model, memoryReflection, autopilot, **workflowLaunch**, contentCollection] with
  `{type:"select", id:"workflowLaunch", name:"Workflow use", currentValue:"when-asked", options:[when-asked "When asked", large-tasks "Large tasks only",
  delegate-all "Delegate all work"]}` (kas-workflow-gateon-3.1.0.jsonl#21"Delegate all work"; the same option also arrives in the `config_option_update`
  pushed before the result). OLD's gate-on session has no such option (kas-workflow-gateon-2.32.0.jsonl#10"contentCollection"). Gate-off sessions on NEW
  have none either (leg 1/2a) — the option exists only when workflows are enabled. The `set_config_option` round trip, the gate-off control and the
  cross-engine `session/load` are in the `wflaunch` lead arm (batch 2). cyril: `/mode`/`/model`/`/effort` pickers model only model/mode/effortLevel
  (`crates/cyril-core/src/types/kas_config.rs:20-48` per the tui lane) → a NEW select option cyril never lists (leg 5 S3 checks the user's view).
- **WF-L1 / TUI-L3 step credits (gate on)**: as in 2a — `credits[]` on every completed `node_complete`, summed on `run_complete`
  (launcher run: 0.334 credits, kas-workflow-gateon-3.1.0.jsonl#2146"0.33413843499170814"); a FAILED step's `node_complete` carries `failureReason` and NO
  `credits` key (kas-workflow-gateon-3.1.0.jsonl#1658"failureReason"). WF-L1 cross-check on the gate-off linear run: node_complete.credits (s1 0.18996,
  s2 0.24822; kas-workflow-gateoff-3.1.0.jsonl#193"0.18995728971807627") EQUAL the step sessions' own `session_usage` totals (kas-workflow-gateoff-3.1.0.jsonl#190"0.18995728971807627")
  and run_complete.credits = their sum 0.43818 (kas-workflow-gateoff-3.1.0.jsonl#291"0.43817720348258704"); snapshot nodes carry the same values.
- **WF-L6 CONFIRMED**: `stepsCompleted/stepsTotal/currentNodeId/waitingOnWatch` are on `_kiro/workflow/list` rows WHILE a run is paused/running
  (kas-workflow-gateoff-3.1.0.jsonl#99"stepsCompleted") and ABSENT from the rows of completed runs in the post-run list
  (kas-workflow-gateoff-3.1.0.jsonl#1152"completed", no progress keys) — progress is live-state only.
- **WF-L7 (2.33.0 "replies Kiro starts on its own no longer cut off")**: measured the streamed text of every parent `workflow-complete-wake` turn
  (chunks tagged `agentInitiatedReason`): gate-off OLD 10 turns, lengths 37…3919 chars (shortest kas-workflow-gateoff-2.32.0.jsonl#1128"review the workflow results");
  NEW 9 turns, 80…3997; gate-on OLD 23 turns 55…9790, NEW 23 turns 45…7835. The short replies are complete one-sentence replays on both engines, not
  truncations — no cut-off visible on OLD in this sample, so the fix is NOT-REACHED live (the symptom did not reproduce on 0.66.43 today).
- No plan-approval frame in either gate-on run (the launcher uses `run_workflow` with a `workflowPath`; `workflowPlan`/`steps_queued` 0 hits on both) —
  the plan flow is exercised by the `wfplan31` lead arm (WF-L3).
- sweep-new-fields (`sweep-new-fields-diff-wfon.txt`): 1394 vs 1345; only-NEW = the 2a families + `notifyId`, `userMessageTag`, the renewal record;
  only-OLD = the removed `preview.*` family again (kas-workflow-gateon-2.32.0.jsonl has 6 preview frames, NEW 0).

### Forwarded leads received (orchestrator, after batch 1 ~21:58 UTC) — host lane HOST-L1…L6
- Plan: HOST-L1 (engine routing of `kiro-cli acp` for an unlisted client, arms a–g + 2.36.0/2.35.2 launcher pins), HOST-L3 (terminal auth method via
  `clientCapabilities._meta["terminal-auth"]`), HOST-L5 (`--agent-engine v1`) and HOST-L2 (extraction layout) are auth-free fresh-store legs → run as
  `probe-kas-host31-3.1.0.py engine|extract` BEFORE batch 2 (archived bin first on PATH, child `/proc/<pid>/exe` recorded, own scratch dirs per arm).
  HOST-L4 = LIVE-F (+ `expiresAt` == store `expires_at` check). HOST-L6 (cyril `--agent-engine v2` against the archived 3.1.0 `kiro-cli acp`) in leg 5
  against a COPY of the store, no prompt — or NOT-RUN with reason.

### Leg 3 / LIVE-B (= TUI-L6) — MCP elicitation + `structuredContent` (DONE 21:52-21:56 UTC)
- `probe-kas-leads-3.1.0.py elicit` ARM=plain on NEW (`kas-lead-elicit-3.1.0.jsonl`, 271 frames, 124.9 s) and OLD (`kas-lead-elicit-2.32.0.jsonl`, 250 frames,
  124.6 s): the extended stdio fixture (`live/fixtures/mcp_probe_server.py`: `probe_elicit` form, `probe_elicit_url`, `probe_structured`) seeded via
  `$HOME/.kiro/settings/mcp.json`; handshake WITHOUT any elicitation flag. 5 billed turns per engine.
- **Elicitation CONFIRMED on BOTH engines, identical shape (the tui correction holds; no client flag needed)**: KAS forwards the server's
  `elicitation/create` as the host callback `_kiro/mcp/elicitation {sessionId, toolCallId, elicitation:{mode:"form", message, requestedSchema}}`
  (kas-lead-elicit-3.1.0.jsonl#130"requestedSchema"; OLD kas-lead-elicit-2.32.0.jsonl#114"requestedSchema") and `{mode:"url", message, url, elicitationId}`
  for the link mode (kas-lead-elicit-3.1.0.jsonl#248"probe-elicit-link"; OLD kas-lead-elicit-2.32.0.jsonl#229"probe-elicit-link"). While it waits, the tool call
  is updated to `status:"pending"` with the whole elicitation mirrored in `_meta.kiro.elicitation` (kas-lead-elicit-3.1.0.jsonl#129"elicitation";
  OLD kas-lead-elicit-2.32.0.jsonl#113"elicitation") — the frame a client that cannot render forms still sees. Client answers round-trip verbatim to the
  MCP server: `{action:"accept", content:{colour:"blue"}}` → tool text `ELICIT_RESULT:{"action": "accept", "content": {"colour": "blue"}}`
  (kas-lead-elicit-3.1.0.jsonl#137"blue"); `{action:"decline"}` → `…"decline"` (kas-lead-elicit-3.1.0.jsonl#176"decline"); an UNANSWERED elicitation keeps the
  tool call pending with no timeout for the 75 s the probe held it (turn 82 s), then the late `{action:"cancel"}` completes it
  (kas-lead-elicit-3.1.0.jsonl#216"cancel"). kiro.log `[ACP Elicitation] Requesting elicitation … mode=form` on both. MCP client caps sent to the server:
  `elicitation:{form:{}, url:{}}`, protocolVersion 2025-11-25 (fixture log). cyril: handles `_kiro/mcp/elicitation` with a visible decline
  (`crates/cyril-core/src/protocol/kas/mcp.rs`) → the declining path above is exactly cyril's; a stuck turn is NOT a risk (KAS waits only for the
  client's reply, which cyril sends at once). Leg 5 S6 shows the user's view.
- **3.1.0 `structuredContent` "extra JSON block": NOT REPRODUCED on either engine.** `probe_structured` returned `content:[{type:"text", text:"The square
  of 7 is 49."}]` + `structuredContent:{n:7, square:49, label:"square-of-7"}` (declared `outputSchema`); the ACP `tool_call_update` carries ONLY the text
  block and `rawOutput {response, imageBase64Urls, message}` on NEW (kas-lead-elicit-3.1.0.jsonl#59"The square of 7 is 49") and OLD
  (kas-lead-elicit-2.32.0.jsonl#56"The square of 7 is 49"), and the model on both engines reports it never received the `label` — so the structured data did not
  reach the model-facing result either. Either the "mirrors it in a text block" heuristic fired (the text names the same number) or the change is
  host-side rendering; lead for the kas-core lane, verdict on the wire: no new content block. cyril: nothing to consume (NOT-SENT).

### Leg 3 / LIVE-C (i) — unparseable shell command (2.34.0 auto-deny) (DONE 21:56-21:57 UTC) — CONFIRMED, NEW asks nothing
- `probe-kas-leads-3.1.0.py unparse` on NEW (`kas-lead-unparse-3.1.0.jsonl`, 120 frames, 19.7 s) and OLD (`kas-lead-unparse-2.32.0.jsonl`, 123 frames, 18.8 s);
  the model ran `echo "unbalanced AUDITQ  ` (unterminated quote) verbatim on both; control `echo balanced AUDITQ` asked once and ran on both.
- OLD: `session/request_permission` with only TWO options (`accept`/`reject`, no always-*) and `_meta.kiro.consent{persistableConsent:false,
  persistableConsentReason:"Kiro could not parse this command, so a saved rule would never match it…"}` (kas-lead-unparse-2.32.0.jsonl#37"persistableConsentReason");
  after allow the command ran under bash and failed (exit 2, kas-lead-unparse-2.32.0.jsonl#55"unexpected EOF").
- NEW: NO permission request (0 `session/request_permission` in the turn); the `tool_call` goes straight to `tool_call_update status:"failed"` whose
  content/rawOutput text is `Tool call denied: Kiro could not parse this shell command to check it against the user's permissions. It was not run.
  Parse error at position 5 … The same command will be denied again, even if it is valid shell. Rewrite the command…` with `Exit Code: -1`
  (kas-lead-unparse-3.1.0.jsonl#39"Tool call denied"); no `_meta.kiro.policyDenial` and no `display_error` — the denial is only tool-result text.
  cyril: the failed update renders as an ordinary failed tool call (HANDLED generically via `tool_call_update` status; nothing to add); the 0.66.43
  2-option ask with `persistableConsent:false` (cyril-k8wc territory) no longer exists for this case.

### Leg 3 / HOST-L1, L2, L3, L5 + LIVE-E (ii) — archived 3.1.0 `kiro-cli` with EMPTY stores (DONE 22:00-22:01 UTC; zero auth risk)
- `probe-kas-host31-3.1.0.py engine` (`host-engine-3.1.0.jsonl`; archived `kiro-cli` sha cc780572…, 220,246,888 B, bin first on PATH, fresh HOME/XDG_DATA_HOME/
  XDG_CONFIG_HOME/KIRO_RUNTIME_DIR per arm; `initialize` + `session/new`), arms a–m; `host-engine-{2.36.0,2.35.2}.jsonl` arms a,b; `host-extract-3.1.0.jsonl`.
- **LIVE-E (ii) / HOST-L3 CONFIRMED — 3.1.0 answers ACP while logged out and advertises a sign-in method**: with an empty store every arm (no flag,
  `--agent-engine v2|v3|v1`, `acp.agentEngine`/`chat.agentEngine` settings, client names cyril/acp-probe/JetBrains.Foo) gets an `initialize` result from
  the HOST itself in 0.5 s: `agentInfo {name:"Kiro CLI Agent", version:"3.1.0"}`, `agentCapabilities {loadSession:false, promptCapabilities all false,
  mcpCapabilities all false, sessionCapabilities:{}, auth:{}}`, NO `_meta.kiro`, and `authMethods:[{id:"kiro-login", name:"Kiro Login",
  description:"Run 'kiro-cli login' in a terminal, then reconnect to this agent. See https://kiro.dev/docs/cli/authentication/"}]`
  (host-engine-3.1.0.jsonl#1"kiro-login"). With `clientCapabilities._meta:{"terminal-auth":true}` the method becomes
  `{type:"terminal", id:"kiro-login-terminal", name:"Log in to Kiro", description:"Runs `kiro-cli login` in a terminal", args:["--login"]}`
  (host-engine-3.1.0.jsonl#8"kiro-login-terminal"; same for `--agent-engine v2` host-engine-3.1.0.jsonl#9"kiro-login-terminal"). `session/new` is refused
  with -32000 "Authentication required. Not logged in. Set the KIRO_API_KEY environment variable or run `kiro-cli login` first. Then reconnect to start the
  Kiro agent." (host-engine-3.1.0.jsonl#1"Authentication required"); stderr has one line `Not logged in. Set the KIRO_API_KEY…` and no `[INFO]` agent lines
  (no agent was spawned: no node child in any arm). `--agent-engine v1` is accepted and behaves identically (host-engine-3.1.0.jsonl#10"kiro-login") — no
  panic (HOST-L5). The `--login` command was NOT run.
- Pinning: the archived **2.36.0 and 2.35.2 launchers exit rc 1 before answering `initialize`** (`error: You are not logged in, please log in with kiro-cli
  login`, host-engine-2.36.0.jsonl#1"not logged in", host-engine-2.35.2.jsonl#1"not logged in") → the answer-while-logged-out behaviour is the 2.37.0 line
  "ACP clients can now sign in from a terminal when `kiro-cli acp` starts logged out", consistent with the host lane's static read.
- **HOST-L1 (engine routing for an unlisted client) is NOT answerable from a logged-out store**: the sign-in gate sits in front of engine resolution (no
  node spawn, identical results for every flag/setting/client name). It is re-run with a TOKEN inside the LIVE-F copy procedure below (arms a and b
  against the COPY of the store, initialize only + session/new, no prompt).
- **HOST-L2 extraction layout (3.1.0)**: `kiro-cli _ extract-runtime` (hidden helper; `kiro-cli _ --help` lists `cleanup-autocomplete-artifacts,
  standalone-cohort, standalone-cutover, extract-runtime`, host-extract-3.1.0.jsonl#3"extract-runtime") installs into
  `$KIRO_RUNTIME_DIR/3.1.0-a53631e2096ad811/{node, bun, tui.js, .complete, kas/node_modules/@kiro/agent/…}` (host-extract-3.1.0.jsonl#1"3.1.0-a53631e2096ad811",
  tree host-extract-3.1.0.jsonl#4"/3.1.0-a53631e2096ad811/kas/node_modules/@kiro/agent/package.json"); `--dir <d>` puts the same `3.1.0-<hash>/` under `<d>`
  (host-extract-3.1.0.jsonl#2"rt2/3.1.0-a53631e2096ad811"). The data dir of a 3.1.0 first run holds `data.sqlite3` (tables auth_kv, conversations,
  conversations_v2, extracted_kas_versions, history, migrations, pinned_bin_versions, state — 8 migrations applied), `run-receipts/`,
  `telemetry-export-drops.lock` and NO `kas/` (host-engine-3.1.0.jsonl#1"telemetry-export-drops.lock"). So the KAS bundle now lives under
  `<runtime-dir>/<host-ver>-<hash>/kas/…` (one directory per host version, siblings removed), not `XDG_DATA_HOME/kiro-cli/kas/<kas-ver>-<sha>/`. Where the
  default runtime dir is WITHOUT `KIRO_RUNTIME_DIR` ("instead of the home directory" per 2.37.0) is measured in the LIVE-F chain (fresh HOME, no auth).
  cyril impact: see the discovery note in the Deltas table (the 3.1.0 self-extracted tree is a different path shape from the 2.32.0 one cyril's
  `discovery.rs` was written against).

### Leg 3 / LIVE-E (i) = KAS-L3 — sign-in refusal when the client cannot provide a token (DONE 22:01-22:03 UTC) — CONFIRMED (NEW only, error-reply arm)
- `probe-kas-leads-3.1.0.py signin` on NEW (`kas-lead-signin-3.1.0.jsonl`, 230 frames, 42.7 s; arms error / empty / nulltoken) and OLD
  (`kas-lead-signin-2.32.0.jsonl`, 156 frames, 28.2 s; arms error / empty). `--auth=acp-callback`; the client answers `_kiro/auth/getAccessToken` with a
  JSON-RPC error (`error` arm), `{}` (`empty`) or `{accessToken:null, expiresAt:null}` (`nulltoken`); KAS retried the callback 9–11× per arm on both.
  initialize and session/new succeed on every arm and engine (the sign-in state is only checked at prompt time).
- **NEW, `error` arm**: `session/prompt` → `-32000 "Sign in to Kiro to send a prompt." data {errorType:"SignInRequiredError", retryErrorType:"CLIENT_ERROR"}`
  in 4 s (kas-lead-signin-3.1.0.jsonl#36"SignInRequiredError"), NO `turn_start`/`user_message_id_assigned` frames, and NOTHING persisted: a second connection's
  `session/load` replays no `user_message_chunk` (kas-lead-signin-3.1.0.jsonl#229"reload_user_chunks"). kiro.log: `auth.refresh.host_callback_failed`,
  `auth.identity.unreadable {trigger:"initialize", error:"CredentialAbsentError"}`. This is the 3.1.0 line "A prompt is refused with a sign-in message,
  and not saved, when Kiro cannot get a sign-in token".
- **NEW, `empty` and `nulltoken` arms**: the OLD behaviour persists — `-32000 "Kiro could not load the available models because you are not signed in.
  Please sign in and retry." data {errorType:"ModelRegistryUnauthenticatedError", detail:"TokenInvalidError"}` (kas-lead-signin-3.1.0.jsonl#112"ModelRegistryUnauthenticatedError"),
  `user_message_id_assigned` IS emitted and the prompt IS saved (replayed on load: kas-lead-signin-3.1.0.jsonl#229"Reply with exactly SIGNEDIN"). So the new
  refusal keys on the callback FAILING (error/`CredentialAbsentError`), not on a malformed token.
- **OLD (0.66.43)**: every arm → `ModelRegistryUnauthenticatedError` (`TokenExpiredError` detail for the error arm, kas-lead-signin-2.32.0.jsonl#40"TokenExpiredError");
  `SignInRequiredError` 0 hits; prompt saved in every arm.
- cyril: cyril's auth responder returns a JSON-RPC error when `get-kas-token` fails (`crates/cyril-core/src/protocol/kas/auth.rs`), i.e. the NEW `error`-arm
  path — the user will see the "Sign in to Kiro to send a prompt." error instead of the model-registry text; the leg-5 cyril-shaped arm (cyril-0rn8) checks
  that the busy state clears. HANDLED as a generic prompt error (`-32000` → `Notification::TurnCompleted` error path); the new `errorType` string is only
  surfaced as text.

### Leg 3 / KAS-L4 = TUI-L5 — "Approve for me" (2.36.0 trust classifier) + permissions list + `_kiro/policy/changed` (DONE 22:03-22:05 UTC) — CONFIRMED on NEW
- `probe-kas-leads-3.1.0.py policyrule` NEW arms rule / enforce / evalonly (`kas-lead-policyrule-3.1.0.jsonl`, 322 frames, 64.1 s), OLD arm rule
  (`kas-lead-policyrule-2.32.0.jsonl`, 180 frames, 33.1 s). Rule = the TUI's `{capability: all, effect: evaluate}` in `$HOME/.kiro/settings/permissions.yaml`.
- **The wire shape (NEW `enforce` arm: yaml pre-seeded + env `KIRO_TRUST_CLASSIFIER_MODE=enforce`)**: `git stash list` ran with NO `session/request_permission`
  at all; the `tool_call_update` frames carry `_meta.kiro.trustClassifierApproved:true` (kas-lead-policyrule-3.1.0.jsonl#157"trustClassifierApproved") through
  to completion (kas-lead-policyrule-3.1.0.jsonl#168"completed"); `ls -la` likewise (0 asks). kiro.log shows a SECOND, separately billed model call per tool:
  `q.converse.dispatch … modelId:"permission-classifier"`, `llm.invokeTrustClassifier.ok durationMs:931`, `acp.policy-eval.classifier-allow`
  (kas-lead-policyrule-3.1.0.jsonl#321"permission-classifier", kas-lead-policyrule-3.1.0.jsonl#321"classifier-allow"). `_kiro/permissions/list` shows the rule with
  `source:<user permissions.yaml path>, scope:"user", effect:"evaluate"` (kas-lead-policyrule-3.1.0.jsonl#143"evaluate"). No `trustClassifier{decision:"prompt",reason}`
  frame was produced (both commands were classified allow); that branch is NOT-REACHED.
- **Gate**: the same yaml WITHOUT the env (`evalonly` arm) → the rule is skipped and the ordinary 4-option ask returns
  (kas-lead-policyrule-3.1.0.jsonl#252"session/request_permission"); kiro.log `[PolicyParser] Skipping rule 0 … effect "evaluate" is not s[upported]`. So on a direct
  spawn the classifier is enabled by the HOST through `KIRO_TRUST_CLASSIFIER_MODE=enforce`, not by the yaml alone; a cyril user can only get "Approve for me" if
  cyril sets that env on the KAS child (lead for the host lane: does `kiro-cli acp` set it when `/permissions` enables the feature?).
- **`_kiro/policy/changed` grew `errors[]` (NEW)**: writing the yaml mid-session → NEW `{sessionId, status:"success", errors:[{scope:"user", source:<path>,
  message:"Skipping rule 0 … effect \"evaluate\" is not s…"}]}` (kas-lead-policyrule-3.1.0.jsonl#28"Skipping rule 0") and NO `_kiro/policy/error`; OLD →
  `{status:"failed", errors:[{…"Invalid effect \"evaluate\"…", fatal:true}]}` (kas-lead-policyrule-2.32.0.jsonl#25"failed") PLUS `_kiro/policy/error`
  (kas-lead-policyrule-2.32.0.jsonl#26"_kiro/policy/error") and then **every tool call is denied** ("Tool call denied: the user's permission file(s) failed to parse",
  kas-lead-policyrule-2.32.0.jsonl#43"failed to parse"; `permissions/explain` → `effect:"deny"` kas-lead-policyrule-2.32.0.jsonl#31"deny"). Cross-version hazard: a
  permissions.yaml that holds an Approve-for-me rule locks out ALL tools on a 0.66.43 host (the user's installed 2.32.0) — relevant to anyone sharing a HOME
  between versions. cyril: `_kiro/policy/changed` is not named anywhere in `crates/` (unread-fields leg 1) → DROPPED; the `trustClassifierApproved` meta is DROPPED.
- `_kiro/permissions/list` rule shape unchanged OLD→NEW: `{capability, effect, scope, source, match?, exclude?}`; the baseline list has `kiro-scope` fs_write
  deny rules for `~/.kiro/settings/`, `workspace-roots/`, `sandbox-state/` (many path spellings) + the `agent-profile` rules on both engines.

### Leg 3 / LIVE-C (ii) = TUI-L11 — subcommand tier via the consent echo + always-allow persistence on 0.66.91 (DONE 22:07-22:11 UTC)
- `probe-kas-leads-3.1.0.py tier` NEW arms tier:session / star:session / tier:user / exact:user (`kas-lead-tier-3.1.0.jsonl`, 906 frames, 169.7 s), OLD arms
  tier:session / exact:user (`kas-lead-tier-2.32.0.jsonl`, 443 frames, 85.9 s). Each arm: `git stash list` → `always-accept` + `_meta.kiro.consent
  {capability:"shell", scope, resource, workspaceRoot}` (the TUI's shape), then `git stash list` again, `git stash show` (sibling subcommand),
  `git status --short`, then a NEW session + `git stash list`.
- **The tier is a client-composed glob; the engine semantics are identical on OLD and NEW**: `resource:"git stash *"` at `scope:"session"` → the sibling
  `git stash show` runs WITHOUT a new ask (kas-lead-tier-3.1.0.jsonl#111"terminal/create" with no request in that turn; OLD kas-lead-tier-2.32.0.jsonl#106"terminal/create"),
  and a new session re-asks (session scope ends with the session: kas-lead-tier-3.1.0.jsonl#192"session/request_permission"; OLD kas-lead-tier-2.32.0.jsonl#185"session/request_permission").
  `resource:"git *"` behaves the same (kas-lead-tier-3.1.0.jsonl#340"terminal/create"). The exact resource `git stash list` at `scope:"user"` persists to
  `$HOME/.kiro/settings/permissions.yaml` as `rules: - capability: shell / effect: allow / match: [git stash list]` on both engines
  (kas-lead-tier-3.1.0.jsonl#905"git stash list", kas-lead-tier-2.32.0.jsonl#442"git stash list"), and the sibling `git stash show` IS re-asked
  (kas-lead-tier-3.1.0.jsonl#786"session/request_permission"; OLD kas-lead-tier-2.32.0.jsonl#326"session/request_permission"). `git stash *` at `scope:"user"` persists as
  `match: [git stash *]` and a fresh session runs `git stash list` with no ask (0 requests after t1; `_kiro/permissions/explain` for `git stash show` →
  `effect:"allow", matchedRule {match:["git stash *"]}, scope:"user", source:<permissions.yaml>` kas-lead-tier-3.1.0.jsonl#905"matchedRule").
  `git status --short` never asked in any arm — it is on Kiro's built-in read-only allowlist, not a control.
- Verdict: 2.36.0 "shell approval rules offer a subcommand tier" = the TUI offering `git stash *` as one of its rule-scope choices; the wire accepts any
  glob in `consent.resource` on both 0.66.43 and 0.66.91. The request frame itself is unchanged (leg 1). cyril (cyril-auxt closed): cyril can offer the
  same tier by composing `<argv0> <argv1> *` itself — nothing new is needed from KAS; `permissions.yaml` shape unchanged (`rules[] {capability, effect, match[]}`).

### Leg 3 / KAS-L5 — memory rollout (DONE 22:11-22:14 UTC)
- `probe-kas-leads-3.1.0.py memory` NEW arms on / optout / plain (`kas-lead-memory-3.1.0.jsonl`, 290 frames, 107.5 s), OLD arm on (`kas-lead-memory-2.32.0.jsonl`,
  78 frames, 36.7 s). `on` = `initialize`+`session/new` `_meta.kiro.settings.memory:{mode:"read_write", reflection:true}`; `optout` adds `userMemoryOptIn:false`;
  `plain` = no memory settings. Two billed turns each ("remember … codename OTTER", "what is my codename?").
- **Default on a bare spawn is still OFF on NEW**: `plain` → session/new `_meta.memoryConfig {mode:"disabled", reflection:false}`, no `memoryReflection`
  option toggled, kiro.log `memory.placement.decided {reason:"memoryOff"}` — the 0.66.58 flag flip (memory_internal_enabled → "all") did not change the
  ACP default; the model worked around it by writing a steering file (kas-lead-memory-3.1.0.jsonl#289"user-preferences.md").
- **With `settings.memory read_write` both engines store a memory through the `memory` tool** (`{command:"add", memoryType:"USER_PREFERENCE", title, …}`;
  OLD kas-lead-memory-2.32.0.jsonl#77"favourite-color-project"), and `_kiro/memory/list` returns it. NEW additions: the notification
  `_kiro/memory/changed {total:1, counts:{byType:{USER_PREFERENCE:1}, byScope:{Global:1}}}` fires on the write (kas-lead-memory-3.1.0.jsonl#45"_kiro/memory/changed");
  `_kiro/memory/list` carries `total`/`counts` (kas-lead-memory-3.1.0.jsonl#289"byScope"); `_kiro/memory/getSyncStatus` → `{state:"off"}` (OLD: -32603 no
  persistence classification). `_kiro/memory/create` REQUIRES `memoryType, scopes, title, summary` (-32603 "create requires memoryType, scopes, title, summary."
  on both, kas-lead-memory-3.1.0.jsonl#289"create requires memoryType") — the covenant's bare `{content}` shape is wrong.
- **`settings.userMemoryOptIn:false` (NEW)**: session/new still echoes `memoryConfig read_write`, but `_kiro/memory/list` → -32602 "Invalid params: Memory is
  not enabled." (kas-lead-memory-3.1.0.jsonl#109"Memory is not enabled"), kiro.log `memory.gate.resolved {decision:"disabledByOptOut"}`, the model reports memory
  disabled, no `memory/changed`. So `userMemoryOptIn` is a hard off-switch above the per-session mode.
- Hidden extraction: `memory.extraction.skipped {nonForcedCount:5, newMessages:5, required:10}` on both engines — the per-turn extraction call needs ≥10
  messages, so no hidden billed extraction appeared in these 2-turn sessions (`promptTurnSummaries` = the user turns only). Memory files:
  `$HOME/.kiro/memories/memories.db` on both.
- cyril: `_kiro/memory/changed` unnamed (DROPPED; leg 4 allowlist proposal, tracked cyril-0na7); cyril never sends `settings.memory`/`userMemoryOptIn`
  (NOT-SENT) — cyril-memory stays the only memory a cyril user has unless they opt in (documented in CLAUDE.md "Cyril's /memory manages Cyril local memory only").

### Leg 3 / WF-L2 = TUI-L8 — `workflowLaunch` option round trip + cross-engine load (DONE 22:13-22:15 UTC) — CONFIRMED
- `probe-kas-leads-3.1.0.py wflaunch` NEW arms on / off / crossload (`kas-lead-wflaunch-3.1.0.jsonl`, 213 frames, 44.9 s); OLD arm on (`kas-lead-wflaunch-2.32.0.jsonl`).
- `on` (handshake + session/new `settings.workflows:{enabled:true}, goal:{enabled:true}`): `workflowLaunch` select in the session/new result, `currentValue
  "when-asked"` (kas-lead-wflaunch-3.1.0.jsonl#21"when-asked"); `session/set_config_option {configId:"workflowLaunch", value:"delegate-all"}` echoes
  `currentValue:"delegate-all"` in its configOptions (kas-lead-wflaunch-3.1.0.jsonl#27"delegate-all"); the model, asked, answers "My Workflow launch setting is
  \"Delegate all work.\"" (kas-lead-wflaunch-3.1.0.jsonl#212"Delegate all work"); kiro.log has no `intake.unknown-key` for it; `_kiro/configuration/getState`
  (session observer) shows only `setting/workflows="on"` — the launch mode is a session config option, not a `krn` setting.
- `off` control: option absent from session/new and from every `config_option_update`; `set_config_option workflowLaunch` is accepted silently and the
  response lists no such option (kas-lead-wflaunch-3.1.0.jsonl#212"Workflow launch").
- `crossload`: a session created by the 0.66.43 carve (gate on; its result has no `workflowLaunch`, kas-lead-wflaunch-3.1.0.jsonl#212"old_configOptions") then
  `session/load`ed by 0.66.91 in the SAME HOME with the gate on → the load result's configOptions have NO `workflowLaunch` either (legacy behaviour,
  kas-lead-wflaunch-3.1.0.jsonl#179"contentCollection") and the model reports "Delegate all work" (kas-lead-wflaunch-3.1.0.jsonl#212"currently")
  — a pre-2.33.1 session keeps the old "always delegate" posture while new sessions default to "when-asked" (the 2.33.1 "workflow now starts only when
  you explicitly ask" line, LIVE). OLD control (0.66.43, gate on): no option, `set_config_option workflowLaunch` silently accepted, the model answers
  "My Workflow launch setting is \"always delegate\"" (kas-lead-wflaunch-2.32.0.jsonl#56"always delegate"). cyril: see leg 5 S3 (option not surfaced).

### Leg 3 / WF-L3 — plan approval with typed feedback (`rejectionReason`) and `appliedPlanFeedback` (DONE 22:14-22:17 UTC)
- `probe-kas-leads-3.1.0.py wfplan31` (gate on; parent prompt → `run_workflow` with a `workflowPrompt`) on NEW (`kas-lead-wfplan31-3.1.0.jsonl`, 508 frames,
  67.0 s) and OLD (`kas-lead-wfplan31-2.32.0.jsonl`, 404 frames, 54.9 s). The first plan approval is answered `reject` + `_meta.kiro.rejectionReason:"Also add a
  final step that writes the word DONE into done.txt."` (the TUI's reject_once shape), the second `accept`; every later tool ask `accept`.
- **Both engines revise the plan from the typed feedback**: ask 1 "Add 1 workflow step" with `_meta.kiro.workflowPlan {workflowId, nodeId:"create-workflow",
  kind:"launch", workflowRef:"generated://…", keptStepIds, steps:[{nodeId, type, agentName, task{text,truncated}}]}` (kas-lead-wfplan31-3.1.0.jsonl#91"Add 1 workflow step";
  OLD kas-lead-wfplan31-2.32.0.jsonl#90"Add 1 workflow step") → after the reject-with-reason the creator comes back with "Add 2 workflow steps" whose second step is
  `write-done` (kas-lead-wfplan31-3.1.0.jsonl#117"write-done"; OLD kas-lead-wfplan31-2.32.0.jsonl#118"write-done"); both runs complete and both workspaces end with
  `hello.txt` + `done.txt`. `_kiro/workflow/steps_queued` fires with the pending steps and then `{pendingSteps:[], resolution:{outcome:"applied"}}`
  (kas-lead-wfplan31-3.1.0.jsonl#135"applied"). So the 2.34.0 "type suggested changes directly into a workflow plan approval" is a TUI input affordance over a
  wire mechanic 0.66.43 already had; the approval options are the plain 4 (allow_once/allow_always/reject_once/reject_always) on both engines.
- **NEW only: `finalState.appliedPlanFeedback[]`** = `[{text:"Also add a final step…", creatorSessionId, toolCallId, consentRound:1, topLevelStepIds:["write-hello",
  "write-done"]}]` on `run_complete` (kas-lead-wfplan31-3.1.0.jsonl#445"appliedPlanFeedback"), with `planRevision:1`; OLD's finalState has `planRevision:1` but no
  `appliedPlanFeedback` (kas-lead-wfplan31-2.32.0.jsonl#349"planRevision", 0 hits for the key). Static attribution: 0.66.58 (2.33.1). The step session's opening
  `<approved_plan_feedback>` message is not streamed to the client (0 hits) — NOT-REACHED on the wire.
- Step credits on this creator run (NEW): `create-workflow` 0.328 credits, run total 1.019 credits (kas-lead-wfplan31-3.1.0.jsonl#445"1.0190662943615258"); OLD node_complete
  has no `credits` (kas-lead-wfplan31-2.32.0.jsonl#135"node_complete").
- cyril: `rejectionReason` is already sent on reject (`crates/cyril-core/src/protocol/convert/kas.rs:219`) and `workflowPlan` is parsed
  (`crates/cyril-core/src/protocol/convert/kas/workflow_plan.rs:60`); `appliedPlanFeedback` DROPPED (0 hits) — leg 4 allowlist proposal, tracked cyril-4u4a.

### Leg 3 / LIVE-H — sub-agent delegation: the child's answer now STREAMS on the parent session (DONE 22:16-22:17 UTC) — CHANGED, P2 for cyril
- `probe-kas-leads-3.1.0.py subagent` on NEW (`kas-lead-subagent-3.1.0.jsonl`, 145 frames, 31.5 s) and OLD (`kas-lead-subagent-2.32.0.jsonl`, 128 frames, 31.9 s);
  handshake + session/new `settings.memory read_write`; workspace agent `helper` (read-only); 3 billed turns each (tool list, one `invoke_sub_agent`
  delegation reading PROBE.txt, one asking the child for a memory). Tool list identical on both (16 tools incl. `invoke_sub_agent`, `memory`).
- Common shape (both): `tool_call "Sub-agent: helper"` with `_meta.kiro {kind:"agent-subtask", agentSubtaskId}` on the PARENT session
  (kas-lead-subagent-3.1.0.jsonl#75"agent-subtask"; OLD kas-lead-subagent-2.32.0.jsonl#72"agent-subtask"); the child's own `Read File` tool_call rides the parent
  session id with `_meta.kiro.agentSubtaskId` (kas-lead-subagent-3.1.0.jsonl#87"Read File"); one `session/request_permission` for the delegation.
- **OLD (0.66.43)**: the child streams NO text; its answer arrives as the orphan `tool_call_update {title:"Subagent Response", status:"completed",
  content:[text "Response recorded."], _meta.kiro{agentSubtaskId, toolOrigin:"acp"}}` (kas-lead-subagent-2.32.0.jsonl#89"Subagent Response") before the invoke
  tool completes; then the parent says `ALPHA`.
- **NEW (0.66.91)**: NO "Subagent Response" frame (0 hits). Instead the child's reply is streamed as `agent_message_chunk` frames ON THE PARENT SESSION ID,
  distinguishable only by `_meta.kiro.agentSubtaskId` (+ `executionId`, `turnId`, `replayId`): "The word" / " is:" / " ALPHA\n\n(" / "from" / " PROBE.txt, line" / " 1)"
  (kas-lead-subagent-3.1.0.jsonl#92"agentSubtaskId" … kas-lead-subagent-3.1.0.jsonl#98"agentSubtaskId"), then the invoke tool completes
  (kas-lead-subagent-3.1.0.jsonl#100"Sub-agent: helper") and the parent's own chunk `ALPHA` follows WITHOUT `agentSubtaskId` (kas-lead-subagent-3.1.0.jsonl#102"ALPHA").
  This is the 3.1.0 line "A sub-agent stopped at its turn limit reports its unfinished work instead of an empty result" family: child text is now on the wire.
- **cyril impact (P2)**: cyril converts every `agent_message_chunk` on the main session through the generic path
  (`crates/cyril-core/src/protocol/convert/kas.rs:835` → `convert/mod.rs:506` → `Notification::AgentMessage`) and never reads `agentSubtaskId`
  (0 production hits; allowlisted "tracked: cyril-ebqu"), so on 0.66.91 the child's streamed text is appended to the PARENT's reply — the user reads
  "The word is: ALPHA (from PROBE.txt, line 1)ALPHA" as one parent message (leg 5 S5 shows the screen). The cyril-1v25 "Subagent Response" orphan handling
  (`crates/cyril-ui/src/state.rs:556`) no longer has a frame to act on. Proposed: route chunks carrying `_meta.kiro.agentSubtaskId` to the sub-agent stream
  (the `agentSubtaskId` is the only key) — extends cyril-ebqu.
- Memory half of the lead: the child answered `NONE` on both (no memory existed: the parent did not store one in this arm and `_kiro/memory/create` requires
  `memoryType, scopes, title, summary` — kas-lead-subagent-3.1.0.jsonl#144"create requires memoryType") → "sub-agents read the dispatcher's memories" NOT-REACHED.

### Leg 3 / zero-turn census (TUI-L4, L9, L10, L12; KAS-L10) (DONE 22:17-22:18 UTC)
- `probe-kas-leads-3.1.0.py zeroturn` NEW arms plain / diagrams (`kas-lead-zeroturn-3.1.0.jsonl`, 120 frames, 10.5 s), OLD plain (`kas-lead-zeroturn-2.32.0.jsonl`).
- **TUI-L12 CONFIRMED**: `clientCapabilities._meta.kiro.diagrams:true` → initialize `extensionMethods` gains `_kiro/diagram/render` and `_meta.kiro.diagramRenderFormats:
  ["mermaid","svg"]` (kas-lead-zeroturn-3.1.0.jsonl#119"diagramRenderFormats"); without the flag neither appears and `_kiro/diagram/render` → -32602
  `DiagramsNotEnabledError`; with it the method wants a `diagramId` ("diagramId is not a diagram id", kas-lead-zeroturn-3.1.0.jsonl#119"diagramId") — it renders a
  stored diagram, it does not take inline IR.
- **TUI-L4**: `_kiro/modelFallback/get` → -32603 "no persistence classification" on both engines (not -32601, but equally absent;
  kas-lead-zeroturn-3.1.0.jsonl#119"_kiro/modelFallback/get"); `modelFallbackReporting:true` on both; no `_kiro/modelFallback/*` in extensionMethods.
- **TUI-L9**: `_kiro/configuration/getState {observer:"connection"}` → `{view:{observer:"connection"}, layers:[9], settings:[21 modelFallback/<model> entries],
  resources}` on NEW (kas-lead-zeroturn-3.1.0.jsonl#119"modelFallback/claude-sonnet-5"); OLD → 0 settings. `_kiro/memory/getSyncStatus` → `{state:"off"}` (NEW only).
- **TUI-L10**: `_kiro/codeIntelligence/startBuild|cancelBuild` → -32000 `CodeIntelligenceNotEnabledError` "The code index is not enabled." on NEW
  (kas-lead-zeroturn-3.1.0.jsonl#119"CodeIntelligenceNotEnabledError"); `indexStatus` → -32603 no persistence classification (not advertised); OLD: all -32603.
  The build API needs `settings.codeIntelligence.enabled` — NOT-REACHED further (zero-turn arm).
- **KAS-L10 `_kiro/configuration/set`**: `{krn, value}` → -32602 "layer: Required"; `{krn:"krn:::setting/tangentMode", value:"on", layer:"user-local"}` → -32602
  "a client may not set this setting at this layer" `{errorType:"ConfigurationSetRefusedError", reason:"notGranted", layer:"user-local"}`
  (kas-lead-zeroturn-3.1.0.jsonl#119"ConfigurationSetRefusedError"); `layer:"bogus"` → the same `notGranted` (no "Unknown layer" text); `krn:::setting/thinking`
  likewise refused; `krn:::setting/contentCollection` → "Unknown setting: contentCollection". So a direct ACP client is NOT granted user-local writes for these
  krns; the method exists for grants the TUI/host negotiate. No `_kiro/configuration/state` push followed any attempt; no user-local settings file was created.
- `_kiro/memory/delete {id:"mem-bogus-zz9"}` → -32603 "no memory with id … to delete." on both (the method exists on both).

### Leg 3 / KAS-L1 — model-fallback flag defaults (DONE 22:18 UTC) — NOT-REACHED on the wire
- `probe-kas-leads-3.1.0.py fallbackflags` plain vs env (`KIRO_FEATURE_MODEL_FALLBACK_{AVAILABILITY,REFUSAL,AUTO_REFUSAL}_ENABLED=false`) on both engines
  (`kas-lead-fallbackflags-{3.1.0,2.32.0}.jsonl`): kiro.log at debug prints only `network.getFeatureConfiguration.ok` and (NEW) `modelFallback.chains.read {models:1}`
  — the resolved flag values are not logged, so the default flip is not observable here. Passive scan of every capture of this lane: 0 frames with
  `kind:"model_fallback"`, `kind:"model_routed"` or `_meta.kiro.modelFallback` (no refusal/outage occurred today). The only LIVE fallback surface is the 21
  `modelFallback/<model>` chains in the connection-layer `_kiro/configuration/state` (above). cyril-aoby: unchanged evidence.

### Leg 3 / TUI-L1 — session recap (DONE 22:18-22:22 UTC) — CONFIRMED gate, but the kind EXISTS on 0.66.43 too
- `probe-kas-leads-3.1.0.py recap` NEW arms on / off / absent / underscore (`kas-lead-recap-3.1.0.jsonl`, 234 frames, 148.8 s), OLD arm on
  (`kas-lead-recap-2.32.0.jsonl`, 56 frames, 36.9 s). One billed read turn, then 25 s idle.
- `settings.sessionRecap:{enabled:true}` (handshake + session/new) → ~1 s after `turn_end` a `session_info_update {kind:"recap", text:"We're reading and
  summarizing the contents of PROBE.txt…"}` (kas-lead-recap-3.1.0.jsonl#57"recap"; timeline turn_end 3.4 s → recap 4.3 s). `enabled:false`, key absent, and the
  zod name `_sessionRecap` → no recap frame (kas-lead-recap-3.1.0.jsonl#233"recap_frames"), and kiro.log shows no `intake.unknown-key` for `sessionRecap`
  (the non-underscore key is the accepted one). No `kind:"suggested_prompts"` in any arm.
- **OLD (0.66.43) emits the same `kind:"recap"` under the same setting** (kas-lead-recap-2.32.0.jsonl#55"recap") — the wire kind pre-dates 3.1.0 (it is in the
  covenant's 18-kind union); the 3.1.0 line adds the TUI setting `chat.enableSessionRecap` and the "above the prompt" rendering. cyril: `recap` kind is
  DROPPED (`kas.rs:822`) and `settings.sessionRecap` NOT-SENT — opt-in P3 at most (cyril's `settings.rs` BOOL_MAP could map `chat.enableSessionRecap`).

### Leg 3 / KAS-L9 — `turn_timing` opt-in (first pass DONE 22:21-22:22 UTC) — NOT-REACHED with the session-level key; prompt-level arm queued (batch 3)
- `probe-kas-leads-3.1.0.py turntiming` NEW arms ttft / perf / control (`kas-lead-turntiming-3.1.0.jsonl`, 140 frames, 30.1 s), OLD ttft: `session/new._meta.kiro.
  automaticTurnProfiling:{ttft:true[,perf:true]}` produced NO `kind:"turn_timing"` on any arm (the first-pass capture was overwritten by the prompt arm; the session-level control is re-run as `kas-lead-turntiming-session-3.1.0.jsonl` below);
  `turn_completion` is unchanged (`promptTurnSummaries, elapsedTime, status, executionId, turnId, requestIds`). The bundle's `extractAutomaticTurnProfiling`
  reads a REQUEST's `_meta.kiro` (allowed prompt-level keys `ttft`, `perf`, `automaticTurnProfiling`, `contextBreakdown`, …) and `turn_timing` is emitted when
  the persisted `ttft` session metadata is replayed → the `prompt` arm (prompt-level opt-in + `session/load`) runs in batch 3.

### Leg 3 / resume after a KAS restart (2.34.0 "pick up where they left off") (DONE 22:22-22:25 UTC) — CONFIRMED on both engines
- `probe-kas-leads-3.1.0.py wfresume` NEW (`kas-lead-wfresume-3.1.0.jsonl`, 331 frames, 88.3 s) and OLD (`kas-lead-wfresume-2.32.0.jsonl`, 315 frames, 86.7 s): a two-step
  file recipe (`fast` writes a file, `slow` runs `sleep 40`); SIGKILL of the KAS process 6 s into `slow` (kas-lead-wfresume-3.1.0.jsonl#128"kill"), respawn with the SAME
  HOME, `session/load` the parent, `_kiro/workflow/inspect` → `status:"paused"` with `fast` completed and `capturedOutputs {fast:"FAST"}`
  (kas-lead-wfresume-3.1.0.jsonl#330"capturedOutputs"), `workflow/list` row `stepsCompleted:1, stepsTotal:2, currentNodeId:"slow"` (NEW only), then `_kiro/workflow/resume`
  → `run_start`, `node_start slow` (no `node_start fast`), `node_complete slow`, `run_complete completed` (kas-lead-wfresume-3.1.0.jsonl#257"run_complete"); files
  `fast.txt=FAST`, `slow.txt=SLOW`; parent woke once. OLD: identical sequence (kas-lead-wfresume-2.32.0.jsonl#246"run_complete", fast not re-run).
- No `_kiro/workflow/*` notification is pushed on `session/load` for the paused run on either engine (the client must `list`/`inspect`); no "interrupted" pause
  text on SIGKILL (as in 2.32.0 TUI-L6). The "without replanning" half (a creator/`generated://` run killed mid-plan) is NOT-REACHED. cyril: resume path unchanged.

### Leg 3 / WF-L4 — wake race: prompt sent the instant `node_complete` arrives (DONE 22:25-22:29 UTC) — same on both engines; OLD prediction REFUTED
- `probe-kas-leads-3.1.0.py wakerace` ROUNDS=3 on NEW (`kas-lead-wakerace-3.1.0.jsonl`, 300 frames, 114.4 s) and OLD (`kas-lead-wakerace-2.32.0.jsonl`, 287 frames,
  113.8 s); gate off; one-step shell recipe; the parent `session/prompt "Reply with exactly RACE<n>"` is sent as soon as `node_complete` is seen.
- All 3 rounds on BOTH engines: the prompt runs immediately and completes (`end_turn` in 1.2–1.6 s, kas-lead-wakerace-3.1.0.jsonl#116"end_turn";
  OLD kas-lead-wakerace-2.32.0.jsonl#110"end_turn"), the run's completion is delivered INTO that turn as two `steering_injected` frames (`notify-<uuid>` step
  notification `[notification/success] Ran \`echo RACEOK\`…` and `notify-wf-<id>` run completion; kas-lead-wakerace-3.1.0.jsonl#104"steering_injected",
  OLD kas-lead-wakerace-2.32.0.jsonl#99"steering_injected"), followed by `steering_cleared`; NO `workflow-complete-wake` turn in either capture (0 hits) and no
  `_kiro/session/notify` frame; kiro.log `workflow.emitNotification.parent_busy_queued` ×3 on both. The parent replies `RACE<n>` only.
- Verdict: the race is handled identically on 0.66.43 and 0.66.91 (no dropped prompt, no second unsolicited turn) — the 3.1.0 line "A prompt sent just as a
  workflow run finishes is no longer dropped" did not reproduce as a defect on OLD in 3/3 rounds (the window may be narrower than a client can hit over stdio).
  NEW's `steering_injected._meta.kiro.notification` carries the richer `{kind, status, notifyId, nodeName, …}` object (kas-lead-wakerace-3.1.0.jsonl#104"notifyId").
  cyril: `steering_injected` with `notify-` ids → `EngineMessageInjected` (cyril-lki9 P6) — HANDLED; `notification.*` allowlisted (cyril-fb1m).

### Leg 3 / WF-L5 — fileCheck `jsonPath` grammar (DONE 22:29-22:30 UTC) — CONFIRMED (and OLD's `$.` fileCheck never matched)
- `probe-kas-leads-3.1.0.py jsonpath` on NEW (`kas-lead-jsonpath-3.1.0.jsonl`, 191 frames, 23.4 s) and OLD (`kas-lead-jsonpath-2.32.0.jsonl`, 569 frames, 58.8 s): two
  repeat recipes (`maxIterations:3`) whose stop condition is `fileCheck {path, jsonPath, value:true}` — `bracket` uses `jsonPath:"a[0]"` and the step writes
  `{"a":[true]}`; `dollar` uses `jsonPath:"$.done"` and the step writes `{"done":true}`. `listRecipes` shows no `validationError` for either on either engine;
  `_kiro/workflow/validate` is still the -32603 "no persistence classification" stub on both.
- **NEW**: `_kiro/workflow/new` on `bracket` → -32603 `"Node 'rep' has fileCheck jsonPath 'a[0]' with unsupported '['; use a dotted key path such as 'verdict' or
  '$.verdict'."` (kas-lead-jsonpath-3.1.0.jsonl#31"unsupported"); `dollar` runs ONE iteration and completes (`loop_iteration` ×1, kas-lead-jsonpath-3.1.0.jsonl#100"run_complete";
  `out/done.json = {"done":true}`). The 23 s total is one real run, not an early error (the bracket rejection is the expected branch).
- **OLD**: `bracket` is accepted (kas-lead-jsonpath-2.32.0.jsonl#28"workflowId") and runs 3 iterations (kas-lead-jsonpath-2.32.0.jsonl#207"run_complete"), and `dollar` ALSO
  runs 3 iterations to the cap (kas-lead-jsonpath-2.32.0.jsonl#489"run_complete") although `done.json` held `{"done":true}` after iteration 1 — on 0.66.43 the `$.done`
  fileCheck never satisfied the stop condition. The grammar error text names no `errorType` (plain -32603 `details`). cyril: recipe authoring concern only
  (`kiro-workflow-authoring` skill note: dotted paths only; `[`-indexing is refused since 0.66.91 and silently useless before).

### Leg 3 / WF-L8 — peer session messaging (`settings.sessionTools.messaging`) (DONE 22:30-22:32 UTC) — NOT-REACHED (key known, tool not offered)
- `probe-kas-leads-3.1.0.py peer` NEW arms on / off (`kas-lead-peer-3.1.0.jsonl`, 219 frames, 76.5 s), OLD on (`kas-lead-peer-2.32.0.jsonl`, 108 frames, 38.1 s): two
  sessions on one workspace; handshake + session/new `settings.sessionTools:{messaging:{enabled:true}}`; prompt A to `send_message` B.
- On NEW the key is KNOWN but not consumed at that layer — kiro.log `configuration.intake.unread-key {key:"setting/sessionTools", layer:"client-connection"}`
  (kas-lead-peer-3.1.0.jsonl#218"unread-key"); on OLD it is `intake.unknown-key` at both the client-connection and session layers (kas-lead-peer-2.32.0.jsonl#107"unknown-key").
  The tool list is the same 15 tools on both arms and engines with NO `send_message` (kas-lead-peer-3.1.0.jsonl#83"send_message" is the model saying it has no such
  tool); no `_kiro/session/notify {sender:"peer"}`, no `deliveryCount`, session B stays idle. The messaging tool is behind a gate a direct ACP client cannot
  flip with this key (likely a backend/feature flag — `peerDeliveryAccount`/`UNIVERSAL_BACKGROUND_TOOL` family in the bundle); cyril's `notify.rs:73-77`
  "unknown workflow message sender" WARN cannot be exercised from here (leg 5 skips it).

### Leg 3 / KAS-L12 — `session/new._meta.kiro.shellEnvironment` with a client-hosted terminal (DONE 22:32 UTC) — applied by KAS, NOT forwarded to the client terminal
- `probe-kas-leads-3.1.0.py shellenv` on NEW (`kas-lead-shellenv-3.1.0.jsonl`, 72 frames, 12.2 s) and OLD (`kas-lead-shellenv-2.32.0.jsonl`, 68 frames, 11.8 s):
  `session/new _meta.kiro.shellEnvironment:{"<ws>":{"AUDIT_VAR":"1"}}`, then `echo AUDIT_VAR=$AUDIT_VAR` through the client's `terminal/create`.
- NEW kiro.log: `session.shell_environment.applied {sessionId, folderCount:1, variableNames:["AUDIT_VAR"]}` (kas-lead-shellenv-3.1.0.jsonl#71"shell_environment.applied");
  OLD logs nothing (the key is new). On BOTH engines the client-hosted `terminal/create` request carries `env:[{KIRO_SESSION_ID}, {AWS_SDK_UA_APP_ID:"kiro-agent"}]`
  only — no `AUDIT_VAR` (kas-lead-shellenv-3.1.0.jsonl#49"AWS_SDK_UA_APP_ID"; OLD kas-lead-shellenv-2.32.0.jsonl#46"AWS_SDK_UA_APP_ID") — and the command printed
  `AUDIT_VAR=` (empty) on both. So the per-folder shell environment applies to KAS's own shell execution, not to `terminal/*` callbacks: a client that hosts
  terminals (cyril does) must inject such variables itself. cyril: NOT-SENT (no `shellEnvironment`), no impact.

### Leg 3 / KAS-L13 — `session/prompt._meta.kiro.displayText` (DONE 22:32-22:33 UTC) — CHANGED: replays on BOTH engines
- `probe-kas-leads-3.1.0.py displaytext` NEW (`kas-lead-displaytext-3.1.0.jsonl`, 85 frames) and OLD (`kas-lead-displaytext-2.32.0.jsonl`, 78 frames): prompt text
  "REAL: reply with exactly the word REALSEEN." with `_meta.kiro.displayText:"SHOWN: this is the display text"`; the model answered REALSEEN on both (it saw REAL).
  Live: no `user_message_chunk` is echoed on either engine. `session/load` replay on a fresh connection: the user chunk is the DISPLAY text
  `SHOWN: this is the display text` with `_meta.kiro {userMessageTag, displayText, messageId, timestamp, replay:true}` on NEW
  (kas-lead-displaytext-3.1.0.jsonl#64"SHOWN") AND on OLD (kas-lead-displaytext-2.32.0.jsonl#58"SHOWN") — the 0.66.58 attribution does not hold live; 0.66.43
  already replayed the display text. cyril: NOT-SENT (`displayText`); replayed `user_message_chunk` HANDLED generically.

### Forwarded leads received (orchestrator, ~22:33 UTC) — v3-coverage lane V1…V7 (leg-5 checklist; last forward)
- V1 regression checklist (shell_type basename + `policy.shell.type.resolved`; first getAccessToken via `chat _ get-kas-token` + any `cps.auth_provider.unknown`;
  Always-allow scope session then user with the consent echo + permissions.yaml + no re-ask; sub-agent rendering incl. the LIVE-H streamed child text; `/<skill> hello`
  as prompt text; `/model` `/effort` `/mode` round trips; `KIRO_HOME`; repeat-cap wake header; `clientInfo.name`), V2 (Always-allow on an unparseable command →
  on 0.66.91 the ask is gone entirely per LIVE-C(i), so V2 becomes "cyril shows no overlay; nothing to save"), V3 (w55l re-check: step-session Always-allow with
  the echo, count rounds), V4 (effort/thinking post-ack), V5 (`/load` credits: replayed `session_usage`/`turn_completion` vs cyril's footer), V6 (node engine
  cyril resolves vs `engines >=22.13.0`), V7 (`_kiro/workspace/active_file` under cyril in kiro.log). Build commit recorded per row (trap 31).

### Leg 3 / LIVE-C extra — a deny rule blocks the call (2.34.0 "no longer reported as cancelled") (DONE 22:33 UTC) — same frame shape on both engines
- `probe-kas-leads-3.1.0.py denyrule` NEW (`kas-lead-denyrule-3.1.0.jsonl`, 64 frames) / OLD (`kas-lead-denyrule-2.32.0.jsonl`, 62 frames): workspace agent `denier`
  with `permissions.rules:[{capability:"shell", match:["echo AUDITDENY*"], effect:"deny"}]`; the model ran `echo AUDITDENY now`.
- Both: no `session/request_permission`; `tool_call_update status:"failed"` with `_meta.kiro.policyDenial {capability:"shell", resource, triggeringResource, effect:"deny",
  matchedRule{capability, match, effect}, scope:"workspace", source:"agent-profile"}` (kas-lead-denyrule-3.1.0.jsonl#45"policyDenial"; OLD kas-lead-denyrule-2.32.0.jsonl#44"policyDenial");
  neither says "cancelled". Only the tool-result text changed: OLD "Tool call denied by user's permissions. Rule: …", NEW "Blocked by a permission rule (deny shell
  matching …); the user did not rejec[t]…" (kas-lead-denyrule-3.1.0.jsonl#45"Blocked by a permission rule"). `_kiro/permissions/explain` → `effect:"deny"` + matchedRule
  on both. The "cancelled" wording in 2.34.0's line is TUI-side. cyril: `policyDenial` is read (`crates/cyril-core/src/protocol/convert/kas.rs` Trust-v2 arm) — HANDLED.

### Leg 3 / LIVE-D = KAS-L7 + KAS-L8 — hooks: PreCompact/PostCompact, PreToolUse exit 2, client- vs KAS-executed paths (DONE 22:34-22:50 UTC; batch 3 re-run)
- Batch-2 hooks arms were VOID (the runner wrote every arm of one engine to the same file; and none advertised the v2 gate). Batch 3 re-ran with the suffixing
  runner: NEW arms `v2`, `v2block`, `hookscap`, `agentfile`, `preblock`; OLD arms `v2`, `hookscap`, `agentfile` (`kas-lead-hooks31-<arm>-{3.1.0,2.32.0}.jsonl`).
  Seeds: workspace agent `hooked` with object-form `hooks {preToolUse:[{matcher:"execute_bash", command:"exit 2"}], preCompact, postCompact, userPromptSubmit}`,
  agent `hooked2` with ARRAY-form hooks, and (v2 arms) a standalone `.kiro/hooks/audit.json` `{version:"v1", hooks:[{name, trigger, matcher?, action:{type:"command",
  command}}]}` (cyril's own loader format). One shell turn, then `_kiro/session/compact`.
- **The gate**: the KAS-side v2 hook runtime exists only when the client advertises `clientCapabilities._meta.kiro.hooks:{enabled:true, v2:true}` (bundle:
  `u.v2===!0 → v2HooksCache`). Without `v2:true` (`agentfile`/`preblock` arms: no hooks cap; `hookscap`: `{enabled:true}` only) `_kiro/hooks/list` (client→agent)
  answers -32603 "`_kiro/hooks/list is not available when v2Hooks is disabled`" on both engines (kas-lead-hooks31-agentfile-3.1.0.jsonl#36"v2Hooks is disabled") and
  NO agent-file or standalone hook runs (hook logs never written; kas-lead-hooks31-agentfile-3.1.0.jsonl#46"HOOKTEST" ran unhindered). The ARRAY-form agent hooks are
  rejected on both engines: `_kiro/customAgent/config_error … hooks.0.name: Required, hooks.0.action: Required` (kas-lead-hooks31-v2-3.1.0.jsonl#87"hooks.0.action").
- **KAS-executed path (`v2:true`)**, NEW: `_kiro/hooks/list {trigger}` returns the registry in exactly cyril's wire shape `{id:"<file>#hook-N", name, action:{type:"runCommand",
  command}, _meta:{trigger, source:"standalone-file", matcher?, filePath, enabled, kiro.resource}}` (kas-lead-hooks31-v2-3.1.0.jsonl#38"audit-pre"); `_kiro/hooks/didChange`
  fires at session start (kas-lead-hooks31-v2-3.1.0.jsonl#15"_kiro/hooks/didChange"); every run is reported as `session_info_update kind:"hook_update" {hook:{hookId,
  operationId, name, status, actionType, output{command, result{exitCode}}}}`. Standalone AND object-form agent hooks both ran (`FILE-PROMPTSUBMIT` + `PROMPTSUBMIT`).
  **KAS-L8 CONFIRMED**: the PreToolUse `exit 2` hook (no output) BLOCKS the tool on NEW — `hook_update` for `audit-pre` (kas-lead-hooks31-v2-3.1.0.jsonl#57"audit-pre"),
  `tool_call_update status:"failed"` (kas-lead-hooks31-v2-3.1.0.jsonl#59"failed"), model: "blocked by a PreToolUse hook (exit code 2)" (kas-lead-hooks31-v2-3.1.0.jsonl#85"blocked by a PreToolUse hook");
  on OLD the same hooks ran but the tool was NOT blocked (`HOOKTEST` completed, kas-lead-hooks31-v2-2.32.0.jsonl#45"HOOKTEST", kas-lead-hooks31-v2-2.32.0.jsonl#50"completed")
  — the 3.1.0 line "exit 2 blocks even with no reason" is LIVE for KAS-executed hooks.
  **KAS-L7 CONFIRMED**: `_kiro/session/compact` on NEW runs the PreCompact hooks (file + agent-profile; `hook_update audit-precompact completed`,
  kas-lead-hooks31-v2-3.1.0.jsonl#81"audit-precompact"), compacts (`summarization_completed`, `{success:true}` kas-lead-hooks31-v2-3.1.0.jsonl#86"success"), then runs the
  PostCompact hooks (kas-lead-hooks31-v2-3.1.0.jsonl#83"audit-postcompact") with a JSON stdin payload `{session_id, hook_event_name:"PostCompact", cwd, compaction_id,
  reason:"manual", message_count, transcript_path, summary}` (kas-lead-hooks31-v2-3.1.0.jsonl#87"postjson_file"). With PreCompact `exit 2` (`v2block`) the compaction is
  REFUSED: `hook_update audit-precompact status:"failed" exitCode 2` (kas-lead-hooks31-v2block-3.1.0.jsonl#82"exitCode"), `_kiro/session/compact` → `{success:false}`
  (kas-lead-hooks31-v2block-3.1.0.jsonl#84"success"), no summarization, PostCompact not run. OLD with the same seeds: compaction ran with NO compaction hook
  (`{success:true}`, kas-lead-hooks31-v2-2.32.0.jsonl#91"success"; 0 `audit-precompact` frames) — new in 0.66.91. `_kiro/hooks/list` is never called WITH a compaction
  trigger on the client path (triggers seen: sessionStart, promptSubmit, preToolUse, postToolUse, agentStop).
- **Client-executed path (`hooks:{enabled:true}`, cyril's shape)**, BOTH engines: KAS calls `_kiro/hooks/list` per trigger and, for the returned runCommand hook,
  first asks `session/request_permission {title:"exit 2", _meta.kiro.hookName}` for the HOOK command (kas-lead-hooks31-hookscap-3.1.0.jsonl#64"exit 2"), then
  `_kiro/hooks/executeHook {hookId, hookName, command, sessionId, userPrompt:<tool args JSON>, operationId}` (kas-lead-hooks31-hookscap-3.1.0.jsonl#72"_kiro/hooks/executeHook").
  The client answered `{output:"", exitCode:2, cancelled:false}` (kas-lead-hooks31-hookscap-3.1.0.jsonl#73"exitCode") → `hook_update status:"completed"`
  (kas-lead-hooks31-hookscap-3.1.0.jsonl#74"completed") and the tool went on to ask and RUN (kas-lead-hooks31-hookscap-3.1.0.jsonl#46"HOOKTEST"); identical on OLD
  (kas-lead-hooks31-hookscap-2.32.0.jsonl#67"_kiro/hooks/executeHook", kas-lead-hooks31-hookscap-2.32.0.jsonl#43"HOOKTEST"). So **exit 2 without output does NOT block
  on the client-executed path on either engine** — the 3.1.0 fix covers only KAS-executed v2 hooks. cyril (which implements the client path:
  `crates/cyril-core/src/protocol/kas/hooks.rs:383` returns `{output, exitCode, cancelled}`): a cyril-hosted PreToolUse hook that exits 2 silently does not block;
  it must print a reason (the 2.7.1 audit's `output:"DENY…"` form blocks). P2 candidate: cyril should synthesize a non-empty `output` when a hook exits 2 silently.
- `hook_update` is HANDLED by cyril (`kas.rs:668`); `_kiro/hooks/didChange` is an unnamed notification (DROPPED; triage in leg 4).

### Leg 3 / KAS-L9 — `turn_timing` (second pass, prompt-level opt-in, DONE 22:50-22:51 UTC) — CONFIRMED (replay-only kind; live data rides the last chunk)
- `probe-kas-leads-3.1.0.py turntiming` ARMS=prompt on NEW (`kas-lead-turntiming-prompt-3.1.0.jsonl`, 106 frames, 95.8 s): t1 with `session/prompt._meta.kiro.
  automaticTurnProfiling:{ttft:true, perf:true}` → nothing; t2 with the bare prompt-level keys `_meta.kiro.{ttft:true, perf:true, contextBreakdown:true}` → the turn's
  LAST `agent_message_chunk` carries `_meta.kiro.ttft {executionId, turnId, promptReceivedAt, modelRequestSentAt, firstTokenReceivedAt, firstChunkEmittedAt,
  agentOutput{version, startMs, endMs, coveredUntilMs, complete, reasons[], intervals[{id, startMs, endMs, ended, kind:"model", modelIndex}]}}`
  (kas-lead-turntiming-prompt-3.1.0.jsonl#54"firstTokenReceivedAt"); no `kind:"turn_timing"` is emitted live (info kinds stay turn_start/turn_completion/session_usage/
  turn_end). `session/load` on a second connection replays `session_info_update {kind:"turn_timing", messageId:"<turnId>-ttft", executionId, turnId, ttft{…}, replay:true}`
  for t2 only (kas-lead-turntiming-prompt-3.1.0.jsonl#86"turn_timing"; t1's turnId is absent). `contextBreakdown:true` also makes every `context_usage` of that turn carry
  the 6-bucket `breakdown` (it did on the un-opted turns too in this session). The session-level `session/new._meta.kiro.automaticTurnProfiling` key had no visible
  effect (first pass; that capture was overwritten by this arm and is re-run as `kas-lead-turntiming-session-3.1.0.jsonl` if time allows). cyril: NOT-SENT; `turn_timing`
  DROPPED (`kas.rs:822`) — opt-in diagnostics (TTFT) available to cyril's `/stats`-class view if wanted (P3 note).

### Leg 3 / LIVE-F = HOST-L4 = TUI-L7 + HOST-L1 (token arm) + HOST-L2 default runtime dir — archived 3.1.0 host with a COPY of the store (DONE 22:53 UTC) — **STOP-AUTH raised**
- `probe-kas-host31-3.1.0.py token` (`host-token-3.1.0.jsonl`), the procedure from the brief: (1) `kiro-cli user whoami` with the INSTALLED 2.32.0 launcher →
  rc 0, token row unchanged, `expires_at 2026-11-08T23:50:59Z` (the driver had already renewed single-flight at 22:50:59 during batch 3's last arm;
  host-token-3.1.0.jsonl#1"token_row_changed"); (2) `cp -a ~/.local/share/kiro-cli` → owner-only temp XDG_DATA_HOME (minus kas/node/bun/tui.js; 8 tables, 11 migrations,
  host-token-3.1.0.jsonl#2"migrations"); (3) archived 3.1.0 `kiro-cli chat _ get-kas-token` (no `--force-refresh`) against the COPY.
- **LIVE-F CONFIRMED (cyril-zbu7 path intact on 3.1.0)**: stdout is `{kind:"getKasToken", data:{accessToken, expiresAt, profileArn, provider}}` with
  `provider:"Enterprise"`, no `authMethod`; `expiresAt` EQUALS the store's `expires_at` (no refresh), rc 0 in 0.65 s, byte-identical on a second call
  (host-token-3.1.0.jsonl#4"getKasToken", host-token-3.1.0.jsonl#4"expiresAt_equals_store_expires_at", host-token-3.1.0.jsonl#5"same_stdout"); the copy's token row was
  UNCHANGED after both calls. `--help` (empty store, host-help-3.1.0.jsonl#6"force-refresh"): `--reason <REASON>`, `--current-expires-at <ISO>` (informational) and
  `--force-refresh` ("Force a network token refresh … set by the TUI host handler when KAS sends forceRefresh … during mid-turn 401 recovery"). cyril's parser
  (`crates/cyril-core/src/protocol/kas/auth.rs:896-914`: `expiresAt`, `profileArn` required, `authMethod`/`provider` optional) matches this envelope.
- **HOST-L1 with a token (arm a only)**: archived 3.1.0 `kiro-cli acp` (no flag, `clientInfo.name:"cyril"`) on the COPY spawned
  `$KIRO_RUNTIME_DIR/3.1.0-a53631e2096ad811/node --experimental-wasm-modules … acp-server.js` (child exe recorded, host-token-3.1.0.jsonl#6"3.1.0-a53631e2096ad811")
  — the **v3 route** — and the initialize result carried `agentCapabilities._meta.kiro` with 34 `extensionMethods` (the 32 of a direct spawn + host-added ones;
  host-token-3.1.0.jsonl#6"extensionMethods_n"); `session/new` succeeded. `acp --help` documents `--agent-engine … v2 when neither is set`
  (host-help-3.1.0.jsonl#7"acp.agentEngine setting"), so **an unlisted client gets v3 despite the documented v2 default** (the 2.36.0 `clientInfo.name`
  routing). The host's own stderr printed nothing (the 3.1.0 `[INFO]` suppression is a host filter; the direct spawns in leg 0 still print them).
- **STOP-AUTH**: 6.7 s into arm a the COPY's `kirocli:odic:token` row changed (host-token-3.1.0.jsonl#7"copy token row changed"; `compare`: only that auth_kv row,
  `state` 33→34 rows, `extracted_kas_versions` 4→2 — the 3.1.0 host pruned stale KAS extraction rows and wrote one state row; schema/migrations unchanged
  host-token-3.1.0.jsonl#8"copy_schema_changed"). The driver stopped arms b/c and deleted the copy (host-token-3.1.0.jsonl#9"copy_deleted"). The only auth line in that
  arm's KAS log is `acp-peer.refreshToken.ok {durationMs:8}` (a host-mediated callback read — 8 ms is too fast for an OIDC round trip), so the change is
  plausibly a re-serialised token row rather than a consumed refresh token, but the copy is gone and it cannot be proven. The real store is untouched
  (same `expires_at`, 11 migrations, same tables, `last-forced-refresh-at` unchanged). Reported to the orchestrator at 22:56Z; from that point this lane
  runs NOTHING that can refresh (no `whoami`, no `--force-refresh`, no archived-binary spawn with a token). **User-facing risk**: if it WAS a refresh, the
  installed 2.32.0 kiro-cli cannot renew at 23:50:59Z and one `kiro-cli login` is needed.
- **HOST-L2 default runtime dir (empty store, no `KIRO_RUNTIME_DIR`)**: `kiro-cli _ extract-runtime` installs to **`<bin dir>/../lib/kiro-cli/3.1.0-a53631e2096ad811/`**
  (`…/extracted/kirocli/lib/kiro-cli/3.1.0-a53631e2096ad811`, host-extract-default-3.1.0.jsonl#1"lib/kiro-cli/3.1.0-a53631e2096ad811"), i.e. for the user's install
  `~/.local/bin/kiro-cli` → `~/.local/lib/kiro-cli/<ver>-<hash>/{node,bun,tui.js,kas/node_modules/@kiro/agent/dist/server/acp-server.js}`; the data dir gets no `kas/`.
  **cyril impact (P1 candidate)**: cyril's discovery searches `<data_dir>/kas/<kas-ver>-<sha>/` only (`crates/cyril-core/src/protocol/kas/discovery.rs:208`) — after
  the user upgrades to 3.x, no new extraction lands there; cyril keeps spawning the STALE 0.66.43 tree left by 2.32.0 (version skew against a 3.1.0 host/token
  path) or, on a fresh machine, fails with "KAS bundle not found" (`discovery.rs:51`). Fix shape: also search `<dir of kiro-cli>/../lib/kiro-cli/<host-ver>-<hash>/kas/`
  and `$KIRO_RUNTIME_DIR/<host-ver>-<hash>/kas/`, and prefer the `node` beside it.
- `acp --help` also documents `--auth-method <cli|client>` ("client: Forward the v3 engine's access-token requests to the ACP client") and `--login`
  ("Log in interactively and exit, for ACP clients that run terminal authentication") (host-help-3.1.0.jsonl#7"--auth-method"); HOST-L1 arms b/c/f/g are re-run
  with `--auth-method client` on EMPTY stores (zero auth risk) as `host-engine-authclient-3.1.0.jsonl`.

### Leg 4 — sweep-new-fields diffs + unread-fields triage (DONE 22:57 UTC)
- `sweep-new-fields.py --diff` per pair saved as `sweep-new-fields-diff-{sweep,wfoff,wfon,trust,elicit,unparse,signin,policyrule,tier,subagent,memory,wfplan31,wflaunch,
  recap,zeroturn,fallbackflags}.txt`. Version deltas (seen in NEW only, same workload): `session_usage`/`credits[]`, `executionId`/`turnId`/`boundarySource`/`timestamp` on
  turn frames, `summarizationThreshold`, `autoCompactionThreshold`, `_kiro/memory/syncStatus.state`, memory `total/counts`, workflow `stepsCompleted/stepsTotal/
  currentNodeId/waitingOnWatch`, `notifyId` (+ `notification.{nodeName,notifyId}` on replayed step messages), `appliedPlanFeedback[]`, `userMessageTag` on turn_start,
  `trustClassifierApproved`, `policy/changed.errors[]`, `diagramRenderFormats`, `ConfigurationSetRefusedError{krn,layer,reason}`; the one removal: `_meta.kiro.preview.*`.
  Everything else in the "only live" columns is driver-summary or coverage (arms that ran on one engine only, model-chosen tool args).
- `unread-fields.py` over the NEW captures cyril would receive (sweep, gate-off workflow, sub-agent, elicitation/MCP, trust, sign-in, policyrule, memory, wfplan31,
  hooks hookscap, denyrule; never a control arm) → `live/unread-fields-3.1.0.txt`: 1632 paths, **70 UNREAD** (19 probe-only result roots skipped). Triage (every line):
  - `_kiro/workflow/*.{currentNodeId,stepsCompleted,stepsTotal,waitingOnWatch}` ×36 (8 events + steps_queued + `workflow/list.runs[]`) — DROPPED by cyril; one P3 issue
    (progress + credits in the workflow panel; cyril-4u4a extension) OR allowlist `_kiro/workflow/*.stepsCompleted`, `*.stepsTotal`, `*.currentNodeId`, `*.waitingOnWatch`
    `# live run progress (0.66.48); tracked: <new issue>`.
  - `_kiro/workflow/run_complete.finalState.appliedPlanFeedback`, `result:_kiro/workflow/inspect.state.appliedPlanFeedback` — allowlist `*.appliedPlanFeedback*
    # plan feedback echo (0.66.58); tracked: cyril-4u4a`. `*.originalUserMessages` (creator runs) — allowlist `*.originalUserMessages # creator-run brief echo; tracked: cyril-4u4a`.
  - `_kiro/session/notify.notifyId` — allowlist `_kiro/session/notify.notifyId # correlation id (0.66.91); the notify- prefix is already stripped from steering ids (cyril-lki9)`.
  - `session/update.update._meta.kiro.{executionId,boundarySource}` — allowlist `# turn identity/boundary fields (0.66.58), not rendered`. `turnId` is already named.
  - `session/update.update._meta.kiro.summarizationThreshold`, `…contextUsage.summarizationThreshold`, `result:initialize.agentCapabilities._meta.kiro.autoCompactionThreshold`
    — allowlist all three `# compaction threshold (0.66.58); context bar could draw it; tracked: cyril-5et2`.
  - `_kiro/memory/syncStatus`, `_kiro/memory/changed` (whole notifications), `session/update.update._meta.kiro.memories`, `result:session/new._meta._meta.kiro.settings.{memory.reflection,
    userMemoryOptIn}` — allowlist `# native KAS memory (opt-in; cyril-memory is cyril's memory); tracked: cyril-0na7`.
  - `_kiro/policy/changed` (whole notification) — P3 issue proposal (surface `status/errors[]`, a bad permissions.yaml silently denies every tool on 0.66.43) or allowlist
    with that issue. `result:session/new._meta.policyErrors` — same issue.
  - `_kiro/customAgent/config_error` (whole notification) — P3 issue: show "agent file invalid: <error>" (today a broken workspace agent silently vanishes from `/mode`).
  - `session/update.update._meta.kiro.trustClassifierApproved` — allowlist `# Approve-for-me classifier verdict (0.66.70); tracked: <new issue>` (badge the row).
  - `session/update.update._meta.kiro.policyDenial` — reported UNREAD by key-name matching, but cyril reads it through the Trust-v2 arm (`convert/kas.rs` `policy_denial`
    snake_case field name does not match the camelCase literal) → HANDLED; allowlist `session/update.update._meta.kiro.policyDenial # read via serde rename (convert/kas.rs)`.
  - `_kiro/mcp/elicitation.elicitation.requestedSchema.properties.*`, `session/update.update._meta.kiro.elicitation.requestedSchema.properties.*` — dynamic schema keys;
    allowlist `*.requestedSchema.properties.* # MCP form field names (dynamic); cyril declines forms (kas/mcp.rs)`.
  - `_kiro/mcp/status.servers[]._meta.kiro.resource.{readFrom,standing}`, `…tools[].inputSchema.properties.*` — allowlist under the existing `_kiro/mcp/status` line
    (tracked cyril-nk4o); the whole-notification line should already cover children — the tool lists nested paths because `_kiro/mcp/status` is named in crates.
  - `session/update.update._meta.kiro.{summarization,conversationSummary}` — `summarization_completed` payload; allowlist `# compaction summary payload; tracked: cyril-58uv`.
  - `session/update.update._meta.kiro.steeringDocuments`, `…breakdown.contextFiles.items[].progressivelyLoaded`, `availableCommands[]._meta.kiro.contextQuery` — standing
    (present on OLD too); allowlist `# steering_inclusion catalog / context breakdown detail / steering command query; tracked: cyril-58uv`.
  - NOT in the list but DROPPED by consumer analysis (key-name matching passes them silently): `kind:"session_usage"` (P2, toolbar), `agentSubtaskId` on parent-session
    `agent_message_chunk` (P2, LIVE-H), `kind:"recap"`, `kind:"turn_timing"`, workflow `credits[]` (the literal "credits" exists in cyril's usage tests).
  - The allowlist line `session/update.update._meta.kiro.preview.*` is now dead on 0.66.91 (field removed) — drop it at the next edit.

### Leg 3 / HOST-L1 (completed on EMPTY stores with `--auth-method client`, DONE 22:59 UTC) — the 3.1.0 `kiro-cli acp` routes an unlisted client to v3
- `probe-kas-host31-3.1.0.py engine` ARMS=a2,b2,c2,d2,f2,g2,k2 (`host-engine-authclient-3.1.0.jsonl`): fresh HOME + fresh XDG_DATA_HOME (no data.sqlite3), archived
  3.1.0 bin first on PATH, `--auth-method client` so the host forwards `_kiro/auth/getAccessToken` to the ACP client (answered with a JSON-RPC error — no token
  anywhere). `--auth-method` is accepted only when the RESOLVED engine is v3 (`error: --auth-method is only supported with --agent-engine=v3`, exit 2 before any
  initialize), which makes it a token-free route discriminator alongside "node child spawned" vs "immediate V2 initialize".
  - no flag, `clientInfo.name:"cyril"` → **v3**: `node <KIRO_RUNTIME_DIR>/3.1.0-a53631e2096ad811/node … acp-server.js` child, initialize `_meta.kiro` with 34 extensionMethods,
    `authMethods` aws-builder-id/aws-iam-identity-center, then `_kiro/auth/getAccessToken` ×5 forwarded to the client (host-engine-authclient-3.1.0.jsonl#1"3.1.0-a53631e2096ad811");
    `--agent-engine v3` identical (host-engine-authclient-3.1.0.jsonl#3"_kiro/auth/getAccessToken"); `clientInfo.name:"JetBrains.Foo"` → v3 too (host-engine-authclient-3.1.0.jsonl#6"3.1.0-a53631e2096ad811").
  - `--agent-engine v2` → not v3 (host-engine-authclient-3.1.0.jsonl#2"only supported with --agent-engine=v3"); scratch `cli.json {"acp.agentEngine":"v2"}` + no flag → not v3
    (host-engine-authclient-3.1.0.jsonl#4"only supported with --agent-engine=v3") — the setting DOES move the engine; `clientInfo.name:"acp-probe"` (a snapshot name) + no flag → not v3
    (host-engine-authclient-3.1.0.jsonl#5"only supported with --agent-engine=v3") — the `clientInfo.name` routing is LIVE; `--agent-engine v1` → not v3 (host-engine-authclient-3.1.0.jsonl#7"only supported").
  - With a token (arm a on the store copy, above) the no-flag cyril route was also v3 and `session/new` succeeded (host-token-3.1.0.jsonl#6"3.1.0-a53631e2096ad811").
- Verdict: on 3.1.0 a bare `kiro-cli acp` from cyril (`clientInfo.name` cyril, not in the v2 snapshot) gets the **v3 engine** despite `--help`'s "v2 when neither is set";
  `acp.agentEngine:"v2"` or `--agent-engine v2` restore v2. cyril's V2 wrapper spawn (`bridge.rs:394` bare `kiro-cli acp`) will therefore be handed KAS on a 3.x
  install and must pin `--agent-engine v2` (or the user sets `acp.agentEngine`); the "engine mismatch" fingerprint is the expected symptom (HOST-L6 NOT-RUN: it would
  expose a token to the archived host). The empty-store v3 route stalls on the forwarded token request (every `getAccessToken` answered with an error; no
  `session/new` result in 90 s) — the 2.37.0/3.1.0 sign-in path from the client's side.
