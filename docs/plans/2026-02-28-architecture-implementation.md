# Architecture Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Decompose the App god struct, split the flat AppEvent enum, eliminate duplicated state, and reorganize cyril-core modules — incrementally, with a compiling app at each commit.

**Architecture:** Two-crate workspace (`cyril-core` for protocol/platform, `cyril` for TUI). Extract `SessionContext` and `CommandExecutor` from App. Nest `AppEvent` into sub-enums grouped by handling semantics. Reorganize cyril-core modules under `protocol/` and `platform/` directories.

**Tech Stack:** Rust 2021 edition, ratatui, agent-client-protocol, tokio (current_thread + LocalSet), serde

**Design doc:** `docs/plans/2026-02-28-architecture-redesign.md`

---

### Task 1: Extract SessionContext into cyril-core

**Files:**
- Modify: `crates/cyril-core/src/session.rs` (replace existing `SessionState`)
- Modify: `crates/cyril-core/src/lib.rs` (no changes needed — `session` already declared)
- Modify: `crates/cyril/src/app.rs` (remove fields, delegate to SessionContext)
- Modify: `crates/cyril/src/main.rs` (call session methods instead of app methods)

**Step 1: Rewrite session.rs with SessionContext**

Replace the existing `SessionState` struct (which is underused) with `SessionContext`. Move `AvailableMode` from `app.rs` into this file.

```rust
// crates/cyril-core/src/session.rs
use std::path::PathBuf;
use agent_client_protocol as acp;

/// An available agent mode from the session.
#[derive(Debug, Clone)]
pub struct AvailableMode {
    pub id: String,
    pub name: String,
}

/// Owns all session-related state: ID, modes, config options, working directory.
///
/// This is the single source of truth for session data. UI components
/// borrow from this rather than maintaining their own copies.
#[derive(Debug)]
pub struct SessionContext {
    pub id: Option<acp::SessionId>,
    pub available_modes: Vec<AvailableMode>,
    pub config_options: Vec<acp::SessionConfigOption>,
    pub cwd: PathBuf,
    pub context_usage_pct: Option<f64>,
}

impl SessionContext {
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            id: None,
            available_modes: Vec::new(),
            config_options: Vec::new(),
            cwd,
            context_usage_pct: None,
        }
    }

    pub fn set_session_id(&mut self, session_id: acp::SessionId) {
        self.id = Some(session_id);
    }

    pub fn set_modes(&mut self, modes: &acp::SessionModeState) {
        self.current_mode_id = Some(modes.current_mode_id.to_string());
        self.available_modes = modes
            .available_modes
            .iter()
            .map(|m| AvailableMode {
                id: m.id.to_string(),
                name: m.name.clone(),
            })
            .collect();
    }

    pub fn set_config_options(&mut self, options: Vec<acp::SessionConfigOption>) {
        self.config_options = options;
    }

    /// Extract the current model value from stored config options.
    pub fn current_model(&self) -> Option<String> {
        self.config_options.iter().find_map(|opt| {
            if opt.id.to_string() == "model" {
                if let acp::SessionConfigKind::Select(ref select) = opt.kind {
                    return Some(select.current_value.to_string());
                }
            }
            None
        })
    }

    /// The current mode ID (from mode updates or initial session response).
    pub fn current_mode(&self) -> Option<&str> {
        // Find from available_modes or track separately
        // This will be refined during implementation
        None
    }

    /// Shortened session ID for display (first 8 chars).
    pub fn display_session_id(&self) -> &str {
        self.id
            .as_ref()
            .map(|id| {
                let s = id.to_string();
                // Can't return a slice of a temporary — store or use differently
                // Implementation will handle this
                "none"
            })
            .unwrap_or("none")
    }
}
```

Note: The exact `current_mode` tracking and `display_session_id` will be refined during implementation. The key point is that `SessionContext` owns this data and provides accessor methods.

**Step 2: Update App to hold SessionContext instead of individual fields**

