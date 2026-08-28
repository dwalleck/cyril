use std::time::Duration;

use cyril_core::types::{
    CommandOption, EffortLevel, HookInfo, MemoryStatusView, Plan, SessionId, VoiceStatus,
};

use crate::theme::Theme;

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
    /// Complete resolved appearance for this frame.
    fn theme(&self) -> Theme;

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
    /// Typed identity of the main session. Keep identity decisions on this
    /// value; `session_label()` is presentation text.
    fn main_session_id(&self) -> Option<&SessionId>;
    fn current_mode(&self) -> Option<&str>;
    fn current_model(&self) -> Option<&str>;
    /// Current thinking-effort level, if a thinking model is active and the
    /// agent has reported it. `None` otherwise. Borrowed so a backend-defined
    /// `EffortLevel::Other` string isn't cloned on every frame.
    fn effort(&self) -> Option<&EffortLevel>;
    /// Count of un-consumed queued steers (ROADMAP K1b). Drives the toolbar chip.
    fn steering_queued(&self) -> usize;
    /// Current voice-input status (ROADMAP CN2). Defaults to `Idle` for state
    /// impls that don't track voice (e.g. render-test mocks).
    fn voice_status(&self) -> VoiceStatus {
        VoiceStatus::Idle
    }
    /// Current voice input level in `0.0..=1.0`, meaningful while listening.
    fn voice_level(&self) -> f32 {
        0.0
    }
    fn memory_status(&self) -> &MemoryStatusView;
    fn context_usage(&self) -> Option<f64>;
    /// KAS categorized context breakdown for the toolbar bar (KAS-2b, cyril-5et2).
    /// `None` on v2 (scalar only) and before the first KAS `context_usage` frame.
    fn context_breakdown(&self) -> Option<&cyril_core::types::ContextBreakdown>;

    /// Stalled-turn display state (cyril-14ou), if the active turn is quiet
    /// past the bridge's threshold. `None` whenever traffic is flowing.
    fn stall(&self) -> Option<StallState>;
    fn credit_usage(&self) -> Option<(f64, f64)>;
    fn last_turn(&self) -> Option<&cyril_core::types::TurnSummary>;
    fn session_cost(&self) -> &cyril_core::types::SessionCost;

    // Overlays
    /// Return a shared view of the active approval only.
    ///
    /// Queue order is private to [`crate::state::UiState`]; external callers
    /// cannot reorder it:
    ///
    /// ```compile_fail,E0616
    /// use cyril_ui::state::UiState;
    ///
    /// let mut state = UiState::new(16);
    /// state.approvals.clear();
    /// ```
    ///
    /// The shared view also cannot consume its owned responder:
    ///
    /// ```compile_fail,E0507
    /// use cyril_core::types::PermissionResponse;
    /// use cyril_ui::traits::TuiState;
    ///
    /// fn cancel(state: &dyn TuiState) {
    ///     let Some(approval) = state.approval() else {
    ///         return;
    ///     };
    ///     drop(approval.responder.send(PermissionResponse::Cancel));
    /// }
    /// ```
    fn approval(&self) -> Option<&ApprovalState>;
    fn picker(&self) -> Option<&PickerState>;
    fn hooks_panel(&self) -> Option<&HooksPanelState>;
    fn code_panel(&self) -> Option<&cyril_core::types::CodePanelData>;
    fn usage_panel(&self) -> Option<&UsagePanelState>;
    fn code_intelligence_active(&self) -> bool;

    // Chat scroll
    fn chat_scroll_back(&self) -> Option<usize>;

    // Terminal
    fn terminal_size(&self) -> (u16, u16);
    fn mouse_captured(&self) -> bool;
    fn should_quit(&self) -> bool;

    // Timing
    fn activity_elapsed(&self) -> Option<Duration>;
    fn is_deep_idle(&self) -> bool;

    // Subagents
    fn subagent_tracker(&self) -> &cyril_core::subagent::SubagentTracker;
    fn subagent_ui(&self) -> &crate::subagent_ui::SubagentUiState;
}

/// A chat message for display purposes.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub kind: ChatMessageKind,
    pub timestamp: std::time::Instant,
}

