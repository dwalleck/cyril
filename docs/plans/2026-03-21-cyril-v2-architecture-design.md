# Cyril v2 Architecture Design

**Date:** 2026-03-21
**Status:** Approved
**Scope:** Full re-architecture — new foundation with types and tests first, then migrate working code

## Design Decisions

| Aspect | Decision | Rationale |
|---|---|---|
| Migration strategy | New foundation, then migrate | Tests before implementation; preserve working code |
| Threading model | `Send` / multi-threaded | Future multi-session/background processing |
| Crate structure | 3 crates | `cyril-core` (types + protocol), `cyril-ui` (rendering), `cyril` (binary) |
| Input widget | Custom (replace tui-textarea) | Full control, testable via TuiState |
| Capabilities layer | None needed | Kiro never calls fs/terminal callbacks |
| Agent scope | Kiro-specific | YAGNI; refactor if multi-agent needed |
| Type ownership | Internal types, convert at boundary | Insulate from ACP crate + handle Kiro spec deviations |
| ACP bridge | Dedicated thread with channels | `!Send` ACP types quarantined; everything else is `Send` |
| Edition / Toolchain | 2024 / Rust 1.94.0 pinned | Latest stable features |

---

## 1. Crate Structure & Dependency Rules

```
cyril/
├── Cargo.toml                    # Workspace root
├── rust-toolchain.toml           # Pin to 1.94.0
├── crates/
│   ├── cyril-core/               # Types + Protocol + Orchestration
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── types/            # Shared internal types (Send + Sync)
│   │   │   │   ├── mod.rs
│   │   │   │   ├── message.rs    # ContentBlock, AgentMessage, AgentThought
│   │   │   │   ├── tool_call.rs  # ToolCall, ToolKind, ToolCallStatus, DiffSummary
│   │   │   │   ├── plan.rs       # Plan, PlanEntry, PlanEntryStatus
│   │   │   │   ├── session.rs    # SessionStatus, SessionMode, SessionConfig
│   │   │   │   ├── event.rs      # Notification, InteractionRequest, ExtensionEvent
│   │   │   │   ├── command.rs    # CommandInfo, CommandOption (from Kiro)
│   │   │   │   └── config.rs     # Config, UiConfig, with serde + OnceLock
│   │   │   │
│   │   │   ├── protocol/         # ACP bridge (only place that imports acp::)
│   │   │   │   ├── mod.rs
│   │   │   │   ├── bridge.rs     # AcpBridge: dedicated thread, channel endpoints
│   │   │   │   ├── client.rs     # KiroClient: acp::Client impl (!Send, lives in bridge)
│   │   │   │   ├── transport.rs  # AgentProcess: subprocess spawning
│   │   │   │   └── convert.rs    # acp:: → internal type conversions
│   │   │   │
│   │   │   ├── session.rs        # SessionController: lifecycle state machine
│   │   │   ├── bus.rs            # EventBus: typed broadcast channel
│   │   │   ├── commands/         # Command registry + dispatch
│   │   │   │   ├── mod.rs        # CommandRegistry, Command trait, CommandContext
│   │   │   │   └── builtin.rs    # /help, /clear, /quit, /new, /load, /mode, /model, etc.
│   │   │   │
│   │   │   └── platform/         # OS abstractions
│   │   │       ├── mod.rs
│   │   │       └── path.rs       # Windows <-> WSL path translation
│   │   │
│   │   └── Cargo.toml
│   │
│   ├── cyril-ui/                 # TUI rendering (no protocol knowledge)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── state.rs          # UiState struct (implements TuiState trait)
│   │   │   ├── traits.rs         # TuiState trait definition
│   │   │   ├── render.rs         # draw(frame, &dyn TuiState) - top-level renderer
│   │   │   ├── stream_buffer.rs  # Semantic chunking for streaming text
│   │   │   ├── widgets/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── chat.rs       # Chat message rendering
│   │   │   │   ├── input.rs      # Custom input widget (replaces tui-textarea)
│   │   │   │   ├── toolbar.rs    # Status bar, spinner, activity indicator
│   │   │   │   ├── approval.rs   # Permission request dialog
│   │   │   │   ├── picker.rs     # Selection dropdown
│   │   │   │   └── markdown.rs   # Markdown parsing + rendering + cache
│   │   │   │
│   │   │   └── highlight.rs      # Syntax highlighting with LRU cache
│   │   │
│   │   └── Cargo.toml
│   │
│   └── cyril/                    # Binary - wires everything together
│       ├── src/
│       │   ├── main.rs           # CLI args, runtime setup, wiring
│       │   ├── app.rs            # App: thin orchestrator, owns components, runs event loop
│       │   └── event_router.rs   # Maps Notifications -> UiState mutations
│       │
│       └── Cargo.toml
│
└── docs/
    └── kiro-acp-protocol.md      # Protocol reference (kept)
```

