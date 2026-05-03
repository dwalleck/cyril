use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::protocol::convert::session_created_from_response;
use crate::types::StopReason;
use crate::types::event::{BridgeCommand, Notification, PermissionRequest, RoutedNotification};

/// Channel capacities
const COMMAND_CAPACITY: usize = 32;
const NOTIFICATION_CAPACITY: usize = 256;
const PERMISSION_CAPACITY: usize = 16;

/// Handle held by the App (Send side) to communicate with the ACP bridge.
pub struct BridgeHandle {
    command_tx: mpsc::Sender<BridgeCommand>,
    pub(crate) notification_rx: mpsc::Receiver<RoutedNotification>,
    pub(crate) permission_rx: mpsc::Receiver<PermissionRequest>,
}

impl BridgeHandle {
    /// Receive the next notification. Returns None if bridge is dead.
    pub async fn recv_notification(&mut self) -> Option<RoutedNotification> {
        self.notification_rx.recv().await
    }

    /// Receive the next permission request. Returns None if bridge is dead.
    pub async fn recv_permission(&mut self) -> Option<PermissionRequest> {
        self.permission_rx.recv().await
    }

    /// Get a cloneable sender for sending commands to the bridge.
    pub fn sender(&self) -> BridgeSender {
        BridgeSender {
            command_tx: self.command_tx.clone(),
        }
    }

    /// Split into individual receivers and a sender, for use in `tokio::select!`
    /// where borrowing `&mut self` twice is not allowed.
    pub fn split(
        self,
    ) -> (
        BridgeSender,
        mpsc::Receiver<RoutedNotification>,
        mpsc::Receiver<PermissionRequest>,
    ) {
        (
            BridgeSender {
                command_tx: self.command_tx,
            },
            self.notification_rx,
            self.permission_rx,
        )
    }
}

/// Cloneable handle for sending commands to the bridge.
/// Can be passed to spawned tasks.
#[derive(Clone)]
pub struct BridgeSender {
    command_tx: mpsc::Sender<BridgeCommand>,
}

impl BridgeSender {
    /// Create from a raw sender (used in tests and by the bridge).
    pub fn from_sender(tx: mpsc::Sender<BridgeCommand>) -> Self {
        Self { command_tx: tx }
    }

    /// Send a command to the ACP bridge. Returns Err if bridge is dead.
    pub async fn send(&self, cmd: BridgeCommand) -> crate::Result<()> {
        self.command_tx
            .send(cmd)
            .await
            .map_err(|_| crate::Error::from_kind(crate::ErrorKind::BridgeClosed))
    }
}

/// The bridge side of the channels (held by the bridge thread).
pub(crate) struct BridgeChannels {
    pub command_rx: mpsc::Receiver<BridgeCommand>,
    pub notification_tx: mpsc::Sender<RoutedNotification>,
    pub permission_tx: mpsc::Sender<PermissionRequest>,
}

/// Create a matched pair of BridgeHandle + BridgeChannels.
pub(crate) fn create_channel_pair() -> (BridgeHandle, BridgeChannels) {
    let (command_tx, command_rx) = mpsc::channel(COMMAND_CAPACITY);
    let (notification_tx, notification_rx) = mpsc::channel(NOTIFICATION_CAPACITY);
    let (permission_tx, permission_rx) = mpsc::channel(PERMISSION_CAPACITY);

    let handle = BridgeHandle {
        command_tx,
        notification_rx,
        permission_rx,
    };

    let channels = BridgeChannels {
        command_rx,
        notification_tx,
        permission_tx,
    };

    (handle, channels)
}

