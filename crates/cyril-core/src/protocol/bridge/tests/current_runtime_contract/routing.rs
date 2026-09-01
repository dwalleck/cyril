use super::*;
use crate::types::SessionId;

fn message(text: &str) -> Notification {
    Notification::AgentMessage(crate::types::AgentMessage {
        text: text.to_owned(),
        is_streaming: true,
    })
}

fn assert_route(
    cell: &str,
    routed: &RoutedNotification,
    expected_session: Option<&str>,
    expected_turn: Option<u64>,
    expected_text: &str,
) {
    assert_eq!(
        routed
            .session_id
            .as_ref()
            .map(crate::types::SessionId::as_str),
        expected_session,
        "C6 {cell}: session route"
    );
    assert_eq!(
        routed.turn.map(crate::types::TurnId::get),
        expected_turn,
        "C6 {cell}: turn route"
    );
    assert!(
        matches!(&routed.notification, Notification::AgentMessage(message) if message.text == expected_text && message.is_streaming),
        "C6 {cell}: typed payload changed: {:?}",
        routed.notification
    );
}

#[tokio::test]
async fn c6_global_main_and_subagent_envelopes_preserve_identity_and_order() {
    let script = Rc::new(RefCell::new(Script::default()));
    let injection = Rc::clone(&script);
    with_harness(
        script,
        move |sender, mut rx, _permission_rx, _gate, loop_handle| async move {
            let main_session_id = start_session(&sender, &mut rx).await;
            let inbound = injection
                .borrow()
                .inbound
                .clone()
                .expect_contract("C6 harness exposes exact inbound sender");
            let fixtures = [
                RoutedNotification::global(message("global")).with_turn(TurnId::new(10)),
                RoutedNotification::scoped(main_session_id, message("main"))
                    .with_turn(TurnId::new(11)),
                RoutedNotification::scoped(SessionId::new("child-7"), message("subagent"))
                    .with_turn(TurnId::new(12)),
            ];
            for fixture in fixtures {
                inbound
                    .send(fixture)
                    .await
                    .expect_contract("C6 inject routed notification");
            }

            let global = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                .await
                .expect_contract("C6 global route timeout")
                .expect_contract("C6 global route channel");
            let main = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                .await
                .expect_contract("C6 main route timeout")
                .expect_contract("C6 main route channel");
            let subagent = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                .await
                .expect_contract("C6 subagent route timeout")
                .expect_contract("C6 subagent route channel");
            assert_route("global", &global, None, Some(10), "global");
            assert_route("main", &main, Some("fake-0"), Some(11), "main");
            assert_route("subagent", &subagent, Some("child-7"), Some(12), "subagent");

            sender
                .send(BridgeCommand::Shutdown)
                .await
                .expect_contract("C6 routing shutdown send");
            loop_handle
                .await
                .expect_contract("C6 routing loop joined")
                .expect_contract("C6 routing loop result");
        },
    )
    .await;
}

#[tokio::test]
async fn c6_agent_eof_emits_one_typed_disconnect_then_closes_commands() {
    let script = Rc::new(RefCell::new(Script::default()));
    with_engine_harness(
        Rc::new(V2Engine),
        script,
        |sender, mut rx, _permission_rx, _gate, loop_handle, kill| async move {
            let _session_id = start_session(&sender, &mut rx).await;
            kill.kill();
            let disconnected = next_notification("agent_eof", &mut rx).await;
            assert!(
                matches!(disconnected, Notification::BridgeDisconnected { ref reason } if reason == "agent connection closed unexpectedly"),
                "C6 agent_eof: {disconnected:?}"
            );
            loop_handle
                .await.expect_contract("C6 agent_eof loop joined").expect_contract("C6 agent_eof loop result");
            let error = sender
                .send(BridgeCommand::ListSettings)
                .await.expect_err_contract("C6 command channel closes after agent EOF");
            assert_eq!(
                error.to_string(),
                "bridge channel closed",
                "C6 agent_eof command error"
            );
        },
    )
    .await;
}

#[tokio::test]
async fn c6_notification_receiver_disconnect_stops_loop_and_closes_commands() {
    let script = Rc::new(RefCell::new(Script::default()));
    with_harness(
        script,
        |sender, rx, _permission_rx, _gate, loop_handle| async move {
            drop(rx);
            sender
                .send(BridgeCommand::NewSession {
                    cwd: std::env::temp_dir(),
                })
                .await
                .expect_contract("C6 notification disconnect accepts triggering command");
            loop_handle
                .await
                .expect_contract("C6 notification disconnect loop joined")
                .expect_contract("C6 notification disconnect loop result");
            let error = sender
                .send(BridgeCommand::ListSettings)
                .await
                .expect_err_contract("C6 command sender closes after output receiver");
            assert_eq!(
                error.to_string(),
                "bridge channel closed",
                "C6 notification disconnect command error"
            );
        },
    )
    .await;
}

