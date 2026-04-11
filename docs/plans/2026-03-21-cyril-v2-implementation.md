# Cyril v2 Re-Architecture Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Re-architect cyril from the ground up with a type-first, test-first approach across 3 crates, preserving working code by migrating it into the new skeleton.

**Architecture:** Three crates (`cyril-core`, `cyril-ui`, `cyril`) with strict dependency rules. ACP bridge on dedicated `!Send` thread communicates via bounded channels. Internal types insulate from `agent-client-protocol` crate. `TuiState` trait separates rendering from state. Command registry with `Command` trait enables testable dispatch.

**Tech Stack:** Rust 2024 edition, tokio 1.50 (multi-thread), ratatui 0.30, crossterm 0.29, agent-client-protocol 0.10, thiserror 2, serde, pulldown-cmark 0.13, syntect 5, insta, rstest

**Design doc:** `docs/plans/2026-03-21-cyril-v2-architecture-design.md`

**Key rules:**
- Zero `.unwrap()` / `.expect()` in non-test code
- Zero `let _ =` discarded results
- Zero `#[allow(...)]` directives
- `anyhow` is test-only; production code uses `thiserror` with `ErrorKind`
- Every type is `Send + Sync + Clone + Debug` (except `PermissionRequest` which owns a oneshot)
- Accessor methods, not pub fields on domain types

---

## Phase 1: Workspace Foundation

This phase creates the 3-crate workspace skeleton with all Cargo.toml configuration, lints, toolchain pinning, and the error types. At the end, `cargo check` passes on an empty workspace.

### Task 1.1: Create workspace Cargo.toml and rust-toolchain.toml

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Create: `rust-toolchain.toml`

**Step 1: Back up old v1 code to a branch**

```bash
git checkout -b v1-archive
git checkout -b v2-rewrite
```

This preserves v1 code on `v1-archive`. All v2 work happens on `v2-rewrite`.

**Step 2: Replace workspace Cargo.toml**

Replace the entire root `Cargo.toml` with the workspace definition from the design doc (Section 6 — workspace Cargo.toml). Include:
- `[workspace]` with `resolver = "2"`, `members` listing all 3 crates
- `[workspace.package]` with `edition = "2024"`, `rust-version = "1.94.0"`
- `[workspace.lints.rust]` with `unsafe_code = "forbid"`
- `[workspace.lints.clippy]` with `unwrap_used = "deny"`, `expect_used = "deny"`, `allow_attributes = "deny"`
- `[workspace.dependencies]` with ALL pinned dependency versions from the design doc
- Build profiles: `[profile.dev]`, `[profile.test]`, `[profile.release]`, `[profile.release-with-debug]`

**Step 3: Create rust-toolchain.toml**

```toml
[toolchain]
channel = "1.94.0"
profile = "minimal"
components = ["rustfmt", "clippy"]
```

**Step 4: Verify toolchain**

Run: `rustup show`
Expected: Active toolchain is `1.94.0`

**Step 5: Commit**

```bash
git add Cargo.toml rust-toolchain.toml
git commit -m "chore: set up v2 workspace with 2024 edition and safety lints"
```

---

### Task 1.2: Create cyril-core crate skeleton with error types

**Files:**
- Modify: `crates/cyril-core/Cargo.toml`
- Create: `crates/cyril-core/src/lib.rs`
- Create: `crates/cyril-core/src/error.rs`

**Step 1: Replace cyril-core Cargo.toml**

Remove all existing dependencies. Replace with:
```toml
[package]
name = "cyril-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true

[lints]
workspace = true

[dependencies]
thiserror = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true, features = ["sync", "rt", "macros", "time"] }
tracing = { workspace = true }

[dev-dependencies]
anyhow = { workspace = true }
rstest = { workspace = true }
tokio = { workspace = true, features = ["full", "test-util"] }
```

Add `version = "0.2.0-alpha.1"` to `[workspace.package]` in root Cargo.toml so it can be inherited.

**Step 2: Write the failing test for Error**

In `crates/cyril-core/src/error.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_displays_kind_message() {
        let err = Error::from_kind(ErrorKind::NoSession);
        assert_eq!(err.to_string(), "no active session");
    }

    #[test]
    fn error_kind_accessible() {
        let err = Error::from_kind(ErrorKind::Protocol { message: "timeout".into() });
        assert!(matches!(err.kind(), ErrorKind::Protocol { message } if message == "timeout"));
    }

    #[test]
    fn error_with_source_chains() {
        let source = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
        let err = Error::with_source(
            ErrorKind::Transport { detail: "connect failed".into() },
            source,
        );
        assert!(err.source().is_some());
    }
}
```

