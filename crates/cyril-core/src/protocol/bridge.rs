use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::types::event::{BridgeCommand, Notification, PermissionRequest};

/// Channel capacities
const COMMAND_CAPACITY: usize = 32;
const NOTIFICATION_CAPACITY: usize = 256;
const PERMISSION_CAPACITY: usize = 16;

/// Handle held by the App (Send side) to communicate with the ACP bridge.
pub struct BridgeHandle {
    command_tx: mpsc::Sender<BridgeCommand>,
    pub(crate) notification_rx: mpsc::Receiver<Notification>,
    pub(crate) permission_rx: mpsc::Receiver<PermissionRequest>,
}

impl BridgeHandle {
    /// Receive the next notification. Returns None if bridge is dead.
    pub async fn recv_notification(&mut self) -> Option<Notification> {
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
        mpsc::Receiver<Notification>,
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

    /// Send an ext method request and await the response.
    /// Used for request/response patterns like `kiro.dev/commands/options`.
    pub async fn send_ext_method_with_response(
        &self,
        method: impl Into<String>,
        params: serde_json::Value,
    ) -> crate::Result<serde_json::Value> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.send(BridgeCommand::ExtMethodWithResponse {
            method: method.into(),
            params,
            response_tx: tx,
        })
        .await?;
        rx.await.map_err(|_| {
            crate::Error::from_kind(crate::ErrorKind::BridgeClosed)
        })?
    }
}

