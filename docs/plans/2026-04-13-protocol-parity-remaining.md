# Protocol Parity — Remaining Tasks (#6–#17)

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement all remaining ACP protocol parity gaps identified by comparing Kiro tui.js 1.29.8 against Cyril's current type and notification coverage. Tasks are grouped by feature area and ordered by dependency.

**Architecture:** All changes follow Cyril's three-crate boundary: `cyril-core` (types + protocol), `cyril-ui` (state + rendering), `cyril` (wiring). New notifications flow through `convert.rs` → `Notification` enum → `apply_notification()` → `TuiState` → renderer. New commands flow through `CommandRegistry` → `BridgeCommand` → `bridge.rs` → ACP wire.

**Context from prior work:** Tasks #1–5 established the pattern. Core types with private fields and accessors in `cyril-core/src/types/`. Conversion from ACP types in `convert.rs`. Parallel state updates in `SessionController` and `UiState`. Display methods on `TrackedToolCall` in `cyril-ui/src/traits.rs`. Widget rendering in `cyril-ui/src/widgets/`.

---

## Group A: Session Resume Flow (#6, #7, #8)

These three tasks form a cohesive feature: the ability to list previous sessions, select one, and resume it with full conversation history displayed. They have a strict dependency chain: #7 must be done before #8 is useful (without user message rendering, resumed sessions show only agent responses).

### Current state

- `/new` command creates a new session via `BridgeCommand::NewSession`
- `/load <session-id>` loads a session by known ID via `BridgeCommand::LoadSession`
- No way to discover session IDs — user must already know them
- When loading a session, Kiro replays conversation via `session/update` notifications including `user_message_chunk` — but Cyril has no handler for this variant, so user messages are silently dropped
- The welcome message from `session/new` response `modes._meta.welcomeMessage` is not extracted

### Wire format reference (from tui.js 1.29.8 extraction)

**`session/new` response:**
```json
{
  "sessionId": "sess_abc",
  "modes": {
    "currentModeId": "kiro_default",
    "availableModes": [
      { "id": "kiro_default", "name": "Default", "description": "..." }
    ],
    "_meta": {
      "welcomeMessage": "Hello! I'm Kiro, your AI assistant."
    }
  },
  "models": null
}
```

**`session/update` with `user_message_chunk`:**
```json
{
  "sessionId": "sess_abc",
  "update": {
    "sessionUpdate": "user_message_chunk",
    "content": { "type": "text", "text": "Fix the auth bug" }
  }
}
```
This is a standard `acp::SessionUpdate::UserMessageChunk(ContentChunk)` variant.

**`kiro.dev/session/list` (extMethod request):**
```json
// Request:
{ "cwd": "/home/user/project" }

// Response:
{
  "sessions": [
    {
      "sessionId": "sess_abc",
      "title": "Fix the auth bug",
      "updatedAt": "2026-04-12T10:30:00Z",
      "messageCount": 42
    }
  ]
}
```
Note: `messageCount` is a Kiro extension field, not in the ACP `SessionInfo` type. The tui.js also accesses `summary` and `msgCount` fields for display formatting.

---

### Task 6: Fully parse session/new response

**Files:**
- Modify: `crates/cyril-core/src/types/event.rs` — add `welcome_message` and `available_modes` to `SessionCreated`
- Modify: `crates/cyril-core/src/protocol/bridge.rs` — extract from `NewSessionResponse`
- Modify: `crates/cyril-core/src/session.rs` — store modes from `SessionCreated`
- Modify: `crates/cyril-ui/src/state.rs` — display welcome message as first chat message

**Current code in bridge.rs (lines 215-250):**
```rust
BridgeCommand::NewSession { cwd: session_cwd } => {
    let translated_cwd = crate::platform::path::to_agent(&session_cwd);
    match conn.new_session(acp::NewSessionRequest::new(translated_cwd)).await {
        Ok(response) => {
            active_session_id = Some(response.session_id.clone());
            let session_id = response.session_id.to_string();
            let current_mode = response.modes.as_ref().map(|m| m.current_mode_id.to_string());
            // TODO: extract current_model from response.models
            let current_model: Option<String> = None;
            let notification = Notification::SessionCreated {
                session_id: crate::types::SessionId::new(session_id),
                current_mode,
                current_model,
            };
            ...
```

