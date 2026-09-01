use std::cell::RefCell;
use std::collections::HashSet;
use std::future::Future;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use agent_client_protocol::schema::v1 as acp;
use agent_client_protocol::{Agent, ConnectionTo, Responder, UntypedMessage};
use tokio::sync::{mpsc, oneshot};

use crate::protocol::bridge::BridgeChannels;
use crate::protocol::engine::Engine;
use crate::protocol::source_observer::{IngressTracker, SourceObserver};
use crate::protocol::turn_mediator::TurnMediator;
use crate::types::event::{Notification, PermissionRequest, RoutedNotification};
use crate::types::{PermissionResponse, SessionId};

pub(crate) mod commands;
pub(crate) mod host;
pub(crate) mod inbound;

const WORK_CAPACITY: usize = 256;
const HOST_CAPACITY: usize = 16;
const TRANSPORT_CLOSED_METHOD: &str = "_cyril.internal/transport_closed";

/// Work accepted by SDK handlers and consumed by the serial domain owner.
/// Every variant owns its payload and is `Send + 'static`; no Engine, source,
/// terminal, or host state crosses the SDK boundary.
#[derive(Debug)]
pub(crate) enum DomainWork {
    UnknownSessionUpdate(UntypedMessage),
    Session(acp::SessionNotification),
    ExtensionNotification(UntypedMessage),
    Permission {
        request: acp::RequestPermissionRequest,
        responder: Responder<acp::RequestPermissionResponse>,
    },
    TransportClosed,
    Routed(RoutedNotification),
    PromptTerminal {
        routed: RoutedNotification,
        source_disposition: crate::types::SourceTurnDisposition,
    },
    CommandOutcome(CommandOutcome),
}

/// The serial follow-up of a command RPC awaited on a spawned task.
///
/// Command RPCs must never be awaited inside the mediator select loop: the
/// SDK's incoming actor dispatches every frame — responses included — through
/// one serial chain whose notification handlers park on the bounded
/// [`WORK_CAPACITY`] channel. A loop that blocks on a response while that
/// channel is full deadlocks the bridge (a `session/load` replaying more than
/// 256 frames is enough). RPCs therefore send synchronously in
/// `handle_command` (preserving wire order), await on a spawned task, and
/// re-enter the loop as one of these variants for the `&mut self` follow-up.
#[derive(Debug)]
pub(crate) enum CommandOutcome {
    /// Deliver one notification to the App exactly as `notify` would have.
    Notify(Box<Notification>),
    /// A `session/new` or `session/load` RPC succeeded: bind the session.
    SessionStarted(Box<SessionStart>),
    /// A fail-stop condition: notify `BridgeDisconnected` and exit the loop.
    FatalDisconnect { reason: String },
    /// The agent answered `session/steer` with -32601: mark the session and
    /// tell the App once.
    SteeringUnsupported { session_id: SessionId },
}

impl CommandOutcome {
    pub(crate) fn notify(notification: Notification) -> Self {
        Self::Notify(Box::new(notification))
    }
}

/// Payload of [`CommandOutcome::SessionStarted`].
#[derive(Debug)]
pub(crate) struct SessionStart {
    pub(crate) session_id: SessionId,
    pub(crate) origin: crate::types::SessionOrigin,
    pub(crate) config_options: Option<Notification>,
    pub(crate) created: Notification,
}
#[derive(Debug)]
pub(crate) enum HostWork {
    ExtensionRequest {
        request: UntypedMessage,
        responder: Responder<serde_json::Value>,
    },
    #[cfg(feature = "kas")]
    Callback(crate::protocol::kas::callbacks::HostCallback),
    #[cfg(all(test, not(feature = "kas")))]
    Probe { index: usize, _padding: [u8; 288] },
}

/// Cloneable bounded ingress used by every SDK handler and the process adapter.
#[derive(Clone, Debug)]
pub(crate) struct DomainChannels {
    work_tx: mpsc::Sender<DomainWork>,
    host_tx: mpsc::Sender<HostWork>,
    transport_token: [u8; 16],
    ingress: IngressTracker,
}

