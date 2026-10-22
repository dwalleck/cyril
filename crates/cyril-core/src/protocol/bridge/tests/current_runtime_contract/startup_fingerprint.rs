//! Fingerprint preflight must outrank root-start admission (caar F2).
//! Uses predecessor APIs through the actual SDK and serial command ingress;
//! the separate startup deadline proof stays unchanged.

use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use agent_client_protocol::{Agent, Client, ConnectionTo, schema::v1 as acp};
use serde_json::json;
use tokio::sync::Notify;

use super::ExpectContract;
use crate::protocol::bridge::create_channel_pair;
use crate::protocol::domain_mediator::{DomainConfig, DomainMediator};
use crate::protocol::engine::{Engine, V2Engine};
use crate::protocol::sdk_runtime::{SdkRuntime, StageChain};
use crate::types::{BridgeCommand, Notification, SessionId};

#[derive(Default)]
struct Gates {
    new_count: AtomicUsize,
    load_count: AtomicUsize,
    received: Notify,
    reply: Notify,
}

// Also own cancellation on a setup failure or an unexpected assertion panic.
// The ordinary path below explicitly joins shutdown before judging the bug.
struct AbortOnDrop {
    mediator: tokio::task::AbortHandle,
    sdk: tokio::task::AbortHandle,
}

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.mediator.abort();
        self.sdk.abort();
    }
}

