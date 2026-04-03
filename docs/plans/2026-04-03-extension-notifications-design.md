# Extension Notifications Design

Handle the remaining Kiro extension notifications that Cyril currently drops silently.

## Context

Cyril handles 6 of the 13 known `kiro.dev/*` extension notifications. The remaining 7 are logged as "unrecognized extension notification" warnings and dropped. Several of these are user-impacting — most notably `error/rate_limit`, which leaves the user with no explanation when the agent stops responding.

### Currently handled

| Extension method | Notification variant |
|---|---|
| `kiro.dev/metadata` | `ContextUsageUpdated` |
| `kiro.dev/compaction/status` | `CompactionStatus` |
| `kiro.dev/clear/status` | `ClearStatus` |
| `kiro.dev/agent/switched` | `AgentSwitched` |
| `kiro.dev/commands/available` | `CommandsUpdated` |
| `kiro.dev/session/update` | `ToolCallChunk` (tool_call_chunk variant only) |

### Missing (this design)

| Extension method | Priority | Action |
|---|---|---|
| `kiro.dev/error/rate_limit` | P1 | New variant, system message |
| `kiro.dev/mcp/server_init_failure` | P1 | New variant, system message |
| `kiro.dev/mcp/server_initialized` | P2 | New variant, system message |
| `kiro.dev/mcp/oauth_request` | P2 | New variant, open browser |
| `kiro.dev/subagent/list_update` | P3 | Acknowledge, do not forward |
| `kiro.dev/session/inbox_notification` | P3 | Acknowledge, do not forward |
| `kiro.dev/session/list_update` | P3 | Acknowledge, do not forward |

## Approach

Typed notification variants (Approach 1). Each extension that needs UI treatment gets its own `Notification` enum variant. Subagent/session-list notifications are acknowledged in `convert.rs` but not forwarded — their UI is part of a separate subagent design.

## Design

### 1. New Notification Variants (`cyril-core/src/types/event.rs`)

```rust
// Kiro extensions — operational status
RateLimitError {
    message: String,
},
McpServerInitFailure {
    server_name: String,
    error: Option<String>,
},
McpServerInitialized {
    server_name: String,
},
McpOAuthRequest {
    url: String,
    server_name: Option<String>,
},
```

### 2. Conversion Layer (`cyril-core/src/protocol/convert.rs`)

Change `to_ext_notification` return type from `Result<Notification>` to `Result<Option<Notification>>`:

- `Ok(Some(notification))` — parsed successfully, forward to App
- `Ok(None)` — known extension, intentionally not forwarded
- `Err(...)` — malformed data or genuinely unknown method

New match arms:

```rust
"kiro.dev/error/rate_limit" => {
    let message = params.get("message")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| error("rate_limit missing or empty message"))?
        .to_string();
    Ok(Some(Notification::RateLimitError { message }))
}
"kiro.dev/mcp/server_init_failure" => {
    let server_name = params.get("serverName")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| error("server_init_failure missing or empty serverName"))?
        .to_string();
    let error_msg = params.get("error")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);
    Ok(Some(Notification::McpServerInitFailure { server_name, error: error_msg }))
}
"kiro.dev/mcp/server_initialized" => {
    let server_name = params.get("serverName")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| error("server_initialized missing or empty serverName"))?
        .to_string();
    Ok(Some(Notification::McpServerInitialized { server_name }))
}
"kiro.dev/mcp/oauth_request" => {
    let url = params.get("url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| error("oauth_request missing or empty url"))?
        .to_string();
    let server_name = params.get("serverName")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);
    Ok(Some(Notification::McpOAuthRequest { url, server_name }))
}
// Known but not yet forwarded — subagent UI is a separate design
"kiro.dev/subagent/list_update"
| "kiro.dev/session/inbox_notification"
| "kiro.dev/session/list_update" => {
    tracing::debug!(method, "extension notification acknowledged, not forwarded");
    Ok(None)
}
```

