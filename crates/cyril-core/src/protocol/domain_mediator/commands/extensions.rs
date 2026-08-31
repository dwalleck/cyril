use std::time::Duration;

use agent_client_protocol::{Agent, ConnectionTo, UntypedMessage};

use super::super::{DomainMediator, DomainWork};
use crate::types::{AgentEngine, Notification, SessionId};

impl DomainMediator {
    pub(super) async fn call_extension(
        &self,
        connection: &ConnectionTo<Agent>,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, agent_client_protocol::Error> {
        let wire_method = if method.starts_with('_') {
            method.to_owned()
        } else {
            format!("_{method}")
        };
        let request = UntypedMessage::new(&wire_method, params)?;
        connection.send_request(request).block_task().await
    }

    pub(super) async fn execute_extension(
        &self,
        connection: &ConnectionTo<Agent>,
        method: String,
        params: serde_json::Value,
    ) -> crate::Result<()> {
        let operation = format!("ext_method '{method}'");
        if let Err(error) = self.call_extension(connection, &method, params).await {
            self.notify(
                Notification::BridgeError {
                    operation,
                    message: error.to_string(),
                }
                .into(),
            )
            .await?;
        }
        Ok(())
    }

    pub(super) async fn query_command_options(
        &self,
        connection: &ConnectionTo<Agent>,
        command: String,
        session_id: SessionId,
    ) -> crate::Result<()> {
        let params = serde_json::json!({
            "command": command,
            "sessionId": session_id.as_str(),
            "partial": ""
        });
        let options = match self
            .call_extension(connection, "kiro.dev/commands/options", params)
            .await
        {
            Ok(value) => crate::protocol::convert::kiro::parse_options_response(&value),
            Err(error) => {
                tracing::warn!(%error, %command, "command option query failed");
                Vec::new()
            }
        };
        self.notify(Notification::CommandOptionsReceived { command, options }.into())
            .await
    }

    pub(super) async fn execute_command(
        &self,
        connection: &ConnectionTo<Agent>,
        command: String,
        session_id: SessionId,
        args: serde_json::Value,
    ) -> crate::Result<()> {
        let params = serde_json::json!({
            "sessionId": session_id.as_str(),
            "command": {"command": command, "args": args}
        });
        let response = match self
            .call_extension(connection, "kiro.dev/commands/execute", params)
            .await
        {
            Ok(response) => response,
            Err(error) => serde_json::json!({
                "success": false,
                "error": error.to_string()
            }),
        };
        self.notify(Notification::CommandExecuted { command, response }.into())
            .await
    }

    pub(super) async fn list_settings(
        &self,
        connection: &ConnectionTo<Agent>,
    ) -> crate::Result<()> {
        match self
            .call_extension(connection, "kiro.dev/settings/list", serde_json::json!({}))
            .await
        {
            Ok(settings) => {
                self.notify(Notification::SettingsList { settings }.into())
                    .await
            }
            Err(error) => {
                self.notify(
                    Notification::BridgeError {
                        operation: "settings/list".into(),
                        message: error.to_string(),
                    }
                    .into(),
                )
                .await
            }
        }
    }

    pub(super) fn query_usage_account(&self, connection: &ConnectionTo<Agent>) {
        let connection = connection.clone();
        let engine = self.config.engine.kind();
        let channels = self.channels.clone();
        tokio::task::spawn_local(async move {
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
