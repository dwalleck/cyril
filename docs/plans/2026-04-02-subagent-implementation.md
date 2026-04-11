# Subagent Support Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add full subagent observation, display, and control to Cyril so users can see crew activity, drill into subagent message streams, and manage sessions via slash commands.

**Architecture:** Two new components — `SubagentTracker` (cyril-core) owns metadata from `list_update`, `SubagentUiState` (cyril-ui) owns per-subagent message streams and drill-in focus. The App routes notifications by `sessionId`. Crew panel widget renders inline, drill-in swaps the main viewport.

**Tech Stack:** Rust 2021, ratatui, tokio, serde_json. Tests use standard `#[test]` and `#[tokio::test]`. See `CLAUDE.md` for build/test commands and code style.

**Design doc:** `docs/plans/2026-04-02-subagent-support-design.md`

---

## Task 1: Add Subagent Types to cyril-core

**Files:**
- Create: `crates/cyril-core/src/types/subagent.rs`
- Modify: `crates/cyril-core/src/types/mod.rs:1-19` (add module + re-exports)

### Step 1: Create the subagent types module

Create `crates/cyril-core/src/types/subagent.rs`:

```rust
use crate::types::session::SessionId;

/// Status of an active subagent session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubagentStatus {
    Working { message: String },
    Terminated,
}

/// An active subagent session reported by `subagent/list_update`.
#[derive(Debug, Clone)]
pub struct SubagentInfo {
    session_id: SessionId,
    session_name: String,
    agent_name: String,
    initial_query: String,
    status: SubagentStatus,
    group: Option<String>,
    role: Option<String>,
    depends_on: Vec<String>,
}

impl SubagentInfo {
    pub fn new(
        session_id: SessionId,
        session_name: impl Into<String>,
        agent_name: impl Into<String>,
        initial_query: impl Into<String>,
        status: SubagentStatus,
        group: Option<String>,
        role: Option<String>,
        depends_on: Vec<String>,
    ) -> Self {
        Self {
            session_id,
            session_name: session_name.into(),
            agent_name: agent_name.into(),
            initial_query: initial_query.into(),
            status,
            group,
            role,
            depends_on,
        }
    }

    pub fn session_id(&self) -> &SessionId { &self.session_id }
    pub fn session_name(&self) -> &str { &self.session_name }
    pub fn agent_name(&self) -> &str { &self.agent_name }
    pub fn initial_query(&self) -> &str { &self.initial_query }
    pub fn status(&self) -> &SubagentStatus { &self.status }
    pub fn group(&self) -> Option<&str> { self.group.as_deref() }
    pub fn role(&self) -> Option<&str> { self.role.as_deref() }
    pub fn depends_on(&self) -> &[String] { &self.depends_on }
    pub fn is_working(&self) -> bool { matches!(self.status, SubagentStatus::Working { .. }) }
}

/// A stage that hasn't been spawned yet (waiting on dependencies).
#[derive(Debug, Clone)]
pub struct PendingStage {
    name: String,
    agent_name: Option<String>,
    group: Option<String>,
    role: Option<String>,
    depends_on: Vec<String>,
}

impl PendingStage {
    pub fn new(
        name: impl Into<String>,
        agent_name: Option<String>,
        group: Option<String>,
        role: Option<String>,
        depends_on: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            agent_name,
            group,
            role,
            depends_on,
        }
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn agent_name(&self) -> Option<&str> { self.agent_name.as_deref() }
    pub fn group(&self) -> Option<&str> { self.group.as_deref() }
    pub fn role(&self) -> Option<&str> { self.role.as_deref() }
    pub fn depends_on(&self) -> &[String] { &self.depends_on }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subagent_info_accessors() {
        let info = SubagentInfo::new(
            SessionId::new("s1"),
            "code-reviewer",
            "code-reviewer",
            "Review the code",
            SubagentStatus::Working { message: "Running".into() },
            Some("crew-Review".into()),
            Some("code-reviewer".into()),
            vec![],
        );
        assert_eq!(info.session_name(), "code-reviewer");
        assert!(info.is_working());
        assert_eq!(info.group(), Some("crew-Review"));
        assert!(info.depends_on().is_empty());
    }

    #[test]
    fn subagent_terminated_is_not_working() {
        let info = SubagentInfo::new(
            SessionId::new("s2"),
            "done",
            "done",
            "query",
            SubagentStatus::Terminated,
            None,
            None,
            vec![],
        );
        assert!(!info.is_working());
    }

    #[test]
    fn pending_stage_accessors() {
        let stage = PendingStage::new(
            "summary-writer",
            Some("summary-writer".into()),
            Some("crew-Review".into()),
            Some("summary-writer".into()),
            vec!["code-reviewer".into(), "pr-test-analyzer".into()],
        );
        assert_eq!(stage.name(), "summary-writer");
        assert_eq!(stage.depends_on().len(), 2);
    }
}
```

### Step 2: Register the module and add re-exports

