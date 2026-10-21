//! Root-start admission through the existing SDK, not a private guard API.

use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use agent_client_protocol::{Agent, Client, ConnectionTo, UntypedMessage, schema::v1 as acp};
use serde_json::json;
use tokio::sync::{Notify, mpsc};

use super::super::{DomainConfig, DomainMediator};
use crate::protocol::bridge::create_channel_pair;
use crate::protocol::engine::KasEngine;
use crate::protocol::sdk_runtime::{SdkRuntime, StageChain};
use crate::test_support::must_succeed;
use crate::types::*;

#[derive(Default)]
struct Gates {
    new_count: AtomicUsize,
    load_count: AtomicUsize,
    reply: Notify,
    fail_load: AtomicBool,
}

async fn stage_first(
    connection: &ConnectionTo<Client>,
    session_id: &str,
) -> agent_client_protocol::Result<()> {
    for (method, params) in [
        (
            "_kiro/mcp/status",
            json!({"sessionId":"sess_foreign","servers":[{"name":"foreign","status":"disabled"}]}),
        ),
        (
            "_kiro/mcp/status",
            json!({"sessionId":session_id,"servers":[{"name":"first-only","status":"connecting"}]}),
        ),
        (
            "_kiro/mcp/governance_disabled",
            json!({"sessionId":session_id,"reason":"api_failure"}),
        ),
        (
            "_kiro/system/notify",
            json!({"level":"info","message":"first MCP state staged"}),
        ),
    ] {
        connection.send_notification(UntypedMessage::new(method, params)?)?;
    }
    Ok(())
}

async fn next(rx: &mut mpsc::Receiver<RoutedNotification>) -> RoutedNotification {
    must_succeed(
        tokio::time::timeout(Duration::from_secs(2), rx.recv()).await,
        "admission notification deadline",
    )
    .unwrap_or_else(|| panic!("admission notification stream closed"))
}

