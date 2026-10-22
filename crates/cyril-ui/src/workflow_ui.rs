//! Per-workflow-step message streams (cyril-jxfu).
//!
//! KAS workflow steps are peer sessions, deliberately outside the subagent
//! domain: no `subagent/list_update` ever names them, so they must not ride
//! `SubagentUiState` or the crew panel. This store receives every frame the
//! App routes as `NotificationRoute::Workflow`, and adopts optimistic
//! subagent streams when a late claim re-parents them. Nothing renders these
//! streams yet — that is cyril-zd8u.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use cyril_core::types::{Notification, SessionId};

use crate::subagent_ui::SubagentStream;
use crate::traits::Activity;

/// Owns per-step-session streams for workflow-owned sessions.
pub struct WorkflowUiState {
    streams: HashMap<SessionId, SubagentStream>,
}

impl WorkflowUiState {
    pub fn new() -> Self {
        Self {
            streams: HashMap::new(),
        }
    }

    /// Route a notification to the workflow stream identified by
    /// `session_id`. Creates the stream on first contact, mirroring the
    /// optimistic subagent path.
    pub fn apply_notification(
        &mut self,
        session_id: &SessionId,
        notification: &Notification,
    ) -> bool {
        let stream = self
            .streams
            .entry(session_id.clone())
            .or_insert_with(SubagentStream::new);
        stream.apply_notification(notification)
    }

    /// Append a system line to `session_id`'s stream, creating it on first
    /// contact like [`Self::apply_notification`].
    pub fn add_system_message(&mut self, session_id: &SessionId, text: String) {
        self.streams
            .entry(session_id.clone())
            .or_insert_with(SubagentStream::new)
            .push_system(text);
    }

    /// Adopt a re-parented optimistic stream, history intact.
    ///
    /// The vacant-key case is the mainline (the sweep runs before any frame
    /// routes here post-claim). An occupied key cannot occur on the
    /// single-threaded App today, but silently dropping either side's history
    /// would be a wrong-output failure, so the earlier (adopted) messages are
    /// spliced in front — chronological order — with a warning.
    pub fn adopt(&mut self, session_id: SessionId, stream: SubagentStream) {
        match self.streams.entry(session_id) {
            Entry::Vacant(slot) => {
                slot.insert(stream);
            }
            Entry::Occupied(mut slot) => {
                tracing::warn!(
                    session_id = slot.key().as_str(),
                    "adopting into an occupied workflow stream; \
                     splicing earlier history in front"
                );
                slot.get_mut().absorb_earlier(stream);
            }
        }
    }

    /// Read-only access to every workflow stream.
    pub fn streams(&self) -> &HashMap<SessionId, SubagentStream> {
        &self.streams
    }

    /// True if any workflow stream is actively streaming or running tools —
    /// feeds the adaptive frame rate alongside the subagent and voice checks.
    pub fn any_active(&self) -> bool {
        self.streams.values().any(|stream| {
            matches!(
                stream.activity(),
                Activity::Streaming | Activity::ToolRunning
            )
        })
    }
}

