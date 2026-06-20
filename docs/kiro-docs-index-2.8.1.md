# Kiro CLI 2.8.1 — embedded documentation index (extracted)

Extracted from `kiro-cli-chat` 2.8.1 — two embedded metadata manifests (`total_docs` 82 @ ~5.4 MB and 118 @ ~440 MB), merged + deduped by `path` → **134 unique doc nodes**. Metadata only — the binary embeds the index, NOT the `.md` bodies (those are served from kiro.dev/docs). This is why Kiro can name/describe a doc but reports it 'couldn't retrieve its contents'.

| category | path | title | description |
|---|---|---|---|
| command | `commands/acp.md` | kiro-cli acp | Start the Agent Client Protocol (ACP) agent for programmatic client integration |
| command | `commands/agent.md` | kiro-cli agent | Manage agent configurations including list, validate, create, edit, migrate, and set-default operations |
| command | `commands/chat.md` | kiro-cli chat | Start AI assistant session with support for agents, models, tool trust, and conversation management |
| command | `commands/diagnostic.md` | kiro-cli diagnostic | Run diagnostic tests and generate system information report for troubleshooting |
| command | `commands/login.md` | kiro-cli login | Authenticate with Kiro CLI service using Builder ID, Social (Google/GitHub), or Identity Center |
| command | `commands/logout.md` | kiro-cli logout | Sign out of Kiro CLI service and clear authentication credentials |
| command | `commands/mcp.md` | kiro-cli mcp | Manage Model Context Protocol servers with add, remove, list, import, and status operations |
| command | `commands/settings.md` | kiro-cli settings | Configure Kiro CLI behavior at global and workspace levels with get, set, list, open, and delete operations |
| command | `commands/update.md` | kiro-cli update | Check for and install Kiro CLI updates |
| command | `commands/whoami.md` | kiro-cli whoami | Display current login session information including user and authentication method |
| feature | `features/agent-configuration.md` | Agent Configuration | Complete guide to agent configuration format including tools, settings, resources, hooks, and MCP servers |
| feature | `features/classic-vs-tui.md` | Classic Mode vs New TUI | Differences between classic mode (V1) and the new TUI experience, including what changed, what's new, and how to switch |
| feature | `features/cmux-integration.md` | cmux Integration | Agent status reporting in the cmux sidebar when running inside cmux |
| feature | `features/code-intelligence.md` | Code Intelligence | Code understanding with tree-sitter (built-in) and LSP integration (optional) for symbol search, pattern matching, and codebase exploration |
| feature | `features/diff-tool.md` | Custom diff tool | Configure an custom diff tool for viewing write tool changes |
| feature | `features/exit-codes.md` | Exit Codes | CLI exit codes for scripting and CI/CD integration |
| feature | `features/experiments.md` | Experiments | Experimental features including tangent mode, thinking, knowledge, todo lists, checkpoints, and delegate |
| feature | `features/file-references.md` | File References | Use @path syntax to include file contents or directory listings inline in chat messages |
| feature | `features/help-agent.md` | Help Agent | Built-in agent that answers questions about Kiro CLI features using documentation |
| feature | `features/hooks.md` | Hooks System | Execute commands at trigger points with JSON input/output and exit code control |
| feature | `features/knowledge-management.md` | Knowledge Management | Persistent knowledge base with semantic search, agent isolation, and auto-sync capabilities |
| feature | `features/mcp-registry.md` | MCP Registry | Enterprise MCP server governance allowing administrators to control which servers users can access |
| feature | `features/mid-turn-steering.md` | Mid-Turn Steering | Send messages to the agent while it's working to redirect or guide its approach |
| feature | `features/paste-chips.md` | Paste Chips | Large pasted text collapses into expandable chips to keep the prompt input readable |
| feature | `features/planning-agent.md` | Planning Agent | Built-in agent that transforms ideas into structured implementation plans with requirements gathering and task breakdown |
| feature | `features/research-surveys.md` | Research Surveys | Optional in-session surveys to rate your experience with Kiro CLI, plan quality, and implementation quality |
| feature | `features/session-management.md` | Session Management | Automatic session saving, resumption, and custom storage via scripts |
| feature | `features/steering-files.md` | steering-files | Markdown files that provide persistent instructions and rules to guide agent behavior across sessions |
| feature | `features/tangent-mode.md` | Tangent Mode | Experimental feature for creating conversation checkpoints to explore side topics without disrupting main flow |
| feature | `features/terminal-progress-indicator.md` | Terminal progress indicator | OSC 9;4 progress indicator in the terminal tab/title bar |
| feature | `features/trust-configuration.md` | Trust Configuration | Configure tool auto-approval at session, agent, and directory levels |
| feature | `features/voice-mode.md` | Voice Mode | Hands-free speech-to-text input for chat using local Whisper transcription |
| setting | `settings/cleanup-period-days.md` | cleanup.periodDays | Days after which old conversations, sessions, and knowledge bases are deleted |
| setting | `settings/context-usage-indicator.md` | chat.enableContextUsageIndicator | Show context usage percentage in prompt |
| setting | `settings/default-agent.md` | chat.defaultAgent | Set default agent configuration for new chat sessions |
| setting | `settings/default-interrupt-mode.md` | chat.defaultInterruptBehavior | Default follow-up delivery mode for new chat sessions |
| setting | `settings/default-model.md` | chat.defaultModel | Set default AI model for new chat sessions |
| setting | `settings/diff-tool-settings.md` | chat.diffTool | Configure an custom diff tool for viewing write tool changes |
| setting | `settings/disable-markdown-rendering.md` | chat.disableMarkdownRendering | Disable markdown formatting in chat output for plain text display |
| setting | `settings/disable-wrap.md` | chat.disableWrap | Disable line wrapping in chat output for clean copy-paste of long lines |
| setting | `settings/enable-checkpoint.md` | chat.enableCheckpoint | Enable checkpoint feature for creating workspace snapshots |
| setting | `settings/enable-code-intelligence.md` | chat.enableCodeIntelligence | Enable code intelligence with LSP integration |
| setting | `settings/enable-knowledge.md` | chat.enableKnowledge | Enable knowledge base functionality for persistent context storage |
| setting | `settings/enable-tangent-mode.md` | chat.enableTangentMode | Enable tangent mode feature for conversation checkpoints |
| setting | `settings/enable-thinking.md` | chat.enableThinking | Enable thinking tool for complex reasoning |
| setting | `settings/enable-todo-list.md` | chat.enableTodoList | Enable TODO list feature for task tracking |
| setting | `settings/greeting-enabled.md` | chat.greetingEnabled | Show or hide greeting message when starting chat sessions |
| setting | `settings/hooks-show-status.md` | hooks.showStatus | Show or hide hook execution status messages (spinner and summary) |
| setting | `settings/introspect-progressive-mode.md` | introspect.progressiveMode | Use progressive loading instead of semantic search for introspect |
| setting | `settings/introspect-tangent-mode.md` | chat.introspectTangentMode | Auto-enter tangent mode for introspect questions |
| setting | `settings/model-defaults.md` | chat.modelDefaults | Per-model additional field defaults like effort level |
| setting | `settings/show-thinking.md` | chat.showThinking | Control how reasoning/thinking blocks are displayed (collapsed, expanded, or off) |
| setting | `settings/tangent-mode-key.md` | chat.tangentModeKey | Configure keyboard shortcut for tangent mode toggle |
| setting | `settings/terminal-title.md` | chat.terminalTitle | Update terminal window title with session info |
| setting | `settings/voice-settings.md` | Voice Settings | Configuration options for voice input mode including model size, timeouts, and language |
| settings-group | `settings/api-service-settings.md` | API and Service Settings | Settings for API timeouts and service configurations |
| settings-group | `settings/chat-interface-settings.md` | Chat Interface Settings | Settings for chat interface behavior and appearance |
| settings-group | `settings/key-bindings-settings.md` | Key Bindings Settings | Settings for keyboard shortcuts and key bindings |
| settings-group | `settings/knowledge-base-settings.md` | Knowledge Base Settings | Settings for knowledge base functionality and indexing |
| settings-group | `settings/mcp-settings.md` | MCP Settings | Settings for Model Context Protocol (MCP) configuration |
| settings-group | `settings/telemetry-privacy-settings.md` | Telemetry and Privacy Settings | Settings for telemetry collection and privacy controls |
| slash_command | `slash-commands/agent-create.md` | /agent create | Create a new agent with AI assistance or manual mode |
| slash_command | `slash-commands/agent-edit.md` | /agent edit | Edit an existing agent configuration |
| slash_command | `slash-commands/agent-generate.md` | /agent generate | Alias for /agent create - Create agent with AI assistance |
| slash_command | `slash-commands/agent-schema.md` | /agent schema | Show agent config schema |
| slash_command | `slash-commands/agent-set-default.md` | /agent set-default | Define a default agent to use when kiro-cli chat launches |
| slash_command | `slash-commands/agent-show.md` | /agent show | Display current agent configuration with syntax highlighting |
| slash_command | `slash-commands/agent-swap.md` | /agent | Switch to different agent configuration during chat session |
| slash_command | `slash-commands/changelog.md` | /changelog | View Kiro CLI changelog and version history with recent updates |
| slash_command | `slash-commands/chat-load.md` | /chat load | Load previously saved conversation from file to resume session |
| slash_command | `slash-commands/chat-new.md` | /chat new | Start a fresh conversation without restarting the CLI |
| slash_command | `slash-commands/chat-save-via-script.md` | /chat save-via-script | Save the current chat session using a custom script that receives conversation JSON via stdin |
| slash_command | `slash-commands/chat-save.md` | /chat save | Save current conversation to file or database for later resumption |
| slash_command | `slash-commands/checkpoint.md` | /checkpoint | Manage workspace checkpoints with init, list, restore, expand, diff, and clean operations |
| slash_command | `slash-commands/clear.md` | /clear | Erase conversation history and context from current session |
| slash_command | `slash-commands/code.md` | /code | Manage code intelligence with init, status, logs, overview, and summary subcommands |
| slash_command | `slash-commands/compact.md` | /compact | Summarize conversation history to free context space while preserving essential information |
| slash_command | `slash-commands/context.md` | /context | View context window usage and manage context files with add, remove, show, and clear operations |
| slash_command | `slash-commands/copy.md` | /copy | Copy the last assistant response to the system clipboard |
| slash_command | `slash-commands/editor.md` | /editor | Open $EDITOR to compose multi-line prompts with optional initial text |
| slash_command | `slash-commands/effort.md` | /effort | Set reasoning effort level for the current model |
| slash_command | `slash-commands/experiment.md` | /experiment | Toggle experimental features like tangent mode, thinking, knowledge, and checkpoints |
| slash_command | `slash-commands/feedback.md` | feedback | Submit feedback, request features, or report issues |
| slash_command | `slash-commands/goal.md` | /goal | Set a goal with validation criteria for iterative agent completion |
| slash_command | `slash-commands/guide.md` | /guide | Switch to the guide agent for help with Kiro CLI features and commands |
| slash_command | `slash-commands/help.md` | /help | Switch to the Help Agent to ask questions about Kiro CLI features and commands |
| slash_command | `slash-commands/hooks.md` | /hooks | View context hooks configuration and execution status |
| slash_command | `slash-commands/issue.md` | /issue | Create GitHub issue or feature request with pre-filled template |
| slash_command | `slash-commands/kiro-cli chat-delete-session.md` | /chat delete | Delete saved chat session by ID |
| slash_command | `slash-commands/kiro-cli-chat-list.md` | /chat list | List all saved chat sessions for current directory |
| slash_command | `slash-commands/knowledge.md` | /knowledge | Manage knowledge base with add, search, remove, show, and clear operations |
| slash_command | `slash-commands/logdump.md` | /logdump | Create zip file with diagnostic logs for support investigation |
| slash_command | `slash-commands/mcp.md` | /mcp | View MCP server status, authentication requirements, and available tools |
| slash_command | `slash-commands/model.md` | /model | Select AI model for current conversation session |
| slash_command | `slash-commands/paste.md` | /paste | Paste image from system clipboard into conversation for vision model analysis |
| slash_command | `slash-commands/plan.md` | /plan | Switch to Plan agent for breaking down ideas into implementation plans |
| slash_command | `slash-commands/prompts.md` | /prompts | Manage local and MCP prompts with list, get, create, edit, and remove operations |
| slash_command | `slash-commands/quit.md` | /quit | Exit the chat session and return to terminal |
| slash_command | `slash-commands/reply.md` | /reply | Open $EDITOR with most recent assistant message quoted for reply |
| slash_command | `slash-commands/rewind.md` | /rewind | Fork conversation at an earlier turn into a new session |
| slash_command | `slash-commands/settings.md` | /settings | Open the settings menu to configure theme, keybindings, terminal, and other preferences |
| slash_command | `slash-commands/spawn.md` | /spawn | Spawn a new agent session with a task to run in parallel |
| slash_command | `slash-commands/stats.md` | /stats | Show request IDs and timings for debugging slow turns |
| slash_command | `slash-commands/tangent.md` | /tangent | Create conversation checkpoints to explore side topics without disrupting main conversation flow |
| slash_command | `slash-commands/theme.md` | /theme | Select and customize the terminal color theme |
| slash_command | `slash-commands/title.md` | /title | Set, clear, or show the terminal window title |
| slash_command | `slash-commands/todo.md` | /todo | View, manage, and resume TODO lists with clear-finished, resume, view, and delete operations |
| slash_command | `slash-commands/tools.md` | /tools | View available tools and manage tool permissions with trust, untrust, and reset operations |
| slash_command | `slash-commands/transcript.md` | /transcript | View or save the full conversation transcript in multiple formats |
| slash_command | `slash-commands/usage.md` | /usage | Show billing and credits information for current session |
| slash_command | `slash-commands/voice.md` | /voice | Start voice input mode for hands-free speech-to-text prompts |
| tool | `tools/code.md` | code | Code intelligence with tree-sitter (built-in) and LSP (optional) for symbol search, pattern matching, and codebase exploration |
| tool | `tools/delegate.md` | delegate | Launch and manage asynchronous agent processes running independently in background |
| tool | `tools/glob.md` | glob | Find files and directories matching glob patterns with .gitignore support |
| tool | `tools/goal.md` | goal | Built-in tool for signaling goal completion or checking progress |
| tool | `tools/grep.md` | grep | Fast regex pattern search in files with configurable output modes and limits |
| tool | `tools/introspect.md` | introspect | Self-awareness tool providing information about Kiro CLI capabilities and documentation |
| tool | `tools/knowledge.md` | knowledge | Store and retrieve information across chat sessions with semantic search capabilities |
| tool | `tools/read.md` | read | Read files, directories, and images with support for line ranges, pattern search, and batch operations |
| tool | `tools/report-issue.md` | report_issue | Open browser with pre-filled GitHub issue template for reporting bugs or feature requests |
| tool | `tools/session-management.md` | session-management | Agent-to-agent orchestration tool for spawning sessions, messaging, and group management |
| tool | `tools/shell.md` | shell | Execute shell commands with output capture. Also known as execute_bash (legacy alias). |
| tool | `tools/subagent.md` | subagent | Spawn and coordinate multiple AI agents in a pipeline (DAG) with dependency management |
| tool | `tools/summary.md` | summary | Subagent tool for reporting task results back to the main agent |
| tool | `tools/switch-to-execution.md` | switch_to_execution | Internal tool for execution mode transitions (not user-facing) |
| tool | `tools/task.md` | task | Task list tool for tracking multi-step work with create, complete, add, remove, and list commands |
| tool | `tools/thinking.md` | thinking | Internal reasoning tool for complex problem-solving and decision-making |
| tool | `tools/todo-list.md` | todo_list | Create and manage TODO lists for tracking multi-step tasks with progress and context |
| tool | `tools/tool-search.md` | tool_search | Find and load MCP tools on demand to reduce context window usage |
| tool | `tools/use-aws.md` | use_aws | Make AWS CLI API calls with service, operation, and parameters |
| tool | `tools/use-subagent.md` | use_subagent | Delegate tasks to specialized subagents running in parallel with isolated context |
| tool | `tools/web-fetch.md` | web_fetch | Fetch and extract content from specific URLs with selective, truncated, or full modes |
| tool | `tools/web-search.md` | web_search | Search the web for current information with automatic source citation |
| tool | `tools/write.md` | write | Create and modify files with support for create, str_replace, insert, and append operations |
