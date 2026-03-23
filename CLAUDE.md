# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

Cyril is a cross-platform TUI client for Kiro CLI, communicating over the Agent Client Protocol (ACP) via JSON-RPC 2.0 over stdio. On Linux it spawns `kiro-cli acp` directly; on Windows it spawns `wsl kiro-cli acp` and translates paths at the protocol boundary. It acts as a thin ACP client — providing filesystem, terminal, and permission capabilities while Kiro handles AI reasoning.

## Build & Test Commands

```sh
cargo build                          # build all crates
cargo check                          # type-check without linking (faster)
cargo run                            # run the cyril TUI binary
cargo run --example test_acp         # run the headless ACP test harness
cargo test -p cyril-core             # run tests in the core crate
cargo test -p cyril-core -- path     # run only path-related tests
```

There is no linter or formatter configured beyond `cargo check`. The project uses Rust 2021 edition.

## Architecture

### Three-Crate Workspace

```
crates/
  cyril-core/     # Library — protocol, types, commands, session, platform
  cyril-ui/       # Library — rendering, widgets, UI state (depends on cyril-core)
  cyril/          # Binary — wires everything together, owns the event loop
```

### Layer Responsibilities

Each crate has a clear responsibility and strict rules about what it must NOT do:

**`cyril-core`** — Domain logic and protocol boundary.
- **Owns:** Types (`types/`), ACP protocol bridge (`protocol/`), command registry (`commands/`), session state (`session.rs`), path translation (`platform/`), error types (`error.rs`)
- **Responsibility:** Convert between ACP wire types and internal domain types. All Kiro protocol quirks are handled in `convert.rs`. The bridge runs on a dedicated `!Send` thread and communicates via typed channels.
- **Must NOT:** Import any UI crate. Reference ratatui, crossterm, or any rendering concept. Know how content is displayed.
- **Dependency rule:** Only crate that imports `agent-client-protocol`. No other crate may reference `acp::` types.

**`cyril-ui`** — Rendering and UI state.
- **Owns:** `UiState` (all mutable UI state), `TuiState` trait (read-only rendering interface), widgets (`widgets/`), markdown rendering, syntax highlighting, file completer, stream buffer
- **Responsibility:** Given notifications, update UI state. Given `&dyn TuiState`, render frames. All rendering decisions live here.
- **Must NOT:** Import `agent-client-protocol`. Know about ACP, JSON-RPC, or the bridge. Send commands to the bridge. Make async calls.
- **Dependency rule:** Depends on `cyril-core` for types only — never `protocol::`.

**`cyril`** — Thin orchestrator binary.
- **Owns:** `App` (event loop), CLI args, terminal setup, wiring between components
- **Responsibility:** Wire `cyril-core` and `cyril-ui` together. Run the `tokio::select!` event loop. Dispatch key events through the layered handler. Route notifications to both `SessionController` and `UiState`. Handle cross-cutting concerns (opening pickers from `CommandOptionsReceived`, extracting model from `CommandExecuted`).
- **Must NOT:** Contain business logic or protocol knowledge. Parse JSON responses (that's `cyril-core`'s job). Make rendering decisions (that's `cyril-ui`'s job).

### Data Flow

```
User input → CommandRegistry::parse() → Command::execute() → BridgeSender::send(BridgeCommand)
                                                                    ↓ (mpsc channel)
                                                              Bridge thread (dedicated OS thread)
                                                                    ↓ (JSON-RPC over stdio)
                                                              kiro-cli acp
                                                                    ↓ (ACP callbacks)
                                                              KiroClient (protocol/client.rs)
                                                                    ↓ (mpsc channels)
                                                    Notification / PermissionRequest
                                                                    ↓
App event loop (tokio::select!):
  ├─ Notification → SessionController::apply_notification()
  │               → UiState::apply_notification()
  │               → cross-cutting handlers (CommandOptionsReceived, CommandExecuted, etc.)
  ├─ PermissionRequest → UiState::show_approval()
  └─ Terminal Event → layered key dispatch
                                                                    ↓
                                              ratatui render (adaptive frame rate)
```

### Key Boundaries

**Bridge thread (`protocol/bridge.rs`):** Runs `!Send` ACP types in a quarantined `current_thread` + `LocalSet` runtime. All communication is via three bounded mpsc channels: commands in, notifications out, permission requests out. The bridge MUST send a notification for every command it processes — including error cases — so the App never gets stuck.

**Conversion boundary (`protocol/convert.rs`):** Single file that imports both `acp::` and internal types. Every Kiro protocol quirk is handled here: name prefix stripping, metadata parsing, content/location extraction, raw_input caching. No other file should import `acp::` types.

**TuiState trait (`cyril-ui/traits.rs`):** Read-only interface the renderer uses. Every method returns a reference or Copy type — compile-time guarantee that rendering cannot mutate state. The renderer receives `&dyn TuiState`, never `&App` or `&mut UiState`.