In `crates/cyril/src/app.rs`:
- Remove `AvailableMode` struct (moved to session.rs)
- Remove fields: `session_id`, `available_modes`, `config_options`, `cwd`
- Add field: `session: SessionContext`
- Replace `self.session_id` with `self.session.id`
- Replace `self.cwd` with `self.session.cwd`
- Replace `self.available_modes` with `self.session.available_modes`
- Replace `self.config_options` with `self.session.config_options`
- Remove methods: `set_session_id()`, `set_modes()`, `set_config_options()`, `current_model_value()`
- Call `self.session.set_session_id()`, `self.session.set_modes()`, etc. instead

**Step 3: Update main.rs to use session methods**

In `crates/cyril/src/main.rs`:
- Replace `app.set_session_id(...)` with `app.session.set_session_id(...)`
- Replace `app.set_modes(...)` with `app.session.set_modes(...)`
- Replace `app.set_config_options(...)` with `app.session.set_config_options(...)`

**Step 4: Verify it compiles and runs**

Run: `cargo check`
Expected: Clean compilation with no errors.

Run: `cargo test -p cyril-core`
Expected: All existing tests pass.

**Step 5: Commit**

```bash
git add crates/cyril-core/src/session.rs crates/cyril/src/app.rs crates/cyril/src/main.rs
git commit -m "refactor: extract SessionContext from App into cyril-core"
```

---

### Task 2: Reorganize cyril-core modules

**Files:**
- Create: `crates/cyril-core/src/protocol/mod.rs`
- Move: `crates/cyril-core/src/client.rs` → `crates/cyril-core/src/protocol/client.rs`
- Move: `crates/cyril-core/src/transport.rs` → `crates/cyril-core/src/protocol/transport.rs`
- Create: `crates/cyril-core/src/platform/mod.rs`
- Move: `crates/cyril-core/src/path.rs` → `crates/cyril-core/src/platform/path.rs`
- Move: `crates/cyril-core/src/capabilities/terminal.rs` → `crates/cyril-core/src/platform/terminal.rs`
- Create: `crates/cyril-core/src/kiro_ext.rs`
- Modify: `crates/cyril-core/src/lib.rs` (update module declarations + re-exports)
- Remove: `crates/cyril-core/src/capabilities/mod.rs` (fs.rs stays or moves)
- Modify: `crates/cyril/src/main.rs` (update import paths)

**Step 1: Create protocol/ directory and move files**

```bash
mkdir -p crates/cyril-core/src/protocol
git mv crates/cyril-core/src/client.rs crates/cyril-core/src/protocol/client.rs
git mv crates/cyril-core/src/transport.rs crates/cyril-core/src/protocol/transport.rs
```

Create `crates/cyril-core/src/protocol/mod.rs`:
```rust
pub mod client;
pub mod transport;
```

**Step 2: Create platform/ directory and move files**

```bash
mkdir -p crates/cyril-core/src/platform
git mv crates/cyril-core/src/path.rs crates/cyril-core/src/platform/path.rs
```

Move terminal.rs from `capabilities/` to `platform/`:
```bash
git mv crates/cyril-core/src/capabilities/terminal.rs crates/cyril-core/src/platform/terminal.rs
```

Create `crates/cyril-core/src/platform/mod.rs`:
```rust
pub mod path;
pub mod terminal;
```

