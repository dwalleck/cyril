use super::*;
use cyril_ui::traits::TuiState;

#[test]
fn c4_app_applies_session_before_ui_and_preserves_terminal_sequence() {
    let main = SessionId::new("app-order-main");
    let mut app = test_app();
    establish_main_session(&mut app, &main);
    let message_baseline = app.ui_state.messages().len();
    app.apply_order.clear();
    app.session.set_status(SessionStatus::Busy);

    app.handle_notification(RoutedNotification::scoped(
        main.clone(),
        Notification::AgentMessage(cyril_core::types::AgentMessage {
            text: "partial answer".to_owned(),
            is_streaming: true,
        }),
    ));
    assert_eq!(
        TuiState::streaming_text(&app.ui_state),
        "partial answer",
        "C4 event: streaming payload reaches UI before any terminal"
    );
    assert!(
        matches!(app.session.status(), SessionStatus::Busy),
        "C4 event: nonterminal traffic keeps session busy"
    );

    app.handle_notification(RoutedNotification::global(Notification::BridgeError {
        operation: "prompt".to_owned(),
        message: "transport failed".to_owned(),
    }));
    assert!(
        matches!(app.session.status(), SessionStatus::Busy),
        "C4 error: advisory error does not synthesize completion"
    );

    app.handle_notification(RoutedNotification::scoped(
        main,
        Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        },
    ));
    assert!(
        matches!(app.session.status(), SessionStatus::Active),
        "C4 completion: terminal releases busy state"
    );
    assert_eq!(
        TuiState::streaming_text(&app.ui_state),
        "",
        "C4 completion: terminal commits streaming text"
    );

    app.handle_notification(RoutedNotification::global(
        Notification::BridgeDisconnected {
            reason: "agent connection closed unexpectedly".to_owned(),
        },
    ));
    assert!(
        matches!(app.session.status(), SessionStatus::Disconnected),
        "C4 disconnect: connection state becomes disconnected"
    );

    assert_eq!(
        app.apply_order,
        [
            "session", "ui", // event
            "session", "ui", // error
            "session", "ui", // completion
            "session", "ui", // disconnect
        ],
        "C4 each typed frame applies SessionController before UiState"
    );
    let projected: Vec<_> = app.ui_state.messages()[message_baseline..]
        .iter()
        .filter_map(|message| match message.kind() {
            ChatMessageKind::System(text) => Some(("system", text.as_str())),
            ChatMessageKind::AgentText(text) => Some(("agent", text.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(
        projected,
        [
            ("system", "prompt failed: transport failed"),
            ("agent", "partial answer"),
            (
                "system",
                "Disconnected: agent connection closed unexpectedly"
            ),
        ],
        "C4 exact error → completion flush → disconnect projection order"
    );
}
