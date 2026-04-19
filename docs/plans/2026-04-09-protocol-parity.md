# Protocol Parity Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Bring Cyril's ACP handling to full parity with the Kiro TUI (tui.js v1.29.6) by handling all extension notifications, extracting all metadata fields, and surfacing error states the user needs to see.

**Architecture:** All changes are additive — new `Notification` variants, new conversion arms, new UI state fields. No existing behavior changes. Change `to_ext_notification` to return `Result<Option<Notification>>` so known-but-unforwarded extensions return `Ok(None)` instead of `Err`. Each task is independent and can be committed separately.

**Tech Stack:** Rust 2021, serde_json, ratatui. Tests use `#[test]` and `#[tokio::test]`. See `CLAUDE.md` for build/test commands and code style.

**Protocol reference:** `docs/kiro-acp-protocol.md`, `docs/kiro-tui-1.29.5.js` (extracted from binary)

**Source of truth for field names:** `docs/kiro-tui-1.29.5.js` line 123378 (`EXT_METHODS`), line 124015 (`convertAcpUpdateToEvent`), and line 103358 (event handler switch). Where this plan names a JSON field, the name was verified against the tui.js source.

---

## Prerequisite: Change `to_ext_notification` Return Type

All tasks below assume `to_ext_notification` returns `Result<Option<Notification>>` instead of `Result<Notification>`. This lets known-but-unforwarded extensions return `Ok(None)`.

**Files:**
- Modify: `crates/cyril-core/src/protocol/convert.rs:87-230`
- Modify: `crates/cyril-core/src/protocol/client.rs` (wherever `to_ext_notification` result is consumed)

### Step 1: Change the signature and wrap all existing `Ok(...)` returns in `Some`

In `convert.rs`, change:
```rust
pub(crate) fn to_ext_notification(
    method: &str,
    params: &serde_json::Value,
) -> crate::Result<Option<Notification>> {
```

Wrap every existing `Ok(Notification::...)` return in `Ok(Some(Notification::...))`.

Change the `other =>` fallback at line 223 to:
```rust
        other => {
            tracing::debug!(method = other, "unknown extension notification");
            Ok(None)
        }
```

Unknown extensions now log and return `Ok(None)` instead of `Err`. This is correct — unknown extensions are not protocol errors; they're future extensions we don't handle yet. Only malformed data should be `Err`.

### Step 2: Update the caller in `client.rs`

```rust
match convert::to_ext_notification(args.method.as_ref(), &params) {
    Ok(Some(notification)) => {
        self.notification_tx.send(notification).await
            .map_err(|_| acp::Error::new(-32603, "bridge closed"))?;
    }
    Ok(None) => {} // known or unknown, intentionally not forwarded
    Err(e) => {
        tracing::warn!(
            error = %e,
            method = %args.method,
            "malformed extension notification"
        );
    }
}
```

### Step 3: Update all tests

Every test that asserts `result.is_ok()` and then matches `Ok(Notification::...)` needs to match `Ok(Some(Notification::...))` instead. The `unknown_method_returns_error` test should change to assert `Ok(None)`.

### Step 4: Run tests

Run: `cargo test -p cyril-core`
Expected: PASS

Run: `cargo check`
Expected: PASS (verify exhaustive matches compile)

### Step 5: Commit

```bash
git add crates/cyril-core/src/protocol/convert.rs crates/cyril-core/src/protocol/client.rs
git commit -m "refactor: to_ext_notification returns Option to distinguish unknown from malformed"
```

---

## Task 1: Rate Limit Error

Users get no feedback when the agent hits a rate limit. `kiro.dev/error/rate_limit` carries `{ message }`.

**tui.js reference:** line 123391 (`RATE_LIMIT_ERROR`), line 103687 (handler shows transient alert for 5s).

**Files:**
- Modify: `crates/cyril-core/src/types/event.rs:9-66` (add variant)
- Modify: `crates/cyril-core/src/protocol/convert.rs` (add match arm)
- Modify: `crates/cyril-ui/src/state.rs` (handle in apply_notification)
- Modify: `crates/cyril/examples/test_bridge.rs` (print handler)

### Step 1: Write failing conversion test

Add to test module in `convert.rs`:

```rust
#[test]
fn parse_rate_limit_error() {
    let params = serde_json::json!({
        "message": "Rate limit exceeded. Please wait before retrying."
    });
    let result = to_ext_notification("kiro.dev/error/rate_limit", &params);
    assert!(result.is_ok());
    if let Ok(Some(Notification::RateLimited { message })) = result {
        assert!(message.contains("Rate limit"));
    } else {
        panic!("expected RateLimited, got {:?}", result);
    }
}

#[test]
fn parse_rate_limit_error_missing_message() {
    let params = serde_json::json!({});
    let result = to_ext_notification("kiro.dev/error/rate_limit", &params);
    assert!(result.is_ok());
    if let Ok(Some(Notification::RateLimited { message })) = result {
        assert!(!message.is_empty());
    } else {
        panic!("expected RateLimited");
    }
}
```

