use std::borrow::Cow;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use cyril_core::types::{
    CommandOption, EffortLevel, HookInfo, MemoryStatusView, Plan, PowerInfo, SessionId, VoiceStatus,
};

pub use crate::feedback_editor::RejectionFeedback;
use crate::message_history::{LiveRevision, MessageHistory};
use crate::theme::Theme;
use crate::tool_diff::{DiffPreview, DiffSource};

#[cfg(test)]
std::thread_local! {
    static DIFF_SELECTION_STEPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn diff_selection_steps() -> usize {
    DIFF_SELECTION_STEPS.with(std::cell::Cell::get)
}

/// Which transcript a line belongs in (cyril-lki9): the main conversation, or
/// the stream of a workflow step / subagent session. The App decides which —
/// it owns session routing — and the UI renders into it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transcript<'a> {
    Main,
    Workflow(&'a cyril_core::types::SessionId),
    Subagent(&'a cyril_core::types::SessionId),
}

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
    fn history(&self) -> &MessageHistory;
    fn streaming_text(&self) -> &str;
    fn streaming_thought(&self) -> Option<&str>;
    fn live_revision(&self) -> LiveRevision;

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
    /// Extended thinking on/off for the current model (cyril-k3lz) — `Some`
    /// only when the model is toggleable AND the engine reported the state.
    /// Unknown, always-on and non-toggleable models are all `None`, so the
    /// toolbar never presents an unknown state as on.
    fn thinking_enabled(&self) -> Option<bool>;
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
    fn review_form(&self) -> Option<&ReviewForm>;
    fn hooks_panel(&self) -> Option<&HooksPanelState>;
    fn powers_panel(&self) -> Option<&PowersPanelState>;
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

/// Which modal overlay owns the keyboard — and, in the reverse order, which one
/// is painted on top.
///
/// Overlays stack: several can be open at once (`/model` while a permission
/// request is pending, a powers panel under an approval), and each widget
/// paints its own [`ratatui::widgets::Clear`] before its content. Whoever paints
/// last therefore *erases* the others inside the overlap, so the paint order and
/// the key-dispatch order MUST be inverses of each other. They were not: both
/// chains ran approval-first, which painted the approval prompt at the bottom
/// (erased by the next panel's `Clear`) while `App::handle_key` still fed it
/// every key — Enter could answer a prompt the user could not see (review
/// findings 3 and 20).
///
/// [`Overlay::ALL`] is the one place the stack order lives. Rendering iterates
/// it front-to-back; the key chain takes the last open layer
/// ([`crate::state::UiState::topmost_overlay`]). Adding a variant breaks both
/// [`crate::render`] and `App::handle_key` at compile time — an overlay cannot
/// be added without deciding where it sits in the stack and which keys it eats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    /// Usage panel (`/usage`).
    Usage,
    /// Code panel (`/code`).
    Code,
    /// Powers panel (`/powers`).
    Powers,
    /// Hooks panel (`/hooks`).
    Hooks,
    /// Command-option picker (`/model`, `/theme`, and every
    /// `CommandOptionsReceived` reply, which also arrives unprompted).
    Picker,
    /// The `/review` consent form (cyril-iowg). Directly below Approval: a
    /// permission prompt from another session still answers first.
    Review,
    /// Permission prompt. Painted last and consulted first: an approval the
    /// user cannot see must never be answerable by a stray Enter.
    Approval,
}

impl Overlay {
    /// Every overlay, bottom-most (painted first, keyed last) to top-most.
    pub const ALL: [Overlay; 7] = [
        Overlay::Usage,
        Overlay::Code,
        Overlay::Powers,
        Overlay::Hooks,
        Overlay::Picker,
        Overlay::Review,
        Overlay::Approval,
    ];
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

/// Which body an approval preview shows for a tool call (cyril-j1b3).
///
/// Classification only — no logging — so the renderer can call it every
/// frame; the one-time malformed-shape warning is logged where the approval
/// is installed (`UiState::show_approval`, cyril-q0kg).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewBody<'a> {
    /// Diff content, rendered by the diff projection.
    Diff,
    /// The tool call's content blocks, every one of them Text (a diff would
    /// have classified as [`PreviewBody::Diff`]): the request's own
    /// explanation, such as a KAS workflow plan (cyril-8wh0).
    Content(&'a [cyril_core::types::ToolCallContent]),
    /// The raw-input string `text` (or, failing that, `content`) payload.
    Text(&'a str),
    /// No raw input and no diff: nothing to show.
    Absent,
    /// Raw input present without a string `text`/`content` field. The value
    /// is never coerced; only its JSON shape is reported.
    Malformed { raw_input_shape: &'static str },
}

/// One-word JSON shape tag for the malformed-raw_input log (never the value
/// itself, which can carry file contents).
fn json_shape(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Object(_) => "object",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Null => "null",
    }
}

/// The authoritative branch selected while preparing a tool's display text.
/// Resolving it never reruns precedence checks or serializes JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolTextSource {
    RawOutput,
    RawField(&'static str),
    ItemText,
    Content(usize),
}

#[derive(Debug)]
pub(crate) enum ToolText<'a> {
    Borrowed {
        source: ToolTextSource,
        text: &'a str,
    },
    Derived(String),
}

impl<'a> ToolText<'a> {
    fn into_cow(self) -> Cow<'a, str> {
        match self {
            Self::Borrowed { text, .. } => Cow::Borrowed(text),
            Self::Derived(text) => Cow::Owned(text),
        }
    }
}

enum RawOutputPolicy {
    /// Preserve the display projection's blank strings and omit error-only fields.
    Display,
    /// Prefer explicit errors and skip blank string candidates during selection.
    Error,
}

/// A tool call enriched for display (wraps `cyril_core::types::ToolCall`).
#[derive(Debug, Clone)]
pub struct TrackedToolCall {
    inner: cyril_core::types::ToolCall,
    diff_preview: OnceLock<Arc<DiffPreview>>,
}

impl TrackedToolCall {
    pub fn new(tc: cyril_core::types::ToolCall) -> Self {
        Self {
            inner: tc,
            diff_preview: OnceLock::new(),
        }
    }

    /// Merge update fields into the existing tool call.
    /// Only overwrites fields that the update provides — preserves
    /// content and locations from the initial ToolCall if the update
    /// doesn't carry them.
    pub fn update(&mut self, tc: &cyril_core::types::ToolCall) {
        if !tc.content().is_empty()
            && self.diff_source() != Self::selected_diff(tc.content()).map(|(_, source)| source)
        {
            self.diff_preview = OnceLock::new();
        }
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

    /// One immutable derivation per effective first-Diff source. Cloned
    /// initialized wrappers share it; changing one wrapper detaches its slot.
    pub(crate) fn diff_preview(&self) -> &DiffPreview {
        self.diff_preview
            .get_or_init(|| Arc::new(DiffPreview::new(self.diff_source())))
    }

    /// Select once during preparation, initializing even an absent preview.
    /// The ordinal belongs to the current content, not the cached derivation.
    pub(crate) fn diff_view(&self) -> Option<(usize, DiffSource<'_>, &DiffPreview)> {
        let selected = Self::selected_diff(self.content());
        let preview = self
            .diff_preview
            .get_or_init(|| Arc::new(DiffPreview::new(selected.map(|(_, source)| source))));
        selected.map(|(ordinal, source)| (ordinal, source, preview.as_ref()))
    }

    pub(crate) fn diff_source(&self) -> Option<DiffSource<'_>> {
        Self::selected_diff(self.content()).map(|(_, source)| source)
    }

    fn selected_diff(
        content: &[cyril_core::types::ToolCallContent],
    ) -> Option<(usize, DiffSource<'_>)> {
        content.iter().enumerate().find_map(|(ordinal, block)| {
            #[cfg(test)]
            DIFF_SELECTION_STEPS.with(|steps| steps.set(steps.get() + 1));
            match block {
                cyril_core::types::ToolCallContent::Diff {
                    old_text, new_text, ..
                } => Some((
                    ordinal,
                    DiffSource {
                        old: old_text.as_deref().unwrap_or(""),
                        new: new_text,
                    },
                )),
                cyril_core::types::ToolCallContent::Text(_) => None,
            }
        })
    }

    pub fn locations(&self) -> &[cyril_core::types::ToolCallLocation] {
        self.inner.locations()
    }

    /// Get the primary file path from locations, then from diff content, then from raw_input.
    ///
    /// The raw_input probe tries the flat `file_path` / `path` keys first, then
    /// the v2 `fs_read` shape, which nests the path under `operations[0].path`
    /// (docs/kiro-2.21.2-wire-audit.md § 6b, cyril-gl0m). Within that probe a
    /// value that is not a string falls through to the next key, and an empty
    /// string is reported as absent (`None`) rather than as `Some("")`; the
    /// `locations` and diff branches return their paths as-is.
    pub fn primary_path(&self) -> Option<&str> {
        if let Some(loc) = self.inner.locations().first() {
            return Some(&loc.path);
        }
        for c in self.inner.content() {
            if let cyril_core::types::ToolCallContent::Diff { path, .. } = c {
                return Some(path);
            }
        }
        let raw = self.inner.raw_input()?;
        raw.get("file_path")
            .and_then(serde_json::Value::as_str)
            .or_else(|| raw.get("path").and_then(serde_json::Value::as_str))
            .or_else(|| raw.get("operations")?.get(0)?.get("path")?.as_str())
            .filter(|path| !path.is_empty())
    }

    /// Classify the approval-preview body: diff content wins, then Text
    /// content blocks (cyril-8wh0), then a string raw-input `text`/`content`
    /// payload; otherwise absent or malformed.
    pub fn preview_body(&self) -> PreviewBody<'_> {
        let content = self.inner.content();
        let has_diff = content
            .iter()
            .any(|c| matches!(c, cyril_core::types::ToolCallContent::Diff { .. }));
        if has_diff {
            return PreviewBody::Diff;
        }
        // Blank Text explains nothing; it must not hide the raw-input
        // fallback or the degraded marker (cyril-j1b3).
        let has_text = content.iter().any(|c| {
            matches!(c, cyril_core::types::ToolCallContent::Text(text) if !text.trim().is_empty())
        });
        if has_text {
            return PreviewBody::Content(content);
        }
        let Some(raw) = self.inner.raw_input() else {
            return PreviewBody::Absent;
        };
        match raw
            .get("text")
            .or_else(|| raw.get("content"))
            .and_then(serde_json::Value::as_str)
        {
            Some(text) => PreviewBody::Text(text),
            None => PreviewBody::Malformed {
                raw_input_shape: json_shape(raw),
            },
        }
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

    /// Extract displayable text without copying authoritative JSON strings.
    /// Only JSON pretty-printing requires owned derived text.
    pub fn output_text(&self) -> Option<Cow<'_, str>> {
        self.output_text_view().map(ToolText::into_cow)
    }

    /// Select display text once; retained rows record the selected source.
    ///
    /// Precedence remains plain string, nonblank stdout/stderr, first item
    /// Text/Json, direct text/content/result/output, then non-object serialization.
    pub(crate) fn output_text_view(&self) -> Option<ToolText<'_>> {
        self.raw_output_text_view(RawOutputPolicy::Display)
    }

    fn raw_output_text_view(&self, policy: RawOutputPolicy) -> Option<ToolText<'_>> {
        let skip_blank = matches!(policy, RawOutputPolicy::Error);
        let output = self.inner.raw_output()?;
        if let Some(text) = output.as_str() {
            return (!skip_blank || !text.trim().is_empty()).then_some(ToolText::Borrowed {
                source: ToolTextSource::RawOutput,
                text,
            });
        }
        let Some(object) = output.as_object() else {
            return serialize_tool_text(output);
        };

        if skip_blank {
            for key in ["error", "message"] {
                if let Some(text) = object.get(key).and_then(serde_json::Value::as_str)
                    && !text.trim().is_empty()
                {
                    return Some(ToolText::Borrowed {
                        source: ToolTextSource::RawField(key),
                        text,
                    });
                }
            }
        }

        for key in ["stdout", "stderr"] {
            if let Some(text) = object.get(key).and_then(serde_json::Value::as_str)
                && !text.trim().is_empty()
            {
                return Some(ToolText::Borrowed {
                    source: ToolTextSource::RawField(key),
                    text,
                });
            }
        }
        if let Some(first) = object
            .get("items")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.first())
        {
            if let Some(text) = first.get("Text").and_then(serde_json::Value::as_str)
                && (!skip_blank || !text.trim().is_empty())
            {
                return Some(ToolText::Borrowed {
                    source: ToolTextSource::ItemText,
                    text,
                });
            }
            if let Some(value) = first.get("Json") {
                return serialize_tool_text(value);
            }
        }
        for key in ["text", "content", "result", "output"] {
            if let Some(text) = object.get(key).and_then(serde_json::Value::as_str)
                && (!skip_blank || !text.trim().is_empty())
            {
                return Some(ToolText::Borrowed {
                    source: ToolTextSource::RawField(key),
                    text,
                });
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

    /// Extract a failed call's first nonblank error from raw output, then content.
    /// The public consumer view shares the renderer's authoritative decoder.
    pub fn error_message(&self) -> Option<Cow<'_, str>> {
        self.error_text_view().map(ToolText::into_cow)
    }

    pub(crate) fn error_text_view(&self) -> Option<ToolText<'_>> {
        if self.inner.status() != cyril_core::types::ToolCallStatus::Failed {
            return None;
        }
        self.raw_output_text_view(RawOutputPolicy::Error)
            .or_else(|| {
                // v2 validation failures carry their first nonblank error only in ACP content.
                self.inner
                    .content()
                    .iter()
                    .enumerate()
                    .find_map(|(index, content)| match content {
                        cyril_core::types::ToolCallContent::Text(text)
                            if !text.trim().is_empty() =>
                        {
                            Some(ToolText::Borrowed {
                                source: ToolTextSource::Content(index),
                                text,
                            })
                        }
                        _ => None,
                    })
            })
    }

    pub(crate) fn resolve_text_source(&self, source: ToolTextSource) -> Option<&str> {
        match source {
            ToolTextSource::RawOutput => self.inner.raw_output()?.as_str(),
            ToolTextSource::RawField(key) => self.inner.raw_output()?.get(key)?.as_str(),
            ToolTextSource::ItemText => self
                .inner
                .raw_output()?
                .get("items")?
                .as_array()?
                .first()?
                .get("Text")?
                .as_str(),
            ToolTextSource::Content(index) => match self.inner.content().get(index)? {
                cyril_core::types::ToolCallContent::Text(text) => Some(text),
                cyril_core::types::ToolCallContent::Diff { .. } => None,
            },
        }
    }
}

