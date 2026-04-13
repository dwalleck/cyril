use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use futures_util::{FutureExt, StreamExt};
use ratatui::DefaultTerminal;
use serde::Deserialize;
use tokio::sync::mpsc;

use cyril_core::commands::{CommandContext, CommandRegistry, CommandResult, CommandResultKind};
use cyril_core::protocol::bridge::{BridgeHandle, BridgeSender};
use cyril_core::session::SessionController;
use cyril_core::types::*;
use cyril_ui::state::{AutocompleteAction, UiState};
use cyril_ui::traits::{Activity, TuiState};

use cyril_core::types::code_panel::CodeCommandResponse;

/// Lines per mouse wheel tick (finer-grained than keyboard half-page scroll).
const MOUSE_SCROLL_LINES: usize = 3;

pub struct App {
    bridge_sender: BridgeSender,
    notification_rx: mpsc::Receiver<RoutedNotification>,
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
        let info: Vec<(String, Option<String>)> = commands
            .all_commands()
            .iter()
            .map(|c| {
                let desc = c.description();
                (c.name().to_string(), Some(desc.to_string()).filter(|s| !s.is_empty()))
            })
            .collect();
        let mut ui_state = UiState::new(max_messages);
        ui_state.set_command_info(info);
        // main.rs enables mouse capture before the event loop, so sync the
        // initial state to avoid an inverted Ctrl+M toggle.
        ui_state.set_mouse_captured(true);
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
                    if let Some(deferred) = self.handle_notification(notification) {
                        match self.bridge_sender.send(deferred).await {
                            Ok(()) => {
                                // Commit session state after successful send
                                self.session.set_status(SessionStatus::Busy);
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "failed to send deferred bridge command");
                                self.ui_state.set_activity(Activity::Idle);
                                self.ui_state.add_system_message(
                                    "Failed to send /code prompt to agent.".into(),
                                );
                            }
                        }
                    }
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

                    // During busy states, redraw every tick so the activity
                    // spinner animates and the elapsed timer increments.
                    if !matches!(
                        self.ui_state.activity(),
                        Activity::Idle | Activity::Ready
                    ) {
                        self.redraw_needed = true;
                    }

                    // Deep idle detection
                    if self.last_activity.elapsed() > Duration::from_secs(30) {
                        self.ui_state.set_deep_idle(true);
                    }
                }
            }

            // Adaptive frame rate — account for subagent activity as well as main session.
            let effective_activity = if self.ui_state.any_subagent_active() {
                Activity::Streaming
            } else {
                self.ui_state.activity()
            };
            let new_duration = Self::redraw_duration(effective_activity);
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

    fn handle_notification(&mut self, routed: RoutedNotification) -> Option<BridgeCommand> {
        let RoutedNotification {
            session_id,
            notification,
        } = routed;

        // Tracker-level notifications (list_update, inbox) are global:
        // apply them regardless of session_id. Returns false for unrelated variants.
        let tracker_changed = self
            .ui_state
            .apply_subagent_tracker_notification(&notification);

        // SubagentListUpdated also informs SubagentUiState so it can mark terminated streams.
        if let Notification::SubagentListUpdated { ref subagents, .. } = notification {
            self.ui_state.apply_subagent_list_update(subagents);
            self.redraw_needed = true;
        }

        // Route session-scoped notifications: if the source session_id is
        // a known subagent, route to SubagentUiState and return early.
        // If session_id is None or matches the main session, fall through.
        if let Some(ref sid) = session_id {
            let is_main = self.session.id().map(|m| m == sid).unwrap_or(false);
            if !is_main && self.ui_state.subagent_tracker().is_subagent(sid) {
                self.ui_state.apply_subagent_notification(sid, &notification);
                self.redraw_needed = true;
                return None;
            }
            if !is_main && self.session.id().is_some() {
                // Session ID doesn't match main and isn't a known subagent.
                // This can happen if a subagent notification arrives before
                // the corresponding SubagentListUpdated. Route it optimistically —
                // the stream will be created on first contact.
                tracing::debug!(
                    session_id = sid.as_str(),
                    "notification for unknown session, routing to subagent stream"
                );
                self.ui_state.apply_subagent_notification(sid, &notification);
                self.redraw_needed = true;
                return None;
            }
        }

        let session_changed = self.session.apply_notification(&notification);
        let ui_changed = self.ui_state.apply_notification(&notification);

        // Register agent commands when they arrive
        if let Notification::CommandsUpdated {
            commands: ref cmds,
            prompts: ref prompt_list,
        } = notification
        {
            self.commands.register_agent_commands(cmds);
            // Update autocomplete with all command info (name + description)
            let mut info: Vec<(String, Option<String>)> = self
                .commands
                .all_commands()
                .iter()
                .map(|cmd| {
                    let desc = cmd.description();
                    (cmd.name().to_string(), Some(desc.to_string()).filter(|s| !s.is_empty()))
                })
                .collect();
            for prompt in prompt_list {
                info.push((
                    prompt.name().to_string(),
                    prompt.description().map(str::to_string).filter(|s| !s.is_empty()),
                ));
            }
            self.ui_state.set_command_info(info);

            // Optimistic code intelligence detection: if .kiro/settings/lsp.json
            // exists in the working directory, assume code intelligence is active
            // until the first /code response confirms or denies it.
            if std::path::Path::new(".kiro/settings/lsp.json").exists() {
                self.ui_state.set_code_intelligence_active(true);
            }
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

        // Handle command execution response. The `hooks` and `code` commands
        // are special-cased; all other commands fall through to the generic
        // command-output path. See `dispatch_command_executed` for the rules.
        let mut deferred_command = None;
        if let Notification::CommandExecuted { ref command, ref response } = notification {
            if command == "code" {
                deferred_command =
                    dispatch_code_command(response, &self.session, &mut self.ui_state);
            } else {
                dispatch_command_executed(command, response, &mut self.ui_state);

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
            }

            self.redraw_needed = true;
        }

        self.redraw_needed =
            self.redraw_needed || session_changed || ui_changed || tracker_changed;
        deferred_command
    }

    async fn handle_terminal_event(&mut self, event: Event) -> cyril_core::Result<()> {
        match event {
            Event::Key(key) => self.handle_key(key).await?,
            Event::Mouse(mouse) => {
                // Respect modal overlay priority — don't scroll chat when
                // an overlay is consuming input.
                if !self.ui_state.has_approval()
                    && !self.ui_state.has_picker()
                    && !self.ui_state.has_hooks_panel()
                    && !self.ui_state.has_code_panel()
                    && self.ui_state.subagent_ui().focused_session_id().is_none()
                {
                    // Mouse wheel uses a fixed 3-line step; keyboard
                    // PgUp/PgDn uses half-page for coarser navigation.
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            self.ui_state.chat_scroll_up(MOUSE_SCROLL_LINES);
                            self.redraw_needed = true;
                        }
                        MouseEventKind::ScrollDown => {
                            self.ui_state.chat_scroll_down(MOUSE_SCROLL_LINES);
                            self.redraw_needed = true;
                        }
                        _ => {}
                    }
                }
            }
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
                let result = if self.ui_state.mouse_captured() {
                    crossterm::execute!(
                        std::io::stdout(),
                        crossterm::event::EnableMouseCapture,
                    )
                } else {
                    crossterm::execute!(
                        std::io::stdout(),
                        crossterm::event::DisableMouseCapture,
                    )
                };
                if let Err(e) = result {
                    tracing::warn!(error = %e, "failed to toggle mouse capture");
                    self.ui_state.toggle_mouse_capture(); // roll back
                }
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
        if self.ui_state.has_hooks_panel() {
            self.handle_hooks_panel_key(key);
            self.redraw_needed = true;
            return Ok(());
        }
        if self.ui_state.has_code_panel() {
            self.handle_code_panel_key(key).await?;
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
                // If drilled into a subagent stream, Esc exits the drill-in first.
                if self.ui_state.subagent_ui().focused_session_id().is_some() {
                    self.ui_state.unfocus_subagent();
                } else if matches!(self.session.status(), SessionStatus::Busy) {
                    self.bridge_sender
                        .send(BridgeCommand::CancelRequest)
                        .await?;
                }
            }
            _ => {
                // Only scroll the main chat when not drilled into a subagent.
                let scroll_consumed =
                    self.ui_state.subagent_ui().focused_session_id().is_none()
                        && dispatch_chat_scroll_key(key, &mut self.ui_state);
                if !scroll_consumed {
                    self.ui_state.handle_input_key(key);
                }
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

    /// Handle key input while the `/hooks` panel overlay is visible.
    /// Esc closes; Up/Down and PgUp/PgDn scroll.
    fn handle_hooks_panel_key(&mut self, key: KeyEvent) {
        dispatch_hooks_panel_key(key, &mut self.ui_state);
    }

    /// Handle key input while the `/code` panel overlay is visible.
    /// Esc closes; `r` refreshes by re-executing the `/code` command.
    async fn handle_code_panel_key(&mut self, key: KeyEvent) -> cyril_core::Result<()> {
        match key.code {
            KeyCode::Esc => self.ui_state.close_code_panel(),
            KeyCode::Char('r') => {
                if let Some(id) = self.session.id().cloned() {
                    self.bridge_sender
                        .send(BridgeCommand::ExecuteCommand {
                            command: "code".into(),
                            session_id: id,
                            args: serde_json::json!({}),
                        })
                        .await?;
                } else {
                    tracing::debug!("code panel refresh requested but no active session");
                    self.ui_state.add_system_message(
                        "No active session — cannot refresh.".into(),
                    );
                    self.ui_state.close_code_panel();
                }
            }
            _ => {} // Consume all other keys
        }
        Ok(())
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
                subagent_tracker: Some(self.ui_state.subagent_tracker()),
            };
            let command_name = cmd.name().to_string();
            let args = args.to_string();
            match cmd.execute(&ctx, &args).await {
                Ok(result) => self.handle_command_result(result),
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        command = %command_name,
                        "slash command execution failed"
                    );
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

/// Parse a `/hooks` response body into a list of `HookInfo`.
///
/// Expects the Kiro wire shape `{data: {hooks: [{trigger, command, matcher?}, ...]}}`.
/// Deserializes the whole `data.hooks` array as a typed `Vec<HookInfo>` in one
/// shot: if any entry is structurally malformed (missing `trigger`, missing
/// `command`, wrong types), the whole response is rejected rather than
/// silently dropping individual entries.
///
/// Returns `None` on any of these conditions, so the caller falls back to
/// `format_command_response` and the user still sees the raw response
/// instead of a silently empty panel:
///
/// - `data` field absent → `debug` log
/// - `data.hooks` field absent → `debug` log
/// - structural deserialization failure → `warn` log
/// - any entry has an empty `trigger` or `command` → `warn` log (display
///   defect — would render as a blank row)
///
/// Uses `Deserialize::deserialize` directly on `&Value` (which implements
/// `Deserializer`) to avoid the deep clone of the hooks array that
/// `serde_json::from_value` would require.
fn parse_hooks_response(response: &serde_json::Value) -> Option<Vec<cyril_core::types::HookInfo>> {
    let Some(data) = response.get("data") else {
        tracing::debug!("/hooks response has no `data` field — falling back");
        return None;
    };
    let Some(hooks_value) = data.get("hooks") else {
        tracing::debug!("/hooks response has no `data.hooks` field — falling back");
        return None;
    };
    let hooks = match Vec::<cyril_core::types::HookInfo>::deserialize(hooks_value) {
        Ok(hooks) => hooks,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "malformed /hooks response, falling back to generic command output"
            );
            return None;
        }
    };
    if hooks
        .iter()
        .any(|h| h.trigger.is_empty() || h.command.is_empty())
    {
        tracing::warn!(
            "/hooks response contained a hook with an empty trigger or command — falling back"
        );
        return None;
    }
    Some(hooks)
}