**Step 3: Run test to verify it fails**

Run: `cargo test -p cyril-core`
Expected: Compilation error — `Error`, `ErrorKind` not defined

**Step 4: Write minimal implementation**

In `crates/cyril-core/src/error.rs`:
```rust
use std::fmt;

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
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

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_deref()
    }
}

impl Error {
    pub fn from_kind(kind: ErrorKind) -> Self {
        Self { kind, source: None }
    }

    pub fn with_source(kind: ErrorKind, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self { kind, source: Some(Box::new(source)) }
    }

    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
}

pub type Result<T> = std::result::Result<T, Error>;
```

In `crates/cyril-core/src/lib.rs`:
```rust
pub mod error;
pub use error::{Error, ErrorKind, Result};
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p cyril-core`
Expected: 3 tests pass

**Step 6: Commit**

```bash
git add crates/cyril-core/
git commit -m "feat(core): add Error and ErrorKind types with tests"
```

---

### Task 1.3: Create cyril-ui crate skeleton with error types

**Files:**
- Create: `crates/cyril-ui/Cargo.toml`
- Create: `crates/cyril-ui/src/lib.rs`
- Create: `crates/cyril-ui/src/error.rs`

**Step 1: Create Cargo.toml**

```toml
[package]
name = "cyril-ui"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true

[lints]
workspace = true

[dependencies]
cyril-core = { path = "../cyril-core" }
ratatui = { workspace = true }
crossterm = { workspace = true }
tracing = { workspace = true }

[dev-dependencies]
anyhow = { workspace = true }
rstest = { workspace = true }
insta = { workspace = true }
```

**Step 2: Write failing test for ui Error**

In `crates/cyril-ui/src/error.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_error_displays() {
        let err = Error::from_kind(ErrorKind::Terminal { detail: "raw mode failed".into() });
        assert_eq!(err.to_string(), "terminal error: raw mode failed");
    }
}
```

**Step 3: Run test to verify it fails**

Run: `cargo test -p cyril-ui`
Expected: Compilation error

**Step 4: Write minimal implementation**

Same pattern as cyril-core's Error, but with `ErrorKind::Terminal` and `ErrorKind::Render` only.

In `crates/cyril-ui/src/lib.rs`:
```rust
pub mod error;
pub use error::{Error, ErrorKind, Result};
```

**Step 5: Run tests**

Run: `cargo test -p cyril-ui`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/cyril-ui/
git commit -m "feat(ui): create cyril-ui crate with error types"
```

---

### Task 1.4: Create cyril binary crate skeleton

**Files:**
- Modify: `crates/cyril/Cargo.toml`
- Modify: `crates/cyril/src/main.rs`

**Step 1: Replace Cargo.toml**

```toml
[package]
name = "cyril"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true

[[bin]]
name = "cyril"
path = "src/main.rs"

[lints]
workspace = true

[dependencies]
cyril-core = { path = "../cyril-core" }
cyril-ui = { path = "../cyril-ui" }
tokio = { workspace = true, features = ["full"] }
clap = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }

