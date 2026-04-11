# Workflows

> Generated: 2026-04-11 | Codebase: Cyril

## Application Startup

```mermaid
sequenceDiagram
    participant User
    participant Main as main.rs
    participant Bridge as spawn_bridge()
    participant App
    participant Agent as kiro-cli acp

    User->>Main: cyril [--cwd] [--prompt] [--agent]
    Main->>Main: Parse CLI args (clap)
    Main->>Main: Setup JSON file logging
    Main->>Main: Load config from ~/.config/cyril/config.toml
    Main->>Bridge: spawn_bridge(agent_name, cwd)
    Bridge->>Agent: Spawn subprocess (stdio piped)
    Bridge->>Bridge: Create channel pairs
    Bridge->>Bridge: Start bridge loop (tokio::spawn)
    Bridge-->>Main: BridgeHandle
    Main->>App: App::new(bridge, max_messages)
    App->>App: Split BridgeHandle → sender + receivers
    App->>App: Register builtin commands
    App->>App: create_initial_session(cwd)
    App->>Agent: BridgeCommand::NewSession
    Agent-->>App: SessionCreated notification
    App->>App: Initialize terminal (ratatui + mouse capture)
    App->>App: Enter event loop
```

## Event Loop (App::run)

```mermaid
graph TB
    subgraph "tokio::select! (biased)"
        P1[Priority 1:<br/>Terminal Events]
        P2[Priority 2:<br/>Permission Requests]
        P3[Priority 3:<br/>Notifications]
        P4[Priority 4:<br/>Redraw Timer]
    end

    P1 -->|KeyEvent| HANDLE_KEY[handle_key]
    P1 -->|MouseEvent| HANDLE_MOUSE[handle mouse]
    P1 -->|Resize| SET_SIZE[set_terminal_size]
    P2 --> SHOW_APPROVAL[show_approval]
    P3 --> ROUTE[Route notification]
    P4 --> REDRAW[terminal.draw]

    HANDLE_KEY -->|has approval| APPROVAL_KEY[handle_approval_key]
    HANDLE_KEY -->|has picker| PICKER_KEY[handle_picker_key]
    HANDLE_KEY -->|has hooks panel| HOOKS_KEY[handle_hooks_panel_key]
    HANDLE_KEY -->|normal| INPUT_KEY[handle_input_key / submit_input]

    ROUTE -->|session matches main| APPLY_MAIN[apply to UiState + Session]
    ROUTE -->|session is subagent| APPLY_SUB[apply to SubagentUiState]
    ROUTE -->|global| APPLY_GLOBAL[apply to main pipeline]
```

## User Input → Agent Response

```mermaid
sequenceDiagram
    participant User
    participant App
    participant Commands as CommandRegistry
    participant Bridge as BridgeSender
    participant Agent as kiro-cli acp
    participant UiState

    User->>App: Enter key pressed
    App->>App: take_input() from UiState
    App->>Commands: parse(input)
    alt Input is a command
        Commands->>Commands: execute(ctx, args)
        Commands-->>App: CommandResult
        App->>UiState: Apply result (system msg / picker / quit)
    else Input is a prompt
        App->>UiState: add_user_message(text)
        App->>UiState: set_activity(Sending)
        App->>Bridge: BridgeCommand::SendPrompt
        Bridge->>Agent: JSON-RPC prompt
        loop Streaming
            Agent-->>App: AgentMessage (is_streaming=true)
            App->>UiState: apply_notification → streaming_text
        end
        Agent-->>App: TurnCompleted
        App->>UiState: commit_streaming()
        App->>UiState: set_activity(Idle)
    end
```

## Tool Call Lifecycle

```mermaid
sequenceDiagram
    participant Agent as kiro-cli acp
    participant Bridge
    participant App
    participant UiState

    Agent->>Bridge: SessionNotification (ToolCall)
    Bridge->>App: RoutedNotification (ToolCallStarted)
    App->>UiState: apply_notification → add to active_tool_calls
    Note over UiState: Activity = ToolRunning

    loop Updates
        Agent->>Bridge: SessionNotification (ToolCallUpdate)
        Bridge->>App: RoutedNotification (ToolCallUpdated)
        App->>UiState: merge_update on existing tool call
    end

    alt Needs Permission
        Agent->>Bridge: request_permission
        Bridge->>App: PermissionRequest
        App->>UiState: show_approval(tool_call, message, options)
        Note over UiState: Render approval overlay
        App-->>Bridge: PermissionResponse (via oneshot)
    end

    Agent->>Bridge: SessionNotification (ToolCall, status=Completed)
    Bridge->>App: RoutedNotification (ToolCallUpdated, Completed)
    App->>UiState: update status, move to message history
```

