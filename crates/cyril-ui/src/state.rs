use std::collections::HashMap;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent};
use cyril_core::types::*;

use crate::file_completer::FileCompleter;
use crate::traits::*;

/// Result of handling a key event when autocomplete is active.
#[derive(Debug, PartialEq, Eq)]
pub enum AutocompleteAction {
    /// No autocomplete was active — caller should handle the key normally.
    NotActive,
    /// Key was consumed by autocomplete (navigation, dismiss). No further action needed.
    Consumed,
    /// A suggestion was accepted into the input. Caller should NOT submit.
    Accepted,
    /// A slash command suggestion was accepted AND should be submitted immediately.
    AcceptedAndSubmit,
}

pub struct UiState {
    // Chat
    messages: Vec<ChatMessage>,
    messages_version: u64,
    streaming_text: String,
    streaming_thought: Option<String>,

    // Tool calls
    active_tool_calls: Vec<TrackedToolCall>,
    tool_call_index: HashMap<ToolCallId, usize>,
    current_plan: Option<Plan>,

    // Input
    input_text: String,
    input_cursor: usize,

    // Autocomplete
    autocomplete_suggestions: Vec<Suggestion>,
    autocomplete_selected: Option<usize>,
    file_completer: Option<FileCompleter>,
    command_info: Vec<(String, Option<String>)>,

    // Session info (projected by App from SessionController)
    activity: Activity,
    activity_since: Option<Instant>,
    session_label: Option<String>,
    current_mode: Option<String>,
    current_model: Option<String>,
    context_usage: Option<f64>,
    credit_usage: Option<(f64, f64)>,
    last_turn: Option<cyril_core::types::TurnSummary>,
    session_cost: cyril_core::types::SessionCost,
    pending_tokens: Option<cyril_core::types::TokenCounts>,
    pending_metering: Option<cyril_core::types::TurnMetering>,

    // Subagent streams and tracker (private — mutated via delegating methods)
    subagents: crate::subagent_ui::SubagentUiState,
    subagent_tracker: cyril_core::subagent::SubagentTracker,

    // Overlays
    approval: Option<ApprovalState>,
    picker: Option<PickerState>,
    hooks_panel: Option<HooksPanelState>,
    code_panel: Option<cyril_core::types::CodePanelData>,

    // Session-projected flags
    code_intelligence_active: bool,

    // Chat scroll (None = follow/auto-scroll, Some(n) = n lines above bottom)
    chat_scroll_back: Option<usize>,

    // Terminal
    terminal_size: (u16, u16),
    mouse_captured: bool,
    quit_requested: bool,
    deep_idle: bool,

    // Config
    max_messages: usize,
}

impl TuiState for UiState {
    fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    fn streaming_text(&self) -> &str {
        &self.streaming_text
    }

    fn streaming_thought(&self) -> Option<&str> {
        self.streaming_thought.as_deref()
    }

    fn messages_version(&self) -> u64 {
        self.messages_version
    }

    fn active_tool_calls(&self) -> &[TrackedToolCall] {
        &self.active_tool_calls
    }

    fn current_plan(&self) -> Option<&Plan> {
        self.current_plan.as_ref()
    }

    fn input_text(&self) -> &str {
        &self.input_text
    }

    fn input_cursor(&self) -> usize {
        self.input_cursor
    }

    fn autocomplete_suggestions(&self) -> &[Suggestion] {
        &self.autocomplete_suggestions
    }

    fn autocomplete_selected(&self) -> Option<usize> {
        self.autocomplete_selected
    }

    fn activity(&self) -> Activity {
        self.activity
    }

    fn session_label(&self) -> Option<&str> {
        self.session_label.as_deref()
    }

    fn current_mode(&self) -> Option<&str> {
        self.current_mode.as_deref()
    }

    fn current_model(&self) -> Option<&str> {
        self.current_model.as_deref()
    }

    fn context_usage(&self) -> Option<f64> {
        self.context_usage
    }

    fn credit_usage(&self) -> Option<(f64, f64)> {
        self.credit_usage
    }

    fn last_turn(&self) -> Option<&cyril_core::types::TurnSummary> {
        self.last_turn.as_ref()
    }

    fn session_cost(&self) -> &cyril_core::types::SessionCost {
        &self.session_cost
    }

    fn approval(&self) -> Option<&ApprovalState> {
        self.approval.as_ref()
    }

    fn picker(&self) -> Option<&PickerState> {
        self.picker.as_ref()
    }

    fn hooks_panel(&self) -> Option<&HooksPanelState> {
        self.hooks_panel.as_ref()
    }

    fn code_panel(&self) -> Option<&cyril_core::types::CodePanelData> {
        self.code_panel.as_ref()
    }

    fn code_intelligence_active(&self) -> bool {
        self.code_intelligence_active
    }

    fn chat_scroll_back(&self) -> Option<usize> {
        self.chat_scroll_back
    }

    fn terminal_size(&self) -> (u16, u16) {
        self.terminal_size
    }

    fn mouse_captured(&self) -> bool {
        self.mouse_captured
    }

    fn should_quit(&self) -> bool {
        self.quit_requested
    }

    fn activity_elapsed(&self) -> Option<Duration> {
        self.activity_since.map(|since| since.elapsed())
    }

    fn is_deep_idle(&self) -> bool {
        self.deep_idle
    }

    fn subagent_tracker(&self) -> &cyril_core::subagent::SubagentTracker {
        &self.subagent_tracker
    }

    fn subagent_ui(&self) -> &crate::subagent_ui::SubagentUiState {
        &self.subagents
    }
}

impl UiState {
    pub fn new(max_messages: usize) -> Self {
        Self {
            messages: Vec::new(),
            messages_version: 0,
            streaming_text: String::new(),
            streaming_thought: None,
            active_tool_calls: Vec::new(),
            tool_call_index: HashMap::new(),
            current_plan: None,
            input_text: String::new(),
            input_cursor: 0,
            autocomplete_suggestions: Vec::new(),
            autocomplete_selected: None,
            file_completer: None,
            command_info: Vec::new(),
            activity: Activity::Idle,
            activity_since: None,
            session_label: None,
            current_mode: None,
            current_model: None,
            context_usage: None,
            credit_usage: None,
            last_turn: None,
            session_cost: cyril_core::types::SessionCost::new(),
            pending_tokens: None,
            pending_metering: None,
            subagents: crate::subagent_ui::SubagentUiState::new(),
            subagent_tracker: cyril_core::subagent::SubagentTracker::new(),
            approval: None,
            picker: None,
            hooks_panel: None,
            code_panel: None,
            code_intelligence_active: false,
            chat_scroll_back: None,
            terminal_size: (80, 24),
            mouse_captured: false,
            quit_requested: false,
            deep_idle: false,
            max_messages,
        }
    }

