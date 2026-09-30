//! cyril-lki9 transport fences: agent-initiated turn frames through the real
//! SDK2 transport and the bridge's inbound path.
//!
//! The converter and the engine hook have unit fences. What they cannot show is
//! that the agent-initiated tag survives ACP deserialization, the SDK's
//! `session/update` handler and the mediator's inbound conversion, and is
//! attached to the SAME frame it came on — the only place the App's per-turn
//! announcement can be derived from.

use super::*;

/// Drive one prompt whose single chunk carries `chunk_meta`, and return the
/// `origin` the App-facing receiver saw on that chunk.
async fn chunk_origin(
    chunk_meta: Option<serde_json::Value>,
) -> Option<crate::types::AgentInitiation> {
    let script = Rc::new(RefCell::new(Script {
        emit_chunks: 1,
        chunk_meta,
        wire_kas: Some(true),
        ..Script::default()
    }));
    let seen: Rc<RefCell<Option<Option<crate::types::AgentInitiation>>>> =
        Rc::new(RefCell::new(None));
    let out = Rc::clone(&seen);
    with_engine_harness(
        Rc::new(crate::protocol::engine::KasEngine::default()),
        script,
        |sender, mut rx, _permission_rx, _gate, _loop_handle, _kill| async move {
            let session_id = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id,
                    prompt: crate::types::PromptEnvelope::prepared(vec!["go".to_owned()], None),
                })
                .await
                .expect_contract("lki9 C3 prompt send");
            for _ in 0..10 {
                let routed = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                    .await
                    .expect_contract("lki9 C3 chunk timeout")
                    .expect_contract("lki9 C3 channel open");
                if let Notification::AgentMessage(message) = &routed.notification {
                    assert_eq!(message.text, "c0", "the probe chunk, not another frame");
                    *out.borrow_mut() = Some(routed.origin.clone());
                    return;
                }
            }
            panic!("lki9 C3: the chunk never reached the App receiver");
        },
    )
    .await;
    let result = seen.borrow().clone();
    result.expect_contract("lki9 C3 chunk observed")
}

/// C3: a tagged chunk reaches the App with `origin` set from ITS OWN `_meta`;
/// the untagged control chunk (the historical shape) arrives with `None`.
#[tokio::test]
async fn origin_is_stamped_on_the_frame_it_came_on() {
    let tagged = chunk_origin(Some(serde_json::json!({
        "kiro": { "agentInitiated": true, "agentInitiatedReason": "workflow-complete-wake" }
    })))
    .await;
    assert_eq!(
        tagged
            .as_ref()
            .and_then(crate::types::AgentInitiation::reason),
        Some("workflow-complete-wake"),
        "tagged chunk must carry its agent-initiated origin, got {tagged:?}"
    );
    let untagged = chunk_origin(None).await;
    assert_eq!(untagged, None, "an untagged chunk must carry no origin");
}