**Dependency rules (strict, enforced by crate boundaries):**

- `cyril-core` -> `agent-client-protocol` (only crate that imports `acp::`)
- `cyril-ui` -> `cyril-core` (for types in `cyril_core::types::` only - never `protocol::`)
- `cyril` -> `cyril-core` + `cyril-ui` (wires them together)
- `cyril-ui` **never** imports `agent-client-protocol`
- `cyril-core::protocol` is `pub(crate)` - external crates cannot access it

---

## 2. Types & Error Handling

### Error Architecture

Each crate exports its own `Error` type with a `kind()` accessor and a `Result<T>` alias. No `anyhow` in production code - `anyhow` is reserved for test helpers only.

**`cyril-core`:**

```rust
#[derive(Debug, thiserror::Error)]
#[error("{kind}")]
pub struct Error {
    kind: ErrorKind,
    #[source]
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl Error {
    pub fn kind(&self) -> &ErrorKind { &self.kind }
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
    #[error("protocol error: {message}")]
    Protocol { message: String },

    #[error("agent process failed: {detail}")]
    Transport { detail: String },

    #[error("agent process exited unexpectedly (code {exit_code:?})")]
    AgentExited { exit_code: Option<i32>, stderr: String },

    #[error("no active session")]
    NoSession,

    #[error("session {id} not found")]
    SessionNotFound { id: String },

    #[error("unknown command: {name}")]
    UnknownCommand { name: String },

    #[error("command failed: {detail}")]
    CommandFailed { detail: String },

    #[error("bridge channel closed")]
    BridgeClosed,

    #[error("permission request timed out")]
    PermissionTimeout,

    #[error("invalid configuration: {detail}")]
    InvalidConfig { detail: String },
}

pub type Result<T> = std::result::Result<T, Error>;
```

**`cyril-ui`:**

```rust
#[derive(Debug, thiserror::Error)]
#[error("{kind}")]
pub struct Error {
    kind: ErrorKind,
    #[source]
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
    #[error("terminal error: {detail}")]
    Terminal { detail: String },

    #[error("render failed: {detail}")]
    Render { detail: String },
}

pub type Result<T> = std::result::Result<T, Error>;
```

**Conversion at boundaries:** `convert.rs` maps `acp::Error` -> `ErrorKind::Protocol`. The `acp` crate's error types never propagate beyond the protocol module.

**Zero-tolerance rules (enforced from day 1):**
- Zero `.unwrap()` / `.expect()` in non-test code
- Zero `let _ =` discarded results - every `Result` handled or propagated
- Zero `#[allow(...)]` directives
- Channel sends return `Result`, callers handle or propagate

### Internal Type System

All types in `cyril_core::types` are `Send + Sync + Clone + Debug`. The `acp::` crate is never visible outside `cyril_core::protocol`.

**Newtype wrappers:**
- `ToolCallId(String)` - Hash + Eq for map keys
- `SessionId(String)` - prevents mixing up string IDs

**Accessor methods, not pub fields** on `ToolCall`, `Plan`, `PlanEntry`, `CommandInfo`.

**Type-enforced invariants:**
- `ContextUsage::new()` clamps to 0-100
- `SessionStatus` enum makes illegal states unrepresentable

**Key event types:**

- `Notification` - all protocol events crossing into the Send world (Clone + Send + Sync)
- `PermissionRequest` - agent requests needing user response (NOT Clone - owns oneshot sender)
- `BridgeCommand` - all commands from App to bridge (exhaustive enum)

---

## 3. ACP Bridge Architecture

The bridge runs on a dedicated OS thread with its own `current_thread` tokio runtime and `LocalSet`. The `!Send` ACP types (`Rc<RefCell<>>`, `#[async_trait(?Send)]`) are quarantined here. Communication with the `Send` world is via bounded channels.

```
Main tokio runtime (multi_thread, Send)     Dedicated thread (!Send)
┌──────────────────────────────┐    ┌──────────────────────────┐
│                              │    │                          │
│  App                         │    │  AcpBridge               │
│  ├── SessionController       │    │  ├── AgentProcess        │
│  ├── UiState                 │    │  ├── KiroClient          │
│  ├── CommandRegistry         │    │  ├── ClientSideConnection│
│  │                           │    │  └── convert.rs          │
│  │   BridgeCommand ─(mpsc)──────→ │                          │
│  │                           │    │                          │
│  │   Notification  ←(mpsc)──────── │                          │
│  │                           │    │                          │
│  │   PermissionReq ←(mpsc)──────── │                          │
│  │        │                  │    │       ↑                  │
│  │        └─(oneshot)────────────→ │  (response)             │
│  │                           │    │                          │
└──────────────────────────────┘    └──────────────────────────┘
```