impl Default for WorkflowUiState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cyril_core::types::message::AgentMessage;
    use cyril_core::types::{ToolCall, ToolCallId, ToolCallStatus, ToolKind};

    fn agent_msg(text: &str, is_streaming: bool) -> Notification {
        Notification::AgentMessage(AgentMessage {
            text: text.into(),
            is_streaming,
        })
    }

    fn tool_started(id: &str, title: &str) -> Notification {
        Notification::ToolCallStarted(ToolCall::new(
            ToolCallId::new(id),
            title.to_owned(),
            ToolKind::Execute,
            ToolCallStatus::InProgress,
            None,
        ))
    }

    fn tool_completed(id: &str) -> Notification {
        Notification::ToolCallUpdated(ToolCall::new(
            ToolCallId::new(id),
            String::new(),
            ToolKind::Execute,
            ToolCallStatus::Completed,
            None,
        ))
    }

    #[test]
    fn first_contact_creates_the_stream() {
        let mut state = WorkflowUiState::new();
        let sid = SessionId::new("step-1");
        state.apply_notification(&sid, &agent_msg("hello", false));
        assert_eq!(state.streams()[&sid].history().messages().len(), 1);
    }

    // C5 substrate: adoption preserves order AND the tool-call index — a
    // ToolCallUpdated arriving after the move must still merge in place.
    #[test]
    fn adopt_preserves_history_and_tool_index() {
        let mut donor = crate::subagent_ui::SubagentUiState::new();
        let sid = SessionId::new("step-1");
        donor.apply_notification(&sid, &agent_msg("before tool", false));
        donor.apply_notification(&sid, &tool_started("tc-1", "run probe"));
        let Some(stream) = donor.remove_stream(&sid) else {
            panic!("donor stream must exist");
        };
        let before_move = stream.history().revision();

        let mut state = WorkflowUiState::new();
        state.adopt(sid.clone(), stream);
        assert_eq!(state.streams()[&sid].history().revision(), before_move);
        assert!(
            state.apply_notification(&sid, &tool_completed("tc-1")),
            "a post-adoption tool update must merge into the moved stream"
        );
        assert!(
            !state.streams()[&sid]
                .history()
                .changed_since(before_move)
                .reset
        );
        assert_eq!(
            state.streams()[&sid]
                .history()
                .changed_since(before_move)
                .changed
                .count(),
            1
        );
        let messages = state.streams()[&sid].history().messages();
        assert_eq!(messages.len(), 2, "adopted history must survive the move");
    }

    // Occupied-key adopt: earlier history splices IN FRONT, later stays, and
    // tool updates on BOTH sides still merge afterwards.
    #[test]
    fn occupied_adopt_splices_earlier_history_in_front() {
        let sid = SessionId::new("step-1");
        let mut donor = crate::subagent_ui::SubagentUiState::new();
        donor.apply_notification(&sid, &agent_msg("earlier-1", false));
        donor.apply_notification(&sid, &tool_started("tc-early", "early tool"));
        let Some(earlier) = donor.remove_stream(&sid) else {
            panic!("donor stream must exist");
        };

        let mut state = WorkflowUiState::new();
        state.apply_notification(&sid, &tool_started("tc-late", "late tool"));
        let before_adoption = state.streams()[&sid].history().revision();
        let donor_revision = earlier.history().revision();
        state.adopt(sid.clone(), earlier);
        assert!(
            state.streams()[&sid]
                .history()
                .changed_since(before_adoption)
                .reset
        );
        assert!(
            state.streams()[&sid]
                .history()
                .changed_since(donor_revision)
                .reset
        );

        let messages = state.streams()[&sid].history().messages();
        assert_eq!(messages.len(), 3, "no side of the merge may drop history");
        assert!(
            state.apply_notification(&sid, &tool_completed("tc-early")),
            "earlier side's tool update must still merge after the splice"
        );
        assert!(
            state.apply_notification(&sid, &tool_completed("tc-late")),
            "later side's tool update must still merge after the splice"
        );
    }

    #[test]
    fn occupied_adoption_rebuilds_tool_positions_with_later_id_winning() {
        use crate::traits::ChatMessageKind;

        let sid = SessionId::new("step-1");
        let mut donor = crate::subagent_ui::SubagentUiState::new();
        donor.apply_notification(&sid, &tool_started("shared", "earlier shared"));
        donor.apply_notification(&sid, &tool_started("early", "earlier unique"));
        donor.apply_notification(&sid, &agent_msg("earlier pending", true));
        let Some(earlier) = donor.remove_stream(&sid) else {
            panic!("donor stream exists");
        };
        let mut state = WorkflowUiState::new();
        state.apply_notification(&sid, &tool_started("shared", "later shared"));
        state.apply_notification(&sid, &tool_started("late", "later unique"));
        state.apply_notification(&sid, &agent_msg("later pending", true));
        let live_before = state.streams()[&sid].live_revision();
        state.adopt(sid.clone(), earlier);
        let stream = &state.streams()[&sid];
        assert_eq!(
            stream.streaming_text(),
            "later pending",
            "recipient live text is not flushed"
        );
        assert_eq!(stream.live_revision(), live_before);
        assert_eq!(stream.activity(), Activity::Streaming);
        assert_eq!(stream.history().messages().len(), 5);
        assert!(matches!(
            stream.history().messages()[2].kind(),
            ChatMessageKind::AgentText(text) if text == "earlier pending"
        ));
        let before_updates = stream.history().revision();
        for id in ["shared", "early", "late"] {
            assert!(state.apply_notification(&sid, &tool_completed(id)));
        }
        let stream = &state.streams()[&sid];
        assert_eq!(
            stream.history().messages().len(),
            5,
            "updates merge without duplicate rows"
        );
        assert_eq!(
            stream
                .history()
                .changed_since(before_updates)
                .changed
                .count(),
            3
        );
        assert!(matches!(
            stream.history().messages()[0].kind(),
            ChatMessageKind::ToolCall(call)
                if call.title() == "earlier shared" && call.status() == ToolCallStatus::InProgress
        ));
        for index in [1, 3, 4] {
            assert!(matches!(
                stream.history().messages()[index].kind(),
                ChatMessageKind::ToolCall(call) if call.status() == ToolCallStatus::Completed
            ));
        }
        assert!(matches!(
            stream.history().messages()[3].kind(),
            ChatMessageKind::ToolCall(call) if call.title() == "later shared"
        ));
    }

    #[test]
    fn occupied_adoption_of_an_empty_stream_leaves_history_and_live_unchanged() {
        let sid = SessionId::new("step-1");
        let mut state = WorkflowUiState::new();
        state.apply_notification(&sid, &agent_msg("pending", true));
        let before = state.streams()[&sid].history().revision();
        let live_before = state.streams()[&sid].live_revision();
        state.adopt(sid.clone(), SubagentStream::new());
        assert_eq!(state.streams()[&sid].history().revision(), before);
        assert_eq!(state.streams()[&sid].live_revision(), live_before);
        assert_eq!(state.streams()[&sid].streaming_text(), "pending");
    }

    // C7 substrate: activity aggregation, with its adversarial counterpart.
    #[test]
    fn any_active_tracks_streaming_and_settles() {
        let mut state = WorkflowUiState::new();
        let sid = SessionId::new("step-1");
        state.apply_notification(&sid, &agent_msg("busy", true));
        assert!(state.any_active(), "a streaming step must count as active");
        state.apply_notification(&sid, &agent_msg(" done", false));
        assert!(
            !state.any_active(),
            "a settled step must not hold the fast tick"
        );
    }
}
