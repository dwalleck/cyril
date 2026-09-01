use super::*;
use crate::types::hook::HookId;

async fn send_command(
    sender: &BridgeSender,
    ledger: &mut Vec<&'static str>,
    command: BridgeCommand,
) {
    let name = command_name(&command);
    ledger.push(name);
    sender
        .send(command)
        .await
        .unwrap_or_else(|error| panic!("C5 {name} command send failed: {error}"));
}

fn assert_bridge_error(cell: &str, notification: &Notification, operation: &str) {
    assert!(
        matches!(notification, Notification::BridgeError { operation: actual, .. } if actual == operation),
        "C5 {cell}: expected BridgeError operation {operation:?}, got {notification:?}"
    );
}

#[tokio::test]
async fn c5_every_bridge_command_has_an_explicit_current_runtime_outcome() {
    let script = Rc::new(RefCell::new(Script::default()));
    let observed = Rc::clone(&script);
    with_harness(
        script,
        move |sender, mut rx, _permission_rx, _gate, loop_handle| async move {
            let mut ledger = Vec::new();

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::NewSession {
                    cwd: std::env::temp_dir(),
                },
            )
            .await;
            let usage_started = next_notification("NewSession.usage", &mut rx).await;
            assert!(
                matches!(
                    usage_started,
                    Notification::UsageSessionStarted {
                        ref session_id,
                        origin: SessionOrigin::Fresh,
                    } if session_id.as_str() == "fake-0"
                ),
                "C5 NewSession.usage: {usage_started:?}"
            );
            let created = next_notification("NewSession.created", &mut rx).await;
            let session_id = match created {
                Notification::SessionCreated { session_id, .. } => session_id,
                other => panic!("C5 NewSession.created: got {other:?}"),
            };
            assert_eq!(session_id.as_str(), "fake-0", "C5 NewSession.sessionId");

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::SendPrompt {
                    session_id: session_id.clone(),
                    prompt: crate::types::PromptEnvelope::prepared(
                        vec!["wire prompt".to_owned()],
                        None,
                    ),
                },
            )
            .await;
            let completed = next_notification("SendPrompt", &mut rx).await;
            assert!(
                matches!(
                    completed,
                    Notification::TurnCompleted {
                        stop_reason: StopReason::EndTurn
                    }
                ),
                "C5 SendPrompt: {completed:?}"
            );

            // Quiet cells (CancelRequest, ExtMethod, SendMessage, SteerSession,
            // ClearSteering) are asserted STRUCTURALLY, not with wall-clock
            // holds: the channel is FIFO and every later notification is
            // exact-matched, so a stray frame from a quiet command surfaces as
            // a mismatch in the very next assertion — and the post-shutdown
            // drain below catches a stray after the last matched frame.
            send_command(&sender, &mut ledger, BridgeCommand::CancelRequest).await;

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::SetMode {
                    mode_id: "oracle-mode".to_owned(),
                },
            )
            .await;
            assert_bridge_error(
                "SetMode",
                &next_notification("SetMode", &mut rx).await,
                "set_mode 'oracle-mode'",
            );

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::SetModel {
                    model_id: "oracle-model".to_owned(),
                },
            )
            .await;
            assert_bridge_error(
                "SetModel",
                &next_notification("SetModel", &mut rx).await,
                "set_model 'oracle-model'",
            );

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::ExtMethod {
                    method: "oracle/exact".to_owned(),
                    params: serde_json::json!({"nested": [1, "two"]}),
                },
            )
            .await;

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::QueryCommandOptions {
                    command: "model".to_owned(),
                    session_id: session_id.clone(),
                },
            )
            .await;
            let options = next_notification("QueryCommandOptions", &mut rx).await;
            assert!(
                matches!(options, Notification::CommandOptionsReceived { ref command, ref options } if command == "model" && options.is_empty()),
                "C5 QueryCommandOptions: {options:?}"
            );

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::ExecuteCommand {
                    command: "tools".to_owned(),
                    session_id: session_id.clone(),
                    args: serde_json::json!({"scope": "all"}),
                },
            )
            .await;
            let executed = next_notification("ExecuteCommand", &mut rx).await;
            assert!(
                matches!(executed, Notification::CommandExecuted { ref command, ref response } if command == "tools" && response == &serde_json::json!({})),
                "C5 ExecuteCommand: {executed:?}"
            );

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::SpawnSession {
                    task: "inspect exact wire".to_owned(),
                    name: "oracle-child".to_owned(),
                },
            )
            .await;
            assert_bridge_error(
                "SpawnSession",
                &next_notification("SpawnSession", &mut rx).await,
                "spawn_session 'oracle-child'",
            );

            let child_id = crate::types::SessionId::new("child-1");
            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::TerminateSession {
                    session_id: child_id.clone(),
                },
            )
            .await;
            let terminated = next_notification("TerminateSession", &mut rx).await;
            assert!(
                matches!(terminated, Notification::SubagentTerminated { ref session_id } if session_id == &child_id),
                "C5 TerminateSession: {terminated:?}"
            );

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::SendMessage {
                    session_id: child_id.clone(),
                    content: "hello child".to_owned(),
                },
            )
            .await;

            send_command(&sender, &mut ledger, BridgeCommand::QueryUsageAccount).await;
            let usage = next_notification("QueryUsageAccount", &mut rx).await;
            assert!(
                matches!(usage, Notification::UsageAccountQueryFailed { ref message } if message == "account usage is available only for the KAS engine"),
                "C5 QueryUsageAccount: {usage:?}"
            );

            send_command(&sender, &mut ledger, BridgeCommand::ListSettings).await;
            let settings = next_notification("ListSettings", &mut rx).await;
            assert!(
                matches!(settings, Notification::SettingsList { ref settings } if settings == &serde_json::json!({})),
                "C5 ListSettings: {settings:?}"
            );

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::SteerSession {
                    session_id: session_id.clone(),
                    message: "change course".to_owned(),
                },
            )
            .await;

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::ClearSteering {
                    session_id: session_id.clone(),
                },
            )
            .await;

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::ListKasHooks {
                    session_id: session_id.clone(),
                    workspace_paths: vec![std::path::PathBuf::from("/oracle workspace")],
                },
            )
            .await;
            assert_bridge_error(
                "ListKasHooks",
                &next_notification("ListKasHooks", &mut rx).await,
                "hooks",
            );

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::SetKasHookEnabled {
                    session_id: session_id.clone(),
                    hook_id: HookId::new("/oracle/hook.json#hook-0"),
                    enabled: false,
                    workspace_paths: vec![std::path::PathBuf::from("/oracle workspace")],
                },
            )
            .await;
            assert_bridge_error(
                "SetKasHookEnabled",
                &next_notification("SetKasHookEnabled", &mut rx).await,
                "hooks",
            );

            let workflow = crate::types::WorkflowOp::ListRecipes;
            let workflow_label = workflow.label().to_owned();
            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::Workflow {
                    session_id: session_id.clone(),
                    workspace_paths: vec![std::path::PathBuf::from("/oracle workspace")],
                    op: workflow,
                },
            )
            .await;
            assert_bridge_error(
                "Workflow",
                &next_notification("Workflow", &mut rx).await,
                &workflow_label,
            );

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::LoadSession {
                    session_id: crate::types::SessionId::new("load-oracle"),
                },
            )
            .await;
            let load = next_notification("LoadSession", &mut rx).await;
            assert!(
                matches!(load, Notification::BridgeDisconnected { ref reason } if reason.starts_with("Failed to load session:")),
                "C5 LoadSession: {load:?}"
            );

            // A failed load is RECOVERABLE (only session/new failure is
            // fatal): the loop must still serve commands afterwards.
            send_command(&sender, &mut ledger, BridgeCommand::ListSettings).await;
            let after_load = next_notification("ListSettings after failed load", &mut rx).await;
            assert!(
                matches!(after_load, Notification::SettingsList { ref settings } if settings == &serde_json::json!({})),
                "C5 failed LoadSession must not kill the bridge: {after_load:?}"
            );

            send_command(&sender, &mut ledger, BridgeCommand::Shutdown).await;
            loop_handle
                .await
                .expect_contract("C5 Shutdown after failed load: loop task joined")
                .expect_contract("C5 Shutdown after failed load: loop returned Ok");
            let closed = sender
                .send(BridgeCommand::Shutdown)
                .await
                .expect_err_contract("C5 shutdown closes command channel");
            assert_eq!(closed.to_string(), "bridge channel closed");

            // The loop has exited and dropped its sender: recv drains any
            // buffered stray frame before yielding None, closing the quiet
            // contract for the commands after the last matched notification.
            let trailing = rx.recv().await;
            assert!(
                trailing.is_none(),
                "C5 quiet: stray notification after the final matched frame: {trailing:?}"
            );

            assert_eq!(
                ledger,
                [
                    "NewSession",
                    "SendPrompt",
                    "CancelRequest",
                    "SetMode",
                    "SetModel",
                    "ExtMethod",
                    "QueryCommandOptions",
                    "ExecuteCommand",
                    "SpawnSession",
                    "TerminateSession",
                    "SendMessage",
                    "QueryUsageAccount",
                    "ListSettings",
                    "SteerSession",
                    "ClearSteering",
                    "ListKasHooks",
                    "SetKasHookEnabled",
                    "Workflow",
                    "LoadSession",
                    "ListSettings",
                    "Shutdown",
                ],
                "C5 exhaustive BridgeCommand ledger"
            );
        },
    )
    .await;

    assert_eq!(
        observed.borrow().ext_calls().clone(),
        [
            (
                "session/set_model".to_owned(),
                serde_json::json!({
                    "sessionId": "fake-0",
                    "modelId": "oracle-model",
                }),
            ),
            (
                "oracle/exact".to_owned(),
                serde_json::json!({"nested": [1, "two"]}),
            ),
            (
                "kiro.dev/commands/options".to_owned(),
                serde_json::json!({
                    "command": "model",
                    "sessionId": "fake-0",
                    "partial": "",
                }),
            ),
            (
                "kiro.dev/commands/execute".to_owned(),
                serde_json::json!({
                    "sessionId": "fake-0",
                    "command": {"command": "tools", "args": {"scope": "all"}},
                }),
            ),
            (
                "session/spawn".to_owned(),
                serde_json::json!({
                    "sessionId": "fake-0",
                    "task": "inspect exact wire",
                    "name": "oracle-child",
                }),
            ),
            (
                "kiro.dev/session/terminate".to_owned(),
                serde_json::json!({"sessionId": "child-1"}),
            ),
            (
                "message/send".to_owned(),
                serde_json::json!({"sessionId": "child-1", "content": "hello child"}),
            ),
            ("kiro.dev/settings/list".to_owned(), serde_json::json!({})),
            (
                "session/steer".to_owned(),
                serde_json::json!({"sessionId": "fake-0", "message": "change course"}),
            ),
            (
                "session/steer/clear".to_owned(),
                serde_json::json!({"sessionId": "fake-0"}),
            ),
            ("kiro.dev/settings/list".to_owned(), serde_json::json!({})),
        ],
        "C5 exact extension methods, order, and params"
    );
    assert_eq!(
        observed.borrow().received().clone(),
        [
            "new_session",
            "prompt",
            "cancel",
            "set_model",
            "ext:oracle/exact",
            "ext:kiro.dev/commands/options",
            "ext:kiro.dev/commands/execute",
            "ext:session/spawn",
            "ext:kiro.dev/session/terminate",
            "ext:message/send",
            "ext:kiro.dev/settings/list",
            "ext:session/steer",
            "ext:session/steer/clear",
            "ext:kiro.dev/settings/list",
        ],
        "C5 exact fake-agent call order"
    );

    with_harness(
        Rc::new(RefCell::new(Script::default())),
        |sender, _rx, _permission_rx, _gate, loop_handle| async move {
            let mut ledger = Vec::new();
            send_command(&sender, &mut ledger, BridgeCommand::Shutdown).await;
            loop_handle
                .await
                .expect_contract("C5 Shutdown: loop task joined")
                .expect_contract("C5 Shutdown: loop returned Ok");
            assert_eq!(ledger, ["Shutdown"], "C5 explicit Shutdown outcome");
        },
    )
    .await;
}

