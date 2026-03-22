use std::time::Duration;

use cyril_core::types::{CommandOption, Plan};

/// Activity state derived from UiState — used for adaptive frame rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Activity {
    #[default]
    Idle,
    Ready,
    Sending,
    Waiting,
    Streaming,
    ToolRunning,
}

/// Read-only trait for the renderer. The renderer receives `&dyn TuiState`
/// and cannot mutate application state.
pub trait TuiState {
    // Chat content
    fn messages(&self) -> &[ChatMessage];
    fn streaming_text(&self) -> &str;
    fn streaming_thought(&self) -> Option<&str>;
    fn messages_version(&self) -> u64;

    // Tool calls & plans
    fn active_tool_calls(&self) -> &[TrackedToolCall];
    fn current_plan(&self) -> Option<&Plan>;

    // Input
    fn input_text(&self) -> &str;
    fn input_cursor(&self) -> usize;
    fn autocomplete_suggestions(&self) -> &[Suggestion];
    fn autocomplete_selected(&self) -> Option<usize>;

    // Session info (projected from SessionController)
    fn activity(&self) -> Activity;
    fn session_label(&self) -> Option<&str>;
    fn current_mode(&self) -> Option<&str>;
    fn current_model(&self) -> Option<&str>;
    fn context_usage(&self) -> Option<f64>;
    fn credit_usage(&self) -> Option<(f64, f64)>;

    // Overlays
    fn approval(&self) -> Option<&ApprovalState>;
    fn picker(&self) -> Option<&PickerState>;

    // Terminal
    fn terminal_size(&self) -> (u16, u16);
    fn mouse_captured(&self) -> bool;
    fn should_quit(&self) -> bool;

    // Timing
    fn activity_elapsed(&self) -> Option<Duration>;
    fn is_deep_idle(&self) -> bool;
}

/// A chat message for display purposes.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub kind: ChatMessageKind,
    pub timestamp: std::time::Instant,
}

#[derive(Debug, Clone)]
pub enum ChatMessageKind {
    UserText(String),
    AgentText(String),
    Thought(String),
    ToolCall(TrackedToolCall),
    Plan(Plan),
    System(String),
    /// Output from an agent command (e.g., /tools, /context, /usage).
    CommandOutput {
        command: String,
        text: String,
    },
}

impl ChatMessage {
    pub fn user_text(text: String) -> Self {
        Self {
            kind: ChatMessageKind::UserText(text),
            timestamp: std::time::Instant::now(),
        }
    }

    pub fn agent_text(text: String) -> Self {
        Self {
            kind: ChatMessageKind::AgentText(text),
            timestamp: std::time::Instant::now(),
        }
    }

    pub fn tool_call(tc: TrackedToolCall) -> Self {
        Self {
            kind: ChatMessageKind::ToolCall(tc),
            timestamp: std::time::Instant::now(),
        }
    }

    pub fn plan(plan: Plan) -> Self {
        Self {
            kind: ChatMessageKind::Plan(plan),
            timestamp: std::time::Instant::now(),
        }
    }

    pub fn system(text: String) -> Self {
        Self {
            kind: ChatMessageKind::System(text),
            timestamp: std::time::Instant::now(),
        }
    }

    pub fn command_output(command: String, text: String) -> Self {
        Self {
            kind: ChatMessageKind::CommandOutput { command, text },
            timestamp: std::time::Instant::now(),
        }
    }

    pub fn thought(text: String) -> Self {
        Self {
            kind: ChatMessageKind::Thought(text),
            timestamp: std::time::Instant::now(),
        }
    }

    pub fn kind(&self) -> &ChatMessageKind {
        &self.kind
    }
}

/// A tool call enriched for display (wraps `cyril_core::types::ToolCall`).
#[derive(Debug, Clone)]
pub struct TrackedToolCall {
    inner: cyril_core::types::ToolCall,
}

impl TrackedToolCall {
    pub fn new(tc: cyril_core::types::ToolCall) -> Self {
        Self { inner: tc }
    }

    pub fn update(&mut self, tc: &cyril_core::types::ToolCall) {
        self.inner = tc.clone();
    }

    pub fn id(&self) -> &cyril_core::types::ToolCallId {
        self.inner.id()
    }

    pub fn name(&self) -> &str {
        self.inner.name()
    }

    pub fn kind(&self) -> cyril_core::types::ToolKind {
        self.inner.kind()
    }

    pub fn status(&self) -> cyril_core::types::ToolCallStatus {
        self.inner.status()
    }

    pub fn title(&self) -> Option<&str> {
        self.inner.title()
    }

    pub fn raw_input(&self) -> Option<&serde_json::Value> {
        self.inner.raw_input()
    }

