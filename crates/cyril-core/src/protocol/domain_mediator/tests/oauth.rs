use std::rc::Rc;
use std::time::Duration;

use agent_client_protocol::schema::v1 as acp;
use agent_client_protocol::{Agent, Client, ConnectionTo, UntypedMessage};
use serde_json::json;

use super::super::{DomainChannels, DomainConfig, DomainMediator, DomainWork};
use super::external_url_wire::external_url_round_trip;
use crate::protocol::bridge::create_channel_pair;
use crate::protocol::engine::KasEngine;
use crate::protocol::kas::callbacks::{DispatchCtx, HostCallback, OpenExternalUrlArgs};
use crate::protocol::sdk_runtime::{SdkRuntime, StageChain};
use crate::protocol::source_observer::IngressTracker;
use crate::test_support::must_succeed;
use crate::types::{BridgeCommand, Notification};

#[tokio::test]
async fn external_url_callback_preserves_exact_global_payload_and_rejects_malformed_params() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let url = " https://login.invalid/authorize?state=exact%2Fstate#fragment\u{1b}[31m ";
            let (replies, notifications) = external_url_round_trip(
                Rc::new(KasEngine::default()),
                vec![json!({"url": url})],
                false,
            )
            .await;
            assert_eq!(replies.len(), 1);
            assert!(replies[0].is_ok());
            assert_eq!(notifications.len(), 1);
            let routed = &notifications[0];
            assert!(routed.session_id.is_none());
            assert!(routed.turn.is_none());
            assert!(routed.origin.is_none());
            assert!(matches!(&routed.notification,
                Notification::ExternalUrlRequested { url: received } if received == url));

            let (replies, notifications) = external_url_round_trip(
                Rc::new(KasEngine::default()),
                vec![
                    json!({}),
                    json!({"url": null}),
                    json!({"url": 7}),
                    json!({"url": {"private": "do-not-reflect-this-query"}}),
                    json!({"url": ""}),
                    json!({"url": " \n\t"}),
                    json!([]),
                ],
                false,
            )
            .await;
            assert_eq!(replies.len(), 7);
            for reply in replies {
                let Err(error) = reply else {
                    panic!("malformed URL shape must be an explicit parameter error");
                };
                assert_eq!(error.code, agent_client_protocol::ErrorCode::InvalidParams);
                assert!(!format!("{error:?}").contains("do-not-reflect-this-query"));
            }
            assert!(notifications.is_empty(), "no fabricated URL events");
        })
        .await;
}

#[tokio::test]
async fn external_url_success_waits_for_bounded_domain_acceptance() {
    let (channels, mut domain_rx, _host_rx) = must_succeed(
        DomainChannels::new(IngressTracker::new()),
        "bounded URL channels",
    );
    while channels.remaining_capacity() > 0 {
        must_succeed(
            channels
                .enqueue(DomainWork::Routed(
                    Notification::SystemNotify {
                        level: crate::types::event::SystemNotifyLevel::Info,
                        message: "occupy capacity".into(),
                    }
                    .into(),
                ))
                .await,
            "fill the bounded domain path",
        );
    }
    let ctx = DispatchCtx {
        notify_tx: channels,
        terminals: Rc::new(crate::protocol::kas::terminal_io::TerminalRegistry::new(
            None,
        )),
        hooks: None,
        hook_ops: crate::protocol::kas::hooks::HookOps::default(),
        cwd: std::path::PathBuf::new(),
        kas_token: None,
    };
    let (reply, mut response_rx) = tokio::sync::oneshot::channel();
    let callback = HostCallback::OpenExternalUrl {
        args: must_succeed(
            OpenExternalUrlArgs::parse(&json!({"url": "https://login.invalid/exact"})),
            "valid URL callback params",
        ),
        reply,
    };
    let delivery = crate::protocol::kas::callbacks::dispatch(callback, &ctx);
    tokio::pin!(delivery);
    assert!(futures_util::poll!(delivery.as_mut()).is_pending());
    assert!(matches!(
        response_rx.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    ));
    must_succeed(domain_rx.try_recv(), "free one bounded domain slot");
    delivery.await;
    let response = must_succeed(
        must_succeed(response_rx.await, "URL response delivered"),
        "URL accepted",
    );
    assert_eq!(
        must_succeed(
            serde_json::from_str::<serde_json::Value>(response.0.get()),
            "URL response JSON"
        ),
        json!({"success": true})
    );
    let mut delivered = 0;
    while let Ok(work) = domain_rx.try_recv() {
        if let DomainWork::Routed(routed) = work
            && let Notification::ExternalUrlRequested { url } = routed.notification
        {
            assert!(routed.session_id.is_none());
            assert_eq!(url, "https://login.invalid/exact");
            delivered += 1;
        }
    }
    assert_eq!(delivered, 1);
}