[dev-dependencies]
anyhow = { workspace = true }
rstest = { workspace = true }
```

**Step 2: Write minimal main.rs**

```rust
fn main() {
    println!("cyril v2 skeleton");
}
```

**Step 3: Verify full workspace builds**

Run: `cargo check`
Expected: All 3 crates compile clean with zero warnings

Run: `cargo clippy`
Expected: Zero warnings

Run: `cargo test`
Expected: All tests pass (error type tests from core + ui)

**Step 4: Commit**

```bash
git add crates/cyril/
git commit -m "feat: create cyril binary crate skeleton, full workspace compiles"
```

---

## Phase 2: Core Types

This phase builds all the internal types in `cyril_core::types` with tests. No protocol code yet — pure data types with invariants.

### Task 2.1: Session types (SessionId, SessionStatus, SessionMode, ContextUsage)

**Files:**
- Create: `crates/cyril-core/src/types/mod.rs`
- Create: `crates/cyril-core/src/types/session.rs`
- Modify: `crates/cyril-core/src/lib.rs`

**Step 1: Write failing tests**

In `crates/cyril-core/src/types/session.rs`, write tests for:
- `SessionId::new()` and `as_str()` round-trip
- `SessionId` implements `Hash + Eq` (use in HashMap)
- `SessionStatus::default()` is `Disconnected`
- `ContextUsage::new(50.0)` stores 50.0
- `ContextUsage::new(150.0)` clamps to 100.0
- `ContextUsage::new(-10.0)` clamps to 0.0
- `SessionMode` accessors return correct values

**Step 2: Run tests — expect compilation failure**

**Step 3: Implement types**

Follow the design doc Section 2 — `SessionId(String)`, `SessionStatus` enum, `SessionMode` with private fields + accessors, `ContextUsage` with clamping constructor.

Add `pub mod types;` to `lib.rs`.

**Step 4: Run tests — expect pass**

**Step 5: Commit**

```bash
git commit -m "feat(core): add session types with invariants"
```

---

### Task 2.2: Tool call types (ToolCallId, ToolKind, ToolCallStatus, ToolCall)

**Files:**
- Create: `crates/cyril-core/src/types/tool_call.rs`

**Step 1: Write failing tests**

Test:
- `ToolCallId::new()` / `as_str()` round-trip
- `ToolCallId` usable as HashMap key
- `ToolCall` accessor methods all return correct values
- `ToolKind` and `ToolCallStatus` equality

**Step 2: Run — fail**

**Step 3: Implement**

Follow design doc — `ToolCallId(String)`, `ToolKind` enum, `ToolCallStatus` enum, `ToolCall` struct with private fields and accessor methods.

**Step 4: Run — pass**

**Step 5: Commit**

```bash
git commit -m "feat(core): add tool call types"
```

---

### Task 2.3: Message and plan types

**Files:**
- Create: `crates/cyril-core/src/types/message.rs`
- Create: `crates/cyril-core/src/types/plan.rs`

**Step 1: Write failing tests**

Test:
- `AgentMessage` construction and field access
- `AgentThought` construction
- `Plan::entries()` returns correct slice
- `PlanEntry` accessors
- `PlanEntryStatus` equality

**Step 2–5: Implement, test, commit**

```bash
git commit -m "feat(core): add message and plan types"
```

---

### Task 2.4: Command and config types

**Files:**
- Create: `crates/cyril-core/src/types/command.rs`
- Create: `crates/cyril-core/src/types/config.rs`

**Step 1: Write failing tests**

Test:
- `CommandInfo` accessors
- `CommandOption` fields
- `ConfigOption` fields
- `Config::default()` has sensible values
- `UiConfig::default().max_messages` is 500
- `AgentConfig::default().agent_name` is "kiro-cli"

Config loading tests (with `tempfile`):
- Load from valid TOML file
- Load with missing file returns defaults
- Env var override replaces config value

**Step 2–5: Implement, test, commit**

For the config tests, use `tempfile::tempdir()` and write a test TOML file. For env var tests, set/unset vars within the test (note: env var tests should be `#[serial]` or use isolated config loading that takes a path parameter rather than relying on the global `OnceLock`).

Design note: Make `Config::load_from(path, env_overrides)` testable, and have the `OnceLock` call that internally.

```bash
git commit -m "feat(core): add command and config types"
```

---

### Task 2.5: Event types (Notification, PermissionRequest, BridgeCommand)

**Files:**
- Create: `crates/cyril-core/src/types/event.rs`

**Step 1: Write failing tests**

Test:
- Each `Notification` variant is constructable
- `Notification` is `Clone + Send + Sync` (compile-time check)
- `PermissionRequest` is `Send` but NOT `Clone` (owns oneshot)
- `PermissionResponse` variants
- `BridgeCommand` variants are exhaustive
- `PermissionOption` construction

For compile-time Send/Sync checks:
```rust
fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}
fn assert_clone<T: Clone>() {}

#[test]
fn notification_is_send_sync_clone() {
    assert_send::<Notification>();
    assert_sync::<Notification>();
    assert_clone::<Notification>();
}

#[test]
fn permission_request_is_send_not_clone() {
    assert_send::<PermissionRequest>();
    // PermissionRequest is NOT Clone — this is by design
}
```

**Step 2–5: Implement, test, commit**

This requires `tokio::sync::oneshot` in the `PermissionRequest` struct. Depends on all prior types (ToolCall, Plan, SessionId, etc.).

```bash
git commit -m "feat(core): add event types (Notification, PermissionRequest, BridgeCommand)"
```

---

### Task 2.6: Types module re-exports

**Files:**
- Modify: `crates/cyril-core/src/types/mod.rs`
- Modify: `crates/cyril-core/src/lib.rs`

**Step 1: Organize re-exports**

