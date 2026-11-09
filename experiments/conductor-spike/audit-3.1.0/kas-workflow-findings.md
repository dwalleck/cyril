# kas-workflow lane: kiro-cli 3.1.0 audit, KAS 0.66.43 (2.32.0) -> 0.66.91 (3.1.0)

Scope: the workflow engine inside `@kiro/agent` (`_kiro/workflow/*`, lifecycle events, plan approvals, settings keys, recipes,
bundled agents, model tools, auto-wake). Six distinct bundles: 0.66.43 (2.32.0, OLD) / 0.66.48 (2.33.0) / 0.66.58 (2.33.1) /
0.66.62 (2.34.0-2.34.2) / 0.66.70 (2.35.0-3.0.0) / 0.66.91 (3.1.0, NEW). Every row is **STATIC** (no Kiro code was executed);
each add/remove was counted in OLD, in the first carve that carries it, and in NEW (traps 2, 3, 30). cyril verdicts are against
HEAD e3a3868d. Sibling lanes: kas-core (auth/permissions/credits outside workflows/`sessionTools`/jobs), tui, host.

## Summary

1. **No `_kiro/workflow/*` method or lifecycle kind was added or removed** (14 requests + 9 event kinds + `recipes_changed` +
   `steps_queued`, identical sets in all six carves). 0.66.91 is built with esbuild keepNames (`a(fn,"name")`), which is why every
   method literal gained exactly 2 occurrences and why NEW has readable function names.
2. **Step credits (0.66.91, 3.1.0):** node state gains `credits:[{usage,unit?,unitPlural?}]`; `node_complete` carries the step's
   `credits`; `run_complete` carries `credits` summed by unit over every node (`runCompleteCreditFields`). cyril DROPS all three
   (`WireNodeSnapshot`/`WireNodeCompleted`/`WireRunCompleted` have no field) and still derives run totals from parent-turn metering.
3. **`workflowLaunch` session config option (0.66.58, 2.33.1):** `{type:"select",id:"workflowLaunch",name:"Workflow use",
   values when-asked|large-tasks|delegate-all}`, default `when-asked` for new sessions, offered only while workflows are gated on;
   drives a `# Workflow launch setting` system-prompt block and a `<session_context>` line. Kiro's TUI restores `chat.workflowLaunch`
   via `session/set_config_option`. cyril neither sets nor shows it (ADR-0013 drift; sessions recorded before 0.66.58 load as
   **delegate-all**).
4. **Plan feedback (0.66.58):** a rejected plan approval's `_meta.kiro.rejectionReason` is persisted on `interaction_resolved`,
   collected into run state `appliedPlanFeedback[]`, fed to the creator's revision and injected as `<approved_plan_feedback>` into
   the affected steps. Wire shape of the approval itself (title/Text/`_meta.kiro.workflowPlan`) is unchanged; cyril already sends the
   reason (HANDLED).
5. **List rows gain progress** `stepsCompleted/stepsTotal/currentNodeId/waitingOnWatch` (0.66.48); cyril DROPS them.
6. **Review-loop fix (0.66.48):** fileCheck `jsonPath` now strips a leading `$` and rejects `[`,`]`,`*`,`..` at load
   (`WorkflowFileCheckJsonPathUnsupportedError`); `github-pr` watch gains `mergeable`/`mergeStateStatus`, review-thread ids and
   conflict re-wake; `wf-pr-responder` acts on conflicts and resolves threads.
7. **Wake path:** 0.66.58 wraps wake prompts in `wakePromptMeta` (adds client profiling opt-ins); 0.66.91 adds `wakeTargetIsBusy`
   (prompt in setup or persistence in flight => queue the completion as steering instead of racing a wake) = the 3.1.0 "prompt not
   dropped" fix. Trigger set (step info/warning/error, `run_complete`, repeat cap) unchanged; cyril-lki9/6lql paths unaffected.
8. **`_kiro/session/notify` sender enum gains `peer`** (0.66.91, `sessionTools.messaging`, default off); cyril's notify adapter drops
   peer messages with a warning. Recipe zod schema unchanged (the 2.26.0 JSON stays valid for the zod; its missing `include` node is
   the pre-existing cyril-inwg gap). Seven bundled recipes byte-identical; agents changed at 0.66.48/0.66.70/0.66.91 (prompts only).
9. Not located in KAS workflow code: 2.33.0 "replies no longer cut off", 2.33.0 Windows drive-letter case, 3.1.0 "changes land on
   your checked-out branch" (no worktree/branch logic in KAS or TUI 3.1.0 workflow code) — leads WF-L7/WF-L10.

## Deltas