    pub fn content(&self) -> &[cyril_core::types::ToolCallContent] {
        self.inner.content()
    }

    pub fn locations(&self) -> &[cyril_core::types::ToolCallLocation] {
        self.inner.locations()
    }

    /// Get the primary file path from locations, then from diff content, then from raw_input.
    pub fn primary_path(&self) -> Option<&str> {
        if let Some(loc) = self.inner.locations().first() {
            return Some(&loc.path);
        }
        for c in self.inner.content() {
            if let cyril_core::types::ToolCallContent::Diff { path, .. } = c {
                return Some(path);
            }
        }
        self.inner
            .raw_input()
            .and_then(|v| v.get("file_path").or_else(|| v.get("path")))
            .and_then(|v| v.as_str())
    }

    /// Extract command string from raw_input for Execute kind.
    pub fn command_text(&self) -> Option<&str> {
        self.inner
            .raw_input()
            .and_then(|v| v.get("command"))
            .and_then(|v| v.as_str())
    }
}

/// Autocomplete suggestion for input.
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub text: String,
    pub description: Option<String>,
}

/// Permission approval dialog state.
#[derive(Debug)]
pub struct ApprovalState {
    pub tool_call: cyril_core::types::ToolCall,
    pub message: String,
    pub options: Vec<cyril_core::types::PermissionOption>,
    pub selected: usize,
    pub responder: tokio::sync::oneshot::Sender<cyril_core::types::PermissionResponse>,
}

/// Selection picker dialog state.
#[derive(Debug)]
pub struct PickerState {
    pub title: String,
    pub options: Vec<CommandOption>,
    pub filter: String,
    pub filtered_indices: Vec<usize>,
    pub selected: usize,
}

#[cfg(test)]
pub mod test_support {
    use super::*;

    /// Mock for rendering tests. Has public fields matching every TuiState method.
    pub struct MockTuiState {
        pub messages: Vec<ChatMessage>,
        pub streaming_text: String,
        pub streaming_thought: Option<String>,
        pub active_tool_calls: Vec<TrackedToolCall>,
        pub current_plan: Option<cyril_core::types::Plan>,
        pub input_text: String,
        pub input_cursor: usize,
        pub autocomplete_suggestions: Vec<Suggestion>,
        pub autocomplete_selected: Option<usize>,
        pub activity: Activity,
        pub session_label: Option<String>,
        pub current_mode: Option<String>,
        pub current_model: Option<String>,
        pub context_usage: Option<f64>,
        pub credit_usage: Option<(f64, f64)>,
        pub approval: Option<ApprovalState>,
        pub picker: Option<PickerState>,
        pub terminal_size: (u16, u16),
        pub mouse_captured: bool,
        pub quit_requested: bool,
        pub activity_elapsed: Option<Duration>,
        pub deep_idle: bool,
    }

    impl Default for MockTuiState {
        fn default() -> Self {
            Self {
                messages: Vec::new(),
                streaming_text: String::new(),
                streaming_thought: None,
                active_tool_calls: Vec::new(),
                current_plan: None,
                input_text: String::new(),
                input_cursor: 0,
                autocomplete_suggestions: Vec::new(),
                autocomplete_selected: None,
                activity: Activity::Idle,
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
                activity_elapsed: None,
                deep_idle: false,
            }
        }
    }

    impl TuiState for MockTuiState {
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
            0
        }
        fn active_tool_calls(&self) -> &[TrackedToolCall] {
            &self.active_tool_calls
        }
        fn current_plan(&self) -> Option<&cyril_core::types::Plan> {
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
            self.activity_elapsed
        }
        fn is_deep_idle(&self) -> bool {
            self.deep_idle
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time proof that TuiState is object-safe.
    #[test]
    fn tui_state_is_object_safe() {
        fn _assert_object_safe(_: &dyn TuiState) {}
    }

    #[test]
    fn chat_message_user() {
        let msg = ChatMessage::user_text("hello".into());
        assert!(matches!(msg.kind(), ChatMessageKind::UserText(t) if t == "hello"));
    }

    #[test]
    fn chat_message_system() {
        let msg = ChatMessage::system("info".into());
        assert!(matches!(msg.kind(), ChatMessageKind::System(_)));
    }

    #[test]
    fn tracked_tool_call_accessors() {
        use cyril_core::types::*;
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "read".into(),
            Some("Reading file".into()),
            ToolKind::Read,
            ToolCallStatus::InProgress,
            None,
        );
        let tracked = TrackedToolCall::new(tc);
        assert_eq!(tracked.name(), "read");
        assert_eq!(tracked.title(), Some("Reading file"));
    }

    #[test]
    fn activity_default_is_idle() {
        assert_eq!(Activity::default(), Activity::Idle);
    }
}
