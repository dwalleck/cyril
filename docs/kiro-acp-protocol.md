# Kiro CLI ACP Protocol Reference

This document describes the Agent Client Protocol (ACP) as implemented by **Kiro CLI v1.29.0**, based on the ACP v2025-01-01 specification. It serves as a reference for any ACP client connecting to Kiro, not just Cyril. All findings were verified empirically by probing `kiro-cli acp` and examining its debug logs.

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
    "version": "1.29.0"
  },
  "agentCapabilities": {
    "loadSession": true,
    "promptCapabilities": {
      "image": true,
      "audio": false,
      "embeddedContext": false
    },
    "mcpCapabilities": {
      "http": true,
      "sse": false
    },
    "sessionCapabilities": {}
  }
}
```

**Notes:**
- `promptCapabilities.image: true` — Kiro supports image content blocks in prompts
- `sessionCapabilities: {}` — Still empty in v1.29.0. Subagent support is via Kiro extensions, not standard ACP session capabilities.
- `mcpCapabilities.http: true` — **Changed in v1.29.0** (was `false` in v1.28.0). MCP servers now support HTTP transport in addition to stdio.
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
      { "id": "code-reviewer", "name": "code-reviewer", "description": "Reviews code..." },
      { "id": "kiro_default", "name": "kiro_default", "description": "The default agent..." },
      {
        "id": "kiro_planner",
        "name": "kiro_planner",
        "description": "Specialized planning agent...",
        "_meta": { "welcomeMessage": "Transform any idea into fully working code. What do you want to build today?" }
      }
    ]
  },
  "models": {
    "currentModelId": "auto",
    "availableModels": [
      { "modelId": "auto", "name": "auto", "description": "Models chosen by task for optimal usage and consistent quality" },
      { "modelId": "claude-opus-4.6", "name": "claude-opus-4.6", "description": "The latest Claude Opus model with 1M context window" },
      { "modelId": "claude-sonnet-4.6", "name": "claude-sonnet-4.6", "description": "The latest Claude Sonnet model with 1M context window" },
      { "modelId": "claude-haiku-4.5", "name": "claude-haiku-4.5", "description": "The latest Claude Haiku model" },
      { "modelId": "deepseek-3.2", "name": "deepseek-3.2", "description": "Experimental preview of DeepSeek V3.2" },
      { "modelId": "minimax-m2.5", "name": "minimax-m2.5", "description": "Experimental preview of MiniMax M2.5" },
      { "modelId": "glm-5", "name": "glm-5", "description": "Experimental preview of GLM-5" },
      { "modelId": "qwen3-coder-next", "name": "qwen3-coder-next", "description": "Experimental preview of Qwen3 Coder Next" }
    ]
  }
}
```

**Notes:**
- **`models` is new in v1.29.0** — provides model list and current selection directly in the session response. Replaces the workaround of extracting model info from `/model` command responses.
- `configOptions` is omitted (was always `null` in v1.28.0)
- Modes come from agent configurations (`.kiro/agents/` directory)
- Modes may include `_meta.welcomeMessage` (e.g., `kiro_planner`)
- After session creation, Kiro sends `kiro.dev/metadata`, `kiro.dev/commands/available`, and `kiro.dev/subagent/list_update` (empty) extension notifications

### 3. `session/load` (client → server)

Load an existing session by ID. **In v1.29.0**, the response now includes `models` and `modes` — same structure as `session/new`.

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

