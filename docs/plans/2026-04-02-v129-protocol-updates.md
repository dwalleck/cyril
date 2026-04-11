# Kiro v1.29.0 Protocol Updates Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Handle new Kiro v1.29.0 protocol features that Cyril currently silently drops: rate limit errors, model list from session/new, structured compaction status, prompt support, and turn metering.

**Architecture:** All changes are additive — new Notification variants, new conversion arms, new UI state fields. No existing behavior changes. Each task is independent and can be landed separately.

**Tech Stack:** Rust 2021, serde_json, ratatui. Tests use `#[test]` and `#[tokio::test]`. See `CLAUDE.md` for build/test commands and code style.

**Protocol reference:** `docs/kiro-acp-protocol.md`

---

## Task 1: Rate Limit Error Handling

Users currently get no feedback when the agent hits a rate limit. `kiro.dev/error/rate_limit` arrives with `{message}` and is silently dropped.

**Files:**
- Modify: `crates/cyril-core/src/types/event.rs:9-66` (add variant)
- Modify: `crates/cyril-core/src/protocol/convert.rs:87-230` (add match arm)
- Modify: `crates/cyril-ui/src/state.rs:196-334` (handle in apply_notification)
- Modify: `crates/cyril/examples/test_bridge.rs:200-336` (print handler)

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
    if let Ok(Notification::RateLimited { message }) = result {
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
    if let Ok(Notification::RateLimited { message }) = result {
        assert!(!message.is_empty());
    } else {
        panic!("expected RateLimited");
    }
}
```

### Step 2: Run test to verify it fails

Run: `cargo test -p cyril-core -- parse_rate_limit`
Expected: FAIL — `RateLimited` variant doesn't exist yet

### Step 3: Add the Notification variant

In `event.rs`, add to the Notification enum in the `// Kiro extensions` section (after `ClearStatus`):

```rust
    RateLimited {
        message: String,
    },
```

### Step 4: Add conversion arm

In `convert.rs` `to_ext_notification`, add before the `other =>` fallback (around line 223):

```rust
        "kiro.dev/error/rate_limit" => {
            let message = params
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Rate limit exceeded")
                .to_string();
            Ok(Notification::RateLimited { message })
        }
```

### Step 5: Handle in UiState

In `state.rs` `apply_notification`, add an arm (before the `CommandsUpdated` catch-all block):

```rust
        Notification::RateLimited { message } => {
            self.add_system_message(format!("Rate limited: {message}"));
            true
        }
```

### Step 6: Update test harness

In `test_bridge.rs` `print_notification`, add:

```rust
        Notification::RateLimited { message } => {
            println!("  [RateLimited] {message}");
        }
```

### Step 7: Run tests

Run: `cargo test -p cyril-core -- parse_rate_limit`
Expected: PASS

Run: `cargo check`
Expected: PASS (verify exhaustive matches compile)

### Step 8: Commit

```bash
git add crates/cyril-core/src/types/event.rs crates/cyril-core/src/protocol/convert.rs crates/cyril-ui/src/state.rs crates/cyril/examples/test_bridge.rs
git commit -m "feat: surface rate limit errors as system messages"
```

---

## Task 2: Extract Models from session/new Response

The `session/new` response now includes a `models` field with `currentModelId` and `availableModels[]`. This replaces the v1.28.0 workaround of extracting the model from `/model` command responses.

**Files:**
- Modify: `crates/cyril-core/src/types/event.rs` (extend SessionCreated variant)
- Modify: `crates/cyril-core/src/protocol/bridge.rs:205-235` (parse models from response)
- Modify: `crates/cyril-core/src/session.rs:109-115` (store model on SessionCreated)
- Modify: `crates/cyril-ui/src/state.rs:295-304` (set model on SessionCreated)
- Modify: `crates/cyril/src/app.rs:207-228` (remove workaround)

### Step 1: Extend SessionCreated variant

In `event.rs`, change `SessionCreated` to include model info:

```rust
    SessionCreated {
        session_id: SessionId,
        current_mode: Option<String>,
        current_model: Option<String>,
    },
```

### Step 2: Parse models from bridge response

