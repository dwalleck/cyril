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

    /// Minimal mock for rendering tests. Returns empty/default values.
    #[derive(Default)]
    pub struct MockTuiState;

    impl TuiState for MockTuiState {
        fn messages(&self) -> &[ChatMessage] {
            &[]
        }

        fn streaming_text(&self) -> &str {
            ""
        }

        fn streaming_thought(&self) -> Option<&str> {
            None
        }

        fn messages_version(&self) -> u64 {
            0
        }

        fn active_tool_calls(&self) -> &[TrackedToolCall] {
            &[]
        }

        fn current_plan(&self) -> Option<&Plan> {
            None
        }

        fn input_text(&self) -> &str {
            ""
        }

        fn input_cursor(&self) -> usize {
            0
        }

        fn autocomplete_suggestions(&self) -> &[Suggestion] {
            &[]
        }

        fn autocomplete_selected(&self) -> Option<usize> {
            None
        }

        fn activity(&self) -> Activity {
            Activity::Idle
        }

        fn session_label(&self) -> Option<&str> {
            None
        }

        fn current_mode(&self) -> Option<&str> {
            None
        }

        fn current_model(&self) -> Option<&str> {
            None
        }

        fn context_usage(&self) -> Option<f64> {
            None
        }

        fn credit_usage(&self) -> Option<(f64, f64)> {
            None
        }

        fn approval(&self) -> Option<&ApprovalState> {
            None
        }

        fn picker(&self) -> Option<&PickerState> {
            None
        }

        fn terminal_size(&self) -> (u16, u16) {
            (80, 24)
        }

        fn mouse_captured(&self) -> bool {
            false
        }

        fn should_quit(&self) -> bool {
            false
        }

        fn activity_elapsed(&self) -> Option<Duration> {
            None
        }

        fn is_deep_idle(&self) -> bool {
            false
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
