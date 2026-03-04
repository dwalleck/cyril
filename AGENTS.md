# AGENTS.md - AI Assistant Guide for Cyril

> **Purpose:** This file provides AI coding assistants with essential context about the Cyril project that is not covered in README.md or other user-facing documentation. It focuses on development patterns, code organization, testing practices, and assistant-specific guidance.

**Last Updated:** 2026-03-03  
**Baseline Commit:** 7b8366b1  
**Documentation Version:** 1.0

---

## Table of Contents

1. [Quick Start for AI Assistants](#quick-start-for-ai-assistants)
2. [Project Structure](#project-structure)
3. [Architecture Overview](#architecture-overview)
4. [Development Patterns](#development-patterns)
5. [Testing Guidelines](#testing-guidelines)
6. [Code Style and Conventions](#code-style-and-conventions)
7. [Common Tasks](#common-tasks)
8. [Troubleshooting](#troubleshooting)
9. [Additional Resources](#additional-resources)

---

## Quick Start for AI Assistants

### What is Cyril?

Cyril is a **cross-platform TUI client** for Kiro CLI that communicates via the Agent Client Protocol (ACP). It's written in Rust and organized as a two-crate workspace.

**Key Facts:**
- **Language:** Rust (Edition 2021)
- **Size:** Large (429 files, ~6,783 LOC)
- **Architecture:** Two-crate workspace (binary + library)
- **Status:** Alpha - functional but under active development
- **License:** MIT

### Core Responsibilities

**Binary Crate (`cyril`):** TUI application
- User interface rendering (ratatui)
- Event handling and input processing
- Command parsing and execution
- Markdown rendering with syntax highlighting

**Library Crate (`cyril-core`):** Protocol and platform logic
- ACP protocol client implementation
- Platform abstraction (Windows/WSL path translation)
- Hook system for extensibility
- File system capabilities

### Key Architectural Decisions

1. **Two-Crate Design:** Separates UI from protocol logic for reusability and testability
2. **Event-Driven:** All interactions flow through an event system
3. **Cross-Platform:** Windows support via WSL bridge with automatic path translation
4. **Hook System:** Extensible automation at the protocol boundary
5. **Streaming:** Real-time markdown rendering as content arrives

---

## Project Structure

### Directory Layout

```
cyril/
├── crates/
│   ├── cyril/              # Binary crate (TUI application)
│   │   ├── src/
│   │   │   ├── main.rs     # Entry point (245 LOC)
│   │   │   ├── app.rs      # Event loop (459 LOC)
│   │   │   ├── commands.rs # Command system (905 LOC) ⭐ Most complex
│   │   │   ├── file_completer.rs # File completion (183 LOC)
│   │   │   ├── tui.rs      # Terminal setup (26 LOC)
│   │   │   ├── event.rs    # Event types (29 LOC)
│   │   │   └── ui/         # UI components
│   │   │       ├── input.rs      # Input field (299 LOC)
│   │   │       ├── chat.rs       # Message display (287 LOC)
│   │   │       ├── markdown.rs   # Markdown rendering (243 LOC)
│   │   │       ├── highlight.rs  # Syntax highlighting (116 LOC)
│   │   │       ├── tool_calls.rs # Tool display (291 LOC)
│   │   │       ├── approval.rs   # Approval UI (203 LOC)
│   │   │       ├── picker.rs     # Selection UI (171 LOC)
│   │   │       ├── toolbar.rs    # Status bar (139 LOC)
│   │   │       └── cache.rs      # LRU cache (89 LOC)
│   │   └── Cargo.toml
│   └── cyril-core/         # Library crate (protocol & platform)
│       ├── src/
│       │   ├── lib.rs      # Public API (12 LOC)
│       │   ├── session.rs  # Session state (216 LOC)
│       │   ├── event.rs    # Event types (77 LOC)
│       │   ├── kiro_ext.rs # Kiro extensions (196 LOC)
│       │   ├── protocol/   # ACP protocol
│       │   │   ├── client.rs    # ACP client (358 LOC)
│       │   │   └── transport.rs # Process mgmt (161 LOC)
│       │   ├── platform/   # Platform abstraction
│       │   │   ├── path.rs      # Path translation (306 LOC)
│       │   │   └── terminal.rs  # Terminal mgmt (361 LOC) ⭐ Highly complex
│       │   ├── hooks/      # Hook system
│       │   │   ├── types.rs     # Hook registry (101 LOC)
│       │   │   ├── config.rs    # Hook loading (452 LOC) ⭐ Most complex in core
│       │   │   └── builtins.rs  # Built-in hooks (41 LOC)
│       │   └── capabilities/ # File operations
│       │       └── fs.rs        # File I/O (73 LOC)
│       └── Cargo.toml
├── docs/               # Documentation and plans
├── .kiro/              # Kiro CLI configuration
│   ├── skills/         # Development skills (15+ skills)
│   ├── agents/         # Custom agents (10+ agents)
│   ├── hooks/          # Git hooks (rustfmt)
│   └── settings/       # LSP configuration
├── .claude/            # Claude AI configuration
├── .agents/            # AI-generated documentation
│   └── summary/        # Comprehensive documentation
└── Cargo.toml          # Workspace manifest
```

⭐ = Most complex components (>400 LOC or high complexity)

### File Organization Patterns

**Binary Crate (`cyril`):**
- `main.rs` - Entry point, CLI parsing, connection setup
- `app.rs` - Main event loop, state management, rendering coordination
- `commands.rs` - Command parsing, execution, autocomplete (largest file)
- `ui/` - UI components, each in its own file
- `event.rs` - Event type definitions

**Library Crate (`cyril-core`):**
- `lib.rs` - Public API exports
- `protocol/` - ACP client and transport
- `platform/` - OS-specific abstractions
- `hooks/` - Hook system implementation
- `capabilities/` - Agent capabilities (file I/O, etc.)
- `session.rs` - Session state management
- `event.rs` - Event type definitions

**Configuration:**
- `.kiro/` - Kiro CLI configuration (skills, agents, hooks, LSP)
- `.claude/` - Claude AI configuration
- `hooks.json` - User-defined hooks (in working directory)

---

## Architecture Overview

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Cyril TUI (Binary)                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │
│  │   UI     │  │   App    │  │ Commands │  │   File   │   │
│  │ Layer    │◄─┤  Event   │◄─┤  System  │◄─┤Completer │   │
│  │(ratatui) │  │   Loop   │  │          │  │          │   │
│  └──────────┘  └────┬─────┘  └──────────┘  └──────────┘   │
└─────────────────────┼─────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                  Cyril Core (Library)                       │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │
│  │   ACP    │  │ Platform │  │   Hook   │  │   File   │   │
│  │  Client  │  │  Layer   │  │  System  │  │   I/O    │   │
│  │          │  │(Path/Term)│  │          │  │          │   │
│  └────┬─────┘  └──────────┘  └──────────┘  └──────────┘   │
└───────┼─────────────────────────────────────────────────────┘
        │
        ▼ JSON-RPC 2.0 over stdio
┌─────────────────────────────────────────────────────────────┐
│              kiro-cli acp (Agent Process)                   │
│         (Linux: native | Windows: via WSL)                  │
└─────────────────────────────────────────────────────────────┘
```

### Key Architectural Patterns

**Event-Driven Architecture:**
- All interactions flow through `AppEvent` enum
- Async event handling with tokio
- Channel-based communication between components

**Separation of Concerns:**
- UI logic in `cyril` crate
- Protocol logic in `cyril-core` crate
- Platform-specific code isolated in `platform/` module

**Cross-Platform Abstraction:**
- Path translation layer for Windows/WSL
- Platform detection at runtime
- Transparent path conversion in JSON payloads

**Hook System:**
- Runs at protocol boundary
- Before hooks can block operations
- After hooks provide feedback
- Glob pattern matching for file filtering

---

## Development Patterns

### Error Handling

**Use `anyhow::Result` for application errors:**
```rust
use anyhow::{Result, Context};

pub fn read_config(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)
        .context("Failed to read config file")?;
    let config = serde_json::from_str(&content)
        .context("Failed to parse config")?;
    Ok(config)
}
```

**Use `thiserror` for library errors:**
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}
```

### Async Patterns

**Use tokio for async operations:**
```rust
use tokio::fs;
use tokio::process::Command;

pub async fn execute_command(cmd: &str) -> Result<String> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .await?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
```

**Use channels for communication:**
```rust
use tokio::sync::mpsc;

let (tx, mut rx) = mpsc::unbounded_channel();

// Send events
tx.send(AppEvent::Internal(InternalEvent::SessionCreated(id)))?;

// Receive events
while let Some(event) = rx.recv().await {
    handle_event(event).await?;
}
```

### State Management

**Centralized state in `App` struct:**
```rust
pub struct App {
    // Core state
    client: KiroClient,
    session: SessionContext,
    
    // UI component states
    chat: ChatState,
    input: InputState,
    toolbar: ToolbarState,
    
    // Tracking
    tool_calls: HashMap<String, TrackedToolCall>,
}
```

**Immutable updates where possible:**
```rust
// Good: Return new state
pub fn with_session_id(mut self, id: String) -> Self {
    self.session_id = Some(id);
    self
}

// Also good: Mutable update when needed
pub fn set_session_id(&mut self, id: String) {
    self.session_id = Some(id);
}
```

### Testing Patterns

**Unit tests in same file:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_translation() {
        let wsl_path = win_to_wsl(r"C:\Users\name\file.txt");
        assert_eq!(wsl_path, "/mnt/c/Users/name/file.txt");
    }
}
```

**Async tests with tokio:**
```rust
#[tokio::test]
async fn test_file_write() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("test.txt");
    
    write_text_file(&path, "content").await.unwrap();
    
    let content = fs::read_to_string(&path).await.unwrap();
    assert_eq!(content, "content");
}
```

---

## Testing Guidelines

### Test Organization

**Test Coverage:**
- 50+ test functions across modules
- Focus on critical paths (path translation, terminal management, hooks)
- Integration tests for protocol communication

**Test Locations:**
- Unit tests: Same file as implementation (`#[cfg(test)]` module)
- Integration tests: `tests/` directory (if needed)

### Running Tests

```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p cyril-core

# Run specific test
cargo test test_path_translation

# Run with output
cargo test -- --nocapture

# Run with logging
RUST_LOG=debug cargo test
```

### Test Patterns

**Roundtrip Testing (Path Translation):**
```rust
#[test]
fn test_roundtrip_win_wsl_win() {
    let original = r"C:\Users\name\project\file.txt";
    let wsl = win_to_wsl(original);
    let back = wsl_to_win(&wsl);
    assert_eq!(original, back);
}
```

**Edge Case Testing:**
```rust
#[test]
fn test_empty_input() {
    let result = parse_command("");
    assert!(matches!(result, ParsedCommand::None));
}

#[test]
fn test_invalid_json() {
    let result = parse_json("{invalid}");
    assert!(result.is_err());
}
```

**Mock-Friendly Design:**
```rust
// Use traits for testability
pub trait FileSystem {
    async fn read(&self, path: &Path) -> Result<String>;
    async fn write(&self, path: &Path, content: &str) -> Result<()>;
}

// Real implementation
pub struct RealFileSystem;

// Test implementation
pub struct MockFileSystem {
    files: HashMap<PathBuf, String>,
}
```

---

## Code Style and Conventions

### Rust Style

**Follow standard Rust conventions:**
- Use `rustfmt` for formatting (automated via git hooks)
- Use `clippy` for linting
- Follow Rust API guidelines

**Naming Conventions:**
- Types: `PascalCase` (e.g., `ChatState`, `ToolCallKind`)
- Functions: `snake_case` (e.g., `parse_command`, `run_hooks`)
- Constants: `SCREAMING_SNAKE_CASE` (e.g., `MAX_OUTPUT_SIZE`)
- Modules: `snake_case` (e.g., `file_completer`, `tool_calls`)

### Documentation

**Document public APIs:**
```rust
/// Parse a command from user input.
///
/// Recognizes slash commands (e.g., `/help`, `/quit`) and agent commands.
/// Returns `ParsedCommand::None` for empty input or non-command text.
///
/// # Examples
///
/// ```
/// let cmd = parse_command("/help");
/// assert!(matches!(cmd, ParsedCommand::Slash(SlashCommand::Help)));
/// ```
pub fn parse_command(input: &str) -> ParsedCommand {
    // Implementation
}
```

**Use inline comments for complex logic:**
```rust
// Translate paths in JSON payload recursively
// This handles nested objects and arrays
pub fn translate_paths_in_json(value: &mut Value, direction: Direction) {
    match value {
        Value::String(s) => {
            // Only translate if it looks like a path
            if looks_like_path(s) {
                *s = translate_path(s, direction);
            }
        }
        Value::Object(map) => {
            for v in map.values_mut() {
                translate_paths_in_json(v, direction);
            }
        }
        // ... handle other cases
    }
}
```

### Git Hooks

**Automated formatting on commit:**
- `.kiro/hooks/rustfmt.sh` - Runs rustfmt on staged Rust files
- `.claude/hooks/rustfmt.sh` - Same for Claude integration

**Hook script:**
```bash
#!/bin/bash
# Format staged Rust files
git diff --cached --name-only --diff-filter=ACM | \
  grep '\.rs$' | \
  xargs -r rustfmt --edition 2021
```

---

## Common Tasks

### Adding a New UI Component

1. Create new file in `crates/cyril/src/ui/`
2. Define state struct
3. Implement rendering method
4. Add to `App` state
5. Call from `App::render()`

**Example:**
```rust
// crates/cyril/src/ui/my_component.rs
use ratatui::{Frame, layout::Rect, widgets::Paragraph};

pub struct MyComponentState {
    content: String,
}

impl MyComponentState {
    pub fn new() -> Self {
        Self {
            content: String::new(),
        }
    }
    
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let widget = Paragraph::new(self.content.as_str());
        frame.render_widget(widget, area);
    }
}
```

### Adding a New ACP Method

1. Add method to `KiroClient` in `crates/cyril-core/src/protocol/client.rs`
2. Define request/response types
3. Implement method using `emit()`
4. Add tests

**Example:**
```rust
impl KiroClient {
    pub async fn my_new_method(&mut self, param: &str) -> Result<String> {
        let params = json!({ "param": param });
        let result = self.emit("acp/myNewMethod", params).await?;
        Ok(result["value"].as_str().unwrap().to_string())
    }
}
```

### Adding a New Hook Event

1. Add event type to `parse_event()` in `crates/cyril-core/src/hooks/config.rs`
2. Add hook target variant if needed
3. Call `run_before()` or `run_after()` at appropriate point
4. Update documentation

**Example:**
```rust
// In parse_event()
"beforeRead" => Some((HookTiming::Before, HookTarget::Read)),
"afterRead" => Some((HookTiming::After, HookTarget::Read)),

// In file read operation
hooks.run_before(HookTarget::Read, &context).await?;
let content = fs::read_to_string(path).await?;
let feedback = hooks.run_after(HookTarget::Read, &context).await;
```

### Adding Path Translation Support

Path translation is automatic for JSON payloads. To add support for new fields:

1. Ensure field is a string in JSON
2. Path detection is automatic (looks for drive letters or `/mnt/` prefix)
3. Translation happens recursively in `translate_paths_in_json()`

No code changes needed unless adding special path formats.

---

## Troubleshooting

### Common Issues

**Issue: Tests fail with "connection refused"**
- Cause: Agent process not starting correctly
- Solution: Check that `kiro-cli` is installed and in PATH
- Debug: Run with `RUST_LOG=debug cargo test`

**Issue: Path translation not working**
- Cause: Path doesn't match expected format
- Solution: Check `looks_like_windows_path()` and `looks_like_wsl_mount_path()`
- Debug: Add logging to `translate_paths_in_json()`

**Issue: UI not updating**
- Cause: Event not being sent or handled
- Solution: Check event flow from source to handler
- Debug: Add logging in `App::handle_event()`

**Issue: Hook not executing**
- Cause: Glob pattern not matching or hook timing wrong
- Solution: Check pattern syntax and event type
- Debug: Add logging in `HookRegistry::run_before/after()`

### Debugging Tips

**Enable debug logging:**
```bash
RUST_LOG=debug cargo run
```

**Check log file:**
```bash
tail -f cyril.log
```

**Use `dbg!()` macro:**
```rust
dbg!(&path);  // Prints path with file:line info
```

**Use `tracing` for structured logging:**
```rust
use tracing::{debug, info, warn, error};

debug!("Processing command: {}", cmd);
info!("Session created: {}", session_id);
warn!("Hook failed: {}", error);
error!("Fatal error: {}", error);
```

---

## Additional Resources

### Comprehensive Documentation

For detailed information, see the `.agents/summary/` directory:

- **index.md** - Documentation index and navigation guide
- **architecture.md** - System architecture and design patterns
- **components.md** - Detailed component documentation
- **interfaces.md** - APIs and integration points
- **data_models.md** - Data structures and models
- **workflows.md** - Process flows and sequences
- **dependencies.md** - External dependencies
- **review_notes.md** - Documentation gaps and recommendations

### External Resources

- [Kiro CLI Documentation](https://kiro.dev/docs/cli/)
- [Agent Client Protocol Specification](https://agentclientprotocol.com)
- [Ratatui Documentation](https://ratatui.rs/)
- [Tokio Documentation](https://tokio.rs/)

### Development Tools

- **LSP:** Configured in `.kiro/settings/lsp.json`
- **Skills:** 15+ development skills in `.kiro/skills/`
- **Agents:** 10+ specialized agents in `.kiro/agents/`
- **Hooks:** Git hooks for formatting in `.kiro/hooks/`

---

## Quick Reference

### Most Important Files

1. **main.rs** (245 LOC) - Entry point, CLI parsing
2. **app.rs** (459 LOC) - Main event loop
3. **commands.rs** (905 LOC) - Command system (most complex)
4. **protocol/client.rs** (358 LOC) - ACP client
5. **platform/path.rs** (306 LOC) - Path translation
6. **platform/terminal.rs** (361 LOC) - Terminal management
7. **hooks/config.rs** (452 LOC) - Hook system

### Key Concepts

- **Two-crate architecture:** UI (cyril) + Protocol (cyril-core)
- **Event-driven:** All interactions via `AppEvent`
- **Cross-platform:** Windows via WSL bridge with path translation
- **Hook system:** Extensible automation at protocol boundary
- **Streaming:** Real-time markdown rendering

### Common Commands

```bash
# Build
cargo build --release

# Run
cargo run

# Test
cargo test

# Format
cargo fmt

# Lint
cargo clippy

# Check
cargo check
```

---

**Remember:** This is an alpha project under active development. Expect changes and improvements. When in doubt, check the comprehensive documentation in `.agents/summary/` or ask the maintainers.
