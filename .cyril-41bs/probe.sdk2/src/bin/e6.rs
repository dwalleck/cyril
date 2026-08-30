use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration as StdDuration, Instant};

use agent_client_protocol::schema::{
    InitializeProxyRequest, ProtocolVersion,
    v1::{Implementation, InitializeRequest, InitializeResponse},
};
use agent_client_protocol::{
    Agent, Channel, Client, Conductor, ConnectTo, ConnectionTo, Error, ErrorCode, Proxy, Responder,
};
use agent_client_protocol_conductor::{ConductorImpl, ProxiesAndAgent};
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::time::{Duration, timeout};

const MAX_WIRE_ENTRIES: usize = 1_000;
const MAX_WIRE_ENTRY_BYTES: usize = 256 * 1024;
const MAX_WIRE_BYTES: usize = 1024 * 1024;
const WIRE_PARSE_LIMIT: StdDuration = StdDuration::from_millis(100);
const CANCELLATION_WAIT: Duration = Duration::from_secs(2);
const WIRE_OVERFLOW_MARKER: &str = "e6:wire-entry-limit-exceeded";
const EXPECTED_CANCELLATION_EVENT: &str = "agent:cancellation-observed";

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Default,
    WrongCancellationEvent,
    CancellationTimeout,
    CancellationChannelClosed,
    MalformedWireEntry,
    WrongTerminalData,
    WrongResponseId,
}

impl Mode {
    fn parse() -> Result<Self> {
        let mut args = std::env::args().skip(1);
        let mode = match args.next() {
            None => Self::Default,
            Some(value) => match value.as_str() {
                "wrong-cancellation-event" => Self::WrongCancellationEvent,
                "cancellation-timeout" => Self::CancellationTimeout,
                "cancellation-channel-closed" => Self::CancellationChannelClosed,
                "malformed-wire-entry" => Self::MalformedWireEntry,
                "wrong-terminal-data" => Self::WrongTerminalData,
                "wrong-response-id" => Self::WrongResponseId,
                _ => bail!("e6: unknown mode"),
            },
        };
        if args.next().is_some() {
            bail!("e6: unexpected extra argument");
        }
        Ok(mode)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CancellationSignal {
    Expected,
    WrongEvent,
    Silent,
    ChannelClosed,
}

impl CancellationSignal {
    fn event(self) -> Option<&'static str> {
        match self {
            Self::Expected => Some(EXPECTED_CANCELLATION_EVENT),
            Self::WrongEvent => Some("agent:cancellation-unexpected"),
            Self::Silent | Self::ChannelClosed => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WireMutation {
    None,
    MalformedEntry,
    WrongResponseId,
}

#[derive(Debug)]
enum CancellationOutcome {
    Event(String),
    Timeout,
    ChannelClosed,
}

impl CancellationOutcome {
    fn event(&self) -> Option<&str> {
        match self {
            Self::Event(event) => Some(event.as_str()),
            Self::Timeout | Self::ChannelClosed => None,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Event(_) => "event",
            Self::Timeout => "timeout",
            Self::ChannelClosed => "channel-closed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WireDirection {
    ClientToAgent,
    AgentToClient,
}

impl WireDirection {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "client->agent" => Some(Self::ClientToAgent),
            "agent->client" => Some(Self::AgentToClient),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::ClientToAgent => "client->agent",
            Self::AgentToClient => "agent->client",
        }
    }
}

#[derive(Debug)]
struct ParsedWireEntry {
    direction: WireDirection,
    value: Value,
}

fn terminal_error_leaf(mut value: &Value) -> Option<&Value> {
    loop {
        let Value::Object(object) = value else {
            return Some(value);
        };
        if object.len() != 2 || object.get("spawned_at").and_then(Value::as_str).is_none() {
            return None;
        }
        value = object.get("data")?;
    }
}

#[derive(Debug, Default)]
struct LogBuffer {
    entries: Vec<String>,
    retained_bytes: usize,
}

#[derive(Clone, Debug)]
struct Log(Arc<Mutex<LogBuffer>>);

impl Log {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(LogBuffer::default())))
    }

    fn lock(&self) -> MutexGuard<'_, LogBuffer> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn push(&self, value: impl Into<String>) {
        let value = value.into();
        let mut log = self.lock();
        if log
            .entries
            .last()
            .is_some_and(|entry| entry == WIRE_OVERFLOW_MARKER)
        {
            return;
        }
        let exceeds_total_bytes = log
            .retained_bytes
            .checked_add(value.len())
            .is_none_or(|bytes| bytes > MAX_WIRE_BYTES);
        if log.entries.len() >= MAX_WIRE_ENTRIES
            || value.len() > MAX_WIRE_ENTRY_BYTES
            || exceeds_total_bytes
        {
            if log.entries.len() >= MAX_WIRE_ENTRIES
                && let Some(last) = log.entries.last_mut()
            {
                *last = WIRE_OVERFLOW_MARKER.to_owned();
            } else {
                log.entries.push(WIRE_OVERFLOW_MARKER.to_owned());
            }
            return;
        }
        log.retained_bytes += value.len();
        log.entries.push(value);
    }