**Changes:**

1. Add fields to `Notification::SessionCreated`:
```rust
SessionCreated {
    session_id: SessionId,
    current_mode: Option<String>,
    current_model: Option<String>,
    welcome_message: Option<String>,     // NEW
    available_modes: Vec<SessionMode>,   // NEW
},
```

2. In `bridge.rs`, extract from the response:
```rust
let welcome_message = response.modes.as_ref()
    .and_then(|m| m.meta.as_ref())
    .and_then(|meta| meta.get("welcomeMessage"))
    .and_then(|v| v.as_str())
    .map(String::from);

let available_modes = response.modes.as_ref()
    .map(|m| {
        m.available_modes.iter().map(|mode| {
            SessionMode::new(
                mode.id.to_string(),
                mode.name.clone(),
                mode.description.clone(),
            )
        }).collect()
    })
    .unwrap_or_default();
```

Note: `response.modes.meta` is `Option<Meta>` where `Meta` is `HashMap<String, serde_json::Value>`. The `welcomeMessage` key is a Kiro extension inside the ACP `_meta` field.

3. In `SessionController::apply_notification`, the `SessionCreated` arm should call `self.set_modes(available_modes.clone())`.

4. In `UiState::apply_notification`, the `SessionCreated` arm should push a `ChatMessage::system(welcome_message)` if present.

5. Fix all construction sites of `SessionCreated` across tests (add `welcome_message: None, available_modes: Vec::new()`).

**Tests:**
- `session_created_stores_available_modes` — apply SessionCreated with 2 modes, assert `ctrl.modes().len() == 2`
- `session_created_welcome_message_added_to_chat` — apply SessionCreated with welcome, assert first message is System with that text
- `session_created_no_welcome_message` — apply with None, assert no system message added

**ACP crate notes:**
- `response.modes` is `Option<acp::SessionModeState>` which has `current_mode_id: SessionModeId`, `available_modes: Vec<SessionMode>`, `meta: Option<Meta>`
- `acp::SessionMode` has `id: SessionModeId`, `name: String`, `description: Option<String>`
- The `models` field is gated behind `unstable_session_model` feature flag — leave the TODO for now
- The `_meta.welcomeMessage` path is the same as what tui.js reads at its session setup

---

### Task 7: Handle user_message_chunk for session resume

**Files:**
- Modify: `crates/cyril-core/src/protocol/convert.rs` — handle `UserMessageChunk` in `session_update_to_notification`
- Modify: `crates/cyril-core/src/types/event.rs` — add `UserMessage` notification variant (if needed)
- Modify: `crates/cyril-ui/src/state.rs` — handle the notification in `apply_notification`

**Current code in convert.rs (line 854):**
The `session_update_to_notification` function matches on `acp::SessionUpdate` variants. Currently handles `AgentMessageChunk`, `AgentThoughtChunk`, `ToolCall`, `ToolCallUpdate`, `Plan`, `AvailableCommandsUpdate`, `CurrentModeUpdate`, `ConfigOptionUpdate`. The `UserMessageChunk` variant falls through to the `_ =>` catch-all which returns `None` (silently dropped).

**Changes:**

1. In `session_update_to_notification`, add a match arm for `UserMessageChunk`:
```rust
acp::SessionUpdate::UserMessageChunk(chunk) => {
    if let acp::ContentBlock::Text(ref text) = chunk.content {
        Some(Notification::AgentMessage(AgentMessage {
            text: text.text.clone(),
            is_streaming: false,  // replay, not live streaming
        }))
    } else {
        None
    }
}
```

Wait — `AgentMessage` is wrong, this is a USER message. We need a different notification.

2. Add a new variant to `Notification`:
```rust
/// User message replayed during session load/resume.
UserMessage {
    text: String,
},
```

3. In `session_update_to_notification`:
```rust
acp::SessionUpdate::UserMessageChunk(chunk) => {
    if let acp::ContentBlock::Text(ref text) = chunk.content {
        Some(Notification::UserMessage {
            text: text.text.clone(),
        })
    } else {
        None
    }
}
```