fn serialize_tool_text(value: &serde_json::Value) -> Option<ToolText<'_>> {
    match serde_json::to_string_pretty(value) {
        Ok(text) => Some(ToolText::Derived(text)),
        Err(error) => {
            tracing::warn!(%error, "failed to serialize tool output");
            None
        }
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
    /// Phase 3: Enter optional feedback for the selected KAS reject_once.
    EnterReason {
        chosen_option_id: cyril_core::types::PermissionOptionId,
        editor: RejectionFeedback,
    },
    /// KAS "Always allow" (cyril-auxt): choose the grant's scope and resource,
    /// as Kiro's TUI does. Shown after an AllowAlways pick on a request that
    /// carries KAS consent and no V2 trust tiers. `selected` indexes
    /// [`ApprovalState::scope_choices`]; `owned` records whether the request
    /// belonged to the main session when the page opened.
    SelectScope {
        chosen_option_id: cyril_core::types::PermissionOptionId,
        scope: cyril_core::always_allow::GrantScope,
        owned: bool,
    },
    /// KAS "Always allow" folder grant: type the folder. The editor starts
    /// with the offer's prefill; `attempted` is set once Enter was pressed on
    /// an invalid folder, so the reason renders as an error from then on.
    /// `owned` is carried over from the scope page.
    EnterFolder {
        chosen_option_id: cyril_core::types::PermissionOptionId,
        scope: cyril_core::always_allow::GrantScope,
        owned: bool,
        offer: cyril_core::always_allow::FolderOffer,
        editor: RejectionFeedback,
        attempted: bool,
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
    /// Immutable request capability derived from the configured engine and
    /// feature availability. It is intentionally separate from option data:
    /// option kind and request capability must both gate feedback entry.
    pub can_reject_with_reason: bool,
    pub phase: ApprovalPhase,
    pub responder: tokio::sync::oneshot::Sender<cyril_core::types::PermissionResponse>,
    /// The request's KAS consent (`_meta.kiro`), when the agent sent one.
    /// Its presence routes an AllowAlways pick to the scope page.
    pub consent: Option<cyril_core::review::consent::PermissionConsent>,
    /// The workflow plan a KAS `update_workflow` request asks the user to
    /// approve (cyril-8wh0); the overlay renders its step tree.
    pub workflow_plan: Option<cyril_core::types::WorkflowPlan>,
}

impl ApprovalState {
    /// This request as the "Always allow" queue release sees it (cyril-auxt).
    pub fn queued_view(&self) -> cyril_core::always_allow::QueuedRequest<'_> {
        cyril_core::always_allow::QueuedRequest {
            session_id: &self.session_id,
            tool_call_id: self.tool_call.id(),
            tool_title: self.tool_call.title(),
            consent: self.consent.as_ref(),
            options: &self.options,
        }
    }

    /// The scope page's rows for the current phase; empty outside
    /// [`ApprovalPhase::SelectScope`].
    pub fn scope_choices(&self) -> Vec<cyril_core::always_allow::ScopeChoice> {
        match (&self.phase, &self.consent) {
            (ApprovalPhase::SelectScope { scope, owned, .. }, Some(consent)) => {
                cyril_core::always_allow::scope_choices(consent, *scope, *owned)
            }
            _ => Vec::new(),
        }
    }

    /// Shared eligibility predicate for the hint, key entry, and state
    /// transition. A feedback phase can only be entered for the selected
    /// reject_once option of a capable request.
    pub fn can_reject_with_reason(&self) -> bool {
        self.can_reject_with_reason
            && matches!(self.phase, ApprovalPhase::SelectOption)
            && self.options.get(self.selected).is_some_and(|option| {
                option.kind == cyril_core::types::PermissionOptionKind::RejectOnce
            })
    }
}