| item | first version (KAS) | gate/default | direction | STATIC | cyril verdict + file:line | issue/note |
|---|---|---|---|---|---|---|
| **Step credits on the wire.** Node schema `cht` gains `credits:x.array(l$a).optional().catch(void 0)`, `l$a=x.object({usage:x.number(),unit:x.string().optional(),unitPlural:x.string().optional()})`. `emitNodeComplete` adds `...n.credits!==void 0?{credits:n.credits}:{}`; every `run_complete` emit (4 sites) spreads `...She(state)` = `{credits:[sum by unit over all nodes]}` (empty => key absent). Filled by `recordSessionCredits(node)` <- `deps.readSessionCredits(sessionId)` <- step session `eventAdapter.sessionUsage()?.credits` or transcript `usage_summary` rows; also on fail/abort (`recordUnfinishedStepCredits` at cancel). Counts: `credits` literal 3 (0.66.43) -> 10 (0.66.58, trust-classifier telemetry) -> 28 (0.66.91) | 3.1.0 (0.66.91) | always (gate-off too) | new fields on `node_complete`, `run_complete`, snapshot nodes (`inspect`/`load`/`new`) | STATIC (schema + emit sites read; amounts body-unproven) | DROPPED: `WireNodeSnapshot` crates/cyril-core/src/protocol/convert/kas/workflow.rs:444-520, `WireNodeCompleted` :259-283, `WireRunCompleted` :207-218 (serde ignores unknown members); run totals come from parent metering instead (crates/cyril-core/src/workflow.rs:487 `run_credits`, :1319 `record_turn_metering`; crates/cyril-ui/src/turn_labels.rs:108-121) ; no per-step credit rendering (crates/cyril-ui/src/workflow_format.rs has no credit field) | **NEW P2** "Model KAS 0.66.91 per-step `credits` and show them on step rows / run completion" (entry: every cyril-launched run, gate-off) |
| **`run_complete.initiator`/`initiatorReason`** (`runCompleteAttributionFields`: `stopInitiator`/`stopReason` from `_kiro/workflow/cancel {initiator:"user", reason}`) | pre-existing (0.66.43 `Rce`) | — | unchanged | STATIC | DROPPED (`WireRunCompleted` :207-218) | NOTE cyril-4u4a (standing gap, not new) |
| **`workflowLaunch` session config option.** `cpo(v)`: `{type:"select",id:"workflowLaunch",name:"Workflow use",currentValue,options:[when-asked "When asked", large-tasks "Large tasks only", delegate-all "Delegate all work"]}`; default `Tze="when-asked"` for every non-step session (`d?.role!=="workflow-step"&&(F.workflowLaunch=Tze)`); offered only when `orchestratesWorkflows(session)` (workflows gated on, mode not autonomous, not a step); persisted in session metadata (forks inherit; `session/new` result `_meta` strips it); unknown recorded value -> warn `session.workflow_launch.unknown_recorded_value` and default. State `spo`: gate off -> `unavailable` (no option, no prompt); recorded -> `offered`; **gate on but nothing recorded (pre-0.66.58 session) -> `legacy` = "Delegate all work"**. Prompt: `<session_context>` line "Workflow launch setting: <label>." + system block `# Workflow launch setting` (three rules, see control plane). Counts: `workflowLaunch` 0 -> 24 (0.66.58) -> 26 | 2.33.1 (0.66.58) | only with `settings.workflows.enabled:true`; default when-asked | new config option + prompt | STATIC | DROPPED on receipt: generic select kept by `to_config_options` crates/cyril-core/src/protocol/convert/mod.rs:109-130 but `KasConfigId::from_wire` knows only model/mode/effortLevel crates/cyril-core/src/types/kas_config.rs:20-48; NOT-SENT: cyril never issues `set_config_option {configId:"workflowLaunch"}` and reads only `chat.enableWorkflows` from cli.json crates/cyril-core/src/protocol/kas/settings.rs:108-116, 140-145 (TUI 3.1.0 restores `chat.workflowLaunch` right after session/new) | **NEW P2** "Honour `chat.workflowLaunch` / expose the `workflowLaunch` option (ADR-0013 parity); loaded pre-0.66.58 sessions run as delegate-all" |
| **`# Workflow Orchestration` system prompt rewritten**: 0.66.43 "Default behavior: always delegate implementation to workflows ... **always** create and launch" -> 0.66.58 "When you start a workflow: 1. Formulate the workflow prompt ... 2. Create and launch ..." (delegation default moved into the launch setting) -> 0.66.91 step 3 rewritten ("give the user's full task or its absolute path ... State task size; leave steps, reviewers, loops and budgets to the creator ... inputs stay short ... never carry the task text as a launch input") | 2.33.1 / 3.1.0 | gate on | prompt | STATIC | n/a (engine prompt) | NOTE cyril-xehm |
| **Plan feedback.** `interaction_resolved` persists `rejectionReason` (0.66.58); plan-approval rows are matched by `toolCallId`+`consentRound` and their reasons become run state `appliedPlanFeedback:[{text,creatorSessionId,toolCallId,consentRound?,topLevelStepIds[]}]` (`o3e`/`qPe`); the creator's next revision receives them, and each new step's first prompt gets `<approved_plan_feedback>` ("Corrections the user typed while reviewing the plan ... where a correction conflicts with the original request, follow the correction"). Log `workflow.plan_feedback.read_failed`. Counts: `rejectionReason` 2 -> 9 (0.66.58) -> 10; `appliedPlanFeedback` 0 -> 9 | 2.33.1 (0.66.58); TUI typing UI 2.34.0 | gate on (plan approvals) | new state field + prompt injection; **client mechanism unchanged** (free text in `session/request_permission` response `_meta.kiro.rejectionReason` on the reject option) | STATIC | HANDLED: cyril attaches `_meta.kiro.rejectionReason` to `RejectOnce` answers crates/cyril-core/src/protocol/convert/kas.rs:205-224 (test crates/cyril/src/app.rs:8919); `appliedPlanFeedback` on `inspect` state DROPPED (`WireSnapshot` workflow.rs:414-443) | NOTE cyril-4u4a (state field); none for the approval |
| **Plan approval shape** (`session/request_permission` title `Add ${n} to ${name}`, Text content = plan text, `_meta.kiro.{toolId:"update_workflow",workflowPlan,...}`, `askAgainAfterRestart`; `pending_interaction` mirror) | unchanged 0.66.43 -> 0.66.91 (6 `workflowPlan:` sites token-identical) | gate on | — | STATIC | HANDLED (crates/cyril-core/src/protocol/convert/kas/workflow_plan.rs, cyril-8wh0 closed) | — (2.33.0 "compact step tree", 2.33.1 "first option highlighted" are TUI-only) |
| **List rows gain progress**: `_kiro/workflow/list` rows add `stepsCompleted`, `stepsTotal`, `currentNodeId?`, `waitingOnWatch` for running/paused runs (`runProgressOf`; `$ts=["running","paused","failed","pending"]` picks `currentNodeId`) | 2.33.0 (0.66.48) | always | new optional fields | STATIC | DROPPED: `WireRunListEntry` workflow.rs:588-618; `/workflow list` prints started_at only crates/cyril-ui/src/workflow_format.rs:125 | NOTE cyril-4u4a / cyril-n8pr |
| **fileCheck `jsonPath` grammar**: evaluator `asu` uses `LHr(path).keys` (leading `$` skipped, so `$.verdict` now resolves; 0.66.43 split on `.` only, so `$.verdict` looked up key `$` and never matched); load-time validation rejects `[`, `]`, `*`, `..` with `WorkflowFileCheckJsonPathUnsupportedError` ("use a dotted key path such as 'verdict' or '$.verdict'") for `stopCondition.fileCheck` and step `completion.fileCheck` | 2.33.0 (0.66.48) | always | validation + semantics | STATIC; causal link to "review loops stop once the reviewer approves" is [INFERENCE] (bundled recipes use `jsonPath:"verdict"`, so the bug hit `$.`-prefixed user/creator recipes) | n/a (recipe authoring); `_kiro/workflow/new` rejection text reaches cyril as an error reply (HANDLED generically) | NOTE cyril-inwg (skill validator must accept `$.`, reject `[ ] * ..`) |
| **`github-pr` watch handler**: `gh pr view --json` field list adds `mergeable,mergeStateStatus`; two GraphQL queries (`reviewThreads` -> `threadId` per inline comment; `baseRef.target.oid` base tip); cursor gains `conflictHeadSha`/`conflictBaseSha`; wakes once per head+base while `CONFLICTING`; payload adds `mergeable`, `mergeStateStatus`, `threadId`; telemetry `workflow.watch.github_pr.{base_tip,base_tip_fetch_failed,conflict,review_threads,review_threads_fetch_failed}`. `wf-pr-responder` prompt: merge the base on a conflicting state, resolve inline threads via `gh api graphql` after replying | 2.33.0 (0.66.48) | always | handler payload | STATIC | n/a (payload rides `{{id.output}}`, not the `watch_poll` event; `WireWatchPoll` workflow.rs:314-329 models outcome/at only) | NOTE cyril-inwg |
| **Watch handler set** `background-process`, `github-pr`, `command` (3 `defaultPollIntervalSec` registry entries) | unchanged | — | — | STATIC | HANDLED (`listWatchHandlers` reply parsed generically) | — |
| **Restart continuity**: `classifyInterruptedNodes` (0.66.62) finishes/fails paused steps whose `completionSignal` was persisted before shutdown without re-running them (`workflow.classify.step_finished_from_signal`, unsaved hand-off summaries written, parent notes queued), then `runLoop` emits one `node_complete` per classified node before continuing; pending hand-off / step update restore is 0.66.43 | 2.34.0 (0.66.62) | always | behaviour; extra `node_complete` burst on resume | STATIC | HANDLED (tracker applies `node_complete` in any order, crates/cyril-core/src/workflow.rs:874) | — |
| **Auto-wake: `wakePromptMeta(session, reason)`** = `{agentInitiated:true, agentInitiatedReason, ...host.automaticTurnProfiling(session)}` (client ttft/perf/contextBreakdown opt-ins now ride wake prompts); reasons `send-message-wake`/`workflow-complete-wake` unchanged; `syntheticUserMessageReason` gains `fallback-continuation` (mid-turn) | 2.33.1 (0.66.58) | — | internal | STATIC | n/a (wake turns arrive as ordinary agent-initiated turns; labels crates/cyril-core/src/workflow.rs:1261-1304) | — |
| **Auto-wake admission (3.1.0 "prompt sent as a run finishes is no longer dropped")**: `autoWakeParentOnComplete` now tests `wakeTargetIsBusy(parent)` = `hasRunningTurn || pendingWakePrompts.has || promptSetupInFlightCount>0 || promptPersistenceInFlightCount>0` and, when busy, appends the completion to the steering buffer (`workflow.emitNotification.parent_busy_queued`) instead of racing a wake prompt; `rewakeWithInfo` guard `promptSetupInFlightCount>0` -> `host.hasUnstoppedPromptInSetup(session)` (a user prompt still in setup and not aborted blocks the wake) | 3.1.0 (0.66.91) | always | behaviour | STATIC (guards readable; user-facing effect body-unproven) | HANDLED by the existing busy-parent steering path (cyril-lki9 B5 notice, crates/cyril-ui/src/turn_labels.rs:59-87) | live lead WF-L4 |
| **Auto-wake trigger set** (`admitWake`): step `info` wakes (idle parent), step `warning`/`error` wake, parent->step wakes unless parent running/terminal, **peer** always wakes; repeat-cap note unchanged (`repeatCapNote`) | 0.66.43 (+peer 0.66.91) | — | unchanged for workflows | STATIC | HANDLED (cyril-lki9/6lql closed) | — |
| **`_kiro/session/notify`**: `sender` enum `["parent","step"]` -> `["parent","step","peer"]`; builder `buildSessionNotifyParams` adds `notifyId` and the tool adds `deliveryCount` (list sends). `send_message` tool: `sessionId` ("'parent' ... Required unless sessionIds is given") or `sessionIds[]` (max 16; "a workflow step may list targets only with severity 'info'"), outcomes `delivered|not_authorized|not_found|budget_exceeded|suppressed|cancelled|failed`, per-session budget ("Stop messaging a session that reached its limit"), description variant chosen by `sessionMessagingEnabled` | 3.1.0 (0.66.91) | `settings.sessionTools.messaging.enabled` (default **off**: `G0("sessionToolsMessaging","off")`) | new enum value + fields | STATIC | `peer` => whole message DROPPED with `warn!("unknown workflow message sender; dropped")` crates/cyril-core/src/protocol/convert/kas/notify.rs:73-77 (`WorkflowMessageSender` crates/cyril-core/src/types/workflow.rs:946-948); `notifyId`/`deliveryCount` silently DROPPED (serde); gate NOT-SENT (settings.rs has no `sessionTools`) so unreachable today | NOTE cyril-fb1m (+ cross-ref kas-core `sessionTools`) |
| **Step steering text** now says "call send_message with sessionId \"parent\"" (lifecycle signal), and a finished step gets "This workflow step has already finished (...), so the workflow no longer accepts notifications from it" | 3.1.0 (0.66.91) | — | prompt | STATIC | n/a | — |
| **`workflows` AgentSettings schema** `{enabled, creatorEffort?}` and intake row `workflows:"off"` + `workflowsCreatorEffort` (effort enum): unchanged 0.66.43 -> 0.66.91; 0.66.58 echoes `workflowsCreatorEffort` in session state/`session/new` `_meta` and `readCreatorEffortSetting` became per-session | 0.66.43 / 0.66.58 | — | schema unchanged | STATIC | `enabled` HANDLED crates/cyril-core/src/protocol/kas/settings.rs:140-145; `creatorEffort` NOT-SENT (TUI 3.1.0 does not send it either: schema `nbr` only) | — |
| **New AgentSettings keys** `sessionTools:{awareness,messaging,cloud}`, `universalBackgroundTool` | 3.1.0 (0.66.91) | off | new keys | STATIC | NOT-SENT (settings.rs) | kas-core lane (cyril-53qx) |
| **`workflowNotifications{enabled,delivery}` + `_kiro/session/setWorkflowNotificationDelivery`**: TUI 3.1.0 still sends both; KAS has 0 occurrences of either in all six carves (dead since <=2.26.0) | — | — | still dead | STATIC | NOT-SENT (correct) | — (already in docs/kiro-2.31.1-wire-audit.md) |
| **Model tools**: `run_workflow` description edits only (2.35.0: brief must be "the task, absolute paths, decisions already made, constraints, whether a worktree is needed, and the check that proves the work done, each once"; 3.1.0: minor); `update_workflow`, `save_workflow_definition`, `inspect_workflow` descriptions byte-identical (3734/1468/855 chars) across all six; schema of `run_workflow` (`workflowPath` nullish | `workflowPrompt` nullish, `inputs`, `runLabel`, ...) unchanged | 2.35.0 / 3.1.0 | gate on | prompt text | STATIC | n/a (model tools) | — |
| **Request handlers** `new`, `invoke`, `load`, `inspect`, `list`, `pause`, `cancel`, `delete`, `listRecipes`, `listWatchHandlers`, `resumeAll`, `resume`, `retry`, `update`: token-identical apart from renames/keepNames; `list` passes `workflow.steps` into the row builder (progress fields); `resume` still takes `extendRepeat{nodeId,additionalIterations}`; `new` still takes `workflowPath|workflow` (inline recipes cannot contain `include`), `inputs`, `parentSessionId`, `workspacePaths`, `runLabel`, `modelId`, `effortLevel` — no `workflowPrompt` on the client method | unchanged | — | — | STATIC | HANDLED (crates/cyril-core/src/protocol/domain_mediator/commands/kas.rs:308-350) | — |
| **Telemetry only**: idle-run scanner (`workflowRun.idle` after 24 h, reasons `paused_no_resume`/`no_progress`), run failure kinds (`model_unavailable`, `model_request_invalid`, `model_response_bad`, `network_error`, `recipe_invalid`, `unknown`; `workflow.node.failed_without_failure_kind`), trust-classifier strike dims | 2.33.0 (0.66.48) | — | metrics, not wire | STATIC | n/a | — |
| **Bundled recipes** (autoresearch, feature-pipeline, goal, investigate, publish-pr, ralph, semantic-review-multi-model): byte-identical in all six carves (`diff -rq`) | — | — | — | STATIC | — | — |
| **Bundled agents**: 0.66.48 `wf-pr-responder` (conflicts, thread resolution); 0.66.70 `wf-coder` (read the hand-off, not every file; run the named verification once), `wf-planner` (do not re-read AGENTS.md/steering; input docs are sources; `## Digest` section), `wf-design-reviewer` (revision mode re-checks prior findings), `wf-workflow-creator` (rewrite: compact node reference incl. `include`, validation list, "Shape" replaces the tier table, model/effort inheritance "auto = inherit"); 0.66.91 `wf-coder` (blocker -> `send_message` warning), `wf-planner` (choose one approach; keep the run's existing merge step), `wf-code-reviewer` ("adds no behavior they do not ask for"), `Explore` (c2s_* tools -> `code_index_build`/`code_index_explore`, `execute_bash`, process tools) | see cell | — | prompt/tool content | STATIC | n/a | NOTE cyril-inwg |

## Workflows control plane

**Method set (requests), identical in all six carves** (`Fmr` dispatch table in 0.66.91, `QGr` in 0.66.43): `new`, `invoke`, `load`,
`inspect`, `list`, `pause`, `cancel`, `delete`, `listRecipes`, `listWatchHandlers`, `resumeAll`, `retry`, `update`, `resume`. Kiro's TUI 3.1.0
calls `cancel, inspect, invoke, list, listRecipes, load, new, pause, resume, retry` (no `delete`, `update`, `resumeAll`, `listWatchHandlers`);
cyril calls `listRecipes, list, inspect, cancel, resume, load, invoke, new` (crates/cyril-core/src/protocol/domain_mediator/commands/kas.rs:308-350).
Handler bodies: token-identical modulo renames/keepNames (region diffs of all 14; see Method notes). Request schemas unchanged: `new`
`{workflowPath|workflow, inputs, parentSessionId, workspacePaths?, runLabel?, modelId?, effortLevel?}` (inline `workflow` may not contain
`include`; `bundled://`, `agent://`, `generated://` and absolute recipe paths; `workflowPrompt` is model-tool only), `resume`
`{workflowId, extendRepeat?{nodeId, additionalIterations}, initiator?:"user", reason?}`, `cancel {workflowId, targetStatus?, initiator?, reason?}`.

**Event set, identical in all six carves** (`kind:"<x>",payload:` literals): `run_start`, `node_start`, `node_complete`, `node_paused`,
`loop_iteration`, `watch_poll`, `paused`, `run_complete`, `steps_queued` (+ `node_failed` only as a method literal, as in 0.66.43) and the
global `recipes_changed` (emitter unchanged: `{sessionId, workspacePaths[], recipes[]}`, gated on `workflowsEnabled`). The bridge still adds
`attention[]` to every event (0.66.43). `node_complete` has one emit site now (the inline step-end emit folded into `emitNodeComplete`).

**Field diffs** (OLD -> NEW):
- `node_complete`: `{workflowId,nodeId,nodePath,status,artifacts?,capturedOutput?,failureReason?}` **+ `credits?[{usage,unit?,unitPlural?}]`** (0.66.91).
- `run_complete`: `{workflowId,status,finalState,parentSessionId?,initiator?,initiatorReason?,attention}` **+ `credits?` (sum by unit)** (0.66.91).
- node snapshot (in `initialState`/`state`/`finalState`, `inspect`, `load`): **+ `credits?`** (0.66.91); run state **+ `appliedPlanFeedback?[]`**
  (0.66.58), `workflowLaunch` is session metadata (not run state).
- `list` rows: **+ `stepsCompleted?`, `stepsTotal?`, `currentNodeId?`, `waitingOnWatch?`** (0.66.48; only for running/paused runs).
- `inspect`: `{workflowId, state, pendingSteps?}` unchanged (0.66.43 `pendingSteps`); `load` subscribes and returns `{workflowId, state, nodePlan}` unchanged.
- `_kiro/session/notify`: **`sender` adds `peer`; + `notifyId?`, `deliveryCount?`** (0.66.91).
- `session/new`/`session/load` result: `configOptions` **+ `{type:"select",id:"workflowLaunch",...}`** when workflows are gated on (0.66.58);
  `_meta` **+ `workflowsCreatorEffort?`** (0.66.58; TUI schema `workflowsCreatorEffort:hr(["low","medium","high","xhigh","max"])`).

**Approval shapes.** Plan approval (`update_workflow replace_remaining`): title `Add ${P} to ${name}`, `content` = plan text,
`_meta.kiro.{toolId:"update_workflow", workflowPlan:{workflowId,nodeId,kind:launch|replace|edit,workflowRef,keptStepIds,steps[],omitted?}}`,
`askAgainAfterRestart` when a hand-off is pending; options accept/always-accept/reject/always-reject — **unchanged 0.66.43 -> 0.66.91**. Typed
suggestions travel as the reject response's `_meta.kiro.rejectionReason` (free text; read only for a `reject_once`/`reject_always`-free reject,
`oje()`), persisted on `interaction_resolved.rejectionReason` (0.66.58) and re-surfaced as `appliedPlanFeedback`. Step-session tool approvals
unchanged; `Permission flow exceeded 20 approval rounds` text present 2 = 2 (cyril-w55l loop structurally unchanged).

