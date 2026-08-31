mod extensions;
#[cfg(feature = "kas")]
pub(super) mod kas;
mod session;
mod subagents;

use agent_client_protocol::{Agent, ConnectionTo};

use super::DomainMediator;
use crate::types::BridgeCommand;
#[cfg(not(feature = "kas"))]
use crate::types::Notification;

impl DomainMediator {
    pub(super) async fn handle_command(
        &mut self,
        connection: &ConnectionTo<Agent>,
        command: BridgeCommand,
    ) -> crate::Result<bool> {
        match command {
            BridgeCommand::NewSession { cwd } => return self.new_session(connection, cwd).await,
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
                self.execute_extension(connection, method, params).await?;
            }
            BridgeCommand::QueryCommandOptions {
                command,
                session_id,
            } => {
                self.query_command_options(connection, command, session_id)
                    .await?;
            }
            BridgeCommand::ExecuteCommand {
                command,
                session_id,
                args,
            } => {
                self.execute_command(connection, command, session_id, args)
                    .await?;
            }
            BridgeCommand::SpawnSession { task, name } => {
                self.spawn_session(connection, task, name).await?;
            }
            BridgeCommand::TerminateSession { session_id } => {
                self.terminate_session(connection, session_id).await?;
            }
            BridgeCommand::SendMessage {
                session_id,
                content,
            } => {
                self.send_message(connection, session_id, content).await?;
            }
            BridgeCommand::ListSettings => self.list_settings(connection).await?,
            BridgeCommand::QueryUsageAccount => self.query_usage_account(connection),
            BridgeCommand::SteerSession {
                session_id,
                message,
            } => {
                self.steer_session(connection, session_id, message).await?;
            }
            BridgeCommand::ClearSteering { session_id } => {
                self.clear_steering(connection, session_id).await?;
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
                if kas::handle_list_hooks(
                    connection,
                    &self.bridge.notification_tx,
                    &session_id,
                    &workspace_paths,
                )
                .await
                {
                    return Ok(true);
                }
            }
            #[cfg(feature = "kas")]
            BridgeCommand::SetKasHookEnabled {
                session_id,
                hook_id,
                enabled,
                workspace_paths,
            } => {
                if kas::handle_set_hook_enabled(
                    connection,
                    &self.bridge.notification_tx,
                    &session_id,
                    hook_id.as_str(),
                    enabled,
                    &workspace_paths,
                )
                .await
                {
                    return Ok(true);
                }
            }
            #[cfg(feature = "kas")]
            BridgeCommand::Workflow {
                session_id,
                workspace_paths,
                op,
            } => {
                if kas::handle_workflow(
                    connection,
                    &self.bridge.notification_tx,
                    &session_id,
                    &workspace_paths,
                    op,
                )
                .await
                {
                    return Ok(true);
                }
            }
            BridgeCommand::Shutdown => return Ok(true),
        }
        Ok(false)
    }
}
