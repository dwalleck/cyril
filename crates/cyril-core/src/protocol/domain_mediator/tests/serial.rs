use agent_client_protocol::UntypedMessage;

use super::super::{DomainChannels, DomainWork};
use crate::protocol::source_observer::IngressTracker;

#[tokio::test]
async fn domain_ingress_is_bounded_and_typed() {
    let (channels, mut work_rx, _host_rx) = DomainChannels::new(IngressTracker::new())
        .unwrap_or_else(|error| panic!("domain channels: {error}"));
    let work = UntypedMessage::new(
        "session/update",
        serde_json::json!({"update": {"sessionUpdate": "future"}}),
    )
    .unwrap_or_else(|error| panic!("typed work fixture is valid: {error}"));
    assert!(
        channels
            .enqueue(DomainWork::UnknownSessionUpdate(work))
            .await
            .is_ok(),
        "open mediator accepts typed ingress"
    );
    assert!(matches!(
        work_rx.recv().await,
        Some(DomainWork::UnknownSessionUpdate(message))
            if message.method() == "session/update"
    ));
}

#[test]
fn transport_close_marker_is_connection_scoped() {
    let (channels, _work_rx, _host_rx) = DomainChannels::new(IngressTracker::new())
        .unwrap_or_else(|error| panic!("first domain channels: {error}"));
    let marker: UntypedMessage = serde_json::from_str(&channels.transport_closed_line())
        .unwrap_or_else(|error| panic!("transport marker parses: {error}"));
    assert!(channels.is_transport_closed(&marker));

    let (other, _other_work_rx, _other_host_rx) = DomainChannels::new(IngressTracker::new())
        .unwrap_or_else(|error| panic!("second domain channels: {error}"));
    assert!(!other.is_transport_closed(&marker));
    let mut forged_params = marker.params.clone();
    let token = forged_params["token"]
        .as_array_mut()
        .unwrap_or_else(|| panic!("transport token is an array"));
    let first = token[0]
        .as_u64()
        .unwrap_or_else(|| panic!("transport token byte is numeric"));
    token[0] = serde_json::Value::from(first ^ 0xff);
    let forged = UntypedMessage::new(marker.method(), forged_params)
        .unwrap_or_else(|error| panic!("forged transport marker parses: {error}"));
    assert!(!channels.is_transport_closed(&forged));
}

#[tokio::test]
async fn initialization_failure_drains_queued_callback_error() {
    use std::rc::Rc;
    use std::time::Duration;

    use agent_client_protocol::Agent;
    use agent_client_protocol::schema::v1 as acp;

    use super::super::{DomainConfig, DomainMediator};
    use crate::protocol::bridge::create_channel_pair;
    use crate::protocol::engine::V2Engine;
    use crate::protocol::sdk_runtime::{SdkRuntime, StageChain};
    use crate::types::Notification;

    tokio::task::LocalSet::new()
        .run_until(async {
            let (handle, bridge) = create_channel_pair();
            let (_sender, mut notification_rx, _permission_rx, _source_rx, _completion_rx) =
                handle.split();
            let config = DomainConfig {
                engine: Rc::new(V2Engine),
                cwd: std::env::temp_dir(),
                present_as: None,
                stall_threshold: Duration::from_secs(30),
                #[cfg(feature = "kas")]
                host_shell: None,
            };
            let (mediator, channels) = DomainMediator::new(config, bridge)
                .unwrap_or_else(|error| panic!("initialization-failure mediator: {error}"));
            let callback_channels = channels.clone();
            let agent = Agent
                .builder()
                .name("initialization-failure-test-agent")
                .on_receive_request(
                    async move |_request: acp::InitializeRequest, responder, _connection| {
                        callback_channels
                            .enqueue(DomainWork::Routed(
                                Notification::BridgeError {
                                    operation: "auth".into(),
                                    message: "credential store unavailable".into(),
                                }
                                .into(),
                            ))
                            .await?;
                        responder.respond_with_error(
                            agent_client_protocol::Error::internal_error()
                                .data("authentication callback failed"),
                        )
                    },
                    agent_client_protocol::on_receive_request!(),
                );
            let runtime = SdkRuntime::start_for_test(agent, channels, StageChain::default())
                .await
                .unwrap_or_else(|error| panic!("initialization-failure runtime: {error}"));

            assert!(
                mediator.run(runtime).await.is_err(),
                "initialization failure must close the runtime"
            );
            let notification = tokio::time::timeout(Duration::from_secs(5), notification_rx.recv())
                .await
                .unwrap_or_else(|_| panic!("queued callback diagnostic timed out"))
                .unwrap_or_else(|| panic!("queued callback diagnostic was dropped"));
            assert!(matches!(
                notification.notification,
                Notification::BridgeError {
                    ref operation,
                    ref message,
                } if operation == "auth" && message == "credential store unavailable"
            ));
        })
        .await;
}

