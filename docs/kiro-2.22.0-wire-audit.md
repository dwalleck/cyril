# kiro-cli 2.22.0 KAS ACP deserialization audit

**Audit status:** focused deserialization audit, not a complete 2.22.0 release
wire audit. The primary 2.22.0 evidence is the committed compact findings
summary; the raw 2.22 JSONL is not repository-retained. Redacted JSON examples
below either use committed captures—`kas-agentsmd-2.18.0.jsonl`,
`kas-content-chunk-2.19.1.jsonl`, and `kas-baseline-live-2.21.0.jsonl`—or are
explicitly labeled field-level reconstructions from the committed 2.22.0
findings summary.

**Conclusion:** Cyril currently preserves the ordinary ACP conversation and
tool lifecycle well enough to render the main turn, and it now parses
permission trust options. The current model is nevertheless lossy in several
useful places: KAS MCP server/catalog notifications have no domain types,
several KAS `session_info_update` kinds are discarded after successful ACP
serde deserialization, permission metadata is projected away, and non-text
tool-call content variants are dropped.

The captured `aws-mcp` server ended in `status: "failed"` with
`errorMessage: "Connection closed"`. The successful MCP catalog and tool
lifecycle in this capture were for `aws-docs`; this document does not claim a
successful `aws-mcp` ACP tool call.

## 1. Evidence and protocol boundary

The committed compact 2.22.0 evidence is
[`kas-mcp-2.22.0-credit-findings.json`](../experiments/conductor-spike/logs/kas-mcp-2.22.0-credit-findings.json).
It records the 2.22.0 MCP status/catalog field inventory, the failed
`aws-mcp` status, and the metered run's summarized permission/tool lifecycle.
The raw 2.22 JSONL captures were retained during probing under a local
temporary directory but are not committed. Every example below is provenance-
labeled: either a hand-redacted excerpt from the committed 2.18.0, 2.19.1, and
2.21.0 captures, or a field-level reconstruction from the committed 2.22.0
findings summary. Examples do not copy prompts, arguments, paths, URIs,
session IDs, or credential-shaped values.

The authoritative KAS type inventory is
[`kiro-kas-acp-covenant.md`](kiro-kas-acp-covenant.md), especially §§ 1c, 4,
and 9. The current Rust comparison is against these declarations and paths:

- `crates/cyril-core/src/types/event.rs`
- `crates/cyril-core/src/types/tool_call.rs`
- `crates/cyril-core/src/protocol/convert/mod.rs`
- `crates/cyril-core/src/protocol/convert/kas.rs`
- `crates/cyril-core/src/protocol/convert/kiro.rs`
- `crates/cyril-core/src/protocol/engine.rs`
- `crates/cyril-core/src/protocol/domain_mediator/mod.rs`
- `crates/cyril-core/src/protocol/domain_mediator/inbound.rs`

There are two distinct wire boundaries:

1. ACP-facing KAS messages, which Cyril must deserialize or deliberately
   preserve. These include `_kiro/mcp/status`, `_kiro/tools/didChange`, and
   KAS data carried inside standard `session/update`.
2. Downstream MCP JSON-RPC exchanged by Kiro or a proxy with an MCP server.
   That traffic is not emitted as ACP frames to Cyril and should not be added
   to Cyril's ACP parser merely because its results originated in MCP.

The ACP library strips the leading underscore from extension method names before
engine dispatch. Examples below use the on-wire spelling (`_kiro/...`); the
KAS engine's extension converter receives the normalized spelling
(`kiro/...`).

## 2. Field-level comparison

