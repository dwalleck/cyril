# Data Models

> Generated: 2026-04-11 | Codebase: Cyril

## Core Domain Types (`cyril-core/src/types/`)

### Session Types (`session.rs`)

```mermaid
classDiagram
    class SessionId {
        -inner: String
        +new(impl Into~String~) SessionId
        +as_str() str
    }

    class SessionStatus {
        <<enum>>
        Disconnected
        Connecting
        Connected
        Error(String)
    }

    class SessionMode {
        +id: String
        +label: String
        +description: Option~String~
    }

    class ContextUsage {
        -percentage: f64
        +new(f64) ContextUsage
        +percentage() f64
    }

    class CreditUsage {
        +used: f64
        +limit: f64
    }

    class TokenCounts {
        +input: u64
        +output: u64
        +cache_read: Option~u64~
        +cache_write: Option~u64~
    }

    class TurnMetering {
        +input_tokens: u64
        +output_tokens: u64
        +cost_usd: Option~f64~
    }

    class SessionCost {
        +total_usd: f64
        +add_turn(TurnMetering)
    }
```

`SessionId` wraps a `String`, implements `Hash + Eq + Clone + Display`. Used as HashMap key for subagent tracking.

### Tool Call Types (`tool_call.rs`)

```mermaid
classDiagram
    class ToolCallId {
        -inner: String
        +new(impl Into~String~) ToolCallId
        +as_str() str
    }

    class ToolCall {
        -id: ToolCallId
        -title: String
        -kind: ToolKind
        -status: ToolCallStatus
        -content: Option~ToolCallContent~
        -locations: Vec~ToolCallLocation~
        -raw_input: Option~Value~
        +new(id, title, kind, status, raw_input) ToolCall
        +with_content(content) ToolCall
        +with_locations(locations) ToolCall
        +merge_update(other)
    }

    class ToolKind {
        <<enum>>
        Read
        Write
        Execute
        Search
        Think
        Fetch
        Other(String)
    }

    class ToolCallStatus {
        <<enum>>
        Pending
        InProgress
        Completed
        Failed
    }

    class ToolCallContent {
        <<enum>>
        Text(String)
        Diff(old: String, new: String)
    }

    class ToolCallLocation {
        +path: String
        +line: Option~u32~
    }

    ToolCall --> ToolCallId
    ToolCall --> ToolKind
    ToolCall --> ToolCallStatus
    ToolCall --> ToolCallContent
    ToolCall --> ToolCallLocation
```

All tool call types are `Send + Sync + Clone`. `ToolCall::merge_update()` applies partial updates (preserves existing fields when update has `None`).

### Event Types (`event.rs`)

See [Interfaces — Notification System](interfaces.md) for the full `Notification` and `BridgeCommand` enums.

Key design decisions:
- `Notification` is `Send + Sync + Clone` — can be freely shared across threads
- `PermissionRequest` is NOT Clone — owns a `oneshot::Sender`
- `RoutedNotification` wraps `Notification` with optional `SessionId` for routing
- `BridgeCommand` is `Send` but not Clone — consumed by the bridge

### Subagent Types (`subagent.rs`)

```mermaid
classDiagram
    class SubagentInfo {
        -session_id: SessionId
        -name: String
        -task: String
        -status: SubagentStatus
        +session_id() SessionId
        +name() str
        +task() str
        +status() SubagentStatus
    }

    class SubagentStatus {
        <<enum>>
        Working(Option~String~)
        Idle
        Terminated
    }

    class PendingStage {
        +name: String
        +status: String
    }

    SubagentInfo --> SubagentStatus
```

### Command Types (`command.rs`)

```mermaid
classDiagram
    class CommandInfo {
        +name: String
        +description: String
        +is_local: bool
        +selection_type: bool
    }

    class CommandOption {
        +label: String
        +value: String
        +description: Option~String~
        +group: Option~String~
        +is_current: bool
    }

    class ConfigOption {
        +id: String
        +label: String
        +description: Option~String~
    }
```

### Message Types (`message.rs`)

```mermaid
classDiagram
    class AgentMessage {
        +text: String
        +is_streaming: bool
    }

    class AgentThought {
        +text: String
    }
```

### Plan Types (`plan.rs`)

```mermaid
classDiagram
    class Plan {
        +entries: Vec~PlanEntry~
    }

    class PlanEntry {
        +description: String
        +status: PlanEntryStatus
    }

    class PlanEntryStatus {
        <<enum>>
        Pending
        InProgress
        Completed
    }

    Plan --> PlanEntry
    PlanEntry --> PlanEntryStatus
```

### Hook Types (`hook.rs`)

```mermaid
classDiagram
    class HookInfo {
        +trigger: String
        +command: String
        +matcher: Option~String~
    }
```

Display-only projection of Kiro's backend `HookConfig`. Trigger values: `PreToolUse`, `PostToolUse`, `UserPromptSubmit`, `Stop`, `AgentSpawn`. Matcher is optional tool name filter.

### Configuration (`config.rs`)

```mermaid
classDiagram
    class Config {
        +ui: UiConfig
        +agent: AgentConfig
        +load_from_path(Path) Config
    }

    class UiConfig {
        +max_messages: usize = 500
        +highlight_cache_size: usize = 20
        +stream_buffer_timeout_ms: u64 = 150
        +mouse_capture: bool = true
    }

    class AgentConfig {
        +agent_name: String = "kiro-cli"
        +extra_args: Vec~String~ = []
    }

    Config --> UiConfig
    Config --> AgentConfig
```

## UI State Types (`cyril-ui/src/traits.rs`)

### Chat Display Types

```mermaid
classDiagram
    class ChatMessage {
        +kind: ChatMessageKind
        +timestamp: Instant
    }

    class ChatMessageKind {
        <<enum>>
        UserText(String)
        AgentText(String)
        Thought(String)
        ToolCall(TrackedToolCall)
        Plan(Plan)
        System(String)
        CommandOutput(command, lines)
    }

    class TrackedToolCall {
        +tool_call: ToolCall
        +is_latest: bool
    }

    ChatMessage --> ChatMessageKind
    ChatMessageKind --> TrackedToolCall
```

### Overlay State Types

```mermaid
classDiagram
    class ApprovalState {
        +tool_call: ToolCall
        +message: String
        +options: Vec~PermissionOption~
        +selected: usize
    }

    class PickerState {
        +title: String
        +items: Vec~CommandOption~
        +filtered: Vec~usize~
        +selected: usize
        +filter_text: String
    }

    class HooksPanelState {
        +hooks: Vec~HookInfo~
        +scroll_offset: usize
    }
```

### Autocomplete

```mermaid
classDiagram
    class Suggestion {
        +text: String
        +display: String
        +kind: SuggestionKind
    }

    class SuggestionKind {
        <<enum>>
        Command
        File
    }

    Suggestion --> SuggestionKind
```

## Subagent UI State (`cyril-ui/src/subagent_ui.rs`)

```mermaid
classDiagram
    class SubagentUiState {
        -streams: HashMap~SessionId, SubagentStream~
        -focused: Option~SessionId~
        +apply_notification(SessionId, Notification)
        +focus(SessionId)
        +unfocus()
        +focused_stream() Option~SubagentStream~
    }

    class SubagentStream {
        -messages: Vec~ChatMessage~
        -streaming_text: String
        -tool_call_index: HashMap~ToolCallId, usize~
        -activity: Activity
        +messages() [ChatMessage]
        +streaming_text() str
        +activity() Activity
        +mark_terminated()
        +is_terminated() bool
    }

    SubagentUiState --> SubagentStream
```