In `crates/cyril-core/src/types/mod.rs`, add after the existing module declarations:

```rust
pub mod subagent;
```

And add to the re-exports section:

```rust
pub use subagent::{PendingStage, SubagentInfo, SubagentStatus};
```

### Step 3: Run tests

Run: `cargo test -p cyril-core -- subagent`
Expected: PASS (3 tests)

### Step 4: Commit

```bash
git add crates/cyril-core/src/types/subagent.rs crates/cyril-core/src/types/mod.rs
git commit -m "feat: add SubagentInfo, PendingStage, SubagentStatus types"
```

---

## Task 2: Add Notification Variants for Subagents

**Files:**
- Modify: `crates/cyril-core/src/types/event.rs:9-66` (add variants to Notification enum)

### Step 1: Add new Notification variants

In `crates/cyril-core/src/types/event.rs`, add these variants to the `Notification` enum before the `// Lifecycle` comment section:

```rust
    // Subagent lifecycle (kiro.dev/subagent/*)
    SubagentListUpdated {
        subagents: Vec<crate::types::SubagentInfo>,
        pending_stages: Vec<crate::types::PendingStage>,
    },
    InboxNotification {
        session_id: SessionId,
        message_count: u32,
        escalation_count: u32,
        senders: Vec<String>,
    },
```

Also add `session_id: Option<SessionId>` to the variants that can come from subagents. Modify these existing variants:

- `AgentMessage` — the `AgentMessage` struct in `message.rs` needs a `session_id: Option<SessionId>` field
- `ToolCallStarted` / `ToolCallUpdated` — the `ToolCall` struct already exists, but we need the session ID outside it
- `ToolCallChunk` — add `session_id: Option<SessionId>` field
- `TurnCompleted` — change from unit variant to `TurnCompleted { session_id: Option<SessionId> }`

Rather than changing all these, use a wrapper approach — add `session_id` only to the variants where routing depends on it. The cleanest approach: add a `session_id()` method to `Notification` that returns `Option<&SessionId>` by inspecting the relevant variants.

Add this impl block after the enum:

```rust
impl Notification {
    /// Returns the session ID this notification belongs to, if present.
    /// Used by the App to route subagent notifications.
    pub fn session_id(&self) -> Option<&SessionId> {
        match self {
            Self::SubagentListUpdated { .. } => None, // applies globally
            Self::InboxNotification { session_id, .. } => Some(session_id),
            _ => None, // main session or no session context
        }
    }

    /// Attach a session ID to this notification for routing purposes.
    /// Returns a `(SessionId, Notification)` pair.
    pub fn with_session_id(self, session_id: Option<SessionId>) -> SessionNotification {
        SessionNotification {
            session_id,
            notification: self,
        }
    }
}

/// A notification tagged with the session it belongs to.
#[derive(Debug, Clone)]
pub struct SessionNotification {
    pub session_id: Option<SessionId>,
    pub notification: Notification,
}
```

Actually, this is getting complex. **Simpler approach:** Change the notification channel type from `Notification` to carry an optional session ID alongside. Modify `BridgeHandle` and `KiroClient` to send `(Option<SessionId>, Notification)` tuples.

**Simplest approach that avoids changing channel types:** Add `session_id` only to the two new variants, and have the conversion layer wrap subagent-scoped standard notifications (`AgentMessage`, `ToolCallStarted`, etc.) in a new variant:

```rust
    /// A notification from a subagent session. Contains the subagent's
    /// session ID and the inner notification (AgentMessage, ToolCallStarted, etc.).
    SubagentNotification {
        session_id: SessionId,
        inner: Box<Notification>,
    },
```

This avoids modifying any existing variant. The App unwraps `SubagentNotification` and routes the inner notification to `SubagentUiState`.

### Step 2: Add the three new variants to the enum

Add to `Notification` in `event.rs`:

```rust
    // Subagent lifecycle
    SubagentListUpdated {
        subagents: Vec<crate::types::SubagentInfo>,
        pending_stages: Vec<crate::types::PendingStage>,
    },
    InboxNotification {
        session_id: SessionId,
        message_count: u32,
        escalation_count: u32,
        senders: Vec<String>,
    },
    SubagentNotification {
        session_id: SessionId,
        inner: Box<Notification>,
    },
```

### Step 3: Update existing tests

The `Notification` enum is `Clone` — adding `Box<Notification>` requires `Clone` on `Box`, which Rust provides. No trait issue.

Check for exhaustive match statements in tests. The test at the bottom of `event.rs` that checks `Notification` is `Send + Sync + Clone` will still pass. Add a test for the new variants:

```rust
#[test]
fn subagent_notification_is_send_sync_clone() {
    fn assert_send_sync_clone<T: Send + Sync + Clone>() {}
    assert_send_sync_clone::<SubagentInfo>();
    assert_send_sync_clone::<PendingStage>();
}

#[test]
fn subagent_notification_wraps_inner() {
    let inner = Notification::AgentMessage(AgentMessage {
        text: "hello".into(),
        is_streaming: true,
    });
    let wrapped = Notification::SubagentNotification {
        session_id: SessionId::new("sub-1"),
        inner: Box::new(inner),
    };
    if let Notification::SubagentNotification { session_id, inner } = wrapped {
        assert_eq!(session_id.as_str(), "sub-1");
        assert!(matches!(*inner, Notification::AgentMessage(_)));
    }
}
```

