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

/// Spawn the voice engine when the `voice` feature is enabled. This is the only
/// feature-gated site — everything downstream operates on the always-present
/// `Option<VoiceHandle>` and cyril-core voice types, so the `select!` arm and
/// command routing need no `#[cfg]`.
#[cfg(feature = "voice")]
fn spawn_voice_engine() -> Option<cyril_core::voice::VoiceHandle> {
    Some(cyril_voice::spawn_voice())
}

#[cfg(not(feature = "voice"))]
fn spawn_voice_engine() -> Option<cyril_core::voice::VoiceHandle> {
    None
}

pub struct App {
    bridge_sender: BridgeSender,
    notification_rx: mpsc::Receiver<RoutedNotification>,
    permission_rx: mpsc::Receiver<PermissionRequest>,
    ui_state: UiState,
    session: SessionController,
    commands: CommandRegistry,
    redraw_needed: bool,
    last_activity: Instant,
    /// The cwd kiro-cli was spawned in — used to resolve the active agent's
    /// workspace config (`<cwd>/.kiro/agents/`) when persisting trust grants.
    cwd: PathBuf,
    /// Voice-input engine handle (ROADMAP CN2). `None` when the `voice` feature
    /// is off (or the engine could not start). The type lives in cyril-core so
    /// this field and its `select!` arm compile regardless of the feature.
    voice: Option<cyril_core::voice::VoiceHandle>,
    /// Authoritative "is voice capturing?" intent. Flipped on each successful
    /// Start/Stop send (and cleared on engine `Error`). Toggling reads this —
    /// NOT the lagging `ui_state.voice_status()` projection — so rapid `/voice`
    /// presses (drained as a batch before the engine's Status echo arrives)
    /// alternate Start/Stop correctly instead of both sending `Start`. In V1a
    /// the engine only changes capture state in response to commands, so this
    /// optimistic model tracks it exactly; see the V1b note in `handle_voice_event`.
    voice_active: bool,
}

