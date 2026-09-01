use agent_client_protocol::{Agent, Client, ConnectTo, ConnectionTo, DynConnectTo};
use agent_client_protocol_conductor::{ConductorImpl, ProxiesAndAgent};
use tokio::sync::oneshot;

use crate::protocol::domain_mediator::DomainChannels;
use crate::protocol::transport::AgentProcess;

mod process;
const REASON_TAIL_LINES: usize = 5;

/// Ordered SDK conductor stages. Empty is intentional: even a zero-stage
/// connection is still owned by `ConductorImpl`, so there is one runtime path.
#[derive(Default)]
pub(crate) struct StageChain {
    stages: Vec<DynConnectTo<agent_client_protocol::Conductor>>,
}

impl StageChain {
    pub(crate) fn new(stages: Vec<DynConnectTo<agent_client_protocol::Conductor>>) -> Self {
        Self { stages }
    }

    fn into_vec(self) -> Vec<DynConnectTo<agent_client_protocol::Conductor>> {
        self.stages
    }
}

pub(crate) struct SdkRuntimeHandle {
    connection: ConnectionTo<Agent>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
    done_rx: Option<oneshot::Receiver<String>>,
    stderr_tail: Option<crate::protocol::transport::StderrTail>,
}

impl SdkRuntimeHandle {
    pub(crate) fn connection(&self) -> &ConnectionTo<Agent> {
        &self.connection
    }
    pub(crate) fn take_done_rx(&mut self) -> Option<oneshot::Receiver<String>> {
        self.done_rx.take()
    }
    #[cfg(test)]
    pub(crate) fn abort_handle(&self) -> Option<tokio::task::AbortHandle> {
        self.task
            .as_ref()
            .map(tokio::task::JoinHandle::abort_handle)
    }

    pub(crate) fn disconnect_reason(&self, reason: String) -> String {
        let snapshot = self
            .stderr_tail
            .as_ref()
            .map_or_else(Vec::new, |tail| tail.snapshot());
        let tail = snapshot
            .iter()
            .map(String::as_str)
            .filter(|line| !line.is_empty())
            .rev()
            .take(REASON_TAIL_LINES)
            .collect::<Vec<_>>();
        if tail.is_empty() {
            reason
        } else {
            format!(
                "{reason}\nagent stderr:\n{}",
                tail.into_iter().rev().collect::<Vec<_>>().join("\n")
            )
        }
    }

    pub(crate) fn request_shutdown(&mut self) {
        if let Some(sender) = self.shutdown_tx.take()
            && sender.send(()).is_err()
        {
            tracing::debug!("SDK runtime shutdown receiver dropped");
        }
    }

    pub(crate) async fn shutdown(mut self) {
        self.request_shutdown();
        if let Some(task) = self.task.take()
            && let Err(error) = task.await
        {
            tracing::debug!(%error, "SDK runtime task join failed");
        }
    }
}

/// The sole active stable-wire-v1 SDK runtime.
pub(crate) struct SdkRuntime;

impl SdkRuntime {
    pub(crate) async fn start(
        process: AgentProcess,
        domain_channels: DomainChannels,
        stages: StageChain,
    ) -> crate::Result<SdkRuntimeHandle> {
        let stderr_tail = process.stderr_tail();
        let process_adapter =
            process::ProcessAdapter::new(process, domain_channels.transport_closed_line());
        Self::start_connector(process_adapter, domain_channels, stages, Some(stderr_tail)).await
    }

    #[cfg(test)]
    pub(super) async fn start_for_test(
        agent: impl ConnectTo<Client> + 'static,
        domain_channels: DomainChannels,
        stages: StageChain,
    ) -> crate::Result<SdkRuntimeHandle> {
        Self::start_connector(agent, domain_channels, stages, None).await
    }

    #[cfg(test)]
    pub(super) async fn start_recording_process_for_test(
        process: AgentProcess,
        domain_channels: DomainChannels,
        stages: StageChain,
        capture: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    ) -> crate::Result<SdkRuntimeHandle> {
        let stderr_tail = process.stderr_tail();
        let process_adapter = process::ProcessAdapter::new_recording(
            process,
            domain_channels.transport_closed_line(),
            capture,
        );
        Self::start_connector(process_adapter, domain_channels, stages, Some(stderr_tail)).await
    }

    async fn start_connector(
        agent: impl ConnectTo<Client> + 'static,
        domain_channels: DomainChannels,
        stages: StageChain,
        stderr_tail: Option<crate::protocol::transport::StderrTail>,
    ) -> crate::Result<SdkRuntimeHandle> {
        let conductor = ConductorImpl::new_agent(
            "cyril",
            ProxiesAndAgent::new(agent).proxies(stages.into_vec()),
        );
        let (connection_tx, connection_rx) = oneshot::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        let mut task = tokio::task::spawn_local(async move {
            let result = crate::protocol::client::run_client(
                conductor,
                domain_channels,
                connection_tx,
                shutdown_rx,
            )
            .await;
            let reason = match result {
                Ok(()) => "agent connection closed unexpectedly".to_owned(),
                Err(error) => format!("agent connection closed: {error}"),
            };
            if done_tx.send(reason).is_err() {
                tracing::debug!("SDK runtime completion receiver dropped");
            }
        });
        let connection = match connection_rx.await {
            Ok(connection) => connection,
            Err(_) => {
                if shutdown_tx.send(()).is_err() {
                    tracing::debug!("SDK runtime stopped before accepting shutdown");
                }
                if let Err(error) = (&mut task).await {
                    tracing::debug!(%error, "failed SDK runtime task join after startup error");
                }
                return Err(crate::Error::from_kind(crate::ErrorKind::Transport {
                    detail: "SDK client runtime stopped before connection setup".into(),
                }));
            }
        };
        Ok(SdkRuntimeHandle {
            connection,
            shutdown_tx: Some(shutdown_tx),
            task: Some(task),
            done_rx: Some(done_rx),
            stderr_tail,
        })
    }
}
#[cfg(test)]
mod tests;
