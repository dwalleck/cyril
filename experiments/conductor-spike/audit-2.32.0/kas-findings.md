# KAS lane (kas-core + kas-workflow): kiro-cli 2.32.0 audit, KAS 0.66.34 -> 0.66.43

Status: COMPLETE (static). Every row is STATIC; nothing from Kiro was executed. Baseline carve
`kas-carves/2.31.1` vs `kas-carves/2.32.0`; under `@kiro/agent` only `dist/server/acp-server.js`
(12,702,158 -> 12,963,334 B), `acp-server.externals.json` and `package.json` differ (RUN-STATE).
Counts are occurrences in OLD -> NEW bundles (`static-kas-ctx-2.32.0.py`), with context quoted.

## Summary

- No new `_kiro/*` method (set of 123 identical), no new `session_info_update` kind (25 = 25), `sessionUpdate`
  variant, auth method, AB flag or KIRO_FEATURE flag (30 flags, same defaults). The initialize result is unchanged.
  The release changes **payloads, settings and model-facing tools**.
- **Workflows, biggest delta:** a `workflowPrompt` launch now starts a run whose first step `create-workflow`
  (wf-workflow-creator) designs the plan and asks the user to approve it. The approval is an ordinary
  `session/request_permission` carrying the plan as `toolCall.content` text plus a new `_meta.kiro.workflowPlan`
  (toolId `update_workflow`). cyril's approval overlay never renders Text content, so a cyril user would approve a
  plan without seeing it.
- **Every `_kiro/workflow/*` lifecycle notification now carries a top-level `attention[]`**
  (`{kind:"permission"|"question", since, toolCallId?, consentRound?, nodeId, nodePath, sessionId}`): which step session
  is waiting for the user, and on what. The same array is added to run state, `_kiro/workflow/list` rows and
  `run_complete`; `inspect` gains `pendingSteps`. cyril drops all of it.
- **New client capability `initialize.clientCapabilities._meta.kiro.diagrams`** registers a new `create_diagram`
  tool. Its tool_call `_meta.kiro` gains `toolId`, `diagram{id,sessionId,schemaVersion}` and `diagramError`; the
  diagram JSON is written under `<session>/diagrams/dg_<uuid>.json`. Dark for cyril (NOT-SENT).
- **AgentSettings `thinking` removed** (cyril still marshals `thinking:{enabled:true}`, now an unknown key). Thinking
  and effort now live in a "session-reasoning" configuration source. `workflows` gains `creatorEffort`, and new hidden
  string settings `evalStrategy`/`evalTreatments` (plus `KIRO_EVAL_STRATEGY`/`KIRO_EVAL_TREATMENTS` env vars) pin a
  backend A/B treatment through `x-kiro-eval-*` headers.
- **Standing gap found (pre-existing; consequential on the v3 track):** cyril answers `_kiro/terminal/shell_type`
  with `"posix"`. KAS maps that name to dialect `"unknown"`, which makes every shell command opaque to the policy
  engine. The engine then answers `ask` unless shell is denied or unconditionally allowed, so "Always allow" or V3
  rules for a specific command can never auto-approve (STATIC; live lead L1). 0.66.43 also feeds this dialect
  straight into execute_bash approvals.
- Other changes: the outputStyle option list grows from [default, concise] to [default, proactive, concise,
  explanatory, learning]; resumed sessions publish a one-option model/effort select while the catalog is cold; a
  max-tokens "reasoning ate the budget" auto-nudge (1 retry, no new stop reason); a real fish parser; MCP registry
  `oauth`/`oauthScopes`; and `@huggingface/transformers` replaced by inlined tokenizers + `onnxruntime-node` for
  Knowledge semantic search (sharp was only a transitive dependency).

## Deltas (core)

