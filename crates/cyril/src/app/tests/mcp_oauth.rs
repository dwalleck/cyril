use super::*;
use cyril_ui::traits::Activity;
use ratatui::backend::TestBackend;

fn draw_mcp_screen(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(160, 32)).expect("test terminal");
    app.draw_frame(&mut terminal, "MCP notification draw")
        .expect("draw");
    let buffer = terminal.backend().buffer();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn external_url_is_visible_before_session_and_survives_creation() {
    let (mut app, mut commands) = test_app_with_engine_and_command_rx(AgentEngine::Kas);
    let prior_status = app.session.status().clone();
    app.ui_state.set_activity(Activity::Waiting);
    app.redraw_needed = false;
    let expected = "Sign-in requested. Open in browser: https://example.com/authorize?code=a%2Fb&state=unchanged#continue";

    let deferred = app.handle_notification(RoutedNotification::global(
        Notification::ExternalUrlRequested {
            url: "https://example.com/authorize?code=a%2Fb&state=unchanged#continue".into(),
        },
    ));

    assert!(deferred.is_empty());
    assert!(commands.try_recv().is_err());
    assert_eq!(system_messages(&app), [expected]);
    assert!(app.redraw_needed);
    assert!(app.session.id().is_none());
    assert_eq!(app.session.status(), &prior_status);
    assert_eq!(app.ui_state.activity(), Activity::Waiting);
    assert!(app.pending_session_notifications.is_empty());
    assert!(!app.ui_state.has_modal_overlay());
    assert!(draw_mcp_screen(&mut app).contains(expected));

    assert!(
        app.handle_notification(session_created_frame(&SessionId::new("mcp-startup")))
            .is_empty()
    );
    assert_eq!(
        system_messages(&app),
        [expected, "Session created: mcp-startup"]
    );
    assert!(draw_mcp_screen(&mut app).contains(expected));
    assert!(commands.try_recv().is_err());
    assert!(!app.ui_state.has_modal_overlay());
}

#[test]
fn mcp_callback_and_error_escape_controls_without_releasing_busy_turn() {
    let (mut app, mut commands) = test_app_with_engine_and_command_rx(AgentEngine::Kas);
    let main = SessionId::new("mcp-busy");
    establish_main_session(&mut app, &main);
    app.handle_notification(RoutedNotification::scoped(
        main.clone(),
        Notification::TurnStarted,
    ));
    app.handle_notification(RoutedNotification::scoped(
        main.clone(),
        Notification::ToolCallStarted(ToolCall::new(
            ToolCallId::new("mcp-active-tool"),
            "Active tool".into(),
            ToolKind::Execute,
            ToolCallStatus::InProgress,
            None,
        )),
    ));
    app.handle_notification(RoutedNotification::scoped(
        main.clone(),
        Notification::TurnStalled {
            quiet: Duration::from_secs(31),
        },
    ));
    app.redraw_needed = false;

    assert!(
        app.handle_notification(RoutedNotification::global(
            Notification::ExternalUrlRequested {
                url: "https://example.com/authorize?\u{1b}[31mstate=x\n\u{202e}next".into(),
            },
        ))
        .is_empty()
    );
    assert!(
        app.handle_notification(RoutedNotification::scoped(
            main.clone(),
            Notification::McpConnectionError {
                message: "MCP failed:\u{1b}[2J\n\u{202e}endpoint".into(),
            },
        ))
        .is_empty()
    );

    let url_notice = r"Sign-in requested. Open in browser: https://example.com/authorize?\u{1b}[31mstate=x\n\u{202e}next";
    let error_notice = r"[error] MCP failed:\u{1b}[2J\n\u{202e}endpoint";
    assert_eq!(
        system_messages(&app),
        ["Session created: mcp-busy", url_notice, error_notice]
    );
    assert!(app.redraw_needed);
    assert_eq!(app.session.id(), Some(&main));
    assert_eq!(app.session.status(), &SessionStatus::Busy);
    assert!(app.session.last_turn().is_none());
    assert_eq!(app.ui_state.activity(), Activity::ToolRunning);
    assert!(app.ui_state.last_turn().is_none());
    assert_eq!(app.ui_state.active_tool_calls().len(), 1);
    assert!(app.ui_state.stall().is_some());
    assert!(!app.ui_state.has_modal_overlay());
    assert!(commands.try_recv().is_err());

    app.handle_notification(RoutedNotification::scoped(
        main,
        Notification::McpServerInitialized {
            server_name: "later-status".into(),
        },
    ));
    let screen = draw_mcp_screen(&mut app);
    assert!(screen.contains(url_notice), "manual URL missing:\n{screen}");
    assert!(
        screen.contains(error_notice),
        "original error missing:\n{screen}"
    );
    assert_eq!(app.session.status(), &SessionStatus::Busy);
    assert_eq!(app.ui_state.activity(), Activity::ToolRunning);
    assert_eq!(app.ui_state.active_tool_calls().len(), 1);
    assert!(app.ui_state.stall().is_some());
    assert!(!app.ui_state.has_modal_overlay());
}

