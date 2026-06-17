use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::protocol::convert::session_created_from_response;
use crate::types::StopReason;
use crate::types::agent_command::AgentCommand;
use crate::types::event::{BridgeCommand, Notification, PermissionRequest, RoutedNotification};

/// Channel capacities
const COMMAND_CAPACITY: usize = 32;
const NOTIFICATION_CAPACITY: usize = 256;
const PERMISSION_CAPACITY: usize = 16;

/// User-facing notice when the backend lacks `_session/steer` (-32601). One copy
/// so the SteerSession and ClearSteering arms can't drift (ROADMAP K1a).
const STEERING_UNSUPPORTED_MSG: &str = "steering requires kiro-cli 2.7.0+";

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
/// On any fail-stop path (runtime construction failure, `run_bridge` returning
/// `Err`), a `Notification::BridgeDisconnected` is emitted before the thread
/// exits so the App receives a structured signal instead of a silent channel
/// close.
pub fn spawn_bridge(agent_command: AgentCommand, cwd: PathBuf) -> crate::Result<BridgeHandle> {
    let (handle, channels) = create_channel_pair();
    // Cloned before `channels` is moved into the thread so that fail-stop
    // paths can still emit a final disconnect notification.
    let disconnect_tx = channels.notification_tx.clone();

    std::thread::Builder::new()
        .name("acp-bridge".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();

            let exit_reason: Option<String> = match rt {
                Ok(rt) => {
                    let local = tokio::task::LocalSet::new();
                    local.block_on(&rt, async move {
                        match run_bridge(&agent_command, &cwd, channels).await {
                            Ok(()) => None,
                            Err(e) => {
                                tracing::error!(error = %e, "bridge terminated with error");
                                Some(e.to_string())
                            }
                        }
                    })
                }
                Err(e) => {
                    tracing::error!(error = %e, "failed to create bridge runtime");
                    Some(format!("failed to create bridge runtime: {e}"))
                }
            };

            if let Some(reason) = exit_reason {
                // Best-effort; the App may already have dropped its receiver.
                if let Err(send_err) = disconnect_tx.try_send(RoutedNotification {
                    session_id: None,
                    notification: Notification::BridgeDisconnected { reason },
                }) {
                    tracing::warn!(
                        error = %send_err,
                        "failed to deliver BridgeDisconnected notification on bridge exit"
                    );
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

/// Outcome of an error from a `_session/steer[/clear]` request (ROADMAP K1a).
#[derive(Debug, PartialEq, Eq)]
enum SteerErrorAction {
    /// First `-32601` for this session: mark unsupported and notify the user once.
    MarkAndNotify,
    /// `-32601` but the session was already marked: stay silent (emit-once).
    ///
    /// NOTE: unreachable from the live handlers — `should_skip_steer` already
    /// `continue`s before the request is sent for any session in the unsupported
    /// set, so `already_unsupported` is always `false` at the call site. The
    /// emit-once guarantee is enforced by that pre-send gate, not by this arm;
    /// the arm exists to keep the classifier total and unit-testable in isolation.
    AlreadyUnsupported,
    /// Any other error: surface as a generic bridge error; do NOT mark unsupported.
    BridgeError,
}

/// Pure classifier so the -32601 gate is unit-testable without a live backend.
/// Only `MethodNotFound` (-32601) means "steering absent"; any other code is a
/// transient/other failure that must NOT permanently disable steering.
fn steer_error_action(
    code: agent_client_protocol::ErrorCode,
    already_unsupported: bool,
) -> SteerErrorAction {
    match (
        code == agent_client_protocol::ErrorCode::MethodNotFound,
        already_unsupported,
    ) {
        (true, false) => SteerErrorAction::MarkAndNotify,
        (true, true) => SteerErrorAction::AlreadyUnsupported,
        (false, _) => SteerErrorAction::BridgeError,
    }
}

/// Pre-send gate for `_session/steer[/clear]`: skip (and emit nothing) when the
/// target session is already known-unsupported. The set is session-keyed, so a
/// `-32601` on one session must NEVER suppress steering on a different one — the
/// guard is per-session, not global. Pure, so that invariant is unit-testable.
fn should_skip_steer(
    unsupported: &std::collections::HashSet<crate::types::SessionId>,
    session_id: &crate::types::SessionId,
) -> bool {
    unsupported.contains(session_id)
}

/// Code-side ext-methods for queue steering (Kiro 2.7.0+). UNPREFIXED on
/// purpose: the ACP library prepends a single `_` (`format!("_{}")`), so the
/// wire shows `_session/steer[/clear]`. A leading `_` here would double it to
/// `__session/steer` (-32601) — the cyril-c1qe regression. cf. `session/spawn`.
/// Regression fence: `steer_methods_are_unprefixed`.
const STEER_EXT_METHOD: &str = "session/steer";
const STEER_CLEAR_EXT_METHOD: &str = "session/steer/clear";

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
    agent_command: &AgentCommand,
    cwd: &std::path::Path,
    channels: BridgeChannels,
) -> crate::Result<()> {
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

    run_loop(std::rc::Rc::new(conn), channels, cwd.to_path_buf()).await
}

/// Handshake + the single-consumer command loop, split out of `run_bridge` so
/// tests can drive it against an in-process fake agent (no `kiro-cli`
/// subprocess). `conn` is `Rc` so a prompt future can be driven off this loop
/// (cyril-84ca) without moving the connection out of the loop's reach.
async fn run_loop(
    conn: std::rc::Rc<agent_client_protocol::ClientSideConnection>,
    mut channels: BridgeChannels,
    cwd: std::path::PathBuf,
) -> crate::Result<()> {
    use acp::Agent;
    use agent_client_protocol as acp;

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
    // Sessions whose backend lacks `_session/steer` (-32601). Remembered so we
    // skip re-sending and surface the unsupported notice only once (ROADMAP K1a).
    // Per-session: an entry is evicted when its session id (re)enters via
    // NewSession/LoadSession, so a re-used id re-probes — mirroring
    // SessionController's reset of `steering_unsupported` on SessionCreated.
    let mut steering_unsupported: std::collections::HashSet<crate::types::SessionId> =
        std::collections::HashSet::new();
    // cyril-84ca: the in-flight turn's task — at most one. `is_finished()` makes
    // the "busy" check self-clearing, so a completed turn never blocks the next.
    let mut prompt_task: Option<tokio::task::JoinHandle<()>> = None;

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
                        // A (re)entered session re-probes steering: drop any stale
                        // unsupported mark so it can't silently swallow steers.
                        steering_unsupported.remove(&crate::types::SessionId::new(
                            response.session_id.to_string(),
                        ));
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
                // cyril-84ca: at most one turn in flight. A SendPrompt arriving
                // while a turn runs must NOT start a second conn.prompt() (two
                // concurrent turns on one session is undefined); reject it with a
                // BridgeError. `is_finished()` self-clears, so the next turn after
                // this one completes is allowed.
                if prompt_task.as_ref().is_some_and(|h| !h.is_finished()) {
                    if notify_or_closed(
                        &channels.notification_tx,
                        Notification::BridgeError {
                            operation: "prompt".into(),
                            message: "a turn is already in progress".into(),
                        },
                    )
                    .await
                    {
                        break;
                    }
                    continue;
                }
                let acp_session_id = acp::SessionId::new(session_id.as_str());
                let prompt: Vec<acp::ContentBlock> = content_blocks
                    .into_iter()
                    .map(acp::ContentBlock::from)
                    .collect();
                let request = acp::PromptRequest::new(acp_session_id, prompt);
                // cyril-84ca: drive the turn OFF this command loop so commands
                // queued mid-turn (steer/cancel/...) are processed instead of
                // blocking behind conn.prompt() for the whole turn. The spawned
                // task owns the turn's single terminal TurnCompleted (success or
                // transport error). The ACP connection multiplexes concurrent
                // requests (.cyril-84ca/findings.md), so a steer the loop issues
                // while this task is pending still crosses the wire.
                let turn_conn = conn.clone();
                let turn_tx = channels.notification_tx.clone();
                prompt_task = Some(tokio::task::spawn_local(async move {
                    let note = match turn_conn.prompt(request).await {
                        Ok(response) => Notification::TurnCompleted {
                            stop_reason: crate::protocol::convert::to_stop_reason(
                                response.stop_reason,
                            ),
                        },
                        Err(e) => {
                            tracing::error!(error = %e, "prompt failed");
                            // No PromptResponse on a failed turn; EndTurn frees the
                            // UI from "busy". App-gone is detected by the command
                            // loop's own recv() ending, so a failed send here only
                            // means the App already left.
                            Notification::TurnCompleted {
                                stop_reason: StopReason::EndTurn,
                            }
                        }
                    };
                    if let Err(e) = turn_tx.send(note.into()).await {
                        tracing::debug!(error = %e, "TurnCompleted send failed (App gone)");
                    }
                }));
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
                    .load_session(acp::LoadSessionRequest::new(
                        acp_session_id.clone(),
                        cwd.clone(),
                    ))
                    .await
                {
                    Ok(response) => {
                        active_session_id = Some(acp_session_id);
                        // A reloaded session re-probes steering: a caller-supplied
                        // id may carry a stale unsupported mark from a prior life.
                        steering_unsupported.remove(&session_id);
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
                // Wire path is the bare ACP form (`_session/spawn` on the wire,
                // sans `_kiro.dev/` prefix). Verified empirically against 2.4.1:
                // `_kiro.dev/session/spawn` returns JSON-RPC -32601 method-not-found,
                // while bare `_session/spawn` accepts the request and spawns the
                // subagent. Note this is the OPPOSITE of `session/terminate`, which
                // requires the `kiro.dev/` prefix. See `docs/cyril-acp-coverage-vs-2.4.1.md`
                // "subagent wire probe" for the captured frames.
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
                    .ext_method(acp::ExtRequest::new("kiro.dev/session/terminate", raw_arc))
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
            BridgeCommand::ListSettings => {
                // Wire request takes empty `{}` params — non-empty hangs the
                // agent (verified empirically; see coverage doc). Singleton
                // request, no sessionId.
                let raw_arc = match to_raw_arc(&serde_json::json!({})) {
                    Ok(arc) => arc,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to serialize settings/list params");
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::BridgeError {
                                operation: "settings/list".into(),
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
                    .ext_method(acp::ExtRequest::new("kiro.dev/settings/list", raw_arc))
                    .await
                {
                    Ok(response) => match parse_response(&response.0) {
                        Ok(value) => {
                            if notify_or_closed(
                                &channels.notification_tx,
                                Notification::SettingsList { settings: value },
                            )
                            .await
                            {
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "failed to parse settings/list response");
                            if notify_or_closed(
                                &channels.notification_tx,
                                Notification::BridgeError {
                                    operation: "settings/list".into(),
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
                        tracing::error!(error = %e, "settings/list failed");
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::BridgeError {
                                operation: "settings/list".into(),
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
            BridgeCommand::SteerSession {
                session_id,
                message,
            } => {
                // Pre-send gate: a session known-unsupported is never re-sent and
                // emits nothing (the unsupported notice fired on the first -32601).
                if should_skip_steer(&steering_unsupported, &session_id) {
                    tracing::debug!(
                        session_id = session_id.as_str(),
                        "steering unsupported for session, skipping steer"
                    );
                    continue;
                }
                let params = serde_json::json!({
                    "sessionId": session_id.as_str(),
                    "message": message,
                });
                let raw_arc = match to_raw_arc(&params) {
                    Ok(arc) => arc,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to serialize steer params");
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::BridgeError {
                                operation: format!("steer '{}'", session_id.as_str()),
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
                // Ok({queued:true}) carries no new info: the `steering_queued`
                // echo is the source of truth, so success emits nothing here.
                // `STEER_EXT_METHOD` is unprefixed — ext_method adds the single
                // `_` (wire `_session/steer`); see the const's doc-comment.
                if let Err(e) = conn
                    .ext_method(acp::ExtRequest::new(STEER_EXT_METHOD, raw_arc))
                    .await
                {
                    // The real lookup (always false today: the pre-send gate
                    // above already skipped marked sessions) rather than a literal
                    // `false`, so emit-once stays correct if that gate is relaxed.
                    match steer_error_action(e.code, steering_unsupported.contains(&session_id)) {
                        SteerErrorAction::MarkAndNotify => {
                            steering_unsupported.insert(session_id.clone());
                            if notify_or_closed(
                                &channels.notification_tx,
                                Notification::SteeringUnsupported {
                                    message: STEERING_UNSUPPORTED_MSG.to_string(),
                                },
                            )
                            .await
                            {
                                break;
                            }
                        }
                        SteerErrorAction::AlreadyUnsupported => {}
                        SteerErrorAction::BridgeError => {
                            tracing::error!(error = %e, session_id = session_id.as_str(), "steer failed");
                            if notify_or_closed(
                                &channels.notification_tx,
                                Notification::BridgeError {
                                    operation: format!("steer '{}'", session_id.as_str()),
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
            }
            BridgeCommand::ClearSteering { session_id } => {
                if should_skip_steer(&steering_unsupported, &session_id) {
                    tracing::debug!(
                        session_id = session_id.as_str(),
                        "steering unsupported for session, skipping clear"
                    );
                    continue;
                }
                let params = serde_json::json!({ "sessionId": session_id.as_str() });
                let raw_arc = match to_raw_arc(&params) {
                    Ok(arc) => arc,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to serialize steer/clear params");
                        if notify_or_closed(
                            &channels.notification_tx,
                            Notification::BridgeError {
                                operation: format!("steer/clear '{}'", session_id.as_str()),
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
                // The `steering_cleared` echo is the source of truth on success.
                // Unprefixed: ext_method adds the single `_` (wire `_session/steer/clear`).
                if let Err(e) = conn
                    .ext_method(acp::ExtRequest::new(STEER_CLEAR_EXT_METHOD, raw_arc))
                    .await
                {
                    // Real lookup, not a literal `false` — see the SteerSession
                    // arm above: keeps emit-once correct if the pre-send gate changes.
                    match steer_error_action(e.code, steering_unsupported.contains(&session_id)) {
                        SteerErrorAction::MarkAndNotify => {
                            steering_unsupported.insert(session_id.clone());
                            if notify_or_closed(
                                &channels.notification_tx,
                                Notification::SteeringUnsupported {
                                    message: STEERING_UNSUPPORTED_MSG.to_string(),
                                },
                            )
                            .await
                            {
                                break;
                            }
                        }
                        SteerErrorAction::AlreadyUnsupported => {}
                        SteerErrorAction::BridgeError => {
                            tracing::error!(error = %e, session_id = session_id.as_str(), "steer/clear failed");
                            if notify_or_closed(
                                &channels.notification_tx,
                                Notification::BridgeError {
                                    operation: format!("steer/clear '{}'", session_id.as_str()),
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
            }
            BridgeCommand::Shutdown => {
                tracing::info!("bridge shutting down");
                // cyril-84ca: abort an in-flight turn so its task doesn't linger
                // past run_loop's return holding the connection. The loop being
                // free (Slice 3) is what lets Shutdown be processed mid-turn at all.
                if let Some(handle) = prompt_task.take() {
                    handle.abort();
                }
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

    // ── cyril-84ca mid-turn harness (Slices 2-7) ──────────────────────────────
    // In-process fake agent so the command loop is exercised with no kiro-cli
    // subprocess: ClientSideConnection(KiroClient) <-> AgentSideConnection(FakeAgent)
    // over a tokio duplex. Everything runs on one LocalSet (the ACP connections are
    // `!Send` and use `spawn_local`).
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::Duration;

    use agent_client_protocol as acp;
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

    use crate::protocol::client::KiroClient;

    #[derive(Default)]
    struct Script {
        /// Method names the agent received, in order (e.g. "new_session", "prompt").
        received: Vec<String>,
        prompt_count: usize,
        /// When set, `prompt` parks on the gate until the test releases it,
        /// modelling a long-running turn (so mid-turn commands can be observed).
        block_prompt: bool,
        /// When set, `prompt` returns an error (the transport/error turn path).
        prompt_err: bool,
        /// Set by `cancel`; makes a woken `prompt` resolve as Cancelled (ACP semantics).
        cancelled: bool,
    }

    struct FakeAgent {
        script: Rc<RefCell<Script>>,
        /// Released by the test (`Notify::notify_one`) to let a blocked `prompt` finish.
        gate: Rc<tokio::sync::Notify>,
        next_session: Cell<u32>,
    }

    #[async_trait::async_trait(?Send)]
    impl acp::Agent for FakeAgent {
        async fn initialize(
            &self,
            _a: acp::InitializeRequest,
        ) -> acp::Result<acp::InitializeResponse> {
            Ok(acp::InitializeResponse::new(acp::ProtocolVersion::V1))
        }
        async fn authenticate(
            &self,
            _a: acp::AuthenticateRequest,
        ) -> acp::Result<acp::AuthenticateResponse> {
            Ok(acp::AuthenticateResponse::new())
        }
        async fn new_session(
            &self,
            _a: acp::NewSessionRequest,
        ) -> acp::Result<acp::NewSessionResponse> {
            self.script.borrow_mut().received.push("new_session".into());
            let n = self.next_session.get();
            self.next_session.set(n + 1);
            Ok(acp::NewSessionResponse::new(acp::SessionId::new(format!(
                "fake-{n}"
            ))))
        }
        async fn prompt(&self, _a: acp::PromptRequest) -> acp::Result<acp::PromptResponse> {
            // Copy the flags out and DROP the borrow before any await — a RefCell
            // borrow held across `.await` would panic on re-entry.
            let (block, err) = {
                let mut s = self.script.borrow_mut();
                s.received.push("prompt".into());
                s.prompt_count += 1;
                (s.block_prompt, s.prompt_err)
            };
            if block {
                self.gate.notified().await;
            }
            // A cancel that woke us wins over the error/normal paths (ACP: a
            // cancelled turn resolves Cancelled).
            if self.script.borrow().cancelled {
                return Ok(acp::PromptResponse::new(acp::StopReason::Cancelled));
            }
            if err {
                return Err(acp::Error::new(-32603, "boom"));
            }
            Ok(acp::PromptResponse::new(acp::StopReason::EndTurn))
        }
        async fn cancel(&self, _a: acp::CancelNotification) -> acp::Result<()> {
            {
                let mut s = self.script.borrow_mut();
                s.received.push("cancel".into());
                s.cancelled = true;
            }
            // Wake a parked prompt so it resolves Cancelled (mirrors a real agent
            // ending the turn on session/cancel).
            self.gate.notify_one();
            Ok(())
        }
        async fn ext_method(&self, args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
            // Record by stripped method name (e.g. "ext:session/steer"); the ACP
            // library already stripped the leading `_` before dispatch.
            self.script
                .borrow_mut()
                .received
                .push(format!("ext:{}", args.method));
            Ok(acp::ExtResponse::new(
                to_raw_arc(&serde_json::json!({})).expect("serialize empty params"),
            ))
        }
    }

    /// Wire a fake agent to `run_loop` over an in-process duplex and run `body`
    /// against the live bridge.
    async fn with_harness<F, Fut>(script: Rc<RefCell<Script>>, body: F)
    where
        F: FnOnce(
            BridgeSender,
            mpsc::Receiver<RoutedNotification>,
            Rc<tokio::sync::Notify>,
            tokio::task::JoinHandle<crate::Result<()>>,
        ) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let gate = Rc::new(tokio::sync::Notify::new());
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async move {
                let (handle, channels) = create_channel_pair();
                let client = KiroClient::new(
                    channels.notification_tx.clone(),
                    channels.permission_tx.clone(),
                );
                let (c_io, a_io) = tokio::io::duplex(64 * 1024);
                let (cr, cw) = tokio::io::split(c_io);
                let (ar, aw) = tokio::io::split(a_io);
                let (conn, c_task) =
                    acp::ClientSideConnection::new(client, cw.compat_write(), cr.compat(), |f| {
                        tokio::task::spawn_local(f);
                    });
                let fake = FakeAgent {
                    script,
                    gate: gate.clone(),
                    next_session: Cell::new(0),
                };
                // Kept alive for the duration so its IO task can route requests to
                // FakeAgent (Slice 7 also uses it to push agent->client updates).
                let (_agent_conn, a_task) =
                    acp::AgentSideConnection::new(fake, aw.compat_write(), ar.compat(), |f| {
                        tokio::task::spawn_local(f);
                    });
                tokio::task::spawn_local(async move {
                    let _ = c_task.await;
                });
                tokio::task::spawn_local(async move {
                    let _ = a_task.await;
                });
                let loop_handle = tokio::task::spawn_local(run_loop(
                    Rc::new(conn),
                    channels,
                    std::env::temp_dir(),
                ));
                let (sender, notif_rx, _perm_rx) = handle.split();
                body(sender, notif_rx, gate, loop_handle).await;
            })
            .await;
    }

    /// Send NewSession and return the created session id.
    async fn start_session(
        sender: &BridgeSender,
        rx: &mut mpsc::Receiver<RoutedNotification>,
    ) -> crate::types::SessionId {
        sender
            .send(BridgeCommand::NewSession {
                cwd: std::env::temp_dir(),
            })
            .await
            .unwrap();
        recv_session_id(rx).await
    }

    /// One notification, or None on a `secs`-second timeout.
    async fn recv_notif(
        rx: &mut mpsc::Receiver<RoutedNotification>,
        secs: u64,
    ) -> Option<Notification> {
        match tokio::time::timeout(Duration::from_secs(secs), rx.recv()).await {
            Ok(Some(r)) => Some(r.notification),
            _ => None,
        }
    }

    /// Drain notifications until the first `TurnCompleted` and return its stop
    /// reason; panic on a 5s timeout (a missing completion is the bug we fence).
    async fn drain_to_turn(rx: &mut mpsc::Receiver<RoutedNotification>) -> StopReason {
        while let Some(n) = recv_notif(rx, 5).await {
            if let Notification::TurnCompleted { stop_reason } = n {
                return stop_reason;
            }
        }
        panic!("no TurnCompleted within 5s");
    }

    /// Poll the agent's `received` log for `marker` (e.g. "ext:session/steer"),
    /// yielding between checks; returns false after `secs`. Used to observe that a
    /// command reached the agent while a turn is still parked.
    async fn wait_for_received(probe: &Rc<RefCell<Script>>, marker: &str, secs: u64) -> bool {
        for _ in 0..(secs * 100) {
            if probe.borrow().received.iter().any(|m| m == marker) {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        false
    }

    async fn recv_session_id(
        rx: &mut mpsc::Receiver<RoutedNotification>,
    ) -> crate::types::SessionId {
        loop {
            let r = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                .await
                .expect("SessionCreated within 5s")
                .expect("notification channel open");
            if let Notification::SessionCreated { session_id, .. } = r.notification {
                return session_id;
            }
        }
    }

    /// Drain notifications until the first `TurnCompleted`; return how many were
    /// seen (expected: exactly 1). Bounded so a hang fails the test instead of stalling.
    async fn count_turn_completions(rx: &mut mpsc::Receiver<RoutedNotification>) -> usize {
        let mut n = 0;
        while let Ok(Some(r)) = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
            if matches!(r.notification, Notification::TurnCompleted { .. }) {
                n += 1;
                return n;
            }
        }
        n
    }

    #[tokio::test]
    async fn harness_drives_one_turn() {
        // Slice 2 baseline: the harness runs NewSession -> SendPrompt against the
        // in-process fake agent, observes exactly one TurnCompleted, and the agent
        // records the prompt it received (bidirectional delivery works).
        let script = Rc::new(RefCell::new(Script::default()));
        let probe = script.clone();
        with_harness(script, |sender, mut rx, _gate, _loop| async move {
            let sid = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id: sid,
                    content_blocks: vec!["hi".into()],
                })
                .await
                .unwrap();
            assert_eq!(
                count_turn_completions(&mut rx).await,
                1,
                "exactly one TurnCompleted"
            );
        })
        .await;
        let s = probe.borrow();
        assert!(
            s.received.contains(&"new_session".to_string()),
            "fake agent received new_session"
        );
        assert_eq!(s.prompt_count, 1, "fake agent received exactly one prompt");
    }

    #[tokio::test]
    async fn loop_frees_during_turn() {
        // C1: with the prompt parked, a ListSettings sent mid-turn is processed and
        // its result arrives BEFORE TurnCompleted — proving the loop returned to
        // recv() rather than blocking on conn.prompt(). The pre-fix inline await
        // would hold the loop, so nothing could surface until the gate releases
        // (and TurnCompleted would then be first). FakeAgent has no settings
        // ext_method, so the result is a BridgeError(operation="settings/list").
        let script = Rc::new(RefCell::new(Script {
            block_prompt: true,
            ..Default::default()
        }));
        with_harness(script, |sender, mut rx, gate, _loop| async move {
            let sid = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id: sid,
                    content_blocks: vec!["go".into()],
                })
                .await
                .unwrap();
            sender.send(BridgeCommand::ListSettings).await.unwrap();
            let first = recv_notif(&mut rx, 5)
                .await
                .expect("a mid-turn notification before turn end");
            // The ListSettings result — SettingsList on Ok, or BridgeError on a
            // settings error — arrives before any TurnCompleted (which can't fire
            // while the gate is held). Either proves the loop processed the command
            // mid-turn; only a leading TurnCompleted would mean the loop blocked.
            assert!(
                matches!(&first, Notification::SettingsList { .. })
                    || matches!(&first, Notification::BridgeError { operation, .. } if operation == "settings/list"),
                "expected mid-turn ListSettings result before TurnCompleted, got {first:?}"
            );
            gate.notify_one();
            assert_eq!(drain_to_turn(&mut rx).await, StopReason::EndTurn);
        })
        .await;
    }

    #[tokio::test]
    async fn turn_error_still_completes_once() {
        // C6: a turn whose prompt fails still emits exactly one TurnCompleted
        // (EndTurn), never zero — the off-loop task must notify on the Err path or
        // the UI stays stuck busy forever.
        let script = Rc::new(RefCell::new(Script {
            prompt_err: true,
            ..Default::default()
        }));
        with_harness(script, |sender, mut rx, _gate, _loop| async move {
            let sid = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id: sid,
                    content_blocks: vec!["go".into()],
                })
                .await
                .unwrap();
            assert_eq!(
                drain_to_turn(&mut rx).await,
                StopReason::EndTurn,
                "error turn completes as EndTurn"
            );
            assert!(
                !matches!(
                    recv_notif(&mut rx, 1).await,
                    Some(Notification::TurnCompleted { .. })
                ),
                "exactly one TurnCompleted (no second)"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn second_prompt_rejected_then_next_turn_starts() {
        // C4: a SendPrompt while a turn is in flight does NOT start a 2nd
        // conn.prompt(); it is rejected with a BridgeError and the agent sees only
        // one prompt for that turn. C9: once the turn finishes, the is_finished
        // guard self-clears, so a later SendPrompt starts a fresh turn.
        let script = Rc::new(RefCell::new(Script {
            block_prompt: true,
            ..Default::default()
        }));
        let probe = script.clone();
        with_harness(script, |sender, mut rx, gate, _loop| async move {
            let sid = start_session(&sender, &mut rx).await;
            // Turn 1 parks on the gate.
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id: sid.clone(),
                    content_blocks: vec!["one".into()],
                })
                .await
                .unwrap();
            // Turn 2 attempt while turn 1 is in flight -> rejected, not spawned.
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id: sid.clone(),
                    content_blocks: vec!["two".into()],
                })
                .await
                .unwrap();
            let n = recv_notif(&mut rx, 5).await.expect("a notification");
            assert!(
                matches!(&n, Notification::BridgeError { operation, .. } if operation == "prompt"),
                "second mid-turn prompt rejected with BridgeError, got {n:?}"
            );
            // Finish turn 1.
            gate.notify_one();
            assert_eq!(drain_to_turn(&mut rx).await, StopReason::EndTurn);
            assert_eq!(
                probe.borrow().prompt_count,
                1,
                "C4: the rejected prompt was never sent to the agent"
            );
            // C9: guard self-cleared -> a fresh SendPrompt starts turn 2.
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id: sid,
                    content_blocks: vec!["three".into()],
                })
                .await
                .unwrap();
            gate.notify_one();
            assert_eq!(drain_to_turn(&mut rx).await, StopReason::EndTurn);
            assert_eq!(
                probe.borrow().prompt_count,
                2,
                "C9: after the first turn completed, the next turn started"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn shutdown_aborts_inflight_prompt() {
        // C7: Shutdown received while a turn is parked aborts the turn task and
        // run_loop returns promptly. Pre-Slice-3 the loop blocked on conn.prompt(),
        // so Shutdown could not be processed until the turn ended; here run_loop
        // must return within the bound even though the prompt never completes.
        let script = Rc::new(RefCell::new(Script {
            block_prompt: true,
            ..Default::default()
        }));
        with_harness(script, |sender, mut rx, _gate, loop_handle| async move {
            let sid = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id: sid,
                    content_blocks: vec!["forever".into()],
                })
                .await
                .unwrap();
            // The turn parks (gate never released). Shutdown must still be processed.
            sender.send(BridgeCommand::Shutdown).await.unwrap();
            let returned = tokio::time::timeout(Duration::from_secs(2), loop_handle).await;
            assert!(
                matches!(returned, Ok(Ok(Ok(())))),
                "run_loop returned cleanly after a mid-turn Shutdown, got {returned:?}"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn steer_reaches_agent_mid_turn() {
        // C2 (headline): a SteerSession sent while the prompt is parked reaches the
        // agent (_session/steer) BEFORE the turn completes. Pre-Slice-3 the blocked
        // loop would not dequeue the steer until the turn ended.
        let script = Rc::new(RefCell::new(Script {
            block_prompt: true,
            ..Default::default()
        }));
        let probe = script.clone();
        with_harness(script, move |sender, mut rx, gate, _loop| async move {
            let sid = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id: sid.clone(),
                    content_blocks: vec!["go".into()],
                })
                .await
                .unwrap();
            sender
                .send(BridgeCommand::SteerSession {
                    session_id: sid,
                    message: "stop".into(),
                })
                .await
                .unwrap();
            assert!(
                wait_for_received(&probe, "ext:session/steer", 5).await,
                "steer reached the agent mid-turn; received = {:?}",
                probe.borrow().received
            );
            gate.notify_one();
            drain_to_turn(&mut rx).await;
        })
        .await;
    }

    #[tokio::test]
    async fn cancel_resolves_busy_turn() {
        // C3 (headline): a CancelRequest mid-turn reaches the agent (session/cancel)
        // and the parked prompt resolves to Cancelled — one TurnCompleted{Cancelled},
        // no hang. The blocked loop (pre-Slice-3) would never dequeue the cancel, so
        // drain_to_turn would time out.
        let script = Rc::new(RefCell::new(Script {
            block_prompt: true,
            ..Default::default()
        }));
        let probe = script.clone();
        with_harness(script, move |sender, mut rx, _gate, _loop| async move {
            let sid = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id: sid,
                    content_blocks: vec!["forever".into()],
                })
                .await
                .unwrap();
            sender.send(BridgeCommand::CancelRequest).await.unwrap();
            assert_eq!(
                drain_to_turn(&mut rx).await,
                StopReason::Cancelled,
                "cancel resolved the parked turn as Cancelled"
            );
            assert!(
                probe.borrow().received.contains(&"cancel".to_string()),
                "agent received the cancel mid-turn"
            );
        })
        .await;
    }

    // Slice DE / design claims 7,8,9 (+ backs claim 10's single-message via
    // AlreadyUnsupported). The fn compiling at all proves claim 7: -32601 is
    // detectable via agent_client_protocol::ErrorCode::MethodNotFound.
    #[test]
    fn steer_error_action_classifies() {
        use agent_client_protocol::ErrorCode;
        // claim 8: first -32601 -> mark + notify.
        assert_eq!(
            steer_error_action(ErrorCode::MethodNotFound, false),
            SteerErrorAction::MarkAndNotify
        );
        // emit-once: already marked -> silent (backs claim 10's single message).
        assert_eq!(
            steer_error_action(ErrorCode::MethodNotFound, true),
            SteerErrorAction::AlreadyUnsupported
        );
        // claim 9: any other code -> BridgeError, NEVER marks unsupported,
        // regardless of the flag (a transient -32603 must not disable steering).
        assert_eq!(
            steer_error_action(ErrorCode::InternalError, false),
            SteerErrorAction::BridgeError
        );
        assert_eq!(
            steer_error_action(ErrorCode::InternalError, true),
            SteerErrorAction::BridgeError
        );
    }

    // Slice 2b / pre-send gate. The unsupported set is session-keyed, so the
    // guard is per-session: a -32601 on one session must not suppress steering
    // on another. Three named fixtures, including "a different session still
    // steers" (the global-vs-per-session guard).
    #[test]
    fn should_skip_steer_is_per_session() {
        use crate::types::SessionId;
        let mut unsupported = std::collections::HashSet::new();
        let a = SessionId::new("sess_a");
        let b = SessionId::new("sess_b");

        // Empty set: nothing is skipped.
        assert!(!should_skip_steer(&unsupported, &a));

        unsupported.insert(a.clone());
        // The marked session is skipped...
        assert!(should_skip_steer(&unsupported, &a));
        // ...but a DIFFERENT session still steers (the guard is not global).
        assert!(!should_skip_steer(&unsupported, &b));
    }

    // Regression fence for cyril-c1qe (the PR's primary outbound fix). The ACP
    // library prepends exactly one `_` to ext methods (`format!("_{}")`,
    // agent-client-protocol lib.rs:213/221), so the code-side method MUST be
    // unprefixed for the wire to show a single leading `_`. K1a passed
    // `"_session/steer"`, which doubled to `__session/steer` -> -32601 and
    // killed steering silently. This is the outbound counterpart to
    // convert::kiro's `steering_rides_stripped_method_not_underscore`.
    #[test]
    fn steer_methods_are_unprefixed() {
        for m in [STEER_EXT_METHOD, STEER_CLEAR_EXT_METHOD] {
            assert!(
                !m.starts_with('_'),
                "steer ext method `{m}` must be unprefixed; the ACP lib adds the single `_`"
            );
            // Mirror the library's prepend: exactly one leading underscore, never two.
            let wire = format!("_{m}");
            assert!(
                !wire.starts_with("__"),
                "wire method `{wire}` must not be double-underscored (-32601)"
            );
        }
        // Pin the exact wire forms kiro 2.7.0+ accepts. Oracle: the post-fix
        // capture in .k1b-steering/fix-verification.log shows `_session/steer`
        // accepted (pre-fix `__session/steer` returned -32601).
        assert_eq!(format!("_{STEER_EXT_METHOD}"), "_session/steer");
        assert_eq!(format!("_{STEER_CLEAR_EXT_METHOD}"), "_session/steer/clear");
    }

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