| item | version | gate/default | direction | STATIC/LIVE | cyril HANDLED/DROPPED/NOT-SENT + file:line | proposed issue? |
|---|---|---|---|---|---|---|
| **Client capability `diagrams`**. The resolver `fGr` adds `diagrams:n.diagrams===!0` where `n=e?._meta?.kiro` (`diagrams:` 0 -> 2). When set, `k.diagrams&&Wt.push(new U5e({origin:"acp"}))` registers tool `create_diagram`, and `this.resolvedCapabilities.diagrams&&eFt("newSession")`. KAS logs `[KiroAgent] Resolved capabilities: {...}` at initialize | 0.66.43 | client opt-in; default false | new client capability | STATIC | NOT-SENT: cyril's `_meta.kiro` carries only `settings` and `hooks` (crates/cyril-core/src/protocol/engine.rs:113-134) | NEW P3 "KAS create_diagram: opt into `_meta.kiro.diagrams` and render diagram IR" |
| **`create_diagram` tool** (`"create_diagram"` 0 -> 1; tool-id census 13 -> 14, only addition). Templates `call-hierarchy`, `module-structure`, `type-hierarchy` and others; the input is a diagram IR (`nodes`, `edges`, `groups`, `participants`, `presentation`, …). On success `withOutput({diagramId,sessionId,schemaVersion})` and `writeDiagram` writes `<sessionDir>/diagrams/<id>.json`, id `/^dg_<uuid>$/`. On failure `diagramError:"rejected"|"fault"` | 0.66.43 | gated by `diagrams` | new tool | STATIC | NOT-SENT (gate never set) | same NEW P3 |
| tool_call / tool_call_update `_meta.kiro`: `toolId` set for `disclose_context`/`create_diagram` (`Seu=new Set(["disclose_context","create_diagram"])`), new `diagram:{id,sessionId?,schemaVersion}` and `diagramError`. They are persisted in the user-message meta schema as `diagram:x.object({id,sessionId,schemaVersion}).optional().catch(void 0),diagramError:x.enum(["rejected","fault"])` (`diagramError:` 0 -> 4) | 0.66.43 | only with create_diagram | new fields | STATIC | DROPPED if ever received: `to_tool_call`/`to_tool_call_update` read no `_meta` at all (crates/cyril-core/src/protocol/convert/mod.rs:182-265) | same NEW P3 |
| **AgentSettings `thinking` removed.** Schema `G2n` had `thinking:$l.optional()`; the new `m6n` has none (settings script: `+['creatorEffort'] -['thinking']`). The intake row `{key:Xn.thinking,wire:AI("thinking")}` is gone (`AI("thinking")` 0 -> 0, row removed from `kBu`), and the known-extra list `uBu` does not contain it, so a client `settings.thinking` reaches `configuration.intake.unknown-key` (trap 23) | 0.66.43 | — | key removed | STATIC | NOT-EFFECTIVE: cyril marshals `chat.enableThinking -> thinking` and defaults it on (crates/cyril-core/src/protocol/kas/settings.rs:79, :96). The thinking toggle still works through configId `thinking` (unchanged; applied only when the model is thinking-toggleable) | NOTE on cyril-53qx (settings marshal drift) |
| **Thinking/effort moved to a configuration source "session-reasoning"** (`session-reasoning` 0 -> 1). The class `T2t` reads and writes catalog rows `Xn.effortLevel` (NEW row, `effortLevel:LE("effortLevel",x.string().optional(),void 0,mI,{hidden:!1})`) and `Xn.thinking`, whose row changed from `thinking:km("thinking","off",{hidden:!1})` (default "off") to `thinking:Kce("thinking",{hidden:!1})` (optional, no default). The model's `defaultThinkingEnabled` now decides when nothing is set. With `configurationState:true` both rows should surface in `_kiro/configuration/state` | 0.66.43 | — | relocation + default change | STATIC | NOT-SENT: cyril does not opt into `configurationState` (no match in crates/) | NOTE on cyril-iq2c |
| **`workflows` setting** `$l` (`{enabled}`) -> `Eoa=zhe({creatorEffort:x.string().optional()})`, where `zhe(e)=x.object({...e,enabled:x.boolean()})`. New catalog row `workflowsCreatorEffort:EHe("workflowsCreatorEffort",d6n,{hidden:!1})` over `d6n=["low","medium","high","xhigh","max"]`, wired `QQ("workflows","creatorEffort")`, read by `readCreatorEffortSetting` for the creator step | 0.66.43 | optional | new sub-key | STATIC | NOT-SENT: cyril never marshals `workflows` (crates/cyril-core/src/protocol/kas/settings.rs:466-484, test `workflows_gate_never_marshaled`) | NOTE on cyril-0asq |
| **`evalStrategy` / `evalTreatments`** catalog rows (hidden, string): `evalTreatments:aq("evalTreatments",x.string().regex(ypr),{hidden:!0}),evalStrategy:aq("evalStrategy",x.string().regex(gpr),{hidden:!0})`. Intake `ZJr("evalTreatments")`/`ZJr("evalStrategy")` on `jR=["session/new","session/load"]`; known-extra list gains them. Fallback env `KIRO_EVAL_TREATMENTS` (`k=v;k=v`, max 16 pairs) / `KIRO_EVAL_STRATEGY` (`^[a-z0-9][a-z0-9-]{0,63}$`). `dut()` adds header `x-kiro-eval-treatments` or `x-kiro-eval-strategy` to `GetFeatureConfiguration` **and** `GenerateAssistantResponse`; each strategy gets a pinned `ExperimentConfigService` (log `evalStrategy.experimentConfig.created`) | 0.66.43 | default unset | dark experiment-pin surface | STATIC | NOT-SENT | none (dark; see Unreleased) |
| **outputStyle options** (configId `outputStyle`, `category:"output_style"`, `_meta:{kiro:{promptScoped:!0}}`, all pre-existing): options `[default, concise]` -> `[default, proactive, concise, explanatory, learning]` (`"Learning"` 0 -> 1). The concise instruction was rewritten ("Lead with the result in the first sentence…"). The option is offered only in the `_kiro/config/template` result (`[...Kcn(...),xyo()]`), and the style is chosen per prompt with `session/prompt` `_meta.kiro.outputStyle` (`ofs=[…,"outputStyle"]`) | 0.66.43 | prompt-scoped | new values | STATIC | NOT-SENT: no `outputStyle` anywhere in crates/; cyril does not call `_kiro/config/template` | NOTE on cyril-lyrx |
| **Cold-catalog pin on resume.** `VOl(e)`: `t.length===0?jOl(e):…`, where `jOl` returns a one-option model select `{value:e,name:eie(e)??e}` without `_meta.kiro`. `Kcn` adds a one-option `effortLevel` select when the catalog is empty and a saved effort exists (log `session.config_options.cold_catalog_pin` 0 -> 1). OLD returned no model option while the catalog was cold | 0.66.43 | always | new transient shape | STATIC | HANDLED-degraded: the toolbar `current_value` works, but the effort picker returns `None` because the pinned model option has no effort metadata, and the model picker offers one row until the full `config_option_update` arrives (crates/cyril-core/src/types/kas_config.rs:190-197, :214-262) | none (live lead L4) |
| **`session/request_permission` gains `toolCall.content`** (`...d.content!==void 0&&{content:[{type:"content",content:{type:"text",text:d.content}}]}`) **and `_meta.kiro.workflowPlan`** (`...d.workflowPlan&&{workflowPlan:d.workflowPlan}`; `workflowPlan` 1 -> 12). The emitted-key census over `kiro:{…}` literals: `+['bufferOnly','callerSessionId','workflowPlan']`. The `pending_interaction` session_info now carries `_meta:n.meta` (`{kiro:{workflowPlan}}`). Consent option builder and validation are unchanged (`K1o`/`S8e`/`rSu` token-identical with `N0o`/`D0o`/`Cuu`) | 0.66.43 | workflow plan approvals only | new fields | STATIC | DROPPED: the approval preview renders only Diff content or `raw_input.text/content`, so a Text block shows "Preview unavailable" (crates/cyril-ui/src/widgets/approval.rs:80-131); the permission `_meta.kiro` is read only for `toolId`/`command`/`consent` (crates/cyril-core/src/protocol/convert/kas.rs:24-60), so `workflowPlan` is dropped; `pending_interaction` falls through `_ => None` (kas.rs:602) | NEW P2 (see Workflows) |
| **Permission asks survive restart**: `askAgainAfterRestart` -> `leaveOpen` (log `acp.interaction.left_open`); restored plan approvals are re-asked over `request_permission` after restart (`workflow.plan_approval.restored_answer`, …) | 0.66.43 | workflow plan approvals | behaviour | STATIC | n/a (an ordinary request) | none |
| **`agentHostsForegroundShell`** client meta key (0 -> 4): `Sjr(caps,sandbox,clientMeta.agentHostsForegroundShell)` returns `"agent"` with reason `client_override`, so KAS runs foreground commands natively even when the client advertised `terminal:true` (log `acp.foreground_host.selected` reasons `client_override`/`terminal_capability`/`sandbox`/`no_terminal_capability`) | 0.66.43 | client opt-in | new client meta key | STATIC | NOT-SENT (crates/cyril-core/src/protocol/engine.rs:113-134) | NOTE on cyril-7p7n |
| **Shell dialect for approvals**: execute_bash and control-process approvals now pass `shellType: terminalManager.dialect` / `processManager.dialect`, and the client terminal manager `kue(...)` gets `clientDialect=jh(shellType)`. A dialect mismatch between approval and run throws `InterpreterChangedError` ("The shell changed after approval…"). A new fish parser `gxo` replaces the old heuristic `Veu` (`fish comment inside an unclosed { or [` 0 -> 1). New metrics `policy.shell.{dialect_fallback,interpreter_mismatch,interpreter_changed}`, `policy.cmd.opaque` | 0.66.43 | always | behaviour | STATIC | **Standing gap:** cyril replies `{"shellType":"posix"}` (crates/cyril-core/src/protocol/kas/host_shell.rs:166-171, terminal_io.rs:283-285). `jh()` only knows concrete shell names (`bash`, `zsh`, `sh`, `fish`, `pwsh`, …), so `"posix"` maps to `"unknown"` (`policy.shell.type.unknown` warn). `_xo(cmd,"unknown")` returns `{subCommands:[e],hasSubstitutions:!0}`, and `evaluateShell` then reaches `n.hasSubstitutions&&!n.substitutionsResolved?i()`, where `i()` is `{effect:"ask"}`. Every command not denied asks, so allow rules and Always-allow cannot apply. 0.66.34 behaved the same (`l.shellType??cb(r.shellType)`) | **P1-candidate** NOTE on cyril-auxt (blocks its fix) + NEW issue "shell_type must report a concrete shell name" once L1 confirms |
| **Max-tokens reasoning recovery**: when a model turn ends with only reasoning and no tool use or text, KAS re-invokes once (`pgo=1`) after injecting a human message ("Your previous response used the entire output budget on internal reasoning…"), persisted with `_meta.kiro.syntheticUserMessageReason:"max-tokens-recovery"` (a new enum value; classed `"mid-turn"`). Logs `agent.max_tokens_recovery.{reinvoke,exhausted,reset}`. `stopReason` literals are unchanged (`end_turn` 15, `cancelled` 4) | 0.66.43 | always (no flag) | behaviour + new enum value | STATIC | n/a live; on `session/load` replay the synthetic message may surface as a user chunk (live lead L7) | none |
| **Knowledge embeddings**: `@huggingface/transformers` (`import("@huggingface/transformers")` + `pipeline("feature-extraction","Xenova/all-MiniLM-L6-v2")`, "required for semantic search in the Knowledge tool") -> an inlined `@huggingface/tokenizers` 0.2.0 (BPE/WordPiece/Unigram…) + `import("onnxruntime-node")` `InferenceSession` (cpu), with mean pooling + L2 norm at 384 dims, max 128 tokens. The model assets (`config.json`, `tokenizer.json`, `tokenizer_config.json`, `onnx/model.onnx`) are pinned to HF commit `751bff37…` with sha256 + size checks, fetched from `https://huggingface.co/<model>/resolve/<commit>/` or `KIRO_KNOWLEDGE_MODEL_REMOTE_HOST` (pre-existing, 1 -> 1), cache `KIRO_KNOWLEDGE_MODEL_CACHE_DIR` (1 -> 1). Memory does not use it (memory default `{mode:"disabled",reflection:!1}` 1 -> 1, unchanged) | 0.66.43 | Knowledge tool | dependency swap | STATIC | n/a | none |
| **sharp/@img/protobufjs/long/flatbuffers/guid-typescript/platform/detect-libc removed**: none was imported by acp-server.js (`sharp` 8 -> 8, all in protobuf descriptors or the monorepo `overrides:{sharp:"^0.35.5"}` metadata). They were transitive dependencies of `@huggingface/transformers` (transformers.js pulls in sharp for image pre-processing and onnxruntime/protobuf for model loading). KAS image resizing was already photon-based (`photon` 10 -> 10) | 0.66.43 | — | packaging | STATIC | n/a | none |
| **Oversized-image refit**: a provider image rejection triggers `mechanism==="imageRefit"`, which re-encodes at the provider profile's size limit and reissues (`q.converse.image_refit.reissue`/`.recovered`, `imageRefit` 0 -> 12). It adds a `fitted-images` session dir and the placeholder `[image omitted: the model provider rejected this image]` | 0.66.43 | always | backend retry | STATIC | n/a (not on ACP) | none |
| MCP org registry: the registry remote schema gains `{oauth:…optional(),oauthScopes:x.array(x.string()).optional()}`, merged as `oauth:r.oauth??s.oauth,oauthScopes:r.oauthScopes??s.oauthScopes` (`oauthScopes` 5 -> 11) | 0.66.43 | registry configs | engine config | STATIC | n/a | none |
| MCP elicitation cancelled by the server: when the MCP server aborts, the elicitation resolves `cancel` and logs `mcp.elicitation.cancelledByServer`. KAS does not cancel the client's open `_kiro/mcp/elicitation` request | 0.66.43 | — | behaviour | STATIC | n/a | none |
| Hooks `_kiro/hooks/didChange`: payload `{sessionId,hooks}` and `projectHookForWire` are unchanged. The source is now each session's registry (agent-profile hooks included; mode switches emit `sessionChanged`), coalesced per subscriber (`hooks.didChange.broadcast`) | 0.66.43 | — | emission cadence | STATIC | n/a (cyril's hook adapter unaffected) | none |
| Session index: saved-session listing serves the manifest and revalidates in the background (`sessionIndex.revalidate.{done,skipped}`, `sessionIndex.serve.{nameReconcile,revalidateQueued}`, manifest `stamps`) | 0.66.43 | — | internal | STATIC | n/a | none |
| Session context prompt: client descriptions dropped (`"the Kiro command-line client"` 1 -> 0; names kept) | 0.66.43 | — | prompt text | STATIC | n/a | none |
| c2s / code_to_spec: `analysisOnly` -> `analysisScope` (`structural-only`/`structural-and-analysis`/`full`) + `semanticDepth`; the SQLite analysis store is rewritten (`functions/types/modules/edges/facts/…` tables with JSON `payload`); the Explore agent prompt is updated | 0.66.43 | `_c2s` gate (unchanged) | model-facing | STATIC | n/a | none |
| Telemetry: new gauges `workflowRun.idle`/`workflowRun.inProgress` (`registerGauge`); the common-dimension list is unchanged (`userCohort`, `activeExperiments` pre-existing) | 0.66.43 | `kuts_telemetry` default true (unchanged) | telemetry | STATIC | n/a | none |
| Unchanged (set-diffed, zero delta): `_kiro/*` method literals 123 = 123; ACP methods 25 = 25; `session_info_update` kinds 25 = 25 (`static-kas-session-info-kinds`); `.kiro.X` read census 22 = 22; AB_* 16 = 16; `process.env.*` 70 = 70; feature-flag table 30 = 30 (defaults identical; `flags-old.txt` == `flags-new.txt` except the path line); `authMethods`; `agentCapabilities._meta.kiro` (incl. `extensionMethods`); `_kiro/terminal/shell_type` probe; consent scope validation | — | — | — | STATIC | session_info coverage unchanged: 8/25 handled (crates/cyril-core/src/protocol/convert/kas.rs:448-601), the rest dropped unlogged at kas.rs:602 | NOTE on cyril-58uv |