/// Lifecycle of a queue-steer the user sent (ROADMAP K1b, cyril-bm1j). The echo
/// is added optimistically on send (`Queued`) and reconciled in place as the
/// wire echoes arrive: `Applied` on `SteeringConsumed`, `Cleared` on
/// `SteeringCleared`, `Unsupported` on `SteeringUnsupported`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteerEchoStatus {
    Queued,
    Applied,
    Cleared,
    Unsupported,
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
    /// A queue-steer the user sent, with its reconciled lifecycle status
    /// (ROADMAP K1b). `text` is the user's own steer message. `message_id` is
    /// the backend queue id, bound in place when the wire `SteeringQueued`
    /// echo arrives (new-family v2 / KAS carry ids; `None` until then and
    /// forever on the old id-less dialect — cyril-vgcm C8). Not rendered;
    /// used only to reconcile id-scoped Consumed/Cleared echoes.
    ///
    /// `note` is the model's own account of how it handled the steer, harvested
    /// from the `[STEERING <id>: …]` trailer it is instructed to emit
    /// (cyril-3qwa). `None` until that trailer arrives — and forever if the
    /// model declines to emit one, so the UI must degrade cleanly without it.
    SteerEcho {
        text: String,
        status: SteerEchoStatus,
        message_id: Option<String>,
        note: Option<String>,
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

    /// A queue-steer echo, optimistically `Queued` (ROADMAP K1b, cyril-bm1j).
    /// `message_id` starts `None` — the wire `SteeringQueued` echo binds it
    /// later (cyril-vgcm C8). `note` starts `None` — the model's
    /// `[STEERING <id>: …]` trailer fills it if one arrives (cyril-3qwa).
    pub fn steer_echo(text: String) -> Self {
        Self {
            kind: ChatMessageKind::SteerEcho {
                text,
                status: SteerEchoStatus::Queued,
                message_id: None,
                note: None,
            },
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

    /// Merge update fields into the existing tool call.
    /// Only overwrites fields that the update provides — preserves
    /// content and locations from the initial ToolCall if the update
    /// doesn't carry them.
    pub fn update(&mut self, tc: &cyril_core::types::ToolCall) {
        self.inner.merge_update(tc);
    }

    pub fn id(&self) -> &cyril_core::types::ToolCallId {
        self.inner.id()
    }

    pub fn kind(&self) -> cyril_core::types::ToolKind {
        self.inner.kind()
    }

    pub fn status(&self) -> cyril_core::types::ToolCallStatus {
        self.inner.status()
    }

    /// The human-readable display text from ACP (e.g., "Reading main.rs").
    pub fn title(&self) -> &str {
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

    /// Access the structured output from tool execution.
    pub fn raw_output(&self) -> Option<&serde_json::Value> {
        self.inner.raw_output()
    }

    /// Extract displayable text from raw_output.
    ///
    /// Tries the following strategies in order:
    /// 1. Plain string value
    /// 2. Shell commands: `stdout` (non-empty), then `stderr` (non-empty)
    /// 3. Kiro item envelope: `items[0].Text`
    /// 4. Kiro item envelope: `items[0].Json` (pretty-printed as JSON)
    /// 5. Direct text fields: `text`, `content`, `result`
    /// 6. Non-object values (arrays, numbers, bools): JSON serialization
    ///
    /// Inspired by the `unwrapResultOutput` / `extractResultText` pattern in
    /// tui.js, with adaptations for the Rust context.
    pub fn output_text(&self) -> Option<String> {
        let output = self.inner.raw_output()?;

        if let Some(s) = output.as_str() {
            return Some(s.to_string());
        }

        let obj = match output.as_object() {
            Some(o) => o,
            None => {
                return serde_json::to_string_pretty(output).ok();
            }
        };

        if let Some(stdout) = obj.get("stdout").and_then(|v| v.as_str())
            && !stdout.trim().is_empty()
        {
            return Some(stdout.to_string());
        }
        if let Some(stderr) = obj.get("stderr").and_then(|v| v.as_str())
            && !stderr.trim().is_empty()
        {
            return Some(stderr.to_string());
        }

        if let Some(items) = obj.get("items").and_then(|v| v.as_array())
            && let Some(first) = items.first()
        {
            if let Some(text) = first.get("Text").and_then(|v| v.as_str()) {
                return Some(text.to_string());
            }
            if let Some(json_val) = first.get("Json") {
                return serde_json::to_string_pretty(json_val).ok();
            }
        }

        for key in ["text", "content", "result"] {
            if let Some(s) = obj.get(key).and_then(|v| v.as_str()) {
                return Some(s.to_string());
            }
        }

        None
    }

    /// Extract exit code from raw_output for Execute-kind tool calls.
    pub fn exit_code(&self) -> Option<i64> {
        if self.inner.kind() != cyril_core::types::ToolKind::Execute {
            return None;
        }
        let output = self.inner.raw_output()?;
        let obj = output.as_object()?;
        obj.get("exit_status").and_then(|v| v.as_i64())
    }

    /// Extract error message when tool call failed.
    pub fn error_message(&self) -> Option<String> {
        if self.inner.status() != cyril_core::types::ToolCallStatus::Failed {
            return None;
        }
        let output = self.inner.raw_output()?;
        if let Some(s) = output.as_str() {
            return Some(s.to_string());
        }
        if let Some(obj) = output.as_object() {
            for key in ["error", "message"] {
                if let Some(s) = obj.get(key).and_then(|v| v.as_str()) {
                    return Some(s.to_string());
                }
            }
        }
        // Fall back to output_text() for any displayable content
        self.output_text()
    }
}

/// Autocomplete suggestion for input.
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub text: String,
    pub description: Option<String>,
}

/// The current phase of the approval dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalPhase {
    /// Phase 1: Select allow/reject option.
    SelectOption,
    /// Phase 2: Select trust tier (shown after AllowAlways when trust options
    /// exist). Carries the id of the phase-1 AllowAlways pick — in this phase
    /// `selected` re-indexes `trust_options`, so the eventual reply's option
    /// id can only come from here.
    SelectTrust {
        chosen_option_id: cyril_core::types::PermissionOptionId,
    },
}
/// User-facing label for an approval's exact wire-origin session id.
///
/// An empty id is invalid but can still arrive from the untrusted ACP
/// boundary. Preserve it in the domain type and project it honestly here.
pub fn approval_origin_label(origin: &SessionId) -> &str {
    if origin.as_str().is_empty() {
        "unknown session"
    } else {
        origin.as_str()
    }
}

/// Stalled-turn display state (cyril-14ou; CONTEXT.md "Stalled turn"): the
/// active turn has gone quiet past the bridge's threshold. Display-only — a
/// stalled turn is still a live turn (a captured one completed 16 minutes
/// late), so nothing here touches busy or input handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StallState {
    /// Quiet duration the bridge reported when the signal fired.
    pub quiet: std::time::Duration,
    /// When the chip went up — the renderer animates a live counter as
    /// `quiet + since.elapsed()` (the bridge sends one signal per quiet
    /// period; the ticking is display-side).
    pub since: std::time::Instant,
    /// The user pressed Esc while stalled — the cancel went out, but the
    /// engine may not be able to honor it mid-stall (cyril-w9oi is the
    /// second-tier escape). Escalates the chip wording.
    pub cancel_sent: bool,
}