In `bridge.rs`, the `NewSession` handler constructs `SessionCreated` at lines 218-221. Update to extract model:

```rust
            BridgeCommand::NewSession { cwd: session_cwd } => {
                let translated_cwd = crate::platform::path::to_agent(&session_cwd);
                match conn
                    .new_session(acp::NewSessionRequest::new(translated_cwd))
                    .await
                {
                    Ok(response) => {
                        active_session_id = Some(response.session_id.clone());
                        let session_id = response.session_id.to_string();
                        let current_mode = response
                            .modes
                            .as_ref()
                            .map(|m| m.current_mode_id.to_string());

                        // New in v1.29.0: extract current model from response
                        let current_model = response
                            .models
                            .as_ref()
                            .and_then(|m| Some(m.current_model_id.to_string()));

                        let notification = Notification::SessionCreated {
                            session_id: crate::types::SessionId::new(session_id),
                            current_mode,
                            current_model,
                        };
                        if channels.notification_tx.send(notification).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => { /* unchanged */ }
                }
            }
```

Note: Check whether the `acp::NewSessionResponse` type actually has a `models` field. The `agent-client-protocol` crate may not expose it yet. If not, parse it from the raw response JSON using `ExtMethod` instead, or check if a newer ACP crate version has it. If the field isn't available in the typed response, extract it via serde:

```rust
// Fallback: if acp crate doesn't expose models, log and skip
tracing::debug!(
    has_models = response.models.is_some(),
    "session/new response"
);
```

### Step 3: Handle in SessionController

In `session.rs` `apply_notification`, update the `SessionCreated` arm:

```rust
        Notification::SessionCreated {
            session_id,
            current_mode,
            current_model,
        } => {
            self.id = Some(session_id.clone());
            self.current_mode_id = current_mode.clone();
            if let Some(model) = current_model {
                self.cached_model = Some(model.clone());
            }
            self.status = SessionStatus::Active;
            true
        }
```

### Step 4: Handle in UiState

In `state.rs` `apply_notification`, update the `SessionCreated` arm:

```rust
        Notification::SessionCreated {
            session_id,
            current_mode,
            current_model,
        } => {
            self.session_label = Some(session_id.as_str().to_string());
            self.current_mode = current_mode.clone();
            if let Some(model) = current_model {
                self.current_model = Some(model.clone());
            }
            self.activity = Activity::Ready;
            true
        }
```

### Step 5: Remove the /model workaround

In `app.rs`, remove the `WORKAROUND(Kiro v1.28.0)` block (lines 214-225) that extracts model from CommandExecuted. The model is now set on session creation. Keep the `CommandExecuted` handler but remove the model extraction if-block.

### Step 6: Fix all compilation errors

The `SessionCreated` variant is destructured in:
- `crates/cyril-core/src/session.rs`
- `crates/cyril-ui/src/state.rs`
- `crates/cyril/src/app.rs`
- `crates/cyril/examples/test_bridge.rs`
- Tests in `crates/cyril-core/src/session.rs` and `crates/cyril-core/src/protocol/convert.rs`

Update all to include `current_model`.

### Step 7: Run tests

Run: `cargo test`
Expected: PASS

### Step 8: Commit

```bash
git add crates/cyril-core/src/types/event.rs crates/cyril-core/src/protocol/bridge.rs crates/cyril-core/src/session.rs crates/cyril-ui/src/state.rs crates/cyril/src/app.rs crates/cyril/examples/test_bridge.rs
git commit -m "feat: extract model from session/new response, remove /model workaround"
```

---

## Task 3: Structured Compaction Status

The `kiro.dev/compaction/status` payload gained a structured `status` object with `type` (`started`/`completed`/`failed`) and optional `error`/`summary` fields. Cyril currently only reads the legacy `message` string.

**Files:**
- Modify: `crates/cyril-core/src/protocol/convert.rs:99-106` (parse structured status)
- Modify: `crates/cyril-ui/src/state.rs:287-290` (differentiate status types)

### Step 1: Write failing test

Add to convert.rs tests:

```rust
#[test]
fn parse_compaction_status_structured_started() {
    let params = serde_json::json!({
        "status": { "type": "started" }
    });
    let result = to_ext_notification("kiro.dev/compaction/status", &params);
    assert!(result.is_ok());
    if let Ok(Notification::CompactionStatus { message }) = result {
        assert!(message.contains("started") || message.contains("Compacting"));
    } else {
        panic!("expected CompactionStatus");
    }
}

#[test]
fn parse_compaction_status_structured_failed() {
    let params = serde_json::json!({
        "status": { "type": "failed", "error": "out of memory" }
    });
    let result = to_ext_notification("kiro.dev/compaction/status", &params);
    assert!(result.is_ok());
    if let Ok(Notification::CompactionStatus { message }) = result {
        assert!(message.contains("failed") || message.contains("out of memory"));
    } else {
        panic!("expected CompactionStatus");
    }
}

#[test]
fn parse_compaction_status_legacy_message() {
    let params = serde_json::json!({
        "message": "Compacting conversation context..."
    });
    let result = to_ext_notification("kiro.dev/compaction/status", &params);
    assert!(result.is_ok());
    if let Ok(Notification::CompactionStatus { message }) = result {
        assert_eq!(message, "Compacting conversation context...");
    } else {
        panic!("expected CompactionStatus");
    }
}
```

### Step 2: Run tests to verify they fail

Run: `cargo test -p cyril-core -- parse_compaction_status`
Expected: FAIL (structured format not parsed)

### Step 3: Update the conversion

Replace the `kiro.dev/compaction/status` arm in `convert.rs`:

```rust
        "kiro.dev/compaction/status" => {
            // v1.29.0 sends structured status; v1.28.0 sent just "message"
            let message = if let Some(status) = params.get("status") {
                let status_type = status
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("unknown");
                match status_type {
                    "started" => "Compacting conversation context...".to_string(),
                    "completed" => {
                        let summary = status
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
                // Legacy format
                params
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            Ok(Notification::CompactionStatus { message })
        }
```

### Step 4: Run tests

Run: `cargo test -p cyril-core -- parse_compaction_status`
Expected: PASS

### Step 5: Commit

```bash
git add crates/cyril-core/src/protocol/convert.rs
git commit -m "feat: handle structured compaction status (started/completed/failed)"
```

---

## Task 4: Prompt Support

Parse the `prompts` array from `commands/available`, register prompts as invocable slash commands, display argument hints. Prompts execute by forwarding `"/<name> args"` as a plain text message.

**Files:**
- Create: `crates/cyril-core/src/types/prompt.rs` (PromptInfo type)
- Modify: `crates/cyril-core/src/types/mod.rs` (add module + re-exports)
- Modify: `crates/cyril-core/src/types/event.rs` (add PromptsUpdated variant)
- Modify: `crates/cyril-core/src/protocol/convert.rs:127-180` (parse prompts array)
- Modify: `crates/cyril/src/app.rs:174-184` (register prompts alongside commands)

### Step 1: Create PromptInfo type

Create `crates/cyril-core/src/types/prompt.rs`:

```rust
/// A prompt argument definition.
#[derive(Debug, Clone)]
pub struct PromptArgument {
    name: String,
    description: Option<String>,
    required: bool,
}

impl PromptArgument {
    pub fn new(
        name: impl Into<String>,
        description: Option<impl Into<String>>,
        required: bool,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.map(Into::into),
            required,
        }
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn description(&self) -> Option<&str> { self.description.as_deref() }
    pub fn required(&self) -> bool { self.required }

    /// Format as hint: `<name>` if required, `[name]` if optional.
    pub fn hint(&self) -> String {
        if self.required {
            format!("<{}>", self.name)
        } else {
            format!("[{}]", self.name)
        }
    }
}

/// Metadata about an available prompt.
#[derive(Debug, Clone)]
pub struct PromptInfo {
    name: String,
    description: Option<String>,
    server_name: Option<String>,
    arguments: Vec<PromptArgument>,
}

impl PromptInfo {
    pub fn new(
        name: impl Into<String>,
        description: Option<impl Into<String>>,
        server_name: Option<impl Into<String>>,
        arguments: Vec<PromptArgument>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.map(Into::into),
            server_name: server_name.map(Into::into),
            arguments,
        }
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn description(&self) -> Option<&str> { self.description.as_deref() }
    pub fn server_name(&self) -> Option<&str> { self.server_name.as_deref() }
    pub fn arguments(&self) -> &[PromptArgument] { &self.arguments }

    /// Format argument hints for display: `<required> [optional]`
    pub fn argument_hints(&self) -> String {
        self.arguments
            .iter()
            .map(|a| a.hint())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argument_hint_formatting() {
        let required = PromptArgument::new("target", Some("file to review"), true);
        assert_eq!(required.hint(), "<target>");

        let optional = PromptArgument::new("depth", None::<String>, false);
        assert_eq!(optional.hint(), "[depth]");
    }

    #[test]
    fn prompt_argument_hints() {
        let prompt = PromptInfo::new(
            "review",
            Some("Review code"),
            Some("file-prompts"),
            vec![
                PromptArgument::new("branch", None::<String>, true),
                PromptArgument::new("scope", None::<String>, false),
            ],
        );
        assert_eq!(prompt.argument_hints(), "<branch> [scope]");
    }

    #[test]
    fn prompt_no_arguments() {
        let prompt = PromptInfo::new(
            "dg",
            Some("Code review"),
            Some("global"),
            vec![],
        );
        assert_eq!(prompt.argument_hints(), "");
    }
}
```

### Step 2: Register module and add Notification variant

In `types/mod.rs`, add:

```rust
pub mod prompt;
```

And re-export:

```rust
pub use prompt::{PromptArgument, PromptInfo};
```

In `event.rs`, add variant:

```rust
    PromptsUpdated(Vec<crate::types::PromptInfo>),
```

### Step 3: Parse prompts from commands/available

In the `kiro.dev/commands/available` arm of `to_ext_notification`, after parsing commands but before the `Ok(Notification::CommandsUpdated(commands))` return, parse the prompts array. Since `CommandsUpdated` only carries commands, emit a second notification or change the approach.

**Approach:** Emit both `CommandsUpdated` and `PromptsUpdated` from a single extension notification. But `to_ext_notification` returns a single `Result<Notification>`. Options:

**(A)** Return a vec: change signature to `-> Result<Vec<Notification>>`. Too invasive.

**(B)** Bundle prompts into the CommandsUpdated payload: change the variant to carry both. Cleaner.

**(C)** Parse prompts in the `commands/available` handler but emit them as a separate notification. The client.rs `ext_notification` handler would need to send multiple notifications.

**Recommended: (B)** — Change `CommandsUpdated` to also carry prompts:

```rust
    CommandsUpdated {
        commands: Vec<CommandInfo>,
        prompts: Vec<crate::types::PromptInfo>,
    },
```

This is a breaking change to the variant shape but keeps things simple. Update all match sites.

Alternatively, keep `CommandsUpdated(Vec<CommandInfo>)` unchanged and add a separate `PromptsUpdated` variant, then have the conversion return `CommandsUpdated` and have `client.rs` send a second `PromptsUpdated` notification by modifying `ext_notification` to detect when `commands/available` has prompts. This is messier.

**Simplest approach for now:** Parse prompts in the `commands/available` arm and include them in a new combined variant. Update all match sites.

Actually, the simplest minimal approach: parse prompts in the `commands/available` conversion and store them. Since the existing `CommandsUpdated` handler in `app.rs` already does custom work (registering commands), just parse prompts there from the raw notification. But the conversion layer has already parsed to typed data...

**Final decision:** Add prompts parsing to the `commands/available` conversion arm. Change `CommandsUpdated` to a struct variant:

```rust
    CommandsUpdated {
        commands: Vec<CommandInfo>,
        prompts: Vec<crate::types::PromptInfo>,
    },
```

Parse prompts in convert.rs:

```rust
            let prompts = params
                .get("prompts")
                .and_then(|p| p.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| {
                            let name = v.get("name")?.as_str()?;
                            let description = v.get("description")
                                .and_then(|d| d.as_str())
                                .map(String::from);
                            let server_name = v.get("serverName")
                                .and_then(|s| s.as_str())
                                .map(String::from);
                            let arguments = v.get("arguments")
                                .and_then(|a| a.as_array())
                                .map(|args| {
                                    args.iter()
                                        .filter_map(|arg| {
                                            let arg_name = arg.get("name")?.as_str()?;
                                            let required = arg.get("required")
                                                .and_then(|r| r.as_bool())
                                                .unwrap_or(false);
                                            let desc = arg.get("description")
                                                .and_then(|d| d.as_str())
                                                .map(String::from);
                                            Some(PromptArgument::new(arg_name, desc, required))
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();

                            Some(PromptInfo::new(name, description, server_name, arguments))
                        })
                        .collect()
                })
                .unwrap_or_default();

            Ok(Notification::CommandsUpdated { commands, prompts })
```

### Step 4: Update all CommandsUpdated match sites

Every place that matches `Notification::CommandsUpdated(cmds)` needs to change to `Notification::CommandsUpdated { commands, prompts }` (or `commands: cmds, prompts: _` where prompts aren't used):

- `crates/cyril-core/src/session.rs` — `commands: cmds, ..` or `commands: cmds, prompts: _`
- `crates/cyril-ui/src/state.rs` — `commands: _, ..` (returns false)
- `crates/cyril/src/app.rs` — use both `commands` and `prompts`
- `crates/cyril/examples/test_bridge.rs` — print both
- Tests in convert.rs that construct `CommandsUpdated`

### Step 5: Register prompts in App

In `app.rs`, update the `CommandsUpdated` handler to also register prompts:

```rust
    if let Notification::CommandsUpdated { ref commands, ref prompts } = notification {
        self.commands.register_agent_commands(commands);
        // TODO: Register prompts for autocomplete with argument hints
        let mut names: Vec<String> = self
            .commands
            .all_commands()
            .iter()
            .map(|cmd| cmd.name().to_string())
            .collect();
        // Add prompt names with "/" prefix for autocomplete
        for prompt in prompts {
            names.push(prompt.name().to_string());
        }
        self.ui_state.set_command_names(names);
    }
```

### Step 6: Write conversion test

```rust
#[test]
fn parse_commands_available_with_prompts() {
    let params = serde_json::json!({
        "commands": [
            { "name": "/help", "description": "Show help" }
        ],
        "prompts": [
            {
                "name": "review-pr",
                "description": "Review a PR",
                "serverName": "file-prompts",
                "arguments": [
                    { "name": "branch", "required": true },
                    { "name": "scope", "required": false }
                ]
            }
        ],
        "tools": [],
        "mcpServers": []
    });
    let result = to_ext_notification("kiro.dev/commands/available", &params);
    assert!(result.is_ok());
    if let Ok(Notification::CommandsUpdated { commands, prompts }) = result {
        assert_eq!(commands.len(), 1);
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].name(), "review-pr");
        assert_eq!(prompts[0].arguments().len(), 2);
        assert!(prompts[0].arguments()[0].required());
        assert!(!prompts[0].arguments()[1].required());
        assert_eq!(prompts[0].argument_hints(), "<branch> [scope]");
    } else {
        panic!("expected CommandsUpdated");
    }
}

#[test]
fn parse_commands_available_no_prompts() {
    let params = serde_json::json!({
        "commands": [{ "name": "/help", "description": "Show help" }]
    });
    let result = to_ext_notification("kiro.dev/commands/available", &params);
    assert!(result.is_ok());
    if let Ok(Notification::CommandsUpdated { prompts, .. }) = result {
        assert!(prompts.is_empty());
    } else {
        panic!("expected CommandsUpdated");
    }
}
```

### Step 7: Run tests

Run: `cargo test -p cyril-core`
Expected: PASS

Run: `cargo check`
Expected: PASS

### Step 8: Commit

```bash
git add crates/cyril-core/src/types/prompt.rs crates/cyril-core/src/types/mod.rs crates/cyril-core/src/types/event.rs crates/cyril-core/src/protocol/convert.rs crates/cyril-core/src/session.rs crates/cyril-ui/src/state.rs crates/cyril/src/app.rs crates/cyril/examples/test_bridge.rs
git commit -m "feat: parse prompts from commands/available with argument support"
```

---

## Task 5: Turn Cost Display via kiro.dev/metadata Metering

**Verified empirically:** The `kiro.dev/metadata` notification carries `meteringUsage` and `turnDurationMs` after each prompt turn. The ACP crate's `UsageUpdate` session update (`unstable_session_usage`) compiles but Kiro v1.29.0 does NOT send it. Token-level counts (`inputTokens`, `outputTokens`) are not accessible through the ACP client interface. Credit cost per turn is the available data.

Post-turn metadata payload (captured from test harness):
```json
{
  "contextUsagePercentage": 7.11,
  "meteringUsage": [
    {"unit": "credit", "unitPlural": "credits", "value": 0.018139567827529027}
  ],
  "turnDurationMs": 1948,
  "sessionId": "00d1ab84-..."
}
```

**Files:**
- Modify: `crates/cyril-core/src/types/event.rs` (extend ContextUsageUpdated or add new variant)
- Modify: `crates/cyril-core/src/types/session.rs` (add TurnMetering type)
- Modify: `crates/cyril-core/src/protocol/convert.rs:92-98` (parse metering from metadata)
- Modify: `crates/cyril-core/src/session.rs` (store last turn cost)
- Modify: `crates/cyril-ui/src/state.rs` (display in toolbar)

### Step 1: Write failing conversion test

Add to convert.rs tests:

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
    assert!(result.is_ok());
    // Verify metering data is captured (exact assertion depends on approach)
}