## Workflows

| item | version | gate/default | direction | STATIC/LIVE | cyril HANDLED/DROPPED/NOT-SENT + file:line | proposed issue? |
|---|---|---|---|---|---|---|
| **Lifecycle notifications carry `attention[]`**: the bridge `lq` emits `c={...o.payload,...parentSessionId,attention:HJ(t.state)}` for every `_kiro/workflow/{run_start,node_start,node_complete,node_failed,node_paused,loop_iteration,watch_poll,paused,run_complete,steps_queued}` (always present, possibly `[]`). The cancel path's direct `run_complete` also gets `attention:HJ(m.state)`. `HJ` flattens node `attention` into `{kind,since,toolCallId?,consentRound?,nodeId,nodePath,sessionId}`. The event literal set (9 `kind:"…",payload:` kinds) and payload key sets are otherwise unchanged | 0.66.43 | workflows | new field on every event | STATIC | DROPPED: serde structs ignore unknown members (e.g. `WireRunCompleted` crates/cyril-core/src/protocol/convert/kas/workflow.rs:202-208; no `deny_unknown_fields` in protocol/) | NEW P2 "Model workflow `attention[]` (step waiting on permission/question)" or NOTE on cyril-4u4a; relates to cyril-5lwp/cyril-w55l |
| **Run state / list schema**: node gains `capturePinned:x.literal(!0)`, `attention:x.array($ir)` (`$ir={kind:x.enum(["permission","question"]),since,toolCallId?,consentRound?}`). State and list rows (`Fhe`) gain `attention:x.array(Uir)` (`Uir=$ir.extend({nodeId,nodePath,sessionId})`); list rows emit it only when non-empty (`...n.length>0&&{attention:n}`). `load`/`invoke` replies recompute it (`Sce(l.state)`) | 0.66.43 | — | new fields | STATIC | DROPPED: `WireSnapshot` (workflow.rs:377-394) and `WireRunListEntry` (workflow.rs:542-556) do not model it | same |
| **`_kiro/workflow/inspect` reply gains `pendingSteps`** (`...f!==void 0&&{pendingSteps:Q0(f.remainingSteps)}`, read from `pending-step-update.json` for non-terminal runs) | 0.66.43 | — | new field | STATIC | DROPPED: `WireStateReply` ignores extra members (workflow.rs:210-220); pending work does arrive via `steps_queued` (HANDLED, crates/cyril-core/src/workflow.rs:750) | NOTE on cyril-4u4a |
| **`steps_queued` re-emitted on restore**: two new emit sites push `{kind:"steps_queued",payload:{workflowId,pendingSteps}}` into `pendingLifecycleEvents` when a run with a persisted pending update or recovered handoff is registered (6 emits, was 4; resolution shapes `applied`/`rejected`/`dropped` unchanged) | 0.66.43 | — | more emissions | STATIC | HANDLED (resolution-free frame replaces pending steps; crates/cyril-core/src/workflow.rs:746-760) | none |
| **workflowPrompt launch = creator step + plan approval** (`run_workflow` description: "The run starts with one step, `${UTe}` [`create-workflow`], in which the workflow-creator agent designs the steps, asks the user to approve them, and adds them to the run"). The creator calls `save_workflow_definition`, then `update_workflow{action:"replace_remaining",workflowRef}`, which raises `request_permission` with title `Add ${P} to ${u}`, `content` = plan text and `_meta.kiro.{toolId:"update_workflow",workflowPlan}`. Options are the standard accept/always-accept/reject/always-reject. OLD ran the creator inside the run_workflow call and relaunched via `generated://` refs (removed: `"generated://<id>" — relaunches…` 1 -> 0) | 0.66.43 | workflows gate | flow change | STATIC | DROPPED: plan text and workflowPlan are not shown (crates/cyril-ui/src/widgets/approval.rs:80-131). cyril's own `/workflow run` sends `workflowPath` only (crates/cyril/src/domain_mediator/commands/kas.rs:350-355), so this arises when the model calls run_workflow with workflowPrompt | NEW P2 "Render KAS workflow plan approvals (Text content + `_meta.kiro.workflowPlan`)"; NOTE on cyril-0asq |
| **`update_workflow` tool**: `remainingSteps` removed (rejected: "update_workflow no longer takes \"remainingSteps\"…"). `replace_remaining` takes `workflowPrompt` (a plain-language change; KAS swaps the remaining steps for an `edit-workflow-r<N>` creator step in Edit Mode, writing a run snapshot `edit-snapshot-r<N>.md`) or `workflowRef` (a saved `generated://` definition, approved by the user). `nodeId` may now name a paused step ("only the owning parent can end a paused step") | 0.66.43 | model-facing | tool schema | STATIC | n/a (model tool) | none |
| **Pending handoff / step update persisted across restart**: `pending-handoff.json` (`{toolCallId,stepSessionId,nodeId,workflowRef,keptStepIds,remainingSteps,additions{runLabel},planText,shapeText?,planRevision?,askedAt}`) and `pending-step-update.json` (`{remainingSteps,keepThroughIndex,additions{inputDeclarations,inputValues,runLabel},handoff{nodeId,summary},planRevision}`) | 0.66.43 | — | durability | STATIC | n/a | none |
| **Buffered messages survive restart**: messages to a session that will not be woken are persisted as user messages with `_meta.kiro.notification.{bufferOnly:true,callerSessionId}` (schema `bufferOnly:x.literal(!0).optional(),callerSessionId:x.string().optional()`). A parent message to a finished step no longer wakes it (`_Su(e,t,r,n){if(e)return n?void 0:{sender:"parent",…}}`) | 0.66.43 | — | behaviour | STATIC | n/a (persisted history; steering ids `notify-` handling unchanged, crates/cyril-core/src/protocol/convert/kas.rs:634-641) | none |
| **Completed steps can message the parent**: `send_message` from a finished step accepts `info` ("This step already finished (completed or skipped); the workflow accepts no success, need-input, or error signal from it. An info message to the parent still works.") | 0.66.43 | — | behaviour | STATIC | `_kiro/session/notify` payload `vPt` unchanged; cyril does not consume it (dropped by the cyril-58uv catch-all) | NOTE on cyril-58uv |
| **New auto-wake trigger: repeat cap**. When a repeat pauses on maxIterations, `sendParentNote(note,"repeat-cap")` delivers "A workflow you launched (\"…\") paused: repeat <id> reached its iteration limit (<n>) without meeting its stop condition…" at severity info, which wakes the parent (`t==="info"&&r` returns `SIo`). `handoff-restored` notes go out at severity `success` (buffered, no wake) | 0.66.43 | repeat with `onMaxIterations:"pause"` | new billed wake | STATIC | n/a: a parent-wake turn arrives as an ordinary turn | NOTE on cyril-lki9 family (closed) and cyril-w55l; P1 only if live shows unsolicited billing on cyril-launched runs |
| **Review loops pause after two rejections**: bundled `feature-pipeline` `design-loop` and `code-loop` go `maxIterations:3,onMaxIterations:"abort"` -> `maxIterations:2,onMaxIterations:"pause"` (`maxIterations:3,` 2 -> 0, `maxIterations:2,` 0 -> 2). The creator prompt default becomes "2 with pause"; wf-planner's convergence loop is "at most 2 iterations that pauses" | 0.66.43 | bundled | recipe content | STATIC | pause handled by the existing `node_paused`/`paused` path (crates/cyril-core/src/workflow.rs:508) | none |
| **Step session opens on its task**: OLD `userMessage=Dbe(prompt, originalUserMessages)` prepended the `<original_user_request>` block; NEW `userMessage=awo(prompt,…)` and passes `originalUserRequest` separately, injected on the first turn as a context text named "Original user request" (`"Original user request"` 0 -> 1) | 0.66.43 | `injectOriginalUserRequest!==false` | behaviour | STATIC | n/a | none |
| **Bundled agents**: new `wf-code-reviewer` (registered in `Zpt`, referenced by no bundled recipe or prompt). `wf-coder`, `wf-planner` and `wf-design` gain the `kiro_powers` tool (`kiro_powers` 18 -> 21); `wf-design` gains `includeMcpJson: true` (49 -> 50). The wf-workflow-creator prompt is rewritten (runs as a step; Edit Mode; "Stop and Ask" via send_message warning; tools + `update_workflow`, `send_message`). The `Explore` prompt learns the store meta keys `analysisDepth`/`semanticDepth`/`completedAt` | 0.66.43 | bundled | prompt/tool content | STATIC | n/a | none |
| Bundled recipe set unchanged: autoresearch, feature-pipeline, goal, investigate, publish-pr, ralph, semantic-review-multi-model | — | — | — | STATIC | — | — |
| Recipe zod schema unchanged (the workflow schema module differs only by minifier rename `of` -> `Od`), so `docs/kiro-kas-workflow-recipe-schema-2.26.0.json` remains valid. `save_workflow_definition` description: the ref is "consumed (deleted) when it is used" and goes "wherever your instructions direct it" | — | — | — | STATIC | — | NOTE on cyril-inwg (skill refresh: `update_workflow` workflowPrompt/workflowRef, creator-as-step) |
| Handlers unchanged (token-diffed): `_kiro/workflow/{new,update,resume,pause,retry,listRecipes,listWatchHandlers,resumeAll}`. `new` already accepted inline `workflow` (unchanged); `_kiro/workflow/update` (client) still takes `remainingSteps` | — | — | — | STATIC | cyril sends `workflowPath, inputs, parentSessionId, workspacePaths` (crates/cyril/src/domain_mediator/commands/kas.rs:350-355) | — |
| Pause texts: "Step paused after a transient error; will resume." -> "…; resuming the run retries the step." (all three variants); new "Model service error (service 5xx or throttling); resuming the run retries the step." | 0.66.43 | — | text | STATIC | displayed verbatim if shown | none |
| Nested workflows refused: run_workflow from a step session returns "Nested workflows are not supported: a workflow step cannot start its own workflow." | 0.66.43 | — | guard | STATIC | n/a | none |

