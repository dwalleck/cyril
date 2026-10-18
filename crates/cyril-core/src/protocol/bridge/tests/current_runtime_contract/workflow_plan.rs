//! cyril-8wh0: over the real bridge, a KAS `update_workflow` permission
//! request reaches the App with its `_meta.kiro.workflowPlan` typed, so the
//! overlay can show the plan the user is asked to approve.

use super::*;
use crate::protocol::engine::KasEngine;
use crate::types::{PermissionOptionId, PermissionResponse, WorkflowPlanKind};

/// The live request's `_meta` (toolId, workflowPlan, consent), from the
/// cyril-8wh0 fixture carved from the 2.32.0 audit.
fn live_meta() -> serde_json::Map<String, serde_json::Value> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/kas_permission/workflow-plan-2.32.0.json");
    let raw = std::fs::read_to_string(&path).expect_contract("read workflow plan fixture");
    let fixture: serde_json::Value =
        serde_json::from_str(&raw).expect_contract("workflow plan fixture is JSON");
    fixture["launch_live"]["_meta"]
        .as_object()
        .cloned()
        .expect_contract("launch_live carries _meta")
}

#[tokio::test]
async fn kas_workflow_plan_reaches_the_app_typed() {
    let script = Rc::new(RefCell::new(Script {
        request_permission_on_prompt: true,
        permission_options: Some(serde_json::json!([
            {"optionId": "accept", "name": "Allow", "kind": "allow_once"},
            {"optionId": "reject", "name": "Deny", "kind": "reject_once"}
        ])),
        permission_meta: Some(live_meta()),
        wire_kas: Some(true),
        sess_ids: Some(true),
        ..Script::default()
    }));
    with_engine_harness(
        Rc::new(KasEngine::default()),
        script,
        move |sender, mut rx, mut permission_rx, _gate, loop_handle, _kill| async move {
            let session_id = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id,
                    prompt: crate::types::PromptEnvelope::prepared(vec!["go".to_owned()], None),
                })
                .await
                .expect_contract("workflow plan prompt accepted");
            let request = tokio::time::timeout(Duration::from_secs(5), permission_rx.recv())
                .await
                .expect_contract("workflow plan permission request timeout")
                .expect_contract("workflow plan permission request channel");
            let plan = request
                .workflow_plan
                .as_ref()
                .expect_contract("the plan must reach the App typed");
            assert_eq!(plan.kind, WorkflowPlanKind::Launch);
            assert_eq!(plan.workflow_id.as_str(), "wf_6b22f0c99f407f0e");
            assert_eq!(plan.steps.len(), 1);
            request
                .responder
                .send(PermissionResponse::Selected {
                    option_id: PermissionOptionId::new("reject"),
                    trust_option: None,
                })
                .expect_contract("workflow plan response accepted");
            sender
                .send(BridgeCommand::Shutdown)
                .await
                .expect_contract("workflow plan shutdown command accepted");
            tokio::time::timeout(Duration::from_secs(5), loop_handle)
                .await
                .expect_contract("workflow plan loop shutdown timeout")
                .expect_contract("workflow plan loop result")
                .expect_contract("workflow plan domain shutdown");
        },
    )
    .await;
}