/// What confirming a picker does (cyril-qaq0).
///
/// The kinds share the viewport, filter, and scrollbar machinery but have
/// different commit semantics: an agent picker sends the selection back over
/// the wire, Cyril's palette picker applies it locally and never writes
/// anything, and a config picker sets a KAS session config value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerKind {
    /// Options owned by the agent: confirming dispatches a bridge command.
    Agent,
    /// Cyril's palette picker: confirming commits locally, never to the wire.
    Theme,
    /// A KAS session config select (`/model` `/effort` `/mode`, cyril-vh69):
    /// confirming revalidates the value against the session's latest config
    /// snapshot and dispatches `SetConfigOption` for the target — never the
    /// v2 Agent path's `ExecuteCommand`. The target names what is set, and an
    /// effort target the model its levels came from.
    Config(cyril_core::types::ConfigTarget),
}

/// The `/review` consent form: what the review will run and everything it
/// may do, so that Enter is informed consent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewForm {
    /// What the review compares; every choice ends at HEAD.
    pub target: cyril_core::review::target::ReviewTarget,
    /// Branches a base-branch review can compare against.
    pub branches: Vec<String>,
    /// The base branch last picked, kept while another mode is shown.
    pub base_choice: Option<String>,
    /// Which field ←/→ changes.
    pub focus: ReviewField,
    /// The Review scope as checkboxes; the checked paths are the pathspecs.
    pub paths: Vec<cyril_core::review::scope::PathChoice>,
    /// The path Space toggles while the paths field has focus.
    pub path_cursor: usize,
    /// Files the target's diff touches in scope; `None` while counting.
    pub file_count: Option<usize>,
    /// Why the current choice cannot be reviewed; Enter is refused while set.
    pub problem: Option<String>,
    /// Something the operator should know that does not block the review.
    pub note: Option<String>,
    /// What runs before the workflow starts.
    pub check: ReviewCheck,
    /// Enter was pressed and the launch is under way.
    pub busy: bool,
    /// Set for `/review resume`: the form is read-only and shows the stored
    /// run; `target` and the choice fields are not used.
    pub resume: Option<ReviewResumeView>,
}

