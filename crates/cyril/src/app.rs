use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers};
use futures_util::{FutureExt, StreamExt};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;

use cyril_core::commands::{CommandContext, CommandRegistry, CommandResult, CommandResultKind};
use cyril_core::protocol::bridge::{BridgeHandle, BridgeSender};
use cyril_core::session::SessionController;
use cyril_core::types::*;
use cyril_ui::state::{AutocompleteAction, UiState};
use cyril_ui::traits::{Activity, TuiState};

pub struct App {
    bridge_sender: BridgeSender,
    notification_rx: mpsc::Receiver<Notification>,
    permission_rx: mpsc::Receiver<PermissionRequest>,
    ui_state: UiState,
    session: SessionController,
    commands: CommandRegistry,
    redraw_needed: bool,
    last_activity: Instant,
}

impl App {
    pub fn new(bridge: BridgeHandle, max_messages: usize) -> Self {
        let (bridge_sender, notification_rx, permission_rx) = bridge.split();
        let commands = CommandRegistry::with_builtins();
        let names: Vec<String> = commands
            .all_commands()
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        let mut ui_state = UiState::new(max_messages);
        ui_state.set_command_names(names);
        Self {
            bridge_sender,
            notification_rx,
            permission_rx,
            ui_state,
            session: SessionController::new(),
            commands,
            redraw_needed: true,
            last_activity: Instant::now(),
        }
    }

