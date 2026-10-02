//! `/review`'s bridge seams (cyril-iowg): a KAS permission request arrives
//! with its consent typed, and `New` / `Invoke` are separate wire calls.

use super::*;
use crate::protocol::engine::KasEngine;
use crate::review::consent::Capability;
use crate::types::{
    PermissionResponse, WorkflowCommandOutcome, WorkflowId, WorkflowOp, WorkflowRunTarget,
};

const NEW_REPLY_2180: &str =
    include_str!("../../../../../tests/fixtures/kas/workflow/new-reply-2.18.0.json");

fn allow_and_reject() -> serde_json::Value {
    serde_json::json!([
        {"optionId": "allow-once", "name": "Allow once", "kind": "allow_once"},
        {"optionId": "reject-once", "name": "Reject once", "kind": "reject_once"}
    ])
}

async fn shut_down(sender: &BridgeSender, loop_handle: tokio::task::JoinHandle<crate::Result<()>>) {
    sender
        .send(BridgeCommand::Shutdown)
        .await
        .expect_contract("review shutdown command accepted");
    tokio::time::timeout(Duration::from_secs(5), loop_handle)
        .await
        .expect_contract("review loop shutdown timeout")
        .expect_contract("review loop result")
        .expect_contract("review domain shutdown");
}