| Wire family | ACP serde representation | Cyril projection | Preserved | Currently lossy or absent |
|---|---|---|---|---|
| `tool_call` | `acp::ToolCall` | `Notification::ToolCallStarted(ToolCall)` | ID, title, kind, status, `rawInput`, `rawOutput`, locations, diff/text content | Non-diff/non-text content variants; unmodeled tool metadata |
| `tool_call_update` | `acp::ToolCallUpdate` | `Notification::ToolCallUpdated(ToolCall)` then `ToolCall::merge_update` | Present title/kind/status/input/output/content/locations; omitted update fields retain prior values | Non-diff/non-text content variants; unmodeled update metadata |
| `_kiro/tools/content_chunk` | Raw `UntypedMessage` / JSON params | No `Notification` or domain type | Nothing beyond transient raw params | Streaming tool output, session/tool IDs, and chunk content |
| `session/request_permission` | `acp::RequestPermissionRequest` | `PermissionRequest` through `DomainWork::Permission` | Session ID, reconstructed tool call, title-derived message, option ID/name/kind, parsed `_meta.trustOptions[]` | Original wire request is not retained; arbitrary `_meta` is not retained; 2.19.1-observed KAS `consent`, `consentRound`, `toolId`, and `command` are lost |
| KAS `session_info_update` | Known `acp::SessionInfoUpdate`; KAS payload in `meta["kiro"]` | Selected `Notification` variants from `session_info_to_notification` | `hook_update`, `turn_completion`, `turn_end`, `context_usage`, steering echo kinds | Unsupported KAS `kind` values deserialize successfully, then return `None` |
| `_kiro/mcp/status` | Raw `UntypedMessage` / JSON params | No `Notification` or domain type | Nothing beyond transient raw params | Server status, failure detail, registry configuration, access mode, resources, prompts, tools, schemas |
| `_kiro/tools/didChange` | Raw `UntypedMessage` / JSON params | No `Notification` or domain type | Nothing | Tool tags, source, descriptions, and session association |
| `_kiro/governance/state` | Raw `UntypedMessage` / JSON params | No current notification type | Nothing | Enterprise state, feature gates, MCP/web-tools policy, disabled reason |
| `_kiro/steering/documents_changed` | Raw `UntypedMessage` / JSON params | No current notification type | Nothing | Steering document names, URIs, content/inclusion metadata |
| `_kiro/sessions/changed` | Raw `UntypedMessage` / JSON params | No current notification type | Nothing | Session additions/deletions and update timestamps |
| `_kiro/progressive_context/items_changed` | Raw `UntypedMessage` / JSON params | No current notification type | Nothing | Progressive-context item catalog and scope metadata |
| `_kiro/powers/items_changed` | Raw JSON params, normalized by `convert::kas::powers` | `Notification::PowersChanged { powers: Vec<PowerInfo> }` | Power name/display name/description, MCP server names, steering-file marker | `sessionId`, `status`, `keywords`, `isAgentPlugin`, and per-power `_meta` are ignored |

## 3. Standard ACP tool calls

The generic converter does have substantial coverage. `to_tool_call` copies
these fields into the domain object:

```rust
ToolCall {
    id,
    title,
    kind,
    status,
    raw_input,
    raw_output,
    content,
    locations,
}
```

Relevant declarations and constructors are in
`crates/cyril-core/src/types/tool_call.rs:65-119` and
`crates/cyril-core/src/protocol/convert/mod.rs:152-168`.

A redacted standard tool-call excerpt from committed
`kas-content-chunk-2.19.1.jsonl`:

```json
{
  "method": "session/update",
  "params": {
    "sessionId": "<redacted>",
    "update": {
      "sessionUpdate": "tool_call",
      "toolCallId": "<redacted>",
      "title": "Run Command",
      "kind": "execute",
      "status": "pending",
      "rawInput": {
        "command": "<redacted>",
        "run_in_background": false
      },
      "_meta": {
        "kiro": { "toolOrigin": "default" }
      }
    }
  }
}
```

This committed example is a non-MCP execute-tool call; it verifies generic
ACP field projection without exposing the original command. The 2.22 findings
summary separately records the MCP variant's tool catalog and lifecycle, but
the raw 2.22 MCP tool frame is not repository-retained.

The corresponding update can omit fields that were present on the initial
call:

```json
{
  "method": "session/update",
  "params": {
    "sessionId": "<redacted>",
    "update": {
      "sessionUpdate": "tool_call_update",
      "toolCallId": "<redacted>",
      "status": "in_progress",
      "rawInput": {
        "command": "<redacted>",
        "run_in_background": false
      },
      "_meta": {
        "kiro": { "toolOrigin": "default" }
      }
    }
  }
}
```

`ToolCall::merge_update_with_presence` protects omitted `kind` and `status`
fields and preserves prior non-empty title/input/output/content/locations.
However, `convert_tool_call_content` uses `filter_map` and only converts ACP
diff and text content. Other ACP content variants are currently discarded at
`crates/cyril-core/src/protocol/convert/mod.rs:171-188`.