    pub async fn create_initial_session(&mut self, cwd: PathBuf) {
        self.ui_state
            .add_system_message("Connecting to agent...".into());

        // Load file completer for @-file autocomplete
        let completer = cyril_ui::file_completer::FileCompleter::load(&cwd).await;
        self.ui_state.set_file_completer(completer);

        if let Err(e) = self
            .bridge_sender
            .send(BridgeCommand::NewSession { cwd })
            .await
        {
            self.ui_state
                .add_system_message(format!("Failed to create session: {e}"));
        }
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> cyril_core::Result<()> {
        let mut event_stream = EventStream::new();
        let mut redraw_interval =
            tokio::time::interval(Self::redraw_duration(Activity::Idle));
        redraw_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // Initial draw
        terminal
            .draw(|frame| cyril_ui::render::draw(frame, &self.ui_state))
            .map_err(|e| {
                cyril_core::Error::with_source(
                    cyril_core::ErrorKind::Transport {
                        detail: "initial draw failed".into(),
                    },
                    e,
                )
            })?;

        loop {
            tokio::select! {
                biased;

                // Priority 1: Terminal input
                Some(event) = event_stream.next() => {
                    match event {
                        Ok(event) => self.handle_terminal_event(event).await?,
                        Err(e) => {
                            tracing::error!(error = %e, "terminal event error");
                        }
                    }
                    // Drain remaining buffered input
                    while let Some(Ok(event)) = event_stream.next().now_or_never().flatten() {
                        self.handle_terminal_event(event).await?;
                    }
                }

                // Priority 2: Notifications from bridge
                Some(notification) = self.notification_rx.recv() => {
                    self.handle_notification(notification);
                }

                // Priority 3: Permission requests from bridge
                Some(request) = self.permission_rx.recv() => {
                    self.ui_state.show_approval(request);
                    self.redraw_needed = true;
                }

                // Priority 4: Redraw tick
                _ = redraw_interval.tick() => {
                    // Flush stream buffer on tick
                    if self.ui_state.flush_stream_buffer() {
                        self.redraw_needed = true;
                    }

                    // Deep idle detection
                    if self.last_activity.elapsed() > Duration::from_secs(30) {
                        self.ui_state.set_deep_idle(true);
                    }
                }
            }

            // Adaptive frame rate
            let new_duration = Self::redraw_duration(self.ui_state.activity());
            redraw_interval = tokio::time::interval(new_duration);
            redraw_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            // Conditional redraw
            if self.redraw_needed {
                terminal
                    .draw(|frame| cyril_ui::render::draw(frame, &self.ui_state))
                    .map_err(|e| {
                        cyril_core::Error::with_source(
                            cyril_core::ErrorKind::Transport {
                                detail: "draw failed".into(),
                            },
                            e,
                        )
                    })?;
                self.redraw_needed = false;
            }

            if self.ui_state.should_quit() {
                if let Err(e) = self.bridge_sender.send(BridgeCommand::Shutdown).await {
                    tracing::warn!(error = %e, "failed to send shutdown to bridge");
                }
                break;
            }
        }

        Ok(())
    }

    fn redraw_duration(activity: Activity) -> Duration {
        match activity {
            Activity::Streaming | Activity::ToolRunning => Duration::from_millis(50),
            Activity::Waiting | Activity::Sending => Duration::from_millis(100),
            Activity::Ready => Duration::from_millis(250),
            Activity::Idle => Duration::from_secs(1),
        }
    }

    fn handle_notification(&mut self, notification: Notification) {
        let session_changed = self.session.apply_notification(&notification);
        let ui_changed = self.ui_state.apply_notification(&notification);

        // Register agent commands when they arrive
        if let Notification::CommandsUpdated {
            commands: ref cmds,
            prompts: ref prompt_list,
        } = notification
        {
            self.commands.register_agent_commands(cmds);
            // Update autocomplete with all command names and prompt names
            let mut names: Vec<String> = self
                .commands
                .all_commands()
                .iter()
                .map(|cmd| cmd.name().to_string())
                .collect();
            for prompt in prompt_list {
                names.push(prompt.name().to_string());
            }
            self.ui_state.set_command_names(names);
        }

        // Handle clear command result
        if let Notification::AgentMessage(ref msg) = notification
            && !msg.is_streaming
            && msg.text == "__clear__"
        {
            self.ui_state.clear_messages();
        }

        // Handle command options received — open picker or show message
        if let Notification::CommandOptionsReceived { ref command, ref options } = notification {
            if options.is_empty() {
                self.ui_state.add_system_message(
                    format!("No {command} options available."),
                );
            } else {
                self.ui_state.show_picker(command.clone(), options.clone());
            }
            self.redraw_needed = true;
        }

        // Handle MCP OAuth request — display URL for the user to copy
        if let Notification::McpOAuthRequest {
            ref server_name,
            ref url,
        } = notification
        {
            self.ui_state.add_system_message(
                format!("MCP server '{server_name}' requires authentication. Open in browser: {url}"),
            );
            self.redraw_needed = true;
        }

        // Handle command execution response
        if let Notification::CommandExecuted { ref command, ref response } = notification {
            let text = format_command_response(command, response);
            self.ui_state
                .add_command_output(command.clone(), text);

            // WORKAROUND(Kiro v1.28.0): Kiro doesn't send ConfigOptionUpdate for
            // model changes (QRK-004), so we extract the model from the /model
            // command response. When Kiro sends proper ConfigOptionUpdate
            // notifications, this block becomes dead code — remove it and rely
            // on the ConfigOptionsUpdated handler in UiState.apply_notification().
            if command == "model" {
                if let Some(model_id) = response
                    .get("data")
                    .and_then(|d| d.get("model"))
                    .and_then(|m| m.get("id"))
                    .and_then(|id| id.as_str())
                {
                    self.ui_state.set_current_model(Some(model_id.to_string()));
                }
            }

            self.redraw_needed = true;
        }

        self.redraw_needed = self.redraw_needed || session_changed || ui_changed;
    }

    async fn handle_terminal_event(&mut self, event: Event) -> cyril_core::Result<()> {
        match event {
            Event::Key(key) => self.handle_key(key).await?,
            Event::Resize(w, h) => {
                self.ui_state.set_terminal_size(w, h);
                self.redraw_needed = true;
            }
            _ => {}
        }
        self.last_activity = Instant::now();
        self.ui_state.set_deep_idle(false);
        Ok(())
    }

    async fn handle_key(&mut self, key: KeyEvent) -> cyril_core::Result<()> {
        // Layer 1: Global shortcuts
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('c'))
            | (KeyModifiers::CONTROL, KeyCode::Char('q')) => {
                self.ui_state.request_quit();
                return Ok(());
            }
            (KeyModifiers::CONTROL, KeyCode::Char('m')) => {
                self.ui_state.toggle_mouse_capture();
                self.redraw_needed = true;
                return Ok(());
            }
            _ => {}
        }

