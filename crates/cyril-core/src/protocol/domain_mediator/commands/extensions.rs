use std::future::Future;
use std::time::Duration;

use agent_client_protocol::{Agent, ConnectionTo, UntypedMessage};

use super::super::{CommandOutcome, DomainMediator, DomainWork};
use super::{COMMAND_RPC_TIMEOUT, await_response};
use crate::types::{AgentEngine, Notification, SessionId};

/// Send one `_`-prefixed extension request synchronously (the frame is on
/// the wire, in order, when this returns) and yield the response future for
/// a spawned task to await.
pub(super) fn send_extension(
    connection: &ConnectionTo<Agent>,
    method: &str,
    params: serde_json::Value,
) -> agent_client_protocol::Result<
    impl Future<Output = agent_client_protocol::Result<serde_json::Value>> + 'static,
> {
    let wire_method = if method.starts_with('_') {
        method.to_owned()
    } else {
        format!("_{method}")
    };
    let request = UntypedMessage::new(&wire_method, params)?;
    Ok(connection.send_request(request).block_task())
}

impl DomainMediator {
    /// Send an extension RPC and hand its result to `outcome` on a spawned
    /// task. A send/serialize failure produces the same outcome path with the
    /// error, so every command still reports exactly once.
    pub(super) fn spawn_extension_command(
        &mut self,
        connection: &ConnectionTo<Agent>,
        method: &str,
        params: serde_json::Value,
        outcome: impl FnOnce(agent_client_protocol::Result<serde_json::Value>) -> Option<CommandOutcome>
        + 'static,
    ) {
        let what = format!("'{method}'");
        let channels = self.channels.clone();
        match send_extension(connection, method, params) {
            Ok(sent) => self.spawn_command(async move {
                let result = await_response(sent, &what, COMMAND_RPC_TIMEOUT).await;
                if let Some(outcome) = outcome(result) {
                    channels.enqueue_outcome(outcome).await;
                }
            }),
            Err(error) => self.spawn_command(async move {
                if let Some(outcome) = outcome(Err(error)) {
                    channels.enqueue_outcome(outcome).await;
                }
            }),
        }
    }

    pub(super) fn execute_extension(
        &mut self,
        connection: &ConnectionTo<Agent>,
        method: String,
        params: serde_json::Value,
    ) {
        let operation = format!("ext_method '{method}'");
        self.spawn_extension_command(connection, &method, params, move |result| match result {
            Ok(_) => None,
            Err(error) => Some(CommandOutcome::notify(Notification::BridgeError {
                operation,
                message: error.to_string(),
            })),
        });
    }

    pub(super) fn query_command_options(
        &mut self,
        connection: &ConnectionTo<Agent>,
        command: String,
        session_id: SessionId,
    ) {
        let params = serde_json::json!({
            "command": command,
            "sessionId": session_id.as_str(),
            "partial": ""
        });
        self.spawn_extension_command(
            connection,
            "kiro.dev/commands/options",
            params,
            move |result| {
                let options = match result {
                    Ok(value) => crate::protocol::convert::kiro::parse_options_response(&value),
                    Err(error) => {
                        tracing::warn!(%error, %command, "command option query failed");
                        Vec::new()
                    }
                };
                Some(CommandOutcome::notify(
                    Notification::CommandOptionsReceived { command, options },
                ))
            },
        );
    }

    pub(super) fn execute_command(
        &mut self,
        connection: &ConnectionTo<Agent>,
        command: String,
        session_id: SessionId,
        args: serde_json::Value,
    ) {
        let params = serde_json::json!({
            "sessionId": session_id.as_str(),
            "command": {"command": command, "args": args}
        });
        self.spawn_extension_command(
            connection,
            "kiro.dev/commands/execute",
            params,
            move |result| {
                let response = match result {
                    Ok(response) => response,
                    Err(error) => serde_json::json!({
                        "success": false,
                        "error": error.to_string()
                    }),
                };
                Some(CommandOutcome::notify(Notification::CommandExecuted {
                    command,
                    response,
                }))
            },
        );
    }

    pub(super) fn list_settings(&mut self, connection: &ConnectionTo<Agent>) {
        self.spawn_extension_command(
            connection,
            "kiro.dev/settings/list",
            serde_json::json!({}),
            |result| {
                Some(CommandOutcome::notify(match result {
                    Ok(settings) => Notification::SettingsList { settings },
                    Err(error) => Notification::BridgeError {
                        operation: "settings/list".into(),
                        message: error.to_string(),
                    },
                }))
            },
        );
    }

    pub(super) fn query_usage_account(&mut self, connection: &ConnectionTo<Agent>) {
        let connection = connection.clone();
        let engine = self.config.engine.kind();
        let channels = self.channels.clone();
        self.spawn_command(async move {
            let notification = match tokio::time::timeout(
                Duration::from_secs(5),
                fetch_usage_account(connection, engine),
            )
            .await
            {
                Ok(notification) => notification,
                Err(_) => Notification::UsageAccountQueryFailed {
                    message: "account usage query timed out after 5s".to_owned(),
                },
            };
            if let Err(error) = channels
                .enqueue(DomainWork::Routed(notification.into()))
                .await
            {
                tracing::debug!(%error, "account usage result dropped after bridge closure");
            }
        });
    }
}

async fn fetch_usage_account(connection: ConnectionTo<Agent>, engine: AgentEngine) -> Notification {
    if engine != AgentEngine::Kas {
        return Notification::UsageAccountQueryFailed {
            message: "account usage is available only for the KAS engine".to_owned(),
        };
    }
    let request = match UntypedMessage::new("_kiro/account/getUsage", serde_json::json!({})) {
        Ok(request) => request,
        Err(error) => {
            return Notification::UsageAccountQueryFailed {
                message: error.to_string(),
            };
        }
    };
    match connection.send_request(request).block_task().await {
        Ok(value) => match parse_usage_account(&value) {
            Ok(account) => Notification::UsageAccountUpdated {
                account,
                fetched_at_ms: super::super::current_timestamp_ms(),
            },
            Err(message) => Notification::UsageAccountQueryFailed { message },
        },
        Err(error) => Notification::UsageAccountQueryFailed {
            message: error.to_string(),
        },
    }
}

fn parse_usage_account(_value: &serde_json::Value) -> Result<crate::types::UsageAccount, String> {
    #[cfg(feature = "kas")]
    {
        crate::protocol::convert::kas::account_usage_from_response(_value)
            .map_err(|error| error.to_string())
    }
    #[cfg(not(feature = "kas"))]
    {
        Err("KAS account usage requires a build with --features kas".to_owned())
    }
}
