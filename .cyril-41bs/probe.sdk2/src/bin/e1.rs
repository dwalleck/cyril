use std::{cell::RefCell, rc::Rc, thread};

use agent_client_protocol::{ConnectionTo, RawJsonRpcMessage, TransportFrame, UntypedRole};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{sync::mpsc, task::LocalSet};

#[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcNotification)]
#[notification(method = "_probe/domain-event")]
struct ProbeNotification {
    sequence: u64,
    message: String,
}

#[derive(Debug)]
struct DomainEvent {
    sequence: u64,
    message: String,
    protocol_thread: String,
}

fn assert_send_static<T: Send + 'static>(_: &T) {}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    let (domain_tx, mut domain_rx) = mpsc::channel::<DomainEvent>(1);
    let server = UntypedRole.builder().on_receive_notification(
        async move |notification: ProbeNotification, _cx: ConnectionTo<UntypedRole>| {
            let event = DomainEvent {
                sequence: notification.sequence,
                message: notification.message,
                protocol_thread: format!("{:?}", thread::current().id()),
            };
            domain_tx.send(event).await.map_err(|error| {
                agent_client_protocol::Error::internal_error().data(error.to_string())
            })?;
            Ok(())
        },
        agent_client_protocol::on_receive_notification!(),
    );
    assert_send_static(&server);

    let (server_transport, peer) = agent_client_protocol::Channel::duplex();
    let server_task = tokio::spawn(server.connect_to(server_transport));
    let notification = RawJsonRpcMessage::notification(
        "_probe/domain-event".to_owned(),
        serde_json::to_value(ProbeNotification {
            sequence: 1,
            message: "bounded-handoff".to_owned(),
        })?,
    )?;
    peer.tx
        .unbounded_send(TransportFrame::Single(notification))
        .context("send raw SDK notification")?;
    drop(peer);

    let local = LocalSet::new();
    let state = Rc::new(RefCell::new(Vec::<String>::new()));
    let state_for_actor = Rc::clone(&state);
    let domain_thread = format!("{:?}", thread::current().id());
    let event = local
        .run_until(async move {
            let event = domain_rx.recv().await.context("domain actor receive")?;
            state_for_actor
                .try_borrow_mut()
                .map_err(|error| anyhow::anyhow!("domain state already borrowed: {error}"))?
                .push(format!("{}:{}", event.sequence, event.message));
            Ok::<DomainEvent, anyhow::Error>(event)
        })
        .await?;

    server_task.await.context("join SDK server actor")??;

    let recorded = state
        .try_borrow()
        .map_err(|error| anyhow::anyhow!("domain state still mutably borrowed: {error}"))?
        .clone();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "claim_ids": ["C1"],
            "sdk_component_is_send_static": true,
            "bounded_channel_capacity": 1,
            "domain_state_type": "Rc<RefCell<Vec<String>>>",
            "domain_state_crossed_send_boundary": false,
            "unsafe_used": false,
            "shared_lock_used_for_domain_state": false,
            "protocol_thread": event.protocol_thread,
            "domain_thread": domain_thread,
            "threads_differed_in_this_run": event.protocol_thread != domain_thread,
            "recorded": recorded,
        }))?
    );
    Ok(())
}