        // Layer 2: Modal overlays
        if self.ui_state.has_approval() {
            self.handle_approval_key(key);
            self.redraw_needed = true;
            return Ok(());
        }
        if self.ui_state.has_picker() {
            self.handle_picker_key(key).await?;
            self.redraw_needed = true;
            return Ok(());
        }

        // Layer 3: Autocomplete (if active — consumes relevant keys)
        match self.ui_state.handle_autocomplete_key(key) {
            AutocompleteAction::Consumed | AutocompleteAction::Accepted => {
                self.redraw_needed = true;
                return Ok(());
            }
            AutocompleteAction::AcceptedAndSubmit => {
                self.submit_input().await?;
                self.redraw_needed = true;
                return Ok(());
            }
            AutocompleteAction::NotActive => {} // Fall through to Layer 4
        }

        // Layer 4: Normal input
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.submit_input().await?;
            }
            (KeyModifiers::NONE, KeyCode::Esc) => {
                if matches!(self.session.status(), SessionStatus::Busy) {
                    self.bridge_sender
                        .send(BridgeCommand::CancelRequest)
                        .await?;
                }
            }
            _ => {
                self.ui_state.handle_input_key(key);
            }
        }

        self.redraw_needed = true;
        Ok(())
    }

    fn handle_approval_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => self.ui_state.approval_select_prev(),
            KeyCode::Down => self.ui_state.approval_select_next(),
            KeyCode::Enter => self.ui_state.approval_confirm(),
            KeyCode::Esc => self.ui_state.approval_cancel(),
            _ => {}
        }
    }

    async fn handle_picker_key(&mut self, key: KeyEvent) -> cyril_core::Result<()> {
        match key.code {
            KeyCode::Up => self.ui_state.picker_select_prev(),
            KeyCode::Down => self.ui_state.picker_select_next(),
            KeyCode::Enter => {
                if let Some((command_name, value)) = self.ui_state.picker_confirm() {
                    if let Some(session_id) = self.session.id() {
                        self.bridge_sender
                            .send(BridgeCommand::ExecuteCommand {
                                command: command_name,
                                session_id: session_id.clone(),
                                args: serde_json::json!({"value": value}),
                            })
                            .await?;
                    }
                }
            }
            KeyCode::Esc => self.ui_state.picker_cancel(),
            KeyCode::Char(c) => self.ui_state.picker_type_char(c),
            KeyCode::Backspace => self.ui_state.picker_backspace(),
            _ => {}
        }
        Ok(())
    }

    async fn submit_input(&mut self) -> cyril_core::Result<()> {
        let text = self.ui_state.take_input();
        if text.is_empty() {
            return Ok(());
        }

        self.last_activity = Instant::now();

        // Try as slash command
        if let Some((cmd, args)) = self.commands.parse(&text) {
            let ctx = CommandContext {
                session: &self.session,
                bridge: &self.bridge_sender,
            };
            let args = args.to_string();
            match cmd.execute(&ctx, &args).await {
                Ok(result) => self.handle_command_result(result),
                Err(e) => {
                    self.ui_state
                        .add_system_message(format!("Command error: {e}"));
                }
            }
            return Ok(());
        }

        // Send as prompt
        let session_id = match self.session.id() {
            Some(id) => id.clone(),
            None => {
                self.ui_state.add_system_message(
                    "No active session. Use /new to create one.".into(),
                );
                return Ok(());
            }
        };

        self.ui_state.add_user_message(&text);
        self.session.set_status(SessionStatus::Busy);
        self.ui_state.set_activity(Activity::Sending);

        let mut content_blocks = vec![text.clone()];

        if let Some(completer) = self.ui_state.file_completer() {
            let root = completer.root().to_path_buf();
            let known = completer.known_files();
            for path in cyril_ui::file_completer::parse_file_references(&text, known) {
                match cyril_ui::file_completer::read_file(&root, &path) {
                    Ok(contents) => {
                        content_blocks
                            .push(format!("<file path=\"{path}\">\n{contents}\n</file>"));
                        tracing::info!("Attached @-referenced file: {path}");
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read @-referenced file {path}: {e}");
                        self.ui_state
                            .add_system_message(format!("Could not attach @{path}: {e}"));
                    }
                }
            }
        }

        self.bridge_sender
            .send(BridgeCommand::SendPrompt {
                session_id,
                content_blocks,
            })
            .await?;

        Ok(())
    }

    fn handle_command_result(&mut self, result: CommandResult) {
        match result.kind {
            CommandResultKind::SystemMessage(text) => {
                if text == "__clear__" {
                    self.ui_state.clear_messages();
                } else {
                    self.ui_state.add_system_message(text);
                }
            }
            CommandResultKind::NotACommand(_text) => {
                // Should not happen since we already checked parse()
            }
            CommandResultKind::ShowPicker { title, options } => {
                self.ui_state.show_picker(title, options);
            }
            CommandResultKind::Dispatched => {
                // Already sent via bridge
            }
            CommandResultKind::Quit => {
                self.ui_state.request_quit();
            }
        }
        self.redraw_needed = true;
    }
}

