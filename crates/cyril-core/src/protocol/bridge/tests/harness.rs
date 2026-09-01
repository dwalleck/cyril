use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use agent_client_protocol::schema::v1 as acp;
use agent_client_protocol::{Agent, Client, ConnectTo, ConnectionTo, UntypedMessage};
use tokio::sync::mpsc;

use super::*;
use crate::protocol::domain_mediator::{DomainChannels, DomainConfig, DomainMediator};
use crate::protocol::engine::{Engine, V2Engine};
use crate::protocol::sdk_runtime::{SdkRuntime, StageChain};

#[derive(Clone)]
pub(super) struct InboundProbe(DomainChannels);

impl InboundProbe {
    pub(super) async fn send(
        &self,
        routed: RoutedNotification,
    ) -> agent_client_protocol::Result<()> {
        self.0.inject(routed).await
    }

    pub(super) fn capacity(&self) -> usize {
        self.0.remaining_capacity()
    }
}

#[derive(Default)]
pub(super) struct Script {
    pub(super) received: Arc<Mutex<Vec<String>>>,
    pub(super) ext_calls: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
    pub(super) negotiated_protocol:
        Arc<Mutex<Option<agent_client_protocol::schema::ProtocolVersion>>>,
    pub(super) emit_chunks: usize,
    pub(super) inbound: Option<InboundProbe>,
    pub(super) emit_unknown_update: bool,
    pub(super) request_extension_during_initialize: bool,
    pub(super) request_malformed_standard_during_initialize: bool,
    pub(super) request_unknown_standard_during_initialize: bool,
    pub(super) request_permission_on_prompt: bool,
    pub(super) fail_extensions: Vec<String>,
    pub(super) fail_new_session: bool,
}

impl Script {
    pub(super) fn received(&self) -> MutexGuard<'_, Vec<String>> {
        lock(&self.received)
    }

    /// Only the v2-only C5 command oracle reads the exact call ledger.
    #[cfg(not(feature = "kas"))]
    pub(super) fn ext_calls(&self) -> MutexGuard<'_, Vec<(String, serde_json::Value)>> {
        lock(&self.ext_calls)
    }

    pub(super) fn negotiated_protocol(
        &self,
    ) -> Option<agent_client_protocol::schema::ProtocolVersion> {
        *lock(&self.negotiated_protocol)
    }
}

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn record(ledger: &Arc<Mutex<Vec<String>>>, value: impl Into<String>) {
    lock(ledger).push(value.into());
}

pub(super) struct AgentKill {
    abort: tokio::task::AbortHandle,
}

impl AgentKill {
    pub(super) fn kill(self) {
        self.abort.abort();
    }
}