async fn scenario(first_load: bool, competitor_load: bool, failed_load: bool) {
    tokio::task::LocalSet::new()
        .run_until(async move {
            let gates = Arc::new(Gates::default());
            gates.fail_load.store(failed_load, Ordering::SeqCst);
            let new_gates = Arc::clone(&gates);
            let load_gates = Arc::clone(&gates);
            let agent = Agent
                .builder()
                .name("nk4o-root-admission")
                .on_receive_request(
                    async move |request: acp::InitializeRequest, responder, _connection| {
                        responder.respond(
                            serde_json::from_value::<acp::InitializeResponse>(json!({
                                "protocolVersion":request.protocol_version,
                                "agentCapabilities":{"loadSession":true,"_meta":{"kiro":{}}}
                            }))
                            .map_err(|error| {
                                agent_client_protocol::Error::internal_error()
                                    .data(error.to_string())
                            })?,
                        )
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_request(
                    async move |_request: acp::NewSessionRequest,
                                responder,
                                connection: ConnectionTo<Client>| {
                        let state = Arc::clone(&new_gates);
                        let index = state.new_count.fetch_add(1, Ordering::SeqCst);
                        let target = connection.clone();
                        connection.spawn(async move {
                            let session_id = format!("sess_new-{index}");
                            if index == 0 && !first_load {
                                stage_first(&target, &session_id).await?;
                            }
                            state.reply.notified().await;
                            responder.respond(
                                serde_json::from_value::<acp::NewSessionResponse>(json!({
                                    "sessionId":session_id,
                                    "configOptions":[]
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
                    async move |request: acp::LoadSessionRequest,
                                responder,
                                connection: ConnectionTo<Client>| {
                        let state = Arc::clone(&load_gates);
                        let index = state.load_count.fetch_add(1, Ordering::SeqCst);
                        let target = connection.clone();
                        connection.spawn(async move {
                            if index == 0 && first_load {
                                stage_first(&target, &request.session_id.to_string()).await?;
                            }
                            state.reply.notified().await;
                            if state.fail_load.load(Ordering::SeqCst) {
                                responder.respond_with_error(
                                    agent_client_protocol::Error::internal_error()
                                        .data("original load cause"),
                                )
                            } else {
                                responder.respond(
                                    serde_json::from_value::<acp::LoadSessionResponse>(
                                        json!({"configOptions":[]}),
                                    )
                                    .map_err(|error| {
                                        agent_client_protocol::Error::internal_error()
                                            .data(error.to_string())
                                    })?,
                                )
                            }
                        })?;
                        Ok(())
                    },
                    agent_client_protocol::on_receive_request!(),
                );
            let directory = must_succeed(tempfile::tempdir(), "admission workspace");
            let (handle, bridge) = create_channel_pair();
            let (sender, mut rx, _permissions, _sources, _completion) = handle.split();
            let (mediator, channels) = must_succeed(
                DomainMediator::new(
                    DomainConfig {
                        engine: Rc::new(KasEngine::default()),
                        cwd: directory.path().into(),
                        present_as: None,
                        stall_threshold: Duration::from_secs(30),
                        host_shell: None,
                        kas_token: None,
                    },
                    bridge,
                ),
                "admission mediator",
            );
            let runtime = must_succeed(
                SdkRuntime::start_for_test(agent, channels, StageChain::default()).await,
                "admission SDK",
            );
            let join = tokio::task::spawn_local(mediator.run(runtime));
            let command = |load, id| {
                if load {
                    BridgeCommand::LoadSession {
                        session_id: SessionId::new(id),
                    }
                } else {
                    BridgeCommand::NewSession {
                        cwd: directory.path().into(),
                    }
                }
            };
            must_succeed(
                sender.send(command(first_load, "sess_loaded")).await,
                "first root start",
            );
            // Wire FIFO and serial ingress make this a receipt that BOTH MCP
            // updates were staged before the competitor, not a sleep or flag.
            assert!(matches!(next(&mut rx).await.notification,
            Notification::SystemNotify { message, .. } if message == "first MCP state staged"));
            must_succeed(
                sender
                    .send(command(competitor_load, "sess_competitor"))
                    .await,
                "competing root start",
            );
            // The no-active-session control error is a FIFO command receipt even
            // on the unfixed predecessor. RED is a missing rejection assertion,
            // not a deadlock waiting for a notification old code never emits.
            must_succeed(
                sender
                    .send(BridgeCommand::SetMode {
                        mode_id: "admission-barrier".into(),
                    })
                    .await,
                "serial command barrier",
            );
            let mut rejected = false;
            loop {
                match next(&mut rx).await.notification {
                    Notification::BridgeError { operation, message }
                        if operation.starts_with("set_mode") =>
                    {
                        assert!(message.contains("no active session"));
                        break;
                    }
                    Notification::BridgeError { operation, message } => {
                        assert_eq!(
                            operation,
                            if competitor_load {
                                "Load session"
                            } else {
                                "Create session"
                            }
                        );
                        assert!(
                            message.contains("already pending") && message.contains("no second"),
                            "{message}"
                        );
                        assert!(!rejected, "one competing command has one rejection");
                        rejected = true;
                    }
                    other => panic!("unexpected pre-response event: {other:?}"),
                }
            }
            assert!(
                rejected,
                "competing root start must be rejected before any second RPC"
            );
            assert_eq!(
                gates.new_count.load(Ordering::SeqCst),
                usize::from(!first_load)
            );
            assert_eq!(
                gates.load_count.load(Ordering::SeqCst),
                usize::from(first_load)
            );
            gates.reply.notify_one();
            let expected = if first_load {
                "sess_loaded"
            } else {
                "sess_new-0"
            };
            let mut controller = crate::session::SessionController::new();
            if failed_load {
                let failure = next(&mut rx).await.notification;
                assert!(
                    matches!(failure, Notification::BridgeError { operation, message }
                if operation == "Load session" && message.contains("original load cause"))
                );
                gates.fail_load.store(false, Ordering::SeqCst);
                // Retrying the SAME ID after failure must not replay the failed
                // attempt's snapshot/policy. The second handler emits neither.
                must_succeed(
                    sender.send(command(true, expected)).await,
                    "retry released load",
                );
                gates.reply.notify_one();
            }
            for label in ["usage", "created", "config"] {
                let event = next(&mut rx).await;
                assert!(
                    match (&event.notification, label) {
                        (Notification::UsageSessionStarted { session_id, .. }, "usage")
                        | (Notification::SessionCreated { session_id, .. }, "created") =>
                            session_id.as_str() == expected,
                        (Notification::ConfigOptionsUpdated(_), "config") => true,
                        _ => false,
                    },
                    "wrong reset ordering: expected {label}, got {event:?}"
                );
                controller.apply_notification(&event.notification);
            }
            if !failed_load {
                for label in ["snapshot", "policy"] {
                    let event = next(&mut rx).await;
                    assert_eq!(
                        event.session_id.as_ref().map(SessionId::as_str),
                        Some(expected)
                    );
                    assert!(matches!(
                        (&event.notification, label),
                        (Notification::McpUpdate(McpUpdate::Snapshot(_)), "snapshot")
                            | (Notification::McpUpdate(McpUpdate::Policy { .. }), "policy")
                    ));
                    controller.apply_notification(&event.notification);
                }
                let state = controller.mcp();
                let snapshot = state
                    .snapshot
                    .as_ref()
                    .unwrap_or_else(|| panic!("first staged snapshot survived competitor"));
                assert_eq!(snapshot.servers.len(), 1);
                assert_eq!(snapshot.servers[0].name, "first-only");
                assert_eq!(state.policy, Some(McpPolicy::ApiFailure));
            }
            must_succeed(
                sender.send(BridgeCommand::Shutdown).await,
                "admission shutdown",
            );
            must_succeed(
                must_succeed(
                    tokio::time::timeout(Duration::from_secs(5), join).await,
                    "admission join deadline",
                )
                .unwrap_or_else(|error| panic!("admission join: {error}")),
                "admission stopped",
            );
            while let Some(event) = rx.recv().await {
                assert!(
                    !matches!(
                        event.notification,
                        Notification::SessionCreated { .. } | Notification::McpUpdate(_)
                    ),
                    "no rejected root or failed staging may publish later: {event:?}"
                );
            }
        })
        .await;
}

#[tokio::test(start_paused = true)]
async fn sdk_competing_root_starts_reject_before_send_and_preserve_first_mcp_staging() {
    for (first_load, competitor_load) in
        [(false, false), (false, true), (true, false), (true, true)]
    {
        scenario(first_load, competitor_load, false).await;
    }
}

#[tokio::test(start_paused = true)]
async fn sdk_recoverable_load_failure_releases_admission_and_discards_only_failed_staging() {
    scenario(true, false, true).await;
}

#[test]
fn staging_slot_admission_releases_on_activation_and_failure_without_a_second_busy_flag() {
    for target in [None, Some(SessionId::new("sess_loaded"))] {
        for success in [false, true] {
            let mut owner = super::super::mcp::McpOwner::default();
            must_succeed(owner.begin_session(target.clone()), "claim root staging");
            for competitor in [None, Some(SessionId::new("sess_competitor"))] {
                assert!(
                    owner.begin_session(competitor).is_err(),
                    "one slot admits one outstanding root start"
                );
            }
            if success {
                owner.activate(
                    target
                        .clone()
                        .unwrap_or_else(|| SessionId::new("sess_created")),
                );
            } else {
                owner.fail_session();
            }
            must_succeed(
                owner.begin_session(None),
                "completion releases the staging claim",
            );
        }
    }
}
