# kas-workflow lane evidence (3.1.0 audit) — quoted bundle context, <=200 chars each

Offsets are byte positions in `~/.local/share/kiro-research/kas-carves/<ver>/node_modules/@kiro/agent/dist/server/acp-server.js`.

## keepNames wrapper (why every method literal gained 2)
- 3.1.0@14286205: `"_kiro/workflow/pause":a(async l=>await t.runner.pause(l.workflowId,MJe("_kiro/workflow/pause",l)),"_kiro/workflow/pause")`
- 2.32.0@11316269: `"_kiro/workflow/pause":async c=>await e.runner.pause(c.workflowId,y8e("_kiro/workflow/pause",c))`

## Step credits (0.66.91)
- 3.1.0@1946842: `l$a=x.object({usage:x.number(),unit:x.string().optional(),unitPlural:x.string().optional()}),cht=x.lazy(…` and node `credits:x.array(l$a).optional().catch(void 0)` @1947387
- 3.1.0@13449636: `emitNodeComplete … failureReason:n.failureReason}:{},...n.credits!==void 0?{credits:n.credits}:{}}})}async recordSessionCredits(e){… let n=await this.deps.readSessionCredits(e.sessionId);e.credits=n.length>0?n:void 0}`
- 3.1.0@13342473: `function She(t){let e=WKe(mGo(t.root));return e.length>0?{credits:e}:{}}a(She,"runCompleteCreditFields");function*mGo(t){yield*t.credits??[];for(let e of t.children??[])yield*mGo(e)}a(mGo,"creditsUnder")`
- 3.1.0@13460640: `emit({kind:"run_complete",payload:{workflowId:e.workflowId,status:e.state.status,finalState:e.state,...vhe(e.state),...She(e.state)}})`
- 3.1.0@14403957: `async sessionCredits(e){…n.eventAdapter.sessionUsage()?.credits??[];…Yhe.fromTranscript(r).report()?.credits??[]}catch(r){return b.warn("workflow.step_credits.unread",…`
- 2.32.0 `kind:"node_complete",payload:` @10548184 ends `...r.failureReason!==void 0?{failureReason:r.failureReason}:{}}})` (no credits); `credits` literal count 3 (0.66.43) / 10 (0.66.58) / 28 (0.66.91).

## workflowLaunch (0.66.58)
- 3.1.0@7339517: `BGn="workflowLaunch",rpo=["when-asked","large-tasks","delegate-all"],Tze="when-asked",ipo={"when-asked":"When asked","large-tasks":"Large tasks only","delegate-all":"Delegate all work"}`
- 3.1.0@7339585: `function cpo(t){return{type:"select",id:BGn,name:"Workflow use",currentValue:t,options:rpo.map(e=>({value:e,name:ipo[e]}))}}`
- 3.1.0@7339122: `function spo(t,e){return t?e===void 0?{kind:"legacy"}:{kind:"offered",value:e}:{kind:"unavailable"}}function apo(t){…let e=t.kind==="legacy"?"delegate-all":t.value;return`${HGn} ${ipo[e]}.`}`
- 3.1.0@15817030: `d?.role!=="workflow-step"&&(F.workflowLaunch=Tze)`; @15841561: `workflowLaunchState(e){return spo(vzs(e),e.workflowLaunch)}` with `vzs` = `orchestratesWorkflows` (@15710791).
- 3.1.0@15820418: `buildNewSessionResponse(e,n){… let{memoryConfigSource:o,workflowLaunch:s,...c}=n;return{_meta:{...c,…` (not echoed in `_meta`).
- 2.33.1@12330620: `AQo=\`# Workflow launch setting\n\nFollow the rule for the value named by "Workflow launch setting:" in the last \<session_context\> block. … ## When asked … ## Large tasks only … ## Delegate all work …`
- counts: `workflowLaunch` 0/0/24/24/24/26; `"when-asked"` 0/0/3/3/3/3.
- TUI 3.1.0: `CHAT_WORKFLOW_LAUNCH:"chat.workflowLaunch"`; `let h=DBt(Xk(p),lz(ea()[Ve.CHAT_WORKFLOW_LAUNCH]));if(h)try{p=(await this.kiroClient.setSessionConfigOption({sessionId:l,configId:ew,value:h})).configOptions??p}` with `ew="workflowLaunch"`.