### Step 2: Run test to verify it fails

Run: `cargo test -p cyril-core -- parse_rate_limit`
Expected: FAIL — `RateLimited` variant doesn't exist yet

### Step 3: Add variant and conversion arm

In `event.rs`, add to the `// Kiro extensions` section:

```rust
    RateLimited {
        message: String,
    },
```

In `convert.rs`, add before the `other =>` fallback:

```rust
        "kiro.dev/error/rate_limit" => {
            let message = params
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Rate limit exceeded")
                .to_string();
            Ok(Some(Notification::RateLimited { message }))
        }
```

### Step 4: Handle in UiState

In `state.rs` `apply_notification`:

```rust
        Notification::RateLimited { message } => {
            self.add_system_message(format!("Rate limited: {message}"));
            true
        }
```

### Step 5: Update test harness

In `test_bridge.rs` `print_notification`:

```rust
        Notification::RateLimited { message } => {
            println!("  [RateLimited] {message}");
        }
```

### Step 6: Run tests

Run: `cargo test -p cyril-core -- parse_rate_limit`
Expected: PASS

Run: `cargo check`
Expected: PASS

### Step 7: Commit

```bash
git add crates/cyril-core/src/types/event.rs crates/cyril-core/src/protocol/convert.rs crates/cyril-ui/src/state.rs crates/cyril/examples/test_bridge.rs
git commit -m "feat: surface rate limit errors as system messages"
```

---

## Task 2: MCP Server Notifications

Three MCP lifecycle notifications: init failure (P1), OAuth request (P2), and initialized (P2).

**tui.js reference:** line 123385-123387 (method names), line 103649-103685 (handlers). Note: the OAuth request uses `oauthUrl` (not `url`) per tui.js line 123858.

**Files:**
- Modify: `crates/cyril-core/src/types/event.rs` (add 3 variants)
- Modify: `crates/cyril-core/src/protocol/convert.rs` (add 3 match arms)
- Modify: `crates/cyril-ui/src/state.rs` (handle in apply_notification)
- Modify: `crates/cyril/src/app.rs` (cross-cutting handler for OAuth)
- Modify: `crates/cyril/examples/test_bridge.rs`

### Step 1: Write failing tests

```rust
#[test]
fn parse_mcp_server_init_failure() {
    let params = serde_json::json!({
        "serverName": "my-mcp",
        "error": "connection refused"
    });
    let result = to_ext_notification("kiro.dev/mcp/server_init_failure", &params);
    if let Ok(Some(Notification::McpServerInitFailure { server_name, error })) = result {
        assert_eq!(server_name, "my-mcp");
        assert_eq!(error.as_deref(), Some("connection refused"));
    } else {
        panic!("expected McpServerInitFailure, got {:?}", result);
    }
}

#[test]
fn parse_mcp_server_init_failure_no_error() {
    let params = serde_json::json!({ "serverName": "my-mcp" });
    let result = to_ext_notification("kiro.dev/mcp/server_init_failure", &params);
    if let Ok(Some(Notification::McpServerInitFailure { server_name, error })) = result {
        assert_eq!(server_name, "my-mcp");
        assert!(error.is_none());
    } else {
        panic!("expected McpServerInitFailure");
    }
}

#[test]
fn parse_mcp_oauth_request() {
    let params = serde_json::json!({
        "serverName": "github-mcp",
        "oauthUrl": "https://github.com/login/oauth/authorize?..."
    });
    let result = to_ext_notification("kiro.dev/mcp/oauth_request", &params);
    if let Ok(Some(Notification::McpOAuthRequest { server_name, url })) = result {
        assert_eq!(server_name, "github-mcp");
        assert!(url.starts_with("https://"));
    } else {
        panic!("expected McpOAuthRequest, got {:?}", result);
    }
}

#[test]
fn parse_mcp_server_initialized() {
    let params = serde_json::json!({ "serverName": "github-mcp" });
    let result = to_ext_notification("kiro.dev/mcp/server_initialized", &params);
    if let Ok(Some(Notification::McpServerInitialized { server_name })) = result {
        assert_eq!(server_name, "github-mcp");
    } else {
        panic!("expected McpServerInitialized, got {:?}", result);
    }
}
```

### Step 2: Run tests to verify they fail