**Response** (same shape as `session/new`):
```json
{
  "sessionId": "4dfac9d3-...",
  "modes": { "currentModeId": "kiro_default", "availableModes": [...] },
  "models": { "currentModelId": "auto", "availableModels": [...] }
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

**Not supported** by Kiro v1.29.0. Returns:
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

### 9. `session/spawn` (client → server, new in v1.29.0)

Spawn a new child session (subagent). Not part of the standard ACP spec — this is a Kiro extension exposed as a top-level method.

**Request:**
```json
{
  "method": "session/spawn",
  "params": {
    "sessionId": "4dfac9d3-...",
    "task": "Review the code for bugs",
    "name": "code-reviewer"
  }
}
```

**Response:**
```json
{
  "sessionId": "b49d53d1-...",
  "name": "code-reviewer"
}
```

The spawned session appears in subsequent `kiro.dev/subagent/list_update` notifications.

### 10. `session/terminate` (client → server, new in v1.29.0)

Terminate a running session.

### 11. `session/attach` (client → server, new in v1.29.0)

Attach to a running session to receive its updates.

### 12. `message/send` (client → server, new in v1.29.0)

Send a message to a specific session.

**Request:**
```json
{
  "method": "message/send",
  "params": {
    "sessionId": "b49d53d1-...",
    "content": "Focus on the error handling in storage/mod.rs"
  }
}
```

### 13. `session/list` (client → server, new in v1.29.0)

List all active sessions. Exact response format not yet observed in production logs.

---

## Session Update Notifications (`session/update`, server → client)

Sent as `SessionNotification` containing a `SessionUpdate` enum, discriminated by the `sessionUpdate` field. Kiro sends `agent_message_chunk`, `tool_call`, and `tool_call_update` as the primary variants. `plan` is sent for complex multi-step tasks.

**Session ID scoping (new in v1.29.0):** Every `session/update` notification carries a `sessionId` field. When subagents are active, notifications arrive for multiple sessions over the same connection. The `sessionId` is the demuxing key — clients must route notifications to the correct session's state. Notifications where `sessionId` matches the main session are handled as before; notifications with a different `sessionId` belong to a subagent session.

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

Standard ACP config option updates. Not sent by Kiro v1.28.0 or v1.29.0. Model switching is done via `kiro.dev/commands/execute` instead. The `models` field in `session/new` response (v1.29.0) provides model info without needing config options.

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

Sent after session creation with the full list of available commands, prompts, tools, and MCP servers.

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

**Full command list (Kiro v1.28.0, may have expanded in v1.29.0):**

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

#### Prompts

The `prompts` array in `commands/available` lists available prompt templates from both file-based sources (`.kiro/prompts/`) and MCP servers. All prompts carry an `arguments` field in the protocol, but **file-based prompts currently always send `arguments: []`** — only MCP prompts populate arguments. The TUI code handles arguments uniformly regardless of source, suggesting file-based prompt arguments may be added in a future release.

```json
{
  "prompts": [
    {
      "name": "review-pr",
      "description": "Review a pull request for issues",
      "serverName": "file-prompts",
      "arguments": [
        { "name": "branch", "required": true },
        { "name": "scope", "required": false }
      ]
    },
    {
      "name": "explain-code",
      "description": "Explain how a piece of code works",
      "serverName": "mcp-docs-server",
      "arguments": [
        { "name": "file_path", "required": true }
      ]
    }
  ]
}
```

**Prompt fields:**
- `name` — the prompt identifier (invoked as `/<name>`)
- `description` — human-readable description
- `serverName` — source of the prompt (e.g., `"file-prompts"` for `.kiro/prompts/`, or an MCP server name)
- `arguments` — **new in v1.29.0 for file-based prompts**: array of parameter definitions
  - `name` — parameter name
  - `required` — whether the argument must be provided

**Argument display:** Clients should show argument hints in autocomplete/picker: `<branch>` for required args, `[scope]` for optional ones.

**Prompt execution:** Prompts are executed by sending the prompt name and arguments as a plain text message via `session/prompt`:

```json
{
  "method": "session/prompt",
  "params": {
    "sessionId": "4dfac9d3-...",
    "content": [
      { "type": "text", "text": "/review-pr main src/api" }
    ]
  }
}
```

The **server** parses the slash command, extracts arguments by position, and resolves the prompt template. The client does not need to perform structured argument passing — it forwards the raw text. This is identical for both file-based and MCP prompts.

**Current state (v1.29.0):**
- File-based prompts (local and global) **do not support arguments** — the `arguments` field is always `[]`. [Kiro docs confirm this](https://kiro.dev/docs/cli/chat/manage-prompts/).
- MCP prompts can declare arguments with `name` and `required` fields
- The `arguments` field is present on all prompts in the protocol regardless of source — the TUI handles them uniformly, suggesting file-based prompt arguments may be added in a future release
- The prompt command appears in slash-command autocomplete alongside regular commands, distinguished by `meta.type: "prompt"`
- **Verified empirically:** Created a test file prompt with YAML frontmatter declaring arguments — kiro-cli ignored the frontmatter and sent `arguments: []`

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

Sent after each turn with session metadata. **In v1.29.0**, the post-turn metadata notification also carries `meteringUsage` and `turnDurationMs`.

**Initial metadata** (on session creation):
```json
{
  "method": "_kiro.dev/metadata",
  "params": {
    "sessionId": "4dfac9d3-...",
    "contextUsagePercentage": 2.28
  }
}
```

**Post-turn metadata** (after a prompt completes, verified empirically):
```json
{
  "method": "_kiro.dev/metadata",
  "params": {
    "sessionId": "4dfac9d3-...",
    "contextUsagePercentage": 7.11,
    "meteringUsage": [
      { "unit": "credit", "unitPlural": "credits", "value": 0.018139567827529027 }
    ],
    "turnDurationMs": 1948
  }
}
```

**New fields (v1.29.0):**
- `meteringUsage` — array of cost entries per turn. Each has `value` (numeric cost), `unit` (singular label), `unitPlural` (plural label). Typically one entry for credits.
- `turnDurationMs` — wall-clock duration of the turn in milliseconds

**Note:** Token-level usage (`inputTokens`, `outputTokens`, `cachedTokens`) is NOT available through the `ext_notification` path. The TUI extracts these from raw stream `MetadataEvent` internals processed by the ACP crate. The ACP schema's `UsageUpdate` session update (`unstable_session_usage` feature flag) compiles but Kiro v1.29.0 does not send it.

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

### `kiro.dev/error/rate_limit` (server → client, notification, new in v1.29.0)

Sent when the agent hits a rate limit. Clients should display the message as a transient error.

```json
{
  "method": "_kiro.dev/error/rate_limit",
  "params": {
    "message": "Rate limit exceeded. Please wait before retrying."
  }
}
```

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

**Session scoping (v1.29.0):** When `sessionId` differs from the main session, this is a tool call from a subagent. The TUI checks `sessionId !== this.sessionId` to route subagent tool events to multi-session handlers.

### `kiro.dev/compaction/status` (server → client, notification)

Reports progress when compacting conversation context via `/compact`. **In v1.29.0**, the payload gained structured status fields alongside the legacy `message` field.

```json
{
  "method": "_kiro.dev/compaction/status",
  "params": {
    "status": {
      "type": "started"
    }
  }
}
```

**`status.type` values:** `"started"`, `"completed"`, `"failed"`

When `type` is `"failed"`, an `error` field contains the failure reason. An optional `summary` field may be present on completion.

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

---

## Subagent & Multi-Session Protocol (new in v1.29.0)

Kiro v1.29.0 introduces subagent support via extension methods. The main agent can spawn child sessions ("stages") that run in parallel, each with its own tool access and message stream. All communication is multiplexed over the existing stdio connection with `sessionId` as the demuxing key.

### Subagent Tool Calls

The main agent uses two tool names to spawn subagents: `"subagent"` and `"agent_crew"`. Both are treated identically by the TUI.

**Tool input:**
```json
{
  "mode": "blocking",
  "task": "Review code changes between current branch and main",
  "stages": [
    {
      "name": "code-reviewer",
      "role": "code-reviewer",
      "prompt_template": "Review the code changes in this PR against main..."
    },
    {
      "name": "pr-test-analyzer",
      "role": "pr-test-analyzer",
      "prompt_template": "Analyze test coverage quality..."
    }
  ]
}
```

**Fields:**
- `mode` — `"blocking"`: parent agent blocks until all stages complete
- `task` — human-readable description of what the crew is doing
- `stages[]` — array of subagent definitions:
  - `name` — unique identifier for this stage
  - `role` — agent mode to use (matches mode IDs from `session/new`)
  - `prompt_template` — the prompt sent to the subagent

The initial `tool_call_chunk` for these tools has `kind: "other"` and `title: "subagent"` (or `"agent_crew"`). The TUI displays them as "Orchestrating"/"Orchestrated".

### `kiro.dev/subagent/list_update` (server → client, notification)

Snapshot notification sent whenever the subagent set changes. Each notification contains the **complete** current state (not a delta). Sent immediately after session creation (with empty arrays) and on every subagent state change.

```json
{
  "method": "_kiro.dev/subagent/list_update",
  "params": {
    "subagents": [
      {
        "sessionId": "b49d53d1-a42a-4ef6-a173-a6224e8e6fcd",
        "sessionName": "code-reviewer",
        "agentName": "code-reviewer",
        "initialQuery": "Review the code changes in this PR...",
        "status": {
          "type": "working",
          "message": "Running"
        },
        "group": "crew-Review code changes ",
        "role": "code-reviewer",
        "dependsOn": []
      }
    ],
    "pendingStages": [
      {
        "name": "summary-writer",
        "agentName": "summary-writer",
        "group": "crew-Review code changes ",
        "role": "summary-writer",
        "dependsOn": ["code-reviewer", "pr-test-analyzer"]
      }
    ]
  }
}
```

**`subagents[]`** — currently running sessions:
- `sessionId` — unique session identifier (used as demuxing key for `session/update`)
- `sessionName` — display name
- `agentName` — agent mode name
- `initialQuery` — the prompt that was sent to this subagent
- `status.type` — `"working"` or `"terminated"`
- `status.message` — human-readable status (e.g., `"Running"`)
- `group` — groups stages from the same `subagent` tool call (e.g., `"crew-Review code changes "`)
- `role` — agent role/mode
- `dependsOn` — stage names this subagent depends on (empty = can run immediately)

**`pendingStages[]`** — stages not yet spawned (waiting on dependencies):
- `name` — stage identifier
- `agentName` — agent mode to use when spawned
- `group`, `role`, `dependsOn` — same semantics as `subagents[]`

**Lifecycle:** When a subagent disappears from `subagents[]` (was present, now absent), it has terminated. The `pendingStages` array shrinks as stages get spawned into `subagents[]`.

### `kiro.dev/session/inbox_notification` (server → client, notification)

Sent when subagents complete and post results back to the parent session.

```json
{
  "method": "_kiro.dev/session/inbox_notification",
  "params": {
    "sessionId": "874046d5-c7ab-47a7-86c5-b15cece1379a",
    "sessionName": "main",
    "messageCount": 1,
    "escalationCount": 0,
    "senders": ["subagent"]
  }
}
```

**Fields:**
- `sessionId` — the target session receiving messages (typically the main session)
- `sessionName` — display name of the target session
- `messageCount` — total pending messages (increments as subagents finish)
- `escalationCount` — number of escalated messages (requiring user attention)
- `senders` — who sent the messages (e.g., `["subagent"]`)

### `kiro.dev/session/list_update` (server → client, notification)

Sent when the session list changes. Contains all active sessions.

```json
{
  "method": "_kiro.dev/session/list_update",
  "params": {
    "sessions": [...]
  }
}
```

### `kiro.dev/session/activity` (server → client, notification)

Per-session activity events.

```json
{
  "method": "_kiro.dev/session/activity",
  "params": {
    "sessionId": "b49d53d1-...",
    "event": { ... }
  }
}
```

### Subagent Session Updates

Each subagent streams via the standard `session/update` notification with its own `sessionId`. The same update types apply — `agent_message_chunk`, `tool_call`, `tool_call_update`:

```json
{
  "method": "session/update",
  "params": {
    "sessionId": "b49d53d1-a42a-4ef6-a173-a6224e8e6fcd",
    "update": {
      "sessionUpdate": "agent_message_chunk",
      "content": { "type": "text", "text": "I'll start by reading the steering files..." }
    }
  }
}
```

Subagent tool calls also flow through `_kiro.dev/session/update` with `sessionUpdate: "tool_call_chunk"`, carrying the subagent's `sessionId`.

### Subagent Session Spawning Flow

1. Main agent streams a `subagent` (or `agent_crew`) tool use
2. Kiro creates a session for each stage — log: `Orchestrated session spawning session_id=... name=... agent=...`
3. `_kiro.dev/subagent/list_update` sent with new subagent in `subagents[]`
4. Each subagent starts streaming its own `session/update` notifications
5. As subagents complete, `list_update` arrives with them removed from `subagents[]`
6. `_kiro.dev/session/inbox_notification` sent to the parent session

---

### `kiro.dev/mcp/oauth_request` (server → client, notification)

Provides an OAuth URL when an MCP server requires authentication. Documented in the [Kiro ACP docs](https://kiro.dev/docs/cli/acp/#kiro-extensions) but not observed in production logs. Exact payload format unknown.

### `kiro.dev/mcp/server_initialized` (server → client, notification)

Indicates an MCP server has finished initializing and its tools are available. Documented in the [Kiro ACP docs](https://kiro.dev/docs/cli/acp/#kiro-extensions) but not observed in production logs. Exact payload format unknown.

### `session/terminate` (server → client, notification)

Terminates a session. In v1.29.0, used for subagent session cleanup. Related to the multi-session methods (`session/spawn`, `session/terminate`, `session/attach`) documented under Connection Lifecycle above.

---

## Unstable ACP Features

These exist in the `agent-client-protocol-schema` crate behind feature flags but are **not advertised** by Kiro v1.29.0 in `sessionCapabilities`:

| Feature Flag | Method | Status in v1.29.0 |
|---|---|---|
| `unstable_session_fork` | `session/fork` | Not advertised. Subagents use `session/spawn` instead. |
| `unstable_session_resume` | `session/resume` | Not advertised. |
| `unstable_session_list` | `session/list` | Available via Kiro extension (not standard ACP). |
| `unstable_session_model` | `session/set_model` | Not advertised. Use `kiro.dev/commands/execute`. |
| `unstable_session_usage` | `UsageUpdate` notification | **Not sent by Kiro v1.29.0.** Tested by enabling `features = ["unstable_session_usage"]` — the variant compiles but Kiro never sends it. Token counts (`inputTokens`, `outputTokens`, `cachedTokens`) are extracted by the TUI from raw stream `MetadataEvent` internals, not from this variant. Per-turn credit cost is available via `kiro.dev/metadata` (see below). |
| `unstable_session_info_update` | `SessionInfoUpdate` notification | Not advertised. |

Note: While `sessionCapabilities` remains `{}`, Kiro v1.29.0 effectively implements multi-session support through its extension methods (`session/spawn`, `session/terminate`, `session/attach`, `session/list`, `message/send`, `kiro.dev/subagent/list_update`). These bypass the standard ACP capability negotiation.

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

## Full Extension Method Reference (v1.29.0)

| Method (wire format) | Direction | Type | Description |
|---|---|---|---|
| `_kiro.dev/commands/available` | server → client | notification | Full command, tool, and MCP server list |
| `_kiro.dev/commands/options` | client → server | request | Query options for selection commands |
| `_kiro.dev/commands/execute` | client → server | request | Execute a slash command |
| `_kiro.dev/metadata` | server → client | notification | Context usage and metering data |
| `_kiro.dev/session/update` | server → client | notification | Kiro-specific session updates (tool_call_chunk) |
| `_kiro.dev/agent/switched` | server → client | notification | Agent mode changed |
| `_kiro.dev/compaction/status` | server → client | notification | Compaction progress |
| `_kiro.dev/clear/status` | server → client | notification | Clear session progress |
| `_kiro.dev/error/rate_limit` | server → client | notification | Rate limit hit |
| `_kiro.dev/mcp/oauth_request` | server → client | notification | MCP OAuth URL |
| `_kiro.dev/mcp/server_init_failure` | server → client | notification | MCP server init failure |
| `_kiro.dev/subagent/list_update` | server → client | notification | Subagent state snapshot |
| `_kiro.dev/session/inbox_notification` | server → client | notification | Subagent completion messages |
| `_kiro.dev/session/list_update` | server → client | notification | Session list changes |
| `_kiro.dev/session/activity` | server → client | notification | Per-session activity events |
| `session/spawn` | client → server | request | Spawn a child session |
| `session/terminate` | client → server | request | Terminate a session |
| `session/attach` | client → server | request | Attach to a session |
| `session/list` | client → server | request | List all sessions |
| `message/send` | client → server | request | Send message to a session |

## Version History

| Date | Kiro Version | ACP Schema | Notes |
|------|-------------|------------|-------|
| 2026-04-02 | v1.29.0 | v0.10.8+ | Subagent support (`subagent`/`agent_crew` tools, `subagent/list_update`, `session/inbox_notification`). Multi-session methods (`session/spawn`, `session/terminate`, `session/attach`, `message/send`, `session/list`). `session/new` response now includes `models` field. `mcpCapabilities.http` now `true`. New: `kiro.dev/error/rate_limit`, `kiro.dev/session/activity`, `kiro.dev/session/list_update`. Compaction status gained structured `status.type` field. Turn metering data available in metadata events. Prompts array in `commands/available` now includes `arguments` field on all prompts (currently only populated for MCP prompts). |
| 2026-03-20 | v1.28.0 | v0.10.8 | Initial investigation. Discovered TuiCommand format, broken set_config_option, agent/switched and tool_call_chunk notifications. |
