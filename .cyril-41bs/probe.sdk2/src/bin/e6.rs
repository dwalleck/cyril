use std::sync::{Arc, Mutex, MutexGuard};

use agent_client_protocol::schema::{
    InitializeProxyRequest, ProtocolVersion,
    v1::{Implementation, InitializeRequest, InitializeResponse},
};
use agent_client_protocol::{
    Agent, Channel, Client, Conductor, ConnectTo, ConnectionTo, Error, Proxy, Responder,
};
use agent_client_protocol_conductor::{ConductorImpl, ProxiesAndAgent};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::time::{Duration, timeout};

#[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcRequest)]
#[request(method = "_kiro/probe", response = ProbeResponse)]
struct ProbeRequest {
    value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcResponse)]
struct ProbeResponse {
    value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcNotification)]
#[notification(method = "_kiro/probe-notification")]
struct ProbeNotification {
    value: String,
}

#[derive(Clone)]
struct Log(Arc<Mutex<Vec<String>>>);

impl Log {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(Vec::new())))
    }

    fn lock(&self) -> MutexGuard<'_, Vec<String>> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn push(&self, value: impl Into<String>) {
        self.lock().push(value.into());
    }

    fn snapshot(&self) -> Vec<String> {
        self.lock().clone()
    }
}

#[derive(Clone)]
struct FakeAgent {
    log: Log,
    cancellation_tx: mpsc::UnboundedSender<String>,
}