Run: `cargo test -p cyril-core -- parse_mcp`
Expected: FAIL

### Step 3: Add variants

In `event.rs`:

```rust
    McpServerInitFailure {
        server_name: String,
        error: Option<String>,
    },
    McpOAuthRequest {
        server_name: String,
        url: String,
    },
    McpServerInitialized {
        server_name: String,
    },
```

### Step 4: Add conversion arms

In `convert.rs`:

```rust
        "kiro.dev/mcp/server_init_failure" => {
            let server_name = params
                .get("serverName")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("unknown")
                .to_string();
            let error = params
                .get("error")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);
            Ok(Some(Notification::McpServerInitFailure { server_name, error }))
        }
        "kiro.dev/mcp/oauth_request" => {
            let server_name = params
                .get("serverName")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let url = params
                .get("oauthUrl")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("")
                .to_string();
            if url.is_empty() {
                tracing::warn!("mcp/oauth_request missing oauthUrl");
                Ok(None)
            } else {
                Ok(Some(Notification::McpOAuthRequest { server_name, url }))
            }
        }
        "kiro.dev/mcp/server_initialized" => {
            let server_name = params
                .get("serverName")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            Ok(Some(Notification::McpServerInitialized { server_name }))
        }
```

### Step 5: Handle in UiState (init failure + initialized)

```rust
        Notification::McpServerInitFailure { server_name, error } => {
            if let Some(err) = error {
                self.add_system_message(
                    format!("MCP server '{server_name}' failed to initialize: {err}")
                );
            } else {
                self.add_system_message(
                    format!("MCP server '{server_name}' failed to initialize")
                );
            }
            true
        }
        Notification::McpServerInitialized { server_name } => {
            self.add_system_message(format!("MCP server '{server_name}' ready"));
            true
        }
```

### Step 6: Handle McpOAuthRequest in App (cross-cutting)

`McpOAuthRequest` is cross-cutting like `CommandOptionsReceived` — the App handles it directly. Add `open` crate to `crates/cyril/Cargo.toml`. In `app.rs`:

```rust
        Notification::McpOAuthRequest { ref server_name, ref url } => {
            match open::that_detached(url) {
                Ok(()) => {
                    self.ui_state.add_system_message(
                        format!("Opening browser for '{server_name}' authentication...")
                    );
                }
                Err(_) => {
                    self.ui_state.add_system_message(
                        format!("Authenticate '{server_name}': {url}")
                    );
                }
            }
        }
```

Do NOT handle this in `UiState.apply_notification` — the App handles it and calls `add_system_message` itself.

### Step 7: Update test harness

In `test_bridge.rs`:

```rust
        Notification::McpServerInitFailure { server_name, error } => {
            println!("  [McpInitFail] {server_name}: {}", error.as_deref().unwrap_or("(no detail)"));
        }
        Notification::McpOAuthRequest { server_name, url } => {
            println!("  [McpOAuth] {server_name}: {url}");
        }
        Notification::McpServerInitialized { server_name } => {
            println!("  [McpReady] {server_name}");
        }
```

### Step 8: Run tests

Run: `cargo test -p cyril-core -- parse_mcp`
Expected: PASS

Run: `cargo check`
Expected: PASS

### Step 9: Commit

```bash
git add crates/cyril-core/src/types/event.rs crates/cyril-core/src/protocol/convert.rs crates/cyril-ui/src/state.rs crates/cyril/src/app.rs crates/cyril/Cargo.toml crates/cyril/examples/test_bridge.rs
git commit -m "feat: handle MCP server lifecycle notifications (init failure, OAuth, initialized)"
```

---

## Task 3: Agent and Model Error Notifications

Four init-time error notifications that Cyril silently drops: `agent/not_found`, `agent/config_error`, `model/not_found` (new in 1.29.6), and `session_error`/`auth_error`.

**tui.js reference:** line 123388-123390 (method names), line 103696-103754 (handlers collect into `initErrors` array and show summarized alert).

**Files:**
- Modify: `crates/cyril-core/src/types/event.rs` (add variants)
- Modify: `crates/cyril-core/src/protocol/convert.rs` (add match arms)
- Modify: `crates/cyril-ui/src/state.rs` (system messages)
- Modify: `crates/cyril/examples/test_bridge.rs`

### Step 1: Write failing tests

