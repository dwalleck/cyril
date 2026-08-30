use agent_client_protocol::{
    ConnectTo, ConnectionTo, Handled, RawJsonRpcMessage, TransportFrame, UntypedMessage,
    UntypedRole,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{
    sync::{Notify, mpsc},
    time::{Duration, timeout},
};

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

async fn run_responsiveness_case() -> Result<bool> {
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let slow_started_tx = event_tx.clone();
    let slow_finished_tx = event_tx.clone();
    let fast_tx = event_tx;
    let release = std::sync::Arc::new(Notify::new());
    let slow_release = std::sync::Arc::clone(&release);
    let component = UntypedRole
        .builder()
        .on_receive_notification(
            async move |_message: SlowNotification, _cx| {
                slow_started_tx.send("slow:started").map_err(|error| {
                    agent_client_protocol::Error::internal_error().data(error.to_string())
                })?;
                slow_release.notified().await;
                slow_finished_tx.send("slow:finished").map_err(|error| {
                    agent_client_protocol::Error::internal_error().data(error.to_string())
                })
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_notification(
            async move |_message: FastNotification, _cx| {
                fast_tx.send("fast:handled").map_err(|error| {
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
    let slow_started = timeout(Duration::from_secs(1), event_rx.recv())
        .await
        .context("slow handler did not start")?
        == Some("slow:started");
    peer.tx
        .unbounded_send(TransportFrame::Single(RawJsonRpcMessage::notification(
            "probe/fast".to_owned(),
            json!({"value": 2}),
        )?))
        .context("send fast notification")?;
    let fast_while_slow_pending = timeout(Duration::from_millis(100), event_rx.recv())
        .await
        .ok()
        .flatten()
        == Some("fast:handled");
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

#[tokio::main]
async fn main() -> Result<()> {
    let (component, events) = untyped_first_component();
    let untyped_first = run_case("untyped-first", component, events).await?;
    let (component, events) = typed_first_component();
    let typed_first = run_case("typed-first-mutation", component, events).await?;
    let (component, events) = typed_only_component();
    let typed_only = run_case("untyped-removed-mutation", component, events).await?;
    let slow_handler_blocks_dispatch = !run_responsiveness_case().await?;
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
