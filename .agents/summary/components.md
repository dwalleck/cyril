# Components

> Generated: 2026-04-11 | Codebase: Cyril

## Crate: `cyril` (Binary)

The application entry point and event loop orchestrator.

### `main.rs` — Entry Point

- Parses CLI args via `clap` (`--cwd`, `--prompt`, `--agent`)
- Sets up JSON file logging to `~/.config/cyril/cyril.log`
- Loads user config from `~/.config/cyril/config.toml`
- Spawns the ACP bridge via `spawn_bridge()`
- Builds a tokio multi-thread runtime and runs the App

### `app.rs` — Event Loop (App)

The central orchestrator. Owns:
- `BridgeSender` + notification/permission receivers (split from `BridgeHandle`)
- `UiState` — all UI state
- `SessionController` — session metadata
- `CommandRegistry` — command dispatch

Key methods:
- `run()` — main `tokio::select!` loop with biased priority
- `handle_terminal_event()` — keyboard/mouse dispatch
- `handle_notification()` — routes `RoutedNotification` to main or subagent state
- `handle_command_result()` — processes `CommandResult` variants
- `submit_input()` — parses input as command or prompt
- `format_command_response()` — formats agent command responses for display
- `parse_hooks_response()` — extracts hook info from `/hooks` command response

### `tests/event_routing.rs` — Integration Tests

Tests for notification routing between main session and subagent sessions.

---

## Crate: `cyril-core` (Library)

Protocol logic, types, commands, and session management.

### `protocol/bridge.rs` — Bridge Architecture

The bridge connects the App to the ACP agent process:

- **`BridgeHandle`** — held by App, provides `recv_notification()`, `recv_permission()`, `sender()`, `split()`
- **`BridgeSender`** — cloneable command sender, passed to commands and spawned tasks
- **`BridgeChannels`** — bridge-side channel endpoints
- **`spawn_bridge()`** — spawns agent process, creates channels, starts bridge loop
- **`run_bridge()`** — the bridge event loop: reads ACP messages, dispatches to `KiroClient`

Channel capacities: commands=32, notifications=256, permissions=16.

### `protocol/client.rs` — KiroClient (ACP Client)

Implements `agent_client_protocol::Client` trait (`!Send`, lives in bridge thread):

- `request_permission()` — converts ACP permission request, sends to App via oneshot
- `session_notification()` — converts session updates, routes via `RoutedNotification::scoped()`
- `ext_notification()` — handles Kiro extension notifications (metadata, commands, subagents)

Maintains `tool_call_inputs: RefCell<HashMap>` cache for permission request lookups.

### `protocol/convert.rs` — Notification Conversion

The largest file in the codebase. Converts raw ACP protocol messages into typed `Notification` variants:

- `session_update_to_notification()` — converts `SessionUpdate` variants
- `to_ext_notification()` — routes `kiro.dev/*` extension methods
- `to_tool_call()` / `to_tool_call_from_permission()` — constructs `ToolCall` from ACP data
- `cache_tool_call_input()` — caches `raw_input` from tool call notifications
- `from_permission_response()` — converts user response back to ACP format
- `parse_*` helpers — parse specific notification types (metadata, subagent list, commands, etc.)

### `protocol/transport.rs` — Agent Process

`AgentProcess::spawn()` — launches `kiro-cli acp` (or `wsl kiro-cli acp` on Windows) with piped stdio.

### `commands/mod.rs` — Command Registry

- **`Command` trait** — `name()`, `description()`, `aliases()`, `is_local()`, `execute()`
- **`CommandRegistry`** — stores `Arc<dyn Command>`, lookup by name/alias, deduplication
- **`CommandContext`** — execution context with session, bridge sender, optional subagent tracker
- **`CommandResult`** / **`CommandResultKind`** — result variants (SystemMessage, ShowPicker, Dispatched, Quit, NotACommand)
- `with_builtins()` — registers help, clear, quit, new, load
- `register_agent_commands()` — dynamically registers server-advertised commands
- `parse()` — parses `/command args` input, returns command + args
- `parse_options_response()` — parses picker options from agent command responses

### `commands/builtin.rs` — Builtin Commands

`HelpCommand`, `ClearCommand`, `QuitCommand`, `NewCommand`, `LoadCommand` — each implements `Command` trait.

### `commands/subagent.rs` — Subagent Commands

`SpawnCommand`, `KillCommand`, `MsgCommand`, `SessionsCommand` — commands for managing subagent sessions. Use `CommandContext::require_tracker()` for subagent lookup.

### `session.rs` — SessionController

Manages session metadata: status, ID, modes, current mode/model, context usage, agent commands, credit usage, session cost. Pure state — no async.

### `subagent.rs` — SubagentTracker

Tracks subagent metadata from `SubagentListUpdated` and `InboxNotification`. Pure state machine — maps `SessionId → SubagentInfo`, tracks pending stages and inbox counts.

### `error.rs` — Error Types

