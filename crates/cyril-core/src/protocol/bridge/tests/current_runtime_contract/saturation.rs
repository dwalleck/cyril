use super::*;
use crate::types::{SessionId, hook::HookId};

const EXPECTED_COMMAND_CAPACITY: usize = 32;
const EXPECTED_NOTIFICATION_CAPACITY: usize = 256;
const EXPECTED_PERMISSION_CAPACITY: usize = 16;

fn all_commands() -> Vec<BridgeCommand> {
    let session_id = SessionId::new("oracle-session");
    vec![
        BridgeCommand::SendPrompt {
            session_id: session_id.clone(),
            prompt: crate::types::PromptEnvelope::prepared(vec!["prompt".to_owned()], None),
        },
        BridgeCommand::NewSession {
            cwd: std::path::PathBuf::from("/oracle"),
        },
        BridgeCommand::LoadSession {
            session_id: session_id.clone(),
        },
        BridgeCommand::CancelRequest,
        BridgeCommand::SetMode {
            mode_id: "mode".to_owned(),
        },
        BridgeCommand::SetModel {
            model_id: "model".to_owned(),
        },
        BridgeCommand::ExtMethod {
            method: "oracle/ext".to_owned(),
            params: serde_json::json!({}),
        },
        BridgeCommand::ListSettings,
        BridgeCommand::QueryUsageAccount,
        BridgeCommand::QueryCommandOptions {
            command: "model".to_owned(),
            session_id: session_id.clone(),
        },
        BridgeCommand::ExecuteCommand {
            command: "tools".to_owned(),
            session_id: session_id.clone(),
            args: serde_json::json!({}),
        },
        BridgeCommand::SpawnSession {
            task: "task".to_owned(),
            name: "name".to_owned(),
        },
        BridgeCommand::TerminateSession {
            session_id: session_id.clone(),
        },
        BridgeCommand::SendMessage {
            session_id: session_id.clone(),
            content: "content".to_owned(),
        },
        BridgeCommand::SteerSession {
            session_id: session_id.clone(),
            message: "steer".to_owned(),
        },
        BridgeCommand::ClearSteering {
            session_id: session_id.clone(),
        },
        BridgeCommand::ListKasHooks {
            session_id: session_id.clone(),
            workspace_paths: vec![std::path::PathBuf::from("/oracle")],
        },
        BridgeCommand::Workflow {
            session_id: session_id.clone(),
            workspace_paths: vec![std::path::PathBuf::from("/oracle")],
            op: crate::types::WorkflowOp::ListRecipes,
        },
        BridgeCommand::SetKasHookEnabled {
            session_id,
            hook_id: HookId::new("hook.json#hook-0"),
            enabled: true,
            workspace_paths: vec![std::path::PathBuf::from("/oracle")],
        },
        BridgeCommand::Shutdown,
    ]
}

fn permission(index: usize) -> PermissionRequest {
    let (responder, _receiver) = tokio::sync::oneshot::channel();
    PermissionRequest {
        session_id: SessionId::new(format!("permission-{index}")),
        tool_call: crate::types::ToolCall::new(
            crate::types::ToolCallId::new(format!("tool-{index}")),
            format!("tool {index}"),
            crate::types::ToolKind::Execute,
            crate::types::ToolCallStatus::Pending,
            None,
        ),
        message: format!("allow {index}"),
        options: Vec::new(),
        trust_options: Vec::new(),
        responder,
    }
}

fn routed_message(index: usize) -> RoutedNotification {
    RoutedNotification::global(Notification::AgentMessage(crate::types::AgentMessage {
        text: index.to_string(),
        is_streaming: true,
    }))
}

