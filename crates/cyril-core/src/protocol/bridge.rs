use std::path::PathBuf;

use tokio::sync::{mpsc, oneshot, watch};

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
    client_lifetime_tx: Option<watch::Sender<()>>,
    pub(crate) notification_rx: mpsc::Receiver<RoutedNotification>,
    pub(crate) permission_rx: mpsc::Receiver<PermissionRequest>,
    source_rx: mpsc::Receiver<crate::types::SourceTurnEvent>,
    completion_rx: oneshot::Receiver<()>,
    review_shell: Option<crate::review::ShellDialect>,
}

impl BridgeHandle {
    /// Dialect of the same host shell resolved for this bridge's KAS session.
    pub fn review_shell(&self) -> Option<crate::review::ShellDialect> {
        self.review_shell
    }

    pub async fn recv_notification(&mut self) -> Option<RoutedNotification> {
        self.notification_rx.recv().await
    }

    pub async fn recv_permission(&mut self) -> Option<PermissionRequest> {
        self.permission_rx.recv().await
    }

    pub fn sender(&self) -> BridgeSender {
        BridgeSender {
            command_tx: self.command_tx.clone(),
            _client_lifetime_tx: self.client_lifetime_tx.clone(),
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
                client_lifetime_tx: None,
                notification_rx,
                permission_rx,
                source_rx,
                completion_rx,
                review_shell: None,
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
                _client_lifetime_tx: self.client_lifetime_tx,
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
    // Ownership only: the final real client closes initialization's watch channel.
    _client_lifetime_tx: Option<watch::Sender<()>>,
}

impl BridgeSender {
    pub fn from_sender(tx: mpsc::Sender<BridgeCommand>) -> Self {
        Self {
            command_tx: tx,
            _client_lifetime_tx: None,
        }
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
    pub client_lifetime_rx: watch::Receiver<()>,
    pub notification_tx: mpsc::Sender<RoutedNotification>,
    pub permission_tx: mpsc::Sender<PermissionRequest>,
    pub source_tx: mpsc::Sender<crate::types::SourceTurnEvent>,
    pub completion_tx: Option<oneshot::Sender<()>>,
}

pub(crate) fn create_channel_pair() -> (BridgeHandle, BridgeChannels) {
    let (command_tx, command_rx) = mpsc::channel(COMMAND_CAPACITY);
    let (client_lifetime_tx, client_lifetime_rx) = watch::channel(());
    let (notification_tx, notification_rx) = mpsc::channel(NOTIFICATION_CAPACITY);
    let (permission_tx, permission_rx) = mpsc::channel(PERMISSION_CAPACITY);
    let (source_tx, source_rx) =
        mpsc::channel(crate::types::source_turn::SOURCE_EVENT_CHANNEL_CAPACITY);
    let (completion_tx, completion_rx) = oneshot::channel();
    (
        BridgeHandle {
            command_tx,
            client_lifetime_tx: Some(client_lifetime_tx),
            notification_rx,
            permission_rx,
            source_rx,
            completion_rx,
            review_shell: None,
        },
        BridgeChannels {
            command_rx,
            client_lifetime_rx,
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
    /// Environment used by both the agent and the existing CLI version probe.
    /// KAS Free launches and Host hooks require `Inherit`; use Wrapper without
    /// Host hooks with `Replace`. See `SpawnEnvironment` for PATH lookup rules.
    pub environment: crate::types::SpawnEnvironment,
    /// Exact CLI version token required for KAS Wrapper launches.
    pub required_cli_version: Option<String>,
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
            environment: crate::types::SpawnEnvironment::Inherit,
            required_cli_version: None,
        }
    }
}

pub fn spawn_bridge(
    agent_command: AgentCommand,
    config: SpawnConfig,
    cwd: PathBuf,
) -> crate::Result<BridgeHandle> {
    let host_shell = resolve_host_shell(&config)?;
    let (mut handle, mut channels) = create_channel_pair();
    handle.review_shell = {
        #[cfg(feature = "kas")]
        {
            host_shell.as_ref().map(|shell| shell.review_shell())
        }
        #[cfg(not(feature = "kas"))]
        {
            None
        }
    };
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
            settings: crate::protocol::kas::settings::settings_extra_value(&config.environment),
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

/// The resolved spawn command plus, on KAS, the kiro-cli the
/// `getAccessToken` responder asks (cyril-zbu7): the PATH `kiro-cli` on the
/// Free path (the same source its login gate just asked), the agent
/// command's own kiro-cli — through the WSL launcher when it routes through
/// one, under the spawn's environment — on the Wrapper path.
#[cfg(feature = "kas")]
async fn resolve_spawn_command(
    agent_command: &AgentCommand,
    config: &SpawnConfig,
) -> Result<
    (
        AgentCommand,
        Option<crate::protocol::kas::auth::KasTokenSource>,
    ),
    String,
> {
    use crate::protocol::kas::auth::KasTokenSource;
    if config.engine == AgentEngine::Kas
        && (config.kas_spawn == KasSpawn::Free || config.kas_hooks == KasHooksMode::Host)
        && matches!(
            config.environment,
            crate::types::SpawnEnvironment::Replace(_)
        )
    {
        return Err(if config.kas_spawn == KasSpawn::Free {
            "KAS Free launches require an inherited environment; use Wrapper for a replacement environment"
        } else {
            "KAS Host hooks require an inherited environment; disable host hooks for a replacement environment"
        }
        .into());
    }
    if config.required_cli_version.is_some()
        && (config.engine != AgentEngine::Kas || config.kas_spawn != KasSpawn::Wrapper)
    {
        return Err("required_cli_version requires the KAS Wrapper launch path".into());
    }
    match config.engine {
        AgentEngine::Kas => match config.kas_spawn {
            KasSpawn::Free => {
                let token_source = KasTokenSource::on_path();
                let command = crate::protocol::kas::discovery::resolve_kas_command(&token_source)
                    .await
                    .map_err(|m| m.reason())?;
                Ok((command, Some(token_source)))
            }
            KasSpawn::Wrapper => {
                let command = crate::protocol::kas::version::build_wrapper_command(
                    agent_command,
                    &config.environment,
                    config.required_cli_version.as_deref(),
                )
                .await?;
                let kiro_cli = crate::protocol::kas::version::kiro_cli_invocation(
                    agent_command,
                    "locate kiro-cli for the KAS access token",
                )?;
                let token_source = KasTokenSource::new(kiro_cli, config.environment.clone());
                Ok((command, Some(token_source)))
            }
        },
        AgentEngine::V2 => Ok((agent_command.clone(), None)),
    }
}

#[cfg(not(feature = "kas"))]
async fn resolve_spawn_command(
    agent_command: &AgentCommand,
    config: &SpawnConfig,
) -> Result<AgentCommand, String> {
    if config.required_cli_version.is_some() {
        return Err(
            "required_cli_version requires a build with --features kas and KAS Wrapper".into(),
        );
    }
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
    let resolved = resolve_spawn_command(agent_command, &config)
        .await
        .map_err(|detail| crate::Error::from_kind(crate::ErrorKind::InvalidConfig { detail }))?;
    #[cfg(feature = "kas")]
    let (command, kas_token) = resolved;
    #[cfg(not(feature = "kas"))]
    let command = resolved;
    crate::platform::path::bind_agent_location(command.program());
    let process =
        crate::protocol::transport::AgentProcess::spawn(&command, cwd, &config.environment).await?;
    #[cfg(not(feature = "kas"))]
    let _host_shell = host_shell;
    let domain_config = crate::protocol::domain_mediator::DomainConfig {
        engine,
        cwd: cwd.to_owned(),
        present_as: config.present_as,
        stall_threshold: config.stall_threshold,
        #[cfg(feature = "kas")]
        host_shell,
        #[cfg(feature = "kas")]
        kas_token,
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
    use crate::protocol::engine::V2Engine;
    #[cfg(not(feature = "kas"))]
    use crate::types::SessionOrigin;
    use crate::types::{StopReason, TurnId};
    use std::cell::RefCell;
    use std::rc::Rc;
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

    // cyril-zbu7 C7: a KAS Wrapper spawn binds a token source that runs the
    // AGENT COMMAND's kiro-cli (here a fake at a private path, symlinked to
    // the committed fixture), and V2 binds none.
    #[cfg(all(unix, feature = "kas"))]
    #[tokio::test]
    async fn wrapper_spawn_binds_the_agent_commands_kiro_cli_as_token_source() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cli = dir.path().join("kiro-cli");
        std::os::unix::fs::symlink(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/fake-kiro-cli.sh"
            ),
            &cli,
        )
        .expect("fixture symlink");
        let agent_command =
            AgentCommand::new(cli.to_str().expect("utf-8 path")).with_args(vec!["acp".to_string()]);
        let config = SpawnConfig {
            engine: AgentEngine::Kas,
            kas_spawn: KasSpawn::Wrapper,
            ..SpawnConfig::default()
        };
        let (command, token_source) = resolve_spawn_command(&agent_command, &config)
            .await
            .expect("wrapper resolves");
        assert_eq!(command.program(), cli.to_str().expect("utf-8 path"));
        let token = token_source
            .expect("KAS Wrapper binds a token source")
            .fetch(false)
            .await
            .expect("the agent command's kiro-cli answers");
        assert_eq!(token.expires_at(), "2099-01-01T00:00:00Z");
        let journal =
            std::fs::read_to_string(dir.path().join("kiro-cli-argv.txt")).expect("journal");
        assert!(journal.ends_with("chat\n_\nget-kas-token\n"), "{journal:?}");

        let v2 = SpawnConfig {
            engine: AgentEngine::V2,
            ..SpawnConfig::default()
        };
        let (_, none) = resolve_spawn_command(&agent_command, &v2)
            .await
            .expect("v2 resolves");
        assert!(none.is_none(), "v2 binds no token source");
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
    mod harness;
    use harness::*;
    mod current_runtime_contract;
}
