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
use cyril_ui::state::UiState;
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
        Self {
            bridge_sender,
            notification_rx,
            permission_rx,
            ui_state: UiState::new(max_messages),
            session: SessionController::new(),
            commands: CommandRegistry::with_builtins(),
            redraw_needed: true,
            last_activity: Instant::now(),
        }
    }

    pub async fn create_initial_session(&mut self, cwd: PathBuf) {
        self.ui_state
            .add_system_message("Connecting to agent...".into());
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
        if let Notification::CommandsUpdated(ref cmds) = notification {
            self.commands.register_agent_commands(cmds);
        }

        // Handle clear command result
        if let Notification::AgentMessage(ref msg) = notification
            && !msg.is_streaming
            && msg.text == "__clear__"
        {
            self.ui_state.clear_messages();
        }

        self.redraw_needed = session_changed || ui_changed;
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

        // Layer 3: Normal input
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
                if let Some(value) = self.ui_state.picker_confirm() {
                    let title = self
                        .ui_state
                        .picker_title()
                        .unwrap_or_default()
                        .to_string();
                    self.bridge_sender
                        .send(BridgeCommand::ExtMethod {
                            method: "kiro.dev/commands/execute".into(),
                            params: serde_json::json!({
                                "command": title,
                                "args": {"value": value}
                            }),
                        })
                        .await?;
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

        self.bridge_sender
            .send(BridgeCommand::SendPrompt {
                session_id,
                text,
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