#[tokio::test]
async fn prompt_terminal_is_processed_after_queued_source_frames() {
    use std::rc::Rc;
    use std::time::Duration;

    use agent_client_protocol::schema::v1 as acp;
    use agent_client_protocol::{Agent, Client, ConnectionTo};

    use super::super::{DomainConfig, DomainMediator};
    use crate::protocol::bridge::create_channel_pair;
    use crate::protocol::engine::V2Engine;
    use crate::protocol::sdk_runtime::{SdkRuntime, StageChain};
    use crate::types::{
        BridgeCommand, PromptEnvelope, SessionId, SourceTurnDisposition, SourceTurnEventKind,
    };

    tokio::task::LocalSet::new()
        .run_until(async {
            let agent = Agent
                .builder()
                .name("source-order-test-agent")
                .on_receive_request(
                    async move |request: acp::InitializeRequest, responder, _connection| {
                        responder.respond(acp::InitializeResponse::new(request.protocol_version))
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_request(
                    async move |request: acp::PromptRequest,
                                responder,
                                connection: ConnectionTo<Client>| {
                        let notification: acp::SessionNotification =
                            serde_json::from_value(serde_json::json!({
                                "sessionId": request.session_id.to_string(),
                                "update": {
                                    "sessionUpdate": "agent_message_chunk",
                                    "content": {"type": "text", "text": "queued assistant"}
                                }
                            }))
                            .map_err(|error| {
                                agent_client_protocol::Error::internal_error()
                                    .data(format!("source-order notification fixture: {error}"))
                            })?;
                        connection.send_notification(notification)?;
                        responder.respond(acp::PromptResponse::new(acp::StopReason::EndTurn))
                    },
                    agent_client_protocol::on_receive_request!(),
                );
            let (handle, bridge) = create_channel_pair();
            let (sender, _notification_rx, _permission_rx, mut source_rx, _completion_rx) =
                handle.split();
            let config = DomainConfig {
                engine: Rc::new(V2Engine),
                cwd: std::env::temp_dir(),
                present_as: None,
                stall_threshold: Duration::from_secs(30),
                #[cfg(feature = "kas")]
                host_shell: None,
            };
            let (mediator, channels) = DomainMediator::new(config, bridge)
                .unwrap_or_else(|error| panic!("source-order mediator: {error}"));
            let runtime = SdkRuntime::start_for_test(agent, channels, StageChain::default())
                .await
                .unwrap_or_else(|error| panic!("source-order runtime: {error}"));
            let loop_handle = tokio::task::spawn_local(mediator.run(runtime));
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id: SessionId::new("source-order"),
                    prompt: PromptEnvelope::prepared(vec!["prompt".to_owned()], None),
                })
                .await
                .unwrap_or_else(|error| panic!("source-order prompt: {error}"));

            let mut events = Vec::new();
            loop {
                let event = tokio::time::timeout(Duration::from_secs(5), source_rx.recv())
                    .await
                    .unwrap_or_else(|_| panic!("source-order event timed out"))
                    .unwrap_or_else(|| panic!("source-order channel closed"));
                let finished = matches!(event.kind(), SourceTurnEventKind::Finished { .. });
                events.push(event);
                if finished {
                    break;
                }
            }
            let assistant = events.iter().position(|event| {
                matches!(
                    event.kind(),
                    SourceTurnEventKind::AssistantFragment { text, .. }
                        if text == "queued assistant"
                )
            });
            let finished = events.iter().position(|event| {
                matches!(
                    event.kind(),
                    SourceTurnEventKind::Finished {
                        disposition: SourceTurnDisposition::Completed,
                        ..
                    }
                )
            });
            assert!(
                assistant.is_some_and(|assistant| finished.is_some_and(|end| assistant < end)),
                "queued source frame must be observed before terminal: {events:?}"
            );
            sender
                .send(BridgeCommand::Shutdown)
                .await
                .unwrap_or_else(|error| panic!("source-order shutdown: {error}"));
            loop_handle
                .await
                .unwrap_or_else(|error| panic!("source-order mediator join: {error}"))
                .unwrap_or_else(|error| panic!("source-order mediator: {error}"));
        })
        .await;
}

