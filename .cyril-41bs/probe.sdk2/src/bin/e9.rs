use std::sync::{Arc, Mutex, MutexGuard};

use agent_client_protocol::{Channel, RawJsonRpcMessage, TransportFrame, schema::v1::RequestId};
use anyhow::{Context, Result};
use futures_util::StreamExt as _;
use serde_json::{Value, json};

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    match value.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let original_prompt = "USER";
    let injected_prompt = "[memory]\nlesson\nUSER";
    let current_contract = json!({
        "project_identity": "canonical-project-1",
        "session_id": "session-1",
        "source_turn_id": "source-turn-1",
        "bridge_turn_id": 7,
        "original_prompt": original_prompt,
        "injection_budget_bytes": 32,
        "terminal_disposition": "Completed",
        "capture_count": 1,
    });

    let prompt = RawJsonRpcMessage::request(
        "session/prompt".to_owned(),
        json!({
            "sessionId": "session-1",
            "prompt": [{"type": "text", "text": injected_prompt}],
        }),
        RequestId::Number(1),
    )?;
    let update = RawJsonRpcMessage::notification(
        "session/update".to_owned(),
        json!({
            "sessionId": "session-1",
            "update": {
                "sessionUpdate": "agent_message_chunk",
                "content": {"type": "text", "text": "ANSWER"},
            },
        }),
    )?;
    let response =
        RawJsonRpcMessage::response(RequestId::Number(1), Ok(json!({"stopReason": "end_turn"})));

    let (producer, bridge_left) = Channel::duplex();
    let (bridge_right, mut consumer) = Channel::duplex();
    let inspected = Arc::new(Mutex::new(Vec::<Value>::new()));
    let inspected_forward = Arc::clone(&inspected);
    let bridge = tokio::spawn(Channel::bridge_with_inspection(
        bridge_left,
        bridge_right,
        move |message| {
            let value = serde_json::to_value(message)?;
            lock(&inspected_forward).push(value);
            Ok(())
        },
        |_message| Ok(()),
    ));
    for message in [prompt, update, response] {
        producer
            .tx
            .unbounded_send(TransportFrame::Single(message))
            .context("send memory parity frame")?;
    }
    for _ in 0..3 {
        consumer
            .rx
            .next()
            .await
            .context("receive memory parity frame")?;
    }
    drop(producer.tx);
    drop(consumer.tx);
    bridge.await.context("join memory parity bridge")??;

    let inspected = lock(&inspected).clone();
    let tapped_prompt = inspected
        .first()
        .and_then(|value| value.pointer("/params/prompt/0/text"))
        .and_then(Value::as_str)
        .context("tap did not observe prompt text")?;
    let tap_methods = inspected
        .iter()
        .map(|value| {
            value
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or("response")
                .to_owned()
        })
        .collect::<Vec<_>>();
    let candidate = json!({
        "observed_prompt": tapped_prompt,
        "methods": tap_methods,
        "valid_message_count": inspected.len(),
        "exactly_once_wire_inspection": inspected.len() == 3,
        "project_identity": null,
        "source_turn_id": null,
        "bridge_turn_id": null,
        "normalized_terminal_disposition": null,
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "claim_ids": ["C9"],
            "current_contract": current_contract,
            "candidate_wire_tap": candidate,
            "original_prompt_preserved": tapped_prompt == original_prompt,
            "injected_prompt_observed": tapped_prompt == injected_prompt,
            "injection_budget_reconstructable_from_tap": false,
            "project_identity_preserved": false,
            "source_turn_identity_preserved": false,
            "terminal_disposition_preserved": false,
            "adapter_neutral_parity": false,
            "decision": "keep first-prompt injection and normalized SourceObserver capture in their current adapters; a wire tap may supplement raw tracing but cannot replace either contract",
        }))?
    );
    Ok(())
}