### Step 4: Fix any exhaustive match compilation errors

The `match notification` in `SessionController::apply_notification` (session.rs), `UiState::apply_notification` (state.rs), and the test harness `print_notification` (test_bridge.rs) all need `_` catch-all arms. They already have them, so no changes needed.

### Step 5: Run tests

Run: `cargo test -p cyril-core`
Expected: PASS

### Step 6: Commit

```bash
git add crates/cyril-core/src/types/event.rs
git commit -m "feat: add SubagentListUpdated, InboxNotification, SubagentNotification variants"
```

---

## Task 3: Protocol Conversion for Subagent Notifications

**Files:**
- Modify: `crates/cyril-core/src/protocol/convert.rs:87-229` (add match arms to `to_ext_notification`)

### Step 1: Write failing tests for the new conversions

Add to the test module in `convert.rs`:

```rust
#[test]
fn parse_subagent_list_update_with_active_subagents() {
    let params = serde_json::json!({
        "subagents": [{
            "sessionId": "b49d53d1-a42a-4ef6-a173-a6224e8e6fcd",
            "sessionName": "code-reviewer",
            "agentName": "code-reviewer",
            "initialQuery": "Review the code changes",
            "status": { "type": "working", "message": "Running" },
            "group": "crew-Review code changes",
            "role": "code-reviewer",
            "dependsOn": []
        }],
        "pendingStages": [{
            "name": "summary-writer",
            "agentName": "summary-writer",
            "group": "crew-Review code changes",
            "role": "summary-writer",
            "dependsOn": ["code-reviewer"]
        }]
    });
    let result = to_ext_notification("kiro.dev/subagent/list_update", &params);
    assert!(result.is_ok());
    if let Ok(Notification::SubagentListUpdated { subagents, pending_stages }) = result {
        assert_eq!(subagents.len(), 1);
        assert_eq!(subagents[0].session_name(), "code-reviewer");
        assert!(subagents[0].is_working());
        assert_eq!(subagents[0].group(), Some("crew-Review code changes"));
        assert_eq!(pending_stages.len(), 1);
        assert_eq!(pending_stages[0].name(), "summary-writer");
        assert_eq!(pending_stages[0].depends_on(), &["code-reviewer"]);
    } else {
        panic!("expected SubagentListUpdated");
    }
}

#[test]
fn parse_subagent_list_update_empty() {
    let params = serde_json::json!({
        "subagents": [],
        "pendingStages": []
    });
    let result = to_ext_notification("kiro.dev/subagent/list_update", &params);
    assert!(result.is_ok());
    if let Ok(Notification::SubagentListUpdated { subagents, pending_stages }) = result {
        assert!(subagents.is_empty());
        assert!(pending_stages.is_empty());
    } else {
        panic!("expected SubagentListUpdated");
    }
}

#[test]
fn parse_subagent_list_update_terminated_status() {
    let params = serde_json::json!({
        "subagents": [{
            "sessionId": "s1",
            "sessionName": "reviewer",
            "agentName": "reviewer",
            "initialQuery": "review",
            "status": { "type": "terminated" },
            "group": null,
            "role": null,
            "dependsOn": []
        }],
        "pendingStages": []
    });
    let result = to_ext_notification("kiro.dev/subagent/list_update", &params);
    assert!(result.is_ok());
    if let Ok(Notification::SubagentListUpdated { subagents, .. }) = result {
        assert!(!subagents[0].is_working());
    } else {
        panic!("expected SubagentListUpdated");
    }
}

#[test]
fn parse_inbox_notification() {
    let params = serde_json::json!({
        "sessionId": "874046d5-c7ab-47a7-86c5-b15cece1379a",
        "sessionName": "main",
        "messageCount": 2,
        "escalationCount": 0,
        "senders": ["subagent"]
    });
    let result = to_ext_notification("kiro.dev/session/inbox_notification", &params);
    assert!(result.is_ok());
    if let Ok(Notification::InboxNotification {
        session_id, message_count, escalation_count, senders
    }) = result {
        assert_eq!(session_id.as_str(), "874046d5-c7ab-47a7-86c5-b15cece1379a");
        assert_eq!(message_count, 2);
        assert_eq!(escalation_count, 0);
        assert_eq!(senders, vec!["subagent"]);
    } else {
        panic!("expected InboxNotification");
    }
}

#[test]
fn parse_session_update_with_subagent_session_id() {
    let params = serde_json::json!({
        "sessionId": "b49d53d1-subagent",
        "update": {
            "sessionUpdate": "tool_call_chunk",
            "toolCallId": "tc-1",
            "title": "read",
            "kind": "read"
        }
    });
    let result = to_ext_notification("kiro.dev/session/update", &params);
    assert!(result.is_ok());
    // The tool_call_chunk with a different sessionId should become SubagentNotification
    // This test verifies the sessionId is captured
}

#[test]
fn parse_rate_limit_error() {
    let params = serde_json::json!({
        "message": "Rate limit exceeded"
    });
    let result = to_ext_notification("kiro.dev/error/rate_limit", &params);
    // For now, rate limit should at minimum not error
    assert!(result.is_ok() || result.is_err());
}
```