Keep `capabilities/fs.rs` in place (it's a pure async utility, not platform-specific). Update `capabilities/mod.rs` to only export `fs`.

**Step 3: Extract KiroExtCommand types to kiro_ext.rs**

Move `KiroExtCommand`, `KiroCommandMeta`, and `KiroExtCommand::is_executable()` from `crates/cyril-core/src/event.rs` to new file `crates/cyril-core/src/kiro_ext.rs`.

Also move `KiroCommandsPayload` from `protocol/client.rs` to `kiro_ext.rs` since it's Kiro-specific deserialization.

**Step 4: Update lib.rs with new module layout**

```rust
// crates/cyril-core/src/lib.rs
pub mod protocol;
pub mod platform;
pub mod session;
pub mod event;
pub mod kiro_ext;
pub mod capabilities;
pub mod hooks;

// Re-exports for backwards compatibility during migration.
// These can be removed once all consumers update their imports.
pub use protocol::client;
pub use protocol::transport;
pub use platform::path;
```

**Step 5: Update internal crate references**

In `protocol/client.rs`, update `use crate::` paths:
- `crate::capabilities` stays (fs.rs didn't move)
- `crate::capabilities::terminal` → `crate::platform::terminal`
- `crate::event::KiroExtCommand` → `crate::kiro_ext::KiroExtCommand`
- `crate::path` → `crate::platform::path`

**Step 6: Update cyril crate imports**

In `crates/cyril/src/main.rs`:
- `cyril_core::client::KiroClient` → `cyril_core::protocol::client::KiroClient` (or use re-export)
- `cyril_core::transport::AgentProcess` → `cyril_core::protocol::transport::AgentProcess` (or use re-export)
- `cyril_core::path` → `cyril_core::platform::path` (or use re-export)

In `crates/cyril/src/app.rs`:
- `cyril_core::path` → `cyril_core::platform::path` (or use re-export)
- `cyril_core::event::KiroExtCommand` → `cyril_core::kiro_ext::KiroExtCommand` (if used directly)

**Step 7: Verify it compiles and runs**

Run: `cargo check`
Expected: Clean compilation.

Run: `cargo test -p cyril-core`
Expected: All tests pass (path tests, fs tests, terminal tests, cache tests all reference internal paths that should be unaffected).

**Step 8: Commit**

```bash
git add -A crates/cyril-core/ crates/cyril/src/main.rs crates/cyril/src/app.rs
git commit -m "refactor: reorganize cyril-core into protocol/, platform/, kiro_ext modules"
```

---

### Task 3: Split AppEvent into nested enums

**Files:**
- Modify: `crates/cyril-core/src/event.rs` (restructure enum)
- Modify: `crates/cyril-core/src/protocol/client.rs` (update emit calls)
- Modify: `crates/cyril/src/app.rs` (update match arms)
- Modify: `crates/cyril/src/main.rs` (update oneshot event matching)
- Modify: `crates/cyril/src/event.rs` (Event::Acp wraps new AppEvent)

**Step 1: Define the sub-enums in event.rs**

Replace the flat `AppEvent` with nested structure. Keep `AppEvent` as the top-level enum so the channel type (`mpsc::UnboundedSender<AppEvent>`) doesn't change.

```rust
// crates/cyril-core/src/event.rs
use agent_client_protocol as acp;
use tokio::sync::oneshot;

/// Protocol-level events from ACP session notifications.
#[derive(Debug)]
pub enum ProtocolEvent {
    AgentMessage { session_id: acp::SessionId, chunk: acp::ContentChunk },
    AgentThought { session_id: acp::SessionId, chunk: acp::ContentChunk },
    ToolCallStarted { session_id: acp::SessionId, tool_call: acp::ToolCall },
    ToolCallUpdated { session_id: acp::SessionId, update: acp::ToolCallUpdate },
    PlanUpdated { session_id: acp::SessionId, plan: acp::Plan },
    ModeChanged { session_id: acp::SessionId, mode: acp::CurrentModeUpdate },
    ConfigOptionsUpdated { session_id: acp::SessionId, config_options: Vec<acp::SessionConfigOption> },
    CommandsUpdated { session_id: acp::SessionId, commands: acp::AvailableCommandsUpdate },
}

/// Requests from the agent that need a user response.
#[derive(Debug)]
pub enum InteractionRequest {
    Permission {
        request: acp::RequestPermissionRequest,
        responder: oneshot::Sender<acp::RequestPermissionResponse>,
    },
}

/// Kiro-specific extension events.
#[derive(Debug)]
pub enum ExtensionEvent {
    KiroCommandsAvailable { commands: Vec<crate::kiro_ext::KiroExtCommand> },
    KiroMetadata { session_id: String, context_usage_pct: f64 },
}

/// Internal application events (not from the agent).
#[derive(Debug)]
pub enum InternalEvent {
    HookFeedback { text: String },
}

/// Top-level event sent from KiroClient to the TUI.
#[derive(Debug)]
pub enum AppEvent {
    Protocol(ProtocolEvent),
    Interaction(InteractionRequest),
    Extension(ExtensionEvent),
    Internal(InternalEvent),
}
```

**Step 2: Update KiroClient emit calls**

In `crates/cyril-core/src/protocol/client.rs`, update every `self.emit(AppEvent::...)` call to use the nested constructors:

```rust
// Before:
self.emit(AppEvent::AgentMessage { session_id, chunk });
// After:
self.emit(AppEvent::Protocol(ProtocolEvent::AgentMessage { session_id, chunk }));

// Before:
self.emit(AppEvent::PermissionRequest { request, responder });
// After:
self.emit(AppEvent::Interaction(InteractionRequest::Permission { request, responder }));

// Before:
self.emit(AppEvent::KiroCommandsAvailable { commands });
// After:
self.emit(AppEvent::Extension(ExtensionEvent::KiroCommandsAvailable { commands }));

// Before:
self.emit(AppEvent::HookFeedback { text });
// After:
self.emit(AppEvent::Internal(InternalEvent::HookFeedback { text }));
```

Add `use crate::event::{ProtocolEvent, InteractionRequest, ExtensionEvent, InternalEvent};` to imports.

**Step 3: Split handle_acp_event in app.rs into four handlers**

Replace the single `handle_acp_event` method with four focused methods:

```rust
fn handle_acp_event(&mut self, event: AppEvent) {
    match event {
        AppEvent::Protocol(e) => self.handle_protocol_event(e),
        AppEvent::Interaction(r) => self.handle_interaction(r),
        AppEvent::Extension(e) => self.handle_extension_event(e),
        AppEvent::Internal(e) => self.handle_internal_event(e),
    }
}

fn handle_protocol_event(&mut self, event: ProtocolEvent) {
    match event {
        ProtocolEvent::AgentMessage { chunk, .. } => { ... }
        ProtocolEvent::AgentThought { chunk, .. } => { ... }
        ProtocolEvent::ToolCallStarted { tool_call, .. } => { ... }
        ProtocolEvent::ToolCallUpdated { update, .. } => { ... }
        ProtocolEvent::PlanUpdated { plan, .. } => { ... }
        ProtocolEvent::ModeChanged { mode, .. } => { ... }
        ProtocolEvent::ConfigOptionsUpdated { config_options, .. } => { ... }
        ProtocolEvent::CommandsUpdated { commands, .. } => { ... }
    }
}

fn handle_interaction(&mut self, request: InteractionRequest) {
    match request {
        InteractionRequest::Permission { request, responder } => { ... }
    }
}

fn handle_extension_event(&mut self, event: ExtensionEvent) {
    match event {
        ExtensionEvent::KiroCommandsAvailable { commands } => { ... }
        ExtensionEvent::KiroMetadata { context_usage_pct, .. } => { ... }
    }
}

fn handle_internal_event(&mut self, event: InternalEvent) {
    match event {
        InternalEvent::HookFeedback { text } => { ... }
    }
}
```

**Step 4: Update main.rs oneshot event matching**

In `run_oneshot()`, the event match uses `AppEvent` variants directly. Update to nested:

```rust
// Before:
AppEvent::AgentMessage { chunk, .. } => { ... }
AppEvent::PermissionRequest { request, responder } => { ... }

// After:
AppEvent::Protocol(ProtocolEvent::AgentMessage { chunk, .. }) => { ... }
AppEvent::Interaction(InteractionRequest::Permission { request, responder }) => { ... }
```

**Step 5: Verify it compiles and runs**

Run: `cargo check`
Expected: Clean compilation.

Run: `cargo test -p cyril-core`
Expected: All tests pass.

**Step 6: Commit**

```bash
git add crates/cyril-core/src/event.rs crates/cyril-core/src/protocol/client.rs \
       crates/cyril/src/app.rs crates/cyril/src/main.rs
git commit -m "refactor: split AppEvent into Protocol, Interaction, Extension, Internal sub-enums"
```

---

### Task 4: Eliminate duplicated state in ToolbarState

**Files:**
- Modify: `crates/cyril/src/ui/toolbar.rs` (remove duplicated fields, update render signature)
- Modify: `crates/cyril/src/app.rs` (update render call, remove mouse_captured field)

**Step 1: Remove duplicated fields from ToolbarState**

In `crates/cyril/src/ui/toolbar.rs`, remove fields that are copies of SessionContext data:

```rust
// Before:
pub struct ToolbarState {
    pub agent_name: String,
    pub agent_version: String,
    pub session_id: Option<String>,          // REMOVE
    pub is_busy: bool,
    pub context_usage_pct: Option<f64>,      // REMOVE
    pub selected_agent: Option<String>,
    pub current_mode: Option<String>,        // REMOVE
    pub current_model: Option<String>,       // REMOVE
    pub mouse_captured: bool,
}

// After:
pub struct ToolbarState {
    pub agent_name: String,
    pub agent_version: String,
    pub is_busy: bool,
    /// The --agent value passed at startup (e.g. "sonnet").
    pub selected_agent: Option<String>,
    /// Whether mouse capture is active (false = copy mode).
    pub mouse_captured: bool,
}
```

**Step 2: Update render() to take &SessionContext**

```rust
use cyril_core::session::SessionContext;

pub fn render(frame: &mut Frame, area: Rect, state: &ToolbarState, session: &SessionContext) {
    // Read session_id from session.id instead of state.session_id
    // Read current_mode from session (or state.selected_agent as fallback)
    // Read current_model from session.current_model()
    // Read context_usage_pct from session.context_usage_pct
    ...
}
```

Also update `render_context_bar` — it currently takes a `pct: f64` parameter which is fine (it's already decoupled). The caller in `App::render()` reads from `self.session.context_usage_pct`.

**Step 3: Update App**

In `crates/cyril/src/app.rs`:

Remove `App.mouse_captured` field. Read from `self.toolbar.mouse_captured` instead.

Update `App::render()`:
```rust
// Before:
toolbar::render(frame, chunks[0], &self.toolbar);

// After:
toolbar::render(frame, chunks[0], &self.toolbar, &self.session);
```

Update context bar:
```rust
// Before:
let pct = self.toolbar.context_usage_pct.unwrap_or(0.0);

// After:
let pct = self.session.context_usage_pct.unwrap_or(0.0);
```

Update `toggle_mouse_capture()`:
```rust
// Before:
self.mouse_captured = !self.mouse_captured;
self.toolbar.mouse_captured = self.mouse_captured;

// After:
self.toolbar.mouse_captured = !self.toolbar.mouse_captured;
```

Remove all sites that sync state to toolbar:
- `self.toolbar.session_id = ...` (removed — render reads from session)
- `self.toolbar.current_mode = ...` (removed — render reads from session)
- `self.toolbar.current_model = ...` (removed — render reads from session)
- `self.toolbar.context_usage_pct = ...` (removed — stored in session)

Update event handlers to write to `self.session` instead:
- `KiroMetadata { context_usage_pct }` → `self.session.context_usage_pct = Some(context_usage_pct)`
- `ModeChanged { mode }` → update mode tracking on `self.session`

**Step 4: Verify it compiles and runs**

Run: `cargo check`
Expected: Clean compilation.

**Step 5: Commit**

```bash
git add crates/cyril/src/ui/toolbar.rs crates/cyril/src/app.rs
git commit -m "refactor: eliminate duplicated state between App, ToolbarState, and SessionContext"
```

---

### Task 5: Extract CommandExecutor from App

**Files:**
- Modify: `crates/cyril/src/commands.rs` (add CommandExecutor)
- Modify: `crates/cyril/src/app.rs` (delegate to CommandExecutor)

**Step 1: Define CommandExecutor in commands.rs**

Move the following methods from `App` into free functions or a `CommandExecutor` struct in `commands.rs`:

- `execute_command()` → `CommandExecutor::execute()`
- `send_prompt()` → `CommandExecutor::send_prompt()`
- `create_new_session()` → `CommandExecutor::create_new_session()`
- `load_session()` → `CommandExecutor::load_session()`
- `execute_agent_command()` → `CommandExecutor::execute_agent_command()`
- `set_mode()` → `CommandExecutor::set_mode()`
- `set_model()` → `CommandExecutor::set_model()`
- `open_model_picker()` → `CommandExecutor::open_model_picker()`
- `handle_picker_confirm()` → `CommandExecutor::handle_picker_confirm()`

These methods need access to: `session: &mut SessionContext`, `conn: &Rc<ClientSideConnection>`, `chat: &mut ChatState`, `input: &mut InputState`, `toolbar: &mut ToolbarState`, `picker: &mut Option<PickerState>`, and the channel senders.

Rather than passing 8 parameters, group the channels:

```rust
/// Channels used by command execution to communicate with the event loop.
pub struct CommandChannels {
    pub prompt_done_tx: mpsc::UnboundedSender<()>,
    pub cmd_response_tx: mpsc::UnboundedSender<String>,
}
```

The executor can be a unit struct with associated functions:

```rust
pub struct CommandExecutor;

impl CommandExecutor {
    pub async fn execute(
        cmd: ParsedCommand,
        session: &mut SessionContext,
        conn: &Rc<acp::ClientSideConnection>,
        chat: &mut ChatState,
        input: &InputState,
        toolbar: &mut ToolbarState,
        picker: &mut Option<PickerState>,
        channels: &CommandChannels,
    ) -> Result<()> {
        match cmd {
            ParsedCommand::Quit => { /* set should_quit flag — return a signal */ }
            ParsedCommand::Clear => { ... }
            ParsedCommand::Help => { ... }
            ParsedCommand::New => Self::create_new_session(session, conn, chat).await?,
            ParsedCommand::Load(id) => Self::load_session(&id, session, conn, chat).await?,
            ParsedCommand::Mode(id) => Self::set_mode(&id, session, conn, chat).await?,
            ParsedCommand::ModelSelect(id) => Self::set_model(&id, session, conn, chat, toolbar, picker, channels).await?,
            ParsedCommand::Agent { name, input: arg } => { ... }
            ParsedCommand::Unknown(cmd) => { ... }
        }
        Ok(())
    }

    // ... private helper methods
}
```

Note: `ParsedCommand::Quit` needs special handling since it sets `App.should_quit`. Options:
- Return a `CommandResult` enum (`Continue`, `Quit`) instead of `Result<()>`
- Or keep `/quit` handling in App and don't pass it to CommandExecutor

The simpler option is to return a signal:

```rust
pub enum CommandResult {
    Continue,
    Quit,
}
```

**Step 2: Update App to delegate**

```rust
// In app.rs
async fn execute_command(&mut self, cmd: ParsedCommand) -> Result<()> {
    let result = commands::CommandExecutor::execute(
        cmd,
        &mut self.session,
        &self.conn,
        &mut self.chat,
        &self.input,
        &mut self.toolbar,
        &mut self.picker,
        &self.channels,
    ).await?;

    if matches!(result, commands::CommandResult::Quit) {
        self.should_quit = true;
    }
    Ok(())
}
```

Similarly for `send_prompt()` — move the prompt-building logic to CommandExecutor but keep the `self.toolbar.is_busy = true` in App (since it's UI state).

**Step 3: Move handle_picker_confirm**

`handle_picker_confirm()` is currently on App. Move to `CommandExecutor::handle_picker_confirm()`. App calls it from `handle_key()` when Enter is pressed in picker mode.

**Step 4: Verify it compiles and runs**

Run: `cargo check`
Expected: Clean compilation.

**Step 5: Commit**

```bash
git add crates/cyril/src/commands.rs crates/cyril/src/app.rs
git commit -m "refactor: extract CommandExecutor from App into commands.rs"
```

---

### Task 6: Add tests for extracted components

**Files:**
- Modify: `crates/cyril-core/src/session.rs` (add tests)
- Modify: `crates/cyril/src/commands.rs` (add tests)

**Step 1: Write SessionContext tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_session_has_no_id() {
        let ctx = SessionContext::new(PathBuf::from("/tmp"));
        assert!(ctx.id.is_none());
        assert!(ctx.available_modes.is_empty());
        assert!(ctx.config_options.is_empty());
    }

    #[test]
    fn set_session_id_stores_id() {
        let mut ctx = SessionContext::new(PathBuf::from("/tmp"));
        let id = acp::SessionId::from("test-123".to_string());
        ctx.set_session_id(id.clone());
        assert_eq!(ctx.id, Some(id));
    }

    #[test]
    fn current_model_returns_none_when_no_config() {
        let ctx = SessionContext::new(PathBuf::from("/tmp"));
        assert!(ctx.current_model().is_none());
    }

    // Test current_model extraction from config_options
    // Test set_modes populates available_modes
}
```

**Step 2: Write command parsing tests**

The existing `parse_command()` and `matching_suggestions()` functions already exist but have no tests. Add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_slash_quit() {
        let result = parse_command("/quit", &[]);
        assert!(matches!(result, Some(ParsedCommand::Quit)));
    }

    #[test]
    fn parse_slash_load_with_arg() {
        let result = parse_command("/load abc-123", &[]);
        assert!(matches!(result, Some(ParsedCommand::Load(ref id)) if id == "abc-123"));
    }

    #[test]
    fn parse_non_slash_returns_none() {
        let result = parse_command("hello world", &[]);
        assert!(result.is_none());
    }

    #[test]
    fn parse_unknown_command() {
        let result = parse_command("/foobar", &[]);
        assert!(matches!(result, Some(ParsedCommand::Unknown(_))));
    }

    #[test]
    fn parse_agent_command() {
        let agent_cmds = vec![AgentCommand {
            name: "compact".to_string(),
            description: "Compact context".to_string(),
            input_hint: None,
        }];
        let result = parse_command("/compact", &agent_cmds);
        assert!(matches!(result, Some(ParsedCommand::Agent { ref name, .. }) if name == "compact"));
    }

    #[test]
    fn matching_suggestions_filters_by_prefix() {
        let results = matching_suggestions("/cl", &[]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].display_name, "/clear");
    }

    #[test]
    fn matching_suggestions_empty_for_non_slash() {
        let results = matching_suggestions("hello", &[]);
        assert!(results.is_empty());
    }
}
```

**Step 3: Run all tests**

Run: `cargo test -p cyril-core`
Expected: All tests pass including new session.rs tests.

Run: `cargo test -p cyril`
Expected: All tests pass including new command tests.

**Step 4: Commit**

```bash
git add crates/cyril-core/src/session.rs crates/cyril/src/commands.rs
git commit -m "test: add unit tests for SessionContext and command parsing"
```

---

## Verification Checklist

After all tasks are complete:

1. `cargo check` — type-check passes
2. `cargo test -p cyril-core` — all tests pass
3. `cargo test -p cyril` — all tests pass
4. `cargo build` — full build succeeds
5. Manual: `cargo run` — TUI launches, toolbar displays correctly, commands work, @-completion works
6. Review: `app.rs` is ~400 lines (down from ~965)
7. Review: No duplicated state between `App`, `ToolbarState`, and `SessionContext`
8. Review: `AppEvent` match blocks are grouped by event category