## 3a. Prior capture: KAS streaming tool output

This section is outside the 2.22.0 MCP captures. The committed
`kas-content-chunk-2.19.1.jsonl` capture contains six `_kiro/tools/content_chunk`
notifications while a shell command was running. A minimal redacted frame is:

```json
{
  "method": "_kiro/tools/content_chunk",
  "params": {
    "sessionId": "<redacted>",
    "toolCallId": "<redacted>",
    "content": {
      "type": "content",
      "content": {
        "type": "text",
        "text": "<redacted streamed output chunk>"
      }
    }
  }
}
```

The current `ToolCallContent::Text` type could represent the nested text, but
there is no `_kiro/tools/content_chunk` converter arm or notification variant.
The existing `Notification::ToolCallChunk` is a different
`kiro.dev/session/update` carrier and does not consume this message. Cyril
therefore currently loses the session/tool association and every streamed
chunk. The 2.21.2 audit records the same gap and the related append-versus-
replace behavior at `docs/kiro-2.21.2-wire-audit.md:612-621`.

This is a prior-capture finding, not evidence that `_kiro/tools/content_chunk`
was present in the 2.22.0 MCP runs.

## 4. Permission requests: useful data retained and lost

The following is an older-version committed example from KAS 2.19.1
(`kas-content-chunk-2.19.1.jsonl`), not a 2.22.0 capture. The committed
2.22.0 findings summary records one `aws-docs` permission with `allow_once`
and its lifecycle, but does not record the permission `_meta` field shape; the
raw 2.22.0 permission frame is not repository-retained.
The object shape shown below is from that committed 2.19.1 frame only.

```json
{
  "method": "session/request_permission",
  "params": {
    "sessionId": "<redacted>",
    "toolCall": {
      "toolCallId": "<redacted>",
      "title": "Run Command",
      "status": "pending"
    },
    "options": [
      { "kind": "allow_once", "name": "Allow", "optionId": "<redacted>" },
      { "kind": "allow_always", "name": "Always allow", "optionId": "<redacted>" },
      { "kind": "reject_once", "name": "Deny", "optionId": "<redacted>" },
      { "kind": "reject_always", "name": "Always deny", "optionId": "<redacted>" }
    ],
    "_meta": {
      "kiro": {
        "toolId": "run_command",
        "command": "<redacted>",
        "consentRound": 1,
        "consent": {
          "capability": "<redacted>",
          "resource": "<redacted>",
          "askType": "<redacted>",
          "triggeringResource": "<redacted>",
          "workspaceRoot": "<redacted>"
        }
      }
    }
  }
}
```

The current domain declaration is
`crates/cyril-core/src/types/event.rs:481-502`:

```rust
PermissionRequest {
    session_id,
    tool_call,
    message,
    options,
    trust_options,
    responder,
}
```

Current behavior:

- `session_id` is retained.
- The tool call is reconstructed from the permission request and then enriched
  from `ToolCallLedger` only when both the request `sessionId` and `toolCallId`
  match a tracked call. Otherwise the permission request's stub is used. See
  `crates/cyril-core/src/protocol/domain_mediator/mod.rs:603-628`.
- `message` comes from the tool-call title, with a fallback when no title is
  present.
- Option `optionId`, `name`, and `kind` are converted into
  `PermissionOption`.
- `_meta.trustOptions[]`, when present, is parsed into `TrustOption` with
  `label`, `display`, `setting_key`, and `patterns`.
- The original wire request and arbitrary request `_meta` are not carried into
  the domain type. In the committed KAS 2.19.1 example, this includes
  `consent`, `consentRound`, `toolId`, and `command`; those values are therefore
  unavailable to UI or policy code after projection.

This is a partial typed projection, not a parse failure. The useful missing
question is whether a future approval/policy surface needs the captured KAS
consent context in addition to the existing trust tiers.

## 5. KAS `session_info_update`

KAS uses a known standard ACP variant as a multiplexer. The outer frame
successfully deserializes as `acp::SessionInfoUpdate`; the KAS discriminator is
inside `_meta.kiro.kind`.