/// Permission approval dialog state.
#[derive(Debug)]
pub struct ApprovalState {
    pub session_id: cyril_core::types::SessionId,
    pub tool_call: TrackedToolCall,
    pub message: String,
    pub options: Vec<cyril_core::types::PermissionOption>,
    pub trust_options: Vec<cyril_core::types::TrustOption>,
    pub selected: usize,
    pub phase: ApprovalPhase,
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

/// Hooks panel overlay state (read-only table display for `/hooks` command).
///
/// Populated from the `hooks` command response (`data.hooks[]`). The panel is
/// purely informational — hooks execute in `kiro-cli-chat`, not in Cyril, so
/// this struct carries no interactive state beyond scroll position.
#[derive(Debug, Clone)]
pub struct HooksPanelState {
    /// Hook list in `(trigger, command)` lexicographic order. Pre-sorted by
    /// [`crate::state::UiState::show_hooks_panel`]; the renderer iterates
    /// this directly without re-sorting. A caller that constructs
    /// `HooksPanelState` outside that method is responsible for sorting —
    /// an unsorted `Vec` will render in insertion order and break the
    /// widget's alphabetical-grouping convention.
    pub hooks: Vec<HookInfo>,
    pub scroll_offset: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UsagePage {
    #[default]
    Overview,
    Costs,
    Context,
    Providers,
    Models,
    Tools,
    Recent,
    Errors,
    Folders,
}

impl UsagePage {
    pub const ALL: [Self; 9] = [
        Self::Overview,
        Self::Costs,
        Self::Context,
        Self::Providers,
        Self::Models,
        Self::Tools,
        Self::Recent,
        Self::Errors,
        Self::Folders,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Costs => "Costs",
            Self::Context => "Context",
            Self::Providers => "Providers",
            Self::Models => "Models",
            Self::Tools => "Tools",
            Self::Recent => "Recent",
            Self::Errors => "Errors",
            Self::Folders => "Folders",
        }
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn previous(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    fn index(self) -> usize {
        match self {
            Self::Overview => 0,
            Self::Costs => 1,
            Self::Context => 2,
            Self::Providers => 3,
            Self::Models => 4,
            Self::Tools => 5,
            Self::Recent => 6,
            Self::Errors => 7,
            Self::Folders => 8,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum UsageAccountStatus {
    #[default]
    Idle,
    Loading,
    Refreshing,
    Fresh,
    Unavailable(String),
    Stale(String),
}

#[derive(Debug, Clone)]
pub struct UsagePanelState {
    pub snapshot: cyril_core::types::UsageSnapshot,
    pub page: UsagePage,
    pub scroll_offset: usize,
    pub account: Option<cyril_core::types::UsageAccount>,
    pub account_fetched_at_ms: Option<u64>,
    pub account_status: UsageAccountStatus,
}

impl UsagePanelState {
    pub fn row_count(&self) -> usize {
        match self.page {
            UsagePage::Overview => {
                if self.snapshot.overview.requests == 0 {
                    1
                } else {
                    // 16 metric lines + the two latency-tail lines added with
                    // p90/max (cyril-9kyk). Asserted against the rendered line
                    // count by `overview_row_count_matches_rendered_lines`.
                    18
                }
            }
            UsagePage::Costs => {
                let model_rows = self
                    .snapshot
                    .models
                    .iter()
                    .filter(|group| {
                        !group.summary.costs.is_empty() || !group.summary.charges.is_empty()
                    })
                    .count();
                3 + model_rows
                    + self.account.as_ref().map_or(0, |account| {
                        3 + account.usage_breakdowns.len()
                            + account.bonus_credits.len()
                            + account.add_on_credits.len()
                    })
            }
            UsagePage::Context => self
                .snapshot
                .context
                .latest
                .as_ref()
                .map_or(1, |latest| if latest.breakdown.is_some() { 10 } else { 6 }),
            UsagePage::Providers => self.snapshot.providers.len(),
            UsagePage::Tools => self
                .snapshot
                .tools
                .iter()
                .map(|group| 1 + group.models.len())
                .sum::<usize>()
                .max(1),
            UsagePage::Models => self.snapshot.models.len(),
            UsagePage::Recent => self.snapshot.recent.len(),
            UsagePage::Errors => self.snapshot.errors.len(),
            UsagePage::Folders => self.snapshot.folders.len(),
        }
    }
}

#[cfg(test)]
pub mod test_support {
    use super::*;
    use ratatui::style::Color;

    pub fn marker_theme() -> Theme {
        Theme {
            syntax: None,
            canvas: Color::Indexed(1),
            chrome: Color::Indexed(2),
            code: Color::Indexed(3),
            selection: Color::Indexed(4),
            text: Color::Indexed(5),
            muted: Color::Indexed(6),
            border: Color::Indexed(7),
            accent: Color::Indexed(8),
            accent_alt: Color::Indexed(9),
            user: Color::Indexed(10),
            agent: Color::Indexed(11),
            system: Color::Indexed(12),
            info: Color::Indexed(13),
            success: Color::Indexed(14),
            warning: Color::Indexed(15),
            danger: Color::Indexed(16),
            diff_add: Color::Indexed(17),
            diff_delete: Color::Indexed(18),
            diff_context: Color::Indexed(19),
            emphasis: Color::Indexed(20),
            accent_tertiary: Color::Indexed(21),
            accent_quaternary: Color::Indexed(22),
            accent_quinary: Color::Indexed(23),
            subdued: Color::Indexed(24),
            subdued_positive: Color::Indexed(25),
            subdued_negative: Color::Indexed(26),
            soft_accent: Color::Indexed(27),
            positive_accent: Color::Indexed(28),
            inset_background: Color::Indexed(29),
            text_secondary: Color::Indexed(30),
            accent_violet: Color::Indexed(31),
        }
    }

    /// Mock for rendering tests. Has public fields matching every TuiState method.
    pub struct MockTuiState {
        pub theme: Theme,
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
        pub main_session_id: Option<SessionId>,
        pub current_mode: Option<String>,
        pub current_model: Option<String>,
        pub effort: Option<EffortLevel>,
        pub steering_queued: usize,
        pub memory_status: MemoryStatusView,
        pub context_usage: Option<f64>,
        pub context_breakdown: Option<cyril_core::types::ContextBreakdown>,
        pub stall: Option<StallState>,
        pub credit_usage: Option<(f64, f64)>,
        pub last_turn: Option<cyril_core::types::TurnSummary>,
        pub session_cost: cyril_core::types::SessionCost,
        pub approval: Option<ApprovalState>,
        pub picker: Option<PickerState>,
        pub hooks_panel: Option<HooksPanelState>,
        pub code_panel: Option<cyril_core::types::CodePanelData>,
        pub usage_panel: Option<UsagePanelState>,
        pub code_intelligence_active: bool,
        pub chat_scroll_back: Option<usize>,
        pub terminal_size: (u16, u16),
        pub mouse_captured: bool,
        pub quit_requested: bool,
        pub activity_elapsed: Option<Duration>,
        pub deep_idle: bool,
        pub subagent_tracker: cyril_core::subagent::SubagentTracker,
        pub subagent_ui: crate::subagent_ui::SubagentUiState,
    }

    impl Default for MockTuiState {
        fn default() -> Self {
            Self {
                theme: marker_theme(),
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
                main_session_id: None,
                current_mode: None,
                current_model: None,
                effort: None,
                steering_queued: 0,
                memory_status: MemoryStatusView::default(),
                context_usage: None,
                context_breakdown: None,
                stall: None,
                credit_usage: None,
                last_turn: None,
                session_cost: cyril_core::types::SessionCost::new(),
                approval: None,
                picker: None,
                hooks_panel: None,
                code_panel: None,
                usage_panel: None,
                code_intelligence_active: false,
                chat_scroll_back: None,
                terminal_size: (80, 24),
                mouse_captured: false,
                quit_requested: false,
                activity_elapsed: None,
                deep_idle: false,
                subagent_tracker: cyril_core::subagent::SubagentTracker::new(),
                subagent_ui: crate::subagent_ui::SubagentUiState::new(),
            }
        }
    }

    impl TuiState for MockTuiState {
        fn theme(&self) -> Theme {
            self.theme
        }
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
        fn main_session_id(&self) -> Option<&SessionId> {
            self.main_session_id.as_ref()
        }
        fn current_mode(&self) -> Option<&str> {
            self.current_mode.as_deref()
        }
        fn current_model(&self) -> Option<&str> {
            self.current_model.as_deref()
        }
        fn effort(&self) -> Option<&EffortLevel> {
            self.effort.as_ref()
        }
        fn steering_queued(&self) -> usize {
            self.steering_queued
        }
        fn memory_status(&self) -> &MemoryStatusView {
            &self.memory_status
        }
        fn context_usage(&self) -> Option<f64> {
            self.context_usage
        }
        fn context_breakdown(&self) -> Option<&cyril_core::types::ContextBreakdown> {
            self.context_breakdown.as_ref()
        }
        fn stall(&self) -> Option<StallState> {
            self.stall
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
        fn usage_panel(&self) -> Option<&UsagePanelState> {
            self.usage_panel.as_ref()
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
            self.activity_elapsed
        }
        fn is_deep_idle(&self) -> bool {
            self.deep_idle
        }
        fn subagent_tracker(&self) -> &cyril_core::subagent::SubagentTracker {
            &self.subagent_tracker
        }
        fn subagent_ui(&self) -> &crate::subagent_ui::SubagentUiState {
            &self.subagent_ui
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// Compile-time proof that TuiState is object-safe.
    #[test]
    fn tui_state_is_object_safe() {
        fn _assert_object_safe(_: &dyn TuiState) {}
    }

    #[test]
    fn approval_origin_label_preserves_valid_ids_and_names_empty_ids() {
        assert_eq!(
            approval_origin_label(&SessionId::new("peer-session")),
            "peer-session"
        );
        assert_eq!(
            approval_origin_label(&SessionId::new("")),
            "unknown session"
        );
    }

    #[test]
    fn mock_tui_state_observes_every_marker_theme_field() {
        let state = test_support::MockTuiState::default();
        assert_eq!(state.theme(), test_support::marker_theme());

        let theme = state.theme();
        let colors = [
            theme.canvas,
            theme.chrome,
            theme.code,
            theme.selection,
            theme.text,
            theme.muted,
            theme.border,
            theme.accent,
            theme.accent_alt,
            theme.user,
            theme.agent,
            theme.system,
            theme.info,
            theme.success,
            theme.warning,
            theme.danger,
            theme.diff_add,
            theme.diff_delete,
            theme.diff_context,
            theme.emphasis,
            theme.accent_tertiary,
            theme.accent_quaternary,
            theme.accent_quinary,
            theme.subdued,
            theme.subdued_positive,
            theme.subdued_negative,
            theme.soft_accent,
            theme.positive_accent,
            theme.inset_background,
        ];
        for (index, color) in colors.iter().enumerate() {
            assert!(!colors[index + 1..].contains(color));
        }
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
            "Reading file".into(),
            ToolKind::Read,
            ToolCallStatus::InProgress,
            None,
        );
        let tracked = TrackedToolCall::new(tc);
        assert_eq!(tracked.title(), "Reading file");
    }

    #[test]
    fn activity_default_is_idle() {
        assert_eq!(Activity::default(), Activity::Idle);
    }

    #[test]
    fn tracked_tool_call_raw_output_accessor() {
        use cyril_core::types::*;
        let output = serde_json::json!({"stdout": "hello\nworld", "exit_status": 0});
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "Running cargo test".into(),
            ToolKind::Execute,
            ToolCallStatus::Completed,
            None,
        )
        .with_raw_output(Some(output.clone()));
        let tracked = TrackedToolCall::new(tc);
        assert_eq!(tracked.raw_output(), Some(&output));
    }

    #[test]
    fn tracked_tool_call_output_text_shell() {
        use cyril_core::types::*;
        let output = serde_json::json!({"stdout": "hello world", "exit_status": 0});
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "shell".into(),
            ToolKind::Execute,
            ToolCallStatus::Completed,
            None,
        )
        .with_raw_output(Some(output));
        let tracked = TrackedToolCall::new(tc);
        assert_eq!(tracked.output_text(), Some("hello world".to_string()));
    }

    #[test]
    fn tracked_tool_call_output_text_stderr_fallback() {
        use cyril_core::types::*;
        let output = serde_json::json!({"stdout": "", "stderr": "error output", "exit_status": 1});
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "shell".into(),
            ToolKind::Execute,
            ToolCallStatus::Completed,
            None,
        )
        .with_raw_output(Some(output));
        let tracked = TrackedToolCall::new(tc);
        assert_eq!(tracked.output_text(), Some("error output".to_string()));
    }

    #[test]
    fn tracked_tool_call_output_text_items_text() {
        use cyril_core::types::*;
        let output = serde_json::json!({"items": [{"Text": "file contents here"}]});
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "read".into(),
            ToolKind::Read,
            ToolCallStatus::Completed,
            None,
        )
        .with_raw_output(Some(output));
        let tracked = TrackedToolCall::new(tc);
        assert_eq!(
            tracked.output_text(),
            Some("file contents here".to_string())
        );
    }

    #[test]
    fn tracked_tool_call_output_text_plain_string() {
        use cyril_core::types::*;
        let output = serde_json::json!("plain text output");
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "tool".into(),
            ToolKind::Other,
            ToolCallStatus::Completed,
            None,
        )
        .with_raw_output(Some(output));
        let tracked = TrackedToolCall::new(tc);
        assert_eq!(tracked.output_text(), Some("plain text output".to_string()));
    }

    #[test]
    fn tracked_tool_call_output_text_none_without_raw_output() {
        use cyril_core::types::*;
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "read".into(),
            ToolKind::Read,
            ToolCallStatus::Completed,
            None,
        );
        let tracked = TrackedToolCall::new(tc);
        assert!(tracked.output_text().is_none());
    }

