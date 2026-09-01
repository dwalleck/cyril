use agent_client_protocol::{Agent, ConnectionTo, UntypedMessage};
use tokio::sync::mpsc;

use crate::types::{Notification, RoutedNotification, SessionId};

async fn send_extension(
    connection: &ConnectionTo<Agent>,
    method: &str,
    params: &serde_json::Value,
) -> agent_client_protocol::Result<serde_json::Value> {
    let wire_method = if method.starts_with('_') {
        method.to_owned()
    } else {
        format!("_{method}")
    };
    let request = UntypedMessage::new(&wire_method, params)?;
    let sent = connection.send_request(request).block_task();
    super::await_response(sent, &wire_method, super::COMMAND_RPC_TIMEOUT).await
}

async fn notify_or_closed(
    tx: &mpsc::Sender<RoutedNotification>,
    notification: Notification,
) -> bool {
    tx.send(notification.into()).await.is_err()
}

fn hooks_listing_notifications(hooks: Vec<crate::types::HookInfo>) -> [Notification; 2] {
    let response = serde_json::json!({ "success": true, "data": { "hooks": hooks } });
    [
        Notification::HooksChanged { hooks },
        Notification::CommandExecuted {
            command: "hooks".to_owned(),
            response,
        },
    ]
}

async fn send_hooks_listing(
    tx: &mpsc::Sender<RoutedNotification>,
    outcome: Result<Vec<crate::types::HookInfo>, Notification>,
) -> bool {
    match outcome {
        Ok(hooks) => {
            for notification in hooks_listing_notifications(hooks) {
                if notify_or_closed(tx, notification).await {
                    return true;
                }
            }
            false
        }
        Err(notification) => notify_or_closed(tx, notification).await,
    }
}

async fn list_hooks(
    connection: &ConnectionTo<Agent>,
    session_id: &SessionId,
    workspace_paths: &[std::path::PathBuf],
    operation: &str,
) -> Result<Vec<crate::types::HookInfo>, Notification> {
    let fail = |message: String| Notification::BridgeError {
        operation: operation.to_owned(),
        message,
    };
    let params = serde_json::json!({
        "sessionId": session_id.as_str(),
        "workspacePaths": workspace_paths,
        "includeDisabled": true,
    });
    let body = send_extension(
        connection,
        crate::protocol::kas::hooks::LIST_METHOD,
        &params,
    )
    .await
    .map_err(|error| {
        tracing::error!(%error, operation, "hooks/list failed");
        fail(error.to_string())
    })?;
    crate::protocol::kas::hooks::parse_wire_hooks(&body)
        .ok_or_else(|| fail("hooks/list reply carried no `hooks` array".to_owned()))
}

pub(in crate::protocol::domain_mediator) async fn handle_list_hooks(
    connection: &ConnectionTo<Agent>,
    tx: &mpsc::Sender<RoutedNotification>,
    session_id: &SessionId,
    workspace_paths: &[std::path::PathBuf],
) -> bool {
    let listing = list_hooks(connection, session_id, workspace_paths, "hooks/list").await;
    send_hooks_listing(tx, listing).await
}

pub(in crate::protocol::domain_mediator) async fn handle_set_hook_enabled(
    connection: &ConnectionTo<Agent>,
    tx: &mpsc::Sender<RoutedNotification>,
    session_id: &SessionId,
    hook_id: &str,
    enabled: bool,
    workspace_paths: &[std::path::PathBuf],
) -> bool {
    let params = serde_json::json!({
        "sessionId": session_id.as_str(),
        "hookId": hook_id,
        "enabled": enabled,
    });
    let outcome = async {
        let response = send_extension(
            connection,
            crate::protocol::kas::hooks::SET_ENABLED_METHOD,
            &params,
        )
        .await
        .map_err(|error| {
            tracing::error!(%error, %hook_id, enabled, "hooks/setEnabled failed");
            Notification::BridgeError {
                operation: format!("hooks/setEnabled '{hook_id}'"),
                message: error.to_string(),
            }
        })?;
        crate::protocol::kas::hooks::interpret_set_enabled_reply(&response.to_string()).map_err(
            |error| {
                tracing::warn!(%error, %hook_id, enabled, "hooks/setEnabled did not confirm");
                Notification::BridgeError {
                    operation: format!("hooks/setEnabled '{hook_id}'"),
                    message: error.to_string(),
                }
            },
        )?;
        list_hooks(connection, session_id, workspace_paths, "hooks/setEnabled").await
    }
    .await;
    send_hooks_listing(tx, outcome).await
}

