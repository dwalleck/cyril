# Type Foundation Batch — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add all new core types, TuiState trait methods, and TrackedToolCall display methods needed by tasks #1–#5 in the protocol parity backlog — in one pass, so individual tasks only need conversion-layer and rendering work.

**Architecture:** New types live in `cyril-core` (data, no display logic). `TurnSummary` groups per-turn data atomically — `SessionController` buffers `MetadataUpdated` fields until `TurnCompleted` assembles them, preventing the renderer from ever seeing mismatched turn data. `TrackedToolCall` in `cyril-ui` gains display methods for `raw_output` interpretation (same boundary as existing `command_text()`/`primary_path()`).

**Tech Stack:** Rust, cyril-core (domain types), cyril-ui (TuiState trait, TrackedToolCall)

---

### Task 1: Add StopReason enum and TurnSummary struct to session types

**Files:**
- Modify: `crates/cyril-core/src/types/session.rs`
- Modify: `crates/cyril-core/src/types/mod.rs` (re-exports)

**Step 1: Write the failing test**

Add to the `tests` module in `session.rs`:

```rust
#[test]
fn stop_reason_default_is_end_turn() {
    assert_eq!(StopReason::default(), StopReason::EndTurn);
}

#[test]
fn turn_summary_accessors() {
    let summary = TurnSummary::new(
        StopReason::MaxTokens,
        Some(TokenCounts::new(1000, 500, Some(200))),
        Some(TurnMetering::new(0.05, Some(3000))),
    );
    assert_eq!(summary.stop_reason(), StopReason::MaxTokens);
    assert!(summary.token_counts().is_some());
    assert!(summary.metering().is_some());
}

#[test]
fn turn_summary_minimal() {
    let summary = TurnSummary::new(StopReason::Cancelled, None, None);
    assert_eq!(summary.stop_reason(), StopReason::Cancelled);
    assert!(summary.token_counts().is_none());
    assert!(summary.metering().is_none());
}

#[test]
fn stop_reason_is_send_sync() {
    assert_send::<StopReason>();
    assert_sync::<StopReason>();
}

#[test]
fn turn_summary_is_send_sync() {
    assert_send::<TurnSummary>();
    assert_sync::<TurnSummary>();
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p cyril-core -- stop_reason`
Expected: FAIL — `StopReason` not defined

**Step 3: Write the types**

Add before the `#[cfg(test)]` block in `session.rs`:

```rust
/// Reason the agent stopped processing a prompt turn.
///
/// Maps 1:1 to `acp::StopReason`. We define our own enum so `cyril-ui`
/// (which must not import ACP types) can read it through `TuiState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StopReason {
    #[default]
    EndTurn,
    MaxTokens,
    MaxTurnRequests,
    Refusal,
    Cancelled,
}

/// Atomic summary of a completed turn.
///
/// Assembled by `SessionController` when `TurnCompleted` arrives: the
/// `stop_reason` comes from the `session/prompt` response; `token_counts`
/// and `metering` were buffered from the preceding `MetadataUpdated`
/// notification. Grouping them prevents the renderer from ever seeing
/// token counts from turn N paired with a stop reason from turn N-1.
#[derive(Debug, Clone)]
pub struct TurnSummary {
    stop_reason: StopReason,
    token_counts: Option<TokenCounts>,
    metering: Option<TurnMetering>,
}

impl TurnSummary {
    pub fn new(
        stop_reason: StopReason,
        token_counts: Option<TokenCounts>,
        metering: Option<TurnMetering>,
    ) -> Self {
        Self {
            stop_reason,
            token_counts,
            metering,
        }
    }

    pub fn stop_reason(&self) -> StopReason {
        self.stop_reason
    }

    pub fn token_counts(&self) -> Option<&TokenCounts> {
        self.token_counts.as_ref()
    }