The committed 2.18.0 and 2.21.0 captures contain the session-info examples
below. The 2.22.0 findings summary confirms the same KAS multiplexer family,
but the raw 2.22 JSONL is not repository-retained.

### Covered examples

A context update from the committed `kas-agentsmd-2.18.0.jsonl` capture:

```json
{
  "method": "session/update",
  "params": {
    "sessionId": "<redacted>",
    "update": {
      "sessionUpdate": "session_info_update",
      "_meta": {
        "kiro": {
          "kind": "context_usage",
          "usagePercentage": 0.9,
          "contextUsage": { "usagePercentage": 0.9 },
          "breakdown": {
            "contextFiles": { "percent": 0, "tokens": "<redacted>", "items": [] },
            "tools": {
              "percent": 0.5,
              "tokens": "<redacted>",
              "builtin": { "percent": 0.5, "tokens": "<redacted>" },
              "mcp": { "percent": 0, "tokens": "<redacted>" }
            },
            "kiroResponses": { "percent": 0, "tokens": "<redacted>" },
            "yourPrompts": { "percent": 0.4, "tokens": "<redacted>" },
            "sessionFiles": { "percent": 0, "tokens": "<redacted>", "items": [] }
          }
        }
      }
    }
  }
}
```
Cyril preserves the scalar percentage and the recognized five-bucket
breakdown as `ContextBreakdownUpdated`. A malformed breakdown degrades to
scalar-only; a missing or invalid percentage drops the frame.

`turn_completion` maps elapsed time, status, metering summaries, used tools,
and request IDs into `TurnMeteringUpdated`. `turn_end` maps `stopReason` into
`TurnCompleted`. These mappings are in
`crates/cyril-core/src/protocol/convert/kas.rs:309-366,393-478`.

### Currently dropped examples

The following examples all successfully deserialize as the known ACP
`SessionInfoUpdate`, but the KAS converter has no corresponding domain
notification. Provenance labels immediately above each block distinguish
committed historical examples from the local 2.22.0 findings reconstruction.
The `context_usage` block above is from the committed 2.18.0 capture.
The dropped-kind blocks below are not all from one release.

Committed KAS 2.19.1 capture; not a 2.22.0 capture:

```json
{
  "sessionUpdate": "session_info_update",
  "_meta": {
    "kiro": {
      "kind": "turn_start",
      "messageId": "<redacted>",
      "turnStart": true
    }
  }
}
```

Committed KAS 2.18.0 capture; not a 2.22.0 capture:

```json
{
  "sessionUpdate": "session_info_update",
  "_meta": {
    "kiro": {
      "kind": "user_message_id_assigned",
      "userMessageId": "<redacted>"
    }
  }
}
```

The committed 2.18.0 captures show only outer `title` and `_meta.kiro.title`
for `focus_update`. The additional `focus.status`, `activity`, and
`_meta.kiro.status` fields below come from local 2.22.0 probing; this combined
shape is not a committed 2.18.0 frame.
Local 2.22.0 findings/probe reconstruction; raw frame not repository-retained:

```json
{
  "sessionUpdate": "session_info_update",
  "title": "<redacted focus title>",
  "_meta": {
    "kiro": {
      "kind": "focus_update",
      "title": "<redacted focus title>",
      "focus": { "status": "in_progress" },
      "activity": {
        "turnActive": true,
        "runningWorkflows": 0,
        "pausedWorkflows": 0
      },
      "status": "in_progress"
    }
  }
}
```

Committed KAS 2.18.0 capture; not a 2.22.0 capture:

```json
{
  "sessionUpdate": "session_info_update",
  "_meta": {
    "kiro": {
      "kind": "steering_inclusion",
      "steeringDocuments": [
        "<redacted>",
        "<redacted>"
      ]
    }
  }
}
```

Committed KAS 2.19.1 capture; not a 2.22.0 capture:

```json
{
  "sessionUpdate": "session_info_update",
  "_meta": {
    "kiro": {
      "kind": "pending_interaction",
      "interactionType": "tool_approval",
      "question": "@aws-docs/search_documentation",
      "toolCallId": "<redacted>",
      "options": [
        { "kind": "allow_once", "name": "Allow", "optionId": "<redacted>" }
      ],
      "pendingInteraction": {
        "interactionType": "tool_approval",
        "question": "@aws-docs/search_documentation",
        "toolCallId": "<redacted>"
      }
    }
  }
}
```