`types/mod.rs` should publicly re-export all types:
```rust
pub mod session;
pub mod tool_call;
pub mod message;
pub mod plan;
pub mod command;
pub mod config;
pub mod event;

// Convenience re-exports
pub use session::*;
pub use tool_call::*;
pub use message::*;
pub use plan::*;
pub use command::*;
pub use event::*;
```

`lib.rs` should expose:
```rust
pub mod error;
pub mod types;
pub use error::{Error, ErrorKind, Result};
```

**Step 2: Verify full workspace compiles**

Run: `cargo check`
Expected: Clean

Run: `cargo test -p cyril-core`
Expected: All tests pass

**Step 3: Commit**

```bash
git commit -m "feat(core): organize types module re-exports"
```

---

## Phase 3: Session Controller & Platform

### Task 3.1: Migrate path translation module

**Files:**
- Move: `crates/cyril-core/src/platform/path.rs` (preserve from v1)
- Create: `crates/cyril-core/src/platform/mod.rs`

**Step 1: Copy the existing path.rs from v1**

The existing `path.rs` is a pure utility module with comprehensive tests. Copy it directly, updating only the module path references.

**Step 2: Verify tests pass**

Run: `cargo test -p cyril-core -- path`
Expected: All 8 path translation tests pass

**Step 3: Commit**

```bash
git commit -m "feat(core): migrate path translation module from v1"
```

---

### Task 3.2: SessionController state machine

**Files:**
- Create: `crates/cyril-core/src/session.rs`
- Modify: `crates/cyril-core/src/lib.rs`

**Step 1: Write failing tests**

Test the full state machine:
- `SessionController::new()` starts as `Disconnected`
- `apply_notification(TurnCompleted)` transitions `Busy -> Active`
- `apply_notification(BridgeDisconnected)` transitions any state -> `Disconnected`
- `apply_notification(ModeChanged)` updates `current_mode_id` and returns `true`
- `apply_notification(ContextUsageUpdated)` stores usage and returns `true`
- `apply_notification(ConfigOptionsUpdated)` updates config
- `apply_notification(CommandsUpdated)` stores commands
- `apply_notification(AgentMessage)` returns `false` (not session state)
- `id()` returns `None` initially
- Session creation flow: call methods to set id, modes, status

**Step 2: Run — fail**

**Step 3: Implement**

Follow design doc Section 4 — `SessionController` with private fields, accessor methods, and `apply_notification()` that pattern-matches on `Notification` variants.

**Step 4: Run — pass**

**Step 5: Commit**

```bash
git commit -m "feat(core): add SessionController state machine"
```

---

## Phase 4: UI Foundation

### Task 4.1: TuiState trait and UiState struct

**Files:**
- Create: `crates/cyril-ui/src/traits.rs`
- Create: `crates/cyril-ui/src/state.rs`
- Modify: `crates/cyril-ui/src/lib.rs`

**Step 1: Write failing tests**

In `state.rs`, test:
- `UiState::new()` has empty messages, version 0
- `apply_notification(AgentMessage)` appends to streaming text, returns true
- `apply_notification(TurnCompleted)` commits streaming to messages, increments version
- `apply_notification(ToolCallStarted)` adds to active tool calls
- `apply_notification(ToolCallUpdated)` updates existing tool call by id
- `activity()` returns correct Activity variant based on state
- Message limit enforcement (push >500 messages, verify oldest removed)

In `traits.rs`, compile-time test:
```rust
// Verify TuiState is object-safe
fn assert_object_safe(_: &dyn TuiState) {}
```

**Step 2–5: Implement, test, commit**

Define `TuiState` trait with all ~25 methods from design doc Section 5. Implement on `UiState`. Include `MockTuiState` in `#[cfg(test)]` module.

```bash
git commit -m "feat(ui): add TuiState trait and UiState implementation"
```

---

### Task 4.2: StreamBuffer

**Files:**
- Create: `crates/cyril-ui/src/stream_buffer.rs`

**Step 1: Write failing tests**

Test:
- `push("hello\nworld")` returns `Some("hello\n")`, leaves "world" in buffer
- `push("no newline")` returns `None`
- `push("```rust\ncode")` flushes at code fence boundary
- `flush()` returns remaining content
- `flush()` on empty buffer returns `None`
- `should_flush()` returns true after timeout elapsed (use `tokio::time::pause()`)

**Step 2–5: Implement, test, commit**

Follow jcode's StreamBuffer pattern: accumulate, flush at newlines or code fences, timeout flush.

