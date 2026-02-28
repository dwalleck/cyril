# Architecture Redesign — Extract Within Current Crates

## Problem

The codebase has three architectural issues that compound as the project grows:

1. **App is a god struct** — ~965 lines, ~20 fields mixing session management, command dispatch, event handling, hook feedback, and UI coordination. Hard to test, hard to extend.

2. **Flat event enum** — `AppEvent` mixes protocol notifications, interaction requests, Kiro-specific extensions, and internal events in a single enum. Adding new event types requires modifying a monolithic match block.

3. **Duplicated state** — `session_id` exists in both `App` and `ToolbarState`. `current_mode`, `current_model`, `context_usage_pct`, and `mouse_captured` are similarly duplicated. Sync bugs are possible when new code paths skip the setter methods.

## Approach

Extract and reorganize within the existing two-crate structure. No new crates — the codebase (~2500 lines) doesn't warrant three crates yet. Module boundaries are drawn so that promoting a module to its own crate later is straightforward.

## Design

### 1. SessionContext

A new struct in `cyril-core/src/session.rs` that owns all session-related state currently scattered across `App`:

```rust
pub struct SessionContext {
    pub id: Option<acp::SessionId>,
    pub available_modes: Vec<AvailableMode>,
    pub config_options: Vec<acp::SessionConfigOption>,
    pub cwd: PathBuf,
}
```

Methods that currently live on `App` move here: `set_session_id()`, `set_modes()`, `set_config_options()`, `current_model_value()`.

`AvailableMode` moves from `app.rs` to `session.rs` in cyril-core since it represents protocol state.

### 2. cyril-core Module Reorganization

```
cyril-core/src/
├── lib.rs
├── event.rs              # AppEvent (nested), ProtocolEvent, InteractionRequest
├── session.rs            # SessionContext, AvailableMode
├── kiro_ext.rs           # KiroExtCommand, KiroCommandMeta, ExtensionEvent, InternalEvent
├── protocol/
│   ├── mod.rs
│   ├── client.rs         # KiroClient (ACP Client trait impl)
│   └── transport.rs      # AgentProcess (subprocess spawning)
├── platform/
│   ├── mod.rs
│   ├── path.rs           # WSL <-> native path translation
│   └── terminal.rs       # TerminalManager, TerminalProcess
└── hooks/
    ├── mod.rs
    ├── types.rs
    ├── config.rs
    └── builtins.rs
```

- `client.rs` and `transport.rs` move under `protocol/` — they implement the ACP protocol
- `path.rs` and `terminal.rs` move under `platform/` — they abstract platform differences
- `kiro_ext.rs` is new — separates Kiro-specific types from generic protocol events
- `lib.rs` re-exports public types for backwards compatibility during migration

### 3. AppEvent Split

The flat `AppEvent` enum becomes nested:

```rust
pub enum AppEvent {
    Protocol(ProtocolEvent),
    Interaction(InteractionRequest),
    Extension(ExtensionEvent),
    Internal(InternalEvent),
}

pub enum ProtocolEvent {
    AgentMessage { session_id, chunk },
    AgentThought { session_id, chunk },
    ToolCallStarted { session_id, tool_call },
    ToolCallUpdated { session_id, update },
    PlanUpdated { session_id, plan },
    ModeChanged { session_id, mode },
    ConfigOptionsUpdated { session_id, config_options },
    CommandsUpdated { session_id, commands },
}

pub enum InteractionRequest {
    Permission { request, responder },
}

pub enum ExtensionEvent {
    KiroCommandsAvailable { commands: Vec<KiroExtCommand> },
    KiroMetadata { session_id: String, context_usage_pct: f64 },
}

pub enum InternalEvent {
    HookFeedback { text: String },
}
```

Event handling in `App` becomes a two-level dispatch:

```rust
fn handle_event(&mut self, event: AppEvent) {
    match event {
        AppEvent::Protocol(e) => self.handle_protocol_event(e),
        AppEvent::Interaction(r) => self.handle_interaction(r),
        AppEvent::Extension(e) => self.handle_extension_event(e),
        AppEvent::Internal(e) => self.handle_internal_event(e),
    }
}
```

### 4. State Deduplication

`ToolbarState` loses its copied fields. It keeps only state it truly owns:

```rust
pub struct ToolbarState {
    pub agent_name: String,
    pub agent_version: String,
    pub is_busy: bool,
    pub mouse_captured: bool,
}
```

`toolbar::render()` takes `&SessionContext` as an additional parameter and reads session-derived values (`session_id`, `current_mode`, `current_model`, `context_usage_pct`) directly from the source of truth.

`App.mouse_captured` is removed — `App` reads from `toolbar.mouse_captured` instead.

### 5. CommandExecutor

Command execution logic (~230 lines) moves from `App` to `commands.rs`:

```rust
pub struct CommandExecutor;

impl CommandExecutor {
    pub async fn execute(
        cmd: ParsedCommand,
        session: &mut SessionContext,
        conn: &Rc<ClientSideConnection>,
        chat: &mut ChatState,
        // ... channels for spawned tasks
    ) -> Result<()> { ... }
}
```

Methods that move: `execute_command()`, `send_prompt()`, `create_new_session()`, `load_session()`, `execute_agent_command()`, `set_mode()`, `set_model()`, `open_model_picker()`, `handle_picker_confirm()`.

`App::handle_enter()` delegates to `CommandExecutor`. `App` shrinks from ~965 to ~400 lines.

## Migration Order

Each step is a standalone commit that compiles and runs:

1. **Extract SessionContext** into `cyril-core/src/session.rs`. App holds `SessionContext` and delegates.
2. **Reorganize cyril-core modules** — file moves under `protocol/` and `platform/`, extract `kiro_ext.rs`. Re-exports in `lib.rs`.
3. **Split AppEvent** into nested enums. Update emit sites in `KiroClient` and match arms in `App`.
4. **Eliminate duplicated state** — change `toolbar::render()` to take `&SessionContext`, remove copied fields from `ToolbarState`.
5. **Extract CommandExecutor** from `App` into `commands.rs`.
6. **Add tests** for `SessionContext`, `CommandExecutor`, and split event handling.

Steps 1, 2, and 3 are largely independent. Steps 4 and 5 depend on Step 1. Step 6 comes last.

## Out of Scope

- Adding a third crate — the codebase doesn't warrant it yet
- Trait-based abstraction — no second implementation to design against
- Changing the `!Send` / `LocalSet` / `RefCell` concurrency model — it's correct for the current architecture
- Changing the render loop or ratatui patterns — they're already well-structured (stateless render functions)