## Workflow Orchestration prompt
- 2.32.0@12024274: `jHo='# Workflow Orchestration\n\n## Default behavior: always delegate implementation to workflows\n\nYou are the top-level orchestrator …**always** create and launch the workflow`
- 2.33.1@12314824: `RQo='# Workflow Orchestration\n\nWhen you start a workflow:\n\n1. Formulate the workflow prompt from what you already know …2. Create and launch the workflow with a \`run_workflow\` call`
- 3.1.0@15096370: `p1s='# Workflow Orchestration … 3. In the brief, give the user's full task or its absolute path … State task size; leave steps, reviewers, loops and budgets to the creator`

## Plan feedback (0.66.58)
- 3.1.0@1948522: `o3e=x.object({text:x.string(),creatorSessionId:x.string(),toolCallId:x.string(),consentRound:x.number().optional(),topLevelStepIds:x.array(x.string())})`
- 2.33.1@462091: `interaction_resolved … executionId:T.string(),consentRound:T.number().optional(),rejectionReason:T.string().optional()`
- 2.33.1@10695877: `find(f=>f.type==="interaction_resolved"&&f.toolCallId===a.toolCallId&&O9e(f.consentRound,a.consentRound)),d=…rejectionReason:void 0;if(d===void 0||d.trim()==="")return;let p={text:d,creatorSessionId:…`
- 2.33.1@10833129: `y=(t.state.appliedPlanFeedback??[]).filter(C=>C.topLevelStepIds.includes(g)),w=[QYr(t.state.originalUserMessages??[]),ONo(y)].filter(C=>C!=="").join(…)`
- literal 0.66.58: `<approved_plan_feedback>\nCorrections the user typed while reviewing the plan these steps belong to, oldest first. … where a correction conflicts with it, follow the correction.`
- Approval builder unchanged: 2.32.0@11099783 / 3.1.0@14036386 `title:\`Add ${P} to ${u}\`,content:R,...C!==void 0&&{workflowPlan:C},...N&&{askAgainAfterRestart:!0}`.

## List-row progress (0.66.48)
- 2.33.0@2033357: `iBn=T.object({stepsCompleted:…,stepsTotal:…,currentNodeId:T.string().optional(),waitingOnWatch:T.boolean().optional()}),$Ne=["stepsCompleted","stepsTotal","currentNodeId","waitingOnWatch"],nme=T.object({workflowId… ,...iBn.shape})`
- 3.1.0@14216071: `return{stepsCompleted:r,stepsTotal:i,...l!==void 0&&{currentNodeId:l},waitingOnWatch:o>0&&s===0}}a(L9t,"runProgressOf")`; `...L9t(t,e)}}a(Qhe,"buildRunSummary")` @14302063.

## fileCheck jsonPath (0.66.48)
- 2.33.0@10376091: `function LHr(e){let t=e.split("."),r=[],n,i=!0;for(let[o,s]of t.entries()){if(s===""){o>0&&o<t.length-1&&(n??="..");continue}let a=i&&s==="$";i=!1,!a&&(n??=ssu(s),r.push(s))}…` `function ssu(e){for(let t of e)if(t==="["||t==="]"||t==="*")return t;…`
- 2.33.0@10434865: `has fileCheck jsonPath '${n}' with unsupported '${i}'; use a dotted key path such as 'verdict' or '$.verdict'.` … `WorkflowFileCheckJsonPathUnsupportedError`
- 2.32.0 evaluator `Pru`: `let r=t.split(".").filter(i=>i.length>0),n=e;for(let i of r){…n=n[i]}` (no `$` handling).

## github-pr watch (0.66.48)
- 2.32.0 -> 2.33.0: `"url,state,isDraft,mergedAt,closedAt,comments,reviews,statusCheckRollup,headRefOid"` -> `…,headRefOid,mergeable,mergeStateStatus"`; new GraphQL `reviewThreads(first:100){nodes{id,comments(first:100){nodes{databaseId}}}}` and `baseRef{target{oid}}`; payload `headSha:X,mergeable:ye,mergeStateStatus:V,newFailedChecks:H`; cursor `{conflictHeadSha,conflictBaseSha}`; log `workflow.watch.github_pr.conflict`.