## Feature flags and gates

Full set diffs (OLD -> NEW):

- KAS feature-flag registry (`static-kas-flags-2.29.1.py`): **30 -> 30, identical keys, defaults and env names** (15
  env-reachable). Added: none. Removed: none. Default changed: none.
- `AB_*` identifiers: 16 -> 16 identical. `KIRO_*` tokens: 63 -> 65, **+`KIRO_EVAL_STRATEGY`, +`KIRO_EVAL_TREATMENTS`**.
  `process.env.X` reads: 70 -> 70 (the eval vars are read through `r[PSa]`/`r[MSa]` indexing with `r=process.env`).
- `clientCapabilities._meta.kiro` (top level): **+`diagrams`** (`resolvedCapabilities`), **+`agentHostsForegroundShell`**
  (clientMeta). Existing keys unchanged (`configurationState`, `inactiveItems`, `telemetryEnabled`, `terminalInput`,
  `secretStorage`, `openExternalUrl`, `knowledge`, `infrastructureSafety`, `c2sViews`, `textSearch`, `findFiles`,
  `streamingShellContent`, `settings`, `hooks`, …).
- `clientCapabilities._meta.kiro.settings` (AgentSettings, 49 keys both sides): **−`thinking`**, **+`workflows.creatorEffort`**
  (`workflows` becomes `{enabled, creatorEffort?}`); known-extra keys **+`evalStrategy`, +`evalTreatments`** (string
  settings read on session/new and session/load).