impl DomainChannels {
    pub(crate) fn new(
        ingress: IngressTracker,
    ) -> crate::Result<(Self, mpsc::Receiver<DomainWork>, mpsc::Receiver<HostWork>)> {
        let mut transport_token = [0_u8; 16];
        getrandom::fill(&mut transport_token).map_err(|error| {
            crate::Error::with_source(
                crate::ErrorKind::Transport {
                    detail: "could not generate private transport EOF token".to_owned(),
                },
                error,
            )
        })?;
        let (work_tx, work_rx) = mpsc::channel(WORK_CAPACITY);
        let (host_tx, host_rx) = mpsc::channel(HOST_CAPACITY);
        Ok((
            Self {
                work_tx,
                host_tx,
                ingress,
                transport_token,
            },
            work_rx,
            host_rx,
        ))
    }

    pub(crate) async fn enqueue(&self, work: DomainWork) -> agent_client_protocol::Result<()> {
        self.work_tx.send(work).await.map_err(|_| {
            agent_client_protocol::Error::internal_error().data("domain mediator closed")
        })
    }

    pub(crate) async fn enqueue_host(&self, work: HostWork) -> agent_client_protocol::Result<()> {
        self.host_tx.send(work).await.map_err(|_| {
            agent_client_protocol::Error::internal_error().data("host mediator closed")
        })
    }

    /// Best-effort enqueue of a command RPC's serial follow-up. A closed work
    /// channel means the mediator is already gone; the outcome is moot.
    pub(crate) async fn enqueue_outcome(&self, outcome: CommandOutcome) {
        if let Err(error) = self.enqueue(DomainWork::CommandOutcome(outcome)).await {
            tracing::debug!(%error, "command outcome dropped after bridge closure");
        }
    }

    pub(crate) fn enter_ingress(&self) -> crate::protocol::source_observer::IngressGuard {
        self.ingress.enter()
    }
    pub(crate) fn transport_closed_line(&self) -> String {
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": TRANSPORT_CLOSED_METHOD,
            "params": {"token": self.transport_token},
        })
        .to_string()
    }

    pub(crate) fn is_transport_closed(&self, message: &UntypedMessage) -> bool {
        message.method() == TRANSPORT_CLOSED_METHOD
            && message.params.get("token") == Some(&serde_json::json!(self.transport_token))
    }

    #[cfg(all(test, not(feature = "kas")))]
    pub(crate) fn remaining_capacity(&self) -> usize {
        self.work_tx.capacity()
    }

    #[cfg(all(test, not(feature = "kas")))]
    pub(crate) async fn inject(
        &self,
        routed: RoutedNotification,
    ) -> agent_client_protocol::Result<()> {
        self.enqueue(DomainWork::Routed(routed)).await
    }
}

/// Configuration captured on the serial bridge thread. The `Rc` engine is
/// deliberately held here and never moved into an SDK handler task.
pub(crate) struct DomainConfig {
    pub(crate) engine: Rc<dyn Engine>,
    pub(crate) cwd: PathBuf,
    pub(crate) present_as: Option<crate::types::present_as::PresentAs>,
    pub(crate) stall_threshold: Duration,
    #[cfg(feature = "kas")]
    pub(crate) host_shell: crate::protocol::client::ResolvedHostShell,
}

pub(crate) struct DomainMediator {
    config: DomainConfig,
    bridge: BridgeChannels,
    work_rx: mpsc::Receiver<DomainWork>,
    host_rx: Option<mpsc::Receiver<HostWork>>,
    channels: DomainChannels,
    source_observer: SourceObserver,
    ingress: IngressTracker,
    active_session_id: Option<SessionId>,
    steering_unsupported: HashSet<SessionId>,
    turn_mediator: TurnMediator,
    tool_call_ledger: crate::protocol::tool_call_ledger::ToolCallLedger,
    turn_liveness: crate::protocol::turn_liveness::TurnLiveness,
    host_mediator: Rc<RefCell<crate::protocol::host_mediator::HostMediator>>,
    prompt_tasks: Vec<tokio::task::JoinHandle<()>>,
    permission_tasks: Vec<tokio::task::JoinHandle<()>>,
    command_tasks: Vec<tokio::task::JoinHandle<()>>,
    #[cfg(feature = "kas")]
    host_ctx: Rc<crate::protocol::kas::callbacks::DispatchCtx>,
}

