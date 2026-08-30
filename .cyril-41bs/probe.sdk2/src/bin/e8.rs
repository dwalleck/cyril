use std::{
    sync::{Arc, Mutex, MutexGuard},
    thread,
    time::{Duration, Instant},
};

use agent_client_protocol::{
    Channel, Error, RawJsonRpcMessage, TransportFrame, schema::v1::RequestId,
};
use anyhow::{Context, Result, ensure};
use futures_util::StreamExt as _;
use serde_json::json;

type InlineObserver = Box<dyn Fn(&RawJsonRpcMessage) + Send>;

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    match value.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn message_method(message: &RawJsonRpcMessage) -> String {
    let value = serde_json::to_value(message)
        .unwrap_or_else(|error| json!({"method": format!("serialization-error:{error}")}));
    value
        .get("method")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("response")
        .to_owned()
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    let (mut producer, bridge_left) = Channel::duplex();
    let (bridge_right, mut consumer) = Channel::duplex();
    let slow_observed = Arc::new(Mutex::new(Vec::<(String, u128)>::new()));
    let fast_observed = Arc::new(Mutex::new(Vec::<(String, u128)>::new()));
    let slow_by_callback = Arc::clone(&slow_observed);
    let fast_by_callback = Arc::clone(&fast_observed);
    let started = Instant::now();
    let slow_callback = move |message: &RawJsonRpcMessage| {
        thread::sleep(Duration::from_millis(50));
        lock(&slow_by_callback).push((message_method(message), started.elapsed().as_millis()));
    };
    let fast_callback = move |message: &RawJsonRpcMessage| {
        lock(&fast_by_callback).push((message_method(message), started.elapsed().as_millis()));
    };
    let inline_observers: [InlineObserver; 2] = [Box::new(slow_callback), Box::new(fast_callback)];
    let started = Instant::now();
    let bridge = tokio::spawn(Channel::bridge_with_inspection(
        bridge_left,
        bridge_right,
        move |message| {
            for observer in &inline_observers {
                observer(message);
            }
            Ok(())
        },
        |_message| Ok(()),
    ));

    let frames = [
        RawJsonRpcMessage::notification("session/update".to_owned(), json!({"seq": 1}))?,
        RawJsonRpcMessage::notification("session/update".to_owned(), json!({"seq": 2}))?,
        RawJsonRpcMessage::request(
            "session/request_permission".to_owned(),
            json!({"options": []}),
            RequestId::Number(7),
        )?,
        RawJsonRpcMessage::notification("session/update".to_owned(), json!({"seq": 3}))?,
    ];
    let producer_tx = producer.tx.clone();
    let streaming_producer = tokio::spawn(async move {
        for frame in frames {
            producer_tx
                .unbounded_send(TransportFrame::Single(frame))
                .map_err(|error| Error::internal_error().data(error.to_string()))?;
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        Ok::<(), Error>(())
    });

    let mut delivered = Vec::new();
    for _ in 0..4 {
        let frame = consumer
            .rx
            .next()
            .await
            .context("receive observer-pressure frame")?;
        let method = match frame {
            TransportFrame::Single(message) => message_method(&message),
            other => format!("unexpected:{other:?}"),
        };
        if method == "session/request_permission" {
            consumer
                .tx
                .unbounded_send(TransportFrame::Single(RawJsonRpcMessage::response(
                    RequestId::Number(7),
                    Ok(json!({"outcome": "allow"})),
                )))
                .context("answer permission on the same linear path")?;
        }
        delivered.push((method, started.elapsed().as_millis()));
    }
    streaming_producer
        .await
        .context("join streaming producer")??;
    let permission_response = tokio::time::timeout(Duration::from_secs(1), producer.rx.next())
        .await
        .context("permission response timed out")?
        .context("permission response channel closed")?;
    let permission_response_returned_same_path = match permission_response {
        TransportFrame::Single(message) => serde_json::to_string(&message)?.contains(r#""id":7"#),
        _ => false,
    };
    drop(producer.tx);
    drop(consumer.tx);
    bridge.await.context("join slow-observer bridge")??;

    let (disconnect_producer, disconnect_left) = Channel::duplex();
    let (disconnect_right, disconnect_consumer) = Channel::duplex();
    let (observer_tx, observer_rx) = std::sync::mpsc::channel::<String>();
    drop(observer_rx);
    let disconnected_observer_bridge = tokio::spawn(Channel::bridge_with_inspection(
        disconnect_left,
        disconnect_right,
        move |message| {
            observer_tx
                .send(message_method(message))
                .map_err(|error| Error::internal_error().data(error.to_string()))
        },
        |_message| Ok(()),
    ));
    disconnect_producer
        .tx
        .unbounded_send(TransportFrame::Single(RawJsonRpcMessage::notification(
            "session/update".to_owned(),
            json!({"seq": 9}),
        )?))
        .context("queue disconnected-observer frame")?;
    drop(disconnect_producer);
    drop(disconnect_consumer);
    let observer_disconnect_terminates_inline_bridge = disconnected_observer_bridge
        .await
        .context("join disconnected-observer bridge")?
        .is_err();

    let (_fresh_sender, mut fresh_receiver) = Channel::duplex();
    let replay_absent_on_fresh_channel =
        tokio::time::timeout(Duration::from_millis(50), fresh_receiver.rx.next())
            .await
            .is_err();

    let slow_observations = lock(&slow_observed).clone();
    let fast_observations = lock(&fast_observed).clone();
    let source_order_preserved = delivered.iter().map(|entry| entry.0.as_str()).eq([
        "session/update",
        "session/update",
        "session/request_permission",
        "session/update",
    ]);
    let slow_observer_delays_forwarding = delivered.last().is_some_and(|entry| entry.1 >= 200);
    let slow_observer_delays_second_inline_observer = slow_observations
        .first()
        .zip(fast_observations.first())
        .is_some_and(|(slow, fast)| fast.1 >= 50 && fast.1 >= slow.1);
    ensure!(
        source_order_preserved
            && permission_response_returned_same_path
            && slow_observer_delays_forwarding
            && slow_observer_delays_second_inline_observer
            && observer_disconnect_terminates_inline_bridge
            && replay_absent_on_fresh_channel,
        "C10 inline observer characterization failed"
    );

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "claim_ids": ["C10"],
            "streamed_frames": 4,
            "stream_inter_frame_delay_ms": 5,
            "inline_observer_count": 2,
            "observer_delay_ms_per_message": 50,
            "slow_observations": slow_observations,
            "fast_observations": fast_observations,
            "delivered": delivered,
            "source_order_preserved": source_order_preserved,
            "permission_response_returned_same_path": permission_response_returned_same_path,
            "slow_observer_delays_forwarding": slow_observer_delays_forwarding,
            "slow_observer_delays_second_inline_observer": slow_observer_delays_second_inline_observer,
            "observer_disconnect_terminates_inline_bridge": observer_disconnect_terminates_inline_bridge,
            "replay_absent_on_fresh_channel": replay_absent_on_fresh_channel,
            "decision": "inline inspection is one synchronous linear path: slow/disconnected observers affect protocol, permission remains on that path, and no replay attachment exists; multi-client observers require a separate broadcaster",
        }))?
    );
    Ok(())
}
