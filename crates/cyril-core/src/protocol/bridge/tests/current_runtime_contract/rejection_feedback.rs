use super::*;
use crate::protocol::engine::KasEngine;
use crate::types::{PermissionOptionId, PermissionResponse};

#[tokio::test]
async fn rejection_feedback_bridge_roundtrip_preserves_capability_and_wire_metadata() {
    let script = Rc::new(RefCell::new(Script {
        request_permission_on_prompt: true,
        permission_options: Some(serde_json::json!([
            {
                "optionId": "allow-once",
                "name": "Allow once",
                "kind": "allow_once"
            },
            {
                "optionId": "reject-once",
                "name": "Reject once",
                "kind": "reject_once"
            }
        ])),
        wire_kas: Some(true),
        sess_ids: Some(true),
        ..Script::default()
    }));
    let observed = Rc::clone(&script);

    with_engine_harness(
        Rc::new(KasEngine::default()),
        script,
        move |sender, mut rx, mut permission_rx, _gate, loop_handle, _kill| async move {
            let session_id = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id,
                    prompt: crate::types::PromptEnvelope::prepared(
                        vec!["request permission".to_owned()],
                        None,
                    ),
                })
                .await
                .expect_contract("rejection feedback prompt command accepted");

            let request = tokio::time::timeout(Duration::from_secs(5), permission_rx.recv())
                .await
                .expect_contract("rejection feedback permission request timeout")
                .expect_contract("rejection feedback permission request channel");
            assert!(
                request.can_reject_with_reason,
                "KAS capability must be derived from the configured engine"
            );
            request
                .responder
                .send(PermissionResponse::RejectWithReason {
                    option_id: PermissionOptionId::new("reject-once"),
                    reason: "Do not use echo. Use printf instead, and include\nthe word PURPLE in the output.".to_owned(),
                })
                .expect_contract("rejection feedback response accepted");

            let response = tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    if let Some(response) = observed.borrow().permission_responses().first().cloned()
                    {
                        break response;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect_contract("rejection feedback wire response timeout");
            assert_eq!(
                response["outcome"]["optionId"],
                serde_json::Value::String("reject-once".to_owned())
            );
            assert_eq!(
                response["_meta"]["kiro"]["rejectionReason"],
                serde_json::Value::String(
                    "Do not use echo. Use printf instead, and include\nthe word PURPLE in the output."
                        .to_owned(),
                )
            );

            sender
                .send(BridgeCommand::Shutdown)
                .await
                .expect_contract("rejection feedback shutdown command accepted");
            tokio::time::timeout(Duration::from_secs(5), loop_handle)
                .await
                .expect_contract("rejection feedback loop shutdown timeout")
                .expect_contract("rejection feedback loop result")
                .expect_contract("rejection feedback domain shutdown");
        },
    )
    .await;
}