impl DomainMediator {
    pub(crate) fn new(
        config: DomainConfig,
        bridge: BridgeChannels,
    ) -> crate::Result<(Self, DomainChannels)> {
        let ingress = IngressTracker::new();
        let (channels, work_rx, host_rx) = DomainChannels::new(ingress.clone())?;
        let source_observer = SourceObserver::new(bridge.source_tx.clone());
        #[cfg(feature = "kas")]
        let host_ctx = Rc::new(crate::protocol::kas::callbacks::DispatchCtx {
            notify_tx: channels.clone(),
            terminals: Rc::new(crate::protocol::kas::terminal_io::TerminalRegistry::new(
                config.host_shell.clone().map(Rc::new),
            )),
            hooks: match config.engine.adapters().hooks {
                crate::protocol::engine::HooksAdapter::Inbound => {
                    Some(Rc::new(crate::protocol::kas::hooks::HookRegistry::load(
                        &config.cwd,
                        crate::kiro_agent_config::home_dir()
                            .map(|home| home.join(".kiro"))
                            .as_deref(),
                    )))
                }
                _ => None,
            },
            hook_ops: crate::protocol::kas::hooks::HookOps::default(),
            cwd: config.cwd.clone(),
        });
        Ok((
            Self {
                config,
                bridge,
                work_rx,
                host_rx: Some(host_rx),
                channels: channels.clone(),
                source_observer,
                ingress,
                active_session_id: None,
                steering_unsupported: HashSet::new(),
                turn_mediator: TurnMediator::new(),
                tool_call_ledger: crate::protocol::tool_call_ledger::ToolCallLedger::new(),
                turn_liveness: crate::protocol::turn_liveness::TurnLiveness::new(),
                host_mediator: Rc::new(RefCell::new(
                    crate::protocol::host_mediator::HostMediator::new(),
                )),
                prompt_tasks: Vec::new(),
                permission_tasks: Vec::new(),
                command_tasks: Vec::new(),
                #[cfg(feature = "kas")]
                host_ctx,
            },
            channels,
        ))
    }
    pub(crate) async fn run(
        mut self,
        mut runtime: crate::protocol::sdk_runtime::SdkRuntimeHandle,
    ) -> crate::Result<()> {
        let connection = runtime.connection().clone();
        let Some(mut io_done) = runtime.take_done_rx() else {
            runtime.shutdown().await;
            return Err(crate::Error::from_kind(crate::ErrorKind::Transport {
                detail: "SDK runtime completion receiver unavailable".into(),
            }));
        };
        let Some(host_rx) = self.host_rx.take() else {
            runtime.shutdown().await;
            return Err(crate::Error::from_kind(crate::ErrorKind::Transport {
                detail: "host mediator receiver unavailable".into(),
            }));
        };
        let host_task = host::run(
            host_rx,
            Rc::clone(&self.config.engine),
            Rc::clone(&self.host_mediator),
            #[cfg(feature = "kas")]
            Rc::clone(&self.host_ctx),
        );
        if let Err(error) = self.initialize(&connection).await {
            let detail = runtime.disconnect_reason(error.to_string());
            let drain_result = self.drain_work_dropping_turn_completion().await;
            self.shutdown_runtime(runtime, host_task).await;
            drain_result?;
            return Err(crate::Error::from_kind(crate::ErrorKind::Transport {
                detail,
            }));
        }
        let mut deferred_disconnect = None;
        let mut stall_tick = tokio::time::interval(Duration::from_secs(5));
        stall_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let result: crate::Result<()> = async {
            loop {
            tokio::select! {
                command = self.bridge.command_rx.recv() => {
                    let Some(command) = command else { break Ok(()) };
                    if self.handle_command(&connection, command).await? {
                        break Ok(());
                    }
                }
                work = self.work_rx.recv() => {
                    let Some(work) = work else { break Ok(()) };
                    if matches!(&work, DomainWork::TransportClosed) {
                        runtime.request_shutdown();
                        let reason = runtime.disconnect_reason(
                            "agent connection closed unexpectedly".to_owned(),
                        );
                        if self.turn_mediator.is_busy() {
                            deferred_disconnect = Some(reason);
                        } else {
                            self.drain_work_dropping_turn_completion().await?;
                            self.notify(Notification::BridgeDisconnected { reason }.into())
                                .await?;
                            break Ok(());
                        }
                        continue;
                    }
                    let completed_turn = match self.handle_work(work).await? {
                        std::ops::ControlFlow::Break(()) => break Ok(()),
                        std::ops::ControlFlow::Continue(completed_turn) => completed_turn,
                    };
                    if completed_turn
                        && let Some(reason) = deferred_disconnect.take()
                    {
                        self.drain_work_dropping_turn_completion().await?;
                        self.notify(
                            Notification::BridgeDisconnected { reason }.into(),
                        )
                        .await?;
                        break Ok(());
                    }
                }
                _ = stall_tick.tick(), if self.turn_mediator.is_busy() => {
                    let now = now_std();
                    let (in_flight, host_transition) = {
                        let host = self.host_mediator.borrow();
                        (host.in_flight(), host.last_transition())
                    };
                    if let Some(quiet) = self.turn_liveness.check(
                        now,
                        in_flight,
                        host_transition,
                        self.config.stall_threshold,
                    ) {
                        let note = Notification::TurnStalled { quiet };
                        let routed = match self.turn_mediator.active_turn_session() {
                            Some(session_id) => RoutedNotification::scoped(session_id.clone(), note),
                            None => RoutedNotification::global(note),
                        };
                        self.notify(routed).await?;
                    }
                }
                completion = &mut io_done, if deferred_disconnect.is_none() => {
                    let reason = completion.unwrap_or_else(|_| {
                        tracing::warn!("SDK runtime completion sender dropped");
                        "agent connection closed unexpectedly".into()
                    });
                    let reason = runtime.disconnect_reason(reason);
                    if self.turn_mediator.is_busy() {
                        deferred_disconnect = Some(reason);
                    } else {
                        self.drain_work_dropping_turn_completion().await?;
                        self.notify(Notification::BridgeDisconnected { reason }.into()).await?;
                        break Ok(());
                    }
                }
            }
            }
        }
        .await;
        let result = match result {
            Err(error) if matches!(error.kind(), crate::ErrorKind::BridgeClosed) => Ok(()),
            other => other,
        };
        self.shutdown_runtime(runtime, host_task).await;
        result
    }
    async fn shutdown_runtime(
        &mut self,
        runtime: crate::protocol::sdk_runtime::SdkRuntimeHandle,
        host_task: tokio::task::JoinHandle<()>,
    ) {
        self.source_observer
            .finish(crate::types::SourceTurnDisposition::Abandoned);
        for task in self.prompt_tasks.drain(..) {
            task.abort();
        }
        for task in self.permission_tasks.drain(..) {
            task.abort();
        }
        for task in self.command_tasks.drain(..) {
            task.abort();
        }
        self.host_mediator.borrow_mut().shutdown();
        host_task.abort();
        runtime.shutdown().await;
    }

