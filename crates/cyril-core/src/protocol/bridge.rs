use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::protocol::convert::session_created_from_response;
use crate::protocol::engine::{Engine, V2Engine};
use crate::types::StopReason;
use crate::types::agent_command::AgentCommand;
use crate::types::agent_engine::AgentEngine;
use crate::types::event::{BridgeCommand, Notification, PermissionRequest, RoutedNotification};
use crate::types::kas_spawn::KasSpawn;

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
pub fn spawn_bridge(
    agent_command: AgentCommand,
    agent_engine: AgentEngine,
    kas_spawn: KasSpawn,
    cwd: PathBuf,
) -> crate::Result<BridgeHandle> {
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

            let (rt, exit_reason): (Option<tokio::runtime::Runtime>, Option<String>) = match rt {
                Ok(rt) => {
                    let local = tokio::task::LocalSet::new();
                    let reason = local.block_on(&rt, async move {
                        match run_bridge(&agent_command, agent_engine, kas_spawn, &cwd, channels)
                            .await
                        {
                            Ok(()) => None,
                            Err(e) => {
                                tracing::error!(error = %e, "bridge terminated with error");
                                Some(e.to_string())
                            }
                        }
                    });
                    (Some(rt), reason)
                }
                Err(e) => {
                    tracing::error!(error = %e, "failed to create bridge runtime");
                    (None, Some(format!("failed to create bridge runtime: {e}")))
                }
            };

            if let Some(reason) = exit_reason {
                emit_failstop_disconnect(
                    rt.as_ref(),
                    &disconnect_tx,
                    reason,
                    FAILSTOP_SEND_TIMEOUT,
                );
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

/// How long the fail-stop [`emit_failstop_disconnect`] waits for a slot on a
/// full notification channel before giving up (cyril-l7tw C9/C10).
const FAILSTOP_SEND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Deliver the thread-exit `BridgeDisconnected` so it cannot be dropped by a
/// full channel (cyril-l7tw C9) — the pre-l7tw `try_send` lost it exactly
/// when a crash mid-streaming-backlog made it most likely. The send is
/// BOUNDED by `timeout`: a full channel with a live App delivers once the
/// App drains; a dropped receiver errors immediately; an App wedged past the
/// bound is warned about and abandoned rather than hanging the exiting
/// thread (C10). Without a runtime (its construction failed — nothing ever
/// ran, channel empty) falls back to best-effort `try_send`.
fn emit_failstop_disconnect(
    rt: Option<&tokio::runtime::Runtime>,
    tx: &mpsc::Sender<RoutedNotification>,
    reason: String,
    timeout: std::time::Duration,
) {
    let routed: RoutedNotification = Notification::BridgeDisconnected { reason }.into();
    match rt {
        Some(rt) => {
            match rt.block_on(async { tokio::time::timeout(timeout, tx.send(routed)).await }) {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    tracing::debug!(error = %e, "BridgeDisconnected receiver already gone on bridge exit");
                }
                Err(_) => {
                    tracing::warn!(
                        timeout_secs = timeout.as_secs_f32(),
                        "BridgeDisconnected not delivered within the bound (App not draining)"
                    );
                }
            }
        }
        None => {
            if let Err(e) = tx.try_send(routed) {
                tracing::warn!(error = %e, "failed to deliver BridgeDisconnected (no runtime)");
            }
        }
    }
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

/// Forward everything queued on the internal channel to the App, dropping
/// `TurnCompleted`s (cyril-l7tw). Called only from the death paths, where no
/// turn is in flight (idle death) or the turn's terminal marker was already
/// forwarded (deferred disconnect) — so every queued completion is a
/// duplicate, exactly as the live inbound arm would judge it. Calling this
/// while a turn is genuinely in flight would eat its completion; both call
/// sites structurally precede the loop's exit, where that can't hold.
async fn drain_inbound_dropping_duplicates(
    rx: &mut mpsc::Receiver<RoutedNotification>,
    tx: &mpsc::Sender<RoutedNotification>,
) {
    while let Ok(routed) = rx.try_recv() {
        if matches!(routed.notification, Notification::TurnCompleted { .. }) {
            continue;
        }
        if tx.send(routed).await.is_err() {
            break;
        }
    }
}

/// Enrich a bridge-fatal error with the agent's stderr tail (cyril-l7tw C7):
/// the failure the user sees (e.g. "ACP initialization failed") is rarely the
/// failure the agent reported (e.g. "You are not logged in, please log in
/// with kiro-cli login" — which only ever reaches stderr). Empty snapshot ⇒
/// the error passes through untouched (no dangling "agent stderr:" stub —
/// C8). The original error is kept as the source.
fn append_stderr_reason(e: crate::Error, snapshot: &[String]) -> crate::Error {
    let Some(tail) = tail_excerpt(snapshot) else {
        return e;
    };
    // Reuse a Protocol error's inner message so re-wrapping doesn't stack a
    // second "protocol error:" Display prefix (seen live in probe run 4).
    let base = match e.kind() {
        crate::ErrorKind::Protocol { message } => message.clone(),
        _ => e.to_string(),
    };
    crate::Error::with_source(
        crate::ErrorKind::Protocol {
            message: format!("{base}\nagent stderr:\n{tail}"),
        },
        e,
    )
}

/// How many trailing agent-stderr lines a user-facing disconnect reason
/// carries (cyril-l7tw). The full tail stays in the tracing log.
const REASON_TAIL_LINES: usize = 5;

/// Format the LAST [`REASON_TAIL_LINES`] of an agent stderr snapshot for a
/// user-facing disconnect reason; `None` when the agent wrote nothing (the
/// reason must not grow a dangling "agent stderr:" stub — l7tw C8).
fn tail_excerpt(snapshot: &[String]) -> Option<String> {
    if snapshot.is_empty() {
        return None;
    }
    let start = snapshot.len().saturating_sub(REASON_TAIL_LINES);
    Some(snapshot[start..].join("\n"))
}

/// Select the engine impl for the bound [`AgentEngine`], or a user-facing reason
/// it is unavailable. v2 is always available; `Kas` resolves to
/// [`crate::protocol::engine::KasEngine`] only under the `kas` cargo feature
/// (ADR-0002) — a default build reports that the feature is required rather than
/// linking any KAS code. Pure — unit-testable without a subprocess, and the
/// single place the engine-to-`AgentEngine` mapping lives.
fn engine_for(agent_engine: AgentEngine) -> Result<std::rc::Rc<dyn Engine>, String> {
    match agent_engine {
        AgentEngine::V2 => Ok(std::rc::Rc::new(V2Engine)),
        #[cfg(feature = "kas")]
        AgentEngine::Kas => Ok(std::rc::Rc::new(crate::protocol::engine::KasEngine)),
        #[cfg(not(feature = "kas"))]
        AgentEngine::Kas => Err("KAS engine requires a build with --features kas".to_string()),
    }
}

/// Resolve the subprocess command to spawn for the bound engine + KAS spawn
/// shape. KAS free path (Part A) discovers the bundled `node + acp-server.js`
/// argv; KAS wrapper (Part B) builds `kiro-cli acp --agent-engine <flag>`. Either
/// missing precondition becomes the actionable reason for a `BridgeDisconnected`
/// (spec B6). v2 spawns the CLI `agent_command`.
#[cfg(feature = "kas")]
fn resolve_spawn_command(
    agent_command: &AgentCommand,
    agent_engine: AgentEngine,
    kas_spawn: KasSpawn,
) -> Result<AgentCommand, String> {
    match agent_engine {
        AgentEngine::Kas => match kas_spawn {
            KasSpawn::Free => {
                crate::protocol::kas::discovery::resolve_kas_command().map_err(|m| m.reason())
            }
            KasSpawn::Wrapper => {
                crate::protocol::kas::version::build_wrapper_command(agent_command)
            }
        },
        AgentEngine::V2 => Ok(agent_command.clone()),
    }
}

/// Default build: only v2 is reachable here — the engine gate already refused
/// `Kas` before any spawn — so the engine/spawn selectors are irrelevant.
#[cfg(not(feature = "kas"))]
fn resolve_spawn_command(
    agent_command: &AgentCommand,
    _agent_engine: AgentEngine,
    _kas_spawn: KasSpawn,
) -> Result<AgentCommand, String> {
    Ok(agent_command.clone())
}

async fn run_bridge(
    agent_command: &AgentCommand,
    agent_engine: AgentEngine,
    kas_spawn: KasSpawn,
    cwd: &std::path::Path,
    channels: BridgeChannels,
) -> crate::Result<()> {
    use agent_client_protocol as acp;
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

    use crate::protocol::client::KiroClient;
    use crate::protocol::transport::AgentProcess;

    // 0. Engine gate (KAS-0, ADR-0001): bind the one engine the bridge uses for
    //    its life BEFORE spawning the subprocess, so an unavailable engine
    //    refuses cleanly (a disconnect notice, no panic) without spawning anything.
    let engine = match engine_for(agent_engine) {
        Ok(engine) => engine,
        Err(reason) => {
            notify_or_closed(
                &channels.notification_tx,
                Notification::BridgeDisconnected { reason },
            )
            .await;
            return Ok(());
        }
    };

    // 1. Resolve the spawn command, then spawn. The KAS free path (KAS-1 Part A)
    //    resolves the bundled node + acp-server.js argv via discovery; any missing
    //    precondition becomes a specific, actionable BridgeDisconnected reason
    //    (spec B6 — no auto-recover, no v2 fallback). v2 (and any default build)
    //    spawns the CLI `agent_command` unchanged. The clone is startup-only.
    let spawn_command = match resolve_spawn_command(agent_command, agent_engine, kas_spawn) {
        Ok(cmd) => cmd,
        Err(reason) => {
            notify_or_closed(
                &channels.notification_tx,
                Notification::BridgeDisconnected { reason },
            )
            .await;
            return Ok(());
        }
    };
    let process = AgentProcess::spawn(&spawn_command, cwd).await?;

    // 2. Create the KiroClient that dispatches conversion through the bound engine.
    // Internal notification channel (ADR-0004): the KiroClient and the off-loop
    // prompt task feed `inbound_tx`; `run_loop` drains `inbound_rx`, observes
    // turn-end, and forwards to the App's `channels.notification_tx`.
    let (inbound_tx, inbound_rx) = mpsc::channel::<RoutedNotification>(NOTIFICATION_CAPACITY);
    // Internal request channel (ADR-0004): server->client requests (permission
    // now; KAS-5/cyril-7bdu fs+terminal later) route through the loop, which
    // FORWARDS them to the App without awaiting resolution — the response flows
    // back on the request's embedded `responder` oneshot, bypassing the loop.
    let (req_tx, req_rx) = mpsc::channel::<PermissionRequest>(PERMISSION_CAPACITY);
    let client = KiroClient::new(inbound_tx.clone(), req_tx, engine.clone());

    // 3. Create the ACP connection.
    //    ClientSideConnection::new returns (conn, io_task).
    //    The io_task must be spawned on the LocalSet so the RPC layer runs.
    //    Grab the stderr tail handle first — stdin/stdout are moved out of
    //    `process` below (cyril-0gke). The tail reaches the user via the io
    //    watcher's disconnect reason and append_stderr_reason (cyril-l7tw).
    let stderr_tail = process.stderr_tail();
    // Second handle for the run_loop Err path below (the first moves into the
    // io watcher).
    let stderr_tail_for_err = process.stderr_tail();
    let (conn, io_task) = acp::ClientSideConnection::new(
        client,
        process.stdin.compat_write(),
        process.stdout.compat(),
        |fut| {
            tokio::task::spawn_local(fut);
        },
    );

    // Spawn the IO pump on the local task set, watched (cyril-l7tw C3/C4): the
    // pump ending — Ok on clean EOF (the common death mode per the l7tw probe)
    // or Err — means the agent connection is gone. The watcher tells run_loop
    // via a oneshot carrying a user-facing reason with the stderr tail, so
    // death while idle no longer needs a next command to become visible.
    let (io_done_tx, io_done_rx) = tokio::sync::oneshot::channel::<String>();
    tokio::task::spawn_local(async move {
        let io_result = io_task.await;
        let snapshot = stderr_tail.snapshot();
        let mut reason = String::from("agent connection closed unexpectedly");
        if let Err(e) = io_result {
            tracing::error!(error = %e, stderr_tail = ?snapshot, "ACP IO task failed");
            reason.push_str(&format!(" ({e})"));
        } else {
            tracing::error!(stderr_tail = ?snapshot, "ACP IO task ended (agent EOF)");
        }
        if let Some(tail) = tail_excerpt(&snapshot) {
            reason.push_str("\nagent stderr:\n");
            reason.push_str(&tail);
        }
        if io_done_tx.send(reason).is_err() {
            tracing::debug!("io-done signal dropped (run_loop already exited)");
        }
    });

    // cyril-l7tw C7: any error run_loop propagates (handshake failure being
    // the archetype) is enriched with the agent's stderr tail before it
    // becomes the fail-stop BridgeDisconnected reason — kiro-cli's actionable
    // "not logged in" text lives only on stderr. Spawn failures above never
    // reach here (no process ⇒ no tail to append — C8).
    run_loop(
        std::rc::Rc::new(conn),
        channels,
        cwd.to_path_buf(),
        engine,
        InternalChannels {
            inbound_tx,
            inbound_rx,
            req_rx,
            io_done: io_done_rx,
        },
    )
    .await
    .map_err(|e| append_stderr_reason(e, &stderr_tail_for_err.snapshot()))
}

/// The loop-internal plumbing (ADR-0004), grouped so `run_loop`'s signature
/// stays at one argument per concern:
/// - `inbound_tx`/`inbound_rx`: single-mediator notification channel — the
///   KiroClient and the off-loop prompt task feed `inbound_tx`; the loop
///   drains `inbound_rx`, observes `TurnCompleted` to clear the busy flag,
///   and forwards to the App.
/// - `req_rx`: server->client requests from the KiroClient; the loop forwards
///   them to the App's `permission_tx` and never awaits their resolution.
/// - `io_done`: fired by the io-pump watcher when the agent connection ends
///   (clean EOF or io error), carrying the user-facing disconnect reason
///   (cyril-l7tw).
struct InternalChannels {
    inbound_tx: mpsc::Sender<RoutedNotification>,
    inbound_rx: mpsc::Receiver<RoutedNotification>,
    req_rx: mpsc::Receiver<PermissionRequest>,
    io_done: tokio::sync::oneshot::Receiver<String>,
}

/// Handshake + the single-consumer command loop, split out of `run_bridge` so
/// tests can drive it against an in-process fake agent (no `kiro-cli`
/// subprocess). `conn` is `Rc` so a prompt future can be driven off this loop
/// (cyril-84ca) without moving the connection out of the loop's reach.
async fn run_loop(
    conn: std::rc::Rc<agent_client_protocol::ClientSideConnection>,
    mut channels: BridgeChannels,
    cwd: std::path::PathBuf,
    engine: std::rc::Rc<dyn Engine>,
    internal: InternalChannels,
) -> crate::Result<()> {
    let InternalChannels {
        inbound_tx,
        mut inbound_rx,
        mut req_rx,
        mut io_done,
    } = internal;
    use acp::Agent;
    use agent_client_protocol as acp;

    // 4. ACP handshake
    let init_request = acp::InitializeRequest::new(acp::ProtocolVersion::V1)
        .client_info(acp::Implementation::new("cyril", env!("CARGO_PKG_VERSION")))
        .client_capabilities(engine.client_capabilities());

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
    // cyril-atjw (KAS-0, ADR-0004): the in-flight turn's task, kept ONLY so
    // `Shutdown` can abort it (so it can't linger holding the connection past
    // run_loop's return). It is no longer the "is a turn running" signal.
    let mut prompt_task: Option<tokio::task::JoinHandle<()>> = None;
    // The session whose turn is in flight — at most one (ADR-0004). Set on
    // SendPrompt, cleared when the loop OBSERVES that turn's `TurnCompleted` on
    // the internal channel (engine-agnostic: v2 synthesizes it from the prompt
    // response, KAS from `session_info_update->turn_end`). Drives the busy-guard
    // and the CancelRequest target — a mid-turn NewSession can retarget
    // `active_session_id`, so cancel must use this, not that.
    //
    // NB (cyril-j16p / KAS-2a): under KAS the prompt task may outlive `turn_end`,
    // so this flag and `prompt_task` will intentionally diverge there; in v2 they
    // clear together (the prompt resolves AT turn-end). Do not re-merge them.
    let mut turn_in_flight: Option<acp::SessionId> = None;
    // cyril-l7tw C4: set when the io watcher reports the connection dead while
    // a turn is in flight. The disconnect is DEFERRED until the loop observes
    // that turn's TurnCompleted (the prompt task's Err arm delivers a
    // BridgeError + TurnCompleted once the rpc layer clears its pending
    // responses), so the App sees BridgeError → TurnCompleted →
    // BridgeDisconnected. Also the load-bearing guard that keeps the select
    // arm from re-polling the completed oneshot (which would panic).
    let mut conn_dead: Option<String> = None;

    loop {
        // Single mediator (ADR-0004): one `select!` services commands AND the
        // internal notification stream, so the loop stays free during a turn
        // (cyril-84ca) and OBSERVES every `TurnCompleted` to clear `turn_in_flight`.
        tokio::select! {
            cmd = channels.command_rx.recv() => {
                let Some(cmd) = cmd else { break }; // App dropped the command channel.
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
                // cyril-84ca / ADR-0004: at most one turn in flight. A SendPrompt
                // arriving while a turn runs must NOT start a second conn.prompt()
                // (two concurrent turns on one session is undefined); reject it with
                // a BridgeError. `turn_in_flight` clears when the loop observes this
                // turn's TurnCompleted, so the next turn is then allowed.
                if turn_in_flight.is_some() {
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
                let request = acp::PromptRequest::new(acp_session_id.clone(), prompt);
                // cyril-84ca: drive the turn OFF this command loop so commands
                // queued mid-turn (steer/cancel/...) are processed instead of
                // blocking behind conn.prompt() for the whole turn. The spawned
                // task owns the turn's single terminal TurnCompleted (success or
                // transport error). The ACP connection multiplexes concurrent
                // requests (.cyril-84ca/findings.md), so a steer the loop issues
                // while this task is pending still crosses the wire.
                let turn_conn = conn.clone();
                // ADR-0004: the synthesized TurnCompleted goes to the INTERNAL
                // channel, so the loop is the single observer that clears the flag.
                let turn_tx = inbound_tx.clone();
                let handle = tokio::task::spawn_local(async move {
                    // One TurnCompleted construction for both arms (success and
                    // transport error) so the terminal marker can't drift between
                    // them — e.g. when KAS-2a adds a turn id field to TurnCompleted.
                    let stop_reason = match turn_conn.prompt(request).await {
                        Ok(response) => crate::protocol::convert::to_stop_reason(response.stop_reason),
                        Err(e) => {
                            tracing::error!(error = %e, "prompt failed");
                            // cyril-l7tw C1: surface the failure to the App BEFORE
                            // the terminal marker (CLAUDE.md: bridge errors must
                            // notify the App — logging alone is invisible). Same
                            // task + channel as the TurnCompleted below, so the
                            // error-before-completion order is deterministic.
                            let err_note = Notification::BridgeError {
                                operation: "prompt".into(),
                                message: e.to_string(),
                            };
                            if let Err(send_err) = turn_tx.send(err_note.into()).await {
                                tracing::debug!(error = %send_err, "BridgeError send failed (App gone)");
                            }
                            // No PromptResponse on a failed turn; EndTurn frees the
                            // UI from "busy". App-gone is detected by the command
                            // loop's own recv() ending, so a failed send here only
                            // means the App already left.
                            StopReason::EndTurn
                        }
                    };
                    let note = Notification::TurnCompleted { stop_reason };
                    if let Err(e) = turn_tx.send(note.into()).await {
                        tracing::debug!(error = %e, "TurnCompleted send failed (App gone)");
                    }
                });
                turn_in_flight = Some(acp_session_id);
                prompt_task = Some(handle);
            }
            BridgeCommand::CancelRequest => {
                // cyril-84ca / ADR-0004: prefer the in-flight turn's own session.
                // The loop is free during a turn, so a mid-turn NewSession/LoadSession
                // can retarget `active_session_id`; cancel must still hit the running
                // turn. Fall back to `active_session_id` when no turn is in flight.
                let cancel_target = turn_in_flight.as_ref().or(active_session_id.as_ref());
                if let Some(session_id) = cancel_target {
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
                // free is what lets Shutdown be processed mid-turn at all.
                if let Some(handle) = prompt_task.take() {
                    handle.abort();
                }
                break;
            }
                } // match cmd
            } // select! command branch
            Some(routed) = inbound_rx.recv() => {
                // ADR-0004 single-mediator forward: observe TurnCompleted to clear
                // the busy flag, then forward to the App. The loop is the one place
                // turn-end is observed.
                //
                // KAS-2a (cyril-j16p) Slice 2 — idempotent completion: a KAS turn
                // emits BOTH a `session_info_update`->`turn_end` (converted to
                // TurnCompleted by the engine) AND a prompt response (synthesized
                // into TurnCompleted by the off-loop task), in either order; v2
                // emits exactly one. Clear and forward only the FIRST per turn —
                // drop a TurnCompleted that arrives when no turn is in flight, so
                // the App commits streaming/metering once and a non-returning
                // prompt response can't freeze the turn (turn_end completes it).
                // (Residual: a stale duplicate arriving after a NEW same-session
                // turn started would need per-turn identity — cyril-a71q.)
                let mut completed_turn = false;
                if matches!(routed.notification, Notification::TurnCompleted { .. }) {
                    if turn_in_flight.is_none() {
                        continue; // duplicate completion for an already-ended turn
                    }
                    turn_in_flight = None;
                    completed_turn = true;
                }
                if channels.notification_tx.send(routed).await.is_err() {
                    break; // App dropped the notification channel.
                }
                // cyril-l7tw C4: the connection died mid-turn and the deferred
                // disconnect waited for this turn's terminal marker. Forward
                // any straggling inbound notifications, then say goodbye and
                // exit — mirrors the idle-death path in the io_done arm.
                if completed_turn && let Some(reason) = conn_dead.take() {
                    drain_inbound_dropping_duplicates(&mut inbound_rx, &channels.notification_tx)
                        .await;
                    notify_or_closed(
                        &channels.notification_tx,
                        Notification::BridgeDisconnected { reason },
                    )
                    .await;
                    break;
                }
            }
            res = &mut io_done, if conn_dead.is_none() => {
                // cyril-l7tw C3/C4: the agent connection is gone. RecvError can
                // only mean the watcher was torn down abnormally (it always
                // sends before exiting) — treat it as death with a generic
                // reason rather than a silent arm.
                let reason = res.unwrap_or_else(|_| {
                    tracing::warn!("io watcher dropped without a reason");
                    "agent connection closed unexpectedly".into()
                });
                if turn_in_flight.is_some() {
                    // Defer: let the in-flight turn's BridgeError+TurnCompleted
                    // (already en route via the inbound channel) reach the App
                    // first; the inbound arm below emits the disconnect after
                    // observing the completion.
                    conn_dead = Some(reason);
                } else {
                    // Idle death: forward anything already queued (dropping
                    // duplicate completions — a parked prompt's Err resolution
                    // may have raced its TurnCompleted into the queue), then
                    // say goodbye and exit. Send failures don't matter — we
                    // are breaking either way.
                    drain_inbound_dropping_duplicates(&mut inbound_rx, &channels.notification_tx)
                        .await;
                    notify_or_closed(
                        &channels.notification_tx,
                        Notification::BridgeDisconnected { reason },
                    )
                    .await;
                    break;
                }
            }
            Some(req) = req_rx.recv() => {
                // ADR-0004 non-blocking forward: hand the server->client request to
                // the App and return immediately. The loop NEVER awaits the response
                // (a permission decision is a human action) — it travels back on the
                // request's embedded `responder` oneshot, bypassing the loop. v2 is
                // identity mediation; KAS-5 (cyril-7bdu) gates/transforms here.
                if channels.permission_tx.send(req).await.is_err() {
                    break; // App dropped the permission channel.
                }
            }
        } // select!
    } // loop

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

    // V2 always selects an engine — NO panic/unwrap.
    #[test]
    fn engine_for_v2_ok() {
        assert!(engine_for(AgentEngine::V2).is_ok(), "v2 selects an engine");
    }

    // l7tw C7 (unit form; the live form is the logged-out kiro-cli run in
    // .cyril-l7tw/findings.md): the reason a user sees carries the LAST lines
    // of agent stderr — where kiro-cli puts its actionable text. Stress: >5
    // lines must keep the last 5, not the first 5.
    #[test]
    fn handshake_failure_reason_includes_stderr_tail() {
        let snapshot: Vec<String> = (1..=7).map(|i| format!("line{i}")).collect();
        let e = crate::Error::from_kind(crate::ErrorKind::Protocol {
            message: "ACP initialization failed: boom".into(),
        });
        let enriched = append_stderr_reason(e, &snapshot).to_string();
        assert!(
            enriched.contains("ACP initialization failed: boom"),
            "original error text preserved, got: {enriched}"
        );
        assert!(
            enriched.contains("line7") && enriched.contains("line3"),
            "last {REASON_TAIL_LINES} stderr lines included, got: {enriched}"
        );
        assert!(
            !enriched.contains("line2"),
            "older lines beyond the excerpt are dropped, got: {enriched}"
        );
        assert!(
            !enriched.contains("protocol error: protocol error"),
            "re-wrapping must not stack Display prefixes (probe run 4), got: {enriched}"
        );
    }

    // l7tw C8 (unit half): an empty snapshot passes the error through
    // byte-identical — no dangling "agent stderr:" stub.
    #[test]
    fn empty_stderr_tail_leaves_reason_untouched() {
        let e = crate::Error::from_kind(crate::ErrorKind::Protocol {
            message: "ACP initialization failed: boom".into(),
        });
        let before = e.to_string();
        let after = append_stderr_reason(e, &[]).to_string();
        assert_eq!(before, after);
        assert!(!after.contains("agent stderr:"));
    }

    // tail_excerpt edge shapes (l7tw slice 5 stress fixture): empty lines are
    // preserved verbatim (they are real stderr output), empty snapshot is None.
    #[test]
    fn tail_excerpt_shapes() {
        assert_eq!(tail_excerpt(&[]), None);
        let mixed = vec![String::new(), "x".into(), String::new()];
        assert_eq!(tail_excerpt(&mixed).as_deref(), Some("\nx\n"));
    }

    // l7tw C9 fence — fails against the pre-l7tw `try_send` by construction
    // (mutation-verified: substituting try_send back fails the "survived"
    // assert): the channel is filled to capacity BEFORE the emission, and the
    // consumer only starts draining afterwards. `try_send` returns Full and
    // the disconnect vanishes; the bounded send waits for the drain and
    // delivers it LAST (after the backlog, in channel order).
    #[test]
    fn failstop_disconnect_survives_full_channel() {
        let (handle, channels) = create_channel_pair();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        for _ in 0..NOTIFICATION_CAPACITY {
            channels
                .notification_tx
                .try_send(
                    Notification::TurnCompleted {
                        stop_reason: StopReason::EndTurn,
                    }
                    .into(),
                )
                .unwrap();
        }
        assert!(
            channels
                .notification_tx
                .try_send(
                    Notification::TurnCompleted {
                        stop_reason: StopReason::EndTurn,
                    }
                    .into(),
                )
                .is_err(),
            "channel is verifiably full — the pre-l7tw try_send would drop here"
        );
        let (_sender, mut rx, _perm) = handle.split();
        let drainer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            let mut total = 0usize;
            let mut disconnects = 0usize;
            let mut last_was_disconnect = false;
            while let Some(r) = rx.blocking_recv() {
                total += 1;
                last_was_disconnect =
                    matches!(r.notification, Notification::BridgeDisconnected { .. });
                if last_was_disconnect {
                    disconnects += 1;
                }
            }
            (total, disconnects, last_was_disconnect)
        });
        emit_failstop_disconnect(
            Some(&rt),
            &channels.notification_tx,
            "the end".into(),
            Duration::from_secs(5),
        );
        drop(channels); // close the channel so the drainer's recv loop ends
        let (total, disconnects, last_was_disconnect) = drainer.join().expect("drainer");
        assert_eq!(disconnects, 1, "the disconnect survived the full channel");
        assert_eq!(
            total,
            NOTIFICATION_CAPACITY + 1,
            "backlog fully delivered too"
        );
        assert!(
            last_was_disconnect,
            "disconnect delivered after the backlog"
        );
    }

    // l7tw C10 fence: with a full channel and an App that NEVER drains, the
    // bounded send gives up at the timeout instead of hanging the exiting
    // bridge thread forever (an unbounded send would hang this test); and a
    // dropped receiver returns immediately.
    #[test]
    fn failstop_disconnect_no_hang_on_wedged_or_dropped_receiver() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        // Wedged App: full channel, receiver alive, nobody draining.
        let (handle, channels) = create_channel_pair();
        for _ in 0..NOTIFICATION_CAPACITY {
            channels
                .notification_tx
                .try_send(
                    Notification::TurnCompleted {
                        stop_reason: StopReason::EndTurn,
                    }
                    .into(),
                )
                .unwrap();
        }
        let start = std::time::Instant::now();
        emit_failstop_disconnect(
            Some(&rt),
            &channels.notification_tx,
            "wedged".into(),
            Duration::from_millis(200),
        );
        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_millis(200) && elapsed < Duration::from_secs(3),
            "gave up at the bound, got {elapsed:?}"
        );
        drop(handle);
        // Dropped receiver: returns immediately.
        let (handle2, channels2) = create_channel_pair();
        drop(handle2);
        let start = std::time::Instant::now();
        emit_failstop_disconnect(
            Some(&rt),
            &channels2.notification_tx,
            "gone".into(),
            Duration::from_secs(5),
        );
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "closed channel errors immediately"
        );
    }

    // l7tw C8 (integration half): a spawn failure (missing binary) still
    // yields a BridgeDisconnected through the REAL spawn_bridge fail-stop
    // path, and the reason is well-formed (no tail stub — the agent never
    // ran). Oracle: the OS refuses the exec; we only assert what the App
    // receives.
    #[tokio::test]
    async fn spawn_failure_disconnect_reason_wellformed() {
        let cmd = AgentCommand::try_from_argv(vec!["cyril-l7tw-no-such-binary".to_string()])
            .expect("argv");
        let handle = spawn_bridge(
            cmd,
            AgentEngine::default(),
            KasSpawn::default(),
            std::env::temp_dir(),
        )
        .expect("bridge thread spawns");
        let (_sender, mut rx, _perm) = handle.split();
        let routed = tokio::time::timeout(Duration::from_secs(10), rx.recv())
            .await
            .expect("notification within 10s of spawn failure")
            .expect("channel open");
        match routed.notification {
            Notification::BridgeDisconnected { reason } => {
                assert!(!reason.is_empty(), "reason must be actionable, not blank");
                assert!(
                    !reason.contains("agent stderr:"),
                    "no tail stub when the agent never ran, got: {reason}"
                );
            }
            other => panic!("expected BridgeDisconnected, got {other:?}"),
        }
    }

    // KAS-1 C4 (gate-on): under `--features kas`, Kas resolves to the KasEngine.
    #[cfg(feature = "kas")]
    #[test]
    fn engine_for_kas_ok_under_feature() {
        assert!(
            engine_for(AgentEngine::Kas).is_ok(),
            "Kas selects the KasEngine when built with --features kas"
        );
    }

    // KAS-1 C5 (gate-off): without the feature, Kas reports a clean reason naming
    // the feature — NO panic/unwrap, and the KAS code is not compiled in.
    #[cfg(not(feature = "kas"))]
    #[test]
    fn engine_for_kas_unavailable_without_feature() {
        match engine_for(AgentEngine::Kas) {
            Err(reason) => assert!(
                reason.contains("--features kas"),
                "Kas gives a clean reason naming the feature, got {reason:?}"
            ),
            Ok(_) => panic!("Kas must error without --features kas"),
        }
    }

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
        /// Number of `agent_message_chunk` notifications `prompt` streams before
        /// resolving (error or success) — models a turn that dies mid-stream.
        emit_chunks: usize,
        /// When set, `prompt` issues a server->client `request_permission` (which
        /// blocks until the client answers) before completing — exercises Slice 3's
        /// loop request-forward path (ADR-0004).
        request_perm: bool,
        /// When set, `prompt` emits a KAS `session_info_update`->`turn_end`
        /// notification before its (possibly parked) response — modelling KAS's
        /// dual completion signal so the loop's dedup (KAS-2a Slice 2) is exercised.
        emit_turn_end: bool,
        /// Set by `cancel`; makes a woken `prompt` resolve as Cancelled (ACP semantics).
        cancelled: bool,
        /// Session ids the agent was asked to cancel, in order. Lets a test assert
        /// WHICH session a CancelRequest targeted (cyril-84ca cancel-retarget fence).
        cancelled_sessions: Vec<String>,
    }

    struct FakeAgent {
        script: Rc<RefCell<Script>>,
        /// Released by the test (`Notify::notify_one`) to let a blocked `prompt` finish.
        gate: Rc<tokio::sync::Notify>,
        next_session: Cell<u32>,
        /// The agent's own side of the connection, populated AFTER construction so
        /// `prompt` can issue server->client requests (request_permission). The
        /// AgentSideConnection holds the FakeAgent, so this cell breaks the cycle.
        agent_conn: Rc<RefCell<Option<Rc<acp::AgentSideConnection>>>>,
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
        async fn prompt(&self, a: acp::PromptRequest) -> acp::Result<acp::PromptResponse> {
            // Copy the flags out and DROP the borrow before any await — a RefCell
            // borrow held across `.await` would panic on re-entry.
            let (block, err, want_perm, emit_turn_end, emit_chunks) = {
                let mut s = self.script.borrow_mut();
                s.received.push("prompt".into());
                s.prompt_count += 1;
                (
                    s.block_prompt,
                    s.prompt_err,
                    s.request_perm,
                    s.emit_turn_end,
                    s.emit_chunks,
                )
            };
            if emit_chunks > 0 {
                use acp::Client as _;
                let conn = self.agent_conn.borrow().clone();
                if let Some(conn) = conn {
                    for i in 0..emit_chunks {
                        let note: acp::SessionNotification =
                            serde_json::from_value(serde_json::json!({
                                "sessionId": a.session_id.to_string(),
                                "update": {
                                    "sessionUpdate": "agent_message_chunk",
                                    "content": { "type": "text", "text": format!("c{i}") }
                                }
                            }))
                            .expect("chunk notification");
                        conn.session_notification(note).await.expect("send chunk");
                    }
                }
            }
            if emit_turn_end {
                // KAS-shaped completion: emit the `turn_end` lifecycle frame
                // BEFORE the response (and before any park) so it drives
                // completion even when the prompt response is late or never comes.
                use acp::Client as _;
                // Drop the RefCell borrow before the await (mirrors the
                // request_permission path) — a Ref held across .await is unsound.
                let conn = self.agent_conn.borrow().clone();
                if let Some(conn) = conn {
                    let note: acp::SessionNotification = serde_json::from_value(serde_json::json!({
                        "sessionId": a.session_id.to_string(),
                        "update": {
                            "sessionUpdate": "session_info_update",
                            "_meta": { "kiro": { "kind": "turn_end", "stopReason": "end_turn" } }
                        }
                    }))
                    .expect("turn_end notification");
                    conn.session_notification(note).await?;
                }
            }
            if want_perm {
                // Server->client permission request. KiroClient sends it to the loop,
                // which FORWARDS it to the App (ADR-0004); this call BLOCKS until the
                // App answers via the embedded responder oneshot. The loop must keep
                // running while we wait here — that's what Slice 3 verifies.
                use acp::Client as _;
                let conn = self.agent_conn.borrow().clone();
                if let Some(conn) = conn {
                    let req = acp::RequestPermissionRequest::new(
                        a.session_id.clone(),
                        acp::ToolCallUpdate::new(
                            acp::ToolCallId::new("tc-perm"),
                            acp::ToolCallUpdateFields::new().title("run a command"),
                        ),
                        vec![acp::PermissionOption::new(
                            acp::PermissionOptionId::new("allow-once"),
                            "Allow once",
                            acp::PermissionOptionKind::AllowOnce,
                        )],
                    );
                    conn.request_permission(req).await?;
                }
            }
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
        async fn cancel(&self, a: acp::CancelNotification) -> acp::Result<()> {
            {
                let mut s = self.script.borrow_mut();
                s.received.push("cancel".into());
                s.cancelled = true;
                s.cancelled_sessions.push(a.session_id.to_string());
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

    /// Kill lever for the fake agent (cyril-l7tw): aborting the agent's io
    /// wrapper task drops the agent-side duplex halves, which the client
    /// observes as a clean EOF — the same mechanism as a SIGKILL'd real agent
    /// (probe finding: agent death is a clean EOF, not an io error).
    struct AgentKill {
        io_handle: tokio::task::JoinHandle<()>,
        conn_cell: Rc<RefCell<Option<Rc<acp::AgentSideConnection>>>>,
    }

    impl AgentKill {
        fn kill(self) {
            *self.conn_cell.borrow_mut() = None;
            self.io_handle.abort();
        }
    }

    /// Wire a fake agent to `run_loop` over an in-process duplex and run `body`
    /// against the live bridge, using the default v2 engine. Tests that need to
    /// kill the agent mid-flight use [`with_engine_harness`] directly for the
    /// [`AgentKill`] lever.
    async fn with_harness<F, Fut>(script: Rc<RefCell<Script>>, body: F)
    where
        F: FnOnce(
            BridgeSender,
            mpsc::Receiver<RoutedNotification>,
            mpsc::Receiver<PermissionRequest>,
            Rc<tokio::sync::Notify>,
            tokio::task::JoinHandle<crate::Result<()>>,
        ) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        with_engine_harness(Rc::new(V2Engine), script, |s, n, p, g, l, _kill| {
            body(s, n, p, g, l)
        })
        .await;
    }

    /// Like [`with_harness`] but with a caller-chosen [`Engine`], so KAS-2a tests
    /// can drive the loop with `KasEngine`.
    async fn with_engine_harness<F, Fut>(
        engine: Rc<dyn Engine>,
        script: Rc<RefCell<Script>>,
        body: F,
    ) where
        F: FnOnce(
            BridgeSender,
            mpsc::Receiver<RoutedNotification>,
            mpsc::Receiver<PermissionRequest>,
            Rc<tokio::sync::Notify>,
            tokio::task::JoinHandle<crate::Result<()>>,
            AgentKill,
        ) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let gate = Rc::new(tokio::sync::Notify::new());
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async move {
                let (handle, channels) = create_channel_pair();
                let (inbound_tx, inbound_rx) =
                    mpsc::channel::<RoutedNotification>(NOTIFICATION_CAPACITY);
                let (req_tx, req_rx) = mpsc::channel::<PermissionRequest>(PERMISSION_CAPACITY);
                let client = KiroClient::new(inbound_tx.clone(), req_tx, engine.clone());
                let (c_io, a_io) = tokio::io::duplex(64 * 1024);
                let (cr, cw) = tokio::io::split(c_io);
                let (ar, aw) = tokio::io::split(a_io);
                let (conn, c_task) =
                    acp::ClientSideConnection::new(client, cw.compat_write(), cr.compat(), |f| {
                        tokio::task::spawn_local(f);
                    });
                let agent_conn_cell: Rc<RefCell<Option<Rc<acp::AgentSideConnection>>>> =
                    Rc::new(RefCell::new(None));
                let fake = FakeAgent {
                    script,
                    gate: gate.clone(),
                    next_session: Cell::new(0),
                    agent_conn: agent_conn_cell.clone(),
                };
                // Kept alive (via the cell) for the duration so its IO task can route
                // requests, and shared INTO the FakeAgent so `prompt` can issue
                // server->client requests (request_permission).
                let (agent_conn, a_task) =
                    acp::AgentSideConnection::new(fake, aw.compat_write(), ar.compat(), |f| {
                        tokio::task::spawn_local(f);
                    });
                *agent_conn_cell.borrow_mut() = Some(Rc::new(agent_conn));
                // Mirror run_bridge's io watcher (cyril-l7tw): the client io
                // task ending = agent connection gone. No real process here,
                // so the reason carries no stderr tail.
                let (io_done_tx, io_done_rx) = tokio::sync::oneshot::channel::<String>();
                tokio::task::spawn_local(async move {
                    let _ = c_task.await;
                    let _ = io_done_tx.send("agent connection closed unexpectedly".into());
                });
                let a_handle = tokio::task::spawn_local(async move {
                    let _ = a_task.await;
                });
                let kill = AgentKill {
                    io_handle: a_handle,
                    conn_cell: agent_conn_cell.clone(),
                };
                let loop_handle = tokio::task::spawn_local(run_loop(
                    Rc::new(conn),
                    channels,
                    std::env::temp_dir(),
                    engine,
                    InternalChannels {
                        inbound_tx,
                        inbound_rx,
                        req_rx,
                        io_done: io_done_rx,
                    },
                ));
                let (sender, notif_rx, perm_rx) = handle.split();
                body(sender, notif_rx, perm_rx, gate, loop_handle, kill).await;
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

    // cyril-l7tw design falsifier (cheapest, run at design time): dropping the
    // agent side of the connection mid-prompt must (a) resolve the pending
    // `conn.prompt()` as `Err` and (b) complete the io task. This is the
    // mechanism the io-watcher fix and every death fence rely on — if EOF hung
    // the prompt or the io task, the design would be wrong. Kept as a permanent
    // mechanism fence (independent of the fix: it tests the acp rpc layer).
    #[tokio::test]
    async fn l7tw_agent_drop_resolves_prompt_err_and_completes_io() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (notif_tx, _notif_rx) =
                    mpsc::channel::<RoutedNotification>(NOTIFICATION_CAPACITY);
                let (req_tx, _req_rx) = mpsc::channel::<PermissionRequest>(PERMISSION_CAPACITY);
                let client = KiroClient::new(notif_tx, req_tx, Rc::new(V2Engine));
                let (c_io, a_io) = tokio::io::duplex(64 * 1024);
                let (cr, cw) = tokio::io::split(c_io);
                let (conn, io_task) =
                    acp::ClientSideConnection::new(client, cw.compat_write(), cr.compat(), |f| {
                        tokio::task::spawn_local(f);
                    });
                let io_handle = tokio::task::spawn_local(io_task);
                use acp::Agent;
                let prompt_fut = conn.prompt(acp::PromptRequest::new(
                    acp::SessionId::new("s1"),
                    vec![acp::ContentBlock::from("hello".to_string())],
                ));
                let killer = async move {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    drop(a_io); // agent dies: clean EOF on the client's read side
                };
                let (prompt_result, ()) = tokio::join!(prompt_fut, killer);
                assert!(
                    prompt_result.is_err(),
                    "pending prompt must resolve Err when the agent connection \
                     drops, got {prompt_result:?}"
                );
                let io_result = tokio::time::timeout(Duration::from_secs(5), io_handle)
                    .await
                    .expect("io task must complete after agent drop");
                // Audit trail: probe run2 says agent death is a CLEAN EOF.
                println!("l7tw falsifier: io task completed with {io_result:?}");
            })
            .await;
    }

    #[tokio::test]
    async fn harness_drives_one_turn() {
        // Slice 2 baseline: the harness runs NewSession -> SendPrompt against the
        // in-process fake agent, observes exactly one TurnCompleted, and the agent
        // records the prompt it received (bidirectional delivery works).
        let script = Rc::new(RefCell::new(Script::default()));
        let probe = script.clone();
        with_harness(
            script,
            |sender, mut rx, _perm_rx, _gate, _loop| async move {
                let sid = start_session(&sender, &mut rx).await;
                sender
                    .send(BridgeCommand::SendPrompt {
                        session_id: sid,
                        content_blocks: vec!["hi".into()],
                    })
                    .await
                    .unwrap();
                let mut completions = 0;
                while let Some(n) = recv_notif(&mut rx, 5).await {
                    // l7tw C12: a successful turn emits zero BridgeError noise.
                    assert!(
                        !matches!(n, Notification::BridgeError { .. }),
                        "successful turn must not emit BridgeError"
                    );
                    if matches!(n, Notification::TurnCompleted { .. }) {
                        completions += 1;
                        break;
                    }
                }
                assert_eq!(completions, 1, "exactly one TurnCompleted");
            },
        )
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
        with_harness(script, |sender, mut rx, _perm_rx, gate, _loop| async move {
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

    // l7tw C1/C2 fence + stress fixture: the agent streams 2 chunks then fails
    // the prompt. The App must receive the streamed chunks, a
    // BridgeError("prompt", <agent's error text>) and exactly ONE
    // TurnCompleted(EndTurn), with the BridgeError strictly before the
    // completion (both are sent sequentially by the prompt task on the same
    // channel, so this order is deterministic). Chunk-vs-completion order is
    // NOT asserted — that's the open notification-vs-response race
    // (cyril-9akh), out of this fence's scope. Catches: silent Err arm
    // (pre-l7tw behavior), error replacing the completion (busy sticks), and
    // error-message loss.
    #[tokio::test]
    async fn prompt_error_emits_bridge_error_before_completion() {
        let script = Rc::new(RefCell::new(Script {
            prompt_err: true,
            emit_chunks: 2,
            ..Default::default()
        }));
        with_harness(
            script,
            |sender, mut rx, _perm_rx, _gate, _loop| async move {
                let sid = start_session(&sender, &mut rx).await;
                sender
                    .send(BridgeCommand::SendPrompt {
                        session_id: sid,
                        content_blocks: vec!["go".into()],
                    })
                    .await
                    .unwrap();
                let mut chunks = 0;
                let mut bridge_error_seen = false;
                let mut completions = 0;
                loop {
                    match recv_notif(&mut rx, 5).await {
                        Some(Notification::AgentMessage(_)) => chunks += 1,
                        Some(Notification::BridgeError { operation, message }) => {
                            assert_eq!(operation, "prompt", "operation names the failed command");
                            assert!(
                                message.contains("boom"),
                                "agent's error text passes through, got: {message}"
                            );
                            assert_eq!(completions, 0, "BridgeError must precede TurnCompleted");
                            bridge_error_seen = true;
                        }
                        Some(Notification::TurnCompleted { stop_reason }) => {
                            assert_eq!(stop_reason, StopReason::EndTurn);
                            completions += 1;
                            break;
                        }
                        Some(_) => {}
                        None => panic!("timed out before TurnCompleted"),
                    }
                }
                // Chunks may trail the completion (cyril-9akh notification-vs-
                // response race) — drain briefly and count them wherever they
                // land; the claim is delivery, not ordering.
                while let Some(n) = recv_notif(&mut rx, 1).await {
                    match n {
                        Notification::AgentMessage(_) => chunks += 1,
                        Notification::TurnCompleted { .. } => completions += 1,
                        _ => {}
                    }
                }
                assert!(bridge_error_seen, "failed turn must surface a BridgeError");
                assert_eq!(chunks, 2, "streamed chunks still reach the App");
                assert_eq!(completions, 1, "exactly one TurnCompleted");
            },
        )
        .await;
    }

    // l7tw C1/C5 via the REAL death mechanism (clean EOF — the probe-proven
    // common mode), complementing the scripted-error fence above: the agent is
    // killed while the prompt is PARKED, so the Err arm is reached via the rpc
    // layer clearing its pending responses on connection end, never via an
    // agent reply. Catches: "Err arm only fires when the agent responds",
    // and a kill lever that doesn't actually close the stream (test times out).
    #[tokio::test]
    async fn death_mid_turn_emits_bridge_error_before_turn_completed() {
        let script = Rc::new(RefCell::new(Script {
            block_prompt: true,
            ..Default::default()
        }));
        let probe = script.clone();
        with_engine_harness(
            Rc::new(V2Engine),
            script,
            |sender, mut rx, _perm_rx, _gate, _loop, kill| async move {
                let sid = start_session(&sender, &mut rx).await;
                sender
                    .send(BridgeCommand::SendPrompt {
                        session_id: sid,
                        content_blocks: vec!["go".into()],
                    })
                    .await
                    .unwrap();
                assert!(
                    wait_for_received(&probe, "prompt", 5).await,
                    "prompt reached the agent before the kill"
                );
                kill.kill();
                let mut saw_bridge_error = false;
                loop {
                    match recv_notif(&mut rx, 5).await {
                        Some(Notification::BridgeError { operation, message }) => {
                            assert_eq!(operation, "prompt");
                            assert!(!message.is_empty(), "error message must not be empty");
                            saw_bridge_error = true;
                        }
                        Some(Notification::TurnCompleted { stop_reason }) => {
                            assert_eq!(stop_reason, StopReason::EndTurn);
                            assert!(
                                saw_bridge_error,
                                "BridgeError must arrive before TurnCompleted on agent death"
                            );
                            break;
                        }
                        Some(_) => {}
                        None => panic!("no TurnCompleted within 5s of agent death"),
                    }
                }
            },
        )
        .await;
    }

    // l7tw C3 fence: agent death while IDLE (no turn in flight) emits a
    // BridgeDisconnected naming the closed connection, and run_loop exits.
    // Fails pre-l7tw by construction: the detached io pump gave the loop no
    // death signal, so this test would time out waiting for the notification.
    #[tokio::test]
    async fn death_while_idle_emits_disconnected_and_exits() {
        let script = Rc::new(RefCell::new(Script::default()));
        with_engine_harness(
            Rc::new(V2Engine),
            script,
            |sender, mut rx, _perm_rx, _gate, loop_handle, kill| async move {
                let _sid = start_session(&sender, &mut rx).await;
                kill.kill();
                let reason = loop {
                    match recv_notif(&mut rx, 5).await {
                        Some(Notification::BridgeDisconnected { reason }) => break reason,
                        Some(_) => {}
                        None => panic!("no BridgeDisconnected within 5s of idle agent death"),
                    }
                };
                assert!(
                    reason.contains("agent connection closed"),
                    "reason names the dead connection, got: {reason}"
                );
                let loop_result = tokio::time::timeout(Duration::from_secs(5), loop_handle)
                    .await
                    .expect("run_loop must exit after idle death");
                assert!(loop_result.is_ok(), "run_loop task completed cleanly");
            },
        )
        .await;
    }

    // l7tw C3 counter-fixture: a NORMAL Shutdown must emit ZERO
    // BridgeDisconnected — catches "the watcher fires on ordinary teardown
    // too" (Shutdown breaks the loop first; the watcher's later send hits a
    // dropped receiver and must stay silent).
    #[tokio::test]
    async fn shutdown_emits_no_disconnect() {
        let script = Rc::new(RefCell::new(Script::default()));
        with_harness(
            script,
            |sender, mut rx, _perm_rx, _gate, _loop| async move {
                let _sid = start_session(&sender, &mut rx).await;
                sender.send(BridgeCommand::Shutdown).await.unwrap();
                // Drain to channel close (loop exit drops the sender); nothing on
                // the way out may be a disconnect.
                while let Some(n) = recv_notif(&mut rx, 2).await {
                    assert!(
                        !matches!(n, Notification::BridgeDisconnected { .. }),
                        "normal shutdown must not report a disconnect"
                    );
                }
            },
        )
        .await;
    }

    // l7tw C4 fence: mid-turn death delivers the full explanation in order —
    // BridgeError (why the turn died) → TurnCompleted (busy clears) →
    // BridgeDisconnected (the bridge is gone) — and run_loop exits. Catches:
    // disconnect emitted before the turn's terminal marker (order inverted)
    // and a deferred disconnect that never fires (test times out).
    #[tokio::test]
    async fn death_mid_turn_disconnect_after_completion() {
        let script = Rc::new(RefCell::new(Script {
            block_prompt: true,
            ..Default::default()
        }));
        let probe = script.clone();
        with_engine_harness(
            Rc::new(V2Engine),
            script,
            |sender, mut rx, _perm_rx, _gate, loop_handle, kill| async move {
                let sid = start_session(&sender, &mut rx).await;
                sender
                    .send(BridgeCommand::SendPrompt {
                        session_id: sid,
                        content_blocks: vec!["go".into()],
                    })
                    .await
                    .unwrap();
                assert!(
                    wait_for_received(&probe, "prompt", 5).await,
                    "prompt reached the agent before the kill"
                );
                kill.kill();
                let mut order = Vec::new();
                loop {
                    match recv_notif(&mut rx, 5).await {
                        Some(Notification::BridgeError { .. }) => order.push("error"),
                        Some(Notification::TurnCompleted { .. }) => order.push("completed"),
                        Some(Notification::BridgeDisconnected { .. }) => {
                            order.push("disconnected");
                            break;
                        }
                        Some(_) => {}
                        None => panic!(
                            "no BridgeDisconnected within 5s of mid-turn death; saw {order:?}"
                        ),
                    }
                }
                assert_eq!(
                    order,
                    ["error", "completed", "disconnected"],
                    "mid-turn death tells the whole story in order"
                );
                let loop_result = tokio::time::timeout(Duration::from_secs(5), loop_handle)
                    .await
                    .expect("run_loop must exit after mid-turn death");
                assert!(loop_result.is_ok(), "run_loop task completed cleanly");
                // l7tw C6: after the disconnect the bridge accepts nothing —
                // a further SendPrompt errors at the sender instead of
                // vanishing into a silent TurnCompleted (the pre-l7tw world).
                let send_result = sender
                    .send(BridgeCommand::SendPrompt {
                        session_id: crate::types::SessionId::new("s-dead"),
                        content_blocks: vec!["hello?".into()],
                    })
                    .await;
                assert!(
                    send_result.is_err(),
                    "sends after disconnect must error, not silently vanish"
                );
            },
        )
        .await;
    }

    // l7tw C4/C13 adversarial fixture (the design's KAS wrinkle): a KAS-style
    // dual-completion turn whose `turn_end` lands BEFORE the agent dies. The
    // io watcher then finds no turn in flight (already cleared) and takes the
    // idle path; the prompt task's late duplicate TurnCompleted is dropped by
    // the existing dedup or the closed channel. Expected: exactly one
    // TurnCompleted, exactly one BridgeDisconnected — never two of either.
    #[cfg(feature = "kas")]
    #[tokio::test]
    async fn death_after_turn_end_single_disconnect() {
        let script = Rc::new(RefCell::new(Script {
            block_prompt: true,
            emit_turn_end: true,
            ..Default::default()
        }));
        let probe = script.clone();
        with_engine_harness(
            Rc::new(crate::protocol::engine::KasEngine),
            script,
            |sender, mut rx, _perm_rx, _gate, _loop, kill| async move {
                let sid = start_session(&sender, &mut rx).await;
                sender
                    .send(BridgeCommand::SendPrompt {
                        session_id: sid,
                        content_blocks: vec!["go".into()],
                    })
                    .await
                    .unwrap();
                assert!(
                    wait_for_received(&probe, "prompt", 5).await,
                    "prompt reached the agent before the kill"
                );
                // turn_end has been emitted by the fake agent before parking;
                // wait for its TurnCompleted so the dual-completion race is
                // settled before the kill.
                assert_eq!(drain_to_turn(&mut rx).await, StopReason::EndTurn);
                kill.kill();
                let mut completions = 0;
                let mut disconnects = 0;
                while let Some(n) = recv_notif(&mut rx, 2).await {
                    match n {
                        Notification::TurnCompleted { .. } => completions += 1,
                        Notification::BridgeDisconnected { .. } => disconnects += 1,
                        _ => {}
                    }
                }
                assert_eq!(
                    completions, 0,
                    "the dual turn completed exactly once (pre-kill)"
                );
                assert_eq!(
                    disconnects, 1,
                    "death after turn end disconnects exactly once"
                );
            },
        )
        .await;
    }

    // l7tw C2 on the death path: the killed turn ends with exactly ONE
    // TurnCompleted — the BridgeError adds visibility without disturbing the
    // single-terminal-marker invariant (cyril-a71q adjacent).
    #[tokio::test]
    async fn death_mid_turn_single_turn_completed() {
        let script = Rc::new(RefCell::new(Script {
            block_prompt: true,
            ..Default::default()
        }));
        let probe = script.clone();
        with_engine_harness(
            Rc::new(V2Engine),
            script,
            |sender, mut rx, _perm_rx, _gate, _loop, kill| async move {
                let sid = start_session(&sender, &mut rx).await;
                sender
                    .send(BridgeCommand::SendPrompt {
                        session_id: sid,
                        content_blocks: vec!["go".into()],
                    })
                    .await
                    .unwrap();
                assert!(
                    wait_for_received(&probe, "prompt", 5).await,
                    "prompt reached the agent before the kill"
                );
                kill.kill();
                let mut completions = 0;
                while let Some(n) = recv_notif(&mut rx, 2).await {
                    if matches!(n, Notification::TurnCompleted { .. }) {
                        completions += 1;
                    }
                }
                assert_eq!(completions, 1, "exactly one TurnCompleted after death");
            },
        )
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
        with_harness(
            script,
            |sender, mut rx, _perm_rx, _gate, _loop| async move {
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
            },
        )
        .await;
    }

    #[cfg(feature = "kas")]
    #[tokio::test]
    async fn kas_turn_end_and_prompt_response_dedupe_to_one() {
        // KAS-2a (cyril-j16p) Slice 2 — double-fire dedup: a KAS turn emits BOTH a
        // `turn_end` notification (-> TurnCompleted via KasEngine) AND a prompt
        // response (-> TurnCompleted via the off-loop task). The loop must forward
        // EXACTLY ONE and clear `turn_in_flight` once, so a follow-up SendPrompt is
        // accepted (not rejected "a turn is already in progress"). Designed to FAIL
        // if the duplicate is forwarded (double-commit) or the flag double-cleared.
        let script = Rc::new(RefCell::new(Script {
            emit_turn_end: true,
            ..Default::default()
        }));
        let probe = script.clone();
        with_engine_harness(
            Rc::new(crate::protocol::engine::KasEngine),
            script,
            |sender, mut rx, _perm_rx, _gate, _loop, _kill| async move {
                let sid = start_session(&sender, &mut rx).await;
                sender
                    .send(BridgeCommand::SendPrompt {
                        session_id: sid.clone(),
                        content_blocks: vec!["one".into()],
                    })
                    .await
                    .unwrap();
                assert_eq!(drain_to_turn(&mut rx).await, StopReason::EndTurn);
                assert!(
                    !matches!(
                        recv_notif(&mut rx, 1).await,
                        Some(Notification::TurnCompleted { .. })
                    ),
                    "exactly one TurnCompleted forwarded — the duplicate is dropped"
                );
                // turn_in_flight cleared once -> a fresh turn is accepted.
                sender
                    .send(BridgeCommand::SendPrompt {
                        session_id: sid,
                        content_blocks: vec!["two".into()],
                    })
                    .await
                    .unwrap();
                assert_eq!(
                    drain_to_turn(&mut rx).await,
                    StopReason::EndTurn,
                    "second turn accepted after the first deduped to one completion"
                );
            },
        )
        .await;
        assert_eq!(
            probe.borrow().prompt_count,
            2,
            "both turns reached the agent (the second was not rejected)"
        );
    }

    #[cfg(feature = "kas")]
    #[tokio::test]
    async fn kas_turn_end_completes_without_prompt_response() {
        // KAS-2a (cyril-j16p) Slice 2 — non-blocking: the prompt response never
        // returns (gate held forever), but the `turn_end` notification still drives
        // completion. Designed to FAIL if completion depended on the prompt response
        // (the skeleton would freeze busy).
        let script = Rc::new(RefCell::new(Script {
            emit_turn_end: true,
            block_prompt: true,
            ..Default::default()
        }));
        with_engine_harness(
            Rc::new(crate::protocol::engine::KasEngine),
            script,
            |sender, mut rx, _perm_rx, _gate, _loop, _kill| async move {
                let sid = start_session(&sender, &mut rx).await;
                sender
                    .send(BridgeCommand::SendPrompt {
                        session_id: sid,
                        content_blocks: vec!["go".into()],
                    })
                    .await
                    .unwrap();
                // The gate is NEVER released, so `prompt` parks indefinitely; only
                // the turn_end frame can complete the turn.
                assert_eq!(
                    drain_to_turn(&mut rx).await,
                    StopReason::EndTurn,
                    "turn_end completes the turn with no prompt response"
                );
            },
        )
        .await;
    }

    #[tokio::test]
    async fn permission_forwards_through_loop_without_blocking() {
        // Slice 3 (D5 / ADR-0004): a server->client permission request routes THROUGH
        // the loop to the App; the loop forwards it and NEVER awaits the decision (a
        // command sent while the permission is outstanding is still processed); and
        // the response round-trips to the agent (the turn completes). Designed to FAIL
        // if the loop awaited the responder — the mid-permission command would never
        // be processed and the turn would never complete (a freeze).
        let script = Rc::new(RefCell::new(Script {
            request_perm: true,
            ..Default::default()
        }));
        with_harness(
            script,
            |sender, mut rx, mut perm_rx, _gate, _loop| async move {
                let sid = start_session(&sender, &mut rx).await;
                sender
                    .send(BridgeCommand::SendPrompt {
                        session_id: sid,
                        content_blocks: vec!["go".into()],
                    })
                    .await
                    .unwrap();
                // The agent's prompt issues request_permission; the loop FORWARDS it
                // to the App. Receiving it here proves the request-forward path works.
                let req = tokio::time::timeout(Duration::from_secs(5), perm_rx.recv())
                    .await
                    .expect("permission forwarded within 5s")
                    .expect("a permission request");
                assert!(!req.options.is_empty(), "forwarded request keeps its options");
                // The permission is OUTSTANDING (unanswered). A command sent now must
                // still be processed — proving the loop did NOT block on the decision.
                sender.send(BridgeCommand::ListSettings).await.unwrap();
                let mid = recv_notif(&mut rx, 5)
                    .await
                    .expect("a command result while the permission is outstanding");
                assert!(
                    matches!(&mid, Notification::SettingsList { .. })
                        || matches!(&mid, Notification::BridgeError { operation, .. } if operation == "settings/list"),
                    "ListSettings processed while permission outstanding (loop not blocked), got {mid:?}"
                );
                // Answer the permission -> the agent's request_permission returns ->
                // the turn completes (the response round-tripped via the responder).
                let option_id = req
                    .options
                    .first()
                    .map(|o| o.id.clone())
                    .expect("scripted request has options");
                req.responder
                    .send(crate::types::event::PermissionResponse::Selected {
                        option_id,
                        trust_option: None,
                    })
                    .unwrap();
                assert_eq!(drain_to_turn(&mut rx).await, StopReason::EndTurn);
            },
        )
        .await;
    }

    #[tokio::test]
    async fn second_prompt_rejected_then_next_turn_starts() {
        // C4: a SendPrompt while a turn is in flight does NOT start a 2nd
        // conn.prompt(); it is rejected with a BridgeError and the agent sees only
        // one prompt for that turn. C9: once the turn's TurnCompleted is observed
        // on the internal channel, `turn_in_flight` clears (ADR-0004), so a later
        // SendPrompt starts a fresh turn.
        let script = Rc::new(RefCell::new(Script {
            block_prompt: true,
            ..Default::default()
        }));
        let probe = script.clone();
        with_harness(script, |sender, mut rx, _perm_rx, gate, _loop| async move {
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
            // C9: turn_in_flight cleared (TurnCompleted observed) -> fresh turn 2.
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
        with_harness(
            script,
            |sender, mut rx, _perm_rx, _gate, loop_handle| async move {
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
            },
        )
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
        with_harness(
            script,
            move |sender, mut rx, _perm_rx, gate, _loop| async move {
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
            },
        )
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
        with_harness(
            script,
            move |sender, mut rx, _perm_rx, _gate, _loop| async move {
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
            },
        )
        .await;
    }

    #[tokio::test]
    async fn cancel_targets_inflight_turn_after_midturn_new_session() {
        // cyril-84ca cancel-retarget fence: the command loop is now free during a
        // turn, so a mid-turn NewSession retargets `active_session_id` to the new
        // session (S2) while the in-flight turn still runs on S1. A subsequent
        // CancelRequest must cancel S1 (the running turn), NOT S2 — otherwise Esc
        // can't stop the busy turn. The buggy code (cancel `active_session_id`)
        // sends session/cancel for S2; this fence asserts the wire-targeted id is S1.
        let script = Rc::new(RefCell::new(Script {
            block_prompt: true,
            ..Default::default()
        }));
        let probe = script.clone();
        with_harness(
            script,
            move |sender, mut rx, _perm_rx, _gate, _loop| async move {
                let s1 = start_session(&sender, &mut rx).await;
                sender
                    .send(BridgeCommand::SendPrompt {
                        session_id: s1.clone(),
                        content_blocks: vec!["forever".into()],
                    })
                    .await
                    .unwrap();
                // Mid-turn NewSession -> S2 becomes `active_session_id` while S1 runs.
                let s2 = start_session(&sender, &mut rx).await;
                assert_ne!(s1.as_str(), s2.as_str(), "second session is distinct");
                // Cancel must resolve S1's parked turn, not the freshly-created S2.
                sender.send(BridgeCommand::CancelRequest).await.unwrap();
                assert_eq!(
                    drain_to_turn(&mut rx).await,
                    StopReason::Cancelled,
                    "the in-flight S1 turn resolved Cancelled"
                );
                let cancelled = probe.borrow().cancelled_sessions.clone();
                assert!(
                    cancelled.contains(&s1.as_str().to_string()),
                    "cancel targeted the in-flight turn's session S1; got {cancelled:?}"
                );
                assert!(
                    !cancelled.contains(&s2.as_str().to_string()),
                    "cancel did NOT target the mid-turn-created session S2; got {cancelled:?}"
                );
            },
        )
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
