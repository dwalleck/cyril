# Kiro CLI ACP Protocol Reference

This document describes the Agent Client Protocol (ACP) as implemented by **Kiro CLI v1.28.0**, based on the ACP v2025-01-01 specification. It serves as a reference for any ACP client connecting to Kiro, not just Cyril. All findings were verified empirically by probing `kiro-cli acp` and examining its debug logs.

## Transport

- **Protocol**: JSON-RPC 2.0 over stdio
- **Spawn command**: `kiro-cli acp` (Linux/macOS) or `wsl kiro-cli acp` (Windows)
- **Flags**: `--agent <name>`, `--model <id>`, `--trust-all-tools`, `--verbose`
- **Logging**: Set `KIRO_LOG_LEVEL=debug` for verbose logs. Override log path with `KIRO_CHAT_LOG_FILE`.
- **Log locations**: `$XDG_RUNTIME_DIR/kiro-log/kiro-chat.log` (Linux), `$TMPDIR/kiro-log/kiro-chat.log` (macOS)

## Extension Method Convention

ACP uses an underscore prefix (`_`) on the wire for extension methods. The `agent-client-protocol` crate (v0.9+) strips this prefix before delivering to handlers. So:
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
      "name": "my-client",
      "version": "1.0.0",
      "title": "My ACP Client"
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

**Notes:**
- `promptCapabilities.image: true` — Kiro supports image content blocks in prompts
- `sessionCapabilities: {}` — No `fork`, `list`, or `resume` support yet
- `mcpCapabilities` — MCP servers only via stdio, not HTTP/SSE
- **Client capabilities are not used** — Kiro handles all file I/O and terminal commands internally via built-in agent tools (`read`, `write`, `shell`, `ls`, `glob`, `grep`, etc.). The ACP spec defines client-side callbacks (`fs/read_text_file`, `fs/write_text_file`, `terminal/create`, etc.) but Kiro never invokes them. Clients only need to handle notifications, permission requests, and extension methods.

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

**Not supported** by Kiro v1.28.0. Returns:
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

**Not supported.** Behind `unstable_session_model` feature flag in the ACP crate. Not advertised in Kiro's `sessionCapabilities`. Use `kiro.dev/commands/execute` for model switching.

---

## Session Update Notifications (`session/update`, server → client)

Sent as `SessionNotification` containing a `SessionUpdate` enum, discriminated by the `sessionUpdate` field. Kiro v1.28.0 sends `agent_message_chunk`, `tool_call`, and `tool_call_update` as the primary variants. `plan` is sent for complex multi-step tasks.

### AgentMessageChunk

Streaming text content from the agent. This is the main output mechanism — the agent's response arrives as a stream of these chunks.

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

Internal reasoning from the agent (extended thinking). Same structure as `AgentMessageChunk`. Sent when the model uses extended thinking — not observed in standard sessions but defined in the ACP spec and handled identically to message chunks.

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

A tool invocation initiated by the agent. Kiro executes tools server-side (via built-in tools like `read`, `write`, `shell`) and reports progress via these notifications. Tool calls follow a two-phase lifecycle through this notification type:

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

**Phase 2 — Pending** (title updated, may await permission):
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

Note: Kiro also sends lightweight `tool_call_chunk` updates via `kiro.dev/session/update` (see Kiro Extensions). These often arrive before the standard `ToolCall` notification and provide early visibility into tool activity.

### ToolCallUpdate

Completion notification for a tool call (phase 3 of the lifecycle):

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

The agent's execution plan for complex tasks. Sent when the agent creates or updates a plan. Each update replaces the previous plan entirely.

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

Standard ACP command list updates. Kiro sends command lists primarily via the `kiro.dev/commands/available` extension notification (which includes richer metadata like `inputType` and `optionsMethod`), but may also send this standard variant.

### CurrentModeUpdate

Standard ACP mode change notification. Kiro sends mode changes primarily via the `kiro.dev/agent/switched` extension notification (which includes `previousAgentName` and `welcomeMessage`), but may also send this standard variant.

```json
{
  "update": {
    "sessionUpdate": "current_mode_update",
    "currentModeId": "kiro_planner"
  }
}
```

### ConfigOptionUpdate

Standard ACP config option updates. Not sent by Kiro v1.28.0 — config options are always `null`. Model switching is done via `kiro.dev/commands/execute` instead.

