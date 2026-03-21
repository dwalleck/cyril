# Generic Selection Command Support

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make all Kiro selection commands (`/agent`, `/prompts`, `/feedback`) work through a generic picker flow, replacing the model-only picker with a reusable mechanism.

**Architecture:** Replace `PickerAction::SetModel` with `PickerAction::ExecuteCommand { command: String }` that stores the command name. When the picker confirms, send `kiro.dev/commands/execute` with `{"command": "<name>", "args": {"value": "<selected>"}}`. Selection commands are no longer filtered out of `agent_commands` — they're registered alongside panel/simple commands and routed through the picker when invoked.

**Tech Stack:** Rust, ratatui (existing picker UI), serde_json, agent-client-protocol

---

### Task 1: Generalize PickerAction

**Files:**
- Modify: `crates/cyril/src/ui/picker.rs:29-32`

**Step 1: Replace `PickerAction::SetModel` with generic variant**

Change `PickerAction` from:
```rust
pub enum PickerAction {
    SetModel,
}
```

To:
```rust
pub enum PickerAction {
    /// Execute a selection command via kiro.dev/commands/execute.
    /// `command` is the command name without slash (e.g. "model", "agent").
    ExecuteCommand { command: String },
}
```

**Step 2: Verify it compiles**

Run: `cargo check 2>&1`
Expected: Compile errors in `commands.rs` where `PickerAction::SetModel` is referenced — that's expected, we fix those in Task 2.

---

### Task 2: Update handle_picker_confirm for generic execution

**Files:**
- Modify: `crates/cyril/src/commands.rs` (handle_picker_confirm and open_model_picker)

**Step 1: Update handle_picker_confirm to use ExecuteCommand**

Replace the `match state.action` block in `handle_picker_confirm` — instead of matching `SetModel` specifically, match `ExecuteCommand { command }` and use the command name generically. Keep the `set_optimistic_model` call only when command == "model".

```rust
match state.action {
    picker::PickerAction::ExecuteCommand { ref command } => {
        let session_id = match &session.id {
            Some(id) => id.clone(),
            None => {
                tracing::warn!("Picker confirmed but no active session");
                return;
            }
        };

        // Optimistic model update for /model specifically
        if command == "model" {
            session.set_optimistic_model(value.clone());
        }

        let args = serde_json::json!({ "value": value });
        let raw_params = match Self::build_execute_params(&session_id, command, args) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to build {command} command params: {e}");
                return;
            }
        };

        Self::spawn_command_execute(
            conn,
            channels,
            raw_params,
            format!("/{command} {value}"),
        );
    }
}
```

**Step 2: Update open_model_picker to use ExecuteCommand**

Change the `PickerAction::SetModel` in `open_model_picker` to:
```rust
picker::PickerAction::ExecuteCommand { command: "model".to_string() }
```

**Step 3: Verify it compiles**

Run: `cargo check 2>&1`
Expected: Clean compile (or only pre-existing warnings).

---

### Task 3: Add generic open_selection_picker method

**Files:**
- Modify: `crates/cyril/src/commands.rs`

**Step 1: Create a generic method for opening a selection picker**

Add a new method that queries `kiro.dev/commands/options` and opens the picker for any command. This generalizes what `set_model` does for the empty-model case.

```rust
/// Query options for a selection command and open the picker.
pub async fn open_selection_picker(
    session: &SessionContext,
    conn: &Rc<acp::ClientSideConnection>,
    chat: &mut chat::ChatState,
    picker: &mut Option<picker::PickerState>,
    command_name: &str,
    title: &str,
) -> Result<()> {
    let session_id = match &session.id {
        Some(id) => id.clone(),
        None => {
            chat.add_system_message("No active session.".to_string());
            return Ok(());
        }
    };

    let params = serde_json::json!({
        "command": command_name,
        "sessionId": session_id.to_string()
    });
    let raw_params = RawValue::from_string(params.to_string())
        .map_err(|e| anyhow::anyhow!("Failed to serialize params: {e}"))?;

    match conn
        .ext_method(acp::ExtRequest::new(
            "kiro.dev/commands/options",
            Arc::from(raw_params),
        ))
        .await
    {
        Ok(resp) => {
            Self::open_picker_from_response(
                chat,
                picker,
                resp.0.get(),
                command_name,
                title,
            );
        }
        Err(e) => {
            chat.add_system_message(format!("Failed to query {command_name} options: {e}"));
        }
    }
    Ok(())
}
```

**Step 2: Extract open_picker_from_response from open_model_picker**

Rename `open_model_picker` to `open_picker_from_response` and add `command_name` and `title` parameters. Use `command_name` for the `PickerAction::ExecuteCommand`.

```rust
pub fn open_picker_from_response(
    chat: &mut chat::ChatState,
    picker: &mut Option<picker::PickerState>,
    raw_json: &str,
    command_name: &str,
    title: &str,
) {
    // ... existing parsing logic ...
    // Change PickerAction to:
    *picker = Some(picker::PickerState::new(
        title,
        picker_options,
        picker::PickerAction::ExecuteCommand {
            command: command_name.to_string(),
        },
    ));
}
```