#[tokio::test]
async fn kas_permission_request_arrives_with_typed_consent() {
    let mut meta = serde_json::Map::new();
    meta.insert(
        "kiro".to_owned(),
        serde_json::json!({
            "toolId": "execute_bash",
            "command": "command-field",
            "consent": {"capability": "shell", "resource": "resource-field", "workspaceRoot": "/ws"}
        }),
    );
    let script = Rc::new(RefCell::new(Script {
        request_permission_on_prompt: true,
        permission_options: Some(allow_and_reject()),
        permission_meta: Some(meta),
        permission_raw_input: Some(serde_json::json!({"command": "raw-command", "cmd": "raw-cmd"})),
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
                .expect_contract("review prompt accepted");
            let request = tokio::time::timeout(Duration::from_secs(5), permission_rx.recv())
                .await
                .expect_contract("review permission request timeout")
                .expect_contract("review permission request channel");
            let consent = request
                .consent
                .clone()
                .expect_contract("KAS request carries consent");
            assert_eq!(consent.capability(), &Capability::Shell);
            assert_eq!(consent.resource(), Some("resource-field"));
            assert_eq!(consent.workspace_root(), Some(std::path::Path::new("/ws")));
            assert_eq!(consent.tool_id(), Some("execute_bash"));
            assert_eq!(consent.command(), Some("command-field"));
            assert_eq!(consent.raw_commands(), ["raw-command", "raw-cmd"]);
            request
                .responder
                .send(PermissionResponse::Cancel)
                .expect_contract("review permission answered");
            shut_down(&sender, loop_handle).await;
        },
    )
    .await;
}

#[tokio::test]
async fn new_mints_without_invoking_and_invoke_starts_the_run() {
    let script = Rc::new(RefCell::new(Script {
        wire_kas: Some(true),
        sess_ids: Some(true),
        ..Script::default()
    }));
    let reply: serde_json::Value =
        serde_json::from_str(NEW_REPLY_2180).expect_contract("fixture is valid JSON");
    script
        .borrow()
        .ext_responses
        .lock()
        .expect_contract("ext responses")
        .push(("kiro/workflow/new".to_owned(), reply));
    let observed = Rc::clone(&script);
    with_engine_harness(
        Rc::new(KasEngine::default()),
        script,
        move |sender, mut rx, _permission_rx, _gate, loop_handle, _kill| async move {
            let session_id = start_session(&sender, &mut rx).await;
            let mut inputs = serde_json::Map::new();
            inputs.insert("rundir".to_owned(), serde_json::json!("/ws/.code-review/r"));
            let recipe =
                std::path::PathBuf::from("/home/u/.kiro/workflows/cyril-review.workflow.json");
            sender
                .send(BridgeCommand::Workflow {
                    session_id: session_id.clone(),
                    workspace_paths: vec![std::path::PathBuf::from("/ws")],
                    op: WorkflowOp::New {
                        target: WorkflowRunTarget::RecipeFile(recipe),
                        inputs,
                    },
                })
                .await
                .expect_contract("new accepted");
            let mut minted = None;
            for _ in 0..4 {
                match recv_notif(&mut rx, 5).await {
                    Some(Notification::WorkflowCommand(WorkflowCommandOutcome::Minted {
                        workflow_id,
                        name,
                    })) => {
                        minted = Some((workflow_id, name));
                        break;
                    }
                    Some(_) => continue,
                    None => break,
                }
            }
            let (workflow_id, name) = minted.expect_contract("new answers Minted");
            assert_eq!(workflow_id.as_str(), "wf_67c4d77ef2bcd709");
            assert_eq!(name, "cyril-reattach2");
            {
                let calls = observed.borrow().ext_calls().clone();
                let methods: Vec<&str> = calls.iter().map(|(method, _)| method.as_str()).collect();
                assert_eq!(methods, ["kiro/workflow/new"], "new must not invoke");
                let params = &calls[0].1;
                assert_eq!(
                    params["workflowPath"],
                    "/home/u/.kiro/workflows/cyril-review.workflow.json"
                );
                assert_eq!(params["inputs"]["rundir"], "/ws/.code-review/r");
                assert_eq!(params["parentSessionId"], session_id.as_str());
                assert_eq!(params["workspacePaths"], serde_json::json!(["/ws"]));
            }

            let id = WorkflowId::try_from("wf_67c4d77ef2bcd709".to_owned())
                .expect_contract("workflow id");
            sender
                .send(BridgeCommand::Workflow {
                    session_id,
                    workspace_paths: vec![std::path::PathBuf::from("/ws")],
                    op: WorkflowOp::Invoke { id: id.clone() },
                })
                .await
                .expect_contract("invoke accepted");
            let mut invoked = false;
            for _ in 0..4 {
                match recv_notif(&mut rx, 5).await {
                    Some(Notification::WorkflowCommand(WorkflowCommandOutcome::Invoked {
                        workflow_id,
                    })) => {
                        assert_eq!(workflow_id, id);
                        invoked = true;
                        break;
                    }
                    Some(_) => continue,
                    None => break,
                }
            }
            assert!(invoked, "invoke answers Invoked");
            let calls = observed.borrow().ext_calls().clone();
            assert_eq!(calls.len(), 2);
            assert_eq!(calls[1].0, "kiro/workflow/invoke");
            assert_eq!(
                calls[1].1,
                serde_json::json!({"workflowId": "wf_67c4d77ef2bcd709"})
            );
            shut_down(&sender, loop_handle).await;
        },
    )
    .await;
}

/// `/review resume` loads a persisted run with this session as its parent
/// (registering it executes nothing), then retries it: two calls, in order,
/// with the wire shapes KAS 2.26.0 reads.
#[tokio::test(flavor = "current_thread")]
async fn load_registers_the_run_and_retry_restarts_it() {
    let script = Rc::new(RefCell::new(Script {
        wire_kas: Some(true),
        sess_ids: Some(true),
        ..Script::default()
    }));
    let state: serde_json::Value =
        serde_json::from_str(NEW_REPLY_2180).expect_contract("fixture is valid JSON");
    {
        let responses = script.borrow();
        let mut responses = responses
            .ext_responses
            .lock()
            .expect_contract("ext responses");
        responses.push(("kiro/workflow/load".to_owned(), state));
        responses.push((
            "kiro/workflow/retry".to_owned(),
            serde_json::json!({
                "workflowId": "wf_67c4d77ef2bcd709",
                "status": "running",
                "retriedNodeIds": ["verify"]
            }),
        ));
    }
    let observed = Rc::clone(&script);
    with_engine_harness(
        Rc::new(KasEngine::default()),
        script,
        move |sender, mut rx, _permission_rx, _gate, loop_handle, _kill| async move {
            let session_id = start_session(&sender, &mut rx).await;
            let id = WorkflowId::try_from("wf_67c4d77ef2bcd709".to_owned()).expect_contract("id");
            for op in [
                WorkflowOp::Load { id: id.clone() },
                WorkflowOp::Retry { id: id.clone() },
            ] {
                sender
                    .send(BridgeCommand::Workflow {
                        session_id: session_id.clone(),
                        workspace_paths: vec![std::path::PathBuf::from("/ws")],
                        op,
                    })
                    .await
                    .expect_contract("op accepted");
            }
            let (mut loaded, mut retried) = (false, false);
            for _ in 0..8 {
                match recv_notif(&mut rx, 5).await {
                    Some(Notification::WorkflowCommand(WorkflowCommandOutcome::Loaded {
                        workflow_id,
                        ..
                    })) => loaded = workflow_id == id,
                    Some(Notification::WorkflowCommand(WorkflowCommandOutcome::Retried {
                        workflow_id,
                        status,
                    })) => {
                        retried = workflow_id == id
                            && status == Some(crate::types::WorkflowRunStatus::Running);
                        break;
                    }
                    Some(_) => continue,
                    None => break,
                }
            }
            assert!(loaded, "load answers Loaded");
            assert!(retried, "retry answers Retried");
            let calls = observed.borrow().ext_calls().clone();
            let methods: Vec<&str> = calls.iter().map(|(method, _)| method.as_str()).collect();
            assert_eq!(methods, ["kiro/workflow/load", "kiro/workflow/retry"]);
            assert_eq!(
                calls[0].1,
                serde_json::json!({
                    "workflowId": "wf_67c4d77ef2bcd709",
                    "parentSessionId": session_id.as_str(),
                    "workspacePaths": ["/ws"]
                })
            );
            assert_eq!(
                calls[1].1,
                serde_json::json!({"workflowId": "wf_67c4d77ef2bcd709"})
            );
            shut_down(&sender, loop_handle).await;
        },
    )
    .await;
}