/// C6's named mutation fence (design falsification table): make `run_loop`'s
/// inbound→App pump forward lossily (`try_send` instead of the fail-stop
/// bounded `send`) and this test goes red.
///
/// Saturation must be REAL, not assumed: frames buffer in the notification
/// channel (256), the pump's held slot (1), the inbound channel (256), and
/// the harness's 64KB duplex (~410 chunk frames), so a flood must exceed
/// their SUM (~930) before any bounded send anywhere can park. With the
/// App-side receiver deliberately undrained, a flood past that sum wedges the
/// cascade — and the only way the pump can be part of that wedge is parked on
/// the FULL App channel, the one state where `try_send` and `send` differ.
#[tokio::test]
async fn c6_run_loop_forwarding_preserves_every_frame_through_a_full_channel() {
    const FLOOD: usize = 4 * EXPECTED_NOTIFICATION_CAPACITY;

    let script = Rc::new(RefCell::new(Script {
        emit_chunks: FLOOD,
        ..Script::default()
    }));
    let observed = Rc::clone(&script);
    with_harness(
        script,
        move |sender, mut rx, _permission_rx, _gate, loop_handle| async move {
            let session_id = start_session(&sender, &mut rx).await;

            sender
                .send(BridgeCommand::SendPrompt {
                    session_id,
                    prompt: crate::types::PromptEnvelope::prepared(vec!["flood".to_owned()], None),
                })
                .await
                .expect("C6 flood SendPrompt send");

            // Wait for the wedge: inbound pinned at zero remaining capacity
            // means the pump has stopped pulling — parked on the full App
            // channel — while the fake still has frames to push. A lossy pump
            // never wedges (it drains inbound by dropping), so the mutation
            // fails here on the timeout, or below in the reconciliation.
            let inbound = observed
                .borrow()
                .inbound
                .clone()
                .expect("C6 harness wires the inbound seam");
            tokio::time::timeout(Duration::from_secs(5), async {
                while inbound.capacity() > 0 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("C6 flood: bridge wedged on the full App channel");

            for index in 0..FLOOD {
                let frame = recv_notif(&mut rx, 5)
                    .await
                    .unwrap_or_else(|| panic!("C6 flood frame {index} lost"));
                assert!(
                    matches!(frame, Notification::AgentMessage(ref message) if message.text == format!("c{index}")),
                    "C6 flood frame {index} out of order or rewritten: {frame:?}"
                );
            }
            let completed = recv_notif(&mut rx, 5).await;
            assert!(
                matches!(
                    completed,
                    Some(Notification::TurnCompleted {
                        stop_reason: StopReason::EndTurn
                    })
                ),
                "C6 flood terminal after every frame: {completed:?}"
            );

            sender
                .send(BridgeCommand::Shutdown)
                .await
                .expect("C6 flood Shutdown send");
            loop_handle
                .await
                .expect("C6 flood loop joined")
                .expect("C6 flood loop Ok");
            assert!(
                rx.recv().await.is_none(),
                "C6 flood: duplicated or trailing frame after reconciliation"
            );
        },
    )
    .await;
}

#[tokio::test]
async fn c6_command_channel_capacity_fifo_and_closed_errors_are_exact() {
    let (handle, mut command_rx) = BridgeHandle::for_tests_with_command_rx();
    let sender = handle.sender();
    let commands = all_commands();
    assert_eq!(commands.len(), 20, "C6 exhaustive command fixture count");
    let expected: Vec<_> = commands.iter().map(command_name).collect();
    for command in commands {
        sender
            .try_send(command)
            .expect("C6 first 20 exhaustive commands fit");
    }
    for _ in expected.len()..EXPECTED_COMMAND_CAPACITY {
        sender
            .try_send(BridgeCommand::ListSettings)
            .expect("C6 command capacity accepts exactly 32");
    }
    let full = sender
        .try_send(BridgeCommand::ListSettings)
        .expect_err("C6 command 33 must report full");
    assert_eq!(full.to_string(), "bridge channel closed", "C6 command full");

    let mut drained = Vec::new();
    for _ in 0..EXPECTED_COMMAND_CAPACITY {
        let command = command_rx
            .recv()
            .await
            .expect("C6 drain full command queue");
        drained.push(command_name(&command));
    }
    assert_eq!(
        &drained[..expected.len()],
        expected,
        "C6 exhaustive command FIFO"
    );
    assert!(
        drained[expected.len()..]
            .iter()
            .all(|name| *name == "ListSettings"),
        "C6 command filler FIFO"
    );

    sender
        .try_send(BridgeCommand::ListSettings)
        .expect("C6 capacity is released by one receive");
    assert_eq!(
        command_name(&command_rx.recv().await.expect("C6 released slot payload")),
        "ListSettings"
    );
    drop(command_rx);
    let closed = sender
        .try_send(BridgeCommand::Shutdown)
        .expect_err("C6 dropped command receiver reports closed");
    assert_eq!(
        closed.to_string(),
        "bridge channel closed",
        "C6 command closed"
    );
}

#[tokio::test]
async fn c6_notification_and_permission_channels_bound_preserve_and_close() {
    let (handle, channels) = create_channel_pair();
    let (_sender, mut notification_rx, mut permission_rx, _source_rx, _completion_rx) =
        handle.split();

    for index in 0..EXPECTED_NOTIFICATION_CAPACITY {
        channels
            .notification_tx
            .try_send(routed_message(index))
            .expect("C6 notification within capacity");
    }
    match channels
        .notification_tx
        .try_send(routed_message(EXPECTED_NOTIFICATION_CAPACITY))
    {
        Err(mpsc::error::TrySendError::Full(routed)) => assert!(
            matches!(routed.notification, Notification::AgentMessage(message) if message.text == EXPECTED_NOTIFICATION_CAPACITY.to_string()),
            "C6 notification full returns exact unsent payload"
        ),
        other => panic!("C6 notification capacity must be exact: {other:?}"),
    }
    for index in 0..EXPECTED_NOTIFICATION_CAPACITY {
        let routed = notification_rx
            .recv()
            .await
            .expect("C6 notification FIFO drain");
        assert!(
            matches!(routed.notification, Notification::AgentMessage(message) if message.text == index.to_string()),
            "C6 notification FIFO index {index}"
        );
    }
    drop(notification_rx);
    assert!(
        matches!(
            channels.notification_tx.try_send(routed_message(0)),
            Err(mpsc::error::TrySendError::Closed(_))
        ),
        "C6 notification closed is typed"
    );

    for index in 0..EXPECTED_PERMISSION_CAPACITY {
        channels
            .permission_tx
            .try_send(permission(index))
            .expect("C6 permission within capacity");
    }
    match channels
        .permission_tx
        .try_send(permission(EXPECTED_PERMISSION_CAPACITY))
    {
        Err(mpsc::error::TrySendError::Full(request)) => assert_eq!(
            request.session_id.as_str(),
            format!("permission-{EXPECTED_PERMISSION_CAPACITY}"),
            "C6 permission full returns exact unsent payload"
        ),
        other => panic!("C6 permission capacity must be exact: {other:?}"),
    }
    for index in 0..EXPECTED_PERMISSION_CAPACITY {
        let request = permission_rx
            .recv()
            .await
            .expect("C6 permission FIFO drain");
        assert_eq!(
            request.session_id.as_str(),
            format!("permission-{index}"),
            "C6 permission FIFO"
        );
    }
    drop(permission_rx);
    assert!(
        matches!(
            channels.permission_tx.try_send(permission(0)),
            Err(mpsc::error::TrySendError::Closed(_))
        ),
        "C6 permission closed is typed"
    );
}
