# Subagent Support Design

**Date:** 2026-04-02
**Branch:** v2-rewrite
**Kiro Version:** 1.29.0

## Context

Kiro CLI v1.29.0 introduces subagent support via the `_kiro.dev/subagent/` extension namespace. The main agent can spawn multiple child sessions ("stages") that run in parallel, each with its own tool access and message stream. All communication is multiplexed over the existing stdio connection, with `sessionId` as the demuxing key.

This design adds subagent observation, display, and basic control to Cyril.

## Protocol Surface

### Triggering — The `subagent` Tool Call

The main agent uses a tool called `"subagent"` with input:

```json
{
  "mode": "blocking",
  "task": "Review code changes",
  "stages": [
    { "name": "code-reviewer", "role": "code-reviewer", "prompt_template": "..." },
    { "name": "pr-test-analyzer", "role": "pr-test-analyzer", "prompt_template": "..." }
  ]
}
```

The parent agent blocks until all stages complete.

### `_kiro.dev/subagent/list_update`

Snapshot notification sent whenever the subagent set changes:

```json
{
  "subagents": [
    {
      "sessionId": "b49d53d1-...",
      "sessionName": "code-reviewer",
      "agentName": "code-reviewer",
      "initialQuery": "Review the code changes...",
      "status": { "type": "working", "message": "Running" },
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
```

### Subagent Session Updates

Each subagent streams via the standard `session/update` notification with its own `sessionId`. The same update types apply: `agent_message_chunk`, `tool_call`, `tool_call_update`. The `_kiro.dev/session/update` extension carries `tool_call_chunk` events with the subagent's sessionId.

### `_kiro.dev/session/inbox_notification`

Sent when subagents complete and post results:

```json
{
  "sessionId": "874046d5-...",
  "sessionName": "main",
  "messageCount": 1,
  "escalationCount": 0,
  "senders": ["subagent"]
}
```

### Client-Initiated Methods

| Method | Params | Description |
|--------|--------|-------------|
| `session/spawn` | `{sessionId, task, name}` | Spawn a new session |
| `session/terminate` | (session ID) | Kill a session |
| `message/send` | `{sessionId, content}` | Send message to a session |

## Architecture: Approach B — New Components, Thin Tracker

`SessionController` keeps its current shape (main session only). Two new components handle subagent state:

- **`SubagentTracker`** in `cyril-core` — data/metadata from `list_update`
- **`SubagentUiState`** in `cyril-ui` — per-subagent message streams, crew panel, drill-in focus

The App routes notifications by `sessionId`: subagent-scoped events go to `SubagentUiState`, main session events follow the existing path.

## New Types (cyril-core)

### Notification Variants

```rust
SubagentListUpdated {
    subagents: Vec<SubagentInfo>,
    pending_stages: Vec<PendingStage>,
},

InboxNotification {
    session_id: SessionId,
    message_count: u32,
    escalation_count: u32,
    senders: Vec<String>,
},
```

Existing notifications that can come from subagents (`AgentMessage`, `AgentThought`, `ToolCallStarted`, `ToolCallUpdated`, `ToolCallChunk`, `TurnCompleted`) gain `session_id: Option<SessionId>`.

### Domain Types

```rust
pub struct SubagentInfo {
    pub session_id: SessionId,
    pub session_name: String,
    pub agent_name: String,
    pub initial_query: String,
    pub status: SubagentStatus,
    pub group: Option<String>,
    pub role: Option<String>,
    pub depends_on: Vec<String>,
}

pub enum SubagentStatus {
    Working { message: String },
    Terminated,
}

pub struct PendingStage {
    pub name: String,
    pub agent_name: Option<String>,
    pub group: Option<String>,
    pub role: Option<String>,
    pub depends_on: Vec<String>,
}
```

## Protocol Layer (cyril-core)

### Conversion (convert.rs)

- `to_subagent_list_update(params) -> Result<Notification>` — parses `_kiro.dev/subagent/list_update`
- `to_inbox_notification(params) -> Result<Notification>` — parses `_kiro.dev/session/inbox_notification`
- `to_ext_notification` match gains two new arms
- `session_update_to_notification` threads `sessionId` from the outer wrapper onto relevant notification variants

### Bridge (bridge.rs)

Three new `BridgeCommand` variants:

```rust
SpawnSession { task: String, name: String },
TerminateSession { session_id: SessionId },
SendMessage { session_id: SessionId, content: String },
```

Each wraps the corresponding extension method call.

## SubagentTracker (cyril-core/src/subagent.rs)

Pure state machine. Owns metadata, not messages.

```rust
pub struct SubagentTracker {
    subagents: HashMap<SessionId, SubagentInfo>,
    pending_stages: Vec<PendingStage>,
    inbox_message_count: u32,
    inbox_escalation_count: u32,
}
```

`apply_notification` handles `SubagentListUpdated` (full snapshot replace) and `InboxNotification` (counter update).