**Settings keys.** `initialize.clientCapabilities._meta.kiro.settings.workflows{enabled,creatorEffort?}` + `.goal{enabled}` (gate; unchanged),
`.workflowNotifications{...}` (dead), new 0.66.91 `.sessionTools{awareness,messaging,cloud}{enabled}` and `.universalBackgroundTool{enabled}`
(kas-core). **Per-session**: `session/set_config_option {configId:"workflowLaunch", value:"when-asked"|"large-tasks"|"delegate-all"}`
(0.66.58). Prompt block (0.66.58, verbatim head): "# Workflow launch setting / Follow the rule for the value named by \"Workflow launch
setting:\" in the last `<session_context>` block. / This setting governs new launches; keep managing workflows that are already running. /
## When asked — Start a workflow only when the user asks for one; otherwise do the work yourself. / ## Large tasks only — Start a workflow on
your own only for large tasks that need multiple steps or would fill this session's context; do the rest yourself. Start one whenever the
user asks. / ## Delegate all work — You are the top-level orchestrator ... **always** create and launch a workflow ...".

## Recipe schema + bundled recipes/agents deltas

- **Recipe zod (`Ep`, was `Od`)**: key sets identical in 0.66.43 and 0.66.91 — top level `name, description?, inputs (default {}), steps,
  modelId?, effortLevel?, injectOriginalUserRequest?, planRevision?`; `step {type,id,agent,prompt,artifacts?,captureOutput?,completion?,modelId?,
  effortLevel?,input(rejected)}`, `sequence {type,id,steps,includedFrom?}`, `repeat {type,id,steps,maxIterations<=1000,stopCondition?|stopWhen?,
  onMaxIterations}`, `parallel {type,id,branches,joinPolicy}`, `watch {type,id,handler,config?,idleTimeoutSec?}`; `fileCheck` strict
  `{path,jsonPath,value}`; `StopCondition {containsText?|fileCheck?|completionSignal?}`. **No updated schema JSON is produced**: the zod did not
  change in this window. Pre-existing gaps of `docs/kiro-kas-workflow-recipe-schema-2.26.0.json` stand (no `include` node, which is expanded
  before validation by `expandWorkflowIncludes`; `includedFrom` only as runtime bookkeeping) — cyril-inwg.