```rust
#[test]
fn parse_agent_not_found() {
    let params = serde_json::json!({
        "requestedAgent": "code-reviewer",
        "fallbackAgent": "default"
    });
    let result = to_ext_notification("kiro.dev/agent/not_found", &params);
    if let Ok(Some(Notification::AgentNotFound { requested, fallback })) = result {
        assert_eq!(requested, "code-reviewer");
        assert_eq!(fallback.as_deref(), Some("default"));
    } else {
        panic!("expected AgentNotFound, got {:?}", result);
    }
}

#[test]
fn parse_agent_config_error() {
    let params = serde_json::json!({
        "path": ".kiro/agents/broken.md",
        "error": "invalid YAML frontmatter"
    });
    let result = to_ext_notification("kiro.dev/agent/config_error", &params);
    if let Ok(Some(Notification::AgentConfigError { path, error })) = result {
        assert_eq!(path, ".kiro/agents/broken.md");
        assert_eq!(error, "invalid YAML frontmatter");
    } else {
        panic!("expected AgentConfigError, got {:?}", result);
    }
}

#[test]
fn parse_model_not_found() {
    let params = serde_json::json!({
        "requestedModel": "claude-opus-5",
        "fallbackModel": "claude-sonnet-4"
    });
    let result = to_ext_notification("kiro.dev/model/not_found", &params);
    if let Ok(Some(Notification::ModelNotFound { requested, fallback })) = result {
        assert_eq!(requested, "claude-opus-5");
        assert_eq!(fallback.as_deref(), Some("claude-sonnet-4"));
    } else {
        panic!("expected ModelNotFound, got {:?}", result);
    }
}
```

### Step 2: Run tests to verify they fail

Run: `cargo test -p cyril-core -- parse_agent_not_found parse_agent_config parse_model_not_found`
Expected: FAIL

### Step 3: Add variants

In `event.rs`:

```rust
    AgentNotFound {
        requested: String,
        fallback: Option<String>,
    },
    AgentConfigError {
        path: String,
        error: String,
    },
    ModelNotFound {
        requested: String,
        fallback: Option<String>,
    },
```

### Step 4: Add conversion arms

```rust
        "kiro.dev/agent/not_found" => {
            let requested = params
                .get("requestedAgent")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let fallback = params
                .get("fallbackAgent")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);
            Ok(Some(Notification::AgentNotFound { requested, fallback }))
        }
        "kiro.dev/agent/config_error" => {
            let path = params
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("(unknown path)")
                .to_string();
            let error = params
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("(no detail)")
                .to_string();
            Ok(Some(Notification::AgentConfigError { path, error }))
        }
        "kiro.dev/model/not_found" => {
            let requested = params
                .get("requestedModel")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let fallback = params
                .get("fallbackModel")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);
            Ok(Some(Notification::ModelNotFound { requested, fallback }))
        }
```

### Step 5: Handle in UiState

```rust
        Notification::AgentNotFound { requested, fallback } => {
            if let Some(ref fb) = fallback {
                self.add_system_message(
                    format!("Agent '{requested}' not found, using '{fb}'")
                );
            } else {
                self.add_system_message(format!("Agent '{requested}' not found"));
            }
            true
        }
        Notification::AgentConfigError { path, error } => {
            self.add_system_message(format!("Agent config error in {path}: {error}"));
            true
        }
        Notification::ModelNotFound { requested, fallback } => {
            if let Some(ref fb) = fallback {
                self.add_system_message(
                    format!("Model '{requested}' not available, using '{fb}'")
                );
            } else {
                self.add_system_message(format!("Model '{requested}' not available"));
            }
            true
        }
```

### Step 6: Update test harness, run tests, commit

Run: `cargo test -p cyril-core`
Expected: PASS

```bash
git add crates/cyril-core/src/types/event.rs crates/cyril-core/src/protocol/convert.rs crates/cyril-ui/src/state.rs crates/cyril/examples/test_bridge.rs
git commit -m "feat: surface agent/model not found and agent config errors"
```

---

## Task 4: Acknowledge Subagent/Session Notifications

Three multi-session notifications that should be acknowledged (not error-logged) but not forwarded to the UI until crew support is implemented.

**tui.js reference:** line 123392-123395 (method names). These feed the crew monitor UI which Cyril doesn't have.

**Files:**
- Modify: `crates/cyril-core/src/protocol/convert.rs` (add match arms returning `Ok(None)`)

### Step 1: Add match arms

```rust
        "kiro.dev/subagent/list_update"
        | "kiro.dev/session/activity"
        | "kiro.dev/session/list_update"
        | "kiro.dev/session/inbox_notification" => {
            tracing::debug!(method, "multi-session notification acknowledged, not forwarded");
            Ok(None)
        }
```

### Step 2: Write test

```rust
#[test]
fn subagent_notifications_acknowledged_not_forwarded() {
    for method in [
        "kiro.dev/subagent/list_update",
        "kiro.dev/session/activity",
        "kiro.dev/session/list_update",
        "kiro.dev/session/inbox_notification",
    ] {
        let result = to_ext_notification(method, &serde_json::json!({}));
        assert!(
            matches!(result, Ok(None)),
            "{method} should return Ok(None), got {result:?}"
        );
    }
}
```

