use agent_client_protocol::{Agent, ConnectionTo, ErrorCode};

use super::super::{CommandOutcome, DomainMediator};
use crate::types::{Notification, SessionId};

impl DomainMediator {
    pub(super) async fn spawn_session(
        &mut self,
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
        self.spawn_extension_command(connection, "session/spawn", params, move |result| {
            Some(CommandOutcome::notify(match result {
                Ok(value) => match value.get("sessionId").and_then(serde_json::Value::as_str) {
                    Some(id) => Notification::SubagentSpawned {
                        session_id: SessionId::new(id),
                        name,
                    },
                    None => Notification::BridgeError {
                        operation,
                        message: "response missing sessionId".into(),
                    },
                },
                Err(error) => Notification::BridgeError {
                    operation,
                    message: error.to_string(),
                },
            }))
        });
        Ok(())
    }

    pub(super) fn terminate_session(
        &mut self,
        connection: &ConnectionTo<Agent>,
        session_id: SessionId,
    ) {
        let operation = format!("terminate_session '{}'", session_id.as_str());
        let params = serde_json::json!({"sessionId": session_id.as_str()});
        self.spawn_extension_command(
            connection,
            "kiro.dev/session/terminate",
            params,
            move |result| {
                Some(CommandOutcome::notify(match result {
                    Ok(_) => Notification::SubagentTerminated { session_id },
                    Err(error) => Notification::BridgeError {
                        operation,
                        message: error.to_string(),
                    },
                }))
            },
        );
    }

    pub(super) fn send_message(
        &mut self,
        connection: &ConnectionTo<Agent>,
        session_id: SessionId,
        content: String,
    ) {
        let operation = format!("send_message to '{}'", session_id.as_str());
        let params = serde_json::json!({
            "sessionId": session_id.as_str(),
            "content": content
        });
        self.spawn_extension_command(
            connection,
            "message/send",
            params,
            move |result| match result {
                Ok(_) => None,
                Err(error) => Some(CommandOutcome::notify(Notification::BridgeError {
                    operation,
                    message: error.to_string(),
                })),
            },
        );
    }

    pub(super) fn steer_session(
        &mut self,
        connection: &ConnectionTo<Agent>,
        session_id: SessionId,
        message: String,
    ) {
        let operation = format!("steer '{}'", session_id.as_str());
        if self.steering_unsupported.contains(&session_id) {
            return;
        }
        let params = serde_json::json!({
            "sessionId": session_id.as_str(),
            "message": message
        });
        self.spawn_extension_command(
            connection,
            "session/steer",
            params,
            move |result| match result {
                Ok(_) => None,
                Err(error) if error.code == ErrorCode::MethodNotFound => {
                    Some(CommandOutcome::SteeringUnsupported { session_id })
                }
                Err(error) => Some(CommandOutcome::notify(Notification::BridgeError {
                    operation,
                    message: error.to_string(),
                })),
            },
        );
    }

    pub(super) fn clear_steering(
        &mut self,
        connection: &ConnectionTo<Agent>,
        session_id: SessionId,
    ) {
        if self.steering_unsupported.contains(&session_id) {
            return;
        }
        let params = serde_json::json!({"sessionId": session_id.as_str()});
        self.spawn_extension_command(connection, "session/steer/clear", params, move |result| {
            match result {
                Ok(_) => None,
                Err(error) if error.code == ErrorCode::MethodNotFound => {
                    Some(CommandOutcome::notify(
                        Notification::SteeringClearUnsupported {
                            message:
                                "steer/clear isn't supported by this backend — queued steers still apply"
                                    .into(),
                        },
                    ))
                }
                Err(error) => Some(CommandOutcome::notify(Notification::BridgeError {
                    operation: format!("steer/clear '{}'", session_id.as_str()),
                    message: error.to_string(),
                })),
            }
        });
    }
}