- Settings catalog rows: 51 -> 55, **+`effortLevel`** (visible), **+`workflowsCreatorEffort`** (visible), **+`evalTreatments`**,
  **+`evalStrategy`** (hidden). `thinking` changed from a default-"off" toggle to optional (no default).

| flag / key | what it gates | default | client-flippable via initialize? |
|---|---|---|---|
| `_meta.kiro.diagrams` (NEW) | registers `create_diagram`; enables diagram `_meta` | false | yes: `initialize.clientCapabilities._meta.kiro.diagrams:true` |
| `_meta.kiro.agentHostsForegroundShell` (NEW) | KAS runs foreground shell itself even when the client hosts terminals | false | yes: `initialize.clientCapabilities._meta.kiro.agentHostsForegroundShell:true` |
| `settings.evalStrategy` / `settings.evalTreatments` (NEW, hidden) | pins a backend experiment treatment; headers `x-kiro-eval-strategy` / `x-kiro-eval-treatments` on feature-config + chat calls | unset | yes: `initialize.clientCapabilities._meta.kiro.settings.evalStrategy:"<slug>"` (validated by regex) |
| `KIRO_EVAL_STRATEGY` / `KIRO_EVAL_TREATMENTS` (NEW env) | same as above (fallback when no setting) | unset | env only |
| `settings.workflows.creatorEffort` (NEW) | effort of the `create-workflow` / `edit-workflow-r<N>` creator step | unset (the creator profile says `effortLevel: high`) | yes, inside `settings.workflows` (needs `enabled`) |
| `settings.thinking` (REMOVED) | — (now an unknown key) | — | no; use configId `thinking` |
| configOption `outputStyle` | per-prompt style instruction | `default` | per prompt: `session/prompt` `_meta.kiro.outputStyle` |