Committed KAS 2.19.1 capture; not a 2.22.0 capture:

```json
{
  "sessionUpdate": "session_info_update",
  "_meta": {
    "kiro": {
      "kind": "interaction_resolved",
      "outcome": "selected",
      "selectedOption": "accept",
      "toolCallId": "<redacted>",
      "interactionResolved": {
        "outcome": "selected",
        "selectedOption": "accept",
        "toolCallId": "<redacted>"
      }
    }
  }
}
```

Local 2.22.0 findings/probe reconstruction; raw frame not repository-retained:

```json
{
  "sessionUpdate": "session_info_update",
  "_meta": {
    "kiro": {
      "kind": "display_error",
      "errorType": "mcp_connection_error",
      "message": "MCP server \"aws-mcp\" failed to connect: Connection closed",
      "displayError": {
        "errorType": "mcp_connection_error",
        "message": "MCP server \"aws-mcp\" failed to connect: Connection closed"
      }
    }
  }
}
```

These are not unknown ACP update tags. They are known ACP `session_info_update`
frames whose KAS sub-kind is unsupported. `session_info_to_notification`
currently handles `hook_update`, `turn_completion`, `turn_end`,
`context_usage`, and the three steering echo kinds; its fallback is `_ => None`:

```text
crates/cyril-core/src/protocol/convert/kas.rs:271-390
```

A lossless fallback for this family must preserve the original JSON or at least
the typed update's raw `meta` before this function returns `None`. An
extension-level `UntypedMessage` fallback alone cannot recover these frames.

## 6. KAS MCP server status and catalog

The committed 2.18.0 and 2.21.0 captures show the empty-catalog form:

```json
{
  "method": "_kiro/mcp/status",
  "params": {
    "sessionId": "<redacted>",
    "servers": []
  }
}
```

The committed 2.22.0 findings summary reports six status notifications and
their field inventory. The larger shape below is a minimal reconstruction from
that summary, not a copied frame from the uncommitted raw 2.22 JSONL:

Only server names/versions/enabled values, status and failure fields, catalog
entry keys, and schema key inventories are represented here; descriptions,
metadata values, and schema bodies are redacted or omitted.

All four registry/final server entries are shown below. The `aws-docs` tool
array contains one representative item; the findings summary records five
`aws-docs` tools, 26 `playwright` tools, and 40 `azure-devops` tools, but their
full objects are not retained.

```json
{
  "method": "_kiro/mcp/status",
  "params": {
    "sessionId": "<redacted>",
    "accessMode": "registry",
    "accessModeFilteredServers": ["<redacted>", "<redacted>"],
    "registryServers": [
      { "name": "aws-docs", "version": "1.1.24", "enabled": true },
      { "name": "playwright", "version": "<redacted>", "enabled": true },
      { "name": "aws-mcp", "version": "1.6.2", "enabled": true },
      { "name": "azure-devops", "version": "<redacted>", "enabled": true }
    ],
    "unresolvedRegistryServers": [],
    "servers": [
      {
        "name": "aws-docs",
        "status": "connected",
        "tools": [
          {
            "name": "search_documentation",
            "description": "<redacted>",
            "disabled": false,
            "inputSchema": { "type": "object" }
          }
        ]
      },
      { "name": "playwright", "status": "connected" },
      {
        "name": "aws-mcp",
        "status": "failed",
        "errorMessage": "Connection closed",
        "failedAuthorization": false
      },
      { "name": "azure-devops", "status": "connected" }
    ]
  }
}
```

Useful data that currently has no Cyril representation:

- whether MCP is registry- or workspace-driven (`accessMode`);
- enabled registry servers and versions;
- filtered and unresolved server names;
- connected, connecting, disabled, or failed state;
- `errorMessage` and `failedAuthorization` for an actionable failure panel;
- server provenance in `_meta.kiro.resource.source`;
- prompts, resources, resource templates, and the complete tool catalog;
- tool descriptions and disabled state;
- arbitrary JSON Schema in each tool's `inputSchema`.

