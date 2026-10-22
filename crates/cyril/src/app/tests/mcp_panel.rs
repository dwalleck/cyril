use super::*;
use std::sync::Arc;

use cyril_core::types::{
    McpCatalog, McpContent, McpElicitationMode, McpElicitationNotice, McpErrorKind, McpOperation,
    McpOperationError, McpOperationResult, McpOutcome, McpPending, McpPreview, McpPrompt,
    McpPromptArgument, McpRequest, McpResource, McpServer, McpServerStatus, McpSnapshot, McpUpdate,
};
use cyril_ui::traits::Activity;
use ratatui::backend::TestBackend;

fn snapshot(session: &SessionId) -> Arc<McpSnapshot> {
    Arc::new(McpSnapshot {
        session_id: session.clone(),
        servers: vec![McpServer {
            name: "server exact / 日".into(),
            auth_type: None,
            provenance: None,
            status: McpServerStatus::Connected {
                catalog: Arc::new(McpCatalog {
                    prompts: vec![McpPrompt {
                        name: "prompt exact".into(),
                        title: None,
                        description: None,
                        arguments: vec![
                            McpPromptArgument {
                                name: "required".into(),
                                description: None,
                                required: true,
                            },
                            McpPromptArgument {
                                name: "optional".into(),
                                description: None,
                                required: false,
                            },
                        ],
                    }],
                    resources: vec![McpResource {
                        uri: "canary://original/%20?name=日".into(),
                        name: "resource exact".into(),
                        title: None,
                        description: None,
                        mime_type: Some("text/plain".into()),
                    }],
                    ..McpCatalog::default()
                }),
            },
        }],
        access_mode: None,
        registry_servers: None,
        access_mode_filtered_servers: None,
        unresolved_registry_servers: None,
        provenance: None,
    })
}

fn push(app: &mut App, session: &SessionId, update: McpUpdate) {
    assert!(
        app.handle_notification(RoutedNotification::scoped(
            session.clone(),
            Notification::McpUpdate(update)
        ))
        .is_empty()
    );
}

async fn open(app: &mut App) {
    app.ui_state.insert_text("/mcp");
    app.submit_input().await.expect("open /mcp");
    assert_eq!(app.ui_state.topmost_overlay(), Some(Overlay::Mcp));
}

fn screen(app: &mut App) -> String {
    screen_at(app, 140, 40)
}

