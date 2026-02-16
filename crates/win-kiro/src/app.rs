use std::path::PathBuf;
use std::rc::Rc;

use agent_client_protocol::{self as acp, Agent};
use anyhow::Result;
use crossterm::event::{EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use win_kiro_core::event::AppEvent;
use win_kiro_core::path;

use crate::commands::{self, ParsedCommand};
use crate::event::Event;
use crate::tui::Tui;
use crate::ui::{approval, chat, input, tool_calls, toolbar};

use ratatui::layout::{Constraint, Layout};

/// Main application state.
pub struct App {
    pub chat: chat::ChatState,
    pub input: input::InputState,
    pub toolbar: toolbar::ToolbarState,
    pub tool_calls: tool_calls::ToolCallsState,
    pub approval: Option<(approval::ApprovalState, oneshot::Sender<acp::RequestPermissionResponse>)>,
    pub should_quit: bool,
    conn: Rc<acp::ClientSideConnection>,
    cwd: PathBuf,
    session_id: Option<acp::SessionId>,
    event_rx: mpsc::UnboundedReceiver<AppEvent>,
    prompt_done_rx: mpsc::UnboundedReceiver<()>,
    prompt_done_tx: mpsc::UnboundedSender<()>,
}

impl App {
    pub fn new(
        conn: Rc<acp::ClientSideConnection>,
        cwd: PathBuf,
        event_rx: mpsc::UnboundedReceiver<AppEvent>,
    ) -> Self {
        let (prompt_done_tx, prompt_done_rx) = mpsc::unbounded_channel();
        Self {
            chat: chat::ChatState::default(),
            input: input::InputState::default(),
            toolbar: toolbar::ToolbarState::default(),
            tool_calls: tool_calls::ToolCallsState::default(),
            approval: None,
            should_quit: false,
            conn,
            cwd,
            session_id: None,
            event_rx,
            prompt_done_rx,
            prompt_done_tx,
        }
    }

    pub fn set_session_id(&mut self, session_id: acp::SessionId) {
        self.toolbar.session_id = Some(session_id.to_string());
        self.session_id = Some(session_id);
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

        let has_tools = self.tool_calls.has_active();
        let tool_height = if has_tools {
            (self.tool_calls.active_calls.len() as u16 + 2).min(8)
        } else {
            0
        };

        let chunks = Layout::vertical([
            Constraint::Length(1),           // toolbar
            Constraint::Min(5),             // chat
            Constraint::Length(tool_height), // tool calls (0 if none)
            Constraint::Length(5),           // input
        ])
        .split(area);

        toolbar::render(frame, chunks[0], &self.toolbar);
        chat::render(frame, chunks[1], &self.chat);

        if has_tools {
            tool_calls::render(frame, chunks[2], &self.tool_calls);
        }

        input::render(frame, chunks[3], &self.input);

        // Approval overlay on top
        if let Some((ref approval_state, _)) = self.approval {
            approval::render(frame, area, approval_state);
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

        // Check if autocomplete is showing
        let has_suggestions = !self.input.suggestions().is_empty();

        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
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
            KeyCode::Enter if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                // Check for slash commands first
                if has_suggestions {
                    // If autocomplete is showing and there's an exact match, apply it
                    self.input.apply_suggestion();
                } else {
                    self.handle_enter().await?;
                }
            }
            KeyCode::Esc => {
                if self.toolbar.is_busy {
                    if let Some(ref session_id) = self.session_id {
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

        if let Some(cmd) = commands::parse_command(&text) {
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
                let mut help = String::from("Available commands:\n");
                for cmd in commands::COMMANDS {
                    help.push_str(&format!("  {:<12} {}\n", cmd.name, cmd.description));
                }
                help.push_str("\nKeyboard shortcuts:\n");
                help.push_str("  Ctrl+C/Q   Quit\n");
                help.push_str("  Esc        Cancel current request\n");
                help.push_str("  Tab        Accept autocomplete suggestion\n");
                help.push_str("  Shift+Enter  Newline in input\n");
                self.chat.add_system_message(help);
                self.chat.scroll_to_bottom();
            }
            ParsedCommand::New => {
                self.create_new_session().await?;
            }
            ParsedCommand::Sessions => {
                self.list_sessions().await?;
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

        let text = self.input.take_input();
        self.chat.add_user_message(text.clone());
        self.chat.begin_streaming();
        self.chat.scroll_to_bottom();
        self.toolbar.is_busy = true;
        self.tool_calls.clear_completed();

        let session_id = match &self.session_id {
            Some(id) => id.clone(),
            None => return Ok(()),
        };

        let conn = self.conn.clone();
        let done_tx = self.prompt_done_tx.clone();
        tokio::task::spawn_local(async move {
            let result = conn
                .prompt(acp::PromptRequest::new(
                    session_id,
                    vec![acp::ContentBlock::Text(acp::TextContent::new(text))],
                ))
                .await;

            if let Err(e) = result {
                tracing::error!("Prompt error: {e}");
            }
            let _ = done_tx.send(());
        });

        Ok(())
    }

    /// Create a new session, replacing the current one.
    async fn create_new_session(&mut self) -> Result<()> {
        let wsl_cwd = path::win_to_wsl(&self.cwd);
        match self.conn.new_session(acp::NewSessionRequest::new(wsl_cwd)).await {
            Ok(response) => {
                self.set_session_id(response.session_id);
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

    /// List available sessions.
    async fn list_sessions(&mut self) -> Result<()> {
        let wsl_cwd = path::win_to_wsl(&self.cwd);
        match self
            .conn
            .list_sessions(acp::ListSessionsRequest::new().cwd(wsl_cwd))
            .await
        {
            Ok(response) => {
                if response.sessions.is_empty() {
                    self.chat
                        .add_system_message("No previous sessions found.".to_string());
                } else {
                    let mut msg = String::from("Previous sessions:\n");
                    for session in &response.sessions {
                        let title = session.title.as_deref().unwrap_or("(untitled)");
                        let updated = session.updated_at.as_deref().unwrap_or("");
                        msg.push_str(&format!(
                            "  {} - {} {}\n",
                            session.session_id, title, updated
                        ));
                    }
                    msg.push_str("\nUse /load <session-id> to resume a session.");
                    self.chat.add_system_message(msg);
                }
                self.chat.scroll_to_bottom();
            }
            Err(e) => {
                self.chat
                    .add_system_message(format!("Failed to list sessions: {e}"));
            }
        }
        Ok(())
    }

    /// Load (resume) a previous session by ID.
    async fn load_session(&mut self, session_id_str: &str) -> Result<()> {
        let wsl_cwd = path::win_to_wsl(&self.cwd);
        let session_id = acp::SessionId::from(session_id_str.to_string());

        match self
            .conn
            .resume_session(acp::ResumeSessionRequest::new(
                session_id.clone(),
                wsl_cwd,
            ))
            .await
        {
            Ok(_) => {
                self.set_session_id(session_id);
                self.chat = chat::ChatState::default();
                self.chat.add_system_message(format!(
                    "Resumed session: {session_id_str}"
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
                self.tool_calls.add_tool_call(
                    tool_call.tool_call_id.to_string(),
                    tool_call.title.clone(),
                );
            }
            AppEvent::ToolCallUpdated { update, .. } => {
                self.tool_calls.update_tool_call(
                    &update.tool_call_id.to_string(),
                    update.fields.status.clone(),
                    update.fields.title.clone(),
                );
            }
            AppEvent::PermissionRequest { request, responder } => {
                let state = approval::ApprovalState::from_request(&request);
                self.approval = Some((state, responder));
            }
            AppEvent::HookFeedback { text } => {
                self.chat.add_system_message(format!("[Hook] {text}"));
                self.send_hook_feedback(text);
            }
            AppEvent::CommandsUpdated { .. } => {}
            AppEvent::ModeChanged { .. } => {}
            AppEvent::PlanUpdated { .. } => {}
        }
    }

    fn send_hook_feedback(&self, text: String) {
        let session_id = match &self.session_id {
            Some(id) => id.clone(),
            None => return,
        };

        let conn = self.conn.clone();
        tokio::task::spawn_local(async move {
            let result = conn
                .prompt(acp::PromptRequest::new(
                    session_id,
                    vec![acp::ContentBlock::Text(acp::TextContent::new(text))],
                ))
                .await;

            if let Err(e) = result {
                tracing::error!("Hook feedback prompt error: {e}");
            }
        });
    }

    fn on_turn_end(&mut self) {
        self.chat.finish_streaming();
        self.toolbar.is_busy = false;
        self.tool_calls.clear_completed();
    }
}