The current Rust `McpServerInitFailure`, `McpOAuthRequest`, and
`McpServerInitialized` variants only correspond to older
`kiro.dev/mcp/*` extension methods. They do not model this KAS status union.
No status notification is created by `convert::kiro::to_ext_notification`; the
unknown extension result reaches the mediator's debug-only `Ok(None)` path at
`crates/cyril-core/src/protocol/domain_mediator/inbound.rs:170-188`.

## 7. KAS tool-tag catalog

The committed `kas-agentsmd-2.18.0.jsonl` capture contains the built-in tag
form:

```json
{
  "method": "_kiro/tools/didChange",
  "params": {
    "sessionId": "<redacted>",
    "tags": [
      { "source": "builtin", "tag": "read", "description": "read-file, diagnostics, search tools" },
      { "source": "builtin", "tag": "write", "description": "write-file tools" },
      { "source": "builtin", "tag": "shell", "description": "run-commands tools" },
      { "source": "builtin", "tag": "web", "description": "web search tools" }
    ]
  }
}
```

The 2.22.0 findings summary additionally records an MCP tag with
`source: "mcp"` and a tag of the form `@server/tool`; the following is a
minimal field-level reconstruction from that summary, not a copied raw
2.22 frame:

```json
{
  "method": "_kiro/tools/didChange",
  "params": {
    "sessionId": "<redacted>",
    "tags": [
      {
        "source": "builtin",
        "tag": "read",
        "description": "read-file, diagnostics, search tools"
      },
      {
        "source": "mcp",
        "tag": "@aws-docs/read_documentation",
        "description": "<redacted>"
      }
    ]
  }
}
```

The captured payload is `{sessionId, tags[]}`, with each tag containing
`source`, `tag`, and `description`. Cyril has no `ToolTag` type, no notification
variant, and no catalog state. This prevents the UI or policy layer from
knowing which MCP capability was added or removed, even though the tool call
itself may subsequently arrive.

## 8. Other extension messages and current type coverage

These examples use committed 2.18.0, 2.19.1, and 2.21.0 captures. The 2.22.0
findings summary supplies additional field inventories, but its raw JSONL is not
repository-retained. The historical 2.21.2 audit listed several of these
families as dropped; current source now handles powers, while the other listed
families still have no domain projection.

### Governance state: currently no domain projection
Committed KAS 2.18.0 capture; not a 2.22.0 capture:

```json
{
  "method": "_kiro/governance/state",
  "params": {
    "sessionId": "<redacted>",
    "isEnterprise": false,
    "features": {
      "mcpEnabled": true,
      "webToolsEnabled": true,
      "usageAnalytics": false,
      "contentCollection": true,
      "promptLogging": false,
      "codeReferenceTracker": false,
      "autonomousAgents": true
    }
  }
}
```

These feature gates could affect which UI actions are safe to offer. No
current `Notification` or governance state type receives them.

### Steering documents: currently no domain projection
Committed KAS 2.18.0 capture; not a 2.22.0 capture:

```json
{
  "method": "_kiro/steering/documents_changed",
  "params": {
    "sessionId": "<redacted>",
    "status": "success",
    "documents": [
      {
        "name": "<redacted>",
        "uri": "<redacted>",
        "content": "<redacted>",
        "inclusion": "<redacted>",
        "scope": "<redacted>",
        "type": "<redacted>",
        "_meta": { "kiro": "<redacted>" }
      }
    ]
  }
}
```

The captured item keys were `_meta`, `content`, `inclusion`, `name`, `scope`,
`type`, and `uri`. This could support a steering/context inspector, but the
current extension converter drops the frame.

### Session roster changes: currently no domain projection
Committed KAS 2.18.0 capture; not a 2.22.0 capture:

```json
{
  "method": "_kiro/sessions/changed",
  "params": {
    "upserted": [
      {
        "sessionId": "<redacted>",
        "source": "local",
        "executionTarget": { "kind": "local" },
        "cwd": "<redacted>",
        "title": "New Session",
        "updatedAt": "<redacted>",
        "status": "idle"
      }
    ],
    "deleted": []
  }
}
```

The roster carries source, execution-target, cwd, title, status, and update
time in addition to the session identifier. This is potentially useful for
multi-session/subagent UI, but it is separate from the existing subagent list
and stream types.

