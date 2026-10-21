//! cyril-fiv1: genuine v2 chunks plus failed updates, never a start.
//!
//! Frames are copied losslessly from trace-2.4.1-multi-subagent.jsonl:5244-5246
//! and v2-live-session-trace-2.11.0.jsonl:51-56. The first includes an unrelated
//! interleaved read chunk. Both errors exist only in content, not rawOutput.

use cyril_core::test_support::{must_succeed, v2_trace_to_routed};
use cyril_core::types::{
    Notification, StopReason, ToolCall, ToolCallContent, ToolCallId, ToolCallStatus, ToolKind,
};
use cyril_ui::state::UiState;
use cyril_ui::traits::{Activity, ChatMessageKind, TrackedToolCall, TuiState};
use cyril_ui::widgets::chat;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn rendered(state: &UiState) -> String {
    let mut terminal = must_succeed(Terminal::new(TestBackend::new(150, 36)), "terminal");
    must_succeed(
        terminal.draw(|frame| chat::render(frame, frame.area(), state, &state.theme())),
        "render validation failure",
    );
    let buffer = terminal.backend().buffer();
    (0..36)
        .map(|y| {
            (0..150)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn v2_failed_validation_captures_render_one_attributed_failure_and_its_error() {
    for (fixture, id, expected_error, expected_frames) in [
        (
            include_str!("fixtures/v2_failed_validation_2_4_1.jsonl"),
            "tooluse_9uJVwIlCX6HkwpKd1az7hm",
            "The tool arguments failed validation: '/home/dwalleck/repos/cyril/crates/cyril-core/src/protocol/convert' is not a file",
            3,
        ),
        (
            include_str!("fixtures/v2_failed_validation_2_11_0.jsonl"),
            "tooluse_s3x9l2sZ9aE7Sazt0allDj",
            "The tool arguments failed validation: '/home/dwalleck/repos/rivets/CONTEXT.md' does not exist",
            6,
        ),
    ] {
        let notifications = v2_trace_to_routed(fixture);
        assert_eq!(
            notifications.len(),
            expected_frames,
            "all v2 frames convert"
        );
        let mut state = UiState::new(500);
        let mut failed_update = None;
        for (_, notification) in &notifications {
            assert!(!matches!(notification, Notification::ToolCallStarted(_)));
            if let Notification::ToolCallUpdated(call) = notification {
                assert_eq!(call.id().as_str(), id);
                assert!(failed_update.replace(call).is_none());
            }
            state.apply_notification(notification);
            if matches!(notification, Notification::ToolCallChunk { .. }) {
                assert_eq!(state.activity(), Activity::ToolRunning);
                assert!(
                    state
                        .messages()
                        .iter()
                        .all(|message| { !matches!(message.kind(), ChatMessageKind::ToolCall(_)) }),
                    "a chunk alone is activity, not a fabricated result"
                );
            }
        }
        state.apply_notification(&Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        });
        let calls: Vec<_> = state
            .messages()
            .iter()
            .filter_map(|message| match message.kind() {
                ChatMessageKind::ToolCall(call) => Some(call),
                _ => None,
            })
            .collect();
        assert_eq!(
            calls.len(),
            1,
            "only the failed update commits a result row"
        );
        let call = calls[0];
        let Some(original) = failed_update else {
            panic!("capture must contain the failed update");
        };
        assert_eq!(call.id().as_str(), id);
        assert_eq!(call.status(), ToolCallStatus::Failed);
        assert_eq!(call.kind(), ToolKind::Read);
        assert_eq!(call.raw_input(), original.raw_input());
        assert!(call.raw_output().is_none());
        assert_eq!(call.error_message().as_deref(), Some(expected_error));
        let text = rendered(&state);
        assert_eq!(text.matches("✗ ").count(), 1, "{text}");
        assert!(
            text.contains(expected_error),
            "validation message must render: {text}"
        );
        assert!(state.active_tool_calls().is_empty());
        assert_eq!(state.activity(), Activity::Ready);

        if id == "tooluse_s3x9l2sZ9aE7Sazt0allDj" {
            let before = text.find("there's an existing CONTEXT.md");
            let failure = text.find("The tool arguments failed validation");
            let after = text.find("No existing domain model.");
            assert!(
                before.is_some() && before < failure && failure < after,
                "{text}"
            );
        }
    }
}

#[test]
fn content_backed_error_fallback_preserves_raw_output_precedence_and_status() {
    let failed = ToolCall::new(
        ToolCallId::new("error-shapes"),
        "read".into(),
        ToolKind::Read,
        ToolCallStatus::Failed,
        None,
    )
    .with_content(vec![
        ToolCallContent::Text(String::new()),
        ToolCallContent::Text("content validation error".into()),
    ]);
    let raw = failed
        .clone()
        .with_raw_output(Some(serde_json::json!({"message": "raw error"})));
    assert_eq!(
        TrackedToolCall::new(raw).error_message().as_deref(),
        Some("raw error")
    );
    assert_eq!(
        TrackedToolCall::new(failed.clone())
            .error_message()
            .as_deref(),
        Some("content validation error")
    );
    let unrecognized_output = failed
        .clone()
        .with_raw_output(Some(serde_json::json!({"other": "value"})));
    assert_eq!(
        TrackedToolCall::new(unrecognized_output)
            .error_message()
            .as_deref(),
        Some("content validation error")
    );
    let mut completed = failed;
    completed.merge_update(&ToolCall::new(
        ToolCallId::new("error-shapes"),
        String::new(),
        ToolKind::Read,
        ToolCallStatus::Completed,
        None,
    ));
    assert_eq!(TrackedToolCall::new(completed).error_message(), None);
}

#[test]
fn blank_raw_errors_fall_back_to_visible_content_without_changing_precedence() {
    const CONTENT_ERROR: &str = "validation error from content";
    const RAW_ERROR: &str = "  nonblank raw error \t";
    for (raw_output, expected_error) in [
        (serde_json::json!(""), CONTENT_ERROR),
        (serde_json::json!(" \t\n"), CONTENT_ERROR),
        (serde_json::json!({"error": ""}), CONTENT_ERROR),
        (serde_json::json!({"message": " \t"}), CONTENT_ERROR),
        (
            serde_json::json!({"error": " \t", "message": ""}),
            CONTENT_ERROR,
        ),
        (serde_json::json!({"output": " \t"}), CONTENT_ERROR),
        (serde_json::json!({"items": [{"Text": ""}]}), CONTENT_ERROR),
        (serde_json::json!(RAW_ERROR), RAW_ERROR),
        (
            serde_json::json!({"error": RAW_ERROR, "message": "lower priority"}),
            RAW_ERROR,
        ),
        (
            serde_json::json!({"error": "", "message": RAW_ERROR}),
            RAW_ERROR,
        ),
        (
            serde_json::json!({"error": "", "stdout": RAW_ERROR}),
            RAW_ERROR,
        ),
        (serde_json::json!({"output": RAW_ERROR}), RAW_ERROR),
    ] {
        let content = vec![
            ToolCallContent::Text(" \t".into()),
            ToolCallContent::Text(CONTENT_ERROR.into()),
            ToolCallContent::Text("later content must stay hidden".into()),
        ];
        let failed = ToolCall::new(
            ToolCallId::new("blank-raw-error"),
            "read".into(),
            ToolKind::Read,
            ToolCallStatus::Failed,
            None,
        )
        .with_raw_output(Some(raw_output.clone()))
        .with_content(content.clone());
        let mut state = UiState::new(500);
        state.apply_notification(&Notification::ToolCallUpdated(failed));
        state.apply_notification(&Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        });
        let calls: Vec<_> = state
            .messages()
            .iter()
            .filter_map(|message| match message.kind() {
                ChatMessageKind::ToolCall(call) => Some(call),
                _ => None,
            })
            .collect();
        assert_eq!(calls.len(), 1, "raw output: {raw_output}");
        let call = calls[0];
        assert_eq!(
            call.error_message().as_deref(),
            Some(expected_error),
            "raw output: {raw_output}"
        );
        assert_eq!(call.raw_output(), Some(&raw_output));
        assert_eq!(call.content(), content.as_slice());
        let text = rendered(&state);
        assert_eq!(text.matches("✗ ").count(), 1, "{text}");
        assert_eq!(
            text.matches(expected_error.trim()).count(),
            1,
            "raw output: {raw_output}; screen: {text}"
        );
        assert!(!text.contains("later content must stay hidden"), "{text}");
        if expected_error == RAW_ERROR {
            assert!(!text.contains(CONTENT_ERROR), "{text}");
        }
    }
}

#[test]
fn failed_raw_selection_skips_blank_fields_without_changing_completed_output() {
    const RAW_ERROR: &str = "  actual raw error \t";
    const CONTENT_ERROR: &str = "content must not hide usable raw text";
    for (raw_output, completed_output) in [
        (serde_json::json!({"text": "", "output": RAW_ERROR}), ""),
        (
            serde_json::json!({"items": [{"Text": " \t"}], "output": RAW_ERROR}),
            " \t",
        ),
    ] {
        let make_call = |status| {
            ToolCall::new(
                ToolCallId::new("mixed-raw-fields"),
                "mixed raw fields".into(),
                ToolKind::Other,
                status,
                None,
            )
            .with_raw_output(Some(raw_output.clone()))
            .with_content(vec![ToolCallContent::Text(CONTENT_ERROR.into())])
        };
        let completed = TrackedToolCall::new(make_call(ToolCallStatus::Completed));
        assert_eq!(
            completed.output_text().as_deref(),
            Some(completed_output),
            "Completed output must retain the first blank candidate: {raw_output}"
        );
        assert_eq!(completed.error_message(), None);
        assert_eq!(completed.raw_output(), Some(&raw_output));

        let failed = make_call(ToolCallStatus::Failed);
        let tracked = TrackedToolCall::new(failed.clone());
        assert_eq!(
            tracked.error_message().as_deref(),
            Some(RAW_ERROR),
            "failed output must continue past blank candidates: {raw_output}"
        );
        assert_eq!(
            tracked.output_text().as_deref(),
            Some(completed_output),
            "error selection must not change the public output projection"
        );
        assert_eq!(tracked.raw_output(), Some(&raw_output));
        let mut state = UiState::new(500);
        state.apply_notification(&Notification::ToolCallUpdated(failed));
        state.apply_notification(&Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        });
        let text = rendered(&state);
        assert_eq!(text.matches("✗ ").count(), 1, "{text}");
        assert_eq!(text.matches(RAW_ERROR.trim()).count(), 1, "{text}");
        assert!(!text.contains(CONTENT_ERROR), "{text}");
    }
}