struct WorkflowOpReply {
    snapshot: Option<crate::types::WorkflowSnapshot>,
    outcome: crate::types::WorkflowCommandOutcome,
}

fn workflow_reply_notifications(reply: WorkflowOpReply) -> Vec<Notification> {
    let mut notifications = Vec::with_capacity(2);
    if let Some(snapshot) = reply.snapshot {
        notifications.push(Notification::WorkflowSnapshot(Box::new(snapshot)));
    }
    notifications.push(Notification::WorkflowCommand(reply.outcome));
    notifications
}

fn workflow_failure(operation: &str, code: Option<i64>, details: String) -> WorkflowOpReply {
    WorkflowOpReply {
        snapshot: None,
        outcome: crate::types::WorkflowCommandOutcome::Failed {
            operation: operation.to_owned(),
            code,
            details,
        },
    }
}

fn ext_error_details(error: &agent_client_protocol::Error) -> (Option<i64>, String) {
    let code = i64::from(i32::from(error.code));
    let details = error
        .data
        .as_ref()
        .and_then(|data| data.get("details"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| error.message.clone());
    (Some(code), details)
}

fn map_workflow_reply(
    op: &crate::types::WorkflowOp,
    body: Result<serde_json::Value, WorkflowOpReply>,
) -> WorkflowOpReply {
    use crate::protocol::convert::kas::workflow as wire;
    use crate::types::{WorkflowCommandOutcome as Outcome, WorkflowFetchVerb, WorkflowOp as Op};

    let operation = op.label();
    let body = match body {
        Ok(body) => body,
        Err(failure) => return failure,
    };
    let parse_failure = |error: &dyn std::fmt::Display| {
        workflow_failure(
            operation,
            None,
            format!("could not read the reply: {error}"),
        )
    };
    match op {
        Op::ListRecipes => match wire::parse_recipes_reply(&body) {
            Ok(listing) => WorkflowOpReply {
                snapshot: None,
                outcome: Outcome::Recipes {
                    recipes: listing.recipes,
                    skipped: listing.skipped.len(),
                },
            },
            Err(error) => parse_failure(&error),
        },
        Op::ListRuns => match wire::parse_list_reply(&body) {
            Ok(listing) => WorkflowOpReply {
                snapshot: None,
                outcome: Outcome::Runs {
                    runs: listing.runs,
                    skipped: listing.skipped.len(),
                },
            },
            Err(error) => parse_failure(&error),
        },
        Op::Attach { .. } | Op::Status { .. } => match wire::parse_state_reply(&body) {
            Ok(snapshot) => {
                let verb = if matches!(op, Op::Attach { .. }) {
                    WorkflowFetchVerb::Attach
                } else {
                    WorkflowFetchVerb::Status
                };
                WorkflowOpReply {
                    snapshot: Some(snapshot.clone()),
                    outcome: Outcome::Fetched {
                        verb,
                        snapshot: Box::new(snapshot),
                    },
                }
            }
            Err(error) => parse_failure(&error),
        },
        Op::Cancel { id } => match wire::parse_cancel_reply(&body) {
            Ok(reply) if reply.ok => WorkflowOpReply {
                snapshot: None,
                outcome: Outcome::Cancelled {
                    workflow_id: id.clone(),
                    previous_status: reply.previous_status,
                },
            },
            Ok(_) => workflow_failure(
                operation,
                None,
                "the agent answered ok=false without an error".to_owned(),
            ),
            Err(error) => parse_failure(&error),
        },
        Op::Resume { .. } => match wire::parse_run_status_reply(&body) {
            Ok((workflow_id, status)) => WorkflowOpReply {
                snapshot: None,
                outcome: Outcome::Resumed {
                    workflow_id,
                    status,
                },
            },
            Err(error) => parse_failure(&error),
        },
        Op::Run { .. } => {
            tracing::error!("map_workflow_reply received a Run op; run is a two-call sequence");
            workflow_failure(operation, None, "internal: run mis-routed".to_owned())
        }
    }
}

fn workflow_op_request(
    session_id: &SessionId,
    workspace_paths: &[std::path::PathBuf],
    op: &crate::types::WorkflowOp,
) -> (&'static str, serde_json::Value) {
    use crate::types::WorkflowOp as Op;
    match op {
        Op::ListRecipes => (
            "kiro/workflow/listRecipes",
            serde_json::json!({
                "sessionId": session_id.as_str(),
                "workspacePaths": workspace_paths,
            }),
        ),
        Op::ListRuns => (
            "kiro/workflow/list",
            serde_json::json!({
                "sessionId": session_id.as_str(),
                "workspacePaths": workspace_paths,
            }),
        ),
        Op::Attach { id } | Op::Status { id } => (
            "kiro/workflow/inspect",
            serde_json::json!({ "workflowId": id.as_str() }),
        ),
        Op::Cancel { id } => (
            "kiro/workflow/cancel",
            serde_json::json!({ "workflowId": id.as_str() }),
        ),
        Op::Resume { id } => (
            "kiro/workflow/resume",
            serde_json::json!({ "workflowId": id.as_str() }),
        ),
        Op::Run { target, inputs } => (
            "kiro/workflow/new",
            serde_json::json!({
                "workflowPath": target.as_workflow_path(),
                "inputs": inputs,
                "parentSessionId": session_id.as_str(),
                "workspacePaths": workspace_paths,
            }),
        ),
    }
}

async fn workflow_extension(
    connection: &ConnectionTo<Agent>,
    operation: &str,
    method: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, WorkflowOpReply> {
    send_extension(connection, method, params)
        .await
        .map_err(|error| {
            tracing::error!(%error, operation, method, "workflow request failed");
            let (code, details) = ext_error_details(&error);
            workflow_failure(operation, code, details)
        })
}

pub(in crate::protocol::domain_mediator) async fn handle_workflow(
    connection: &ConnectionTo<Agent>,
    tx: &mpsc::Sender<RoutedNotification>,
    session_id: &SessionId,
    workspace_paths: &[std::path::PathBuf],
    op: crate::types::WorkflowOp,
) -> bool {
    use crate::protocol::convert::kas::workflow as wire;
    use crate::types::{WorkflowCommandOutcome as Outcome, WorkflowOp as Op};

    let operation = op.label();
    let (method, params) = workflow_op_request(session_id, workspace_paths, &op);
    let reply = match &op {
        Op::ListRecipes
        | Op::ListRuns
        | Op::Attach { .. }
        | Op::Status { .. }
        | Op::Cancel { .. }
        | Op::Resume { .. } => {
            let body = workflow_extension(connection, operation, method, &params).await;
            map_workflow_reply(&op, body)
        }
        Op::Run { .. } => {
            let body = workflow_extension(connection, operation, method, &params).await;
            let snapshot = match body.map(|body| wire::parse_state_reply(&body)) {
                Ok(Ok(snapshot)) => snapshot,
                Ok(Err(error)) => {
                    let failure = workflow_failure(
                        operation,
                        None,
                        format!("could not read the new-run reply: {error}"),
                    );
                    return send_workflow_reply(tx, failure).await;
                }
                Err(failure) => return send_workflow_reply(tx, failure).await,
            };
            let workflow_id = snapshot.workflow_id().clone();
            let name = snapshot.workflow_name().to_owned();
            if notify_or_closed(tx, Notification::WorkflowSnapshot(Box::new(snapshot))).await {
                return true;
            }
            let params = serde_json::json!({ "workflowId": workflow_id.as_str() });
            match workflow_extension(connection, operation, "kiro/workflow/invoke", &params).await {
                Ok(_body) => WorkflowOpReply {
                    snapshot: None,
                    outcome: Outcome::Launched { workflow_id, name },
                },
                Err(failure) => failure,
            }
        }
    };
    send_workflow_reply(tx, reply).await
}

async fn send_workflow_reply(
    tx: &mpsc::Sender<RoutedNotification>,
    reply: WorkflowOpReply,
) -> bool {
    for notification in workflow_reply_notifications(reply) {
        if notify_or_closed(tx, notification).await {
            return true;
        }
    }
    false
}