    /// Apply a notification from the bridge. Returns `true` if the UI state changed.
    pub fn apply_notification(&mut self, notification: &Notification) -> bool {
        match notification {
            Notification::AgentMessage(msg) => {
                if msg.is_streaming {
                    self.streaming_text.push_str(&msg.text);
                    self.set_activity(Activity::Streaming);
                } else {
                    self.streaming_text.push_str(&msg.text);
                    self.commit_streaming();
                    self.set_activity(Activity::Ready);
                }
                true
            }
            Notification::AgentThought(thought) => {
                self.streaming_thought = Some(thought.text.clone());
                true
            }
            Notification::ToolCallStarted(tc) => {
                // Flush any accumulated text before the tool call starts.
                // This prevents text before and after a tool call from
                // concatenating into one message.
                if !self.streaming_text.is_empty() {
                    let text = std::mem::take(&mut self.streaming_text);
                    self.messages.push(ChatMessage::agent_text(text));
                    self.messages_version += 1;
                }

                // Commit tool call directly to messages in chronological position.
                // This ensures tool calls stay between the text segments that
                // surround them, rather than moving to the end on TurnCompleted.
                let tracked = TrackedToolCall::new(tc.clone());
                let idx = self.messages.len();
                self.messages.push(ChatMessage::tool_call(tracked));
                self.tool_call_index.insert(tc.id().clone(), idx);
                self.messages_version += 1;

                // Also keep in active_tool_calls for the live display section
                self.active_tool_calls
                    .push(TrackedToolCall::new(tc.clone()));
                self.set_activity(Activity::ToolRunning);
                true
            }
            Notification::ToolCallUpdated(tc) => {
                // Update in active_tool_calls (for live display)
                for tracked in &mut self.active_tool_calls {
                    if tracked.id() == tc.id() {
                        tracked.update(tc);
                        break;
                    }
                }
                // Update in committed messages (for history)
                if let Some(&idx) = self.tool_call_index.get(tc.id()) {
                    if let Some(msg) = self.messages.get_mut(idx) {
                        if let ChatMessageKind::ToolCall(ref mut tracked) = msg.kind {
                            tracked.update(tc);
                        }
                    }
                }
                true
            }
            Notification::PlanUpdated(plan) => {
                self.current_plan = Some(plan.clone());
                true
            }
            Notification::MetadataUpdated {
                context_usage,
                metering,
                tokens,
            } => {
                self.context_usage = Some(context_usage.percentage());
                self.pending_tokens = tokens.clone();
                if let Some(m) = metering {
                    self.pending_metering = Some(m.clone());
                }
                true
            }
            Notification::TurnCompleted { stop_reason } => {
                self.commit_streaming();
                self.last_turn = Some(cyril_core::types::TurnSummary::new(
                    *stop_reason,
                    self.pending_tokens.take(),
                    self.pending_metering.take(),
                ));
                if let Some(m) = self.last_turn.as_ref().and_then(|t| t.metering()) {
                    self.session_cost.record_turn(m);
                }
                self.set_activity(Activity::Ready);
                true
            }
            Notification::BridgeDisconnected { reason } => {
                self.add_system_message(format!("Disconnected: {reason}"));
                self.last_turn = None;
                self.pending_tokens = None;
                self.pending_metering = None;
                self.set_activity(Activity::Idle);
                true
            }
            Notification::ModeChanged { mode_id } => {
                self.current_mode = Some(mode_id.clone());
                true
            }
            Notification::AgentSwitched { name, welcome, .. } => {
                self.current_mode = Some(name.clone());
                if let Some(msg) = welcome {
                    self.add_system_message(format!("Switched to {name}: {msg}"));
                } else {
                    self.add_system_message(format!("Switched to {name}"));
                }
                true
            }
            Notification::CompactionStatus { phase, summary } => {
                match phase {
                    cyril_core::types::CompactionPhase::Started => {
                        self.add_system_message("Compacting conversation context...".into());
                        self.set_activity(Activity::Streaming);
                    }
                    cyril_core::types::CompactionPhase::Completed => {
                        if let Some(s) = summary {
                            self.add_system_message(format!("Compaction completed: {s}"));
                        } else {
                            self.add_system_message("Compaction completed".into());
                        }
                        self.set_activity(Activity::Ready);
                    }
                    cyril_core::types::CompactionPhase::Failed { error } => {
                        if let Some(e) = error {
                            self.add_system_message(format!("Compaction failed: {e}"));
                        } else {
                            self.add_system_message("Compaction failed".into());
                        }
                        self.set_activity(Activity::Ready);
                    }
                }
                true
            }
            Notification::ClearStatus { message } => {
                self.add_system_message(format!("Clear: {message}"));
                true
            }
            Notification::RateLimited { message } => {
                self.add_system_message(format!("Rate limited: {message}"));
                true
            }
            Notification::SessionCreated {
                session_id,
                current_mode,
                current_model,
                welcome_message,
                ..
            } => {
                self.session_label = Some(session_id.as_str().to_string());
                self.current_mode = current_mode.clone();
                if let Some(model) = current_model {
                    self.current_model = Some(model.clone());
                }
                self.last_turn = None;
                self.pending_tokens = None;
                self.pending_metering = None;
                self.session_cost = cyril_core::types::SessionCost::new();
                if let Some(msg) = welcome_message {
                    self.add_system_message(msg.clone());
                } else {
                    self.add_system_message(format!("Session created: {}", session_id.as_str()));
                }
                self.set_activity(Activity::Ready);
                true
            }
            Notification::ToolCallChunk { .. } => {
                self.set_activity(Activity::ToolRunning);
                true
            }
            Notification::ConfigOptionsUpdated(options) => {
                if let Some(model_opt) = options.iter().find(|o| o.key == "model") {
                    self.current_model = model_opt.value.clone();
                    true
                } else {
                    false
                }
            }
            Notification::CommandsUpdated { .. } => {
                // Consumed by the App layer (registers in CommandRegistry).
                false
            }
            Notification::CommandOptionsReceived { .. } => {
                // Handled by the App layer (opens picker or shows message).
                false
            }
            Notification::CommandExecuted { .. } => {
                // Handled by the App layer (formats and displays the response).
                false
            }
            Notification::McpServerInitFailure { server_name, error } => {
                if let Some(err) = error {
                    self.add_system_message(
                        format!("MCP server '{server_name}' failed to initialize: {err}"),
                    );
                } else {
                    self.add_system_message(
                        format!("MCP server '{server_name}' failed to initialize"),
                    );
                }
                true
            }
            Notification::McpServerInitialized { server_name } => {
                self.add_system_message(format!("MCP server '{server_name}' ready"));
                true
            }
            Notification::McpOAuthRequest { .. } => {
                // Handled by App (cross-cutting concern: displays URL for manual browser opening)
                false
            }
            Notification::AgentNotFound { requested, fallback } => {
                if let Some(fb) = fallback {
                    self.add_system_message(
                        format!("Agent '{requested}' not found, using '{fb}'")
                    );
                } else {
                    self.add_system_message(format!("Agent '{requested}' not found"));
                }
                true
            }
            Notification::AgentConfigError { path, error } => {
                self.add_system_message(format!("Agent config error in {path}: {error}"));
                true
            }
            Notification::ModelNotFound { requested, fallback } => {
                if let Some(fb) = fallback {
                    self.add_system_message(
                        format!("Model '{requested}' not available, using '{fb}'")
                    );
                } else {
                    self.add_system_message(format!("Model '{requested}' not available"));
                }
                true
            }

            Notification::SessionsListed { .. } => {
                // Handled by the App layer (opens session picker).
                false
            }
            Notification::SettingsReceived { .. } => {
                // Not yet consumed — no handler in the App layer.
                false
            }
            Notification::UserMessage { text } => {
                self.messages.push(ChatMessage::user_text(text.clone()));
                self.messages_version += 1;
                self.enforce_message_limit();
                true
            }

            // Subagent list and inbox notifications are handled by SubagentTracker,
            // which is owned by UiState but updated separately via
            // apply_subagent_tracker_notification(). These variants are no-ops here.
            Notification::SubagentListUpdated { .. } | Notification::InboxNotification { .. } => {
                false
            }

            // Spawn/terminate/error notifications surface as system messages.
            Notification::SubagentSpawned { session_id, name } => {
                self.add_system_message(format!(
                    "Spawned subagent '{name}' ({})",
                    session_id.as_str()
                ));
                true
            }
            Notification::SubagentTerminated { session_id } => {
                // Try to resolve name via the tracker for a friendlier message.
                let name = self
                    .subagent_tracker
                    .get(session_id)
                    .map(|info| info.session_name().to_string());
                let msg = match name {
                    Some(n) => format!("Terminated subagent '{n}' ({})", session_id.as_str()),
                    None => format!("Terminated subagent ({})", session_id.as_str()),
                };
                self.add_system_message(msg);
                true
            }
            Notification::BridgeError { operation, message } => {
                self.add_system_message(format!("{operation} failed: {message}"));
                true
            }
        }
    }

    /// Flush remaining streaming text and clear active tool call display.
    /// Tool calls are already committed to messages in chronological position
    /// (done in ToolCallStarted handler), so we only flush trailing text here.
    pub fn commit_streaming(&mut self) {
        if !self.streaming_text.is_empty() {
            let text = std::mem::take(&mut self.streaming_text);
            self.messages.push(ChatMessage::agent_text(text));
            self.messages_version += 1;
        }

        // Clear active display — tool calls are already in messages
        self.active_tool_calls.clear();
        self.tool_call_index.clear();
        self.streaming_thought = None;
        self.enforce_message_limit();
    }

    /// Add a user message to the chat history.
    pub fn add_user_message(&mut self, text: &str) {
        self.messages.push(ChatMessage::user_text(text.to_string()));
        self.messages_version += 1;
        self.enforce_message_limit();
    }

    /// Add a system message to the chat history.
    pub fn add_system_message(&mut self, text: String) {
        self.messages.push(ChatMessage::system(text));
        self.messages_version += 1;
        self.enforce_message_limit();
    }

    /// Set the current model name (displayed in toolbar).
    pub fn set_current_model(&mut self, model: Option<String>) {
        self.current_model = model;
    }

    /// Add a command output message to the chat.
    pub fn add_command_output(&mut self, command: String, text: String) {
        self.messages
            .push(ChatMessage::command_output(command, text));
        self.messages_version += 1;
        self.enforce_message_limit();
    }

    /// Update the activity state and record when it changed.
    /// The timer starts when entering a busy state from idle/ready, and
    /// persists across busy→busy transitions (Sending→Streaming→ToolRunning)
    /// so it tracks total turn time. Cleared on Ready/Idle.
    pub fn set_activity(&mut self, activity: Activity) {
        if self.activity != activity {
            let was_busy = !matches!(
                self.activity,
                Activity::Idle | Activity::Ready
            );
            let is_busy = !matches!(activity, Activity::Idle | Activity::Ready);

            self.activity = activity;

            if !is_busy {
                self.activity_since = None;
            } else if !was_busy {
                self.activity_since = Some(Instant::now());
            }
            // busy→busy: keep existing timer

            self.deep_idle = false;
        }
    }

    /// Mark the state as deep idle (no repaints needed).
    pub fn set_deep_idle(&mut self, deep: bool) {
        self.deep_idle = deep;
    }

    /// Signal that the application should quit.
    pub fn request_quit(&mut self) {
        self.quit_requested = true;
    }

    /// Show an approval dialog from a permission request.
    pub fn show_approval(&mut self, request: PermissionRequest) {
        self.approval = Some(ApprovalState {
            tool_call: request.tool_call,
            message: request.message,
            options: request.options,
            selected: 0,
            responder: request.responder,
        });
    }

    /// Take the current input text, clearing the input buffer, cursor, and
    /// chat scroll offset (returns to follow mode so the agent's response
    /// is visible).
    pub fn take_input(&mut self) -> String {
        self.input_cursor = 0;
        self.autocomplete_suggestions.clear();
        self.autocomplete_selected = None;
        self.chat_scroll_back = None;
        std::mem::take(&mut self.input_text)
    }

    /// Update the terminal size.
    pub fn set_terminal_size(&mut self, w: u16, h: u16) {
        self.terminal_size = (w, h);
    }

    /// Set mouse capture state (used to sync with terminal on startup).
    pub fn set_mouse_captured(&mut self, captured: bool) {
        self.mouse_captured = captured;
    }

    /// Toggle mouse capture mode.
    pub fn toggle_mouse_capture(&mut self) {
        self.mouse_captured = !self.mouse_captured;
    }