```bash
git commit -m "feat(ui): add StreamBuffer for semantic streaming chunks"
```

---

### Task 4.3: Migrate syntax highlighting

**Files:**
- Create: `crates/cyril-ui/src/highlight.rs`

**Step 1: Copy highlight.rs from v1**

Migrate `crates/cyril/src/ui/highlight.rs`. Update imports (remove `super::cache` dependency — either inline the cache or use a simple `HashMap` with LRU eviction for now).

**Step 2: Update for new crate structure**

- Replace `use super::cache::HashCache` with an inline LRU cache or migrate `cache.rs` as a utility module first
- Verify no `unwrap()` / `expect()` in non-test code — replace with proper error handling
- Replace any `let _ =` patterns

**Step 3: Run tests**

Run: `cargo test -p cyril-ui -- highlight`
Expected: All existing highlight tests pass

**Step 4: Commit**

```bash
git commit -m "feat(ui): migrate syntax highlighting from v1"
```

---

### Task 4.4: Migrate markdown rendering

**Files:**
- Create: `crates/cyril-ui/src/widgets/mod.rs`
- Create: `crates/cyril-ui/src/widgets/markdown.rs`

**Step 1: Add pulldown-cmark to cyril-ui dependencies**

```toml
pulldown-cmark = { workspace = true }
syntect = { workspace = true }
similar = { workspace = true }
nucleo-matcher = { workspace = true }
```

**Step 2: Migrate markdown.rs from v1**

Copy `crates/cyril/src/ui/markdown.rs`. Update:
- Import paths to use `crate::highlight` instead of `super::highlight`
- Replace cache with migrated version
- Audit for zero-tolerance violations (unwrap, let _ =)

**Step 3: Run tests**

Run: `cargo test -p cyril-ui -- markdown`
Expected: All existing markdown tests pass

**Step 4: Commit**

```bash
git commit -m "feat(ui): migrate markdown rendering from v1"
```

---

### Task 4.5: Panic-safe renderer skeleton

**Files:**
- Create: `crates/cyril-ui/src/render.rs`

**Step 1: Write test**

```rust
#[test]
fn draw_fallback_does_not_panic() {
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|frame| {
        draw_fallback(frame);
    }).unwrap();
    // If we get here, fallback rendering works
}
```

**Step 2: Implement**

```rust
pub fn draw(frame: &mut ratatui::Frame, state: &dyn crate::traits::TuiState) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        draw_inner(frame, state);
    }));
    if result.is_err() {
        draw_fallback(frame);
    }
}

fn draw_inner(frame: &mut ratatui::Frame, state: &dyn crate::traits::TuiState) {
    // Layout: toolbar | chat | input | status bar
    // Placeholder widgets for now — each widget task fills these in
    let area = frame.area();
    let text = ratatui::widgets::Paragraph::new("cyril v2");
    frame.render_widget(text, area);
}

fn draw_fallback(frame: &mut ratatui::Frame) {
    let text = ratatui::widgets::Paragraph::new("Render error - press Ctrl+C to quit");
    frame.render_widget(text, frame.area());
}
```

**Step 3: Run tests — pass**

**Step 4: Commit**

```bash
git commit -m "feat(ui): add panic-safe renderer skeleton"
```

---

## Phase 5: ACP Bridge

### Task 5.1: Transport (AgentProcess spawning)

**Files:**
- Create: `crates/cyril-core/src/protocol/mod.rs`
- Create: `crates/cyril-core/src/protocol/transport.rs`

**Step 1: Add agent-client-protocol to cyril-core**

Update `crates/cyril-core/Cargo.toml`:
```toml
agent-client-protocol = { workspace = true }
async-trait = { workspace = true }
tokio-util = { workspace = true }
futures-util = { workspace = true }
```

**Step 2: Implement AgentProcess**

Migrate from v1's `transport.rs`, adapting to use cyril-core error types. This module spawns the `kiro-cli acp` subprocess and provides async stdin/stdout handles.

Key changes from v1:
- Return `crate::Result<AgentProcess>` not `anyhow::Result`
- Map `std::io::Error` to `ErrorKind::Transport`
- No `unwrap()` — handle process spawn failure gracefully

**Step 3: Verify compiles**

