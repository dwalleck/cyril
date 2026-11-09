| ver | # | line | location (lane: placement) |
|---|---|---|---|
| 2.33.0 | 1 | [added] [V3] Kiro asks you to trust a folder before it loads that folder's hooks, agents, MCP servers or workf… | tui: **L T16** — TUI-local gate, no frame to KAS (`workspaceTrust` 0 -> 70) · kas-core: host/TUI prompt (`resolveWorkspaceTrust`, `workspaceTrusted` file); KAS entry hard-codes `workspaceTrusted:!0`; gates pre-exist (Deltas) · kas-workflow: kas-core lane (trust); `_kiro/workflow/new` refusal text for out-of-force recipes unchanged since 0.66.43 — WF-L9 |
| 2.33.0 | 2 | [added] [V3] Workflow plan approvals show a compact step tree with expandable details | tui: **L T15** (`workflowPlan` 0 -> 3) · kas-core: kas-workflow / TUI · kas-workflow: TUI-only; `workflowPlan` builder/approval shape unchanged |
| 2.33.0 | 3 | [changed] Faster `grep` tool searches on large projects: files are now scanned in parallel across CPU cores | kas-core: KAS-internal (ripgrep concurrency), no wire |
| 2.33.0 | 4 | [changed] Mermaid flowcharts with subgraphs now draw in chat instead of showing their source | tui: TUI-local (renderer) · kas-core: TUI |
| 2.33.0 | 5 | [changed] [V3] Long sessions hold less memory, so the agent stays responsive after hours of use | kas-core: KAS-internal (not located statically; `sessionEviction` pre-exists) |
| 2.33.0 | 6 | [changed] [V3] Workflow GitHub PR watches now act on merge conflicts, failed checks, and answered threads | kas-core: kas-workflow (zod +`pullRequest, reviewThreads, baseRef, oid` @0.66.48) · kas-workflow: 0.66.48 `github-pr` handler (`mergeable,mergeStateStatus`, reviewThreads/baseRef GraphQL, conflict re-wake) + `wf-pr-responder` prompt |
| 2.33.0 | 7 | [fixed] [V3] Terminal title shows the session title when `chat.terminalTitle` is enabled | tui: TUI-local (`chat.terminalTitle` 1 == 1) · kas-core: TUI |
| 2.33.0 | 8 | [fixed] [V3] Replies Kiro starts on its own, such as after a workflow finishes, no longer cut off after a few … | kas-core: KAS-internal (agent-initiated prompt path) · kas-workflow: not located: wake path and `promptAgentInitiated` token-identical 0.66.43 -> 0.66.48; candidate TUI rendering or non-workflow turn code — WF-L7 |
| 2.33.0 | 9 | [fixed] [V3] `/agent`, `/model`, and `/effort` complete a partly typed argument inline again | tui: TUI-local · kas-core: TUI |
| 2.33.0 | 10 | [fixed] [V3] Editing sandbox network rules in `permissions.yaml` now applies to the running sandbox | kas-core: KAS-internal (`setNetworkRules`, `updateProxyPolicy`) |
| 2.33.0 | 11 | [fixed] [V3] `/sessions` no longer freezes for seconds at a time while its search index catches up | tui: TUI-local · kas-core: KAS-internal (session index) |
| 2.33.0 | 12 | [fixed] [V3] Fallback models can be set again from the `/model` pane and `/model fallback` | tui: **L T6** — `_kiro/modelFallback/{get,set}` added; **dark on KAS** · kas-core: `_kiro/configuration/set` fallback chains (0.66.62) / TUI; flags Deltas |
| 2.33.0 | 13 | [fixed] Clearer Mermaid node descriptions wrap across up to four rows, with an ellipsis for longer text | tui: TUI-local · host: TUI |
| 2.33.0 | 14 | [fixed] Starting Kiro from your home directory no longer lists your global agents twice | tui: no wire part; not located to a literal · kas-core: TUI / KAS-internal · host: host/`agent` crate (logic |
| 2.33.0 | 15 | [fixed] [V3] In your home directory, `/sandbox` no longer treats your global sandbox settings as project setti… | tui: TUI-local |
| 2.33.0 | 16 | [fixed] [V3] `kiro-cli chat --agent <agent> @agent-sop:<name>` runs the SOP instead of sending the text as typ… | tui: TUI/host argument preprocessing; no wire literal located · kas-core: TUI/host · host: host launch (logic) |
| 2.33.0 | 17 | [fixed] [V3] Cloud config keeps syncing after a request that never gets an answer | kas-core: cloud config sync — KAS cloud-config client internal (`fetch_cloud_config` hidden tool pre-exists); no wire field (hand-placed) |
| 2.33.0 | 18 | [fixed] [V3] `/code` in cloud sessions shows the sandbox workspace root and `lsp.json` path | tui: TUI-local (2.32.0 T16 fields) |
| 2.33.0 | 19 | [fixed] [V3] Sessions no longer show a cloud config sync card when your cloud config is empty | tui: TUI-local · kas-core: cloud / TUI |
| 2.33.0 | 20 | [fixed] [V3] The `code` tool reports diagnostics from Python and other slow language servers | kas-core: KAS-internal (LSP init) |
| 2.33.0 | 21 | [fixed] [V3] Throttling errors now tell an overloaded model apart from your own rate limit | kas-core: `display_error` variants (`reason:throttled`, `outcome:throttled` @0.66.58) — cyril handles only `mcp_connection_error` display_errors (kas.rs:634-651; cyril-58uv) |
| 2.33.0 | 22 | [fixed] [V3] Windows workflows no longer reject recipe files whose drive letter case differs | kas-workflow: not located (no literal change; code-only) |
| 2.33.0 | 23 | [fixed] [V3] Workflow review loops now stop once the reviewer approves instead of running to their limit | kas-workflow: 0.66.48 fileCheck `jsonPath` parser (`$.` prefix resolves; `[ ] * ..` rejected at load) [INFERENCE on causality] — WF-L5 |
| 2.33.0 | 24 | [fixed] [V3] Workflows now run when your organization disables the Auto model instead of failing | kas-core: kas-workflow · kas-workflow: 0.66.48 run-failure classification (`model_unavailable` <- `MODEL_TEMPORARILY_UNAVAILABLE`); the step model fallback chain (step -> workflow -> parent) is unchanged, so the "run" half is not located (body-unproven) |
| 2.33.0 | 25 | [fixed] Pressing Tab after `/model ` shows `set-current-as-default` as a sub-command again, instead of listing… | tui: TUI-local (`set-current-as-default` 6 == 6) · host: TUI |
| 2.33.0 | 26 | [fixed] [V3] `/code summary` now generates codebase documentation from the code intelligence overview instead … | tui: **L T24** (`summary:` 0 -> 1) · kas-core: TUI / KAS-internal |
| 2.33.0 | 27 | [security] [V3] Sandbox network rules now match host names regardless of letter case | kas-core: KAS sandbox network-rule matcher (internal; no literal change) — cyril has no sandbox (hand-placed) |
| 2.33.0 | 28 | [security] [V3] On Windows, hook commands no longer run a same-named program from the working directory | kas-core: KAS-internal (sandbox; hook spawn hardening) |
| 2.33.1 | 1 | [changed] [V3] When Workflows are enabled, a workflow now starts only when you explicitly ask for one | kas-core: kas-workflow (`workflowLaunch` zod @0.66.58) · kas-workflow: 0.66.58 `workflowLaunch` default `when-asked` + `# Workflow Orchestration` rewrite |
| 2.33.1 | 2 | [fixed] `/tools` lists each tool with one labelled line per field when the screen reader setting is on, so scr… | tui: TUI-local (`screen reader` 3 == 3) · host: TUI |
| 2.33.1 | 3 | [fixed] `kiro-cli mcp add --agent` now grants the new server to agents whose `tools` list lacks it, and upgrad… | host: **host** `cli/mcp` (logic |
| 2.33.1 | 4 | [fixed] [V3] Resuming a V2 session now tells you its saved tool approvals weren't carried over and how to keep… | tui: not located in the TUI (2.33.1 adds no such copy); KAS/host · kas-core: TUI · host: host/TUI (logic) |
| 2.33.1 | 5 | [fixed] [V3] Compaction retries without images when the model rejects history images | kas-core: KAS-internal (`summarization.skip_repeated_compaction` @0.66.58; image strip on compaction retry) (hand-placed) |
| 2.33.1 | 6 | [fixed] [V3] MCP sign-in no longer leaves a pinned OAuth callback port occupied afterward | kas-core: KAS-internal (MCP OAuth) |
| 2.33.1 | 7 | [fixed] [V3] Unavailable optional power MCP servers no longer block prompt turns | kas-core: KAS-internal (`resolvePowerServer`, later `kind:mcp_unavailable` @0.66.70) |
| 2.33.1 | 8 | [fixed] [V3] Long image sessions omit older images to stay within the request limit | kas-core: KAS-internal (`summarization.skip_repeated_compaction` @0.66.58; image strip) |
| 2.33.1 | 9 | [fixed] [V3] Workflow plan approvals open with the first offered option highlighted, so Enter selects it | tui: TUI-local · kas-core: TUI · kas-workflow: TUI-only |
| 2.33.1 | 10 | [fixed] HTML doctypes and tag brackets in code previews use theme colors with readable contrast | tui: TUI-local · host: TUI |
| 2.33.1 | 11 | [fixed] [V3] Custom agents running as subagents can search their own knowledge bases | kas-core: KAS-internal (`subagent.profileSkills.resolve` @0.66.58) |
| 2.33.1 | 12 | [fixed] [V3] MCP servers added from the registry launch the catalog's package version again | kas-core: KAS-internal |
| 2.33.1 | 13 | [fixed] [V3] Messages sent after a spec task run can keep updating its tasks | kas-core: KAS-internal (spec) |
| 2.33.1 | 14 | [fixed] [V3] Agents that use `@builtin` now include web search, as in V2 | kas-core: KAS-internal (tool tag expansion) · host: KAS |
| 2.33.1 | 15 | [fixed] [V3] Dispatched subagents now load their own skills instead of the parent agent's skills | kas-core: KAS-internal (`subagent.profileSkills.resolve` @0.66.58) (hand-placed) |
| 2.33.1 | 16 | [fixed] [V3] User agents keep their file allow and deny rules when a session starts in the home folder | kas-core: KAS-internal (policy load) |
| 2.34.0 | 1 | [added] [V3] Support for MCP elicitation, so MCP tools can ask you for input through a form or a link | tui: **L T8** · kas-core: **no KAS delta**: `_kiro/mcp/elicitation` call site identical (2 = 2), modes form/url pre-exist; TUI responder (Deltas) |
| 2.34.0 | 2 | [added] [V3] Type suggested changes directly into a workflow plan approval | tui: **L T14** — `reject_once` + `_meta.kiro.rejectionReason` · kas-core: kas-workflow (`planFeedback`, `appliedPlanFeedback` zod @0.66.58) · kas-workflow: TUI UI over the existing `_meta.kiro.rejectionReason`; engine side (0.66.58) persists it and builds `appliedPlanFeedback` |
| 2.34.0 | 3 | [added] [V3] Delete memories you no longer want from `/memories` | tui: **L T17** · kas-core: TUI over pre-existing `_kiro/memory/delete` (4 hits in 0.66.43) |
| 2.34.0 | 4 | [added] `/model edit` hides models you do not use from the `/model` picker | tui: **L T23** · kas-core: TUI · host: **host key** `chat.modelPicker.hiddenModels` + TUI |
| 2.34.0 | 5 | [added] [V3] Workflow use under Features in `/settings` to choose when Kiro starts workflows on its own | tui: **L T20** — `set_config_option configId:"workflowLaunch"` · kas-core: kas-workflow / TUI · kas-workflow: TUI `chat.workflowLaunch` -> `session/set_config_option workflowLaunch` (engine option 0.66.58) |
| 2.34.0 | 6 | [added] [V3] `--trust-workspace` skips the workspace trust prompt for one launch without saving the decision | tui: **L T16** (one-launch skip, no persistence) · host: **host flag** (clap text [0,1]) |
| 2.34.0 | 7 | [changed] [V3] Tool approvals list Allow once and Deny once first, then the options that create an allow or de… | tui: **L T13** · kas-core: TUI ordering; KAS `buildPermissionOptions` unchanged |
| 2.34.0 | 8 | [changed] [V3] The session dashboard asks `Do you want to delete …?` before deleting a session | tui: TUI-local (0 -> 1) · kas-core: TUI |
| 2.34.0 | 9 | [changed] [V3] Shell commands Kiro cannot parse are denied automatically instead of prompting you to approve t… | kas-core: KAS `PolicySession.unparseable()` (0.66.62; Deltas) |
| 2.34.0 | 10 | [changed] [V3] Subagents no longer get the MCP servers declared inline in their parent agent | kas-core: KAS-internal (`mcp.delegation.*`, `mcp.session.admission_denied` @0.66.62) |
| 2.34.0 | 11 | [fixed] Show `Working...` when the agent goes quiet while preparing a tool call instead of a frozen screen | tui: TUI-local (0 -> 1) · kas-core: TUI · host: TUI |
| 2.34.0 | 12 | [fixed] [V3] Headless mode no longer answers the agent's questions or approves requests that name no trusted t… | tui: TUI `noInteractive` guards; no wire change · host: **host** non-interactive client (`launch/secret_storage.rs` [0,8] = the saved-sign-in store) |
| 2.34.0 | 13 | [fixed] [V3] Headless mode reuses MCP sign-ins saved by interactive sessions | kas-core: host/TUI; MCP credential store · host: **host** non-interactive client (`launch/secret_storage.rs` [0,8] = the saved-sign-in store) |
| 2.34.0 | 14 | [fixed] [V3] `/sessions` opens and scrolls without freezing when you have many saved sessions | tui: TUI-local |
| 2.34.0 | 15 | [fixed] Enterprise overage status and consumed credits are visible in `/usage` | tui: **L** — TUI renders new fields of the existing `_kiro/account/getUsage` reply (`overage` 91 -> 103); field names are the kas-core lane's · kas-core: `_kiro/account/getUsage` consumer (TUI); no schema delta located · host: TUI/KAS (no host literal) |
| 2.34.0 | 16 | [fixed] [V3] Workflows pick up where they left off after a restart, without replanning or repeating steps | kas-core: kas-workflow · kas-workflow: 0.66.62 `classifyInterruptedNodes` (+ 0.66.43 pending hand-off/step-update restore) |
| 2.34.0 | 17 | [fixed] A refusal partway through a reply now continues on the fallback model instead of starting over | kas-core: model fallback (`model_fallback_auto_refusal` flag @0.66.58; default true @0.66.91) · host: host v2 api_client (logic) |
| 2.34.0 | 18 | [fixed] With `chat.allowIcons` off, the TUI drops box drawing and block characters, and arrow keys read as words | tui: TUI-local (1 == 1) · host: TUI |
| 2.34.0 | 19 | [fixed] Mermaid diagrams in chat render correctly in more cases instead of showing raw HTML or failing | tui: TUI-local · host: TUI |
| 2.34.0 | 20 | [fixed] [V3] Deny rules save at the scope you pick, and a call a rule blocks is no longer reported as cancelled | tui: **L** — consent echo now applies to `reject_always` (`Kk`: scope default `session` when a resource is chosen; `kiro:{consent` 1 -> 2) + KAS · kas-core: consent scope (pre-existing echo); status mapping literals unchanged in count; `policyDenial` moved into the tool_call_update builder (`withPolicyDenial`, hunks) — verify live (status `failed` + `_meta.kiro.policyDenial`) |
| 2.34.0 | 21 | [fixed] [V3] `/rewind` to a failed or unfinished turn now forks at the right point | kas-core: KAS-internal (`session.fork.snapshot_id_refused` @0.66.62) |
| 2.34.0 | 22 | [fixed] [V3] Tool approvals in the workflow monitor open on the current call and close with `q` | tui: TUI-local · kas-core: TUI · kas-workflow: TUI-only; no approval-shape change 0.66.58 -> 0.66.62 |
| 2.34.0 | 23 | [fixed] [V3] `kiro-cli acp` no longer shuts down unexpectedly while the agent is still running | kas-core: host (stdin/teardown); KAS `disposeAllSessionsOnce` unchanged in shape · host: host acp supervision (logic) |
| 2.34.0 | 24 | [fixed] Pasted images, and pasted or dictated text in menu and picker filters, now work as expected | tui: TUI-local · host: TUI |
| 2.34.0 | 25 | [fixed] [V3] Switching agents with `/agent` now loads the skills declared by the agent you switch to | kas-core: KAS-internal |
| 2.34.0 | 26 | [fixed] [V3] `ssh` and `ssh-keygen` no longer fail with `No user exists for uid` in the macOS sandbox | kas-core: sandbox / fs internal (macOS sandbox uid resolution); cyril has no sandbox (hand-placed) |
| 2.34.0 | 27 | [fixed] [V3] Deleting files outside the workspace folders no longer fails on Windows | kas-core: sandbox / fs internal |
| 2.34.0 | 28 | [security] [V3] Remote MCP servers over HTTP now follow only same-origin redirects | kas-core: KAS-internal (MCP client) · host: KAS (host rmcp unchanged) |
| 2.34.1 | 1 | [fixed] A queued message with blank lines no longer shows as multiple queued messages | kas-core: TUI · host: TUI |
| 2.34.2 | 1 | [changed] Kiro CLI now offers to switch you to V3 when you start a chat | kas-core: TUI/host · host: **host**: rollout `v3_prompt.treatment_percent` 50 → 100 (no code change) |
| 2.35.0 | 1 | [added] Warning when Kiro CLI runs as root or Administrator, with a global `app.warnOnElevatedPrivileges` sett… | tui: **L T26** (`warnOnElevatedPrivileges` 0 -> 2, `KIRO_ELEVATED_PRIVILEGES`) · host: **host** (`KIRO_ELEVATED_PRIVILEGES` [0,1], key [0,3], "Warning: Kiro CLI is running as root…") |
| 2.35.0 | 2 | [added] `/fullscreen` pins the prompt you are reading to the top row; click it to jump back to the prompt | tui: TUI-local · host: TUI |
| 2.35.0 | 3 | [changed] Original Kiro ghost greets you on startup and stays visible in the conversation | tui: TUI-local · host: TUI |
| 2.35.0 | 4 | [changed] A refused response on Auto is now retried so another model can answer instead of ending the turn | kas-core: `model_fallback_auto_refusal` (flag; backend-resolved until 0.66.91 default true) · host: **host** rollout `model_fallback_auto_refusal` → 100% all + `auth/mod.rs` |
| 2.35.0 | 5 | [changed] [V3] MCP servers and web tools now follow your organization's settings | kas-core: `Sr.mcpEnabled` startup configuration, `mcp_tools.enabled`/`web_tools.enabled` @0.66.70 (Deltas) |
| 2.35.0 | 6 | [fixed] Model fallback no longer hangs when a refused response contains a tool call | kas-core: KAS-internal / TUI · host: host v2 (logic) |
| 2.35.0 | 7 | [fixed] Dictation commands like "scratch that" and other bursts of Backspace now delete text in the chat promp… | tui: TUI-local · host: TUI |
| 2.35.0 | 8 | [fixed] [V3] Resuming a V2 session no longer warns that agent `default` was not found or drops the planner | host: host (`v2_builtin_default_agent.rs`, logic) |
| 2.35.0 | 9 | [fixed] [V3] On Linux, resuming a session after a reboot no longer reports it as open in another terminal | tui: TUI/host session lock; no wire · kas-core: host / KAS session liveness · host: host/KAS session lock (logic) |
| 2.35.0 | 10 | [fixed] `/chat` and `--resume` no longer time out when listing a large session history | tui: host IPC; no wire · kas-core: host/TUI · host: host v2 `persist.rs` (logic) |
| 2.35.0 | 11 | [fixed] [V3] `/mcp` now explains which configured servers your organization's MCP registry hid | tui: TUI render of existing `_kiro/mcp/status` data |
| 2.35.0 | 12 | [fixed] [V3] `kiro-cli mcp list` no longer shows a skipped registry server as active | host: **host** `cli/mcp` (logic) |
| 2.35.0 | 13 | [fixed] [V3] HTTP MCP servers that answer the protocol probe with HTTP 403 now connect instead of failing | kas-core: KAS-internal (`mcp.era.*` @0.66.70) / TUI |
| 2.35.0 | 14 | [fixed] [V3] A refusal with no output right after a tool result now ends the turn instead of looping | kas-core: KAS-internal (graph) |
| 2.35.0 | 15 | [fixed] [V3] `read_file` with an offset past the end of a file no longer reports the file as missing | kas-core: KAS-internal · host: KAS/TUI |
| 2.35.1 | 1 | [changed] Performance improvements | host: host-only release (logic; no literal); KAS and TUI byte-identical to 2.35.0 (hand-placed) |
| 2.35.2 | 1 | [changed] Retiring autocomplete desktop app - gradual rollout | kas-core: host |
| 2.36.0 | 1 | [added] [V3] Approve for me: Kiro approves routine actions, blocks risky ones and asks when unsure; turn it on… | tui: **L T10** (dark: `KIRO_TRUST_CLASSIFIER_MODE=enforce`) · kas-core: trust classifier (0.66.58; Deltas) + TUI `pit` rule · host: rollout `trust_classifier_enforce` → all users |
| 2.36.0 | 2 | [added] [V3] Shell approval rules offer a subcommand tier such as `git stash *` between the exact command and … | tui: **L T12** · kas-core: TUI-composed `match` pattern in the consent echo; KAS accepts any pattern (pre-existing) |
| 2.36.0 | 3 | [added] `kiro-cli agent upgrade` warns when the upgraded rules deny, hide, or ask for something V2 allowed | host: **host** (serde `V3PermissionRule [4]`, `DetectorConfig [9]`, "allowReadOnly let V2 read any file without asking…" [0,1]) |
| 2.36.0 | 4 | [added] [V3] Notice once per session for each agent whose V3 permission rules block what V2 allowed | tui: **L T27** (host IPC `permissionConflicts`) · kas-core: host / TUI · host: TUI (`KIRO_AGENT_UPGRADE_NOTICE` arrives 2.37.0) |
| 2.36.0 | 5 | [added] `acp.agentEngine` setting to choose the engine for `kiro-cli acp` | tui: **not in the TUI** (0 hits); host lane · kas-core: host (0 hits in KAS) · host: **host** [0,5] + `acp_engine.rs` router (the client lists are NOT in the feed) |
| 2.36.0 | 6 | [added] Click the Kiro logo ghost in `/fullscreen` to play the next of five animations | tui: TUI-local (`hop nod shake wink`) · host: TUI |
| 2.36.0 | 7 | [changed] [V3] Clearer questions with progress indicators during `/spec` requirements analysis | tui: TUI render + KAS; no new wire literal · kas-core: TUI / KAS spec |
| 2.36.0 | 8 | [fixed] [V3] Shell tool cards title themselves with the model's description and show the Shell command below i… | tui: TUI-local · host: KAS/TUI |
| 2.36.0 | 9 | [fixed] [V3] Rule rows on the approval prompt keep the file name of a long path instead of cutting it short | tui: TUI-local · kas-core: TUI · host: KAS/TUI |
| 2.36.0 | 10 | [fixed] Mermaid diagrams display titles and diagram-wide notes | tui: TUI-local · host: TUI |
| 2.36.0 | 11 | [fixed] Crash when an agent config lists unavailable MCP tools with non-ASCII names | tui: not located; host/KAS · kas-core: TUI / KAS-internal · host: host/TUI (logic) |
| 2.36.0 | 12 | [fixed] Indented lines in a wrapped tool command no longer render as an empty row in narrow terminals | tui: TUI-local · host: TUI |
| 2.36.0 | 13 | [security] [V3] Update the bundled Node.js runtime to 22.23.4 | kas-core: host runtime (RUN-STATE) · host: **host embedded frame** (124,679,552 → 124,902,584 B) |
| 2.36.0 | 14 | [security] Shell commands that set git config through env vars like `GIT_CONFIG_GLOBAL` now ask for approval | kas-core: shell policy (`GIT_CONFIG_GLOBAL` 0 hits in KAS: either host v2 or a regex table) — not located in KAS · host: **host v2** shell_permission (`GIT_CONFIG_GLOBAL` 0 → 2) |
| 2.36.0 | 15 | [security] Update the bundled Bun runtime to 1.4.4 for the Brotli decoder fix | host: **host embedded frame** (79,500,640 → 107,308,936 B) |
| 2.37.0 | 1 | [added] ACP clients can now sign in from a terminal when `kiro-cli acp` starts logged out | tui: **not in the TUI** (handler unchanged; `kiro-cli login` 16 == 16); host lane · kas-core: host; KAS side = `CredentialAbsentError` (0.66.48) lets the agent start without a token · host: **host** (`acp --login`, `cli/acp_sign_in.rs`, `terminal-auth` pointers) |
| 2.37.0 | 2 | [added] `KIRO_RUNTIME_DIR` sets where Kiro extracts its bundled runtime instead of the home directory | tui: **not in the TUI** (0 hits); host lane · host: **host** (`runtime_dir.rs`, `_ extract-runtime`) |
| 2.37.0 | 3 | [changed] Fullscreen keeps the prompt on the bottom rows, even with a short transcript or after Ctrl+L | tui: TUI-local · host: TUI |
| 2.37.0 | 4 | [changed] The Kiro logo ghost keeps its full color at rest instead of fading after the load animation | tui: TUI-local · host: TUI |
| 2.37.0 | 5 | [changed] On V3, V2-format agents are now upgraded automatically at launch; set `chat.enableAutoAgentUpgrade` … | tui: host (setting 0 hits in the TUI); the TUI part is the `KIRO_AGENT_UPGRADE_NOTICE` banner (T26) · host: **host** `launch/auto_migrate.rs` ("Auto-upgraded agent config to universal format" |
| 2.37.0 | 6 | [changed] [V3] Approval prompts in the main chat show only a multi-line command's first line until `ctrl+o` | tui: TUI-local · kas-core: TUI · host: TUI |
| 2.37.0 | 7 | [changed] [V3] The approval rule page highlights the scope choices, with the `s` hint beside them | tui: TUI-local · host: TUI |
| 2.37.0 | 8 | [changed] Mermaid diagrams preserve node shapes, type colors, and logical group markers | tui: TUI-local · host: TUI |
| 2.37.0 | 9 | [changed] `kiro-cli update` now suggests `brew upgrade --cask kiro-cli`, without `--greedy-auto-updates` | kas-core: TUI / host · host: **host** (`greedy-auto-updates` 2 → 0) |
| 2.37.0 | 10 | [fixed] `--no-interactive` runs that stop on an agent now say `stopping (exit 4)` instead of claiming a fallback | tui: host/TUI non-interactive; no wire · kas-core: host · host: **host** [0,1] |
| 2.37.0 | 11 | [fixed] [V3] `--no-interactive` runs stopped by a V2-format agent name `kiro-cli agent upgrade <dir>` | host: host (logic |
| 2.37.0 | 12 | [fixed] Clicks in fullscreen, including the scroll-to-bottom pill, no longer land one row above the target whe… | tui: TUI-local · host: TUI |
| 2.37.0 | 13 | [fixed] [V3] The entire-tool row on the approval rule page names the tool, such as Shell, instead of one call | tui: TUI-local (`entire tool${…}`) · host: TUI |
| 2.37.0 | 14 | [fixed] Mouse-wheel scrolling in `/fullscreen` works in AWS CloudShell and other terminals running older tmux … | tui: TUI-local · host: TUI |
| 2.37.0 | 15 | [fixed] The Kiro logo ghost stays on the home screen while a command menu such as `/agent` or a panel such as … | tui: TUI-local · host: TUI |
| 2.37.0 | 16 | [fixed] Mermaid flowcharts render nested containers instead of falling back to source | tui: TUI-local · host: TUI |
| 2.37.0 | 17 | [fixed] Mermaid diagrams fit long labels within narrow terminals | tui: TUI-local · host: TUI |
| 2.37.0 | 18 | [security] [V3] `/spec` prompts, review comments and `/spec view` summary rows no longer print escape sequence… | tui: TUI-local · kas-core: TUI · host: TUI |
| 2.37.1 | 1 | [changed] Refreshed look for the welcome screen, startup notices and What's new, without the startup tip | host: TUI |
| 2.37.1 | 2 | [changed] The `/fullscreen` scroll-to-bottom pill has a dark fill, lights up on hover, and its dot turns green… | host: TUI |
| 2.37.1 | 3 | [changed] [V3] An answer cut off at the model's output limit now says so and suggests asking Kiro to continue | kas-core: TUI (stop reason `max_tokens` pre-exists on the wire) · host: KAS/TUI |
| 2.37.1 | 4 | [fixed] [V3] Idle sessions use far less CPU tracking the engine's process tree | kas-core: host · host: **host** launch (logic) |
| 3.0.0 | 1 | [added] Kiro CLI V3 is now the default experience, with workflows, specs, cloud sessions and more | kas-workflow: policy; KAS 0.66.70 unchanged · host: **host**: `chat.agentEngine` persistence + switch prompts (strings since 2.37.0 |
| 3.0.0 | 2 | [removed] The Classic experience is retired; new capabilities are built in the terminal UI | host: **host** "Kiro CLI 3.0 no longer runs Classic (the v1 engine), so this run ignores the Classic selection." (2.37.0 string |
| 3.0.0 | 3 | [removed] Inline shell autocomplete and the CLI desktop app are retired | host: **host packaging**: launcher, term, `q`, `qchat` removed |
| 3.1.0 | 1 | [added] `PreCompact` and `PostCompact` hooks that run before and after a conversation is compacted | kas-core: KAS 0.66.91 (Deltas) |
| 3.1.0 | 2 | [added] `KIRO_DEFAULT_SETTINGS` names a root-owned file of default settings that your own `cli.json` overrides | tui: **L T25** |
| 3.1.0 | 3 | [added] `/usage` and an optional `Session credits` status line segment show the session credits so far | tui: **L T4** (`session_usage`) · kas-core: `session_usage` kind (Deltas) · kas-workflow: kas-core (`session_usage`); same `sumCreditsByUnit` as step credits |
| 3.1.0 | 4 | [added] Workflow step rows show the credits each step used | tui: **L T5** · kas-core: kas-workflow (`credits` on node payloads via `readSessionCredits`/`SessionUsage`) · kas-workflow: 0.66.91 `credits` on node state / `node_complete` / `run_complete` |
| 3.1.0 | 5 | [added] Session recap above the prompt after a few idle seconds; opt in through `/settings` under Features or … | tui: **L T2/T3** — engine-fed · host: **host key** `chat.enableSessionRecap` [0,3] + doc node (behaviour TUI) |
| 3.1.0 | 6 | [changed] Sub-agents can read the memories of the agent that dispatched them | kas-core: `memory.policy.parentResolved` (0.66.70) + 0.66.91 memory rollout |
| 3.1.0 | 7 | [changed] A prompt is refused with a sign-in message, and not saved, when Kiro cannot get a sign-in token | kas-core: `SignInRequiredError` (Deltas) |
| 3.1.0 | 8 | [changed] Agent `file://` resources are capped at 32 files and 100,000 characters; large files are cut or skip… | kas-core: new literal "…was cut to fit the agent's resource limits…" (numbers not extracted as literals) |
| 3.1.0 | 9 | [changed] Notifications queue into one row, most urgent first, with a `+N waiting` count | tui: TUI-local (` waiting` 5 -> 6) |
| 3.1.0 | 10 | [changed] Warnings and errors stay on screen until you dismiss them with a keypress | tui: TUI-local · kas-core: TUI |
| 3.1.0 | 11 | [changed] Startup cloud config, learnings and steering fetches no longer print tool cards; cloud config instea… | tui: TUI-local filter (`fetch_cloud_config` 0 -> 7) · kas-core: hidden-tool set `new Set(["fetch_cloud_config"])` pre-exists (1 = 1); the card suppression for learnings/steering is TUI |
| 3.1.0 | 12 | [changed] `web_fetch` asks sites for markdown, so docs pages keep their code blocks and links | kas-core: Accept header (Deltas) |
| 3.1.0 | 13 | [fixed] MCP tool results that return `structuredContent` alongside text or images now include that data as an … | tui: **not in the TUI** (0 hits); KAS (T29) · kas-core: **no KAS delta** (zero hunks); SDK helper in both |
| 3.1.0 | 14 | [fixed] Global skills now load in every session when `kiro-cli acp` is started from the home directory | kas-core: KAS-internal (skill discovery) |
| 3.1.0 | 15 | [fixed] `/agent swap kiro_default` now switches to the default agent | tui: not located to a TUI literal; KAS agent switch · kas-core: TUI |
| 3.1.0 | 16 | [fixed] A line of a reply occasionally missing or repeated in the chat transcript | tui: TUI-local |
| 3.1.0 | 17 | [fixed] A message sent while a sub-agent runs leaves the queued tray once the sub-agent reads it | tui: TUI-local |
| 3.1.0 | 18 | [fixed] A prompt keeps retrying an unanswered connection for 90 seconds instead of giving up sooner | kas-core: `KIRO_CONVERSE_CONNECTION_TIMEOUT_MS` pre-exists; KAS-internal |
| 3.1.0 | 19 | [fixed] A sub-agent stopped at its turn limit reports its unfinished work instead of an empty result | kas-core: Subagent Response text (Deltas) |
| 3.1.0 | 20 | [fixed] A token-endpoint outage fails the MCP tool call but keeps the server's sign-in | kas-core: `mcp.oauth.token_endpoint.answered` @0.66.91 |
| 3.1.0 | 21 | [fixed] A turn cut off by the output-token cap or the step limit now says it stopped early | tui: not in the TUI (`stopped early` 0); KAS copy · kas-core: stop reasons `max_tokens`/`max_turn_requests` pre-exist; wording is TUI; `[IterationLimit] Iteration limit reached` log · kas-workflow: kas-core |
| 3.1.0 | 22 | [fixed] `/context show` lists a `skill://` file that could not be loaded instead of leaving it out | kas-core: KAS-internal |
| 3.1.0 | 23 | [fixed] Sessions whose transcript grew past 512 MiB can be opened again | tui: TUI-local transcript store / host · kas-core: KAS-internal (persistence) |
| 3.1.0 | 24 | [fixed] Shell commands run in `bash` when it is available and `SHELL` is unset, instead of `/bin/sh` | kas-core: `getDefaultShell` (Deltas) |
| 3.1.0 | 25 | [fixed] Starting a session no longer deletes the logs of another session that is still running | kas-core: KAS-internal (log prune; `logsBasePath` 6 = 6) |
| 3.1.0 | 26 | [fixed] `str_replace` writes `$$`, `$&` and other `$` patterns in the new text as written | kas-core: KAS-internal |
| 3.1.0 | 27 | [fixed] Sub-agents load the `file://` resources their own agent profile declares | kas-core: `subagent.profileFiles.resolve` @0.66.91 |
| 3.1.0 | 28 | [fixed] A prompt sent just as a workflow run finishes is no longer dropped | kas-core: kas-workflow · kas-workflow: 0.66.91 `wakeTargetIsBusy` + `hasUnstoppedPromptInSetup` — WF-L4 |
| 3.1.0 | 29 | [fixed] Spec task runs record property-based test results without asking for approval | kas-core: KAS spec internal (property-based test result recording); no wire field (hand-placed) |
| 3.1.0 | 30 | [fixed] Workflow runs land their changes on your checked-out branch, not a leftover worktree branch | kas-workflow: not located in KAS 0.66.91 workflow code, prompts or recipes, nor in TUI 3.1.0 — WF-L10 |
| 3.1.0 | 31 | [fixed] `kiro-cli agent upgrade` and `/upgrade-agent` now warn when an agent lists V2 tools that V3 doesn't ha… | tui: host IPC; TUI renders the warning (`no-v3-tool` literal added 3.1.0) · kas-core: host · host: **host** ("V3 has no tool for these V2 tools, so V3 ignores them…" [0,1]) |
| 3.1.0 | 32 | [fixed] Tool cards in scrollback no longer show a `ctrl+o to toggle` hint after they stop responding to it | tui: TUI-local (`ctrl+o to toggle` 2 == 2; behavioural) · kas-core: TUI |
| 3.1.0 | 33 | [fixed] An empty model reply after a tool call ends the turn instead of retrying up to 300 times | kas-core: `graphs.empty_end_turn_after_tool_response` @0.66.91 |
| 3.1.0 | 34 | [fixed] MCP and web tools stay on for organization users when a later profile check fails | kas-core: `DiscoveredProfile` / governance (0.66.91) · host: host/KAS (new controlplane call `GetEffectiveEnterpriseManagementResourceWithToken` is the host-side candidate) [INFERENCE] |
| 3.1.0 | 35 | [fixed] MCP sign-in links disappear once their server connects, with the server named in the toast | tui: **L T31** — the TUI's 2.32.0 `_kiro/mcp/oauthCompleted` consumer is now fed: KAS 0.66.91 emits it (`oauthCompleted` 0 -> 2) and passes `serverName`/`oauthCompletion` on `_kiro/openExternalUrl` · kas-core: `_kiro/mcp/oauthCompleted` now sent (Deltas) |
| 3.1.0 | 36 | [fixed] "Run required tasks" on a spec finishes every required task instead of stopping partway | kas-core: KAS spec internal |
| 3.1.0 | 37 | [fixed] Shell commands that read input get end of input at once instead of waiting until they time out | kas-core: agent-hosted `stdio:["ignore",…]` (3 -> 5 sites); cyril nulls stdin already |
| 3.1.0 | 38 | [fixed] Mermaid sequence diagrams render multiline labels, branches, async arrows and `rect` regions correctly | tui: TUI-local |
| 3.1.0 | 39 | [fixed] `kiro-cli agent validate` now exits non-zero when the agent config is invalid or missing | kas-core: host · host: host `cli/agent` (logic) |
| 3.1.0 | 40 | [fixed] A damaged knowledge listing no longer prevents a session or sub-agent from starting | kas-core: `knowledge.listing.failed` @0.66.91 |
| 3.1.0 | 41 | [fixed] Agent requests reach the correct service hosts in isolated AWS regions | kas-core: `@aws-sdk/util-endpoints` + API-key region discovery (`DiscoveredProfile`) · host: host endpoint resolution (logic |
| 3.1.0 | 42 | [fixed] `PreToolUse` hooks that exit 2 block the tool even when they give no reason | kas-core: v2 hooks executor (Deltas) |
| 3.1.0 | 43 | [fixed] Stopping a Windows shell command also stops the child processes it started | kas-core: `taskkill_failed` kind @0.66.91 |
| 3.1.0 | 44 | [fixed] Sub-agent knowledge bases refresh when agent files change instead of serving stale text | kas-core: KAS-internal |
| 3.1.0 | 45 | [fixed] Resizing the terminal no longer blanks the chat or paints rows at the old width | tui: TUI-local |
| 3.1.0 | 46 | [fixed] Scrolled-back chat keeps the line you were reading when the terminal width changes | tui: TUI-local |
| 3.1.0 | 47 | [fixed] The footer's session credit total no longer resets to zero when a session is reopened | tui: **L T4** (`session_usage` with `replay:true`) · kas-core: `session_usage` replay on `session/load` |
| 3.1.0 | 48 | [fixed] `--no-interactive` runs and `kiro-cli acp` no longer print the agent's `[INFO]` startup lines on stderr | kas-core: KAS logger (`[KiroAgent]` info lines) / host; not pinned statically — check cyril's stderr capture live · host: **host** (logic |
| 3.1.0 | 49 | [fixed] `--no-interactive` output starts the text that follows a tool call on a new line | tui: host plain-text printer · kas-core: TUI/host · host: host launch text mode (logic) |
| 3.1.0 | 50 | [fixed] Cloud sessions in an unsupported region show as unavailable instead of failing every turn | tui: **L** — TUI maps the KAS error (`cloud_session_unsupported`, `CodeIntelligenceCloudSessionUnsupportedError` added 3.1.0) · kas-core: cloud |
| 3.1.0 | 51 | [fixed] Reopening a session after a crash during a save no longer drops the next entry written to its history | tui: TUI-local / host transcript · kas-core: KAS-internal (persistence) |
| 3.1.0 | 52 | [fixed] Pages over the 10 MB `web_fetch` limit are reported as too large instead of as a network error | kas-core: `kWe=10*1024*1024` pre-exists; error text change |
| 3.1.0 | 53 | [fixed] Zero-result `web_search` calls show the search provider's message instead of an empty result | kas-core: `parseSearchBody` + `readMessage` (`message:cJo(s)`) @0.66.91 |
| 3.1.0 | 54 | [fixed] Tool output stays on the same screen row when expanded or collapsed in fullscreen chat | tui: TUI-local |
| 3.1.0 | 55 | [security] Tool-use telemetry is exported when an organization turns on only an OpenTelemetry export, not just… | host: **host** telemetry (logic |
| 3.1.0 | 56 | [security] Remote MCP server registrations and credential folders are now owner-only on Unix | kas-core: `configuration.userLocalSettings.write` passes a `mode:` constant on mkdir (`mode:Mi`, minified name; the 0 -> 38 count is a rename artefact and the value was not read) — weak static evidence · host: KAS or host `secret_storage`/ `owner_only_sweep` (no new host literal |
| 3.1.0 | 57 | [security] [V2] `git` commands prefixed with `GIT_DIR` or `GIT_WORK_TREE` now ask for approval | kas-core: v2 host (out of lane) · host: **host v2** (`GIT_DIR` 0 → 2, `GIT_WORK_TREE` 0 → 2) |

_188 lines; 0 unplaced after hand placement of 7 bundled kas-core rows_
