use std::collections::HashMap;
use std::time::{Duration, Instant};

use cyril_core::types::*;

use crate::traits::*;

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
                    self.streaming_text.clone_from(&msg.text);
                    self.set_activity(Activity::Streaming);
                } else {
                    self.streaming_text.clear();
                    self.messages.push(ChatMessage::agent_text(msg.text.clone()));
                    self.messages_version += 1;
                    self.enforce_message_limit();
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
                self.active_tool_calls.clear();
                self.tool_call_index.clear();
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
            Notification::SessionCreated { session_id } => {
                self.session_label = Some(session_id.as_str().to_string());
                true
            }
            Notification::ConfigOptionsUpdated(_) | Notification::CommandsUpdated(_) => {
                // These are consumed by the App layer, not UiState directly.
                false
            }
        }
    }

    /// Flush any streaming text into the message list.
    pub fn commit_streaming(&mut self) {
        if !self.streaming_text.is_empty() {
            let text = std::mem::take(&mut self.streaming_text);
            self.messages.push(ChatMessage::agent_text(text));
            self.messages_version += 1;
            self.enforce_message_limit();
        }
        self.streaming_thought = None;
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

    /// Update the activity state and record when it changed.
    pub fn set_activity(&mut self, activity: Activity) {
        if self.activity != activity {
            self.activity = activity;
            self.activity_since = Some(Instant::now());
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
        std::mem::take(&mut self.input_text)
    }

    /// Update the terminal size.
    pub fn set_terminal_size(&mut self, w: u16, h: u16) {
        self.terminal_size = (w, h);
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

    #[test]
    fn message_limit_enforced() {
        let mut state = UiState::new(3);
        for i in 0..5 {
            state.add_user_message(&format!("msg {i}"));
        }
        assert_eq!(state.messages().len(), 3);
        // Oldest messages removed
        assert!(
            matches!(state.messages()[0].kind(), ChatMessageKind::UserText(t) if t == "msg 2")
        );
    }

    #[test]
    fn add_system_message() {
        let mut state = UiState::new(500);
        state.add_system_message("Welcome".into());
        assert_eq!(state.messages().len(), 1);
        assert!(
            matches!(state.messages()[0].kind(), ChatMessageKind::System(t) if t == "Welcome")
        );
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