#[tokio::test]
async fn c6_unknown_update_handler_precedes_typed_handler_without_poisoning_connection() {
    let script = Rc::new(RefCell::new(Script {
        emit_chunks: 1,
        emit_unknown_update: true,
        ..Script::default()
    }));
    with_harness(
        script,
        |sender, mut rx, _permission_rx, _gate, loop_handle| async move {
            let session_id = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id,
                    prompt: crate::types::PromptEnvelope::prepared(
                        vec!["future update fence".to_owned()],
                        None,
                    ),
                })
                .await
                .unwrap_or_else(|error| panic!("unknown-update prompt send failed: {error}"));
            let message = recv_notif(&mut rx, 5)
                .await
                .unwrap_or_else(|| panic!("known update after unknown update was lost"));
            assert!(
                matches!(&message, Notification::AgentMessage(message) if message.text == "c0"),
                "known typed update must follow the retained unknown update: {message:?}"
            );
            let completed = recv_notif(&mut rx, 5)
                .await
                .unwrap_or_else(|| panic!("turn completion after unknown update was lost"));
            assert!(
                matches!(
                    &completed,
                    Notification::TurnCompleted {
                        stop_reason: StopReason::EndTurn
                    }
                ),
                "unknown update must not poison the SDK connection: {completed:?}"
            );
            sender
                .send(BridgeCommand::Shutdown)
                .await
                .unwrap_or_else(|error| panic!("unknown-update shutdown failed: {error}"));
            loop_handle
                .await
                .unwrap_or_else(|error| panic!("unknown-update loop join failed: {error}"))
                .unwrap_or_else(|error| panic!("unknown-update loop failed: {error}"));
        },
    )
    .await;
}

#[tokio::test]
async fn c8_host_request_drain_is_live_during_initialize() {
    let script = Rc::new(RefCell::new(Script {
        request_extension_during_initialize: true,
        ..Script::default()
    }));
    let observed = Rc::clone(&script);
    let during = Rc::clone(&script);
    with_harness(
        script,
        move |sender, mut rx, _permission_rx, _gate, loop_handle| async move {
            sender
                .send(BridgeCommand::NewSession {
                    cwd: std::env::temp_dir(),
                })
                .await
                .expect_contract("C8 new-session command accepted");
            let first = recv_notif(&mut rx, 5).await;
            assert!(
                matches!(first, Some(Notification::UsageSessionStarted { .. })),
                "C8 initialize callback failed before session startup; received={first:?}; ledger={:?}",
                during.borrow().received().as_slice()
            );
            let created = recv_notif(&mut rx, 5)
                .await
                .expect_contract("C8 session-created notification");
            assert!(matches!(created, Notification::SessionCreated { .. }));
            sender
                .send(BridgeCommand::Shutdown)
                .await
                .expect_contract("C8 shutdown command accepted");
            loop_handle
                .await
                .expect_contract("C8 loop task joined")
                .expect_contract("C8 loop result");
        },
    )
    .await;
    assert_eq!(
        observed.borrow().received().as_slice(),
        [
            "initialize_callback_started",
            "initialize_callback",
            "new_session"
        ],
        "C8 initialize callback completed before session startup"
    );
}

#[tokio::test]
async fn c7_malformed_standard_request_is_rejected_before_extension_fallback() {
    let script = Rc::new(RefCell::new(Script {
        request_malformed_standard_during_initialize: true,
        ..Script::default()
    }));
    let observed = Rc::clone(&script);
    with_harness(
        script,
        |sender, mut rx, _permission_rx, _gate, loop_handle| async move {
            let _session_id = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::Shutdown)
                .await
                .expect_contract("C7 shutdown command accepted");
            loop_handle
                .await
                .expect_contract("C7 loop task joined")
                .expect_contract("C7 loop result");
        },
    )
    .await;
    assert_eq!(
        observed.borrow().received().as_slice(),
        [
            "malformed_standard_started",
            "malformed_standard_rejected",
            "new_session",
        ],
        "C7 malformed standard request cannot receive an extension null response"
    );
}

#[tokio::test]
async fn c7_unknown_standard_request_returns_method_not_found() {
    let script = Rc::new(RefCell::new(Script {
        request_unknown_standard_during_initialize: true,
        ..Script::default()
    }));
    let observed = Rc::clone(&script);
    with_harness(
        script,
        |sender, mut rx, _permission_rx, _gate, loop_handle| async move {
            let _session_id = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::Shutdown)
                .await
                .expect_contract("C7 unknown-standard shutdown accepted");
            loop_handle
                .await
                .expect_contract("C7 unknown-standard loop joined")
                .expect_contract("C7 unknown-standard loop result");
        },
    )
    .await;
    assert_eq!(
        observed.borrow().received().as_slice(),
        [
            "unknown_standard_started",
            "unknown_standard_rejected",
            "new_session",
        ],
        "C7 unknown standard requests retain method-not-found semantics"
    );
}

#[tokio::test]
async fn c11_sdk_runtime_negotiates_stable_wire_v1() {
    let script = Rc::new(RefCell::new(Script::default()));
    let observed = Rc::clone(&script);
    with_harness(
        script,
        |sender, mut rx, _permission_rx, _gate, loop_handle| async move {
            let _session_id = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::Shutdown)
                .await
                .expect_contract("C11 shutdown command accepted");
            loop_handle
                .await
                .expect_contract("C11 loop task joined")
                .expect_contract("C11 loop result");
        },
    )
    .await;
    assert_eq!(
        observed.borrow().negotiated_protocol(),
        Some(agent_client_protocol::schema::ProtocolVersion::V1),
        "C11 production runtime must negotiate stable ACP wire v1"
    );
}