Run: `cargo check -p cyril-core`
Expected: Clean (can't unit test subprocess spawning without a live kiro-cli)

**Step 4: Commit**

```bash
git commit -m "feat(core): add AgentProcess transport"
```

---

### Task 5.2: Type conversion layer (convert.rs)

**Files:**
- Create: `crates/cyril-core/src/protocol/convert.rs`

**Step 1: Write failing tests**

Test each conversion function:
- `to_tool_kind(acp::ToolKind::Read)` -> `ToolKind::Read`
- `to_tool_kind(acp::ToolKind::Other)` -> `ToolKind::Other`
- `to_tool_call_status` for all variants
- `to_notification` for `AgentMessageChunk` -> `Notification::AgentMessage`
- `to_notification` for `ToolCall` -> `Notification::ToolCallStarted`
- `to_ext_notification` for known methods (metadata, commands/available, agent/switched, compaction/status, clear/status)
- `to_ext_notification` for unknown method returns `Err(ErrorKind::Protocol)`
- `to_permission_options` extracts options correctly

**Step 2: Run — fail**

**Step 3: Implement**

Follow design doc Section 3 — `convert.rs` is `pub(crate)` and the only file importing both `acp::` and `crate::types::`.

**Step 4: Run — pass**

**Step 5: Commit**

```bash
git commit -m "feat(core): add ACP type conversion layer"
```

---

### Task 5.3: BridgeHandle and channel setup

**Files:**
- Create: `crates/cyril-core/src/protocol/bridge.rs`

**Step 1: Write failing tests**

Test:
- `BridgeHandle::send()` on a closed channel returns `Err(BridgeClosed)`
- `BridgeHandle::recv_notification()` returns `None` when sender dropped
- `BridgeHandle::recv_permission()` returns `None` when sender dropped
- Channel capacity is bounded (send 33 commands to a capacity-32 channel — the 33rd should not block indefinitely; use `try_send` or timeout)

**Step 2: Run — fail**

**Step 3: Implement BridgeHandle**

```rust
pub struct BridgeHandle {
    command_tx: mpsc::Sender<BridgeCommand>,
    notification_rx: mpsc::Receiver<Notification>,
    permission_rx: mpsc::Receiver<PermissionRequest>,
}
```

With methods: `send()`, `recv_notification()`, `recv_permission()`, `clone_sender()`.

Also implement `BridgeSender` (the cloneable command-sending half):
```rust
#[derive(Clone)]
pub struct BridgeSender {
    command_tx: mpsc::Sender<BridgeCommand>,
}
```

**Step 4: Run — pass**

**Step 5: Commit**

```bash
git commit -m "feat(core): add BridgeHandle and channel infrastructure"
```

---

### Task 5.4: KiroClient (acp::Client implementation)

**Files:**
- Create: `crates/cyril-core/src/protocol/client.rs`

**Step 1: Implement KiroClient**

This is the `!Send` implementation of `acp::Client`. It lives inside the bridge thread and uses `RefCell<HashMap>` for tool call input caching.

Migrate from v1's `client.rs`, with these changes:
- Use `convert::to_notification()` instead of manually constructing `AppEvent`
- Send through `mpsc::Sender<Notification>` instead of `UnboundedSender<AppEvent>`
- Use `convert::to_permission_options()` for permission requests
- All `.send()` calls return `Result` — propagate errors, never discard

**Step 2: Verify compiles**

Run: `cargo check -p cyril-core`
Expected: Clean (KiroClient is not directly unit-testable without an ACP connection, but convert.rs tests cover the logic)

**Step 3: Commit**

```bash
git commit -m "feat(core): add KiroClient ACP implementation"
```

---

### Task 5.5: Bridge spawn function

**Files:**
- Modify: `crates/cyril-core/src/protocol/bridge.rs`

**Step 1: Implement spawn_bridge()**

```rust
pub fn spawn_bridge(agent: &str, cwd: PathBuf) -> Result<BridgeHandle> { ... }
```

This spawns a dedicated `std::thread` with its own `current_thread` tokio runtime and `LocalSet`. The bridge thread runs `run_bridge()` which:
1. Spawns AgentProcess
2. Creates KiroClient
3. Runs ACP handshake
4. Enters command loop

Follow design doc Section 3 exactly.

**Step 2: Verify compiles**

Run: `cargo check -p cyril-core`
Expected: Clean

**Step 3: Commit**

```bash
git commit -m "feat(core): add bridge spawn with dedicated thread"
```

---

## Phase 6: Command System

### Task 6.1: Command trait and registry

**Files:**
- Create: `crates/cyril-core/src/commands/mod.rs`
- Modify: `crates/cyril-core/src/lib.rs`

**Step 1: Write failing tests**

Test:
- Registry starts empty (no commands registered) — `parse("/unknown")` returns `None`
- Register a command, `parse("/name")` returns it
- Register with alias, both name and alias resolve
- `parse("not a command")` (no slash) returns `None`
- `parse("/name args here")` splits name and args correctly
- `all_commands()` deduplicates aliases

**Step 2–5: Implement, test, commit**

Define `Command` trait, `CommandContext`, `CommandResult`, `CommandResultKind`, `CommandRegistry`. No builtins yet — just the registry infrastructure.

```bash
git commit -m "feat(core): add Command trait and CommandRegistry"
```

---

### Task 6.2: Builtin commands

**Files:**
- Create: `crates/cyril-core/src/commands/builtin.rs`

**Step 1: Write failing tests**

Test each command in isolation with mock context:
- `HelpCommand.execute()` returns `SystemMessage`
- `ClearCommand.execute()` returns `SystemMessage`
- `QuitCommand.execute()` returns `Quit`
- `NewCommand.execute()` sends `BridgeCommand::NewSession` and returns `Dispatched`

**Step 2–5: Implement, test, commit**

Implement `HelpCommand`, `ClearCommand`, `QuitCommand`, `NewCommand`, `LoadCommand`, `ModeCommand`, `ModelCommand`, `AgentProxyCommand`.

```bash
git commit -m "feat(core): add builtin commands"
```

---

## Phase 7: Widgets

Each widget task follows the same pattern: write snapshot test with MockTuiState + TestBackend + insta, implement the widget, verify snapshot.

### Task 7.1: Toolbar widget

**Files:**
- Create: `crates/cyril-ui/src/widgets/toolbar.rs`

**Step 1: Write snapshot test**

```rust
#[test]
fn toolbar_renders_session_info() {
    let mut state = MockTuiState::default();
    state.session_label = Some("my-session".into());
    state.current_mode = Some("code".into());
    state.current_model = Some("claude-sonnet-4".into());
    state.terminal_size = (80, 1);

    let backend = TestBackend::new(80, 1);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| {
        toolbar::render(frame, frame.area(), &state);
    }).unwrap();

    insta::assert_snapshot!(format_buffer(terminal.backend().buffer()));
}
```

**Step 2–5: Implement, test, commit**

```bash
git commit -m "feat(ui): add toolbar widget with snapshot tests"
```

---

### Task 7.2: Chat widget

Similar pattern — snapshot tests for: empty chat, user + agent messages, streaming text, tool calls inline.

```bash
git commit -m "feat(ui): add chat widget with snapshot tests"
```

---

### Task 7.3: Input widget

Custom input replacing tui-textarea. Snapshot tests for: empty input, text with cursor, autocomplete dropdown visible.

Unit tests for `InputState`: insert char, delete, cursor movement, history navigation, take_text clears.

```bash
git commit -m "feat(ui): add custom input widget with snapshot tests"
```

---

### Task 7.4: Approval widget

Snapshot tests for: permission request with options, selected option highlighted.

```bash
git commit -m "feat(ui): add approval widget with snapshot tests"
```

---

### Task 7.5: Picker widget

Snapshot tests for: options list, filter applied, selected item.

Unit tests for `PickerState`: filter updates, selection navigation, selected_option.

```bash
git commit -m "feat(ui): add picker widget with snapshot tests"
```

---

### Task 7.6: Status bar widget

Snapshot tests for: context usage gauge, credit display.

```bash
git commit -m "feat(ui): add status bar widget with snapshot tests"
```

---

### Task 7.7: Wire renderer to widgets

**Files:**
- Modify: `crates/cyril-ui/src/render.rs`

Replace the placeholder in `draw_inner()` with actual layout + widget calls. Add a full-frame snapshot test with MockTuiState.

```bash
git commit -m "feat(ui): wire renderer to all widgets"
```

---

## Phase 8: App Wiring

### Task 8.1: Logging setup and CLI args

**Files:**
- Modify: `crates/cyril/src/main.rs`

Implement:
- clap CLI args (--cwd, --prompt, --agent)
- `setup_logging()` function (JSON to file)
- tokio multi-thread runtime setup

```bash
git commit -m "feat: add CLI args and logging setup"
```

---

### Task 8.2: App struct and event loop

**Files:**
- Create: `crates/cyril/src/app.rs`

Implement the App struct and event loop from design doc Section 4:
- `biased` select with 4 arms
- Adaptive frame rate
- Conditional redraw
- Input draining
- Notification routing to SessionController + UiState

```bash
git commit -m "feat: add App struct with event loop"
```

---

### Task 8.3: Key handling and command submission

**Files:**
- Modify: `crates/cyril/src/app.rs`

Implement:
- Three-layer key handling (global -> modal -> normal)
- Command submission through registry
- CommandResult handling
- Prompt sending via BridgeCommand

```bash
git commit -m "feat: add key handling and command submission"
```

---

### Task 8.4: Wire main.rs to App

**Files:**
- Modify: `crates/cyril/src/main.rs`

Wire everything together:
1. Parse CLI args
2. Setup logging
3. Spawn bridge
4. Create SessionController, UiState, CommandRegistry
5. Create App
6. Initialize terminal (crossterm raw mode, alternate screen)
7. Run event loop
8. Restore terminal on exit

```bash
git commit -m "feat: wire main.rs to App, complete TUI lifecycle"
```

---

## Phase 9: Integration & Polish

### Task 9.1: End-to-end smoke test

**Files:**
- Create: `crates/cyril/tests/event_routing.rs`

Write an integration test that:
1. Creates a MockBridge with queued notifications
2. Creates App with the mock bridge
3. Feeds notifications through
4. Asserts UiState and SessionController have correct state

```bash
git commit -m "test: add end-to-end event routing integration test"
```

---

### Task 9.2: File completer migration

**Files:**
- Create: `crates/cyril-ui/src/file_completer.rs`

Migrate `@`-reference autocomplete from v1. Update to use async file loading.

```bash
git commit -m "feat(ui): migrate file completer from v1"
```

---

### Task 9.3: Cleanup and final verification

**Step 1: Run full test suite**

```bash
cargo test
```

**Step 2: Run clippy**

```bash
cargo clippy -- -D warnings
```

**Step 3: Check for zero-tolerance violations**

```bash
# Should return zero matches in non-test code
grep -rn '\.unwrap()' crates/*/src/*.rs crates/*/src/**/*.rs --include='*.rs' | grep -v '#\[cfg(test)\]' | grep -v 'mod tests'
grep -rn '\.expect(' crates/*/src/*.rs crates/*/src/**/*.rs --include='*.rs' | grep -v '#\[cfg(test)\]' | grep -v 'mod tests'
grep -rn 'let _ =' crates/*/src/*.rs crates/*/src/**/*.rs --include='*.rs' | grep -v '#\[cfg(test)\]' | grep -v 'mod tests'
```

**Step 4: Build release**

```bash
cargo build --release
```

**Step 5: Commit and tag**

```bash
git commit -m "chore: final cleanup and verification"
git tag v0.2.0-alpha.1
```

---

## Task Dependency Summary

```
Phase 1: Workspace Foundation
  1.1 Workspace Cargo.toml ─→ 1.2 cyril-core errors ─→ 1.3 cyril-ui errors ─→ 1.4 binary skeleton

Phase 2: Core Types (all depend on 1.2)
  2.1 Session types ─→ 2.2 Tool call types ─→ 2.3 Message/plan ─→ 2.4 Command/config ─→ 2.5 Events ─→ 2.6 Re-exports

Phase 3: Session & Platform (depends on 2.6)
  3.1 Path translation (independent)
  3.2 SessionController (depends on 2.6)

Phase 4: UI Foundation (depends on 1.3 + 2.6)
  4.1 TuiState trait + UiState
  4.2 StreamBuffer (independent)
  4.3 Highlight (independent)
  4.4 Markdown (depends on 4.3)
  4.5 Renderer skeleton (depends on 4.1)

Phase 5: ACP Bridge (depends on 2.6)
  5.1 Transport ─→ 5.2 Convert ─→ 5.3 BridgeHandle ─→ 5.4 KiroClient ─→ 5.5 Bridge spawn

Phase 6: Commands (depends on 2.6 + 5.3)
  6.1 Registry ─→ 6.2 Builtins

Phase 7: Widgets (depends on 4.1)
  7.1-7.6 each independent
  7.7 Wire renderer (depends on 7.1-7.6)

Phase 8: App Wiring (depends on all phases)
  8.1 CLI ─→ 8.2 Event loop ─→ 8.3 Key handling ─→ 8.4 Wire main

Phase 9: Integration
  9.1 E2E test ─→ 9.2 File completer ─→ 9.3 Cleanup
```

**Parallelizable:** Phases 3, 4, 5 can run in parallel after Phase 2 completes. Phases 6 and 7 can run in parallel after their respective dependencies.