impl App {
    pub fn new(bridge: BridgeHandle, max_messages: usize, cwd: PathBuf) -> Self {
        let (bridge_sender, notification_rx, permission_rx) = bridge.split();
        let commands = CommandRegistry::with_builtins();
        let info: Vec<(String, Option<String>)> = commands
            .all_commands()
            .iter()
            .map(|c| {
                let desc = c.description();
                (
                    c.name().to_string(),
                    Some(desc.to_string()).filter(|s| !s.is_empty()),
                )
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
            cwd,
            voice: spawn_voice_engine(),
            voice_active: false,
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
        let mut redraw_interval = tokio::time::interval(Self::redraw_duration(Activity::Idle));
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
                    for deferred in self.handle_notification(notification) {
                        // SendPrompt triggers a real turn → mark session Busy.
                        // Session-management commands (LoadSession,
                        // TerminateSession) don't start a turn so leave status
                        // alone. See `/code` (busy) vs `/rewind` (not busy).
                        let starts_turn = matches!(deferred, BridgeCommand::SendPrompt { .. });
                        match self.bridge_sender.send(deferred).await {
                            Ok(()) => {
                                if starts_turn {
                                    self.session.set_status(SessionStatus::Busy);
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "failed to send deferred bridge command");
                                self.ui_state.set_activity(Activity::Idle);
                                self.ui_state.add_system_message(
                                    "Failed to dispatch follow-up command to agent.".into(),
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

                // Priority 4: Voice engine events (CN2). Resolves to `pending`
                // (never fires) when the voice feature is off — `voice` is None.
                voice_event = Self::next_voice_event(&mut self.voice) => {
                    match voice_event {
                        Some(ev) => self.handle_voice_event(ev),
                        // Channel closed: the engine thread exited. Stop polling
                        // so the branch parks on `pending` instead of busy-looping.
                        None => self.voice = None,
                    }
                }

                // Priority 5: Redraw tick
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

            // Adaptive frame rate — account for subagent and voice activity as
            // well as the main session (the voice meter animates while listening).
            let effective_activity =
                if self.ui_state.any_subagent_active() || self.ui_state.any_voice_active() {
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

    fn handle_notification(&mut self, routed: RoutedNotification) -> Vec<BridgeCommand> {
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
                self.ui_state
                    .apply_subagent_notification(sid, &notification);
                self.redraw_needed = true;
                return Vec::new();
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
                self.ui_state
                    .apply_subagent_notification(sid, &notification);
                self.redraw_needed = true;
                return Vec::new();
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
                    (
                        cmd.name().to_string(),
                        Some(desc.to_string()).filter(|s| !s.is_empty()),
                    )
                })
                .collect();
            for prompt in prompt_list {
                info.push((
                    prompt.name().to_string(),
                    prompt
                        .description()
                        .map(str::to_string)
                        .filter(|s| !s.is_empty()),
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
        if let Notification::CommandOptionsReceived {
            ref command,
            ref options,
        } = notification
        {
            if options.is_empty() {
                self.ui_state
                    .add_system_message(format!("No {command} options available."));
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
            self.ui_state.add_system_message(format!(
                "MCP server '{server_name}' requires authentication. Open in browser: {url}"
            ));
            self.redraw_needed = true;
        }

        // Handle command execution response. The `hooks` and `code` commands
        // are special-cased; all other commands fall through to the generic
        // command-output path. See `dispatch_command_executed` for the rules.
        let mut deferred_commands: Vec<BridgeCommand> = Vec::new();
        if let Notification::CommandExecuted {
            ref command,
            ref response,
        } = notification
        {
            if command == "code" {
                deferred_commands.extend(dispatch_code_command(
                    response,
                    &self.session,
                    &mut self.ui_state,
                ));
            } else if command == "rewind" {
                // `/rewind` orchestration: when the agent signals
                // `switchSession: true` in the response, fire the
                // load+terminate pair to transition to the new session.
                // No new ACP method needed — the bridge already has
                // LoadSession and TerminateSession primitives.
                deferred_commands.extend(dispatch_rewind_command(
                    response,
                    &self.session,
                    &mut self.ui_state,
                ));
                dispatch_command_executed(command, response, &mut self.ui_state);
            } else {
                dispatch_command_executed(command, response, &mut self.ui_state);

                // WORKAROUND(Kiro v1.28.0): Kiro doesn't send ConfigOptionUpdate for
                // model changes (QRK-004), so we extract the model from the /model
                // command response. When Kiro sends proper ConfigOptionUpdate
                // notifications, this block becomes dead code — remove it and rely
                // on the ConfigOptionsUpdated handler in UiState.apply_notification().
                if command == "model"
                    && let Some(model_id) = response
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

        self.redraw_needed = self.redraw_needed || session_changed || ui_changed || tracker_changed;
        deferred_commands
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
            Event::Paste(text) => {
                self.ui_state.insert_text(&text);
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
                    crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture,)
                } else {
                    crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture,)
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
                let scroll_consumed = self.ui_state.subagent_ui().focused_session_id().is_none()
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
            KeyCode::Enter => {
                // A confirmed trust tier (phase 2) returns the chosen option so
                // we can persist it across sessions to the active agent's config.
                if let Some(trust) = self.ui_state.approval_confirm() {
                    self.persist_trust_grant(&trust);
                }
            }
            KeyCode::Esc => self.ui_state.approval_cancel(),
            _ => {}
        }
    }

    /// Persist a granted trust tier to the active agent's config file so it
    /// survives across sessions. The session-scoped grant has already been sent;
    /// this write is non-fatal. Built-in agents and agents with no on-disk config
    /// are intentionally skipped (logged at debug); a genuine write/parse failure
    /// is surfaced to the user, since they explicitly asked to "always allow".
    fn persist_trust_grant(&mut self, trust: &cyril_core::types::TrustOption) {
        use cyril_core::kiro_agent_config::{TrustPersistError, persist_trust_grant};

        // Own the agent name so the immutable session borrow ends before we may
        // need `&mut self.ui_state` to report a failure below.
        let Some(agent) = self
            .session
            .current_mode_id()
            .map(|m| m.as_str().to_string())
        else {
            tracing::debug!("no active agent identity; trust grant not persisted");
            return;
        };
        match persist_trust_grant(&agent, &self.cwd, &trust.setting_key, &trust.patterns) {
            Ok(path) => {
                tracing::info!(path = %path.display(), "persisted trust grant across sessions")
            }
            Err(e @ (TrustPersistError::BuiltinAgent(_) | TrustPersistError::NoConfig(_))) => {
                // Expected for the default/built-in agents and ad-hoc agents
                // without a config file — session-scoped trust still applies.
                tracing::debug!(reason = %e, "trust grant not persisted");
            }
            Err(e) => {
                // A genuine persistence failure (write/parse/serialize/invalid
                // config). Don't let it vanish into the log — the user must learn
                // the grant won't survive the session.
                tracing::warn!(error = %e, "failed to persist trust grant");
                self.ui_state.add_system_message(format!(
                    "Trust applied for this session, but saving it across sessions failed: {e}"
                ));
            }
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
                    self.ui_state
                        .add_system_message("No active session — cannot refresh.".into());
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
                if let Some((command_name, value)) = self.ui_state.picker_confirm()
                    && let Some(session_id) = self.session.id()
                {
                    self.bridge_sender
                        .send(BridgeCommand::ExecuteCommand {
                            command: command_name,
                            session_id: session_id.clone(),
                            args: serde_json::json!({"value": value}),
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
                subagent_tracker: Some(self.ui_state.subagent_tracker()),
            };
            let command_name = cmd.name().to_string();
            let args = args.to_string();
            match cmd.execute(&ctx, &args).await {
                // /steer needs the async steer path (echo + SteerSession); route it
                // through the same dispatch_steer as Enter-while-busy.
                Ok(CommandResult {
                    kind: CommandResultKind::Steer { text },
                }) => {
                    return dispatch_steer(
                        &mut self.ui_state,
                        &self.session,
                        &self.bridge_sender,
                        text,
                    )
                    .await;
                }
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

        // Route by session state (K1b, cyril-bm1j): a busy turn steers instead of
        // firing a second SendPrompt the bridge would reject — the cyril-2vcc fix.
        // Prompt/NoSession fall through to the existing block (which handles the
        // no-session advisory itself).
        if classify_submit(self.session.status(), self.session.id().is_some()) == SubmitRoute::Steer
        {
            return dispatch_steer(&mut self.ui_state, &self.session, &self.bridge_sender, text)
                .await;
        }

        // Send as prompt (idle path, unchanged)
        let session_id = match self.session.id() {
            Some(id) => id.clone(),
            None => {
                self.ui_state
                    .add_system_message("No active session. Use /new to create one.".into());
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
                        content_blocks.push(format!("<file path=\"{path}\">\n{contents}\n</file>"));
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
            CommandResultKind::Steer { .. } => {
                // Routed in submit_input before reaching here (needs async
                // dispatch_steer). Reaching this arm is a routing bug.
                tracing::error!("Steer result reached handle_command_result — routing bug");
            }
            CommandResultKind::ToggleVoice => {
                self.toggle_voice();
            }
            CommandResultKind::Quit => {
                self.ui_state.request_quit();
            }
        }
        self.redraw_needed = true;
    }

    /// Await the next event from the voice engine, or never resolve when voice
    /// is disabled (the handle is `None`). Lets the `select!` arm stay cfg-free.
    async fn next_voice_event(
        voice: &mut Option<cyril_core::voice::VoiceHandle>,
    ) -> Option<VoiceEvent> {
        match voice {
            Some(handle) => handle.recv_event().await,
            None => std::future::pending().await,
        }
    }

    /// Apply a voice engine event to UI state (ROADMAP CN2).
    fn handle_voice_event(&mut self, event: VoiceEvent) {
        match event {
            VoiceEvent::Level(level) => self.ui_state.set_voice_level(level),
            VoiceEvent::Status(status) => self.ui_state.set_voice_status(status),
            // The payoff: a finished transcript drops into the input buffer.
            VoiceEvent::Transcript(text) => self.ui_state.insert_text(&text),
            VoiceEvent::Error(msg) => {
                // The engine bailed → it is no longer capturing. Clear intent so
                // the next /voice starts fresh.
                self.voice_active = false;
                self.ui_state.set_voice_status(VoiceStatus::Idle);
                self.ui_state
                    .add_system_message(format!("Voice error: {msg}"));
            }
        }
        // Note (V1b): when the engine gains engine-initiated stops (silence
        // timeout), do NOT naively reconcile `voice_active` from a `Status(Idle)`
        // event — a stale Idle from a completed Stop can arrive after a newer
        // Start and wedge the toggle. Tag commands/events with a generation, or
        // emit a distinct auto-stopped event, and reconcile on that.
        self.redraw_needed = true;
    }

    /// Toggle voice capture (the `/voice` command). Decides Start vs Stop from
    /// the authoritative `voice_active` intent (not the lagging UI projection),
    /// and reports gracefully if voice isn't compiled in or is backpressured.
    /// `redraw_needed` is set by the caller (`handle_command_result`).
    fn toggle_voice(&mut self) {
        let Some(handle) = self.voice.as_ref() else {
            self.ui_state.add_system_message(
                "Voice input isn't compiled in — rebuild with `--features voice`.".into(),
            );
            return;
        };
        let cmd = if self.voice_active {
            VoiceCommand::Stop
        } else {
            VoiceCommand::Start
        };
        match handle.try_send_command(cmd) {
            // Flip intent only on a successful send so it never drifts from
            // what the engine was actually told.
            Ok(()) => self.voice_active = !self.voice_active,
            Err(e) => {
                tracing::warn!(error = %e, "failed to send voice command");
                let detail = match e {
                    cyril_core::voice::VoiceError::Busy => "Voice engine busy — try again.",
                    cyril_core::voice::VoiceError::ChannelClosed => "Voice subsystem unavailable.",
                };
                self.ui_state.add_system_message(detail.into());
            }
        }
    }
}

/// Where a non-empty, non-command Enter submit should go (ROADMAP K1b, cyril-bm1j).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubmitRoute {
    /// Busy turn in flight → steer it mid-turn instead of starting a 2nd prompt.
    Steer,
    /// Idle/other state → send as a normal prompt (unchanged pre-K1b behavior).
    Prompt,
    /// No active session → advisory; nothing to prompt or steer.
    NoSession,
}

/// Classify a non-empty, non-command Enter submit. Pure decision (the CI-testable
/// seam behind `submit_input`): only `Busy` steers; everything else with a session
/// prompts; no session is advisory. `has_session` is checked first — you cannot
/// steer or prompt a session that does not exist.
///
/// Precondition (sanity-hint, caller-guaranteed): called only for non-empty,
/// non-command text — `submit_input` early-returns on empty and dispatches slash
/// commands before reaching here. The function ignores text content, so a
/// violation still yields a correct route; no runtime enforcement is needed.
fn classify_submit(status: &SessionStatus, has_session: bool) -> SubmitRoute {
    if !has_session {
        SubmitRoute::NoSession
    } else if matches!(status, SessionStatus::Busy) {
        SubmitRoute::Steer
    } else {
        SubmitRoute::Prompt
    }
}

/// Whether a steer can be delivered, or why not (ROADMAP K1b, cyril-bm1j).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SteerGate {
    Send,
    AdvisoryUnsupported,
    AdvisoryNoSession,
}

/// Decide whether a steer should be sent. `has_session` is checked BEFORE
/// `unsupported` — the message a user sees for "no session" must win over
/// "unsupported", and a steer needs a session id regardless. Pure (CI-testable).
fn steer_gate(unsupported: bool, has_session: bool) -> SteerGate {
    if !has_session {
        SteerGate::AdvisoryNoSession
    } else if unsupported {
        SteerGate::AdvisoryUnsupported
    } else {
        SteerGate::Send
    }
}

/// Dispatch a queue-steer: the single path shared by Enter-while-busy and the
/// `/steer` command (ROADMAP K1b, cyril-bm1j). Applies `steer_gate`, adds the
/// optimistic echo, and emits `SteerSession` — or an advisory when it can't.
/// Gating on `steering_unsupported()` is the keystone that keeps the optimistic
/// echo reconcilable: the bridge drops a steer on a known-unsupported session
/// silently (no notification), so we must not add a `Queued` echo that would
/// then never resolve.
///
/// Precondition (sanity-hint, caller-guaranteed): `text` is non-empty —
/// `submit_input` early-returns on empty input and `/steer` returns usage for an
/// empty arg. An empty steer would be a backend no-op, not wrong cyril output, so
/// a `debug_assert!` suffices (no release-time refusal needed).
async fn dispatch_steer(
    ui: &mut UiState,
    session: &SessionController,
    bridge: &BridgeSender,
    text: String,
) -> cyril_core::Result<()> {
    debug_assert!(
        !text.is_empty(),
        "dispatch_steer callers guarantee non-empty text"
    );
    match steer_gate(session.steering_unsupported(), session.id().is_some()) {
        SteerGate::Send => {
            // id() is Some — steer_gate just checked has_session.
            let Some(session_id) = session.id().cloned() else {
                return Ok(());
            };
            ui.add_steer_echo(&text);
            bridge
                .send(BridgeCommand::SteerSession {
                    session_id,
                    message: text,
                })
                .await?;
        }
        SteerGate::AdvisoryUnsupported => ui.add_system_message(
            "Steering isn't supported by this backend (needs kiro-cli 2.7.0+).".into(),
        ),
        SteerGate::AdvisoryNoSession => {
            ui.add_system_message("No active session — nothing to steer.".into())
        }
    }
    Ok(())
}

/// Produce a concise one-line summary from a (possibly multi-line) tool description.
///
/// Tool descriptions frequently begin with a leading newline and hard-wrap their
/// opening sentence across physical lines (e.g. the `subagent` tool's first line ends
/// mid-sentence at "Each stage runs as a"). Taking the first physical line therefore
/// truncates mid-sentence. Instead, take the first paragraph (up to a blank line),
/// collapse its internal whitespace, and return its first sentence.
fn summarize_description(desc: &str) -> String {
    // First paragraph: everything up to the first blank line.
    let first_para = desc.trim().split("\n\n").next().unwrap_or("").trim();
    // Collapse hard-wrapped newlines and runs of whitespace into single spaces.
    let collapsed = first_para.split_whitespace().collect::<Vec<_>>().join(" ");
    // Prefer the first sentence — the earliest sentence terminator followed by a
    // space — to keep rows short. Fall back to the whole collapsed paragraph when
    // there is no sentence boundary. `..=idx` is byte-safe: every terminator is
    // ASCII, so `idx` lands on a char boundary.
    let boundary = [". ", "? ", "! "]
        .into_iter()
        .filter_map(|term| collapsed.find(term))
        .min();
    match boundary {
        Some(idx) => collapsed[..=idx].to_string(),
        None => collapsed,
    }
}

/// Append per-file context items (indented) under a context-breakdown category.
///
/// Kiro's `/context` response nests an `items` array under categories like
/// `contextFiles`/`sessionFiles`, each item carrying `name`, `tokens`,
/// `percent`, `matched`, and an optional `auto_included` flag. Items are
/// rendered largest-first so the heaviest contributors surface at the top.
/// Categories without an `items` array (e.g. `tools`) are left untouched.
fn append_context_items(out: &mut String, category: &serde_json::Value) {
    let Some(items) = category.get("items").and_then(|i| i.as_array()) else {
        return;
    };

    // Sort by token count descending without mutating the source array.
    let mut sorted: Vec<&serde_json::Value> = items.iter().collect();
    sorted.sort_by(|a, b| {
        let at = a.get("tokens").and_then(|t| t.as_u64()).unwrap_or(0);
        let bt = b.get("tokens").and_then(|t| t.as_u64()).unwrap_or(0);
        bt.cmp(&at)
    });

    for item in sorted {
        let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("?");
        let tokens = item.get("tokens").and_then(|t| t.as_u64()).unwrap_or(0);
        let pct = item.get("percent").and_then(|p| p.as_f64()).unwrap_or(0.0);
        // Optional flags: surface only when they tell the user something useful.
        let auto = item
            .get("auto_included")
            .and_then(|a| a.as_bool())
            .unwrap_or(false);
        let matched = item
            .get("matched")
            .and_then(|m| m.as_bool())
            .unwrap_or(true);
        let mut tags = String::new();
        if auto {
            tags.push_str(" (auto)");
        }
        if !matched {
            tags.push_str(" (unmatched)");
        }
        out.push_str(&format!("    {name} — {tokens} tokens ({pct:.1}%){tags}\n"));
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
    if let Some(tools) = data.and_then(|d| d.get("tools")).and_then(|t| t.as_array()) {
        let mut out = format!("{message}\n\n");
        for tool in tools {
            let name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("?");
            let source = tool.get("source").and_then(|s| s.as_str()).unwrap_or("");
            let desc = summarize_description(
                tool.get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or(""),
            );
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
                let cat_pct = cat.get("percent").and_then(|p| p.as_f64()).unwrap_or(0.0);
                if tokens > 0 {
                    out.push_str(&format!("  {label}: {tokens} tokens ({cat_pct:.1}%)\n"));
                    append_context_items(&mut out, cat);
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
            let pct = bd.get("percentage").and_then(|p| p.as_u64()).unwrap_or(0);
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
) -> Vec<BridgeCommand> {
    if !is_success_response(response) {
        let text = format_command_response("code", response);
        ui_state.add_command_output("code".to_string(), text);
        return Vec::new();
    }

    match CodeCommandResponse::from_json(response) {
        CodeCommandResponse::Panel(data) => {
            ui_state.set_code_intelligence_active(data.status == LspStatus::Initialized);
            ui_state.show_code_panel(data);
            Vec::new()
        }
        CodeCommandResponse::Prompt { text, label } => {
            let session_id = match session.id().cloned() {
                Some(id) => id,
                None => {
                    tracing::warn!("/code prompt response arrived with no active session");
                    ui_state.add_system_message(
                        "/code: received prompt but no active session — try again.".into(),
                    );
                    return Vec::new();
                }
            };
            let display = label.as_deref().unwrap_or("Code Intelligence");
            ui_state.add_system_message(format!("/code: {display}"));
            ui_state.add_user_message(&text);
            ui_state.set_activity(Activity::Sending);

            vec![BridgeCommand::SendPrompt {
                session_id,
                content_blocks: vec![text],
            }]
        }
        CodeCommandResponse::Unknown(ref value) => {
            let text = format_command_response("code", value);
            ui_state.add_command_output("code".to_string(), text);
            Vec::new()
        }
    }
}

/// Dispatch a `/rewind` command response.
///
/// When the agent selects a new session (response carries
/// `data.switchSession: true` plus `data.sessionId`), emit the
/// `session/load` + `session/terminate` pair that client-orchestrates the
/// "fork" — Kiro doesn't have a `session/fork` method; the rewind primitive
/// is `commands/execute rewind {value: "<idx>"}` returning a new session id
/// that the client must explicitly load and switch from. See
/// `docs/cyril-acp-coverage-vs-2.4.1.md` for the wire trace.
///
/// Returns an empty vec for the no-args panel-data response (the panel is
/// rendered via `dispatch_command_executed`) and on any error case.
fn dispatch_rewind_command(
    response: &serde_json::Value,
    session: &cyril_core::session::SessionController,
    ui_state: &mut cyril_ui::state::UiState,
) -> Vec<BridgeCommand> {
    if !is_success_response(response) {
        return Vec::new();
    }
    let data = match response.get("data") {
        Some(d) => d,
        None => return Vec::new(),
    };
    let switch = data
        .get("switchSession")
        .and_then(|s| s.as_bool())
        .unwrap_or(false);
    if !switch {
        // Panel-data response (no selection yet). The `turns` payload is
        // rendered by the generic command-output path.
        return Vec::new();
    }
    let new_session_id = match data
        .get("sessionId")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
    {
        Some(id) => SessionId::new(id),
        None => {
            tracing::warn!("/rewind response had switchSession:true but no sessionId — skipping");
            return Vec::new();
        }
    };
    let old_session_id = match session.id().cloned() {
        Some(id) => id,
        None => {
            tracing::warn!(
                "/rewind switchSession response arrived with no active session — skipping"
            );
            return Vec::new();
        }
    };
    ui_state.add_system_message(format!(
        "/rewind: switched to new session {} (old session {} will be terminated)",
        new_session_id.as_str(),
        old_session_id.as_str()
    ));
    vec![
        BridgeCommand::LoadSession {
            session_id: new_session_id,
        },
        BridgeCommand::TerminateSession {
            session_id: old_session_id,
        },
    ]
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
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    // cyril-bm1j Slice 9 / claims C1, C2: submit routing truth table.
    #[test]
    fn classify_submit_truth_table() {
        // C1: busy + session -> Steer.
        assert_eq!(
            classify_submit(&SessionStatus::Busy, true),
            SubmitRoute::Steer
        );
        // C2: idle (Active) + session -> Prompt.
        assert_eq!(
            classify_submit(&SessionStatus::Active, true),
            SubmitRoute::Prompt
        );
        // No session -> NoSession.
        assert_eq!(
            classify_submit(&SessionStatus::Disconnected, false),
            SubmitRoute::NoSession
        );
        // Adversarial: busy but no session -> NoSession (no-session beats busy).
        assert_eq!(
            classify_submit(&SessionStatus::Busy, false),
            SubmitRoute::NoSession
        );
        // Only Busy steers — other present-session states prompt (unchanged path).
        assert_eq!(
            classify_submit(&SessionStatus::Compacting, true),
            SubmitRoute::Prompt
        );
        assert_eq!(
            classify_submit(&SessionStatus::Initializing, true),
            SubmitRoute::Prompt
        );
        // Error is the only data-carrying SessionStatus variant; the design lists
        // (Error,true)->Prompt. A future broadening of the steer predicate must not
        // silently route an errored session's Enter to a steer it can't accept.
        assert_eq!(
            classify_submit(
                &SessionStatus::Error {
                    message: "boom".into()
                },
                true
            ),
            SubmitRoute::Prompt
        );
    }

    // cyril-bm1j Slice 10 / claim C7: steer gate truth table.
    #[test]
    fn steer_gate_truth_table() {
        assert_eq!(steer_gate(false, true), SteerGate::Send);
        assert_eq!(steer_gate(true, true), SteerGate::AdvisoryUnsupported);
        assert_eq!(steer_gate(false, false), SteerGate::AdvisoryNoSession);
        // Adversarial: unsupported AND no session -> NoSession wins (checked first).
        assert_eq!(steer_gate(true, false), SteerGate::AdvisoryNoSession);
    }

    // cyril-bm1j Slice 11 / claims C1+C3+C7 integration + cyril-2vcc regression.
    #[tokio::test]
    async fn dispatch_steer_busy_sends_steer_and_echoes() {
        let (tx, mut rx) = mpsc::channel(8);
        let bridge = BridgeSender::from_sender(tx);
        let mut ui = UiState::new(500);
        let mut session = SessionController::new();
        session.set_session(SessionId::new("sess_1"), SessionStatus::Busy);

        dispatch_steer(&mut ui, &session, &bridge, "halt".into())
            .await
            .unwrap();

        // cyril-2vcc: a busy submit emits SteerSession, NOT a second SendPrompt
        // (which the bridge would reject -> the message would be lost).
        match rx.try_recv() {
            Ok(BridgeCommand::SteerSession { message, .. }) => assert_eq!(message, "halt"),
            other => panic!("expected SteerSession{{halt}}, got {other:?}"),
        }
        // Optimistic Queued echo present immediately.
        assert!(
            ui.messages().iter().any(|m| matches!(
                m.kind(),
                cyril_ui::traits::ChatMessageKind::SteerEcho {
                    text,
                    status: cyril_ui::traits::SteerEchoStatus::Queued,
                } if text == "halt"
            )),
            "expected a Queued steer echo for 'halt'"
        );
    }

    // cyril-bm1j Slice 11 / claim C7 keystone: unsupported -> no send, no echo.
    #[tokio::test]
    async fn dispatch_steer_unsupported_sends_nothing_no_echo() {
        let (tx, mut rx) = mpsc::channel(8);
        let bridge = BridgeSender::from_sender(tx);
        let mut ui = UiState::new(500);
        let mut session = SessionController::new();
        session.set_session(SessionId::new("sess_1"), SessionStatus::Busy);
        session.apply_notification(&Notification::SteeringUnsupported {
            message: "steering requires kiro-cli 2.7.0+".into(),
        });

        dispatch_steer(&mut ui, &session, &bridge, "halt".into())
            .await
            .unwrap();

        // Keystone: nothing sent on a known-unsupported session, so no optimistic
        // echo can ever get stuck (the bridge drops such steers silently).
        assert!(
            rx.try_recv().is_err(),
            "unsupported session must not send a SteerSession"
        );
        assert!(
            !ui.messages().iter().any(|m| matches!(
                m.kind(),
                cyril_ui::traits::ChatMessageKind::SteerEcho {
                    status: cyril_ui::traits::SteerEchoStatus::Queued,
                    ..
                }
            )),
            "no Queued echo on an unsupported session"
        );
        assert!(
            ui.messages()
                .iter()
                .any(|m| matches!(m.kind(), cyril_ui::traits::ChatMessageKind::System(_))),
            "an advisory system message is shown instead"
        );
    }

    #[test]
    fn format_response_tools_list() {
        let response = serde_json::json!({
            "success": true,
            "message": "Available tools:",
            "data": {
                "tools": [
                    {"name": "read", "description": "Read a file.\nMore details", "source": "built-in"},
                    {"name": "fetch", "description": "Fetch a URL", "source": "mcp-server"}
                ]
            }
        });
        let result = format_command_response("tools", &response);
        assert!(result.contains("Available tools:"));
        // First sentence only; the trailing line is dropped.
        assert!(result.contains("  read — Read a file.\n"));
        assert!(result.contains("  fetch — Fetch a URL (mcp-server)\n"));
    }

    #[test]
    fn summarize_description_joins_hard_wrapped_first_sentence() {
        // Mirrors the real `subagent` tool description, whose opening sentence is
        // hard-wrapped across physical lines and preceded by a leading newline.
        let desc = "\nSpawn and coordinate multiple AI agents in a pipeline (DAG). Each stage runs as a\npersistent session. Stages with no depends_on start immediately in parallel.\n\nMODES:\n- blocking";
        // Must not cut off mid-sentence at "Each stage runs as a".
        assert_eq!(
            summarize_description(desc),
            "Spawn and coordinate multiple AI agents in a pipeline (DAG)."
        );
    }

    #[test]
    fn summarize_description_no_sentence_boundary_returns_paragraph() {
        let desc = "Read a file\nMore details";
        assert_eq!(summarize_description(desc), "Read a file More details");
    }

    #[test]
    fn summarize_description_truncates_at_question_or_exclamation() {
        // A first sentence ending in '?' or '!' must truncate there — not fall
        // through and return the whole paragraph (the prior ". "-only split did).
        // Relevant for third-party (MCP) tool descriptions cyril doesn't control.
        assert_eq!(
            summarize_description("Need a file? Use the read tool."),
            "Need a file?"
        );
        assert_eq!(
            summarize_description("Run it! Then check the output."),
            "Run it!"
        );
        // The earliest terminator wins regardless of which kind it is.
        assert_eq!(summarize_description("Why? Because. More."), "Why?");
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
    fn format_response_context_breakdown_lists_files() {
        let response = serde_json::json!({
            "success": true,
            "message": "",
            "data": {
                "contextUsagePercentage": 7.6,
                "model": "auto",
                "breakdown": {
                    "contextFiles": {
                        "tokens": 8495,
                        "percent": 4.2,
                        "items": [
                            {"name": "AGENTS.md", "tokens": 1843, "percent": 0.92, "matched": true},
                            {"name": "review-process.md", "tokens": 5004, "percent": 2.5, "matched": true},
                            {"name": "SKILL.md", "tokens": 130, "percent": 0.06, "matched": true, "auto_included": true},
                            {"name": "stale.md", "tokens": 50, "percent": 0.02, "matched": false}
                        ]
                    },
                    "tools": {"tokens": 6665, "percent": 3.3}
                }
            }
        });
        let result = format_command_response("context", &response);
        // Category summary still present.
        assert!(result.contains("Context files: 8495 tokens (4.2%)"));
        // Per-file rows are rendered, indented under the category. The trailing
        // newline pins the exact row format (indent, em-dash, .1 precision) and
        // proves a plain matched row carries no stray (auto)/(unmatched) tag.
        assert!(result.contains("    AGENTS.md — 1843 tokens (0.9%)\n"));
        // Heaviest file sorts before lighter ones.
        let heavy = result.find("review-process.md").unwrap();
        let light = result.find("AGENTS.md").unwrap();
        assert!(heavy < light, "items should be sorted by tokens descending");
        // Optional flags surface useful state.
        assert!(result.contains("SKILL.md — 130 tokens (0.1%) (auto)"));
        assert!(result.contains("stale.md — 50 tokens (0.0%) (unmatched)"));
        // Categories without items (tools) render no child rows.
        assert!(result.contains("Tools: 6665 tokens (3.3%)"));
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
        assert!(result.is_empty());
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
    fn dispatch_code_panel_failed_resets_active_flag() {
        let session = code_session();
        let mut ui = UiState::new(500);
        ui.set_code_intelligence_active(true);
        assert!(ui.code_intelligence_active());

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
        assert!(
            !ui.code_intelligence_active(),
            "failed status should reset the flag"
        );
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
        assert!(result.is_empty());
        assert!(
            !ui.has_code_panel(),
            "panel should NOT open on success:false"
        );
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
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], BridgeCommand::SendPrompt { .. }));
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
        assert!(result.is_empty());
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
        assert!(
            has_default_label,
            "should use 'Code Intelligence' as default label"
        );
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
        assert!(result.is_empty());
        assert!(!ui.has_code_panel());
        assert!(!ui.messages().is_empty());
    }

    // --- dispatch_rewind_command tests ---
    //
    // /rewind selection orchestration: the agent's commands/execute response
    // carrying `switchSession: true` + a new sessionId must produce the
    // LoadSession + TerminateSession pair that client-orchestrates the fork.
    // See `docs/cyril-acp-coverage-vs-2.4.1.md` "TUI recorder findings" for
    // the empirically-captured wire sequence.

    fn rewind_session() -> cyril_core::session::SessionController {
        let mut session = cyril_core::session::SessionController::new();
        session.set_session(SessionId::new("old-session-uuid"), SessionStatus::Active);
        session
    }

    #[test]
    fn dispatch_rewind_panel_data_returns_empty() {
        // No-args rewind call returns the turn list — no switchSession means
        // no follow-up dispatch.
        let session = rewind_session();
        let mut ui = UiState::new(500);
        let result = dispatch_rewind_command(
            &serde_json::json!({
                "success": true,
                "data": {
                    "turns": [
                        {
                            "group": "2%",
                            "label": "Say hello",
                            "logIndex": 0,
                            "responseSnippet": "Hello."
                        }
                    ]
                }
            }),
            &session,
            &mut ui,
        );
        assert!(
            result.is_empty(),
            "panel data should produce no deferred commands"
        );
    }

    #[test]
    fn dispatch_rewind_switch_session_emits_load_and_terminate() {
        let session = rewind_session();
        let mut ui = UiState::new(500);
        let result = dispatch_rewind_command(
            &serde_json::json!({
                "success": true,
                "message": "Rewound to earlier turn (new session new-session-uuid)",
                "data": {
                    "sessionId": "new-session-uuid",
                    "switchSession": true
                }
            }),
            &session,
            &mut ui,
        );
        assert_eq!(
            result.len(),
            2,
            "should emit LoadSession + TerminateSession"
        );
        match &result[0] {
            BridgeCommand::LoadSession { session_id } => {
                assert_eq!(session_id.as_str(), "new-session-uuid");
            }
            other => panic!("expected LoadSession first, got {other:?}"),
        }
        match &result[1] {
            BridgeCommand::TerminateSession { session_id } => {
                assert_eq!(session_id.as_str(), "old-session-uuid");
            }
            other => panic!("expected TerminateSession second, got {other:?}"),
        }
        // System message announces the swap
        assert!(!ui.messages().is_empty());
    }

    #[test]
    fn dispatch_rewind_switch_session_without_sessionid_is_noop() {
        // Defensive: if Kiro signals switchSession but omits the new
        // sessionId, we can't orchestrate. Warn and return empty.
        let session = rewind_session();
        let mut ui = UiState::new(500);
        let result = dispatch_rewind_command(
            &serde_json::json!({
                "success": true,
                "data": { "switchSession": true }
            }),
            &session,
            &mut ui,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn dispatch_rewind_switch_session_without_active_session_is_noop() {
        // Defensive: switchSession with no current session ID — we can't
        // terminate "the old one" because there isn't one yet.
        let session = cyril_core::session::SessionController::new();
        let mut ui = UiState::new(500);
        let result = dispatch_rewind_command(
            &serde_json::json!({
                "success": true,
                "data": {
                    "sessionId": "new-session-uuid",
                    "switchSession": true
                }
            }),
            &session,
            &mut ui,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn dispatch_rewind_success_false_returns_empty() {
        let session = rewind_session();
        let mut ui = UiState::new(500);
        let result = dispatch_rewind_command(
            &serde_json::json!({
                "success": false,
                "message": "Cannot rewind beyond first turn"
            }),
            &session,
            &mut ui,
        );
        assert!(result.is_empty());
    }
}
