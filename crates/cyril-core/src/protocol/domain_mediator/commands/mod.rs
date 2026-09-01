mod extensions;
#[cfg(feature = "kas")]
pub(super) mod kas;
mod session;
mod subagents;

use std::future::Future;
use std::time::Duration;

use agent_client_protocol::{Agent, ConnectionTo};

use super::DomainMediator;
use crate::types::BridgeCommand;
#[cfg(not(feature = "kas"))]
use crate::types::Notification;

/// Bound on `session/new` and `session/load` — load replays the session's
/// history before answering, so it gets more headroom than a control call.
pub(super) const SESSION_RPC_TIMEOUT: Duration = Duration::from_secs(30);
/// Bound on every other command RPC (mode/model/extension/subagent calls).
/// Follows the 5s `QueryUsageAccount` precedent with headroom for a busy
/// backend; `session/prompt` deliberately has NO timeout (turns are unbounded
/// — the stall watchdog owns that surface).
pub(super) const COMMAND_RPC_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) fn timeout_error(what: &str, timeout: Duration) -> agent_client_protocol::Error {
    agent_client_protocol::Error::new(
        agent_client_protocol::ErrorCode::InternalError.into(),
        format!("{what} timed out after {}s", timeout.as_secs()),
    )
}

/// Await an already-sent RPC's response with a bound. The request frame went
/// out when `send_request` was called; this only bounds how long the spawned
/// task waits before reporting the command as failed.
pub(super) async fn await_response<T>(
    sent: impl Future<Output = agent_client_protocol::Result<T>>,
    what: &str,
    timeout: Duration,
) -> agent_client_protocol::Result<T> {
    match tokio::time::timeout(timeout, sent).await {
        Ok(result) => result,
        Err(_) => Err(timeout_error(what, timeout)),
    }
}

impl DomainMediator {
    pub(super) async fn handle_command(
        &mut self,
        connection: &ConnectionTo<Agent>,
        command: BridgeCommand,
    ) -> crate::Result<bool> {
        match command {
            BridgeCommand::NewSession { cwd } => self.new_session(connection, cwd)?,
            BridgeCommand::LoadSession { session_id } => {
                return self.load_session(connection, session_id).await;
            }
            BridgeCommand::SendPrompt { session_id, prompt } => {
                self.start_prompt(connection.clone(), session_id, prompt)
                    .await?;
            }
            BridgeCommand::CancelRequest => self.cancel_active(connection).await,
            BridgeCommand::SetMode { mode_id } => self.set_mode(connection, mode_id).await?,
            BridgeCommand::SetModel { model_id } => {
                self.set_model(connection, model_id).await?;
            }
            BridgeCommand::ExtMethod { method, params } => {
                self.execute_extension(connection, method, params);
            }
            BridgeCommand::QueryCommandOptions {
                command,
                session_id,
            } => {
                self.query_command_options(connection, command, session_id);
            }
            BridgeCommand::ExecuteCommand {
                command,
                session_id,
                args,
            } => {
                self.execute_command(connection, command, session_id, args);
            }
            BridgeCommand::SpawnSession { task, name } => {
                self.spawn_session(connection, task, name).await?;
            }
            BridgeCommand::TerminateSession { session_id } => {
                self.terminate_session(connection, session_id);
            }
            BridgeCommand::SendMessage {
                session_id,
                content,
            } => {
                self.send_message(connection, session_id, content);
            }
            BridgeCommand::ListSettings => self.list_settings(connection),
            BridgeCommand::QueryUsageAccount => self.query_usage_account(connection),
            BridgeCommand::SteerSession {
                session_id,
                message,
            } => {
                self.steer_session(connection, session_id, message);
            }
            BridgeCommand::ClearSteering { session_id } => {
                self.clear_steering(connection, session_id);
            }
            #[cfg(not(feature = "kas"))]
            BridgeCommand::ListKasHooks { .. } | BridgeCommand::SetKasHookEnabled { .. } => {
                self.notify(
                    Notification::BridgeError {
                        operation: "hooks".into(),
                        message: "this build has no KAS support (cargo feature `kas`)".into(),
                    }
                    .into(),
                )
                .await?;
            }
            #[cfg(not(feature = "kas"))]
            BridgeCommand::Workflow { op, .. } => {
                self.notify(
                    Notification::BridgeError {
                        operation: op.label().to_owned(),
                        message: "this build has no KAS support (cargo feature `kas`)".into(),
                    }
                    .into(),
                )
                .await?;
            }
            #[cfg(feature = "kas")]
            BridgeCommand::ListKasHooks {
                session_id,
                workspace_paths,
            } => {
                let connection = connection.clone();
                let tx = self.bridge.notification_tx.clone();
                self.spawn_command(async move {
                    // `true` means the App dropped its receiver; the loop
                    // discovers that itself on its next notify.
                    let _closed =
                        kas::handle_list_hooks(&connection, &tx, &session_id, &workspace_paths)
                            .await;
                });
            }
            #[cfg(feature = "kas")]
            BridgeCommand::SetKasHookEnabled {
                session_id,
                hook_id,
                enabled,
                workspace_paths,
            } => {
                let connection = connection.clone();
                let tx = self.bridge.notification_tx.clone();
                self.spawn_command(async move {
                    let _closed = kas::handle_set_hook_enabled(
                        &connection,
                        &tx,
                        &session_id,
                        hook_id.as_str(),
                        enabled,
                        &workspace_paths,
                    )
                    .await;
                });
            }
            #[cfg(feature = "kas")]
            BridgeCommand::Workflow {
                session_id,
                workspace_paths,
                op,
            } => {
                let connection = connection.clone();
                let tx = self.bridge.notification_tx.clone();
                self.spawn_command(async move {
                    let _closed =
                        kas::handle_workflow(&connection, &tx, &session_id, &workspace_paths, op)
                            .await;
                });
            }
            BridgeCommand::Shutdown => return Ok(true),
        }
        Ok(false)
    }
}