#[test]
fn parse_metadata_without_metering() {
    let params = serde_json::json!({
        "sessionId": "s1",
        "contextUsagePercentage": 2.28
    });
    let result = to_ext_notification("kiro.dev/metadata", &params);
    assert!(result.is_ok());
    // Should still work — metering is optional
}
```

### Step 2: Add TurnMetering and SessionCost types

In `crates/cyril-core/src/types/session.rs`:

```rust
/// Per-turn metering data from kiro.dev/metadata.
#[derive(Debug, Clone)]
pub struct TurnMetering {
    /// Credits consumed by this turn.
    pub credits: f64,
    /// Wall-clock duration of the turn in milliseconds.
    pub duration_ms: Option<u64>,
}

/// Running session cost accumulator.
#[derive(Debug, Clone, Default)]
pub struct SessionCost {
    /// Total credits consumed across all turns in this session.
    total_credits: f64,
    /// Number of turns completed.
    turn_count: u32,
    /// Credits consumed by the most recent turn.
    last_turn_credits: Option<f64>,
    /// Duration of the most recent turn.
    last_turn_duration_ms: Option<u64>,
}

impl SessionCost {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a completed turn's metering data.
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

    /// Format duration as human-readable string (e.g., "1.9s", "2m 15s").
    pub fn last_turn_duration_display(&self) -> Option<String> {
        self.last_turn_duration_ms.map(|ms| {
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
```

### Step 3: Decide notification approach

Two options:

**(A) Extend ContextUsageUpdated** to carry optional metering:
```rust
ContextUsageUpdated {
    usage: ContextUsage,
    metering: Option<TurnMetering>,
}
```
This changes the variant shape but keeps one notification.

**(B) Emit a separate TurnMeteringUpdated notification** alongside ContextUsageUpdated. Cleaner but requires the conversion to return two notifications (same issue as prompts — `to_ext_notification` returns one).

**Recommended: (A)** — extend the existing variant. The metering always arrives with the context percentage on the same notification.

### Step 4: Update conversion

In `convert.rs`, update the `kiro.dev/metadata` arm:

```rust
        "kiro.dev/metadata" => {
            let pct = params
                .get("contextUsagePercentage")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            let metering = params
                .get("meteringUsage")
                .and_then(|m| m.as_array())
                .map(|arr| {
                    let credits: f64 = arr
                        .iter()
                        .filter_map(|u| u.get("value").and_then(|v| v.as_f64()))
                        .sum();
                    let duration_ms = params
                        .get("turnDurationMs")
                        .and_then(|d| d.as_u64());
                    TurnMetering { credits, duration_ms }
                });

            Ok(Notification::ContextUsageUpdated(ContextUsage::new(pct)))
            // TODO: include metering in the notification
        }
```

### Step 5: Update all ContextUsageUpdated match sites

Fix destructuring in SessionController, UiState, and test harness to handle the new variant shape.

### Step 6: Store running total in SessionController

Add a `session_cost: SessionCost` field to `SessionController`. In `apply_notification`:

```rust
        Notification::ContextUsageUpdated { usage, metering } => {
            self.context_usage = Some(usage.clone());
            if let Some(ref m) = metering {
                self.session_cost.record_turn(m);
            }
            true
        }
```

Expose via accessor:

```rust
    pub fn session_cost(&self) -> &SessionCost { &self.session_cost }
```

### Step 7: Surface cost data in UiState

Add fields to `UiState`:

```rust
    /// Running total of credits consumed this session.
    total_credits: f64,
    /// Credits consumed by the most recent turn.
    last_turn_credits: Option<f64>,
    /// Duration of the most recent turn.
    last_turn_duration_ms: Option<u64>,
```

In `apply_notification`:

```rust
        Notification::ContextUsageUpdated { usage, metering } => {
            self.context_usage = Some(usage.percentage());
            if let Some(ref m) = metering {
                self.total_credits += m.credits;
                self.last_turn_credits = Some(m.credits);
                self.last_turn_duration_ms = m.duration_ms;
            }
            true
        }
```

Add to `TuiState` trait:

```rust
    fn total_credits(&self) -> f64;
    fn last_turn_credits(&self) -> Option<f64>;
    fn last_turn_duration_display(&self) -> Option<String>;
```

### Step 8: Display in toolbar

Update the toolbar rendering to show cost info alongside context usage. Layout:

```
                                    ctx 7.1% | 0.018 cr (0.54 total) | 1.9s
```

- **`0.018 cr`** — last turn cost (only shown after first turn completes)
- **`(0.54 total)`** — running session total
- **`1.9s`** — last turn duration

When no metering has arrived yet (session just created), only show context percentage as before. The display is additive — no visual change until the first turn completes.

Format credits to 3 decimal places. Format duration using `SessionCost::last_turn_duration_display()`.

### Step 9: Write display tests

```rust
#[test]
fn session_cost_accumulates() {
    let mut cost = SessionCost::new();
    cost.record_turn(&TurnMetering { credits: 0.018, duration_ms: Some(1948) });
    cost.record_turn(&TurnMetering { credits: 0.042, duration_ms: Some(5200) });

    assert_eq!(cost.turn_count(), 2);
    assert!((cost.total_credits() - 0.060).abs() < 0.001);
    assert!((cost.last_turn_credits().unwrap() - 0.042).abs() < 0.001);
    assert_eq!(cost.last_turn_duration_ms(), Some(5200));
}

#[test]
fn duration_display_formatting() {
    let mut cost = SessionCost::new();
    cost.record_turn(&TurnMetering { credits: 0.01, duration_ms: Some(500) });
    assert_eq!(cost.last_turn_duration_display(), Some("500ms".into()));

    cost.record_turn(&TurnMetering { credits: 0.01, duration_ms: Some(1948) });
    assert_eq!(cost.last_turn_duration_display(), Some("1.9s".into()));

    cost.record_turn(&TurnMetering { credits: 0.01, duration_ms: Some(135000) });
    assert_eq!(cost.last_turn_duration_display(), Some("2m 15s".into()));
}

#[test]
fn session_cost_no_metering() {
    let cost = SessionCost::new();
    assert_eq!(cost.total_credits(), 0.0);
    assert_eq!(cost.turn_count(), 0);
    assert!(cost.last_turn_credits().is_none());
    assert!(cost.last_turn_duration_display().is_none());
}
```

### Step 10: Run tests

Run: `cargo test`
Expected: PASS

### Step 11: Commit

```bash
git add crates/cyril-core/src/types/event.rs crates/cyril-core/src/types/session.rs crates/cyril-core/src/protocol/convert.rs crates/cyril-core/src/session.rs crates/cyril-ui/src/state.rs crates/cyril-ui/src/traits.rs
git commit -m "feat: display per-turn and cumulative credit cost with turn duration"
```