/// The persisted run a `/review resume` form asks to continue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewResumeView {
    /// The run directory's name.
    pub run: String,
    /// The agent's persisted status: failed or paused.
    pub status: cyril_core::types::WorkflowRunStatus,
    /// The target the run was gathered with, as stored.
    pub target: String,
    /// Run directories passed over because their `run.json` is unreadable.
    pub unreadable: Vec<String>,
}

/// A field of the `/review` form that takes ←/→.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReviewField {
    #[default]
    Target,
    /// Shown, and focusable, only in base-branch mode.
    Base,
    /// The scope checkboxes: ←/→ move, Space toggles.
    Paths,
}

impl ReviewForm {
    /// A form for `target`, still reading settings and counting.
    pub fn opening(target: cyril_core::review::target::ReviewTarget) -> Self {
        Self {
            target,
            branches: Vec::new(),
            base_choice: None,
            focus: ReviewField::Target,
            paths: Vec::new(),
            path_cursor: 0,
            file_count: None,
            problem: None,
            note: None,
            check: ReviewCheck::Reading,
            busy: false,
            resume: None,
        }
    }

    /// A read-only form to continue the stored run `view`.
    pub fn resuming(view: ReviewResumeView, scope: Vec<String>, files: usize) -> Self {
        Self {
            paths: scope
                .into_iter()
                .map(|path| cyril_core::review::scope::PathChoice {
                    path,
                    files: 0,
                    checked: true,
                })
                .collect(),
            file_count: Some(files),
            check: ReviewCheck::NotRerun,
            resume: Some(view),
            ..Self::opening(cyril_core::review::target::ReviewTarget::Auto)
        }
    }

    /// The checked pathspecs.
    pub fn scope(&self) -> Vec<String> {
        cyril_core::review::scope::selected(&self.paths)
    }

    /// The focusable fields, in Tab order.
    pub fn fields(&self) -> Vec<ReviewField> {
        let mut fields = vec![ReviewField::Target];
        if self.shows_base() {
            fields.push(ReviewField::Base);
        }
        if !self.paths.is_empty() {
            fields.push(ReviewField::Paths);
        }
        fields
    }

    /// Whether the base field is on the form.
    pub fn shows_base(&self) -> bool {
        matches!(
            self.target,
            cyril_core::review::target::ReviewTarget::Base(_)
        )
    }

    /// Enter starts the review: the count is in and nothing is wrong.
    pub fn ready(&self) -> bool {
        !self.busy && self.problem.is_none() && self.file_count.is_some()
    }
}

/// The repository check command shown on the consent form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewCheck {
    /// The repository's settings are still being read.
    Reading,
    /// A resumed run keeps the check results it already recorded.
    NotRerun,
    NotConfigured,
    WillRun {
        command: String,
        timeout_secs: u64,
    },
}

/// Selection picker dialog state.
#[derive(Debug)]
pub struct PickerState {
    pub title: String,
    pub options: Vec<CommandOption>,
    pub filter: String,
    pub filtered_indices: Vec<usize>,
    pub selected: usize,
    pub kind: PickerKind,
}

/// What confirming a picker selected (cyril-vh69).
///
/// `title` doubles as the agent command name for `PickerKind::Agent` — the
/// v2 contract, where the picker opens on `CommandOptionsReceived.command`
/// and confirm hands it back as the command to execute. A
/// `PickerKind::Config` carries its own dispatch key (the target), so the
/// App never overloads the display title with routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerConfirmation {
    pub kind: PickerKind,
    pub title: String,
    pub value: String,
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

/// Powers the `/powers` panel shows at once: five powers are 15 content rows
/// plus the two borders = 17, which still fits the 24-row terminal the panel's
/// geometry is checked against.
///
/// This is the panel's DESIRED window, not the one it always gets:
/// `modal::place` squeezes the popup into the rows above the input, so a short
/// terminal or a tall input shows fewer powers. The window that actually
/// decides layout is `widgets::powers_panel::placement`'s, and the keyboard's
/// scroll bound is measured from that same number (`render::powers_window`) —
/// bounding it here would strand the tail of a long catalog in exactly those
/// frames.
pub const MAX_VISIBLE_POWERS: usize = 5;

