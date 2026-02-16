# Project: Windows-Native ACP Client for Kiro CLI

## Problem Statement

Kiro CLI doesn't run natively on Windows — only in WSL. For .NET/C# development work that needs to happen in Windows, working through WSL is clunky. The goal is a Windows-native experience that leverages Kiro CLI's full agent capabilities running in WSL, connected via the Agent Client Protocol (ACP).

## Key Insight: ACP Architecture

ACP (Agent Client Protocol) standardizes communication between code editors (clients) and AI coding agents (servers). It uses JSON-RPC 2.0 over stdio.

- **Agents (servers):** Kiro CLI, OpenCode, Claude Code, Goose — they implement `kiro-cli acp`, `opencode acp`, etc.
- **Clients:** Editors like Zed, JetBrains IDEs, Neovim plugins. The client spawns the agent as a subprocess and communicates over stdin/stdout.
- **Both sides advertise capabilities during initialization.** The client provides fs and terminal access; the agent provides AI reasoning, tool orchestration, MCP servers, skills, etc.

The agent calls *back into the client* for file system operations and terminal execution. The client doesn't contain any agent logic — it's a thin shell that provides I/O capabilities and renders the agent's output.

## Proposed Architecture

### Communication Flow

```
Windows Native (Rust TUI)  ←── stdin/stdout JSON-RPC ──→  WSL (kiro-cli acp)
       │                                                        │
       ├── UI (renders streaming output)                        ├── Agent loop (reasoning, planning)
       ├── Path Translator (C:\ ↔ /mnt/c/)                     ├── MCP Servers
       ├── Capability Provider (fs, terminal)                   ├── Skills, Steering Rules
       └── Hook System (pre/post execution)                     └── AI Model (cloud)
```

### Spawning

```
wsl kiro-cli acp
```

Stdin/stdout piping works across the WSL boundary, which is exactly what ACP's local transport requires.

### Path Translation

Critical piece — Kiro in WSL sees Linux paths, but the actual work lives on Windows:
- Kiro sees: `/mnt/c/Users/Daryl/project/Program.cs`
- Windows sees: `C:\Users\Daryl\project\Program.cs`
- The client translates at the boundary in both directions

### Terminal Execution

When Kiro requests terminal commands, the client can choose to run them natively in Windows (e.g., `dotnet build` against the Windows SDK) rather than inside WSL. This is important for .NET work.

## The Hook System (Key Differentiator)

Because the client owns the fs and terminal capabilities, every agent side effect passes through the client. This creates a middleware/hook layer that Kiro CLI doesn't natively support.

### Hook Types

**Before hooks (pre-execution):**
- Validate paths — block writes outside project directory
- Require approval for certain file patterns (`.env`, `appsettings.json`)
- Transform commands — rewrite Linux-flavored commands to Windows equivalents

**After hooks (post-execution):**
- File write → run `dotnet format` on the file
- File write to `*.cs` → trigger a build to check for compile errors
- Terminal output → parse for errors and feed back as context
- Any write → auto-commit to git with a descriptive message

**Aggregation hooks (across operations):**
- Track all files modified in a turn, run tests at TurnEnd
- Generate a summary of all changes in a session

**Feedback loops:**
- After hooks that produce useful output (build errors, test failures) can be injected as follow-up `session/prompt` calls automatically, creating tighter feedback loops than Kiro would have on its own.

### Why This Matters

- Kiro CLI has limitations around hooks natively
- These hooks are invisible to Kiro — it just sees success/failure responses
- Hooks survive Kiro updates since they operate at the protocol boundary
- Clean separation: Kiro owns the intelligence, the client owns the policy

## ACP Protocol Details

### Core Methods (Client → Agent)

| Method | Description |
|--------|-------------|
| `initialize` | Handshake, exchange capabilities |
| `session/new` | Create session (accepts cwd, mcpServers) |
| `session/load` | Load existing session |
| `session/prompt` | Send user input |
| `session/cancel` | Cancel current operation |
| `session/set_mode` | Switch agent mode |
| `session/set_model` | Change model |

### Notifications (Agent → Client)

| Type | Description |
|------|-------------|
| `AgentMessageChunk` | Streaming text/content |
| `ToolCall` | Tool invocation with name, params, status |
| `ToolCallUpdate` | Progress updates |
| `TurnEnd` | Agent turn completed |

### Client Capabilities (provided to agent)

- `fs.readTextFile` / `fs.writeTextFile`
- `terminal` execution
- Image support in prompts

### Kiro Extensions (prefixed `_kiro.dev/`)

- `commands/available` — sent after session creation, lists slash commands
- `commands/options` — autocomplete suggestions
- `commands/execute` — execute a slash command
- `mcp/oauth_request` — OAuth for MCP servers
- `mcp/server_initialized` — MCP server ready
- `compaction/status` — context compaction progress
- `clear/status` — session clear progress

### Session Storage

```
~/.kiro/sessions/cli/
├── <session-id>.json   # metadata
└── <session-id>.jsonl  # event log
```

## UI Requirements

- Slash command autocomplete (driven by `commands/available` and `commands/options`)
- Mode/model switching UI
- Tool call display (what Kiro is doing)
- Approval prompts for writes/commands
- Agent plan display
- Streaming markdown rendering
- Hook configuration/management

## Technology Choices

- **Language:** Rust (developer is experienced, Rust ACP SDK exists)
- **TUI framework:** TBD (ratatui, crossterm, etc.)
- **Target:** Windows native

## Minimum Viable Version

1. Spawn `wsl kiro-cli acp`
2. Handle initialize handshake
3. Create sessions with path translation
4. Send prompts, render streaming responses
5. Handle fs callbacks with path mapping
6. Handle terminal callbacks (run on Windows)

Then layer on incrementally:
- Hook system
- Slash command autocomplete
- Approval workflows
- Session management

## References

- ACP Spec: https://agentclientprotocol.com
- Kiro CLI ACP docs: https://kiro.dev/docs/cli/acp/
- ACP Rust SDK: available via the spec's libraries page
- ACP GitHub: https://github.com/agentclientprotocol/agent-client-protocol