### Step 3: Run tests, commit

Run: `cargo test -p cyril-core -- subagent_notifications`
Expected: PASS

```bash
git add crates/cyril-core/src/protocol/convert.rs
git commit -m "feat: acknowledge multi-session notifications without forwarding"
```

---

## Task 5: Turn Metering and Token Metadata

The `kiro.dev/metadata` notification carries three data sets Cyril currently ignores:
1. `meteringUsage` — array of `{ unit, unitPlural, value }` (credits per turn)
2. `turnDurationMs` — wall-clock turn duration
3. `inputTokens`, `outputTokens`, `cachedTokens` — token counts

**tui.js reference:** line 123809-123828 (`handleMetadataUpdate` — parses `contextUsagePercentage`, `meteringUsage`, `turnDurationMs`). Line 103603-103613 (handler sets context usage + token counts).

Note: tui.js distinguishes `context_usage` from `metadata` (tokens) as separate internal events, both sourced from the same `kiro.dev/metadata` wire notification. Cyril should extract all fields from the single notification.

**Files:**
- Modify: `crates/cyril-core/src/types/session.rs` (add `TurnMetering`, `SessionCost`)
- Modify: `crates/cyril-core/src/types/event.rs` (add `MetadataUpdated` variant)
- Modify: `crates/cyril-core/src/protocol/convert.rs` (parse metering from metadata)
- Modify: `crates/cyril-core/src/session.rs` (accumulate cost)
- Modify: `crates/cyril-ui/src/state.rs` (store for display)
- Modify: `crates/cyril-ui/src/traits.rs` (expose via TuiState)

### Step 1: Add types

In `crates/cyril-core/src/types/session.rs`:

```rust
/// Per-turn metering data from kiro.dev/metadata.
#[derive(Debug, Clone)]
pub struct TurnMetering {
    credits: f64,
    duration_ms: Option<u64>,
}

impl TurnMetering {
    pub fn new(credits: f64, duration_ms: Option<u64>) -> Self {
        Self { credits, duration_ms }
    }

    pub fn credits(&self) -> f64 { self.credits }
    pub fn duration_ms(&self) -> Option<u64> { self.duration_ms }

    pub fn duration_display(&self) -> Option<String> {
        self.duration_ms.map(|ms| {
            if ms < 1000 {
                format!("{ms}ms")
            } else if ms < 60_000 {
                format!("{:.1}s", ms as f64 / 1000.0)
            } else {
                let mins = ms / 60_000;
                let secs = (ms % 60_000) / 1000;
                format!("{mins}m {secs}s")
            }
        })
    }
}

/// Running session cost accumulator.
#[derive(Debug, Clone, Default)]
pub struct SessionCost {
    total_credits: f64,
    turn_count: u32,
    last_turn_credits: Option<f64>,
    last_turn_duration_ms: Option<u64>,
}

impl SessionCost {
    pub fn new() -> Self { Self::default() }

    pub fn record_turn(&mut self, metering: &TurnMetering) {
        self.total_credits += metering.credits;
        self.turn_count += 1;
        self.last_turn_credits = Some(metering.credits);
        self.last_turn_duration_ms = metering.duration_ms;
    }

    pub fn total_credits(&self) -> f64 { self.total_credits }
    pub fn turn_count(&self) -> u32 { self.turn_count }
    pub fn last_turn_credits(&self) -> Option<f64> { self.last_turn_credits }
    pub fn last_turn_duration_ms(&self) -> Option<u64> { self.last_turn_duration_ms }
}

/// Token counts from a single turn.
#[derive(Debug, Clone)]
pub struct TokenCounts {
    pub input: u64,
    pub output: u64,
    pub cached: u64,
}
```

### Step 2: Add `MetadataUpdated` variant

In `event.rs`, add a new variant that carries all metadata fields. Keep `ContextUsageUpdated` for backward compat — `MetadataUpdated` is the full-fidelity replacement:

```rust
    MetadataUpdated {
        context_usage: ContextUsage,
        metering: Option<TurnMetering>,
        tokens: Option<TokenCounts>,
    },
```

### Step 3: Update the conversion

Replace the `kiro.dev/metadata` arm in `convert.rs`:

```rust
        "kiro.dev/metadata" => {
            let pct = params
                .get("contextUsagePercentage")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            let metering = params
                .get("meteringUsage")
                .and_then(|m| m.as_array())
                .and_then(|arr| {
                    let credits: f64 = arr
                        .iter()
                        .filter_map(|u| u.get("value").and_then(|v| v.as_f64()))
                        .sum();
                    if credits > 0.0 {
                        let duration_ms = params
                            .get("turnDurationMs")
                            .and_then(|d| d.as_u64());
                        Some(TurnMetering::new(credits, duration_ms))
                    } else {
                        None
                    }
                });

            let tokens = {
                let input = params.get("inputTokens").and_then(|v| v.as_u64());
                let output = params.get("outputTokens").and_then(|v| v.as_u64());
                let cached = params.get("cachedTokens").and_then(|v| v.as_u64());
                match (input, output) {
                    (Some(i), Some(o)) => Some(TokenCounts {
                        input: i,
                        output: o,
                        cached: cached.unwrap_or(0),
                    }),
                    _ => None,
                }
            };

            Ok(Some(Notification::MetadataUpdated {
                context_usage: ContextUsage::new(pct),
                metering,
                tokens,
            }))
        }
```

### Step 4: Migrate `ContextUsageUpdated` callers

Replace all `Notification::ContextUsageUpdated(...)` matches with `Notification::MetadataUpdated { context_usage, metering, tokens }`. Remove the `ContextUsageUpdated` variant from `event.rs`.

In `SessionController::apply_notification`:
```rust
        Notification::MetadataUpdated { context_usage, metering, .. } => {
            self.context_usage = Some(context_usage.clone());
            if let Some(ref m) = metering {
                self.session_cost.record_turn(m);
            }
            true
        }
```

Add `session_cost: SessionCost` field to `SessionController` with a `pub fn session_cost(&self) -> &SessionCost` accessor.

In `UiState::apply_notification`:
```rust
        Notification::MetadataUpdated { context_usage, metering, tokens } => {
            self.context_usage = Some(context_usage.percentage());
            if let Some(ref m) = metering {
                self.total_credits += m.credits();
                self.last_turn_credits = Some(m.credits());
                self.last_turn_duration_ms = m.duration_ms();
            }
            if let Some(ref t) = tokens {
                self.last_turn_tokens = Some(t.clone());
            }
            true
        }
```

### Step 5: Write tests

```rust
#[test]
fn parse_metadata_with_metering() {
    let params = serde_json::json!({
        "sessionId": "s1",
        "contextUsagePercentage": 7.11,
        "meteringUsage": [
            {"unit": "credit", "unitPlural": "credits", "value": 0.018}
        ],
        "turnDurationMs": 1948
    });
    let result = to_ext_notification("kiro.dev/metadata", &params);
    if let Ok(Some(Notification::MetadataUpdated { context_usage, metering, .. })) = result {
        assert!((context_usage.percentage() - 7.11).abs() < 0.01);
        let m = metering.unwrap();
        assert!((m.credits() - 0.018).abs() < 0.001);
        assert_eq!(m.duration_ms(), Some(1948));
        assert_eq!(m.duration_display(), Some("1.9s".into()));
    } else {
        panic!("expected MetadataUpdated, got {:?}", result);
    }
}

#[test]
fn parse_metadata_without_metering() {
    let params = serde_json::json!({
        "sessionId": "s1",
        "contextUsagePercentage": 2.28
    });
    let result = to_ext_notification("kiro.dev/metadata", &params);
    if let Ok(Some(Notification::MetadataUpdated { metering, tokens, .. })) = result {
        assert!(metering.is_none());
        assert!(tokens.is_none());
    } else {
        panic!("expected MetadataUpdated");
    }
}

#[test]
fn session_cost_accumulates() {
    let mut cost = SessionCost::new();
    cost.record_turn(&TurnMetering::new(0.018, Some(1948)));
    cost.record_turn(&TurnMetering::new(0.042, Some(5200)));
    assert_eq!(cost.turn_count(), 2);
    assert!((cost.total_credits() - 0.060).abs() < 0.001);
    assert!((cost.last_turn_credits().unwrap() - 0.042).abs() < 0.001);
    assert_eq!(cost.last_turn_duration_ms(), Some(5200));
}

#[test]
fn duration_display_formatting() {
    assert_eq!(TurnMetering::new(0.01, Some(500)).duration_display(), Some("500ms".into()));
    assert_eq!(TurnMetering::new(0.01, Some(1948)).duration_display(), Some("1.9s".into()));
    assert_eq!(TurnMetering::new(0.01, Some(135000)).duration_display(), Some("2m 15s".into()));
    assert!(TurnMetering::new(0.01, None).duration_display().is_none());
}
```

### Step 6: Run tests, commit

Run: `cargo test -p cyril-core`
Expected: PASS