    pub fn metering(&self) -> Option<&TurnMetering> {
        self.metering.as_ref()
    }
}
```

Update re-exports in `crates/cyril-core/src/types/mod.rs`:

```rust
pub use session::{
    ContextUsage, CreditUsage, SessionCost, SessionId, SessionMode, SessionStatus,
    StopReason, TokenCounts, TurnMetering, TurnSummary,
};
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p cyril-core -- stop_reason turn_summary`
Expected: PASS

**Step 5: Commit**

```
feat(core): add StopReason enum and TurnSummary struct
```

---

### Task 2: Add PlanEntryPriority to PlanEntry

**Files:**
- Modify: `crates/cyril-core/src/types/plan.rs`
- Modify: `crates/cyril-core/src/types/mod.rs` (re-exports)
- Fix: `crates/cyril-core/src/protocol/convert.rs` (PlanEntry::new call site)

**Step 1: Write the failing test**

Add to `plan.rs` tests:

```rust
#[test]
fn plan_entry_priority_default_is_medium() {
    assert_eq!(PlanEntryPriority::default(), PlanEntryPriority::Medium);
}

#[test]
fn plan_entry_with_priority() {
    let entry = PlanEntry::new("Critical fix", PlanEntryStatus::InProgress, PlanEntryPriority::High);
    assert_eq!(entry.priority(), PlanEntryPriority::High);
}

#[test]
fn plan_entry_priority_is_send_sync() {
    assert_send::<PlanEntryPriority>();
    assert_sync::<PlanEntryPriority>();
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p cyril-core -- plan_entry_priority`
Expected: FAIL

**Step 3: Write the type and update PlanEntry**

```rust
/// Priority level of a plan entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlanEntryPriority {
    High,
    #[default]
    Medium,
    Low,
}
```

Update `PlanEntry`:

```rust
pub struct PlanEntry {
    title: String,
    status: PlanEntryStatus,
    priority: PlanEntryPriority,
}

impl PlanEntry {
    pub fn new(
        title: impl Into<String>,
        status: PlanEntryStatus,
        priority: PlanEntryPriority,
    ) -> Self {
        Self {
            title: title.into(),
            status,
            priority,
        }
    }

    pub fn title(&self) -> &str { &self.title }
    pub fn status(&self) -> PlanEntryStatus { self.status }
    pub fn priority(&self) -> PlanEntryPriority { self.priority }
}
```

Fix all `PlanEntry::new` call sites — add `PlanEntryPriority::Medium` as the third argument:

- `crates/cyril-core/src/protocol/convert.rs:897`
- `crates/cyril-core/src/types/plan.rs` test fixtures

Update re-exports in `mod.rs`:

```rust
pub use plan::{Plan, PlanEntry, PlanEntryPriority, PlanEntryStatus};
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p cyril-core -- plan`
Expected: PASS

**Step 5: Commit**

```
feat(core): add PlanEntryPriority to PlanEntry
```

---

### Task 3: Add raw_output to ToolCall

**Files:**
- Modify: `crates/cyril-core/src/types/tool_call.rs`

**Step 1: Write the failing test**

Add to `tool_call.rs` tests:

```rust
#[test]
fn tool_call_raw_output_accessor() {
    let output = serde_json::json!({"stdout": "hello", "exit_status": 0});
    let tc = ToolCall::new(
        ToolCallId::new("tc_1"),
        "Running cargo test".into(),
        ToolKind::Execute,
        ToolCallStatus::Completed,
        None,
    )
    .with_raw_output(Some(output.clone()));
    assert_eq!(tc.raw_output(), Some(&output));
}

