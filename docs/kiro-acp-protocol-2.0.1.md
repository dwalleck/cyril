# Kiro CLI ACP Protocol Reference — v2.0.1

> **Source of truth:** extracted from `docs/kiro-tui-2.0.1.js` (the Kiro CLI TUI bundle). Every field, method, and type definition cites the line number of the Zod schema that defines it. Every claim can be verified by reading that line in the bundle.
>
> **Not derived from prior documentation.** This document was built from the bundle, not from the older `kiro-acp-protocol.md`.

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

### `kiro.dev/model/not_found`

User requested a model that doesn't exist; Kiro fell back to another.

Handler: `handleModelNotFound` (tui.js:124862).

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
  name?: string;                       // subagent mode/role (matches availableModes[].id)
}
```

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