/// The bridge side of the channels (held by the bridge thread).
pub(crate) struct BridgeChannels {
    pub command_rx: mpsc::Receiver<BridgeCommand>,
    pub notification_tx: mpsc::Sender<Notification>,
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
pub fn spawn_bridge(agent: &str, cwd: PathBuf) -> crate::Result<BridgeHandle> {
    let (handle, channels) = create_channel_pair();
    let agent = agent.to_string();

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
                        if let Err(e) = run_bridge(&agent, &cwd, channels).await {
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

async fn run_bridge(
    agent: &str,
    cwd: &std::path::Path,
    mut channels: BridgeChannels,
) -> crate::Result<()> {
    use agent_client_protocol as acp;
    use acp::Agent;
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

    use crate::protocol::client::KiroClient;
    use crate::protocol::transport::AgentProcess;

    // 1. Spawn agent process
    let process = AgentProcess::spawn(agent, cwd).await?;

    // 2. Create KiroClient
    let client = KiroClient::new(
        channels.notification_tx.clone(),
        channels.permission_tx.clone(),
    );

    // 3. Create the ACP connection.
    //    ClientSideConnection::new returns (conn, io_future).
    //    The io_future must be spawned on the LocalSet so the RPC layer runs.
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
        .client_info(acp::Implementation::new(
            "cyril",
            env!("CARGO_PKG_VERSION"),
        ))
        .client_capabilities(acp::ClientCapabilities::new());

    let _init_response: acp::InitializeResponse =
        conn.initialize(init_request).await.map_err(|e| {
            crate::Error::from_kind(crate::ErrorKind::Protocol {
                message: format!("ACP initialization failed: {e}"),
            })
        })?;

    tracing::info!("ACP bridge initialized");

    // 5. Command loop
    while let Some(cmd) = channels.command_rx.recv().await {
        match cmd {
            BridgeCommand::NewSession { cwd: session_cwd } => {
                let translated_cwd = crate::platform::path::to_agent(&session_cwd);
                match conn.new_session(acp::NewSessionRequest::new(translated_cwd)).await {
                    Ok(response) => {
                        let session_id = response.session_id.to_string();
                        let notification = Notification::SessionCreated {
                            session_id: crate::types::SessionId::new(session_id),
                        };
                        if channels.notification_tx.send(notification).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "new_session failed");
                    }
                }
            }
            BridgeCommand::SendPrompt { session_id, text } => {
                let acp_session_id = acp::SessionId::new(session_id.as_str());
                let prompt = vec![acp::ContentBlock::from(text)];
                let request = acp::PromptRequest::new(acp_session_id, prompt);
                match conn.prompt(request).await {
                    Ok(_) => {
                        if channels
                            .notification_tx
                            .send(Notification::TurnCompleted)
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "prompt failed");
                    }
                }
            }
            BridgeCommand::CancelRequest => {
                // Cancel requires a session_id which we don't have in this variant.
                // TODO: track active session and issue CancelNotification
                tracing::info!("cancel requested (not yet implemented)");
            }
            BridgeCommand::SetMode { mode_id } => {
                // TODO: implement when active session tracking is added
                tracing::info!(mode_id, "set_mode requested (not yet implemented)");
            }
            BridgeCommand::LoadSession { session_id } => {
                let acp_session_id = acp::SessionId::new(session_id.as_str());
                match conn
                    .load_session(acp::LoadSessionRequest::new(acp_session_id, cwd))
                    .await
                {
                    Ok(_) => {
                        tracing::info!(session_id = session_id.as_str(), "session loaded");
                    }
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            session_id = session_id.as_str(),
                            "load_session failed"
                        );
                    }
                }
            }
            BridgeCommand::ExtMethod { method, params } => {
                let raw = match serde_json::value::RawValue::from_string(
                    serde_json::to_string(&params).unwrap_or_else(|_| "null".to_string()),
                ) {
                    Ok(raw) => raw,
                    Err(e) => {
                        tracing::error!(error = %e, method, "failed to serialize ext params");
                        continue;
                    }
                };
                let raw_arc: Arc<serde_json::value::RawValue> = raw.into();
                match conn
                    .ext_method(acp::ExtRequest::new(&*method, raw_arc))
                    .await
                {
                    Ok(_response) => {}
                    Err(e) => {
                        tracing::error!(error = %e, method, "ext_method failed");
                    }
                }
            }
            BridgeCommand::ExtMethodWithResponse {
                method,
                params,
                response_tx,
            } => {
                let raw = match serde_json::value::RawValue::from_string(
                    serde_json::to_string(&params).unwrap_or_else(|_| "null".to_string()),
                ) {
                    Ok(raw) => raw,
                    Err(e) => {
                        let _ = response_tx.send(Err(crate::Error::from_kind(
                            crate::ErrorKind::Protocol {
                                message: format!("failed to serialize ext params: {e}"),
                            },
                        )));
                        continue;
                    }
                };
                let raw_arc: Arc<serde_json::value::RawValue> = raw.into();
                match conn
                    .ext_method(acp::ExtRequest::new(&*method, raw_arc))
                    .await
                {
                    Ok(response) => {
                        let value: serde_json::Value =
                            serde_json::from_str(response.0.get())
                                .unwrap_or(serde_json::Value::Null);
                        let _ = response_tx.send(Ok(value));
                    }
                    Err(e) => {
                        tracing::error!(error = %e, method, "ext_method failed");
                        let _ = response_tx.send(Err(crate::Error::from_kind(
                            crate::ErrorKind::Protocol {
                                message: format!("ext_method {method} failed: {e}"),
                            },
                        )));
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
        let notification = Notification::TurnCompleted;
        bridge_side.notification_tx.send(notification).await?;
        let received = handle.recv_notification().await;
        assert!(matches!(received, Some(Notification::TurnCompleted)));
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
    async fn ext_method_with_response_roundtrip() -> anyhow::Result<()> {
        let (handle, mut bridge_side) = create_channel_pair();
        let sender = handle.sender();

        let (resp_tx, _resp_rx) = tokio::sync::oneshot::channel();
        sender
            .send(BridgeCommand::ExtMethodWithResponse {
                method: "kiro.dev/commands/options".into(),
                params: serde_json::json!({"command": "model"}),
                response_tx: resp_tx,
            })
            .await?;

        let cmd = bridge_side.command_rx.recv().await;
        assert!(matches!(
            cmd,
            Some(BridgeCommand::ExtMethodWithResponse { .. })
        ));
        Ok(())
    }

    #[tokio::test]
    async fn send_ext_method_with_response_returns_err_when_bridge_dead() {
        let (handle, _bridge_side) = create_channel_pair();
        let sender = handle.sender();
        drop(_bridge_side);

        let result = sender
            .send_ext_method_with_response(
                "kiro.dev/commands/options",
                serde_json::json!({"command": "model"}),
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn send_ext_method_with_response_returns_err_when_responder_dropped() {
        let (handle, mut bridge_side) = create_channel_pair();
        let sender = handle.sender();

        // Spawn a task that receives the command but drops the response_tx
        let join = tokio::spawn(async move {
            if let Some(BridgeCommand::ExtMethodWithResponse { response_tx, .. }) =
                bridge_side.command_rx.recv().await
            {
                drop(response_tx);
            }
        });

        let result = sender
            .send_ext_method_with_response(
                "kiro.dev/commands/options",
                serde_json::json!({"command": "model"}),
            )
            .await;

        join.await.unwrap();
        assert!(result.is_err());
    }
}