    async fn handle_work(
        &mut self,
        work: DomainWork,
    ) -> crate::Result<std::ops::ControlFlow<(), bool>> {
        use std::ops::ControlFlow;
        match work {
            DomainWork::UnknownSessionUpdate(message) => {
                tracing::debug!(
                    method = %message.method(),
                    "unknown session update retained by untyped fence"
                );
                Ok(ControlFlow::Continue(false))
            }
            DomainWork::Session(notification) => self
                .handle_session(notification)
                .await
                .map(ControlFlow::Continue),
            DomainWork::ExtensionNotification(notification) => self
                .handle_extension_notification(notification)
                .await
                .map(ControlFlow::Continue),
            DomainWork::TransportClosed => {
                tracing::debug!("duplicate transport-close marker ignored");
                Ok(ControlFlow::Continue(false))
            }
            DomainWork::Permission { request, responder } => {
                self.handle_permission(request, responder);
                Ok(ControlFlow::Continue(false))
            }
            DomainWork::Routed(routed) => {
                self.handle_routed(routed).await.map(ControlFlow::Continue)
            }
            DomainWork::PromptTerminal {
                routed,
                source_disposition,
            } => self
                .handle_routed_with_source_disposition(routed, Some(source_disposition))
                .await
                .map(ControlFlow::Continue),
            DomainWork::CommandOutcome(outcome) => self.apply_command_outcome(outcome).await,
        }
    }

