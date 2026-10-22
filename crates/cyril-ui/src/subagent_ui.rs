use std::collections::HashMap;

use cyril_core::types::{Notification, SessionId, ToolCallId, message::AgentMessage};

use crate::message_history::{LiveRevision, MessageHistory, MessagePosition};
use crate::traits::{Activity, ChatMessage, ChatMessageKind, TrackedToolCall};

/// Per-subagent message stream.
pub struct SubagentStream {
    messages: MessageHistory,
    streaming_text: String,
    live_revision: LiveRevision,
    tool_call_index: HashMap<ToolCallId, MessagePosition>,
    activity: Activity,
}

impl SubagentStream {
    pub(crate) fn new() -> Self {
        Self {
            messages: MessageHistory::new(),
            streaming_text: String::new(),
            live_revision: LiveRevision::default(),
            tool_call_index: HashMap::new(),
            activity: Activity::Idle,
        }
    }

    pub fn history(&self) -> &MessageHistory {
        &self.messages
    }

    pub fn live_revision(&self) -> LiveRevision {
        self.live_revision
    }

    pub fn streaming_text(&self) -> &str {
        &self.streaming_text
    }

    /// The stream's latest agent reply — still streaming, else the newest
    /// committed one (cyril-9104: a parked workflow step's question is its
    /// own last reply). `None` when it has said nothing.
    pub fn latest_agent_text(&self) -> Option<&str> {
        if !self.streaming_text.trim().is_empty() {
            return Some(&self.streaming_text);
        }
        self.messages
            .messages()
            .iter()
            .rev()
            .find_map(|message| match message.kind() {
                ChatMessageKind::AgentText(text) => Some(text.as_str()),
                _ => None,
            })
    }

    pub fn activity(&self) -> Activity {
        self.activity
    }

    /// Mark this stream as terminated — sets activity to Ready so it no longer
    /// counts as active for frame rate purposes.
    pub fn mark_terminated(&mut self) {
        self.activity = Activity::Ready;
    }

    /// True if the stream is in a settled state (Ready or Idle) — i.e., it
    /// is not actively streaming text or running tools.
    ///
    /// Note: this also returns true for newly-created streams that have
    /// never received a notification (they start in `Activity::Idle`). The
    /// method is named for its usage in `apply_list_update`, which uses it
    /// to skip streams that are already settled when a session disappears
    /// from the active list.
    pub fn is_terminated(&self) -> bool {
        matches!(self.activity, Activity::Ready | Activity::Idle)
    }

    /// Merges `earlier` — an adopted optimistic stream whose frames all
    /// predate this stream's — in front of this history (cyril-jxfu
    /// occupied-adopt). Chronological order is preserved (adopted messages
    /// arrived first) and both sides' tool-call indices stay live; on an id
    /// collision the later (self) entry wins.
    pub(crate) fn absorb_earlier(&mut self, mut earlier: SubagentStream) {
        earlier.flush_streaming_text();
        if earlier.messages.messages().is_empty() {
            return;
        }
        self.messages.prepend(earlier.messages);
        self.tool_call_index.clear();
        let start = self.messages.retained_range().start.0;
        for (index, message) in self.messages.messages().iter().enumerate() {
            if let ChatMessageKind::ToolCall(tracked) = message.kind() {
                self.tool_call_index
                    .insert(tracked.id().clone(), MessagePosition(start + index));
            }
        }
    }

    /// Append a system line after committing any streaming text, so it lands
    /// at its chronological position (cyril-lki9 C16 — same discipline as
    /// `UiState::add_system_message`).
    pub(crate) fn push_system(&mut self, text: String) {
        self.flush_streaming_text();
        self.messages.push(ChatMessage::system(text));
    }

    fn flush_streaming_text(&mut self) {
        if !self.streaming_text.is_empty() {
            let text = std::mem::take(&mut self.streaming_text);
            self.messages.push(ChatMessage::agent_text(text));
            self.live_revision.bump_text();
        }
    }