Accessors: `subagents()`, `pending_stages()`, `get()`, `is_subagent()`, `active_count()`, `inbox_message_count()`, `all_groups()`.

## SubagentUiState (cyril-ui/src/subagent_ui.rs)

Owns per-subagent message streams and drill-in focus.

```rust
pub struct SubagentUiState {
    streams: HashMap<SessionId, SubagentStream>,
    focused: Option<SessionId>,
    crew_scroll: usize,
}

pub struct SubagentStream {
    messages: Vec<ChatMessage>,
    streaming_text: String,
    streaming_thought: Option<String>,
    active_tool_calls: Vec<TrackedToolCall>,
    tool_call_index: HashMap<ToolCallId, usize>,
    activity: Activity,
}
```

`SubagentStream` mirrors the main stream's message handling pattern (streaming_text → commit_streaming → messages, chronological tool call insertion). Intentional duplication — subagent streams lack input, autocomplete, overlays, plan.

Key methods:
- `apply_notification(session_id, notification) -> bool` — routes to the right stream, creates on first contact
- `apply_list_update(subagents) -> bool` — cleans up terminated streams
- `focus(session_id)` / `unfocus()` — drill-in control
- `focused_stream()` / `streams()` — for the renderer

`TuiState` trait gains: `subagent_focused()`, `subagent_stream()`, `subagent_streams()`, `crew_scroll()`.

## App Routing (cyril binary)

```
notification arrives
  → SubagentTracker.apply_notification() (always, for list_update/inbox)
  → if session_id is a known subagent:
      SubagentUiState.apply_notification(session_id, notif)
  → else:
      SessionController.apply_notification(notif)  // existing
      UiState.apply_notification(notif)             // existing
      cross-cutting handlers                        // existing
```

Esc during a subagent crew cancels the main session (parent owns the blocking tool call; server cascades to children). Individual `/kill` for targeted termination.

Frame rate adapter checks subagent activity — if any subagent is streaming, use 50ms tick.

## Rendering

### Crew Panel

New widget at `cyril-ui/src/widgets/crew_panel.rs`. Rendered when subagents or pending stages exist. Horizontal slice above the input box.

```
┌─ crew: Review code changes ──────────────────┐
│ ● code-reviewer        Reading CLAUDE.md:1    │
│ ● pr-test-analyzer     Running grep           │
│ ○ summary-writer       Waiting (depends: 2)   │
│ ◆ code-simplifier      Terminated             │
└───────────────────────────────────────────────┘
```

Status icons: `●` working (green), `◆` terminated (dim), `○` pending (grey). Permission badge `⚠` when pending approval. Selected row highlighted for drill-in.

### Drill-In View

When focused on a subagent, the main viewport renders that subagent's message stream. Header bar shows subagent name and `[Esc] Back`. Crew panel collapses to a one-line summary (`3 working · 1 pending · 2 done`). Existing chat message widgets reused — no new content widgets.

### No Subagents

Zero visual overhead. No crew panel rendered.

## Slash Commands

- **`/sessions`** — Lists active subagents and pending stages from `SubagentTracker`. No bridge call.
- **`/spawn <name> <task>`** — Sends `BridgeCommand::SpawnSession`. Shows up via `list_update`.
- **`/kill [name]`** — With name: terminates that subagent. Without name: opens picker of active subagents. While drilled in: targets focused subagent.
- **`/msg [name] <text>`** — With name: sends message. Without name: opens picker then input. While drilled in: targets focused subagent.

`CommandContext` extended with `&SubagentTracker` for session name → ID lookup.

## Permission Handling

One approval overlay at a time (existing model). Subagent permissions are queued. The overlay message includes the subagent name for context. Crew panel shows `⚠` badge on rows with pending approvals.

## Error Handling

- **Bridge errors**: `SpawnSession`, `TerminateSession`, `SendMessage` send error notifications so the UI never gets stuck.
- **Disappears mid-drill-in**: Stream persists, crew row shows "Terminated". No forced ejection.
- **Unknown session ID**: Create `SubagentStream` optimistically. Next `list_update` fills in metadata. Handles race between `session/update` and `list_update`.
- **Rapid list_update churn**: Full snapshot replace in tracker. Stream cleanup is lazy and cheap.

## Testing

### State lifecycle (cyril-core)
- SubagentTracker: empty → N working → partial terminate → all gone
- Inbox counter progression
- `is_subagent()` accuracy through lifecycle

### Conversion (cyril-core)
- Parse real `list_update` JSON payloads from logs
- Parse `inbox_notification` payloads
- `session_id` threaded through on existing notification variants
- Missing `sessionId` defaults to `None` (backward compat)

### UI state (cyril-ui)
- SubagentStream message lifecycle (same pattern as main stream tests)
- Routing: subagent sessionId → SubagentUiState, main/missing → main stream
- Focus/unfocus transitions
- Stream cleanup on list_update

### Render (cyril-ui)
- Crew panel status icons and tool descriptions
- Drill-in viewport swap
- Collapsed crew panel during drill-in
- No crew panel when no subagents