    /// Apply the serial follow-up of a command RPC that completed on a
    /// spawned task. `Break` exits the mediator loop (fail-stop).
    async fn apply_command_outcome(
        &mut self,
        outcome: CommandOutcome,
    ) -> crate::Result<std::ops::ControlFlow<(), bool>> {
        use std::ops::ControlFlow;
        match outcome {
            CommandOutcome::Notify(notification) => {
                self.notify((*notification).into()).await?;
                Ok(ControlFlow::Continue(false))
            }
            CommandOutcome::SessionStarted(start) => {
                let SessionStart {
                    session_id,
                    origin,
                    config_options,
                    created,
                } = *start;
                self.publish_session_start(session_id, origin, config_options, created)
                    .await?;
                Ok(ControlFlow::Continue(false))
            }
            CommandOutcome::FatalDisconnect { reason } => {
                self.notify(Notification::BridgeDisconnected { reason }.into())
                    .await?;
                Ok(ControlFlow::Break(()))
            }
            CommandOutcome::SteeringUnsupported { session_id } => {
                if self.steering_unsupported.insert(session_id) {
                    self.notify(
                        Notification::SteeringUnsupported {
                            message: "steering requires kiro-cli 2.7.0+".into(),
                        }
                        .into(),
                    )
                    .await?;
                }
                Ok(ControlFlow::Continue(false))
            }
        }
    }

    /// Spawn one command RPC's await + outcome enqueue, tracked so shutdown
    /// aborts it. The RPC's request frame must already be on the wire (the
    /// synchronous `send_request` half) before this is called.
    fn spawn_command(&mut self, task: impl Future<Output = ()> + 'static) {
        let handle = tokio::task::spawn_local(task);
        self.command_tasks.retain(|task| !task.is_finished());
        self.command_tasks.push(handle);
    }

    async fn drain_work_dropping_turn_completion(&mut self) -> crate::Result<()> {
        while let Ok(work) = self.work_rx.try_recv() {
            if matches!(
                &work,
                DomainWork::TransportClosed
                    | DomainWork::PromptTerminal { .. }
                    | DomainWork::Routed(RoutedNotification {
                        notification: Notification::TurnCompleted { .. },
                        ..
                    })
            ) {
                continue;
            }
            if self.handle_work(work).await?.is_break() {
                break;
            }
        }
        Ok(())
    }

    async fn initialize(&self, connection: &ConnectionTo<Agent>) -> crate::Result<()> {
        let requested = self.config.present_as.unwrap_or_default();
        let effective =
            crate::protocol::identity::effective_present_as(self.config.engine.kind(), requested);
        if effective != requested {
            match self.config.present_as {
                Some(explicit) => tracing::warn!(
                    configured = explicit.wire_name(),
                    "[agent] present_as is inert on the v2 engine; presenting the honest identity"
                ),
                None => tracing::debug!(
                    configured = requested.wire_name(),
                    "default present_as is inert on the v2 engine"
                ),
            }
        }
        if let Some(advisory) =
            crate::protocol::identity::identity_advisory(self.config.engine.kind(), effective)
        {
            tracing::info!("{advisory}");
        }
        tracing::debug!(
            emits_wire_turn_end = self.config.engine.emits_wire_turn_end(),
            "engine terminal-source shape"
        );
        let request =
            acp::InitializeRequest::new(agent_client_protocol::schema::ProtocolVersion::V1)
                .client_info(crate::protocol::bridge::client_info(effective))
                .client_capabilities(crate::protocol::engine::client_capabilities(
                    self.config.engine.as_ref(),
                ));
        let response = connection
            .send_request(request)
            .block_task()
            .await
            .map_err(|error| {
                crate::Error::from_kind(crate::ErrorKind::Protocol {
                    message: format!("ACP initialization failed: {error}"),
                })
            })?;
        if let Some(reason) = crate::protocol::fingerprint::init_mismatch(
            self.config.engine.kind(),
            &response,
            cfg!(feature = "kas"),
        ) {
            return Err(crate::Error::from_kind(crate::ErrorKind::Protocol {
                message: reason,
            }));
        }
        Ok(())
    }

