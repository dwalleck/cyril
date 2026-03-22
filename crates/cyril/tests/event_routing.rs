#![allow(clippy::unwrap_used, clippy::expect_used)]

use cyril_core::session::SessionController;
use cyril_core::types::*;
use cyril_ui::state::UiState;
use cyril_ui::traits::{Activity, ChatMessageKind, TuiState};

#[test]
fn agent_message_streams_to_ui_state() {
    let mut ui = UiState::new(500);
    let notification = Notification::AgentMessage(AgentMessage {
        text: "Hello world".into(),
        is_streaming: true,
    });

    let changed = ui.apply_notification(&notification);

    assert!(changed);
    assert!(ui.streaming_text().contains("Hello"));
    assert_eq!(ui.activity(), Activity::Streaming);
}

#[test]
fn streaming_chunks_append_not_replace() {
    let mut ui = UiState::new(500);

    ui.apply_notification(&Notification::AgentMessage(AgentMessage {
        text: "Hello ".into(),
        is_streaming: true,
    }));
    ui.apply_notification(&Notification::AgentMessage(AgentMessage {
        text: "world".into(),
        is_streaming: true,
    }));

    assert_eq!(ui.streaming_text(), "Hello world");
}

#[test]
fn turn_completed_commits_streaming_and_updates_session() {
    let mut ui = UiState::new(500);
    let mut session = SessionController::new();
    session.set_status(SessionStatus::Busy);

    // Stream some text
    ui.apply_notification(&Notification::AgentMessage(AgentMessage {
        text: "Response text".into(),
        is_streaming: true,
    }));

    // Turn completes
    let turn = Notification::TurnCompleted;
    let ui_changed = ui.apply_notification(&turn);
    let session_changed = session.apply_notification(&turn);

    assert!(ui_changed);
    assert!(session_changed);

    // Streaming committed to messages
    assert!(ui.streaming_text().is_empty());
    assert_eq!(ui.messages().len(), 1);
    assert!(matches!(
        ui.messages()[0].kind(),
        ChatMessageKind::AgentText(_)
    ));

    // Session back to Active
    assert_eq!(session.status(), &SessionStatus::Active);
    assert_eq!(ui.activity(), Activity::Ready);
}

#[test]
fn tool_call_lifecycle() {
    let mut ui = UiState::new(500);

    // Tool starts
    let tc = ToolCall::new(
        ToolCallId::new("tc_1"),
        "read_file".into(),
        Some("Reading main.rs".into()),
        ToolKind::Read,
        ToolCallStatus::InProgress,
        None,
    );
    ui.apply_notification(&Notification::ToolCallStarted(tc));
    assert_eq!(ui.active_tool_calls().len(), 1);
    assert_eq!(ui.activity(), Activity::ToolRunning);

    // Tool updates
    let tc_updated = ToolCall::new(
        ToolCallId::new("tc_1"),
        "read_file".into(),
        Some("Read main.rs (245 lines)".into()),
        ToolKind::Read,
        ToolCallStatus::Completed,
        None,
    );
    ui.apply_notification(&Notification::ToolCallUpdated(tc_updated));
    assert_eq!(
        ui.active_tool_calls()[0].title(),
        Some("Read main.rs (245 lines)")
    );
}

#[test]
fn context_usage_flows_to_both() {
    let mut ui = UiState::new(500);
    let mut session = SessionController::new();

    let notification = Notification::ContextUsageUpdated(ContextUsage::new(85.0));
    ui.apply_notification(&notification);
    session.apply_notification(&notification);

    assert!((ui.context_usage().unwrap_or(0.0) - 85.0).abs() < f64::EPSILON);
    assert!(
        (session
            .context_usage()
            .map(|u| u.percentage())
            .unwrap_or(0.0)
            - 85.0)
            .abs()
            < f64::EPSILON
    );
}

