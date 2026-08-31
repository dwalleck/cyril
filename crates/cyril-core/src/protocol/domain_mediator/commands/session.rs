use std::time::Duration;

use agent_client_protocol::{Agent, ConnectionTo, UntypedMessage, schema::v1 as acp};

use super::super::{DomainMediator, DomainWork};
use crate::protocol::bridge::source_disposition;
use crate::protocol::turn_mediator::BeginTurn;
use crate::types::{
    Notification, RoutedNotification, SessionId, SessionOrigin, SourceTurnDisposition, StopReason,
};

async fn call_standard<Request, Response>(
    connection: &ConnectionTo<Agent>,
    method: &str,
    request: Request,
) -> agent_client_protocol::Result<(Response, serde_json::Value)>
where
    Request: serde::Serialize,
    Response: serde::de::DeserializeOwned,
{
    let params = serde_json::to_value(request).map_err(|error| {
        agent_client_protocol::Error::internal_error()
            .data(format!("serialize {method} request: {error}"))
    })?;
    let message = UntypedMessage::new(method, params)?;
    let value = connection.send_request(message).block_task().await?;
    let response = serde_json::from_value(value.clone()).map_err(|error| {
        agent_client_protocol::Error::internal_error()
            .data(format!("deserialize {method} response: {error}"))
    })?;
    Ok((response, value))
}

