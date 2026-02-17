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
- **`cyril`** — The ratatui TUI binary. Owns all rendering, input handling, and the main event loop.

### Data Flow

```
User input → App::send_prompt() → acp::ClientSideConnection::prompt()
                                         ↓ (JSON-RPC over stdio to WSL)
                                    kiro-cli acp
                                         ↓ (callbacks)
KiroClient (implements acp::Client) ← agent requests fs/terminal/permissions
         ↓ (mpsc channel)
    AppEvent enum
         ↓
    App::handle_acp_event() → updates ChatState, ToolbarState, etc.
         ↓
    ratatui render loop (~30fps)
```

### Key Boundary: KiroClient (`cyril-core/src/client.rs`)

This is the ACP `Client` trait implementation — the single point where all agent callbacks arrive. It:
- Translates WSL paths to Windows paths on every fs read/write
- Runs before/after hooks at the protocol boundary
- Manages terminal processes (native Windows execution)
- Sends `AppEvent`s over an mpsc channel to the TUI

Everything is `!Send` — uses `Rc<RefCell<_>>` and `#[async_trait(?Send)]`. The tokio runtime is `current_thread` with a `LocalSet`.

### Path Translation (`cyril-core/src/path.rs`)

All paths crossing the WSL boundary go through `win_to_wsl()` / `wsl_to_win()`. The agent sees `/mnt/c/...` paths; the client operates on `C:\...` paths. `translate_paths_in_json()` handles recursive translation in JSON payloads.

### Event Architecture

`AppEvent` (in `cyril-core/src/event.rs`) is the bridge between the protocol layer and TUI. Events flow one-way from `KiroClient` → `App`. Permission requests use a `oneshot` channel for the response.

### Chat Model: Interleaved Content Blocks

`ChatState` uses `Vec<ContentBlock>` where `ContentBlock` is either `Text(String)` or `ToolCall(TrackedToolCall)`. During streaming, blocks accumulate in `stream_blocks`; on turn end they move to `messages`. This keeps text and tool calls in chronological order.

### Tool Call Display (`cyril/src/ui/tool_calls.rs`)

`TrackedToolCall` wraps a full `acp::ToolCall` and caches a `DiffSummary` (computed via the `similar` crate). Tool calls render inline in chat with kind-specific labels (`Read(path)`, `Edit(path)`, `Execute(cmd)`) and actual code diffs for edits.

### Hook System (`cyril-core/src/hooks/`)

Hooks intercept agent operations at the protocol boundary. Before-hooks can block or modify; after-hooks can produce feedback that gets injected as follow-up prompts. Configured via `hooks.json` in the working directory.

## ACP Protocol Notes

- The `agent-client-protocol` crate (v0.9) from crates.io is the source of truth for ACP types — not the Amazon Q codebase.
- Actual type definitions live in `agent-client-protocol-schema` (transitive dependency).
- Kiro sends extension notifications (`kiro.dev/commands/available`, `kiro.dev/metadata`) that are parsed in `KiroClient::ext_notification()`.
- Tool calls with `kind == ToolKind::Other` are "planning" steps from the agent and are filtered from display.

## Platform Constraints

- Windows-only: spawns `wsl` to reach kiro-cli
- Requires WSL with kiro-cli installed and authenticated (`wsl kiro-cli login`)
- Terminal commands from the agent run natively on Windows (not in WSL)
- Logs go to `cyril.log` in the working directory (append mode) to avoid TUI conflicts