4. In `UiState::apply_notification`, handle the new variant:
```rust
Notification::UserMessage { text } => {
    self.messages.push(ChatMessage::user_text(text.clone()));
    self.messages_version += 1;
    true
}
```

Note: `ChatMessageKind::UserText(String)` already exists in the message model. Currently only used when the user types input locally. For session replay, the same variant works — the content is identical, just the source differs.

**Design decision:** Should replayed user messages be visually distinct? The tui.js renders them identically to live input. Matching that behavior is simpler and correct — the user sees their conversation history as it was. No need for a `is_replayed` flag.

**Tests:**
- `user_message_chunk_added_to_messages` — apply UserMessage notification, assert messages contain a UserText entry
- `user_message_chunk_in_session_replay_sequence` — apply a realistic replay: UserMessage, AgentMessage, ToolCallStarted, ToolCallUpdated, TurnCompleted — verify message order is preserved
- In convert.rs: `session_update_to_notification_user_message_chunk` — construct an `acp::SessionUpdate::UserMessageChunk` and verify it produces `Notification::UserMessage`

**ACP crate notes:**
- `acp::SessionUpdate::UserMessageChunk(ContentChunk)` where `ContentChunk` has `content: ContentBlock`
- `ContentBlock::Text(TextContent)` where `TextContent` has `text: String`
- Non-text content blocks (images) in user messages should be ignored for now (return `None`)

---

### Task 8: Implement session list and resume picker

**Files:**
- Modify: `crates/cyril-core/src/types/event.rs` — add `ListSessions` to `BridgeCommand`, add `SessionsListed` to `Notification`
- Create: `crates/cyril-core/src/types/session_entry.rs` — `SessionEntry` type
- Modify: `crates/cyril-core/src/types/mod.rs` — module + re-export
- Modify: `crates/cyril-core/src/protocol/bridge.rs` — handle `ListSessions` command
- Modify: `crates/cyril-core/src/commands/builtin.rs` — add `/resume` command
- Modify: `crates/cyril-core/src/commands/mod.rs` — register `/resume`
- Modify: `crates/cyril/src/app.rs` — handle `SessionsListed` notification → open picker

**New type:**
```rust
// crates/cyril-core/src/types/session_entry.rs
/// A saved session returned by the session list query.
pub struct SessionEntry {
    session_id: SessionId,
    title: Option<String>,
    updated_at: Option<String>,
}

impl SessionEntry {
    pub fn new(session_id: SessionId, title: Option<String>, updated_at: Option<String>) -> Self {
        Self { session_id, title, updated_at }
    }
    pub fn session_id(&self) -> &SessionId { &self.session_id }
    pub fn title(&self) -> Option<&str> { self.title.as_deref() }
    pub fn updated_at(&self) -> Option<&str> { self.updated_at.as_deref() }
}
```

**New BridgeCommand variant:**
```rust
ListSessions,
```

**New Notification variant:**
```rust
SessionsListed {
    sessions: Vec<SessionEntry>,
},
```

**Bridge handler:**
Kiro doesn't use standard ACP `session/list` — it uses the extension method `kiro.dev/session/list`. The bridge should call `conn.ext_method("kiro.dev/session/list", json!({"cwd": cwd}))` and parse the response.

```rust
BridgeCommand::ListSessions => {
    if let Some(ref sid) = active_session_id {
        let cwd = session_cwd.to_string_lossy().to_string();
        match conn.ext_method("kiro.dev/session/list", serde_json::json!({"cwd": cwd})).await {
            Ok(response) => {
                let sessions = parse_session_list(&response);
                let _ = channels.notification_tx
                    .send(Notification::SessionsListed { sessions }.into())
                    .await;
            }
            Err(e) => {
                tracing::warn!(error = %e, "session list failed");
                let _ = channels.notification_tx
                    .send(Notification::SessionsListed { sessions: Vec::new() }.into())
                    .await;
            }
        }
    }
}
```

