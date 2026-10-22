use super::*;
use cyril_ui::traits::Activity;
use ratatui::backend::TestBackend;
use ratatui::buffer::CellWidth;

fn draw_mcp_screen(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(160, 32)).expect("test terminal");
    app.draw_frame(&mut terminal, "MCP notification draw")
        .expect("draw");
    let buffer = terminal.backend().buffer();
    (0..buffer.area.height)
        .map(|y| {
            let mut row = String::new();
            let mut x = 0;
            while x < buffer.area.width {
                let cell = &buffer[(x, y)];
                row.push_str(cell.symbol());
                x = x.saturating_add(cell.cell_width().max(1));
            }
            row
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

    assert!(app
        .handle_notification(session_created_frame(&SessionId::new("mcp-startup")))
        .is_empty());
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

    assert!(app
        .handle_notification(RoutedNotification::global(
            Notification::ExternalUrlRequested {
                url: "https://example.com/authorize?\u{1b}[31mstate=x\n\u{202e}next".into(),
            },
        ))
        .is_empty());
    assert!(app
        .handle_notification(RoutedNotification::scoped(
            main.clone(),
            Notification::McpConnectionError {
                message: "MCP failed:\u{1b}[2J\n\u{202e}endpoint".into(),
            },
        ))
        .is_empty());

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
        main.clone(),
        Notification::McpServerInitialized {
            session_id: Some(main),
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
    assert!(app
        .handle_notification(RoutedNotification::global(Notification::McpOAuthRequest {
            session_id: None,
            server_name: "legacy-mcp".into(),
            url: "https://example.com/v2?state=original#finish".into(),
        }))
        .is_empty());
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

    assert!(app
        .handle_notification(session_created_frame(&SessionId::new("mcp-startup")))
        .is_empty());
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

fn mcp_history(messages: &[cyril_ui::traits::ChatMessage]) -> Vec<(&'static str, &str)> {
    messages
        .iter()
        .filter_map(|message| match message.kind() {
            ChatMessageKind::AgentText(text) => Some(("agent", text.as_str())),
            ChatMessageKind::System(text) => Some(("system", text.as_str())),
            _ => None,
        })
        .collect()
}

#[test]
fn global_mcp_url_reveals_main_after_child_focus_and_prior_browse() {
    let (mut app, mut commands) = test_app_with_engine_and_command_rx(AgentEngine::Kas);
    let main = SessionId::new("mcp-visible-main");
    let child = SessionId::new("mcp-visible-child");
    establish_main_session(&mut app, &main);
    app.handle_notification(RoutedNotification::scoped(
        main.clone(),
        Notification::TurnStarted,
    ));
    for _ in 0..40 {
        app.ui_state.add_system_message("Older main history".into());
    }
    app.handle_notification(agent_text_frame(&main, "Main text before sign-in", true));
    app.handle_notification(agent_text_frame(
        &child,
        "Child viewport before sign-in",
        true,
    ));
    app.ui_state.insert_text("keep café draft");
    app.ui_state.handle_input_key(KeyCode::Left.into());
    let cursor = app.ui_state.input_cursor();
    let model = app.session.current_model().map(str::to_owned);
    app.ui_state.chat_scroll_up(usize::MAX);
    assert!(app.ui_state.focus_subagent(child.clone()));
    assert!(draw_mcp_screen(&mut app).contains("Child viewport before sign-in"));
    assert!(app.ui_state.chat_scroll_back().is_some());
    app.redraw_needed = false;

    assert!(app
        .handle_notification(RoutedNotification::global(
            Notification::ExternalUrlRequested {
                url: "https://example.com/a'b\"c\\日?state=x\u{1b}\n\u{202e}#finish".into(),
            }
        ))
        .is_empty());

    let expected = r#"Sign-in requested. Open in browser: https://example.com/a'b"c\日?state=x\u{1b}\n\u{202e}#finish"#;
    assert!(app.redraw_needed);
    let screen = draw_mcp_screen(&mut app);
    assert!(
        screen.contains(expected),
        "global manual URL hidden:\n{screen}"
    );
    assert!(app.ui_state.subagent_ui().focused_session_id().is_none());
    assert_eq!(app.ui_state.chat_scroll_back(), None);
    assert_eq!(
        system_messages(&app)
            .iter()
            .filter(|text| text.as_str() == expected)
            .count(),
        1
    );
    let stream = &app.ui_state.subagent_ui().streams()[&child];
    assert!(stream.history().messages().is_empty());
    assert_eq!(stream.streaming_text(), "Child viewport before sign-in");
    assert_eq!(stream.activity(), Activity::Streaming);
    assert!(mcp_history(app.ui_state.history().messages())
        .ends_with(&[("agent", "Main text before sign-in"), ("system", expected),]));
    assert_eq!(app.ui_state.input_text(), "keep café draft");
    assert_eq!(app.ui_state.input_cursor(), cursor);
    assert_eq!(app.session.id(), Some(&main));
    assert_eq!(app.ui_state.main_session_id(), Some(&main));
    assert_eq!(app.session.current_model(), model.as_deref());
    assert_eq!(app.session.status(), &SessionStatus::Busy);
    assert_eq!(app.ui_state.activity(), Activity::Streaming);
    assert!(app.session.last_turn().is_none());
    assert!(app.ui_state.last_turn().is_none());
    assert!(!app.ui_state.has_modal_overlay());
    assert!(commands.try_recv().is_err());
}

#[rstest::rstest]
#[case::main("main")]
#[case::main_outranks_workflow_claim("main-claimed")]
#[case::tracked_child("tracked")]
#[case::optimistic_child("optimistic")]
#[case::claimed_workflow("workflow")]
fn mcp_error_keeps_canonical_owner_and_reveals_main_presentation(#[case] route: &str) {
    let (mut app, mut commands) = test_app_with_engine_and_command_rx(AgentEngine::Kas);
    let main = SessionId::new("mcp-main");
    let peer = SessionId::new("peer'\"\\tail\u{1b}\n\u{202e}");
    let viewer = SessionId::new("mcp-viewer");
    let main_owned = matches!(route, "main" | "main-claimed");
    let owner = if main_owned { &main } else { &peer };
    establish_main_session(&mut app, &main);
    if route == "tracked" {
        app.handle_notification(RoutedNotification::global(
            Notification::SubagentListUpdated {
                subagents: vec![SubagentInfo::new(
                    peer.clone(),
                    "reviewer",
                    "reviewer",
                    "query",
                    cyril_core::types::SubagentStatus::Working { message: None },
                )],
                pending_stages: Vec::new(),
            },
        ));
        assert!(app.ui_state.subagent_tracker().is_subagent(&peer));
    }
    if matches!(route, "workflow" | "main-claimed") {
        app.handle_notification(RoutedNotification::global(workflow_run_started_frame(
            "wf-mcp",
        )));
        app.handle_notification(RoutedNotification::global(workflow_node_claim_frame(
            "wf-mcp", "step", owner,
        )));
        assert!(app.workflow_tracker.session_owner(owner).is_some());
    }
    app.handle_notification(RoutedNotification::scoped(
        main.clone(),
        Notification::TurnStarted,
    ));
    app.handle_notification(RoutedNotification::scoped(
        main.clone(),
        Notification::ToolCallStarted(ToolCall::new(
            ToolCallId::new("mcp-live-tool"),
            "Active tool".into(),
            ToolKind::Execute,
            ToolCallStatus::InProgress,
            None,
        )),
    ));
    for _ in 0..40 {
        app.ui_state.add_system_message("Older main history".into());
    }
    app.handle_notification(agent_text_frame(&main, "Main before error", true));
    app.handle_notification(RoutedNotification::scoped(
        main.clone(),
        Notification::TurnStalled {
            quiet: Duration::from_secs(31),
        },
    ));
    if !main_owned {
        app.handle_notification(agent_text_frame(owner, "Owner before error", true));
    }
    app.handle_notification(agent_text_frame(&viewer, "Other focused child", true));
    app.ui_state.insert_text("keep café draft");
    app.ui_state.handle_input_key(KeyCode::Left.into());
    let cursor = app.ui_state.input_cursor();
    let model = app.session.current_model().map(str::to_owned);
    app.ui_state.chat_scroll_up(usize::MAX);
    assert!(app.ui_state.focus_subagent(viewer.clone()));
    assert!(draw_mcp_screen(&mut app).contains("Other focused child"));
    app.redraw_needed = false;

    assert!(app
        .handle_notification(RoutedNotification::scoped(
            owner.clone(),
            Notification::McpConnectionError {
                message: "MCP can't open \"C:\\café\" \u{1b}\n\u{202e}".into(),
            },
        ))
        .is_empty());

    let canonical = r#"[error] MCP can't open "C:\café" \u{1b}\n\u{202e}"#;
    let presentation = if main_owned {
        canonical
    } else {
        r#"[error] [session peer'"\tail\u{1b}\n\u{202e}] MCP can't open "C:\café" \u{1b}\n\u{202e}"#
    };
    assert!(app.redraw_needed);
    let screen = draw_mcp_screen(&mut app);
    assert!(
        screen.contains(presentation),
        "route={route}, notice hidden:\n{screen}"
    );
    assert!(app.ui_state.subagent_ui().focused_session_id().is_none());
    assert_eq!(app.ui_state.chat_scroll_back(), None);
    assert!(mcp_history(app.ui_state.history().messages())
        .ends_with(&[("agent", "Main before error"), ("system", presentation),]));
    assert_eq!(
        system_messages(&app)
            .iter()
            .filter(|text| text.starts_with("[error]"))
            .count(),
        1
    );
    if main_owned {
        assert!(!app.ui_state.subagent_ui().streams().contains_key(&main));
        assert!(!app.ui_state.workflow_streams().contains_key(&main));
    } else {
        let stream = if route == "workflow" {
            assert!(!app.ui_state.subagent_ui().streams().contains_key(owner));
            &app.ui_state.workflow_streams()[owner]
        } else {
            assert!(!app.ui_state.workflow_streams().contains_key(owner));
            &app.ui_state.subagent_ui().streams()[owner]
        };
        assert_eq!(
            mcp_history(stream.history().messages()),
            [("agent", "Owner before error"), ("system", canonical),]
        );
        assert_eq!(stream.activity(), Activity::Streaming);
        assert!(stream.streaming_text().is_empty());
        assert!(!screen.contains("Owner before error"));
    }
    assert_eq!(
        app.ui_state.subagent_ui().streams()[&viewer].streaming_text(),
        "Other focused child"
    );
    assert_eq!(app.ui_state.input_text(), "keep café draft");
    assert_eq!(app.ui_state.input_cursor(), cursor);
    assert_eq!(app.session.id(), Some(&main));
    assert_eq!(app.ui_state.main_session_id(), Some(&main));
    assert_eq!(app.session.current_model(), model.as_deref());
    assert_eq!(app.session.status(), &SessionStatus::Busy);
    assert_eq!(app.ui_state.activity(), Activity::Streaming);
    assert_eq!(app.ui_state.active_tool_calls().len(), 1);
    assert!(app.ui_state.stall().is_some());
    assert!(app.session.last_turn().is_none());
    assert!(app.ui_state.last_turn().is_none());
    assert!(!app.ui_state.has_modal_overlay());
    assert!(commands.try_recv().is_err());

    app.handle_notification(agent_text_frame(owner, "Owner after error", false));
    app.handle_notification(RoutedNotification::scoped(
        owner.clone(),
        Notification::McpServerInitialized {
            session_id: Some(owner.clone()),
            server_name: "later-status".into(),
        },
    ));
    assert!(draw_mcp_screen(&mut app).contains(presentation));
    assert_eq!(
        system_messages(&app)
            .iter()
            .filter(|text| text.as_str() == presentation)
            .count(),
        1
    );
    let history = if main_owned {
        app.ui_state.history().messages()
    } else if route == "workflow" {
        app.ui_state.workflow_streams()[owner].history().messages()
    } else {
        app.ui_state.subagent_ui().streams()[owner]
            .history()
            .messages()
    };
    let history = mcp_history(history);
    let error_index = history
        .iter()
        .position(|entry| *entry == ("system", canonical))
        .expect("durable canonical error");
    assert_eq!(history[error_index + 1], ("agent", "Owner after error"));
    assert_eq!(app.session.status(), &SessionStatus::Busy);
    assert!(commands.try_recv().is_err());
}

#[rstest::rstest]
#[case::main("main")]
#[case::child("child")]
#[case::workflow("workflow")]
fn premain_mcp_error_waits_for_authority_then_presents_once(#[case] route: &str) {
    let (mut app, mut commands) = test_app_with_engine_and_command_rx(AgentEngine::Kas);
    let main = SessionId::new("mcp-pending-main");
    let peer = SessionId::new("mcp-pending-peer");
    let owner = if route == "main" { &main } else { &peer };
    for frame in [
        agent_text_frame(owner, "Before pending error", true),
        RoutedNotification::scoped(
            owner.clone(),
            Notification::McpConnectionError {
                message: "Pending endpoint refused".into(),
            },
        ),
        agent_text_frame(owner, "After pending error", false),
    ] {
        assert!(app.handle_notification(frame).is_empty());
    }
    assert_eq!(app.pending_session_notifications.len(), 3);
    assert!(system_messages(&app).is_empty());
    assert!(app.ui_state.subagent_ui().streams().is_empty());
    assert!(app.ui_state.workflow_streams().is_empty());
    assert!(!draw_mcp_screen(&mut app).contains("Pending endpoint refused"));
    if route == "workflow" {
        app.handle_notification(RoutedNotification::global(workflow_run_started_frame(
            "wf-pending-mcp",
        )));
        app.handle_notification(RoutedNotification::global(workflow_node_claim_frame(
            "wf-pending-mcp",
            "step",
            owner,
        )));
        assert_eq!(app.pending_session_notifications.len(), 3);
        assert!(system_messages(&app).is_empty());
    }
    assert!(commands.try_recv().is_err());

    assert!(app
        .handle_notification(session_created_frame(&main))
        .is_empty());

    let canonical = "[error] Pending endpoint refused";
    let presentation = if route == "main" {
        canonical
    } else {
        "[error] [session mcp-pending-peer] Pending endpoint refused"
    };
    assert!(app.pending_session_notifications.is_empty());
    assert_eq!(
        system_messages(&app),
        ["Session created: mcp-pending-main", presentation]
    );
    let history = match route {
        "main" => &app.ui_state.history().messages()[1..],
        "child" => {
            assert!(!app.ui_state.workflow_streams().contains_key(owner));
            app.ui_state.subagent_ui().streams()[owner]
                .history()
                .messages()
        }
        "workflow" => {
            assert!(!app.ui_state.subagent_ui().streams().contains_key(owner));
            app.ui_state.workflow_streams()[owner].history().messages()
        }
        _ => unreachable!("fixed route fixtures"),
    };
    assert_eq!(
        mcp_history(history),
        [
            ("agent", "Before pending error"),
            ("system", canonical),
            ("agent", "After pending error"),
        ]
    );
    assert!(draw_mcp_screen(&mut app).contains(presentation));
    assert_eq!(app.session.id(), Some(&main));
    assert_eq!(app.session.status(), &SessionStatus::Active);
    assert!(app.session.last_turn().is_none());
    assert!(!app.ui_state.has_modal_overlay());
    assert!(commands.try_recv().is_err());
}

#[test]
fn late_workflow_adoption_moves_canonical_mcp_error_without_representing_it() {
    let (mut app, mut commands) = test_app_with_engine_and_command_rx(AgentEngine::Kas);
    let main = SessionId::new("mcp-adoption-main");
    let peer = SessionId::new("mcp-adopted-peer");
    establish_main_session(&mut app, &main);
    app.handle_notification(agent_text_frame(&peer, "Before late claim", true));
    app.handle_notification(RoutedNotification::scoped(
        peer.clone(),
        Notification::McpConnectionError {
            message: "Adopted endpoint refused".into(),
        },
    ));
    let presentation = "[error] [session mcp-adopted-peer] Adopted endpoint refused";
    assert!(draw_mcp_screen(&mut app).contains(presentation));
    let main_notices = system_messages(&app);
    assert_eq!(
        main_notices
            .iter()
            .filter(|text| text.as_str() == presentation)
            .count(),
        1
    );
    assert!(app.ui_state.focus_subagent(peer.clone()));
    app.ui_state.chat_scroll_up(8);

    app.handle_notification(RoutedNotification::global(workflow_run_started_frame(
        "wf-adopt-mcp",
    )));
    app.handle_notification(RoutedNotification::global(workflow_node_claim_frame(
        "wf-adopt-mcp",
        "step",
        &peer,
    )));

    assert!(!app.ui_state.subagent_ui().streams().contains_key(&peer));
    assert!(app.ui_state.subagent_ui().focused_session_id().is_none());
    assert_eq!(
        app.ui_state.chat_scroll_back(),
        Some(8),
        "adoption must not emit another reveal"
    );
    assert_eq!(system_messages(&app), main_notices);
    assert_eq!(
        mcp_history(app.ui_state.workflow_streams()[&peer].history().messages()),
        [
            ("agent", "Before late claim"),
            ("system", "[error] Adopted endpoint refused"),
        ]
    );
    assert_eq!(
        app.ui_state.workflow_streams()[&peer].activity(),
        Activity::Streaming
    );
    app.handle_notification(agent_text_frame(&peer, "After late claim", false));
    assert_eq!(
        mcp_history(app.ui_state.workflow_streams()[&peer].history().messages()),
        [
            ("agent", "Before late claim"),
            ("system", "[error] Adopted endpoint refused"),
            ("agent", "After late claim"),
        ]
    );
    assert_eq!(system_messages(&app), main_notices);
    app.ui_state.chat_scroll_reset();
    assert!(draw_mcp_screen(&mut app).contains(presentation));
    assert_eq!(app.session.id(), Some(&main));
    assert!(commands.try_recv().is_err());
}