/// Dispatch a `CommandExecuted` response to the UI.
///
/// For `command == "hooks"` with a successful response (`success: true` or
/// absent), parses the hooks and opens the overlay panel. For any other
/// command, or for hooks responses that are structurally invalid or report
/// `success: false`, falls through to `format_command_response` so the
/// backend's `message` field surfaces as a normal command-output line.
///
/// Extracted as a free function so it can be tested directly without
/// constructing a full `App`. Model-specific workarounds (see
/// `App::handle_notification`) stay at the caller site because they mutate
/// session state, not UI state.
fn dispatch_command_executed(
    command: &str,
    response: &serde_json::Value,
    ui_state: &mut cyril_ui::state::UiState,
) {
    let handled_as_panel = command == "hooks" && is_success_response(response) && {
        match parse_hooks_response(response) {
            Some(hooks) => {
                ui_state.show_hooks_panel(hooks);
                true
            }
            None => false,
        }
    };

    if !handled_as_panel {
        let text = format_command_response(command, response);
        ui_state.add_command_output(command.to_string(), text);
    }
}

/// Handle a `/code` command response.
///
/// If the response reports `success: false`, falls through to generic command
/// output (matching the `hooks` pattern). Otherwise routes by response shape:
/// - Panel: shows overlay and, if `Initialized`, marks code intelligence active.
/// - Prompt: validates session, pre-populates chat, sets Busy, returns a
///   deferred `SendPrompt` command (deferred because `handle_notification` is
///   sync and cannot `.await` the bridge send).
/// - Unknown: falls through to generic formatting.
fn dispatch_code_command(
    response: &serde_json::Value,
    session: &cyril_core::session::SessionController,
    ui_state: &mut cyril_ui::state::UiState,
) -> Option<BridgeCommand> {
    if !is_success_response(response) {
        let text = format_command_response("code", response);
        ui_state.add_command_output("code".to_string(), text);
        return None;
    }

    match CodeCommandResponse::from_json(response) {
        CodeCommandResponse::Panel(data) => {
            if data.status == LspStatus::Initialized {
                ui_state.set_code_intelligence_active(true);
            }
            ui_state.show_code_panel(data);
            None
        }
        CodeCommandResponse::Prompt { text, label } => {
            let session_id = match session.id().cloned() {
                Some(id) => id,
                None => {
                    tracing::warn!("/code prompt response arrived with no active session");
                    ui_state.add_system_message(
                        "/code: received prompt but no active session — try again.".into(),
                    );
                    return None;
                }
            };
            let display = label.as_deref().unwrap_or("Code Intelligence");
            ui_state.add_system_message(format!("/code: {display}"));
            ui_state.add_user_message(&text);
            ui_state.set_activity(Activity::Sending);

            Some(BridgeCommand::SendPrompt {
                session_id,
                content_blocks: vec![text],
            })
        }
        CodeCommandResponse::Unknown(ref value) => {
            let text = format_command_response("code", value);
            ui_state.add_command_output("code".to_string(), text);
            None
        }
    }
}

