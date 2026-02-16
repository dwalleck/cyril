# Cyril

A Windows-native TUI client for [Kiro CLI](https://kiro.dev) via the [Agent Client Protocol (ACP)](https://agentclientprotocol.com).

> **Status:** Alpha — functional but under active development.

## What is this?

Kiro CLI doesn't run natively on Windows — only in WSL. Cyril bridges the gap by running as a native Windows terminal application that communicates with Kiro CLI in WSL over ACP (JSON-RPC 2.0 over stdio).

This means you get Kiro's full AI agent capabilities while your file system operations and terminal commands execute natively on Windows — important for workflows like .NET/C# development where you need the Windows SDK.

```
Windows (Cyril TUI)  <── stdin/stdout JSON-RPC ──>  WSL (kiro-cli acp)
       |                                                    |
       ├── Streaming markdown rendering                     ├── AI reasoning & planning
       ├── Path translation (C:\ <-> /mnt/c/)               ├── MCP servers
       ├── Native terminal execution                        ├── Skills & steering rules
       └── JSON-configurable hook system                    └── Cloud AI model
```

## Features

- **Streaming TUI** — ratatui-based interface with real-time markdown rendering
- **Path translation** — automatic `C:\` to `/mnt/c/` mapping at the protocol boundary
- **Native terminal execution** — agent-requested commands run on Windows, not in WSL
- **Hook system** — JSON-configurable before/after hooks on file writes and terminal commands
- **Slash commands** — autocomplete-enabled commands from both the client and the Kiro agent
- **Tool call display** — see what the agent is doing in real time
- **Approval prompts** — review and approve file writes and command execution
- **Session management** — create, load, and switch between sessions

## Prerequisites

- Windows 10/11 with [WSL](https://learn.microsoft.com/en-us/windows/wsl/install) installed
- [Kiro CLI](https://kiro.dev/docs/cli/) installed inside WSL
- [Rust toolchain](https://rustup.rs/) (for building from source)

## Installation

```sh
cargo install cyril
```

Or build from source:

```sh
git clone https://github.com/dwalleck/cyril.git
cd cyril
cargo build --release
```

The binary will be at `target/release/cyril.exe`.

## Usage

Launch the interactive TUI:

```sh
cyril
```

Send a one-shot prompt:

```sh
cyril --prompt "Explain what this project does"
```

Specify a working directory:

```sh
cyril -d C:\Users\you\project
```

### Keyboard shortcuts

| Key | Action |
|-----|--------|
| `Enter` | Send message |
| `Shift+Enter` | Newline in input |
| `Tab` | Accept autocomplete suggestion |
| `Esc` | Cancel current request |
| `Ctrl+C` / `Ctrl+Q` | Quit |

### Slash commands

| Command | Description |
|---------|-------------|
| `/help` | Show available commands |
| `/new` | Start a new session |
| `/load <id>` | Load a previous session |
| `/clear` | Clear the chat |
| `/quit` | Quit |

Agent-provided slash commands are also available with autocomplete.

## Hook system

Create a `hooks.json` in your working directory to configure hooks that run on agent actions:

```json
{
  "hooks": [
    {
      "name": "Format on save",
      "event": "afterWrite",
      "pattern": "*.cs",
      "command": "dotnet format --include {{file}}"
    }
  ]
}
```

Hooks run at the protocol boundary — the agent never sees them, just the results.

## Project structure

```
crates/
  cyril/          # TUI application (binary)
  cyril-core/     # Protocol logic, path translation, hooks, capabilities
```

## License

[MIT](LICENSE)