Parse function:
```rust
fn parse_session_list(response: &serde_json::Value) -> Vec<SessionEntry> {
    response.get("sessions")
        .and_then(|s| s.as_array())
        .map(|arr| {
            arr.iter().filter_map(|v| {
                let session_id = v.get("sessionId")?.as_str()?;
                let title = v.get("title").and_then(|t| t.as_str()).map(String::from);
                let updated_at = v.get("updatedAt").and_then(|t| t.as_str()).map(String::from);
                // Filter out sessions without titles (matching tui.js behavior)
                if title.is_none() {
                    return None;
                }
                Some(SessionEntry::new(
                    SessionId::new(session_id),
                    title,
                    updated_at,
                ))
            }).collect()
        })
        .unwrap_or_default()
}
```

**`/resume` command:**
```rust
pub struct ResumeCommand;

#[async_trait::async_trait]
impl Command for ResumeCommand {
    fn name(&self) -> &str { "resume" }
    fn description(&self) -> &str { "Resume a previous session" }

    async fn execute(&self, ctx: &CommandContext<'_>, _args: &str) -> crate::Result<CommandResult> {
        ctx.bridge.send(BridgeCommand::ListSessions).await?;
        Ok(CommandResult::dispatched())
    }
}
```

**App handler for `SessionsListed`:**
In `app.rs`, when `SessionsListed` is received, convert sessions into `CommandOption` items and open a picker:
```rust
Notification::SessionsListed { sessions } => {
    let options: Vec<CommandOption> = sessions.iter().map(|s| {
        let label = s.title().unwrap_or("Untitled");
        let id_prefix = &s.session_id().as_str()[..8.min(s.session_id().as_str().len())];
        CommandOption {
            label: format!("{label} ({id_prefix})"),
            value: s.session_id().as_str().to_string(),
            description: s.updated_at().map(String::from),
            group: None,
            is_current: false,
        }
    }).collect();
    if options.is_empty() {
        // Show system message: no sessions found
    } else {
        ui_state.show_picker("Resume session", options);
        // On picker selection, dispatch BridgeCommand::LoadSession { session_id }
    }
}
```

The picker result handler needs to detect that this picker was for session resume and dispatch `LoadSession` with the selected value.

**Tests:**
- `parse_session_list_filters_no_title` — input with one titled and one untitled session, assert only titled one returned
- `parse_session_list_empty_response` — empty or missing sessions array returns empty vec
- `resume_command_dispatches_list_sessions` — verify the command sends the right BridgeCommand

---

## Group B: Protocol Enrichment (#9, #10, #11)

### Task 9: Enrich compaction status with phases and summary

**Files:**
- Modify: `crates/cyril-core/src/types/event.rs` — replace `CompactionStatus { message }` with richer variant
- Modify: `crates/cyril-core/src/types/session.rs` — add `CompactionPhase` enum (optional, could inline)
- Modify: `crates/cyril-core/src/protocol/convert.rs` — update parser (already partially parses phases)
- Modify: `crates/cyril-ui/src/state.rs` — handle phases: set SessionStatus::Compacting on Started, inject summary on Completed

**Current code in convert.rs (line 267):**
Already parses `status.type` as "started"|"completed"|"failed" and `summary` — but flattens everything into a single `message: String`. The enrichment would change the notification to carry structured data.

**Wire format:**
```json
// Started:
{"status": {"type": "started"}}

// Completed with summary:
{"status": {"type": "completed"}, "summary": "3 turns removed, context reduced by 40%"}

// Failed:
{"status": {"type": "failed", "error": "out of memory"}}
```

