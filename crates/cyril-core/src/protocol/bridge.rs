use std::path::PathBuf;

use tokio::sync::{mpsc, oneshot};

use crate::protocol::engine::{Engine, V2Engine};
use crate::types::agent_command::AgentCommand;
use crate::types::agent_engine::AgentEngine;
use crate::types::event::{BridgeCommand, Notification, PermissionRequest, RoutedNotification};
use crate::types::kas_hooks::KasHooksMode;
use crate::types::kas_spawn::KasSpawn;
use crate::types::present_as::PresentAs;

const COMMAND_CAPACITY: usize = 32;
const NOTIFICATION_CAPACITY: usize = 256;
const PERMISSION_CAPACITY: usize = 16;
const FAILSTOP_SEND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

pub(crate) const fn source_disposition(
    stop_reason: crate::types::StopReason,
) -> crate::types::SourceTurnDisposition {
    match stop_reason {
        crate::types::StopReason::EndTurn => crate::types::SourceTurnDisposition::Completed,
        crate::types::StopReason::Cancelled => crate::types::SourceTurnDisposition::Interrupted,
        crate::types::StopReason::MaxTokens
        | crate::types::StopReason::MaxTurnRequests
        | crate::types::StopReason::Refusal => crate::types::SourceTurnDisposition::Failed,
    }
}

pub struct BridgeHandle {
    command_tx: mpsc::Sender<BridgeCommand>,
    pub(crate) notification_rx: mpsc::Receiver<RoutedNotification>,
    pub(crate) permission_rx: mpsc::Receiver<PermissionRequest>,
    source_rx: mpsc::Receiver<crate::types::SourceTurnEvent>,
    completion_rx: oneshot::Receiver<()>,
}

impl BridgeHandle {
    pub async fn recv_notification(&mut self) -> Option<RoutedNotification> {
        self.notification_rx.recv().await
    }

    pub async fn recv_permission(&mut self) -> Option<PermissionRequest> {
        self.permission_rx.recv().await
    }

    pub fn sender(&self) -> BridgeSender {
        BridgeSender {
            command_tx: self.command_tx.clone(),
        }
    }

    #[doc(hidden)]
    pub fn for_tests() -> Self {
        Self::for_tests_with_command_rx().0
    }

    #[doc(hidden)]
    pub fn for_tests_with_command_rx() -> (Self, mpsc::Receiver<BridgeCommand>) {
        let (command_tx, command_rx) = mpsc::channel(COMMAND_CAPACITY);
        let (_notification_tx, notification_rx) = mpsc::channel(NOTIFICATION_CAPACITY);
        let (_permission_tx, permission_rx) = mpsc::channel(PERMISSION_CAPACITY);
        let (_source_tx, source_rx) =
            mpsc::channel(crate::types::source_turn::SOURCE_EVENT_CHANNEL_CAPACITY);
        let (_completion_tx, completion_rx) = oneshot::channel();
        (
            Self {
                command_tx,
                notification_rx,
                permission_rx,
                source_rx,
                completion_rx,
            },
            command_rx,
        )
    }

    pub fn split(
        self,
    ) -> (
        BridgeSender,
        mpsc::Receiver<RoutedNotification>,
        mpsc::Receiver<PermissionRequest>,
        mpsc::Receiver<crate::types::SourceTurnEvent>,
        oneshot::Receiver<()>,
    ) {
        (
            BridgeSender {
                command_tx: self.command_tx,
            },
            self.notification_rx,
            self.permission_rx,
            self.source_rx,
            self.completion_rx,
        )
    }
}

#[derive(Clone)]
pub struct BridgeSender {
    command_tx: mpsc::Sender<BridgeCommand>,
}

impl BridgeSender {
    pub fn from_sender(tx: mpsc::Sender<BridgeCommand>) -> Self {
        Self { command_tx: tx }
    }