#[test]
fn v2_mcp_oauth_request_keeps_existing_manual_message() {
    let (mut app, mut commands) = test_app_with_command_rx();
    let main = SessionId::new("mcp-v2");
    establish_main_session(&mut app, &main);
    app.handle_notification(RoutedNotification::scoped(main, Notification::TurnStarted));
    assert!(
        app.handle_notification(RoutedNotification::global(Notification::McpOAuthRequest {
            server_name: "legacy-mcp".into(),
            url: "https://example.com/v2?state=original#finish".into(),
        }))
        .is_empty()
    );
    let expected = "MCP server 'legacy-mcp' requires authentication. Open in browser: https://example.com/v2?state=original#finish";
    assert_eq!(system_messages(&app), ["Session created: mcp-v2", expected]);
    assert!(draw_mcp_screen(&mut app).contains(expected));
    assert_eq!(app.session.status(), &SessionStatus::Busy);
    assert_eq!(app.ui_state.activity(), Activity::Waiting);
    assert!(!app.ui_state.has_modal_overlay());
    assert!(commands.try_recv().is_err());
}

#[cfg(feature = "kas")]
#[test]
fn kas_mcp_display_error_before_new_response_reaches_visible_history() {
    const FRAME: &str = r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"mcp-startup","update":{"sessionUpdate":"session_info_update","_meta":{"kiro":{"kind":"display_error","errorType":"mcp_connection_error","message":"MCP connection failed: café endpoint refused"}}}}}"#;
    let (mut app, mut commands) = test_app_with_engine_and_command_rx(AgentEngine::Kas);
    let mut converted = cyril_core::test_support::kas_capture_routed(FRAME);
    assert_eq!(
        converted.len(),
        1,
        "production conversion must retain the error"
    );
    let error = converted.remove(0);
    assert_eq!(error.session_id, Some(SessionId::new("mcp-startup")));
    assert!(app.handle_notification(error).is_empty());
    assert_eq!(app.pending_session_notifications.len(), 1);
    assert!(system_messages(&app).is_empty());

    assert!(
        app.handle_notification(session_created_frame(&SessionId::new("mcp-startup")))
            .is_empty()
    );
    let expected = "[error] MCP connection failed: café endpoint refused";
    assert_eq!(
        system_messages(&app),
        ["Session created: mcp-startup", expected]
    );
    assert!(app.pending_session_notifications.is_empty());
    assert!(draw_mcp_screen(&mut app).contains(expected));
    assert_eq!(app.session.status(), &SessionStatus::Active);
    assert!(!app.ui_state.has_modal_overlay());
    assert!(commands.try_recv().is_err());

    app.handle_notification(session_created_frame(&SessionId::new("mcp-next")));
    assert_eq!(
        system_messages(&app),
        [
            "Session created: mcp-startup",
            expected,
            "Session created: mcp-next"
        ]
    );
    assert!(draw_mcp_screen(&mut app).contains(expected));
}

#[test]
fn scoped_mcp_error_stays_in_subagent_history_and_renders_on_focus() {
    let (mut app, mut commands) = test_app_with_engine_and_command_rx(AgentEngine::Kas);
    let main = SessionId::new("mcp-main");
    let peer = SessionId::new("mcp-peer");
    establish_main_session(&mut app, &main);
    app.handle_notification(RoutedNotification::scoped(main, Notification::TurnStarted));
    app.handle_notification(RoutedNotification::scoped(
        peer.clone(),
        Notification::AgentMessage(AgentMessage {
            text: "Peer is still working".into(),
            is_streaming: true,
        }),
    ));
    assert!(
        app.handle_notification(RoutedNotification::scoped(
            peer.clone(),
            Notification::McpConnectionError {
                message: "Peer MCP failed:\u{1b}[2J\u{202e}endpoint".into(),
            },
        ))
        .is_empty()
    );

    assert_eq!(system_messages(&app), ["Session created: mcp-main"]);
    assert_eq!(app.session.status(), &SessionStatus::Busy);
    assert_eq!(app.ui_state.activity(), Activity::Waiting);
    assert!(app.ui_state.subagent_ui().focused_stream().is_none());
    assert!(!app.ui_state.has_modal_overlay());
    assert!(commands.try_recv().is_err());

    assert!(app.ui_state.focus_subagent(peer));
    let stream = app
        .ui_state
        .subagent_ui()
        .focused_stream()
        .expect("focused peer stream");
    assert_eq!(stream.activity(), Activity::Streaming);
    assert_eq!(stream.messages().len(), 2);
    assert!(matches!(
        stream.messages()[0].kind(),
        ChatMessageKind::AgentText(text) if text == "Peer is still working"
    ));
    let expected = r"[error] Peer MCP failed:\u{1b}[2J\u{202e}endpoint";
    assert!(matches!(
        stream.messages()[1].kind(),
        ChatMessageKind::System(text) if text == expected
    ));
    let screen = draw_mcp_screen(&mut app);
    assert!(screen.contains(expected), "peer error missing:\n{screen}");
    assert_eq!(app.session.status(), &SessionStatus::Busy);
    assert_eq!(app.ui_state.activity(), Activity::Waiting);
}
