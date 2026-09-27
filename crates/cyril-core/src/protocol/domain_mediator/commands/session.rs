use std::future::Future;

use agent_client_protocol::{Agent, ConnectionTo, UntypedMessage, schema::v1 as acp};
use serde::Deserialize;

use super::super::{CommandOutcome, DomainMediator, SessionStart};
use super::{COMMAND_RPC_TIMEOUT, SESSION_RPC_TIMEOUT, await_response};
use crate::protocol::bridge::source_disposition;
use crate::protocol::turn_mediator::BeginTurn;
use crate::types::{
    Notification, RoutedNotification, SessionId, SessionOrigin, SourceTurnDisposition, StopReason,
};

/// The JSON `params` object a standard-method request goes out as — the one
/// serialization step between a typed request and the wire frame.
fn standard_params<Request>(
    method: &str,
    request: Request,
) -> agent_client_protocol::Result<serde_json::Value>
where
    Request: serde::Serialize,
{
    serde_json::to_value(request).map_err(|error| {
        agent_client_protocol::Error::internal_error()
            .data(format!("serialize {method} request: {error}"))
    })
}

/// Serialize and send one standard-method request synchronously — the frame
/// is on the wire (in send order) when this returns — yielding the future
/// that resolves to the raw JSON response. Awaiting that future belongs on a
/// spawned task, never in the mediator loop.
fn send_standard<Request>(
    connection: &ConnectionTo<Agent>,
    method: &str,
    request: Request,
) -> agent_client_protocol::Result<
    impl Future<Output = agent_client_protocol::Result<serde_json::Value>> + 'static,
>
where
    Request: serde::Serialize,
{
    let params = standard_params(method, request)?;
    let message = UntypedMessage::new(method, params)?;
    Ok(connection.send_request(message).block_task())
}

/// The `session/new` request cyril sends for `cwd`. Fenced by
/// `tests::new_session_request_always_carries_mcp_servers` (cyril-0dv4).
fn new_session_request(cwd: &std::path::Path) -> acp::NewSessionRequest {
    acp::NewSessionRequest::new(crate::platform::path::to_agent(cwd))
}

/// The `session/load` request cyril sends to resume `session_id` in `cwd`.
/// Fenced by `tests::load_session_request_always_carries_mcp_servers`
/// (cyril-0dv4).
fn load_session_request(session_id: &SessionId, cwd: &std::path::Path) -> acp::LoadSessionRequest {
    acp::LoadSessionRequest::new(
        acp::SessionId::new(session_id.as_str()),
        crate::platform::path::to_agent(cwd),
    )
}

fn parse_standard<Response>(
    method: &str,
    value: serde_json::Value,
) -> agent_client_protocol::Result<(Response, serde_json::Value)>
where
    Response: serde::de::DeserializeOwned,
{
    let response = serde_json::from_value(value.clone()).map_err(|error| {
        agent_client_protocol::Error::internal_error()
            .data(format!("deserialize {method} response: {error}"))
    })?;
    Ok((response, value))
}

fn session_started_outcome(
    session_id: SessionId,
    origin: SessionOrigin,
    response: &acp::NewSessionResponse,
    raw_response: &serde_json::Value,
) -> CommandOutcome {
    let config_options = response.config_options.as_deref().map(|options| {
        Notification::ConfigOptionsUpdated(crate::protocol::convert::to_config_options(options))
    });
    let created = crate::protocol::convert::session_created_from_response(
        session_id.as_str().to_owned(),
        response.modes.as_ref(),
        raw_response.get("models"),
    );
    CommandOutcome::SessionStarted(Box::new(SessionStart {
        session_id,
        origin,
        config_options,
        created,
    }))
}

/// The recoverable failure of `session/load`: an operation error on a bridge
/// (and any live session) that keeps running.
fn load_failed(error: impl std::fmt::Display) -> Notification {
    Notification::BridgeError {
        operation: "Load session".into(),
        message: error.to_string(),
    }
}