#[tokio::test]
async fn c5_command_failures_preserve_legacy_operation_labels() {
    let script = Rc::new(RefCell::new(Script {
        fail_extensions: vec![
            "oracle/error".to_owned(),
            "kiro.dev/session/terminate".to_owned(),
            "message/send".to_owned(),
            "session/steer".to_owned(),
            "session/steer/clear".to_owned(),
        ],
        ..Script::default()
    }));
    with_harness(
        script,
        |sender, mut rx, _permission_rx, _gate, loop_handle| async move {
            let session_id = start_session(&sender, &mut rx).await;
            let child_id = crate::types::SessionId::new("label-child");
            let mut ledger = Vec::new();

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::ExtMethod {
                    method: "oracle/error".to_owned(),
                    params: serde_json::json!({}),
                },
            )
            .await;
            assert_bridge_error(
                "ExtMethod label",
                &next_notification("ExtMethod label", &mut rx).await,
                "ext_method 'oracle/error'",
            );

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::TerminateSession {
                    session_id: child_id.clone(),
                },
            )
            .await;
            assert_bridge_error(
                "TerminateSession label",
                &next_notification("TerminateSession label", &mut rx).await,
                "terminate_session 'label-child'",
            );

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::SendMessage {
                    session_id: child_id,
                    content: "message".to_owned(),
                },
            )
            .await;
            assert_bridge_error(
                "SendMessage label",
                &next_notification("SendMessage label", &mut rx).await,
                "send_message to 'label-child'",
            );

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::SteerSession {
                    session_id: session_id.clone(),
                    message: "steer".to_owned(),
                },
            )
            .await;
            assert_bridge_error(
                "SteerSession label",
                &next_notification("SteerSession label", &mut rx).await,
                "steer 'fake-0'",
            );

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::ClearSteering { session_id },
            )
            .await;
            assert_bridge_error(
                "ClearSteering label",
                &next_notification("ClearSteering label", &mut rx).await,
                "steer/clear 'fake-0'",
            );

            send_command(&sender, &mut ledger, BridgeCommand::Shutdown).await;
            loop_handle
                .await
                .expect_contract("C5 label loop joined")
                .expect_contract("C5 label loop result");
        },
    )
    .await;
}