## Unreleased/dark candidates

| candidate | gate | default | evidence |
|---|---|---|---|
| `create_diagram` + diagram IR viewer contract (`schemaVersion`, templates `module-structure`, `call-hierarchy`, `type-hierarchy`, …; file `<session>/diagrams/dg_<uuid>.json`) | `clientCapabilities._meta.kiro.diagrams` | off | `diagrams:n.diagrams===!0`; `k.diagrams&&Wt.push(new U5e({origin:"acp"}))`; `withOutput({diagramId:p.id,sessionId:i.chatSessionId,schemaVersion:D5e})`; telemetry `CreateDiagram:{name:"create_diagram",description:"Tracks the create diagram tool"}` |
| Experiment / eval pinning (A/B treatment override) | `settings.evalStrategy`/`evalTreatments`, `KIRO_EVAL_*` | unset | `gYn(e,t,r=process.env){return hYn(e)??fYn(t)??hYn(r[PSa])??fYn(r[MSa])}`; `ASa="x-kiro-eval-treatments",ISa="x-kiro-eval-strategy"`; `dut(a,this.featureConfigRegistry?.evalStrategy)` on `GenerateAssistantResponseCommand` |
| Bundled `wf-code-reviewer` agent | none (registered, unreferenced) | registered | `Zpt=[["wf-auto-researcher",qui],["wf-code-reviewer",Hui],…]`; no `agent:"wf-code-reviewer"` (0) |
| Agent-hosted foreground shell | `_meta.kiro.agentHostsForegroundShell` | off | `Sjr(e,t,r){if(r===!0)return S.info("acp.foreground_host.selected",{host:"agent",reason:"client_override"}),"agent";…}` |
| Native memory | `AB_MEMORY_INTERNAL`/`AB_MEMORY_EXTERNAL` (unchanged) | disabled (unchanged) | `Object.freeze({mode:"disabled",reflection:!1})` 1 -> 1 |

## Leads for the live lane

All legs: fresh temp `HOME`, real `XDG_DATA_HOME`, direct `node acp-server.js` from carve 2.32.0 (pair with 2.31.1 where
noted), same day. Positive control on every leg: `initialize.clientCapabilities._meta.kiro.configurationState:true`
(expect `_kiro/configuration/state` pushes; traps 5/28). KAS debug log: `KIRO_LOG_LEVEL=debug`, file
`$HOME/.kiro/logs/<ts>/kiro.log`.

- **L1 shell_type "posix" defeats approvals (P1-candidate; one or two turns per arm).** Handshake like cyril:
  `initialize.clientCapabilities.terminal:true`, `fs.readTextFile/writeTextFile:true`, `_meta.kiro.configurationState:true`.
  Answer the agent's `_kiro/terminal/shell_type` request with arm A `{"shellType":"posix"}` and arm B (control)
  `{"shellType":"bash"}`. Workspace agent file with V3 rule `permissions.rules:[{capability:"shell",match:["echo AUDITX*"],effect:"allow"}]`
  (shape per the 2.31.1 auxt evidence). Prompt "Run exactly: echo AUDITX". Expected: arm A sends
  `session/request_permission` for the command and kiro.log has `policy.shell.type.unknown`; arm B runs `terminal/create`
  with no permission request. Repeat on 2.31.1 to show it is pre-existing.
- **L2 `diagrams` capability (zero turns, then one).** Arm A `initialize.clientCapabilities._meta.kiro.diagrams:true`, arm B
  absent. Expected: kiro.log line `[KiroAgent] Resolved capabilities: {…,"diagrams":true}` (A) vs `"diagrams":false` (B).
  One prompt ("draw a call-hierarchy diagram of <function>") on A should produce a tool_call titled Create Diagram with
  `_meta.kiro.toolId:"create_diagram"`, a completion `_meta.kiro.diagram:{id:"dg_<uuid>",sessionId,schemaVersion}`, and the
  file `<sessions>/…/diagrams/dg_<uuid>.json`. Record the IR for cyril's renderer.