impl DomainMediator {
    pub(super) fn new_session(
        &mut self,
        connection: &ConnectionTo<Agent>,
        cwd: std::path::PathBuf,
    ) -> crate::Result<()> {
        let request = new_session_request(&cwd);
        let engine_kind = self.config.engine.kind();
        let channels = self.channels.clone();
        match send_standard(connection, "session/new", request) {
            Ok(sent) => self.spawn_command(async move {
                let outcome = match await_response(sent, "session/new", SESSION_RPC_TIMEOUT)
                    .await
                    .and_then(|value| {
                        parse_standard::<acp::NewSessionResponse>("session/new", value)
                    }) {
                    Ok((response, raw_response)) => {
                        match crate::protocol::fingerprint::session_id_mismatch(
                            engine_kind,
                            &response.session_id.to_string(),
                            cfg!(feature = "kas"),
                        ) {
                            Some(reason) => CommandOutcome::FatalDisconnect { reason },
                            None => session_started_outcome(
                                SessionId::new(response.session_id.to_string()),
                                SessionOrigin::Fresh,
                                &response,
                                &raw_response,
                            ),
                        }
                    }
                    Err(error) => CommandOutcome::FatalDisconnect {
                        reason: format!("Failed to create session: {error}"),
                    },
                };
                channels.enqueue_outcome(outcome).await;
            }),
            Err(error) => {
                let reason = format!("Failed to create session: {error}");
                self.spawn_command(async move {
                    channels
                        .enqueue_outcome(CommandOutcome::FatalDisconnect { reason })
                        .await;
                });
            }
        }
        Ok(())
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
        let request = load_session_request(&session_id, &self.config.cwd);
        let channels = self.channels.clone();
        match send_standard(connection, "session/load", request) {
            Ok(sent) => self.spawn_command(async move {
                let outcome = match await_response(sent, "session/load", SESSION_RPC_TIMEOUT)
                    .await
                    .and_then(|value| {
                        parse_standard::<acp::LoadSessionResponse>("session/load", value)
                    }) {
                    Ok((response, raw_response)) => {
                        let config_options = response.config_options.as_deref().map(|options| {
                            Notification::ConfigOptionsUpdated(
                                crate::protocol::convert::to_config_options(options),
                            )
                        });
                        let created = crate::protocol::convert::session_created_from_response(
                            session_id.as_str().to_owned(),
                            response.modes.as_ref(),
                            raw_response.get("models"),
                        );
                        CommandOutcome::SessionStarted(Box::new(SessionStart {
                            session_id,
                            origin: SessionOrigin::Loaded,
                            config_options,
                            created,
                        }))
                    }
                    // Unlike session/new (deliberately fatal, fenced by
                    // c5_new_session_rpc_failure_is_fatal), a failed load is
                    // recoverable: the id may be a typo or expired, and any
                    // live session must survive it. Main-line behavior: the
                    // App is told, the bridge keeps running. Reported as a
                    // `BridgeError`, not `BridgeDisconnected`: the latter
                    // resets the live session's state (status, context,
                    // thinking) as if the connection had died (cyril-k3lz).
                    Err(error) => CommandOutcome::notify(load_failed(error)),
                };
                channels.enqueue_outcome(outcome).await;
            }),
            Err(error) => {
                let notification = load_failed(error);
                self.spawn_command(async move {
                    channels
                        .enqueue_outcome(CommandOutcome::notify(notification))
                        .await;
                });
            }
        }
        Ok(false)
    }