```json
{
  "update": {
    "sessionUpdate": "config_option_update",
    "configOptions": [...]
  }
}
```

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

## Kiro Extension Methods

These are Kiro-specific methods not part of the standard ACP specification. They use the `_kiro.dev/` prefix on the wire.

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

**CRITICAL:** The `command` field must be an object `{"command": "<name>", "args": {<args>}}`, not a string. Sending a string crashes `kiro-cli` with a deserialization error.

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

Sent after each turn with session metadata.

```json
{
  "method": "_kiro.dev/metadata",
  "params": {
    "sessionId": "4dfac9d3-...",
    "contextUsagePercentage": 3.09
  }
}
```

### `kiro.dev/agent/switched` (server → client, notification)

Sent when the agent is switched (e.g. via `/agent` picker).

```json
{
  "method": "_kiro.dev/agent/switched",
  "params": {
    "sessionId": "4dfac9d3-...",
    "agentName": "code-reviewer",
    "previousAgentName": "kiro_default",
    "welcomeMessage": null
  }
}
```

**Fields:**
- `agentName` — the new agent name (matches mode IDs from `session/new`)
- `previousAgentName` — the agent that was active before
- `welcomeMessage` — optional greeting from the new agent (typically null)

### `kiro.dev/session/update` (server → client, notification)

Kiro-specific session updates sent via the extension mechanism, separate from the standard ACP `session/update`. Currently one variant observed:

#### `tool_call_chunk`

Lightweight tool call progress. Sent alongside (or before) the standard ACP `session/update` → `ToolCall` notifications. Provides just the tool name and kind without full rawInput.

```json
{
  "method": "_kiro.dev/session/update",
  "params": {
    "sessionId": "4dfac9d3-...",
    "update": {
      "sessionUpdate": "tool_call_chunk",
      "toolCallId": "tooluse_abc123",
      "title": "read",
      "kind": "read"
    }
  }
}
```

**`kind` values observed:** `read`, `execute`, `search`

**`title` values observed:** `read`, `ls`, `glob`, `shell`

### `kiro.dev/compaction/status` (server → client, notification)

Reports progress when compacting conversation context via `/compact`.

```json
{
  "method": "_kiro.dev/compaction/status",
  "params": {
    "message": "Compacting conversation context..."
  }
}
```

### `kiro.dev/clear/status` (server → client, notification)

Reports status when clearing session history via `/clear`.

```json
{
  "method": "_kiro.dev/clear/status",
  "params": {
    "message": "Clearing session history..."
  }
}
```

### `kiro.dev/mcp/oauth_request` (server → client, notification)

Provides an OAuth URL when an MCP server requires authentication. Documented in the [Kiro ACP docs](https://kiro.dev/docs/cli/acp/#kiro-extensions) but not observed in production logs. Exact payload format unknown.

### `kiro.dev/mcp/server_initialized` (server → client, notification)

Indicates an MCP server has finished initializing and its tools are available. Documented in the [Kiro ACP docs](https://kiro.dev/docs/cli/acp/#kiro-extensions) but not observed in production logs. Exact payload format unknown.

### `session/terminate` (server → client, notification)

Terminates a subagent session. Documented in the [Kiro ACP docs](https://kiro.dev/docs/cli/acp/#kiro-extensions). Related to the unstable `session/fork` capability in the ACP crate (behind `unstable_session_fork` feature flag). Not active since Kiro v1.28.0 reports `sessionCapabilities: {}`.

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

### Common errors in kiro-cli logs

**Wrong command format:**
```
Connection error: Parse error: {
  "error": "invalid type: string \"/context\", expected adjacently tagged enum TuiCommand",
  "json": { "command": "/context", "sessionId": "..." },
  "phase": "deserialization"
}
```
This means the `command` field was sent as a string instead of the required `{"command": "<name>", "args": {}}` object format. This error crashes the kiro-cli agent connection.

**Unsupported method:**
```
Method not found: "session/set_config_option"
```
The ACP method is not implemented by this version of Kiro.

---

## Version History

| Date | Kiro Version | ACP Schema | Notes |
|------|-------------|------------|-------|
| 2026-03-20 | v1.28.0 | v0.10.8 | Initial investigation. Discovered TuiCommand format, broken set_config_option, agent/switched and tool_call_chunk notifications. |