async fn wrong_dialect_load_during_pending_root(
    engine: Rc<dyn Engine>,
    kas: bool,
    pending_load: bool,
) {
    tokio::task::LocalSet::new()
        .run_until(async move {
            let gates = Arc::new(Gates::default());
            let new_gates = Arc::clone(&gates);
            let load_gates = Arc::clone(&gates);
            let agent = Agent
                .builder()
                .name("caar-fingerprint-preflight")
                .on_receive_request(
                    async move |request: acp::InitializeRequest, responder, _connection| {
                        let mut response = json!({
                            "protocolVersion": request.protocol_version,
                            "agentCapabilities": {"loadSession": true}
                        });
                        // Match the bound engine here: the contradiction must
                        // come from the competing load, not initialize.
                        if kas {
                            response["agentCapabilities"]["_meta"] = json!({"kiro": {}});
                        }
                        responder.respond(
                            serde_json::from_value::<acp::InitializeResponse>(response).map_err(
                                |error| {
                                    agent_client_protocol::Error::internal_error()
                                        .data(error.to_string())
                                },
                            )?,
                        )
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_request(
                    async move |_request: acp::NewSessionRequest,
                                responder,
                                connection: ConnectionTo<Client>| {
                        let state = Arc::clone(&new_gates);
                        state.new_count.fetch_add(1, Ordering::SeqCst);
                        state.received.notify_one();
                        connection.spawn(async move {
                            state.reply.notified().await;
                            let session_id = if kas { "sess_valid-new" } else { "valid-new" };
                            responder.respond(
                                serde_json::from_value::<acp::NewSessionResponse>(json!({
                                    "sessionId": session_id
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
                    async move |_request: acp::LoadSessionRequest,
                                responder,
                                connection: ConnectionTo<Client>| {
                        let state = Arc::clone(&load_gates);
                        state.load_count.fetch_add(1, Ordering::SeqCst);
                        state.received.notify_one();
                        connection.spawn(async move {
                            state.reply.notified().await;
                            responder.respond(
                                serde_json::from_value::<acp::LoadSessionResponse>(json!({}))
                                    .map_err(|error| {
                                        agent_client_protocol::Error::internal_error()
                                            .data(error.to_string())
                                    })?,
                            )
                        })?;
                        Ok(())
                    },
                    agent_client_protocol::on_receive_request!(),
                );
            let directory = tempfile::tempdir().expect_contract("fingerprint workspace");
            let (handle, bridge) = create_channel_pair();
            let (sender, mut rx, _permissions, _sources, _completion) = handle.split();
            let (mediator, channels) = DomainMediator::new(
                DomainConfig {
                    engine,
                    cwd: directory.path().into(),
                    present_as: None,
                    stall_threshold: Duration::from_secs(30),
                    #[cfg(feature = "kas")]
                    host_shell: None,
                    #[cfg(feature = "kas")]
                    kas_token: None,
                },
                bridge,
            )
            .expect_contract("fingerprint mediator");
            let runtime = SdkRuntime::start_for_test(agent, channels, StageChain::default())
                .await
                .expect_contract("fingerprint SDK");
            let sdk_abort = runtime
                .abort_handle()
                .expect_contract("fingerprint SDK cancellation owner");
            let mut join = tokio::task::spawn_local(mediator.run(runtime));
            let _owned_tasks = AbortOnDrop {
                mediator: join.abort_handle(),
                sdk: sdk_abort,
            };
            let first = if pending_load {
                BridgeCommand::LoadSession {
                    session_id: SessionId::new(if kas { "sess_valid-load" } else { "valid-load" }),
                }
            } else {
                BridgeCommand::NewSession {
                    cwd: directory.path().into(),
                }
            };
            sender.send(first).await.expect_contract("valid root command");
            tokio::time::timeout(Duration::from_secs(5), gates.received.notified())
                .await
                .expect_contract("valid root RPC reached the SDK agent");
            sender
                .send(BridgeCommand::LoadSession {
                    session_id: SessionId::new(if kas { "wrong-v2-id" } else { "sess_wrong-kas-id" }),
                })
                .await
                .expect_contract("competing wrong-dialect load");
            let observed = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await;
            let disconnected = matches!(
                &observed,
                Ok(Some(frame)) if matches!(frame.notification, Notification::BridgeDisconnected { .. })
            );

            // On the actual broken baseline this is a recoverable busy error.
            // Stop and join that still-live owner BEFORE the RED assertion, so
            // neither its held responder nor SDK task escapes a failing test.
            if !disconnected
                && let Err(error) = sender.send(BridgeCommand::Shutdown).await
            {
                tracing::debug!(%error, "fingerprint RED cleanup owner already closed");
            }
            // A held success becomes eligible only after the fatal decision
            // (or cleanup shutdown) has been observed/queued. It must not
            // publish a root after the owner has decided to stop.
            gates.reply.notify_one();
            let joined = tokio::time::timeout(Duration::from_secs(5), &mut join).await;
            let stream_closed = tokio::time::timeout(Duration::from_secs(5), async {
                let mut trailing = Vec::new();
                while let Some(frame) = rx.recv().await {
                    trailing.push(frame);
                }
                trailing
            })
            .await;
            let ingress_closed = sender.send(BridgeCommand::Shutdown).await.is_err();

            joined
                .expect_contract("fingerprint owner shutdown deadline")
                .expect_contract("fingerprint owner joined")
                .expect_contract("fingerprint owner stopped cleanly");
            let trailing = stream_closed.expect_contract("fingerprint notification stream closed");
            let frame = observed
                .expect_contract("fingerprint result deadline")
                .expect_contract("fingerprint result notification");
            let reason = match frame.notification {
                Notification::BridgeDisconnected { reason } => reason,
                other => panic!(
                    "wrong-dialect load must retain fatal preflight while root load={pending_load} is pending; got {other:?}"
                ),
            };
            assert!(reason.contains("engine mismatch"), "{reason}");
            assert!(reason.contains("session id"), "{reason}");
            assert!(!reason.contains("already pending"), "{reason}");
            let remedy = if kas { "--agent-engine v2" } else { "--agent-engine kas" };
            assert!(reason.contains(remedy), "missing {remedy}: {reason}");
            if !kas && !cfg!(feature = "kas") {
                assert!(reason.contains("--features kas"), "{reason}");
            }
            assert_eq!(gates.new_count.load(Ordering::SeqCst), usize::from(!pending_load));
            assert_eq!(gates.load_count.load(Ordering::SeqCst), usize::from(pending_load));
            assert!(ingress_closed, "fatal preflight must close command ingress");
            assert!(trailing.is_empty(), "late root publication/duplicate fatal: {trailing:?}");
        })
        .await;
}

#[tokio::test(start_paused = true)]
async fn caar_f2_actual_v2_wrong_dialect_load_is_fatal_during_pending_new() {
    wrong_dialect_load_during_pending_root(Rc::new(V2Engine), false, false).await;
}

#[tokio::test(start_paused = true)]
async fn caar_f2_actual_v2_wrong_dialect_load_is_fatal_during_pending_load() {
    wrong_dialect_load_during_pending_root(Rc::new(V2Engine), false, true).await;
}

#[cfg(feature = "kas")]
#[tokio::test(start_paused = true)]
async fn caar_f2_actual_kas_wrong_dialect_load_is_fatal_during_pending_new() {
    wrong_dialect_load_during_pending_root(
        Rc::new(crate::protocol::engine::KasEngine::default()),
        true,
        false,
    )
    .await;
}

#[cfg(feature = "kas")]
#[tokio::test(start_paused = true)]
async fn caar_f2_actual_kas_wrong_dialect_load_is_fatal_during_pending_load() {
    wrong_dialect_load_during_pending_root(
        Rc::new(crate::protocol::engine::KasEngine::default()),
        true,
        true,
    )
    .await;
}
