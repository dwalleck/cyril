const EXPECTED_TOOL_ID_BYTES: usize = 1024;
const EXPECTED_TOOL_NAME_BYTES: usize = 8 * 1024;
const EXPECTED_TOOL_STATUS_BYTES: usize = 256;
const EXPECTED_TOOL_INPUT_BYTES: usize = 24 * 1024;
const EXPECTED_TOOL_RESULT_BYTES: usize = 24 * 1024;
use super::*;
use crate::types::source_turn::{SOURCE_EVENT_CHANNEL_CAPACITY, SOURCE_FRAGMENT_BYTES};

#[test]
fn c8_source_contract_matrix() {
    c8_original_only_capture();
    c8_utf8_fragmentation_boundaries();
    c8_disposition_matrix();
    c8_identity_matrix();
    c8_tool_field_boundaries();
}

fn c8_original_only_capture() {
    let cell = "original_only.prepared_context_excluded";
    let (tx, mut rx) = mpsc::channel(8);
    let observer = SourceObserver::new(tx);
    let prompt = PromptEnvelope::prepared(
        vec!["original user prompt".to_owned()],
        Some("private prepared context".to_owned()),
    );
    let source_turn_id = observer
        .begin(
            SessionId::new("session-original"),
            TurnId::new(1),
            prompt.original_blocks(),
        )
        .unwrap_or_else(|error| panic!("C8 {cell}: source id generation failed: {error}"));
    let events = drain(&mut rx);

    assert_eq!(events.len(), 2, "C8 {cell}: unexpected event count");
    assert_eq!(
        events[0].source_turn_id(),
        source_turn_id,
        "C8 {cell}: started identity"
    );
    assert_eq!(
        events[1].source_turn_id(),
        source_turn_id,
        "C8 {cell}: prompt identity"
    );
    assert_eq!(events[0].sequence(), 0, "C8 {cell}: started sequence");
    assert_eq!(events[1].sequence(), 1, "C8 {cell}: prompt sequence");
    assert!(
        matches!(
            events[0].kind(),
            SourceTurnEventKind::Started {
                bridge_turn_id: 1,
                block_count: 1,
                ..
            }
        ),
        "C8 {cell}: incorrect started ledger"
    );
    assert!(
        matches!(
            events[1].kind(),
            SourceTurnEventKind::PromptFragment {
                block_index: 0,
                fragment_index: 0,
                text,
                is_last: true,
            } if text == "original user prompt"
        ),
        "C8 {cell}: original block was not captured exactly"
    );
    assert!(
        events
            .iter()
            .all(|event| !format!("{:?}", event.kind()).contains("private prepared context")),
        "C8 {cell}: prepared context leaked into source capture"
    );
}

fn c8_utf8_fragmentation_boundaries() {
    let exact = "a".repeat(SOURCE_FRAGMENT_BYTES);
    let prefix = "a".repeat(SOURCE_FRAGMENT_BYTES - "🦀".len() + 1);
    let plus_one = format!("{prefix}🦀");
    let cases = [
        ("utf8.exact", exact, vec!["a".repeat(SOURCE_FRAGMENT_BYTES)]),
        (
            "utf8.plus_one_multibyte_boundary",
            plus_one,
            vec![prefix, "🦀".to_owned()],
        ),
    ];

    for (cell, prompt, expected_fragments) in cases {
        let (tx, mut rx) = mpsc::channel(8);
        let observer = SourceObserver::new(tx);
        observer
            .begin(
                SessionId::new("session-utf8"),
                TurnId::new(2),
                std::slice::from_ref(&prompt),
            )
            .unwrap_or_else(|error| panic!("C8 {cell}: source id generation failed: {error}"));
        let events = drain(&mut rx);
        let prompt_events: Vec<_> = events
            .iter()
            .filter_map(|event| match event.kind() {
                SourceTurnEventKind::PromptFragment {
                    block_index,
                    fragment_index,
                    text,
                    is_last,
                } => Some((*block_index, *fragment_index, text, *is_last)),
                _ => None,
            })
            .collect();
        let actual_fragments: Vec<_> = prompt_events
            .iter()
            .map(|(_, _, text, _)| (*text).clone())
            .collect();
        for (expected_index, (block_index, fragment_index, text, is_last)) in
            prompt_events.iter().enumerate()
        {
            assert_eq!(*block_index, 0, "C8 {cell}: block index");
            assert_eq!(*fragment_index, expected_index, "C8 {cell}: fragment index");
            assert!(
                text.is_char_boundary(text.len()),
                "C8 {cell}: split code point"
            );
            assert_eq!(
                *is_last,
                expected_index + 1 == prompt_events.len(),
                "C8 {cell}: fragment last marker"
            );
        }
        assert_eq!(
            prompt_events.len(),
            expected_fragments.len(),
            "C8 {cell}: fragment count"
        );
        assert_eq!(
            actual_fragments, expected_fragments,
            "C8 {cell}: UTF-8 fragment ledger"
        );
        assert_eq!(
            actual_fragments.concat(),
            prompt,
            "C8 {cell}: fragmented bytes changed"
        );
        assert!(
            actual_fragments
                .iter()
                .all(|fragment| fragment.len() <= SOURCE_FRAGMENT_BYTES),
            "C8 {cell}: fragment exceeded byte boundary"
        );
    }
}

