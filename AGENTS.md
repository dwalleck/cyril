# AGENTS.md — Cyril

> Cross-platform TUI client for [Kiro CLI](https://kiro.dev) via the [Agent Client Protocol (ACP)](https://agentclientprotocol.com). Alpha status.

<!-- metadata: auto-generated 2026-04-11, see .agents/summary/ for detailed docs -->

## Workspace Layout

Four-crate Rust workspace (Edition 2024, Rust ≥1.94.0):

```
crates/
  cyril/          # Binary — event loop, terminal I/O, rendering orchestration
    src/
      main.rs       # CLI parsing (clap), bridge spawn, tokio runtime
      app.rs        # Event loop (tokio::select!), notification routing, command dispatch
    tests/
      event_routing.rs  # Integration tests for notification routing
    examples/
      test_bridge.rs    # Bridge testing utility

  cyril-core/     # Library — protocol, types, commands, session management
    src/
      protocol/
        bridge.rs     # BridgeHandle/BridgeSender channel pair, spawn_bridge(), bridge loop
        client.rs     # KiroClient — implements acp::Client trait (!Send, bridge thread)
        convert.rs    # Notification conversion layer (largest file — ACP → typed Notification)
        transport.rs  # AgentProcess::spawn() — launches kiro-cli acp subprocess
      commands/
        mod.rs        # CommandRegistry, Command trait, CommandContext, CommandResult
        builtin.rs    # help, clear, quit, new, load
        subagent.rs   # spawn, kill, msg, sessions
      types/          # Domain types: event, tool_call, session, subagent, command, config, etc.
      session.rs      # SessionController — session metadata state machine
      subagent.rs     # SubagentTracker — tracks subagent roster from list_update notifications
      error.rs        # Error + ErrorKind enum
      platform/
        path.rs       # Windows ↔ WSL path translation (C:\ ↔ /mnt/c/)

  cyril-ui/       # Library — UI state, rendering, widgets
    src/
      state.rs        # UiState — central state machine, implements TuiState trait
      traits.rs       # TuiState (read-only renderer trait), Activity, ChatMessage, overlay types
      render.rs       # Frame layout (panic-safe), widget orchestration
      subagent_ui.rs  # SubagentUiState — per-subagent message streams, drill-in focus
      stream_buffer.rs # Semantic-boundary streaming buffer
      file_completer.rs # @-file autocomplete (async, .gitignore-aware)
      highlight.rs    # Syntect-based syntax highlighting with LRU cache
      cache.rs        # Generic LRU cache
      widgets/
        chat.rs       # Message display, tool call diffs, subagent drill-in
        markdown.rs   # pulldown-cmark → ratatui spans with syntax highlighting
        input.rs      # Multi-line input with cursor + autocomplete overlay
        toolbar.rs    # Top bar (session/mode/model) + bottom status bar (context/activity)
        crew_panel.rs # Subagent status panel (max 6 rows + overflow)
        hooks_panel.rs # Hooks overlay popup (three-column table)
        picker.rs     # Fuzzy-filtered selection list (nucleo-matcher)
        approval.rs   # Permission approval dialog

  cyril-voice/    # Library — voice input (speech-to-text) engine
    src/
      lib.rs        # STT engine entry point
```

## Architecture

### Bridge Pattern

The bridge connects the App to `kiro-cli acp` via three async channels:

- **App → Bridge:** `BridgeCommand` (mpsc, cap 32) — prompts, session control, agent commands
- **Bridge → App:** `RoutedNotification` (mpsc, cap 256) — agent output, tool calls, metadata
- **Bridge → App:** `PermissionRequest` (mpsc, cap 16) — approval dialogs (oneshot response)

`BridgeHandle.split()` yields a cloneable `BridgeSender` + two receivers for `tokio::select!`.

### Notification Routing

Every `RoutedNotification` carries `Option<SessionId>`:
- `None` → global (bridge lifecycle, subagent list updates) → main pipeline
- `Some(id)` matching main session → main state machines
- `Some(id)` not matching → `SubagentUiState` (subagent stream)

### State / Renderer Separation

`UiState` implements `TuiState` (read-only trait). The renderer receives `&dyn TuiState` and cannot mutate state. Mutations happen only in the App event loop.

### Command Registry

Commands implement `Command` trait (`name`, `description`, `execute`). `CommandRegistry` stores `Arc<dyn Command>`, supports:
- Builtin commands registered at startup
- Agent commands dynamically registered from server-advertised `CommandsUpdated`
- Subagent commands (`spawn`, `kill`, `msg`, `sessions`)
- Alias resolution and deduplication

`CommandResult` variants: `SystemMessage`, `ShowPicker`, `Dispatched`, `Quit`, `NotACommand`.

### Event Loop Priority (biased `tokio::select!`)

1. Terminal input (keyboard/mouse)
2. Permission requests
3. Notifications
4. Redraw timer (adaptive: 33ms streaming → 500ms idle)

## Rust Engineering Rules

These are the non-negotiable conventions for this workspace. They take precedence over generic Rust habits.

### Build, Test & Verify (run before declaring work done)
A green build is necessary but not sufficient — run the full gate, not just `cargo build`:

```bash
cargo fmt --all                              # format (edition 2024)
cargo clippy --workspace --all-targets -- -D warnings   # lints must be clean
cargo test --workspace                       # unit + integration tests
```

- Lints are CI-grade discipline even though no CI workflow exists yet: treat any clippy warning as a failure.
- `unwrap_used = "deny"` and `expect_used = "warn"` are enforced workspace-wide (`Cargo.toml [workspace.lints]`). A clippy run surfaces violations.
- When `cargo clippy --fix` rewrites files via the shell, formatting is reapplied by `.claude/hooks/cargo-fmt-after-clippy.sh` — do not hand-format around it.

### Error Handling
- `cyril-core` and `cyril-ui` each define their own `Error`/`ErrorKind` enums via `thiserror`. The binary crate uses `Box<dyn Error>` (and `anyhow`) only at the top level.
- **Map external errors at the boundary.** Errors from `agent-client-protocol`, `serde_json`, `toml`, etc. get translated into a typed `ErrorKind` variant inside the module that calls them — they must not leak into a crate's public API.
- Preserve the source chain: put inner errors in `#[source]` / `#[from]` fields rather than formatting them into a `String` early. Only flatten to a string at the outermost boundary where the chain is logged.
- Prefer dedicated enum variants over `reason: String` sentinels when a caller might branch on the cause.

### No `.unwrap()` / `.expect()` in production code (tests are exempt)
- `unwrap_used = "deny"` makes `.unwrap()` a hard error outside tests. Handle the `None`/`Err` case with `?`, `match`, or a typed error.
- `expect_used` is `warn` only because tests legitimately use `.expect()`. Do not introduce `.expect()` in non-test code.
- Do not silence these with `#[allow(...)]` at the call site — fix the code. If a lint is genuinely wrong, change it in `[workspace.lints]` with a comment, not inline.

### Parse, don't validate, at deserialization boundaries
- Untrusted input — most importantly the raw ACP protocol messages decoded in `convert.rs` — should be turned into typed domain values as early as possible, not passed around as loose strings/`Value`s and re-checked later.
- A free `validate_x(&T)` that nothing constructs is usually a missed newtype. Wrap the invariant in a type with a fallible constructor.

### Exhaustive matching over `_ =>` for protocol and error enums
- When converting ACP messages (`convert.rs`) or projecting one error/event enum into another, match every variant explicitly. Avoid `_ => ...` catch-alls.
- This makes a new ACP protocol variant a compile error that forces a conscious decision, instead of silently falling through. `convert.rs` is the file most likely to drift as `agent-client-protocol` is upgraded — exhaustiveness is the guardrail.

## Repo-Specific Patterns

### ACP Client (`!Send`)
`KiroClient` implements `acp::Client` with `async_trait(?Send)` because it uses `RefCell<HashMap>` for tool call input caching. Lives exclusively in the bridge thread.

### Notification Conversion (`convert.rs`)
The largest file. Translates raw ACP protocol messages → typed `Notification` variants. Maintains a `tool_call_inputs` cache because permission requests arrive without `raw_input`. Most likely file to need changes when the ACP protocol evolves.

### Streaming Buffer
`StreamBuffer` flushes at semantic boundaries (newlines, code fences) or after a configurable timeout (default 150ms). Prevents partial-line rendering during streaming.

### Panic-Safe Rendering
`render::draw()` wraps the inner draw in `catch_unwind`. On panic, renders a fallback "Render error" message instead of crashing.

## Config & Tooling

### User Config
`~/.config/cyril/config.toml` (TOML). Falls back to defaults if missing/invalid.

Key options: `ui.max_messages` (500), `ui.stream_buffer_timeout_ms` (150), `ui.mouse_capture` (true), `agent.agent_name` ("kiro-cli").

### Git Hooks
`.claude/hooks/rustfmt.sh` — runs `rustfmt --edition 2024` on staged `.rs` files before commit.

### Logging
JSON-structured logs to `~/.config/cyril/cyril.log` via `tracing-subscriber`. Enable debug: `RUST_LOG=debug cargo run`.

### Testing
431 test functions. Unit tests in-file (`#[cfg(test)]`), integration tests in `tests/`. Uses `rstest` for fixtures, `insta` for snapshots, `tempfile` for temp dirs. `MockTuiState` in `traits.rs` for widget testing.

### Key Dependencies
- `agent-client-protocol` 0.10 — ACP trait + transport (critical, version-sensitive)
- `ratatui` 0.30 — TUI framework (uses `unstable-rendered-line-info`)
- `crossterm` 0.29 — terminal I/O (event-stream feature)
- `syntect` 5 — syntax highlighting
- `pulldown-cmark` 0.13 — markdown parsing
- `similar` 2 — diff computation for tool call content
- `nucleo-matcher` 0.3 — fuzzy matching for picker

## Detailed Documentation

See `.agents/summary/index.md` for the full documentation index with query routing guidance.

## Custom Instructions
<!-- This section is for human and agent-maintained operational knowledge.
     Add repo-specific conventions, gotchas, and workflow rules here.
     This section is preserved exactly as-is when re-running codebase-summary. -->