    #[test]
    fn tracked_tool_call_exit_code() {
        use cyril_core::types::*;
        let output = serde_json::json!({"stdout": "", "exit_status": 1});
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "shell".into(),
            ToolKind::Execute,
            ToolCallStatus::Completed,
            None,
        )
        .with_raw_output(Some(output));
        let tracked = TrackedToolCall::new(tc);
        assert_eq!(tracked.exit_code(), Some(1));
    }

    #[test]
    fn tracked_tool_call_exit_code_none_for_non_execute() {
        use cyril_core::types::*;
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "read".into(),
            ToolKind::Read,
            ToolCallStatus::Completed,
            None,
        );
        let tracked = TrackedToolCall::new(tc);
        assert!(tracked.exit_code().is_none());
    }

    #[test]
    fn tracked_tool_call_error_message_on_failed() {
        use cyril_core::types::*;
        let output = serde_json::json!("Command timed out");
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "shell".into(),
            ToolKind::Execute,
            ToolCallStatus::Failed,
            None,
        )
        .with_raw_output(Some(output));
        let tracked = TrackedToolCall::new(tc);
        assert_eq!(
            tracked.error_message(),
            Some("Command timed out".to_string())
        );
    }

    #[test]
    fn tracked_tool_call_error_message_none_when_not_failed() {
        use cyril_core::types::*;
        let output = serde_json::json!({"stdout": "ok"});
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "shell".into(),
            ToolKind::Execute,
            ToolCallStatus::Completed,
            None,
        )
        .with_raw_output(Some(output));
        let tracked = TrackedToolCall::new(tc);
        assert!(tracked.error_message().is_none());
    }

    #[test]
    fn tracked_tool_call_error_message_from_object() {
        use cyril_core::types::*;
        let output = serde_json::json!({"error": "permission denied"});
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "write".into(),
            ToolKind::Write,
            ToolCallStatus::Failed,
            None,
        )
        .with_raw_output(Some(output));
        let tracked = TrackedToolCall::new(tc);
        assert_eq!(
            tracked.error_message(),
            Some("permission denied".to_string())
        );
    }

    #[test]
    fn tracked_tool_call_output_text_items_json() {
        use cyril_core::types::*;
        let output = serde_json::json!({"items": [{"Json": {"key": "value"}}]});
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "tool".into(),
            ToolKind::Other,
            ToolCallStatus::Completed,
            None,
        )
        .with_raw_output(Some(output));
        let tracked = TrackedToolCall::new(tc);
        let text = tracked.output_text().expect("should extract Json item");
        assert!(text.contains("\"key\""));
        assert!(text.contains("\"value\""));
    }

    #[test]
    fn tracked_tool_call_output_text_text_field() {
        use cyril_core::types::*;
        let output = serde_json::json!({"text": "plain output"});
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "tool".into(),
            ToolKind::Other,
            ToolCallStatus::Completed,
            None,
        )
        .with_raw_output(Some(output));
        let tracked = TrackedToolCall::new(tc);
        assert_eq!(tracked.output_text(), Some("plain output".to_string()));
    }

    #[test]
    fn tracked_tool_call_output_text_unknown_object_returns_none() {
        use cyril_core::types::*;
        let output = serde_json::json!({"unknownKey": "data"});
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "tool".into(),
            ToolKind::Other,
            ToolCallStatus::Completed,
            None,
        )
        .with_raw_output(Some(output));
        let tracked = TrackedToolCall::new(tc);
        assert!(tracked.output_text().is_none());
    }

    #[test]
    fn tracked_tool_call_output_text_json_array_falls_through() {
        use cyril_core::types::*;
        let output = serde_json::json!([1, 2, 3]);
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "tool".into(),
            ToolKind::Other,
            ToolCallStatus::Completed,
            None,
        )
        .with_raw_output(Some(output));
        let tracked = TrackedToolCall::new(tc);
        let text = tracked
            .output_text()
            .expect("should serialize array as JSON");
        assert!(text.contains("1"));
    }

    #[test]
    fn tracked_tool_call_error_message_falls_back_to_output_text() {
        use cyril_core::types::*;
        let output = serde_json::json!({"stderr": "permission denied", "exit_status": 1});
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "shell".into(),
            ToolKind::Execute,
            ToolCallStatus::Failed,
            None,
        )
        .with_raw_output(Some(output));
        let tracked = TrackedToolCall::new(tc);
        assert_eq!(
            tracked.error_message(),
            Some("permission denied".to_string())
        );
    }

    #[test]
    fn tracked_tool_call_error_message_from_message_key() {
        use cyril_core::types::*;
        let output = serde_json::json!({"message": "not found"});
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "tool".into(),
            ToolKind::Other,
            ToolCallStatus::Failed,
            None,
        )
        .with_raw_output(Some(output));
        let tracked = TrackedToolCall::new(tc);
        assert_eq!(tracked.error_message(), Some("not found".to_string()));
    }
}