    pub async fn send(&self, cmd: BridgeCommand) -> crate::Result<()> {
        self.command_tx
            .send(cmd)
            .await
            .map_err(|_| crate::Error::from_kind(crate::ErrorKind::BridgeClosed))
    }

    pub fn try_send(&self, cmd: BridgeCommand) -> crate::Result<()> {
        self.command_tx
            .try_send(cmd)
            .map_err(|_| crate::Error::from_kind(crate::ErrorKind::BridgeClosed))
    }
}

pub(crate) struct BridgeChannels {
    pub command_rx: mpsc::Receiver<BridgeCommand>,
    pub notification_tx: mpsc::Sender<RoutedNotification>,
    pub permission_tx: mpsc::Sender<PermissionRequest>,
    pub source_tx: mpsc::Sender<crate::types::SourceTurnEvent>,
    pub completion_tx: Option<oneshot::Sender<()>>,
}

pub(crate) fn create_channel_pair() -> (BridgeHandle, BridgeChannels) {
    let (command_tx, command_rx) = mpsc::channel(COMMAND_CAPACITY);
    let (notification_tx, notification_rx) = mpsc::channel(NOTIFICATION_CAPACITY);
    let (permission_tx, permission_rx) = mpsc::channel(PERMISSION_CAPACITY);
    let (source_tx, source_rx) =
        mpsc::channel(crate::types::source_turn::SOURCE_EVENT_CHANNEL_CAPACITY);
    let (completion_tx, completion_rx) = oneshot::channel();
    (
        BridgeHandle {
            command_tx,
            notification_rx,
            permission_rx,
            source_rx,
            completion_rx,
        },
        BridgeChannels {
            command_rx,
            notification_tx,
            permission_tx,
            source_tx,
            completion_tx: Some(completion_tx),
        },
    )
}

#[derive(Debug, Clone)]
pub struct SpawnConfig {
    pub engine: AgentEngine,
    pub kas_spawn: KasSpawn,
    pub shell: Option<String>,
    pub present_as: Option<PresentAs>,
    pub kas_hooks: KasHooksMode,
    pub stall_threshold: std::time::Duration,
}

pub const DEFAULT_STALL_THRESHOLD: std::time::Duration = std::time::Duration::from_secs(30);

impl Default for SpawnConfig {
    fn default() -> Self {
        Self {
            engine: AgentEngine::default(),
            kas_spawn: KasSpawn::default(),
            shell: None,
            present_as: None,
            kas_hooks: KasHooksMode::default(),
            stall_threshold: DEFAULT_STALL_THRESHOLD,
        }
    }
}

pub fn spawn_bridge(
    agent_command: AgentCommand,
    config: SpawnConfig,
    cwd: PathBuf,
) -> crate::Result<BridgeHandle> {
    let host_shell = resolve_host_shell(&config)?;
    let (handle, mut channels) = create_channel_pair();
    let disconnect_tx = channels.notification_tx.clone();
    let completion_tx = channels.completion_tx.take();
    std::thread::Builder::new()
        .name("acp-bridge".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            let (runtime, reason) = match runtime {
                Ok(runtime) => {
                    let local = tokio::task::LocalSet::new();
                    let reason = local.block_on(&runtime, async move {
                        match run_bridge(&agent_command, config, &cwd, channels, host_shell).await {
                            Ok(()) => None,
                            Err(error) => {
                                tracing::error!(%error, "bridge terminated with error");
                                Some(error.to_string())
                            }
                        }
                    });
                    (Some(runtime), reason)
                }
                Err(error) => (
                    None,
                    Some(format!("failed to create bridge runtime: {error}")),
                ),
            };
            if let Some(reason) = reason {
                emit_failstop_disconnect(runtime.as_ref(), &disconnect_tx, reason);
            }
            if let Some(completion_tx) = completion_tx
                && completion_tx.send(()).is_err()
            {
                tracing::debug!("bridge completion receiver dropped");
            }
        })
        .map_err(|error| {
            crate::Error::with_source(
                crate::ErrorKind::Transport {
                    detail: "failed to spawn bridge thread".into(),
                },
                error,
            )
        })?;
    Ok(handle)
}