/// Powers panel overlay state (read-only display for `/powers`).
///
/// Populated from a `PowersChanged` push — the agent announces its installed
/// powers unprompted, and cyril never asks for them. Powers execute inside the
/// agent, so the panel carries no interactive state beyond scroll position.
#[derive(Debug, Clone)]
pub struct PowersPanelState {
    /// Powers in display order: title (ASCII-lowercase), tie-broken by
    /// identifier. Pre-sorted by [`crate::state::UiState::show_powers_panel`];
    /// the renderer iterates this directly. A caller constructing
    /// `PowersPanelState` by hand owns that ordering.
    pub powers: Vec<PowerInfo>,
    /// Index of the first power in the viewport, not a line offset: the widget
    /// always renders whole powers, so a scroll position that is not a power
    /// boundary cannot exist.
    ///
    /// [`crate::state::UiState`] keeps it inside the last full window of the
    /// popup's ACTUAL size, so this is always a position the widget renders as
    /// given; the widget still clamps a hand-built state into range.
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

/// Whether the panel's usage snapshot is current, being recomputed, or failed.
///
/// Snapshots are computed off the App event loop (cyril-nanu), so the values on
/// screen can be a turn behind the log. That is acceptable only because it is
/// stated: this status is what makes the lag visible rather than silent.
/// Mirrors `UsageAccountStatus`, which cyril-kryv established for the same
/// shape on the account query.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UsageRefreshStatus {
    /// No snapshot has completed yet in this process, so there is nothing to
    /// show. Distinct from a completed snapshot over an empty log, which
    /// renders the "no usage recorded" placeholder instead.
    Computing,
    /// A snapshot is held and a newer one is in flight.
    Refreshing,
    /// A snapshot is held and nothing is in flight.
    #[default]
    Idle,
    /// The last attempt failed. The held values stay on screen beside the
    /// reason, rather than the panel silently showing stale numbers.
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct UsagePanelState {
    pub snapshot: cyril_core::types::UsageSnapshot,
    /// See [`UsageRefreshStatus`]. Rendered in the panel title.
    pub refresh: UsageRefreshStatus,
    /// Whether `snapshot` came from a completed read rather than being the
    /// placeholder a not-yet-computed panel is opened with.
    ///
    /// The status alone cannot answer this: a first snapshot that FAILS leaves
    /// a `Failed` status over a default snapshot, and rendering that as the
    /// empty-log placeholder would report "no usage recorded" for a log nobody
    /// has read yet — the exact conflation this change exists to avoid.
    pub has_snapshot: bool,
    pub page: UsagePage,
    pub scroll_offset: usize,
    pub account: Option<cyril_core::types::UsageAccount>,
    pub account_fetched_at_ms: Option<u64>,
    pub account_status: UsageAccountStatus,
}

impl UsagePanelState {
    pub fn row_count(&self) -> usize {
        // Nothing has been read yet, so no page has rows to count.
        if !self.has_snapshot {
            return 1;
        }
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
        pub messages: MessageHistory,
        pub live_revision: LiveRevision,
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
        pub thinking_enabled: Option<bool>,
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
        pub powers_panel: Option<PowersPanelState>,
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
                messages: MessageHistory::new(),
                live_revision: LiveRevision::default(),
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
                thinking_enabled: None,
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
                powers_panel: None,
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

    impl MockTuiState {
        pub fn bump_live_text(&mut self) {
            self.live_revision.bump_text();
        }

        pub fn bump_live_thought(&mut self) {
            self.live_revision.bump_thought();
        }

        pub fn set_live_text(&mut self, text: String) {
            self.streaming_text = text;
            self.bump_live_text();
        }

        pub fn set_live_thought(&mut self, thought: Option<String>) {
            self.streaming_thought = thought;
            self.bump_live_thought();
        }
    }

    impl TuiState for MockTuiState {
        fn theme(&self) -> Theme {
            self.theme
        }
        fn history(&self) -> &MessageHistory {
            &self.messages
        }
        fn streaming_text(&self) -> &str {
            &self.streaming_text
        }
        fn streaming_thought(&self) -> Option<&str> {
            self.streaming_thought.as_deref()
        }
        fn live_revision(&self) -> LiveRevision {
            self.live_revision
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
        fn thinking_enabled(&self) -> Option<bool> {
            self.thinking_enabled
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
        fn review_form(&self) -> Option<&ReviewForm> {
            None
        }
        fn hooks_panel(&self) -> Option<&HooksPanelState> {
            self.hooks_panel.as_ref()
        }
        fn powers_panel(&self) -> Option<&PowersPanelState> {
            self.powers_panel.as_ref()
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
    fn mock_history_ownership_and_live_revision_helpers_are_real() {
        let mut first = test_support::MockTuiState::default();
        let second = test_support::MockTuiState::default();
        assert!(
            second
                .history()
                .changed_since(first.history().revision())
                .reset
        );
        let initial = first.live_revision();
        first.set_live_text("text".to_owned());
        assert_eq!(first.streaming_text(), "text");
        assert_ne!(first.live_revision().text, initial.text);
        assert_eq!(first.live_revision().thought, initial.thought);
        let after_text = first.live_revision();
        first.streaming_text.push_str(" delta");
        first.bump_live_text();
        assert_ne!(first.live_revision().text, after_text.text);
        first.set_live_thought(Some("thought".to_owned()));
        assert_eq!(first.streaming_thought(), Some("thought"));
        assert_ne!(first.live_revision().thought, initial.thought);
        let before = first.live_revision();
        first.streaming_thought = None;
        first.bump_live_thought();
        assert_eq!(first.live_revision().text, before.text);
        assert_ne!(first.live_revision().thought, before.thought);
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

    fn diff_cache_call(old: Option<&str>, new: &str) -> cyril_core::types::ToolCall {
        use cyril_core::types::*;
        ToolCall::new(
            ToolCallId::new("diff-cache"),
            "Edit".into(),
            ToolKind::Write,
            ToolCallStatus::Completed,
            None,
        )
        .with_content(vec![ToolCallContent::Diff {
            path: "first.rs".into(),
            old_text: old.map(str::to_owned),
            new_text: new.into(),
        }])
    }

    #[test]
    fn tracked_diff_view_selects_once_and_initializes_absence() {
        use cyril_core::types::ToolCallContent;

        let tracked = TrackedToolCall::new(diff_cache_call(None, "").with_content(vec![
            ToolCallContent::Text("first".into()),
            ToolCallContent::Text("second".into()),
        ]));
        assert!(tracked.diff_preview.get().is_none());
        let before = diff_selection_steps();
        assert!(tracked.diff_view().is_none());
        assert_eq!(diff_selection_steps() - before, 2);
        assert!(tracked.diff_preview.get().is_some());
        assert_eq!(tracked.diff_preview().summary(), None);
        assert_eq!(diff_selection_steps() - before, 2);
    }

    #[test]
    fn tracked_diff_view_retains_preview_when_selected_ordinal_moves() {
        use crate::tool_diff::text_diff_constructions;
        use cyril_core::types::ToolCallContent;

        let first = ToolCallContent::Diff {
            path: "first.rs".into(),
            old_text: None,
            new_text: "selected\n".into(),
        };
        let later = ToolCallContent::Diff {
            path: "later.rs".into(),
            old_text: None,
            new_text: "not selected\n".into(),
        };
        let mut tracked = TrackedToolCall::new(diff_cache_call(None, "").with_content(vec![
            ToolCallContent::Text("preceding".into()),
            first.clone(),
            later.clone(),
        ]));
        let constructions = text_diff_constructions();
        let before = diff_selection_steps();
        let (ordinal, source, preview) = tracked.diff_view().unwrap();
        assert_eq!(diff_selection_steps() - before, 2);
        assert_eq!(ordinal, 1);
        assert_eq!(source.new, "selected\n");
        assert_eq!(preview.summary(), Some((1, 0)));
        assert_eq!(preview.rows()[0].text(source).as_ptr(), source.new.as_ptr());
        let retained = tracked.diff_preview.get().unwrap().clone();

        tracked.update(&diff_cache_call(None, "").with_content(vec![
            ToolCallContent::Text("another preceding".into()),
            ToolCallContent::Text("preceding".into()),
            first,
            later,
        ]));
        assert!(Arc::ptr_eq(&retained, tracked.diff_preview.get().unwrap()));
        let before = diff_selection_steps();
        let (ordinal, source, preview) = tracked.diff_view().unwrap();
        assert_eq!(diff_selection_steps() - before, 3);
        assert_eq!(ordinal, 2);
        assert_eq!(source.new, "selected\n");
        assert!(std::ptr::eq(preview, retained.as_ref()));
        assert_eq!(preview.rows()[0].text(source).as_ptr(), source.new.as_ptr());
        assert_eq!(text_diff_constructions() - constructions, 1);
    }

    #[test]
    fn tracked_diff_cache_is_lazy_and_caches_absence_and_no_change() {
        use crate::tool_diff::text_diff_constructions;
        use cyril_core::types::*;

        let before = text_diff_constructions();
        let mut no_diff = TrackedToolCall::new(
            diff_cache_call(None, "").with_content(vec![ToolCallContent::Text("output".into())]),
        );
        assert!(no_diff.diff_preview.get().is_none());
        assert!(no_diff.clone().diff_preview.get().is_none());
        assert_eq!(text_diff_constructions(), before);
        assert_eq!(no_diff.diff_preview().summary(), None);
        let absent = no_diff.diff_preview.get().unwrap().clone();
        no_diff.update(
            &diff_cache_call(None, "")
                .with_content(vec![ToolCallContent::Text("different output".into())]),
        );
        assert!(Arc::ptr_eq(&absent, no_diff.diff_preview.get().unwrap()));
        assert_eq!(text_diff_constructions(), before);

        let unchanged = TrackedToolCall::new(diff_cache_call(Some("same\n"), "same\n"));
        let empty_clone = unchanged.clone();
        assert!(empty_clone.diff_preview.get().is_none());
        assert_eq!(unchanged.diff_preview().summary(), None);
        assert!(unchanged.diff_preview().rows().is_empty());
        assert_eq!(text_diff_constructions() - before, 1);
        assert!(empty_clone.diff_preview.get().is_none());
        let initialized_clone = unchanged.clone();
        assert!(Arc::ptr_eq(
            unchanged.diff_preview.get().unwrap(),
            initialized_clone.diff_preview.get().unwrap()
        ));
        assert_eq!(initialized_clone.diff_preview().summary(), None);
        assert_eq!(text_diff_constructions() - before, 1);
    }

    #[test]
    fn tracked_diff_cache_retains_effective_input_across_metadata_and_equivalent_updates() {
        use crate::tool_diff::text_diff_constructions;
        use cyril_core::types::*;

        let mut tracked = TrackedToolCall::new(diff_cache_call(None, "new\n"));
        let before = text_diff_constructions();
        assert_eq!(tracked.diff_preview().summary(), Some((1, 0)));
        let retained = tracked.diff_preview.get().unwrap().clone();
        for status in [
            ToolCallStatus::Pending,
            ToolCallStatus::InProgress,
            ToolCallStatus::Failed,
        ] {
            let update = ToolCall::new(
                tracked.id().clone(),
                "metadata changed".into(),
                ToolKind::Other,
                status,
                Some(serde_json::json!({"path": "elsewhere.rs"})),
            )
            .with_locations(vec![ToolCallLocation {
                path: "elsewhere.rs".into(),
                line: Some(99),
            }])
            .with_raw_output(Some(serde_json::json!({"stdout": "metadata"})));
            tracked.update(&update);
            assert_eq!(tracked.status(), status);
            assert_eq!(tracked.diff_preview().summary(), Some((1, 0)));
            assert!(Arc::ptr_eq(&retained, tracked.diff_preview.get().unwrap()));
            assert_eq!(tracked.diff_source().unwrap().new, "new\n");
        }
        // Replacing owned source strings is safe: descriptors contain only
        // ranges, and None/empty old input are the same effective diff.
        tracked.update(&diff_cache_call(Some(""), "new\n").with_content(vec![
            ToolCallContent::Text("unrelated".into()),
            ToolCallContent::Diff {
                path: "renamed.rs".into(),
                old_text: Some(String::new()),
                new_text: "new\n".into(),
            },
            ToolCallContent::Diff {
                path: "later.rs".into(),
                old_text: None,
                new_text: "not selected\n".into(),
            },
        ]));
        assert_eq!(tracked.primary_path(), Some("elsewhere.rs"));
        assert_eq!(tracked.diff_preview().summary(), Some((1, 0)));
        assert!(Arc::ptr_eq(&retained, tracked.diff_preview.get().unwrap()));
        let source = tracked.diff_source().unwrap();
        assert_eq!(tracked.diff_preview().rows()[0].text(source), "new");
        assert_eq!(
            tracked.diff_preview().rows()[0].text(source).as_ptr(),
            source.new.as_ptr()
        );
        assert_eq!(text_diff_constructions() - before, 1);
    }

    #[test]
    fn tracked_diff_cache_source_changes_detach_initialized_clones() {
        use crate::tool_diff::text_diff_constructions;
        use cyril_core::types::ToolCallContent;

        let mut tracked = TrackedToolCall::new(diff_cache_call(Some("old\n"), "new\n"));
        let before = text_diff_constructions();
        assert_eq!(tracked.diff_preview().summary(), Some((1, 1)));
        let snapshot = tracked.clone();
        assert!(Arc::ptr_eq(
            tracked.diff_preview.get().unwrap(),
            snapshot.diff_preview.get().unwrap()
        ));
        tracked.update(&diff_cache_call(Some("old\n"), "new\nextra\n"));
        assert!(tracked.diff_preview.get().is_none());
        assert_eq!(text_diff_constructions() - before, 1);
        assert_eq!(tracked.diff_preview().summary(), Some((2, 1)));
        assert_eq!(snapshot.diff_preview().summary(), Some((1, 1)));
        assert!(!Arc::ptr_eq(
            tracked.diff_preview.get().unwrap(),
            snapshot.diff_preview.get().unwrap()
        ));
        assert_eq!(snapshot.diff_source().unwrap().new, "new\n");
        assert_eq!(
            tracked.diff_preview().rows()[2].text(tracked.diff_source().unwrap()),
            "extra"
        );
        assert_eq!(text_diff_constructions() - before, 2);

        tracked.update(&diff_cache_call(Some("older\nremoved\n"), "new\nextra\n"));
        assert!(tracked.diff_preview.get().is_none());
        assert_eq!(tracked.diff_preview().summary(), Some((2, 2)));
        assert_eq!(text_diff_constructions() - before, 3);

        tracked.update(
            &diff_cache_call(None, "")
                .with_content(vec![ToolCallContent::Text("diff removed".into())]),
        );
        assert!(tracked.diff_preview.get().is_none());
        assert_eq!(tracked.diff_preview().summary(), None);
        assert!(tracked.diff_preview().rows().is_empty());
        assert_eq!(text_diff_constructions() - before, 3);
        tracked.update(&diff_cache_call(None, "added\n"));
        assert!(tracked.diff_preview.get().is_none());
        assert_eq!(tracked.diff_preview().summary(), Some((1, 0)));
        assert_eq!(text_diff_constructions() - before, 4);
        assert_eq!(snapshot.diff_preview().summary(), Some((1, 1)));
    }

    #[test]
    fn tracked_diff_cache_tracks_first_diff_selection_not_later_blocks() {
        use crate::tool_diff::text_diff_constructions;
        use cyril_core::types::ToolCallContent;

        let first = ToolCallContent::Diff {
            path: "first.rs".into(),
            old_text: Some("same\n".into()),
            new_text: "same\n".into(),
        };
        let later = ToolCallContent::Diff {
            path: "later.rs".into(),
            old_text: Some("old\n".into()),
            new_text: "new\nextra\n".into(),
        };
        let mut tracked = TrackedToolCall::new(
            diff_cache_call(None, "").with_content(vec![first.clone(), later.clone()]),
        );
        let before = text_diff_constructions();
        assert_eq!(tracked.diff_preview().summary(), None);
        let retained = tracked.diff_preview.get().unwrap().clone();
        tracked.update(&diff_cache_call(None, "").with_content(vec![
            first,
            ToolCallContent::Text("later diff removed".into()),
        ]));
        assert!(Arc::ptr_eq(&retained, tracked.diff_preview.get().unwrap()));
        tracked.update(&diff_cache_call(None, "").with_content(vec![later]));
        assert!(tracked.diff_preview.get().is_none());
        assert_eq!(tracked.diff_preview().summary(), Some((2, 1)));
        assert_eq!(text_diff_constructions() - before, 2);
    }

    /// A `read` tool call carrying only `rawInput` — no `locations`, no content.
    fn read_call_with_raw_input(raw_input: serde_json::Value) -> TrackedToolCall {
        use cyril_core::types::*;
        TrackedToolCall::new(ToolCall::new(
            ToolCallId::new("tc_read"),
            "Reading PROBE.txt:1".into(),
            ToolKind::Read,
            ToolCallStatus::InProgress,
            Some(raw_input),
        ))
    }

    /// cyril-q0kg: the preview classification the renderer and the
    /// install-time warning share. `text` wins over `content`; a non-string
    /// `text` does not fall through to `content` (the cyril-j1b3 lookup).
    #[rstest::rstest]
    #[case::text(serde_json::json!({"text": "t", "content": "c"}), PreviewBody::Text("t"))]
    #[case::content(serde_json::json!({"content": "c"}), PreviewBody::Text("c"))]
    #[case::non_string_text(
        serde_json::json!({"text": 42, "content": "c"}),
        PreviewBody::Malformed { raw_input_shape: "object" }
    )]
    #[case::command_only(
        serde_json::json!({"command": "ls"}),
        PreviewBody::Malformed { raw_input_shape: "object" }
    )]
    #[case::array(serde_json::json!(["x"]), PreviewBody::Malformed { raw_input_shape: "array" })]
    #[case::string(serde_json::json!("x"), PreviewBody::Malformed { raw_input_shape: "string" })]
    #[case::null(serde_json::Value::Null, PreviewBody::Malformed { raw_input_shape: "null" })]
    fn preview_body_classifies_raw_input(
        #[case] raw_input: serde_json::Value,
        #[case] expected: PreviewBody<'static>,
    ) {
        assert_eq!(read_call_with_raw_input(raw_input).preview_body(), expected);
    }

    /// cyril-8wh0: a Text content block is the request's own explanation
    /// (KAS's workflow plan approval carries the plan there), so it wins over
    /// the raw-input fallback — including a raw input with no text field,
    /// which would otherwise classify as malformed. A diff still wins over it.
    #[rstest::rstest]
    #[case::over_malformed_raw_input(
        Some(serde_json::json!({"action": "replace_remaining", "workflowRef": "generated://g"}))
    )]
    #[case::over_raw_input_text(Some(serde_json::json!({"text": "raw"})))]
    #[case::without_raw_input(None)]
    fn preview_body_prefers_text_content(#[case] raw_input: Option<serde_json::Value>) {
        preview_body_prefers_text_content_case(raw_input);
    }

