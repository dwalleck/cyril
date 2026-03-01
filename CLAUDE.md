# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

Cyril is a Windows-native TUI client for Kiro CLI, communicating over the Agent Client Protocol (ACP) via JSON-RPC 2.0 over stdio. It spawns `wsl kiro-cli acp` as a subprocess and acts as a thin ACP client — providing filesystem, terminal, and permission capabilities while Kiro handles AI reasoning.

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

### Workspace Layout

Two crates in a Cargo workspace:

- **`cyril-core`** — Protocol logic, no UI. Implements the ACP `Client` trait, path translation, hooks, terminal management.
  - `protocol/` — ACP client implementation (`client.rs`) and transport (`transport.rs`)
  - `platform/` — OS-specific abstractions: path translation (`path.rs`), terminal management (`terminal.rs`)
  - `capabilities/` — Filesystem operations (`fs.rs`)
  - `hooks/` — Hook system (config, execution)
  - `kiro_ext.rs` — Kiro-specific extension types (`KiroExtCommand`, `KiroCommandsPayload`)
  - `session.rs` — `SessionContext` (session state: modes, model, config options)
  - `event.rs` — `AppEvent` and sub-enums bridging protocol → TUI
- **`cyril`** — The ratatui TUI binary. Owns all rendering, input handling, and the main event loop.
  - `app.rs` — Thin coordinator: event loop, dispatches to handlers
  - `commands.rs` — `CommandExecutor` (stateless slash command + prompt execution)
  - `ui/` — All rendering: `chat.rs`, `toolbar.rs`, `input.rs`, `approval.rs`, `picker.rs`, `markdown.rs`, `tool_calls.rs`

### Data Flow

```
User input → CommandExecutor::send_prompt() → acp::ClientSideConnection::prompt()
                                                    ↓ (JSON-RPC over stdio to WSL)
                                               kiro-cli acp
                                                    ↓ (callbacks)
KiroClient (implements acp::Client) ← agent requests fs/terminal/permissions
         ↓ (mpsc channel)
    AppEvent (wraps sub-enums)
         ↓
    App::handle_acp_event() dispatches to:
      ├─ Protocol(e)    → handle_protocol_event()   → ChatState, session
      ├─ Interaction(r) → handle_interaction()       → approval popup
      ├─ Extension(e)   → handle_extension_event()   → commands, context %
      └─ Internal(e)    → handle_internal_event()    → hook feedback
         ↓
    ratatui render loop (~30fps)
```

### Key Boundary: KiroClient (`cyril-core/src/protocol/client.rs`)

This is the ACP `Client` trait implementation — the single point where all agent callbacks arrive. It:
- Translates WSL paths to Windows paths on every fs read/write
- Runs before/after hooks at the protocol boundary
- Manages terminal processes (native Windows execution)
- Sends `AppEvent`s over an mpsc channel to the TUI

Everything is `!Send` — uses `Rc<RefCell<_>>` and `#[async_trait(?Send)]`. The tokio runtime is `current_thread` with a `LocalSet`.

### Path Translation (`cyril-core/src/platform/path.rs`)

All paths crossing the WSL boundary go through `win_to_wsl()` / `wsl_to_win()`. The agent sees `/mnt/c/...` paths; the client operates on `C:\...` paths. `translate_paths_in_json()` handles recursive translation in JSON payloads.

### Event Architecture

`AppEvent` (in `cyril-core/src/event.rs`) is the bridge between the protocol layer and TUI. Events flow one-way from `KiroClient` → `App`. It wraps four sub-enums:

- **`ProtocolEvent`** — Standard ACP session updates (agent messages, tool calls, mode/config changes, plan updates)
- **`InteractionRequest`** — Requests needing a user response (permission requests via oneshot channel)
- **`ExtensionEvent`** — Kiro-specific extension notifications (commands, metadata)
- **`InternalEvent`** — App-internal events (hook feedback)

`App::handle_acp_event()` pattern-matches the top-level variant and dispatches to a dedicated handler per sub-enum.

### SessionContext (`cyril-core/src/session.rs`)

Single source of truth for session state: session ID, modes, config options, context usage, and cached model. Lives in `cyril-core` so both crates can reference it.

Key invariant: `config_options` has a setter (`set_config_options()`) that maintains the `cached_model` cache. Fields with setters are private — use the getter/setter API. `set_optimistic_model()` allows immediate UI feedback before the server confirms.

### CommandExecutor (`cyril/src/commands.rs`)

Stateless executor for slash commands and prompts. Each method is an associated function that takes only the dependencies it needs as parameters. `App` is a thin coordinator — it owns the state and calls into `CommandExecutor`.

Pattern for spawned async work: use `tokio::task::spawn_local` with cloned channels, and use `send_or_log()` instead of `let _ = send()` to prevent silent failures.

### Chat Model: Interleaved Content Blocks

`ChatState` uses `Vec<ContentBlock>` where `ContentBlock` is `Text(String)`, `ToolCall(TrackedToolCall)`, or `Plan(acp::Plan)`. During streaming, blocks accumulate in `stream_blocks`; on turn end they move to `messages`. This keeps text, tool calls, and plans in chronological order. Plan updates replace the existing plan block (the agent sends the full plan each time).

### Tool Call Display (`cyril/src/ui/tool_calls.rs`)

`TrackedToolCall` wraps a full `acp::ToolCall` and caches a `DiffSummary` (computed via the `similar` crate). Tool calls render inline in chat with kind-specific labels (`Read(path)`, `Edit(path)`, `Execute(cmd)`) and actual code diffs for edits.

### Hook System (`cyril-core/src/hooks/`)

Hooks intercept agent operations at the protocol boundary. Before-hooks can block or modify; after-hooks can produce feedback that gets injected as follow-up prompts. Configured via `hooks.json` in the working directory.

## ACP Protocol Notes

- **Protocol**: JSON-RPC 2.0 over stdio (ACP v2025-01-01)
- The `agent-client-protocol` crate (v0.9) from crates.io is the source of truth for ACP types. Actual type definitions live in `agent-client-protocol-schema` (transitive dependency).
- Tool calls with `kind == ToolKind::Other` are "planning" steps from the agent and are filtered from display.

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

### Kiro Extension Notifications

Parsed in `KiroClient::ext_notification()`:
- `kiro.dev/commands/available` — slash commands after session creation (multiple payload shapes supported)
- `kiro.dev/metadata` — session metadata with `contextUsagePercentage` after each turn
- Panel commands (`/context`) return structured JSON responses with a `message` field for display

### `session/new` Response

Includes more than just `session_id`:
- `modes` — `SessionModeState` with `current_mode_id` and `available_modes` list (displayed in toolbar)
- `config_options` — optional session configuration (currently logged only)

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

## Platform Constraints

- Windows-only: spawns `wsl` to reach kiro-cli
- Requires WSL with kiro-cli installed and authenticated (`wsl kiro-cli login`)
- Terminal commands from the agent run natively on Windows (not in WSL)
- Logs go to `cyril.log` in the working directory (append mode) to avoid TUI conflicts