**Channel capacities (bounded, explicit backpressure):**

| Channel | Direction | Capacity |
|---|---|---|
| `command_tx` | App -> Bridge | 32 |
| `notification_tx` | Bridge -> App | 256 |
| `permission_tx` | Bridge -> App | 16 |

**`BridgeHandle`** is the Send-safe handle held by App. Methods: `send()`, `recv_notification()`, `recv_permission()`.

**`KiroClient`** implements `acp::Client` inside the bridge thread. It:
- Caches `raw_input` from ToolCall/ToolCallUpdate notifications
- Enriches permission requests with cached raw_input
- Converts all acp types to internal types via `convert.rs`
- Sends notifications and permission requests through channels
- Never discards send results

**`convert.rs`** is the single file that imports both `acp::` and `cyril_core::types::`. All Kiro spec deviations are handled here.

**Permission request flow:**
1. kiro-cli calls `request_permission()` on KiroClient (bridge thread)
2. KiroClient creates oneshot channel, sends `PermissionRequest` through `permission_tx`
3. App receives, shows approval dialog on UiState
4. User selects Allow/Reject, App sends response through oneshot
5. KiroClient receives, converts back to acp type, returns to kiro-cli

If the main thread drops the oneshot sender (user quits), the bridge gets `Err` - no silent failure.

---

## 4. App Architecture & Event Loop

`App` is a thin orchestrator with no business logic:

```rust
pub struct App {
    bridge: BridgeHandle,
    ui_state: UiState,
    session: SessionController,
    commands: CommandRegistry,
    redraw_needed: bool,
    last_activity: Instant,
}
```

### Event Loop

```rust
loop {
    tokio::select! {
        biased;
        // Priority 1: Terminal input
        Some(event) = event_stream.next() => { ... }
        // Priority 2: Notifications from bridge
        Some(notification) = self.bridge.recv_notification() => { ... }
        // Priority 3: Permission requests from bridge
        Some(request) = self.bridge.recv_permission() => { ... }
        // Priority 4: Redraw tick
        _ = redraw_interval.tick() => { ... }
    }

    // Adaptive frame rate based on activity
    redraw_interval = tokio::time::interval(Self::redraw_duration(self.ui_state.activity()));

    // Conditional redraw
    if self.redraw_needed {
        terminal.draw(|frame| cyril_ui::render::draw(frame, &self.ui_state))?;
        self.redraw_needed = false;
    }
}
```

**Key properties:**
- `biased` - keyboard input always wins over ticks
- Input draining - after one key event, drain all remaining buffered events before drawing
- Adaptive frame rate: Streaming 20fps, Waiting 10fps, Ready 4fps, Idle 1fps
- Conditional redraw - only call `draw()` when state changed

### Event Routing

Notifications are applied to both `SessionController` and `UiState`:

```rust
fn handle_notification(&mut self, notification: Notification) {
    let session_changed = self.session.apply_notification(&notification);
    let ui_changed = self.ui_state.apply_notification(&notification);
    self.redraw_needed = session_changed || ui_changed;
}
```

Both `apply_notification` methods are pure state transitions - no async, no side effects, trivially testable.

### Key Handling Layers

1. **Global shortcuts** (always active): Ctrl+C, Ctrl+Q, Ctrl+M
2. **Modal overlays** (consume input if active): approval dialog, picker
3. **Normal input**: text entry, autocomplete, command submission

### Command Submission

Input goes through `CommandRegistry::parse()` first. If it's a slash command, execute it and handle the `CommandResult`. Otherwise send as a prompt via `BridgeCommand::SendPrompt`.

---

## 5. UiState & TuiState Trait

### TuiState Trait

Read-only trait with ~25 methods. The renderer receives `&dyn TuiState`, never `&App`. Every method returns a reference or Copy type - compile-time guarantee that rendering cannot mutate state.

Key methods: `messages()`, `streaming_text()`, `activity()`, `approval()`, `picker()`, `messages_version()`, `terminal_size()`, `context_usage()`.

### UiState

Implements `TuiState`. Owns:
- Chat messages (`Vec<ChatMessage>` with version counter)
- Stream buffer (semantic chunking with 150ms timeout)
- Active tool calls with `HashMap<ToolCallId, usize>` index for O(1) updates
- Input state (custom widget: text, cursor, history, autocomplete)
- Approval state (current + pending queue)
- Picker state (filter, fuzzy match, selection)

`apply_notification()` handles streaming text, tool calls, plans, turn completion. On `TurnCompleted`, streaming content is committed to the message list and the version counter increments.

### ChatMessage

Display-layer type with `ChatMessageKind` enum: `UserText`, `AgentText`, `Thought`, `ToolCall(TrackedToolCall)`, `Plan`, `System`.