**New notification shape:**
```rust
CompactionStatus {
    phase: CompactionPhase,
    summary: Option<String>,
},
```

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompactionPhase {
    Started,
    Completed,
    Failed { error: Option<String> },
}
```

**UiState handling:**
- `Started` → set activity to a compacting state, show system message "Compacting..."
- `Completed` → reset activity, inject summary as system message if present
- `Failed` → reset activity, show error as system message

**Tests:** Existing convert.rs tests cover the parsing. Add UiState tests for phase-driven behavior.

---

### Task 10: Capture command effect field for smarter response parsing

**Files:**
- Modify: `crates/cyril-core/src/types/command.rs` — add `effect: Option<String>` to `CommandInfo`
- Modify: `crates/cyril-core/src/protocol/convert.rs` — extract `meta.effect` in commands/available handler
- Modify: `crates/cyril/src/app.rs` — use effect for `CommandExecuted` dispatch instead of name matching

**Current code in convert.rs (line 373):**
Extracts `meta.inputType` and `meta.local` but NOT `meta.effect`. The effect field tells the App how to interpret the `commands/execute` response:

```
updateModel, updateAgent, showContextPanel, showHelpPanel, showUsagePanel,
showMcpPanel, showToolsPanel, showHooksPanel, showKnowledgePanel, showCodePanel,
showFeedbackUrl, pasteImage, executePrompt, replyEditor, newSession, loadSession
```

**Changes:**
1. Add `effect: Option<String>` to `CommandInfo` struct
2. Extract: `let effect = meta.and_then(|m| m.get("effect")).and_then(|e| e.as_str()).map(String::from);`
3. Pass to `CommandInfo::new()` (add parameter)
4. In App's `CommandExecuted` handler, look up the command's effect from `SessionController::agent_commands()` and dispatch based on effect rather than command name

**CommandInfo::new signature change** will require updating all call sites (same ripple pattern as PlanEntry).

---

### Task 11: Capture trust options from permission requests

**Files:**
- Modify: `crates/cyril-core/src/types/event.rs` — add `trust_options` to `PermissionRequest`
- Modify: `crates/cyril-core/src/protocol/bridge.rs` — extract from `_meta.trustOptions` on permission request
- Modify: `crates/cyril-ui/src/traits.rs` — add trust options to `ApprovalState`
- Modify: `crates/cyril-ui/src/widgets/` — render trust page when options present

**Wire format (from tui.js):**
Permission requests can include `_meta.trustOptions`:
```json
{
  "sessionId": "sess_1",
  "options": [...],
  "toolCall": {...},
  "_meta": {
    "trustOptions": [
      { "label": "allow_always", "display": "Always allow this tool" },
      { "label": "trust_session", "display": "Trust all tools for this session" }
    ]
  }
}
```

When responding with a trust option, the response includes:
```json
{
  "outcome": { "outcome": "selected", "optionId": "allow_always" },
  "_meta": { "trustOption": "trust_session" }
}
```

**New type:**
```rust
pub struct TrustOption {
    pub label: String,
    pub display: String,
}
```

This is lower priority — the basic allow/reject flow already works.

---

## Group C: Content Type Expansion (#12, #13)

### Task 12: Handle terminal ToolCallContent type

**Files:**
- Modify: `crates/cyril-core/src/types/tool_call.rs` — add `Terminal { terminal_id: String }` to `ToolCallContent`
- Modify: `crates/cyril-core/src/protocol/convert.rs` — map from `acp::ToolCallContent::Terminal`
- Modify: `crates/cyril-ui/src/widgets/chat.rs` — render as placeholder

**Wire format:**
```json
{ "type": "terminal", "terminalId": "term_abc" }
```

**Current code in convert.rs `convert_tool_call_content` (line 54):**
```rust
fn convert_tool_call_content(acp_content: &[acp::ToolCallContent]) -> Vec<ToolCallContent> {
    acp_content.iter().filter_map(|c| match c {
        acp::ToolCallContent::Diff(diff) => Some(ToolCallContent::Diff { ... }),
        acp::ToolCallContent::Content(content) => {
            if let acp::ContentBlock::Text(text) = &content.content {
                Some(ToolCallContent::Text(text.text.clone()))
            } else {
                None  // <-- silently drops non-text content
            }
        }
        _ => None,  // <-- silently drops terminal and future types
    }).collect()
}
```

**Changes:** Add `Terminal` variant, map it in the `_ => None` catch-all. Render as `"[terminal: term_abc]"` in the chat widget.

---

### Task 13: Add image/resource content block indicators

**Files:**
- Same as Task 12 — extends the content conversion
- Add `Image { mime_type: String }` and `ResourceLink { uri: String, name: String }` to `ToolCallContent`

**Wire format:**
```json
// Image:
{ "type": "image", "data": "base64...", "mimeType": "image/png" }

