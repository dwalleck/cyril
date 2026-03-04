# Cyril

A cross-platform TUI client for [Kiro CLI](https://kiro.dev) via the [Agent Client Protocol (ACP)](https://agentclientprotocol.com).

> **Status:** Alpha — functional but under active development.

## What is this?

Cyril is a terminal-based frontend for Kiro CLI, communicating over ACP (JSON-RPC 2.0 over stdio). It provides streaming markdown rendering, tool call visibility, and an approval workflow — all from your terminal.

- **On Linux** — runs `kiro-cli acp` directly as a subprocess
- **On Windows** — bridges to `kiro-cli` running inside WSL, with automatic path translation between `C:\` and `/mnt/c/` paths

```
Linux:   Cyril TUI  <── stdin/stdout JSON-RPC ──>  kiro-cli acp

Windows: Cyril TUI  <── stdin/stdout JSON-RPC ──>  WSL (kiro-cli acp)
              + automatic C:\ <-> /mnt/c/ path translation
```

## Features

- **Streaming TUI** — ratatui-based interface with real-time markdown rendering
- **Cross-platform** — runs natively on Linux; bridges to WSL on Windows with automatic path translation
- **Hook system** — JSON-configurable before/after hooks on file writes and terminal commands
- **Slash commands** — autocomplete-enabled commands from both the client and the Kiro agent
- **Tool call display** — see what the agent is doing in real time
- **Approval prompts** — review and approve file writes and command execution
- **Session management** — create, load, and switch between sessions

## Prerequisites

- [Kiro CLI](https://kiro.dev/docs/cli/) installed and authenticated (`kiro-cli login`)
- [Rust toolchain](https://rustup.rs/) (for building from source)
- **Windows only:** [WSL](https://learn.microsoft.com/en-us/windows/wsl/install) with kiro-cli installed inside it

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

The binary will be at `target/release/cyril` (or `cyril.exe` on Windows).

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
cyril -d /path/to/project        # Linux
cyril -d C:\Users\you\project    # Windows
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
