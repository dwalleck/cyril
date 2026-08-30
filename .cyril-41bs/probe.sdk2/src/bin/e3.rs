use agent_client_protocol::{
    ConnectTo, ConnectionTo, Handled, RawJsonRpcMessage, TransportFrame, UntypedMessage,
    UntypedRole,
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{
    sync::{Notify, mpsc},
    time::{Duration, timeout},
};

const WRONG_CONTAINMENT_DIAGNOSTIC: &str =
    "wrong-containment-expectation: untyped-first containment unexpectedly matched";
const CLOSED_CHANNEL_DIAGNOSTIC: &str =
    "closed-channel-control: event channel closed while waiting for slow:started";
const UNEXPECTED_EVENT_DIAGNOSTIC: &str =
    "unexpected-event-control: unexpected event while waiting for slow:started";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Default,
    WrongContainmentExpectation,
    ClosedChannelControl,
    UnexpectedEventControl,
}

impl Mode {
    fn from_args() -> Result<Self> {
        match std::env::args().nth(1).as_deref() {
            None => Ok(Self::Default),
            Some("wrong-containment-expectation") => Ok(Self::WrongContainmentExpectation),
            Some("closed-channel-control") => Ok(Self::ClosedChannelControl),
            Some("unexpected-event-control") => Ok(Self::UnexpectedEventControl),
            Some(mode) => bail!("unknown e3 mode: {mode}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcNotification)]
#[notification(method = "session/update")]
struct StrictSessionNotification {
    update: StrictUpdate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "sessionUpdate", rename_all = "snake_case")]
enum StrictUpdate {
    Known { value: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcNotification)]
#[notification(method = "probe/slow")]
struct SlowNotification {
    value: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcNotification)]
#[notification(method = "probe/fast")]
struct FastNotification {
    value: u8,
}

#[derive(Debug, Serialize)]
struct CaseResult {
    name: &'static str,
    connection_ok: bool,
    error: Option<String>,
    events: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum WaitOutcome {
    Event(String),
    Closed,
    TimedOut,
}

async fn wait_for_event(
    events: &mut mpsc::UnboundedReceiver<String>,
    duration: Duration,
) -> WaitOutcome {
    match timeout(duration, events.recv()).await {
        Ok(Some(event)) => WaitOutcome::Event(event),
        Ok(None) => WaitOutcome::Closed,
        Err(_) => WaitOutcome::TimedOut,
    }
}

async fn run_case<C>(
    name: &'static str,
    component: C,
    mut events: mpsc::UnboundedReceiver<String>,
) -> Result<CaseResult>
where
    C: ConnectTo<UntypedRole>,
{
    let (transport, peer) = agent_client_protocol::Channel::duplex();
    let task = tokio::spawn(component.connect_to(transport));
    let unknown = RawJsonRpcMessage::notification(
        "session/update".to_owned(),
        json!({
            "update": {
                "sessionUpdate": "future_variant",
                "payload": {"preserved": true}
            }
        }),
    )?;
    peer.tx
        .unbounded_send(TransportFrame::Single(unknown))
        .context("send unknown standard update")?;
    drop(peer);
    let result = task.await.context("join handler component")?;
    let mut observed = Vec::new();
    while let Ok(event) = events.try_recv() {
        observed.push(event);
    }
    Ok(CaseResult {
        name,
        connection_ok: result.is_ok(),
        error: result.err().map(|error| error.to_string()),
        events: observed,
    })
}

async fn run_responsiveness_case(mode: Mode) -> Result<bool> {
    if mode == Mode::ClosedChannelControl {
        let (closed_tx, mut closed_rx) = mpsc::unbounded_channel::<String>();
        drop(closed_tx);
        match wait_for_event(&mut closed_rx, Duration::from_secs(1)).await {
            WaitOutcome::Closed => bail!("{CLOSED_CHANNEL_DIAGNOSTIC}"),
            WaitOutcome::TimedOut => {
                bail!("closed-channel-control: timeout waiting for slow:started")
            }
            WaitOutcome::Event(event) => {
                bail!("closed-channel-control: unexpected event {event}")
            }
        };
    }

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<String>();
    let slow_started_tx = event_tx.clone();
    let slow_finished_tx = event_tx.clone();
    if mode == Mode::UnexpectedEventControl {
        event_tx
            .send("unexpected:control".to_owned())
            .map_err(|error| {
                agent_client_protocol::Error::internal_error().data(error.to_string())
            })?;
    }
    let fast_tx = event_tx;
    let release = std::sync::Arc::new(Notify::new());
    let slow_release = std::sync::Arc::clone(&release);
    let component = UntypedRole
        .builder()
        .on_receive_notification(
            async move |_message: SlowNotification, _cx| {
                slow_started_tx
                    .send("slow:started".to_owned())
                    .map_err(|error| {
                        agent_client_protocol::Error::internal_error().data(error.to_string())
                    })?;
                slow_release.notified().await;
                slow_finished_tx
                    .send("slow:finished".to_owned())
                    .map_err(|error| {
                        agent_client_protocol::Error::internal_error().data(error.to_string())
                    })
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_notification(
            async move |_message: FastNotification, _cx| {
                fast_tx.send("fast:handled".to_owned()).map_err(|error| {
                    agent_client_protocol::Error::internal_error().data(error.to_string())
                })
            },
            agent_client_protocol::on_receive_notification!(),
        );
    let (transport, peer) = agent_client_protocol::Channel::duplex();
    let task = tokio::spawn(component.connect_to(transport));
    peer.tx
        .unbounded_send(TransportFrame::Single(RawJsonRpcMessage::notification(
            "probe/slow".to_owned(),
            json!({"value": 1}),
        )?))
        .context("send slow notification")?;
    let slow_started = match wait_for_event(&mut event_rx, Duration::from_secs(1)).await {
        WaitOutcome::Event(event) if event == "slow:started" => true,
        WaitOutcome::Event(_) if mode == Mode::UnexpectedEventControl => {
            bail!("{UNEXPECTED_EVENT_DIAGNOSTIC}")
        }
        WaitOutcome::Event(event) => {
            bail!("unexpected event while waiting for slow:started: {event}")
        }
        WaitOutcome::Closed => bail!("event channel closed while waiting for slow:started"),
        WaitOutcome::TimedOut => bail!("timeout waiting for slow:started"),
    };
    peer.tx
        .unbounded_send(TransportFrame::Single(RawJsonRpcMessage::notification(
            "probe/fast".to_owned(),
            json!({"value": 2}),
        )?))
        .context("send fast notification")?;
    let fast_while_slow_pending =
        match wait_for_event(&mut event_rx, Duration::from_millis(100)).await {
            WaitOutcome::TimedOut => false,
            WaitOutcome::Event(event) if event == "fast:handled" => true,
            WaitOutcome::Event(event) => {
                bail!("unexpected event while waiting for fast:handled: {event}")
            }
            WaitOutcome::Closed => bail!("event channel closed while waiting for fast:handled"),
        };
    release.notify_one();
    drop(peer);
    let connection_ok = task.await.context("join responsiveness component")?.is_ok();
    Ok(slow_started && fast_while_slow_pending && connection_ok)
}

fn untyped_first_component() -> (impl ConnectTo<UntypedRole>, mpsc::UnboundedReceiver<String>) {
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let untyped_tx = event_tx.clone();
    let typed_tx = event_tx;
    let component = UntypedRole
        .builder()
        .on_receive_notification(
            async move |message: UntypedMessage, cx: ConnectionTo<UntypedRole>| {
                let is_unknown_update = message.method == "session/update"
                    && message
                        .params
                        .pointer("/update/sessionUpdate")
                        .and_then(serde_json::Value::as_str)
                        == Some("future_variant");
                if is_unknown_update {
                    untyped_tx
                        .send("untyped:unknown-contained".to_owned())
                        .map_err(|error| {
                            agent_client_protocol::Error::internal_error().data(error.to_string())
                        })?;
                    return Ok(Handled::Yes);
                }
                Ok(Handled::No {
                    message: (message, cx),
                    retry: false,
                })
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_notification(
            async move |_message: StrictSessionNotification, _cx: ConnectionTo<UntypedRole>| {
                typed_tx.send("typed:known".to_owned()).map_err(|error| {
                    agent_client_protocol::Error::internal_error().data(error.to_string())
                })?;
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        );
    (component, event_rx)
}

fn typed_first_component() -> (impl ConnectTo<UntypedRole>, mpsc::UnboundedReceiver<String>) {
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let typed_tx = event_tx.clone();
    let untyped_tx = event_tx;
    let component = UntypedRole
        .builder()
        .on_receive_notification(
            async move |_message: StrictSessionNotification, _cx: ConnectionTo<UntypedRole>| {
                typed_tx.send("typed:known".to_owned()).map_err(|error| {
                    agent_client_protocol::Error::internal_error().data(error.to_string())
                })?;
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_notification(
            async move |_message: UntypedMessage, _cx: ConnectionTo<UntypedRole>| {
                untyped_tx
                    .send("untyped:late".to_owned())
                    .map_err(|error| {
                        agent_client_protocol::Error::internal_error().data(error.to_string())
                    })?;
                Ok(Handled::Yes)
            },
            agent_client_protocol::on_receive_notification!(),
        );
    (component, event_rx)
}

fn typed_only_component() -> (impl ConnectTo<UntypedRole>, mpsc::UnboundedReceiver<String>) {
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let component = UntypedRole.builder().on_receive_notification(
        async move |_message: StrictSessionNotification, _cx: ConnectionTo<UntypedRole>| {
            event_tx.send("typed:known".to_owned()).map_err(|error| {
                agent_client_protocol::Error::internal_error().data(error.to_string())
            })?;
            Ok(())
        },
        agent_client_protocol::on_receive_notification!(),
    );
    (component, event_rx)
}

fn assert_default_case_results(
    untyped_first: &CaseResult,
    typed_first: &CaseResult,
    typed_only: &CaseResult,
) -> Result<()> {
    ensure!(
        untyped_first.connection_ok && untyped_first.error.is_none(),
        "C5 untyped-first connection failed"
    );
    ensure!(
        untyped_first.events == vec!["untyped:unknown-contained".to_owned()],
        "C5 untyped-first event list changed"
    );
    ensure!(
        typed_first.connection_ok && typed_first.error.is_none(),
        "C5 typed-first connection failed"
    );
    ensure!(
        typed_first.events == Vec::<String>::new(),
        "C5 typed-first unexpectedly handled an unknown update"
    );
    ensure!(
        typed_only.connection_ok && typed_only.error.is_none(),
        "C5 typed-only connection failed"
    );
    ensure!(
        typed_only.events == Vec::<String>::new(),
        "C5 typed-only unexpectedly handled an unknown update"
    );
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let mode = Mode::from_args()?;
    let (component, events) = untyped_first_component();
    let untyped_first = run_case("untyped-first", component, events).await?;
    let (component, events) = typed_first_component();
    let typed_first = run_case("typed-first-mutation", component, events).await?;
    let (component, events) = typed_only_component();
    let typed_only = run_case("untyped-removed-mutation", component, events).await?;
    assert_default_case_results(&untyped_first, &typed_first, &typed_only)?;
    if mode == Mode::WrongContainmentExpectation {
        ensure!(
            untyped_first.events.is_empty(),
            "{WRONG_CONTAINMENT_DIAGNOSTIC}"
        );
    }
    let slow_handler_blocks_dispatch = !run_responsiveness_case(mode).await?;
    ensure!(
        slow_handler_blocks_dispatch,
        "C5 expected the SDK slow-handler negative control to block dispatch"
    );

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "claim_ids": ["C5"],
            "untyped_first": untyped_first,
            "typed_first_mutation": typed_first,
            "untyped_removed_mutation": typed_only,
            "slow_handler_blocks_dispatch": slow_handler_blocks_dispatch,
            "mutation_expected_to_fail": true,
        }))?
    );
    Ok(())
}