fn emit_failstop_disconnect(
    runtime: Option<&tokio::runtime::Runtime>,
    tx: &mpsc::Sender<RoutedNotification>,
    reason: String,
) {
    let routed: RoutedNotification = Notification::BridgeDisconnected { reason }.into();
    if let Some(runtime) = runtime {
        match runtime
            .block_on(async { tokio::time::timeout(FAILSTOP_SEND_TIMEOUT, tx.send(routed)).await })
        {
            Ok(Ok(())) => {}
            Ok(Err(_)) => tracing::debug!("disconnect receiver dropped"),
            Err(_) => tracing::warn!("timed out delivering bridge disconnect"),
        }
    } else if tx.try_send(routed).is_err() {
        tracing::debug!("could not deliver bridge disconnect without runtime");
    }
}

fn engine_for(config: &SpawnConfig) -> Result<std::rc::Rc<dyn Engine>, String> {
    match config.engine {
        AgentEngine::V2 => Ok(std::rc::Rc::new(V2Engine)),
        #[cfg(feature = "kas")]
        AgentEngine::Kas => Ok(std::rc::Rc::new(crate::protocol::engine::KasEngine {
            hooks_mode: config.kas_hooks,
        })),
        #[cfg(not(feature = "kas"))]
        AgentEngine::Kas => Err("KAS engine requires a build with --features kas".to_string()),
    }
}

#[cfg(feature = "kas")]
fn resolve_host_shell(
    config: &SpawnConfig,
) -> crate::Result<crate::protocol::client::ResolvedHostShell> {
    match config.engine {
        AgentEngine::V2 => Ok(None),
        AgentEngine::Kas => {
            crate::protocol::kas::host_shell::HostShell::resolve(config.shell.as_deref())
                .map(Some)
                .map_err(|source| {
                    crate::Error::with_source(
                        crate::ErrorKind::InvalidConfig {
                            detail: source.to_string(),
                        },
                        source,
                    )
                })
        }
    }
}

#[cfg(not(feature = "kas"))]
fn resolve_host_shell(
    _config: &SpawnConfig,
) -> crate::Result<crate::protocol::client::ResolvedHostShell> {
    Ok(crate::protocol::client::ResolvedHostShell)
}

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
    config: SpawnConfig,
    cwd: &std::path::Path,
    channels: BridgeChannels,
    host_shell: crate::protocol::client::ResolvedHostShell,
) -> crate::Result<()> {
    let engine = engine_for(&config)
        .map_err(|detail| crate::Error::from_kind(crate::ErrorKind::InvalidConfig { detail }))?;
    let command = resolve_spawn_command(agent_command, config.engine, config.kas_spawn)
        .map_err(|detail| crate::Error::from_kind(crate::ErrorKind::InvalidConfig { detail }))?;
    crate::platform::path::bind_agent_location(command.program());
    let process = crate::protocol::transport::AgentProcess::spawn(&command, cwd).await?;
    #[cfg(not(feature = "kas"))]
    let _host_shell = host_shell;
    let domain_config = crate::protocol::domain_mediator::DomainConfig {
        engine,
        cwd: cwd.to_owned(),
        present_as: config.present_as,
        stall_threshold: config.stall_threshold,
        #[cfg(feature = "kas")]
        host_shell,
    };
    let (mediator, domain_channels) =
        crate::protocol::domain_mediator::DomainMediator::new(domain_config, channels)?;
    let runtime = crate::protocol::sdk_runtime::SdkRuntime::start(
        process,
        domain_channels,
        crate::protocol::sdk_runtime::StageChain::new(Vec::new()),
    )
    .await?;
    mediator.run(runtime).await
}