    fn snapshot(&self) -> Vec<String> {
        self.lock().entries.clone()
    }
}

struct FakeAgent {
    log: Log,
    cancellation_tx: Option<mpsc::UnboundedSender<String>>,
    cancellation_signal: CancellationSignal,
}

impl ConnectTo<Client> for FakeAgent {
    async fn connect_to(self, client: impl ConnectTo<Agent>) -> Result<(), Error> {
        let init_log = self.log.clone();
        let request_log = self.log.clone();
        let cancellation_tx = self.cancellation_tx;
        let cancellation_signal = self.cancellation_signal;
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
                            if cancellation_signal == CancellationSignal::ChannelClosed {
                                return Ok(());
                            }
                            if let Some(event) = cancellation_signal.event() {
                                let tx = match tx {
                                    Some(tx) => tx,
                                    None => {
                                        return Err(Error::internal_error()
                                            .data("cancellation sender unavailable"));
                                    }
                                };
                                tx.send(event.to_owned()).map_err(|error| {
                                    Error::internal_error().data(error.to_string())
                                })?;
                            }
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

struct FailingAgent {
    terminal_data: &'static str,
}

impl ConnectTo<Client> for FailingAgent {
    async fn connect_to(self, client: impl ConnectTo<Agent>) -> Result<(), Error> {
        let terminal_data = self.terminal_data;
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
            .connect_with(client, async move |_cx| {
                tokio::time::sleep(Duration::from_millis(25)).await;
                let error: Error = ErrorCode::InternalError.into();
                Err::<(), Error>(error.data(json!(terminal_data)))
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

fn fake_agent_parts(
    log: Log,
    cancellation_signal: CancellationSignal,
) -> (FakeAgent, mpsc::UnboundedReceiver<String>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let cancellation_tx = match cancellation_signal {
        CancellationSignal::ChannelClosed => None,
        CancellationSignal::Expected
        | CancellationSignal::WrongEvent
        | CancellationSignal::Silent => Some(tx),
    };
    (
        FakeAgent {
            log,
            cancellation_tx,
            cancellation_signal,
        },
        rx,
    )
}

fn mutate_wire(wire: &mut Vec<String>, mutation: WireMutation) -> Result<()> {
    match mutation {
        WireMutation::None => Ok(()),
        WireMutation::MalformedEntry => {
            wire.insert(0, "client->agent:{malformed".to_owned());
            Ok(())
        }
        WireMutation::WrongResponseId => {
            for entry in wire.iter_mut() {
                let (direction, raw) = match entry.split_once(':') {
                    Some(parts) => parts,
                    None => continue,
                };
                if direction != "agent->client" {
                    continue;
                }
                let mut value: Value = serde_json::from_str(raw)?;
                let has_probe_result = value
                    .pointer("/result/value")
                    .and_then(Value::as_str)
                    .is_some();
                if has_probe_result {
                    value["id"] = json!("wrong-response-id");
                    *entry = format!("agent->client:{}", serde_json::to_string(&value)?);
                    return Ok(());
                }
            }
            bail!("e6: wrong response id setup missing terminal response")
        }
    }
}

fn parse_wire(wire: &[String]) -> Result<(Vec<ParsedWireEntry>, u128)> {
    if wire.last().map(String::as_str) == Some(WIRE_OVERFLOW_MARKER) {
        bail!("e6: wire entry cap exceeded limit={MAX_WIRE_ENTRIES}");
    }
    if wire.len() > MAX_WIRE_ENTRIES {
        bail!(
            "e6: wire entry cap exceeded count={} limit={MAX_WIRE_ENTRIES}",
            wire.len()
        );
    }
    let started = Instant::now();
    let mut parsed = Vec::with_capacity(wire.len());
    for (index, entry) in wire.iter().enumerate() {
        if started.elapsed() > WIRE_PARSE_LIMIT {
            bail!("e6: wire parse exceeded 100ms index={index} direction=unknown");
        }
        let (direction_text, raw) = match entry.split_once(':') {
            Some(parts) => parts,
            None => bail!("e6: malformed wire entry index={index} direction=unknown"),
        };
        let direction = match WireDirection::parse(direction_text) {
            Some(direction) => direction,
            None => bail!("e6: malformed wire entry index={index} direction={direction_text}"),
        };
        let value_result: std::result::Result<Value, serde_json::Error> = serde_json::from_str(raw);
        if started.elapsed() > WIRE_PARSE_LIMIT {
            bail!(
                "e6: wire parse exceeded 100ms index={index} direction={}",
                direction.label()
            );
        }
        let value = match value_result {
            Ok(value) => value,
            Err(_) => bail!(
                "e6: malformed wire entry index={index} direction={}",
                direction.label()
            ),
        };
        parsed.push(ParsedWireEntry { direction, value });
    }
    let elapsed = started.elapsed();
    if elapsed > WIRE_PARSE_LIMIT {
        bail!("e6: wire parse exceeded 100ms index=end direction=none");
    }
    Ok((parsed, elapsed.as_millis()))
}

fn first_probe_request_id(entries: &[ParsedWireEntry]) -> Option<Value> {
    for entry in entries {
        if entry.direction != WireDirection::ClientToAgent {
            continue;
        }
        if entry.value.get("method").and_then(Value::as_str) == Some("_kiro/probe")
            && let Some(id) = entry.value.get("id")
        {
            return Some(id.clone());
        }
    }
    None
}

fn first_probe_response_id(entries: &[ParsedWireEntry]) -> Option<Value> {
    for entry in entries {
        if entry.direction != WireDirection::AgentToClient {
            continue;
        }
        let has_probe_result = entry
            .value
            .pointer("/result/value")
            .and_then(Value::as_str)
            .is_some();
        if has_probe_result && let Some(id) = entry.value.get("id") {
            return Some(id.clone());
        }
    }
    None
}

async fn run_case<C>(
    name: &'static str,
    conductor: C,
    agent_log: Log,
    mut cancellation_rx: mpsc::UnboundedReceiver<String>,
    wire_mutation: WireMutation,
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
            let cancellation_outcome =
                match timeout(CANCELLATION_WAIT, cancellation_rx.recv()).await {
                    Ok(Some(event)) => CancellationOutcome::Event(event),
                    Ok(None) => CancellationOutcome::ChannelClosed,
                    Err(_) => CancellationOutcome::Timeout,
                };
            Ok::<_, Error>((response, cancellation_outcome))
        })
        .await
        .map_err(|error| anyhow::anyhow!("{name} conductor case failed: {error:?}"))?;
    let (response, cancellation_outcome) = response;
    let mut wire = wire_after.snapshot();
    mutate_wire(&mut wire, wire_mutation)?;
    let (parsed_wire, wire_parse_elapsed_ms) = parse_wire(&wire)?;
    let outer_request_id = first_probe_request_id(&parsed_wire);
    let outer_response_id = first_probe_response_id(&parsed_wire);
    let cancellation_count = parsed_wire
        .iter()
        .filter(|entry| {
            entry.direction == WireDirection::ClientToAgent
                && entry.value.get("method").and_then(Value::as_str) == Some("$/cancel_request")
        })
        .count();
    let terminal_cancellation_event = cancellation_outcome.event().map(str::to_owned);
    let terminal_cancellation_observed =
        terminal_cancellation_event.as_deref() == Some(EXPECTED_CANCELLATION_EVENT);
    let response_identity_preserved =
        outer_request_id.is_some() && outer_request_id == outer_response_id;
    Ok(json!({
        "name": name,
        "response": response.value,
        "notifications": notifications.snapshot(),
        "terminal_cancellation_event": terminal_cancellation_event,
        "cancellation_outcome": cancellation_outcome.label(),
        "terminal_cancellation_observed": terminal_cancellation_observed,
        "agent_events": agent_log.snapshot(),
        "outer_request_id": outer_request_id,
        "outer_response_id": outer_response_id,
        "response_identity_preserved": response_identity_preserved,
        "cancellation_frames": cancellation_count,
        "cancellation_forwarded_once": cancellation_count == 1,
        "wire_entries_parsed": parsed_wire.len(),
        "wire_parse_elapsed_ms": wire_parse_elapsed_ms,
        "wire_parse_budget_ms": WIRE_PARSE_LIMIT.as_millis(),
        "wire_parse_within_budget": true,
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
        Err(error) => {
            let terminal_error_code = error.code;
            let terminal_error_data = error.data;
            ensure!(
                matches!(terminal_error_code, ErrorCode::InternalError),
                "e6: terminal error code mismatch"
            );
            let terminal_error_leaf = terminal_error_data
                .as_ref()
                .and_then(terminal_error_leaf)
                .cloned();
            ensure!(
                terminal_error_leaf == Some(json!("agent-crash")),
                "e6: terminal error data mismatch"
            );
            Ok(json!({
                "name": name,
                "connection_failed": true,
                "terminal_error_code": terminal_error_code,
                "terminal_error_data": terminal_error_data,
                "terminal_error_leaf": terminal_error_leaf,
                "terminal_error_code_is_internal": true,
                "terminal_error_data_is_agent_crash": true,
                "terminal_error_preserved": true,
            }))
        }
    }
}

async fn run_negative_mode(mode: Mode) -> Result<()> {
    match mode {
        Mode::WrongCancellationEvent
        | Mode::CancellationTimeout
        | Mode::CancellationChannelClosed
        | Mode::MalformedWireEntry
        | Mode::WrongResponseId => {
            let log = Log::new();
            let signal = match mode {
                Mode::WrongCancellationEvent => CancellationSignal::WrongEvent,
                Mode::CancellationTimeout => CancellationSignal::Silent,
                Mode::CancellationChannelClosed => CancellationSignal::ChannelClosed,
                Mode::MalformedWireEntry | Mode::WrongResponseId => CancellationSignal::Expected,
                Mode::Default | Mode::WrongTerminalData => unreachable!(),
            };
            let wire_mutation = match mode {
                Mode::MalformedWireEntry => WireMutation::MalformedEntry,
                Mode::WrongResponseId => WireMutation::WrongResponseId,
                Mode::WrongCancellationEvent
                | Mode::CancellationTimeout
                | Mode::CancellationChannelClosed => WireMutation::None,
                Mode::Default | Mode::WrongTerminalData => unreachable!(),
            };
            let (agent, cancellation_rx) = fake_agent_parts(log.clone(), signal);
            let conductor = ConductorImpl::new_agent("negative", ProxiesAndAgent::new(agent));
            let result = run_case("negative", conductor, log, cancellation_rx, wire_mutation).await;
            match mode {
                Mode::WrongCancellationEvent => {
                    let result = result?;
                    ensure!(
                        result["terminal_cancellation_event"] == EXPECTED_CANCELLATION_EVENT,
                        "e6: wrong cancellation event"
                    );
                    bail!("e6: wrong cancellation event was not detected")
                }
                Mode::CancellationTimeout => {
                    let result = result?;
                    ensure!(
                        result["cancellation_outcome"] == "event",
                        "e6: cancellation wait timed out"
                    );
                    bail!("e6: cancellation timeout was not detected")
                }
                Mode::CancellationChannelClosed => {
                    let result = result?;
                    ensure!(
                        result["cancellation_outcome"] == "event",
                        "e6: cancellation channel closed"
                    );
                    bail!("e6: cancellation channel closure was not detected")
                }
                Mode::MalformedWireEntry => match result {
                    Ok(_) => bail!("e6: malformed wire entry was not detected"),
                    Err(error) => Err(error),
                },
                Mode::WrongResponseId => {
                    let result = result?;
                    ensure!(
                        result["response_identity_preserved"] == true,
                        "e6: response identity mismatch"
                    );
                    bail!("e6: wrong response id was not detected")
                }
                Mode::Default | Mode::WrongTerminalData => unreachable!(),
            }
        }
        Mode::WrongTerminalData => {
            let conductor = ConductorImpl::new_agent(
                "negative-failure",
                ProxiesAndAgent::new(FailingAgent {
                    terminal_data: "not-agent-crash",
                }),
            );
            match run_failure_case("negative-failure", conductor).await {
                Ok(_) => bail!("e6: wrong terminal data was not detected"),
                Err(error) => Err(error),
            }
        }
        Mode::Default => unreachable!(),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mode = Mode::parse()?;
    if mode != Mode::Default {
        return run_negative_mode(mode).await;
    }

    let zero_log = Log::new();
    let (zero_agent, zero_cancel_rx) =
        fake_agent_parts(zero_log.clone(), CancellationSignal::Expected);
    let zero = ConductorImpl::new_agent("zero", ProxiesAndAgent::new(zero_agent));
    let zero_result = run_case(
        "zero-proxy",
        zero,
        zero_log,
        zero_cancel_rx,
        WireMutation::None,
    )
    .await?;

    let noop_log = Log::new();
    let (noop_agent, noop_cancel_rx) =
        fake_agent_parts(noop_log.clone(), CancellationSignal::Expected);
    let noop = ConductorImpl::new_agent(
        "noop",
        ProxiesAndAgent::new(noop_agent).proxy(Proxy.builder()),
    );
    let noop_result = run_case(
        "no-op-proxy",
        noop,
        noop_log,
        noop_cancel_rx,
        WireMutation::None,
    )
    .await?;

    let transform_log = Log::new();
    let (transform_agent, transform_cancel_rx) =
        fake_agent_parts(transform_log.clone(), CancellationSignal::Expected);
    let transform = ConductorImpl::new_agent(
        "transform",
        ProxiesAndAgent::new(transform_agent).proxy(TransformProxy("proxy")),
    );
    let transform_result = run_case(
        "transforming-proxy",
        transform,
        transform_log,
        transform_cancel_rx,
        WireMutation::None,
    )
    .await?;

    let distinct_log = Log::new();
    let (distinct_agent, distinct_cancel_rx) =
        fake_agent_parts(distinct_log.clone(), CancellationSignal::Expected);
    let distinct = ConductorImpl::new_agent(
        "distinct",
        ProxiesAndAgent::new(distinct_agent)
            .proxy(Proxy.builder())
            .proxy(TransformProxy("proxy")),
    );
    let distinct_result = run_case(
        "distinct-two-stage",
        distinct,
        distinct_log,
        distinct_cancel_rx,
        WireMutation::None,
    )
    .await?;

    let repeated_log = Log::new();
    let (repeated_agent, repeated_cancel_rx) =
        fake_agent_parts(repeated_log.clone(), CancellationSignal::Expected);
    let repeated = ConductorImpl::new_agent(
        "repeated",
        ProxiesAndAgent::new(repeated_agent)
            .proxy(TransformProxy("first"))
            .proxy(TransformProxy("second")),
    );
    let repeated_result = run_case(
        "repeated-transform-stage",
        repeated,
        repeated_log,
        repeated_cancel_rx,
        WireMutation::None,
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
    for case in [
        &zero_result,
        &noop_result,
        &transform_result,
        &distinct_result,
        &repeated_result,
    ] {
        ensure!(
            case["terminal_cancellation_event"] == EXPECTED_CANCELLATION_EVENT
                && case["cancellation_outcome"] == "event"
                && case["terminal_cancellation_observed"] == true,
            "C5 exact cancellation event contract failed"
        );
        ensure!(
            case["cancellation_frames"] == 1 && case["cancellation_forwarded_once"] == true,
            "C5 cancellation frame cardinality contract failed"
        );
        ensure!(
            case["response_identity_preserved"] == true,
            "C2 outer response identity contract failed"
        );
    }
    let zero_failure = run_failure_case(
        "zero-proxy-failure",
        ConductorImpl::new_agent(
            "zero-failure",
            ProxiesAndAgent::new(FailingAgent {
                terminal_data: "agent-crash",
            }),
        ),
    )
    .await?;
    let noop_failure = run_failure_case(
        "no-op-proxy-failure",
        ConductorImpl::new_agent(
            "noop-failure",
            ProxiesAndAgent::new(FailingAgent {
                terminal_data: "agent-crash",
            })
            .proxy(Proxy.builder()),
        ),
    )
    .await?;
    let transform_failure = run_failure_case(
        "transforming-proxy-failure",
        ConductorImpl::new_agent(
            "transform-failure",
            ProxiesAndAgent::new(FailingAgent {
                terminal_data: "agent-crash",
            })
            .proxy(TransformProxy("proxy")),
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
        }))?
    );
    Ok(())
}