    fn handle_permission(
        &mut self,
        args: acp::RequestPermissionRequest,
        responder: Responder<acp::RequestPermissionResponse>,
    ) {
        let session_id = SessionId::new(args.session_id.to_string());
        let tool_call_id = crate::types::ToolCallId::new(args.tool_call.tool_call_id.to_string());
        let joinable = !session_id.as_str().is_empty() && !tool_call_id.as_str().is_empty();
        let tool_call = if !joinable {
            tracing::warn!(
                %session_id,
                %tool_call_id,
                "permission request has an empty sessionId or toolCallId; approval preview unavailable"
            );
            crate::protocol::convert::to_tool_call_from_permission(&args)
        } else if let Some(snapshot) = self.tool_call_ledger.snapshot(&session_id, &tool_call_id) {
            let mut from_request = crate::protocol::convert::to_tool_call_from_permission(&args);
            from_request.merge_update(&snapshot);
            from_request
        } else {
            tracing::warn!(
                %session_id,
                %tool_call_id,
                "permission request has no matching tracked tool call; approval preview unavailable"
            );
            crate::protocol::convert::to_tool_call_from_permission(&args)
        };
        let (response_tx, response_rx) = oneshot::channel::<PermissionResponse>();
        let request = PermissionRequest {
            session_id,
            tool_call,
            message: crate::protocol::convert::extract_permission_message(&args),
            options: crate::protocol::convert::to_permission_options(&args),
            trust_options: crate::protocol::convert::extract_trust_options(&args),
            responder: response_tx,
        };
        let permission_tx = self.bridge.permission_tx.clone();
        let task = tokio::task::spawn_local(async move {
            if permission_tx.send(request).await.is_err() {
                if responder
                    .respond_with_error(
                        agent_client_protocol::Error::internal_error()
                            .data("permission receiver dropped"),
                    )
                    .is_err()
                {
                    tracing::debug!("permission response receiver dropped");
                }
                return;
            }
            match response_rx.await {
                Ok(response) => {
                    let converted =
                        crate::protocol::convert::from_permission_response(response, &args);
                    if responder.respond(converted).is_err() {
                        tracing::debug!("permission responder dropped");
                    }
                }
                Err(error) => {
                    tracing::debug!(%error, "permission response channel closed");
                    if responder
                        .respond_with_error(
                            agent_client_protocol::Error::internal_error()
                                .data("permission response unavailable"),
                        )
                        .is_err()
                    {
                        tracing::debug!("permission responder dropped");
                    }
                }
            }
        });
        self.permission_tasks.retain(|task| !task.is_finished());
        self.permission_tasks.push(task);
    }

    async fn notify(&self, notification: RoutedNotification) -> crate::Result<()> {
        self.bridge
            .notification_tx
            .send(notification)
            .await
            .map_err(|_| crate::Error::from_kind(crate::ErrorKind::BridgeClosed))
    }
}

fn now_std() -> std::time::Instant {
    tokio::time::Instant::now().into_std()
}

fn canonical_extension_method(method: &str) -> &str {
    method.strip_prefix('_').unwrap_or(method)
}

fn current_timestamp_ms() -> Option<u64> {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => match u64::try_from(duration.as_millis()) {
            Ok(timestamp) => Some(timestamp),
            Err(error) => {
                tracing::warn!(%error, "system time does not fit in u64 milliseconds");
                None
            }
        },
        Err(error) => {
            tracing::warn!(%error, "system clock before Unix epoch");
            None
        }
    }
}

#[cfg(test)]
mod tests;