### Progressive context changes: currently no domain projection
Committed KAS 2.18.0 capture; not a 2.22.0 capture:

```json
{
  "method": "_kiro/progressive_context/items_changed",
  "params": {
    "sessionId": "<redacted>",
    "status": "success",
    "items": []
  }
}
```

The committed captures show empty item lists. The KAS covenant describes
non-empty items with `name`, `uri`, `description`, `scope`, `type`, and `_meta`.
This catalog is related to context selection, not the five-bucket
`context_usage` breakdown that Cyril already parses.

### Powers: currently partially covered

The committed captures show an empty catalog (`"powers": []`). The non-empty
shape below is the established KAS schema used by the current converter
fixtures, not a claim that these captures contained a `datadog` row.
Established converter-fixture shape; not observed in the committed 2.18.0 or
2.21.0 captures and not asserted as a raw 2.22.0 frame:

```json
{
  "method": "_kiro/powers/items_changed",
  "params": {
    "sessionId": "<redacted>",
    "status": "success",
    "powers": [
      {
        "name": "datadog",
        "displayName": "Datadog Observability",
        "description": "<redacted>",
        "keywords": ["datadog", "observability"],
        "mcpServerNames": ["datadog"],
        "hasSteeringFiles": true,
        "isAgentPlugin": false,
        "_meta": { "kiro": "<redacted>" }
      }
    ]
  }
}
```

`convert::kas::powers` maps the display-relevant fields into
`PowerInfo`/`Notification::PowersChanged { powers }`. It currently ignores
`sessionId`, `status`, `keywords`, `isAgentPlugin`, and `_meta`. That behavior
is documented in `crates/cyril-core/src/protocol/convert/kas/powers.rs:36-39,50-93`
and is not the same gap as MCP server status.

### Handled but not observed in these examples

These current paths are not part of the missing MCP/status model:

- `_kiro/powers/items_changed` is claimed by the KAS powers adapter and reaches
  `Notification::PowersChanged { powers }`; only non-display fields listed in
  the matrix are ignored.
- `_kiro/hooks/didChange` is special-cased by the inbound mediator and routed
  to the hooks host callback as `HooksChanged` when the hooks adapter is active.
  It is not a generic `Notification` converter arm.
- `_kiro/system/notify` is normalized to `kiro/system/notify` and converted by
  `convert::kiro::to_ext_notification` into `Notification::SystemNotify`.
- `_kiro/terminal/shell_type` is a request, not a notification; the current KAS
  host adapter answers it.

## 9. Fallback design consequence

There are two different lossless-fallback requirements:

```text
unknown _kiro/* extension
    raw UntypedMessage
    → extension converter
    → Ok(None) if no method arm

known ACP session_info_update with unknown KAS kind
    acp::SessionInfoUpdate
    → session_info_to_notification
    → None if _meta.kiro.kind has no arm
```

The first can preserve the raw `UntypedMessage` at extension dispatch. The
second requires preserving the original JSON or the typed update's raw metadata
alongside `acp::SessionInfoUpdate` before the KAS converter discards it. An
extension-only fallback cannot recover an unsupported `session_info_update`
sub-kind.

## 10. Recommended implementation order

1. Add typed KAS MCP server-status/catalog models. Keep each tool's
   `inputSchema` as `serde_json::Value` because it is arbitrary JSON Schema.
2. Add a typed KAS tool-tag catalog notification.
3. Decide which governance, steering-document, session-roster, and progressive
   context notifications have actual consumers before adding domain state.
4. Add typed notifications for useful observed `session_info_update` kinds:
   `display_error`, `pending_interaction`, `interaction_resolved`,
   `focus_update`, and `user_message_id_assigned` are the clearest immediate
   candidates.
5. Preserve unknown KAS `session_info_update` metadata as a raw fallback rather
   than silently discarding it.
6. Extend permission-domain metadata only if approval/policy UX needs KAS MCP
   identity or consent context; trust-option parsing already exists.
7. Add redacted fixture tests for each retained wire family before changing
   rendering or policy behavior.

The downstream MCP proxy's standard `initialize` exchange remains a separate
MCP transport observation. It is not evidence that Cyril needs to deserialize
MCP `tools/list` or `tools/call` directly through its ACP parser.