fn c8_disposition_matrix() {
    let cases = [
        ("disposition.completed", SourceTurnDisposition::Completed),
        (
            "disposition.interrupted",
            SourceTurnDisposition::Interrupted,
        ),
        ("disposition.failed", SourceTurnDisposition::Failed),
        ("disposition.abandoned", SourceTurnDisposition::Abandoned),
        (
            "disposition.capture_overflow",
            SourceTurnDisposition::CaptureOverflow,
        ),
    ];
    for (cell, disposition) in cases {
        let (tx, mut rx) = mpsc::channel(8);
        let observer = SourceObserver::new(tx);
        observer
            .begin(
                SessionId::new("session-disposition"),
                TurnId::new(3),
                &["prompt".to_owned()],
            )
            .unwrap_or_else(|error| panic!("C8 {cell}: source id generation failed: {error}"));
        observer.finish(disposition);
        let events = drain(&mut rx);
        let actual = events.iter().find_map(|event| match event.kind() {
            SourceTurnEventKind::Finished { disposition, .. } => Some(*disposition),
            _ => None,
        });
        assert_eq!(actual, Some(disposition), "C8 {cell}: terminal disposition");
    }
}

fn c8_identity_matrix() {
    let cases = [
        (
            "identity.session_a_turn_7_first",
            SessionId::new("session-a"),
            TurnId::new(7),
            "first",
        ),
        (
            "identity.session_a_turn_7_reuse",
            SessionId::new("session-a"),
            TurnId::new(7),
            "reuse",
        ),
        (
            "identity.session_b_turn_7_change_session",
            SessionId::new("session-b"),
            TurnId::new(7),
            "session-change",
        ),
        (
            "identity.session_b_turn_8_change_turn",
            SessionId::new("session-b"),
            TurnId::new(8),
            "turn-change",
        ),
    ];
    let (tx, mut rx) = mpsc::channel(16);
    let observer = SourceObserver::new(tx);
    let mut source_ids = Vec::new();

    for (cell, session, turn, label) in cases {
        let source_turn_id = observer
            .begin(session.clone(), turn, &[label.to_owned()])
            .unwrap_or_else(|error| panic!("C8 {cell}: source id generation failed: {error}"));
        observer.finish(SourceTurnDisposition::Completed);
        let events = drain(&mut rx);
        let started = events
            .first()
            .unwrap_or_else(|| panic!("C8 {cell}: missing started event"));
        assert_eq!(
            started.session_id(),
            &session,
            "C8 {cell}: session identity"
        );
        assert_eq!(
            started.source_turn_id(),
            source_turn_id,
            "C8 {cell}: source identity"
        );
        assert_eq!(started.sequence(), 0, "C8 {cell}: sequence reset");
        assert!(
            matches!(started.kind(), SourceTurnEventKind::Started { bridge_turn_id, .. } if *bridge_turn_id == turn.get()),
            "C8 {cell}: bridge turn identity"
        );
        let prompt = events
            .get(1)
            .unwrap_or_else(|| panic!("C8 {cell}: missing prompt event"));
        assert!(
            matches!(prompt.kind(), SourceTurnEventKind::PromptFragment { text, .. } if text == label),
            "C8 {cell}: prompt identity ledger"
        );
        source_ids.push(source_turn_id);
    }

    for (index, source_id) in source_ids.iter().enumerate() {
        assert_eq!(
            source_ids
                .iter()
                .filter(|candidate| *candidate == source_id)
                .count(),
            1,
            "C8 identity.row_{index}: source turn id reused"
        );
    }
}