### Notification-Driven Architecture

All agent interactions are notification-driven. Commands return immediately; results arrive as notifications:

| User action | BridgeCommand | Notification back | App reacts |
|---|---|---|---|
| Send prompt | `SendPrompt` | `AgentMessage`, `ToolCallStarted`, `TurnCompleted` | Streams to chat |
| `/new` | `NewSession` | `SessionCreated` | Updates session state |
| `/model` (no args) | `QueryCommandOptions` | `CommandOptionsReceived` | Opens picker |
| `/tools` | `ExecuteCommand` | `CommandExecuted` | Shows formatted response |
| Picker confirms | `ExecuteCommand` | `CommandExecuted` | Shows confirmation |

**The event loop must NEVER block on command execution.** Commands send a `BridgeCommand` and return `Dispatched`. Results come back asynchronously as notifications.

### Key Handling Layers

Input dispatch follows strict priority (each layer consumes or passes through):

1. **Global shortcuts** (Ctrl+C, Ctrl+Q, Ctrl+M) — always active
2. **Approval overlay** — consumes all keys if active, early return
3. **Picker overlay** — consumes all keys if active, early return
4. **Autocomplete** — `handle_autocomplete_key()` returns `AutocompleteAction` enum (Consumed/Accepted/AcceptedAndSubmit/NotActive), early return unless NotActive
5. **Normal input** — Enter submits, Esc cancels, other keys go to textarea

### Streaming Content Model

Agent text and tool calls commit to the message list in chronological order as they arrive:

- `AgentMessage` chunks accumulate in `streaming_text`
- When `ToolCallStarted` arrives, flush `streaming_text` to a committed `AgentText` message, then commit the tool call to messages at that position
- `ToolCallUpdated` updates the committed tool call in-place via `merge_update` (preserves content/locations from initial notification)
- When `TurnCompleted` arrives, flush any remaining `streaming_text`
- Result: messages list has `[AgentText, ToolCall, AgentText, ...]` in arrival order

### Path Translation (`cyril-core/src/platform/path.rs`)

On Windows, all paths crossing the WSL boundary go through `win_to_wsl()` / `wsl_to_win()`. On Linux, path translation is a no-op.

## ACP Protocol Notes

For the comprehensive protocol reference with example requests/responses, see **[docs/kiro-acp-protocol.md](docs/kiro-acp-protocol.md)**.

- **Protocol**: JSON-RPC 2.0 over stdio (ACP v2025-01-01)
- The `agent-client-protocol` crate (v0.9) from crates.io is the source of truth for ACP types. Actual type definitions live in `agent-client-protocol-schema` (transitive dependency).
- Tool calls with `kind == ToolKind::Other` are "planning" steps from the agent and are filtered from display.
- **Kiro logs**: `$XDG_RUNTIME_DIR/kiro-log/kiro-chat.log` (Linux). Set `KIRO_LOG_LEVEL=debug` for verbose output.

### Session Updates (`session/update`)

Sent as `SessionNotification` containing a `SessionUpdate` enum. **Turn completion is signaled by the `session/prompt` response** (with `stop_reason: EndTurn`), not by a notification.

Key variants: `AgentMessageChunk`, `AgentThoughtChunk`, `ToolCall`, `ToolCallUpdate`, `Plan`, `AvailableCommandsUpdate`, `CurrentModeUpdate`, `ConfigOptionUpdate`.

### Tool Call Lifecycle

Tool calls follow a three-phase lifecycle:
1. `ToolCall` with `status: InProgress` — tool initiated
2. `ToolCall` with `status: Pending` — title updated (e.g., "Reading file.rs:1"), awaiting permission if needed
3. `ToolCallUpdate` with `status: Completed` — execution finished

The agent may initiate multiple tool calls in parallel before waiting for permission responses.

### Permission Requests (`session/request_permission`)

A server-to-client request (has an `id`, expects a JSON-RPC response). The agent asks for permission before executing certain tools.

- **File reads** do not require permission — they execute automatically
- **Shell commands** require permission — options are typically `Yes(AllowOnce)`, `Always(AllowAlways)`, `No(RejectOnce)`
- `AllowAlways` makes the agent remember the choice for the rest of the session

### `session/cancel`

A notification (fire-and-forget, no response expected). Cyril sends this on Esc when `is_busy`.

### Kiro Extension Commands (`kiro.dev/commands/*`)

**`commands/execute`** — The `command` field must be an object `{"command": "<name>", "args": {<args>}}` (a `TuiCommand` adjacently tagged enum), NOT a plain string. Sending a string crashes kiro-cli. Selection commands pass their value as `{"value": "<selected>"}` in args.

**`commands/options`** — Query available options for selection commands. Options use `label` (not `name`) for display, plus `value`, `description`, `group`, and optional `current` boolean.

