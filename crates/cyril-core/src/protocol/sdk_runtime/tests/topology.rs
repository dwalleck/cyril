use std::sync::{Arc, Mutex};

#[cfg(not(feature = "kas"))]
use agent_client_protocol::ConnectionTo;
use agent_client_protocol::schema::v1 as acp;
use agent_client_protocol::{
    Agent, Channel, Client, Conductor, ConnectTo, DynConnectTo, Proxy, RawJsonRpcMessage,
    Result as AcpResult, Role,
};

use super::super::{SdkRuntime, StageChain};
use crate::protocol::domain_mediator::DomainChannels;
use crate::protocol::source_observer::IngressTracker;

#[test]
fn zero_stage_runtime_still_has_a_conductor_stage_chain() {
    let stages = StageChain::default();
    assert!(stages.stages.is_empty());
}

#[cfg(not(feature = "kas"))]
#[tokio::test]
async fn unknown_standard_notification_does_not_enter_domain_queue() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let agent = Agent
                .builder()
                .name("unknown-notification-test-agent")
                .on_receive_request(
                    async move |request: acp::InitializeRequest,
                                responder,
                                connection: ConnectionTo<Client>| {
                        connection.send_notification(
                            agent_client_protocol::UntypedMessage::new(
                                "future/notification",
                                serde_json::json!({"ignored": true}),
                            )?,
                        )?;
                        let typed: acp::SessionNotification =
                            serde_json::from_value(serde_json::json!({
                                "sessionId": "known-after-unknown",
                                "update": {
                                    "sessionUpdate": "agent_message_chunk",
                                    "content": {"type": "text", "text": "known"}
                                }
                            }))
                            .map_err(|error| {
                                agent_client_protocol::Error::internal_error()
                                    .data(format!("known notification fixture: {error}"))
                            })?;
                        connection.send_notification(typed)?;
                        responder.respond(acp::InitializeResponse::new(request.protocol_version))
                    },
                    agent_client_protocol::on_receive_request!(),
                );
            let (channels, mut work_rx, _host_rx) = DomainChannels::new(IngressTracker::new())
                .unwrap_or_else(|error| panic!("unknown notification channels: {error}"));
            let runtime = SdkRuntime::start_for_test(agent, channels, StageChain::default())
                .await
                .unwrap_or_else(|error| panic!("unknown notification runtime: {error}"));
            runtime
                .connection()
                .send_request(acp::InitializeRequest::new(
                    agent_client_protocol::schema::ProtocolVersion::V1,
                ))
                .block_task()
                .await
                .unwrap_or_else(|error| panic!("unknown notification initialize: {error}"));
            let work = tokio::time::timeout(std::time::Duration::from_secs(5), work_rx.recv())
                .await
                .unwrap_or_else(|_| panic!("known notification timed out"))
                .unwrap_or_else(|| panic!("known notification channel closed"));
            assert!(matches!(
                work,
                crate::protocol::domain_mediator::DomainWork::Session(_)
            ));
            assert!(
                work_rx.try_recv().is_err(),
                "unknown standard notification must not consume bounded domain capacity"
            );
            runtime.shutdown().await;
        })
        .await;
}

type MessageInspector = Box<dyn FnMut(&RawJsonRpcMessage) -> AcpResult<()> + Send + Sync>;

struct SnoopComponent<R: Role> {
    base: DynConnectTo<R>,
    incoming: MessageInspector,
    outgoing: MessageInspector,
}

impl<R: Role> SnoopComponent<R> {
    fn new(
        base: impl ConnectTo<R>,
        incoming: impl FnMut(&RawJsonRpcMessage) -> AcpResult<()> + Send + Sync + 'static,
        outgoing: impl FnMut(&RawJsonRpcMessage) -> AcpResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            base: DynConnectTo::new(base),
            incoming: Box::new(incoming),
            outgoing: Box::new(outgoing),
        }
    }
}

impl<R: Role> ConnectTo<R> for SnoopComponent<R> {
    async fn connect_to(self, client: impl ConnectTo<R::Counterpart>) -> AcpResult<()> {
        let (client_channel, client_future) = client.into_channel_and_future();
        let (base_channel, base_future) = self.base.into_channel_and_future();
        let snoop = Channel::bridge_with_inspection(
            client_channel,
            base_channel,
            self.incoming,
            self.outgoing,
        );
        tokio::try_join!(client_future, base_future, snoop)?;
        Ok(())
    }
}

fn recording_stage(
    label: &'static str,
    events: Arc<Mutex<Vec<(&'static str, String)>>>,
) -> DynConnectTo<Conductor> {
    DynConnectTo::new(SnoopComponent::new(
        Proxy.builder(),
        move |message| {
            if let RawJsonRpcMessage::Request(request) = message {
                events
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push((label, request.method.to_string()));
            }
            Ok(())
        },
        |_| Ok(()),
    ))
}

fn terminal_agent() -> impl ConnectTo<Client> + 'static {
    Agent
        .builder()
        .name("ordered-stage-test-agent")
        .on_receive_request(
            async |request: acp::InitializeRequest, responder, _connection| {
                responder.respond(acp::InitializeResponse::new(request.protocol_version))
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async |_request: acp::NewSessionRequest, responder, _connection| {
                responder.respond(acp::NewSessionResponse::new("terminal"))
            },
            agent_client_protocol::on_receive_request!(),
        )
}

#[tokio::test]
async fn ordered_stage_chain_preserves_runtime_frame_order() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let events = Arc::new(Mutex::new(Vec::new()));
            let stages = StageChain::new(vec![
                recording_stage("first", Arc::clone(&events)),
                recording_stage("second", Arc::clone(&events)),
            ]);
            let (channels, _work_rx, _host_rx) = DomainChannels::new(IngressTracker::new())
                .unwrap_or_else(|error| panic!("ordered-stage domain channels: {error}"));
            let runtime = SdkRuntime::start_for_test(terminal_agent(), channels, stages)
                .await
                .unwrap_or_else(|error| panic!("ordered-stage runtime: {error}"));
            let connection = runtime.connection().clone();
            connection
                .send_request(acp::InitializeRequest::new(
                    agent_client_protocol::schema::ProtocolVersion::V1,
                ))
                .block_task()
                .await
                .unwrap_or_else(|error| panic!("ordered-stage initialize: {error}"));
            let response = connection
                .send_request(acp::NewSessionRequest::new(std::env::temp_dir()))
                .block_task()
                .await
                .unwrap_or_else(|error| panic!("ordered-stage session/new: {error}"));
            assert_eq!(response.session_id.to_string(), "terminal");
            let order = events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .filter_map(|(label, method)| (method == "session/new").then_some(*label))
                .collect::<Vec<_>>();
            assert_eq!(order, ["first", "second"]);
            runtime.shutdown().await;
        })
        .await;
}