#[must_use]
pub(crate) fn client_info(
    present_as: PresentAs,
) -> agent_client_protocol::schema::v1::Implementation {
    agent_client_protocol::schema::v1::Implementation::new(
        present_as.wire_name(),
        env!("CARGO_PKG_VERSION"),
    )
    .title("Cyril")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(feature = "kas"))]
    use crate::protocol::engine::V2Engine;
    #[cfg(not(feature = "kas"))]
    use crate::types::{SessionOrigin, StopReason, TurnId};
    #[cfg(not(feature = "kas"))]
    use std::cell::RefCell;
    #[cfg(not(feature = "kas"))]
    use std::rc::Rc;
    #[cfg(not(feature = "kas"))]
    use std::time::Duration;

    #[test]
    fn source_disposition_is_authoritative() {
        assert_eq!(
            source_disposition(crate::types::StopReason::EndTurn),
            crate::types::SourceTurnDisposition::Completed
        );
        assert_eq!(
            source_disposition(crate::types::StopReason::Cancelled),
            crate::types::SourceTurnDisposition::Interrupted
        );
    }

    #[test]
    fn client_identity_preserves_wire_name_and_title() {
        let info = client_info(PresentAs::Cyril);
        assert_eq!(info.name, "cyril");
        assert_eq!(info.title.as_deref(), Some("Cyril"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn initialization_failure_preserves_last_five_agent_stderr_lines() {
        let command = AgentCommand::try_from_argv(vec![
            "/bin/sh".to_owned(),
            "-c".to_owned(),
            "printf 'alpha\\nbeta\\ngamma\\ndelta\\nepsilon\\nzeta\\n' >&2; exit 1".to_owned(),
        ])
        .unwrap_or_else(|error| panic!("stderr fixture command is valid: {error}"));
        let handle = spawn_bridge(command, SpawnConfig::default(), std::env::temp_dir())
            .unwrap_or_else(|error| panic!("stderr fixture bridge starts: {error}"));
        let (_sender, mut rx, _permission_rx, _source_rx, _completion_rx) = handle.split();
        let routed = tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv())
            .await
            .unwrap_or_else(|error| panic!("stderr fixture notification timed out: {error}"))
            .unwrap_or_else(|| panic!("stderr fixture notification channel closed"));
        let Notification::BridgeDisconnected { reason } = &routed.notification else {
            panic!("expected BridgeDisconnected, got {:?}", routed.notification);
        };
        assert!(
            reason.contains("agent stderr:\nbeta\ngamma\ndelta\nepsilon\nzeta"),
            "disconnect must preserve the last five stderr lines: {reason}"
        );
        assert!(
            !reason.contains("agent stderr:\nalpha"),
            "disconnect must cap the diagnostic suffix at five lines: {reason}"
        );
    }

    #[tokio::test]
    async fn spawn_failure_emits_one_actionable_disconnect() {
        let command =
            AgentCommand::try_from_argv(vec!["cyril-gl5s-intentionally-missing-agent".to_owned()])
                .unwrap_or_else(|error| panic!("missing-agent command is valid: {error}"));
        let handle = spawn_bridge(command, SpawnConfig::default(), std::env::temp_dir())
            .unwrap_or_else(|error| panic!("bridge thread starts before process spawn: {error}"));
        let (_sender, mut rx, _permission_rx, _source_rx, _completion_rx) = handle.split();
        let routed = tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv())
            .await
            .unwrap_or_else(|error| panic!("spawn failure notification timed out: {error}"))
            .unwrap_or_else(|| panic!("spawn failure notification channel closed"));
        assert!(
            matches!(
                &routed.notification,
                Notification::BridgeDisconnected { reason }
                    if !reason.is_empty() && !reason.contains("agent stderr:")
            ),
            "spawn failure must emit one actionable disconnect without an empty stderr stub: {:?}",
            routed.notification
        );
    }
    #[cfg(not(feature = "kas"))]
    mod harness;
    #[cfg(not(feature = "kas"))]
    use harness::*;
    #[cfg(not(feature = "kas"))]
    mod current_runtime_contract;
}