impl DomainMediator {
    pub(super) async fn new_session(
        &mut self,
        connection: &ConnectionTo<Agent>,
        cwd: std::path::PathBuf,
    ) -> crate::Result<bool> {
        let request = acp::NewSessionRequest::new(crate::platform::path::to_agent(&cwd));
        match call_standard::<_, acp::NewSessionResponse>(connection, "session/new", request).await
        {
            Ok((response, raw_response)) => {
                if let Some(reason) = crate::protocol::fingerprint::session_id_mismatch(
                    self.config.engine.kind(),
                    &response.session_id.to_string(),
                    cfg!(feature = "kas"),
                ) {
                    self.notify(Notification::BridgeDisconnected { reason }.into())
                        .await?;
                    return Ok(true);
                }
                let session_id = SessionId::new(response.session_id.to_string());
                let config_options = response.config_options.as_deref().map(|options| {
                    Notification::ConfigOptionsUpdated(crate::protocol::convert::to_config_options(
                        options,
                    ))
                });
                let created = crate::protocol::convert::session_created_from_response(
                    session_id.as_str().to_owned(),
                    response.modes.as_ref(),
                    raw_response.get("models"),
                );
                self.publish_session_start(
                    session_id,
                    SessionOrigin::Fresh,
                    config_options,
                    created,
                )
                .await?;
            }
            Err(error) => {
                self.notify(
                    Notification::BridgeDisconnected {
                        reason: format!("Failed to create session: {error}"),
                    }
                    .into(),
                )
                .await?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) async fn load_session(
        &mut self,
        connection: &ConnectionTo<Agent>,
        session_id: SessionId,
    ) -> crate::Result<bool> {
        if let Some(reason) = crate::protocol::fingerprint::session_id_mismatch(
            self.config.engine.kind(),
            session_id.as_str(),
            cfg!(feature = "kas"),
        ) {
            self.notify(Notification::BridgeDisconnected { reason }.into())
                .await?;
            return Ok(true);
        }
        let request = acp::LoadSessionRequest::new(
            acp::SessionId::new(session_id.as_str()),
            crate::platform::path::to_agent(&self.config.cwd),
        );
        match call_standard::<_, acp::LoadSessionResponse>(connection, "session/load", request)
            .await
        {
            Ok((response, raw_response)) => {
                let config_options = response.config_options.as_deref().map(|options| {
                    Notification::ConfigOptionsUpdated(crate::protocol::convert::to_config_options(
                        options,
                    ))
                });
                let created = crate::protocol::convert::session_created_from_response(
                    session_id.as_str().to_owned(),
                    response.modes.as_ref(),
                    raw_response.get("models"),
                );
                self.publish_session_start(
                    session_id,
                    SessionOrigin::Loaded,
                    config_options,
                    created,
                )
                .await?;
            }
            Err(error) => {
                self.notify(
                    Notification::BridgeDisconnected {
                        reason: format!("Failed to load session: {error}"),
                    }
                    .into(),
                )
                .await?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn publish_session_start(
        &mut self,
        session_id: SessionId,
        origin: SessionOrigin,
        config_options: Option<Notification>,
        created: Notification,
    ) -> crate::Result<()> {
        self.active_session_id = Some(session_id.clone());
        self.steering_unsupported.remove(&session_id);
        self.notify(Notification::UsageSessionStarted { session_id, origin }.into())
            .await?;
        if let Some(config_options) = config_options {
            self.notify(config_options.into()).await?;
        }
        self.notify(created.into()).await
    }

    pub(super) async fn cancel_active(&mut self, connection: &ConnectionTo<Agent>) {
        let session_id = self
            .turn_mediator
            .active_turn_session()
            .cloned()
            .or_else(|| self.active_session_id.clone());
        let Some(session_id) = session_id else {
            tracing::warn!("cancel requested but no active session");
            return;
        };
        let acp_session_id = acp::SessionId::new(session_id.as_str());
        if let Err(error) =
            connection.send_notification(acp::CancelNotification::new(acp_session_id.clone()))
        {
            tracing::warn!(%error, "failed to send cancellation notification");
        }
        #[cfg(feature = "kas")]
        self.host_ctx.terminals.reap_session(&acp_session_id).await;
        self.host_mediator.borrow_mut().cancel_scope(&session_id);
    }

    pub(super) async fn set_mode(
        &self,
        connection: &ConnectionTo<Agent>,
        mode_id: String,
    ) -> crate::Result<()> {
        let operation = format!("set_mode '{mode_id}'");
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
        let request = acp::SetSessionModeRequest::new(
            acp::SessionId::new(session_id.as_str()),
            acp::SessionModeId::new(mode_id),
        );
        if let Err(error) = connection.send_request(request).block_task().await {
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

    pub(super) async fn set_model(
        &self,
        connection: &ConnectionTo<Agent>,
        model_id: String,
    ) -> crate::Result<()> {
        let operation = format!("set_model '{model_id}'");
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
        let request = agent_client_protocol::UntypedMessage::new(
            "session/set_model",
            serde_json::json!({
                "sessionId": session_id.as_str(),
                "modelId": model_id,
            }),
        )
        .map_err(|error| {
            crate::Error::from_kind(crate::ErrorKind::Protocol {
                message: format!("failed to build {operation}: {error}"),
            })
        })?;
        if let Err(error) = connection.send_request(request).block_task().await {
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

    pub(crate) async fn start_prompt(
        &mut self,
        connection: ConnectionTo<Agent>,
        session_id: SessionId,
        prompt: crate::types::PromptEnvelope,
    ) -> crate::Result<()> {
        let owner = match self
            .turn_mediator
            .begin_turn(session_id.clone(), self.config.engine.emits_wire_turn_end())
        {
            BeginTurn::Accepted(owner) => owner,
            refused => {
                let message = match refused {
                    BeginTurn::Busy => "a turn is already in progress",
                    BeginTurn::Exhausted | BeginTurn::Accepted(_) => {
                        "turn identity space exhausted"
                    }
                };
                return self
                    .notify(
                        Notification::BridgeError {
                            operation: "prompt".into(),
                            message: message.into(),
                        }
                        .into(),
                    )
                    .await;
            }
        };
        self.turn_liveness.begin(super::super::now_std());
        if let Err(error) =
            self.source_observer
                .begin(session_id.clone(), owner, prompt.original_blocks())
        {
            tracing::warn!(%error, "failed to allocate source turn");
        }
        let ingress = self.ingress.clone();
        let channels = self.channels.clone();
        let usage_session_id = session_id.clone();
        let request = acp::PromptRequest::new(
            acp::SessionId::new(session_id.as_str()),
            prompt
                .into_wire_blocks()
                .into_iter()
                .map(acp::ContentBlock::from)
                .collect(),
        );
        let task = tokio::task::spawn_local(async move {
            let (stop_reason, usage, disposition) =
                match connection.send_request(request).block_task().await {
                    Ok(response) => {
                        let stop_reason =
                            crate::protocol::convert::to_stop_reason(response.stop_reason);
                        (
                            stop_reason,
                            response
                                .usage
                                .as_ref()
                                .map(crate::protocol::convert::to_token_usage),
                            source_disposition(stop_reason),
                        )
                    }
                    Err(error) => {
                        if let Err(send_error) = channels
                            .enqueue(DomainWork::Routed(
                                Notification::BridgeError {
                                    operation: "prompt".into(),
                                    message: error.to_string(),
                                }
                                .into(),
                            ))
                            .await
                        {
                            tracing::debug!(%send_error, "prompt error notification dropped");
                        }
                        (StopReason::EndTurn, None, SourceTurnDisposition::Failed)
                    }
                };
            if tokio::time::timeout(Duration::from_millis(50), ingress.wait_quiescent())
                .await
                .is_err()
            {
                tracing::warn!("source observer quiescence timed out");
            }
            if let Some(usage) = usage {
                let routed = RoutedNotification::scoped(
                    usage_session_id,
                    Notification::TurnUsageCaptured(usage),
                )
                .with_turn(owner);
                if let Err(error) = channels.enqueue(DomainWork::Routed(routed)).await {
                    tracing::debug!(%error, "turn usage notification dropped");
                }
            }
            let routed = RoutedNotification::from(Notification::TurnCompleted { stop_reason })
                .with_turn(owner);
            if let Err(error) = channels
                .enqueue(DomainWork::PromptTerminal {
                    routed,
                    source_disposition: disposition,
                })
                .await
            {
                tracing::debug!(%error, "turn completion notification dropped");
            }
        });
        self.prompt_tasks.retain(|task| !task.is_finished());
        if self.prompt_tasks.len() > 2 {
            tracing::debug!(
                live = self.prompt_tasks.len(),
                "more live prompt tasks than the researched bound"
            );
        }
        self.prompt_tasks.push(task);
        Ok(())
    }
}