#[tokio::test]
async fn c13_extension_params_preserve_array_and_null_shapes() {
    let script = Rc::new(RefCell::new(Script::default()));
    let observed = Rc::clone(&script);
    with_harness(
        script,
        |sender, mut rx, _permission_rx, _gate, loop_handle| async move {
            let _session_id = start_session(&sender, &mut rx).await;
            let mut ledger = Vec::new();
            for (method, params) in [
                ("oracle/array", serde_json::json!([1, {"nested": true}])),
                ("oracle/null", serde_json::Value::Null),
            ] {
                send_command(
                    &sender,
                    &mut ledger,
                    BridgeCommand::ExtMethod {
                        method: method.to_owned(),
                        params,
                    },
                )
                .await;
            }
            send_command(&sender, &mut ledger, BridgeCommand::QueryUsageAccount).await;
            let delimiter = next_notification("C13 extension params delimiter", &mut rx).await;
            assert!(matches!(
                delimiter,
                Notification::UsageAccountQueryFailed { .. }
            ));
            send_command(&sender, &mut ledger, BridgeCommand::Shutdown).await;
            loop_handle
                .await
                .expect_contract("C13 extension params loop joined")
                .expect_contract("C13 extension params loop result");
        },
    )
    .await;
    assert_eq!(
        observed.borrow().ext_calls().as_slice(),
        [
            (
                "oracle/array".to_owned(),
                serde_json::json!([1, {"nested": true}]),
            ),
            ("oracle/null".to_owned(), serde_json::Value::Null),
        ],
        "C13 extension params must remain exact JSON values"
    );
}

#[tokio::test]
async fn c5_new_session_rpc_failure_is_fatal() {
    let script = Rc::new(RefCell::new(Script {
        fail_new_session: true,
        ..Script::default()
    }));
    with_harness(
        script,
        |sender, mut rx, _permission_rx, _gate, loop_handle| async move {
            sender
                .send(BridgeCommand::NewSession {
                    cwd: std::env::temp_dir(),
                })
                .await
                .expect_contract("C5 failing NewSession command accepted");
            let notification = next_notification("NewSession fatal", &mut rx).await;
            assert!(
                matches!(
                    notification,
                    Notification::BridgeDisconnected { ref reason }
                        if reason.starts_with("Failed to create session:")
                ),
                "C5 NewSession failure must disconnect: {notification:?}"
            );
            loop_handle
                .await
                .expect_contract("C5 NewSession fatal loop joined")
                .expect_contract("C5 NewSession fatal loop result");
            let closed = sender
                .send(BridgeCommand::Shutdown)
                .await
                .expect_err_contract("C5 NewSession failure closes commands");
            assert_eq!(closed.to_string(), "bridge channel closed");
        },
    )
    .await;
}
