use std::sync::mpsc;

use agent_client_protocol::{
    Channel, Error, RawJsonRpcMessage, TransportBatch, TransportBatchEntry, TransportFrame,
    schema::v1::RequestId,
};
use anyhow::{Context, Result, bail, ensure};
use futures_util::StreamExt as _;
use serde_json::json;

fn observe(tx: &mpsc::Sender<String>, message: &RawJsonRpcMessage) -> Result<(), Error> {
    let serialized = serde_json::to_string(message)
        .map_err(|error| Error::internal_error().data(error.to_string()))?;
    tx.send(serialized)
        .map_err(|error| Error::internal_error().data(error.to_string()))
}

#[tokio::main]
async fn main() -> Result<()> {
    let extreme_input = r#" { "jsonrpc" : "2.0", "method" : "_kiro.dev/metadata", "params" : { "extreme" : 1e400, "label" : "probe" } } "#;
    let extreme_wire_capture = extreme_input.as_bytes().to_vec();
    let extreme_semantic_parse_error = serde_json::from_str::<RawJsonRpcMessage>(extreme_input)
        .err()
        .map(|error| error.to_string());
    let extension_input = r#" { "jsonrpc" : "2.0", "method" : "_kiro.dev/metadata", "params" : { "value" : 1.2300, "label" : "probe" } } "#;
    let extension: RawJsonRpcMessage =
        serde_json::from_str(extension_input).context("parse extension frame")?;

    let unknown_input = r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":{"sessionUpdate":"future_variant","payload":{"ok":true}}}}"#;
    let unknown: RawJsonRpcMessage =
        serde_json::from_str(unknown_input).context("parse unknown update frame")?;
    let response = RawJsonRpcMessage::response(RequestId::Number(7), Ok(json!({"ok": true})));
    let batch = TransportBatch::from_entries([
        TransportBatchEntry::message(unknown),
        TransportBatchEntry::malformed(
            json!({"not": "json-rpc"}),
            Error::invalid_request().data("probe malformed batch entry"),
        ),
        TransportBatchEntry::message(response),
    ])
    .context("non-empty batch")?;
    let malformed_raw = "{malformed-frame".to_owned();

    let (producer, bridge_left) = Channel::duplex();
    let (bridge_right, mut consumer) = Channel::duplex();
    let (left_tx, left_rx) = mpsc::channel();
    let (right_tx, right_rx) = mpsc::channel();

    let bridge = tokio::spawn(Channel::bridge_with_inspection(
        bridge_left,
        bridge_right,
        move |message| observe(&left_tx, message),
        move |message| observe(&right_tx, message),
    ));

    producer
        .tx
        .unbounded_send(TransportFrame::Single(extension))
        .context("send single frame")?;
    producer
        .tx
        .unbounded_send(TransportFrame::Batch(batch))
        .context("send batch frame")?;
    producer
        .tx
        .unbounded_send(TransportFrame::Malformed {
            raw: malformed_raw.clone(),
            error: Error::parse_error().data("probe malformed top-level frame"),
        })
        .context("send malformed frame")?;

    let single = consumer.rx.next().await.context("receive single frame")?;
    let batch = consumer.rx.next().await.context("receive batch frame")?;
    let malformed = consumer
        .rx
        .next()
        .await
        .context("receive malformed frame")?;

    let single_serialized = match single {
        TransportFrame::Single(message) => serde_json::to_string(&message)?,
        other => bail!("first frame changed shape: {other:?}"),
    };
    let (batch_len, batch_serialized, batch_kinds) = match batch {
        TransportFrame::Batch(batch) => {
            let kinds = batch
                .entries()
                .map(|entry| match entry {
                    TransportBatchEntry::Message(_) => "message",
                    TransportBatchEntry::Malformed { .. } => "malformed",
                })
                .collect::<Vec<_>>();
            (batch.len(), serde_json::to_string(&batch)?, kinds)
        }
        other => bail!("second frame changed shape: {other:?}"),
    };
    let malformed_preserved = match malformed {
        TransportFrame::Malformed { raw, .. } => raw == malformed_raw,
        other => bail!("third frame changed shape: {other:?}"),
    };

    drop(producer.tx);
    drop(consumer.tx);
    bridge.await.context("join bridge task")??;

    let left_observations = left_rx.try_iter().collect::<Vec<_>>();
    let inspected_only_valid_messages = left_observations.len() == 3;
    let preparse_capture_preserved_exact_lexeme = extreme_wire_capture == extreme_input.as_bytes()
        && String::from_utf8_lossy(&extreme_wire_capture).contains("1e400");
    let unknown_update_preserved = batch_serialized.contains("future_variant");
    let parseable_number_lexeme_preserved_after_parse = single_serialized.contains("1.2300");
    ensure!(
        batch_len == 3
            && batch_kinds == ["message", "malformed", "message"]
            && malformed_preserved
            && inspected_only_valid_messages
            && unknown_update_preserved
            && !parseable_number_lexeme_preserved_after_parse
            && preparse_capture_preserved_exact_lexeme,
        "C4 frame or preparse-capture contract failed"
    );
    let right_observations = right_rx.try_iter().collect::<Vec<_>>();
    let output = json!({
        "claim_ids": ["C4"],
        "single_shape_preserved": true,
        "batch_shape_preserved": true,
        "batch_len": batch_len,
        "batch_kinds": batch_kinds,
        "malformed_raw_preserved": malformed_preserved,
        "inspection_left_to_right": left_observations,
        "inspection_right_to_left": right_observations,
        "inspection_skipped_malformed_entries": inspected_only_valid_messages,
        "unknown_update_preserved": unknown_update_preserved,
        "extreme_semantic_parse_error": extreme_semantic_parse_error,
        "parseable_number_lexeme_preserved_after_parse": parseable_number_lexeme_preserved_after_parse,
        "preparse_capture_preserved_exact_lexeme": preparse_capture_preserved_exact_lexeme,
        "semantic_original_wire_bytes_preserved": single_serialized == extension_input,
        "single_serialized": single_serialized,
        "batch_serialized": batch_serialized,
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
