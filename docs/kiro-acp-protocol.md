# Kiro CLI ACP Protocol Reference

> **Wire-shape baseline: Kiro CLI 2.0.1** — extracted from `kiro-tui-2.0.1.js`, the bundled TUI source. Every field, method, and type definition in Sections 1–10 cites the line number of the Zod schema that defines it. Every claim can be verified by reading that line in the bundle.
>
> **Behavior updates since 2.0.1:** see [§ 11 — Changes since 2.0.1](#11-changes-since-201) for additions and corrections discovered through Kiro 2.4.1.
>
> **Last empirically verified:** Kiro CLI 2.4.1 (2026-05-21). Verification artifacts:
> - `experiments/conductor-spike/conductor-2.4.1*.log` — JSON-RPC frames from cyril ↔ kiro-cli-chat sessions
> - `experiments/conductor-spike/trace-2.4.1-tui-recorder.jsonl` — TUI-side frames via `KIRO_ACP_RECORD_PATH` (built-in recorder added in 2.4.0)
>
> **Reproducibility:** re-running the test_bridge harness against a newer kiro-cli binary regenerates the captures. To re-verify after a Kiro release, compare new captures against this document section-by-section; any divergence indicates the doc needs a § 11 entry.

---

## Table of Contents

1. [Meta](#1-meta)
2. [Connection lifecycle](#2-connection-lifecycle)
3. [Session lifecycle (C→S)](#3-session-lifecycle-cs)
4. [Client-side callbacks (S→C)](#4-client-side-callbacks-sc)
5. [`session/update` notification variants](#5-sessionupdate-notification-variants)
6. [Kiro extension notifications (S→C)](#6-kiro-extension-notifications-sc)
7. [Kiro extension requests (C→S)](#7-kiro-extension-requests-cs)
8. [Shared types](#8-shared-types)
9. [Kiro command effects — `commands/execute` subcommands](#9-kiro-command-effects)
10. [Appendix — method catalogs and union roots](#10-appendix)
11. [Changes since 2.0.1](#11-changes-since-201)

---

## 1. Meta

### Transport

- **Protocol:** JSON-RPC 2.0 over stdio, newline-delimited.
- **Spawn command:** `kiro-cli acp` (Linux/macOS). On Windows: `wsl kiro-cli acp`.
- **SDK:** `@agentclientprotocol/sdk@0.5.1`, bundled from `node_modules/.bun/@agentclientprotocol+sdk@0.5.1/node_modules/@agentclientprotocol/sdk/dist/schema.js` (tui.js:122972).
- **Protocol version:** `1` (integer, not a date string). Defined as `var PROTOCOL_VERSION = 1` at tui.js:122994. Clients send `protocolVersion: 1` in `initialize`.
- **Logging (server-side):** Set `KIRO_LOG_LEVEL=debug`. Default log path: `$XDG_RUNTIME_DIR/kiro-log/kiro-chat.log` (Linux) or `$TMPDIR/kiro-log/kiro-chat.log` (macOS). Override with `KIRO_CHAT_LOG_FILE`.

### Extension-prefix convention

Kiro extensions use a `_` prefix on the wire. Examples: `_kiro.dev/commands/execute`, `_session/spawn`, `_message/send`.

The SDK strips the leading `_` when dispatching extensions. See `ClientSideConnection.extMethod()` at tui.js:124039:

```js
async extMethod(method, params) {
  return await this.#connection.sendRequest(`_${method}`, params);
}
```

Handlers receive the bare name (e.g. `kiro.dev/commands/execute`), while the wire carries the underscored name.

### Authoritative method catalogs

The SDK defines two constant tables that enumerate all standard ACP methods:

**`AGENT_METHODS`** — methods the agent handles (C→S). tui.js:122973:
```ts
{
  authenticate:       "authenticate",
  initialize:         "initialize",
  session_cancel:     "session/cancel",   // notification
  session_load:       "session/load",
  session_new:        "session/new",
  session_prompt:     "session/prompt",
  session_set_mode:   "session/set_mode",
  session_set_model:  "session/set_model",
}
```

**`CLIENT_METHODS`** — methods the client handles (S→C). tui.js:122983:
```ts
{
  fs_read_text_file:           "fs/read_text_file",
  fs_write_text_file:          "fs/write_text_file",
  session_request_permission:  "session/request_permission",
  session_update:              "session/update",           // notification
  terminal_create:             "terminal/create",
  terminal_kill:               "terminal/kill",
  terminal_output:             "terminal/output",
  terminal_release:            "terminal/release",
  terminal_wait_for_exit:      "terminal/wait_for_exit",
}
```

Any method **not** in these tables is either a Kiro extension (prefixed with `_`) or a plain-named extension routed through the same `extMethod()` dispatcher — see Sections 6–7.

### The `_meta` field

Every request, response, and notification schema in the SDK accepts an optional `_meta: Record<string, unknown>` field at the top level. It is used by both sides for out-of-band metadata (for example, Kiro populates `_meta.welcomeMessage` on entries of `SessionMode` — see Section 3, "Supporting types"). For brevity, `_meta` is omitted from the type signatures in this document unless Kiro populates it with a documented field.

### Union discriminator roots

The SDK composes every valid message as a union of concrete schemas. These roots are useful when searching the bundle:

| Union | Line | Composes |
|---|---|---|
| `agentRequestSchema` | 123435 | All requests the agent can *send* to the client |
| `agentNotificationSchema` | 123685 | All notifications the agent can *send* to the client |
| `agentResponseSchema` | 123703 | All responses the agent can *send* to the client |
| `clientRequestSchema` | 123689 | All requests the client can *send* to the agent |
| `clientNotificationSchema` | 123369 | All notifications the client can *send* to the agent |
| `clientResponseSchema` | 123674 | All responses the client can *send* to the agent |

See Section 10 for the full expansion of each union.

---

## 2. Connection lifecycle

### `initialize` — C→S request

First method on any connection. Exchanges protocol versions, capabilities, and identifying info. The client sends this immediately after stdio is established; no other method is valid until `initialize` has succeeded.

**Request** (`initializeRequestSchema`, tui.js:123668):

```ts
type InitializeRequest = {
  protocolVersion: number;
  clientCapabilities?: ClientCapabilities;
  clientInfo?: Implementation | null;
}
```

**Response** (`initializeResponseSchema`, tui.js:123468):

```ts
type InitializeResponse = {
  protocolVersion: number;
  agentInfo?: Implementation | null;
  agentCapabilities?: AgentCapabilities;
  authMethods?: AuthMethod[];
}
```

**Supporting types** used only by this method:

```ts
type Implementation = {              // tui.js:123243
  name: string;
  version: string;
  title?: string | null;
}

type ClientCapabilities = {          // tui.js:123458
  fs?: FileSystemCapability;
  terminal?: boolean;
}

type FileSystemCapability = {        // tui.js:123298
  readTextFile?: boolean;
  writeTextFile?: boolean;
}

type AgentCapabilities = {           // tui.js:123446
  loadSession?: boolean;
  mcpCapabilities?: McpCapabilities;
  promptCapabilities?: PromptCapabilities;
}

type McpCapabilities = {             // tui.js:123254
  http?: boolean;
  sse?: boolean;
}

type PromptCapabilities = {          // tui.js:123259
  audio?: boolean;
  embeddedContext?: boolean;
  image?: boolean;
}

type AuthMethod = {                  // tui.js:123248
  id: string;
  name: string;
  description?: string | null;
}
```

**Example (wire):**

```json
// ─── Request
{
  "jsonrpc": "2.0",
  "id": 0,
  "method": "initialize",
  "params": {
    "protocolVersion": 1,
    "clientCapabilities": {
      "fs": { "readTextFile": true, "writeTextFile": true },
      "terminal": true
    },
    "clientInfo": {
      "name": "cyril",
      "version": "0.1.0",
      "title": "Cyril TUI"
    }
  }
}

// ─── Response
{
  "jsonrpc": "2.0",
  "id": 0,
  "result": {
    "protocolVersion": 1,
    "agentInfo": {
      "name": "Kiro CLI Agent",
      "version": "2.0.1"
    },
    "agentCapabilities": {
      "loadSession": true,
      "mcpCapabilities": { "http": true, "sse": false },
      "promptCapabilities": {
        "audio": false,
        "embeddedContext": false,
        "image": true
      }
    },
    "authMethods": []
  }
}
```

**Notes:**

- **No `sessionCapabilities` field.** The v2.0.1 `agentCapabilities` schema contains only `loadSession`, `mcpCapabilities`, and `promptCapabilities`. Multi-session support (`session/spawn`, `session/attach`, etc.) is advertised implicitly by the presence of Kiro extension methods, not through capability negotiation — see Section 7.
- **Client capabilities are advisory.** `clientCapabilities.fs.*` and `clientCapabilities.terminal` declare that the client can service `fs/*` and `terminal/*` callbacks, but Kiro's built-in tools (`read`, `write`, `shell`, etc.) perform file and shell I/O inside the agent process. In normal operation the callbacks in Section 4 are never invoked. Clients may still advertise the capability for forward compatibility.
- **`authMethods`** is typically `[]`. Kiro authenticates out-of-band via `kiro-cli login` (AWS Builder ID / IAM Identity Center), and `authenticate` is rarely driven by ACP.

### `authenticate` — C→S request

Completes an authentication challenge named by one of the `authMethods` returned from `initialize`. As noted above, this is rarely reached in a normal Kiro deployment.

**Request** (`authenticateRequestSchema`, tui.js:123096):

```ts
type AuthenticateRequest = {
  methodId: string;  // must match an AuthMethod.id from initialize
}
```

**Response** (`authenticateResponseSchema`, tui.js:123068):

```ts
type AuthenticateResponse = {
  // empty — _meta only
}
```

**Example (wire):**

```json
// ─── Request
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "authenticate",
  "params": { "methodId": "sso" }
}

// ─── Response
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {}
}
```

**Notes:**

- The response body is effectively empty (only `_meta` is declared). Success is signaled by absence of a JSON-RPC `error` object.
- If `methodId` is not recognized, the agent returns a standard JSON-RPC error (code/message shape — see `errorSchema` at tui.js:123063).

---

## 3. Session lifecycle (C→S)

All methods in this section target an existing session identified by `sessionId`, except `session/new` which creates one.

### `session/new` — C→S request

Creates a conversation session bound to a working directory and an optional list of MCP servers.

**Request** (`newSessionRequestSchema`, tui.js:123413):

```ts
type NewSessionRequest = {
  cwd: string;
  mcpServers: McpServer[];
}
```

**Response** (`newSessionResponseSchema`, tui.js:123398):

```ts
type NewSessionResponse = {
  sessionId: string;
  modes?: SessionModeState | null;
  models?: SessionModelState | null;
}
```

**Notes:**

- The response carries `modes` and `models` state — the full set of agent modes and models available for this session, plus which one is currently active. **This is the only place the client receives this data**; it is not re-sent as a notification. The client must capture it here (and in `session/load`).
- `NewSessionRequest` has **only** `cwd` and `mcpServers` in v2.0.1's 0.5.1 SDK. No `configOptions` field exists on the request, and no `sessionCapabilities` field on the response — earlier protocol docs that showed these reflect older SDK versions.

**Example:**

```json
// ─── Request
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "session/new",
  "params": {
    "cwd": "/home/user/project",
    "mcpServers": []
  }
}

// ─── Response
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "sessionId": "4dfac9d3-2a7b-4dda-8f7c-13900cc29028",
    "modes": {
      "currentModeId": "kiro_default",
      "availableModes": [
        { "id": "kiro_default", "name": "kiro_default", "description": "The default agent..." },
        {
          "id": "kiro_planner",
          "name": "kiro_planner",
          "description": "Specialized planning agent...",
          "_meta": { "welcomeMessage": "Transform any idea into fully working code." }
        }
      ]
    },
    "models": {
      "currentModelId": "auto",
      "availableModels": [
        { "modelId": "auto", "name": "auto", "description": "Models chosen by task..." },
        { "modelId": "claude-opus-4.6", "name": "claude-opus-4.6", "description": "Latest Claude Opus..." }
      ]
    }
  }
}
```

### `session/load` — C→S request

Resumes a previously created session by ID. Requires `agentCapabilities.loadSession: true` (see Section 2).

**Request** (`loadSessionRequestSchema`, tui.js:123418):

```ts
type LoadSessionRequest = {
  sessionId: string;
  cwd: string;
  mcpServers: McpServer[];
}
```

**Response** (`loadSessionResponseSchema`, tui.js:123404):

```ts
type LoadSessionResponse = {
  modes?: SessionModeState | null;
  models?: SessionModelState | null;
}
```

**Notes:**

- `LoadSessionResponse` **does not echo `sessionId`** — the client already has it from the request. Otherwise the payload is the same shape as `NewSessionResponse`.
- The agent replays buffered session state as `session/update` notifications after the response returns, so a client loading an existing session should be prepared to receive updates before the next user prompt.

**Example:**

```json
// ─── Request
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "session/load",
  "params": {
    "sessionId": "4dfac9d3-2a7b-4dda-8f7c-13900cc29028",
    "cwd": "/home/user/project",
    "mcpServers": []
  }
}

// ─── Response
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "modes":  { "currentModeId": "kiro_default", "availableModes": [ /* ... */ ] },
    "models": { "currentModelId": "auto",         "availableModels": [ /* ... */ ] }
  }
}
```

### `session/prompt` — C→S request

Sends a user message to the agent and starts a **turn**. The agent streams its work via `session/update` notifications (Section 5) and returns the `session/prompt` response only when the turn ends.

**Request** (`promptRequestSchema`, tui.js:123424):

```ts
type PromptRequest = {
  sessionId: string;
  prompt: ContentBlock[];   // see Section 8 for ContentBlock shape
}
```

**Response** (`promptResponseSchema`, tui.js:123074):

```ts
type PromptResponse = {
  stopReason:
    | "end_turn"            // agent finished normally
    | "max_tokens"          // hit the model's token budget
    | "max_turn_requests"   // exceeded configured per-turn request cap
    | "refusal"             // model refused to continue
    | "cancelled";          // client-side cancel via session/cancel
}
```

**Notes:**

- `stopReason` has **5 values** in the 0.5.1 SDK schema. Older docs that enumerate only `end_turn`/`max_tokens`/`cancelled` are incomplete — `max_turn_requests` and `refusal` are first-class.
- Turn boundary is strictly between the request and the response. While the request is outstanding, `session/update` notifications for this `sessionId` belong to this turn.
- Multiple turns can be multiplexed across different `sessionId`s over the same connection (see Section 7 for subagent sessions).

**Example:**

```json
// ─── Request
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "session/prompt",
  "params": {
    "sessionId": "4dfac9d3-...",
    "prompt": [{ "type": "text", "text": "Explain main.rs" }]
  }
}

// ─── (many session/update notifications stream here — Section 5)

// ─── Response
{
  "jsonrpc": "2.0",
  "id": 4,
  "result": { "stopReason": "end_turn" }
}
```

### `session/cancel` — C→S notification

Fire-and-forget cancellation for an in-progress turn. **No response is expected or sent** — JSON-RPC notifications have no `id`.

**Schema** (`cancelNotificationSchema`, tui.js:123161):

```ts
type CancelNotification = {
  sessionId: string;
}
```

**Notes:**

- When the agent honors the cancel, the outstanding `session/prompt` response returns with `stopReason: "cancelled"`.
- Cancel is not guaranteed to be instant; the agent finishes the current step (tool call, model chunk) before acknowledging.

**Example:**

```json
{
  "jsonrpc": "2.0",
  "method": "session/cancel",
  "params": { "sessionId": "4dfac9d3-..." }
}
```

### `session/set_mode` — C→S request

Switches the active agent mode for the session. Valid `modeId` values come from `NewSessionResponse.modes.availableModes[].id`.

**Request** (`setSessionModeRequestSchema`, tui.js:123100):

```ts
type SetSessionModeRequest = {
  sessionId: string;
  modeId: string;
}
```

**Response** (`setSessionModeResponseSchema`, tui.js:123071):

```ts
type SetSessionModeResponse = {
  // empty — _meta only
}
```

**Notes:**

- After a successful mode change, Kiro sends a `_kiro.dev/agent/switched` notification (Section 6) carrying the old and new agent name plus an optional welcome message.
- The standard ACP `current_mode_update` `session/update` variant (Section 5) may also fire.

**Example:**

```json
// ─── Request
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "session/set_mode",
  "params": { "sessionId": "4dfac9d3-...", "modeId": "kiro_planner" }
}

// ─── Response
{ "jsonrpc": "2.0", "id": 5, "result": {} }
```

### `session/set_model` — C→S request

Switches the active model for the session. Valid `modelId` values come from `NewSessionResponse.models.availableModels[].modelId`.

**Request** (`setSessionModelRequestSchema`, tui.js:123105):

```ts
type SetSessionModelRequest = {
  sessionId: string;
  modelId: string;
}
```

**Response** (`setSessionModelResponseSchema`, tui.js:123084):

```ts
type SetSessionModelResponse = {
  // empty — _meta only
}
```

**Notes:**

- **First-class in v2.0.1.** This method appears in `AGENT_METHODS` (tui.js:122981) with a dedicated sender `setSessionModel()` in `ClientSideConnection` (tui.js:124027). No capability flag gates it. Documentation that describes it as "unstable, not advertised" is obsolete.
- Model changes also propagate via `session/update` (possibly via a `config_option_update` variant — see Section 5) and via `_kiro.dev/metadata` (Section 6).

**Example:**

```json
// ─── Request
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "session/set_model",
  "params": { "sessionId": "4dfac9d3-...", "modelId": "claude-opus-4.6" }
}

// ─── Response
{ "jsonrpc": "2.0", "id": 6, "result": {} }
```

### Supporting types (used in this section)

```ts
type McpServer =                        // tui.js:123309 (3-way union)
  | { type: "http"; name: string; url: string; headers: HttpHeader[] }
  | { type: "sse";  name: string; url: string; headers: HttpHeader[] }
  | StdioMcpServer;

type StdioMcpServer = {                 // tui.js:123303
  name: string;
  command: string;
  args: string[];
  env: EnvVariable[];
  // NOTE: no `type` discriminator on the wire — this is the fall-through case.
  // Recipients distinguish stdio from http/sse by absence of `type`.
}

type HttpHeader = {                     // tui.js:123111
  name: string;
  value: string;
}

type EnvVariable = {                    // tui.js:123238
  name: string;
  value: string;
}

type SessionMode = {                    // tui.js:123271
  id: string;
  name: string;
  description?: string | null;
  // Kiro populates _meta.welcomeMessage on some modes (observed on kiro_planner)
}

type SessionModeState = {               // tui.js:123282
  currentModeId: string;
  availableModes: SessionMode[];
}

type ModelInfo = {                      // tui.js:123265
  modelId: string;
  name: string;
  description?: string | null;
}

type SessionModelState = {              // tui.js:123277
  currentModelId: string;
  availableModels: ModelInfo[];
}
```

## 4. Client-side callbacks (S→C)

These methods are sent **from the agent to the client**. They are defined in `CLIENT_METHODS` (tui.js:122983) and dispatched in `ClientSideConnection`'s request handler (tui.js:123950–123994).

**Practical note on what Kiro actually uses.** Kiro's built-in tools (`read`, `write`, `shell`, `ls`, `glob`, `grep`, etc.) perform file and shell I/O inside the agent process. **In normal Kiro operation, `fs/*` and `terminal/*` callbacks are never invoked** — the agent does not delegate these to the client. The one client-side callback that is actively used is `session/request_permission`, which Kiro sends before executing tools marked as requiring permission (notably shell commands). All other methods in this section are defined in the 0.5.1 SDK schema and implementable for forward compatibility, but won't be exercised by current Kiro releases.

### `session/request_permission` — S→C request

The agent asks the client to confirm (or reject) a pending tool call. The agent pauses the turn until the client responds.

**Request** (`requestPermissionRequestSchema`, tui.js:123373):

```ts
type RequestPermissionRequest = {
  sessionId: string;
  options: PermissionOption[];
  toolCall: {
    toolCallId: string;
    title?: string | null;
    kind?: ToolKind | null;                // see Section 8
    status?: ToolCallStatus | null;        // see Section 8
    content?: ToolCallContent[] | null;    // see Section 8
    locations?: ToolCallLocation[] | null; // see Section 8
    rawInput?: Record<string, unknown>;
    rawOutput?: Record<string, unknown>;
  };
}
```

**Response** (`requestPermissionResponseSchema`, tui.js:123133):

```ts
type RequestPermissionResponse = {
  outcome:
    | { outcome: "cancelled" }
    | { outcome: "selected"; optionId: string };
}
```

**Supporting type:**

```ts
type PermissionOption = {                  // tui.js:123166
  optionId: string;                        // value sent back in the "selected" outcome
  name: string;                            // display label
  kind:
    | "allow_once"
    | "allow_always"
    | "reject_once"
    | "reject_always";
}
```

**Notes:**

- **Nested discriminated union on `outcome`.** The `outcome` field is itself an object with an `outcome` discriminator — two nested `outcome` keys. Do not flatten when deserializing; `response.outcome.outcome` is the literal tag.
- **`kind` has 4 values.** `reject_always` is present in the 0.5.1 SDK even though Kiro typically only sends `allow_once`/`allow_always`/`reject_once` in its permission prompts.
- `optionId` is the authoritative value the client sends back. `name` is a display label only — clients must not use name-matching to determine intent; rely on `kind` or `optionId`.
- `rawInput` is unrestricted; for shell tools Kiro populates it with `{ command: "..." }`.
- **Kiro's current policy:** shell commands require permission, file reads do not. An `allow_always` selection is remembered for the session.

**Example (shell command):**

```json
// ─── Request
{
  "jsonrpc": "2.0", "id": 10, "method": "session/request_permission",
  "params": {
    "sessionId": "4dfac9d3-...",
    "toolCall": {
      "toolCallId": "tc_002",
      "title": "Run `npm test`",
      "kind": "execute",
      "status": "pending",
      "rawInput": { "command": "npm test" }
    },
    "options": [
      { "optionId": "allow_once",   "name": "Yes",    "kind": "allow_once"   },
      { "optionId": "allow_always", "name": "Always", "kind": "allow_always" },
      { "optionId": "reject_once",  "name": "No",     "kind": "reject_once"  }
    ]
  }
}

// ─── Response (user approves once)
{
  "jsonrpc": "2.0", "id": 10,
  "result": { "outcome": { "outcome": "selected", "optionId": "allow_once" } }
}

// ─── Response (user cancelled the picker)
{
  "jsonrpc": "2.0", "id": 10,
  "result": { "outcome": { "outcome": "cancelled" } }
}
```

### `fs/read_text_file` — S→C request

Not invoked by current Kiro releases. Defined in the SDK schema for forward compatibility.

**Request** (`readTextFileRequestSchema`, tui.js:123004):

```ts
type ReadTextFileRequest = {
  sessionId: string;
  path: string;
  line?: number | null;    // 1-based starting line (partial read)
  limit?: number | null;   // max lines to read from `line` (SDK does not specify units beyond "number")
}
```

**Response** (`readTextFileResponseSchema`, tui.js:123129):

```ts
type ReadTextFileResponse = {
  content: string;
}
```

**Notes:**

- The `line`/`limit` pair supports **partial reads** — an agent can request lines 100–150 of a large file without streaming the whole thing. Clients that advertise `clientCapabilities.fs.readTextFile: true` must handle both `null` (read all) and numeric values.

### `fs/write_text_file` — S→C request

Not invoked by current Kiro releases.

**Request** (`writeTextFileRequestSchema`, tui.js:122998):

```ts
type WriteTextFileRequest = {
  sessionId: string;
  path: string;
  content: string;
}
```

**Response** (`writeTextFileResponseSchema`, tui.js:123126):

```ts
type WriteTextFileResponse = {
  // empty — _meta only
}
```

### `terminal/create` — S→C request

Creates a pseudo-terminal on the client side, for the agent to execute commands in. Not invoked by current Kiro releases.

**Request** (`createTerminalRequestSchema`, tui.js:123389):

```ts
type CreateTerminalRequest = {
  sessionId: string;
  command: string;
  args?: string[];
  cwd?: string | null;
  env?: EnvVariable[];
  outputByteLimit?: number | null;
}
```

**Response** (`createTerminalResponseSchema`, tui.js:123145):

```ts
type CreateTerminalResponse = {
  terminalId: string;   // opaque handle used by subsequent terminal/* methods
}
```

The client returns a `terminalId`; the agent then drives the terminal's lifecycle via `terminal/output`, `terminal/wait_for_exit`, `terminal/kill`, `terminal/release`.

### `terminal/output` — S→C request

**Request** (`terminalOutputRequestSchema`, tui.js:123011):

```ts
type TerminalOutputRequest = {
  sessionId: string;
  terminalId: string;
}
```

**Response** (`terminalOutputResponseSchema`, tui.js:123429):

```ts
type TerminalOutputResponse = {
  output: string;
  truncated: boolean;
  exitStatus?: TerminalExitStatus | null;
}
```

Returns the accumulated terminal output and, if the terminal has exited, its nested `TerminalExitStatus`. `truncated: true` indicates the `outputByteLimit` cap from `terminal/create` was hit.

### `terminal/wait_for_exit` — S→C request

**Request** (`waitForTerminalExitRequestSchema`, tui.js:123021) — same shape as `terminal/output`:

```ts
type WaitForTerminalExitRequest = {
  sessionId: string;
  terminalId: string;
}
```

**Response** (`waitForTerminalExitResponseSchema`, tui.js:123152):

```ts
type WaitForTerminalExitResponse = {
  exitCode?: number | null;
  signal?: string | null;
}
```

**Note:** this response has `exitCode`/`signal` **inlined at the top level**, not nested inside a `TerminalExitStatus` object the way `terminal/output` does. The two shapes are structurally equivalent but wire-format different — a client handler must not share a deserializer between them.

### `terminal/kill` — S→C request

Request has same shape as `terminal/output` (`killTerminalCommandRequestSchema`, tui.js:123026): `{ sessionId, terminalId }`.

Response (`killTerminalResponseSchema`, tui.js:123157) is empty (`_meta` only).

### `terminal/release` — S→C request

Releases the client-side handle so its resources can be reclaimed. Request has same shape as `terminal/output` (`releaseTerminalRequestSchema`, tui.js:123016). Response (`releaseTerminalResponseSchema`, tui.js:123149) is empty.

### Supporting types (used in this section)

```ts
type PermissionOption = {              // tui.js:123166
  optionId: string;
  name: string;
  kind: "allow_once" | "allow_always" | "reject_once" | "reject_always";
}

type TerminalExitStatus = {            // tui.js:123364
  exitCode?: number | null;
  signal?: string | null;
}

// EnvVariable is defined in Section 3 (tui.js:123238).
// ToolKind, ToolCallStatus, ToolCallContent, ToolCallLocation are defined in Section 8.
```

## 5. `session/update` notification variants

`session/update` is a **server-to-client notification** (no response). It is the primary channel for everything the agent produces during a turn: text, extended thinking, tool calls, plans, and mode/command-list changes. Every payload is discriminated by an inner `sessionUpdate` field.

### Wrapper

`sessionNotificationSchema`, tui.js:123475:

```ts
type SessionNotification = {
  sessionId: string;
  update: SessionUpdate;
}

type SessionUpdate =
  | UserMessageChunk
  | AgentMessageChunk
  | AgentThoughtChunk
  | ToolCall
  | ToolCallUpdate
  | Plan
  | AvailableCommandsUpdate
  | CurrentModeUpdate;
```

**8 variants** in the 0.5.1 SDK schema. The variants are defined inline in one large `exports_external.union([...])` rather than as separate named schemas — grepping for `sessionUpdate:` will locate each one.

### `user_message_chunk` — tui.js:123521

Agent-replayed user message. Most commonly emitted during `session/load` to replay conversation history. Was **not present** in earlier protocol documentation; it is a first-class variant in the 0.5.1 SDK.

```ts
type UserMessageChunk = {
  sessionUpdate: "user_message_chunk";
  content: ContentBlock;     // single block, not an array
}
```

### `agent_message_chunk` — tui.js:123565

Streaming output from the agent. The primary output channel during a turn.

```ts
type AgentMessageChunk = {
  sessionUpdate: "agent_message_chunk";
  content: ContentBlock;     // single block per chunk
}
```

**Example:**

```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "4dfac9d3-...",
    "update": {
      "sessionUpdate": "agent_message_chunk",
      "content": { "type": "text", "text": "Looking at the code..." }
    }
  }
}
```

**Note on arrays vs singles:** chunk variants carry a *single* `content` block per notification, not an array. This differs from `session/prompt` (Section 3), which sends `prompt: ContentBlock[]` as a single bundled message.

### `agent_thought_chunk` — tui.js:123609

Extended-thinking content. Structurally identical to `agent_message_chunk`. Clients typically render this in a collapsed or distinct style.

```ts
type AgentThoughtChunk = {
  sessionUpdate: "agent_thought_chunk";
  content: ContentBlock;
}
```

### `tool_call` — tui.js:123629

**Initial** announcement of a tool call. Tool calls are identified by `toolCallId` and can be updated later via `tool_call_update`.

```ts
type ToolCall = {
  sessionUpdate: "tool_call";
  toolCallId: string;
  title: string;                           // REQUIRED on initial tool_call
  kind?:                                   // inline union (not a toolKindSchema reference)
    | "read" | "edit" | "delete" | "move" | "search"
    | "execute" | "think" | "fetch" | "switch_mode" | "other";
  status?: "pending" | "in_progress" | "completed" | "failed";
  content?: ToolCallContent[];             // see Section 8
  locations?: ToolCallLocation[];          // see Section 8
  rawInput?: Record<string, unknown>;      // opaque tool args
  rawOutput?: Record<string, unknown>;     // opaque tool result
}
```

**Notes:**

- **`kind` is inline, not a schema reference.** The 0.5.1 SDK declares the `kind` union directly inside the `tool_call` variant (tui.js:123614–123625) instead of referencing the shared `toolKindSchema` that `tool_call_update` uses. The string values are identical. Consumers can treat them as the same `ToolKind` enum.
- **Optional fields are not nullable here.** Every optional field uses `.optional()` (may be absent) but *not* `.optional().nullable()` — if the field is present it cannot be `null`. This differs from `tool_call_update` below.
- **`title` is required** on the initial `tool_call`. Updates may elide it; see below.

### `tool_call_update` — tui.js:123646

Progress/completion update for an existing tool call. Client matches on `toolCallId` and merges fields that are present.

```ts
type ToolCallUpdate = {
  sessionUpdate: "tool_call_update";
  toolCallId: string;                      // REQUIRED — identifies the tool call to update
  title?: string | null;
  kind?: ToolKind | null;
  status?: ToolCallStatus | null;
  content?: ToolCallContent[] | null;
  locations?: ToolCallLocation[] | null;
  rawInput?: Record<string, unknown>;
  rawOutput?: Record<string, unknown>;
}
```

**Notes:**

- **All fields except `toolCallId` are `optional().nullable()`** (123641–123649). Contrast with `tool_call` (non-nullable optionals).
- **Merge rule:** an update that does not mention a field should leave that field unchanged. Do not overwrite existing state with empty strings or empty arrays just because the update "has" those values — in practice Kiro omits unchanged fields. Treat missing *and* null as "no change."
- **Observed lifecycle:** initial `tool_call` (`status: "in_progress"` or `"pending"`) → zero or more `tool_call_update` carrying intermediate state → final `tool_call_update` with `status: "completed"` or `"failed"` and usually a non-empty `content` (tool result) or `rawOutput`.

### `plan` — tui.js:123651

Multi-step plan from the agent. Sent when the agent establishes or revises a task list.

```ts
type Plan = {
  sessionUpdate: "plan";
  entries: PlanEntry[];
}

type PlanEntry = {                         // tui.js:123287
  content: string;
  priority: "high" | "medium" | "low";
  status: "pending" | "in_progress" | "completed";
}
```

**Note:** each `plan` update **replaces the previous plan entirely**. It is not a delta. Clients should atomically swap in the new `entries` array.

### `available_commands_update` — tui.js:123656

Updated slash-command list. Allows the command menu to refresh mid-session (for example when an MCP server finishes initializing and registers new commands).

```ts
type AvailableCommandsUpdate = {
  sessionUpdate: "available_commands_update";
  availableCommands: AvailableCommand[];
}

type AvailableCommand = {                  // tui.js:123452
  name: string;
  description: string;
  input?: { hint: string } | null;         // unstructuredCommandInputSchema (tui.js:123090)
}
```

**Note:** The SDK's `AvailableCommand` is minimal — `input` is just `{ hint: string }`. Kiro enriches each command with additional metadata (`optionsMethod`, `inputType`, `local`, etc.) via the `_meta` field and via its own `_kiro.dev/commands/available` notification (Section 6). Standard ACP clients that don't handle Kiro extensions will see the bare `name`/`description`/`input` only.

### `current_mode_update` — tui.js:123661

Agent mode changed.

```ts
type CurrentModeUpdate = {
  sessionUpdate: "current_mode_update";
  currentModeId: string;
}
```

**Note:** Kiro also sends a richer `_kiro.dev/agent/switched` notification (Section 6) carrying the previous agent name and optional welcome message. A mode change may produce both notifications; clients should prefer the extension for display data and use `current_mode_update` for the ID.

### Variants NOT in the 0.5.1 SDK

- **`config_option_update`.** Earlier protocol docs referenced it for model/config changes. It is **absent** from `sessionNotificationSchema` in 2.0.1. Model switching now uses `session/set_model` (Section 3) plus `_kiro.dev/metadata` (Section 6); no standard ACP variant carries config-option deltas.

### Realistic turn excerpt

```json
// Tool call announced
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "4dfac9d3-...",
    "update": {
      "sessionUpdate": "tool_call",
      "toolCallId": "tc_100",
      "title": "Reading main.rs",
      "kind": "read",
      "status": "in_progress",
      "rawInput": { "path": "/home/user/main.rs" },
      "locations": [{ "path": "/home/user/main.rs", "line": 1 }]
    }
  }
}

// Tool completes
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "4dfac9d3-...",
    "update": {
      "sessionUpdate": "tool_call_update",
      "toolCallId": "tc_100",
      "status": "completed",
      "content": [
        { "type": "content", "content": { "type": "text", "text": "fn main() { ... }" } }
      ]
    }
  }
}

// Agent explains
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "4dfac9d3-...",
    "update": {
      "sessionUpdate": "agent_message_chunk",
      "content": { "type": "text", "text": "The file contains a simple main function." }
    }
  }
}
```

## 6. Kiro extension notifications (S→C)

Kiro extensions are **not validated** by a Zod schema. The SDK defines `extNotificationSchema = record(unknown)` (tui.js:123089, 123165), so the protocol layer passes raw `params` through to the TUI handler. The shapes below are **reconstructed from property accesses in the handler functions** — they document what the TUI reads, not what Kiro's agent must or may send. Fields the TUI ignores are not documented here.

**Wire format.** Every notification in this section arrives with an underscore prefix on the wire (e.g. `_kiro.dev/metadata`). The SDK's `extNotification` dispatcher strips the `_` before calling the client's handler (tui.js:123897, 124042), so handler lookups use the bare name (`kiro.dev/...`). This document uses the bare form in headings; the wire form always starts with `_`.

**Method catalog.** All extension notifications are enumerated in the `EXT_METHODS` constant table at tui.js:124335. The dispatch map that routes each method to its handler is `AcpClient.extNotificationHandlers` at tui.js:124719:

```ts
// tui.js:124335
var EXT_METHODS = {
  COMMANDS_AVAILABLE:       "kiro.dev/commands/available",
  METADATA:                 "kiro.dev/metadata",
  COMPACTION_STATUS:        "kiro.dev/compaction/status",
  CLEAR_STATUS:             "kiro.dev/clear/status",
  MCP_SERVER_INIT_FAILURE:  "kiro.dev/mcp/server_init_failure",
  MCP_OAUTH_REQUEST:        "kiro.dev/mcp/oauth_request",
  MCP_SERVER_INITIALIZED:   "kiro.dev/mcp/server_initialized",
  AGENT_NOT_FOUND:          "kiro.dev/agent/not_found",
  AGENT_CONFIG_ERROR:       "kiro.dev/agent/config_error",
  MODEL_NOT_FOUND:          "kiro.dev/model/not_found",
  RATE_LIMIT_ERROR:         "kiro.dev/error/rate_limit",
  SUBAGENT_LIST_UPDATE:     "kiro.dev/subagent/list_update",
  SESSION_ACTIVITY:         "kiro.dev/session/activity",
  SESSION_LIST_UPDATE:      "kiro.dev/session/list_update",
  INBOX_NOTIFICATION:       "kiro.dev/session/inbox_notification",
  AGENT_SWITCHED:           "kiro.dev/agent/switched",
  SESSION_UPDATE:           "kiro.dev/session/update",
  // ... extension requests follow (see Section 7)
};
```

---

### `kiro.dev/commands/available`

Full list of slash commands, prompts, tools, and MCP servers available in the session. Typically sent once after session creation.

Handler: `handleCommandsAdvertising` (tui.js:124738).

```ts
type CommandsAvailableNotification = {
  commands?: KiroCommand[];
  prompts?: Prompt[];
  tools?: Tool[];           // length is read; other fields are opaque to the TUI
  mcpServers?: McpServerStatus[];
}

type KiroCommand = {                   // distinct from SDK's `AvailableCommand` (Section 5)
  name: string;
  description: string;
  meta?: {                             // opaque; rendered but not enforced
    optionsMethod?: string;            // e.g. "_kiro.dev/commands/model/options"
    inputType?: "selection" | "panel"; // governs how the TUI prompts for input
    hint?: string;
    local?: boolean;                   // handled client-side, no backend round-trip
    type?: "prompt";                   // distinguishes prompts from commands
  };
}

type Prompt = {
  name: string;
  description: string;
  serverName: string;                  // "file-prompts" or an MCP server name
  arguments: Array<{ name: string; required: boolean }>;
}

type McpServerStatus = {
  // handler reads .status === "running" to count running servers
  status: "running" | "failed" | "pending" | string;
  // other fields are forwarded opaque to subscribers
}
```

**Notes:**

- The TUI only derives the **count of running MCP servers** and a count of tools for decorative display; it does not enforce any fields on tools or mcpServers.
- `commands[].meta` is read opaquely and attached to the in-UI command record as `meta`. The object values above are what Kiro populates in practice, but neither the TUI nor the SDK validates them.
- **File-based prompts** use `serverName: "file-prompts"` and have `arguments: []`. **MCP prompts** may declare typed arguments.

### `kiro.dev/metadata`

Sent after each turn with session metadata (context usage, credit cost, turn duration). Also sent once at session creation with `contextUsagePercentage` only.

Handler: `handleMetadataUpdate` (tui.js:124766).

```ts
type MetadataNotification = {
  sessionId?: string;                  // ignored unless it matches the current session
  contextUsagePercentage?: number | null;
  meteringUsage?: MeteringEntry[];
  turnDurationMs?: number;
}

type MeteringEntry = {
  value: number;
  unitPlural: string;                  // used as the aggregation key (e.g. "credits")
  unit?: string;                       // singular form; present in Kiro output, not read by TUI
}
```

**Notes:**

- The TUI silently discards the notification if `sessionId` is present and does not match the current (main) session (tui.js:124768). To target a subagent session, Kiro uses the subagent's session id here.
- `meteringUsage` entries are aggregated by `unitPlural`. The TUI does not read `unit` (singular) in 2.0.1, although Kiro still sends it.
- **Token-level usage** (`inputTokens`, `outputTokens`, `cachedTokens`) is not exposed via this notification in 2.0.1. The SDK has no `UsageUpdate` session-update variant either (Section 5). If token counts are needed, they must be derived from lower-level agent events outside the ACP surface.

**Example:**

```json
{
  "jsonrpc": "2.0",
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

### `kiro.dev/compaction/status`

Progress of `/compact` (conversation summarization).

Handler: `handleCompactionStatus` (tui.js:124790).

```ts
type CompactionStatusNotification = {
  status?: {
    type: "started" | "completed" | "failed" | string;
    error?: string;                    // populated on type === "failed"
  };
  summary?: unknown;                   // passed through opaquely
}
```

**Note:** the TUI treats the notification as a no-op if `status` is missing. Only `status.type` and `status.error` are interpreted; `summary` is forwarded as-is.

### `kiro.dev/clear/status`

Sent during `/clear` (session-history wipe). **No fields are read by the TUI** — the handler only logs at debug level (tui.js:124787). Payload shape is unenforced; Kiro may send any object.

### `kiro.dev/mcp/server_initialized`

An MCP server finished initializing; its tools are now available.

Handler: `handleMcpServerInitialized` (tui.js:124823).

```ts
type McpServerInitializedNotification = {
  serverName: string;
}
```

### `kiro.dev/mcp/server_init_failure`

An MCP server failed to initialize.

Handler: `handleMcpServerInitFailure` (tui.js:124803).

```ts
type McpServerInitFailureNotification = {
  serverName: string;
  error: unknown;                      // logged and forwarded, not parsed
}
```

### `kiro.dev/mcp/oauth_request`

An MCP server requires OAuth; Kiro supplies the URL for the user to open.

Handler: `handleMcpOauthRequest` (tui.js:124813).

```ts
type McpOauthRequestNotification = {
  serverName: string;
  oauthUrl: string;
}
```

### `kiro.dev/error/rate_limit`

Agent hit a rate limit; the message is typically rendered as a transient error banner.

Handler: `handleRateLimitError` (tui.js:124831).

```ts
type RateLimitErrorNotification = {
  message: string;
}
```

### `kiro.dev/agent/not_found`

User requested an agent mode that doesn't exist; Kiro fell back to another.

Handler: `handleAgentNotFound` (tui.js:124839).

```ts
type AgentNotFoundNotification = {
  requestedAgent: string;
  fallbackAgent: string;
}
```

### `kiro.dev/agent/config_error`

An agent configuration file failed to parse.

Handler: `handleAgentConfigError` (tui.js:124852).

```ts
type AgentConfigErrorNotification = {
  path: string;                        // the offending config file
  error: unknown;                      // parse error detail; forwarded opaque
}
```

### `kiro.dev/model/not_found` (REMOVED — see § 11.3)

> **⚠ Removed in or before Kiro 2.4.1.** Documented here as 2.0.1 baseline; verified absent from 2.4.1 tui.js bundle (no method string, no handler). Cyril retains a stale handler in `convert/kiro.rs:511`. See § 11.3 for the cross-version status.

User requested a model that doesn't exist; Kiro fell back to another.

Handler: `handleModelNotFound` (tui.js:124862, **2.0.1 only**).

```ts
type ModelNotFoundNotification = {
  requestedModel: string;
  fallbackModel: string;
}
```

### `kiro.dev/agent/switched`

Agent mode changed. Sent in addition to (and richer than) `session/update` with `current_mode_update` (Section 5).

Handler: `handleAgentSwitched` (tui.js:124938).

```ts
type AgentSwitchedNotification = {
  agentName: string;                   // new mode id
  previousAgentName: string;           // prior mode id
  welcomeMessage?: unknown;            // opaque; typically null
  model?: unknown;                     // opaque; forwarded for display
}
```

### `kiro.dev/session/update`

**Extension variant** distinct from the standard ACP `session/update` (Section 5). Used primarily for lightweight `tool_call_chunk` updates — an early, rawInput-less preview of an imminent tool call.

Handler: `handleExtSessionUpdate` (tui.js:124948). In 2.0.1 the handler only interprets `update.sessionUpdate === "tool_call_chunk"`; any other variant is silently ignored.

```ts
type ExtSessionUpdateNotification = {
  sessionId?: string;                  // when different from main session → subagent
  update: {
    sessionUpdate: "tool_call_chunk";  // only variant the TUI reads in 2.0.1
    toolCallId: string;
    title: string;                     // e.g. "read", "shell", "ls"
    kind: ToolKind;                    // e.g. "read", "execute", "search"
  };
}
```

**Notes:**

- If `sessionId` differs from the main session, the TUI routes the event to subagent handlers (tui.js:124955).
- `tool_call_chunk` arrives **before** (or alongside) the standard `session/update` → `tool_call` notification. It's the earliest visible signal that a tool is about to run. The standard notification will follow with full `rawInput`.
- Although the SDK has no schema here, the handler is a switch that only branches on `sessionUpdate === "tool_call_chunk"` — other variants fall through to a debug log. Kiro may emit other `sessionUpdate` values in the future; 2.0.1 clients ignore them.

### `kiro.dev/subagent/list_update`

Snapshot of all running subagents and pending stages. Each notification carries the **complete** current state, not a delta.

Handler: `handleSubagentListUpdate` (tui.js:124875). Both arrays are defaulted to `[]` if absent.

```ts
type SubagentListUpdateNotification = {
  subagents?: SubagentEntry[];
  pendingStages?: PendingStageEntry[];
}

type SubagentEntry = {                 // fields read by consumer at tui.js:125991
  sessionId: string;
  sessionName?: string;                // falls back to agentName if empty
  agentName: string;
  status?: { type: "working" | "terminated" | string };  // "working"→busy, "terminated"→terminated, else→idle
  group?: string;
  parentSessionId?: string;
  role?: string;
  dependsOn?: string[];                // other stage names this subagent depends on
}

type PendingStageEntry = {             // fields read by consumer at tui.js:126018
  name: string;
  agentName?: string;                  // display name; falls back to `name`
  group?: string;
  role?: string;
  dependsOn?: string[];
}
```

**Notes:**

- **Snapshot semantics.** When a subagent disappears from `subagents[]` between notifications, it has terminated (or been cancelled). When a pending stage disappears from `pendingStages[]`, it has either started (and will now appear in `subagents[]`) or been abandoned.
- **Group tags cluster stages.** The `group` string identifies stages spawned from a single parent `subagent`/`agent_crew` tool call.
- **Older doc fields that don't appear in 2.0.1 handlers:** `initialQuery` and `status.message` were documented earlier but the 2.0.1 TUI doesn't read them. Kiro may still send them; they just aren't consumed.

### `kiro.dev/session/list_update`

Changes to the session list (used in the session picker).

Handler: `handleSessionListUpdate` (tui.js:124887). The handler defaults `sessions` to `[]` and fans it out via the same subagent-list handler set — so list entries share the `SubagentEntry` shape in 2.0.1.

```ts
type SessionListUpdateNotification = {
  sessions?: SubagentEntry[];          // same shape as subagent/list_update entries
}
```

### `kiro.dev/session/activity`

Per-session activity event (for example, an agent-thinking heartbeat or status update from a subagent).

Handler: `handleSessionActivity` (tui.js:124880). Requires both `sessionId` and `event`; if either is missing the notification is silently dropped.

```ts
type SessionActivityNotification = {
  sessionId: string;
  event: unknown;                      // forwarded opaquely to multi-session handlers
}
```

**Note:** the `event` payload is not introspected by the TUI in 2.0.1. Its shape is defined by whatever consumer subscribes via `onMultiSessionUpdate()`.

### `kiro.dev/session/inbox_notification`

Subagents have posted results back to a parent session.

Handler: `handleInboxNotification` (tui.js:124891). The whole `params` object is forwarded to inbox subscribers; no field-level access is performed by the notification router.

```ts
type InboxNotification = {
  // Fields observed in Kiro output (not enforced):
  sessionId?: string;                  // the target (usually main) session
  sessionName?: string;
  messageCount?: number;
  escalationCount?: number;
  senders?: string[];                  // e.g. ["subagent"]
}
```

**Note:** because the TUI forwards the raw `params` unchanged, the documented fields reflect observed Kiro behavior rather than TUI requirements. A client that only cares about "something changed in the inbox" can treat this as a signaling notification and ignore the payload.

## 7. Kiro extension requests (C→S)

Extension requests from client to agent are dispatched through `ClientSideConnection.extMethod()` (tui.js:124039), which prepends an underscore to the method name on the wire. The SDK validates neither request nor response (`extMethodRequestSchema` / `extMethodResponseSchema` are both `record(unknown)`, tui.js:123031, 123087). Shapes below are from the TUI's sender functions, not from a schema.

**Important reconciliation: `EXT_METHODS` ≠ wire format.** The `EXT_METHODS` constant table at tui.js:124335 lists some methods with plain ACP names (`SESSION_TERMINATE: "session/terminate"`, `SESSION_LIST: "session/list"`), but the actual sender functions (`terminateSession`, `listSessions`) bypass those constants and call `extMethod()` with a hardcoded `"kiro.dev/session/terminate"` / `"kiro.dev/session/list"` — producing `_kiro.dev/session/terminate` / `_kiro.dev/session/list` on the wire, not `_session/terminate` / `_session/list`.

The actual wire methods in 2.0.1 are enumerated in the table below. The EXT_METHODS constants that conflict with this behavior appear to be vestigial or reserved for future use; the SDK's `extMethod()` helper is the source of truth.

| Wire method (with leading `_`) | Caller | Location |
|---|---|---|
| `_kiro.dev/commands/execute` | `AcpClient.executeCommand()` | tui.js:124581 |
| `_kiro.dev/commands/options` | `AcpClient.getCommandOptions()` | tui.js:124603 |
| `_kiro.dev/session/terminate` | `AcpClient.terminateSession()` | tui.js:124621 |
| `_kiro.dev/session/list` | `AcpClient.listSessions()` | tui.js:124630 |
| `_kiro.dev/settings/list` | `AcpClient.listSettings()` | tui.js:124635 |
| `_session/spawn` | `AcpClient.spawnSession()` | tui.js:124910 |
| `_message/send` | `AcpClient.sendMessage()` | tui.js:124932 |

`_session/attach` is defined in `EXT_METHODS` (tui.js:124356) but has **no caller** in 2.0.1. It is likely reserved; treat it as undefined behavior.

---

### `_kiro.dev/commands/execute` — request

Execute a slash command on the backend. The full catalog of commands and their response shapes is covered in Section 9.

**Request:**

```ts
type CommandsExecuteRequest = {
  sessionId: string;
  command: TuiCommand;
}

type TuiCommand = {
  command: string;                     // e.g. "model", "context", "compact"
  args: Record<string, unknown>;       // e.g. { value: "claude-opus-4.6" } for /model
}
```

**Response:**

```ts
type CommandsExecuteResponse = {
  success: boolean;
  message?: string;                    // human-readable result
  data?: unknown;                      // command-specific; see Section 9
}
```

**Note on wire format (critical):** The `command` field is an **object**, not a string. Sending `{"command": "context", ...}` instead of `{"command": {"command": "context", "args": {}}, ...}` causes `kiro-cli` to respond with a deserialization error naming `TuiCommand` as the expected adjacently-tagged enum. Clients implementing slash commands must wrap the command name inside a `{command, args}` object.

**Example (selection command):**

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 20,
  "method": "_kiro.dev/commands/execute",
  "params": {
    "sessionId": "4dfac9d3-...",
    "command": { "command": "model", "args": { "value": "claude-opus-4.6" } }
  }
}

// Response
{
  "jsonrpc": "2.0",
  "id": 20,
  "result": {
    "success": true,
    "message": "Model changed to claude-opus-4.6",
    "data": { "model": { "id": "claude-opus-4.6", "name": "claude-opus-4.6" } }
  }
}
```

### `_kiro.dev/commands/options` — request

Query selectable options for a command (e.g. the list of models for `/model`).

**Request:**

```ts
type CommandsOptionsRequest = {
  sessionId: string;
  command: string;                     // bare name, no leading slash (TUI strips "/")
  partial: string;                     // user input so far, for filtering (may be "")
}
```

**Response:**

```ts
type CommandsOptionsResponse = {
  options: CommandOption[];
  hasMore?: boolean;
}

type CommandOption = {
  value: string;                       // ID sent back in a subsequent commands/execute
  label: string;                       // display name (prefer this over `name`)
  description?: string;
  group?: string;                      // grouping label (e.g. "1.00x credits" for /model)
  current?: boolean;                   // true if this is the currently active option
}
```

**Note:** the `getCommandOptions` caller (tui.js:124603) calls `commandName.replace(/^\//, "")` before sending, so clients should not include a leading slash in `command`. On network failure the sender falls back to `{options: []}` rather than surfacing the error.

### `_kiro.dev/settings/list` — request

Fetch user/workspace settings from the agent.

**Request:** `{}`

**Response:** an opaque `Record<string, unknown>`. The TUI returns the result directly from `listSettings()` (tui.js:124635) and does not introspect any fields at this layer. Downstream consumers may parse specific keys.

### `_kiro.dev/session/terminate` — request

Terminate a running session (typically a subagent).

**Request:**

```ts
type SessionTerminateRequest = {
  sessionId: string;
}
```

**Response:** the TUI's caller is wrapped in try/catch and logs "best-effort" failure (tui.js:124626–124627). Response shape is not introspected; clients can treat a successful return as acknowledgement.

### `_kiro.dev/session/list` — request

List all sessions (main + subagents) in a working directory.

**Request:**

```ts
type SessionListRequest = {
  cwd: string;
}
```

**Response:**

```ts
type SessionListResponse = {
  sessions: SubagentEntry[];           // same shape as subagent/list_update entries (Section 6)
}
```

The TUI destructures `{sessions}` from the response (tui.js:102547, 126115, 126168).

### `_session/spawn` — request

Spawn a child subagent session. Plain-name method routed through the extension dispatcher.

**Request:**

```ts
type SessionSpawnRequest = {
  sessionId: string;                   // the parent session
  task: string;                        // human-readable description
  name?: string;                       // UI label for the crew monitor — NOT a mode selector
}
```

> **Correction (2026-05-23):** Earlier versions of this document annotated `name` as `"subagent mode/role (matches availableModes[].id)"` based on field-name inference. **This is wrong.** Per the user-facing Kiro CLI documentation for `/spawn`, `--name` is purely a display label for the crew monitor — it does not select the subagent's mode. Empirically verified on Kiro 2.4.1: spawning with `name: "kiro_planner"` produces a subagent whose mode is `kiro_default` (inherited from the parent), not `kiro_planner`. The spawned subagent always inherits the parent session's mode at spawn time. The Kiro docs explicitly distinguish `/spawn` (user-initiated, parallel long-running session, label only) from agent-initiated subagents created via the agent's `subagent` tool (which support role specialization through the tool's stages array). At the wire level both surface in `_kiro.dev/subagent/list_update`, but their semantics differ.

**Response:**

```ts
type SessionSpawnResponse = {
  sessionId: string;                   // new subagent's session id
  name?: string;
}
```

The spawned subagent subsequently appears in `_kiro.dev/subagent/list_update` notifications (Section 6) and streams its own `session/update` events under its new `sessionId`.

**Example:**

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 30,
  "method": "_session/spawn",
  "params": {
    "sessionId": "4dfac9d3-...",
    "task": "Review the code for bugs",
    "name": "code-reviewer"
  }
}

// Response
{
  "jsonrpc": "2.0",
  "id": 30,
  "result": {
    "sessionId": "b49d53d1-a42a-4ef6-a173-a6224e8e6fcd",
    "name": "code-reviewer"
  }
}
```

### `_message/send` — request

Send a follow-up message to a specific session (most commonly a subagent).

**Request:**

```ts
type MessageSendRequest = {
  sessionId: string;
  content: unknown;                    // typically a string or an ACP ContentBlock[]
}
```

**Response:** the TUI's sender does not inspect the response (tui.js:124932–124937). Treat a successful return as acknowledgement.

**Example:**

```json
{
  "jsonrpc": "2.0",
  "id": 31,
  "method": "_message/send",
  "params": {
    "sessionId": "b49d53d1-...",
    "content": "Focus on error handling in storage/mod.rs"
  }
}
```

## 8. Shared types

Types referenced by multiple methods live here. Types specific to a single method are defined next to that method (Sections 2–7).

### `ContentBlock` — tui.js:123324

A discriminated union of content payloads. Used in `session/prompt`, `agent_message_chunk`, `agent_thought_chunk`, `user_message_chunk`, and (nested as `ToolCallContent.content`) in certain `ToolCallContent` variants.

```ts
type ContentBlock =
  | {
      type: "text";
      text: string;
      annotations?: Annotations | null;
    }
  | {
      type: "image";
      data: string;                    // base64-encoded
      mimeType: string;
      uri?: string | null;             // optional resource URI
      annotations?: Annotations | null;
    }
  | {
      type: "audio";
      data: string;                    // base64-encoded
      mimeType: string;
      annotations?: Annotations | null;
    }
  | {
      type: "resource_link";
      name: string;
      uri: string;
      mimeType?: string | null;
      description?: string | null;
      title?: string | null;
      size?: number | null;
      annotations?: Annotations | null;
    }
  | {
      type: "resource";
      resource: EmbeddedResourceContents;
      annotations?: Annotations | null;
    };
```

**Notes:**

- The union tag is `type` (standard adjacently-tagged union, compatible with Rust serde `#[serde(tag = "type", rename_all = "snake_case")]`).
- The schema is inlined in several places (e.g., each chunk variant in `sessionNotificationSchema` repeats the union). The structure is identical; the type above is the canonical shape.
- `image` is advertised via `promptCapabilities.image` (Section 2). Kiro sets this to `true`, so clients can send image blocks in prompts.
- `audio` and `embeddedContext` (the capability flag) are declared in `promptCapabilities` but Kiro sets both to `false` — Kiro does not accept audio prompts or embedded-resource prompts.

### `Role` — tui.js:123032

```ts
type Role = "assistant" | "user";
```

Used inside `Annotations.audience`. Only two values.

### `Annotations` — tui.js:123116

Metadata attached to content blocks and tool-call content. Entirely optional; clients typically ignore it unless they implement audience-scoped rendering or cache invalidation.

```ts
type Annotations = {
  audience?: Role[] | null;            // which roles should see this content
  lastModified?: string | null;        // ISO timestamp
  priority?: number | null;
}
```

### `EmbeddedResourceContents` — tui.js:123122

Union of text and blob resource contents. Used inside `ContentBlock` (variant `resource`) and `ToolCallContent` (variant `content` with inner type `resource`).

```ts
type EmbeddedResourceContents =
  | TextResourceContents
  | BlobResourceContents;

type TextResourceContents = {          // tui.js:123033
  uri: string;
  text: string;
  mimeType?: string | null;
}

type BlobResourceContents = {          // tui.js:123039
  uri: string;
  blob: string;                        // base64-encoded
  mimeType?: string | null;
}
```

**Note:** the two variants are not tagged — disambiguate by presence of `text` vs. `blob`.

### `ToolKind` — tui.js:123045

```ts
type ToolKind =
  | "read"
  | "edit"
  | "delete"
  | "move"
  | "search"
  | "execute"
  | "think"
  | "fetch"
  | "switch_mode"
  | "other";
```

10 values. Used in `ToolCall`, `ToolCallUpdate`, and `session/request_permission.toolCall.kind`.

**Notes:**

- The `tool_call` session-update variant inlines this union rather than referencing `toolKindSchema` (see Section 5). Values are identical.
- `"other"` is a catch-all and also marks "planning" steps that Kiro itself filters from display. Clients may want to suppress them from chat rendering.

### `ToolCallStatus` — tui.js:123057

```ts
type ToolCallStatus =
  | "pending"
  | "in_progress"
  | "completed"
  | "failed";
```

4 values. Used in tool-call lifecycle notifications.

### `ToolCallContent` — tui.js:123177

A 3-variant union carrying tool output. Tagged by `type`.

```ts
type ToolCallContent =
  | {
      type: "content";
      content: ContentBlock;           // one of the 5 ContentBlock variants (inlined in schema)
    }
  | {
      type: "diff";
      path: string;
      newText: string;
      oldText?: string | null;         // null for new files (no prior content)
    }
  | {
      type: "terminal";
      terminalId: string;              // live terminal feed (useful only if client holds the handle)
    };
```

**Notes:**

- The `content` variant embeds a full ContentBlock — tool output can be arbitrary content, including images.
- The `diff` variant is how edit-kind tools report changes (before/after for a given path).
- The `terminal` variant references a terminal id. In practice, Kiro does not use client-side terminals (Section 4), so this variant is rare. When it appears, the terminal is being managed by the agent — the id is informational only for a standard Kiro client.

### `ToolCallLocation` — tui.js:123233

```ts
type ToolCallLocation = {
  path: string;
  line?: number | null;                // 1-based; omitted means "the whole file"
}
```

Used by `ToolCall.locations` / `ToolCallUpdate.locations` to hint which files/lines a tool is operating on, for UI navigation.

### `PlanEntry` — tui.js:123287

```ts
type PlanEntry = {
  content: string;                     // free-text description of the step
  priority: "high" | "medium" | "low";
  status: "pending" | "in_progress" | "completed";
}
```

See `Plan` variant in Section 5. Each `plan` update carries a full array of these and replaces the previous plan.

### `Error` — tui.js:123063

JSON-RPC error shape used when a request fails. This is the standard JSON-RPC 2.0 error object; the SDK declares it explicitly so that handler code can parse it.

```ts
type Error = {
  code: number;
  message: string;
  data?: Record<string, unknown>;
}
```

**Typical codes observed from Kiro:**

| Code | Meaning |
|---|---|
| `-32601` | Method not found (e.g. older Kiro versions rejecting `session/set_config_option`) |
| `-32602` | Invalid params (e.g. malformed `commands/execute` payload) |
| `-32700` | Parse error (e.g. invalid JSON on the wire) |

Kiro also populates `data` with structured detail in some error paths — the TUI does not enforce a shape, so clients should treat `data` as opaque.

### Summary table — shared types and their homes

| Type | Line | Defined in section | Used in |
|---|---|---|---|
| `ContentBlock` | 123324 | 8 | `session/prompt`, all `*_message_chunk` variants, `ToolCallContent` |
| `Role` | 123032 | 8 | `Annotations` |
| `Annotations` | 123116 | 8 | `ContentBlock`, `ToolCallContent` (inlined variants) |
| `EmbeddedResourceContents` | 123122 | 8 | `ContentBlock` (resource variant) |
| `ToolKind` | 123045 | 8 | `ToolCall`, `ToolCallUpdate`, `session/request_permission` |
| `ToolCallStatus` | 123057 | 8 | `ToolCall`, `ToolCallUpdate`, `session/request_permission` |
| `ToolCallContent` | 123177 | 8 | `ToolCall`, `ToolCallUpdate`, `session/request_permission` |
| `ToolCallLocation` | 123233 | 8 | `ToolCall`, `ToolCallUpdate`, `session/request_permission` |
| `PlanEntry` | 123287 | 8 | `plan` session-update variant |
| `Error` | 123063 | 8 | JSON-RPC error responses |
| `PermissionOption` | 123166 | 4 | `session/request_permission` |
| `TerminalExitStatus` | 123364 | 4 | `terminal/output` |
| `McpServer` | 123309 | 3 | `session/new`, `session/load` |
| `HttpHeader` | 123111 | 3 | `McpServer` (http/sse variants) |
| `EnvVariable` | 123238 | 3 | `terminal/create`, `McpServer` (stdio variant) |
| `SessionMode` | 123271 | 3 | `SessionModeState` |
| `SessionModeState` | 123282 | 3 | `session/new`, `session/load` responses |
| `ModelInfo` | 123265 | 3 | `SessionModelState` |
| `SessionModelState` | 123277 | 3 | `session/new`, `session/load` responses |
| `AvailableCommand` | 123452 | 5 | `available_commands_update` variant (SDK-defined; has `input`) |
| `KiroCommand` | — (handler-inferred) | 6 | `_kiro.dev/commands/available` notification (Kiro-specific; has `meta`, no SDK schema) |
| `Implementation` | 123243 | 2 | `initialize` |
| `ClientCapabilities` | 123458 | 2 | `initialize` request |
| `AgentCapabilities` | 123446 | 2 | `initialize` response |
| `McpCapabilities` | 123254 | 2 | `AgentCapabilities` |
| `PromptCapabilities` | 123259 | 2 | `AgentCapabilities` |
| `FileSystemCapability` | 123298 | 2 | `ClientCapabilities` |
| `AuthMethod` | 123248 | 2 | `initialize` response |

## 9. Kiro command effects

Every slash command the user types is eventually routed through `_kiro.dev/commands/execute` (Section 7). The response is a generic `{ success, message?, data? }` envelope whose `data` shape varies per command. This section catalogs all 26 commands and their `data` payloads.

### Dispatch architecture

The TUI uses a two-layer dispatch:

1. **`commandEffects`** (tui.js:101756) maps each slash-command name to an **effect** name (a logical operation, e.g. `updateModel`).
2. **`effectHandlers`** (tui.js:101784) maps each effect name to the function that reads `result.data` and updates UI state.

Multiple commands can map to the same effect (e.g. `plan` is an alias for `agent`; `exit` for `quit`). A handler that returns `true` short-circuits the default behavior of displaying `result.message` as an alert.

### Response envelope

Every `_kiro.dev/commands/execute` call returns:

```ts
type CommandsExecuteResponse = {
  success: boolean;
  message?: string;                    // human-readable result, shown unless the effect handler returns true
  data?: unknown;                      // effect-specific; see tables below
}
```

The TUI's generic dispatcher (inside `commands/execute`'s caller) displays `message` as a success/error alert when `success` controls the color and the effect handler does not suppress it.

### `commandEffects` map

From tui.js:101756:

```ts
{
  feedback:   "showFeedbackUrl",
  help:       "showHelpPanel",
  model:      "updateModel",
  agent:      "updateAgent",
  plan:       "updateAgent",           // alias — same handler as `agent`
  context:    "showContextPanel",
  usage:      "showUsagePanel",
  prompts:    "executePrompt",
  clear:      "clearMessages",
  quit:       "quit",
  exit:       "quit",                  // alias
  mcp:        "showMcpPanel",
  tools:      "showToolsPanel",
  hooks:      "showHooksPanel",
  knowledge:  "showKnowledgePanel",
  paste:      "pasteImage",
  editor:     "promptEditor",
  chat:       "loadSession",
  reply:      "replyEditor",
  code:       "showCodePanel",
  spawn:      "spawnSession",
  copy:       "copyToClipboard",
  transcript: "openRawView",
  theme:      "showThemeMenu",
  tui:        "showTuiPanel",
  guide:      "switchToGuideAgent",
}
```

Plus two commands that are **not in `commandEffects`** — they are dispatched outside the generic pipeline:

- **`new`** — invokes `newSession` handler (tui.js:101956) directly; calls `kiro.newSession()` and optionally sends an initial prompt from `args`.
- **`switch`** — invokes `switchSession` handler (tui.js:102044) directly; opens a session picker or switches to a named session.

### Per-command `data` shapes

The following table documents, for each command, what the effect handler reads from `data`. Fields not listed may be present on the wire; the TUI does not consume them in 2.0.1.

#### Panel-opening commands

These commands return a `data` payload that populates an in-UI panel.

| Command | Effect | `data` shape | Handler |
|---|---|---|---|
| `/help` | `showHelpPanel` | `{ commands: Array<{ name, description, usage }> }` | 101833 |
| `/context` | `showContextPanel` | `{ breakdown: ContextBreakdown, contextUsagePercentage?: number, initialExpanded?: boolean }` | 101821 |
| `/usage` | `showUsagePanel` | opaque — passed through to the panel | 101845 |
| `/mcp` | `showMcpPanel` | `{ servers: McpServerInfo[], mode?: "list" \| string }` (mode defaults to `"list"`) | 101848 |
| `/tools` | `showToolsPanel` | `{ tools: ToolInfo[] }` | 101852 |
| `/hooks` | `showHooksPanel` | `{ hooks: HookInfo[] }` (defaults to `[]` if absent) | 101858 |
| `/knowledge` | `showKnowledgePanel` | `{ entries: KnowledgeEntry[], status: KnowledgeStatus }` — if `data.entries` absent, closes panel and shows `message` as alert | 101862 |
| `/code` | `showCodePanel` | **Dual-purpose** — see note below | 101922 |
| `/tui` | `showTuiPanel` | none read | 102162 |

**Context breakdown shape** (inferred from `/context` panel fields observed in earlier Kiro versions):

```ts
type ContextBreakdown = {
  contextFiles:   { tokens: number; percent: number; items: ContextItem[] };
  tools:          { tokens: number; percent: number };
  kiroResponses:  { tokens: number; percent: number };
  yourPrompts:    { tokens: number; percent: number };
  sessionFiles:   { tokens: number; percent: number };
}

type ContextItem = { name: string; tokens: number; matched: boolean; percent: number };
```

**HookInfo shape** (from backend; three columns `trigger`, `command`, optional `matcher`):

```ts
type HookInfo = {
  trigger: string;                     // e.g. "pre-tool", "post-response"
  command: string;                     // shell command to run
  matcher?: string;                    // optional trigger filter
}
```

**Dual-purpose `showCodePanel`** (tui.js:101922): the effect handler branches on `data.executePrompt`. If present, the handler calls `ctx.sendMessage(data.executePrompt, undefined, data.label)` and returns `true` (short-circuit). Otherwise, if `data` is present, the code panel opens. If `data` is absent and a `message` is present, the message is shown as an alert. So `/code` can either open a panel **or** forward a prompt for submission, discriminated by the presence of `data.executePrompt` in the response.

#### State-update commands

| Command | Effect | `data` shape | Handler |
|---|---|---|---|
| `/model` | `updateModel` | `{ model: { id, name } }` | 101785 |
| `/model <id>` (selection) | `updateModel` | same | 101785 |
| `/agent` | `updateAgent` | `{ agent: AgentInfo }` OR `{ path: string }` | 101791 |
| `/agent <name>` | `updateAgent` | `{ agent: AgentInfo }` | 101791 |
| `/plan` | `updateAgent` (alias) | same as `/agent` | 101791 |
| `/guide` | `switchToGuideAgent` | `{ agent: AgentInfo, prompt?: string }` | 102402 |

**`updateAgent` is dual-purpose.** If `data.path` is present, the handler opens `$VISUAL` or `$EDITOR` on the file, validates that the saved JSON has a `name` field, and shows a success/error alert (returns `true` to short-circuit). Otherwise if `data.agent` is present, it sets the current agent. A command that wants to let the user edit a config file returns `{ path }`; one that simply switches agents returns `{ agent }`.

**`switchToGuideAgent` may also send a prompt.** If `data.prompt` is present, the handler calls `ctx.sendMessage(data.prompt)` after setting the agent. This allows `/guide` to "set agent + start a turn" atomically.

#### Prompt-forwarding commands

These commands turn their response into a user prompt without showing a panel.

| Command | Effect | `data` shape | Handler |
|---|---|---|---|
| `/prompts` | `executePrompt` | `{ executePrompt: string }` | 101875 |
| `/code` (alt path) | `showCodePanel` | `{ executePrompt: string, label?: string }` | 101922 |
| `/paste` | `pasteImage` | `{ data: string (base64), mimeType: string }` | 101946 |

**`pasteImage` recipe.** When the backend returns image data, the handler calls `ctx.sendMessage(formatImageLabel(data), [{ base64: data.data, mimeType: data.mimeType }])`. This is the concrete pattern for attaching images to a prompt: `sendMessage`'s second parameter is an array of `{ base64, mimeType }` image attachments that the TUI converts to `ContentBlock` entries (type `image`) in the outgoing `session/prompt`.

#### Feedback / notification commands

| Command | Effect | `data` shape | Handler |
|---|---|---|---|
| `/feedback` | `showFeedbackUrl` | `{ url: string }` — handler shows `message` (or `url` as fallback) as a warning alert for 10s and returns `true` | 101939 |

#### Editor-based commands (shell out to `$EDITOR`)

| Command | Effect | `data` shape | Handler |
|---|---|---|---|
| `/editor` | `promptEditor` | *(ignored — args passed as initial content)* | 101888 |
| `/reply` | `replyEditor` | `{ initialContent: string }` — editor pre-populated with this content | 101902 |

Both handlers use `openEditorSync` to write a temp file, invoke `$EDITOR`, read the result, validate (non-empty for `editor`, must differ from `initialContent` for `reply`), and call `ctx.sendMessage(content)`. Both return `true` to short-circuit.

#### Session management commands

| Command | Effect | `data` shape | Handler |
|---|---|---|---|
| `/chat save <name>` | `loadSession` | backend fills in `message` + `success`; handler shows alert and returns `true` | 101985 |
| `/chat load <name>` | `loadSession` | `{ sessionId: string }` on success; handler then calls `kiro.loadSession(sessionId, ...)` | 101985 |
| `/chat new [prompt]` | *(special — `newSession` handler)* | backend response ignored; handler calls `kiro.newSession()` then optionally `sendMessage(prompt)` | 101956 |
| `/spawn <task> [--name <id>]` | `spawnSession` | response ignored; handler parses args, calls `kiro.spawnSession(task, name)`, adds to local session store | 102086 |
| `/switch [name]` | *(special — `switchSession` handler)* | handler reads the local session list, not backend `data` | 102044 |

**Note on `/chat`:** the handler dispatches on whether `args` starts with `save` or `load`. The `load` branch parses `sessionId` from `data` and calls the async `loadSession`, replaying up to `MAX_DISPLAY_TURNS = 10` recent turns from the buffered event stream.

#### UI/local commands (no or minimal data)

| Command | Effect | `data` shape | Handler |
|---|---|---|---|
| `/clear` | `clearMessages` | none | 101881 |
| `/quit`, `/exit` | `quit` | none (calls `kiro.close()` + `process.exit(0)`) | 101884 |
| `/copy` | `copyToClipboard` | none — copies most recent assistant message | 102128 |
| `/transcript` | `openRawView` | none — opens the conversation in `$PAGER` | 102153 |
| `/theme` | `showThemeMenu` | none — handler opens a theme picker from local prefs + bundled themes | 102165 |

`/quit`, `/exit`, `/editor`, `/spawn`, `/copy`, `/theme`, `/transcript` are declared as **local** commands in the TUI's own default command list (marked `source: "local"` in the slash-command registry). The backend is not consulted for these — the TUI calls the effect handler directly. They appear in this section because their dispatch path uses the same effect architecture.

### Effect handlers that short-circuit

A handler that returns `true` tells the outer dispatcher not to show `result.message` as an alert. Effects that do this:

| Effect | When it short-circuits | Why |
|---|---|---|
| `updateAgent` | When `data.path` flow completes (success or error) | Handler already shows its own editor-result alert |
| `promptEditor` | Always | Handler already shows editor failures or sends the message |
| `replyEditor` | Always | Same |
| `showCodePanel` | When `data.executePrompt` is present | Response was treated as a prompt, not a panel |
| `showFeedbackUrl` | When `data.url` is present | Handler already showed the URL as a warning alert |
| `openRawView` | On success | Handler opens the pager; alert would be redundant |
| `newSession` | Always | Handler shows its own lifecycle alerts |

### Pattern summary

1. **All backend commands flow through `_kiro.dev/commands/execute`.** The TUI handles the response envelope centrally and delegates to an effect handler based on `commandEffects`.
2. **Unknown commands degrade gracefully.** If a command name isn't in `commandEffects`, the dispatcher falls back to rendering `message` as an alert — so new backend commands "just work" for the simple message path.
3. **Local commands use the same pipeline.** The TUI's own commands (`/quit`, `/copy`, etc.) register in the same effect table even though they don't round-trip to the backend.
4. **Data-shape discrimination within an effect is common.** `updateAgent` (agent vs. path), `showCodePanel` (executePrompt vs. panel), and `showKnowledgePanel` (entries vs. message fallback) all branch on which `data.*` field is present.
5. **`sendMessage` is the universal entry point for prompt submission.** Whether triggered by `/editor`, `/reply`, `/paste`, `/prompts`, or a `/code` prompt-forward — the terminal call is always `ctx.sendMessage(...)`, which goes out as a `session/prompt` (Section 3).

## 10. Appendix

### A. Complete method catalog

| # | Wire name | Direction | Type | Section |
|---|---|---|---|---|
| 1 | `initialize` | C→S | request | 2 |
| 2 | `authenticate` | C→S | request | 2 |
| 3 | `session/new` | C→S | request | 3 |
| 4 | `session/load` | C→S | request | 3 |
| 5 | `session/prompt` | C→S | request | 3 |
| 6 | `session/cancel` | C→S | notification | 3 |
| 7 | `session/set_mode` | C→S | request | 3 |
| 8 | `session/set_model` | C→S | request | 3 |
| 9 | `fs/read_text_file` | S→C | request | 4 |
| 10 | `fs/write_text_file` | S→C | request | 4 |
| 11 | `session/request_permission` | S→C | request | 4 |
| 12 | `session/update` | S→C | notification | 4, 5 |
| 13 | `terminal/create` | S→C | request | 4 |
| 14 | `terminal/output` | S→C | request | 4 |
| 15 | `terminal/wait_for_exit` | S→C | request | 4 |
| 16 | `terminal/kill` | S→C | request | 4 |
| 17 | `terminal/release` | S→C | request | 4 |
| 18 | `_kiro.dev/commands/available` | S→C | notification | 6 |
| 19 | `_kiro.dev/metadata` | S→C | notification | 6 |
| 20 | `_kiro.dev/compaction/status` | S→C | notification | 6 |
| 21 | `_kiro.dev/clear/status` | S→C | notification | 6 |
| 22 | `_kiro.dev/mcp/server_initialized` | S→C | notification | 6 |
| 23 | `_kiro.dev/mcp/server_init_failure` | S→C | notification | 6 |
| 24 | `_kiro.dev/mcp/oauth_request` | S→C | notification | 6 |
| 25 | `_kiro.dev/error/rate_limit` | S→C | notification | 6 |
| 26 | `_kiro.dev/agent/not_found` | S→C | notification | 6 |
| 27 | `_kiro.dev/agent/config_error` | S→C | notification | 6 |
| 28 | `_kiro.dev/agent/switched` | S→C | notification | 6 |
| 29 | `_kiro.dev/model/not_found` | S→C | notification | 6 |
| 30 | `_kiro.dev/session/update` | S→C | notification | 6 |
| 31 | `_kiro.dev/subagent/list_update` | S→C | notification | 6 |
| 32 | `_kiro.dev/session/activity` | S→C | notification | 6 |
| 33 | `_kiro.dev/session/list_update` | S→C | notification | 6 |
| 34 | `_kiro.dev/session/inbox_notification` | S→C | notification | 6 |
| 35 | `_kiro.dev/commands/execute` | C→S | request | 7 |
| 36 | `_kiro.dev/commands/options` | C→S | request | 7 |
| 37 | `_kiro.dev/settings/list` | C→S | request | 7 |
| 38 | `_kiro.dev/session/terminate` | C→S | request | 7 |
| 39 | `_kiro.dev/session/list` | C→S | request | 7 |
| 40 | `_session/spawn` | C→S | request | 7 |
| 41 | `_message/send` | C→S | request | 7 |

**Total: 41 wire methods** in Kiro CLI 2.0.1 — 17 standard ACP (tracked by SDK constants) + 24 Kiro extensions (17 notifications + 7 requests).

### B. SDK union roots (tui.js:123674–123747)

The SDK composes every valid JSON-RPC envelope as a union of concrete schemas. These are the union definitions from the bundle.

**Client → Agent (what the client can send):**

```ts
// clientRequestSchema — tui.js:123689
type ClientRequest =
  | InitializeRequest
  | AuthenticateRequest
  | NewSessionRequest
  | LoadSessionRequest
  | SetSessionModeRequest
  | PromptRequest
  | SetSessionModelRequest
  | ExtMethodRequest;   // record(unknown) — used for _kiro.dev/* extensions

// clientNotificationSchema — tui.js:123369
type ClientNotification =
  | CancelNotification
  | ExtNotification;    // record(unknown)

// clientResponseSchema — tui.js:123674 (responses TO requests from the agent)
type ClientResponse =
  | WriteTextFileResponse
  | ReadTextFileResponse
  | RequestPermissionResponse
  | CreateTerminalResponse
  | TerminalOutputResponse
  | ReleaseTerminalResponse
  | WaitForTerminalExitResponse
  | KillTerminalResponse
  | ExtMethodResponse;  // record(unknown)
```

**Agent → Client (what the agent can send):**

```ts
// agentRequestSchema — tui.js:123435
type AgentRequest =
  | WriteTextFileRequest
  | ReadTextFileRequest
  | RequestPermissionRequest
  | CreateTerminalRequest
  | TerminalOutputRequest
  | ReleaseTerminalRequest
  | WaitForTerminalExitRequest
  | KillTerminalCommandRequest
  | ExtMethodRequest;

// agentNotificationSchema — tui.js:123685
type AgentNotification =
  | SessionNotification  // the session/update wrapper — see Section 5
  | ExtNotification;

// agentResponseSchema — tui.js:123703 (responses TO requests from the client)
type AgentResponse =
  | InitializeResponse
  | AuthenticateResponse
  | NewSessionResponse
  | LoadSessionResponse
  | SetSessionModeResponse
  | PromptResponse
  | SetSessionModelResponse
  | ExtMethodResponse;
```

### C. Top-level envelope schemas (tui.js:123713–123747)

The SDK wraps the unions above into generic JSON-RPC message shapes:

```ts
// Generic JSON-RPC request (tui.js:123463 / 123713)
type Request<Params> = {
  id: null | number | string;
  method: string;
  params?: Params | null;
}

// Generic notification (tui.js:123409 / 123699)
type Notification<Params> = {
  method: string;
  params?: Params | null;
}

// Generic response union — either result or error (tui.js:123718 / 123731)
type Response<Result> =
  | { result: Result }
  | { error: Error };     // errorSchema (tui.js:123063)
```

Concrete combinations:

```ts
// What the client is allowed to put on the wire (tui.js:123726)
type ClientOutgoingMessage =
  | Request<ClientRequest>
  | Response<ClientResponse>
  | Notification<ClientNotification>;

// What the agent is allowed to put on the wire (tui.js:123739)
type AgentOutgoingMessage =
  | Request<AgentRequest>
  | Response<AgentResponse>
  | Notification<AgentNotification>;

// The full protocol surface (tui.js:123744) — what either side can send
type AgentClientProtocol = AgentOutgoingMessage | ClientOutgoingMessage;
```

### D. JSON-RPC framing

Every message on the wire is a JSON object on a single line (newline-delimited JSON). Pretty-printing does not occur — line boundaries separate messages.

```
Content → { ...JSON... }\n
```

- **Request:** has `id`, has `method`, has `params` (optional).
- **Notification:** has `method`, has `params` (optional). **No `id` field.** (Absence of `id` is the only thing distinguishing a notification from a request.)
- **Response:** has `id`, has either `result` or `error` (mutually exclusive), has no `method`.

### E. Error codes observed from Kiro

| Code | Standard meaning | Observed Kiro cause |
|---|---|---|
| `-32600` | Invalid request | Malformed JSON-RPC envelope |
| `-32601` | Method not found | Unsupported method (e.g. `session/set_config_option`) |
| `-32602` | Invalid params | Schema validation failure (e.g. wrong `TuiCommand` shape — see Section 7) |
| `-32603` | Internal error | Backend panic |
| `-32700` | Parse error | Invalid JSON on the wire |

Kiro may populate the optional `error.data` field with structured detail. The SDK schema leaves it as `Record<string, unknown>` (tui.js:123063–123067); clients should treat `data` as opaque.

### F. Version history

| Kiro version | SDK version | Notable protocol changes |
|---|---|---|
| **2.0.1** | `@agentclientprotocol/sdk@0.5.1` | `PROTOCOL_VERSION = 1`. Documented in this file. Wire-format methods: 41. `session/set_model` is a first-class method (no longer behind an unstable flag). `stopReason` expanded to 5 values. `user_message_chunk` variant added to `session/update`. `sessionCapabilities` removed from `initializeResponse`. New extension notifications: `agent/not_found`, `agent/config_error`, `model/not_found`. Multi-session methods (`_session/spawn`, `_message/send`) are dispatched through the extension mechanism with plain ACP-style names (no `kiro.dev/` prefix). |

### G. Reverse-engineering notes

If you need to re-verify or extend this document against a future Kiro release, the reliable anchors are:

1. **`AGENT_METHODS` / `CLIENT_METHODS` constant tables** — search the bundle for the exact strings. These are the ground truth for standard ACP methods and their direction.
2. **`EXT_METHODS` table** — search for `_kiro.dev/` string literals. The table enumerates extensions, but **do not trust it for wire names alone** — some entries in 2.0.1 are vestigial (e.g. `SESSION_TERMINATE: "session/terminate"` is not actually used on the wire). Always cross-check against the sender function.
3. **Zod schemas** — search for `exports_external.object({` near line 122995 onward. Each schema is named `<name>Schema` and maps directly to a TypeScript type.
4. **Handler dispatch tables** — for Kiro extensions, the `extNotificationHandlers` map (`AcpClient.extNotificationHandlers` around line 124719) is the routing layer. Each entry's handler function reveals what fields the TUI actually reads.
5. **`commandEffects` and `effectHandlers`** — the two-layer dispatch for slash commands, around line 101756 and 101784. Use these to trace any `_kiro.dev/commands/execute` subcommand to the UI effect it produces.

Avoid relying on property-access grep results alone — they can be misleading when the bundle includes dead library code (e.g. the `AgentSideConnection` class at line ~123800 is bundled but never instantiated; only `ClientSideConnection` at line 123945 is wired up). Always verify the containing class and check `new X(` to confirm instantiation.

---

## 11. Changes since 2.0.1

Wire-surface additions and behavior changes discovered through empirical capture and binary diff between Kiro CLI 2.0.1 (the baseline of Sections 1–10) and Kiro CLI 2.4.1 (current production as of 2026-05-23). Per-release detail in [`kiro-2.3.0-wire-audit.md`](kiro-2.3.0-wire-audit.md) and [`kiro-2.4.1-wire-audit.md`](kiro-2.4.1-wire-audit.md). Coverage gap against cyril in [`cyril-acp-coverage-vs-2.4.1.md`](cyril-acp-coverage-vs-2.4.1.md).

### 11.1 New slash commands (`_kiro.dev/commands/execute`)

| Command | Added | Notes |
|---|---|---|
| `/stats` | 2.3.0 | Per-backend-request statistics (1 entry per round-trip, N entries for non-trivial turns). Response shape: `data.stats[].{duration_ms, ttfc_ms, input_tokens, output_tokens, request_id, status_code, had_tool_use, error}` + `data.summary.{avg_ms, p90_ms, max_ms, errors}`. **`input_tokens`/`output_tokens` are null** in every capture through 2.4.1 — backend rollout state, independent of model and effort level. |
| `/effort` | 2.4.0 | Thinking-effort level for the session. `inputType: "selection"`. Options-list is **model-conditional**: empty under non-thinking models (haiku/auto), 5 values under Opus 4.7: `low`/`medium`/`high`/`xhigh`/`max`. Active value is signaled via `label: "xHigh  [active]"` suffix (not a structured `current: true` field). |
| `/rewind` | 2.4.0 | "Rewind conversation to a previous turn (forks into a new session)". `inputType: "panel"`. Two-step orchestration: (1) no-args call returns `data.turns[].{group, label, logIndex, responseSnippet}`; (2) selection call with `args: {value: "<logIndex-as-string>"}` returns `data.{sessionId, switchSession: true}`. Client then calls `session/load` (new sessionId) + `_kiro.dev/session/terminate` (old sessionId). Note: `args.value` is a **string**, not a number — sending `{value: 0}` (integer) hangs the agent silently. |

### 11.2 New extension methods

| Method | Direction | Added | Wire shape |
|---|---|---|---|
| `_kiro.dev/settings/list` | C→S | 2.3.0 | Request: `{}` (empty params required — non-empty hangs the agent silently). Response: flat dotted-key map mirroring `~/.kiro/settings/cli.json` (`chat.enableThinking: true`, `introspect.progressiveMode: true`, etc.) with optional sub-object nesting (`chat: {enableNotifications: true}` alongside `chat.enableNotifications: true`). |
| `_kiro.dev/mcp/governance_disabled` | S→C | 2.3.0 | Notification with payload `{ apiFailure: boolean }`. Fires when MCP governance is administratively disabled. |

`_kiro.dev/settings/set` is in tui.js's method-name constants table but has **zero call sites** — settings are persisted by the TUI writing `~/.kiro/settings/cli.json` directly, no ACP roundtrip. Do not implement.

### 11.3 Removed (agent-side) extension methods

| Method | Removed | Notes |
|---|---|---|
| `_kiro.dev/agent/config_error` | 2.3.0 | tui.js still has `handleAgentConfigError` defensively, but the 2.3.0+ agent doesn't emit it. |
| `_kiro.dev/session/list` (notification) | 2.3.0 | Agent stopped emitting; tui.js retains the handler. (Note: the C→S request `_kiro.dev/session/list` for listing past sessions is still implemented — separate from the notification.) |
| `_kiro.dev/model/not_found` | between 2.0.1 and 2.4.1 | Documented in Section 6 (carried over from 2.0.1) but **absent from the 2.4.1 tui.js bundle** — both the method string and any handler are gone. Cyril still has a handler in `convert/kiro.rs:511` — dead code. Exact removal release not narrowed down. |

### 11.4 New fields on existing methods

#### `_kiro.dev/metadata` — `effort` field

Under thinking-capable models (Opus 4.7+) the post-turn metadata notification gains an `effort` field carrying the active effort level as a string (`low`/`medium`/`high`/`xhigh`/`max`). Sent immediately on model-switch and on every subsequent metadata notification. Absent under haiku-class models (model-conditional).

Also confirmed in 2.4.1: bare `{sessionId}` metadata notifications occur (keep-alives). All fields after `sessionId` are optional.

#### `session/update` → `tool_call` and `tool_call_update` — field drift

Section 5's schemas were derived from the 2.0.1 Zod definitions. In practice on 2.4.1:

- The `kind` field on `tool_call` and `tool_call_update` is consistently populated; matches the 0.5.1 SDK `ToolKind` enum.
- `locations[]` appears on file-touching tool calls with `[{path: "<abs-path>"}]` entries.
- `rawOutput` is in practice a tagged-union container: `rawOutput.items[]` where each item is `{Text: "<string>"}` (file content) OR `{Json: {...}}` (shell exec stdout/stderr/exit_status, web-search results, etc.). The Zod schema declares `rawOutput` as `Record<string, unknown>` so this is shape rather than schema — clients should pattern-match on the inner discriminator.

#### `_kiro.dev/session/inbox_notification` — `sessionName` field

Captured on 2.4.1: `params.sessionName: "main"` accompanies the existing `sessionId`. Documented in Section 6 as observed (not enforced); confirmed still present.

#### `session/request_permission` — `_meta.trustOptions[]`

Major undocumented sub-structure on shell/grep/out-of-workspace-read permission requests:

```json
"_meta": {
  "trustOptions": [
    {
      "label":       "Full command",
      "display":     "find ~/.cargo/registry/src ...",
      "setting_key": "allowedCommands",
      "patterns":    ["find \\~/\\.cargo/...", "head \\-3"]
    },
    { "label": "Partial command", "display": "...", "setting_key": "allowedCommands", "patterns": [...] },
    { "label": "Base command",    "display": "...", "setting_key": "allowedCommands", "patterns": [...] }
  ]
}
```

Three permission tiers (Full / Partial / Base) the agent proposes when the user wants "Always". Each carries a `setting_key` indicating where the client should persist the chosen pattern. Web-search permission requests omit `_meta` (atomic permission, no decomposition).

### 11.5 Subagent result delivery (the "Summarizing" tool_call)

After a `_session/spawn`-ed subagent completes its turn, the **parent agent** emits a `session/update` of variant `tool_call` on the **main session**, with:

```json
"update": {
  "sessionUpdate": "tool_call",
  "toolCallId": "tooluse_...",
  "title": "Summarizing",
  "kind": "other",
  "rawInput": {
    "__tool_use_purpose": "Task is complete, reporting back.",
    "taskDescription":    "<the original task we sent>",
    "taskResult":         "<the subagent's final message>"
  }
}
```

`rawInput.taskResult` is the canonical source for "what did the subagent say." The `_kiro.dev/session/inbox_notification` only carries counters (`messageCount`, `escalationCount`, `senders[]`), not the message body. Clients wanting full per-message history of the subagent should consume its `session/update` stream (routed by the subagent's `sessionId`).

The `__tool_use_purpose` field (double-underscore prefix) is an internal convention seen on multiple tool calls — annotation the agent attaches to its own tool invocations.

### 11.6 Built-in TUI recorder — `KIRO_ACP_RECORD_PATH` (2.4.0+)

The bundled TUI gained a built-in ACP wire recorder. Set `KIRO_ACP_RECORD_PATH=/path/to/trace.jsonl` before `kiro-cli chat --tui` and the TUI writes every JSON-RPC frame to that file. Implemented in tui.js via a `TransformStream` tap; hooks `SIGINT`/`SIGTERM`/`beforeExit` for flush.

Format (3 keys):

```json
{"ts": <unix-millis>, "dir": "out" | "in", "msg": <raw-JSON-RPC>}
```

`dir`: `out` = client→agent, `in` = agent→client (from the TUI client's perspective). Only active when the env var is set; only captures `kiro-cli chat --tui` mode (NOT `kiro-cli acp` mode that cyril uses). For cyril-side captures the rust proxy at `experiments/kiro-proxy-rs/` is still required.

### 11.7 KAS scaffolding (not yet on the wire)

2.3.0 added a `--agent-engine rust|kas` flag plus env vars (`KIRO_AGENT_ENGINE`, `KIRO_KAS_SERVER_PATH`, `KIRO_MODE`) and asset-extraction code (`crates/chat-cli/src/embedded_tui.rs`). Running `kiro-cli acp --agent-engine kas` on 2.4.1 still errors with "KAS assets not embedded and KIRO_KAS_SERVER_PATH not set." Binary delta 2.3.0 → 2.4.1 is only +2.36 MB on `kiro-cli-chat` (the IDE-shipped KAS bundle is ~36 MB) — assets are not landed. KAS, when it lands, is expected to use a parallel `_kiro/*` namespace (no `.dev`) per IDE evidence.

### 11.8 Authoritative Kiro extension inventory (2.4.1)

The full list of Kiro-specific wire surfaces present in `kiro-tui-2.4.1.js`, extracted by string-grep on `"_?kiro\.dev/[a-z/_]+"` and the EXT_METHODS-equivalent constants table at bundle position ~12054000. This table is the source of truth for "what Kiro extensions exist in 2.4.1" — Sections 6 and 7 above describe each method's shape; this index just enumerates them.

**`_kiro.dev/*` namespace (23 methods):**

| Method | Direction | Section | Cyril handles? |
|---|---|---|---|
| `commands/available` | S→C notif | §6 | ✓ |
| `commands/execute` | C→S request | §7 | ✓ (outbound via `BridgeCommand::ExecuteCommand`) |
| `commands/options` | C→S request | §7 | ✓ (outbound via `BridgeCommand::QueryCommandOptions`) |
| `metadata` | S→C notif | §6 | ✓ |
| `compaction/status` | S→C notif | §6 | ✓ |
| `clear/status` | S→C notif | §6 | ✓ |
| `agent/switched` | S→C notif | §6 | ✓ |
| `agent/not_found` | S→C notif | §6 | ✓ |
| `agent/config_error` | S→C notif | §6 | ✓ (handler kept; agent stopped emitting in 2.3.0, see § 11.3) |
| `mcp/server_initialized` | S→C notif | §6 | ✓ |
| `mcp/server_init_failure` | S→C notif | §6 | ✓ |
| `mcp/oauth_request` | S→C notif | §6 | ✓ |
| `mcp/governance_disabled` | S→C notif | § 11.2 | **gap** (added in 2.3.0; coverage doc Tier 2) |
| `error/rate_limit` | S→C notif | §6 | ✓ |
| `subagent/list_update` | S→C notif | §6 | ✓ |
| `session/inbox_notification` | S→C notif | §6 | ✓ |
| `session/list_update` | S→C notif | §6 | ✓ |
| `session/activity` | S→C notif | §6 | ✓ (dispatched together with `session/list_update`) |
| `session/update` | S→C notif | §6 | ✓ (lightweight `tool_call_chunk`) |
| `session/list` | C→S request | §7 | — (not yet exercised by cyril) |
| `session/terminate` | C→S request | §7 | ✓ (outbound via `BridgeCommand::TerminateSession`) |
| `settings/list` | C→S request | § 11.2 | — (gap; coverage doc Tier 2) |
| `settings/set` | (dead surface) | § 11.2 | **don't implement** — no caller anywhere |

**Bare-path Kiro extensions (5 methods):**

| Method | Direction | Section | Cyril handles? |
|---|---|---|---|
| `session/spawn` | C→S request | §7 | ✓ (outbound via `BridgeCommand::SpawnSession`) |
| `session/attach` | C→S request | §7 | — (no caller in tui.js per Section 7; reserved) |
| `session/list` | (vestigial) | §7 | — (EXT_METHODS constant present but `kiro.dev/session/list` is the live wire path) |
| `session/terminate` | (vestigial) | §7 | — (same; `kiro.dev/session/terminate` is the live path) |
| `message/send` | C→S request | §7 | ✓ (outbound via `BridgeCommand::SendMessage`) |

**Method removed since 2.0.1**: `kiro.dev/model/not_found` (see § 11.3).

To regenerate this inventory after a future Kiro release, run:

```sh
grep -oE '"_?kiro\.dev/[a-z/_]+"' ~/.local/share/kiro-research/tui-bundles/kiro-tui-<ver>.js | sort -u
```

Then diff against the prior version's output to find additions/removals. The EXT_METHODS-like constants table region (find via `grep -bn 'SESSION_TERMINATE\|SESSION_SPAWN' <bundle>`) lists the bare-path extensions.

### 11.9 Handler-extracted field verification (2.4.1)

Each Kiro extension handler in `kiro-tui-2.4.1.js` has been read and its parameter-destructuring extracted. The fields below are **what the TUI actually reads**, which is the ground truth for "what fields exist on the wire" — anything else is either dead-on-arrival or not consumed.

Methodology: the handler functions retain human-readable names through the bundler (e.g., `handleInboxNotification`, `handleMetadataUpdate`). Search the bundle for each handler name, read the function body, transcribe the `e.field` / `let {field} = e` accesses.

| Method | Handler | Fields read by TUI handler |
|---|---|---|
| `_kiro.dev/commands/available` | `handleCommandsAdvertising` | `commands[]`, `prompts[]`, `tools[]`, `mcpServers[]` |
| `_kiro.dev/metadata` | `handleMetadataUpdate` | `sessionId`, `contextUsagePercentage`, `meteringUsage[]`, `turnDurationMs`, `effort` |
| `_kiro.dev/compaction/status` | `handleCompactionStatus` | `status.{type, error}`, `summary` (top-level) |
| `_kiro.dev/clear/status` | `handleClearStatus` | **none — handler takes no params** (`[CLEAR_STATUS]:()=>this.handleClearStatus()` in the dispatch). Any `message` field on the wire is dropped. |
| `_kiro.dev/agent/switched` | `handleAgentSwitched` | `agentName`, `previousAgentName`, `welcomeMessage`, `model` |
| `_kiro.dev/agent/not_found` | `handleAgentNotFound` | `requestedAgent`, `fallbackAgent` |
| `_kiro.dev/agent/config_error` | `handleAgentConfigError` | `path`, `error` |
| `_kiro.dev/mcp/server_initialized` | `handleMcpServerInitialized` | `serverName` |
| `_kiro.dev/mcp/server_init_failure` | `handleMcpServerInitFailure` | `serverName`, `error` |
| `_kiro.dev/mcp/oauth_request` | `handleMcpOauthRequest` | `serverName`, `oauthUrl` |
| `_kiro.dev/mcp/governance_disabled` | `handleMcpGovernanceDisabled` | `apiFailure` (boolean; defaults to `false`) |
| `_kiro.dev/error/rate_limit` | `handleRateLimitError` | `message` |
| `_kiro.dev/subagent/list_update` | `handleSubagentListUpdate` | `subagents[]`, `pendingStages[]` |
| `_kiro.dev/session/inbox_notification` | `handleInboxNotification` | **opaque** — handler is `(e) => this.inboxHandlers.forEach(h => h(e))`; whole params object is forwarded to subscribers without destructuring. Empirically observed fields: `sessionId`, `sessionName`, `messageCount`, `escalationCount`, `senders[]`. |
| `_kiro.dev/session/list_update` | `handleSessionListUpdate` | `sessions[]` |
| `_kiro.dev/session/activity` | `handleSessionActivity` | `sessionId`, `event` |
| `_kiro.dev/session/update` (tool_call_chunk variant) | `handleExtSessionUpdate` | `update.{sessionUpdate, toolCallId, title, kind}`, `sessionId` |
| `_kiro.dev/session/update` (retry_warning variant) | `handleExtSessionUpdate` | `update.{sessionUpdate, attempt, maxAttempts, delaySecs, message}` |

**Findings beyond the documentation above:**

- **`clear/status` carries no readable params on the wire.** The handler ignores any payload. If a future feature needs to communicate clear status detail, the wire would need updating *and* the handler. Cyril's parser at `convert/kiro.rs:215` reads `params.message`, which the TUI drops — verify with a live capture before relying on it.
- **`retry_warning` field shape is now documented**: `attempt`, `maxAttempts`, `delaySecs`, `message`. This was previously listed in § 5 as "not in the 0.5.1 SDK" without field detail. Cyril doesn't have a handler for retry_warning yet (coverage doc Tier 2).
- **`metadata.effort` confirmed** at the handler-read level (not just empirical observation): `handleMetadataUpdate` reads `e.effort` explicitly and emits an `effort_update` stream event. This matches the empirical capture in § 11.4 and confirms the field is part of the TUI's expected schema, not just a sometimes-present field.
- **`mcp/governance_disabled.apiFailure`** confirmed as the only field with a `false` default when absent. Matches § 11.2.

**What this verification does and doesn't cover:**

- ✅ **Field NAMES** for each notification handler are taken directly from the TUI's destructuring code.
- ✅ **Default values** observable in the handler (e.g., `??""`, `??false`, `??null`) are recorded.
- ⚠ **Types** are partial — the handler accepts whatever JavaScript value is at each field. TypeScript types come from the upstream `@agentclientprotocol/sdk` for standard methods; Kiro extensions don't have published schemas, so types are inferred from usage.
- ⚠ **Field VALUE constraints** (enums, regex patterns, etc.) require reading downstream consumers of `broadcastStreamEvent`, which is more work than the handler alone.

To re-verify after a future Kiro release:

```sh
TUI=~/.local/share/kiro-research/tui-bundles/kiro-tui-<ver>.js
for h in handleCommandsAdvertising handleMetadataUpdate handleCompactionStatus \
         handleClearStatus handleAgentSwitched handleAgentNotFound \
         handleAgentConfigError handleMcpServerInitialized \
         handleMcpServerInitFailure handleMcpOauthRequest \
         handleMcpGovernanceDisabled handleRateLimitError \
         handleSubagentListUpdate handleInboxNotification \
         handleSessionListUpdate handleSessionActivity handleExtSessionUpdate; do
  echo "--- $h ---"
  # The handler retains its name through the bundler; extract function body with brace matching
  python3 -c "
import re, sys
with open('$TUI') as f: data = f.read()
m = re.search(rf'$h\s*\([^)]*\)\s*\{{', data)
if not m: print('NOT FOUND'); sys.exit(0)
start = m.start(); depth = 0
for i, c in enumerate(data[start:start+3000]):
  if c == '{': depth += 1
  elif c == '}':
    depth -= 1
    if depth == 0: print(data[start:start+i+1]); break
"
done
```

Diff against the prior version's output to find handler signature changes.

### 11.10 Sender-side (C→S) field verification (2.4.1)

Companion to § 11.9 — for each Kiro extension that the client *sends* to the agent, read the sender function body and transcribe the exact params object it constructs. This is the ground truth for "what the wire request looks like" — anything not in the params here doesn't reach the agent.

Same methodology as § 11.9: sender function names retain their human-readable identifiers in the bundler (`executeCommand`, `terminateSession`, etc.). Multiple functions share these names (mocks, stream wrappers), so the verification filters for "body contains `extMethod` call" to find the real sender.

| Method | Sender function | Params sent |
|---|---|---|
| `_kiro.dev/commands/execute` | `executeCommand(e)` | `{sessionId, command: e}` — `e` is the full `TuiCommand` object `{command, args}`; the doc's note about the wrap-in-object requirement in § 7 is canonical |
| `_kiro.dev/commands/options` | `getCommandOptions(e, t)` | `{sessionId, command: e.replace(/^\//, ""), partial: t}` — **strips leading `/`** from the command name before sending. Clients passing `/model` get `"model"` on the wire. |
| `_kiro.dev/settings/list` | `listSettings()` | `{}` — confirmed empty (§ 11.2 stated non-empty hangs the agent; this is the canonical empty-params sender) |
| `_kiro.dev/session/list` | `listSessions(e)` | `{cwd: e}` |
| `_kiro.dev/session/terminate` | `terminateSession(e)` | `{sessionId: e}` — **best-effort**: wrapped in try/catch, response is discarded, failures are logged but not propagated |
| `_session/spawn` | `spawnSession(e, t)` | `{sessionId: this.sessionId, task: e, name: t}` — **no mode/agent parameter**. Confirms § 7 and § 11.5: the spawned subagent inherits the parent's mode. Returns `{sessionId, name}` (with `name ?? t ?? ""` as fallback). |
| `_message/send` | `sendMessage(e, t)` | `{sessionId: e, content: t}` — **positional signature**: `e` is sessionId, `t` is content. `content` is opaque per the SDK (typically string or `ContentBlock[]`). Response is discarded. |

**Findings beyond the per-method documentation above:**

- **`commands/options` slash-stripping is canonical** behavior, not optional. Any future client implementation must strip the leading `/` from the command name before sending. The doc's prior note ("TUI strips '/'") is confirmed at the sender code level.
- **`terminateSession` is fire-and-forget by design.** The sender doesn't propagate errors — cyril's `BridgeCommand::TerminateSession` matches this semantics (logs and continues on failure). Any client expecting a strong delivery guarantee for termination needs to check separately (e.g., await the next `subagent/list_update` to confirm removal).
- **`spawnSession` returns `{sessionId, name}` only.** No other response fields are exposed. Anything the agent returns beyond those two keys is dropped by the sender; the spawned subagent's subsequent state arrives via `subagent/list_update` notifications, not the spawn response.
- **`sendMessage` response is discarded.** Same fire-and-forget pattern as terminate. Per the SDK, the response is `unknown` anyway.
- **No `_kiro.dev/settings/set` sender exists in 2.4.1.** Confirms § 11.2's "dead surface" claim — the method name is in the constants table but no function calls `extMethod` with that method. If cyril ever surfaces a settings-edit UX, write the JSON file directly per § 11.2; do not attempt `settings/set`.

To re-verify after a future Kiro release, the extraction filter is "function whose body contains `extMethod`":

```sh
TUI=~/.local/share/kiro-research/tui-bundles/kiro-tui-<ver>.js
python3 -c "
import re
with open('$TUI') as f: data = f.read()
for name in ['executeCommand','getCommandOptions','listSettings','listSessions',
             'terminateSession','spawnSession','sendMessage']:
    for m in re.finditer(rf'(async\s+)?\b{name}\s*\([^)]*\)\s*\{{', data):
        start = m.start(); depth = 0
        for i, c in enumerate(data[start:start+3000]):
            if c == '{': depth += 1
            elif c == '}':
                depth -= 1
                if depth == 0:
                    body = data[start:start+i+1]
                    if 'extMethod' in body:
                        print(f'--- {name} ---\n{body[:800]}\n')
                    break
        if 'extMethod' in (body if 'body' in dir() else ''): break
"
```

Diff against the prior version's output to catch sender-signature changes.

### 11.11 Empirical wire-type verification (2.4.1 captures)

§§ 11.9 and 11.10 verified field NAMES via handler/sender source reading. JavaScript is dynamically typed — the handler reads `e.field` but doesn't tell us whether `field` is a string or a number or an enum. This subsection adds the third verification layer: empirical TYPES extracted from actual on-the-wire JSON-RPC frames.

Methodology: union all Kiro-extension JSON-RPC frames from our capture artifacts (`experiments/conductor-spike/logs/conductor-2.4.1*.log`, `experiments/conductor-spike/trace-2.4.1-tui-recorder.jsonl`, `/tmp/conductor-spike/logs-241/*.log`, `/tmp/kiro-proxy-poc/messages-rs.jsonl`). For each method's params, record the observed JSON type per field, sample-set for string-typed fields (to spot enums), and presence rate (to spot optional fields).

#### Methods with empirical type data

| Method | Frames | Verified type-shape |
|---|---|---|
| `_kiro.dev/commands/available` | 33 | `{sessionId: string, commands: KiroCommand[], prompts: Prompt[], tools: Tool[], mcpServers: object[]}`. `commands[]` entries: `{name: string, description: string, meta?: {inputType: string, hint: string, optionsMethod?: string, subcommands?: string[], subcommandHints?: object}}`. `prompts[]` entries: `{name: string, description: string\|null, arguments: array, serverName: string}`. `tools[]` entries: `{name: string, description: string, source: string}` (source observed: `"built-in"`). |
| `_kiro.dev/commands/execute` | 160 | `{sessionId: string, command: {command: string, args: object}}`. **`command.args.value` is canonically `string`** — the integer instances in our captures came from a buggy probe (`{value: 0}` instead of `{value: "0"}`) which silently hung the agent. |
| `_kiro.dev/commands/options` | 134 | `{sessionId: string, command: string, partial: string}`. Observed `command` values (5): `agent`, `chat`, `effort`, `model`, `prompts`. `partial` was always empty `""` in our captures. |
| `_kiro.dev/compaction/status` | 8 | `{sessionId: string, status: {type: string, error?: string}, summary: string\|null}`. **`status.type` enum**: `"started"` \| `"completed"` \| (per § 6: `"failed"`, not in our captures). **`summary` is `null` when `status.type === "started"`, `string` when `"completed"`** — top-level, not nested under status. |
| `_kiro.dev/metadata` | 226 | All fields optional except `sessionId`. `{sessionId: string, contextUsagePercentage?: number, effort?: string, meteringUsage?: {unit: string, unitPlural: string, value: number}[], turnDurationMs?: integer}`. **`effort` enum (model-conditional)**: `"low"` \| `"medium"` \| `"high"` \| `"xhigh"` \| `"max"` (4 observed; `"low"` documented but not seen). `meteringUsage[].unit` observed: `"credit"`. **Bare `{sessionId}` frames are valid** (keep-alives). |
| `_kiro.dev/session/inbox_notification` | 4 | `{sessionId: string, sessionName: string, messageCount: integer, escalationCount: integer, senders: string[]}`. All fields present in every frame. `sessionName` observed: `"main"`. `senders` observed: `["subagent"]`. |
| `_kiro.dev/session/list` | 1 | Request: `{cwd: string}`. Response shape from § 7.X / § 11.x: `{sessions: SessionInfo[]}`. |
| `_session/spawn` | 13 | Request: `{sessionId: string, task: string, name?: string}`. **`name` is genuinely optional** — when omitted, Kiro auto-generates one (observed: `"Lancelot"`). Response: `{sessionId: string, name: string}` — `name` is always present, either echoed from request or auto-generated. **`role: null` empirically confirmed** for client-spawned subagents (`subagent/list_update` entry shows `role=None`, `agentName=kiro_default`). No `mode`/`agent`/`mode_id` field is honored. |
| `_kiro.dev/session/terminate` | 4 | `{sessionId: string}`. Response `{}`. |
| `_kiro.dev/session/update` (tool_call_chunk only) | 158 | `{sessionId: string, update: {sessionUpdate: "tool_call_chunk", toolCallId: string, title: string, kind: string}}`. **`update.kind` enum** observed: `"read"`, `"search"`, `"execute"`, `"other"`. **`update.title` observed**: `"read"`, `"grep"`, `"shell"`, `"code"`, `"web_search"`, `"summary"` — note `"summary"` is what shows up for the parent-agent-emits-on-subagent-completion `Summarizing` tool_call (§ 11.5). |
| `_kiro.dev/settings/list` | 5 | Canonical request: `{}` (empty). Probes that sent `{sessionId}` hung — confirmed silent rejection of non-empty params (§ 11.2). |
| `_kiro.dev/subagent/list_update` | 51 | `{subagents: SubagentInfo[], pendingStages: array}`. **`SubagentInfo`**: `{sessionId: string, sessionName: string, agentName: string, initialQuery: string, role: null, group: string, dependsOn: array, status: {type: string, message?: string}}`. **`subagents[].status.type` enum**: `"working"`, `"awaitingInstruction"` (2 observed; others likely exist). `agentName` observed: `"kiro_default"`. `role` is consistently `null` in our probes; was a string in older Kiro per § 6 — possibly retired. `group` observed: `"default"`. |

#### Multi-subagent crew capture (2026-05-23)

A real `/agent review-orchestrator` session ran a 4-stage `subagent` tool crew with `KIRO_ACP_RECORD_PATH` enabled, hit rate-limit retries, and went through full subagent lifecycle. Saved as `experiments/conductor-spike/trace-2.4.1-multi-subagent.jsonl` (1099 frames, ~1.4 MB). Yielded empirical types for **6 additional methods/variants**:

| Method | Frames | Verified type-shape |
|---|---|---|
| `_kiro.dev/error/rate_limit` | 1 | `{sessionId: string, message: string}`. `sessionId` was a **subagent's** id, not the main — rate limits surface per-session. `message`: "Rate limit exceeded. Please wait a moment before trying again." |
| `session/update.retry_warning` | 26 | `{sessionUpdate: "retry_warning", attempt: integer, maxAttempts: integer, delaySecs: integer, message: string}`. `attempt` observed: `2`. `maxAttempts` observed: `3`. `delaySecs` observed: `8`, `10`. `message`: "Retrying in 8s (attempt 2/3)". Fires on the subagent's session/update stream when a rate-limit retry kicks in. |
| `_kiro.dev/agent/switched` | 1 | **Schema now includes `welcomeMessage: string \| null`** — was always string before. When switching to `review-orchestrator` (a custom mode), `welcomeMessage` was `null`. Other fields unchanged: `{sessionId, agentName, previousAgentName, welcomeMessage: string\|null, model: string}`. |
| `_kiro.dev/subagent/list_update` (extended) | 41 | **New enum value**: `status.type` includes `"terminated"` (23 occurrences in this trace). Full enum so far: `"working" \| "awaitingInstruction" \| "terminated"`. **`role` field is non-null for agent-initiated subagents** (`"code-reviewer"`, `"silent-failure-hunter"`, `"type-design-analyzer"`, `"pr-test-analyzer"` observed) — confirms client-vs-agent spawn asymmetry. `group` format: `"crew-<task-description-prefix>"`. `pendingStages` stayed `[]` even with 4 concurrent stages — likely only populates when stages have unresolved `dependsOn` dependencies. |
| `session/update` variants on subagents | 1352+ | Each subagent's own sessionId carries a full session/update stream: `agent_message_chunk`, `tool_call`, `tool_call_update`. Confirms subagents are first-class ACP sessions with their own streams (not just "annotations" on the parent stream). |
| `_kiro.dev/session/inbox_notification` (multi) | 3 | Confirmed `messageCount` is a **running counter** of reports-from-subagents, not "currently unread." Values went 1 → 2 → 3 as each subagent terminated in sequence. |

**The agent's `subagent` tool — wire shape confirmed:**

When the parent agent (when configured with the subagent tool, e.g., under `review-orchestrator` mode) invokes it, the wire emits a `session/update` of variant `tool_call` with:

```json
{
  "sessionUpdate": "tool_call",
  "toolCallId": "tooluse_...",
  "title": "Spawning agent crew",
  "rawInput": {
    "__tool_use_purpose": "...",
    "task": "<high-level task description>",
    "mode": "blocking",                         // observed: "blocking"; "background" mentioned in older docs
    "stages": [
      {
        "name": "code-reviewer",                // identifier (used for crew monitor + correlation)
        "role": "code-reviewer",                // mode/role to spawn the subagent as
        "prompt_template": "Review the code..." // instruction for that subagent
        // optional: "depends_on": [string[]]   // snake_case in the stage spec, not present in our parallel-stages capture
      },
      ...
    ]
  }
}
```

**Stage spec uses snake_case**: `prompt_template` and `depends_on` (when present), not camelCase. Contrasts with the `subagent/list_update.subagents[].dependsOn` (camelCase) — Kiro normalizes between the tool-spec format and the notification format.

This is the **only canonical path to role-specialized subagents**. The client-side `_session/spawn` request (§ 7) does NOT support a `role` field; spawned-from-client subagents inherit the parent's mode. Spawned-from-agent (via this tool) subagents get the `role` from the stage spec.

**Crucial: `role` values reference custom agents defined in `.kiro/agents/<role>.json` on disk** — they're not built-in identifiers. In this capture the 4 reviewer roles (`code-reviewer`, `silent-failure-hunter`, `type-design-analyzer`, `pr-test-analyzer`) all map to JSON files in the user's repo at `.kiro/agents/`. Each file defines the subagent's prompt, tool allowlist, and resource bindings. The parent agent (`review-orchestrator`, also a custom agent) has the `subagent` tool in its allowed-tools list and a prompt instructing it to invoke the tool with the four reviewer stages.

This means the role-specialization capability is **config-driven, not wire-driven**:

- Wire layer: `subagent` tool with `stages[].role: "<name>"`
- Disk layer: `.kiro/agents/<name>.json` defines what `<name>` means
- Agent layer: an orchestrator-mode prompt has to invoke the tool with the right stages (the LLM has to comply)

For a workflow runner: cyril can write/manage `.kiro/agents/*.json` files and switch to a configured orchestrator mode, but ultimately depends on the LLM invoking the subagent tool. The workflow engine doesn't have a wire-level "spawn this role" command — it has "configure agents on disk, then ask the orchestrator to spawn them."

Custom agents seen in this repo (provenance for the captured `role` values):

```
.kiro/agents/code-reviewer.json
.kiro/agents/code-simplifier.json
.kiro/agents/comment-analyzer.json
.kiro/agents/pr-test-analyzer.json
.kiro/agents/review-orchestrator.json   ← has the `subagent` tool
.kiro/agents/silent-failure-hunter.json
.kiro/agents/type-design-analyzer.json
```

Cyril's `convert/kiro.rs` already parses `agentName` and `role` on `subagent/list_update` entries; consumers (cyril's crew panel, the workflow engine when built) should treat `role` as a free-form string that maps to user-defined agent identities, not a Kiro-defined enum.

#### Confirmed: 3 methods are DORMANT in 2.4.1

Despite intense activity (4 parallel subagents, rate limits, retries, agent switching) the following methods that have handlers in tui.js **never fired**:

| Method | Capture frames | Status |
|---|---|---|
| `_kiro.dev/session/activity` | 0 | **Dormant.** Handler exists; trigger condition unknown or absent in 2.4.1. Across all our captures (~10k frames including this multi-subagent run), zero occurrences. |
| `_kiro.dev/session/list_update` | 0 | **Dormant.** Same as above. Possibly intended for multi-client scenarios that we don't reproduce. |
| `_kiro.dev/agent/not_found` | 0 | **Dormant.** Invalid `/agent` returns `{success: false, message: "Unknown agent: ..."}` in the `commands/execute` response — no separate notification. The doc-described "Kiro fell back to another" behavior is not what 2.4.1 does. |

The cyril handlers for these are defensive — same level as tui.js's. Not strictly dead code but never exercised. If 2.5.x or later starts firing them, we'll find out.

#### Additional methods verified by the gap-fill probe (2026-05-23)

A targeted probe (saved as `experiments/conductor-spike/logs/conductor-2.4.1-gap-probe.log`) triggered `/agent swap`, `/agent <invalid>`, `/clear`, `SpawnSession`, and `SendMessage` to expand coverage. New empirical-type confirmations:

| Method | Frames | Verified type-shape |
|---|---|---|
| `_kiro.dev/agent/switched` | 2 | `{sessionId: string, agentName: string, previousAgentName: string, welcomeMessage: string, model: string}`. All five fields present in every frame; matches § 6 documentation exactly. |
| `_kiro.dev/clear/status` | 2 | `{sessionId: string}` — **only sessionId**. No `message`, no `status`, no other fields. Matches the parameterless dispatch finding from § 11.9 (the handler ignores params anyway, but for completeness the wire does carry sessionId). |
| `_message/send` | 2 | `{sessionId: string, content: string}` — request only; response is `{}` and discarded per § 11.10. |

#### Methods confirmed to NOT fire under expected triggers

The probe attempted to elicit four notifications that the doc lists as expected behaviors. **They did not fire**:

| Method | Attempted trigger | What actually happened |
|---|---|---|
| `_kiro.dev/agent/not_found` | `/agent` with `value: "definitely-not-real-agent-xyz123"` | Kiro returned `{success: false, message: "Unknown agent: ... Run /agent to browse available agents."}` in the `commands/execute` response. **No separate `agent/not_found` notification was emitted.** Per § 6, this notification was documented as "user requested an agent that doesn't exist; Kiro fell back to another." On 2.4.1 the behavior is "Kiro rejects the swap and tells you via the command response." The notification's handler exists but may be dormant. |
| `_kiro.dev/session/activity` | Subagent spawn + send-message round trip | Never observed. Across all our captures (~thousands of frames), zero `session/activity` notifications. Likely dormant. |
| `_kiro.dev/session/list_update` | Subagent activity, agent switching, /chat options | Never observed. Same — zero across all captures. Likely dormant. |

These three methods are **handler-only**: the TUI has dispatch code for them but no observed wire fire. Possible causes:
- Defensive reservation for future use (handlers were added speculatively)
- Methods that only fire under conditions we haven't reproduced (multi-client scenarios, specific error states)
- Effectively removed but handler kept defensively (cf. `agent/config_error` after 2.3.0)

For cyril's current `convert/kiro.rs` handlers — these are NOT dead code in the same sense as `model/not_found` (§ 11.3), because tui.js still has the handlers. But they're not exercised on the wire in normal usage either. If cyril wants to be defensive against future Kiro firing them, keep the handlers; if cyril wants to minimize unexercised code, the handlers can be removed and re-added if/when the methods are confirmed live.

#### Methods still without empirical data (5 — infrastructure-dependent)

| Method | Trigger required | Best documentation we have |
|---|---|---|
| `_kiro.dev/error/rate_limit` | Exceed Kiro account quota | § 6 + handler (§ 11.9) |
| `_kiro.dev/mcp/server_initialized` | Configure MCP server | § 6 + handler |
| `_kiro.dev/mcp/server_init_failure` | Misconfigure MCP server | § 6 + handler |
| `_kiro.dev/mcp/oauth_request` | OAuth-requiring MCP server | § 6 + handler |
| `_kiro.dev/mcp/governance_disabled` | Disable MCP governance | § 11.2 + handler |

To extend coverage, configure MCP servers (`~/.kiro/mcp.json`) or work against a rate-limit-throttled session. These are not reproducible without external infrastructure.

For these, the §§ 11.9 and 11.10 source-extraction is the best we have. Empirical type verification would require capturing wire frames under each triggering condition.

#### What this verification establishes

- **For 12 of 28 Kiro extension methods**: every field's name, JSON type, and enum membership (where applicable) is empirically confirmed against captured frames.
- **Optionality is verified**: fields marked optional in this doc that appear in <100% of frames are genuinely sometimes-absent on the wire (not just defensively documented).
- **Enum values are observable**: for fields like `metadata.effort`, `update.kind`, `status.type`, the observed value set is documented. Unobserved values from § 6 are noted (e.g. `"low"` for effort, `"failed"` for compaction status) — they may exist but aren't proved by our captures.

#### Reproducibility

The script that produced this table:

```py
import json, re, glob
from collections import defaultdict
def js_type(v):
    if v is None: return 'null'
    if isinstance(v, bool): return 'boolean'
    if isinstance(v, int): return 'integer'
    if isinstance(v, float): return 'number'
    if isinstance(v, str): return 'string'
    if isinstance(v, list): return f'array<{js_type(v[0]) if v else ""}>'
    if isinstance(v, dict): return 'object'
schemas = defaultdict(lambda: defaultdict(lambda: {'types': set(), 'samples': set()}))
def walk(method, prefix, obj):
    if isinstance(obj, dict):
        for k, v in obj.items():
            path = f'{prefix}.{k}' if prefix else k
            schemas[method][path]['types'].add(js_type(v))
            if isinstance(v, str) and len(v) < 60: schemas[method][path]['samples'].add(v)
            if isinstance(v, dict): walk(method, path, v)
            if isinstance(v, list) and v and isinstance(v[0], dict): walk(method, f'{path}[]', v[0])
sources = (glob.glob('experiments/conductor-spike/logs/*.log') +
           glob.glob('experiments/conductor-spike/trace-*.jsonl') +
           glob.glob('/tmp/conductor-spike/logs-241/*.log'))
for src in sources:
    for line in open(src):
        try: obj = json.loads(line.strip())
        except:
            m = re.search(r'(\{.*\})$', line); obj = json.loads(m.group(1)) if m else None
        if not obj: continue
        msg = obj.get('msg', obj)
        if not isinstance(msg, dict): continue
        method = msg.get('method', '')
        if 'kiro.dev/' not in method and method not in ('session/spawn','session/terminate','session/list','message/send'): continue
        walk(method, '', msg.get('params', {}))
# Print schemas[method] for each method
```

To extend coverage to the 11 currently-unobserved methods, run the test harness under specific conditions (MCP-configured, rate-limited, etc.) and re-run.

### 11.12 Tarball additions (2.4.0+)

Two new shell shims:

- `bin/q` (`sh -c '"$HOME/.local/bin/kiro-cli" --show-legacy-warning "$@"'`)
- `bin/qchat` (same, with `chat` inserted before `"$@"`)

Backward-compat entry points for Amazon Q legacy users. New `--show-legacy-warning` flag on `kiro-cli` prints a deprecation notice.
