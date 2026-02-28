use std::path::PathBuf;
use std::rc::Rc;

use std::sync::Arc;

use agent_client_protocol::{self as acp, Agent};
use anyhow::Result;
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, EventStream, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers,
};
use futures_util::StreamExt;
use serde_json::value::RawValue;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use cyril_core::event::AppEvent;
use cyril_core::path;
use cyril_core::session::SessionContext;

use crate::commands::{self, ParsedCommand};
use crate::event::Event;
use crate::file_completer;
use crate::tui::Tui;
use crate::ui::{approval, chat, input, picker, toolbar};

use ratatui::layout::{Constraint, Layout};

/// Main application state.
pub struct App {
    pub chat: chat::ChatState,
    pub input: input::InputState,
    pub toolbar: toolbar::ToolbarState,
    pub approval: Option<(approval::ApprovalState, oneshot::Sender<acp::RequestPermissionResponse>)>,
    pub picker: Option<picker::PickerState>,
    pub should_quit: bool,
    pub mouse_captured: bool,
    pub session: SessionContext,
    conn: Rc<acp::ClientSideConnection>,
    event_rx: mpsc::UnboundedReceiver<AppEvent>,
    prompt_done_rx: mpsc::UnboundedReceiver<()>,
    prompt_done_tx: mpsc::UnboundedSender<()>,
    /// Channel for command responses to display in chat.
    cmd_response_rx: mpsc::UnboundedReceiver<String>,
    cmd_response_tx: mpsc::UnboundedSender<String>,
    /// Queued hook feedback to send after the current turn completes.
    pending_hook_feedback: Vec<String>,
}

impl App {
    pub fn new(
        conn: Rc<acp::ClientSideConnection>,
        cwd: PathBuf,
        event_rx: mpsc::UnboundedReceiver<AppEvent>,
    ) -> Self {
        let (prompt_done_tx, prompt_done_rx) = mpsc::unbounded_channel();
        let (cmd_response_tx, cmd_response_rx) = mpsc::unbounded_channel();

        let mut input = input::InputState::default();
        let file_completer = file_completer::FileCompleter::new(cwd.clone());
        input.file_completer = Some(file_completer);

        Self {
            chat: chat::ChatState::default(),
            input,
            toolbar: toolbar::ToolbarState { mouse_captured: true, ..Default::default() },
            approval: None,
            picker: None,
            should_quit: false,
            mouse_captured: true,
            session: SessionContext::new(cwd),
            conn,
            event_rx,
            prompt_done_rx,
            prompt_done_tx,
            cmd_response_rx,
            cmd_response_tx,
            pending_hook_feedback: Vec::new(),
        }
    }

    /// Load project files for @-completion asynchronously.
    pub async fn load_project_files(&mut self) {
        if let Some(ref mut completer) = self.input.file_completer {
            if let Err(e) = completer.load_files().await {
                tracing::warn!("Failed to load project files: {e}");
                self.chat.add_system_message(format!(
                    "@-file completion unavailable: {e}"
                ));
            }
        }
    }