    /// cyril-8wh0 review: blank Text content explains nothing, so it falls
    /// through to the raw-input text, or to absent — never to an empty
    /// preview.
    #[rstest::rstest]
    #[case::raw_text(Some(serde_json::json!({"text": "raw"})), PreviewBody::Text("raw"))]
    #[case::no_raw_input(None, PreviewBody::Absent)]
    fn blank_text_content_falls_through(
        #[case] raw_input: Option<serde_json::Value>,
        #[case] expected: PreviewBody<'static>,
    ) {
        use cyril_core::types::*;
        let call = ToolCall::new(
            ToolCallId::new("tc_blank"),
            "Write File".into(),
            ToolKind::Write,
            ToolCallStatus::Pending,
            raw_input,
        )
        .with_content(vec![
            ToolCallContent::Text(String::new()),
            ToolCallContent::Text(" \n\t".into()),
        ]);
        assert_eq!(TrackedToolCall::new(call).preview_body(), expected);
    }

    fn preview_body_prefers_text_content_case(raw_input: Option<serde_json::Value>) {
        use cyril_core::types::*;
        let content = vec![
            ToolCallContent::Text("Steps to add:".into()),
            ToolCallContent::Text("write-hello: step".into()),
        ];
        let call = ToolCall::new(
            ToolCallId::new("tc_plan"),
            "Add 1 workflow step to wf_1".into(),
            ToolKind::Other,
            ToolCallStatus::Pending,
            raw_input,
        )
        .with_content(content.clone());
        assert_eq!(
            TrackedToolCall::new(call.clone()).preview_body(),
            PreviewBody::Content(&content)
        );

        let mut with_diff = content.clone();
        with_diff.push(ToolCallContent::Diff {
            path: "/tmp/a.md".into(),
            old_text: None,
            new_text: "x".into(),
        });
        assert_eq!(
            TrackedToolCall::new(call.with_content(with_diff)).preview_body(),
            PreviewBody::Diff
        );
    }