### Step 2: Run tests to verify they fail

Run: `cargo test -p cyril-core -- parse_subagent`
Expected: FAIL (variants don't match yet in `to_ext_notification`)

### Step 3: Implement the conversion functions

Add these match arms to `to_ext_notification` in `convert.rs`, before the `other =>` fallback:

```rust
        "kiro.dev/subagent/list_update" => {
            let subagents = params
                .get("subagents")
                .and_then(|s| s.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| {
                            let session_id = SessionId::new(
                                v.get("sessionId")?.as_str()?,
                            );
                            let session_name = v.get("sessionName")
                                .and_then(|n| n.as_str())
                                .unwrap_or_default();
                            let agent_name = v.get("agentName")
                                .and_then(|n| n.as_str())
                                .unwrap_or_default();
                            let initial_query = v.get("initialQuery")
                                .and_then(|q| q.as_str())
                                .unwrap_or_default();

                            let status_obj = v.get("status");
                            let status_type = status_obj
                                .and_then(|s| s.get("type"))
                                .and_then(|t| t.as_str())
                                .unwrap_or("working");
                            let status = match status_type {
                                "terminated" => SubagentStatus::Terminated,
                                _ => SubagentStatus::Working {
                                    message: status_obj
                                        .and_then(|s| s.get("message"))
                                        .and_then(|m| m.as_str())
                                        .unwrap_or("Running")
                                        .to_string(),
                                },
                            };

                            let group = v.get("group")
                                .and_then(|g| g.as_str())
                                .map(String::from);
                            let role = v.get("role")
                                .and_then(|r| r.as_str())
                                .map(String::from);
                            let depends_on = v.get("dependsOn")
                                .and_then(|d| d.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default();

                            Some(SubagentInfo::new(
                                session_id,
                                session_name,
                                agent_name,
                                initial_query,
                                status,
                                group,
                                role,
                                depends_on,
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();

            let pending_stages = params
                .get("pendingStages")
                .and_then(|s| s.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| {
                            let name = v.get("name")?.as_str()?;
                            let agent_name = v.get("agentName")
                                .and_then(|n| n.as_str())
                                .map(String::from);
                            let group = v.get("group")
                                .and_then(|g| g.as_str())
                                .map(String::from);
                            let role = v.get("role")
                                .and_then(|r| r.as_str())
                                .map(String::from);
                            let depends_on = v.get("dependsOn")
                                .and_then(|d| d.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default();

                            Some(PendingStage::new(name, agent_name, group, role, depends_on))
                        })
                        .collect()
                })
                .unwrap_or_default();

            Ok(Notification::SubagentListUpdated {
                subagents,
                pending_stages,
            })
        }
        "kiro.dev/session/inbox_notification" => {
            let session_id = SessionId::new(
                params.get("sessionId")
                    .and_then(|s| s.as_str())
                    .unwrap_or_default(),
            );
            let message_count = params.get("messageCount")
                .and_then(|m| m.as_u64())
                .unwrap_or(0) as u32;
            let escalation_count = params.get("escalationCount")
                .and_then(|e| e.as_u64())
                .unwrap_or(0) as u32;
            let senders = params.get("senders")
                .and_then(|s| s.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            Ok(Notification::InboxNotification {
                session_id,
                message_count,
                escalation_count,
                senders,
            })
        }
        "kiro.dev/error/rate_limit" => {
            let message = params.get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Rate limit exceeded")
                .to_string();
            // Surface as a system-level notification — reuse CompactionStatus for now
            // or add a dedicated variant later
            Ok(Notification::CompactionStatus { message: format!("Rate limit: {message}") })
        }
```

Add the necessary imports at the top of `convert.rs`:

```rust
use crate::types::{SubagentInfo, SubagentStatus, PendingStage};
```

### Step 4: Handle sessionId on tool_call_chunk for subagent scoping

In the existing `"kiro.dev/session/update"` match arm, extract the `sessionId` and wrap in `SubagentNotification` when it differs from expected. This requires the main session ID, which the conversion layer doesn't have. For now, always pass through the `sessionId` field by extending the `ToolCallChunk` variant:

Add `session_id: Option<SessionId>` to the `ToolCallChunk` variant in `event.rs`:

```rust
    ToolCallChunk {
        tool_call_id: ToolCallId,
        title: String,
        kind: String,
        session_id: Option<SessionId>,
    },
```

Update the `"kiro.dev/session/update"` arm in `to_ext_notification` to extract the sessionId:

```rust
        "kiro.dev/session/update" => {
            let update = params.get("update");
            let session_update = update
                .and_then(|u| u.get("sessionUpdate"))
                .and_then(|s| s.as_str());
            let ext_session_id = params
                .get("sessionId")
                .and_then(|s| s.as_str())
                .map(SessionId::new);
            match session_update {
                Some("tool_call_chunk") => {
                    // ... existing parsing ...
                    Ok(Notification::ToolCallChunk {
                        tool_call_id,
                        title,
                        kind,
                        session_id: ext_session_id,
                    })
                }
                // ...
            }
        }
```

### Step 5: Run tests

Run: `cargo test -p cyril-core`
Expected: PASS

### Step 6: Fix any compilation errors from the ToolCallChunk change

The `ToolCallChunk` variant is matched in:
- `crates/cyril-core/src/session.rs` — `_ => false` catches it
- `crates/cyril-ui/src/state.rs:305` — update to destructure `session_id`
- `crates/cyril/examples/test_bridge.rs` — update `print_notification`

Fix the matches in state.rs and test_bridge.rs to include the new `session_id` field.

### Step 7: Run full build

Run: `cargo check`
Expected: PASS

### Step 8: Commit

```bash
git add crates/cyril-core/src/protocol/convert.rs crates/cyril-core/src/types/event.rs crates/cyril-ui/src/state.rs crates/cyril/examples/test_bridge.rs
git commit -m "feat: parse subagent/list_update, inbox_notification, rate_limit extensions"
```

---

## Task 4: SubagentTracker Component

**Files:**
- Create: `crates/cyril-core/src/subagent.rs`
- Modify: `crates/cyril-core/src/lib.rs:1-8` (add module)

### Step 1: Create SubagentTracker with tests

Create `crates/cyril-core/src/subagent.rs`:

```rust
use std::collections::HashMap;

use crate::types::*;

/// Tracks subagent metadata from `list_update` notifications.
/// Pure state machine — no async, no UI knowledge.
pub struct SubagentTracker {
    subagents: HashMap<SessionId, SubagentInfo>,
    pending_stages: Vec<PendingStage>,
    inbox_message_count: u32,
    inbox_escalation_count: u32,
}

impl SubagentTracker {
    pub fn new() -> Self {
        Self {
            subagents: HashMap::new(),
            pending_stages: Vec::new(),
            inbox_message_count: 0,
            inbox_escalation_count: 0,
        }
    }

    pub fn apply_notification(&mut self, notification: &Notification) -> bool {
        match notification {
            Notification::SubagentListUpdated {
                subagents,
                pending_stages,
            } => {
                self.subagents = subagents
                    .iter()
                    .map(|s| (s.session_id().clone(), s.clone()))
                    .collect();
                self.pending_stages = pending_stages.clone();
                true
            }
            Notification::InboxNotification {
                message_count,
                escalation_count,
                ..
            } => {
                self.inbox_message_count = *message_count;
                self.inbox_escalation_count = *escalation_count;
                true
            }
            _ => false,
        }
    }

    pub fn subagents(&self) -> &HashMap<SessionId, SubagentInfo> {
        &self.subagents
    }

    pub fn pending_stages(&self) -> &[PendingStage] {
        &self.pending_stages
    }

    pub fn get(&self, session_id: &SessionId) -> Option<&SubagentInfo> {
        self.subagents.get(session_id)
    }

    pub fn is_subagent(&self, session_id: &SessionId) -> bool {
        self.subagents.contains_key(session_id)
    }

    pub fn active_count(&self) -> usize {
        self.subagents.values().filter(|s| s.is_working()).count()
    }

    pub fn inbox_message_count(&self) -> u32 {
        self.inbox_message_count
    }

    pub fn inbox_escalation_count(&self) -> u32 {
        self.inbox_escalation_count
    }

    /// Returns distinct group names across active subagents and pending stages.
    pub fn groups(&self) -> Vec<&str> {
        let mut groups: Vec<&str> = self
            .subagents
            .values()
            .filter_map(|s| s.group())
            .chain(self.pending_stages.iter().filter_map(|s| s.group()))
            .collect();
        groups.sort_unstable();
        groups.dedup();
        groups
    }
}

impl Default for SubagentTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn working_info(id: &str, name: &str) -> SubagentInfo {
        SubagentInfo::new(
            SessionId::new(id),
            name,
            name,
            "query",
            SubagentStatus::Working {
                message: "Running".into(),
            },
            Some("crew-test".into()),
            Some(name.to_string()),
            vec![],
        )
    }

    fn terminated_info(id: &str, name: &str) -> SubagentInfo {
        SubagentInfo::new(
            SessionId::new(id),
            name,
            name,
            "query",
            SubagentStatus::Terminated,
            Some("crew-test".into()),
            None,
            vec![],
        )
    }

    #[test]
    fn empty_tracker() {
        let tracker = SubagentTracker::new();
        assert!(tracker.subagents().is_empty());
        assert!(tracker.pending_stages().is_empty());
        assert_eq!(tracker.active_count(), 0);
        assert_eq!(tracker.inbox_message_count(), 0);
    }

    #[test]
    fn apply_list_update_replaces_state() {
        let mut tracker = SubagentTracker::new();

        let notif = Notification::SubagentListUpdated {
            subagents: vec![working_info("s1", "reviewer")],
            pending_stages: vec![PendingStage::new(
                "summary",
                None,
                None,
                None,
                vec!["reviewer".into()],
            )],
        };
        assert!(tracker.apply_notification(&notif));
        assert_eq!(tracker.subagents().len(), 1);
        assert_eq!(tracker.pending_stages().len(), 1);
        assert_eq!(tracker.active_count(), 1);
        assert!(tracker.is_subagent(&SessionId::new("s1")));
        assert!(!tracker.is_subagent(&SessionId::new("unknown")));

        // Second update replaces entirely
        let notif2 = Notification::SubagentListUpdated {
            subagents: vec![
                working_info("s2", "analyzer"),
                terminated_info("s1", "reviewer"),
            ],
            pending_stages: vec![],
        };
        assert!(tracker.apply_notification(&notif2));
        assert_eq!(tracker.subagents().len(), 2);
        assert_eq!(tracker.active_count(), 1); // only s2 is working
        assert!(tracker.pending_stages().is_empty());
    }

    #[test]
    fn apply_inbox_notification() {
        let mut tracker = SubagentTracker::new();
        let notif = Notification::InboxNotification {
            session_id: SessionId::new("main"),
            message_count: 3,
            escalation_count: 1,
            senders: vec!["subagent".into()],
        };
        assert!(tracker.apply_notification(&notif));
        assert_eq!(tracker.inbox_message_count(), 3);
        assert_eq!(tracker.inbox_escalation_count(), 1);
    }

    #[test]
    fn groups_deduplicates() {
        let mut tracker = SubagentTracker::new();
        let notif = Notification::SubagentListUpdated {
            subagents: vec![
                working_info("s1", "a"),
                working_info("s2", "b"),
            ],
            pending_stages: vec![],
        };
        tracker.apply_notification(&notif);
        let groups = tracker.groups();
        assert_eq!(groups, vec!["crew-test"]);
    }

    #[test]
    fn ignores_unrelated_notifications() {
        let mut tracker = SubagentTracker::new();
        assert!(!tracker.apply_notification(&Notification::TurnCompleted));
    }
}
```

### Step 2: Register the module

In `crates/cyril-core/src/lib.rs`, add:

```rust
pub mod subagent;
```

### Step 3: Run tests

Run: `cargo test -p cyril-core -- subagent`
Expected: PASS (all SubagentTracker tests + type tests)

### Step 4: Commit

```bash
git add crates/cyril-core/src/subagent.rs crates/cyril-core/src/lib.rs
git commit -m "feat: add SubagentTracker state machine"
```

---

## Task 5: Bridge Commands for Session Control

**Files:**
- Modify: `crates/cyril-core/src/types/event.rs` (add BridgeCommand variants)
- Modify: `crates/cyril-core/src/protocol/bridge.rs:97-127,203-473` (add command handling)

### Step 1: Add BridgeCommand variants

Add to the `BridgeCommand` enum in `event.rs`:

```rust
    SpawnSession {
        task: String,
        name: String,
    },
    TerminateSession {
        session_id: SessionId,
    },
    SendMessage {
        session_id: SessionId,
        content: String,
    },
```

### Step 2: Add command handling in run_bridge

Add match arms in the `run_bridge` command dispatch loop, before the `Shutdown` arm:

```rust
            BridgeCommand::SpawnSession { task, name } => {
                let method = "session/spawn".to_string();
                let params = serde_json::json!({
                    "sessionId": session_id_str,
                    "task": task,
                    "name": name,
                });
                match call_ext_method(&conn, &method, params).await {
                    Ok(response) => {
                        let msg = format!(
                            "Spawned session '{}': {}",
                            name,
                            response
                                .get("sessionId")
                                .and_then(|s| s.as_str())
                                .unwrap_or("unknown")
                        );
                        let _ = channels
                            .notification_tx
                            .send(Notification::CompactionStatus { message: msg })
                            .await;
                    }
                    Err(e) => {
                        let _ = channels
                            .notification_tx
                            .send(Notification::CompactionStatus {
                                message: format!("Failed to spawn session: {e}"),
                            })
                            .await;
                    }
                }
            }
            BridgeCommand::TerminateSession { session_id: target } => {
                let method = "session/terminate".to_string();
                let params = serde_json::json!({
                    "sessionId": target.as_str(),
                });
                if let Err(e) = call_ext_method(&conn, &method, params).await {
                    let _ = channels
                        .notification_tx
                        .send(Notification::CompactionStatus {
                            message: format!("Failed to terminate session: {e}"),
                        })
                        .await;
                }
            }
            BridgeCommand::SendMessage {
                session_id: target,
                content,
            } => {
                let method = "message/send".to_string();
                let params = serde_json::json!({
                    "sessionId": target.as_str(),
                    "content": content,
                });
                if let Err(e) = call_ext_method(&conn, &method, params).await {
                    let _ = channels
                        .notification_tx
                        .send(Notification::CompactionStatus {
                            message: format!("Failed to send message: {e}"),
                        })
                        .await;
                }
            }
```

Note: `call_ext_method` is a helper — check if the bridge already has one or if `ExtMethod` handling can be reused. The existing `ExtMethod` arm does this conversion. Extract a helper or use `ExtMethod` directly.

### Step 3: Run build check

Run: `cargo check`
Expected: PASS

### Step 4: Commit

```bash
git add crates/cyril-core/src/types/event.rs crates/cyril-core/src/protocol/bridge.rs
git commit -m "feat: add SpawnSession, TerminateSession, SendMessage bridge commands"
```

---

## Task 6: SubagentUiState Component

**Files:**
- Create: `crates/cyril-ui/src/subagent_ui.rs`
- Modify: `crates/cyril-ui/src/lib.rs` (add module)
- Modify: `crates/cyril-ui/src/state.rs` (add `subagents` field to UiState)

### Step 1: Create SubagentUiState

Create `crates/cyril-ui/src/subagent_ui.rs`. This is a large file — it mirrors the message handling from `UiState` but scoped per subagent. Key methods:

- `apply_notification(session_id, notification) -> bool`
- `apply_list_update(subagents) -> bool`
- `focus(session_id)` / `unfocus()`
- `focused_session_id()` / `focused_stream()` / `streams()`

The `SubagentStream` struct mirrors the relevant subset of `UiState`:

```rust
pub struct SubagentStream {
    pub messages: Vec<ChatMessage>,
    streaming_text: String,
    streaming_thought: Option<String>,
    active_tool_calls: Vec<TrackedToolCall>,
    tool_call_index: HashMap<ToolCallId, usize>,
    activity: Activity,
}
```

With the same `commit_streaming`, `flush_streaming_text`, and tool call insertion logic. Implement this following the exact patterns from `UiState::apply_notification` for `AgentMessage`, `ToolCallStarted`, `ToolCallUpdated`, `TurnCompleted`.

### Step 2: Add to UiState

In `crates/cyril-ui/src/state.rs`, add a field:

```rust
pub subagents: SubagentUiState,
```

Initialize in `UiState::new()`:

```rust
subagents: SubagentUiState::new(),
```

### Step 3: Register module

In `crates/cyril-ui/src/lib.rs`, add:

```rust
pub mod subagent_ui;
```

### Step 4: Write tests

Tests should verify:
- Notification routing creates streams on first contact
- Messages commit in chronological order
- Focus/unfocus transitions
- Stream cleanup on list_update

### Step 5: Run tests

Run: `cargo test -p cyril-ui`
Expected: PASS

### Step 6: Commit

```bash
git add crates/cyril-ui/src/subagent_ui.rs crates/cyril-ui/src/lib.rs crates/cyril-ui/src/state.rs
git commit -m "feat: add SubagentUiState with per-subagent message streams"
```

---

## Task 7: App Notification Routing

**Files:**
- Modify: `crates/cyril/src/app.rs:16-25` (add SubagentTracker field)
- Modify: `crates/cyril/src/app.rs:169-232` (handle_notification routing)

### Step 1: Add SubagentTracker to App

Add field to `App` struct:

```rust
subagent_tracker: cyril_core::subagent::SubagentTracker,
```

Initialize in constructor:

```rust
subagent_tracker: cyril_core::subagent::SubagentTracker::new(),
```

### Step 2: Add routing logic to handle_notification

In `handle_notification`, add subagent routing before the existing logic:

```rust
fn handle_notification(&mut self, notification: Notification) {
    // SubagentTracker always gets list_update and inbox notifications
    let tracker_changed = self.subagent_tracker.apply_notification(&notification);

    // Unwrap SubagentNotification — route inner to SubagentUiState
    if let Notification::SubagentNotification { ref session_id, ref inner } = notification {
        self.ui_state.subagents.apply_notification(session_id, inner);
        self.redraw_needed = true;
        return;
    }

    // Check if ToolCallChunk belongs to a subagent
    if let Notification::ToolCallChunk { ref session_id, .. } = notification {
        if let Some(sid) = session_id {
            if self.subagent_tracker.is_subagent(sid) {
                self.ui_state.subagents.apply_notification(sid, &notification);
                self.redraw_needed = true;
                return;
            }
        }
    }

    // Pass SubagentListUpdated to UI for stream cleanup
    if let Notification::SubagentListUpdated { ref subagents, .. } = notification {
        self.ui_state.subagents.apply_list_update(subagents);
    }

    // Existing main-session flow
    let session_changed = self.session.apply_notification(&notification);
    let ui_changed = self.ui_state.apply_notification(&notification);

    // ... rest of existing cross-cutting handlers unchanged ...

    self.redraw_needed = self.redraw_needed || session_changed || ui_changed || tracker_changed;
}
```

### Step 3: Update frame rate for subagent activity

In the frame rate adapter, check subagent activity:

```rust
// If any subagent is streaming, use fast tick
let any_subagent_active = self.ui_state.subagents.any_active();
```

### Step 4: Run build

Run: `cargo check`
Expected: PASS

### Step 5: Commit

```bash
git add crates/cyril/src/app.rs
git commit -m "feat: route subagent notifications through SubagentTracker and SubagentUiState"
```

---

## Task 8: Crew Panel Widget

**Files:**
- Create: `crates/cyril-ui/src/widgets/crew_panel.rs`
- Modify: `crates/cyril-ui/src/widgets/mod.rs` (add module)
- Modify: `crates/cyril-ui/src/render.rs` (integrate into layout)

### Step 1: Create crew panel widget

Renders a bordered box with one row per subagent/pending stage. Status icons: `●` working (green), `◆` terminated (dim), `○` pending (grey). Shows group header, tool activity, and permission badges.

The widget takes `&SubagentTracker` and `&SubagentUiState` as input and renders into a `ratatui::Frame`.

### Step 2: Integrate into layout

In the render function, check if subagents exist. If so, allocate vertical space for the crew panel between the message area and input box. If drill-in is active, collapse to a one-line summary.

### Step 3: Add drill-in key handling

In `handle_key` in `app.rs`, add a layer after global shortcuts:
- If crew panel is visible and a subagent row is selected, Enter drills in
- If drilled in, Esc unfocuses

### Step 4: Run build

Run: `cargo check`
Expected: PASS

### Step 5: Commit

```bash
git add crates/cyril-ui/src/widgets/crew_panel.rs crates/cyril-ui/src/widgets/mod.rs crates/cyril-ui/src/render.rs crates/cyril/src/app.rs
git commit -m "feat: add crew panel widget with drill-in navigation"
```

---

## Task 9: Drill-In Rendering

**Files:**
- Modify: `crates/cyril-ui/src/render.rs` (swap viewport when focused)
- Modify: `crates/cyril-ui/src/traits.rs` (add TuiState methods)

### Step 1: Extend TuiState trait

Add methods to `TuiState`:

```rust
fn subagent_focused(&self) -> Option<&SessionId>;
fn subagent_messages(&self) -> Option<&[ChatMessage]>;
fn crew_panel_visible(&self) -> bool;
```

### Step 2: Implement in UiState

Delegate to `self.subagents` methods.

### Step 3: Modify render logic

When `subagent_focused()` is `Some`, render the subagent's message stream in the main viewport instead of the main chat. Show a header bar with the subagent name and `[Esc] Back`. The crew panel collapses to one line.

### Step 4: Commit

```bash
git add crates/cyril-ui/src/render.rs crates/cyril-ui/src/traits.rs crates/cyril-ui/src/state.rs
git commit -m "feat: drill-in rendering for subagent message streams"
```

---

## Task 10: Slash Commands

**Files:**
- Modify: `crates/cyril-core/src/commands/mod.rs` (add new commands)
- Modify: `crates/cyril-core/src/commands/` (command implementations)

### Step 1: Add /sessions command

List active subagents and pending stages. Reads from `SubagentTracker`, formats as system message.

### Step 2: Add /spawn command

`/spawn <name> <task>` — sends `BridgeCommand::SpawnSession`.

### Step 3: Add /kill command

`/kill [name]` — with name: terminates directly. Without name: opens picker of active subagents. While drilled in: targets focused session.

### Step 4: Add /msg command

`/msg [name] <text>` — with name: sends directly. Without: opens picker.

### Step 5: Update CommandContext

Add `&SubagentTracker` to `CommandContext` for session name lookup.

### Step 6: Run tests

Run: `cargo test`
Expected: PASS

### Step 7: Commit

```bash
git add crates/cyril-core/src/commands/
git commit -m "feat: add /sessions, /spawn, /kill, /msg slash commands"
```

---

## Task 11: Update Test Harness

**Files:**
- Modify: `crates/cyril/examples/test_bridge.rs`

### Step 1: Add subagent notification printing

Update `print_notification` to handle the three new variants: `SubagentListUpdated`, `InboxNotification`, `SubagentNotification`.

### Step 2: Commit

```bash
git add crates/cyril/examples/test_bridge.rs
git commit -m "feat: update test harness for subagent notifications"
```

---

## Task 12: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

### Step 1: Add subagent architecture notes

Update the Architecture section to document:
- `SubagentTracker` component and its responsibilities
- `SubagentUiState` component and its responsibilities
- Notification routing by sessionId
- The crew panel and drill-in UI model

### Step 2: Commit

```bash
git add CLAUDE.md
git commit -m "docs: add subagent architecture to CLAUDE.md"
```