    pub(crate) async fn publish_session_start(
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
        // `SessionCreated` is a reset boundary for per-session state (model,
        // effort, thinking), so the response's own config snapshot must land
        // AFTER it — emitted first, the reset would discard it (cyril-k3lz).
        self.notify(created.into()).await?;
        if let Some(config_options) = config_options {
            self.notify(config_options.into()).await?;
        }
        Ok(())
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
        &mut self,
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
        let sent = connection.send_request(request);
        let channels = self.channels.clone();
        self.spawn_command(async move {
            if let Err(error) =
                await_response(sent.block_task(), &operation, COMMAND_RPC_TIMEOUT).await
            {
                channels
                    .enqueue_outcome(CommandOutcome::notify(Notification::BridgeError {
                        operation,
                        message: error.to_string(),
                    }))
                    .await;
            }
        });
        Ok(())
    }

    pub(super) async fn set_model(
        &mut self,
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
        let sent = connection.send_request(request);
        let channels = self.channels.clone();
        self.spawn_command(async move {
            if let Err(error) =
                await_response(sent.block_task(), &operation, COMMAND_RPC_TIMEOUT).await
            {
                channels
                    .enqueue_outcome(CommandOutcome::notify(Notification::BridgeError {
                        operation,
                        message: error.to_string(),
                    }))
                    .await;
            }
        });
        Ok(())
    }