// Resource link:
{ "type": "resource_link", "uri": "file:///path", "name": "readme.md", "title": "...", "description": "..." }
```

**Changes:** Add variants, map in `convert_tool_call_content`, render as `"[image: image/png]"` or `"[resource: readme.md]"` in the chat widget.

---

## Group D: Subagent & Infrastructure (#14, #15, #16, #17)

### Task 14: Add SubagentInfo.parentSessionId field

**Files:**
- Modify: `crates/cyril-core/src/types/subagent.rs` — add `parent_session_id: Option<SessionId>`
- Modify: `crates/cyril-core/src/protocol/convert.rs` — extract from `parentSessionId` in subagent list update

**Wire format:**
```json
{
  "subagents": [{
    "sessionId": "sess_child",
    "parentSessionId": "sess_parent",
    ...
  }]
}
```

**Current code:** The `parse_subagent_list_update` function in convert.rs already parses many fields. Adding `parentSessionId` extraction is a single line.

---

### Task 15: Investigate kiro.dev/session/activity vs RoutedNotification routing

**This is a research task, not an implementation task.**

**Question:** Does `kiro.dev/session/activity` carry events that Cyril's `RoutedNotification` session-ID-based routing misses?

**Current state:**
- Cyril routes subagent events by comparing `session_id` on standard `session/update` notifications against the main session ID
- `kiro.dev/session/activity` is dropped (returns `Ok(None)` in convert.rs line 661)
- tui.js routes `session/activity` events to its `multiSessionHandlers`

**Investigation steps:**
1. Run Cyril with a subagent-spawning task
2. Enable debug logging for the dropped `kiro.dev/session/activity` notifications
3. Compare the events received via standard routing vs activity channel
4. If any events are unique to the activity channel, implement handling

---

### Task 16: Add kiro.dev/settings/list support

**Files:**
- Modify: `crates/cyril-core/src/types/event.rs` — add `QuerySettings` to `BridgeCommand`, `SettingsReceived` to `Notification`
- Modify: `crates/cyril-core/src/protocol/bridge.rs` — call `ext_method("kiro.dev/settings/list", {})`
- Modify: `crates/cyril/src/app.rs` — query on startup after session creation

**Wire format:**
```json
// Response:
{
  "chat.greeting.enabled": true,
  "chat.enableNotifications": false,
  "chat.notificationMethod": "bell",
  "chat.autoExpandToolOutput": true
}
```

**Relevant settings for Cyril:**
- `chat.autoExpandToolOutput` — could control whether tool output is auto-expanded in chat
- `chat.greeting.enabled` — could suppress the welcome message

---

### Task 17: Add CommandOption.hint field for input pre-fill

**Files:**
- Modify: `crates/cyril-core/src/types/command.rs` — add `hint: Option<String>` to `CommandOption`
- Modify: `crates/cyril-core/src/protocol/convert.rs` — extract from options response
- Modify: `crates/cyril-ui/src/state.rs` — when picker selection has a hint, pre-fill input

**Wire format:**
```json
{
  "options": [{
    "label": "/agent my-agent",
    "value": "my-agent",
    "hint": "Enter task description",
    "description": "Custom agent"
  }]
}
```

When `hint` is present, tui.js pre-fills the input with `"label "` and shows the hint as placeholder text. This enables commands like `/agent my-agent <task>` where selecting the agent pre-fills and prompts for the task.

---

## Dependency Graph

```
#6 (session/new parsing)
    ↓
#7 (user_message_chunk)  ←  independent of #6, but both needed for #8
    ↓
#8 (session list + resume picker)  ←  depends on #7

#9 (compaction status)     — independent
#10 (command effect)       — independent
#11 (trust options)        — independent

#12 (terminal content)     — independent
#13 (image/resource)       — independent, can combine with #12

#14 (parentSessionId)      — independent
#15 (activity investigation) — independent research
#16 (settings/list)        — independent
#17 (CommandOption.hint)   — independent
```

## Suggested execution order

1. **#7** (user_message_chunk) — small, unblocks #8
2. **#6** (session/new parsing) — enriches session start
3. **#8** (session list + resume) — completes the resume feature
4. **#9** (compaction status) — enriches existing notification
5. **#10** (command effect) — cleaner command dispatch
6. **#12 + #13** (content types) — batch together, both touch same conversion function
7. **#14** (parentSessionId) — tiny
8. **#11** (trust options) — lower priority, complex UI
9. **#17** (CommandOption.hint) — nice-to-have
10. **#16** (settings/list) — nice-to-have
11. **#15** (activity investigation) — research, do when convenient
