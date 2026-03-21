# Kiro CLI ACP Protocol Reference

This document describes the Agent Client Protocol (ACP) as implemented by **Kiro CLI v1.28.0**, based on the ACP v2025-01-01 specification. All findings were verified empirically by probing `kiro-cli acp` and examining its debug logs at `/run/user/$UID/kiro-log/kiro-chat.log` (Linux) or `$TMPDIR/kiro-log/kiro-chat.log` (macOS).

## Transport

- **Protocol**: JSON-RPC 2.0 over stdio
- **Spawn command**: `kiro-cli acp` (Linux) or `wsl kiro-cli acp` (Windows)
- **Flags**: `--agent <name>`, `--model <id>`, `--trust-all-tools`, `--verbose`
- **Logging**: Set `KIRO_LOG_LEVEL=debug` for verbose logs. Override log path with `KIRO_CHAT_LOG_FILE`.

## Extension Method Convention

ACP uses an underscore prefix (`_`) on the wire for extension methods. The `agent-client-protocol` crate strips this prefix before delivering to handlers. So:
- On the wire: `_kiro.dev/commands/execute`
- In `ext_notification`/`ext_method` handlers: `kiro.dev/commands/execute`

---

## Connection Lifecycle

### 1. `initialize` (client → server)

Exchange capabilities and identify both sides.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 0,
  "method": "initialize",
  "params": {
    "protocolVersion": "2025-01-01",
    "clientCapabilities": {},
    "clientInfo": {
      "name": "cyril",
      "version": "0.1.0",
      "title": "Cyril"
    }
  }
}
```

**Response:**
```json
{
  "agentInfo": {
    "name": "Kiro CLI Agent",
    "version": "1.28.0"
  },
  "agentCapabilities": {
    "loadSession": true,
    "promptCapabilities": {
      "image": true,
      "audio": false,
      "embeddedContext": false
    },
    "mcpCapabilities": {
      "http": false,
      "sse": false
    },
    "sessionCapabilities": {}
  }
}
```

**Key observations:**
- `promptCapabilities.image: true` — Kiro supports image content blocks in prompts
- `sessionCapabilities: {}` — No `fork`, `list`, or `resume` support (these are behind unstable feature flags in the ACP crate)
- `mcpCapabilities` — MCP servers only via stdio, not HTTP/SSE

### 2. `session/new` (client → server)

Create a new conversation session.

**Request:**
```json
{
  "method": "session/new",
  "params": {
    "cwd": "/home/user/project"
  }
}
```

**Response:**
```json
{
  "sessionId": "4dfac9d3-2a7b-4dda-8f7c-13900cc29028",
  "modes": {
    "currentModeId": "kiro_default",
    "availableModes": [
      { "id": "code-reviewer", "name": "code-reviewer" },
      { "id": "kiro_default", "name": "kiro_default" },
      { "id": "kiro_planner", "name": "kiro_planner" }
    ]
  },
  "configOptions": null
}
```

**Notes:**
- `configOptions` is always `null` in Kiro v1.28.0
- Modes come from agent configurations (`.kiro/agents/` directory)
- After session creation, Kiro sends `kiro.dev/metadata` and `kiro.dev/commands/available` extension notifications

### 3. `session/load` (client → server)

Load an existing session by ID.

**Request:**
```json
{
  "method": "session/load",
  "params": {
    "sessionId": "4dfac9d3-2a7b-4dda-8f7c-13900cc29028",
    "cwd": "/home/user/project"
  }
}
```

### 4. `session/prompt` (client → server)

Send a user message to the agent. This starts a "turn" — the agent processes the prompt, streams responses via `session/update` notifications, and returns when done.

**Request:**
```json
{
  "method": "session/prompt",
  "params": {
    "sessionId": "4dfac9d3-...",
    "content": [
      { "type": "text", "text": "Explain this code" }
    ]
  }
}
```

**Response** (returned when the turn completes):
```json
{
  "stopReason": "end_turn"
}
```

**Stop reasons:** `end_turn`, `max_tokens`, `cancelled`

### 5. `session/cancel` (client → server, notification)

Cancel the current operation. Fire-and-forget — no response expected.

```json
{
  "method": "session/cancel",
  "params": {
    "sessionId": "4dfac9d3-..."
  }
}
```

### 6. `session/set_mode` (client → server)

Switch the agent mode.

**Request:**
```json
{
  "method": "session/set_mode",
  "params": {
    "sessionId": "4dfac9d3-...",
    "modeId": "kiro_planner"
  }
}
```

**Response:**
```json
{
  "meta": null
}
```

### 7. `session/set_config_option` (client → server)

**NOT IMPLEMENTED** by Kiro v1.28.0. Returns:
```json
{
  "error": {
    "code": -32601,
    "message": "Method not found: \"session/set_config_option\""
  }
}
```

Use `kiro.dev/commands/execute` with the `model` command instead (see Kiro Extensions below).

### 8. `session/set_model` (client → server)

Behind `unstable_session_model` feature flag in the ACP crate. Not advertised in Kiro's `sessionCapabilities`. Status unknown — use `kiro.dev/commands/execute` for model switching.

---

## Session Update Notifications (`session/update`, server → client)

Sent as `SessionNotification` containing a `SessionUpdate` enum, discriminated by the `sessionUpdate` field.

### AgentMessageChunk

Streaming text content from the agent.

```json
{
  "method": "session/update",
  "params": {
    "sessionId": "4dfac9d3-...",
    "update": {
      "sessionUpdate": "agent_message_chunk",
      "content": {
        "type": "text",
        "text": "Here is the explanation..."
      }
    }
  }
}
```

### AgentThoughtChunk

Internal reasoning from the agent (extended thinking).

```json
{
  "update": {
    "sessionUpdate": "agent_thought_chunk",
    "content": {
      "type": "text",
      "text": "Let me analyze the code structure..."
    }
  }
}
```

### ToolCall

A tool invocation has been initiated. Follows a three-phase lifecycle:

**Phase 1 — InProgress** (tool initiated):
```json
{
  "update": {
    "sessionUpdate": "tool_call",
    "toolCallId": "tc_001",
    "name": "read",
    "status": "in_progress",
    "rawInput": { "path": "/home/user/src/main.rs" }
  }
}
```

**Phase 2 — Pending** (title updated, awaiting permission if needed):
```json
{
  "update": {
    "sessionUpdate": "tool_call",
    "toolCallId": "tc_001",
    "name": "read",
    "status": "pending",
    "title": "Reading main.rs:1-50"
  }
}
```

### ToolCallUpdate

**Phase 3 — Completed:**
```json
{
  "update": {
    "sessionUpdate": "tool_call_update",
    "toolCallId": "tc_001",
    "status": "completed",
    "output": "fn main() { ... }"
  }
}
```

### Plan

The agent's execution plan for complex tasks. Each update replaces the previous plan entirely.

```json
{
  "update": {
    "sessionUpdate": "plan",
    "title": "Implementation Plan",
    "steps": [
      { "description": "Read the config file", "status": "completed" },
      { "description": "Add the new field", "status": "in_progress" },
      { "description": "Update tests", "status": "pending" }
    ]
  }
}
```

### AvailableCommandsUpdate

Standard ACP command updates. These may arrive during the session alongside the Kiro-specific `kiro.dev/commands/available`.

### CurrentModeUpdate

```json
{
  "update": {
    "sessionUpdate": "current_mode_update",
    "currentModeId": "kiro_planner"
  }
}
```

### ConfigOptionUpdate

```json
{
  "update": {
    "sessionUpdate": "config_option_update",
    "configOptions": [...]
  }
}
```

**Note:** Kiro v1.28.0 does not send `ConfigOptionUpdate` notifications. Config options are always `null`.

---

## Permission Requests (`session/request_permission`, server → client)

A JSON-RPC request (has an `id`, expects a response). The agent asks for permission before executing certain tools.

**Request:**
```json
{
  "method": "session/request_permission",
  "params": {
    "toolCall": {
      "toolCallId": "tc_002",
      "name": "shell",
      "rawInput": { "command": "npm test" }
    },
    "options": [
      { "id": "allow_once", "label": "Yes" },
      { "id": "allow_always", "label": "Always" },
      { "id": "reject_once", "label": "No" }
    ]
  }
}
```

**Response:**
```json
{
  "outcome": {
    "type": "selected",
    "optionId": "allow_once"
  }
}
```

Or to cancel:
```json
{
  "outcome": {
    "type": "cancelled"
  }
}
```

**Observations:**
- File reads do not require permission
- Shell commands require permission
- `allow_always` makes the agent remember the choice for the session

---

## Client Capabilities (NOT used by Kiro v1.28.0)

The ACP spec defines client-side callbacks for filesystem and terminal operations (`fs/read_text_file`, `fs/write_text_file`, `terminal/create`, etc.). These would allow the server to delegate host operations to the client.

**Kiro does not use these.** Instead, Kiro has its own built-in agent tools (`read`, `write`, `shell`, `ls`, `glob`, `grep`, etc.) that it executes server-side. The client only needs to handle:
- `session/update` notifications (streaming content, tool calls)
- `session/request_permission` requests (permission approval UI)
- Extension notifications/methods (`kiro.dev/*`)

Cyril advertises empty `clientCapabilities` during `initialize`. The `tool_call_inputs` cache enriches permission requests with `rawInput` for the approval UI, but actual file I/O and command execution happen inside kiro-cli.

---

## Kiro Extension Methods

### `kiro.dev/commands/available` (server → client, notification)

Sent after session creation with the full list of available commands, tools, and MCP servers.

```json
{
  "method": "_kiro.dev/commands/available",
  "params": {
    "sessionId": "4dfac9d3-...",
    "commands": [
      {
        "name": "/agent",
        "description": "Select or list available agents",
        "meta": {
          "optionsMethod": "_kiro.dev/commands/agent/options",
          "inputType": "selection",
          "hint": ""
        }
      },
      {
        "name": "/clear",
        "description": "Clear conversation history"
      },
      {
        "name": "/context",
        "description": "Manage context files or show token usage",
        "meta": {
          "inputType": "panel",
          "hint": "add <path>, remove <path>, clear"
        }
      },
      {
        "name": "/model",
        "description": "Select or list available models",
        "meta": {
          "optionsMethod": "_kiro.dev/commands/model/options",
          "inputType": "selection",
          "hint": ""
        }
      },
      {
        "name": "/quit",
        "description": "Quit the application",
        "meta": { "local": true }
      }
    ],
    "prompts": [],
    "tools": [
      {
        "name": "read",
        "description": "A tool for viewing file contents...",
        "source": "built-in"
      }
    ],
    "mcpServers": []
  }
}
```

**Command types** (determined by `meta.inputType`):
- **`selection`** — requires a picker UI; has `optionsMethod` for querying choices
- **`panel`** — returns structured data (execute and display the `message` field)
- **`(none)`** — simple fire-and-execute command
- **`local: true`** — handled entirely client-side (e.g. `/quit`)

**Full command list (Kiro v1.28.0):**

| Command | inputType | local | optionsMethod |
|---------|-----------|-------|---------------|
| `/agent` | selection | no | `_kiro.dev/commands/agent/options` |
| `/chat` | selection | yes | — |
| `/clear` | — | no | — |
| `/compact` | — | no | — |
| `/context` | panel | no | — |
| `/feedback` | selection | no | — |
| `/help` | panel | no | — |
| `/knowledge` | panel | no | — |
| `/mcp` | panel | no | — |
| `/model` | selection | no | `_kiro.dev/commands/model/options` |
| `/paste` | — | no | — |
| `/plan` | — | no | — |
| `/prompts` | selection | no | `_kiro.dev/commands/prompts/options` |
| `/quit` | — | yes | — |
| `/reply` | — | no | — |
| `/tools` | panel | no | — |
| `/usage` | panel | no | — |

### `kiro.dev/commands/options` (client → server, request)

Query available options for a selection command.

**Request:**
```json
{
  "method": "_kiro.dev/commands/options",
  "params": {
    "command": "model",
    "sessionId": "4dfac9d3-...",
    "partial": ""
  }
}
```

**Response (model):**
```json
{
  "options": [
    {
      "value": "auto",
      "label": "auto",
      "description": "Models chosen by task for optimal usage and consistent quality",
      "group": "1.00x credits"
    },
    {
      "value": "claude-opus-4.6",
      "label": "claude-opus-4.6",
      "description": "Experimental preview of Claude Opus 4.6",
      "group": "2.20x credits"
    },
    {
      "value": "claude-sonnet-4.6",
      "label": "claude-sonnet-4.6",
      "description": "Experimental preview of the latest Claude Sonnet model",
      "group": "1.30x credits"
    },
    {
      "value": "claude-haiku-4.5",
      "label": "claude-haiku-4.5",
      "description": "The latest Claude Haiku model",
      "group": "0.40x credits"
    }
  ],
  "hasMore": false
}
```

**Option fields:**
- `value` — the ID to send back in `commands/execute`
- `label` — display name (use this, not `name`)
- `description` — longer description
- `group` — grouping label (e.g. credit tier)
- `current` — (optional) boolean, true if this is the active option

### `kiro.dev/commands/execute` (client → server, request)

Execute a slash command. The `command` field is a `TuiCommand` adjacently tagged enum.

**CRITICAL:** The `command` field must be an object `{"command": "<name>", "args": {<args>}}`, not a string. Sending a string crashes `kiro-cli`.

#### Panel command (no args):

**Request:**
```json
{
  "method": "_kiro.dev/commands/execute",
  "params": {
    "sessionId": "4dfac9d3-...",
    "command": {
      "command": "context",
      "args": {}
    }
  }
}
```

**Response:**
```json
{
  "success": true,
  "message": "Context breakdown - 5% used",
  "data": {
    "model": "auto",
    "contextUsagePercentage": 5.547,
    "verbose": false,
    "breakdown": {
      "contextFiles": {
        "tokens": 6124,
        "percent": 3.062,
        "items": [
          { "name": "AGENTS.md", "tokens": 5284, "matched": true, "percent": 2.642 },
          { "name": "README.md", "tokens": 840, "matched": true, "percent": 0.420 }
        ]
      },
      "tools": { "tokens": 4913, "percent": 2.457 },
      "kiroResponses": { "tokens": 0, "percent": 0.0 },
      "yourPrompts": { "tokens": 57, "percent": 0.029 },
      "sessionFiles": { "tokens": 0, "percent": 0.0 }
    }
  }
}
```

#### Selection command (with value):

**Request:**
```json
{
  "method": "_kiro.dev/commands/execute",
  "params": {
    "sessionId": "4dfac9d3-...",
    "command": {
      "command": "model",
      "args": {
        "value": "claude-haiku-4.5"
      }
    }
  }
}
```

**Response:**
```json
{
  "success": true,
  "message": "Model changed to claude-haiku-4.5",
  "data": {
    "model": {
      "id": "claude-haiku-4.5",
      "name": "claude-haiku-4.5"
    }
  }
}
```

#### Simple command (no args):

**Request:**
```json
{
  "method": "_kiro.dev/commands/execute",
  "params": {
    "sessionId": "4dfac9d3-...",
    "command": {
      "command": "compact",
      "args": {}
    }
  }
}
```

**Response:**
```json
{
  "success": false,
  "message": "Conversation too short to compact."
}
```

**Response format:**
- `success: bool` — whether the command succeeded
- `message: string` — human-readable result (always present)
- `data: object` — optional structured data (command-specific)

### `kiro.dev/metadata` (server → client, notification)

Sent after each turn with session metadata. **Not documented in official Kiro docs** but consistently sent.

```json
{
  "method": "_kiro.dev/metadata",
  "params": {
    "sessionId": "4dfac9d3-...",
    "contextUsagePercentage": 3.09
  }
}
```

---

## Unimplemented Kiro Extensions

These are documented in the Kiro ACP docs but not yet handled by Cyril.

### `kiro.dev/mcp/oauth_request` (server → client, notification)

Provides an OAuth URL when an MCP server requires authentication.

### `kiro.dev/mcp/server_initialized` (server → client, notification)

Indicates an MCP server has finished initializing and its tools are available.

### `kiro.dev/compaction/status` (server → client, notification)

Reports progress when compacting conversation context.

### `kiro.dev/clear/status` (server → client, notification)

Reports status when clearing session history.

### `session/terminate` (server → client, notification)

Terminates a subagent session. Related to the unstable `session/fork` capability in the ACP crate (behind `unstable_session_fork` feature flag). Not active since `sessionCapabilities` is empty.

---

## Unstable ACP Features

These exist in the `agent-client-protocol-schema` crate behind feature flags but are **not advertised** by Kiro v1.28.0:

| Feature Flag | Method | Description |
|---|---|---|
| `unstable_session_fork` | `session/fork` | Fork a session to create an independent child session |
| `unstable_session_resume` | `session/resume` | Resume a paused session |
| `unstable_session_list` | `session/list` | List existing sessions |
| `unstable_session_model` | `session/set_model` | Dedicated model switching (vs config option) |
| `unstable_session_usage` | `UsageUpdate` notification | Token/cost usage per turn |
| `unstable_session_info_update` | `SessionInfoUpdate` notification | Session title/metadata changes |

---

## Debugging

### Log locations

- **Linux**: `$XDG_RUNTIME_DIR/kiro-log/kiro-chat.log` (typically `/run/user/1000/kiro-log/kiro-chat.log`)
- **macOS**: `$TMPDIR/kiro-log/kiro-chat.log`

### Environment variables

- `KIRO_LOG_LEVEL=debug` — verbose logging
- `KIRO_CHAT_LOG_FILE=/path/to/custom.log` — custom log path

### Common errors in kiro-cli logs

**Wrong command format:**
```
Connection error: Parse error: {
  "error": "invalid type: string \"/context\", expected adjacently tagged enum TuiCommand",
  "json": { "command": "/context", "sessionId": "..." },
  "phase": "deserialization"
}
```
This means the `command` field was sent as a string instead of the required `{"command": "<name>", "args": {}}` object format.

**Unsupported method:**
```
Method not found: "session/set_config_option"
```
The ACP method is not implemented by this version of Kiro.

---

## Version History

| Date | Kiro Version | ACP Schema | Notes |
|------|-------------|------------|-------|
| 2026-03-20 | v1.28.0 | v0.10.8 | Initial investigation. Discovered TuiCommand format, broken set_config_option. |