    /// Clear all messages from the chat history.
    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.messages_version += 1;
    }

    /// Check if there is an active approval dialog.
    pub fn has_approval(&self) -> bool {
        self.approval.is_some()
    }

    /// Check if there is an active picker dialog.
    pub fn has_picker(&self) -> bool {
        self.picker.is_some()
    }

    /// Handle a key event for the input field.
    pub fn handle_input_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        let mut text_changed = false;

        match key.code {
            KeyCode::Char(c) => {
                self.input_text.insert(self.input_cursor, c);
                self.input_cursor += c.len_utf8();
                text_changed = true;
            }
            KeyCode::Backspace => {
                if self.input_cursor > 0 {
                    // Find the previous character boundary
                    let prev = self.input_text[..self.input_cursor]
                        .char_indices()
                        .next_back()
                        .map(|(idx, _)| idx)
                        .unwrap_or(0);
                    self.input_text.drain(prev..self.input_cursor);
                    self.input_cursor = prev;
                    text_changed = true;
                }
            }
            KeyCode::Delete => {
                if self.input_cursor < self.input_text.len() {
                    let next = self.input_text[self.input_cursor..]
                        .char_indices()
                        .nth(1)
                        .map(|(idx, _)| self.input_cursor + idx)
                        .unwrap_or(self.input_text.len());
                    self.input_text.drain(self.input_cursor..next);
                    text_changed = true;
                }
            }
            KeyCode::Left => {
                if self.input_cursor > 0 {
                    self.input_cursor = self.input_text[..self.input_cursor]
                        .char_indices()
                        .next_back()
                        .map(|(idx, _)| idx)
                        .unwrap_or(0);
                }
            }
            KeyCode::Right => {
                if self.input_cursor < self.input_text.len() {
                    self.input_cursor = self.input_text[self.input_cursor..]
                        .char_indices()
                        .nth(1)
                        .map(|(idx, _)| self.input_cursor + idx)
                        .unwrap_or(self.input_text.len());
                }
            }
            KeyCode::Home => {
                self.input_cursor = 0;
            }
            KeyCode::End => {
                self.input_cursor = self.input_text.len();
            }
            _ => {}
        }

        if text_changed {
            self.update_autocomplete();
        }
    }

    // --- File completer and autocomplete ---

    /// Set the file completer for @-file autocomplete.
    pub fn set_file_completer(&mut self, completer: FileCompleter) {
        self.file_completer = Some(completer);
    }

    /// Get a reference to the file completer, if loaded.
    pub fn file_completer(&self) -> Option<&FileCompleter> {
        self.file_completer.as_ref()
    }

    /// Command info tuples `(name, description)` available for slash autocomplete.
    /// Names are stored without the leading `/`.
    pub fn set_command_info(&mut self, mut info: Vec<(String, Option<String>)>) {
        info.sort_by(|(a, _), (b, _)| a.to_lowercase().cmp(&b.to_lowercase()));
        self.command_info = info;
    }

    /// Read-only access to the subagent tracker.
    pub fn subagent_tracker(&self) -> &cyril_core::subagent::SubagentTracker {
        &self.subagent_tracker
    }

    /// Apply a notification to the subagent tracker. Returns true if tracker state changed.
    pub fn apply_subagent_tracker_notification(
        &mut self,
        notification: &Notification,
    ) -> bool {
        self.subagent_tracker.apply_notification(notification)
    }

    /// Read-only access to subagent UI state.
    pub fn subagent_ui(&self) -> &crate::subagent_ui::SubagentUiState {
        &self.subagents
    }

    /// Route a notification to the per-subagent stream identified by `session_id`.
    /// Creates the stream on first contact.
    pub fn apply_subagent_notification(
        &mut self,
        session_id: &SessionId,
        notification: &Notification,
    ) -> bool {
        self.subagents.apply_notification(session_id, notification)
    }

    /// Apply a list update to subagent streams — marks terminated streams
    /// that are no longer in the active list.
    pub fn apply_subagent_list_update(
        &mut self,
        subagents: &[cyril_core::types::SubagentInfo],
    ) -> bool {
        self.subagents.apply_list_update(subagents)
    }

    /// Focus a subagent for drill-in rendering. Returns true if the
    /// session has a stream and focus was set.
    pub fn focus_subagent(&mut self, session_id: SessionId) -> bool {
        self.subagents.focus(session_id)
    }

    /// Exit drill-in mode.
    pub fn unfocus_subagent(&mut self) {
        self.subagents.unfocus();
    }

    /// True if any subagent stream is actively streaming or running tools.
    pub fn any_subagent_active(&self) -> bool {
        self.subagents.any_active()
    }

    /// Recompute autocomplete suggestions based on current input text.
    fn update_autocomplete(&mut self) {
        let text = &self.input_text;
        let trimmed = text.trim();

        // Slash command autocomplete
        if trimmed.starts_with('/') && !trimmed.contains(' ') {
            let query = trimmed[1..].to_lowercase();
            self.autocomplete_suggestions = self
                .command_info
                .iter()
                .filter(|(name, _)| name.to_lowercase().starts_with(&query))
                .map(|(name, desc)| Suggestion {
                    text: format!("/{name}"),
                    description: desc.clone(),
                })
                .collect();
            self.autocomplete_selected = if self.autocomplete_suggestions.is_empty() {
                None
            } else {
                Some(0)
            };
            return;
        }

        // File autocomplete — look for @ trigger
        if let Some(at_pos) = text[..self.input_cursor].rfind('@') {
            let query = &text[at_pos + 1..self.input_cursor];
            if !query.is_empty()
                && !query.contains(' ')
                && let Some(ref completer) = self.file_completer
            {
                let suggestions: Vec<Suggestion> = completer
                    .suggest(query, 10)
                    .into_iter()
                    .map(|path| Suggestion {
                        text: format!("@{path}"),
                        description: None,
                    })
                    .collect();
                if !suggestions.is_empty() {
                    self.autocomplete_suggestions = suggestions;
                    self.autocomplete_selected = Some(0);
                    return;
                }
            }
        }

        // No autocomplete
        self.autocomplete_suggestions.clear();
        self.autocomplete_selected = None;
    }

    /// Accept the currently selected autocomplete suggestion.
    /// Returns true if a suggestion was accepted.
    pub fn accept_autocomplete(&mut self) -> bool {
        let selected = match self.autocomplete_selected {
            Some(idx) => idx,
            None => return false,
        };
        let suggestion = match self.autocomplete_suggestions.get(selected) {
            Some(s) => s.text.clone(),
            None => return false,
        };

        // For slash commands, replace the entire input
        if suggestion.starts_with('/') {
            self.input_text = format!("{suggestion} ");
            self.input_cursor = self.input_text.len();
        }
        // For @file references, replace from the @ to the cursor
        else if suggestion.starts_with('@')
            && let Some(at_pos) = self.input_text[..self.input_cursor].rfind('@')
        {
            let after_cursor = self.input_text[self.input_cursor..].to_string();
            self.input_text = format!("{}{suggestion} {after_cursor}", &self.input_text[..at_pos]);
            self.input_cursor = at_pos + suggestion.len() + 1; // +1 for space
        }

        self.autocomplete_suggestions.clear();
        self.autocomplete_selected = None;
        true
    }

    /// Move autocomplete selection to the previous item.
    pub fn autocomplete_prev(&mut self) {
        if let Some(ref mut idx) = self.autocomplete_selected
            && *idx > 0
        {
            *idx -= 1;
        }
    }

    /// Move autocomplete selection to the next item.
    pub fn autocomplete_next(&mut self) {
        if let Some(ref mut idx) = self.autocomplete_selected
            && *idx + 1 < self.autocomplete_suggestions.len()
        {
            *idx += 1;
        }
    }

    /// Dismiss the autocomplete menu.
    pub fn dismiss_autocomplete(&mut self) {
        self.autocomplete_suggestions.clear();
        self.autocomplete_selected = None;
    }

    /// Handle a key event when autocomplete is active (Layer 2.5).
    /// Returns an action telling the caller what to do next.
    ///
    /// This is the single authority for autocomplete key handling — the caller
    /// should NOT inspect autocomplete state or make decisions about it.
    pub fn handle_autocomplete_key(&mut self, key: KeyEvent) -> AutocompleteAction {
        if self.autocomplete_suggestions.is_empty() {
            return AutocompleteAction::NotActive;
        }

        match key.code {
            KeyCode::Tab => {
                self.accept_autocomplete();
                AutocompleteAction::Accepted
            }
            KeyCode::Up => {
                self.autocomplete_prev();
                AutocompleteAction::Consumed
            }
            KeyCode::Down => {
                self.autocomplete_next();
                AutocompleteAction::Consumed
            }
            KeyCode::Esc => {
                self.dismiss_autocomplete();
                AutocompleteAction::Consumed
            }
            KeyCode::Enter => {
                let is_slash = self
                    .autocomplete_suggestions
                    .first()
                    .is_some_and(|s| s.text.starts_with('/'));
                self.accept_autocomplete();
                if is_slash {
                    AutocompleteAction::AcceptedAndSubmit
                } else {
                    AutocompleteAction::Accepted
                }
            }
            _ => {
                // Any other key dismisses autocomplete and passes through to normal input
                self.dismiss_autocomplete();
                AutocompleteAction::NotActive
            }
        }
    }

    // --- Approval dialog methods ---

    /// Move approval selection to the previous option.
    pub fn approval_select_prev(&mut self) {
        if let Some(ref mut approval) = self.approval
            && approval.selected > 0
        {
            approval.selected -= 1;
        }
    }

    /// Move approval selection to the next option.
    pub fn approval_select_next(&mut self) {
        if let Some(ref mut approval) = self.approval
            && approval.selected + 1 < approval.options.len()
        {
            approval.selected += 1;
        }
    }

    /// Confirm the current approval selection, sending the response.
    pub fn approval_confirm(&mut self) {
        if let Some(approval) = self.approval.take() {
            let response = match approval.selected {
                0 => PermissionResponse::AllowOnce,
                1 => PermissionResponse::AllowAlways,
                _ => PermissionResponse::Reject,
            };
            // Ignore send error — the bridge may have dropped the receiver
            let _ = approval.responder.send(response);
        }
    }

    /// Cancel the approval dialog, sending a Cancel response.
    pub fn approval_cancel(&mut self) {
        if let Some(approval) = self.approval.take() {
            // Ignore send error — the bridge may have dropped the receiver
            let _ = approval.responder.send(PermissionResponse::Cancel);
        }
    }

    // --- Picker dialog methods ---

    /// Show a picker dialog with the given title and options.
    pub fn show_picker(&mut self, title: String, options: Vec<CommandOption>) {
        let filtered_indices: Vec<usize> = (0..options.len()).collect();
        self.picker = Some(PickerState {
            title,
            options,
            filter: String::new(),
            filtered_indices,
            selected: 0,
        });
    }

    /// Get the picker title, if a picker is active.
    pub fn picker_title(&self) -> Option<&str> {
        self.picker.as_ref().map(|p| p.title.as_str())
    }

    /// Move picker selection to the previous option.
    pub fn picker_select_prev(&mut self) {
        if let Some(ref mut picker) = self.picker
            && picker.selected > 0
        {
            picker.selected -= 1;
        }
    }

    /// Move picker selection to the next option.
    pub fn picker_select_next(&mut self) {
        if let Some(ref mut picker) = self.picker
            && !picker.filtered_indices.is_empty()
            && picker.selected + 1 < picker.filtered_indices.len()
        {
            picker.selected += 1;
        }
    }

    /// Confirm the picker selection. Returns the selected value if any.
    /// Confirm the picker selection and close the dialog.
    /// Returns (command_name, selected_value) — both are needed by the caller
    /// to construct the bridge command. Returns None if nothing was selected.
    pub fn picker_confirm(&mut self) -> Option<(String, String)> {
        let picker = self.picker.take()?;
        let idx = picker.filtered_indices.get(picker.selected).copied()?;
        let value = picker.options.get(idx)?.value.clone();
        Some((picker.title.clone(), value))
    }

    /// Cancel and close the picker dialog.
    pub fn picker_cancel(&mut self) {
        self.picker = None;
    }

    /// Type a character into the picker filter.
    pub fn picker_type_char(&mut self, c: char) {
        if let Some(ref mut picker) = self.picker {
            picker.filter.push(c);
            Self::refilter_picker(picker);
        }
    }

    /// Delete the last character from the picker filter.
    pub fn picker_backspace(&mut self) {
        if let Some(ref mut picker) = self.picker {
            picker.filter.pop();
            Self::refilter_picker(picker);
        }
    }

    /// Re-compute filtered indices after filter text changes.
    fn refilter_picker(picker: &mut PickerState) {
        let filter_lower = picker.filter.to_lowercase();
        picker.filtered_indices = picker
            .options
            .iter()
            .enumerate()
            .filter(|(_, opt)| {
                filter_lower.is_empty()
                    || opt.label.to_lowercase().contains(&filter_lower)
                    || opt.value.to_lowercase().contains(&filter_lower)
            })
            .map(|(i, _)| i)
            .collect();
        // Clamp selected index
        if picker.filtered_indices.is_empty() {
            picker.selected = 0;
        } else if picker.selected >= picker.filtered_indices.len() {
            picker.selected = picker.filtered_indices.len() - 1;
        }
    }

    // --- Hooks panel methods ---

    /// Open the hooks panel overlay with a list of hooks from the `hooks` command.
    ///
    /// Hooks are sorted on insert by `(trigger, command)` so the widget can
    /// iterate `state.hooks` directly without re-sorting on every render
    /// frame. The stored order is the panel's display order — callers that
    /// need the original wire order should keep their own copy.
    pub fn show_hooks_panel(&mut self, mut hooks: Vec<HookInfo>) {
        hooks.sort_by(|a, b| {
            a.trigger
                .cmp(&b.trigger)
                .then_with(|| a.command.cmp(&b.command))
        });
        self.hooks_panel = Some(HooksPanelState {
            hooks,
            scroll_offset: 0,
        });
    }

    /// Close the hooks panel overlay.
    pub fn hide_hooks_panel(&mut self) {
        self.hooks_panel = None;
    }

    /// Check if the hooks panel is currently visible.
    pub fn has_hooks_panel(&self) -> bool {
        self.hooks_panel.is_some()
    }

    /// Scroll the hooks panel up by `lines`. Saturates at 0.
    pub fn hooks_panel_scroll_up(&mut self, lines: usize) {
        if let Some(panel) = self.hooks_panel.as_mut() {
            panel.scroll_offset = panel.scroll_offset.saturating_sub(lines);
        }
    }

    /// Scroll the hooks panel down by `lines`. Saturates at `hooks.len() - 1`
    /// so `scroll_offset` never exceeds the last hook's index. This is a
    /// strict clamp on the index, not a viewport-aware bound — with a tall
    /// panel and a short list, the last item can end up alone at the top of
    /// the visible area with blank rows below it. Matches `PickerState`'s
    /// scroll convention in this codebase; viewport-aware clamping would
    /// require threading `visible_rows` through from the renderer.
    pub fn hooks_panel_scroll_down(&mut self, lines: usize) {
        if let Some(panel) = self.hooks_panel.as_mut() {
            let max = panel.hooks.len().saturating_sub(1);
            panel.scroll_offset = (panel.scroll_offset + lines).min(max);
        }
    }

    // --- Code panel ---

    pub fn show_code_panel(&mut self, data: cyril_core::types::CodePanelData) {
        self.code_panel = Some(data);
    }

    pub fn close_code_panel(&mut self) {
        self.code_panel = None;
    }

    pub fn has_code_panel(&self) -> bool {
        self.code_panel.is_some()
    }

    pub fn set_code_intelligence_active(&mut self, active: bool) {
        self.code_intelligence_active = active;
    }

    // --- Chat scroll ---

    /// Scroll chat up by `lines`. Enters browse mode from follow mode,
    /// or scrolls further up if already browsing.
    pub fn chat_scroll_up(&mut self, lines: usize) {
        self.chat_scroll_back = Some(
            self.chat_scroll_back.unwrap_or(0).saturating_add(lines),
        );
    }

    /// Scroll chat down by `lines`. Returns to follow mode when offset
    /// reaches zero.
    pub fn chat_scroll_down(&mut self, lines: usize) {
        match self.chat_scroll_back {
            None => {}
            Some(n) if n <= lines => {
                self.chat_scroll_back = None;
            }
            Some(n) => {
                self.chat_scroll_back = Some(n - lines);
            }
        }
    }

    /// Return to follow mode (snap to bottom).
    pub fn chat_scroll_reset(&mut self) {
        self.chat_scroll_back = None;
    }

    /// No-op stub — streaming text is committed directly in
    /// `apply_notification`, so no timeout-based buffer flush is needed.
    /// Returns `false` unconditionally.
    pub fn flush_stream_buffer(&mut self) -> bool {
        false
    }

    /// Trim oldest messages to stay within the configured limit.
    fn enforce_message_limit(&mut self) {
        if self.messages.len() > self.max_messages {
            let excess = self.messages.len() - self.max_messages;
            self.messages.drain(..excess);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_is_empty() {
        let state = UiState::new(500);
        assert!(state.messages().is_empty());
        assert_eq!(state.messages_version(), 0);
        assert_eq!(state.streaming_text(), "");
        assert_eq!(state.activity(), Activity::Idle);
        assert!(!state.should_quit());
    }

    #[test]
    fn apply_agent_message_streams() {
        let mut state = UiState::new(500);
        let changed = state.apply_notification(&Notification::AgentMessage(AgentMessage {
            text: "hello ".into(),
            is_streaming: true,
        }));
        assert!(changed);
        assert_eq!(state.streaming_text(), "hello ");
        assert_eq!(state.activity(), Activity::Streaming);
    }

    #[test]
    fn apply_turn_completed_commits() {
        let mut state = UiState::new(500);
        state.apply_notification(&Notification::AgentMessage(AgentMessage {
            text: "response".into(),
            is_streaming: true,
        }));
        state.apply_notification(&Notification::TurnCompleted {
            stop_reason: cyril_core::types::StopReason::EndTurn,
        });

        assert_eq!(state.streaming_text(), "");
        assert_eq!(state.messages().len(), 1);
        assert!(
            matches!(state.messages()[0].kind(), ChatMessageKind::AgentText(t) if t == "response")
        );
        assert_eq!(state.messages_version(), 1);
        assert_eq!(state.activity(), Activity::Ready);
    }

    #[test]
    fn apply_tool_call_started() {
        let mut state = UiState::new(500);
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "read".into(),
            ToolKind::Read,
            ToolCallStatus::InProgress,
            None,
        );
        let changed = state.apply_notification(&Notification::ToolCallStarted(tc));
        assert!(changed);
        assert_eq!(state.active_tool_calls().len(), 1);
        assert_eq!(state.activity(), Activity::ToolRunning);
    }

    #[test]
    fn apply_tool_call_updated() {
        let mut state = UiState::new(500);
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "read".into(),
            ToolKind::Read,
            ToolCallStatus::InProgress,
            None,
        );
        state.apply_notification(&Notification::ToolCallStarted(tc));

        let updated = ToolCall::new(
            ToolCallId::new("tc_1"),
            "Reading src/main.rs".into(),
            ToolKind::Read,
            ToolCallStatus::Completed,
            None,
        );
        state.apply_notification(&Notification::ToolCallUpdated(updated));

        assert_eq!(state.active_tool_calls()[0].title(), "Reading src/main.rs");
    }

    // --- Turn lifecycle tests ---
    // These test realistic sequences of notifications, not just individual events.

    #[test]
    fn turn_with_text_and_tool_calls_commits_both() {
        let mut state = UiState::new(500);

        // Simulate: agent streams text, starts a tool call, completes it, streams more text
        state.apply_notification(&Notification::AgentMessage(AgentMessage {
            text: "Let me read that file.".into(),
            is_streaming: true,
        }));
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "Reading main.rs".into(),
            ToolKind::Read,
            ToolCallStatus::InProgress,
            None,
        );
        state.apply_notification(&Notification::ToolCallStarted(tc));

        assert_eq!(state.active_tool_calls().len(), 1);
        // Text is flushed to messages when tool call starts, and tool call is
        // committed immediately in chronological position
        assert_eq!(state.streaming_text(), "");
        assert_eq!(state.messages().len(), 2, "text + tool call should be committed immediately");

        // Turn completes
        state.apply_notification(&Notification::TurnCompleted {
            stop_reason: cyril_core::types::StopReason::EndTurn,
        });

        // Both text and tool call should be in committed messages
        assert!(state.active_tool_calls().is_empty(), "active tool calls should be cleared");
        assert_eq!(state.streaming_text(), "", "streaming text should be cleared");

        let messages = state.messages();
        assert!(messages.len() >= 2, "should have text + tool call messages, got {}", messages.len());

        let has_agent_text = messages.iter().any(|m| matches!(m.kind(), ChatMessageKind::AgentText(_)));
        let has_tool_call = messages.iter().any(|m| matches!(m.kind(), ChatMessageKind::ToolCall(_)));
        assert!(has_agent_text, "committed messages should include agent text");
        assert!(has_tool_call, "committed messages should include tool call");
    }

    #[test]
    fn turn_with_diff_content_preserves_diff_in_history() {
        use cyril_core::types::{ToolCallContent, ToolCallLocation};

        let mut state = UiState::new(500);

        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "Editing main.rs".into(),
            ToolKind::Write,
            ToolCallStatus::Completed,
            None,
        )
        .with_content(vec![ToolCallContent::Diff {
            path: "src/main.rs".into(),
            old_text: Some("fn main() {}".into()),
            new_text: "fn main() {\n    println!(\"hello\");\n}".into(),
        }])
        .with_locations(vec![ToolCallLocation {
            path: "src/main.rs".into(),
            line: Some(1),
        }]);

        state.apply_notification(&Notification::ToolCallStarted(tc));
        state.apply_notification(&Notification::TurnCompleted {
            stop_reason: cyril_core::types::StopReason::EndTurn,
        });

        // The tool call should be committed with its diff content intact
        let messages = state.messages();
        let tc_msg = messages.iter().find(|m| matches!(m.kind(), ChatMessageKind::ToolCall(_)));
        assert!(tc_msg.is_some(), "tool call should be in committed messages");

        if let ChatMessageKind::ToolCall(tracked) = tc_msg.unwrap().kind() {
            assert!(!tracked.content().is_empty(), "diff content should be preserved");
            assert!(!tracked.locations().is_empty(), "locations should be preserved");
            assert_eq!(tracked.primary_path(), Some("src/main.rs"));
        } else {
            panic!("expected ToolCall message kind");
        }
    }

    #[test]
    fn multiple_turns_preserve_all_content() {
        let mut state = UiState::new(500);

        // Turn 1: text only
        state.apply_notification(&Notification::AgentMessage(AgentMessage {
            text: "First response.".into(),
            is_streaming: true,
        }));
        state.apply_notification(&Notification::TurnCompleted {
            stop_reason: cyril_core::types::StopReason::EndTurn,
        });

        // Turn 2: text + tool call
        state.apply_notification(&Notification::AgentMessage(AgentMessage {
            text: "Second response.".into(),
            is_streaming: true,
        }));
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "read".into(),
            ToolKind::Read,
            ToolCallStatus::Completed,
            None,
        );
        state.apply_notification(&Notification::ToolCallStarted(tc));
        state.apply_notification(&Notification::TurnCompleted {
            stop_reason: cyril_core::types::StopReason::EndTurn,
        });

        // Both turns should be in history
        let messages = state.messages();
        let agent_texts: Vec<_> = messages
            .iter()
            .filter(|m| matches!(m.kind(), ChatMessageKind::AgentText(_)))
            .collect();
        let tool_calls: Vec<_> = messages
            .iter()
            .filter(|m| matches!(m.kind(), ChatMessageKind::ToolCall(_)))
            .collect();

        assert_eq!(agent_texts.len(), 2, "both turns should have agent text");
        assert_eq!(tool_calls.len(), 1, "second turn should have tool call");
    }

    #[test]
    fn turn_with_only_tool_calls_no_text() {
        let mut state = UiState::new(500);

        // Agent does tool calls without streaming any text
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "Reading config".into(),
            ToolKind::Read,
            ToolCallStatus::Completed,
            None,
        );
        state.apply_notification(&Notification::ToolCallStarted(tc));
        state.apply_notification(&Notification::TurnCompleted {
            stop_reason: cyril_core::types::StopReason::EndTurn,
        });

        let messages = state.messages();
        assert_eq!(messages.len(), 1, "tool call should be committed even with no text");
        assert!(matches!(messages[0].kind(), ChatMessageKind::ToolCall(_)));
    }

    #[test]
    fn message_limit_enforced() {
        let mut state = UiState::new(3);
        for i in 0..5 {
            state.add_user_message(&format!("msg {i}"));
        }
        assert_eq!(state.messages().len(), 3);
        // Oldest messages removed
        assert!(matches!(state.messages()[0].kind(), ChatMessageKind::UserText(t) if t == "msg 2"));
    }

    #[test]
    fn add_system_message() {
        let mut state = UiState::new(500);
        state.add_system_message("Welcome".into());
        assert_eq!(state.messages().len(), 1);
        assert!(matches!(state.messages()[0].kind(), ChatMessageKind::System(t) if t == "Welcome"));
    }

    #[test]
    fn take_input_clears() {
        let mut state = UiState::new(500);
        state.input_text = "hello".into();
        state.input_cursor = 5;
        let text = state.take_input();
        assert_eq!(text, "hello");
        assert_eq!(state.input_text(), "");
        assert_eq!(state.input_cursor(), 0);
    }

    #[test]
    fn request_quit() {
        let mut state = UiState::new(500);
        assert!(!state.should_quit());
        state.request_quit();
        assert!(state.should_quit());
    }

    #[test]
    fn text_before_tool_call_commits_separately() {
        // Simulates: agent says "I'll edit that" → starts tool call → says "Done editing"
        // Text should NOT concatenate into "I'll edit thatDone editing"
        let mut state = UiState::new(500);

        // Agent streams text before tool call
        state.apply_notification(&Notification::AgentMessage(AgentMessage {
            text: "I'll edit that file.".into(),
            is_streaming: true,
        }));
        assert_eq!(state.streaming_text(), "I'll edit that file.");

        // Tool call starts — should flush text to messages
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "write".into(),
            ToolKind::Write,
            ToolCallStatus::InProgress,
            None,
        );
        state.apply_notification(&Notification::ToolCallStarted(tc));

        // Text and tool call should both be committed in order
        assert_eq!(state.streaming_text(), "", "streaming text should be flushed");
        assert_eq!(state.messages().len(), 2, "text + tool call committed");
        assert!(
            matches!(state.messages()[0].kind(), ChatMessageKind::AgentText(t) if t == "I'll edit that file."),
            "first message should be the pre-tool-call text"
        );
        assert!(
            matches!(state.messages()[1].kind(), ChatMessageKind::ToolCall(_)),
            "second message should be the tool call"
        );

        // Agent streams text after tool call
        state.apply_notification(&Notification::AgentMessage(AgentMessage {
            text: "Done editing.".into(),
            is_streaming: true,
        }));

        // Post-tool-call text should be separate from pre-tool-call text
        assert_eq!(state.streaming_text(), "Done editing.");

        // Turn completes
        state.apply_notification(&Notification::TurnCompleted {
            stop_reason: cyril_core::types::StopReason::EndTurn,
        });

        // Should have: text, tool call, text — in chronological order
        let messages = state.messages();
        assert_eq!(messages.len(), 3, "should have text + tool call + text");
        assert!(
            matches!(messages[0].kind(), ChatMessageKind::AgentText(t) if t == "I'll edit that file."),
            "first: pre-tool-call text"
        );
        assert!(
            matches!(messages[1].kind(), ChatMessageKind::ToolCall(_)),
            "second: tool call in chronological position"
        );
        assert!(
            matches!(messages[2].kind(), ChatMessageKind::AgentText(t) if t == "Done editing."),
            "third: post-tool-call text"
        );
    }

    // --- Tool call update merge tests ---
    // These test the exact Kiro scenario: initial ToolCall has content,
    // ToolCallUpdate only changes status, content must survive into committed messages.

    #[test]
    fn tool_call_update_preserves_diff_content() {
        use cyril_core::types::{ToolCallContent, ToolCallLocation};

        let mut state = UiState::new(500);

        // Phase 1: Initial ToolCall with diff content and location (Kiro sends this first)
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "Editing main.rs".into(),
            ToolKind::Write,
            ToolCallStatus::Pending,
            Some(serde_json::json!({"file_path": "src/main.rs"})),
        )
        .with_content(vec![ToolCallContent::Diff {
            path: "src/main.rs".into(),
            old_text: Some("fn main() {}".into()),
            new_text: "fn main() {\n    println!(\"hello\");\n}".into(),
        }])
        .with_locations(vec![ToolCallLocation {
            path: "src/main.rs".into(),
            line: Some(1),
        }]);
        state.apply_notification(&Notification::ToolCallStarted(tc));

        assert_eq!(state.active_tool_calls().len(), 1);
        assert_eq!(state.active_tool_calls()[0].content().len(), 1);
        assert_eq!(state.active_tool_calls()[0].locations().len(), 1);

        // Phase 2: ToolCallUpdate with status=Completed but NO content/locations
        // (This is exactly what Kiro sends — only the changed fields)
        let update = ToolCall::new(
            ToolCallId::new("tc_1"),
            "Editing main.rs".into(),
            ToolKind::Write,
            ToolCallStatus::Completed,
            None,
        );
        // Note: no .with_content() or .with_locations() — empty vecs
        state.apply_notification(&Notification::ToolCallUpdated(update));

        // Content and locations must survive the update
        assert_eq!(
            state.active_tool_calls()[0].content().len(),
            1,
            "diff content must survive ToolCallUpdate"
        );
        assert_eq!(
            state.active_tool_calls()[0].locations().len(),
            1,
            "locations must survive ToolCallUpdate"
        );
        assert_eq!(
            state.active_tool_calls()[0].status(),
            ToolCallStatus::Completed,
            "status should be updated"
        );

        // Phase 3: TurnCompleted — tool calls commit to message history
        state.apply_notification(&Notification::TurnCompleted {
            stop_reason: cyril_core::types::StopReason::EndTurn,
        });

        let tc_msg = state
            .messages()
            .iter()
            .find(|m| matches!(m.kind(), ChatMessageKind::ToolCall(_)));
        assert!(tc_msg.is_some(), "tool call should be in committed messages");

        if let ChatMessageKind::ToolCall(tracked) = tc_msg.unwrap().kind() {
            assert_eq!(
                tracked.content().len(),
                1,
                "diff content must survive through commit"
            );
            assert!(
                matches!(&tracked.content()[0], ToolCallContent::Diff { new_text, .. }
                    if new_text.contains("println")),
                "diff should contain the actual code change"
            );
            assert_eq!(tracked.primary_path(), Some("src/main.rs"));
        }
    }

    #[test]
    fn multiple_tool_call_updates_preserve_content() {
        use cyril_core::types::ToolCallContent;

        let mut state = UiState::new(500);

        // Initial ToolCall with content
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "Editing lib.rs".into(),
            ToolKind::Write,
            ToolCallStatus::InProgress,
            None,
        )
        .with_content(vec![ToolCallContent::Diff {
            path: "src/lib.rs".into(),
            old_text: Some("// old".into()),
            new_text: "// new".into(),
        }]);
        state.apply_notification(&Notification::ToolCallStarted(tc));

        // First update: status changes to Pending
        let update1 = ToolCall::new(
            ToolCallId::new("tc_1"),
            "Editing lib.rs".into(),
            ToolKind::Write,
            ToolCallStatus::Pending,
            None,
        );
        state.apply_notification(&Notification::ToolCallUpdated(update1));
        assert_eq!(state.active_tool_calls()[0].content().len(), 1, "content survives first update");

        // Second update: status changes to Completed
        let update2 = ToolCall::new(
            ToolCallId::new("tc_1"),
            "Editing lib.rs".into(),
            ToolKind::Write,
            ToolCallStatus::Completed,
            None,
        );
        state.apply_notification(&Notification::ToolCallUpdated(update2));
        assert_eq!(state.active_tool_calls()[0].content().len(), 1, "content survives second update");
        assert_eq!(state.active_tool_calls()[0].status(), ToolCallStatus::Completed);
    }

    // --- Notification handler tests for new variants ---

    #[test]
    fn rate_limited_adds_system_message() {
        let mut state = UiState::new(500);
        let changed = state.apply_notification(&Notification::RateLimited {
            message: "Too many requests".into(),
        });
        assert!(changed);
        assert!(
            matches!(state.messages().last().unwrap().kind(), ChatMessageKind::System(t) if t.contains("Too many requests"))
        );
    }

    #[test]
    fn mcp_server_init_failure_with_error() {
        let mut state = UiState::new(500);
        let changed = state.apply_notification(&Notification::McpServerInitFailure {
            server_name: "my-mcp".into(),
            error: Some("connection refused".into()),
        });
        assert!(changed);
        assert!(
            matches!(state.messages().last().unwrap().kind(), ChatMessageKind::System(t) if t.contains("my-mcp") && t.contains("connection refused"))
        );
    }

    #[test]
    fn mcp_server_init_failure_without_error() {
        let mut state = UiState::new(500);
        let changed = state.apply_notification(&Notification::McpServerInitFailure {
            server_name: "my-mcp".into(),
            error: None,
        });
        assert!(changed);
        assert!(
            matches!(state.messages().last().unwrap().kind(), ChatMessageKind::System(t) if t.contains("my-mcp") && t.contains("failed"))
        );
    }

    #[test]
    fn mcp_server_initialized_adds_system_message() {
        let mut state = UiState::new(500);
        let changed = state.apply_notification(&Notification::McpServerInitialized {
            server_name: "github-mcp".into(),
        });
        assert!(changed);
        assert!(
            matches!(state.messages().last().unwrap().kind(), ChatMessageKind::System(t) if t.contains("github-mcp") && t.contains("ready"))
        );
    }

    #[test]
    fn mcp_oauth_request_not_handled_by_ui() {
        let mut state = UiState::new(500);
        let changed = state.apply_notification(&Notification::McpOAuthRequest {
            server_name: "server".into(),
            url: "https://example.com".into(),
        });
        assert!(!changed, "McpOAuthRequest should be handled by App, not UiState");
        assert!(state.messages().is_empty());
    }

    #[test]
    fn agent_not_found_with_fallback() {
        let mut state = UiState::new(500);
        let changed = state.apply_notification(&Notification::AgentNotFound {
            requested: "code-reviewer".into(),
            fallback: Some("default".into()),
        });
        assert!(changed);
        assert!(
            matches!(state.messages().last().unwrap().kind(), ChatMessageKind::System(t) if t.contains("code-reviewer") && t.contains("default"))
        );
    }

    #[test]
    fn agent_not_found_without_fallback() {
        let mut state = UiState::new(500);
        let changed = state.apply_notification(&Notification::AgentNotFound {
            requested: "code-reviewer".into(),
            fallback: None,
        });
        assert!(changed);
        assert!(
            matches!(state.messages().last().unwrap().kind(), ChatMessageKind::System(t) if t.contains("code-reviewer") && t.contains("not found"))
        );
    }

    #[test]
    fn agent_config_error_adds_system_message() {
        let mut state = UiState::new(500);
        let changed = state.apply_notification(&Notification::AgentConfigError {
            path: ".kiro/agents/broken.md".into(),
            error: "invalid YAML".into(),
        });
        assert!(changed);
        assert!(
            matches!(state.messages().last().unwrap().kind(), ChatMessageKind::System(t) if t.contains("broken.md") && t.contains("invalid YAML"))
        );
    }

    #[test]
    fn model_not_found_with_fallback() {
        let mut state = UiState::new(500);
        let changed = state.apply_notification(&Notification::ModelNotFound {
            requested: "claude-opus-5".into(),
            fallback: Some("claude-sonnet-4".into()),
        });
        assert!(changed);
        assert!(
            matches!(state.messages().last().unwrap().kind(), ChatMessageKind::System(t) if t.contains("claude-opus-5") && t.contains("claude-sonnet-4"))
        );
    }

    #[test]
    fn model_not_found_without_fallback() {
        let mut state = UiState::new(500);
        let changed = state.apply_notification(&Notification::ModelNotFound {
            requested: "claude-opus-5".into(),
            fallback: None,
        });
        assert!(changed);
        assert!(
            matches!(state.messages().last().unwrap().kind(), ChatMessageKind::System(t) if t.contains("claude-opus-5") && t.contains("not available"))
        );
    }

    #[test]
    fn tool_call_chunk_sets_tool_running() {
        // Subagent routing is now at the App layer via RoutedNotification —
        // UiState only sees chunks for the main session, and always sets activity.
        let mut state = UiState::new(500);
        let changed = state.apply_notification(&Notification::ToolCallChunk {
            tool_call_id: ToolCallId::new("tc-1"),
            title: "read".into(),
            kind: "read".into(),
            session_id: None,
        });
        assert!(changed);
        assert_eq!(state.activity(), Activity::ToolRunning);
    }

    #[test]
    fn subagent_spawned_adds_system_message() {
        let mut state = UiState::new(500);
        let changed = state.apply_notification(&Notification::SubagentSpawned {
            session_id: SessionId::new("sub-1"),
            name: "reviewer".into(),
        });
        assert!(changed);
        assert!(
            matches!(state.messages().last().unwrap().kind(), ChatMessageKind::System(t) if t.contains("reviewer") && t.contains("sub-1"))
        );
    }

    #[test]
    fn subagent_terminated_adds_system_message_with_name_if_tracked() {
        let mut state = UiState::new(500);
        // Register the subagent first so state can look up the name
        register_subagent(&mut state, "sub-1", "reviewer");

        let changed = state.apply_notification(&Notification::SubagentTerminated {
            session_id: SessionId::new("sub-1"),
        });
        assert!(changed);
        assert!(
            matches!(state.messages().last().unwrap().kind(), ChatMessageKind::System(t) if t.contains("reviewer") && t.contains("sub-1"))
        );
    }

    #[test]
    fn subagent_terminated_adds_system_message_without_name_if_not_tracked() {
        let mut state = UiState::new(500);
        let changed = state.apply_notification(&Notification::SubagentTerminated {
            session_id: SessionId::new("ghost"),
        });
        assert!(changed);
        // Message should still include the session id even if name can't be resolved
        assert!(
            matches!(state.messages().last().unwrap().kind(), ChatMessageKind::System(t) if t.contains("ghost") && t.contains("Terminated"))
        );
    }

    #[test]
    fn bridge_error_adds_system_message() {
        let mut state = UiState::new(500);
        let changed = state.apply_notification(&Notification::BridgeError {
            operation: "set_mode".into(),
            message: "connection refused".into(),
        });
        assert!(changed);
        assert!(
            matches!(state.messages().last().unwrap().kind(), ChatMessageKind::System(t) if t.contains("set_mode") && t.contains("connection refused"))
        );
    }

    // ── Subagent routing tests ───────────────────────────────────────────
    // These verify the routing contract that App::handle_notification depends
    // on: session-scoped notifications must flow to SubagentUiState, not the
    // main UiState; list updates must reach both tracker and stream cleanup.

    fn register_subagent(state: &mut UiState, id: &str, name: &str) {
        let info = cyril_core::types::SubagentInfo::new(
            SessionId::new(id),
            name,
            name,
            "query",
            cyril_core::types::SubagentStatus::Working {
                message: Some("Running".into()),
            },
            Some("crew-test".into()),
            None,
            vec![],
        );
        let notif = Notification::SubagentListUpdated {
            subagents: vec![info],
            pending_stages: vec![],
        };
        state.apply_subagent_tracker_notification(&notif);
        if let Notification::SubagentListUpdated { subagents, .. } = &notif {
            state.apply_subagent_list_update(subagents);
        }
    }

    #[test]
    fn subagent_notification_routes_to_subagent_stream_not_main() {
        let mut state = UiState::new(500);
        register_subagent(&mut state, "sub-1", "reviewer");

        let sid = SessionId::new("sub-1");
        assert!(state.subagent_tracker().is_subagent(&sid));

        // Send an AgentMessage scoped to the subagent
        state.apply_subagent_notification(
            &sid,
            &Notification::AgentMessage(AgentMessage {
                text: "subagent text".into(),
                is_streaming: false,
            }),
        );

        // Main messages should be empty
        assert!(state.messages().is_empty());

        // Subagent stream should have the message
        let stream = state
            .subagent_ui()
            .streams()
            .get(&sid)
            .expect("stream should exist");
        assert_eq!(stream.messages().len(), 1);
    }

    #[test]
    fn subagent_list_update_marks_removed_streams_terminated() {
        let mut state = UiState::new(500);
        register_subagent(&mut state, "sub-1", "reviewer");

        // Stream for sub-1 exists and is working
        let sid = SessionId::new("sub-1");
        state.apply_subagent_notification(
            &sid,
            &Notification::AgentMessage(AgentMessage {
                text: "working".into(),
                is_streaming: true,
            }),
        );
        assert!(state.any_subagent_active());

        // List update removes sub-1
        let changed = state.apply_subagent_list_update(&[]);
        assert!(changed);
        // Stream still exists (preserved for history)
        assert!(state.subagent_ui().streams().contains_key(&sid));
        // But is no longer active
        assert!(!state.any_subagent_active());
    }

    #[test]
    fn subagent_list_update_no_op_returns_false() {
        let mut state = UiState::new(500);
        // No streams registered, list update is a no-op
        let changed = state.apply_subagent_list_update(&[]);
        assert!(!changed);
    }

    #[test]
    fn focus_subagent_requires_existing_stream() {
        let mut state = UiState::new(500);
        let sid = SessionId::new("ghost");
        // Focus without a stream should fail
        assert!(!state.focus_subagent(sid.clone()));
        assert!(state.subagent_ui().focused_session_id().is_none());

        // After registering + notifying, focus should succeed
        register_subagent(&mut state, "sub-1", "reviewer");
        let sid = SessionId::new("sub-1");
        state.apply_subagent_notification(
            &sid,
            &Notification::AgentMessage(AgentMessage {
                text: "hi".into(),
                is_streaming: false,
            }),
        );
        assert!(state.focus_subagent(sid.clone()));
        assert_eq!(state.subagent_ui().focused_session_id(), Some(&sid));
    }

    #[test]
    fn unfocus_subagent_clears_focus() {
        let mut state = UiState::new(500);
        register_subagent(&mut state, "sub-1", "reviewer");
        let sid = SessionId::new("sub-1");
        state.apply_subagent_notification(
            &sid,
            &Notification::AgentMessage(AgentMessage {
                text: "hi".into(),
                is_streaming: false,
            }),
        );
        assert!(state.focus_subagent(sid));
        state.unfocus_subagent();
        assert!(state.subagent_ui().focused_session_id().is_none());
    }

    // --- Hooks panel tests ---

    fn sample_hook(trigger: &str, command: &str, matcher: Option<&str>) -> HookInfo {
        HookInfo {
            trigger: trigger.into(),
            command: command.into(),
            matcher: matcher.map(String::from),
        }
    }

    #[test]
    fn code_panel_lifecycle() {
        use cyril_core::types::{CodePanelData, LspStatus};

        let mut state = UiState::new(500);
        assert!(state.code_panel().is_none());
        assert!(!state.has_code_panel());

        let data = CodePanelData {
            status: LspStatus::Initialized,
            message: Some("LSP servers ready".into()),
            warning: None,
            root_path: Some("/home/user/project".into()),
            detected_languages: vec!["rust".into()],
            project_markers: vec!["Cargo.toml".into()],
            config_path: Some(".kiro/settings/lsp.json".into()),
            doc_url: None,
            lsps: vec![],
        };

        state.show_code_panel(data);
        assert!(state.has_code_panel());
        assert!(state.code_panel().is_some());

        state.close_code_panel();
        assert!(!state.has_code_panel());
    }

    #[test]
    fn code_intelligence_active_defaults_false() {
        let state = UiState::new(500);
        assert!(!state.code_intelligence_active());
    }

    #[test]
    fn set_code_intelligence_active() {
        let mut state = UiState::new(500);
        state.set_code_intelligence_active(true);
        assert!(state.code_intelligence_active());
    }

    #[test]
    fn hooks_panel_starts_hidden() {
        let state = UiState::new(500);
        assert!(!state.has_hooks_panel());
        assert!(state.hooks_panel().is_none());
    }

    #[test]
    fn show_hooks_panel_sets_state() {
        let mut state = UiState::new(500);
        // Input order is (Pre, Post) but show_hooks_panel sorts by
        // (trigger, command), so the stored order is (Post, Pre).
        let hooks = vec![
            sample_hook("PreToolUse", "echo pre", Some("read")),
            sample_hook("PostToolUse", "echo post", None),
        ];
        state.show_hooks_panel(hooks);
        assert!(state.has_hooks_panel());
        let panel = state.hooks_panel().expect("panel should exist");
        assert_eq!(panel.hooks.len(), 2);
        assert_eq!(panel.hooks[0].trigger, "PostToolUse");
        assert_eq!(panel.hooks[0].matcher, None);
        assert_eq!(panel.hooks[1].trigger, "PreToolUse");
        assert_eq!(panel.hooks[1].matcher.as_deref(), Some("read"));
        assert_eq!(panel.scroll_offset, 0);
    }

    #[test]
    fn hide_hooks_panel_clears_state() {
        let mut state = UiState::new(500);
        state.show_hooks_panel(vec![sample_hook("Stop", "noop", None)]);
        assert!(state.has_hooks_panel());
        state.hide_hooks_panel();
        assert!(!state.has_hooks_panel());
    }

    #[test]
    fn show_hooks_panel_with_empty_list() {
        let mut state = UiState::new(500);
        state.show_hooks_panel(Vec::new());
        assert!(state.has_hooks_panel());
        assert_eq!(state.hooks_panel().expect("panel").hooks.len(), 0);
    }

    #[test]
    fn show_hooks_panel_sorts_on_insert_by_trigger() {
        // Unsorted input; expect sorted by trigger in the stored state so the
        // widget can iterate without re-sorting on every render frame.
        let mut state = UiState::new(500);
        state.show_hooks_panel(vec![
            sample_hook("Stop", "stop-cmd", None),
            sample_hook("AgentSpawn", "spawn-cmd", None),
            sample_hook("PreToolUse", "pre-cmd", Some("read")),
        ]);
        let triggers: Vec<&str> = state
            .hooks_panel()
            .expect("panel")
            .hooks
            .iter()
            .map(|h| h.trigger.as_str())
            .collect();
        assert_eq!(triggers, vec!["AgentSpawn", "PreToolUse", "Stop"]);
    }

    #[test]
    fn show_hooks_panel_sorts_by_command_within_trigger() {
        // Same trigger, different commands — expect alphabetical order by
        // command as the tiebreaker.
        let mut state = UiState::new(500);
        state.show_hooks_panel(vec![
            sample_hook("PreToolUse", "zebra", None),
            sample_hook("PreToolUse", "alpha", None),
            sample_hook("PreToolUse", "middle", None),
        ]);
        let commands: Vec<&str> = state
            .hooks_panel()
            .expect("panel")
            .hooks
            .iter()
            .map(|h| h.command.as_str())
            .collect();
        assert_eq!(commands, vec!["alpha", "middle", "zebra"]);
    }

    #[test]
    fn hooks_panel_scroll_down_respects_bound() {
        let mut state = UiState::new(500);
        let hooks = vec![
            sample_hook("A", "a", None),
            sample_hook("B", "b", None),
            sample_hook("C", "c", None),
        ];
        state.show_hooks_panel(hooks);
        state.hooks_panel_scroll_down(10); // way past the end
        // Max index is len-1 = 2
        assert_eq!(state.hooks_panel().expect("panel").scroll_offset, 2);
    }

    #[test]
    fn hooks_panel_scroll_up_saturates_at_zero() {
        let mut state = UiState::new(500);
        state.show_hooks_panel(vec![
            sample_hook("A", "a", None),
            sample_hook("B", "b", None),
        ]);
        state.hooks_panel_scroll_down(1);
        state.hooks_panel_scroll_up(5); // way past the start
        assert_eq!(state.hooks_panel().expect("panel").scroll_offset, 0);
    }

    #[test]
    fn hooks_panel_scroll_noop_when_hidden() {
        let mut state = UiState::new(500);
        // These should silently do nothing when there's no panel.
        state.hooks_panel_scroll_up(3);
        state.hooks_panel_scroll_down(3);
        assert!(!state.has_hooks_panel());
    }

    // --- Chat scroll tests ---

    #[test]
    fn chat_scroll_up_enters_browse_mode() {
        let mut state = UiState::new(500);
        assert!(state.chat_scroll_back().is_none());
        state.chat_scroll_up(5);
        assert_eq!(state.chat_scroll_back(), Some(5));
    }

    #[test]
    fn chat_scroll_up_accumulates() {
        let mut state = UiState::new(500);
        state.chat_scroll_up(5);
        state.chat_scroll_up(3);
        assert_eq!(state.chat_scroll_back(), Some(8));
    }

    #[test]
    fn chat_scroll_down_reduces_offset() {
        let mut state = UiState::new(500);
        state.chat_scroll_up(10);
        state.chat_scroll_down(3);
        assert_eq!(state.chat_scroll_back(), Some(7));
    }

    #[test]
    fn chat_scroll_down_returns_to_follow_mode() {
        let mut state = UiState::new(500);
        state.chat_scroll_up(3);
        state.chat_scroll_down(5);
        assert!(state.chat_scroll_back().is_none());
    }

    #[test]
    fn chat_scroll_down_noop_in_follow_mode() {
        let mut state = UiState::new(500);
        state.chat_scroll_down(5);
        assert!(state.chat_scroll_back().is_none());
    }

    #[test]
    fn chat_scroll_reset_returns_to_follow_mode() {
        let mut state = UiState::new(500);
        state.chat_scroll_up(10);
        state.chat_scroll_reset();
        assert!(state.chat_scroll_back().is_none());
    }

    #[test]
    fn take_input_resets_chat_scroll() {
        let mut state = UiState::new(500);
        state.chat_scroll_up(10);
        state.handle_input_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('h'),
        ));
        let _ = state.take_input();
        assert!(state.chat_scroll_back().is_none());
    }

    #[test]
    fn set_command_info_propagates_descriptions() {
        use crossterm::event::{KeyCode, KeyEvent};

        let mut state = UiState::new(500);
        state.set_command_info(vec![
            ("model".into(), Some("Switch model".into())),
            ("mode".into(), Some("Switch mode".into())),
            ("new".into(), None),
        ]);

        // Type "/" to trigger autocomplete for all commands
        state.handle_input_key(KeyEvent::from(KeyCode::Char('/')));
        let suggestions = state.autocomplete_suggestions();
        assert_eq!(suggestions.len(), 3);

        // Verify descriptions propagated correctly
        let model = suggestions.iter().find(|s| s.text == "/model").unwrap();
        assert_eq!(model.description.as_deref(), Some("Switch model"));

        let new = suggestions.iter().find(|s| s.text == "/new").unwrap();
        assert!(new.description.is_none(), "None description should stay None");
    }

    // --- Activity timer tests ---

    #[test]
    fn set_activity_idle_to_busy_starts_timer() {
        let mut state = UiState::new(500);
        assert!(state.activity_elapsed().is_none());
        state.set_activity(Activity::Sending);
        assert!(state.activity_elapsed().is_some());
    }

    #[test]
    fn set_activity_busy_to_busy_preserves_timer() {
        let mut state = UiState::new(500);
        state.set_activity(Activity::Sending);
        let first_elapsed = state.activity_elapsed();
        assert!(first_elapsed.is_some());

        // Transition to another busy state — timer should NOT reset
        state.set_activity(Activity::ToolRunning);
        assert!(state.activity_elapsed().is_some());
        // Timer should still be running from the original start
        assert_eq!(state.activity(), Activity::ToolRunning);
    }

    #[test]
    fn set_activity_busy_to_idle_clears_timer() {
        let mut state = UiState::new(500);
        state.set_activity(Activity::Sending);
        assert!(state.activity_elapsed().is_some());

        state.set_activity(Activity::Ready);
        assert!(state.activity_elapsed().is_none());
    }

    #[test]
    fn set_activity_same_state_is_noop() {
        let mut state = UiState::new(500);
        state.set_activity(Activity::Sending);
        state.set_activity(Activity::Sending);
        // Should not panic or change behavior
        assert!(state.activity_elapsed().is_some());
    }

    // --- TurnSummary buffer assembly tests ---

    #[test]
    fn ui_state_turn_summary_assembled_from_metadata_and_turn_completed() {
        let mut state = UiState::new(500);

        state.apply_notification(&Notification::MetadataUpdated {
            context_usage: ContextUsage::new(50.0),
            metering: Some(TurnMetering::new(0.03, Some(2000))),
            tokens: Some(TokenCounts::new(800, 400, Some(100))),
        });
        assert!(
            state.last_turn().is_none(),
            "no TurnSummary until TurnCompleted"
        );

        state.apply_notification(&Notification::TurnCompleted {
            stop_reason: cyril_core::types::StopReason::EndTurn,
        });
        let summary = state
            .last_turn()
            .expect("TurnSummary should exist after TurnCompleted");
        assert_eq!(summary.stop_reason(), cyril_core::types::StopReason::EndTurn);
        assert!(summary.token_counts().is_some());
        assert_eq!(summary.token_counts().unwrap().input(), 800);
        assert!(summary.metering().is_some());
        assert!((summary.metering().unwrap().credits() - 0.03).abs() < 0.001);
    }

    #[test]
    fn ui_state_turn_summary_cleared_on_session_created() {
        let mut state = UiState::new(500);

        state.apply_notification(&Notification::MetadataUpdated {
            context_usage: ContextUsage::new(10.0),
            metering: Some(TurnMetering::new(0.01, None)),
            tokens: None,
        });
        state.apply_notification(&Notification::TurnCompleted {
            stop_reason: cyril_core::types::StopReason::EndTurn,
        });
        assert!(state.last_turn().is_some());

        state.apply_notification(&Notification::SessionCreated {
            session_id: SessionId::new("s2"),
            current_mode: None,
            current_model: None,
            welcome_message: None,
            available_modes: Vec::new(),
        });
        assert!(
            state.last_turn().is_none(),
            "TurnSummary should be cleared on new session"
        );
    }

    #[test]
    fn ui_state_session_cost_accumulates() {
        let mut state = UiState::new(500);

        // Turn 1
        state.apply_notification(&Notification::MetadataUpdated {
            context_usage: ContextUsage::new(10.0),
            metering: Some(TurnMetering::new(0.02, Some(1000))),
            tokens: None,
        });
        state.apply_notification(&Notification::TurnCompleted {
            stop_reason: cyril_core::types::StopReason::EndTurn,
        });

        // Turn 2
        state.apply_notification(&Notification::MetadataUpdated {
            context_usage: ContextUsage::new(20.0),
            metering: Some(TurnMetering::new(0.03, Some(2000))),
            tokens: None,
        });
        state.apply_notification(&Notification::TurnCompleted {
            stop_reason: cyril_core::types::StopReason::EndTurn,
        });

        assert_eq!(state.session_cost().turn_count(), 2);
        assert!((state.session_cost().total_credits() - 0.05).abs() < 0.001);
    }

    #[test]
    fn ui_state_session_cost_reset_on_session_created() {
        let mut state = UiState::new(500);

        state.apply_notification(&Notification::MetadataUpdated {
            context_usage: ContextUsage::new(10.0),
            metering: Some(TurnMetering::new(0.05, Some(2000))),
            tokens: None,
        });
        state.apply_notification(&Notification::TurnCompleted {
            stop_reason: cyril_core::types::StopReason::EndTurn,
        });
        assert!(state.session_cost().total_credits() > 0.0);

        state.apply_notification(&Notification::SessionCreated {
            session_id: SessionId::new("s2"),
            current_mode: None,
            current_model: None,
            welcome_message: None,
            available_modes: Vec::new(),
        });

        assert_eq!(state.session_cost().total_credits(), 0.0);
        assert_eq!(state.session_cost().turn_count(), 0);
    }

    #[test]
    fn session_created_welcome_message_shown() {
        let mut state = UiState::new(500);
        state.apply_notification(&Notification::SessionCreated {
            session_id: SessionId::new("s1"),
            current_mode: None,
            current_model: None,
            welcome_message: Some("Hello! I'm Kiro.".into()),
            available_modes: Vec::new(),
        });
        assert_eq!(state.messages().len(), 1);
        assert!(matches!(
            state.messages()[0].kind(),
            ChatMessageKind::System(msg) if msg == "Hello! I'm Kiro."
        ));
    }

    #[test]
    fn session_created_no_welcome_falls_back_to_session_id() {
        let mut state = UiState::new(500);
        state.apply_notification(&Notification::SessionCreated {
            session_id: SessionId::new("sess_abc"),
            current_mode: None,
            current_model: None,
            welcome_message: None,
            available_modes: Vec::new(),
        });
        assert_eq!(state.messages().len(), 1);
        assert!(matches!(
            state.messages()[0].kind(),
            ChatMessageKind::System(msg) if msg.contains("sess_abc")
        ));
    }

    #[test]
    fn user_message_added_to_messages() {
        let mut state = UiState::new(500);
        state.apply_notification(&Notification::UserMessage {
            text: "Fix the auth bug".into(),
        });
        assert_eq!(state.messages().len(), 1);
        assert!(
            matches!(state.messages()[0].kind(), ChatMessageKind::UserText(t) if t == "Fix the auth bug")
        );
    }

    #[test]
    fn user_message_in_session_replay_sequence() {
        let mut state = UiState::new(500);

        // Simulate a replay: user message, then agent response, then turn complete
        state.apply_notification(&Notification::UserMessage {
            text: "What is 2+2?".into(),
        });
        state.apply_notification(&Notification::AgentMessage(
            cyril_core::types::message::AgentMessage {
                text: "4".into(),
                is_streaming: false,
            },
        ));
        state.apply_notification(&Notification::TurnCompleted {
            stop_reason: cyril_core::types::StopReason::EndTurn,
        });

        assert_eq!(state.messages().len(), 2);
        assert!(matches!(
            state.messages()[0].kind(),
            ChatMessageKind::UserText(_)
        ));
        assert!(matches!(
            state.messages()[1].kind(),
            ChatMessageKind::AgentText(_)
        ));
    }

    #[test]
    fn compaction_started_sets_streaming_activity() {
        let mut state = UiState::new(500);
        state.apply_notification(&Notification::CompactionStatus {
            phase: cyril_core::types::CompactionPhase::Started,
            summary: None,
        });
        assert!(matches!(state.activity(), Activity::Streaming));
        assert_eq!(state.messages().len(), 1);
        assert!(matches!(
            state.messages()[0].kind(),
            ChatMessageKind::System(msg) if msg.contains("Compacting")
        ));
    }

    #[test]
    fn compaction_completed_resets_activity_and_shows_summary() {
        let mut state = UiState::new(500);
        state.set_activity(Activity::Streaming);
        state.apply_notification(&Notification::CompactionStatus {
            phase: cyril_core::types::CompactionPhase::Completed,
            summary: Some("3 turns removed".into()),
        });
        assert!(matches!(state.activity(), Activity::Ready));
        assert!(matches!(
            state.messages()[0].kind(),
            ChatMessageKind::System(msg) if msg.contains("3 turns removed")
        ));
    }

    #[test]
    fn compaction_completed_no_summary() {
        let mut state = UiState::new(500);
        state.apply_notification(&Notification::CompactionStatus {
            phase: cyril_core::types::CompactionPhase::Completed,
            summary: None,
        });
        assert!(matches!(
            state.messages()[0].kind(),
            ChatMessageKind::System(msg) if msg == "Compaction completed"
        ));
    }

    #[test]
    fn compaction_failed_shows_error_and_resets_activity() {
        let mut state = UiState::new(500);
        state.set_activity(Activity::Streaming);
        state.apply_notification(&Notification::CompactionStatus {
            phase: cyril_core::types::CompactionPhase::Failed {
                error: Some("out of memory".into()),
            },
            summary: None,
        });
        assert!(matches!(state.activity(), Activity::Ready));
        assert!(matches!(
            state.messages()[0].kind(),
            ChatMessageKind::System(msg) if msg.contains("out of memory")
        ));
    }

    #[test]
    fn compaction_failed_no_error_detail() {
        let mut state = UiState::new(500);
        state.apply_notification(&Notification::CompactionStatus {
            phase: cyril_core::types::CompactionPhase::Failed { error: None },
            summary: None,
        });
        assert!(matches!(
            state.messages()[0].kind(),
            ChatMessageKind::System(msg) if msg == "Compaction failed"
        ));
    }
}