fn fake_agent(script: &Rc<RefCell<Script>>) -> impl ConnectTo<Client> + 'static {
    let received_initialize = Arc::clone(&script.borrow().received);
    let received_new = Arc::clone(&script.borrow().received);
    let received_prompt = Arc::clone(&script.borrow().received);
    let received_cancel = Arc::clone(&script.borrow().received);
    let received_ext = Arc::clone(&script.borrow().received);
    let ext_calls = Arc::clone(&script.borrow().ext_calls);
    let negotiated_protocol = Arc::clone(&script.borrow().negotiated_protocol);
    let emit_chunks = script.borrow().emit_chunks;
    let emit_unknown_update = script.borrow().emit_unknown_update;
    let request_extension_during_initialize = script.borrow().request_extension_during_initialize;
    let request_malformed_standard_during_initialize =
        script.borrow().request_malformed_standard_during_initialize;
    let request_unknown_standard_during_initialize =
        script.borrow().request_unknown_standard_during_initialize;
    let next_session = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let request_permission_on_prompt = script.borrow().request_permission_on_prompt;
    let fail_extensions = script.borrow().fail_extensions.clone();
    let fail_new_session = script.borrow().fail_new_session;

    Agent
        .builder()
        .name("cyril-sdk2-test-agent")
        .on_receive_request(
            async move |request: acp::InitializeRequest,
                        responder,
                        connection: ConnectionTo<Client>| {
                *lock(&negotiated_protocol) = Some(request.protocol_version);
                if request_extension_during_initialize
                    || request_malformed_standard_during_initialize
                    || request_unknown_standard_during_initialize
                {
                    let callback_connection = connection.clone();
                    let received_callback = Arc::clone(&received_initialize);
                    connection.spawn(async move {
                        if request_extension_during_initialize {
                            record(&received_callback, "initialize_callback_started");
                            let callback = UntypedMessage::new(
                                "_probe/initialize",
                                serde_json::json!({"phase": "initialize"}),
                            )?;
                            let response = callback_connection
                                .send_request(callback)
                                .block_task()
                                .await?;
                            if !response.is_null() {
                                return Err(agent_client_protocol::Error::internal_error()
                                    .data("initialize callback expected null response"));
                            }
                            record(&received_callback, "initialize_callback");
                        }
                        if request_malformed_standard_during_initialize {
                            record(&received_callback, "malformed_standard_started");
                            let malformed = UntypedMessage::new(
                                "session/request_permission",
                                serde_json::json!({"invalid": true}),
                            )?;
                            match callback_connection
                                .send_request(malformed)
                                .block_task()
                                .await
                            {
                                Err(error)
                                    if error.code
                                        == agent_client_protocol::ErrorCode::InvalidParams =>
                                {
                                    record(&received_callback, "malformed_standard_rejected");
                                }
                                Ok(response) => {
                                    return Err(agent_client_protocol::Error::internal_error().data(
                                        format!(
                                            "malformed standard request unexpectedly succeeded: {response}"
                                        ),
                                    ));
                                }
                                Err(error) => {
                                    return Err(agent_client_protocol::Error::internal_error().data(
                                        format!(
                                            "malformed standard request returned wrong error: {error}"
                                        ),
                                    ));
                                }
                            }
                        }
                        if request_unknown_standard_during_initialize {
                            record(&received_callback, "unknown_standard_started");
                            let unknown = UntypedMessage::new(
                                "future/request",
                                serde_json::json!({"value": true}),
                            )?;
                            match callback_connection
                                .send_request(unknown)
                                .block_task()
                                .await
                            {
                                Err(error)
                                    if error.code
                                        == agent_client_protocol::ErrorCode::MethodNotFound =>
                                {
                                    record(&received_callback, "unknown_standard_rejected");
                                }
                                Ok(response) => {
                                    return Err(agent_client_protocol::Error::internal_error().data(
                                        format!(
                                            "unknown standard request unexpectedly succeeded: {response}"
                                        ),
                                    ));
                                }
                                Err(error) => {
                                    return Err(agent_client_protocol::Error::internal_error().data(
                                        format!(
                                            "unknown standard request returned wrong error: {error}"
                                        ),
                                    ));
                                }
                            }
                        }
                        responder.respond(acp::InitializeResponse::new(request.protocol_version))
                    })?;
                    Ok(())
                } else {
                    responder.respond(acp::InitializeResponse::new(request.protocol_version))
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |_request: acp::NewSessionRequest, responder, _connection| {
                record(&received_new, "new_session");
                let index = next_session.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if fail_new_session {
                    return responder.respond_with_error(
                        agent_client_protocol::Error::internal_error().data("scripted new failure"),
                    );
                }
                responder.respond(acp::NewSessionResponse::new(format!("fake-{index}")))
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: acp::PromptRequest,
                        responder,
                        connection: ConnectionTo<Client>| {
                if request_permission_on_prompt {
                    let permission = acp::RequestPermissionRequest::new(
                        request.session_id.clone(),
                        acp::ToolCallUpdate::new(
                            "permission-tool",
                            acp::ToolCallUpdateFields::new(),
                        ),
                        Vec::new(),
                    );
                    let permission_connection = connection.clone();
                    connection.spawn(async move {
                        let _response = permission_connection
                            .send_request(permission)
                            .block_task()
                            .await?;
                        Ok(())
                    })?;
                }
                record(&received_prompt, "prompt");
                if emit_unknown_update {
                    let notification = UntypedMessage::new(
                        "session/update",
                        serde_json::json!({
                            "sessionId": request.session_id.to_string(),
                            "update": {
                                "sessionUpdate": "future_session_update",
                                "payload": {"kept": true}
                            }
                        }),
                    )?;
                    connection.send_notification(notification)?;
                }
                for index in 0..emit_chunks {
                    let notification: acp::SessionNotification =
                        serde_json::from_value(serde_json::json!({
                            "sessionId": request.session_id.to_string(),
                            "update": {
                                "sessionUpdate": "agent_message_chunk",
                                "content": {"type": "text", "text": format!("c{index}")}
                            }
                        }))
                        .map_err(|error| {
                            agent_client_protocol::Error::internal_error().data(error.to_string())
                        })?;
                    connection.send_notification(notification)?;
                }
                responder.respond(acp::PromptResponse::new(acp::StopReason::EndTurn))
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_notification(
            async move |_notification: acp::CancelNotification, _connection| {
                record(&received_cancel, "cancel");
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_request(
            async move |_request: acp::SetSessionModeRequest, responder, _connection| {
                responder.respond_with_error(agent_client_protocol::Error::method_not_found())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |_request: acp::LoadSessionRequest, responder, _connection| {
                responder.respond_with_error(agent_client_protocol::Error::method_not_found())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: UntypedMessage, responder, _connection| {
                if request.method() == "session/set_model" {
                    record(&received_ext, "set_model");
                    lock(&ext_calls).push((
                        "session/set_model".to_owned(),
                        request.params.clone(),
                    ));
                    return responder
                        .respond_with_error(agent_client_protocol::Error::method_not_found());
                }
                let Some(method) = request.method().strip_prefix('_').map(str::to_owned) else {
                    return responder
                        .respond_with_error(agent_client_protocol::Error::method_not_found());
                };
                record(&received_ext, format!("ext:{method}"));
                if fail_extensions.contains(&method) {
                    return responder.respond_with_error(
                        agent_client_protocol::Error::internal_error().data("scripted failure"),
                    );
                }
                lock(&ext_calls).push((method, request.params.clone()));
                responder.respond(serde_json::json!({}))
            },
            agent_client_protocol::on_receive_request!(),
        )
}

pub(super) async fn recv_notif(
    rx: &mut mpsc::Receiver<RoutedNotification>,
    timeout_seconds: u64,
) -> Option<Notification> {
    tokio::time::timeout(Duration::from_secs(timeout_seconds), rx.recv())
        .await
        .ok()
        .flatten()
        .map(|routed| routed.notification)
}

pub(super) async fn start_session(
    sender: &BridgeSender,
    rx: &mut mpsc::Receiver<RoutedNotification>,
) -> crate::types::SessionId {
    sender
        .send(BridgeCommand::NewSession {
            cwd: std::env::temp_dir(),
        })
        .await
        .unwrap_or_else(|error| panic!("new session command failed: {error}"));
    let usage = recv_notif(rx, 5)
        .await
        .unwrap_or_else(|| panic!("missing usage-session notification"));
    assert!(matches!(usage, Notification::UsageSessionStarted { .. }));
    match recv_notif(rx, 5)
        .await
        .unwrap_or_else(|| panic!("missing session-created notification"))
    {
        Notification::SessionCreated { session_id, .. } => session_id,
        other => panic!("expected SessionCreated, got {other:?}"),
    }
}

pub(super) async fn with_harness<F, Fut>(script: Rc<RefCell<Script>>, body: F)
where
    F: FnOnce(
        BridgeSender,
        mpsc::Receiver<RoutedNotification>,
        mpsc::Receiver<PermissionRequest>,
        Rc<tokio::sync::Notify>,
        tokio::task::JoinHandle<crate::Result<()>>,
    ) -> Fut,
    Fut: Future<Output = ()>,
{
    with_engine_harness(
        Rc::new(V2Engine),
        script,
        |sender, rx, permission_rx, gate, loop_handle, kill| async move {
            let _keep_agent_alive = kill;
            body(sender, rx, permission_rx, gate, loop_handle).await;
        },
    )
    .await;
}

pub(super) async fn with_engine_harness<F, Fut>(
    engine: Rc<dyn Engine>,
    script: Rc<RefCell<Script>>,
    body: F,
) where
    F: FnOnce(
        BridgeSender,
        mpsc::Receiver<RoutedNotification>,
        mpsc::Receiver<PermissionRequest>,
        Rc<tokio::sync::Notify>,
        tokio::task::JoinHandle<crate::Result<()>>,
        AgentKill,
    ) -> Fut,
    Fut: Future<Output = ()>,
{
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async move {
            let agent = fake_agent(&script);
            let (handle, bridge_channels) = create_channel_pair();
            let (sender, notification_rx, permission_rx, _source_rx, _completion_rx) =
                handle.split();
            let config = DomainConfig {
                engine,
                cwd: std::env::temp_dir(),
                present_as: None,
                stall_threshold: Duration::from_secs(30),
                #[cfg(feature = "kas")]
                host_shell: None,
            };
            let (mediator, domain_channels) = DomainMediator::new(config, bridge_channels)
                .unwrap_or_else(|error| panic!("SDK2 harness domain channels: {error}"));
            script.borrow_mut().inbound = Some(InboundProbe(domain_channels.clone()));
            let runtime = tokio::time::timeout(
                Duration::from_secs(5),
                SdkRuntime::start_for_test(agent, domain_channels, StageChain::new(Vec::new())),
            )
            .await
            .unwrap_or_else(|_| panic!("SDK2 harness startup timed out"))
            .unwrap_or_else(|error| panic!("SDK2 harness startup failed: {error}"));
            let abort = runtime
                .abort_handle()
                .unwrap_or_else(|| panic!("SDK2 harness runtime has an abort handle"));
            let loop_handle = tokio::task::spawn_local(mediator.run(runtime));
            body(
                sender,
                notification_rx,
                permission_rx,
                Rc::new(tokio::sync::Notify::new()),
                loop_handle,
                AgentKill { abort },
            )
            .await;
        })
        .await;
}
