use agent_client_protocol::{Agent, ConnectionTo, ErrorCode};

use super::super::DomainMediator;
use crate::types::{Notification, SessionId};

impl DomainMediator {
    pub(super) async fn spawn_session(
        &self,
        connection: &ConnectionTo<Agent>,
        task: String,
        name: String,
    ) -> crate::Result<()> {
        let operation = format!("spawn_session '{name}'");
        let Some(session_id) = self.active_session_id.as_ref() else {
            return self
                .notify(
                    Notification::BridgeError {
                        operation,
                        message: "no active session — run /new or /load first".into(),
                    }
                    .into(),
                )
                .await;
        };
        let params = serde_json::json!({
            "sessionId": session_id.as_str(),
            "task": task,
            "name": name
        });
        match self
            .call_extension(connection, "session/spawn", params)
            .await
        {
            Ok(value) => {
                if let Some(id) = value.get("sessionId").and_then(serde_json::Value::as_str) {
                    self.notify(
                        Notification::SubagentSpawned {
                            session_id: SessionId::new(id),
                            name,
                        }
                        .into(),
                    )
                    .await
                } else {
                    self.notify(
                        Notification::BridgeError {
                            operation,
                            message: "response missing sessionId".into(),
                        }
                        .into(),
                    )
                    .await
                }
            }
            Err(error) => {
                self.notify(
                    Notification::BridgeError {
                        operation,
                        message: error.to_string(),
                    }
                    .into(),
                )
                .await
            }
        }
    }

    pub(super) async fn terminate_session(
        &self,
        connection: &ConnectionTo<Agent>,
        session_id: SessionId,
    ) -> crate::Result<()> {
        let operation = format!("terminate_session '{}'", session_id.as_str());
        match self
            .call_extension(
                connection,
                "kiro.dev/session/terminate",
                serde_json::json!({"sessionId": session_id.as_str()}),
            )
            .await
        {
            Ok(_) => {
                self.notify(Notification::SubagentTerminated { session_id }.into())
                    .await
            }
            Err(error) => {
                self.notify(
                    Notification::BridgeError {
                        operation,
                        message: error.to_string(),
                    }
                    .into(),
                )
                .await
            }
        }
    }

    pub(super) async fn send_message(
        &self,
        connection: &ConnectionTo<Agent>,
        session_id: SessionId,
        content: String,
    ) -> crate::Result<()> {
        let operation = format!("send_message to '{}'", session_id.as_str());
        if let Err(error) = self
            .call_extension(
                connection,
                "message/send",
                serde_json::json!({
                    "sessionId": session_id.as_str(),
                    "content": content
                }),
            )
            .await
        {
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

    pub(super) async fn steer_session(
        &mut self,
        connection: &ConnectionTo<Agent>,
        session_id: SessionId,
        message: String,
    ) -> crate::Result<()> {
        let operation = format!("steer '{}'", session_id.as_str());
        if self.steering_unsupported.contains(&session_id) {
            return Ok(());
        }
        match self
            .call_extension(
                connection,
                "session/steer",
                serde_json::json!({
                    "sessionId": session_id.as_str(),
                    "message": message
                }),
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(error) if error.code == ErrorCode::MethodNotFound => {
                self.steering_unsupported.insert(session_id);
                self.notify(
                    Notification::SteeringUnsupported {
                        message: "steering requires kiro-cli 2.7.0+".into(),
                    }
                    .into(),
                )
                .await
            }
            Err(error) => {
                self.notify(
                    Notification::BridgeError {
                        operation,
                        message: error.to_string(),
                    }
                    .into(),
                )
                .await
            }
        }
    }

    pub(super) async fn clear_steering(
        &self,
        connection: &ConnectionTo<Agent>,
        session_id: SessionId,
    ) -> crate::Result<()> {
        if self.steering_unsupported.contains(&session_id) {
            return Ok(());
        }
        if let Err(error) = self
            .call_extension(
                connection,
                "session/steer/clear",
                serde_json::json!({"sessionId": session_id.as_str()}),
            )
            .await
        {
            let notification = if error.code == ErrorCode::MethodNotFound {
                Notification::SteeringClearUnsupported {
                    message:
                        "steer/clear isn't supported by this backend — queued steers still apply"
                            .into(),
                }
            } else {
                Notification::BridgeError {
                    operation: format!("steer/clear '{}'", session_id.as_str()),
                    message: error.to_string(),
                }
            };
            self.notify(notification.into()).await?;
        }
        Ok(())
    }
}