    pub(crate) fn apply_notification(&mut self, notification: &Notification) -> bool {
        match notification {
            Notification::AgentMessage(AgentMessage { text, is_streaming }) => {
                if !text.is_empty() {
                    self.live_revision.bump_text();
                }
                if *is_streaming {
                    self.streaming_text.push_str(text);
                    self.activity = Activity::Streaming;
                } else {
                    self.streaming_text.push_str(text);
                    self.flush_streaming_text();
                    self.activity = Activity::Ready;
                }
                true
            }
            Notification::ToolCallStarted(tc) => {
                self.flush_streaming_text();
                let tracked = TrackedToolCall::new(tc.clone());
                let position = self.messages.push(ChatMessage::tool_call(tracked));
                self.tool_call_index.insert(tc.id().clone(), position);
                self.activity = Activity::ToolRunning;
                true
            }
            Notification::ToolCallUpdated(tc) => {
                if let Some(&position) = self.tool_call_index.get(tc.id())
                    && let Some((message, _)) = self.messages.get(position)
                    && matches!(message.kind(), ChatMessageKind::ToolCall(tracked) if tracked.id() == tc.id())
                {
                    self.messages.edit(position, |message| {
                        if let ChatMessageKind::ToolCall(tracked) = &mut message.kind {
                            tracked.update(tc);
                        }
                    });
                    return true;
                }
                tracing::debug!(
                    tool_call_id = tc.id().as_str(),
                    "ToolCallUpdated for unknown tool call id in subagent stream, dropping"
                );
                false
            }
            Notification::ToolCallChunk { .. } => {
                self.activity = Activity::ToolRunning;
                true
            }
            Notification::TurnCompleted { .. } => {
                self.flush_streaming_text();
                self.activity = Activity::Ready;
                true
            }
            Notification::PlanUpdated(plan) => {
                self.messages.push(ChatMessage::plan(plan.clone()));
                true
            }
            _ => false,
        }
    }
}

/// Owns per-subagent message streams and drill-in focus.
pub struct SubagentUiState {
    streams: HashMap<SessionId, SubagentStream>,
    focused: Option<SessionId>,
}

impl SubagentUiState {
    pub fn new() -> Self {
        Self {
            streams: HashMap::new(),
            focused: None,
        }
    }

    /// Route a notification to the appropriate subagent stream.
    /// Creates the stream on first contact.
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

    /// Update streams when a `SubagentListUpdated` arrives. For each stream
    /// whose session_id is no longer in the active list, marks its activity
    /// as Ready so it no longer counts toward the active frame rate.
    ///
    /// Streams are **never removed** — their message history remains
    /// viewable in drill-in after the subagent terminates. Returns true only
    /// if at least one stream's activity actually transitioned.
    pub fn apply_list_update(&mut self, subagents: &[cyril_core::types::SubagentInfo]) -> bool {
        let active_ids: std::collections::HashSet<&SessionId> =
            subagents.iter().map(|s| s.session_id()).collect();

        // Don't remove streams — they may still have messages the user wants to see.
        // Just mark activity as Ready for terminated sessions that are still active.
        let mut changed = false;
        for (id, stream) in &mut self.streams {
            if !active_ids.contains(id) && !stream.is_terminated() {
                stream.mark_terminated();
                changed = true;
            }
        }
        changed
    }

    /// Focus a subagent session for drill-in rendering. Returns `true` if
    /// the session has a stream (focus was set), `false` if the session is
    /// unknown (focus is a no-op).
    pub fn focus(&mut self, session_id: SessionId) -> bool {
        if self.streams.contains_key(&session_id) {
            self.focused = Some(session_id);
            true
        } else {
            tracing::warn!(
                session_id = session_id.as_str(),
                "focus() called for unknown session, ignoring"
            );
            false
        }
    }

    pub fn unfocus(&mut self) {
        self.focused = None;
    }

    /// Removes a stream from the store, returning it for adoption elsewhere
    /// (cyril-jxfu re-parenting: a late workflow claim moves the optimistic
    /// stream, history intact, out of the subagent domain). Clears drill-in
    /// focus when it pointed at the removed session — this is the one seam
    /// where a dangling focus cannot be forgotten by a caller.
    pub fn remove_stream(&mut self, session_id: &SessionId) -> Option<SubagentStream> {
        let removed = self.streams.remove(session_id);
        if removed.is_some() && self.focused.as_ref() == Some(session_id) {
            self.focused = None;
        }
        removed
    }