fn c8_tool_field_boundaries() {
    #[derive(Clone, Copy)]
    enum Field {
        Id,
        Name,
        Status,
        Input,
        Result,
    }

    let cases = [
        ("tool.id.exact", Field::Id, EXPECTED_TOOL_ID_BYTES),
        ("tool.id.plus_one", Field::Id, EXPECTED_TOOL_ID_BYTES + 1),
        ("tool.name.exact", Field::Name, EXPECTED_TOOL_NAME_BYTES),
        (
            "tool.name.plus_one",
            Field::Name,
            EXPECTED_TOOL_NAME_BYTES + 1,
        ),
        (
            "tool.status.exact",
            Field::Status,
            EXPECTED_TOOL_STATUS_BYTES,
        ),
        (
            "tool.status.plus_one",
            Field::Status,
            EXPECTED_TOOL_STATUS_BYTES + 1,
        ),
        ("tool.input.exact", Field::Input, EXPECTED_TOOL_INPUT_BYTES),
        (
            "tool.input.plus_one",
            Field::Input,
            EXPECTED_TOOL_INPUT_BYTES + 1,
        ),
        (
            "tool.result.exact",
            Field::Result,
            EXPECTED_TOOL_RESULT_BYTES,
        ),
        (
            "tool.result.plus_one",
            Field::Result,
            EXPECTED_TOOL_RESULT_BYTES + 1,
        ),
    ];

    for (cell, field, requested_bytes) in cases {
        let id_value = if matches!(field, Field::Id) {
            "i".repeat(requested_bytes)
        } else {
            "tool-id".to_owned()
        };
        let name_value = if matches!(field, Field::Name) {
            "n".repeat(requested_bytes)
        } else {
            "tool-name".to_owned()
        };
        let status_value = if matches!(field, Field::Status) {
            "s".repeat(requested_bytes)
        } else {
            "status".to_owned()
        };
        let input_payload = if matches!(field, Field::Input) {
            "p".repeat(requested_bytes - 2)
        } else {
            String::new()
        };
        let result_value = if matches!(field, Field::Result) {
            "r".repeat(requested_bytes)
        } else {
            String::new()
        };
        let expected_kept = match field {
            Field::Id => "i".repeat(EXPECTED_TOOL_ID_BYTES.min(requested_bytes)),
            Field::Name => "n".repeat(EXPECTED_TOOL_NAME_BYTES.min(requested_bytes)),
            Field::Status => "s".repeat(EXPECTED_TOOL_STATUS_BYTES.min(requested_bytes)),
            Field::Input => {
                let serialized = format!("\"{}\"", "p".repeat(requested_bytes - 2));
                serialized[..EXPECTED_TOOL_INPUT_BYTES.min(serialized.len())].to_owned()
            }
            Field::Result => "r".repeat(EXPECTED_TOOL_RESULT_BYTES.min(requested_bytes)),
        };
        let expected_dropped = requested_bytes.saturating_sub(match field {
            Field::Id => EXPECTED_TOOL_ID_BYTES,
            Field::Name => EXPECTED_TOOL_NAME_BYTES,
            Field::Status => EXPECTED_TOOL_STATUS_BYTES,
            Field::Input => EXPECTED_TOOL_INPUT_BYTES,
            Field::Result => EXPECTED_TOOL_RESULT_BYTES,
        });

        let (tx, mut rx) = mpsc::channel(8);
        let observer = SourceObserver::new(tx);
        let session = SessionId::new("session-tool");
        observer
            .begin(session.clone(), TurnId::new(4), &["prompt".to_owned()])
            .unwrap_or_else(|error| panic!("C8 {cell}: source id generation failed: {error}"));
        let tool_id = ToolCallId::new(id_value.clone());
        let notification = if matches!(field, Field::Status) {
            Notification::ToolCallChunk {
                tool_call_id: tool_id,
                title: name_value.clone(),
                kind: status_value.to_owned(),
                session_id: None,
            }
        } else {
            let raw_input = if matches!(field, Field::Input) {
                Some(serde_json::Value::String(input_payload))
            } else {
                None
            };
            let content = if matches!(field, Field::Result) {
                vec![ToolCallContent::Text(result_value)]
            } else {
                Vec::new()
            };
            Notification::ToolCallStarted(
                ToolCall::new(
                    tool_id,
                    name_value,
                    ToolKind::Read,
                    ToolCallStatus::Completed,
                    raw_input,
                )
                .with_content(content),
            )
        };
        observer.observe(&RoutedNotification::scoped(session, notification));
        let events = drain(&mut rx);
        let snapshot = events.iter().find_map(|event| match event.kind() {
            SourceTurnEventKind::ToolSnapshot {
                tool_id,
                name,
                status,
                input,
                result,
                source_truncated_chars,
                ..
            } => Some((tool_id, name, status, input, result, source_truncated_chars)),
            _ => None,
        });
        let Some((actual_id, actual_name, actual_status, actual_input, actual_result, dropped)) =
            snapshot
        else {
            panic!("C8 {cell}: missing tool snapshot");
        };
        let actual = match field {
            Field::Id => actual_id,
            Field::Name => actual_name,
            Field::Status => actual_status,
            Field::Input => actual_input,
            Field::Result => actual_result,
        };
        assert_eq!(actual, &expected_kept, "C8 {cell}: bounded field bytes");
        assert_eq!(*dropped, expected_dropped, "C8 {cell}: truncation metadata");
        assert!(
            actual.is_char_boundary(actual.len()),
            "C8 {cell}: field split UTF-8"
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn c9_bounded_pressure_contract() {
    tokio::task::LocalSet::new()
        .run_until(async {
            c9_source_channel_full_overflow().await;
            c9_source_channel_capacity_boundary().await;
        })
        .await;
}

async fn c9_source_channel_full_overflow() {
    let cell = "source.full.disposition";
    let (tx, mut rx) = mpsc::channel(1);
    let observer = SourceObserver::new(tx);
    observer
        .begin(
            SessionId::new("session-pressure"),
            TurnId::new(5),
            &["prompt".to_owned()],
        )
        .unwrap_or_else(|error| panic!("C9 {cell}: source id generation failed: {error}"));
    observer.finish(SourceTurnDisposition::Completed);

    let started = rx
        .recv()
        .await
        .unwrap_or_else(|| panic!("C9 {cell}: missing started event"));
    assert!(
        matches!(started.kind(), SourceTurnEventKind::Started { .. }),
        "C9 {cell}: first queued event changed"
    );
    let terminal = rx
        .recv()
        .await
        .unwrap_or_else(|| panic!("C9 {cell}: missing overflow terminal"));
    assert!(
        matches!(
            terminal.kind(),
            SourceTurnEventKind::Finished {
                disposition: SourceTurnDisposition::CaptureOverflow,
                ..
            }
        ),
        "C9 {cell}: full source channel did not type overflow"
    );
    assert_eq!(terminal.sequence(), 1, "C9 {cell}: overflow sequence");
    assert!(
        rx.try_recv().is_err(),
        "C9 {cell}: duplicate terminal event"
    );
}

async fn c9_source_channel_capacity_boundary() {
    let cell = "source.capacity_32.monotonic_pressure";
    let (tx, mut rx) = mpsc::channel(SOURCE_EVENT_CHANNEL_CAPACITY);
    let observer = SourceObserver::new(tx);
    let session = SessionId::new("session-pressure-boundary");
    observer
        .begin(session.clone(), TurnId::new(6), &["prompt".to_owned()])
        .unwrap_or_else(|error| panic!("C9 {cell}: source id generation failed: {error}"));

    for index in 0..(SOURCE_EVENT_CHANNEL_CAPACITY * 4) {
        observer.observe(&RoutedNotification::scoped(
            session.clone(),
            Notification::ToolCallChunk {
                tool_call_id: ToolCallId::new(format!("pressure-{index}")),
                title: "read".to_owned(),
                kind: "read".to_owned(),
                session_id: None,
            },
        ));
    }
    observer.finish(SourceTurnDisposition::Completed);

    let mut events = Vec::with_capacity(SOURCE_EVENT_CHANNEL_CAPACITY);
    for index in 0..SOURCE_EVENT_CHANNEL_CAPACITY {
        events.push(
            rx.recv()
                .await
                .unwrap_or_else(|| panic!("C9 {cell}: missing queued event {index}")),
        );
    }
    for (expected, event) in events.iter().enumerate() {
        assert_eq!(
            event.sequence(),
            expected as u64,
            "C9 {cell}: sequence at queue index {expected}"
        );
    }
    let terminal = rx
        .recv()
        .await
        .unwrap_or_else(|| panic!("C9 {cell}: missing bounded terminal"));
    assert_eq!(
        terminal.sequence(),
        SOURCE_EVENT_CHANNEL_CAPACITY as u64,
        "C9 {cell}: terminal sequence"
    );
    assert!(
        matches!(
            terminal.kind(),
            SourceTurnEventKind::Finished {
                disposition: SourceTurnDisposition::CaptureOverflow,
                ..
            }
        ),
        "C9 {cell}: bounded pressure disposition"
    );
    assert!(
        rx.try_recv().is_err(),
        "C9 {cell}: pressure emitted an extra event"
    );
}