/// Format a `kiro.dev/commands/execute` response for display as a system message.
///
/// The response shape is `{"success": bool, "message": "...", "data": {...}}`.
/// This handles tools lists, context breakdowns, usage breakdowns, and generic messages
/// as a priority cascade.
fn format_command_response(command: &str, response: &serde_json::Value) -> String {
    let message = response
        .get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("");
    let data = response.get("data");

    // If there's tool data, format as a list
    if let Some(tools) = data
        .and_then(|d| d.get("tools"))
        .and_then(|t| t.as_array())
    {
        let mut out = format!("{message}\n\n");
        for tool in tools {
            let name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("?");
            let source = tool
                .get("source")
                .and_then(|s| s.as_str())
                .unwrap_or("");
            let desc = tool
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .trim();
            let source_tag = if !source.is_empty() && source != "built-in" {
                format!(" ({source})")
            } else {
                String::new()
            };
            out.push_str(&format!("  {name} — {desc}{source_tag}\n"));
        }
        return out;
    }

    // If there's a context breakdown, format it
    if let Some(breakdown) = data.and_then(|d| d.get("breakdown")) {
        let pct = data
            .and_then(|d| d.get("contextUsagePercentage"))
            .and_then(|p| p.as_f64())
            .unwrap_or(0.0);
        let model = data
            .and_then(|d| d.get("model"))
            .and_then(|m| m.as_str())
            .unwrap_or("unknown");
        let mut out = format!("Context: {pct:.1}% used (model: {model})\n\n");
        let categories = [
            ("contextFiles", "Context files"),
            ("tools", "Tools"),
            ("yourPrompts", "Your prompts"),
            ("kiroResponses", "Kiro responses"),
            ("sessionFiles", "Session files"),
        ];
        for (key, label) in &categories {
            if let Some(cat) = breakdown.get(*key) {
                let tokens = cat.get("tokens").and_then(|t| t.as_u64()).unwrap_or(0);
                let cat_pct = cat
                    .get("percent")
                    .and_then(|p| p.as_f64())
                    .unwrap_or(0.0);
                if tokens > 0 {
                    out.push_str(&format!("  {label}: {tokens} tokens ({cat_pct:.1}%)\n"));
                }
            }
        }
        return out;
    }

    // If there's usage breakdown data
    if let Some(breakdowns) = data
        .and_then(|d| d.get("usageBreakdowns"))
        .and_then(|u| u.as_array())
    {
        let plan = data
            .and_then(|d| d.get("planName"))
            .and_then(|p| p.as_str())
            .unwrap_or("Unknown");
        let mut out = format!("Plan: {plan}\n\n");
        for bd in breakdowns {
            let name = bd
                .get("displayName")
                .and_then(|n| n.as_str())
                .unwrap_or("?");
            let used = bd.get("used").and_then(|u| u.as_f64()).unwrap_or(0.0);
            let limit = bd.get("limit").and_then(|l| l.as_f64()).unwrap_or(0.0);
            let pct = bd
                .get("percentage")
                .and_then(|p| p.as_u64())
                .unwrap_or(0);
            out.push_str(&format!("  {name}: {used:.0} / {limit:.0} ({pct}%)\n"));
        }
        return out;
    }

    // For well-formatted messages, just use them
    if !message.is_empty() {
        return message.to_string();
    }

    // Fallback
    let success = response
        .get("success")
        .and_then(|s| s.as_bool())
        .unwrap_or(true);
    if success {
        format!("/{command}: done.")
    } else {
        format!("/{command}: command failed.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_response_tools_list() {
        let response = serde_json::json!({
            "success": true,
            "message": "Available tools:",
            "data": {
                "tools": [
                    {"name": "read", "description": "Read a file\nMore details", "source": "built-in"},
                    {"name": "fetch", "description": "Fetch a URL", "source": "mcp-server"}
                ]
            }
        });
        let result = format_command_response("tools", &response);
        assert!(result.contains("Available tools:"));
        assert!(result.contains("  read — Read a file\n"));
        assert!(result.contains("  fetch — Fetch a URL (mcp-server)\n"));
    }

    #[test]
    fn format_response_context_breakdown() {
        let response = serde_json::json!({
            "success": true,
            "message": "",
            "data": {
                "contextUsagePercentage": 42.5,
                "model": "claude-sonnet",
                "breakdown": {
                    "contextFiles": {"tokens": 1000, "percent": 10.0},
                    "tools": {"tokens": 500, "percent": 5.0},
                    "yourPrompts": {"tokens": 2000, "percent": 20.0},
                    "kiroResponses": {"tokens": 0, "percent": 0.0}
                }
            }
        });
        let result = format_command_response("context", &response);
        assert!(result.contains("Context: 42.5% used (model: claude-sonnet)"));
        assert!(result.contains("Context files: 1000 tokens (10.0%)"));
        assert!(result.contains("Tools: 500 tokens (5.0%)"));
        assert!(result.contains("Your prompts: 2000 tokens (20.0%)"));
        // Zero-token categories should be omitted
        assert!(!result.contains("Kiro responses"));
    }

    #[test]
    fn format_response_usage_breakdowns() {
        let response = serde_json::json!({
            "success": true,
            "message": "",
            "data": {
                "planName": "Pro",
                "usageBreakdowns": [
                    {"displayName": "Fast requests", "used": 150.0, "limit": 500.0, "percentage": 30}
                ]
            }
        });
        let result = format_command_response("usage", &response);
        assert!(result.contains("Plan: Pro"));
        assert!(result.contains("Fast requests: 150 / 500 (30%)"));
    }

    #[test]
    fn format_response_plain_message() {
        let response = serde_json::json!({
            "success": true,
            "message": "Context compacted successfully."
        });
        let result = format_command_response("compact", &response);
        assert_eq!(result, "Context compacted successfully.");
    }

    #[test]
    fn format_response_success_fallback() {
        let response = serde_json::json!({"success": true});
        let result = format_command_response("compact", &response);
        assert_eq!(result, "/compact: done.");
    }

    #[test]
    fn format_response_failure_fallback() {
        let response = serde_json::json!({"success": false});
        let result = format_command_response("compact", &response);
        assert_eq!(result, "/compact: command failed.");
    }

    #[test]
    fn format_response_null_data() {
        let response = serde_json::Value::Null;
        let result = format_command_response("test", &response);
        assert_eq!(result, "/test: done.");
    }

    #[test]
    fn format_response_tools_builtin_source_omitted() {
        let response = serde_json::json!({
            "success": true,
            "message": "Tools:",
            "data": {
                "tools": [
                    {"name": "read", "description": "Read a file", "source": "built-in"}
                ]
            }
        });
        let result = format_command_response("tools", &response);
        // built-in source tag should NOT appear
        assert!(!result.contains("(built-in)"));
        assert!(result.contains("  read — Read a file\n"));
    }
}