```bash
git add crates/cyril-core/src/types/session.rs crates/cyril-core/src/types/event.rs crates/cyril-core/src/protocol/convert.rs crates/cyril-core/src/session.rs crates/cyril-ui/src/state.rs crates/cyril-ui/src/traits.rs crates/cyril/examples/test_bridge.rs
git commit -m "feat: extract turn metering, duration, and token counts from metadata"
```

---

## Task 6: Structured Compaction Status

The `kiro.dev/compaction/status` payload gained a structured `status` object with `type` (`started`/`completed`/`failed`) and optional `error`/`summary` fields. Cyril currently only reads the legacy `message` string.

**tui.js reference:** line 123833-123844 (`handleCompactionStatus` — reads `status.type`, `status.error`, `summary`). Line 103615-103627 (handler differentiates started/completed/failed).

**Files:**
- Modify: `crates/cyril-core/src/protocol/convert.rs:99-106` (parse structured status)

### Step 1: Write failing tests

```rust
#[test]
fn parse_compaction_status_structured_started() {
    let params = serde_json::json!({ "status": { "type": "started" } });
    let result = to_ext_notification("kiro.dev/compaction/status", &params);
    if let Ok(Some(Notification::CompactionStatus { message })) = result {
        assert!(message.contains("Compacting"), "got: {message}");
    } else {
        panic!("expected CompactionStatus");
    }
}

#[test]
fn parse_compaction_status_structured_failed() {
    let params = serde_json::json!({ "status": { "type": "failed", "error": "out of memory" } });
    let result = to_ext_notification("kiro.dev/compaction/status", &params);
    if let Ok(Some(Notification::CompactionStatus { message })) = result {
        assert!(message.contains("out of memory"), "got: {message}");
    } else {
        panic!("expected CompactionStatus");
    }
}

#[test]
fn parse_compaction_status_structured_completed() {
    let params = serde_json::json!({ "status": { "type": "completed" }, "summary": "3 turns removed" });
    let result = to_ext_notification("kiro.dev/compaction/status", &params);
    if let Ok(Some(Notification::CompactionStatus { message })) = result {
        assert!(message.contains("completed") || message.contains("3 turns"), "got: {message}");
    } else {
        panic!("expected CompactionStatus");
    }
}

#[test]
fn parse_compaction_status_legacy_message() {
    let params = serde_json::json!({ "message": "Compacting conversation context..." });
    let result = to_ext_notification("kiro.dev/compaction/status", &params);
    if let Ok(Some(Notification::CompactionStatus { message })) = result {
        assert_eq!(message, "Compacting conversation context...");
    } else {
        panic!("expected CompactionStatus");
    }
}
```

### Step 2: Update the conversion

Replace the `kiro.dev/compaction/status` arm:

```rust
        "kiro.dev/compaction/status" => {
            let message = if let Some(status) = params.get("status") {
                let status_type = status
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("unknown");
                match status_type {
                    "started" => "Compacting conversation context...".to_string(),
                    "completed" => {
                        let summary = params
                            .get("summary")
                            .and_then(|s| s.as_str())
                            .unwrap_or("done");
                        format!("Compaction completed: {summary}")
                    }
                    "failed" => {
                        let error = status
                            .get("error")
                            .and_then(|e| e.as_str())
                            .unwrap_or("unknown error");
                        format!("Compaction failed: {error}")
                    }
                    other => format!("Compaction: {other}"),
                }
            } else {
                params
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            Ok(Some(Notification::CompactionStatus { message }))
        }
```

### Step 3: Run tests, commit

Run: `cargo test -p cyril-core -- parse_compaction`
Expected: PASS

```bash
git add crates/cyril-core/src/protocol/convert.rs
git commit -m "feat: handle structured compaction status (started/completed/failed)"
```

---

## Task 7: Extract Models from session/new Response

The `session/new` response includes `models.currentModelId` and `models.availableModels[]`. This replaces the v1.28.0 workaround of extracting model from `/model` command responses.

**tui.js reference:** line 123531-123551 (`newSession` — parses `sessionResult.models`).

**Files:**
- Modify: `crates/cyril-core/src/types/event.rs` (extend `SessionCreated`)
- Modify: `crates/cyril-core/src/protocol/bridge.rs` (parse models from response)
- Modify: `crates/cyril-core/src/session.rs` (store model)
- Modify: `crates/cyril-ui/src/state.rs` (set model on session create)
- Modify: `crates/cyril/src/app.rs` (remove /model workaround)

### Step 1: Extend SessionCreated variant

```rust
    SessionCreated {
        session_id: SessionId,
        current_mode: Option<String>,
        current_model: Option<String>,
    },
```