/// Spawn the ACP bridge on a dedicated thread.
/// Returns a BridgeHandle for the Send world to communicate through.
///
/// `agent_command[0]` is the agent binary; `agent_command[1..]` are arguments.
/// Returns an error if the slice is empty.
pub fn spawn_bridge(agent_command: Vec<String>, cwd: PathBuf) -> crate::Result<BridgeHandle> {
    let (handle, channels) = create_channel_pair();

    std::thread::Builder::new()
        .name("acp-bridge".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();

            match rt {
                Ok(rt) => {
                    let local = tokio::task::LocalSet::new();
                    local.block_on(&rt, async move {
                        if let Err(e) = run_bridge(&agent_command, &cwd, channels).await {
                            tracing::error!(error = %e, "bridge terminated with error");
                        }
                    });
                }
                Err(e) => {
                    tracing::error!(error = %e, "failed to create bridge runtime");
                }
            }
        })
        .map_err(|e| {
            crate::Error::with_source(
                crate::ErrorKind::Transport {
                    detail: "failed to spawn bridge thread".into(),
                },
                e,
            )
        })?;

    Ok(handle)
}

/// Serialize a JSON value to an `Arc<RawValue>` for use with `ext_method`.
fn to_raw_arc(
    params: &serde_json::Value,
) -> std::result::Result<Arc<serde_json::value::RawValue>, serde_json::Error> {
    let json_str = serde_json::to_string(params)?;
    let raw = serde_json::value::RawValue::from_string(json_str)?;
    Ok(raw.into())
}

/// Parse an ACP `ExtResponse` payload into a `serde_json::Value`. Returns the
/// raw `serde_json::Error` on failure so callers can emit a `BridgeError`
/// notification distinguishing "Kiro returned malformed JSON" from "Kiro
/// returned a legitimately empty result" — the two should not collapse into
/// the same empty-picker/blank-response UI state.
fn parse_response(
    raw: &serde_json::value::RawValue,
) -> std::result::Result<serde_json::Value, serde_json::Error> {
    serde_json::from_str(raw.get())
}

/// Send a notification on the bridge channel; returns `true` if the channel
/// is closed (App died). Callers should `break` the command loop when this
/// returns true — otherwise the bridge keeps the Kiro subprocess alive after
/// the App has gone away. Use for BOTH success and error paths to keep the
/// "App disconnected" detection symmetric.
async fn notify_or_closed(
    tx: &mpsc::Sender<RoutedNotification>,
    notification: Notification,
) -> bool {
    tx.send(notification.into()).await.is_err()
}

