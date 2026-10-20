//! cyril-1v25: orphan updates use the ordinary chronological tool-row path.
//!
//! Fixture rows preserve the original JSON-RPC messages from these KAS captures:
//! audit-2.31.1/live/kas-lead-mcp-2.31.1.jsonl:43 and
//! audit-2.31.1/live/kas-v3c-v2-2.31.1.jsonl:46. The latter is KAS despite its name.
//!
//! The successful-output fixture is the byte-preserved sanitized completion from
//! /tmp/cyril-xavt-live.ZiHasL/evidence/completed-tool-call-nonsecret.json.
//! SHA-256: 0d613a49514f698c5dd3862d62a24b0fe5616f544d0078880898a441b68720cc.

use cyril_core::test_support::must_succeed;
use cyril_core::types::{
    AgentMessage, AgentThought, Notification, SessionId, StopReason, ToolCall, ToolCallContent,
    ToolCallId, ToolCallLocation, ToolCallStatus, ToolKind, UserMessage,
};
use cyril_ui::render::{self, RenderState};
use cyril_ui::state::UiState;
use cyril_ui::traits::{ChatMessageKind, TrackedToolCall, TuiState};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn orphan_call(id: &str) -> ToolCall {
    ToolCall::new(
        ToolCallId::new(id),
        "Tool Load".into(),
        ToolKind::Other,
        ToolCallStatus::Completed,
        Some(serde_json::json!({"tool_ids": ["probesrv::probe_echo"]})),
    )
    .with_raw_output(Some(serde_json::json!({"message": "loaded"})))
    .with_content(vec![ToolCallContent::Text("loaded".into())])
    .with_locations(vec![ToolCallLocation {
        path: "tool.json".into(),
        line: Some(7),
    }])
}

fn calls(state: &UiState) -> Vec<&TrackedToolCall> {
    state
        .history()
        .messages()
        .iter()
        .filter_map(|message| match message.kind() {
            ChatMessageKind::ToolCall(call) => Some(call),
            _ => None,
        })
        .collect()
}