    /// Run the main event loop.
    pub async fn run(&mut self, terminal: &mut Tui) -> Result<()> {
        let mut crossterm_events = EventStream::new();
        let tick_rate = tokio::time::Duration::from_millis(33); // ~30fps
        let mut tick_interval = tokio::time::interval(tick_rate);

        loop {
            terminal.draw(|frame| self.render(frame))?;

            let event = tokio::select! {
                ct_event = crossterm_events.next() => {
                    match ct_event {
                        Some(Ok(e)) => Some(Event::from(e)),
                        Some(Err(_)) => continue,
                        None => break,
                    }
                }
                acp_event = self.event_rx.recv() => {
                    match acp_event {
                        Some(e) => Some(Event::Acp(e)),
                        None => break,
                    }
                }
                _ = self.prompt_done_rx.recv() => {
                    self.on_turn_end();
                    None
                }
                msg = self.cmd_response_rx.recv() => {
                    if let Some(text) = msg {
                        self.chat.append_streaming(&text);
                        self.chat.scroll_to_bottom();
                    }
                    None
                }
                _ = tick_interval.tick() => {
                    None
                }
            };

            if let Some(event) = event {
                self.handle_event(event).await?;
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    fn render(&mut self, frame: &mut ratatui::Frame) {
        let area = frame.area();

        let chunks = Layout::vertical([
            Constraint::Length(1),  // toolbar
            Constraint::Min(5),    // chat (includes inline tool calls)
            Constraint::Length(5), // input
            Constraint::Length(1), // context bar
        ])
        .split(area);

        toolbar::render(frame, chunks[0], &self.toolbar);
        chat::render(frame, chunks[1], &self.chat);
        input::render(frame, chunks[2], &mut self.input);

        let pct = self.toolbar.context_usage_pct.unwrap_or(0.0);
        toolbar::render_context_bar(frame, chunks[3], pct);

        // Overlay popups
        if let Some((ref approval_state, _)) = self.approval {
            approval::render(frame, area, approval_state);
        } else if let Some(ref picker_state) = self.picker {
            picker::render(frame, area, picker_state);
        }
    }

    async fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => self.handle_key(key).await?,
            Event::Acp(acp_event) => self.handle_acp_event(acp_event),
            Event::Mouse(mouse) => {
                use crossterm::event::MouseEventKind;
                match mouse.kind {
                    MouseEventKind::ScrollUp => self.chat.scroll_up(),
                    MouseEventKind::ScrollDown => self.chat.scroll_down(),
                    _ => {}
                }
            }
            Event::Tick | Event::Resize(_, _) => {}
        }
        Ok(())
    }

    async fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        // Handle approval mode first
        if let Some((ref mut approval_state, _)) = self.approval {
            match key.code {
                KeyCode::Up => approval_state.select_prev(),
                KeyCode::Down => approval_state.select_next(),
                KeyCode::Enter => {
                    if let Some((approval_state, responder)) = self.approval.take() {
                        if let Some(option_id) = approval_state.selected_option_id() {
                            let _ = responder.send(acp::RequestPermissionResponse::new(
                                acp::RequestPermissionOutcome::Selected(
                                    acp::SelectedPermissionOutcome::new(option_id.to_string()),
                                ),
                            ));
                        }
                    }
                }
                KeyCode::Esc => {
                    if let Some((_, responder)) = self.approval.take() {
                        let _ = responder.send(acp::RequestPermissionResponse::new(
                            acp::RequestPermissionOutcome::Cancelled,
                        ));
                    }
                }
                _ => {}
            }
            return Ok(());
        }

        // Handle picker mode
        if let Some(ref mut picker_state) = self.picker {
            match key.code {
                KeyCode::Up => picker_state.select_prev(),
                KeyCode::Down => picker_state.select_next(),
                KeyCode::Enter => {
                    if let Some(picker_state) = self.picker.take() {
                        self.handle_picker_confirm(picker_state);
                    }
                }
                KeyCode::Esc => {
                    self.picker = None;
                }
                _ => {}
            }
            return Ok(());
        }

        // Check if autocomplete is showing
        let has_suggestions = self.input.has_suggestions();

        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('m') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.toggle_mouse_capture();
            }
            KeyCode::Tab if has_suggestions => {
                self.input.apply_suggestion();
            }
            KeyCode::Up if has_suggestions => {
                self.input.autocomplete_up();
            }
            KeyCode::Down if has_suggestions => {
                self.input.autocomplete_down();
            }
            KeyCode::Enter if !key.modifiers.contains(KeyModifiers::SHIFT) && has_suggestions => {
                let is_slash = matches!(self.input.active_popup(), input::ActivePopup::SlashCommand);
                self.input.apply_suggestion();
                if is_slash {
                    // Slash command: accept and execute immediately
                    self.handle_enter().await?;
                }
                // File popup: just accept the path (don't submit)
            }
            KeyCode::Enter if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.handle_enter().await?;
            }
            KeyCode::Esc => {
                if self.toolbar.is_busy {
                    if let Some(ref session_id) = self.session.id {
                        let _ = self.conn.cancel(acp::CancelNotification::new(session_id.clone())).await;
                    }
                }
            }
            _ => {
                self.input.textarea.input(key);
                // Reset autocomplete selection when input changes
                self.input.autocomplete_selected = 0;
            }
        }

        Ok(())
    }

    /// Handle Enter key — either execute a slash command or send a prompt.
    async fn handle_enter(&mut self) -> Result<()> {
        if self.input.is_empty() {
            return Ok(());
        }

        let text = self.input.current_text();

        if let Some(cmd) = commands::parse_command(&text, &self.input.agent_commands) {
            self.input.take_input();
            self.execute_command(cmd).await?;
        } else {
            self.send_prompt().await?;
        }

        Ok(())
    }

    /// Execute a parsed slash command.
    async fn execute_command(&mut self, cmd: ParsedCommand) -> Result<()> {
        match cmd {
            ParsedCommand::Quit => {
                self.should_quit = true;
            }
            ParsedCommand::Clear => {
                self.chat = chat::ChatState::default();
                self.chat.add_system_message("Chat cleared.".to_string());
            }
            ParsedCommand::Help => {
                let mut help = String::from("Local commands:\n");
                for cmd in commands::COMMANDS {
                    help.push_str(&format!("  {:<14} {}\n", cmd.name, cmd.description));
                }
                if !self.input.agent_commands.is_empty() {
                    help.push_str("\nAgent commands:\n");
                    for cmd in &self.input.agent_commands {
                        help.push_str(&format!("  {:<20} {}\n", cmd.display_name(), cmd.description));
                    }
                }
                help.push_str("\nKeyboard shortcuts:\n");
                help.push_str("  Ctrl+C/Q     Quit\n");
                help.push_str("  Ctrl+M       Toggle mouse (off = copy mode)\n");
                help.push_str("  Esc          Cancel current request\n");
                help.push_str("  Tab          Accept autocomplete suggestion\n");
                help.push_str("  Shift+Enter  Newline in input\n");
                self.chat.add_system_message(help);
                self.chat.scroll_to_bottom();
            }
            ParsedCommand::New => {
                self.create_new_session().await?;
            }
            ParsedCommand::Load(session_id) => {
                if session_id.is_empty() {
                    self.chat.add_system_message(
                        "Usage: /load <session-id>\nUse /sessions to list available sessions."
                            .to_string(),
                    );
                } else {
                    self.load_session(&session_id).await?;
                }
            }
            ParsedCommand::Mode(mode_id) => {
                self.set_mode(&mode_id).await?;
            }
            ParsedCommand::ModelSelect(model_id) => {
                self.set_model(&model_id).await?;
            }
            ParsedCommand::Agent { name, input } => {
                // Build the full command string: "/name" or "/name input"
                let command = if let Some(input_text) = input {
                    format!("/{name} {input_text}")
                } else {
                    format!("/{name}")
                };
                self.execute_agent_command(&command).await?;
            }
            ParsedCommand::Unknown(cmd) => {
                self.chat.add_system_message(format!(
                    "Unknown command: {cmd}\nType /help for available commands."
                ));
            }
        }
        Ok(())
    }

    async fn send_prompt(&mut self) -> Result<()> {
        if self.input.is_empty() || self.toolbar.is_busy {
            return Ok(());
        }

        let session_id = match &self.session.id {
            Some(id) => id.clone(),
            None => {
                self.chat.add_system_message("No active session. Use /new to start one.".to_string());
                return Ok(());
            }
        };

        let text = self.input.take_input();
        self.chat.add_user_message(text.clone());
        self.chat.begin_streaming();
        self.chat.scroll_to_bottom();
        self.toolbar.is_busy = true;

        // Build content blocks: user text + any @-referenced file contents
        let mut content_blocks = vec![acp::ContentBlock::Text(acp::TextContent::new(text.clone()))];

        if let Some(ref completer) = self.input.file_completer {
            for path in file_completer::parse_file_references(&text, completer) {
                match completer.read_file(&path) {
                    Ok(contents) => {
                        let file_block = format!("<file path=\"{path}\">\n{contents}\n</file>");
                        content_blocks
                            .push(acp::ContentBlock::Text(acp::TextContent::new(file_block)));
                        tracing::info!("Attached @-referenced file: {path}");
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read @-referenced file {path}: {e}");
                    }
                }
            }
        }

        let conn = self.conn.clone();
        let done_tx = self.prompt_done_tx.clone();
        let response_tx = self.cmd_response_tx.clone();
        tokio::task::spawn_local(async move {
            let result = conn
                .prompt(acp::PromptRequest::new(session_id, content_blocks))
                .await;

            if let Err(e) = result {
                tracing::error!("Prompt error: {e}");
                let _ = response_tx.send(format!("[Error] Prompt failed: {e}"));
            }
            let _ = done_tx.send(());
        });

        Ok(())
    }

    /// Create a new session, replacing the current one.
    async fn create_new_session(&mut self) -> Result<()> {
        let agent_cwd = path::to_agent(&self.session.cwd);
        match self.conn.new_session(acp::NewSessionRequest::new(agent_cwd)).await {
            Ok(response) => {
                self.toolbar.session_id = Some(response.session_id.to_string());
                self.session.set_session_id(response.session_id);
                if let Some(ref modes) = response.modes {
                    self.session.set_modes(modes);
                    self.toolbar.current_mode = self.session.current_mode_id.clone();
                }
                if let Some(config_options) = response.config_options {
                    self.session.set_config_options(config_options);
                    self.toolbar.current_model = self.session.current_model();
                }
                self.chat = chat::ChatState::default();
                self.chat
                    .add_system_message("New session started.".to_string());
                self.chat.scroll_to_bottom();
            }
            Err(e) => {
                self.chat
                    .add_system_message(format!("Failed to create session: {e}"));
            }
        }
        Ok(())
    }

    /// Execute a Kiro agent command via the extension method.
    /// The `command` should be the full slash command string, e.g. "/compact".
    async fn execute_agent_command(&mut self, command: &str) -> Result<()> {
        let session_id = match &self.session.id {
            Some(id) => id.clone(),
            None => {
                self.chat.add_system_message("No active session. Use /new to start one.".to_string());
                return Ok(());
            }
        };

        let params = serde_json::json!({
            "sessionId": session_id.to_string(),
            "command": command
        });

        let raw_params = RawValue::from_string(params.to_string())
            .map_err(|e| anyhow::anyhow!("Failed to serialize command params: {e}"))?;

        self.chat.begin_streaming();
        self.chat.scroll_to_bottom();
        self.toolbar.is_busy = true;

        let conn = self.conn.clone();
        let done_tx = self.prompt_done_tx.clone();
        let response_tx = self.cmd_response_tx.clone();
        let cmd_str = command.to_string();
        tokio::task::spawn_local(async move {
            let result = conn
                .ext_method(acp::ExtRequest::new(
                    "kiro.dev/commands/execute",
                    Arc::from(raw_params),
                ))
                .await;

            match result {
                Ok(resp) => {
                    tracing::info!("Command {cmd_str} response: {}", resp.0);
                    // Extract the "message" field from the response for display
                    let displayed = if let Ok(val) = serde_json::from_str::<serde_json::Value>(resp.0.get()) {
                        if let Some(msg) = val.get("message").and_then(|m| m.as_str()) {
                            let _ = response_tx.send(msg.to_string());
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    if !displayed {
                        // Show the raw response so the user sees something
                        let _ = response_tx.send(resp.0.to_string());
                    }
                }
                Err(e) => {
                    tracing::error!("Command {cmd_str} error: {e}");
                    let _ = response_tx.send(format!("[Error] Command {cmd_str} failed: {e}"));
                }
            }
            let _ = done_tx.send(());
        });

        Ok(())
    }

    /// Switch the agent mode via session/set_mode.
    async fn set_mode(&mut self, mode_id: &str) -> Result<()> {
        if mode_id.is_empty() {
            // List available modes
            if self.session.available_modes.is_empty() {
                self.chat.add_system_message(
                    "No modes available. The agent did not advertise any modes.".to_string(),
                );
            } else {
                let mut msg = String::from("Available modes:\n");
                for mode in &self.session.available_modes {
                    let current = self
                        .session
                        .current_mode_id
                        .as_deref()
                        .map_or(false, |c| c == mode.id);
                    let marker = if current { " (active)" } else { "" };
                    msg.push_str(&format!("  {:<20} {}{}\n", mode.id, mode.name, marker));
                }
                msg.push_str("\nUsage: /mode <id>");
                self.chat.add_system_message(msg);
            }
            return Ok(());
        }

        let session_id = match &self.session.id {
            Some(id) => id.clone(),
            None => {
                self.chat
                    .add_system_message("No active session.".to_string());
                return Ok(());
            }
        };

        match self
            .conn
            .set_session_mode(acp::SetSessionModeRequest::new(
                session_id,
                mode_id.to_string(),
            ))
            .await
        {
            Ok(_) => {
                self.session.current_mode_id = Some(mode_id.to_string());
                self.toolbar.current_mode = Some(mode_id.to_string());
                self.chat.add_system_message(format!("Switched to mode: {mode_id}"));
            }
            Err(e) => {
                self.chat
                    .add_system_message(format!("Failed to set mode: {e}"));
            }
        }
        Ok(())
    }

    /// Switch the model via Kiro extension commands.
    /// No arg: query `_kiro.dev/commands/options` for available models.
    /// With arg: execute `/model <id>` via `_kiro.dev/commands/execute`.
    async fn set_model(&mut self, model_id: &str) -> Result<()> {
        let session_id = match &self.session.id {
            Some(id) => id.clone(),
            None => {
                self.chat
                    .add_system_message("No active session.".to_string());
                return Ok(());
            }
        };

        if model_id.is_empty() {
            // Query available models from Kiro
            let params = serde_json::json!({
                "command": "model",
                "sessionId": session_id.to_string()
            });
            let raw_params = RawValue::from_string(params.to_string())
                .map_err(|e| anyhow::anyhow!("Failed to serialize params: {e}"))?;

            match self
                .conn
                .ext_method(acp::ExtRequest::new(
                    "kiro.dev/commands/options",
                    Arc::from(raw_params),
                ))
                .await
            {
                Ok(resp) => {
                    self.open_model_picker(resp.0.get());
                }
                Err(e) => {
                    self.chat
                        .add_system_message(format!("Failed to query models: {e}"));
                }
            }
            return Ok(());
        }

        // Set model via set_session_config_option (spawned to avoid blocking the event loop)
        self.toolbar.current_model = Some(model_id.to_string());

        let conn = self.conn.clone();
        let response_tx = self.cmd_response_tx.clone();
        let model_str = model_id.to_string();
        tokio::task::spawn_local(async move {
            match conn
                .set_session_config_option(acp::SetSessionConfigOptionRequest::new(
                    session_id,
                    "model".to_string(),
                    model_str.clone(),
                ))
                .await
            {
                Ok(_) => {
                    tracing::info!("Set model to: {model_str}");
                    let _ = response_tx.send(format!("Switched to model: {model_str}"));
                }
                Err(e) => {
                    tracing::error!("Failed to set model: {e}");
                    let _ = response_tx.send(format!("Failed to set model: {e}"));
                }
            }
        });
        Ok(())
    }

    /// Parse model options from a `_kiro.dev/commands/options` response and open a picker.
    fn open_model_picker(&mut self, raw_json: &str) {
        let val: serde_json::Value = match serde_json::from_str(raw_json) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Failed to parse model options: {e}");
                self.chat
                    .add_system_message("Failed to parse model options.".to_string());
                return;
            }
        };

        // Response is typically { "options": [{ "value": "...", "name": "..." }, ...] }
        // or possibly a bare array.
        let options = val
            .get("options")
            .and_then(|v| v.as_array())
            .or_else(|| val.as_array());

        match options {
            Some(opts) if !opts.is_empty() => {
                let picker_options: Vec<picker::PickerOption> = opts
                    .iter()
                    .map(|opt| {
                        let value = opt
                            .get("value")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?")
                            .to_string();
                        let name = opt
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&value)
                            .to_string();
                        let active = opt
                            .get("current")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        picker::PickerOption {
                            label: name,
                            value,
                            active,
                        }
                    })
                    .collect();
                self.picker = Some(picker::PickerState::new(
                    "Select Model",
                    picker_options,
                    picker::PickerAction::SetModel,
                ));
            }
            _ => {
                tracing::info!("Model options raw response: {raw_json}");
                self.chat.add_system_message(
                    "No model options returned. Try /model <model-id> directly.".to_string(),
                );
            }
        }
    }

    /// Handle a confirmed picker selection.
    fn handle_picker_confirm(&mut self, state: picker::PickerState) {
        let value = match state.selected_value() {
            Some(v) => v.to_string(),
            None => return,
        };

        match state.action {
            picker::PickerAction::SetModel => {
                let session_id = match &self.session.id {
                    Some(id) => id.clone(),
                    None => return,
                };

                self.toolbar.current_model = Some(value.clone());

                let conn = self.conn.clone();
                let response_tx = self.cmd_response_tx.clone();
                tokio::task::spawn_local(async move {
                    match conn
                        .set_session_config_option(acp::SetSessionConfigOptionRequest::new(
                            session_id,
                            "model".to_string(),
                            value.clone(),
                        ))
                        .await
                    {
                        Ok(_) => {
                            tracing::info!("Set model to: {value}");
                            let _ = response_tx.send(format!("Switched to model: {value}"));
                        }
                        Err(e) => {
                            tracing::error!("Failed to set model: {e}");
                            let _ = response_tx.send(format!("Failed to set model: {e}"));
                        }
                    }
                });
            }
        }
    }

    /// Load a previous session by ID.
    async fn load_session(&mut self, session_id_str: &str) -> Result<()> {
        let agent_cwd = path::to_agent(&self.session.cwd);
        let session_id = acp::SessionId::from(session_id_str.to_string());

        match self
            .conn
            .load_session(acp::LoadSessionRequest::new(
                session_id.clone(),
                agent_cwd,
            ))
            .await
        {
            Ok(_) => {
                self.toolbar.session_id = Some(session_id.to_string());
                self.session.set_session_id(session_id);
                self.chat = chat::ChatState::default();
                self.chat.add_system_message(format!(
                    "Loaded session: {session_id_str}"
                ));
                self.chat.scroll_to_bottom();
            }
            Err(e) => {
                self.chat
                    .add_system_message(format!("Failed to load session: {e}"));
            }
        }
        Ok(())
    }

    fn handle_acp_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::AgentMessage { chunk, .. } => {
                if let acp::ContentBlock::Text(text) = &chunk.content {
                    self.chat.append_streaming(&text.text);
                    self.chat.scroll_to_bottom();
                }
            }
            AppEvent::AgentThought { chunk, .. } => {
                if let acp::ContentBlock::Text(text) = &chunk.content {
                    self.chat.append_streaming(&text.text);
                }
            }
            AppEvent::ToolCallStarted { tool_call, .. } => {
                self.chat.add_tool_call(tool_call);
            }
            AppEvent::ToolCallUpdated { update, .. } => {
                self.chat.update_tool_call(update);
            }
            AppEvent::PermissionRequest { request, responder } => {
                let state = approval::ApprovalState::from_request(&request);
                self.approval = Some((state, responder));
            }
            AppEvent::HookFeedback { text } => {
                self.chat.add_system_message(format!("[Hook] {text}"));
                self.queue_hook_feedback(text);
            }
            AppEvent::CommandsUpdated { commands, .. } => {
                self.input.agent_commands = commands
                    .available_commands
                    .iter()
                    .map(commands::AgentCommand::from_available)
                    .collect();
                tracing::info!(
                    "Received {} agent commands",
                    self.input.agent_commands.len()
                );
            }
            AppEvent::KiroCommandsAvailable { commands: kiro_cmds } => {
                // Filter out:
                // - Commands Cyril handles locally (/clear, /help, /quit, /load, /new)
                // - Commands needing special UI (inputType: "selection", "panel")
                // - Local-only commands (meta.local: true)
                // Strip leading "/" from names (Kiro sends "/compact", but
                // AgentCommand.display_name() adds its own "/" prefix).
                const LOCAL_COMMANDS: &[&str] = &["/agent", "/clear", "/help", "/quit", "/load", "/new", "/model"];
                self.input.agent_commands = kiro_cmds
                    .into_iter()
                    .filter(|cmd| {
                        !LOCAL_COMMANDS.contains(&cmd.name.as_str())
                            && cmd.is_executable()
                    })
                    .map(|cmd| {
                        let name = cmd.name.strip_prefix('/').unwrap_or(&cmd.name).to_string();
                        commands::AgentCommand {
                            name,
                            description: cmd.description,
                            input_hint: cmd.input_hint,
                        }
                    })
                    .collect();
                tracing::info!(
                    "Loaded {} commands from kiro.dev/commands/available",
                    self.input.agent_commands.len()
                );
            }
            AppEvent::KiroMetadata { context_usage_pct, .. } => {
                self.session.context_usage_pct = Some(context_usage_pct);
                self.toolbar.context_usage_pct = Some(context_usage_pct);
            }
            AppEvent::ModeChanged { mode, .. } => {
                let mode_id = mode.current_mode_id.to_string();
                self.session.current_mode_id = Some(mode_id.clone());
                self.toolbar.current_mode = Some(mode_id);
            }
            AppEvent::PlanUpdated { plan, .. } => {
                self.chat.update_plan(plan);
            }
            AppEvent::ConfigOptionsUpdated { config_options, .. } => {
                self.session.set_config_options(config_options);
                self.toolbar.current_model = self.session.current_model();
            }
        }
    }

    fn queue_hook_feedback(&mut self, text: String) {
        self.pending_hook_feedback.push(text);
    }

    /// Send the next queued hook feedback as a prompt. Called from on_turn_end().
    fn flush_next_hook_feedback(&mut self) {
        if self.pending_hook_feedback.is_empty() {
            return;
        }

        let session_id = match &self.session.id {
            Some(id) => id.clone(),
            None => {
                self.pending_hook_feedback.clear();
                return;
            }
        };

        let text = self.pending_hook_feedback.remove(0);
        self.toolbar.is_busy = true;
        self.chat.begin_streaming();

        let conn = self.conn.clone();
        let done_tx = self.prompt_done_tx.clone();
        let response_tx = self.cmd_response_tx.clone();
        tokio::task::spawn_local(async move {
            let result = conn
                .prompt(acp::PromptRequest::new(
                    session_id,
                    vec![acp::ContentBlock::Text(acp::TextContent::new(text))],
                ))
                .await;

            if let Err(e) = result {
                tracing::error!("Hook feedback prompt error: {e}");
                let _ = response_tx.send(format!("[Error] Hook feedback failed: {e}"));
            }
            let _ = done_tx.send(());
        });
    }

    fn toggle_mouse_capture(&mut self) {
        self.mouse_captured = !self.mouse_captured;
        self.toolbar.mouse_captured = self.mouse_captured;
        let mut stdout = std::io::stdout();
        if self.mouse_captured {
            let _ = crossterm::execute!(stdout, EnableMouseCapture);
        } else {
            let _ = crossterm::execute!(stdout, DisableMouseCapture);
        }
    }

    fn on_turn_end(&mut self) {
        self.chat.finish_streaming();
        self.toolbar.is_busy = false;

        // If hook feedback is queued, send the next one now that the turn is done.
        self.flush_next_hook_feedback();
    }
}