/// Dispatch a key press while the `/hooks` panel is visible.
///
/// Extracted as a free function so the full key-map can be unit-tested
/// without constructing an `App`. Esc hides the panel; arrow keys scroll
/// one line; page keys scroll ten lines; other keys are no-ops.
fn dispatch_hooks_panel_key(key: KeyEvent, ui_state: &mut cyril_ui::state::UiState) {
    match key.code {
        KeyCode::Esc => ui_state.hide_hooks_panel(),
        KeyCode::Up => ui_state.hooks_panel_scroll_up(1),
        KeyCode::Down => ui_state.hooks_panel_scroll_down(1),
        KeyCode::PageUp => ui_state.hooks_panel_scroll_up(10),
        KeyCode::PageDown => ui_state.hooks_panel_scroll_down(10),
        _ => {}
    }
}

/// Handle PageUp/PageDown for main chat scrolling.
/// Returns `true` if the key was consumed.
fn dispatch_chat_scroll_key(key: KeyEvent, ui_state: &mut cyril_ui::state::UiState) -> bool {
    let (_, h) = ui_state.terminal_size();
    let half_page = ((h as usize) / 2).max(1);
    match key.code {
        KeyCode::PageUp => {
            ui_state.chat_scroll_up(half_page);
            true
        }
        KeyCode::PageDown => {
            ui_state.chat_scroll_down(half_page);
            true
        }
        _ => false,
    }
}