**Step 3: Simplify set_model to use the generic method**

Replace the empty-model branch in `set_model` with:
```rust
if model_id.is_empty() {
    Self::open_selection_picker(session, conn, chat, picker, "model", "Select Model").await?;
    return Ok(());
}
```

**Step 4: Verify it compiles and tests pass**

Run: `cargo test 2>&1`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add crates/cyril/src/ui/picker.rs crates/cyril/src/commands.rs
git commit -m "refactor: generalize picker to support any selection command"
```

---

### Task 4: Register selection commands and route through picker

**Files:**
- Modify: `crates/cyril/src/app.rs:397-415` (handle_extension_event)
- Modify: `crates/cyril/src/commands.rs` (AgentCommand, parse_command, execute)

**Step 1: Add options_method field to AgentCommand**

Add an `options_method` field to `AgentCommand` to flag commands that need the picker flow:

```rust
pub struct AgentCommand {
    pub name: String,
    pub description: String,
    pub input_hint: Option<String>,
    /// If set, this command uses a selection picker (e.g. "model", "agent").
    pub is_selection: bool,
}
```

**Step 2: Update handle_extension_event to include selection commands**

In `app.rs`, remove `is_executable()` filter (which excluded selection commands) and instead include all non-local commands. Carry the `is_selection` flag from the metadata:

```rust
ExtensionEvent::KiroCommandsAvailable { commands: kiro_cmds } => {
    const LOCAL_COMMANDS: &[&str] = &["/clear", "/help", "/quit", "/load", "/new", "/model"];
    self.input.agent_commands = kiro_cmds
        .into_iter()
        .filter(|cmd| {
            let is_local = cmd.meta.as_ref().is_some_and(|m| m.local);
            !is_local && !LOCAL_COMMANDS.contains(&cmd.name.as_str())
        })
        .map(|cmd| {
            let is_selection = cmd.meta.as_ref()
                .is_some_and(|m| m.input_type.as_deref() == Some("selection"));
            let name = cmd.name.strip_prefix('/').unwrap_or(&cmd.name).to_string();
            commands::AgentCommand {
                name,
                description: cmd.description,
                input_hint: cmd.input_hint,
                is_selection,
            }
        })
        .collect();
}
```

Note: `/agent` is removed from `LOCAL_COMMANDS` since it should flow through the picker now.

**Step 3: Route selection commands through picker in execute()**

Update the `ParsedCommand::Agent` handler in `execute()` to check `is_selection`. If the command is a selection type and no argument was provided, open the picker instead of executing directly:

```rust
ParsedCommand::Agent { name, input: arg } => {
    let is_selection = agent_commands.iter()
        .any(|ac| ac.name == name && ac.is_selection);

    if is_selection && arg.is_none() {
        // Selection command with no value — open picker
        let title = format!("Select {}", name);
        Self::open_selection_picker(
            session, conn, chat, picker, &name, &title,
        ).await?;
    } else {
        // Regular execute (simple, panel, or selection with value)
        let command = if let Some(input_text) = arg {
            format!("/{name} {input_text}")
        } else {
            format!("/{name}")
        };
        Self::execute_agent_command(
            session, conn, chat, toolbar, channels, &command,
        ).await?;
    }
    Ok(CommandResult::Continue)
}
```

**Step 4: Verify it compiles and tests pass**

Run: `cargo test 2>&1`
Expected: All tests pass. Fix any test that constructs `AgentCommand` without `is_selection`.

**Step 5: Commit**

```bash
git add crates/cyril/src/app.rs crates/cyril/src/commands.rs
git commit -m "feat: route selection commands through generic picker"
```

---

### Task 5: Update test harness and run integration test

**Files:**
- Modify: `crates/cyril/examples/test_acp.rs`

**Step 1: Add /agent options + execute test to harness**

Add a test that queries agent options and executes a selection:

```rust
println!("[6] Testing kiro.dev/commands/options (agent)...");
{
    let params = serde_json::json!({
        "command": "agent",
        "sessionId": session_id.to_string()
    });
    let raw_params = serde_json::value::RawValue::from_string(params.to_string())
        .expect("valid json");

    match conn
        .ext_method(acp::ExtRequest::new(
            "kiro.dev/commands/options",
            std::sync::Arc::from(raw_params),
        ))
        .await
    {
        Ok(resp) => {
            println!("    Raw response: {}", resp.0);
        }
        Err(e) => {
            println!("    FAILED: {e}");
        }
    }
}
```

**Step 2: Run the harness**

Run: `cargo run --example test_acp 2>/dev/null`
Expected: Agent options returned, no crashes.

**Step 3: Commit**

```bash
git add crates/cyril/examples/test_acp.rs
git commit -m "test: add agent selection command test to harness"
```