    pub(super) async fn set_config_option(
        &mut self,
        connection: &ConnectionTo<Agent>,
        config_id: String,
        value: String,
    ) -> crate::Result<()> {
        let operation = "set_config_option";
        let Some(session_id) = self.active_session_id.as_ref() else {
            return self
                .notify(
                    Notification::BridgeError {
                        operation: operation.into(),
                        message: "no active session — run /new or /load first".into(),
                    }
                    .into(),
                )
                .await;
        };
        let request = acp::SetSessionConfigOptionRequest::new(
            acp::SessionId::new(session_id.as_str()),
            acp::SessionConfigId::new(config_id.as_str()),
            acp::SessionConfigValueId::new(value),
        );
        // The rebuilt options describe THIS session; bind the answer to it
        // (cyril-k3lz review finding 7).
        let session_id = session_id.clone();
        // Keep wire order with other commands; only the response wait is spawned.
        let sent = send_standard(connection, "session/set_config_option", request);
        let channels = self.channels.clone();
        self.spawn_command(async move {
            let result = async {
                let response = await_response(sent?, operation, COMMAND_RPC_TIMEOUT).await?;
                let raw_options = response.get("configOptions").ok_or_else(|| {
                    agent_client_protocol::Error::internal_error()
                        .data("set_config_option response is missing configOptions")
                })?;
                // Bypass the outer tolerant catalog adapter; borrow the raw value
                // so nested loss checks do not clone JSON or parse choices twice.
                let options =
                    Vec::<acp::SessionConfigOption>::deserialize(raw_options).map_err(|error| {
                        agent_client_protocol::Error::internal_error().data(format!(
                            "deserialize set_config_option configOptions: {error}"
                        ))
                    })?;
                for (option_index, option) in options.iter().enumerate() {
                    if let acp::SessionConfigKind::Select(select) = &option.kind
                        && let acp::SessionConfigSelectOptions::Grouped(groups) = &select.options
                    {
                        for (group_index, group) in groups.iter().enumerate() {
                            // Group.options also uses DefaultOnError<VecSkipError>.
                            // A non-array or a dropped required choice is not an ack.
                            let count =
                                raw_options[option_index]["options"][group_index]["options"]
                                    .as_array()
                                    .map(Vec::len);
                            if count != Some(group.options.len()) {
                                return Err(agent_client_protocol::Error::internal_error().data(
                                    "set_config_option response contains malformed grouped choices",
                                ));
                            }
                        }
                    }
                }
                Ok(options)
            }
            .await;
            let notification = match result {
                Ok(options) => Notification::ConfigOptionSet {
                    config_id,
                    options: crate::protocol::convert::to_config_options(&options),
                },
                Err(error) => Notification::BridgeError {
                    operation: operation.into(),
                    message: error.to_string(),
                },
            };
            channels
                .enqueue_outcome(CommandOutcome::for_session(
                    session_id,
                    operation,
                    notification,
                ))
                .await;
        });
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
        // Send synchronously so the prompt keeps wire order with commands
        // dispatched around it; only the (unbounded) response await is spawned.
        let sent = connection.send_request(request);
        let task = tokio::task::spawn_local(async move {
            let (stop_reason, usage, disposition) = match sent.block_task().await {
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
                        .enqueue(super::super::DomainWork::Routed(
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
            if tokio::time::timeout(
                std::time::Duration::from_millis(50),
                ingress.wait_quiescent(),
            )
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
                if let Err(error) = channels
                    .enqueue(super::super::DomainWork::Routed(routed))
                    .await
                {
                    tracing::debug!(%error, "turn usage notification dropped");
                }
            }
            let routed = RoutedNotification::from(Notification::TurnCompleted { stop_reason })
                .with_turn(owner);
            if let Err(error) = channels
                .enqueue(super::super::DomainWork::PromptTerminal {
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

#[cfg(test)]
mod tests {
    use super::{load_session_request, new_session_request, standard_params};
    use crate::test_support::must_succeed;
    use crate::types::SessionId;

    /// Fence (cyril-0dv4): the params object cyril sends MUST carry
    /// `mcpServers` as a JSON array — empty is fine, absent is not.
    ///
    /// `session/new` failure mode guarded: kiro-cli treats a `session/new`
    /// whose params lack `mcpServers` as malformed and exits rc=0 with NO
    /// stderr — a clean exit that reads as "the binary is fine, the spawn is
    /// wrong" and is the most expensive signature to diagnose
    /// (docs/kirocrew-acp-seam-findings.md §4.1; KiroCrew `_dispatch.py:70-76`).
    ///
    /// `session/load`: the key is required by the ACP schema, and kiro-cli
    /// RE-INITIALIZES the session's MCP servers from it. Per §4.1 even an
    /// EMPTY list is applied and un-pools the resumed session for its whole
    /// life — that is the state cyril sends today, and this fence deliberately
    /// does NOT guard it; what the list must carry on resume belongs to the
    /// session resume contract (cyril-rtrh).
    ///
    /// Omission is impossible through today's SDK type: `agent-client-protocol
    /// =2.0.0` re-exports `agent-client-protocol-schema 1.5.0`, whose
    /// `NewSessionRequest` / `LoadSessionRequest` declare
    /// `mcp_servers: Vec<McpServer>` with no `skip_serializing_if` and seed it
    /// with `vec![]` in `::new()`. This test is the tripwire for an SDK bump
    /// changing that — the schema crate already uses the skip-when-empty
    /// convention on the newer `additional_directories` field, so the drift is
    /// plausible and would otherwise land silently on the wire.
    fn assert_mcp_servers_is_array(method: &str, why_required: &str, params: &serde_json::Value) {
        let mcp_servers = params.get("mcpServers");
        assert!(
            mcp_servers.is_some_and(serde_json::Value::is_array),
            "{method} params must carry `mcpServers` as an array ({why_required}); \
             got {mcp_servers:?} in {params}"
        );
    }

    #[test]
    fn new_session_request_always_carries_mcp_servers() {
        let request = new_session_request(&std::env::temp_dir());
        let params = must_succeed(
            standard_params("session/new", request),
            "serialize session/new params",
        );
        assert_mcp_servers_is_array(
            "session/new",
            "kiro-cli exits rc=0 with no stderr when it is missing",
            &params,
        );
    }

    #[test]
    fn load_session_request_always_carries_mcp_servers() {
        let session_id = SessionId::new("sess_fence-0dv4");
        let request = load_session_request(&session_id, &std::env::temp_dir());
        let params = must_succeed(
            standard_params("session/load", request),
            "serialize session/load params",
        );
        assert_mcp_servers_is_array(
            "session/load",
            "required by the ACP schema; kiro-cli re-initializes the session's MCP servers from it",
            &params,
        );
    }
}
