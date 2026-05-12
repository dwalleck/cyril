OH
EsH
+sH
(PH
=PH
¦sH
sH
FRH
{RH
ÒJH
IRH
°sH
ÒJH
internal error: entered unreachable codeMap must not be polled after it returned `Poll::Ready`  falseMethod not found: event crates/agent/src/agent/mcp/service.rs:299mcpmessageagent::agent::mcp::servicecrates/agent/src/agent/mcp/service.rsevent crates/agent/src/agent/mcp/service.rs:293event crates/agent/src/agent/mcp/service.rs:290event crates/agent/src/agent/mcp/service.rs:296event crates/agent/src/agent/mcp/service.rs:302: 2025-06-18Transport is closedError reading from stream: idLSP process exited during initialization with code: all branches are disabled and there is no else branchSending workspace/didChangeConfiguration to LSP server: Channel closedvariant identifierstruct TrustOptionstruct McpArgsstruct ChatArgsstruct CodeArgsstruct HelpArgsstruct PlanArgsstruct QuitArgsadjacently tagged enum TuiCommandstruct CompactArgsstruct ContextArgsstruct PromptsArgsstruct FeedbackArgsstruct SendRequestArgsstruct KnowledgeArgsstruct UserTurnMetadatastruct PasteImageArgsstruct ExecuteCmdErrora Display implementation returned an error unexpectedlymid > lendescription() is deprecated; use Displaycalled `Result::unwrap()` on an `Err` valueHandle logging before handshake failed.Received ping request. Ignored.Received unexpected messageinitialize responsesend initialize requestsend initialized notificationrefresh token present, attempting refreshOAuth client not configuredNo refresh token availableAccess token expired or nearly expired, refreshing.Refreshed access token.Token refresh not possible, re-authorization required.Mcp-Session-Idthis server doesn't support deleting sessionerrormsgNotPresentNotUnicodeIntegerSqliteFailureSqliteSingleThreadedModeFromSqlConversionFailureIntegralValueOutOfRangeUtf8ErrorInvalidParameterNameInvalidPathExecuteReturnedResultsQueryReturnedNoRowsInvalidColumnIndexInvalidColumnNameInvalidColumnTypeStatementChangedRowsToSqlConversionFailureInvalidQueryUnwindingPanicMultipleStatementInvalidParameterCountSqlInputErrorsqloffsetInvalidDatabaseIndexevent crates/agent/src/agent/agent_config/load.rs:86agent_configscrates/agent/src/agent/agent_config/load.rsevent crates/agent/src/agent/agent_config/load.rs:77eevent crates/agent/src/agent/agent_config/load.rs:56skill://.kiro/skills/*/SKILL.mdskill://~/.kiro/skills/*/SKILL.md.kirofile://valid resourceAmazonQ.mdfile://AmazonQ.mdfile://.amazonq/rules/**/*.mdThe default agent for Kiro CLI# Kiro CLI Default Agent

You are the default Kiro CLI agent, bringing the power of AI-assisted development directly to the user's terminal. You help with coding tasks, system operations, AWS management, and development workflows.
{
  "name": "kiro_planner",
  "description": "Specialized planning agent that helps break down ideas into implementation plans",
  "tools": [
    "read",
    "glob",
    "grep",
    "web_fetch",
    "web_search",
    "report",
    "shell",
    "todo",
    "knowledge",
    "introspect",
    "switch_to_execution"
  ],
  "toolsSettings": {
    "shell": {
      "autoAllowReadonly": true,
      "denyByDefault": true
    }
  },
  "includeMcpJson": false,
  "keyboardShortcut": "shift+tab",
  "welcomeMessage": "Transform any idea into fully working code. What do you want to build today?"
}
Invalid kiro_planner.jsonYou are a specialized planning agent that helps break down ideas into implementation plans. The user does NOT want you to execute yet -- you MUST NOT make any edits, run any non-readonly tools, or otherwise make any changes to the system. If asked to implement, fix, or modify files, respond: "I'm a planning agent - I can read and analyze code but not modify it. I can help you plan the implementation instead."

## Planning Workflow

### Step 1: Requirements Gathering

Guide the user through structured questions to refine the initial idea and develop a specification.

**Constraints:**
- You MAY explore the codebase by reading relevant files to understand context. Use `grep` and `glob` tools to navigate the codebase effectively.
- You MUST summarize your understanding by briefly restating what user wants in 1-2 sentences
- You MUST ask AT MOST THREE structured questions per turn and wait for the user's response
- You MUST wait for the user's response before asking the next set of questions
- Once you have their response, append the user's answer to the plan
- Only then proceed to formulating the next set of questions
- You SHOULD ask about edge cases, user experience, technical constraints, and success criteria
- You SHOULD adapt follow-up questions based on previous answers
- You MAY recognize when requirements clarification appears to have reached a natural conclusion

### Step 2: Implementation Plan

Conduct research on relevant technologies or existing code that could inform the design. Develop a design based on the requirements and research. Create a structured plan with a series of steps for implementing the design.

**Constraints:**
- You MUST identify areas where research is needed based on the requirements
- You MUST ask the user for input on the research using structured questions, including:
  - Additional topics that should be researched
  - Specific resources (files, websites, tools) the user recommends
  - Areas where the user has existing knowledge to contribute
- You MUST create a design based on the research and requirements
- You SHOULD include diagrams or visual representations when appropriate using mermaid syntax
- You MUST use the following specific instructions when creating the task list:
  ```
  Convert the design into a series of task that will build each component in a test-driven manner following agile best practices. Each task must result in a working, demoable increment of functionality. Prioritize best practices, incremental progress, and early testing, ensuring no big jumps in complexity at any stage. Make sure that each task builds on the previous tasks, and ends with wiring things together. There should be no hanging or orphaned code that isn't integrated into a previous task.
  ```
- You MUST format the task list as a numbered series of detailed steps
- Each task in the plan MUST be written as a clear implementation objective
- Each task MUST begin with "Task N:" where N is the sequential number
- You MUST ensure each task includes:
  - A clear objective
  - General implementation guidance
  - Test requirements where appropriate
  - Demo: description of the working functionality that can be demonstrated after completing this task

After presenting overall plan, ask: "Does this plan look good, or would you like me to adjust anything?". Wait for user confirmation before calling switch_to_execution.

### Step 3: Call switch_to_execution

**Constraints:**
- You MUST only call switch_to_execution after user confirms the plan looks good
- You MUST have completed Step 1 (requirements gathering) before calling switch_to_execution
- You MUST have completed Step 2 (implementation plan) before calling switch_to_execution
- You MUST pass the complete plan as the `plan` parameter


## Example Implementation Plan
```
**Implementation Plan - [Feature Name]:**

**Problem Statement:**
[What problem are we solving and its scope]

**Requirements:**
[Requirement gathering based on user question]

**Background:**
[Findings based on the research and other context]

**Proposed Solution:**
[High-level approach which addresses the requirements]

**Task Breakdown:**
[Checklist of tasks and detailed description for each task]
```

## Example Structured Question
```
[1]: [Clear question ending with ?]
a. **[Label]** - [Description of implications/trade-offs]
b. **[Label]** - [Description]
c. **Other** - Provide your own answer

(Use the chat to answer any subset: eg., "1=a or provide your wwn answer)
```event crates/agent/src/agent/agent_config/load.rs:289event crates/agent/src/agent/agent_config/load.rs:309event crates/agent/src/agent/agent_config/load.rs:338event crates/agent/src/agent/agent_config/load.rs:314event crates/agent/src/agent/agent_config/load.rs:303event crates/agent/src/agent/agent_config/load.rs:230presentcanonicaltoolsSettingsfsReadfs_readwritefsWriteshellexecute_bashexecuteBashexecuteCmdexecute_cmduse_awsuseAwsawsagent_crewagentCrewuse_subagentAgent config has duplicate toolsSettings alias keys, merging to canonicalevent crates/agent/src/agent/agent_config/load.rs:418event crates/agent/src/agent/agent_config/load.rs:410event crates/agent/src/agent/agent_config/load.rs:389event crates/agent/src/agent/agent_config/load.rs:401KIRO_TEST_AGENTS_DIRagentsamazonqcli-agentscreateaddremovecleartrust-alltrustuntrustresetnewupdatecancelstatussummarytrust-all, trust <name>, untrust <name>, resetsave <path>, load <path>, new [prompt]_kiro.dev/commands/prompts/options/help/model/agent/clear/quit/usage/paste/tools/plan/feedback/chat/knowledge/reply/code/hooksmodelcontextsession_idUSER_PROMPTprompttool_nametool_inputtool_responseassistant_response ... truncatedfailed to execute command: command timed out after  ms , Partial commandBase commandevent crates/agent/src/agent/mcp/mod.rs:486agent::agent::mcpserver_namecrates/agent/src/agent/mcp/mod.rsevent crates/agent/src/agent/mcp/mod.rs:479event crates/agent/src/agent/mcp/mod.rs:470evtevent crates/agent/src/agent/mcp/mod.rs:475event crates/agent/src/agent/mcp/mod.rs:491oauth_urlevent crates/agent/src/agent/mcp/mod.rs:494failed to send server initialized messageduplicated server. old server droppedevent was not from an initializing MCP serverreceived oauth requestMCP server tool list changedevent crates/agent/src/agent/mcp/mod.rs:382reqevent crates/agent/src/agent/mcp/mod.rs:448event crates/agent/src/agent/mcp/mod.rs:357event crates/agent/src/agent/mcp/mod.rs:363errevent crates/agent/src/agent/mcp/mod.rs:366event crates/agent/src/agent/mcp/actor.rs:431agent::agent::mcp::actorcrates/agent/src/agent/mcp/actor.rsevent crates/agent/src/agent/mcp/actor.rs:425event crates/agent/src/agent/mcp/actor.rs:439request_idresultevent crates/agent/src/agent/mcp/actor.rs:408event crates/agent/src/agent/mcp/actor.rs:410event crates/agent/src/agent/mcp/actor.rs:326event crates/agent/src/agent/mcp/actor.rs:332event crates/agent/src/agent/mcp/actor.rs:365event crates/agent/src/agent/mcp/actor.rs:302event crates/agent/src/agent/mcp/actor.rs:309event crates/agent/src/agent/mcp/actor.rs:312event crates/agent/src/agent/mcp/service.rs:143event crates/agent/src/agent/mcp/service.rs:182event crates/agent/src/agent/mcp/service.rs:227event crates/agent/src/agent/mcp/service.rs:210event crates/agent/src/agent/mcp/service.rs:140event crates/agent/src/agent/mcp/service.rs:159event crates/agent/src/agent/mcp/service.rs:193event crates/agent/src/agent/mcp/service.rs:536event crates/agent/src/agent/mcp/service.rs:529event crates/agent/src/agent/mcp/service.rs:539event crates/agent/src/agent/mcp/service.rs:546event crates/agent/src/agent/mcp/service.rs:577event crates/agent/src/agent/mcp/service.rs:580event crates/agent/src/agent/mcp/service.rs:573event crates/agent/src/agent/mcp/service.rs:604event crates/agent/src/agent/mcp/service.rs:488sending payloadrequest receiver has closedresponse tx dropped before sending a resultInvalid working directory '': commandworking_dirExecuteCmdevent crates/agent/src/agent/tools/execute_cmd/unix.rs:232agent::agent::tools::execute_cmd::unixcrates/agent/src/agent/tools/execute_cmd/unix.rsDetected and removed  hidden charsAmazonQ-For-CLIVersion1.29.6 /AWS_EXECUTION_ENVKIRO_TEST_SESSIONS_DIRHOME directory not foundclitasksFailed to create task dir: Failed to write task: Failed to create task file: Failed to lock task file: Failed to serialize task: Active Task List for current session:

Description: 
Progress:  tasks completed


Recent Context:

Modified Files:
- [â][ ] (NEXT) #. event crates/agent/src/agent/tools/task/store.rs:220crates/agent/src/agent/tools/task/store.rsSkipping malformed task file Failed to read task file: Failed to read task dir: Failed to read dir entry: Failed to delete task file: Failed to parse task Task  not found: .jsonbuf.len() must fit in remaining(); buf.len() = mpsc bounded channel requires buffer > 0max receiversbroadcast channel capacity cannot be zerobroadcast channel capacity exceeded `usize::MAX / 2`assertion failed: queued.load(Relaxed)valid_up_toerror_lenAgentIdparent_idextended_codeInternalMalfunctionDatabaseBusyDatabaseLockedOutOfMemoryOperationInterruptedSystemIoFailureDatabaseCorruptCannotOpenFileLockingProtocolFailedSchemaChangedTooBigConstraintViolationTypeMismatchApiMisuseNoLargeFileSupportAuthorizationForStatementDeniedParameterOutOfRangeNotADatabaseUnknownOkErrServerNotInitializedServerCurrentlyInitializingServerFailedServerAlreadyLaunchedChannelCustomNotIdleAgentLoopErrorAgentLoopResponseMcpManagerSendingRequestConsumingResponsePendingToolUseResultsUserTurnEndedErroredfailed to expand default credential pathTrustOptionlabeldisplaysetting_keyServer with the name  is not initialized is currently initializing failed to initialize has already launchedThe channel has closedLaunchServerconfigGetToolSpecsGetPromptsGetPromptargumentsExecuteToolTerminateToolsPromptsAgent is not idleAn error occurred with an MCP server: The agent channel has closedSendPromptCancelSendApprovalResultCreateSnapshotGetMcpPromptsGetFilePromptsGetMcpPromptSwapAgentCompactConversationClearConversationGetToolInfoAddResourceRemoveResourceGetResourcesClearSessionResourcesGetLastAssistantMessageTrustAllToolsTrustToolsUntrustToolsResetToolPermissionsImageResourceLinkMissingHomeDirMissingDataLocalDirJsonWithContextsourceIoPathExpandGlobsetErrorGlobIterateDbOpenErrorPoisonErrorStringFromUtf8StrFromUtf8AgentLoopIdToolSpecsPromptTerminateAcknowledgedSuccessMcpPromptsFilePromptsMcpPromptSwapCompleteMcpServerInfoResourcesLastAssistantMessageToolTrustResultchangedinvalidApprovalResultoption_idreasontrust_optionSendPromptArgscontentshould_continue_turnCancelledMissing a home directoryMissing a local data directoryFailed to open database: AllowOnceAllowAlwaysToolAllowAlwaysToolArgsRejectOnceRejectAlwaysToolArgsAllowAlwaysRejectAlwaysInvalidJsonassistant_textinvalid_toolsvalid_toolsStreamServiceInitializingInitializedserve_durationlist_tools_durationlist_prompts_durationInitializeErrorOauthRequestToolListChangedSendApprovalResultArgsunexpected empty broadcast channelThe model produced invalid JSONAn error occurred with the service: DidNotRunUserTurnEndToolUseRejectedStreamMetadatatool_usesstreamSendRequestArgstool_specssystem_promptGetExecutionStateSendRequestloop_idmessage_idstotal_request_countbuiltin_tool_usesturn_durationend_reasonend_timestampinput_token_countoutput_token_countcontext_usage_percentagemetering_usageExecutionStatePendingToolUsesAssistantTextToolUseStartToolUseResponseStreamEndLoopStateChangetoStreamCurrentlyExecutingAgentLoopExitedA response stream is currently being consumedThe agent loop has already exiteddata did not match any variant of untagged enum LinkedEditingRangeServerCapabilitiesdynamicRegistrationrelatedDocumentSupportdata did not match any variant of untagged enum DiagnosticServerCapabilities