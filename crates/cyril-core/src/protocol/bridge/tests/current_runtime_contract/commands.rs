use super::*;
use crate::types::hook::HookId;

async fn send_command(
    sender: &BridgeSender,
    ledger: &mut Vec<&'static str>,
    command: BridgeCommand,
) {
    ledger.push(command_name(&command));
    sender
        .send(command)
        .await
        .unwrap_or_else(|error| panic!("C5 accepted command send failed: {error}"));
}

async fn assert_notification_quiet(cell: &str, rx: &mut mpsc::Receiver<RoutedNotification>) {
    assert!(
        tokio::time::timeout(Duration::from_millis(25), rx.recv())
            .await
            .is_err(),
        "C5 {cell}: command promised no immediate notification"
    );
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

            send_command(&sender, &mut ledger, BridgeCommand::CancelRequest).await;
            assert_notification_quiet("CancelRequest", &mut rx).await;

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
            assert_notification_quiet("ExtMethod", &mut rx).await;

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
            assert_notification_quiet("SendMessage", &mut rx).await;

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
            assert_notification_quiet("SteerSession", &mut rx).await;

            send_command(
                &sender,
                &mut ledger,
                BridgeCommand::ClearSteering {
                    session_id: session_id.clone(),
                },
            )
            .await;
            assert_notification_quiet("ClearSteering", &mut rx).await;

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

            send_command(&sender, &mut ledger, BridgeCommand::Shutdown).await;
            loop_handle
                .await
                .expect("C5 Shutdown: loop task joined")
                .expect("C5 Shutdown: loop returned Ok");

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
                    "Shutdown",
                ],
                "C5 exhaustive BridgeCommand ledger"
            );
        },
    )
    .await;

    assert_eq!(
        observed.borrow().ext_calls,
        [
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
        ],
        "C5 exact extension methods, order, and params"
    );
    assert_eq!(
        observed.borrow().received,
        [
            "new_session",
            "prompt",
            "cancel",
            "ext:oracle/exact",
            "ext:kiro.dev/commands/options",
            "ext:kiro.dev/commands/execute",
            "ext:session/spawn",
            "ext:kiro.dev/session/terminate",
            "ext:message/send",
            "ext:kiro.dev/settings/list",
            "ext:session/steer",
            "ext:session/steer/clear",
        ],
        "C5 exact fake-agent call order"
    );
}