`Error` + `ErrorKind` enum with variants: Protocol, Transport, AgentExited, NoSession, SessionNotFound, UnknownCommand, CommandFailed, BridgeClosed, PermissionTimeout, InvalidConfig.

### `types/` — Domain Types

| Module | Key Types | Purpose |
|--------|-----------|---------|
| `event.rs` | `Notification`, `RoutedNotification`, `BridgeCommand`, `PermissionRequest` | Core event types |
| `tool_call.rs` | `ToolCall`, `ToolCallId`, `ToolKind`, `ToolCallStatus`, `ToolCallContent`, `ToolCallLocation` | Tool call model |
| `session.rs` | `SessionId`, `SessionStatus`, `SessionMode`, `ContextUsage`, `CreditUsage`, `TokenCounts`, `TurnMetering` | Session types |
| `subagent.rs` | `SubagentInfo`, `SubagentStatus`, `PendingStage` | Subagent types |
| `command.rs` | `CommandInfo`, `CommandOption`, `ConfigOption` | Command metadata |
| `config.rs` | `Config`, `UiConfig`, `AgentConfig` | User configuration |
| `message.rs` | `AgentMessage`, `AgentThought` | Agent output types |
| `plan.rs` | `Plan`, `PlanEntry`, `PlanEntryStatus` | Task plan types |
| `prompt.rs` | `PromptInfo`, `PromptArgument` | Prompt metadata |
| `hook.rs` | `HookInfo` | Hook display metadata |

### `platform/path.rs` — Path Translation

Windows ↔ WSL path translation (`C:\Users\...` ↔ `/mnt/c/Users/...`). Recursive JSON payload translation.

---

## Crate: `cyril-ui` (Library)

UI state machine, rendering, and widgets.

### `state.rs` — UiState

The central UI state machine. Implements `TuiState` trait. Manages:

- Chat messages (with enforced limit), streaming text/thought buffers
- Tool call tracking (`TrackedToolCall` with index for O(1) updates)
- Input text, cursor, autocomplete state
- Overlay state: approval, picker, hooks panel
- Activity tracking with elapsed time
- Subagent integration: `SubagentTracker` + `SubagentUiState`

Key methods:
- `apply_notification()` — main notification dispatch
- `commit_streaming()` — flushes streaming buffers to message history
- `show_picker()` / `show_approval()` / `show_hooks_panel()` — overlay management
- `focus_subagent()` / `unfocus_subagent()` — subagent drill-in

### `traits.rs` — TuiState Trait + Display Types

- **`TuiState`** — read-only trait for the renderer (messages, input, activity, overlays, subagents)
- **`Activity`** — enum: Idle, Ready, Sending, Waiting, Streaming, ToolRunning
- **`ChatMessage`** / **`ChatMessageKind`** — display message types
- **`TrackedToolCall`** — tool call with display metadata
- **`ApprovalState`** / **`PickerState`** / **`HooksPanelState`** — overlay state types
- **`Suggestion`** — autocomplete suggestion

Includes `test_support::MockTuiState` for widget testing.

### `render.rs` — Frame Layout

`draw()` — panic-safe top-level renderer. Lays out: toolbar, chat, crew panel, input, status bar. Renders overlays on top (approval, picker, hooks panel).

### `widgets/` — Widget Modules

| Widget | File | Purpose |
|--------|------|---------|
| `chat` | `chat.rs` | Message display with tool call diffs, subagent drill-in |
| `markdown` | `markdown.rs` | Pulldown-cmark → ratatui spans with syntax highlighting |
| `input` | `input.rs` | Multi-line input with cursor and autocomplete overlay |
| `toolbar` | `toolbar.rs` | Top bar (session, mode, model) + bottom status bar (context, activity) |
| `crew_panel` | `crew_panel.rs` | Subagent status panel (bordered, max 6 rows + overflow) |
| `hooks_panel` | `hooks_panel.rs` | Hooks overlay popup (three-column table) |
| `picker` | `picker.rs` | Fuzzy-filtered selection list overlay |
| `approval` | `approval.rs` | Permission approval dialog overlay |

### `subagent_ui.rs` — SubagentUiState

Per-subagent message streams (`SubagentStream`). Manages focused subagent for drill-in view. Routes notifications to the correct subagent stream.

### `stream_buffer.rs` — StreamBuffer

Buffers streaming text and flushes at semantic boundaries (newlines, code fences) or after a configurable timeout. Prevents partial-line rendering during streaming.

### `file_completer.rs` — FileCompleter

Provides `@path/to/file` autocomplete. Loads directory tree asynchronously, respects `.gitignore`.

### `highlight.rs` — Syntax Highlighting

Syntect-based code highlighting with LRU cache integration.

### `cache.rs` — LRU Cache

Generic LRU cache used by the markdown renderer and syntax highlighter.

### `error.rs` — UI Error Types

`Error` + `ErrorKind` for UI-specific errors.