- **Semantics added without a zod change**: load-time `WorkflowFileCheckJsonPathUnsupportedError` for `[`, `]`, `*`, `..` in any `fileCheck.jsonPath`;
  evaluator accepts `$.a.b`; `duplicate_node_conflict`/"Merged nodes entries" for include merging (0.66.48).
- **Bundled recipes**: 7, byte-identical across all six carves (`tmp/recipes/<ver>/*.js`, `diff -rq` silent). The 2.32.0 note stands
  (feature-pipeline loops `maxIterations:2, onMaxIterations:"pause"`, `jsonPath:"verdict"`).
- **Bundled agents** (26 profiles in every carve; `tmp/agents/<ver>/`, diffs in `tmp/agents-diffs.txt`): changed sets listed in the Deltas
  table. `wf-workflow-creator` 0.66.70 rewrite is the one that matters for authoring: `include {type,id,path,inputs}` documented, validation
  list ("nesting over 8 levels, more than 50 step nodes (included steps counted), duplicate id, step without prompt, repeat with both
  stopCondition and stopWhen, stopWhen naming an unknown watch id, fileCheck.path outside the workspace ..."), "`auto` means inherit",
  worktree/artifact-dir setup steps sized `effortLevel:"low"`. No agent gained or lost a tool between 0.66.43 and 0.66.91 except `Explore`
  (c2s_* -> code_index_*; not a workflow agent).
- **Workflow-step agent prompt wrapper** (engine-injected): lifecycle signal text now names `sessionId "parent"` (0.66.91).

## ADR-0013 drift check

| surface | KAS 0.66.43 -> 0.66.91 | Kiro TUI 3.1.0 | cyril HEAD | drift |
|---|---|---|---|---|
| `workflow-from-prompt` recipe (`zSo`/`buildCreatorRunWorkflow`): `{name:"workflow-from-prompt", inputs:{workflow_brief:"prompt"}, steps:[{type:"step", id:"create-workflow", agent:"wf-workflow-creator", prompt:<wrapper>}]}` | identical text (constants `UTe/aze/hzr/BSo` -> `dIe/AKe/Yur/VWo`) | n/a | `/workflow new` not implemented yet (cyril-0asq open); only labels reference the names (crates/cyril-ui/src/turn_labels.rs:440-443) | none yet; shape to copy unchanged |
| creator brief wrapper (`renderCreatorBrief`): `<workflow_prompt>…</workflow_prompt>`, run-context lines (workspace root(s), default model/effort), "Design the workflow this brief asks for ... 1. Save the definition with save_workflow_definition until it returns saved: true. 2. Call update_workflow ... replace_remaining ... workflowRef ... This run's only input is workflow_brief ..." | identical | n/a | n/a | none |
| gate marshal `settings.workflows{enabled}` + `settings.goal{enabled}` from `chat.enableWorkflows` | engine reads `workflows.enabled`; `goal` separate gate | `o_()={enabled:Bo(CHAT_ENABLE_WORKFLOWS,false)}` (the 2.32.0 `available:rollout("workflows")` half is gone); `k_r`: `e.workflows={enabled}, e.goal={enabled}`; also sends dead `workflowNotifications` | settings.rs:140-145 sends both; omits `workflowNotifications` | none |
| **`chat.workflowLaunch` -> `set_config_option workflowLaunch`** right after `session/new` when the option is offered (`DBt(Xk(options), saved)`), `/settings` "Workflow use" | option 0.66.58 | sends | NOT-SENT / option DROPPED | **drift (new row)** |
| `workflows.creatorEffort` | accepted | not sent | not sent | none |
| TUI `/workflow new` canned prompt ("Create a reusable workflow recipe for this goal ... Your FIRST action must be delegating to the `wf-workflow-creator` agent ...") | n/a | unchanged 2.32.0 -> 3.1.0 | ADR-0013 chose the inline-recipe launch instead | none |
| gate-off reachability of `_kiro/workflow/*` | unchanged (no `workflowsEnabled` check on handlers) | — | relied upon by `/workflow run`, `/review` | none |

## Unreleased / dark candidates

1. **Cloud-sandbox workflow steps (0.66.91)**: prompt "### Workflow Step Environment — You are running a workflow step in a Kiro Web cloud
   session's internal sandbox. Repositories are Brazil packages on GitFarm; there is no GitHub here ... no `brazil_*` or `crux_*` tools ...
   The top-level session creates the CRUX code review after the workflow." (AWS-internal Kiro Web; `SANDBOX_SPEC_EXTERNAL` sibling).
2. **Peer session messaging** (`sessionTools.messaging`, `sender:"peer"`, `sessionIds[]`, delivery budget) — off by default; kas-core owns `sessionTools`.
3. **`workflows.creatorEffort` / `workflowsCreatorEffort`** — accepted and echoed since 0.66.58, no client sends it.
4. **`workflowNotifications` / `_kiro/session/setWorkflowNotificationDelivery`** — TUI-side only; absent from KAS.
5. **Idle-run telemetry** (`workflowRun.idle`, 24 h) and run-failure-kind classification — metrics only.
6. `cloudWorkflowsDir`/`isCloudWorkflowPath` ("cloud recipe root", 0.66.43) — keepNames made the functions visible; no new behaviour found.

## Leads for the live lane

All legs: fresh temp `HOME`, real `XDG_DATA_HOME`, direct `node acp-server.js` from the 3.1.0 carve on `~/.local/share/kiro-research/binaries/3.1.0/node`,
paired same day with the 2.32.0 carve where noted. Positive control on every leg: `initialize.clientCapabilities._meta.kiro.configurationState:true`
(expect `_kiro/configuration/state`). Fixtures: the one-step shell recipe of `probe-kas-leads-2.32.0.py interrupt` (`sleep`/`echo` step) and the
`linear`/`loop` recipes written by `probe-kas-workflow-2.29.1.py`; capture frames must be cited as `<capture>.jsonl#<N>"<expect>"`.

- **WF-L1 step credits (gate off, 1 run).** `_kiro/workflow/new {workflowPath:<one-step shell recipe>, inputs:{}, parentSessionId, workspacePaths}`
  + `invoke`. Expect NEW: `_kiro/workflow/node_complete` with `credits:[{usage:<n>,unit?,unitPlural?}]`, `_kiro/workflow/run_complete` with
  `credits:[...]` (sum), `inspect` reply `state.root.children[0].credits`; OLD 0.66.43: no `credits` key. Record units (expect `credit`/`credits`
  per `normalizeMeteringUnit`) and compare the sum with the step session's `session_info_update` metering (kas-core lane: `session_usage`).
- **WF-L2 `workflowLaunch` option (gate on, 0-1 turns).** `initialize.clientCapabilities._meta.kiro.settings.workflows:{enabled:true}` and
  `.goal:{enabled:true}`. Expect `session/new` result `configOptions` to contain `{type:"select",id:"workflowLaunch",name:"Workflow use",
  currentValue:"when-asked",options:[when-asked,large-tasks,delegate-all]}`; `session/set_config_option {sessionId, configId:"workflowLaunch",
  value:"delegate-all"}` echoes `currentValue:"delegate-all"`; kiro.log shows no `unknown-key`. Control: gate off => no `workflowLaunch` option.
  Then `session/load` a session created by the 2.32.0 carve (same HOME) with the gate on: expect the option absent/legacy and, with one cheap
  prompt ("what is your Workflow launch setting?"), the model reporting "Delegate all work".
- **WF-L3 plan feedback (gate on, creator run).** Prompt the parent to `run_workflow` with `workflowPrompt`; answer the first plan approval with
  `{outcome:{outcome:"selected",optionId:"reject"}, _meta:{kiro:{rejectionReason:"use python, not node"}}}`; expect a second approval whose plan
  reflects it, then `inspect` `state.appliedPlanFeedback:[{text:"use python, not node",creatorSessionId,toolCallId,consentRound?,topLevelStepIds[]}]`
  and the first step session's opening user message containing `<approved_plan_feedback>`. OLD 0.66.43: no `appliedPlanFeedback`.
- **WF-L4 wake race (gate off, 2 runs).** One-step recipe; on receiving `node_complete` send a `session/prompt` to the parent immediately. Expect NEW:
  the prompt runs and the completion arrives as steering (`workflow.emitNotification.parent_busy_queued` in kiro.log, notice mid-turn, no second
  `turn_start` without a prompt); OLD: a dropped prompt or an unsolicited `workflow-complete-wake` turn. Repeat 3x per arm (timing-sensitive).
- **WF-L5 jsonPath grammar (gate off, 0 turns + 1 run).** `_kiro/workflow/new` with an inline recipe whose repeat has `fileCheck:{path,jsonPath:"a[0]",
  value:true}`: NEW expect an error naming `WorkflowFileCheckJsonPathUnsupportedError`/"unsupported '['"; OLD accepts. Then a repeat with
  `jsonPath:"$.done"` and a step that writes `{"done":true}`: NEW stops after 1 iteration, OLD runs to `maxIterations`.
- **WF-L6 list progress (rides L1).** `_kiro/workflow/list {sessionId}` while the run is `running`: expect `stepsCompleted`, `stepsTotal`,
  `currentNodeId`, `waitingOnWatch:false`; after `run_complete` the keys are absent.
- **WF-L7 wake reply length (passive, rides L1/L4).** Record the `workflow-complete-wake` turn's streamed text length on OLD and NEW for the same
  recipe (2.33.0 "replies Kiro starts on its own no longer cut off" — not located statically).
- **WF-L8 peer notify + cyril adapter (gate on, 1 turn).** `initialize.clientCapabilities._meta.kiro.settings.sessionTools:{messaging:{enabled:true}}`,
  two sessions on the same workspace; prompt one to `send_message` the other with `sessionIds:[<other>]`. Expect `_kiro/session/notify
  {sender:"peer", notifyId, deliveryCount?}` and the tool result listing `outcome:"delivered"`. cyril-on-KAS arm: expect the WARN "unknown workflow
  message sender; dropped" (notify.rs:73-77).
- **WF-L9 trust gating of recipes (gate off, 0 turns; kas-core owns trust).** `_kiro/workflow/new {workflowPath:<untrusted workspace recipe>}` on NEW:
  record whether the reply is the 0.66.43 text "the workflow recipe at '…' is not available in this session's configuration." or a trust refusal.
- **WF-L10 branch landing (optional, expensive).** A creator run whose brief says "needs a worktree": compare where the coder's commits land on OLD vs
  NEW and capture the creator's `<session_context>`/brief lines for branch information (3.1.0 "land changes on your checked-out branch" — not
  located statically in KAS or TUI).

