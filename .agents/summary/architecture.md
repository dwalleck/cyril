# Architecture

> Generated: 2026-04-11 | Codebase: Cyril

## System Overview

Cyril is a three-crate Rust workspace that implements a TUI frontend for Kiro CLI over the Agent Client Protocol (ACP). The architecture separates concerns into protocol logic (`cyril-core`), UI state and rendering (`cyril-ui`), and application orchestration (`cyril`).

```mermaid
graph TB
    subgraph "cyril (binary)"
        MAIN[main.rs<br/>CLI parsing, bridge spawn]
        APP[App<br/>Event loop, orchestration]
    end

    subgraph "cyril-ui (library)"
        STATE[UiState<br/>State machine]
        RENDER[render.rs<br/>Frame layout]
        WIDGETS[Widgets<br/>chat, input, toolbar,<br/>crew_panel, hooks_panel,<br/>picker, approval, markdown]
        TRAITS[TuiState trait<br/>Read-only view]
    end

    subgraph "cyril-core (library)"
        BRIDGE[BridgeHandle / BridgeSender<br/>Channel pair]
        CLIENT[KiroClient<br/>ACP Client impl]
        CONVERT[convert.rs<br/>Notification conversion]
        COMMANDS[CommandRegistry<br/>Trait-based commands]
        SESSION[SessionController<br/>Session state]
        SUBAGENT[SubagentTracker<br/>Multi-session tracking]
        TYPES[types/<br/>Domain types]
    end

    subgraph "External"
        KIRO[kiro-cli acp<br/>Agent process]
        ACP_CRATE[agent-client-protocol<br/>ACP trait + transport]
    end

    MAIN --> APP
    APP --> STATE
    APP --> SESSION
    APP --> COMMANDS
    APP --> BRIDGE
    STATE -.->|implements| TRAITS
    RENDER -->|reads| TRAITS
    RENDER --> WIDGETS
    BRIDGE --> CLIENT
    CLIENT --> ACP_CRATE
    CLIENT --> CONVERT
    ACP_CRATE <-->|JSON-RPC 2.0 stdio| KIRO
    CONVERT --> TYPES
    COMMANDS --> BRIDGE
    APP --> SUBAGENT
```

## Core Architectural Patterns

### Bridge Pattern (Channel-Based IPC)

The bridge is the central communication hub between the App and the ACP agent process. It uses three async channels:

```mermaid
graph LR
    subgraph "App Thread"
        APP[App]
        SENDER[BridgeSender<br/>Clone + Send]
    end

    subgraph "Bridge Thread (!Send)"
        CLIENT[KiroClient]
        ACP[ACP transport]
    end

    APP -->|BridgeCommand| SENDER
    SENDER -->|mpsc 32| CLIENT
    CLIENT -->|RoutedNotification<br/>mpsc 256| APP
    CLIENT -->|PermissionRequest<br/>mpsc 16| APP
    ACP <-->|stdio JSON-RPC| KIRO[kiro-cli acp]
```

- `BridgeCommand` — App → Bridge: prompts, session control, agent commands
- `RoutedNotification` — Bridge → App: agent output, tool calls, metadata
- `PermissionRequest` — Bridge → App: approval dialogs (oneshot response)

`BridgeHandle` is split into `BridgeSender` (cloneable, passed to commands) and two receivers consumed by `tokio::select!` in the event loop.

### Routed Notification System

Every notification from the ACP bridge carries an optional `session_id` for routing:

- `session_id == None` → global notification (bridge lifecycle, subagent list updates)
- `session_id == Some(id)` matching main session → dispatched to main state machines
- `session_id == Some(id)` not matching main → routed to `SubagentUiState`

This enables multi-session support where the main session and subagent sessions share a single bridge connection.

### State / Renderer Separation (TuiState Trait)

The renderer receives `&dyn TuiState` — a read-only trait — and cannot mutate application state. `UiState` implements `TuiState` and owns all mutable UI state.

```mermaid
graph TB
    APP[App] -->|mutates| UISTATE[UiState]
    UISTATE -.->|implements| TRAIT[TuiState trait]
    RENDER[render::draw] -->|reads| TRAIT
    RENDER --> WIDGETS[Widget functions]
```

This enforces a unidirectional data flow: mutations happen in the App event loop, rendering is a pure function of state.

### Command Registry Pattern

Commands are registered as trait objects implementing `Command`:

```mermaid
graph TB
    REGISTRY[CommandRegistry] -->|lookup| CMD[dyn Command]
    CMD -->|execute| RESULT[CommandResult]
    RESULT -->|variant| SYS[SystemMessage]
    RESULT -->|variant| PICK[ShowPicker]
    RESULT -->|variant| DISP[Dispatched]
    RESULT -->|variant| QUIT[Quit]
    RESULT -->|variant| NAC[NotACommand]
```

Builtin commands (`help`, `clear`, `quit`, `new`, `load`) are registered at startup. Agent commands from the server are dynamically registered via `register_agent_commands()`. Subagent commands (`spawn`, `kill`, `msg`, `sessions`) are registered separately.

### Notification Conversion Layer

`convert.rs` is the largest file in the codebase. It translates raw ACP protocol messages into typed `Notification` variants:

```mermaid
graph LR
    ACP_SESSION[acp::SessionNotification] --> CONVERT[convert.rs]
    ACP_EXT[acp::ExtNotification] --> CONVERT
    ACP_PERM[acp::RequestPermission] --> CONVERT
    CONVERT --> NOTIF[Notification variants]
    CONVERT --> TOOL[ToolCall construction]
    CONVERT --> PERM[PermissionRequest]
```

The conversion layer also maintains a `tool_call_inputs` cache (via `RefCell<HashMap>`) because permission requests arrive without `raw_input` — the client looks it up from previously cached tool call notifications.

## Crate Dependency Graph

```mermaid
graph TD
    CYRIL[cyril<br/>binary] --> CORE[cyril-core]
    CYRIL --> UI[cyril-ui]
    UI --> CORE
    CORE --> ACP[agent-client-protocol]
```

`cyril-core` has no dependency on `cyril-ui`. The binary crate depends on both.

## Event Loop Architecture

The App's `run()` method uses `tokio::select!` with biased priority:

1. **Terminal input** (highest) — keyboard/mouse events from crossterm
2. **Permission requests** — approval dialogs from the bridge
3. **Notifications** — agent output, tool calls, metadata updates
4. **Redraw timer** — adaptive frame rate based on `Activity` state

```mermaid
stateDiagram-v2
    [*] --> Idle
    Idle --> Sending: user submits prompt
    Sending --> Waiting: prompt sent to bridge
    Waiting --> Streaming: agent starts responding
    Streaming --> ToolRunning: tool call started
    ToolRunning --> Streaming: tool call completed
    Streaming --> Idle: turn completed
    Waiting --> Idle: turn completed (no output)
```

The `Activity` enum drives adaptive frame rate: `Idle` redraws at 500ms, `Streaming` at 33ms (30fps).