- **L3 `settings.thinking` intake (zero turns).** `initialize.clientCapabilities._meta.kiro.settings:{"thinking":{"enabled":true}}`
  + `configurationState:true`, on OLD and NEW. Expected: NEW logs `configuration.intake.unknown-key` for `thinking`;
  `_kiro/configuration/state` on NEW lists `effortLevel`/`thinking` items from source `session-reasoning` after a
  `session/set_config_option {configId:"thinking",value:"on"}`; OLD accepts the key. Control: `settings:{"evalStrategy":"audit-probe"}`
  must NOT log unknown-key on NEW (known extra) and should log `evalStrategy.experimentConfig.created` at session/new.
  (Do not set `evalTreatments` to a real experiment name.)
- **L4 cold-catalog pin (passive).** On every `session/load`, record whether the result's `configOptions` model select has
  exactly one option without `_meta.kiro` plus a one-option `effortLevel` select, followed later by a full
  `config_option_update`. Do not forge a slow catalog (trap 13).
- **L5 workflow `attention[]` (one workflow run).** `settings.workflows:{"enabled":true}`; `_kiro/workflow/new`
  `{workflowPath:"bundled://investigate", inputs:{…}, parentSessionId, workspacePaths}` + `_kiro/workflow/invoke`; do not answer
  the first step-session `session/request_permission` for about 10 s. Expected: every `_kiro/workflow/*` notification has
  top-level `attention` (`[]` before the ask); while the ask is open, `_kiro/workflow/list {sessionId}` and `inspect` show
  `attention:[{kind:"permission",since,toolCallId,consentRound,nodeId,nodePath,sessionId}]`; `inspect` `pendingSteps` is
  present only when an update is queued.
- **L6 workflowPrompt plan approval (one parent turn + creator step).** Gate on as in L5; prompt the parent "Use run_workflow
  with workflowPrompt: <small task>". Expected: run starts with node `create-workflow`; the creator session raises
  `session/request_permission` with title `Add <n> steps to <name>`, `toolCall.content[0].content.text` = plan text,
  `_meta.kiro.toolId:"update_workflow"`, `_meta.kiro.workflowPlan:{workflowId,kind:"launch",workflowRef,keptStepIds,steps[…],omitted?}`,
  options accept/always-accept/reject/always-reject; the `pending_interaction` session_info carries `_meta.kiro.workflowPlan`.
  Reject once and confirm the creator asks via `send_message` warning.
- **L7 max-tokens recovery (passive).** Record any user message persisted with `_meta.kiro.syntheticUserMessageReason:"max-tokens-recovery"`
  and whether `session/load` replays it as a `user_message_chunk` (cyril would render it as if the user typed it).
- **L8 repeat-cap wake (opportunistic, rides L5/L6).** If a repeat pauses on maxIterations, expect a parent `turn_start`
  without a client `session/prompt` (billed wake) and a `_kiro/session/notify` with the repeat-cap text.

## Changelog reconciliation

KAS 2.31.1 (0.66.34) -> 2.32.0 (0.66.43) is a real code delta, so every [V3] line can be KAS code.

| line | location |
|---|---|
| `--cloud`/`--repo` in non-interactive runs | not my lane (host/TUI) |
| OSC 7501 program status; `KIRO_NO_PROGRAM_STATUS` | not in KAS (`KIRO_*` set gains only KIRO_EVAL_*) -> TUI/host |
| [V3] Write approvals allow a custom folder at session/workspace/user scope | KAS consent path unchanged (`S8e`: `resource:w?.resource??…`, `scope:w?.scope||"session"`; `rSu` scope ∈ session/user/workspace). The custom folder rides the existing `_meta.kiro.consent.{scope,resource}` on the response -> TUI composes it |
| [V3] `/workflow run` confirmation form with plan preview | no new method; `listRecipes` rows already carry `plan[]` (cyril-4u4a) -> TUI |
| [V3] Proactive, Explanatory, Learning output styles | KAS outputStyle option list +3 (Deltas) |
| [V3] Workflow coding/planning/design steps can use Powers | wf-coder/wf-planner/wf-design +`kiro_powers` |
| [V3] Workflow design steps can use configured MCP servers | wf-design +`includeMcpJson: true` |
| Startup "What's new" introduces memory | not my lane (TUI); KAS memory unchanged |
| [V3] Always Allow scope page shows scope, `s` cycles | not my lane (TUI) |
| V2 subagents follow model fallback | not my lane (v2 host) |
| [V3] Step session opens on its task | runner passes `originalUserRequest` as a first-turn context text instead of prepending it (Workflows) |
| [V3] Creating a workflow from a prompt shows its plan for approval | creator-as-step + `update_workflow` plan approval via `request_permission` + `workflowPlan` (Workflows) |
| [V3] No longer bundles `sharp`/libvips | externals −`@huggingface/transformers` +`onnxruntime-node`; sharp et al. were transitive dependencies of transformers (Deltas) |
| [V3] Running workflows accept plain-language changes | `update_workflow{replace_remaining, workflowPrompt}` -> `edit-workflow-r<N>` creator step in Edit Mode (Workflows) |
| [V3] Saved-session lists refresh without blocking | session index background revalidation (Deltas) |
| [V3] Review loops pause after two rejected reviews | feature-pipeline loops 3/abort -> 2/pause; creator default; repeat-cap parent note (Workflows) |
| [V3] `/chat save` zip naming; `/chat load` altered archive | not in KAS (`session archive` 0 -> 0; `_kiro/session/export` unchanged) -> TUI/host |
| [V3] Non-interactive `--resume-id` cloud | not my lane (host/TUI) |
| MCP OAuth links disappear on connect | KAS adds only `mcp.elicitation.cancelledByServer`; link UI = TUI |
| Plain-text non-interactive explains cancel/refusal; stdout closed exits nonzero | not my lane |
| V2 `/model` fallback None; fallback row names vendor default; V2 crew stage error | not my lane (v2/TUI) |
| [V3] Paused step in a parallel branch finishes on success/failure | `update_workflow` `nodeId` may name a paused step; `workflow.statusUpdate.paused_step_finished` |
| [V3] Turn no longer ends early when reasoning eats the output budget | max-tokens recovery nudge, `pgo=1` (Deltas) |
| [V3] Buffered workflow messages survive a restart | `bufferOnly`/`callerSessionId` persisted notifications + pending-handoff/step-update files (Workflows) |
| [V3] Command approval checks shell syntax for the running shell, incl. fish | terminal dialect fed to approvals; new fish parser `gxo` (Deltas) |
| [V3] Completed workflow steps can send follow-up info to their parent | finished-step `send_message` info allowed; parent->finished-step buffered (Workflows) |
| [V3] Org MCP registry `oauth`/`oauthScopes` | registry remote schema + merge (Deltas) |
| [V3] Oversized images retry at the provider's size limit | `imageRefit` mechanism (Deltas) |
| [V3] Resumed sessions show saved model and effort before the model list loads | cold-catalog pin `jOl` + one-option effortLevel (Deltas) |
| [V3] Workflow pause messages no longer say a run resumes on its own | pause-reason texts reworded (Workflows) |