## Changelog reconciliation

Own-version entries only (each `feed-<ver>.json` also repeats the previous version's entry). "0.66.43" = shipped in 2.32.0's KAS and
re-announced here (feed lag, see docs/kiro-2.32.0-wire-audit.md §4).

| feed | line | location |
|---|---|---|
| 2.33.0 | Kiro asks you to trust a folder before it loads that folder's hooks, agents, MCP servers or workflows | kas-core lane (trust); `_kiro/workflow/new` refusal text for out-of-force recipes unchanged since 0.66.43 — WF-L9 |
| 2.33.0 | Workflow plan approvals show a compact step tree with expandable details | TUI-only; `workflowPlan` builder/approval shape unchanged |
| 2.33.0 | Workflow GitHub PR watches now act on merge conflicts, failed checks, and answered threads | 0.66.48 `github-pr` handler (`mergeable,mergeStateStatus`, reviewThreads/baseRef GraphQL, conflict re-wake) + `wf-pr-responder` prompt |
| 2.33.0 | Replies Kiro starts on its own, such as after a workflow finishes, no longer cut off after a few words | not located: wake path and `promptAgentInitiated` token-identical 0.66.43 -> 0.66.48; candidate TUI rendering or non-workflow turn code — WF-L7 |
| 2.33.0 | Windows workflows no longer reject recipe files whose drive letter case differs | not located (no literal change; code-only) |
| 2.33.0 | Workflow review loops now stop once the reviewer approves instead of running to their limit | 0.66.48 fileCheck `jsonPath` parser (`$.` prefix resolves; `[ ] * ..` rejected at load) [INFERENCE on causality] — WF-L5 |
| 2.33.0 | Workflows now run when your organization disables the Auto model instead of failing | 0.66.48 run-failure classification (`model_unavailable` <- `MODEL_TEMPORARILY_UNAVAILABLE`); the step model fallback chain (step -> workflow -> parent) is unchanged, so the "run" half is not located (body-unproven) |
| 2.33.0 | `/workflow run` opens a confirmation form with a plan preview before a workflow starts | TUI-only (`listRecipes` `plan[]` since 2.26.0) |
| 2.33.0 | Workflow coding, planning, and design steps can use Powers | 0.66.43 (`kiro_powers` on wf-coder/wf-planner/wf-design) |
| 2.33.0 | Workflow design steps can use configured MCP servers | 0.66.43 (`includeMcpJson` on wf-design) |
| 2.33.0 | A workflow step's session opens on its task instead of a copy of the parent conversation | 0.66.43 |
| 2.33.0 | Creating a workflow from a prompt shows its plan for your approval before any step runs | 0.66.43 (creator-as-step) |
| 2.33.0 | Running workflows accept plain-language changes to their remaining steps | 0.66.43 (`update_workflow workflowPrompt`) |
| 2.33.0 | Workflow review loops pause after two rejected reviews and ask you how to continue | 0.66.43 (feature-pipeline `maxIterations:2,onMaxIterations:"pause"`) |
| 2.33.0 | A paused step in a parallel workflow branch finishes when it reports success or failure | not located precisely (0.66.43/0.66.48 code-only; keepNames `parallelBranchLeaves` visible only in 0.66.91) |
| 2.33.0 | Buffered workflow messages survive an agent restart | 0.66.43 (`notification.bufferOnly`) |
| 2.33.0 | Completed workflow steps can send follow-up information to their parent | 0.66.43 (`info` from a finished step) |
| 2.33.0 | Workflow pause messages no longer say a run will resume on its own | 0.66.43 ("resuming the run retries the step") |
| 2.33.1 | When Workflows are enabled, a workflow now starts only when you explicitly ask for one | 0.66.58 `workflowLaunch` default `when-asked` + `# Workflow Orchestration` rewrite |
| 2.33.1 | Workflow plan approvals open with the first offered option highlighted, so Enter selects it | TUI-only |
| 2.34.0 | Type suggested changes directly into a workflow plan approval | TUI UI over the existing `_meta.kiro.rejectionReason`; engine side (0.66.58) persists it and builds `appliedPlanFeedback` |
| 2.34.0 | Workflow use under Features in `/settings` to choose when Kiro starts workflows on its own | TUI `chat.workflowLaunch` -> `session/set_config_option workflowLaunch` (engine option 0.66.58) |
| 2.34.0 | Workflows pick up where they left off after a restart, without replanning or repeating steps | 0.66.62 `classifyInterruptedNodes` (+ 0.66.43 pending hand-off/step-update restore) |
| 2.34.0 | Tool approvals in the workflow monitor open on the current call and close with `q` | TUI-only; no approval-shape change 0.66.58 -> 0.66.62 |
| 2.34.1, 2.34.2, 2.35.0, 2.35.1, 2.35.2, 2.36.0, 2.37.0, 2.37.1 | no workflow line | 0.66.70 (2.35.0) nevertheless rewrote wf-coder/wf-planner/wf-design-reviewer/wf-workflow-creator and the `run_workflow` brief text — unannounced |
| 3.0.0 | Kiro CLI V3 is now the default experience, with workflows, specs, cloud sessions and more | policy; KAS 0.66.70 unchanged |
| 3.1.0 | Workflow step rows show the credits each step used | 0.66.91 `credits` on node state / `node_complete` / `run_complete` |
| 3.1.0 | `/usage` and an optional Session credits status line segment show the session credits so far | kas-core (`session_usage`); same `sumCreditsByUnit` as step credits |
| 3.1.0 | A prompt sent just as a workflow run finishes is no longer dropped | 0.66.91 `wakeTargetIsBusy` + `hasUnstoppedPromptInSetup` — WF-L4 |
| 3.1.0 | Workflow runs land their changes on your checked-out branch, not a leftover worktree branch | not located in KAS 0.66.91 workflow code, prompts or recipes, nor in TUI 3.1.0 — WF-L10 |
| 3.1.0 | A turn cut off by the output-token cap or the step limit now says it stopped early | kas-core |
| unannounced | 0.66.48 list-row progress fields, idle-run telemetry, include-merge errors; 0.66.58 `wakePromptMeta`/profiling on wake prompts, `fallback-continuation`, `workflowsCreatorEffort` echo; 0.66.91 `peer` notify + `sessionIds`, step steering text, Orchestration prompt step 3, cloud-sandbox step environment | this document |

## Proposed issues / notes

Existing issues grepped (`.rivets/issues.jsonl`, w1 + "workflow": 92 rows; relevant open: cyril-xehm, cyril-0asq, cyril-4u4a, cyril-fb1m,
cyril-inwg, cyril-w55l, cyril-53qx, cyril-n8pr, cyril-jrs3, cyril-jxw3, cyril-ct2q, cyril-wu87, cyril-zd8u; closed and relevant: cyril-2lh8,
cyril-8wh0, cyril-6lql, cyril-lki9, cyril-sm36).

1. **NEW P2 — "KAS 0.66.91 per-step `credits`: model them and show step/run credits from the engine."** `node_complete.credits[]`,
   `run_complete.credits[]`, snapshot node `credits[]`, item `{usage:number, unit?:string, unitPlural?:string}`. Entry point: every cyril-launched
   run (`/workflow run`, `/review`; gate-off), so P2. Replaces/augments the parent-metering estimate of cyril-2lh8 (crates/cyril-core/src/workflow.rs:487,
   :1319; crates/cyril-ui/src/turn_labels.rs:108-121) and gives `/workflow status` per-step costs (Kiro TUI 3.1.0 parity). Converter:
   crates/cyril-core/src/protocol/convert/kas/workflow.rs:207-218, :259-283, :444-520. Live confirmation: WF-L1.
2. **NEW P2 — "KAS 0.66.58 `workflowLaunch` (Workflow use): honour `chat.workflowLaunch`, expose the option, and mind the legacy default."**
   With `chat.enableWorkflows=true` cyril's sessions always run `when-asked` (engine default) even when the user chose `delegate-all`/`large-tasks`
   in Kiro's `/settings` (`chat.workflowLaunch` in cli.json), and a `session/load` of a pre-0.66.58 session runs **delegate-all** (`legacy`).
   Parity with the TUI: read `chat.workflowLaunch` (settings.rs), send `session/set_config_option {configId:"workflowLaunch"}` after
   `session/new` when the option is offered, and let `KasConfigId` (types/kas_config.rs:20-48) carry it. ADR-0013 decision 1 ("follow the user's
   Kiro setting"); reachable only gate-on (user opt-in), so P2 not P1. Live: WF-L2.
3. **NOTE cyril-4u4a** — add: list-row progress `stepsCompleted/stepsTotal/currentNodeId/waitingOnWatch` (0.66.48), run state
   `appliedPlanFeedback[]` (0.66.58), `run_complete.initiator/initiatorReason` (pre-existing), session/new `_meta.workflowsCreatorEffort` (0.66.58).
4. **NOTE cyril-fb1m** — `_kiro/session/notify` `sender:"peer"` (0.66.91) makes the adapter drop the whole message (notify.rs:73-77); new
   `notifyId`/`deliveryCount`. Unreachable until `settings.sessionTools.messaging` is marshalled (kas-core/cyril-53qx); model the enum value now
   so a future gate flip does not lose messages.
5. **NOTE cyril-inwg** — skill/schema refresh items from this window: `jsonPath` grammar (`$.` accepted, `[ ] * ..` rejected at load), `include`
   node (still undocumented in the 2.26.0 JSON), creator prompt 0.66.70 (compact reference, validation list, `auto` = inherit), `github-pr` payload
   fields (`mergeable`, `mergeStateStatus`, `threadId`, conflict cursor), step steering "sessionId \"parent\"", `credits` on nodes, `send_message`
   `sessionIds` rule for steps.
6. **NOTE cyril-w55l** — the step-session `allow_always` loop is structurally unchanged through 0.66.91 (`Permission flow exceeded 20 approval rounds`
   2 = 2 occurrences; consentRound sites added at 0.66.58 are plan-feedback/attention bookkeeping only).
7. **NOTE cyril-xehm / cyril-0asq** — `workflow-from-prompt` recipe and `renderCreatorBrief` wrapper byte-identical 0.66.43 -> 0.66.91 (no drift to
   copy); the TUI's own `/workflow new` canned prompt also unchanged. New drift row: `chat.workflowLaunch` (issue 2).
8. **NOTE cyril-lki9 / cyril-6lql (closed)** — wake trigger set unchanged; 0.66.91 `wakeTargetIsBusy` queues completions as steering when a prompt
   is in setup/persistence, so the B5 mid-turn notice path will fire more often than a fresh wake.
9. **NOTE cyril-53qx (kas-core)** — new AgentSettings keys `sessionTools{awareness,messaging,cloud}`, `universalBackgroundTool` (0.66.91).

## Method notes

- Carves read: `~/.local/share/kiro-research/kas-carves/{2.32.0,2.33.0,2.33.1,2.34.0,2.35.0,3.1.0}/node_modules/@kiro/agent/dist/server/acp-server.js`
  (12,963,334 / 13,015,888 / 13,196,017 / 15,003,551 / 15,025,207 / 16,099,406 B). 2.34.1/2.34.2 == 2.34.0 and 2.35.1..3.0.0 == 2.35.0 by sha256.
  `dist/server/*.d.ts` mention no workflow symbol (new `discovered-profile`, `node-compile-cache`, `standalone-launcher` are host-facing).
- **0.66.91 is built with keepNames** (`a(fn,"name")` after every function). Consequences: (a) every `_kiro/workflow/<m>` literal +2 with no new
  call site; (b) ~17k "new-only" literals in the 0.66.70 -> 0.66.91 literal delta are function names, not features — a literal appearing only in
  3.1.0 is NOT evidence of newness unless its code/context is new (trap 2); (c) NEW is far easier to read by function name (`a(X,"runCompleteCreditFields")`).
- New scripts (read-only inputs, execute nothing): `static-kas-workflow-sets-3.1.0.py` (vocabulary-set census per carve + set diff),
  `static-kas-workflow-ctx-3.1.0.py` (count in N carves + context), `static-kas-workflow-region-diff-3.1.0.py` (anchored token diff between any two
  carves, `--before`, `--new-anchor`, `--occ`), `static-kas-workflow-agents-3.1.0.py` (bundled profiles per carve),
  `static-kas-workflow-recipes-3.1.0.py` (balanced-brace extraction of the 7 bundled recipe objects per carve). Reused: `static-kas-literals-2.31.1.py`.
- Regenerable bulk under `tmp/`: `sets/` (per-carve JSON + `set-diff.txt`), `literals/delta-*.txt` (OLD->NEW and five consecutive pairs),
  `agents/<ver>/*.md` + `agents-diffs.txt`, `recipes/<ver>/*.js`, `tooldesc/*.txt` (model-tool description templates per carve),
  `consentRound-*.txt`, `FINDINGS-worklog.md`. Small evidence beside this file: `evidence.md` (quoted context per Deltas row).
- Census blind spots to remember: the bridge-level `attention` spread and `L9t`/`She` spreads (`...L9t(t,e)`, `...She(state)`) hide new payload
  keys from `kind:"…",payload:{…}` literal scans — read the emit sites; the `_kiro/workflow/listWatchHandlers` handler is an arrow (`c=>Promise.resolve`)
  so `":async` anchors miss it; the `send_message` and `inspect_workflow` descriptions are built from several templates, so a single-anchor
  template extraction returns the wrong (short) literal — the literal delta files are the reliable source for description changes.
- Two feed lines could not be placed in KAS workflow code (2.33.0 "cut off", 3.1.0 "checked-out branch"); the TUI 3.1.0 bundle has no worktree
  logic either (only gitlens colour keys). Leave them to the live lane (WF-L7, WF-L10) rather than guess.