async fn request_url(
    connection: &ConnectionTo<Client>,
    url: &str,
) -> agent_client_protocol::Result<()> {
    let reply = connection
        .send_request(UntypedMessage::new(
            "_kiro/openExternalUrl",
            json!({"url": url}),
        )?)
        .block_task()
        .await?;
    if reply != json!({"success": true}) {
        return Err(agent_client_protocol::Error::internal_error()
            .data("incorrect URL delivery acknowledgement"));
    }
    Ok(())
}

fn session_info(
    kiro: serde_json::Value,
) -> agent_client_protocol::Result<acp::SessionNotification> {
    serde_json::from_value(json!({
        "sessionId": "sess_oauth-runtime-session",
        "update": {"sessionUpdate": "session_info_update", "_meta": {"kiro": kiro}},
    }))
    .map_err(|error| agent_client_protocol::Error::internal_error().data(error.to_string()))
}

#[tokio::test]
async fn kas_sdk_url_callbacks_and_mcp_errors_survive_initialize_and_new_session() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let session_bound = std::sync::Arc::new(tokio::sync::Notify::new());
            let agent_session_bound = std::sync::Arc::clone(&session_bound);
            let agent = Agent
                .builder()
                .name("oauth-lifecycle-test-agent")
                .on_receive_request(
                    async move |request: acp::InitializeRequest,
                                responder,
                                connection: ConnectionTo<Client>| {
                        let request_json = serde_json::to_value(&request).map_err(|error| {
                            agent_client_protocol::Error::internal_error().data(error.to_string())
                        })?;
                        assert_eq!(
                            request_json.pointer("/clientCapabilities/_meta/kiro/openExternalUrl"),
                            Some(&json!(true))
                        );
                        assert!(
                            request_json
                                .pointer("/clientCapabilities/_meta/kiro/secretStorage")
                                .is_none()
                        );
                        let callback_connection = connection.clone();
                        connection.spawn(async move {
                            request_url(
                                &callback_connection,
                                "https://login.invalid/during-initialize",
                            )
                            .await?;
                            responder.respond(
                                serde_json::from_value(json!({
                                    "protocolVersion": request.protocol_version,
                                    "agentCapabilities": {"_meta": {"kiro": {}}},
                                }))
                                .map_err(|error| {
                                    agent_client_protocol::Error::internal_error()
                                        .data(error.to_string())
                                })?,
                            )
                        })?;
                        Ok(())
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_request(
                    async move |_request: acp::NewSessionRequest,
                                responder,
                                connection: ConnectionTo<Client>| {
                        let callback_connection = connection.clone();
                        let session_bound = std::sync::Arc::clone(&agent_session_bound);
                        connection.spawn(async move {
                            request_url(&callback_connection, "https://login.invalid/during-new")
                                .await?;
                            responder.respond(acp::NewSessionResponse::new(
                                "sess_oauth-runtime-session",
                            ))?;
                            request_url(&callback_connection, "https://login.invalid/after-new")
                                .await?;
                            // Wait for the real SessionStarted outcome to bind main
                            // before emitting a main-session turn bracket.
                            session_bound.notified().await;
                            callback_connection
                                .send_notification(session_info(json!({"kind": "turn_start"}))?)?;
                            callback_connection.send_notification(session_info(json!({
                                "kind": "display_error", "errorType": "mcp_connection_error",
                                "message": "MCP server 'flat' failed to connect.",
                            }))?)?;
                            callback_connection.send_notification(session_info(json!({
                                "kind": "display_error", "errorType": "mcp_connection_error",
                                "message": "  MCP server 'mirror' requires authorization.  ",
                                "displayError": {"errorType": "mcp_connection_error",
                                    "message": "  MCP server 'mirror' requires authorization.  "},
                            }))?)?;
                            // An ordinary message after both failures proves the SDK/domain
                            // stream remains usable without a synthetic terminal/disconnect.
                            let after: acp::SessionNotification = serde_json::from_value(json!({
                                "sessionId": "sess_oauth-runtime-session",
                                "update": {"sessionUpdate": "agent_message_chunk",
                                    "content": {"type": "text", "text": "after MCP errors"}},
                            }))
                            .map_err(|error| {
                                agent_client_protocol::Error::internal_error()
                                    .data(error.to_string())
                            })?;
                            callback_connection.send_notification(after)?;
                            Ok(())
                        })?;
                        Ok(())
                    },
                    agent_client_protocol::on_receive_request!(),
                );
            let directory = must_succeed(tempfile::tempdir(), "OAuth runtime workspace");
            let (handle, bridge) = create_channel_pair();
            let (sender, mut notifications, _permissions, _source, _completion) = handle.split();
            let (mediator, channels) = must_succeed(
                DomainMediator::new(
                    DomainConfig {
                        engine: Rc::new(KasEngine::default()),
                        cwd: directory.path().to_path_buf(),
                        present_as: None,
                        stall_threshold: Duration::from_secs(30),
                        host_shell: None,
                        kas_token: None,
                    },
                    bridge,
                ),
                "OAuth runtime mediator",
            );
            let runtime = must_succeed(
                SdkRuntime::start_for_test(agent, channels, StageChain::default()).await,
                "OAuth SDK runtime",
            );
            let loop_handle = tokio::task::spawn_local(mediator.run(runtime));
            must_succeed(
                sender
                    .send(BridgeCommand::NewSession {
                        cwd: directory.path().to_path_buf(),
                    })
                    .await,
                "request new OAuth test session",
            );
            let mut urls = Vec::new();
            let mut errors = Vec::new();
            let mut created = false;
            let mut started = false;
            loop {
                let routed = must_succeed(
                    tokio::time::timeout(Duration::from_secs(5), notifications.recv()).await,
                    "OAuth runtime event deadline",
                )
                .unwrap_or_else(|| panic!("OAuth runtime event channel closed"));
                match routed.notification {
                    Notification::ExternalUrlRequested { url } => {
                        assert!(routed.session_id.is_none());
                        urls.push(url);
                    }
                    Notification::McpConnectionError { message } => {
                        assert_eq!(
                            routed
                                .session_id
                                .as_ref()
                                .map(crate::types::SessionId::as_str),
                            Some("sess_oauth-runtime-session")
                        );
                        assert!(started, "error arrives during a still-active turn");
                        errors.push(message);
                    }
                    Notification::SessionCreated { .. } => {
                        created = true;
                        session_bound.notify_one();
                    }
                    Notification::TurnStarted => started = true,
                    Notification::AgentMessage(message) if message.text == "after MCP errors" => {
                        break;
                    }
                    Notification::TurnCompleted { .. }
                    | Notification::BridgeDisconnected { .. } => {
                        panic!("display_error must not imply completion or disconnect")
                    }
                    _ => {}
                }
            }
            assert!(created);
            assert!(started);
            assert_eq!(
                urls,
                [
                    "https://login.invalid/during-initialize",
                    "https://login.invalid/during-new",
                    "https://login.invalid/after-new"
                ]
            );
            assert_eq!(
                errors,
                [
                    "MCP server 'flat' failed to connect.",
                    "  MCP server 'mirror' requires authorization.  "
                ]
            );
            must_succeed(
                sender.send(BridgeCommand::Shutdown).await,
                "OAuth runtime shutdown",
            );
            must_succeed(
                must_succeed(loop_handle.await, "OAuth mediator join"),
                "OAuth mediator shutdown",
            );
        })
        .await;
}