    #[test]
    fn preview_body_diff_wins_and_missing_raw_input_is_absent() {
        use cyril_core::types::*;
        let bare = ToolCall::new(
            ToolCallId::new("tc_w"),
            "Write File".into(),
            ToolKind::Write,
            ToolCallStatus::Pending,
            None,
        );
        assert_eq!(
            TrackedToolCall::new(bare.clone()).preview_body(),
            PreviewBody::Absent
        );
        let with_diff = bare.with_content(vec![ToolCallContent::Diff {
            path: "/tmp/a.md".into(),
            old_text: None,
            new_text: "x".into(),
        }]);
        assert_eq!(
            TrackedToolCall::new(with_diff).preview_body(),
            PreviewBody::Diff
        );
    }

    /// The v2 `fs_read` shape nests the path under `operations[]` and has no
    /// flat `path`/`file_path` key (docs/kiro-2.21.2-wire-audit.md § 6b,
    /// cyril-gl0m). With `locations` absent the rawInput fallback must read it.
    #[test]
    fn primary_path_resolves_fs_read_operations_path_without_locations() {
        let tracked = read_call_with_raw_input(serde_json::json!({
            "__tool_use_purpose": "Read PROBE.txt to get the single word.",
            "operations": [{"mode": "Line", "path": "/abs/path/PROBE.txt"}]
        }));
        assert!(tracked.locations().is_empty());
        assert_eq!(tracked.primary_path(), Some("/abs/path/PROBE.txt"));
    }

    #[test]
    fn primary_path_still_resolves_flat_raw_input_keys() {
        let by_path = read_call_with_raw_input(serde_json::json!({"path": "/flat/by-path.rs"}));
        assert_eq!(by_path.primary_path(), Some("/flat/by-path.rs"));

        let by_file_path =
            read_call_with_raw_input(serde_json::json!({"file_path": "/flat/by-file-path.rs"}));
        assert_eq!(by_file_path.primary_path(), Some("/flat/by-file-path.rs"));
    }

    #[test]
    fn primary_path_is_none_when_operations_entry_lacks_path() {
        let no_path = read_call_with_raw_input(serde_json::json!({
            "operations": [{"mode": "Directory"}]
        }));
        assert_eq!(no_path.primary_path(), None);

        let empty = read_call_with_raw_input(serde_json::json!({"operations": []}));
        assert_eq!(empty.primary_path(), None);
    }