## Subagent Lifecycle

```mermaid
sequenceDiagram
    participant User
    participant App
    participant Bridge
    participant Agent as kiro-cli acp
    participant Tracker as SubagentTracker
    participant SubUI as SubagentUiState

    User->>App: /spawn worker "do task"
    App->>Bridge: BridgeCommand::SpawnSession(name, task)
    Bridge->>Agent: session/spawn
    Agent-->>App: SubagentSpawned(session_id, name)
    App->>Tracker: register subagent

    loop Subagent working
        Agent-->>App: RoutedNotification(session_id=sub, AgentMessage)
        App->>App: session_id ≠ main → route to subagent
        App->>SubUI: apply_notification(session_id, notification)
    end

    Agent-->>App: SubagentListUpdated (status=Terminated)
    App->>Tracker: apply_notification
    App->>SubUI: mark_terminated

    User->>App: Focus subagent (crew panel)
    App->>SubUI: focus(session_id)
    Note over SubUI: Chat widget shows subagent stream
```

## Command Execution (Agent Commands)

```mermaid
sequenceDiagram
    participant User
    participant App
    participant Registry as CommandRegistry
    participant Bridge
    participant Agent as kiro-cli acp

    User->>App: /tools (or /context, /usage, etc.)
    App->>Registry: parse("/tools")
    Registry-->>App: AgentCommand (selection_type=false)
    App->>Bridge: BridgeCommand::ExecuteCommand("tools", session_id, {})
    Bridge->>Agent: kiro.dev/commands/execute
    Agent-->>App: CommandExecuted(command, response)
    App->>App: format_command_response(response)
    App->>App: add_command_output or show_picker

    Note over App: Selection-type commands (e.g., /model)
    User->>App: /model
    App->>Registry: parse("/model")
    Registry-->>App: AgentCommand (selection_type=true)
    App->>Bridge: BridgeCommand::QueryCommandOptions("model", session_id)
    Bridge->>Agent: kiro.dev/commands/options
    Agent-->>App: CommandOptionsReceived(options)
    App->>App: show_picker(title, options)
    User->>App: Select option
    App->>Bridge: BridgeCommand::ExecuteCommand("model", session_id, {value: selected})
```

## Rendering Pipeline

```mermaid
graph TB
    TIMER[Redraw timer tick] --> DRAW[render::draw]
    DRAW --> LAYOUT[Layout::vertical]
    LAYOUT --> TOOLBAR[toolbar::render]
    LAYOUT --> CHAT[chat::render]
    LAYOUT --> CREW[crew_panel::render]
    LAYOUT --> INPUT[input::render]
    LAYOUT --> STATUS[toolbar::render_status_bar]

    CHAT --> MD[markdown::render]
    CHAT --> DIFF[render_diff_lines]
    CHAT --> TC[render_tool_call]
    CHAT --> DRILL[render_subagent_drill_in]

    MD --> PULLDOWN[pulldown-cmark parser]
    MD --> SYNTECT[syntect highlighter]
    MD --> CACHE[LRU cache]

    subgraph "Overlays (on top)"
        APPROVAL[approval::render]
        PICKER[picker::render]
        HOOKS[hooks_panel::render]
    end
```

## Adaptive Frame Rate

```mermaid
stateDiagram-v2
    state "Idle (500ms)" as IDLE
    state "Ready (200ms)" as READY
    state "Sending (100ms)" as SENDING
    state "Waiting (100ms)" as WAITING
    state "Streaming (33ms / 30fps)" as STREAMING
    state "ToolRunning (100ms)" as TOOL

    [*] --> IDLE
    IDLE --> SENDING: user submits
    SENDING --> WAITING: prompt sent
    WAITING --> STREAMING: first agent text
    STREAMING --> TOOL: tool call started
    TOOL --> STREAMING: tool completed
    STREAMING --> IDLE: turn completed
    IDLE --> READY: any interaction
    READY --> IDLE: 2s no activity (deep idle)
```

The `redraw_duration()` method maps `Activity` to frame intervals. Deep idle (2s+ no activity) further reduces to 1s intervals.

## Path Translation (Windows/WSL)

```mermaid
graph LR
    WIN[C:\Users\name\file.txt] -->|win_to_wsl| WSL[/mnt/c/Users/name/file.txt]
    WSL -->|wsl_to_win| WIN
    JSON[JSON payload] -->|translate_paths_in_json| JSON2[Translated JSON]
```

Automatic recursive translation in JSON payloads. Detects paths by drive letter prefix (`C:\`) or WSL mount prefix (`/mnt/`).
