# Cyril

*The polished TUI for the [Agent Client Protocol](https://agentclientprotocol.com) ecosystem.*

## What is this?

Cyril is a polished terminal interface for the Agent Client Protocol ecosystem. Run any of 37+ registered agents — Claude, Cursor, Codex, Cline, Goose, Kiro, and more — through a single interface. Beneath the TUI, composable proxy stages add behaviors no agent ships natively: skill systems, transcript audit, organizational permission policies, persistent memory across sessions, multi-client observers. Vendor neutrality is a feature, not a roadmap; stages are how cyril compounds value over time.

> **Status:** Alpha. Today cyril works against [Kiro CLI](https://kiro.dev); vendor-neutral agent selection and the proxy-stage layer are in active development. The features and usage documentation below describe the current Kiro-focused implementation.

## Features

- **Streaming TUI** — ratatui-based interface with real-time markdown rendering (headings, bold, italic, code blocks with syntax highlighting, tables, lists, blockquotes)
- **Cross-platform** — runs natively on Linux; bridges to WSL on Windows with automatic path translation
- **Slash commands** — autocomplete-enabled commands from both the client and the Kiro agent
- **Tool call display** — see what the agent is doing in real time with inline diffs
- **Approval prompts** — review and approve command execution with Yes/Always/No options
- **Session management** — create, load, and resume previous sessions via `/chat`
- **Agent/model switching** — switch agents (`/agent`) and models (`/model`) via picker UI
- **Live activity indicator** — animated spinner with elapsed time and current tool activity in the toolbar
- **Context bar** — visual gauge showing context window usage
- **@-file references** — reference files in prompts with `@path/to/file` autocomplete

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
| `Ctrl+M` | Toggle mouse capture (off = copy mode) |
| `Ctrl+C` / `Ctrl+Q` | Quit |

### Slash commands

**Local commands** (handled by Cyril):

| Command | Description |
|---------|-------------|
| `/help` | Show available commands |
| `/new` | Start a new session |
| `/load <id>` | Load a session by ID |
| `/clear` | Clear the chat |
| `/mode <id>` | Switch agent mode |
| `/model [id]` | Switch model (opens picker if no ID given) |
| `/quit` | Quit |

**Agent commands** (forwarded to Kiro via ACP):

| Command | Description |
|---------|-------------|
| `/agent` | Switch agent (picker) |
| `/chat` | Resume a previous session (picker) |
| `/compact` | Compact conversation history |
| `/context` | Show context/token usage breakdown |
| `/knowledge` | Manage knowledge bases |
| `/mcp` | Show configured MCP servers |
| `/plan` | Switch to planning agent |
| `/prompts` | Select from available prompts (picker) |
| `/tools` | Show available agent tools |
| `/usage` | Show billing and usage info |

## Project structure

```
crates/
  cyril/          # TUI application (binary)
  cyril-core/     # Protocol logic, path translation, session state
docs/
  kiro-acp-protocol.md  # Comprehensive Kiro ACP protocol reference
```

## License

[MIT](LICENSE)