## Wake path
- 2.33.1@11637337: `wakePromptMeta(t,r){return{agentInitiated:!0,agentInitiatedReason:r,...this.host.automaticTurnProfiling(t)}}`
- 3.1.0 (region diff 2.35.0->3.1.0): `rewakeWithInfo … ||r.promptSetupInFlightCount>0)` -> `||this.host.hasUnstoppedPromptInSetup(r))`; new `wakeTargetIsBusy(e,n){return this.host.controller.hasRunningTurn(e)||this.pendingWakePrompts.has(e)||n.promptSetupInFlightCount>0||n.promptPersistenceInFlightCount>0}` used by `autoWakeParentOnComplete`; else-branch keeps `steeringBuffer.append … "workflow.emitNotification.parent_busy_queued"`.
- 3.1.0@15715608: `function kzs(t,e){for(let n of EV.get(t)?.keys()??[])if(n!==e&&!n.signal.aborted)return!0;return!1}a(kzs,"hasUnstoppedPromptInSetup")`
- 3.1.0@14330416: `function yml(t,e,n,r){if(t==="parent"){let i=r==="running";return BVe(r)||i?void 0:{sender:"parent",severity:e}}if(t==="peer")return{sender:"peer",severity:e};if(e==="info"&&n)return Ers;if(e==="warning"||e==="error")return{sender:"step",severity:e}}a(yml,"admitWake")`
- 2.32.0 -> 2.33.0 wake region: only renames (`readCreatorEffortSetting:()=>…` -> `async o=>…(o)`).

## notify / send_message (0.66.91)
- 2.32.0@2039590 `Xir=x.enum(["parent","step"])` -> 3.1.0@1965055 `Pvn=x.enum(["parent","step","peer"])`
- 3.1.0@14054341: `function Fqt(t,e,n,r,i){return{sessionId:t,...i!==void 0&&{notifyId:i},...r.sessionId!==void 0&&{callerSessionId:r.sessionId},message:e,severity:n,sender:r.role,…}}a(Fqt,"buildSessionNotifyParams")`; `{...Fqt(…,f.notifyId),...h!==void 0?{deliveryCount:h}:{}}` @14063976
- 3.1.0@1965893: `N$a=x.object({sessionId:x.string().max(256),outcome:x.enum(["delivered","not_authorized","not_found","budget_exceeded","suppressed","cancelled","failed"]),parent:x.literal(!0).optional()}),Ovn=x.object({recipients:x.array(N$a).min(1).max(_ht)})` with `_ht=16`
- 3.1.0@2786736: `P_={awareness:new NUe("awareness","sessionToolsAwareness","sessionAwarenessEnabled"),messaging:new NUe("messaging","sessionToolsMessaging","sessionMessagingEnabled"),…}`; `NUe … this.setting=G0(n,"off",{hidden:!1})` (default off).

## Settings schema
- 2.32.0@2055692 `Eoa=zhe({creatorEffort:x.string().optional()})` … `workflows:Eoa.optional()`; 3.1.0@1955502 `h$a=lve({creatorEffort:x.string().optional()})` … `workflows:h$a.optional(),sessionTools:x.object({awareness:nl.optional(),messaging:nl.optional(),cloud:nl.optional()}).optional(),…universalBackgroundTool:nl.optional()`
- intake both: `workflows:Zm("workflows","off",{hidden:!1}),…workflowsCreatorEffort:PUe("workflowsCreatorEffort",gae,{hidden:!1})`
- `workflowNotifications` / `setWorkflowNotificationDelivery`: 0 occurrences in all six carves; TUI 3.1.0 `Rhe="_kiro/session/setWorkflowNotificationDelivery"`.

## Creator recipe / brief (ADR-0013)
- 2.32.0@10353302 / 3.1.0@13268978: `UTe="create-workflow",aze="workflow_brief",hzr="workflow-from-prompt",BSo="workflow_prompt"` -> `dIe=…,AKe=…,Yur=…,VWo=…`; wrapper lines identical: `"Design the workflow this brief asks for, then add its steps to this run:","1. Save the definition with save_workflow_definition until it returns saved: true.",…`
- TUI 3.1.0: `function o_(){return{enabled:Bo(Ve.CHAT_ENABLE_WORKFLOWS,!1)}}`; `function k_r(e,t){e.workflows={enabled:t.enabled},e.goal={enabled:t.enabled}}`.

## Dark candidate
- 3.1.0@13835910: `Sil="### Workflow Step Environment\n\nYou are running a workflow step in a Kiro Web cloud session's internal sandbox. Repositories are Brazil packages on GitFarm; there is no GitHub here…"` (0 in earlier carves).