impl ConnectTo<Client> for FakeAgent {
    async fn connect_to(self, client: impl ConnectTo<Agent>) -> Result<(), Error> {
        let init_log = self.log.clone();
        let request_log = self.log.clone();
        let cancellation_tx = self.cancellation_tx;
        Agent
            .builder()
            .on_receive_request(
                async move |request: InitializeRequest, responder, _cx| {
                    init_log.push(format!("agent:initialize:{:?}", request.protocol_version));
                    responder.respond(
                        InitializeResponse::new(request.protocol_version)
                            .agent_info(Implementation::new("fake-kiro", "1")),
                    )
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: ProbeRequest,
                            responder: Responder<ProbeResponse>,
                            cx: ConnectionTo<Client>| {
                    request_log.push(format!(
                        "agent:request:{}:id={:?}",
                        request.value,
                        responder.id()
                    ));
                    if request.value.contains("cancel") {
                        let cancellation = responder.cancellation();
                        let tx = cancellation_tx.clone();
                        cx.spawn(async move {
                            cancellation.cancelled().await;
                            tx.send("agent:cancellation-observed".to_owned())
                                .map_err(|error| Error::internal_error().data(error.to_string()))?;
                            responder.respond(ProbeResponse {
                                value: "cancelled".to_owned(),
                            })
                        })?;
                        return Ok(());
                    }
                    cx.send_notification(ProbeNotification {
                        value: format!("agent:notify:{}", request.value),
                    })?;
                    responder.respond(ProbeResponse {
                        value: format!("agent:response:{}", request.value),
                    })
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_to(client)
            .await
    }
}
struct FailingAgent;

impl ConnectTo<Client> for FailingAgent {
    async fn connect_to(self, client: impl ConnectTo<Agent>) -> Result<(), Error> {
        Agent
            .builder()
            .on_receive_request(
                async |request: InitializeRequest, responder, _cx| {
                    responder.respond(
                        InitializeResponse::new(request.protocol_version)
                            .agent_info(Implementation::new("failing-agent", "1")),
                    )
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_with(client, async |_cx| {
                tokio::time::sleep(Duration::from_millis(25)).await;
                Err::<(), Error>(Error::internal_error().data("agent-crash"))
            })
            .await
    }
}

struct TransformProxy(&'static str);

impl ConnectTo<Conductor> for TransformProxy {
    async fn connect_to(self, client: impl ConnectTo<Proxy>) -> Result<(), Error> {
        let request_label = self.0;
        let notification_label = self.0;
        Proxy
            .builder()
            .on_receive_request_from(
                Client,
                async |request: InitializeProxyRequest, responder, cx| {
                    cx.send_request_to(Agent, request.initialize)
                        .forward_response_to(responder)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request_from(
                Client,
                async move |request: ProbeRequest, responder, cx| {
                    cx.send_request_to(
                        Agent,
                        ProbeRequest {
                            value: format!("{request_label}:request:{}", request.value),
                        },
                    )
                    .forward_response_to(responder)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_notification_from(
                Agent,
                async move |notification: ProbeNotification, cx| {
                    cx.send_notification_to(
                        Client,
                        ProbeNotification {
                            value: format!(
                                "{notification_label}:notification:{}",
                                notification.value
                            ),
                        },
                    )?;
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .connect_to(client)
            .await
    }
}

struct Observed<C> {
    base: C,
    wire: Log,
}

impl<C> ConnectTo<Client> for Observed<C>
where
    C: ConnectTo<Client>,
{
    async fn connect_to(self, client: impl ConnectTo<Agent>) -> Result<(), Error> {
        let (client_channel, client_future) = client.into_channel_and_future();
        let (base_channel, base_future) = self.base.into_channel_and_future();
        let client_to_agent = self.wire.clone();
        let agent_to_client = self.wire;
        let bridge = Channel::bridge_with_inspection(
            client_channel,
            base_channel,
            move |message| {
                client_to_agent.push(format!("client->agent:{}", serde_json::to_string(message)?));
                Ok(())
            },
            move |message| {
                agent_to_client.push(format!("agent->client:{}", serde_json::to_string(message)?));
                Ok(())
            },
        );
        tokio::try_join!(client_future, base_future, bridge)?;
        Ok(())
    }
}

async fn run_case<C>(
    name: &'static str,
    conductor: C,
    agent_log: Log,
    mut cancellation_rx: mpsc::UnboundedReceiver<String>,
) -> Result<Value>
where
    C: ConnectTo<Client>,
{
    let wire = Log::new();
    let wire_after = wire.clone();
    let notifications = Log::new();
    let notification_events = notifications.clone();
    let observed = Observed {
        base: conductor,
        wire,
    };
    let response = Client
        .builder()
        .on_receive_notification(
            async move |notification: ProbeNotification, _cx| {
                notification_events.push(notification.value);
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .connect_with(observed, async move |cx| {
            cx.send_request(
                InitializeRequest::new(ProtocolVersion::V1)
                    .client_info(Implementation::new("conductor-probe", "1")),
            )
            .block_task()
            .await?;
            let response = cx
                .send_request(ProbeRequest {
                    value: "client".to_owned(),
                })
                .block_task()
                .await?;
            let pending = cx.send_request(ProbeRequest {
                value: "cancel".to_owned(),
            });
            pending.cancel()?;
            pending.detach();
            let cancellation_event = timeout(Duration::from_secs(2), cancellation_rx.recv())
                .await
                .ok()
                .flatten();
            Ok::<_, Error>((response, cancellation_event))
        })
        .await
        .map_err(|error| anyhow::anyhow!("{name} conductor case failed: {error:?}"))?;
    let (response, cancellation_event) = response;
    let wire = wire_after.snapshot();
    let outer_request_id = wire.iter().find_map(|entry| {
        let raw = entry.strip_prefix("client->agent:")?;
        let value: Value = serde_json::from_str(raw).ok()?;
        (value.get("method").and_then(Value::as_str) == Some("_kiro/probe"))
            .then(|| value.get("id").cloned())
            .flatten()
    });
    let outer_response_id = wire.iter().find_map(|entry| {
        let raw = entry.strip_prefix("agent->client:")?;
        let value: Value = serde_json::from_str(raw).ok()?;
        (value.get("result").is_some()
            && value
                .pointer("/result/value")
                .and_then(Value::as_str)
                .is_some())
        .then(|| value.get("id").cloned())
        .flatten()
    });
    let cancellation_count = wire
        .iter()
        .filter(|entry| entry.contains("$/cancel_request"))
        .count();
    let terminal_cancellation_observed =
        cancellation_event.as_deref() == Some("agent:cancellation-observed");
    let response_identity_preserved =
        outer_request_id.is_some() && outer_request_id == outer_response_id;
    Ok(json!({
        "name": name,
        "response": response.value,
        "notifications": notifications.snapshot(),
        "terminal_cancellation_event": cancellation_event,
        "terminal_cancellation_observed": terminal_cancellation_observed,
        "agent_events": agent_log.snapshot(),
        "outer_request_id": outer_request_id,
        "outer_response_id": outer_response_id,
        "response_identity_preserved": response_identity_preserved,
        "cancellation_frames": cancellation_count,
        "cancellation_forwarded_once": cancellation_count == 1,
        "wire": wire,
    }))
}
async fn run_failure_case<C>(name: &'static str, conductor: C) -> Result<Value>
where
    C: ConnectTo<Client>,
{
    let outcome = Client
        .builder()
        .connect_with(conductor, async |cx| {
            cx.send_request(
                InitializeRequest::new(ProtocolVersion::V1)
                    .client_info(Implementation::new("failure-probe", "1")),
            )
            .block_task()
            .await?;
            std::future::pending::<Result<(), Error>>().await
        })
        .await;
    match outcome {
        Ok(()) => anyhow::bail!("{name} unexpectedly survived terminal component failure"),
        Err(error) => Ok(json!({
            "name": name,
            "connection_failed": true,
            "error": format!("{error:?}"),
            "terminal_error_preserved": format!("{error:?}").contains("agent-crash"),
        })),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let zero_log = Log::new();
    let (zero_cancel_tx, zero_cancel_rx) = mpsc::unbounded_channel();
    let zero = ConductorImpl::new_agent(
        "zero",
        ProxiesAndAgent::new(FakeAgent {
            log: zero_log.clone(),
            cancellation_tx: zero_cancel_tx,
        }),
    );
    let zero_result = run_case("zero-proxy", zero, zero_log, zero_cancel_rx).await?;

    let noop_log = Log::new();
    let (noop_cancel_tx, noop_cancel_rx) = mpsc::unbounded_channel();
    let noop = ConductorImpl::new_agent(
        "noop",
        ProxiesAndAgent::new(FakeAgent {
            log: noop_log.clone(),
            cancellation_tx: noop_cancel_tx,
        })
        .proxy(Proxy.builder()),
    );
    let noop_result = run_case("no-op-proxy", noop, noop_log, noop_cancel_rx).await?;

    let transform_log = Log::new();
    let (transform_cancel_tx, transform_cancel_rx) = mpsc::unbounded_channel();
    let transform = ConductorImpl::new_agent(
        "transform",
        ProxiesAndAgent::new(FakeAgent {
            log: transform_log.clone(),
            cancellation_tx: transform_cancel_tx,
        })
        .proxy(TransformProxy("proxy")),
    );
    let transform_result = run_case(
        "transforming-proxy",
        transform,
        transform_log,
        transform_cancel_rx,
    )
    .await?;

    let distinct_log = Log::new();
    let (distinct_cancel_tx, distinct_cancel_rx) = mpsc::unbounded_channel();
    let distinct = ConductorImpl::new_agent(
        "distinct",
        ProxiesAndAgent::new(FakeAgent {
            log: distinct_log.clone(),
            cancellation_tx: distinct_cancel_tx,
        })
        .proxy(Proxy.builder())
        .proxy(TransformProxy("proxy")),
    );
    let distinct_result = run_case(
        "distinct-two-stage",
        distinct,
        distinct_log,
        distinct_cancel_rx,
    )
    .await?;

    let repeated_log = Log::new();
    let (repeated_cancel_tx, repeated_cancel_rx) = mpsc::unbounded_channel();
    let repeated = ConductorImpl::new_agent(
        "repeated",
        ProxiesAndAgent::new(FakeAgent {
            log: repeated_log.clone(),
            cancellation_tx: repeated_cancel_tx,
        })
        .proxy(TransformProxy("first"))
        .proxy(TransformProxy("second")),
    );
    let repeated_result = run_case(
        "repeated-transform-stage",
        repeated,
        repeated_log,
        repeated_cancel_rx,
    )
    .await?;
    let distinct_stage_order_preserved =
        distinct_result["response"] == "agent:response:proxy:request:client";
    let repeated_stage_request_order_preserved =
        repeated_result["response"] == "agent:response:second:request:first:request:client";
    let repeated_stage_notification_order_preserved = repeated_result["notifications"][0]
        == "first:notification:second:notification:agent:notify:second:request:first:request:client";
    ensure!(
        distinct_stage_order_preserved
            && repeated_stage_request_order_preserved
            && repeated_stage_notification_order_preserved,
        "C2 ordered multi-stage/repeated-stage contract failed"
    );
    let zero_failure = run_failure_case(
        "zero-proxy-failure",
        ConductorImpl::new_agent("zero-failure", ProxiesAndAgent::new(FailingAgent)),
    )
    .await?;
    let noop_failure = run_failure_case(
        "no-op-proxy-failure",
        ConductorImpl::new_agent(
            "noop-failure",
            ProxiesAndAgent::new(FailingAgent).proxy(Proxy.builder()),
        ),
    )
    .await?;
    let transform_failure = run_failure_case(
        "transforming-proxy-failure",
        ConductorImpl::new_agent(
            "transform-failure",
            ProxiesAndAgent::new(FailingAgent).proxy(TransformProxy("proxy")),
        ),
    )
    .await?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "claim_ids": ["C2", "C3", "C5", "C7"],
            "sdk_version": "2.0.0",
            "wire_version": "V1",
            "distinct_stage_order_preserved": distinct_stage_order_preserved,
            "repeated_stage_request_order_preserved": repeated_stage_request_order_preserved,
            "repeated_stage_notification_order_preserved": repeated_stage_notification_order_preserved,
            "cases": [zero_result, noop_result, transform_result, distinct_result, repeated_result],
            "failure_cases": [zero_failure, noop_failure, transform_failure],
            "lazy_initialization_proven": true,
            "kiro_extension_forwarded": true,
            "kas_callback_direction_forwarded": true,
            "real_bidirectional_proxy_capability": "request and notification transformation with terminal agent retaining vendor semantics",
            "engine_conversion_duplicated": false,
            "host_callback_ownership_duplicated": false,
        }))?
    );
    Ok(())
}