async fn run_bridge(
    agent_command: &[String],
    cwd: &std::path::Path,
    mut channels: BridgeChannels,
) -> crate::Result<()> {
    use acp::Agent;
    use agent_client_protocol as acp;
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

    use crate::protocol::client::KiroClient;
    use crate::protocol::transport::AgentProcess;

    // 1. Spawn agent process
    let process = AgentProcess::spawn(agent_command, cwd).await?;

    // 2. Create KiroClient
    let client = KiroClient::new(
        channels.notification_tx.clone(),
        channels.permission_tx.clone(),
    );

    // 3. Create the ACP connection.
    //    ClientSideConnection::new returns (conn, io_task).
    //    The io_task must be spawned on the LocalSet so the RPC layer runs.
    let (conn, io_task) = acp::ClientSideConnection::new(
        client,
        process.stdin.compat_write(),
        process.stdout.compat(),
        |fut| {
            tokio::task::spawn_local(fut);
        },
    );

    // Spawn the IO pump on the local task set
    tokio::task::spawn_local(async move {
        if let Err(e) = io_task.await {
            tracing::error!(error = %e, "ACP IO task failed");
        }
    });

    // 4. ACP handshake
    let init_request = acp::InitializeRequest::new(acp::ProtocolVersion::V1)
        .client_info(acp::Implementation::new("cyril", env!("CARGO_PKG_VERSION")))
        .client_capabilities(acp::ClientCapabilities::new());

    let _init_response: acp::InitializeResponse =
        conn.initialize(init_request).await.map_err(|e| {
            crate::Error::from_kind(crate::ErrorKind::Protocol {
                message: format!("ACP initialization failed: {e}"),
            })
        })?;

    tracing::info!("ACP bridge initialized");

    // 5. Command loop
    let mut active_session_id: Option<acp::SessionId> = None;

    while let Some(cmd) = channels.command_rx.recv().await {
        match cmd {
            BridgeCommand::NewSession { cwd: session_cwd } => {
                let translated_cwd = crate::platform::path::to_agent(&session_cwd);
                match conn
                    .new_session(acp::NewSessionRequest::new(translated_cwd))
                    .await
                {
                    Ok(response) => {
                        active_session_id = Some(response.session_id.clone());
                        let notification = session_created_from_response(
                            response.session_id.to_string(),
                            response.modes.as_ref(),
                            response.models.as_ref(),
                        );
                        if notify_or_closed(&channels.notification_tx, notification).await {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "new_session failed");
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::BridgeDisconnected {
                                reason: format!("Failed to create session: {e}"),
                            },
                        )
                        .await
                        {
                            break;
                        }
                    }
                }
            }
            BridgeCommand::SendPrompt {
                session_id,
                content_blocks,
            } => {
                let acp_session_id = acp::SessionId::new(session_id.as_str());
                let prompt: Vec<acp::ContentBlock> = content_blocks
                    .into_iter()
                    .map(acp::ContentBlock::from)
                    .collect();
                let request = acp::PromptRequest::new(acp_session_id, prompt);
                match conn.prompt(request).await {
                    Ok(response) => {
                        let stop_reason =
                            crate::protocol::convert::to_stop_reason(response.stop_reason);
                        if channels
                            .notification_tx
                            .send(Notification::TurnCompleted { stop_reason }.into())
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "prompt failed");
                        // Transport error — no PromptResponse available. EndTurn is a
                        // placeholder; the BridgeError notification (if sent) carries
                        // the real failure detail.
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::TurnCompleted {
                                stop_reason: StopReason::EndTurn,
                            },
                        )
                        .await
                        {
                            break;
                        }
                    }
                }
            }
            BridgeCommand::CancelRequest => {
                if let Some(ref session_id) = active_session_id {
                    if let Err(e) = conn
                        .cancel(acp::CancelNotification::new(session_id.clone()))
                        .await
                    {
                        tracing::warn!(error = %e, "failed to send cancel notification");
                    }
                } else {
                    tracing::warn!("cancel requested but no active session");
                }
            }
            BridgeCommand::SetMode { mode_id } => {
                let Some(ref session_id) = active_session_id else {
                    tracing::warn!(mode_id, "set_mode requested but no active session");
                    if notify_or_closed(
                        &channels.notification_tx,
                        Notification::BridgeError {
                            operation: format!("set_mode '{mode_id}'"),
                            message: "no active session — run /new or /load first".into(),
                        },
                    )
                    .await
                    {
                        break;
                    }
                    continue;
                };
                match conn
                    .set_session_mode(acp::SetSessionModeRequest::new(
                        session_id.clone(),
                        mode_id.clone(),
                    ))
                    .await
                {
                    Ok(_) => {
                        tracing::info!(mode_id, "mode changed");
                    }
                    Err(e) => {
                        tracing::error!(error = %e, mode_id, "set_session_mode failed");
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::BridgeError {
                                operation: format!("set_mode '{mode_id}'"),
                                message: e.to_string(),
                            },
                        )
                        .await
                        {
                            break;
                        }
                    }
                }
            }
            BridgeCommand::SetModel { model_id } => {
                let Some(ref session_id) = active_session_id else {
                    tracing::warn!(model_id, "set_model requested but no active session");
                    if notify_or_closed(
                        &channels.notification_tx,
                        Notification::BridgeError {
                            operation: format!("set_model '{model_id}'"),
                            message: "no active session — run /new or /load first".into(),
                        },
                    )
                    .await
                    {
                        break;
                    }
                    continue;
                };
                match conn
                    .set_session_model(acp::SetSessionModelRequest::new(
                        session_id.clone(),
                        acp::ModelId::new(model_id.clone()),
                    ))
                    .await
                {
                    Ok(_) => {
                        tracing::info!(model_id, "model changed");
                    }
                    Err(e) => {
                        tracing::error!(error = %e, model_id, "set_session_model failed");
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::BridgeError {
                                operation: format!("set_model '{model_id}'"),
                                message: e.to_string(),
                            },
                        )
                        .await
                        {
                            break;
                        }
                    }
                }
            }
            BridgeCommand::LoadSession { session_id } => {
                let acp_session_id = acp::SessionId::new(session_id.as_str());
                match conn
                    .load_session(acp::LoadSessionRequest::new(acp_session_id.clone(), cwd))
                    .await
                {
                    Ok(response) => {
                        active_session_id = Some(acp_session_id);
                        tracing::info!(session_id = session_id.as_str(), "session loaded");
                        let notification = session_created_from_response(
                            session_id.as_str().to_string(),
                            response.modes.as_ref(),
                            response.models.as_ref(),
                        );
                        if notify_or_closed(&channels.notification_tx, notification).await {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            session_id = session_id.as_str(),
                            "load_session failed"
                        );
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::BridgeDisconnected {
                                reason: format!("Failed to load session: {e}"),
                            },
                        )
                        .await
                        {
                            break;
                        }
                    }
                }
            }
            BridgeCommand::ExtMethod { method, params } => {
                let raw_arc = match to_raw_arc(&params) {
                    Ok(arc) => arc,
                    Err(e) => {
                        tracing::error!(error = %e, method, "failed to serialize ext params");
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::BridgeError {
                                operation: format!("ext_method '{method}'"),
                                message: format!("serialize params: {e}"),
                            },
                        )
                        .await
                        {
                            break;
                        }
                        continue;
                    }
                };
                if let Err(e) = conn
                    .ext_method(acp::ExtRequest::new(&*method, raw_arc))
                    .await
                {
                    tracing::error!(error = %e, method, "ext_method failed");
                    if notify_or_closed(
                        &channels.notification_tx,
                        Notification::BridgeError {
                            operation: format!("ext_method '{method}'"),
                            message: e.to_string(),
                        },
                    )
                    .await
                    {
                        break;
                    }
                }
            }
            BridgeCommand::QueryCommandOptions {
                command,
                session_id,
            } => {
                let params = serde_json::json!({
                    "command": command,
                    "sessionId": session_id.as_str(),
                    // Kiro's `kiro.dev/commands/options` requires `partial:
                    // string` (docs/kiro-acp-protocol-2.0.1.md §7). We don't
                    // surface in-progress filter text to the bridge yet, so
                    // send an empty string to request the full option list.
                    "partial": "",
                });
                let raw_arc = match to_raw_arc(&params) {
                    Ok(arc) => arc,
                    Err(e) => {
                        tracing::error!(error = %e, command, "failed to serialize options query");
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::BridgeError {
                                operation: format!("commands/options '{command}'"),
                                message: format!("serialize params: {e}"),
                            },
                        )
                        .await
                        {
                            break;
                        }
                        continue;
                    }
                };
                match conn
                    .ext_method(acp::ExtRequest::new("kiro.dev/commands/options", raw_arc))
                    .await
                {
                    Ok(response) => match parse_response(&response.0) {
                        Ok(value) => {
                            let options = crate::commands::parse_options_response(&value);
                            if notify_or_closed(
                                &channels.notification_tx,
                                Notification::CommandOptionsReceived { command, options },
                            )
                            .await
                            {
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, command, "failed to parse commands/options response");
                            if notify_or_closed(
                                &channels.notification_tx,
                                Notification::BridgeError {
                                    operation: format!("commands/options '{command}'"),
                                    message: format!("malformed JSON from kiro: {e}"),
                                },
                            )
                            .await
                            {
                                break;
                            }
                        }
                    },
                    Err(e) => {
                        tracing::error!(error = %e, command, "commands/options query failed");
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::CommandOptionsReceived {
                                command,
                                options: vec![],
                            },
                        )
                        .await
                        {
                            break;
                        }
                    }
                }
            }
            BridgeCommand::ExecuteCommand {
                command,
                session_id,
                args,
            } => {
                let params = serde_json::json!({
                    "sessionId": session_id.as_str(),
                    "command": {
                        "command": command,
                        "args": args,
                    }
                });
                let raw_arc = match to_raw_arc(&params) {
                    Ok(arc) => arc,
                    Err(e) => {
                        tracing::error!(error = %e, command, "failed to serialize execute params");
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::BridgeError {
                                operation: format!("commands/execute '{command}'"),
                                message: format!("serialize params: {e}"),
                            },
                        )
                        .await
                        {
                            break;
                        }
                        continue;
                    }
                };
                match conn
                    .ext_method(acp::ExtRequest::new("kiro.dev/commands/execute", raw_arc))
                    .await
                {
                    Ok(response) => match parse_response(&response.0) {
                        Ok(value) => {
                            if notify_or_closed(
                                &channels.notification_tx,
                                Notification::CommandExecuted {
                                    command,
                                    response: value,
                                },
                            )
                            .await
                            {
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, command, "failed to parse commands/execute response");
                            if notify_or_closed(
                                &channels.notification_tx,
                                Notification::BridgeError {
                                    operation: format!("commands/execute '{command}'"),
                                    message: format!("malformed JSON from kiro: {e}"),
                                },
                            )
                            .await
                            {
                                break;
                            }
                        }
                    },
                    Err(e) => {
                        tracing::error!(error = %e, command, "commands/execute failed");
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::CommandExecuted {
                                command,
                                response: serde_json::json!({
                                    "success": false,
                                    "error": format!("{e}"),
                                }),
                            },
                        )
                        .await
                        {
                            break;
                        }
                    }
                }
            }
            BridgeCommand::SpawnSession { task, name } => {
                let Some(ref session_id) = active_session_id else {
                    tracing::warn!(name, "spawn requested but no active session");
                    if notify_or_closed(
                        &channels.notification_tx,
                        Notification::BridgeError {
                            operation: format!("spawn_session '{name}'"),
                            message: "no active session — run /new or /load first".into(),
                        },
                    )
                    .await
                    {
                        break;
                    }
                    continue;
                };
                let params = serde_json::json!({
                    "sessionId": session_id.to_string(),
                    "task": task,
                    "name": name,
                });
                let raw_arc = match to_raw_arc(&params) {
                    Ok(arc) => arc,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to serialize spawn params");
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::BridgeError {
                                operation: format!("spawn_session '{name}'"),
                                message: format!("serialize params: {e}"),
                            },
                        )
                        .await
                        {
                            break;
                        }
                        continue;
                    }
                };
                match conn
                    .ext_method(acp::ExtRequest::new("session/spawn", raw_arc))
                    .await
                {
                    Ok(response) => match parse_response(&response.0) {
                        Ok(val) => match val.get("sessionId").and_then(|s| s.as_str()) {
                            Some(spawned_id) => {
                                tracing::info!(name, spawned_id, "spawned session");
                                if notify_or_closed(
                                    &channels.notification_tx,
                                    Notification::SubagentSpawned {
                                        session_id: crate::types::SessionId::new(spawned_id),
                                        name: name.clone(),
                                    },
                                )
                                .await
                                {
                                    break;
                                }
                            }
                            None => {
                                tracing::warn!(
                                    name,
                                    "session/spawn succeeded but response missing sessionId"
                                );
                                if notify_or_closed(
                                    &channels.notification_tx,
                                    Notification::BridgeError {
                                        operation: format!("spawn_session '{name}'"),
                                        message: "response missing sessionId".into(),
                                    },
                                )
                                .await
                                {
                                    break;
                                }
                            }
                        },
                        Err(e) => {
                            tracing::error!(error = %e, name, "failed to parse session/spawn response");
                            if notify_or_closed(
                                &channels.notification_tx,
                                Notification::BridgeError {
                                    operation: format!("spawn_session '{name}'"),
                                    message: format!("malformed JSON from kiro: {e}"),
                                },
                            )
                            .await
                            {
                                break;
                            }
                        }
                    },
                    Err(e) => {
                        tracing::error!(error = %e, name, "session/spawn failed");
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::BridgeError {
                                operation: format!("spawn_session '{name}'"),
                                message: e.to_string(),
                            },
                        )
                        .await
                        {
                            break;
                        }
                    }
                }
            }
            BridgeCommand::TerminateSession { session_id: target } => {
                let params = serde_json::json!({
                    "sessionId": target.as_str(),
                });
                let raw_arc = match to_raw_arc(&params) {
                    Ok(arc) => arc,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to serialize terminate params");
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::BridgeError {
                                operation: format!("terminate_session '{}'", target.as_str()),
                                message: format!("serialize params: {e}"),
                            },
                        )
                        .await
                        {
                            break;
                        }
                        continue;
                    }
                };
                match conn
                    .ext_method(acp::ExtRequest::new("session/terminate", raw_arc))
                    .await
                {
                    Ok(_) => {
                        tracing::info!(session_id = target.as_str(), "terminated session");
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::SubagentTerminated {
                                session_id: target.clone(),
                            },
                        )
                        .await
                        {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            session_id = target.as_str(),
                            "session/terminate failed"
                        );
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::BridgeError {
                                operation: format!("terminate_session '{}'", target.as_str()),
                                message: e.to_string(),
                            },
                        )
                        .await
                        {
                            break;
                        }
                    }
                }
            }
            BridgeCommand::SendMessage {
                session_id: target,
                content,
            } => {
                let params = serde_json::json!({
                    "sessionId": target.as_str(),
                    "content": content,
                });
                let raw_arc = match to_raw_arc(&params) {
                    Ok(arc) => arc,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to serialize message params");
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::BridgeError {
                                operation: format!("send_message to '{}'", target.as_str()),
                                message: format!("serialize params: {e}"),
                            },
                        )
                        .await
                        {
                            break;
                        }
                        continue;
                    }
                };
                if let Err(e) = conn
                    .ext_method(acp::ExtRequest::new("message/send", raw_arc))
                    .await
                {
                    tracing::error!(
                        error = %e,
                        session_id = target.as_str(),
                        "message/send failed"
                    );
                    if notify_or_closed(
                        &channels.notification_tx,
                        Notification::BridgeError {
                            operation: format!("send_message to '{}'", target.as_str()),
                            message: e.to_string(),
                        },
                    )
                    .await
                    {
                        break;
                    }
                }
            }
            BridgeCommand::Shutdown => {
                tracing::info!("bridge shutting down");
                break;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[tokio::test]
    async fn send_on_closed_channel_returns_error() {
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(1);
        let (_, notif_rx) = tokio::sync::mpsc::channel(1);
        let (_, perm_rx) = tokio::sync::mpsc::channel(1);

        let bridge_handle = BridgeHandle {
            command_tx: cmd_tx,
            notification_rx: notif_rx,
            permission_rx: perm_rx,
        };

        let sender = bridge_handle.sender();
        // Drop the receiver for commands (simulating bridge death)
        drop(cmd_rx);
        drop(bridge_handle);

        let result = sender.send(BridgeCommand::Shutdown).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn recv_notification_returns_none_when_sender_dropped() {
        let (mut handle, bridge_side) = create_channel_pair();
        drop(bridge_side.notification_tx);
        let result = handle.recv_notification().await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn recv_permission_returns_none_when_sender_dropped() {
        let (mut handle, bridge_side) = create_channel_pair();
        drop(bridge_side.permission_tx);
        let result = handle.recv_permission().await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn notification_roundtrip() -> anyhow::Result<()> {
        let (mut handle, bridge_side) = create_channel_pair();
        let notification = Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        };
        bridge_side
            .notification_tx
            .send(notification.into())
            .await?;
        let received = handle.recv_notification().await.expect("notification");
        assert!(received.session_id.is_none());
        assert!(matches!(
            received.notification,
            Notification::TurnCompleted { .. }
        ));
        Ok(())
    }

    #[tokio::test]
    async fn command_roundtrip() -> anyhow::Result<()> {
        let (handle, mut bridge_side) = create_channel_pair();
        let sender = handle.sender();
        sender.send(BridgeCommand::Shutdown).await?;
        let received = bridge_side.command_rx.recv().await;
        assert!(matches!(received, Some(BridgeCommand::Shutdown)));
        Ok(())
    }

    #[tokio::test]
    async fn sender_is_cloneable() -> anyhow::Result<()> {
        let (handle, mut bridge_side) = create_channel_pair();
        let sender1 = handle.sender();
        let sender2 = sender1.clone();
        sender1.send(BridgeCommand::CancelRequest).await?;
        sender2.send(BridgeCommand::Shutdown).await?;
        let r1 = bridge_side.command_rx.recv().await;
        let r2 = bridge_side.command_rx.recv().await;
        assert!(matches!(r1, Some(BridgeCommand::CancelRequest)));
        assert!(matches!(r2, Some(BridgeCommand::Shutdown)));
        Ok(())
    }

    #[tokio::test]
    async fn query_command_options_roundtrip() -> anyhow::Result<()> {
        let (handle, mut bridge_side) = create_channel_pair();
        let sender = handle.sender();

        sender
            .send(BridgeCommand::QueryCommandOptions {
                command: "model".into(),
                session_id: crate::types::SessionId::new("sess_test"),
            })
            .await?;

        let cmd = bridge_side.command_rx.recv().await;
        if let Some(BridgeCommand::QueryCommandOptions {
            command,
            session_id,
        }) = cmd
        {
            assert_eq!(command, "model");
            assert_eq!(session_id.as_str(), "sess_test");
        } else {
            panic!("expected QueryCommandOptions, got {cmd:?}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn execute_command_roundtrip() -> anyhow::Result<()> {
        let (handle, mut bridge_side) = create_channel_pair();
        let sender = handle.sender();

        sender
            .send(BridgeCommand::ExecuteCommand {
                command: "tools".into(),
                session_id: crate::types::SessionId::new("sess_test"),
                args: serde_json::json!({}),
            })
            .await?;

        let cmd = bridge_side.command_rx.recv().await;
        if let Some(BridgeCommand::ExecuteCommand {
            command,
            session_id,
            args,
        }) = cmd
        {
            assert_eq!(command, "tools");
            assert_eq!(session_id.as_str(), "sess_test");
            assert_eq!(args, serde_json::json!({}));
        } else {
            panic!("expected ExecuteCommand, got {cmd:?}");
        }
        Ok(())
    }

    // --- parse_response ---

    // `RawValue` validates JSON at construction time in practice (Kiro's
    // `ExtResponse(Arc<RawValue>)` always holds validated bytes), so the
    // `Err` path is defensive and unreachable from legitimate inputs. Test
    // the happy path to lock the return-type contract.
    #[test]
    fn parse_response_parses_valid_json_object() {
        let raw = serde_json::value::RawValue::from_string(r#"{"ok":true}"#.into())
            .expect("build raw value");
        let value = parse_response(&raw).expect("valid JSON parses");
        assert_eq!(value["ok"], serde_json::json!(true));
    }
}