### Step 2: Parse models from bridge response

In `bridge.rs`, the `NewSession` handler constructs `SessionCreated`. Add model extraction:

```rust
let current_model = response
    .models
    .as_ref()
    .map(|m| m.current_model_id.to_string());
```

Note: Check whether the `acp::NewSessionResponse` type actually has a `models` field. If the `agent-client-protocol` crate doesn't expose it, log and skip. The tui.js confirms Kiro sends it.

### Step 3: Update all SessionCreated destructuring sites

Fix all matches in `session.rs`, `state.rs`, `app.rs`, `test_bridge.rs`, and test code to include `current_model`.

### Step 4: Remove the /model workaround in app.rs

Remove the `WORKAROUND(Kiro v1.28.0)` block that extracts model from `CommandExecuted`. The model is now set on session creation.

### Step 5: Run tests, commit

Run: `cargo test`
Expected: PASS

```bash
git commit -m "feat: extract model from session/new response, remove /model workaround"
```

---

## Task 8: Prompt Support from commands/available

Parse the `prompts` array from `kiro.dev/commands/available` so prompt names appear in autocomplete.

**tui.js reference:** line 123781-123807 (`handleCommandsAdvertising` — extracts `params.prompts`). tui.js treats prompts as slash commands that send their text as a plain prompt.

**Files:**
- Create: `crates/cyril-core/src/types/prompt.rs`
- Modify: `crates/cyril-core/src/types/mod.rs` (add module)
- Modify: `crates/cyril-core/src/types/event.rs` (change `CommandsUpdated` to struct variant)
- Modify: `crates/cyril-core/src/protocol/convert.rs` (parse prompts)
- Modify: all `CommandsUpdated` match sites

### Step 1: Create PromptInfo type

See existing plan `2026-04-02-v129-protocol-updates.md` Task 4, Step 1 for the full `PromptInfo` and `PromptArgument` types with tests.

### Step 2: Change `CommandsUpdated` to struct variant

```rust
    CommandsUpdated {
        commands: Vec<CommandInfo>,
        prompts: Vec<crate::types::PromptInfo>,
    },
```

### Step 3: Parse prompts in the commands/available arm

Add after existing commands parsing:

```rust
let prompts = params
    .get("prompts")
    .and_then(|p| p.as_array())
    .map(|arr| {
        arr.iter()
            .filter_map(|v| {
                let name = v.get("name")?.as_str()?;
                let description = v.get("description")
                    .and_then(|d| d.as_str()).map(String::from);
                let server_name = v.get("serverName")
                    .and_then(|s| s.as_str()).map(String::from);
                let arguments = v.get("arguments")
                    .and_then(|a| a.as_array())
                    .map(|args| {
                        args.iter().filter_map(|arg| {
                            let arg_name = arg.get("name")?.as_str()?;
                            let required = arg.get("required")
                                .and_then(|r| r.as_bool()).unwrap_or(false);
                            let desc = arg.get("description")
                                .and_then(|d| d.as_str()).map(String::from);
                            Some(PromptArgument::new(arg_name, desc, required))
                        }).collect()
                    })
                    .unwrap_or_default();
                Some(PromptInfo::new(name, description, server_name, arguments))
            })
            .collect()
    })
    .unwrap_or_default();

Ok(Some(Notification::CommandsUpdated { commands, prompts }))
```

### Step 4: Update all CommandsUpdated match sites

Every `Notification::CommandsUpdated(cmds)` becomes `Notification::CommandsUpdated { commands, prompts }` (or `commands, ..` where prompts aren't used).

### Step 5: Register prompt names for autocomplete

In `app.rs`, add prompt names alongside command names when registering autocomplete.

### Step 6: Run tests, commit

```bash
git commit -m "feat: parse prompts from commands/available with argument support"
```

---

## Deferred (Not In This Plan)

These are known gaps that require more design work and are intentionally excluded:

| Gap | Reason deferred |
|---|---|
| Tool call output display (`rawOutput` from `tool_call_finished`) | Requires UI widget changes for collapsible output — design needed |
| `toolContent` diff rendering from initial `tool_call` | Cyril already captures diffs via `ToolCallContent::Diff`; rendering enhancement is a UI task |
| `user_message` echo from agent | Low value — the user already sees what they typed |
| Auth error / session error (`auth_error`, `session_error`) | These are not `kiro.dev/*` extension notifications — they arrive through a different mechanism (possibly error responses to `session/prompt`). Need to trace the actual wire format before implementing. |
| Multi-session crew UI | Acknowledged in Task 4 but UI design needed |
| Toolbar rendering for metering/tokens | Task 5 captures the data; rendering is a separate UI task |