    /// The live 2.21.2 capture carries both `locations` and `operations`;
    /// branch 1 must keep winning so the fallback stays exactly that.
    #[test]
    fn primary_path_prefers_locations_over_operations() {
        use cyril_core::types::*;
        let tc = ToolCall::new(
            ToolCallId::new("tc_read"),
            "Reading PROBE.txt:1".into(),
            ToolKind::Read,
            ToolCallStatus::InProgress,
            Some(serde_json::json!({
                "operations": [{"mode": "Line", "path": "/from/raw-input.txt"}]
            })),
        )
        .with_locations(vec![ToolCallLocation {
            path: "/from/locations.txt".into(),
            line: Some(1),
        }]);
        assert_eq!(
            TrackedToolCall::new(tc).primary_path(),
            Some("/from/locations.txt")
        );
    }

    /// PR #133 review F1: with both a flat key and `operations[]` present the
    /// flat key wins, so the probe-order-swap mutant is detected.
    #[test]
    fn primary_path_prefers_flat_key_over_operations() {
        let tracked = read_call_with_raw_input(serde_json::json!({
            "file_path": "/flat/wins.rs",
            "operations": [{"mode": "Line", "path": "/ops/loses.rs"}]
        }));
        assert_eq!(tracked.primary_path(), Some("/flat/wins.rs"));
    }

    /// PR #133 review F2: a flat key that exists but is not a string falls
    /// through to the next probe instead of ending the whole lookup.
    #[test]
    fn primary_path_falls_through_non_string_flat_key() {
        let to_operations = read_call_with_raw_input(serde_json::json!({
            "path": null,
            "operations": [{"mode": "Line", "path": "/ops/after-null.rs"}]
        }));
        assert_eq!(to_operations.primary_path(), Some("/ops/after-null.rs"));

        let to_next_flat_key = read_call_with_raw_input(serde_json::json!({
            "file_path": 42,
            "path": "/flat/after-number.rs"
        }));
        assert_eq!(
            to_next_flat_key.primary_path(),
            Some("/flat/after-number.rs")
        );
    }

    /// PR #133 review F3: an empty path is absent, not a sentinel (CLAUDE.md
    /// "no sentinel values"); both the nested and the flat branch yield `None`.
    #[test]
    fn primary_path_is_none_for_empty_path_strings() {
        let nested = read_call_with_raw_input(serde_json::json!({
            "operations": [{"mode": "Line", "path": ""}]
        }));
        assert_eq!(nested.primary_path(), None);

        let flat = read_call_with_raw_input(serde_json::json!({"path": ""}));
        assert_eq!(flat.primary_path(), None);
    }

    /// Re-review N2: with several operations the first entry is the primary
    /// path — the requester scoped the probe to `operations[0]`.
    #[test]
    fn primary_path_uses_first_of_several_operations() {
        let tracked = read_call_with_raw_input(serde_json::json!({
            "operations": [
                {"mode": "Line", "path": "/ops/first.rs"},
                {"mode": "Line", "path": "/ops/second.rs"}
            ]
        }));
        assert_eq!(tracked.primary_path(), Some("/ops/first.rs"));
    }

    /// Re-review N3: the empty-path filter is terminal (review F3 as
    /// approved), so an empty flat key is not skipped in favour of a nested
    /// path. No known tool sends this shape; revisit here if one appears.
    #[test]
    fn primary_path_empty_flat_key_does_not_fall_through_to_operations() {
        let tracked = read_call_with_raw_input(serde_json::json!({
            "file_path": "",
            "operations": [{"mode": "Line", "path": "/ops/masked.rs"}]
        }));
        assert_eq!(tracked.primary_path(), None);
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
        assert_eq!(tracked.output_text().as_deref(), Some("hello world"));
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
        assert_eq!(tracked.output_text().as_deref(), Some("error output"));
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
        assert_eq!(tracked.output_text().as_deref(), Some("file contents here"));
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
        assert_eq!(tracked.output_text().as_deref(), Some("plain text output"));
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
            tracked.error_message().as_deref(),
            Some("Command timed out")
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
            tracked.error_message().as_deref(),
            Some("permission denied")
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
        assert_eq!(tracked.output_text().as_deref(), Some("plain output"));
    }

    #[test]
    fn tracked_tool_call_output_field_preserves_existing_precedence() {
        use cyril_core::types::*;
        let cases = [
            (serde_json::json!({"stdout": "stdout wins"}), "stdout wins"),
            (
                serde_json::json!({"stdout": " \n", "stderr": "stderr wins"}),
                "stderr wins",
            ),
            (
                serde_json::json!({"items": [{"Text": "item text wins"}]}),
                "item text wins",
            ),
            (
                serde_json::json!({"items": [{"Json": {"selected": true}}]}),
                "{\n  \"selected\": true\n}",
            ),
            (serde_json::json!({"text": "text wins"}), "text wins"),
            (
                serde_json::json!({"content": "content wins"}),
                "content wins",
            ),
            (serde_json::json!({"result": "result wins"}), "result wins"),
        ];
        for (mut output, expected) in cases {
            output["output"] = serde_json::json!("lower-priority captured output");
            let tracked = TrackedToolCall::new(
                ToolCall::new(
                    ToolCallId::new("output-precedence"),
                    "tool".into(),
                    ToolKind::Other,
                    ToolCallStatus::Completed,
                    None,
                )
                .with_raw_output(Some(output.clone())),
            );
            assert_eq!(
                tracked.output_text().as_deref(),
                Some(expected),
                "established extraction must beat output: {output}",
            );
        }
    }

    #[test]
    fn tracked_tool_call_output_field_accepts_only_strings_without_message_fallback() {
        use cyril_core::types::*;
        for value in [
            serde_json::Value::Null,
            serde_json::json!(false),
            serde_json::json!(42),
            serde_json::json!(["nested output"]),
            serde_json::json!({"text": "nested output"}),
        ] {
            let output = serde_json::json!({
                "output": value,
                "message": "formatted message must not become stdout",
            });
            let tracked = TrackedToolCall::new(
                ToolCall::new(
                    ToolCallId::new("output-non-string"),
                    "tool".into(),
                    ToolKind::Other,
                    ToolCallStatus::Completed,
                    None,
                )
                .with_raw_output(Some(output.clone())),
            );
            assert_eq!(
                tracked.output_text(),
                None,
                "do not coerce output or fall back to message: {output}",
            );
        }
        for (output, expected) in [
            (serde_json::json!({"output": ""}), Some("")),
            (serde_json::json!({"message": "formatted message"}), None),
        ] {
            let tracked = TrackedToolCall::new(
                ToolCall::new(
                    ToolCallId::new("output-string-boundary"),
                    "tool".into(),
                    ToolKind::Other,
                    ToolCallStatus::Completed,
                    None,
                )
                .with_raw_output(Some(output)),
            );
            assert_eq!(tracked.output_text().as_deref(), expected);
        }
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
            tracked.error_message().as_deref(),
            Some("permission denied")
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
        assert_eq!(tracked.error_message().as_deref(), Some("not found"));
    }
}