#[test]
fn merge_update_preserves_raw_output_when_update_has_none() {
    let output = serde_json::json!({"stdout": "ok"});
    let mut tc = ToolCall::new(
        ToolCallId::new("tc_1"),
        "shell".into(),
        ToolKind::Execute,
        ToolCallStatus::InProgress,
        None,
    )
    .with_raw_output(Some(output.clone()));

    let update = ToolCall::new(
        ToolCallId::new("tc_1"),
        "shell".into(),
        ToolKind::Execute,
        ToolCallStatus::Completed,
        None,
    );
    tc.merge_update(&update);
    assert_eq!(tc.raw_output(), Some(&output), "raw_output should be preserved");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p cyril-core -- raw_output`
Expected: FAIL

**Step 3: Write the field, builder, accessor, and merge logic**

Add `raw_output: Option<serde_json::Value>` field to `ToolCall` struct. Initialize to `None` in `new()`. Add:

```rust
#[must_use]
pub fn with_raw_output(mut self, raw_output: Option<serde_json::Value>) -> Self {
    self.raw_output = raw_output;
    self
}

pub fn raw_output(&self) -> Option<&serde_json::Value> {
    self.raw_output.as_ref()
}
```

In `merge_update`, add (same semantics as `raw_input`):

```rust
if update.raw_output.is_some() {
    self.raw_output = update.raw_output.clone();
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p cyril-core -- raw_output`
Expected: PASS

**Step 5: Commit**

```
feat(core): add raw_output field to ToolCall
```

---

### Task 4: Update Notification enum — TurnCompleted and AgentSwitched

**Files:**
- Modify: `crates/cyril-core/src/types/event.rs`
- Fix: `crates/cyril-core/src/session.rs` (match arms)
- Fix: `crates/cyril-core/src/protocol/bridge.rs` (construction sites)
- Fix: `crates/cyril-core/src/subagent.rs` (match arm)
- Fix: `crates/cyril-ui/src/state.rs` (match arms)
- Fix: `crates/cyril-ui/src/subagent_ui.rs` (match arm)
- Fix: `crates/cyril/tests/event_routing.rs` (construction sites)
- Fix: `crates/cyril/examples/test_bridge.rs` (match arms)

**Step 1: Modify Notification variants**

In `event.rs`, change:

```rust
// Before:
TurnCompleted,
AgentSwitched {
    name: String,
    welcome: Option<String>,
},

// After:
TurnCompleted {
    stop_reason: StopReason,
},
AgentSwitched {
    name: String,
    welcome: Option<String>,
    previous_agent: Option<String>,
    model: Option<String>,
},
```

**Step 2: Fix all compilation errors**

Every `Notification::TurnCompleted` construction becomes `Notification::TurnCompleted { stop_reason: StopReason::EndTurn }` (default — the bridge will extract the real value later when Task #2 wires the conversion).

Every `Notification::AgentSwitched { name, welcome }` match arm adds `..` to ignore new fields. Construction sites in `convert.rs` add `previous_agent: None, model: None` (wired later in Task #4).

Every `Notification::AgentSwitched { name, .. }` match arm (like in `session.rs`) already uses `..` and is unaffected.

**Step 3: Update event.rs tests**

Fix test fixtures to use the new variant shapes.

**Step 4: Run full workspace check**

Run: `cargo check`
Expected: PASS (no compilation errors)

**Step 5: Run full test suite**

Run: `cargo test -p cyril-core && cargo test -p cyril-ui`
Expected: PASS

**Step 6: Commit**

```
feat(core): add stop_reason to TurnCompleted, extend AgentSwitched
```

---

### Task 5: Update SessionController — TurnSummary assembly with buffered metadata

**Files:**
- Modify: `crates/cyril-core/src/session.rs`

**Architecture note:** Both `SessionController` and `UiState` receive the same notification
stream independently (neither references the other). Both need `TurnSummary` — the controller
for session state, UiState for the renderer via `TuiState`. Each does its own buffer+assembly.
This avoids cross-component pushing and preserves the "notifications flow one way" invariant.

**Step 1: Write the failing tests**

```rust
#[test]
fn turn_summary_assembled_from_metadata_and_turn_completed() {
    let mut ctrl = SessionController::new();
    ctrl.set_status(SessionStatus::Busy);

    // Metadata arrives first (buffered)
    ctrl.apply_notification(&Notification::MetadataUpdated {
        context_usage: ContextUsage::new(50.0),
        metering: Some(TurnMetering::new(0.03, Some(2000))),
        tokens: Some(TokenCounts::new(800, 400, Some(100))),
    });
    assert!(ctrl.last_turn().is_none(), "no TurnSummary until turn completes");

    // Turn completes — assembles TurnSummary
    ctrl.apply_notification(&Notification::TurnCompleted {
        stop_reason: StopReason::EndTurn,
    });
    let summary = ctrl.last_turn().expect("TurnSummary should exist after TurnCompleted");
    assert_eq!(summary.stop_reason(), StopReason::EndTurn);
    assert!(summary.token_counts().is_some());
    assert_eq!(summary.token_counts().unwrap().input(), 800);
    assert!(summary.metering().is_some());
}

#[test]
fn turn_summary_cleared_on_new_session() {
    let mut ctrl = SessionController::new();
    ctrl.apply_notification(&Notification::MetadataUpdated {
        context_usage: ContextUsage::new(10.0),
        metering: Some(TurnMetering::new(0.01, None)),
        tokens: None,
    });
    ctrl.apply_notification(&Notification::TurnCompleted {
        stop_reason: StopReason::EndTurn,
    });
    assert!(ctrl.last_turn().is_some());

    ctrl.apply_notification(&Notification::SessionCreated {
        session_id: SessionId::new("s2"),
        current_mode: None,
        current_model: None,
    });
    assert!(ctrl.last_turn().is_none(), "TurnSummary cleared on new session");
}

#[test]
fn turn_summary_cleared_on_bridge_disconnect() {
    let mut ctrl = SessionController::new();
    ctrl.apply_notification(&Notification::TurnCompleted {
        stop_reason: StopReason::EndTurn,
    });
    assert!(ctrl.last_turn().is_some());

    ctrl.apply_notification(&Notification::BridgeDisconnected {
        reason: "process exited".into(),
    });
    assert!(ctrl.last_turn().is_none(), "TurnSummary cleared on disconnect");
}

#[test]
fn turn_summary_without_metadata() {
    let mut ctrl = SessionController::new();
    ctrl.apply_notification(&Notification::TurnCompleted {
        stop_reason: StopReason::Cancelled,
    });
    let summary = ctrl.last_turn().expect("TurnSummary even without prior metadata");
    assert_eq!(summary.stop_reason(), StopReason::Cancelled);
    assert!(summary.token_counts().is_none());
    assert!(summary.metering().is_none());
}

#[test]
fn second_turn_overwrites_last_turn() {
    let mut ctrl = SessionController::new();

    // Turn 1
    ctrl.apply_notification(&Notification::MetadataUpdated {
        context_usage: ContextUsage::new(10.0),
        metering: Some(TurnMetering::new(0.01, None)),
        tokens: Some(TokenCounts::new(100, 50, None)),
    });
    ctrl.apply_notification(&Notification::TurnCompleted {
        stop_reason: StopReason::EndTurn,
    });
    assert_eq!(ctrl.last_turn().unwrap().token_counts().unwrap().input(), 100);

    // Turn 2 — overwrites
    ctrl.apply_notification(&Notification::MetadataUpdated {
        context_usage: ContextUsage::new(20.0),
        metering: Some(TurnMetering::new(0.05, Some(5000))),
        tokens: Some(TokenCounts::new(800, 400, Some(200))),
    });
    ctrl.apply_notification(&Notification::TurnCompleted {
        stop_reason: StopReason::MaxTokens,
    });
    let summary = ctrl.last_turn().unwrap();
    assert_eq!(summary.stop_reason(), StopReason::MaxTokens);
    assert_eq!(summary.token_counts().unwrap().input(), 800);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p cyril-core -- turn_summary`
Expected: FAIL — `last_turn()` not defined

**Step 3: Implement buffering and assembly**

Add fields to `SessionController`:

```rust
pending_tokens: Option<TokenCounts>,
pending_metering: Option<TurnMetering>,
last_turn: Option<TurnSummary>,
```

Initialize all to `None` in `new()`.

Add accessor:

```rust
pub fn last_turn(&self) -> Option<&TurnSummary> {
    self.last_turn.as_ref()
}
```

Update `apply_notification`:

- `MetadataUpdated` arm: buffer `tokens` into `pending_tokens`, buffer `metering` into `pending_metering` (in addition to existing `context_usage` and `session_cost` logic).
- `TurnCompleted { stop_reason }` arm: assemble `TurnSummary` from `stop_reason` + `pending_tokens.take()` + `pending_metering.take()`, store in `last_turn`. Then set `status = Active`.
- `SessionCreated` arm: clear `last_turn`, `pending_tokens`, `pending_metering`.
- `BridgeDisconnected` arm: clear `last_turn`, `pending_tokens`, `pending_metering`.

**Step 4: Run tests**

Run: `cargo test -p cyril-core -- turn_summary`
Expected: PASS

Run: `cargo test -p cyril-core`
Expected: PASS (all existing tests still pass)

**Step 5: Commit**

```
feat(core): assemble TurnSummary from buffered metadata + TurnCompleted
```

---

### Task 6: Update TuiState trait, UiState impl, and MockTuiState

**Files:**
- Modify: `crates/cyril-ui/src/traits.rs` (trait + mock)
- Modify: `crates/cyril-ui/src/state.rs` (UiState impl + fields + apply_notification)

**Architecture note:** `TuiState` is implemented by `UiState` (in `cyril-ui`), NOT by `App`.
`UiState` has no reference to `SessionController` — both receive notifications independently.
So `UiState` needs its own `last_turn`, `session_cost`, and buffer fields, parallel to
`SessionController`. This matches the existing pattern where `context_usage` and
`current_model` are independently tracked by both components.

**Step 1: Add new trait methods to TuiState**

In the `TuiState` trait, add under the `// Session info` section:

```rust
    fn last_turn(&self) -> Option<&cyril_core::types::TurnSummary>;
    fn session_cost(&self) -> &cyril_core::types::SessionCost;
```

**Step 2: Add fields to UiState**

In `crates/cyril-ui/src/state.rs`, add to the `UiState` struct:

```rust
    last_turn: Option<cyril_core::types::TurnSummary>,
    session_cost: cyril_core::types::SessionCost,
    pending_tokens: Option<cyril_core::types::TokenCounts>,
    pending_metering: Option<cyril_core::types::TurnMetering>,
```

Initialize in `new()`: all `None` / `SessionCost::new()`.

**Step 3: Update UiState::apply_notification**

Mirror the SessionController buffering logic:

- `MetadataUpdated` arm (already exists, handles `context_usage`): also buffer `tokens` → `pending_tokens`, `metering` → `pending_metering`, and call `session_cost.record_turn(m)` for metering.
- `TurnCompleted { stop_reason }` arm (already exists, flushes streaming text): also assemble `TurnSummary` from `stop_reason` + `pending_tokens.take()` + `pending_metering.take()`, store in `last_turn`.
- `SessionCreated` arm (already exists): also clear `last_turn`, `session_cost = SessionCost::new()`, clear pending buffers.
- `BridgeDisconnected` arm (if handled): also clear `last_turn` and pending buffers.

**Step 4: Implement TuiState methods on UiState**

```rust
fn last_turn(&self) -> Option<&cyril_core::types::TurnSummary> {
    self.last_turn.as_ref()
}
fn session_cost(&self) -> &cyril_core::types::SessionCost {
    &self.session_cost
}
```

**Step 5: Update MockTuiState**

Add fields:

```rust
pub last_turn: Option<cyril_core::types::TurnSummary>,
pub session_cost: cyril_core::types::SessionCost,
```

Default both to `None` / `SessionCost::new()`.

Implement the trait methods:

```rust
fn last_turn(&self) -> Option<&cyril_core::types::TurnSummary> {
    self.last_turn.as_ref()
}
fn session_cost(&self) -> &cyril_core::types::SessionCost {
    &self.session_cost
}
```

**Step 6: Run workspace check and tests**

Run: `cargo check && cargo test -p cyril-ui && cargo test -p cyril-core`
Expected: PASS

**Step 7: Commit**

```
feat(ui): add last_turn and session_cost to TuiState, wire UiState buffer assembly
```

---

### Task 7: Add raw_output display methods to TrackedToolCall

**Files:**
- Modify: `crates/cyril-ui/src/traits.rs`

**Step 1: Write the failing tests**

```rust
#[test]
fn tracked_tool_call_raw_output_accessor() {
    use cyril_core::types::*;
    let output = serde_json::json!({"stdout": "hello\nworld", "exit_status": 0});
    let tc = ToolCall::new(
        ToolCallId::new("tc_1"),
        "Running cargo test".into(),
        ToolKind::Execute,
        ToolCallStatus::Completed,
        None,
    )
    .with_raw_output(Some(output.clone()));
    let tracked = TrackedToolCall::new(tc);
    assert_eq!(tracked.raw_output(), Some(&output));
}

#[test]
fn tracked_tool_call_output_text_shell() {
    use cyril_core::types::*;
    let output = serde_json::json!({"stdout": "hello world", "exit_status": 0});
    let tc = ToolCall::new(
        ToolCallId::new("tc_1"),
        "shell".into(),
        ToolKind::Execute,
        ToolCallStatus::Completed,
        None,
    )
    .with_raw_output(Some(output));
    let tracked = TrackedToolCall::new(tc);
    assert_eq!(tracked.output_text(), Some("hello world".to_string()));
}

#[test]
fn tracked_tool_call_output_text_items_text() {
    use cyril_core::types::*;
    let output = serde_json::json!({"items": [{"Text": "file contents here"}]});
    let tc = ToolCall::new(
        ToolCallId::new("tc_1"),
        "read".into(),
        ToolKind::Read,
        ToolCallStatus::Completed,
        None,
    )
    .with_raw_output(Some(output));
    let tracked = TrackedToolCall::new(tc);
    assert_eq!(tracked.output_text(), Some("file contents here".to_string()));
}

#[test]
fn tracked_tool_call_exit_code() {
    use cyril_core::types::*;
    let output = serde_json::json!({"stdout": "", "exit_status": 1});
    let tc = ToolCall::new(
        ToolCallId::new("tc_1"),
        "shell".into(),
        ToolKind::Execute,
        ToolCallStatus::Completed,
        None,
    )
    .with_raw_output(Some(output));
    let tracked = TrackedToolCall::new(tc);
    assert_eq!(tracked.exit_code(), Some(1));
}

#[test]
fn tracked_tool_call_exit_code_none_for_non_execute() {
    use cyril_core::types::*;
    let tc = ToolCall::new(
        ToolCallId::new("tc_1"),
        "read".into(),
        ToolKind::Read,
        ToolCallStatus::Completed,
        None,
    );
    let tracked = TrackedToolCall::new(tc);
    assert_eq!(tracked.exit_code(), None);
}

#[test]
fn tracked_tool_call_error_message_on_failed() {
    use cyril_core::types::*;
    let output = serde_json::json!("Command timed out");
    let tc = ToolCall::new(
        ToolCallId::new("tc_1"),
        "shell".into(),
        ToolKind::Execute,
        ToolCallStatus::Failed,
        None,
    )
    .with_raw_output(Some(output));
    let tracked = TrackedToolCall::new(tc);
    assert_eq!(tracked.error_message(), Some("Command timed out".to_string()));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p cyril-ui -- tracked_tool_call_raw_output`
Expected: FAIL

**Step 3: Implement display methods**

```rust
pub fn raw_output(&self) -> Option<&serde_json::Value> {
    self.inner.raw_output()
}

/// Extract displayable output text from raw_output.
///
/// Tries multiple unwrapping strategies matching tui.js `unwrapResultOutput`:
/// 1. Shell commands: `raw_output.stdout` or `raw_output.stderr`
/// 2. Kiro item envelope: `raw_output.items[0].Text`
/// 3. Direct text fields: `raw_output.text`, `.content`, `.result`
/// 4. String value: raw_output as plain string
pub fn output_text(&self) -> Option<String> {
    let output = self.inner.raw_output()?;

    // Plain string output
    if let Some(s) = output.as_str() {
        return Some(s.to_string());
    }

    let obj = output.as_object()?;

    // Shell: stdout/stderr
    if let Some(stdout) = obj.get("stdout").and_then(|v| v.as_str()) {
        if !stdout.trim().is_empty() {
            return Some(stdout.to_string());
        }
    }
    if let Some(stderr) = obj.get("stderr").and_then(|v| v.as_str()) {
        if !stderr.trim().is_empty() {
            return Some(stderr.to_string());
        }
    }

    // Kiro item envelope: items[0].Text or items[0].Json
    if let Some(items) = obj.get("items").and_then(|v| v.as_array()) {
        if let Some(first) = items.first() {
            if let Some(text) = first.get("Text").and_then(|v| v.as_str()) {
                return Some(text.to_string());
            }
            if let Some(json_val) = first.get("Json") {
                return serde_json::to_string_pretty(json_val).ok();
            }
        }
    }

    // Generic text fields
    for key in ["text", "content", "result"] {
        if let Some(s) = obj.get(key).and_then(|v| v.as_str()) {
            return Some(s.to_string());
        }
    }

    None
}

/// Extract exit code from raw_output for Execute-kind tool calls.
pub fn exit_code(&self) -> Option<i64> {
    if self.inner.kind() != cyril_core::types::ToolKind::Execute {
        return None;
    }
    let output = self.inner.raw_output()?;
    let obj = output.as_object()?;
    obj.get("exit_status").and_then(|v| v.as_i64())
}

/// Extract error message when tool call failed.
pub fn error_message(&self) -> Option<String> {
    if self.inner.status() != cyril_core::types::ToolCallStatus::Failed {
        return None;
    }
    let output = self.inner.raw_output()?;
    if let Some(s) = output.as_str() {
        return Some(s.to_string());
    }
    if let Some(obj) = output.as_object() {
        for key in ["error", "message"] {
            if let Some(s) = obj.get(key).and_then(|v| v.as_str()) {
                return Some(s.to_string());
            }
        }
    }
    None
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p cyril-ui -- tracked_tool_call`
Expected: PASS

**Step 5: Commit**

```
feat(ui): add raw_output display methods to TrackedToolCall
```

---

### Task 8: Final workspace verification

**Step 1: Full compile check**

Run: `cargo check`
Expected: PASS

**Step 2: Full test suite**

Run: `cargo test`
Expected: PASS

**Step 3: Commit any remaining fixups**

---

## Summary of changes by file

| File | Changes |
|---|---|
| `cyril-core/src/types/session.rs` | `StopReason`, `TurnSummary` |
| `cyril-core/src/types/plan.rs` | `PlanEntryPriority`, `PlanEntry` gains `priority` field |
| `cyril-core/src/types/tool_call.rs` | `ToolCall` gains `raw_output` field + builder + accessor + merge |
| `cyril-core/src/types/event.rs` | `TurnCompleted { stop_reason }`, `AgentSwitched` + 2 fields |
| `cyril-core/src/types/mod.rs` | Re-export `StopReason`, `TurnSummary`, `PlanEntryPriority` |
| `cyril-core/src/session.rs` | Buffer fields, `last_turn()`, assembly logic, clear on disconnect |
| `cyril-core/src/protocol/convert.rs` | Fix `PlanEntry::new` call site |
| `cyril-core/src/protocol/bridge.rs` | Fix `TurnCompleted` construction sites |
| `cyril-core/src/subagent.rs` | Fix `TurnCompleted` match |
| `cyril-ui/src/traits.rs` | `TuiState` + 2 methods, `MockTuiState` + 2 fields, `TrackedToolCall` + 4 methods |
| `cyril-ui/src/state.rs` | Fix `TurnCompleted`/`AgentSwitched` matches, add buffer fields + assembly + `session_cost` |
| `cyril-ui/src/subagent_ui.rs` | Fix `TurnCompleted` match |
| `cyril/tests/event_routing.rs` | Fix `TurnCompleted` construction |
| `cyril/examples/test_bridge.rs` | Fix `TurnCompleted` and `AgentSwitched` matches |

## Design decisions

**Why `UiState` and `SessionController` both buffer independently:**
`TuiState` is implemented by `UiState` (in `cyril-ui`), not by `App`. `UiState` has no reference
to `SessionController` — both receive the same notification stream via the App's event loop.
Adding a cross-component push (App copies data from SessionController to UiState after each
notification) would break the "notifications flow one way" architecture. Instead, both components
do their own lightweight buffer+assembly from the same `MetadataUpdated` → `TurnCompleted` sequence.

**Why `BridgeDisconnected` clears pending buffers:**
Without this, a bridge crash mid-turn leaves stale `pending_tokens`/`pending_metering` that would
leak into the next session's first `TurnSummary`, creating a cross-session data integrity violation.

**Why `StopReason::EndTurn` is used as placeholder in bridge construction sites:**
The `Ok(_)` path in `bridge.rs` currently discards the `PromptResponse` (`Ok(_)`). Extracting the
real `stop_reason` from `PromptResponse.stop_reason` is deferred to Task #2. For the error path,
`EndTurn` is a known compromise — the bridge sends `TurnCompleted` on error to unstick the UI.
A future improvement would send `BridgeError` instead of `TurnCompleted` on transport failures.

**Why `output_text()` returns `None` instead of JSON fallback for unknown shapes:**
tui.js falls back to `JSON.stringify` for unrecognized rawOutput. In a TUI with limited vertical
space, dumping raw JSON for every unrecognized tool output would be noisy. Better to show nothing
and let the title/status icon carry the information. The `raw_output()` accessor is available for
future rendering if needed.