/// Returns `true` if the response either has no `success` field (legacy or
/// optional) or has `success: true`. `success: false` reports a backend
/// error and should never be swallowed by panel-style handlers.
fn is_success_response(response: &serde_json::Value) -> bool {
    response
        .get("success")
        .and_then(|s| s.as_bool())
        .unwrap_or(true)
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

    // --- parse_hooks_response tests ---

    #[test]
    fn parse_hooks_response_well_formed() {
        let response = serde_json::json!({
            "success": true,
            "data": {
                "hooks": [
                    {"trigger": "PreToolUse", "command": "echo pre", "matcher": "read"},
                    {"trigger": "Stop", "command": "notify done"}
                ]
            }
        });
        let hooks = parse_hooks_response(&response).expect("should parse");
        assert_eq!(hooks.len(), 2);
        assert_eq!(hooks[0].trigger, "PreToolUse");
        assert_eq!(hooks[0].command, "echo pre");
        assert_eq!(hooks[0].matcher.as_deref(), Some("read"));
        assert_eq!(hooks[1].trigger, "Stop");
        assert!(hooks[1].matcher.is_none());
    }

    #[test]
    fn parse_hooks_response_empty_array() {
        let response = serde_json::json!({"data": {"hooks": []}});
        let hooks = parse_hooks_response(&response).expect("should parse");
        assert!(hooks.is_empty());
    }

    #[test]
    fn parse_hooks_response_missing_data_returns_none() {
        let response = serde_json::json!({"success": true, "message": "no data"});
        assert!(parse_hooks_response(&response).is_none());
    }

    #[test]
    fn parse_hooks_response_data_without_hooks_field() {
        let response = serde_json::json!({"data": {"other": "stuff"}});
        assert!(parse_hooks_response(&response).is_none());
    }

    #[test]
    fn parse_hooks_response_hooks_wrong_type_returns_none() {
        let response = serde_json::json!({"data": {"hooks": "not an array"}});
        assert!(parse_hooks_response(&response).is_none());
    }

    #[test]
    fn parse_hooks_response_rejects_any_malformed_entry() {
        // Fail-fast semantics: a single bad entry rejects the whole response,
        // so the caller falls through to the generic command-output path and
        // the user sees the raw JSON instead of a silently truncated panel.
        let response = serde_json::json!({
            "data": {
                "hooks": [
                    {"trigger": "PreToolUse", "command": "valid"},
                    {"not_a_hook": true},
                    {"trigger": "Stop", "command": "also valid", "matcher": "write"}
                ]
            }
        });
        assert!(parse_hooks_response(&response).is_none());
    }

    #[test]
    fn parse_hooks_response_rejects_entry_missing_required_field() {
        let response = serde_json::json!({
            "data": {
                "hooks": [
                    {"trigger": "PreToolUse"} // missing required `command`
                ]
            }
        });
        assert!(parse_hooks_response(&response).is_none());
    }

    #[test]
    fn parse_hooks_response_rejects_entry_with_empty_trigger() {
        // Structural Serde validation accepts empty strings for required
        // fields, so we guard against them explicitly to prevent the widget
        // from rendering a blank trigger column.
        let response = serde_json::json!({
            "data": {
                "hooks": [
                    {"trigger": "", "command": "echo hi"}
                ]
            }
        });
        assert!(parse_hooks_response(&response).is_none());
    }

    #[test]
    fn parse_hooks_response_rejects_entry_with_empty_command() {
        let response = serde_json::json!({
            "data": {
                "hooks": [
                    {"trigger": "PreToolUse", "command": ""}
                ]
            }
        });
        assert!(parse_hooks_response(&response).is_none());
    }

    #[test]
    fn parse_hooks_response_rejects_mixed_valid_and_empty_entries() {
        // Fail-fast: one bad entry rejects the whole response, same as
        // the malformed-entry case.
        let response = serde_json::json!({
            "data": {
                "hooks": [
                    {"trigger": "PreToolUse", "command": "valid"},
                    {"trigger": "Stop", "command": ""}
                ]
            }
        });
        assert!(parse_hooks_response(&response).is_none());
    }

    #[test]
    fn parse_hooks_response_preserves_ordering() {
        // parse_hooks_response preserves wire order; sorting happens in
        // `UiState::show_hooks_panel`, not in the parser or the widget.
        let response = serde_json::json!({
            "data": {
                "hooks": [
                    {"trigger": "Stop", "command": "z"},
                    {"trigger": "AgentSpawn", "command": "a"},
                ]
            }
        });
        let hooks = parse_hooks_response(&response).expect("should parse");
        assert_eq!(hooks[0].trigger, "Stop");
        assert_eq!(hooks[1].trigger, "AgentSpawn");
    }

    // --- is_success_response tests ---

    #[test]
    fn is_success_missing_field_defaults_true() {
        let response = serde_json::json!({"data": {"hooks": []}});
        assert!(is_success_response(&response));
    }

    #[test]
    fn is_success_explicit_true() {
        let response = serde_json::json!({"success": true, "data": {}});
        assert!(is_success_response(&response));
    }

    #[test]
    fn is_success_explicit_false() {
        let response = serde_json::json!({"success": false, "message": "oops"});
        assert!(!is_success_response(&response));
    }

    #[test]
    fn is_success_wrong_type_defaults_true() {
        // Non-bool success field is treated as missing and defaults to true.
        let response = serde_json::json!({"success": "yes", "data": {}});
        assert!(is_success_response(&response));
    }

    // --- dispatch_command_executed tests ---

    fn valid_hooks_response() -> serde_json::Value {
        serde_json::json!({
            "success": true,
            "data": {
                "hooks": [
                    {"trigger": "PreToolUse", "command": "echo pre", "matcher": "read"},
                    {"trigger": "Stop", "command": "notify done"}
                ]
            }
        })
    }

    #[test]
    fn dispatch_hooks_valid_response_opens_panel_and_adds_no_message() {
        let mut ui_state = UiState::new(500);
        let response = valid_hooks_response();

        dispatch_command_executed("hooks", &response, &mut ui_state);

        assert!(ui_state.has_hooks_panel(), "panel should be open");
        assert_eq!(
            ui_state.hooks_panel().expect("panel").hooks.len(),
            2,
            "both hooks should be parsed"
        );
        assert_eq!(
            ui_state.messages().len(),
            0,
            "no command-output message when panel handles the response"
        );
    }

    #[test]
    fn dispatch_hooks_malformed_entry_falls_through_to_message() {
        let mut ui_state = UiState::new(500);
        // Missing `command` field — whole response rejected.
        let response = serde_json::json!({
            "success": true,
            "message": "",
            "data": {
                "hooks": [
                    {"trigger": "PreToolUse"}
                ]
            }
        });

        dispatch_command_executed("hooks", &response, &mut ui_state);

        assert!(
            !ui_state.has_hooks_panel(),
            "panel should NOT open for malformed response"
        );
        assert_eq!(
            ui_state.messages().len(),
            1,
            "should fall through and add a command-output message"
        );
    }

    #[test]
    fn dispatch_hooks_success_false_surfaces_error_message() {
        let mut ui_state = UiState::new(500);
        // Backend reports an error — critical case: the previous implementation
        // opened an empty panel and discarded the `message` field, hiding the
        // error from the user.
        let response = serde_json::json!({
            "success": false,
            "message": "session expired",
            "data": {"hooks": []}
        });

        dispatch_command_executed("hooks", &response, &mut ui_state);

        assert!(
            !ui_state.has_hooks_panel(),
            "panel should NOT open when backend reports success: false"
        );
        assert_eq!(
            ui_state.messages().len(),
            1,
            "error message should be added as a command-output message"
        );
        // The error message should be visible to the user. format_command_response
        // returns the `message` field directly when no structured data shape matches.
        let msg_text = match ui_state.messages()[0].kind() {
            cyril_ui::traits::ChatMessageKind::CommandOutput { text, .. } => text.clone(),
            other => panic!("expected CommandOutput, got {other:?}"),
        };
        assert!(
            msg_text.contains("session expired"),
            "user should see the backend error message; got: {msg_text}"
        );
    }

    #[test]
    fn dispatch_hooks_missing_data_falls_through_to_message() {
        let mut ui_state = UiState::new(500);
        let response = serde_json::json!({"success": true, "message": "no hooks data"});

        dispatch_command_executed("hooks", &response, &mut ui_state);

        assert!(!ui_state.has_hooks_panel());
        assert_eq!(ui_state.messages().len(), 1);
    }

    #[test]
    fn dispatch_non_hooks_command_adds_message() {
        let mut ui_state = UiState::new(500);
        let response = serde_json::json!({
            "success": true,
            "message": "Context compacted successfully."
        });

        dispatch_command_executed("compact", &response, &mut ui_state);

        assert!(
            !ui_state.has_hooks_panel(),
            "non-hooks commands should never open the hooks panel"
        );
        assert_eq!(ui_state.messages().len(), 1);
    }

    // --- dispatch_hooks_panel_key tests ---

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn state_with_open_panel(num_hooks: usize) -> UiState {
        let mut ui_state = UiState::new(500);
        let hooks: Vec<cyril_core::types::HookInfo> = (0..num_hooks)
            .map(|i| cyril_core::types::HookInfo {
                trigger: format!("T{i}"),
                command: format!("cmd{i}"),
                matcher: None,
            })
            .collect();
        ui_state.show_hooks_panel(hooks);
        ui_state
    }

    #[test]
    fn hooks_panel_key_esc_closes_panel() {
        let mut ui_state = state_with_open_panel(3);
        dispatch_hooks_panel_key(key(KeyCode::Esc), &mut ui_state);
        assert!(!ui_state.has_hooks_panel());
    }

    #[test]
    fn hooks_panel_key_down_scrolls_down_one() {
        let mut ui_state = state_with_open_panel(5);
        dispatch_hooks_panel_key(key(KeyCode::Down), &mut ui_state);
        assert_eq!(ui_state.hooks_panel().expect("panel").scroll_offset, 1);
    }

    #[test]
    fn hooks_panel_key_up_scrolls_up_one() {
        let mut ui_state = state_with_open_panel(5);
        // Scroll down twice first to have something to scroll up from.
        dispatch_hooks_panel_key(key(KeyCode::Down), &mut ui_state);
        dispatch_hooks_panel_key(key(KeyCode::Down), &mut ui_state);
        dispatch_hooks_panel_key(key(KeyCode::Up), &mut ui_state);
        assert_eq!(ui_state.hooks_panel().expect("panel").scroll_offset, 1);
    }

    #[test]
    fn hooks_panel_key_pgdown_scrolls_down_ten() {
        let mut ui_state = state_with_open_panel(20);
        dispatch_hooks_panel_key(key(KeyCode::PageDown), &mut ui_state);
        assert_eq!(ui_state.hooks_panel().expect("panel").scroll_offset, 10);
    }

    #[test]
    fn hooks_panel_key_pgup_scrolls_up_ten() {
        let mut ui_state = state_with_open_panel(20);
        // Scroll down past 10 first.
        dispatch_hooks_panel_key(key(KeyCode::PageDown), &mut ui_state);
        dispatch_hooks_panel_key(key(KeyCode::PageDown), &mut ui_state);
        // Now at offset ~19 (clamped from 20 to len-1 = 19).
        dispatch_hooks_panel_key(key(KeyCode::PageUp), &mut ui_state);
        assert_eq!(ui_state.hooks_panel().expect("panel").scroll_offset, 9);
    }

    #[test]
    fn hooks_panel_key_unknown_is_noop() {
        let mut ui_state = state_with_open_panel(5);
        dispatch_hooks_panel_key(key(KeyCode::Char('x')), &mut ui_state);
        assert!(ui_state.has_hooks_panel(), "panel should still be open");
        assert_eq!(
            ui_state.hooks_panel().expect("panel").scroll_offset,
            0,
            "unknown key should not affect scroll"
        );
    }

    #[test]
    fn hooks_panel_key_scroll_down_on_empty_panel_is_noop() {
        // Edge case: empty panel. saturating_sub(1) on len=0 yields 0; scroll
        // must stay at 0 without panicking.
        let mut ui_state = state_with_open_panel(0);
        dispatch_hooks_panel_key(key(KeyCode::PageDown), &mut ui_state);
        assert_eq!(ui_state.hooks_panel().expect("panel").scroll_offset, 0);
    }

    // --- Chat scroll key dispatch tests ---

    #[test]
    fn chat_scroll_pageup_consumed_and_enters_browse_mode() {
        let mut ui_state = UiState::new(500);
        let consumed = dispatch_chat_scroll_key(key(KeyCode::PageUp), &mut ui_state);
        assert!(consumed, "PageUp should be consumed");
        assert!(
            ui_state.chat_scroll_back().is_some(),
            "should enter browse mode"
        );
    }

    #[test]
    fn chat_scroll_pagedown_consumed() {
        let mut ui_state = UiState::new(500);
        ui_state.chat_scroll_up(20);
        let consumed = dispatch_chat_scroll_key(key(KeyCode::PageDown), &mut ui_state);
        assert!(consumed, "PageDown should be consumed");
    }

    #[test]
    fn chat_scroll_non_scroll_key_not_consumed() {
        let mut ui_state = UiState::new(500);
        let consumed = dispatch_chat_scroll_key(key(KeyCode::Char('a')), &mut ui_state);
        assert!(!consumed, "regular key should not be consumed");
        assert!(
            ui_state.chat_scroll_back().is_none(),
            "scroll state should not change"
        );
    }

    #[test]
    fn chat_scroll_pageup_uses_half_terminal_height() {
        let mut ui_state = UiState::new(500);
        ui_state.set_terminal_size(80, 24);
        dispatch_chat_scroll_key(key(KeyCode::PageUp), &mut ui_state);
        assert_eq!(ui_state.chat_scroll_back(), Some(12));
    }

    // --- dispatch_code_command tests ---

    fn code_session() -> cyril_core::session::SessionController {
        let mut session = cyril_core::session::SessionController::new();
        session.set_session(SessionId::new("sess_1"), SessionStatus::Active);
        session
    }

    #[test]
    fn dispatch_code_panel_opens_overlay() {
        let session = code_session();
        let mut ui = UiState::new(500);
        let result = dispatch_code_command(
            &serde_json::json!({
                "success": true,
                "data": {
                    "status": "initialized",
                    "detectedLanguages": ["rust"],
                    "projectMarkers": [],
                    "lsps": []
                }
            }),
            &session,
            &mut ui,
        );
        assert!(result.is_none());
        assert!(ui.has_code_panel());
        assert!(ui.code_intelligence_active());
    }

    #[test]
    fn dispatch_code_panel_failed_does_not_set_active() {
        let session = code_session();
        let mut ui = UiState::new(500);
        dispatch_code_command(
            &serde_json::json!({
                "success": true,
                "data": {
                    "status": "failed",
                    "detectedLanguages": [],
                    "projectMarkers": [],
                    "lsps": []
                }
            }),
            &session,
            &mut ui,
        );
        assert!(ui.has_code_panel());
        assert!(!ui.code_intelligence_active());
    }

    #[test]
    fn dispatch_code_success_false_falls_through_to_message() {
        let session = code_session();
        let mut ui = UiState::new(500);
        let result = dispatch_code_command(
            &serde_json::json!({
                "success": false,
                "message": "Not configured",
                "data": {
                    "status": "initialized",
                    "lsps": []
                }
            }),
            &session,
            &mut ui,
        );
        assert!(result.is_none());
        assert!(!ui.has_code_panel(), "panel should NOT open on success:false");
        assert!(!ui.code_intelligence_active());
    }

    #[test]
    fn dispatch_code_prompt_returns_deferred_command() {
        let session = code_session();
        let mut ui = UiState::new(500);
        let result = dispatch_code_command(
            &serde_json::json!({
                "success": true,
                "data": {
                    "executePrompt": "Analyze the code...",
                    "label": "Code Summary"
                }
            }),
            &session,
            &mut ui,
        );
        assert!(matches!(result, Some(BridgeCommand::SendPrompt { .. })));
        assert_eq!(ui.activity(), Activity::Sending);
    }

    #[test]
    fn dispatch_code_prompt_no_session_shows_error() {
        let session = cyril_core::session::SessionController::new(); // no session ID
        let mut ui = UiState::new(500);
        let result = dispatch_code_command(
            &serde_json::json!({
                "success": true,
                "data": {
                    "executePrompt": "Analyze...",
                    "label": "Summary"
                }
            }),
            &session,
            &mut ui,
        );
        assert!(result.is_none());
        // Should show error, not the prompt system message
        assert!(!ui.messages().is_empty());
        assert_eq!(ui.activity(), Activity::Idle);
    }

    #[test]
    fn dispatch_code_prompt_without_label_uses_default() {
        let session = code_session();
        let mut ui = UiState::new(500);
        dispatch_code_command(
            &serde_json::json!({
                "success": true,
                "data": {
                    "executePrompt": "Analyze..."
                }
            }),
            &session,
            &mut ui,
        );
        // System message should use the default label
        let has_default_label = ui.messages().iter().any(|m| {
            matches!(m.kind(), cyril_ui::traits::ChatMessageKind::System(s) if s.contains("Code Intelligence"))
        });
        assert!(has_default_label, "should use 'Code Intelligence' as default label");
    }

    #[test]
    fn dispatch_code_unknown_adds_command_output() {
        let session = code_session();
        let mut ui = UiState::new(500);
        let result = dispatch_code_command(
            &serde_json::json!({
                "success": true,
                "message": "Something unexpected"
            }),
            &session,
            &mut ui,
        );
        assert!(result.is_none());
        assert!(!ui.has_code_panel());
        assert!(!ui.messages().is_empty());
    }
}
