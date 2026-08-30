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
            let inbound = injection
                .borrow()
                .inbound
                .clone()
                .expect("C6 harness exposes exact inbound sender");
            let fixtures = [
                RoutedNotification::global(message("global")).with_turn(TurnId::new(10)),
                RoutedNotification::scoped(SessionId::new("fake-main"), message("main"))
                    .with_turn(TurnId::new(11)),
                RoutedNotification::scoped(SessionId::new("child-7"), message("subagent"))
                    .with_turn(TurnId::new(12)),
            ];
            for fixture in fixtures {
                inbound
                    .send(fixture)
                    .await
                    .expect("C6 inject routed notification");
            }

            let global = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                .await
                .expect("C6 global route timeout")
                .expect("C6 global route channel");
            let main = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                .await
                .expect("C6 main route timeout")
                .expect("C6 main route channel");
            let subagent = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                .await
                .expect("C6 subagent route timeout")
                .expect("C6 subagent route channel");
            assert_route("global", &global, None, Some(10), "global");
            assert_route("main", &main, Some("fake-main"), Some(11), "main");
            assert_route("subagent", &subagent, Some("child-7"), Some(12), "subagent");

            sender
                .send(BridgeCommand::Shutdown)
                .await
                .expect("C6 routing shutdown send");
            loop_handle
                .await
                .expect("C6 routing loop joined")
                .expect("C6 routing loop result");
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
                .await
                .expect("C6 agent_eof loop joined")
                .expect("C6 agent_eof loop result");
            let error = sender
                .send(BridgeCommand::ListSettings)
                .await
                .expect_err("C6 command channel closes after agent EOF");
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
                .expect("C6 notification disconnect accepts triggering command");
            loop_handle
                .await
                .expect("C6 notification disconnect loop joined")
                .expect("C6 notification disconnect loop result");
            let error = sender
                .send(BridgeCommand::ListSettings)
                .await
                .expect_err("C6 command sender closes after output receiver");
            assert_eq!(
                error.to_string(),
                "bridge channel closed",
                "C6 notification disconnect command error"
            );
        },
    )
    .await;
}