#[cfg(feature = "kas")]
#[tokio::test]
async fn kas_runtime_preserves_callbacks_commands_and_wire_terminal_order() {
    use std::rc::Rc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use agent_client_protocol::schema::v1 as acp;
    use agent_client_protocol::{Agent, Client, ConnectionTo};

    use super::super::{DomainConfig, DomainMediator};
    use crate::protocol::bridge::create_channel_pair;
    use crate::protocol::engine::KasEngine;
    use crate::protocol::sdk_runtime::{SdkRuntime, StageChain};
    use crate::types::hook::HookId;
    use crate::types::{
        BridgeCommand, Notification, SessionId, SourceTurnEventKind, StopReason,
        WorkflowCommandOutcome, WorkflowOp,
    };

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let directory = tempfile::tempdir()
                .unwrap_or_else(|error| panic!("KAS callback fixture tempdir: {error}"));
            let path = directory.path().join("callback.txt");
            std::fs::write(&path, "host callback fixture\n")
                .unwrap_or_else(|error| panic!("KAS callback fixture write: {error}"));
            let callback_completed = Arc::new(AtomicBool::new(false));
            let observed = Arc::clone(&callback_completed);
            let received_methods = Arc::new(Mutex::new(Vec::new()));
            let agent_methods = Arc::clone(&received_methods);
            let prompt_release = Arc::new(tokio::sync::Notify::new());
            let agent_prompt_release = Arc::clone(&prompt_release);
            let prompt_responded = Arc::new(AtomicBool::new(false));
            let agent_prompt_responded = Arc::clone(&prompt_responded);
            let agent = Agent
                .builder()
                .name("kas-callback-test-agent")
                .on_receive_request(
                    async move |request: acp::InitializeRequest,
                                responder,
                                connection: ConnectionTo<Client>| {
                        let callback_connection = connection.clone();
                        let callback_path = path.clone();
                        let completed = Arc::clone(&callback_completed);
                        connection.spawn(async move {
                            let response = callback_connection
                                .send_request(acp::ReadTextFileRequest::new(
                                    acp::SessionId::new("kas-callback-session"),
                                    callback_path,
                                ))
                                .block_task()
                                .await?;
                            if response.content != "host callback fixture\n" {
                                return Err(agent_client_protocol::Error::internal_error()
                                    .data("KAS read callback returned the wrong content"));
                            }
                            completed.store(true, Ordering::Release);
                            let initialize = serde_json::from_value(serde_json::json!({
                                "protocolVersion": request.protocol_version,
                                "agentCapabilities": {
                                    "loadSession": true,
                                    "_meta": {"kiro": {}}
                                }
                            }))
                            .map_err(|error| {
                                agent_client_protocol::Error::internal_error()
                                    .data(format!("KAS initialize fixture: {error}"))
                            })?;
                            responder.respond(initialize)
                        })?;
                        Ok(())
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_request(
                    async move |_request: acp::NewSessionRequest, responder, _connection| {
                        responder.respond(acp::NewSessionResponse::new("sess_kas-runtime"))
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_request(
                    async move |request: acp::PromptRequest,
                                responder,
                                connection: ConnectionTo<Client>| {
                        let terminal: acp::SessionNotification =
                            serde_json::from_value(serde_json::json!({
                                "sessionId": request.session_id.to_string(),
                                "update": {
                                    "sessionUpdate": "session_info_update",
                                    "_meta": {
                                        "kiro": {
                                            "kind": "turn_end",
                                            "stopReason": "end_turn"
                                        }
                                    }
                                }
                            }))
                            .map_err(|error| {
                                agent_client_protocol::Error::internal_error()
                                    .data(format!("KAS terminal fixture: {error}"))
                            })?;
                        connection.send_notification(terminal)?;
                        agent_prompt_release.notified().await;
                        agent_prompt_responded.store(true, Ordering::Release);
                        responder.respond(acp::PromptResponse::new(acp::StopReason::EndTurn))
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_request(
                    async move |request: UntypedMessage, responder, _connection| {
                        agent_methods
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .push(request.method().to_owned());
                        match request.method() {
                            "_kiro/hooks/list" => {
                                responder.respond(serde_json::json!({"hooks": []}))
                            }
                            "_kiro/hooks/setEnabled" => {
                                assert_eq!(request.params["enabled"], false);
                                responder.respond(serde_json::json!({"success": true}))
                            }
                            "_kiro/workflow/listRecipes" => {
                                responder.respond(serde_json::json!({"recipes": []}))
                            }
                            _ => responder.respond_with_error(
                                agent_client_protocol::Error::method_not_found(),
                            ),
                        }
                    },
                    agent_client_protocol::on_receive_request!(),
                );

            let (handle, bridge) = create_channel_pair();
            let (sender, mut notification_rx, _permission_rx, mut source_rx, _completion_rx) =
                handle.split();
            let config = DomainConfig {
                engine: Rc::new(KasEngine::default()),
                cwd: directory.path().to_path_buf(),
                present_as: None,
                stall_threshold: Duration::from_secs(30),
                host_shell: None,
            };
            let (mediator, channels) = DomainMediator::new(config, bridge)
                .unwrap_or_else(|error| panic!("KAS callback mediator: {error}"));
            let runtime = SdkRuntime::start_for_test(agent, channels, StageChain::default())
                .await
                .unwrap_or_else(|error| panic!("KAS callback runtime: {error}"));
            let loop_handle = tokio::task::spawn_local(mediator.run(runtime));

            tokio::time::timeout(Duration::from_secs(5), async {
                while !observed.load(Ordering::Acquire) {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap_or_else(|_| panic!("KAS callback did not complete during initialize"));
            sender
                .send(BridgeCommand::ListKasHooks {
                    session_id: SessionId::new("kas-callback-session"),
                    workspace_paths: vec![directory.path().to_path_buf()],
                })
                .await
                .unwrap_or_else(|error| panic!("KAS hooks command send: {error}"));
            let changed = tokio::time::timeout(Duration::from_secs(5), notification_rx.recv())
                .await
                .unwrap_or_else(|_| panic!("KAS hooks changed notification timed out"))
                .unwrap_or_else(|| panic!("KAS hooks changed channel closed"));
            assert!(matches!(
                changed.notification,
                Notification::HooksChanged { ref hooks } if hooks.is_empty()
            ));
            let executed = tokio::time::timeout(Duration::from_secs(5), notification_rx.recv())
                .await
                .unwrap_or_else(|_| panic!("KAS hooks command notification timed out"))
                .unwrap_or_else(|| panic!("KAS hooks command channel closed"));
            assert!(matches!(
                executed.notification,
                Notification::CommandExecuted { ref command, .. } if command == "hooks"
            ));
            assert_eq!(
                *received_methods
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
                ["_kiro/hooks/list"]
            );
            sender
                .send(BridgeCommand::SetKasHookEnabled {
                    session_id: SessionId::new("kas-callback-session"),
                    hook_id: HookId::new("/oracle/hook.json#hook-0"),
                    enabled: false,
                    workspace_paths: vec![directory.path().to_path_buf()],
                })
                .await
                .unwrap_or_else(|error| panic!("KAS set-hook command send: {error}"));
            for expected in ["hooks changed", "hooks executed"] {
                let notification =
                    tokio::time::timeout(Duration::from_secs(5), notification_rx.recv())
                        .await
                        .unwrap_or_else(|_| panic!("KAS {expected} notification timed out"))
                        .unwrap_or_else(|| panic!("KAS {expected} channel closed"));
                match expected {
                    "hooks changed" => assert!(matches!(
                        notification.notification,
                        Notification::HooksChanged { ref hooks } if hooks.is_empty()
                    )),
                    "hooks executed" => assert!(matches!(
                        notification.notification,
                        Notification::CommandExecuted { ref command, .. } if command == "hooks"
                    )),
                    other => panic!("unexpected KAS hook expectation: {other}"),
                }
            }
            sender
                .send(BridgeCommand::Workflow {
                    session_id: SessionId::new("kas-callback-session"),
                    workspace_paths: vec![directory.path().to_path_buf()],
                    op: WorkflowOp::ListRecipes,
                })
                .await
                .unwrap_or_else(|error| panic!("KAS workflow command send: {error}"));
            let workflow = tokio::time::timeout(Duration::from_secs(5), notification_rx.recv())
                .await
                .unwrap_or_else(|_| panic!("KAS workflow command notification timed out"))
                .unwrap_or_else(|| panic!("KAS workflow command channel closed"));
            assert!(matches!(
                workflow.notification,
                Notification::WorkflowCommand(WorkflowCommandOutcome::Recipes {
                    ref recipes,
                    skipped: 0,
                }) if recipes.is_empty()
            ));
            assert_eq!(
                *received_methods
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
                [
                    "_kiro/hooks/list",
                    "_kiro/hooks/setEnabled",
                    "_kiro/hooks/list",
                    "_kiro/workflow/listRecipes",
                ]
            );
            sender
                .send(BridgeCommand::NewSession {
                    cwd: directory.path().to_path_buf(),
                })
                .await
                .unwrap_or_else(|error| panic!("KAS new-session command send: {error}"));
            let usage = tokio::time::timeout(Duration::from_secs(5), notification_rx.recv())
                .await
                .unwrap_or_else(|_| panic!("KAS usage-session notification timed out"))
                .unwrap_or_else(|| panic!("KAS usage-session channel closed"));
            assert!(
                matches!(
                    &usage.notification,
                    Notification::UsageSessionStarted { .. }
                ),
                "expected KAS UsageSessionStarted, got {:?}",
                usage.notification
            );
            let created = tokio::time::timeout(Duration::from_secs(5), notification_rx.recv())
                .await
                .unwrap_or_else(|_| panic!("KAS session-created notification timed out"))
                .unwrap_or_else(|| panic!("KAS session-created channel closed"));
            let session_id = match created.notification {
                Notification::SessionCreated { session_id, .. } => session_id,
                other => panic!("expected KAS SessionCreated, got {other:?}"),
            };
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id,
                    prompt: crate::types::PromptEnvelope::prepared(
                        vec!["prove KAS wire terminal".to_owned()],
                        None,
                    ),
                })
                .await
                .unwrap_or_else(|error| panic!("KAS prompt command send: {error}"));
            let started = tokio::time::timeout(Duration::from_secs(5), source_rx.recv())
                .await
                .unwrap_or_else(|_| panic!("KAS source-start notification timed out"))
                .unwrap_or_else(|| panic!("KAS source-start channel closed"));
            assert!(matches!(
                started.kind(),
                SourceTurnEventKind::Started { .. }
            ));
            let disposition = tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let event = source_rx
                        .recv()
                        .await
                        .unwrap_or_else(|| panic!("KAS source-finish channel closed"));
                    if let SourceTurnEventKind::Finished { disposition, .. } = event.kind() {
                        break *disposition;
                    }
                }
            })
            .await
            .unwrap_or_else(|_| panic!("KAS source-finish notification timed out"));
            assert_eq!(disposition, crate::types::SourceTurnDisposition::Completed);
            let terminal = tokio::time::timeout(Duration::from_secs(5), notification_rx.recv())
                .await
                .unwrap_or_else(|_| panic!("KAS wire terminal notification timed out"))
                .unwrap_or_else(|| panic!("KAS wire terminal channel closed"));
            assert!(matches!(
                terminal.notification,
                Notification::TurnCompleted {
                    stop_reason: StopReason::EndTurn
                }
            ));
            assert!(
                !prompt_responded.load(Ordering::Acquire),
                "wire terminal and source finish must precede the prompt RPC response"
            );
            prompt_release.notify_one();
            tokio::time::timeout(Duration::from_secs(5), async {
                while !prompt_responded.load(Ordering::Acquire) {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap_or_else(|_| panic!("KAS prompt response did not finish"));
            tokio::time::sleep(Duration::from_millis(100)).await;
            assert!(
                notification_rx.try_recv().is_err(),
                "prompt RPC completion must not duplicate the KAS wire terminal"
            );
            sender
                .send(BridgeCommand::Shutdown)
                .await
                .unwrap_or_else(|error| panic!("KAS callback shutdown send: {error}"));
            loop_handle
                .await
                .unwrap_or_else(|error| panic!("KAS callback loop join: {error}"))
                .unwrap_or_else(|error| panic!("KAS callback loop: {error}"));
        })
        .await;
}

/// cyril PR #115 review, finding 3: a FOREIGN session's `turn_end` forwarded
/// mid-turn (Disposition::Forward — a workflow-step or subagent peer ending
/// its own turn) must NOT finalize the MAIN turn's source capture. Before the
/// fix, any forwarded `TurnCompleted` took the observer's single active slot
/// with the peer's stop reason, so every later main-turn fragment was
/// silently dropped and the real terminal's `finish` no-oped.
#[cfg(feature = "kas")]
#[tokio::test]
async fn foreign_turn_end_does_not_finalize_main_source_capture() {
    use std::rc::Rc;
    use std::time::Duration;

    use agent_client_protocol::schema::v1 as acp;
    use agent_client_protocol::{Agent, Client, ConnectionTo};

    use super::super::{DomainConfig, DomainMediator};
    use crate::protocol::bridge::create_channel_pair;
    use crate::protocol::engine::KasEngine;
    use crate::protocol::sdk_runtime::{SdkRuntime, StageChain};
    use crate::types::{BridgeCommand, SourceTurnDisposition, SourceTurnEventKind, StopReason};

    fn turn_end(session: &str) -> acp::SessionNotification {
        match serde_json::from_value(serde_json::json!({
            "sessionId": session,
            "update": {
                "sessionUpdate": "session_info_update",
                "_meta": {"kiro": {"kind": "turn_end", "stopReason": "end_turn"}}
            }
        })) {
            Ok(notification) => notification,
            Err(error) => panic!("turn_end fixture: {error}"),
        }
    }

    fn chunk(session: &str, text: &str) -> acp::SessionNotification {
        match serde_json::from_value(serde_json::json!({
            "sessionId": session,
            "update": {
                "sessionUpdate": "agent_message_chunk",
                "content": {"type": "text", "text": text}
            }
        })) {
            Ok(notification) => notification,
            Err(error) => panic!("chunk fixture: {error}"),
        }
    }

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let directory = tempfile::tempdir()
                .unwrap_or_else(|error| panic!("foreign-terminal tempdir: {error}"));
            let agent = Agent
                .builder()
                .name("foreign-terminal-test-agent")
                .on_receive_request(
                    async move |request: acp::InitializeRequest, responder, _connection| {
                        let initialize = serde_json::from_value(serde_json::json!({
                            "protocolVersion": request.protocol_version,
                            "agentCapabilities": {
                                "loadSession": true,
                                "_meta": {"kiro": {}}
                            }
                        }))
                        .map_err(|error| {
                            agent_client_protocol::Error::internal_error()
                                .data(format!("initialize fixture: {error}"))
                        })?;
                        responder.respond(initialize)
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_request(
                    async move |_request: acp::NewSessionRequest, responder, _connection| {
                        responder.respond(acp::NewSessionResponse::new("sess_foreign-terminal"))
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_request(
                    async move |request: acp::PromptRequest,
                                responder,
                                connection: ConnectionTo<Client>| {
                        let main = request.session_id.to_string();
                        connection.send_notification(chunk(&main, "before"))?;
                        // A peer session ends its turn while the main turn
                        // is still streaming.
                        connection.send_notification(turn_end("sess_peer-step"))?;
                        connection.send_notification(chunk(&main, "after"))?;
                        connection.send_notification(turn_end(&main))?;
                        responder.respond(acp::PromptResponse::new(acp::StopReason::EndTurn))
                    },
                    agent_client_protocol::on_receive_request!(),
                );

            let (handle, bridge) = create_channel_pair();
            let (sender, mut notification_rx, _permission_rx, mut source_rx, _completion_rx) =
                handle.split();
            let config = DomainConfig {
                engine: Rc::new(KasEngine::default()),
                cwd: directory.path().to_path_buf(),
                present_as: None,
                stall_threshold: Duration::from_secs(30),
                host_shell: None,
            };
            let (mediator, channels) = DomainMediator::new(config, bridge)
                .unwrap_or_else(|error| panic!("foreign-terminal mediator: {error}"));
            let runtime = SdkRuntime::start_for_test(agent, channels, StageChain::default())
                .await
                .unwrap_or_else(|error| panic!("foreign-terminal runtime: {error}"));
            let loop_handle = tokio::task::spawn_local(mediator.run(runtime));

            sender
                .send(BridgeCommand::NewSession {
                    cwd: directory.path().to_path_buf(),
                })
                .await
                .unwrap_or_else(|error| panic!("foreign-terminal session send: {error}"));
            let session_id = loop {
                let routed = tokio::time::timeout(Duration::from_secs(5), notification_rx.recv())
                    .await
                    .unwrap_or_else(|_| panic!("foreign-terminal session timeout"))
                    .unwrap_or_else(|| panic!("foreign-terminal session channel closed"));
                if let crate::types::Notification::SessionCreated { session_id, .. } =
                    routed.notification
                {
                    break session_id;
                }
            };
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id: session_id.clone(),
                    prompt: crate::types::PromptEnvelope::prepared(
                        vec!["stream across a peer terminal".to_owned()],
                        None,
                    ),
                })
                .await
                .unwrap_or_else(|error| panic!("foreign-terminal prompt send: {error}"));

            // Wait for the MAIN turn's completion on the notification side.
            loop {
                let routed = tokio::time::timeout(Duration::from_secs(5), notification_rx.recv())
                    .await
                    .unwrap_or_else(|_| panic!("foreign-terminal turn timeout"))
                    .unwrap_or_else(|| panic!("foreign-terminal channel closed"));
                if matches!(
                    routed.notification,
                    crate::types::Notification::TurnCompleted {
                        stop_reason: StopReason::EndTurn
                    }
                ) {
                    break;
                }
            }

            sender
                .send(BridgeCommand::Shutdown)
                .await
                .unwrap_or_else(|error| panic!("foreign-terminal shutdown send: {error}"));
            loop_handle
                .await
                .unwrap_or_else(|error| panic!("foreign-terminal loop join: {error}"))
                .unwrap_or_else(|error| panic!("foreign-terminal loop: {error}"));

            // Drain the capture stream: both fragments must precede exactly
            // one Finished, and its disposition is the MAIN turn's.
            let mut fragments = Vec::new();
            let mut finishes = Vec::new();
            while let Ok(event) = source_rx.try_recv() {
                match event.kind() {
                    SourceTurnEventKind::AssistantFragment { text, .. } => {
                        fragments.push(text.clone());
                    }
                    SourceTurnEventKind::Finished { disposition, .. } => {
                        finishes.push(*disposition);
                        assert_eq!(
                            fragments,
                            ["before", "after"],
                            "a forwarded peer terminal truncated main-turn capture"
                        );
                    }
                    _ => {}
                }
            }
            assert_eq!(
                finishes,
                [SourceTurnDisposition::Completed],
                "exactly one Finished, owned by the main turn"
            );
        })
        .await;
}

/// cyril-dn91 wiring fence (PR #115 review, finding 13): a V2-BOUND bridge on
/// a kas build must REFUSE inbound `_kiro` host requests end-to-end — the
/// request travels the real path (client catch-all → host channel →
/// `dispatch_extension` → the fenced `supports()` gate) and comes back
/// -32601. The unit half (`probe_dn91`) only pairs derived bits; this drives
/// the wire, the defect class where a v2-bound build answered KAS host
/// callbacks (executeHook ran arbitrary commands).
#[cfg(feature = "kas")]
#[tokio::test]
async fn v2_bound_bridge_refuses_inbound_kiro_host_requests_end_to_end() {
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use agent_client_protocol::schema::v1 as acp;
    use agent_client_protocol::{Agent, Client, ConnectionTo, UntypedMessage};

    use super::super::{DomainConfig, DomainMediator};
    use crate::protocol::bridge::create_channel_pair;
    use crate::protocol::engine::V2Engine;
    use crate::protocol::sdk_runtime::{SdkRuntime, StageChain};
    use crate::types::BridgeCommand;

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let refused = Arc::new(AtomicUsize::new(0));
            let observed = Arc::clone(&refused);
            let agent = Agent
                .builder()
                .name("dn91-wiring-test-agent")
                .on_receive_request(
                    async move |request: acp::InitializeRequest,
                                responder,
                                connection: ConnectionTo<Client>| {
                        let probe_connection = connection.clone();
                        let refused = Arc::clone(&refused);
                        connection.spawn(async move {
                            for method in [
                                "_kiro/auth/getAccessToken",
                                "_kiro/hooks/executeHook",
                                "_kiro/fs/stat",
                            ] {
                                let probe =
                                    UntypedMessage::new(method, serde_json::json!({}))?;
                                match probe_connection.send_request(probe).block_task().await {
                                    Err(error)
                                        if error.code
                                            == agent_client_protocol::ErrorCode::MethodNotFound =>
                                    {
                                        refused.fetch_add(1, Ordering::Release);
                                    }
                                    Ok(response) => {
                                        return Err(
                                            agent_client_protocol::Error::internal_error().data(
                                                format!(
                                                    "{method} unexpectedly answered on a v2-bound bridge: {response}"
                                                ),
                                            ),
                                        );
                                    }
                                    Err(error) => {
                                        return Err(
                                            agent_client_protocol::Error::internal_error().data(
                                                format!("{method} wrong refusal: {error}"),
                                            ),
                                        );
                                    }
                                }
                            }
                            responder
                                .respond(acp::InitializeResponse::new(request.protocol_version))
                        })?;
                        Ok(())
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_request(
                    async move |_request: acp::NewSessionRequest, responder, _connection| {
                        responder.respond(acp::NewSessionResponse::new("dn91-wire"))
                    },
                    agent_client_protocol::on_receive_request!(),
                );

            let directory = tempfile::tempdir()
                .unwrap_or_else(|error| panic!("dn91 wiring tempdir: {error}"));
            let (handle, bridge) = create_channel_pair();
            let (sender, mut notification_rx, _permission_rx, _source_rx, _completion_rx) =
                handle.split();
            let config = DomainConfig {
                engine: Rc::new(V2Engine),
                cwd: directory.path().to_path_buf(),
                present_as: None,
                stall_threshold: Duration::from_secs(30),
                host_shell: None,
            };
            let (mediator, channels) = DomainMediator::new(config, bridge)
                .unwrap_or_else(|error| panic!("dn91 wiring mediator: {error}"));
            let runtime = SdkRuntime::start_for_test(agent, channels, StageChain::default())
                .await
                .unwrap_or_else(|error| panic!("dn91 wiring runtime: {error}"));
            let loop_handle = tokio::task::spawn_local(mediator.run(runtime));

            sender
                .send(BridgeCommand::NewSession {
                    cwd: directory.path().to_path_buf(),
                })
                .await
                .unwrap_or_else(|error| panic!("dn91 wiring session send: {error}"));
            loop {
                let routed = tokio::time::timeout(Duration::from_secs(5), notification_rx.recv())
                    .await
                    .unwrap_or_else(|_| panic!("dn91 wiring session timeout"))
                    .unwrap_or_else(|| panic!("dn91 wiring channel closed"));
                if matches!(
                    routed.notification,
                    crate::types::Notification::SessionCreated { .. }
                ) {
                    break;
                }
            }
            assert_eq!(
                observed.load(Ordering::Acquire),
                3,
                "every inbound _kiro host request must be refused before initialize completes"
            );

            sender
                .send(BridgeCommand::Shutdown)
                .await
                .unwrap_or_else(|error| panic!("dn91 wiring shutdown send: {error}"));
            loop_handle
                .await
                .unwrap_or_else(|error| panic!("dn91 wiring loop join: {error}"))
                .unwrap_or_else(|error| panic!("dn91 wiring loop: {error}"));
        })
        .await;
}
