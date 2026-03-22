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
    command_names: Vec<String>,

    // Session info (projected by App from SessionController)
    activity: Activity,
    activity_since: Option<Instant>,
    session_label: Option<String>,
    current_mode: Option<String>,
    current_model: Option<String>,
    context_usage: Option<f64>,
    credit_usage: Option<(f64, f64)>,

    // Overlays
    approval: Option<ApprovalState>,
    picker: Option<PickerState>,

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

    fn approval(&self) -> Option<&ApprovalState> {
        self.approval.as_ref()
    }

    fn picker(&self) -> Option<&PickerState> {
        self.picker.as_ref()
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
            command_names: Vec::new(),
            activity: Activity::Idle,
            activity_since: None,
            session_label: None,
            current_mode: None,
            current_model: None,
            context_usage: None,
            credit_usage: None,
            approval: None,
            picker: None,
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
                let tracked = TrackedToolCall::new(tc.clone());
                let idx = self.active_tool_calls.len();
                self.active_tool_calls.push(tracked);
                self.tool_call_index.insert(tc.id().clone(), idx);
                self.set_activity(Activity::ToolRunning);
                true
            }
            Notification::ToolCallUpdated(tc) => {
                if let Some(&idx) = self.tool_call_index.get(tc.id())
                    && let Some(tracked) = self.active_tool_calls.get_mut(idx)
                {
                    tracked.update(tc);
                }
                true
            }
            Notification::PlanUpdated(plan) => {
                self.current_plan = Some(plan.clone());
                true
            }
            Notification::ContextUsageUpdated(usage) => {
                self.context_usage = Some(usage.percentage());
                true
            }
            Notification::TurnCompleted => {
                self.commit_streaming();
                self.set_activity(Activity::Ready);
                true
            }
            Notification::BridgeDisconnected { reason } => {
                self.add_system_message(format!("Disconnected: {reason}"));
                self.set_activity(Activity::Idle);
                true
            }
            Notification::ModeChanged { mode_id } => {
                self.current_mode = Some(mode_id.clone());
                true
            }
            Notification::AgentSwitched { name, welcome } => {
                self.current_mode = Some(name.clone());
                if let Some(msg) = welcome {
                    self.add_system_message(format!("Switched to {name}: {msg}"));
                } else {
                    self.add_system_message(format!("Switched to {name}"));
                }
                true
            }
            Notification::CompactionStatus { message } => {
                self.add_system_message(format!("Compaction: {message}"));
                true
            }
            Notification::ClearStatus { message } => {
                self.add_system_message(format!("Clear: {message}"));
                true
            }
            Notification::SessionCreated {
                session_id,
                current_mode,
            } => {
                self.session_label = Some(session_id.as_str().to_string());
                self.current_mode = current_mode.clone();
                self.add_system_message(format!("Session created: {}", session_id.as_str()));
                self.set_activity(Activity::Ready);
                true
            }
            Notification::ToolCallChunk {
                tool_call_id: _,
                title: _,
                kind: _,
            } => {
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
            Notification::CommandsUpdated(_) => {
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
        }
    }

    /// Flush streaming text and active tool calls into the message list.
    pub fn commit_streaming(&mut self) {
        let had_content = !self.streaming_text.is_empty() || !self.active_tool_calls.is_empty();

        if !self.streaming_text.is_empty() {
            let text = std::mem::take(&mut self.streaming_text);
            self.messages.push(ChatMessage::agent_text(text));
        }

        // Commit tool calls as individual messages so they appear in history
        for tc in self.active_tool_calls.drain(..) {
            self.messages.push(ChatMessage::tool_call(tc));
        }
        self.tool_call_index.clear();

        self.streaming_thought = None;

        if had_content {
            self.messages_version += 1;
            self.enforce_message_limit();
        }
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
    /// Elapsed time is only tracked for busy states — cleared on Ready/Idle.
    pub fn set_activity(&mut self, activity: Activity) {
        if self.activity != activity {
            self.activity = activity;
            self.activity_since = match activity {
                Activity::Idle | Activity::Ready => None,
                _ => Some(Instant::now()),
            };
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

    /// Take the current input text, clearing the input buffer and cursor.
    pub fn take_input(&mut self) -> String {
        self.input_cursor = 0;
        self.autocomplete_suggestions.clear();
        self.autocomplete_selected = None;
        std::mem::take(&mut self.input_text)
    }

    /// Update the terminal size.
    pub fn set_terminal_size(&mut self, w: u16, h: u16) {
        self.terminal_size = (w, h);
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

    /// Command names available for slash autocomplete.
    pub fn set_command_names(&mut self, names: Vec<String>) {
        self.command_names = names;
    }

    /// Recompute autocomplete suggestions based on current input text.
    fn update_autocomplete(&mut self) {
        let text = &self.input_text;
        let trimmed = text.trim();

        // Slash command autocomplete
        if trimmed.starts_with('/') && !trimmed.contains(' ') {
            let query = trimmed[1..].to_lowercase();
            self.autocomplete_suggestions = self
                .command_names
                .iter()
                .filter(|name| name.to_lowercase().starts_with(&query))
                .map(|name| Suggestion {
                    text: format!("/{name}"),
                    description: None,
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

    /// Flush the stream buffer if it has timed-out content.
    /// Returns `true` if content was flushed (UI changed).
    pub fn flush_stream_buffer(&mut self) -> bool {
        // The stream buffer is managed externally via apply_notification.
        // This method handles the timeout-based flush of streaming_text.
        // Currently streaming_text is set directly, so no buffered flush needed.
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
        state.apply_notification(&Notification::TurnCompleted);

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
            None,
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
            None,
            ToolKind::Read,
            ToolCallStatus::InProgress,
            None,
        );
        state.apply_notification(&Notification::ToolCallStarted(tc));

        let updated = ToolCall::new(
            ToolCallId::new("tc_1"),
            "read".into(),
            Some("Reading src/main.rs".into()),
            ToolKind::Read,
            ToolCallStatus::Completed,
            None,
        );
        state.apply_notification(&Notification::ToolCallUpdated(updated));

        assert_eq!(
            state.active_tool_calls()[0].title(),
            Some("Reading src/main.rs")
        );
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
            "read".into(),
            Some("Reading main.rs".into()),
            ToolKind::Read,
            ToolCallStatus::InProgress,
            None,
        );
        state.apply_notification(&Notification::ToolCallStarted(tc));

        assert_eq!(state.active_tool_calls().len(), 1);
        assert_eq!(state.streaming_text(), "Let me read that file.");

        // Turn completes
        state.apply_notification(&Notification::TurnCompleted);

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
            "write".into(),
            Some("Editing main.rs".into()),
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
        state.apply_notification(&Notification::TurnCompleted);

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
        state.apply_notification(&Notification::TurnCompleted);

        // Turn 2: text + tool call
        state.apply_notification(&Notification::AgentMessage(AgentMessage {
            text: "Second response.".into(),
            is_streaming: true,
        }));
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "read".into(),
            None,
            ToolKind::Read,
            ToolCallStatus::Completed,
            None,
        );
        state.apply_notification(&Notification::ToolCallStarted(tc));
        state.apply_notification(&Notification::TurnCompleted);

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
            "read".into(),
            Some("Reading config".into()),
            ToolKind::Read,
            ToolCallStatus::Completed,
            None,
        );
        state.apply_notification(&Notification::ToolCallStarted(tc));
        state.apply_notification(&Notification::TurnCompleted);

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
}