### TrackedToolCall

Wraps internal `ToolCall` and caches a computed `DiffSummary` and display label.

### Renderer

```rust
pub fn draw(frame: &mut Frame, state: &dyn TuiState) {
    // Panic-safe wrapper with fallback rendering
}
```

Layout: toolbar (1 line) | chat (min 5) | input (5 lines) | status bar (1 line). Overlays (approval, picker) render on top.

---

## 6. Command System & Configuration

### Command Registry

```rust
pub trait Command: Send + Sync {
    fn name(&self) -> &str;
    fn aliases(&self) -> &[&str] { &[] }
    fn description(&self) -> &str;
    fn is_local(&self) -> bool { true }
    async fn execute(&self, ctx: &CommandContext<'_>, args: &str) -> Result<CommandResult>;
}
```

`CommandRegistry` stores `HashMap<String, Arc<dyn Command>>`. Agent-provided commands (from `kiro.dev/commands/available`) are auto-registered as `AgentProxyCommand` instances.

`CommandResult` has variants: `SystemMessage`, `NotACommand`, `ShowPicker`, `Dispatched`, `Quit`. App handles each variant without knowing command internals.

`CommandContext` provides read-only session state and a bridge sender - no mutable UI state access.

### Configuration

`OnceLock<Config>` loaded from `~/.config/cyril/config.toml` with env var overrides. Sections: `UiConfig` (max_messages, highlight_cache_size, stream_buffer_timeout_ms, mouse_capture) and `AgentConfig` (agent_name, extra_args). All fields have sensible defaults via `#[serde(default)]`.

### Workspace Cargo.toml

- `edition = "2024"`, `rust-version = "1.94.0"`
- `unsafe_code = "forbid"`, `unwrap_used = "deny"`, `expect_used = "deny"`, `allow_attributes = "deny"`
- All deps in `[workspace.dependencies]`, referenced with `{ workspace = true }`
- `default-features = false` on crossterm, pulldown-cmark, futures-util, syntect
- Build profiles: dev (fast compile), test (opt-level 1), release (lto fat, strip symbols), release-with-debug

---

## 7. Test Strategy & Project Infrastructure

### Test Organization

- **Unit tests colocated** (`#[cfg(test)] mod tests` in same file) for: types, convert.rs, SessionController, UiState, StreamBuffer, commands, path translation
- **Integration tests in `tests/`** for: session lifecycle, bridge command round-trips, command registry, render snapshots
- **Snapshot tests** using `ratatui::TestBackend` + `insta` for visual regression testing of widgets

### Test Doubles

- `MockBridge` - records sent commands, plays back canned notifications
- `MockTuiState` - default implementation of TuiState trait, override fields per test
- `MockBridgeSender` - records ext_method calls, queues responses

### Test Phases (mirrors architecture)

1. **Types** - invariant enforcement (clamping, status transitions)
2. **Conversion** - acp -> internal type mapping, unknown variant handling
3. **State machines** - SessionController + UiState notification processing
4. **Commands** - each command in isolation with mock context
5. **Rendering** - widget snapshots with MockTuiState
6. **Integration** - full event routing with mock bridge

### Dev Dependencies (test-only)

`anyhow`, `rstest`, `insta`, `tempfile`, `tokio` with `test-util` feature.

### Logging

Structured JSON to `~/.config/cyril/cyril.log`. Session context via `tracing::info_span!`. No terminal output.

### Toolchain

`rust-toolchain.toml` pins to `1.94.0` with `profile = "minimal"` and `components = ["rustfmt", "clippy"]`.

---

## Reference: Quality Checklist Compliance

From `rust-project-quality-checklist.md`:

| Rule | Status |
|---|---|
| Forbid unsafe globally | Enforced in workspace lints |
| Inherit lints in every crate | `[lints] workspace = true` |
| Pin exact Rust version | `rust-toolchain.toml` with `1.94.0` |
| All versions in one place | `[workspace.dependencies]` |
| Disable default features | Applied to crossterm, pulldown-cmark, futures-util, syntect |
| thiserror with ErrorKind enum | Per-crate Error + ErrorKind |
| Exported Result alias | `pub type Result<T>` in each crate |
| Accessor methods, not pub fields | ToolCall, Plan, PlanEntry, CommandInfo, Error |
| Zero unwrap/expect | Clippy deny |
| Zero let _ = discarded results | All channel sends return Result |
| Zero allow directives | Clippy deny |
| Map external errors at boundary | convert.rs handles acp:: errors |
| Arc<dyn Trait> for shared ownership | Command registry, bridge handle |
| Unit tests colocated | #[cfg(test)] mod tests |
| Integration tests in tests/ | Per-crate tests/ directory |
| tempfile for isolation | Dev dependency |
| Build profiles | dev/test/release/release-with-debug |