fn screen_at(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    app.draw_frame(&mut terminal, "MCP panel draw")
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

async fn prompt_form(app: &mut App) {
    for code in [KeyCode::Tab, KeyCode::Tab, KeyCode::Enter] {
        app.handle_key(key(code)).await.expect("open arguments");
    }
}

#[tokio::test]
async fn kas_command_opens_unknown_and_pushes_only_refresh_open_panel() {
    let (mut app, mut commands) = test_app_with_engine_and_command_rx(AgentEngine::Kas);
    let session = SessionId::new("mcp-panel");
    establish_main_session(&mut app, &session);
    open(&mut app).await;
    assert!(screen(&mut app).contains("inventory unreported"));
    assert!(commands.try_recv().is_err());
    push(&mut app, &session, McpUpdate::Snapshot(snapshot(&session)));
    assert!(screen(&mut app).contains("server exact / 日"));
    app.handle_key(key(KeyCode::Esc)).await.expect("close");
    push(&mut app, &session, McpUpdate::Snapshot(snapshot(&session)));
    assert!(app.ui_state.mcp_panel().is_none());
    assert!(commands.try_recv().is_err());
    open(&mut app).await;
    assert!(screen(&mut app).contains("server exact / 日"));
}

#[tokio::test]
async fn keys_dispatch_only_exact_typed_operations_and_pending_survives_reopen() {
    let (mut app, mut commands) = test_app_with_engine_and_command_rx(AgentEngine::Kas);
    let session = SessionId::new("mcp-panel");
    establish_main_session(&mut app, &session);
    push(&mut app, &session, McpUpdate::Snapshot(snapshot(&session)));
    open(&mut app).await;
    app.handle_key(key(KeyCode::Char('r')))
        .await
        .expect("reset");
    let BridgeCommand::Mcp { request } = commands.try_recv().expect("typed reset") else {
        panic!("MCP command only")
    };
    assert_eq!(
        request,
        McpRequest {
            session_id: session.clone(),
            server_name: "server exact / 日".into(),
            operation: McpOperation::Reset { start_oauth: false }
        }
    );
    push(
        &mut app,
        &session,
        McpUpdate::Pending(McpPending {
            id: 1,
            request: request.clone(),
        }),
    );
    app.handle_key(key(KeyCode::Esc)).await.expect("close");
    open(&mut app).await;
    assert!(screen(&mut app).contains("PENDING"));
    app.handle_key(key(KeyCode::Char('a')))
        .await
        .expect("duplicate blocked");
    assert!(commands.try_recv().is_err());
    push(
        &mut app,
        &session,
        McpUpdate::Outcome(McpOutcome {
            id: 1,
            request,
            result: Ok(McpOperationResult::Acknowledged),
        }),
    );
    app.handle_key(key(KeyCode::Char('a')))
        .await
        .expect("authenticate");
    let BridgeCommand::Mcp { request } = commands.try_recv().expect("typed auth") else {
        panic!("MCP command only")
    };
    assert_eq!(request.operation, McpOperation::Reset { start_oauth: true });
    assert_eq!(request.session_id, session);
    assert_eq!(request.server_name, "server exact / 日");
    assert!(commands.try_recv().is_err());
}

#[tokio::test]
async fn argument_paste_preserves_values_and_preview_never_submits_model_turn() {
    let (mut app, mut commands) = test_app_with_engine_and_command_rx(AgentEngine::Kas);
    let session = SessionId::new("mcp-read");
    establish_main_session(&mut app, &session);
    push(&mut app, &session, McpUpdate::Snapshot(snapshot(&session)));
    open(&mut app).await;
    prompt_form(&mut app).await;
    app.handle_key(key(KeyCode::Enter))
        .await
        .expect("missing required rejected");
    assert!(commands.try_recv().is_err());
    let value = format!("  \t\r\n{}'\"\\  ", "日".repeat(4_500));
    app.handle_terminal_event(Event::Paste(value.clone()))
        .await
        .expect("paste argument");
    app.handle_key(key(KeyCode::Enter))
        .await
        .expect("fetch prompt");
    let BridgeCommand::Mcp { request } = commands.try_recv().expect("get prompt") else {
        panic!("MCP only, no model prompt")
    };
    let McpOperation::GetPrompt { name, arguments } = &request.operation else {
        panic!("get prompt")
    };
    assert_eq!(name, "prompt exact");
    assert_eq!(arguments.get("required"), Some(&value));
    assert!(!arguments.contains_key("optional"));
    assert_eq!(request.session_id, session);
    assert_eq!(request.server_name, "server exact / 日");
    push(
        &mut app,
        &session,
        McpUpdate::Pending(McpPending {
            id: 2,
            request: request.clone(),
        }),
    );
    push(
        &mut app,
        &session,
        McpUpdate::Outcome(McpOutcome {
            id: 2,
            request,
            result: Ok(McpOperationResult::Preview(Arc::new(McpPreview::Prompt {
                description: Some("external preview".into()),
                messages: vec![cyril_core::types::McpMessage {
                    role: "assistant".into(),
                    content: McpContent::Text {
                        text: "CANARY external 'quoted' \\ 日".into(),
                        mime_type: Some("text/plain".into()),
                        uri: None,
                    },
                }],
            }))),
        }),
    );
    let text = screen(&mut app);
    assert!(text.contains("READ-ONLY EXTERNAL DATA"));
    assert!(text.contains("CANARY external 'quoted' \\ 日"));
    app.handle_terminal_event(Event::Paste("must not enter chat".into()))
        .await
        .expect("preview paste");
    app.handle_key(key(KeyCode::Enter))
        .await
        .expect("preview enter inert");
    assert!(app.ui_state.input_text().is_empty());
    assert!(commands.try_recv().is_err());
    assert!(
        !system_messages(&app)
            .iter()
            .any(|message| message.contains("CANARY"))
    );
}

#[tokio::test]
async fn resource_read_uses_advertised_uri_and_shows_failures_across_close() {
    let (mut app, mut commands) = test_app_with_engine_and_command_rx(AgentEngine::Kas);
    let session = SessionId::new("mcp-resource");
    establish_main_session(&mut app, &session);
    push(&mut app, &session, McpUpdate::Snapshot(snapshot(&session)));
    open(&mut app).await;
    for code in [KeyCode::Tab, KeyCode::Tab, KeyCode::Tab, KeyCode::Enter] {
        app.handle_key(key(code)).await.expect("resource key");
    }
    let BridgeCommand::Mcp { request } = commands.try_recv().expect("resource read") else {
        panic!("MCP only")
    };
    assert_eq!(
        request.operation,
        McpOperation::GetResource {
            uri: "canary://original/%20?name=日".into()
        }
    );
    push(
        &mut app,
        &session,
        McpUpdate::Outcome(McpOutcome {
            id: 3,
            request,
            result: Err(McpOperationError {
                kind: McpErrorKind::Policy,
                message: "governance refused exact".into(),
            }),
        }),
    );
    assert!(screen(&mut app).contains("governance refused exact"));
    app.handle_key(key(KeyCode::Esc)).await.expect("back");
    app.handle_key(key(KeyCode::Esc)).await.expect("close");
    open(&mut app).await;
    // Scroll to the policy error retained by core, if catalog focus is lower.
    app.handle_key(key(KeyCode::PageUp)).await.expect("scroll");
    assert!(screen(&mut app).contains("governance refused exact"));
    assert!(commands.try_recv().is_err());
}

#[tokio::test]
async fn topmost_approval_owns_keys_paste_and_paint_above_mcp() {
    let (mut app, mut commands) = test_app_with_engine_and_command_rx(AgentEngine::Kas);
    let session = SessionId::new("mcp-priority");
    establish_main_session(&mut app, &session);
    push(&mut app, &session, McpUpdate::Snapshot(snapshot(&session)));
    open(&mut app).await;
    prompt_form(&mut app).await;
    let (request, _response) = trust_request("mcp-priority");
    app.ui_state.show_approval(request);
    assert_eq!(app.ui_state.topmost_overlay(), Some(Overlay::Approval));
    app.handle_terminal_event(Event::Paste("blocked by approval".into()))
        .await
        .expect("guard paste");
    app.handle_key(key(KeyCode::Char('r')))
        .await
        .expect("approval key");
    assert!(commands.try_recv().is_err());
    let text = screen(&mut app);
    assert!(
        text.contains("Allow command?"),
        "topmost approval must remain visible"
    );
    app.handle_key(key(KeyCode::Esc))
        .await
        .expect("dismiss approval");
    assert_eq!(app.ui_state.topmost_overlay(), Some(Overlay::Mcp));
    app.handle_key(key(KeyCode::Enter))
        .await
        .expect("required still omitted");
    assert!(commands.try_recv().is_err());
    app.handle_terminal_event(Event::Paste("allowed exact".into()))
        .await
        .expect("editor paste");
    app.handle_key(key(KeyCode::Enter))
        .await
        .expect("read after approval");
    let BridgeCommand::Mcp { request } = commands.try_recv().expect("typed request") else {
        panic!("MCP")
    };
    let McpOperation::GetPrompt { arguments, .. } = request.operation else {
        panic!("prompt")
    };
    assert_eq!(arguments["required"], "allowed exact");
    assert!(app.ui_state.input_text().is_empty());
}

#[test]
fn elicitation_decline_notice_does_not_open_a_panel_or_send_a_command() {
    let (mut app, mut commands) = test_app_with_engine_and_command_rx(AgentEngine::Kas);
    let session = SessionId::new("actual-mcp-session");
    establish_main_session(&mut app, &session);
    for mode in [McpElicitationMode::Form, McpElicitationMode::Url] {
        push(
            &mut app,
            &session,
            McpUpdate::ElicitationDeclined(McpElicitationNotice {
                session_id: session.clone(),
                tool_call_id: ToolCallId::new("actual-tool"),
                mode,
                message: "real request".into(),
                url: Some("https://never-open.invalid/".into()),
            }),
        );
    }
    let messages = system_messages(&app).join("\n");
    for expected in [
        "form elicitation declined",
        "URL elicitation declined",
        "actual-mcp-session",
        "actual-tool",
        "not opened",
    ] {
        assert!(messages.contains(expected), "missing {expected}");
    }
    assert!(app.ui_state.topmost_overlay().is_none());
    assert!(commands.try_recv().is_err());
}

#[rstest::rstest]
#[case::form(McpElicitationMode::Form, "MCP form elicitation declined")]
#[case::url(McpElicitationMode::Url, "MCP URL elicitation declined")]
fn elicitation_decline_reveals_main_after_child_focus_without_changing_authority(
    #[case] mode: McpElicitationMode,
    #[case] expected: &str,
) {
    let (mut app, mut commands) = test_app_with_engine_and_command_rx(AgentEngine::Kas);
    let main = SessionId::new("actual-mcp-session");
    let child = SessionId::new("mcp-decline-viewer");
    establish_main_session(&mut app, &main);
    push(&mut app, &main, McpUpdate::Snapshot(snapshot(&main)));
    assert!(
        app.handle_notification(RoutedNotification::scoped(
            main.clone(),
            Notification::TurnStarted,
        ))
        .is_empty()
    );
    assert!(
        app.handle_notification(RoutedNotification::scoped(
            main.clone(),
            Notification::ToolCallStarted(ToolCall::new(
                ToolCallId::new("mcp-live-tool"),
                "Active tool".into(),
                ToolKind::Execute,
                ToolCallStatus::InProgress,
                None,
            )),
        ))
        .is_empty()
    );
    for _ in 0..40 {
        app.ui_state.add_system_message("Older main history".into());
    }
    assert!(
        app.handle_notification(agent_text_frame(&main, "Main before MCP decline", true))
            .is_empty()
    );
    assert!(
        app.handle_notification(agent_text_frame(&child, "Child before MCP decline", true))
            .is_empty()
    );
    app.ui_state.insert_text("keep café draft");
    app.ui_state.handle_input_key(KeyCode::Left.into());
    let cursor = app.ui_state.input_cursor();
    let model = app.session.current_model().map(str::to_owned);
    let authority = app.session.mcp();
    assert!(authority.snapshot.is_some());
    app.ui_state.chat_scroll_up(usize::MAX);
    assert!(app.ui_state.focus_subagent(child.clone()));
    assert!(screen(&mut app).contains("Child before MCP decline"));
    assert!(app.ui_state.chat_scroll_back().is_some());
    app.redraw_needed = false;

    push(
        &mut app,
        &main,
        McpUpdate::ElicitationDeclined(McpElicitationNotice {
            session_id: main.clone(),
            tool_call_id: ToolCallId::new("actual-tool"),
            mode,
            message: "real request".into(),
            url: Some("https://never-open.invalid/".into()),
        }),
    );

    let messages = system_messages(&app);
    assert_eq!(
        messages
            .iter()
            .filter(|text| text.starts_with(expected))
            .count(),
        1
    );
    for context in [
        "actual-mcp-session",
        "actual-tool",
        "real request",
        "not opened",
    ] {
        assert!(
            messages
                .iter()
                .any(|text| { text.starts_with(expected) && text.contains(context) }),
            "decline notice lost {context}"
        );
    }
    assert!(app.redraw_needed);
    assert_eq!(app.ui_state.input_text(), "keep café draft");
    assert_eq!(app.ui_state.input_cursor(), cursor);
    assert_eq!(app.session.id(), Some(&main));
    assert_eq!(app.ui_state.main_session_id(), Some(&main));
    assert_eq!(app.session.current_model(), model.as_deref());
    assert_eq!(app.session.status(), &SessionStatus::Busy);
    assert_eq!(app.ui_state.activity(), Activity::Streaming);
    assert_eq!(app.ui_state.active_tool_calls().len(), 1);
    assert!(app.session.last_turn().is_none());
    assert!(app.ui_state.last_turn().is_none());
    let retained = app.session.mcp();
    assert_eq!(retained.session_id, authority.session_id);
    assert_eq!(retained.snapshot, authority.snapshot);
    assert_eq!(retained.policy, authority.policy);
    assert_eq!(retained.diagnostic, authority.diagnostic);
    assert_eq!(retained.authorization_urls, authority.authorization_urls);
    assert_eq!(retained.pending, authority.pending);
    assert_eq!(retained.outcomes, authority.outcomes);
    let stream = &app.ui_state.subagent_ui().streams()[&child];
    assert!(stream.messages().is_empty());
    assert_eq!(stream.streaming_text(), "Child before MCP decline");
    assert_eq!(stream.activity(), Activity::Streaming);
    assert!(app.ui_state.topmost_overlay().is_none());
    assert!(commands.try_recv().is_err());

    let visible = screen(&mut app);
    assert!(
        visible.contains(expected),
        "decline notice remains behind child focus or prior main browse:\n{visible}"
    );
    assert!(app.ui_state.subagent_ui().focused_session_id().is_none());
    assert_eq!(app.ui_state.chat_scroll_back(), None);
    assert!(!app.ui_state.has_modal_overlay());
    assert!(commands.try_recv().is_err());
}

#[tokio::test]
async fn session_change_discards_editor_and_foreign_push_cannot_change_panel() {
    let (mut app, mut commands) = test_app_with_engine_and_command_rx(AgentEngine::Kas);
    let session = SessionId::new("mcp-old");
    establish_main_session(&mut app, &session);
    push(&mut app, &session, McpUpdate::Snapshot(snapshot(&session)));
    open(&mut app).await;
    prompt_form(&mut app).await;
    app.handle_terminal_event(Event::Paste("old draft".into()))
        .await
        .expect("draft");
    let next = SessionId::new("mcp-new");
    establish_main_session(&mut app, &next);
    assert!(app.ui_state.mcp_panel().is_none());
    push(&mut app, &session, McpUpdate::Snapshot(snapshot(&session)));
    assert!(app.session.mcp().snapshot.is_none());
    open(&mut app).await;
    assert!(screen(&mut app).contains("inventory unreported"));
    assert!(commands.try_recv().is_err());
}

async fn assert_offscreen_server_action_stays_off_the_bridge(code: KeyCode, start_oauth: bool) {
    let (mut app, mut commands) = test_app_with_engine_and_command_rx(AgentEngine::Kas);
    let session = SessionId::new("mcp-hidden-action");
    establish_main_session(&mut app, &session);
    let mut inventory = (*snapshot(&session)).clone();
    inventory.servers.push(McpServer {
        name: "other visible / 日".into(),
        auth_type: None,
        status: McpServerStatus::Connected {
            catalog: Arc::new(McpCatalog::default()),
        },
        provenance: None,
    });
    push(&mut app, &session, McpUpdate::Snapshot(Arc::new(inventory)));
    open(&mut app).await;
    let draft = "untouched chat draft";
    app.ui_state.insert_text(draft);
    let initial = screen_at(&mut app, 80, 6);
    assert!(initial.contains("> server exact / 日"));
    assert!(!initial.contains("other visible / 日"));
    assert!(
        !initial.contains("MCP ·"),
        "compact popup must not expose a target header"
    );
    assert!(commands.try_recv().is_err());

    app.handle_key(key(KeyCode::PageDown))
        .await
        .expect("free scroll");
    let scrolled = screen_at(&mut app, 80, 6);
    assert!(
        scrolled.contains("other visible / 日"),
        "another server is actually painted"
    );
    assert!(
        !scrolled.contains("server exact / 日"),
        "the retained target is not painted"
    );
    app.handle_key(key(code))
        .await
        .expect("reanchor hidden server");
    assert!(
        commands.try_recv().is_err(),
        "the first reset/auth activation must not reach the bridge before another draw"
    );
    assert_eq!(app.ui_state.input_text(), draft);
    let reanchored = screen_at(&mut app, 80, 6);
    assert!(reanchored.contains("> server exact / 日"));
    assert!(!reanchored.contains("other visible / 日"));

    app.handle_key(key(code))
        .await
        .expect("activate visible original server");
    let BridgeCommand::Mcp { request } = commands.try_recv().expect("one explicit MCP command")
    else {
        panic!("MCP command only");
    };
    assert_eq!(
        request,
        McpRequest {
            session_id: session,
            server_name: "server exact / 日".into(),
            operation: McpOperation::Reset { start_oauth },
        }
    );
    assert!(commands.try_recv().is_err());
    assert_eq!(app.ui_state.input_text(), draft);
    app.handle_key(key(KeyCode::Esc)).await.expect("close");
    assert!(app.ui_state.mcp_panel().is_none());
    assert_eq!(app.ui_state.input_text(), draft);
}

#[tokio::test]
async fn offscreen_server_reset_reanchors_before_any_bridge_dispatch() {
    assert_offscreen_server_action_stays_off_the_bridge(KeyCode::Char('r'), false).await;
}

#[tokio::test]
async fn offscreen_server_authenticate_reanchors_before_any_bridge_dispatch() {
    assert_offscreen_server_action_stays_off_the_bridge(KeyCode::Char('a'), true).await;
}