## Proposed issues/notes

Searched `.rivets/issues.jsonl` (425 rows) for: outputStyle, diagram, thinking, attention, evalStrategy, pending_interaction,
foreground, creator, max_tokens, shell_type/posix, always-allow.

- **P1-candidate (needs L1):** NOTE on **cyril-auxt** (open P1): even with V3 `permissions.rules`, cyril's `shell_type`
  reply `"posix"` makes KAS evaluate every shell command with dialect `"unknown"`, which returns `ask`. File NEW
  "KAS shell_type must report a concrete shell name (bash/zsh/fish/pwsh), not the family `posix`" (fix at
  crates/cyril-core/src/protocol/kas/host_shell.rs:166-171). P1 if L1 confirms (default-engine regression of approvals);
  related: cyril-6bol (closed), cyril-gn07, cyril-w55l.
- **NEW P2** "Render KAS workflow plan approvals": show `request_permission` `toolCall.content` Text and
  `_meta.kiro.workflowPlan` (crates/cyril-ui/src/widgets/approval.rs:80-131 renders only diffs/raw_input). Labels `kiro-2-32-0`, `kas`, `w1`.
- **NEW P2** (or NOTE on cyril-4u4a) "Model workflow `attention[]` on lifecycle events, state, list and run_complete;
  `inspect.pendingSteps`": crates/cyril-core/src/protocol/convert/kas/workflow.rs:202-208, :377-394, :542-556. Labels `kas`, `w1`.
- **NEW P3** "Opt into `_meta.kiro.diagrams` and render `create_diagram` output" (engine.rs:113-134; dark surface).
- NOTE **cyril-53qx**: AgentSettings `thinking` removed in 0.66.43; cyril's `chat.enableThinking -> thinking` (settings.rs:79/:96)
  is now an unknown key, so thinking follows the model default unless configId `thinking` is set.
- NOTE **cyril-iq2c**: "session-reasoning" configuration source (effortLevel/thinking rows) is visible via `configurationState`.
- NOTE **cyril-lyrx**: config/template outputStyle options +proactive/explanatory/learning; per-prompt `_meta.kiro.outputStyle`.
- NOTE **cyril-0asq**: workflow-creator now runs as a step with plan approval; `workflows.creatorEffort` setting.
- NOTE **cyril-inwg**: authoring skill refresh (update_workflow workflowPrompt/workflowRef; generated:// relaunch gone; 2/pause loops).
- NOTE **cyril-58uv**: kind and method sets unchanged; `pending_interaction` now may carry `_meta.kiro.workflowPlan`;
  `_kiro/session/notify` severity `success` (handoff-restored) appears.
- NOTE **cyril-7p7n**: new client meta `agentHostsForegroundShell`.
- NOTE **cyril-w55l** / lki9 family: new repeat-cap parent wake (severity info) and attention-based step waiting.

## Method notes

- Artefacts (all regenerable bulk in `kas/tmp/`, git-ignored): `tmp/bundle/{old,new}.{raw,norm}`, `norm.diff` (1160 hunks;
  old 337,890 / new 344,285 statements), `hunks.txt` (976 KB), `changed.txt` (moved-pair-cancelled statements:
  dels 3405 / adds 9800, 2158 moved pairs cancelled, kept −1247 / +7642). Small evidence beside this file:
  `literals-delta.txt` (1917 changed; 1189 new-only; 172 old-only), `flags-{old,new}.txt`, `settings.txt`,
  `session-info-kinds.txt`, `surface/` (surface census JSON + stdout), `bundled-agents/{old,new}/*.md` (26 vs 25 profiles).
- Reused read-only scripts: `static-kas-bundle-diff-2.31.1.py`, `static-kas-hunks-2.31.1.py`, `static-kas-literals-2.31.1.py`,
  `static-kas-flags-2.29.1.py`, `static-kas-settings-2.29.1.py`, `static-kas-session-info-kinds-2.29.1.py`, `static-kas-surface-2.29.1.py`.
  New scripts: `static-kas-ctx-2.32.0.py` (count in both bundles + context), `static-kas-region-diff-2.32.0.py`
  (token diff of one anchored region; derails across long code moves, so keep LEN small), `static-kas-moved-filter-2.32.0.py`
  (cancels reorder pairs in norm.diff), `static-kas-bundled-agents-2.32.0.py` (extracts `---\nname:` profiles).
- The surface census missed two new client keys (`diagrams` read via an alias `n=e?._meta?.kiro`; `agentHostsForegroundShell`
  via `clientMeta?.X`) and the bridge-level `attention` (added by spread in `lq`, not in `kind:…,payload:{}` literals).
  Read the capability resolver (`fGr`) and the notification bridge directly each cycle; the payload-key census alone is not enough.
- The settings script's "−thinking" is real (schema plus intake row removed), not a rename; confirmed via the catalog-row
  diff (51 -> 55 rows) and the `kBu` intake table.
- The shell-dialect gap is a standing-coverage finding (trap 24): present in 0.66.34 as `l.shellType??cb(r.shellType)`.
  The delta lane only found it because 0.66.43 touched the code. Ask the live lane to verify before filing P1.
- The covenant doc (docs/kiro-kas-acp-covenant.md) has no `diagrams`, `agentHostsForegroundShell`, `workflowPlan` or
  `attention`; sync it when the covenant package next updates.
