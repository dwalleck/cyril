use std::path::PathBuf;
use std::rc::Rc;

use agent_client_protocol::{self as acp, Agent};
use anyhow::Result;
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, EventStream, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers,
};
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use cyril_core::event::{AppEvent, ExtensionEvent, InteractionRequest, InternalEvent, ProtocolEvent};
use cyril_core::session::SessionContext;

use crate::commands::{self, CommandChannels, CommandExecutor, CommandResult};
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
    pub session: SessionContext,
    conn: Rc<acp::ClientSideConnection>,
    event_rx: mpsc::UnboundedReceiver<AppEvent>,
    prompt_done_rx: mpsc::UnboundedReceiver<()>,
    /// Channel for command responses to display in chat.
    cmd_response_rx: mpsc::UnboundedReceiver<String>,
    channels: CommandChannels,
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
            session: SessionContext::new(cwd),
            conn,
            event_rx,
            prompt_done_rx,
            cmd_response_rx,
            channels: CommandChannels { prompt_done_tx, cmd_response_tx },
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

        toolbar::render(frame, chunks[0], &self.toolbar, &self.session);
        chat::render(frame, chunks[1], &self.chat);
        input::render(frame, chunks[2], &mut self.input);

        let pct = self.session.context_usage_pct.unwrap_or(0.0);
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
                            let response = acp::RequestPermissionResponse::new(
                                acp::RequestPermissionOutcome::Selected(
                                    acp::SelectedPermissionOutcome::new(option_id.to_string()),
                                ),
                            );
                            if responder.send(response).is_err() {
                                tracing::warn!("Permission response could not be delivered — agent may have cancelled");
                            }
                        }
                    }
                }
                KeyCode::Esc => {
                    if let Some((_, responder)) = self.approval.take() {
                        let response = acp::RequestPermissionResponse::new(
                            acp::RequestPermissionOutcome::Cancelled,
                        );
                        if responder.send(response).is_err() {
                            tracing::warn!("Permission response could not be delivered — agent may have cancelled");
                        }
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
                        CommandExecutor::handle_picker_confirm(
                            &mut self.session,
                            &self.conn,
                            &self.channels,
                            picker_state,
                        );
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
                    self.handle_enter().await?;
                }
            }
            KeyCode::Enter if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.handle_enter().await?;
            }
            KeyCode::Esc => {
                if self.toolbar.is_busy {
                    if let Some(ref session_id) = self.session.id {
                        if let Err(e) = self.conn.cancel(acp::CancelNotification::new(session_id.clone())).await {
                            tracing::warn!("Failed to send cancel notification: {e}");
                        }
                    }
                }
            }
            _ => {
                self.input.textarea.input(key);
                self.input.autocomplete_selected = 0;
            }
        }

        Ok(())
    }

    /// Handle Enter key -- either execute a slash command or send a prompt.
    async fn handle_enter(&mut self) -> Result<()> {
        if self.input.is_empty() {
            return Ok(());
        }

        let text = self.input.current_text();

        if let Some(cmd) = commands::parse_command(&text, &self.input.agent_commands) {
            self.input.take_input();
            let result = CommandExecutor::execute(
                cmd,
                &mut self.session,
                &self.conn,
                &mut self.chat,
                &self.input.agent_commands,
                &mut self.toolbar,
                &mut self.picker,
                &self.channels,
            )
            .await?;

            if matches!(result, CommandResult::Quit) {
                self.should_quit = true;
            }
        } else {
            CommandExecutor::send_prompt(
                &self.session,
                &self.conn,
                &mut self.chat,
                &mut self.input,
                &mut self.toolbar,
                &self.channels,
            )
            .await?;
        }

        Ok(())
    }

    fn handle_acp_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Protocol(e) => self.handle_protocol_event(e),
            AppEvent::Interaction(r) => self.handle_interaction(r),
            AppEvent::Extension(e) => self.handle_extension_event(e),
            AppEvent::Internal(e) => self.handle_internal_event(e),
        }
    }

    fn handle_protocol_event(&mut self, event: ProtocolEvent) {
        match event {
            ProtocolEvent::AgentMessage { chunk, .. } => {
                if let acp::ContentBlock::Text(text) = &chunk.content {
                    self.chat.append_streaming(&text.text);
                    self.chat.scroll_to_bottom();
                }
            }
            ProtocolEvent::AgentThought { chunk, .. } => {
                if let acp::ContentBlock::Text(text) = &chunk.content {
                    self.chat.append_streaming(&text.text);
                }
            }
            ProtocolEvent::ToolCallStarted { tool_call, .. } => {
                self.chat.add_tool_call(tool_call);
            }
            ProtocolEvent::ToolCallUpdated { update, .. } => {
                self.chat.update_tool_call(update);
            }
            ProtocolEvent::CommandsUpdated { commands, .. } => {
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
            ProtocolEvent::ModeChanged { mode, .. } => {
                self.session.current_mode_id = Some(mode.current_mode_id.to_string());
            }
            ProtocolEvent::PlanUpdated { plan, .. } => {
                self.chat.update_plan(plan);
            }
            ProtocolEvent::ConfigOptionsUpdated { config_options, .. } => {
                self.session.set_config_options(config_options);
            }
        }
    }

    fn handle_interaction(&mut self, request: InteractionRequest) {
        match request {
            InteractionRequest::Permission { request, responder } => {
                let state = approval::ApprovalState::from_request(&request);
                self.approval = Some((state, responder));
            }
        }
    }

    fn handle_extension_event(&mut self, event: ExtensionEvent) {
        match event {
            ExtensionEvent::KiroCommandsAvailable { commands: kiro_cmds } => {
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
            ExtensionEvent::KiroMetadata { context_usage_pct, .. } => {
                self.session.context_usage_pct = Some(context_usage_pct);
            }
        }
    }

    fn handle_internal_event(&mut self, event: InternalEvent) {
        match event {
            InternalEvent::HookFeedback { text } => {
                self.chat.add_system_message(format!("[Hook] {text}"));
                self.pending_hook_feedback.push(text);
            }
        }
    }

    fn toggle_mouse_capture(&mut self) {
        self.toolbar.mouse_captured = !self.toolbar.mouse_captured;
        let mut stdout = std::io::stdout();
        if self.toolbar.mouse_captured {
            let _ = crossterm::execute!(stdout, EnableMouseCapture);
        } else {
            let _ = crossterm::execute!(stdout, DisableMouseCapture);
        }
    }

    fn on_turn_end(&mut self) {
        self.chat.finish_streaming();
        self.toolbar.is_busy = false;

        CommandExecutor::flush_next_hook_feedback(
            &self.session,
            &self.conn,
            &mut self.chat,
            &mut self.toolbar,
            &self.channels,
            &mut self.pending_hook_feedback,
        );
    }
}