#[test]
fn bridge_disconnect_updates_both() {
    let mut ui = UiState::new(500);
    let mut session = SessionController::new();
    session.set_status(SessionStatus::Active);

    let notification = Notification::BridgeDisconnected {
        reason: "process exited".into(),
    };
    ui.apply_notification(&notification);
    session.apply_notification(&notification);

    assert_eq!(session.status(), &SessionStatus::Disconnected);
    assert_eq!(ui.activity(), Activity::Idle);
    // UI should show a disconnect message
    assert!(!ui.messages().is_empty());
}

#[test]
fn mode_change_updates_session() {
    let mut session = SessionController::new();
    let changed = session.apply_notification(&Notification::ModeChanged {
        mode_id: "chat".into(),
    });
    assert!(changed);
    assert_eq!(session.current_mode_id(), Some("chat"));
}

#[test]
fn commands_updated_stores_in_session() {
    let mut session = SessionController::new();
    let cmds = vec![
        CommandInfo::new("model", "Switch model", Some("Change model"), true, false, false),
        CommandInfo::new("compact", "Compact", None::<&str>, false, false, false),
    ];
    session.apply_notification(&Notification::CommandsUpdated(cmds));
    assert_eq!(session.agent_commands().len(), 2);
}

#[test]
fn agent_switched_shows_message_and_activates() {
    let mut ui = UiState::new(500);
    let mut session = SessionController::new();
    session.set_status(SessionStatus::Busy);

    let notification = Notification::AgentSwitched {
        name: "code-agent".into(),
        welcome: Some("Ready to code!".into()),
    };
    ui.apply_notification(&notification);
    session.apply_notification(&notification);

    assert_eq!(session.status(), &SessionStatus::Active);
    // UI should show welcome message
    assert!(!ui.messages().is_empty());
}

#[test]
fn message_limit_enforced() {
    let mut ui = UiState::new(3);
    for i in 0..5 {
        ui.add_user_message(&format!("msg {i}"));
    }
    assert_eq!(ui.messages().len(), 3);
}

#[test]
fn command_registry_with_builtins_resolves() {
    let registry = cyril_core::commands::CommandRegistry::with_builtins();
    assert!(registry.parse("/help").is_some());
    assert!(registry.parse("/quit").is_some());
    assert!(registry.parse("/q").is_some());
    assert!(registry.parse("/new").is_some());
    assert!(registry.parse("/load session_1").is_some());
    assert!(registry.parse("not a command").is_none());
}

#[tokio::test]
async fn command_sends_to_bridge() {
    let registry = cyril_core::commands::CommandRegistry::with_builtins();
    let session = SessionController::new();
    let (tx, mut rx) = tokio::sync::mpsc::channel(4);
    let sender = cyril_core::protocol::bridge::BridgeSender::from_sender(tx);

    let (cmd, args) = registry.parse("/new").expect("should parse /new");
    let ctx = cyril_core::commands::CommandContext {
        session: &session,
        bridge: &sender,
    };
    let result = cmd.execute(&ctx, args).await;
    assert!(result.is_ok());

    let bridge_cmd = rx.recv().await;
    assert!(matches!(
        bridge_cmd,
        Some(BridgeCommand::NewSession { .. })
    ));
}

#[test]
fn session_created_activates_both_controllers() {
    let mut ui = UiState::new(500);
    let mut session = SessionController::new();

    let notification = Notification::SessionCreated {
        session_id: SessionId::new("sess_123"),
        current_mode: Some("kiro_default".into()),
    };

    let ui_changed = ui.apply_notification(&notification);
    let session_changed = session.apply_notification(&notification);

    assert!(ui_changed);
    assert!(session_changed);

    // Session should be active with an ID
    assert_eq!(session.status(), &SessionStatus::Active);
    assert_eq!(session.id().map(SessionId::as_str), Some("sess_123"));

    // UI should show the session label
    assert_eq!(ui.session_label(), Some("sess_123"));
}