    pub fn focused_session_id(&self) -> Option<&SessionId> {
        self.focused.as_ref()
    }

    pub fn focused_stream(&self) -> Option<&SubagentStream> {
        self.focused.as_ref().and_then(|id| self.streams.get(id))
    }

    pub fn streams(&self) -> &HashMap<SessionId, SubagentStream> {
        &self.streams
    }

    /// Returns true if any subagent stream is actively streaming or running tools.
    pub fn any_active(&self) -> bool {
        self.streams
            .values()
            .any(|s| matches!(s.activity, Activity::Streaming | Activity::ToolRunning))
    }
}

impl Default for SubagentUiState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use cyril_core::types::{ToolCall, ToolCallId, ToolCallStatus, ToolKind};

    fn make_agent_msg(text: &str, is_streaming: bool) -> Notification {
        Notification::AgentMessage(AgentMessage {
            text: text.into(),
            is_streaming,
        })
    }

    fn make_tool_call(id: &str, title: &str) -> Notification {
        Notification::ToolCallStarted(ToolCall::new(
            ToolCallId::new(id),
            title.into(),
            ToolKind::Read,
            ToolCallStatus::InProgress,
            None,
        ))
    }

    #[test]
    fn live_text_revision_changes_only_for_real_source_mutations() {
        let mut stream = SubagentStream::new();
        let initial = stream.live_revision();
        let history = stream.history().revision();
        stream.apply_notification(&make_agent_msg("", true));
        assert_eq!(stream.live_revision(), initial);
        stream.apply_notification(&make_agent_msg("live", true));
        let streaming = stream.live_revision();
        assert_ne!(streaming.text, initial.text);
        assert_eq!(streaming.thought, initial.thought);
        assert_eq!(stream.history().revision(), history);
        stream.mark_terminated();
        assert_eq!(stream.live_revision(), streaming);
        stream.flush_streaming_text();
        assert_ne!(stream.live_revision().text, streaming.text);
        assert_ne!(stream.history().revision(), history);
        let settled = stream.live_revision();
        stream.flush_streaming_text();
        assert_eq!(stream.live_revision(), settled);
    }

    #[test]
    fn unknown_and_wrong_id_child_updates_never_edit_an_unrelated_row() {
        let mut stream = SubagentStream::new();
        stream.apply_notification(&make_tool_call("b", "retained b"));
        let b = stream.tool_call_index[&ToolCallId::new("b")];
        let before = stream.history().revision();
        let update_a = Notification::ToolCallUpdated(ToolCall::new(
            ToolCallId::new("a"),
            "late a".to_owned(),
            ToolKind::Read,
            ToolCallStatus::Failed,
            None,
        ));
        assert!(
            !stream.apply_notification(&update_a),
            "child unknown-update policy remains drop"
        );
        assert_eq!(stream.history().revision(), before);
        stream.tool_call_index.insert(ToolCallId::new("a"), b);
        assert!(
            !stream.apply_notification(&update_a),
            "validate the requested id before editing"
        );
        assert_eq!(stream.history().revision(), before);
        assert_eq!(stream.history().messages().len(), 1);
        let update_b = Notification::ToolCallUpdated(ToolCall::new(
            ToolCallId::new("b"),
            "updated b".to_owned(),
            ToolKind::Read,
            ToolCallStatus::Completed,
            None,
        ));
        assert!(stream.apply_notification(&update_b));
        assert_eq!(
            stream
                .history()
                .changed_since(before)
                .changed
                .collect::<Vec<_>>(),
            [b]
        );
        assert!(matches!(
            stream.history().get(b).map(|(message, _)| message.kind()),
            Some(ChatMessageKind::ToolCall(call)) if call.title() == "updated b"
        ));
    }

    #[test]
    fn repeated_child_starts_remain_distinct_and_latest_start_receives_updates() {
        let mut stream = SubagentStream::new();
        stream.apply_notification(&make_tool_call("same", "earlier"));
        stream.apply_notification(&make_tool_call("same", "later"));
        assert_eq!(stream.history().messages().len(), 2);
        let before = stream.history().revision();
        let latest = stream.tool_call_index[&ToolCallId::new("same")];
        stream.apply_notification(&Notification::ToolCallUpdated(ToolCall::new(
            ToolCallId::new("same"),
            String::new(),
            ToolKind::Read,
            ToolCallStatus::Completed,
            None,
        )));
        assert_eq!(
            stream
                .history()
                .changed_since(before)
                .changed
                .collect::<Vec<_>>(),
            [latest]
        );
        assert!(matches!(
            stream.history().messages()[0].kind(),
            ChatMessageKind::ToolCall(call)
                if call.title() == "earlier" && call.status() == ToolCallStatus::InProgress
        ));
        assert!(matches!(
            stream.history().messages()[1].kind(),
            ChatMessageKind::ToolCall(call)
                if call.title() == "later" && call.status() == ToolCallStatus::Completed
        ));
    }

    #[test]
    fn creates_stream_on_first_notification() {
        let mut state = SubagentUiState::new();
        let sid = SessionId::new("sub-1");
        state.apply_notification(&sid, &make_agent_msg("hello", true));
        assert!(state.streams.contains_key(&sid));
        assert_eq!(state.streams[&sid].streaming_text(), "hello");
    }

    #[test]
    fn message_lifecycle_text_tool_text() {
        let mut state = SubagentUiState::new();
        let sid = SessionId::new("sub-1");

        // Stream some text
        state.apply_notification(&sid, &make_agent_msg("part1 ", true));
        state.apply_notification(&sid, &make_agent_msg("part2", true));

        // Tool call starts — flushes streaming text
        state.apply_notification(&sid, &make_tool_call("tc-1", "Reading file.rs"));

        // More text after tool call
        state.apply_notification(&sid, &make_agent_msg("done", false));

        let stream = &state.streams[&sid];
        assert_eq!(stream.history().messages().len(), 3);
        assert!(matches!(
            stream.history().messages()[0].kind,
            ChatMessageKind::AgentText(_)
        ));
        assert!(matches!(
            stream.history().messages()[1].kind,
            ChatMessageKind::ToolCall(_)
        ));
        assert!(matches!(
            stream.history().messages()[2].kind,
            ChatMessageKind::AgentText(_)
        ));

        if let ChatMessageKind::AgentText(ref text) = stream.history().messages()[0].kind {
            assert_eq!(text, "part1 part2");
        }
    }

    #[test]
    fn turn_completed_flushes_streaming() {
        let mut state = SubagentUiState::new();
        let sid = SessionId::new("sub-1");
        state.apply_notification(&sid, &make_agent_msg("final text", true));
        state.apply_notification(
            &sid,
            &Notification::TurnCompleted {
                stop_reason: cyril_core::types::StopReason::EndTurn,
            },
        );

        let stream = &state.streams[&sid];
        assert_eq!(stream.streaming_text(), "");
        assert_eq!(stream.history().messages().len(), 1);
        assert_eq!(stream.activity(), Activity::Ready);
    }

    #[test]
    fn focus_unfocus_transitions() {
        let mut state = SubagentUiState::new();
        let sid = SessionId::new("sub-1");

        assert!(state.focused_session_id().is_none());

        // Create a stream first so focus() accepts the session.
        state.apply_notification(&sid, &make_agent_msg("hello", false));

        assert!(state.focus(sid.clone()));
        assert_eq!(state.focused_session_id(), Some(&sid));

        state.unfocus();
        assert!(state.focused_session_id().is_none());
    }

    #[test]
    fn focus_rejects_unknown_session() {
        let mut state = SubagentUiState::new();
        let unknown = SessionId::new("ghost");
        assert!(!state.focus(unknown));
        assert!(state.focused_session_id().is_none());
    }

    #[test]
    fn focused_stream_returns_correct_stream() {
        let mut state = SubagentUiState::new();
        let sid = SessionId::new("sub-1");
        state.apply_notification(&sid, &make_agent_msg("hello", false));
        state.focus(sid.clone());

        let stream = state.focused_stream().expect("should have focused stream");
        assert_eq!(stream.history().messages().len(), 1);
    }

    #[test]
    fn any_active_detects_streaming() {
        let mut state = SubagentUiState::new();
        assert!(!state.any_active());

        let sid = SessionId::new("sub-1");
        state.apply_notification(&sid, &make_agent_msg("streaming", true));
        assert!(state.any_active());

        state.apply_notification(
            &sid,
            &Notification::TurnCompleted {
                stop_reason: cyril_core::types::StopReason::EndTurn,
            },
        );
        assert!(!state.any_active());
    }

    #[test]
    fn list_update_marks_removed_streams_ready() {
        let mut state = SubagentUiState::new();
        let sid = SessionId::new("sub-1");
        state.apply_notification(&sid, &make_agent_msg("working", true));
        assert!(state.any_active());

        // List update with no subagents — sub-1 is gone
        state.apply_list_update(&[]);
        assert!(!state.any_active());
        // Stream is preserved for history
        assert!(state.streams.contains_key(&sid));
    }

    #[test]
    fn list_update_preserves_active_stream() {
        let mut state = SubagentUiState::new();
        let sid1 = SessionId::new("sub-1");
        let sid2 = SessionId::new("sub-2");
        state.apply_notification(&sid1, &make_agent_msg("streaming", true));
        state.apply_notification(&sid2, &make_agent_msg("also streaming", true));

        // List update keeps sub-1 but removes sub-2
        let active = vec![cyril_core::types::SubagentInfo::new(
            sid1.clone(),
            "reviewer",
            "reviewer",
            "query",
            cyril_core::types::SubagentStatus::Working {
                message: Some("Running".into()),
            },
        )];
        state.apply_list_update(&active);

        // sub-1 should still be streaming
        assert_eq!(state.streams[&sid1].activity(), Activity::Streaming);
        // sub-2 should be marked ready (terminated)
        assert_eq!(state.streams[&sid2].activity(), Activity::Ready);
    }

    #[test]
    fn tool_call_inserted_at_correct_position() {
        let mut state = SubagentUiState::new();
        let sid = SessionId::new("sub-1");

        state.apply_notification(&sid, &make_agent_msg("before tool", true));
        state.apply_notification(&sid, &make_tool_call("tc-1", "read"));
        state.apply_notification(&sid, &make_agent_msg("after tool", true));
        state.apply_notification(
            &sid,
            &Notification::TurnCompleted {
                stop_reason: cyril_core::types::StopReason::EndTurn,
            },
        );

        let stream = &state.streams[&sid];
        // Should be: AgentText("before tool"), ToolCall, AgentText("after tool")
        assert_eq!(stream.history().messages().len(), 3);
    }

    // ── cyril-jxfu C8 seam: remove_stream ────────────────────────────────────

    #[test]
    fn remove_stream_missing_key_is_none_and_store_unchanged() {
        let mut state = SubagentUiState::new();
        state.apply_notification(&SessionId::new("sub-1"), &make_agent_msg("hi", false));

        assert!(state.remove_stream(&SessionId::new("sub-absent")).is_none());
        assert_eq!(state.streams.len(), 1);
    }

    #[test]
    fn remove_stream_clears_focus_and_returns_history() {
        let mut state = SubagentUiState::new();
        let sid = SessionId::new("sub-1");
        state.apply_notification(&sid, &make_agent_msg("first", false));
        state.apply_notification(&sid, &make_tool_call("tc-1", "read"));
        assert!(state.focus(sid.clone()));

        let Some(stream) = state.remove_stream(&sid) else {
            panic!("existing stream must be returned");
        };
        assert_eq!(
            stream.history().messages().len(),
            2,
            "history rides the removal"
        );
        assert!(
            state.focused_session_id().is_none(),
            "removing the focused stream must clear drill-in focus"
        );
        assert!(state.streams.is_empty());
    }

    #[test]
    fn remove_stream_of_unfocused_session_retains_focus() {
        let mut state = SubagentUiState::new();
        let kept = SessionId::new("sub-kept");
        let removed = SessionId::new("sub-removed");
        state.apply_notification(&kept, &make_agent_msg("kept", false));
        state.apply_notification(&removed, &make_agent_msg("removed", false));
        assert!(state.focus(kept.clone()));

        assert!(state.remove_stream(&removed).is_some());
        assert_eq!(
            state.focused_session_id(),
            Some(&kept),
            "removing an unfocused stream must not disturb focus elsewhere"
        );
    }
}