Required fields use `.filter(|s| !s.is_empty())` to treat empty strings as absent, per the "errors are not default values" principle.

### 3. Client Update (`cyril-core/src/protocol/client.rs`)

Update the `ext_notification` match to handle the `Option` wrapping:

```rust
match convert::to_ext_notification(args.method.as_ref(), &params) {
    Ok(Some(notification)) => {
        self.notification_tx.send(notification).await
            .map_err(|_| acp::Error::new(-32603, "bridge closed"))?;
    }
    Ok(None) => {} // known, intentionally not forwarded
    Err(e) => {
        tracing::warn!(
            error = %e,
            method = %args.method,
            "unrecognized extension notification"
        );
    }
}
```

### 4. App Event Routing (`cyril/src/app.rs`)

| Variant | Routed to | Behavior |
|---|---|---|
| `RateLimitError` | `UiState` | `add_system_message` |
| `McpServerInitFailure` | `UiState` | `add_system_message` |
| `McpServerInitialized` | `UiState` | `add_system_message` |
| `McpOAuthRequest` | App (cross-cutting) | `open::that_detached(&url)`, then `add_system_message` |

`McpOAuthRequest` is cross-cutting like `CommandOptionsReceived` — the App handles it directly rather than delegating to `UiState`:

- Call `open::that_detached(&url)` to launch the browser without blocking the event loop
- On success: `add_system_message("Opening browser for {server_name} authentication...")`
- On failure: `add_system_message("Authenticate MCP server: {url}")` with the raw URL for manual copy

### 5. UiState (`cyril-ui/src/state.rs`)

New arms in `apply_notification`, following the existing `CompactionStatus`/`ClearStatus` pattern:

```rust
Notification::RateLimitError { message } => {
    self.add_system_message(format!("Rate limit: {message}"));
    true
}
Notification::McpServerInitFailure { server_name, error } => {
    if let Some(err) = error {
        self.add_system_message(format!("MCP server '{server_name}' failed to initialize: {err}"));
    } else {
        self.add_system_message(format!("MCP server '{server_name}' failed to initialize"));
    }
    true
}
Notification::McpServerInitialized { server_name } => {
    self.add_system_message(format!("MCP server '{server_name}' ready"));
    true
}
```

`McpOAuthRequest` is not handled in `UiState.apply_notification` — the App handles it directly and calls `add_system_message` itself.

### 6. Dependencies

Add `open` crate to `cyril/Cargo.toml` (binary crate only). Use `open::that_detached()` to avoid blocking the async event loop.

## Testing

### Conversion tests (`cyril-core/src/protocol/convert.rs`)

For each new extension method:
- Happy path: valid params → `Ok(Some(expected_variant))`
- Missing required field → `Err`
- Empty required field → `Err`
- Missing optional field → `Ok(Some(...))` with `None`

For acknowledged-but-not-forwarded methods:
- `subagent/list_update` → `Ok(None)`
- `session/inbox_notification` → `Ok(None)`
- `session/list_update` → `Ok(None)`

Existing tests updated for the `Result<Notification>` → `Result<Option<Notification>>` return type change (wrap expected values in `Some`).

### State tests (`cyril-ui/src/state.rs`)

For each new variant handled in `apply_notification`:
- Apply the notification, assert system message text matches expected format
- Follow the existing `add_system_message` test pattern

### Not tested

Browser open (`open::that_detached`) — host OS side effect, not unit-testable.

## Notes

- The "Adding New Features > New ACP event type" recipe in CLAUDE.md references `ProtocolEvent`/`ExtensionEvent`/`AppEvent` sub-enums that do not exist in the current code. This design follows the actual code pattern (variants on `Notification`). The CLAUDE.md recipe should be updated separately.
- The `kiro.dev/mcp/oauth_request` and `kiro.dev/mcp/server_initialized` payload formats are documented in the Kiro ACP docs but have not been observed in production logs. Field names (`serverName`, `url`, `error`) are based on the documented schema and the TUI source.