fn render_text(state: &UiState) -> String {
    let mut terminal = must_succeed(Terminal::new(TestBackend::new(140, 45)), "terminal");
    let mut render_state = RenderState::default();
    must_succeed(
        terminal.draw(|frame| render::draw(frame, state, &mut render_state)),
        "draw root",
    );
    let buffer = terminal.backend().buffer();
    (0..45)
        .map(|y| {
            (0..140)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn kas_missing_start_updates_render_tool_load_and_subagent_response_once() {
    let notifications = cyril_core::test_support::kas_trace_to_routed(include_str!(
        "fixtures/kas_missing_start_2_31_1.jsonl"
    ));
    assert_eq!(
        notifications.len(),
        2,
        "both captured orphan updates convert"
    );
    let mut state = UiState::new(500);
    for (_, notification) in &notifications {
        assert!(matches!(notification, Notification::ToolCallUpdated(_)));
        state.apply_notification(notification);
    }
    assert_eq!(calls(&state).len(), 2);
    assert_eq!(
        calls(&state)[1]
            .raw_input()
            .and_then(|input| input["response"].as_str()),
        Some("ALPHA"),
    );
    let text = render_text(&state);
    assert_eq!(text.matches("Tool Load").count(), 1, "{text}");
    assert_eq!(text.matches("Subagent Response").count(), 1, "{text}");

    for (_, notification) in &notifications {
        state.apply_notification(notification);
    }
    state.commit_streaming();
    for (_, notification) in &notifications {
        state.apply_notification(notification);
    }
    assert_eq!(
        calls(&state).len(),
        2,
        "updates never duplicate retained IDs"
    );
    assert!(
        state.active_tool_calls().is_empty(),
        "late updates remain history"
    );
}

#[test]
fn kas_completed_output_capture_renders_exact_body_without_kind() {
    let captured: serde_json::Value = must_succeed(
        serde_json::from_str(include_str!("fixtures/kas_completed_raw_output.json")),
        "sanitized completed capture",
    );
    let frame = &captured["parsed"];
    let update = &frame["params"]["update"];
    assert!(
        update.get("kind").is_none(),
        "the real completion omits kind"
    );
    assert_eq!(
        update["content"],
        serde_json::json!([{"terminalId": "term-1", "type": "terminal"}]),
        "do not replace the terminal reference with synthetic text",
    );
    assert_eq!(
        update["rawOutput"],
        serde_json::json!({
            "exitCode": 0,
            "message": "Output:\nXAVT_BEGIN XAVT_END\n\n\nExit Code: 0",
            "output": "XAVT_BEGIN XAVT_END\n",
        }),
    );
    let notifications = cyril_core::test_support::kas_capture_to_routed(&must_succeed(
        serde_json::to_string(frame),
        "unchanged captured JSON-RPC frame",
    ));
    assert_eq!(notifications.len(), 1, "the completed frame converts");
    let (scope, notification) = &notifications[0];
    assert_eq!(
        scope.as_ref().map(SessionId::as_str),
        Some("sess_973df141-0181-47bf-977a-2f58ca21220f"),
    );
    assert!(matches!(notification, Notification::ToolCallUpdated(_)));
    let mut state = UiState::new(500);
    state.apply_notification(notification);
    state.commit_streaming();
    assert_eq!(calls(&state).len(), 1, "one completed row is retained");
    let retained = calls(&state)[0];
    assert_eq!(retained.status(), ToolCallStatus::Completed);
    assert_eq!(retained.kind(), ToolKind::Other);
    assert_eq!(retained.raw_input(), Some(&update["rawInput"]));
    assert_eq!(retained.raw_output(), Some(&update["rawOutput"]));
    let text = render_text(&state);
    assert_eq!(
        text.lines()
            .filter(|line| line.trim() == "✓ Run the exact sed/tr pipeline requested")
            .count(),
        1,
        "the completed header renders once\n{text}",
    );
    assert_eq!(
        (
            retained.output_text().as_deref(),
            text.lines()
                .filter(|line| line.trim() == "| XAVT_BEGIN XAVT_END")
                .count(),
        ),
        (Some("XAVT_BEGIN XAVT_END\n"), 1),
        "stdout must be an exact body line, not a header/command match\n{text}",
    );
    assert!(
        !text.contains("| Output:") && !text.contains("| Exit Code:"),
        "the formatted message is not the stdout projection\n{text}",
    );
}

#[test]
fn missing_start_update_commits_preceding_streams_at_arrival() {
    let mut state = UiState::new(500);
    state.apply_notification(&Notification::UserMessage(UserMessage {
        text: "user before".into(),
        is_streaming: true,
    }));
    state.apply_notification(&Notification::AgentMessage(AgentMessage {
        text: "agent before".into(),
        is_streaming: true,
    }));
    state.apply_notification(&Notification::AgentThought(AgentThought {
        text: "thought before".into(),
    }));
    let call = orphan_call("orphan");
    let revision = state.history().revision();
    state.apply_notification(&Notification::ToolCallUpdated(call.clone()));
    assert_ne!(state.history().revision(), revision);
    let mut changes = state.history().changed_since(revision);
    assert!(
        !changes.reset,
        "orphan insertion is an incremental history change"
    );
    assert!(
        changes.changed.any(|position| matches!(
            state.history().get(position),
            Some((message, _))
                if matches!(message.kind(), ChatMessageKind::ToolCall(tool) if tool.id() == call.id())
        )),
        "the inserted tool row invalidates rendering"
    );
    assert_eq!(state.history().messages().len(), 4);
    assert!(
        matches!(state.history().messages()[0].kind(), ChatMessageKind::UserText(t) if t == "user before")
    );
    assert!(
        matches!(state.history().messages()[1].kind(), ChatMessageKind::AgentText(t) if t == "agent before")
    );
    assert!(
        matches!(state.history().messages()[2].kind(), ChatMessageKind::Thought(t) if t == "thought before")
    );
    assert!(matches!(
        state.history().messages()[3].kind(),
        ChatMessageKind::ToolCall(_)
    ));
    assert_eq!(state.streaming_text(), "");
    assert!(state.streaming_thought().is_none());

    state.apply_notification(&Notification::ToolCallStarted(call.clone()));
    state.apply_notification(&Notification::ToolCallUpdated(call));
    assert_eq!(
        calls(&state).len(),
        1,
        "later starts reuse the existing row"
    );
    assert_eq!(state.active_tool_calls().len(), 1);
    state.apply_notification(&Notification::AgentMessage(AgentMessage {
        text: "agent after".into(),
        is_streaming: true,
    }));
    state.apply_notification(&Notification::TurnCompleted {
        stop_reason: StopReason::EndTurn,
    });
    assert!(
        matches!(state.history().messages()[4].kind(), ChatMessageKind::AgentText(t) if t == "agent after")
    );
}

#[test]
fn known_tool_updates_keep_normal_field_preserving_merge() {
    let mut state = UiState::new(500);
    let original = orphan_call("known");
    state.apply_notification(&Notification::ToolCallStarted(original.clone()));
    let revision = state.history().revision();
    state.apply_notification(&Notification::ToolCallUpdated(ToolCall::new(
        ToolCallId::new("known"),
        String::new(),
        ToolKind::Read,
        ToolCallStatus::Failed,
        None,
    )));
    assert_ne!(
        state.history().revision(),
        revision,
        "merged rows invalidate rendering"
    );
    let mut changes = state.history().changed_since(revision);
    assert!(
        !changes.reset,
        "a field merge is an incremental history edit"
    );
    let Some(position) = changes.changed.next() else {
        panic!("merged tool row must change");
    };
    assert!(
        changes.changed.next().is_none(),
        "only the merged row changes"
    );
    let Some((message, _)) = state.history().get(position) else {
        panic!("changed row is retained");
    };
    assert!(
        matches!(message.kind(), ChatMessageKind::ToolCall(call) if call.id() == original.id()),
        "the merge invalidates the resolved tool row"
    );
    for call in [calls(&state)[0], &state.active_tool_calls()[0]] {
        assert_eq!(call.title(), original.title());
        assert_eq!(call.kind(), ToolKind::Read);
        assert_eq!(call.status(), ToolCallStatus::Failed);
        assert_eq!(call.raw_input(), original.raw_input());
        assert_eq!(call.raw_output(), original.raw_output());
        assert_eq!(call.content(), original.content());
        assert_eq!(call.locations()[0].path, "tool.json");
        assert_eq!(call.locations()[0].line, Some(7));
    }
}

#[test]
fn retained_tool_index_survives_commit_and_history_pruning() {
    let mut state = UiState::new(2);
    state.add_system_message("discarded prefix".into());
    state.apply_notification(&Notification::ToolCallUpdated(orphan_call("retained")));
    state.commit_streaming();
    state.add_system_message("following message".into());
    let update = ToolCall::new(
        ToolCallId::new("retained"),
        "Renamed load".into(),
        ToolKind::Other,
        ToolCallStatus::Completed,
        None,
    );
    state.apply_notification(&Notification::ToolCallUpdated(update));
    assert_eq!(state.history().messages().len(), 2);
    assert!(
        matches!(state.history().messages()[0].kind(), ChatMessageKind::ToolCall(call) if call.title() == "Renamed load")
    );
    assert!(
        matches!(state.history().messages()[1].kind(), ChatMessageKind::System(text) if text == "following message")
    );
    assert!(state.active_tool_calls().is_empty());

    state.add_system_message("evict the tool".into());
    state.apply_notification(&Notification::ToolCallUpdated(orphan_call("retained")));
    assert_eq!(calls(&state).len(), 1, "evicted IDs can create a new row");
    assert!(matches!(
        state
            .history()
            .messages()
            .last()
            .map(|message| message.kind()),
        Some(ChatMessageKind::ToolCall(_))
    ));
}

#[test]
fn clearing_history_does_not_leave_a_stale_tool_index_or_duplicate_live_call() {
    let mut state = UiState::new(500);
    let call = orphan_call("cleared");
    state.apply_notification(&Notification::ToolCallStarted(call.clone()));
    state.clear_messages();
    state.add_system_message("unrelated retained row".into());
    state.apply_notification(&Notification::ToolCallUpdated(call));
    assert_eq!(state.history().messages().len(), 2);
    assert!(matches!(
        state.history().messages()[0].kind(),
        ChatMessageKind::System(_)
    ));
    assert_eq!(calls(&state).len(), 1);
    assert_eq!(state.active_tool_calls().len(), 1);
}

#[test]
fn reused_tool_id_in_a_new_session_preserves_the_old_session_row() {
    let session = |id| Notification::SessionCreated {
        session_id: SessionId::new(id),
        current_mode: None,
        current_model: None,
        available_modes: Vec::new(),
        available_models: Vec::new(),
    };
    for starts_with_update in [false, true] {
        let mut state = UiState::new(6);
        state.apply_notification(&session("session-a"));
        let original = orphan_call("session-local-id");
        state.apply_notification(&Notification::ToolCallStarted(original.clone()));
        state.commit_streaming();
        // Even an old-session live entry must retire at the session boundary.
        state.apply_notification(&Notification::ToolCallUpdated(orphan_call("old-live-id")));
        state.apply_notification(&session("session-b"));

        let current = ToolCall::new(
            ToolCallId::new("session-local-id"),
            "Current session read".into(),
            ToolKind::Read,
            ToolCallStatus::InProgress,
            Some(serde_json::json!({"path": "current-session.txt"})),
        )
        .with_content(vec![ToolCallContent::Text("current session body".into())]);
        let notification = if starts_with_update {
            Notification::ToolCallUpdated(current.clone())
        } else {
            Notification::ToolCallStarted(current.clone())
        };
        state.apply_notification(&notification);
        state.add_system_message("history suffix one".into());
        state.add_system_message("history suffix two".into());
        // Pruning shifts the retained old row and the current row. Only the
        // current session's index may receive this partial completion update.
        state.apply_notification(&Notification::ToolCallUpdated(ToolCall::new(
            ToolCallId::new("session-local-id"),
            "Current session completed".into(),
            ToolKind::Read,
            ToolCallStatus::Completed,
            None,
        )));
        let reused: Vec<_> = calls(&state)
            .into_iter()
            .filter(|call| call.id().as_str() == "session-local-id")
            .collect();
        assert_eq!(
            reused.len(),
            2,
            "session-local IDs need distinct history rows"
        );
        assert_eq!(reused[0].title(), original.title());
        assert_eq!(reused[0].raw_input(), original.raw_input());
        assert_eq!(reused[0].content(), original.content());
        assert_eq!(reused[0].raw_output(), original.raw_output());
        assert_eq!(reused[1].title(), "Current session completed");
        assert_eq!(reused[1].status(), ToolCallStatus::Completed);
        assert_eq!(reused[1].raw_input(), current.raw_input());
        assert_eq!(reused[1].content(), current.content());
        assert_eq!(state.active_tool_calls().len(), 1);
        assert_eq!(
            state.active_tool_calls()[0].id().as_str(),
            "session-local-id"
        );
    }
}