**`commands/available`** — Notification sent after session creation with the full command list, tools, and MCP servers.

**`metadata`** — Notification with `contextUsagePercentage` after each turn. Not in official docs.

### `session/new` Response

Includes more than just `session_id`:
- `modes` — `SessionModeState` with `current_mode_id` and `available_modes` list (displayed in toolbar)
- `config_options` — always `null` in Kiro v1.28.0 (`session/set_config_option` is not implemented)

### Methods NOT implemented by Kiro v1.28.0

- `session/set_config_option` — returns "Method not found". Use `kiro.dev/commands/execute` with `model` command instead.
- `session/set_model` — behind unstable feature flag, not advertised in capabilities.
- `session/fork`, `session/resume`, `session/list` — unstable, `sessionCapabilities: {}`.

## Adding New Features

### New ACP event type
1. Add a variant to the appropriate sub-enum in `event.rs` (`ProtocolEvent` for standard ACP, `ExtensionEvent` for Kiro-specific)
2. Emit it from `KiroClient` in `protocol/client.rs` wrapped in `AppEvent::Protocol(...)` or `AppEvent::Extension(...)`
3. Handle it in the matching `App::handle_*_event()` method in `app.rs`

### New slash command
1. Add the command name to `parse_command()` match in `commands.rs`
2. Implement the handler as an associated function on `CommandExecutor` — take only what you need as parameters
3. Call it from the `execute()` dispatch match

### New session state
1. Add a private field to `SessionContext` in `session.rs` with a getter and setter
2. If the field has a cache invariant (like `cached_model`), maintain it in the setter
3. Update from the appropriate event handler in `app.rs`

### New UI component
1. Create a module in `cyril/src/ui/` with a `State` struct and `render()` function
2. Add the state to `App` in `app.rs`
3. Call the render function from `App::render()`
4. Handle input in `App::handle_key()` (overlay popups take priority — check approval/picker first)

### Channel sends in spawned tasks
Always use `CommandExecutor::send_or_log()` instead of `let _ = sender.send()`. Silent send failures can freeze the UI (e.g., `toolbar.is_busy` stuck true).

## Design Principles

### Make illegal states unrepresentable

Use the type system to prevent bugs at compile time rather than catching them at runtime.

**Use newtypes for domain identifiers.** `SessionId`, `ToolCallId` — never pass raw `String` where a typed ID is expected. Every field that carries a session or tool call identifier must use the newtype, not `String`.

**Use `Option` for absent values, not sentinels.** Never use a concrete enum variant (like `ToolKind::Other`) or a magic value (like `0.0` or `""`) to mean "not specified." If a value may be absent, the type should be `Option<T>`. Sentinel values break `merge_update` patterns — you can't distinguish "explicitly set to X" from "not provided."

**Guard partial updates.** When merging update fields into existing state, only overwrite fields the update actually provides. An update with an empty string for `name` means "name was not provided," not "set name to empty." Use guards like `if update.field.is_some()` or `if !update.field.is_empty()`.

**Errors are not default values.** Never use `unwrap_or(0.0)`, `unwrap_or("")`, or `unwrap_or_default()` to handle parse failures or missing data. These hide real errors as plausible-looking defaults. Instead:
- Return `None` / skip the notification if the data is genuinely optional
- Return `Err` if the data is required
- At minimum, log a warning before falling back

**Bridge errors must notify the App.** Every failed bridge operation (`prompt`, `new_session`, `load_session`, `set_session_mode`) must send a notification back through the channel so the UI can recover. Logging alone is invisible to the user — the UI will get stuck in a transitional state.

**`commit_streaming` flushes text on boundaries.** When a tool call starts, flush accumulated streaming text to a committed message first. This prevents text segments from concatenating across tool call boundaries. Content commits in chronological order — tool calls go into messages at the position where they arrived, not at the end.

### Testing layers

State tests verify data transitions. Render tests verify presentation. Both are needed:

- **State lifecycle tests**: Apply a realistic sequence of notifications (text → tool call → update → turn complete) and verify committed messages contain all content in order.
- **Render order tests**: Render to `TestBackend`, extract the buffer, assert character positions maintain chronological order.
- **Merge tests**: Verify that partial updates preserve existing fields (content, locations, title, raw_input) when the update doesn't provide them.

## Platform Constraints

- **Linux:** spawns `kiro-cli acp` directly; requires kiro-cli installed and on PATH
- **Windows:** spawns `wsl kiro-cli acp`; requires WSL with kiro-cli installed and authenticated (`wsl kiro-cli login`)
- Path translation (`C:\` ↔ `/mnt/c/`) is active only on Windows; on Linux it's a no-op
- Terminal commands from the agent run natively on the host OS
- Logs go to `cyril.log` in the working directory (append mode) to avoid TUI conflicts
