use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use cyril_memory::{
    AdminClient, ClientError, ContextBlock, HealthResponse, LessonId, LessonListResponse,
    LessonRecord, LessonText, MAX_CONTEXT_CHARS, MemoryConfig, MemoryConfigState, MemoryEndpoint,
    MemoryErrorCode, MemoryPaths, ProjectScope, RuntimeHealth, RuntimeLaunchConfig, TeachResponse,
};
use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use thiserror::Error;
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;

/// Bound on the first-prompt context round trip. A slow companion delays a
/// fresh session's first prompt by at most this much; after that the prompt
/// goes out without lessons.
pub(crate) const FIRST_PROMPT_CONTEXT_TIMEOUT: Duration = Duration::from_millis(250);

const STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(100);
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);
const FORCE_KILL_WAIT: Duration = Duration::from_millis(500);
const OUTER_SHUTDOWN_BOUND: Duration = Duration::from_millis(4_500);
const RUNTIME_PATH_ENV: &str = "CYRIL_MEMORY_RUNTIME_PATH";
const RUNTIME_DIR_ENV: &str = "CYRIL_MEMORY_RUNTIME_DIR";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MemoryDisabledReason {
    Absent,
    ConfiguredOff,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MemoryRuntimeFailure {
    InvalidConfig(String),
    ConfigUnreadable(String),
    ConfigUnparseable(String),
    DataRootUnavailable,
    RuntimeExecutableUnavailable,
    SpawnFailed,
    StartupTimedOut,
    RuntimeExited,
    RuntimeReported(MemoryErrorCode),
}

impl MemoryRuntimeFailure {
    pub(crate) fn message(&self) -> &str {
        match self {
            Self::InvalidConfig(message)
            | Self::ConfigUnreadable(message)
            | Self::ConfigUnparseable(message) => message,
            Self::DataRootUnavailable => "memory data root is unavailable",
            Self::RuntimeExecutableUnavailable => "memory runtime executable is unavailable",
            Self::SpawnFailed => "memory runtime could not be started",
            Self::StartupTimedOut => "memory runtime startup timed out",
            Self::RuntimeExited => "memory runtime exited unexpectedly",
            Self::RuntimeReported(code) => code.as_str(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MemoryRuntimeStatus {
    Disabled(MemoryDisabledReason),
    Starting,
    Ready(HealthResponse),
    Degraded(MemoryRuntimeFailure),
    Failed(MemoryRuntimeFailure),
}

#[derive(Clone, Debug)]
struct ProjectAccess {
    endpoint: MemoryEndpoint,
    credential: cyril_memory::AdminCredential,
    request_timeout: Duration,
}

/// Why a project-scoped operation cannot reach the runtime right now.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub(crate) enum ProjectMemoryUnavailable {
    #[error("memory is disabled")]
    Disabled,
    #[error("memory runtime is still starting")]
    Starting,
    #[error("memory runtime is degraded")]
    Degraded,
    #[error("memory runtime failed to start")]
    Failed,
    #[error("memory runtime is no longer running")]
    RuntimeLost,
}

#[derive(Debug, Error)]
pub(crate) enum ProjectMemoryError {
    #[error("project memory is unavailable: {0}")]
    Unavailable(ProjectMemoryUnavailable),
    #[error(transparent)]
    Client(#[from] ClientError),
}

/// Why a fresh session's first prompt went out without lessons.
#[derive(Debug, Error)]
pub(crate) enum FirstPromptContextError {
    #[error("{0}")]
    Unavailable(ProjectMemoryUnavailable),
    #[error("memory runtime did not answer within {0:?}")]
    TimedOut(Duration),
    #[error(transparent)]
    Client(ClientError),
}

impl FirstPromptContextError {
    /// Whether the session should stay eligible so its NEXT prompt is
    /// augmented: only a companion that has not finished starting yet.
    /// Disabled/failed/timed-out/malformed are not going to change by
    /// themselves, and re-trying them would spend the deadline on every
    /// prompt.
    pub(crate) const fn retry_on_next_prompt(&self) -> bool {
        matches!(self, Self::Unavailable(ProjectMemoryUnavailable::Starting))
    }
}

/// One workspace bound to project memory for the life of the process.
#[derive(Clone, Debug)]
pub(crate) struct ProjectMemory {
    project: ProjectScope,
    status: watch::Receiver<MemoryRuntimeStatus>,
    access: watch::Receiver<Option<ProjectAccess>>,
}

impl ProjectMemory {
    async fn client(&self) -> Result<AdminClient, ProjectMemoryError> {
        let mut status = self.status.clone();
        let current = status.borrow_and_update().clone();
        let status_closed = status.has_changed().is_err();
        let unavailable = match &current {
            MemoryRuntimeStatus::Disabled(_) => Some(ProjectMemoryUnavailable::Disabled),
            MemoryRuntimeStatus::Starting if status_closed => {
                Some(ProjectMemoryUnavailable::RuntimeLost)
            }
            MemoryRuntimeStatus::Starting => Some(ProjectMemoryUnavailable::Starting),
            MemoryRuntimeStatus::Degraded(_) => Some(ProjectMemoryUnavailable::Degraded),
            MemoryRuntimeStatus::Failed(_) => Some(ProjectMemoryUnavailable::Failed),
            MemoryRuntimeStatus::Ready(_) if status_closed => {
                Some(ProjectMemoryUnavailable::RuntimeLost)
            }
            MemoryRuntimeStatus::Ready(_) => None,
        };
        if let Some(unavailable) = unavailable {
            return Err(ProjectMemoryError::Unavailable(unavailable));
        }
        let mut access = self.access.clone();
        let current_access = access.borrow_and_update().clone();
        if access.has_changed().is_err() {
            return Err(ProjectMemoryError::Unavailable(
                ProjectMemoryUnavailable::RuntimeLost,
            ));
        }
        let access = current_access.ok_or(ProjectMemoryError::Unavailable(
            ProjectMemoryUnavailable::RuntimeLost,
        ))?;
        AdminClient::connect(access.endpoint, access.credential, access.request_timeout)
            .await
            .map_err(ProjectMemoryError::from)
    }

    pub(crate) fn project(&self) -> &ProjectScope {
        &self.project
    }

    pub(crate) async fn teach(
        &self,
        text: LessonText,
    ) -> Result<TeachResponse, ProjectMemoryError> {
        Ok(self.client().await?.teach(&self.project, text).await?)
    }

    pub(crate) async fn replace(
        &self,
        replaced_id: LessonId,
        text: LessonText,
    ) -> Result<TeachResponse, ProjectMemoryError> {
        Ok(self
            .client()
            .await?
            .replace(&self.project, replaced_id, text)
            .await?)
    }

    pub(crate) async fn list(&self) -> Result<LessonListResponse, ProjectMemoryError> {
        Ok(self.client().await?.list(&self.project).await?)
    }

    pub(crate) async fn inspect(&self, id: LessonId) -> Result<LessonRecord, ProjectMemoryError> {
        Ok(self.client().await?.inspect(&self.project, id).await?)
    }

    pub(crate) async fn context(
        &self,
        max_chars: u16,
    ) -> Result<Option<ContextBlock>, ProjectMemoryError> {
        Ok(self
            .client()
            .await?
            .context(&self.project, max_chars)
            .await?)
    }

    /// The bounded first-prompt context block, or `None` when the project
    /// has no active lessons. Every failure names its cause so the caller
    /// can log it and decide whether the session stays eligible.
    pub(crate) async fn first_prompt_context(
        &self,
    ) -> Result<Option<String>, FirstPromptContextError> {
        match tokio::time::timeout(
            FIRST_PROMPT_CONTEXT_TIMEOUT,
            self.context(MAX_CONTEXT_CHARS),
        )
        .await
        {
            Ok(Ok(block)) => Ok(block
                .map(|block| block.text().to_owned())
                .filter(|text| !text.is_empty())),
            Ok(Err(ProjectMemoryError::Unavailable(reason))) => {
                Err(FirstPromptContextError::Unavailable(reason))
            }
            Ok(Err(ProjectMemoryError::Client(error))) => {
                Err(FirstPromptContextError::Client(error))
            }
            Err(_) => Err(FirstPromptContextError::TimedOut(
                FIRST_PROMPT_CONTEXT_TIMEOUT,
            )),
        }
    }
}

/// How the startup workspace resolved against project memory. Orthogonal to
/// runtime health: a Ready runtime with an unbound workspace still cannot
/// serve lesson commands, and the cause must be visible.
#[derive(Clone, Debug)]
pub(crate) enum ProjectBinding {
    /// Memory is not enabled; no binding was attempted.
    Disabled,
    /// The workspace resolved to a project.
    Bound(ProjectMemory),
    /// The workspace could not be resolved.
    Unbound { reason: String },
}

impl ProjectBinding {
    pub(crate) const fn memory(&self) -> Option<&ProjectMemory> {
        match self {
            Self::Bound(memory) => Some(memory),
            Self::Disabled | Self::Unbound { .. } => None,
        }
    }

    /// The project axis of `/memory status`; absent when memory is disabled.
    pub(crate) fn status_view(&self) -> Option<cyril_core::types::MemoryProjectBinding> {
        match self {
            Self::Disabled => None,
            Self::Bound(memory) => Some(cyril_core::types::MemoryProjectBinding::bound(
                memory.project().display_path().to_string_lossy(),
            )),
            Self::Unbound { reason } => {
                Some(cyril_core::types::MemoryProjectBinding::unbound(reason))
            }
        }
    }

    /// What a lesson command tells the user when it cannot run.
    pub(crate) fn unavailable_message(&self) -> Option<String> {
        match self {
            Self::Bound(_) => None,
            Self::Disabled => Some(
                "Memory is disabled. Set `[memory] enabled = true` in Cyril's config to use project lessons."
                    .to_owned(),
            ),
            Self::Unbound { reason } => {
                Some(format!("Memory is unavailable for this project: {reason}"))
            }
        }
    }
}

pub(crate) struct MemoryRuntimeHandle {
    status: watch::Receiver<MemoryRuntimeStatus>,
    access: watch::Receiver<Option<ProjectAccess>>,
    status_closed: bool,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl MemoryRuntimeHandle {
    pub(crate) fn start(state: MemoryConfigState) -> Self {
        match state {
            MemoryConfigState::Absent => {
                Self::terminal(MemoryRuntimeStatus::Disabled(MemoryDisabledReason::Absent))
            }
            MemoryConfigState::Valid(config) if !config.enabled() => Self::terminal(
                MemoryRuntimeStatus::Disabled(MemoryDisabledReason::ConfiguredOff),
            ),
            MemoryConfigState::Valid(config) => Self::launch(config, None),
            MemoryConfigState::Invalid(diagnostics) => {
                let detail = diagnostics
                    .first()
                    .map_or("memory configuration is invalid", |value| value.message())
                    .to_owned();
                Self::terminal(MemoryRuntimeStatus::Failed(
                    MemoryRuntimeFailure::InvalidConfig(detail),
                ))
            }
            MemoryConfigState::Unreadable(diagnostic) => {
                Self::terminal(MemoryRuntimeStatus::Failed(
                    MemoryRuntimeFailure::ConfigUnreadable(diagnostic.message().to_owned()),
                ))
            }
            MemoryConfigState::ConfigUnparseable(diagnostic) => {
                Self::terminal(MemoryRuntimeStatus::Failed(
                    MemoryRuntimeFailure::ConfigUnparseable(diagnostic.message().to_owned()),
                ))
            }
        }
    }

    #[cfg(test)]
    fn start_with_options(config: MemoryConfig, executable: PathBuf, runtime_dir: PathBuf) -> Self {
        Self::launch(
            config,
            Some(LaunchOptions {
                executable,
                runtime_dir,
            }),
        )
    }

    fn launch(config: MemoryConfig, options: Option<LaunchOptions>) -> Self {
        let (status_tx, status) = watch::channel(MemoryRuntimeStatus::Starting);
        let (access_tx, access) = watch::channel(None);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            supervise(config, options, status_tx, access_tx, shutdown_rx).await;
        });
        Self {
            status,
            access,
            status_closed: false,
            shutdown: Some(shutdown_tx),
            task: Some(task),
        }
    }

    fn terminal(status_value: MemoryRuntimeStatus) -> Self {
        let (_status_tx, status) = watch::channel(status_value);
        let (_access_tx, access) = watch::channel(None);
        Self {
            status,
            access,
            status_closed: false,
            shutdown: None,
            task: None,
        }
    }

    pub(crate) fn status(&self) -> MemoryRuntimeStatus {
        self.status.borrow().clone()
    }
    pub(crate) fn status_view(&self) -> cyril_core::types::MemoryStatusView {
        project_status(&self.status())
    }

    /// Resolve the startup workspace once. A binding failure is reported,
    /// never collapsed into "no project": the runtime may be perfectly
    /// healthy while this workspace still cannot use it, and the user needs
    /// the reason (a relocated worktree, a malformed `.git` file, ...).
    pub(crate) fn bind_project(&self, workspace: &Path) -> ProjectBinding {
        if matches!(self.status(), MemoryRuntimeStatus::Disabled(_)) {
            return ProjectBinding::Disabled;
        }
        match ProjectScope::resolve(workspace) {
            Ok(project) => ProjectBinding::Bound(ProjectMemory {
                project,
                status: self.status.clone(),
                access: self.access.clone(),
            }),
            Err(error) => {
                tracing::warn!(
                    workspace = %workspace.display(),
                    error = %error,
                    "memory project binding failed"
                );
                ProjectBinding::Unbound {
                    reason: error.to_string(),
                }
            }
        }
    }

    pub(crate) async fn changed(&mut self) -> Option<cyril_core::types::MemoryStatusView> {
        if self.status_closed {
            return None;
        }
        match self.status.changed().await {
            Ok(()) => Some(self.status_view()),
            Err(error) => {
                self.status_closed = true;
                let current = self.status_view();
                let terminal = match current.status() {
                    cyril_core::types::MemoryStatus::Starting => {
                        cyril_core::types::MemoryStatusView::failed(
                            "memory runtime status channel closed during startup",
                        )
                    }
                    cyril_core::types::MemoryStatus::Ready => {
                        cyril_core::types::MemoryStatusView::degraded(
                            "memory runtime status channel closed",
                        )
                    }
                    cyril_core::types::MemoryStatus::Disabled
                    | cyril_core::types::MemoryStatus::Degraded
                    | cyril_core::types::MemoryStatus::Failed => current,
                };
                tracing::warn!(
                    error = %error,
                    terminal_status = ?terminal.status(),
                    "memory runtime status channel closed"
                );
                Some(terminal)
            }
        }
    }

    pub(crate) async fn shutdown(&mut self) {
        if let Some(shutdown) = self.shutdown.take()
            && shutdown.send(()).is_err()
        {
            tracing::debug!("memory runtime supervisor already stopped");
        }
        let Some(mut task) = self.task.take() else {
            return;
        };
        if tokio::time::timeout(OUTER_SHUTDOWN_BOUND, &mut task)
            .await
            .is_err()
        {
            tracing::warn!("memory runtime supervisor exceeded shutdown bound; aborting");
            task.abort();
        }
    }
}
fn project_status(status: &MemoryRuntimeStatus) -> cyril_core::types::MemoryStatusView {
    match status {
        MemoryRuntimeStatus::Disabled(MemoryDisabledReason::Absent) => {
            cyril_core::types::MemoryStatusView::disabled(
                cyril_core::types::MemoryDisabledReason::Absent,
            )
        }
        MemoryRuntimeStatus::Disabled(MemoryDisabledReason::ConfiguredOff) => {
            cyril_core::types::MemoryStatusView::disabled(
                cyril_core::types::MemoryDisabledReason::ConfiguredOff,
            )
        }
        MemoryRuntimeStatus::Starting => cyril_core::types::MemoryStatusView::starting(),
        MemoryRuntimeStatus::Ready(health) => {
            let Some(versions) = health.store_versions() else {
                return cyril_core::types::MemoryStatusView::failed(
                    "memory runtime ready health omitted store versions",
                );
            };
            let versions = cyril_core::types::MemoryStoreVersions::new(
                versions.memory(),
                versions.knowledge(),
            );
            cyril_core::types::MemoryStatusView::ready(
                health.instance_id(),
                health.protocol_version(),
                versions,
            )
        }
        MemoryRuntimeStatus::Degraded(failure) => {
            cyril_core::types::MemoryStatusView::degraded(failure.message())
        }
        MemoryRuntimeStatus::Failed(failure) => {
            cyril_core::types::MemoryStatusView::failed(failure.message())
        }
    }
}

impl Drop for MemoryRuntimeHandle {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take()
            && shutdown.send(()).is_err()
        {
            tracing::debug!("memory runtime supervisor stopped before handle drop");
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

struct LaunchOptions {
    executable: PathBuf,
    runtime_dir: PathBuf,
}

async fn supervise(
    config: MemoryConfig,
    options: Option<LaunchOptions>,
    status: watch::Sender<MemoryRuntimeStatus>,
    access: watch::Sender<Option<ProjectAccess>>,
    mut shutdown: oneshot::Receiver<()>,
) {
    let paths = match MemoryPaths::prepare(config.data_root()) {
        Ok(paths) => paths,
        Err(error) => {
            tracing::warn!(error = %error, "memory data root unavailable");
            send_status(
                &status,
                MemoryRuntimeStatus::Failed(MemoryRuntimeFailure::DataRootUnavailable),
            );
            return;
        }
    };
    let (executable, runtime_dir) = match options {
        Some(options) => (options.executable, options.runtime_dir),
        None => match (resolve_runtime_executable(), resolve_runtime_dir()) {
            (Ok(executable), Ok(runtime_dir)) => (executable, runtime_dir),
            (Err(error), _) | (_, Err(error)) => {
                tracing::warn!(error = %error, "memory runtime launch path unavailable");
                send_status(
                    &status,
                    MemoryRuntimeStatus::Failed(MemoryRuntimeFailure::RuntimeExecutableUnavailable),
                );
                return;
            }
        },
    };
    let executable = match validate_executable(&executable) {
        Ok(executable) => executable,
        Err(error) => {
            tracing::warn!(error = %error, "memory runtime executable invalid");
            send_status(
                &status,
                MemoryRuntimeStatus::Failed(MemoryRuntimeFailure::RuntimeExecutableUnavailable),
            );
            return;
        }
    };
    let endpoint = match create_endpoint(&runtime_dir) {
        Ok(endpoint) => endpoint,
        Err(error) => {
            tracing::warn!(error = %error, "memory runtime endpoint unavailable");
            send_status(
                &status,
                MemoryRuntimeStatus::Failed(MemoryRuntimeFailure::DataRootUnavailable),
            );
            return;
        }
    };
    let credential = match cyril_memory::AdminCredential::generate() {
        Ok(credential) => credential,
        Err(error) => {
            tracing::warn!(error = %error, "memory runtime credential generation failed");
            send_status(
                &status,
                MemoryRuntimeStatus::Failed(MemoryRuntimeFailure::SpawnFailed),
            );
            return;
        }
    };
    let launch = RuntimeLaunchConfig::new(
        paths,
        endpoint.clone(),
        credential.clone(),
        Duration::from_millis(config.request_timeout_ms()),
    );
    let mut command = CommandWrap::with_new(&executable, |command| {
        launch.apply_to_command(command);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    });
    #[cfg(unix)]
    command.wrap(process_wrap::tokio::ProcessGroup::leader());
    #[cfg(windows)]
    command.wrap(process_wrap::tokio::JobObject);
    command.wrap(KillOnDrop);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            tracing::warn!(error = %error, "memory runtime spawn failed");
            send_status(
                &status,
                MemoryRuntimeStatus::Failed(MemoryRuntimeFailure::SpawnFailed),
            );
            return;
        }
    };

    let startup_deadline =
        tokio::time::Instant::now() + Duration::from_millis(config.startup_timeout_ms());
    let ready = loop {
        if !matches!(
            shutdown.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ) {
            stop_child(&mut child, &endpoint, &credential, launch.request_timeout()).await;
            return;
        }
        match child.try_wait() {
            Ok(Some(exit)) => {
                tracing::warn!(?exit, "memory runtime exited during startup");
                send_status(
                    &status,
                    MemoryRuntimeStatus::Failed(MemoryRuntimeFailure::RuntimeExited),
                );
                return;
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(error = %error, "memory runtime status check failed");
                send_status(
                    &status,
                    MemoryRuntimeStatus::Failed(MemoryRuntimeFailure::RuntimeExited),
                );
                stop_child(&mut child, &endpoint, &credential, launch.request_timeout()).await;
                return;
            }
        }
        if tokio::time::Instant::now() >= startup_deadline {
            send_status(
                &status,
                MemoryRuntimeStatus::Failed(MemoryRuntimeFailure::StartupTimedOut),
            );
            stop_child(&mut child, &endpoint, &credential, launch.request_timeout()).await;
            return;
        }
        if let Ok(mut client) = AdminClient::connect(
            endpoint.clone(),
            credential.clone(),
            launch.request_timeout(),
        )
        .await
        {
            match client.health().await {
                Ok(health) if health.status() == RuntimeHealth::Ready => break Some(health),
                Ok(health) if health.status() == RuntimeHealth::Failed => {
                    let failure = health
                        .error()
                        .map_or(MemoryErrorCode::Internal, |error| error.code());
                    send_status(
                        &status,
                        MemoryRuntimeStatus::Failed(MemoryRuntimeFailure::RuntimeReported(failure)),
                    );
                    break None;
                }
                Ok(_) | Err(_) => {}
            }
        }
        tokio::time::sleep(STARTUP_POLL_INTERVAL).await;
    };
    let was_ready = ready.is_some();
    if let Some(health) = ready {
        access.send_replace(Some(ProjectAccess {
            endpoint: endpoint.clone(),
            credential: credential.clone(),
            request_timeout: launch.request_timeout(),
        }));
        send_status(&status, MemoryRuntimeStatus::Ready(health));
    }

    let wait = child.wait();
    tokio::select! {
        exit = wait => {
            access.send_replace(None);
            if was_ready {
                tracing::warn!(?exit, "memory runtime exited after readiness");
                send_status(
                    &status,
                    MemoryRuntimeStatus::Degraded(MemoryRuntimeFailure::RuntimeExited),
                );
            }
        }
        _ = &mut shutdown => {
            access.send_replace(None);
            stop_child(&mut child, &endpoint, &credential, launch.request_timeout()).await;
        }
    }
}

async fn stop_child(
    child: &mut Box<dyn ChildWrapper>,
    endpoint: &MemoryEndpoint,
    credential: &cyril_memory::AdminCredential,
    request_timeout: Duration,
) {
    if let Ok(mut client) =
        AdminClient::connect(endpoint.clone(), credential.clone(), request_timeout).await
        && let Err(error) = client.shutdown().await
    {
        tracing::debug!(error = %error, "memory runtime graceful shutdown request failed");
    }
    let wait = child.wait();
    if tokio::time::timeout(SHUTDOWN_GRACE, wait).await.is_ok() {
        return;
    }
    let kill = Box::into_pin(child.kill());
    if tokio::time::timeout(FORCE_KILL_WAIT, kill).await.is_err() {
        tracing::warn!("memory runtime process tree did not reap after force kill");
    }
}

fn send_status(status: &watch::Sender<MemoryRuntimeStatus>, value: MemoryRuntimeStatus) {
    status.send_replace(value);
}

fn resolve_runtime_executable() -> Result<PathBuf, std::io::Error> {
    let candidate = match std::env::var_os(RUNTIME_PATH_ENV) {
        Some(path) => PathBuf::from(path),
        None => {
            let current = std::env::current_exe()?;
            let parent = current.parent().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "current executable has no parent",
                )
            })?;
            #[cfg(windows)]
            let name = "cyril-memory-runtime.exe";
            #[cfg(not(windows))]
            let name = "cyril-memory-runtime";
            parent.join(name)
        }
    };
    validate_executable(&candidate)
}

fn validate_executable(path: &Path) -> Result<PathBuf, std::io::Error> {
    if !path.is_absolute() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "memory runtime executable must be absolute",
        ));
    }
    let canonical = path.canonicalize()?;
    if !canonical.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "memory runtime executable is not a file",
        ));
    }
    Ok(canonical)
}

fn resolve_runtime_dir() -> Result<PathBuf, std::io::Error> {
    if let Some(value) = std::env::var_os(RUNTIME_DIR_ENV) {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            return Ok(path.join("cyril"));
        }
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "memory runtime directory override must be absolute",
        ));
    }
    #[cfg(target_os = "linux")]
    if let Some(value) = std::env::var_os("XDG_RUNTIME_DIR") {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            return Ok(path.join("cyril"));
        }
    }
    Ok(std::env::temp_dir().join("cyril-runtime"))
}

fn create_endpoint(runtime_dir: &Path) -> Result<MemoryEndpoint, cyril_memory::IpcError> {
    let mut random = [0_u8; 8];
    getrandom::fill(&mut random).map_err(cyril_memory::IpcError::Random)?;
    MemoryEndpoint::from_path(&runtime_dir.join(format!("m-{}", hex::encode(random))))
}

/// An in-process `cyril-memory` runtime plus a hand-driven status channel,
/// so `App` tests can observe real lesson injection and drive the
/// Starting → Ready transition without a child process.
#[cfg(test)]
#[expect(clippy::expect_used)]
pub(crate) mod test_support {
    use super::*;

    pub(crate) struct InProcessRuntime {
        endpoint: MemoryEndpoint,
        credential: cyril_memory::AdminCredential,
        request_timeout: Duration,
        health: HealthResponse,
        status_tx: watch::Sender<MemoryRuntimeStatus>,
        access_tx: watch::Sender<Option<ProjectAccess>>,
        status: watch::Receiver<MemoryRuntimeStatus>,
        access: watch::Receiver<Option<ProjectAccess>>,
        task: JoinHandle<Result<(), cyril_memory::RuntimeError>>,
        _roots: (tempfile::TempDir, tempfile::TempDir),
    }

    impl InProcessRuntime {
        /// Start a runtime on private roots and wait for it to report Ready.
        /// The published status starts as Ready; `set_starting` rewinds it.
        pub(crate) async fn start() -> Self {
            let data_root = tempfile::tempdir().expect("data root");
            // Unix sockets need a short path; `/tmp` keeps macOS's long
            // per-user temp roots out of the 100-byte limit.
            #[cfg(unix)]
            let runtime_root = tempfile::Builder::new()
                .prefix("cyril-m-")
                .tempdir_in("/tmp")
                .expect("runtime root");
            #[cfg(not(unix))]
            let runtime_root = tempfile::tempdir().expect("runtime root");
            let paths = MemoryPaths::prepare(Some(data_root.path())).expect("memory paths");
            let endpoint = create_endpoint(runtime_root.path()).expect("endpoint");
            let credential = cyril_memory::AdminCredential::generate().expect("credential");
            let request_timeout = Duration::from_secs(2);
            let launch = RuntimeLaunchConfig::new(
                paths,
                endpoint.clone(),
                credential.clone(),
                request_timeout,
            );
            let task = tokio::spawn(cyril_memory::run_runtime(launch));
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            let health = loop {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "in-process memory runtime did not become ready"
                );
                if let Ok(mut client) =
                    AdminClient::connect(endpoint.clone(), credential.clone(), request_timeout)
                        .await
                    && let Ok(health) = client.health().await
                    && health.status() == RuntimeHealth::Ready
                {
                    break health;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            };
            let (status_tx, status) = watch::channel(MemoryRuntimeStatus::Ready(health.clone()));
            let (access_tx, access) = watch::channel(Some(ProjectAccess {
                endpoint: endpoint.clone(),
                credential: credential.clone(),
                request_timeout,
            }));
            Self {
                endpoint,
                credential,
                request_timeout,
                health,
                status_tx,
                access_tx,
                status,
                access,
                task,
                _roots: (data_root, runtime_root),
            }
        }

        pub(crate) fn bind(&self, workspace: &Path) -> ProjectMemory {
            ProjectMemory {
                project: ProjectScope::resolve(workspace).expect("workspace resolves"),
                status: self.status.clone(),
                access: self.access.clone(),
            }
        }

        /// Publish "still starting" without touching the live runtime.
        pub(crate) fn set_starting(&self) {
            self.access_tx.send_replace(None);
            self.status_tx.send_replace(MemoryRuntimeStatus::Starting);
        }

        pub(crate) fn set_ready(&self) {
            self.access_tx.send_replace(Some(ProjectAccess {
                endpoint: self.endpoint.clone(),
                credential: self.credential.clone(),
                request_timeout: self.request_timeout,
            }));
            self.status_tx
                .send_replace(MemoryRuntimeStatus::Ready(self.health.clone()));
        }

        pub(crate) async fn client(&self) -> AdminClient {
            AdminClient::connect(
                self.endpoint.clone(),
                self.credential.clone(),
                self.request_timeout,
            )
            .await
            .expect("admin client")
        }

        pub(crate) async fn shutdown(self) {
            self.client()
                .await
                .shutdown()
                .await
                .expect("graceful shutdown");
            tokio::time::timeout(Duration::from_secs(5), self.task)
                .await
                .expect("runtime task exits")
                .expect("runtime task joins")
                .expect("runtime exits cleanly");
        }
    }
}

#[cfg(test)]
#[expect(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn executable_validation_rejects_relative_and_directory_paths() {
        let relative = validate_executable(Path::new("cyril-memory-runtime"));
        assert!(relative.is_err());
        let directory = tempfile::tempdir().expect("directory");
        assert!(validate_executable(directory.path()).is_err());
    }

    #[tokio::test]
    async fn disabled_and_invalid_states_never_launch() {
        let mut absent = MemoryRuntimeHandle::start(MemoryConfigState::Absent);
        assert_eq!(
            absent.status(),
            MemoryRuntimeStatus::Disabled(MemoryDisabledReason::Absent)
        );
        absent.shutdown().await;
    }

    #[tokio::test]
    async fn missing_runtime_is_a_failed_status_not_an_error() {
        let root = tempfile::tempdir().expect("root");
        let config_path = root.path().join("config.toml");
        std::fs::write(
            &config_path,
            format!(
                "[memory]\nenabled = true\ndata_root = {:?}\nstartup_timeout_ms = 100\nrequest_timeout_ms = 50\n",
                root.path().join("data").to_string_lossy()
            ),
        )
        .expect("config");
        let report = cyril_memory::load_config_report(&config_path);
        let MemoryConfigState::Valid(config) = report.memory().clone() else {
            panic!("valid config expected");
        };
        let handle = MemoryRuntimeHandle::start_with_options(
            config,
            root.path().join("missing-runtime"),
            root.path().join("ipc"),
        );
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        while matches!(handle.status(), MemoryRuntimeStatus::Starting)
            && tokio::time::Instant::now() < deadline
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(matches!(
            handle.status(),
            MemoryRuntimeStatus::Failed(MemoryRuntimeFailure::RuntimeExecutableUnavailable)
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn shutdown_reaps_runtime_process_group_within_bound() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().expect("root");
        let marker = root.path().join("grandchild.pid");
        let args_marker = root.path().join("args.txt");
        let credential_marker = root.path().join("credential.txt");
        let runtime = root.path().join("runtime-fixture.sh");
        std::fs::write(
            &runtime,
            format!(
                "#!/bin/sh\necho \"$#\" > '{}'\necho \"$CYRIL_MEMORY_ADMIN_CREDENTIAL\" > '{}'\nsleep 5 &\necho $! > '{}'\nwait\n",
                args_marker.display(),
                credential_marker.display(),
                marker.display()
            ),
        )
        .expect("runtime fixture");
        std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700))
            .expect("runtime mode");
        let config_path = root.path().join("config.toml");
        std::fs::write(
            &config_path,
            format!(
                "[memory]\nenabled = true\ndata_root = {:?}\nstartup_timeout_ms = 100\nrequest_timeout_ms = 50\n",
                root.path().join("data").to_string_lossy()
            ),
        )
        .expect("config");
        let report = cyril_memory::load_config_report(&config_path);
        let MemoryConfigState::Valid(config) = report.memory().clone() else {
            panic!("valid config expected");
        };
        let runtime_root = tempfile::Builder::new()
            .prefix("cyril-memory-")
            .tempdir_in("/tmp")
            .expect("short runtime root");
        let mut handle = MemoryRuntimeHandle::start_with_options(
            config,
            runtime,
            runtime_root.path().to_path_buf(),
        );
        let marker_deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        while !marker.exists() && tokio::time::Instant::now() < marker_deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            marker.exists(),
            "runtime fixture did not start: {:?}",
            handle.status()
        );
        let grandchild = std::fs::read_to_string(&marker).expect("grandchild marker");
        assert_eq!(
            std::fs::read_to_string(&args_marker)
                .expect("argument marker")
                .trim(),
            "0",
            "runtime credential or metadata leaked through argv"
        );
        assert_eq!(
            std::fs::read_to_string(&credential_marker)
                .expect("credential marker")
                .trim()
                .len(),
            64,
            "runtime credential was not a 256-bit hex environment value"
        );
        let started = tokio::time::Instant::now();
        handle.shutdown().await;
        assert!(started.elapsed() <= OUTER_SHUTDOWN_BOUND);

        let process_path = PathBuf::from(format!("/proc/{}", grandchild.trim()));
        let reap_deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        while process_path.exists() && tokio::time::Instant::now() < reap_deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            !process_path.exists(),
            "runtime grandchild survived shutdown"
        );
    }
    #[tokio::test]
    async fn closed_status_channel_transitions_once() {
        let (sender, status) = watch::channel(MemoryRuntimeStatus::Starting);
        drop(sender);
        let (_access_sender, access) = watch::channel(None);
        let mut handle = MemoryRuntimeHandle {
            status,
            access,
            status_closed: false,
            shutdown: None,
            task: None,
        };

        let terminal = handle.changed().await.expect("explicit terminal status");
        assert_eq!(terminal.status(), cyril_core::types::MemoryStatus::Failed);
        assert_eq!(
            terminal.detail(),
            Some("memory runtime status channel closed during startup")
        );
        assert!(handle.changed().await.is_none());
    }

    #[tokio::test]
    async fn project_access_tracks_runtime_lifecycle_without_exposing_admin() {
        let workspace = tempfile::tempdir().expect("workspace");
        let project = ProjectScope::resolve(workspace.path()).expect("project");
        let cases = [
            (
                MemoryRuntimeStatus::Disabled(MemoryDisabledReason::Absent),
                ProjectMemoryUnavailable::Disabled,
            ),
            (
                MemoryRuntimeStatus::Starting,
                ProjectMemoryUnavailable::Starting,
            ),
            (
                MemoryRuntimeStatus::Degraded(MemoryRuntimeFailure::RuntimeExited),
                ProjectMemoryUnavailable::Degraded,
            ),
            (
                MemoryRuntimeStatus::Failed(MemoryRuntimeFailure::SpawnFailed),
                ProjectMemoryUnavailable::Failed,
            ),
            (
                MemoryRuntimeStatus::Ready(HealthResponse::ready(
                    "test-instance".to_owned(),
                    cyril_memory::MemoryStoreVersions::new(2, 1),
                )),
                ProjectMemoryUnavailable::RuntimeLost,
            ),
        ];
        for (status, expected) in cases {
            let (_status_sender, status) = watch::channel(status);
            let (_access_sender, access) = watch::channel(None);
            let memory = ProjectMemory {
                project: project.clone(),
                status,
                access,
            };
            assert!(matches!(
                memory.list().await,
                Err(ProjectMemoryError::Unavailable(actual)) if actual == expected
            ));
        }
        let (status_sender, status) =
            watch::channel(MemoryRuntimeStatus::Ready(HealthResponse::ready(
                "closed-instance".to_owned(),
                cyril_memory::MemoryStoreVersions::new(2, 1),
            )));
        let runtime_dir = tempfile::tempdir().expect("runtime directory");
        let endpoint =
            MemoryEndpoint::from_path(runtime_dir.path()).expect("private test endpoint");
        let access_value = ProjectAccess {
            endpoint,
            credential: cyril_memory::AdminCredential::generate().expect("credential"),
            request_timeout: Duration::from_millis(10),
        };
        let (access_sender, access) = watch::channel(Some(access_value));
        let memory = ProjectMemory {
            project,
            status,
            access,
        };
        drop(status_sender);
        drop(access_sender);
        assert!(matches!(
            memory.list().await,
            Err(ProjectMemoryError::Unavailable(
                ProjectMemoryUnavailable::RuntimeLost
            ))
        ));
    }

    #[test]
    fn unavailability_reads_as_a_sentence_not_a_variant_name() {
        assert_eq!(
            ProjectMemoryError::Unavailable(ProjectMemoryUnavailable::Degraded).to_string(),
            "project memory is unavailable: memory runtime is degraded"
        );
        assert_eq!(
            ProjectMemoryError::Unavailable(ProjectMemoryUnavailable::RuntimeLost).to_string(),
            "project memory is unavailable: memory runtime is no longer running"
        );
        let starting = FirstPromptContextError::Unavailable(ProjectMemoryUnavailable::Starting);
        assert_eq!(starting.to_string(), "memory runtime is still starting");
        assert!(starting.retry_on_next_prompt());
        assert!(
            !FirstPromptContextError::Unavailable(ProjectMemoryUnavailable::Disabled)
                .retry_on_next_prompt()
        );
        assert!(
            !FirstPromptContextError::TimedOut(Duration::from_millis(250)).retry_on_next_prompt()
        );
    }

    #[tokio::test]
    async fn project_binding_reports_its_cause_and_never_falls_back() {
        let disabled = MemoryRuntimeHandle::start(MemoryConfigState::Absent);
        let workspace = tempfile::tempdir().expect("workspace");
        assert!(matches!(
            disabled.bind_project(workspace.path()),
            ProjectBinding::Disabled
        ));
        assert!(
            disabled
                .bind_project(workspace.path())
                .status_view()
                .is_none()
        );
        assert!(
            disabled
                .bind_project(workspace.path())
                .unavailable_message()
                .expect("disabled message")
                .contains("Memory is disabled")
        );

        // A runtime that is (or will be) alive: binding is attempted.
        let (_status_tx, status) = watch::channel(MemoryRuntimeStatus::Starting);
        let (_access_tx, access) = watch::channel(None);
        let handle = MemoryRuntimeHandle {
            status,
            access,
            status_closed: false,
            shutdown: None,
            task: None,
        };
        let bound = handle.bind_project(workspace.path());
        assert!(bound.memory().is_some());
        assert!(bound.unavailable_message().is_none());
        assert!(matches!(
            bound.status_view(),
            Some(cyril_core::types::MemoryProjectBinding::Bound { .. })
        ));

        // A relocated worktree: `.git` names a gitdir that no longer exists.
        let stale = tempfile::tempdir().expect("stale worktree");
        std::fs::write(
            stale.path().join(".git"),
            "gitdir: /old/primary/.git/worktrees/x\n",
        )
        .expect("stale marker");
        let unbound = handle.bind_project(stale.path());
        assert!(unbound.memory().is_none());
        let message = unbound.unavailable_message().expect("unbound message");
        assert!(
            message.starts_with(
                "Memory is unavailable for this project: could not inspect Git metadata at"
            ),
            "{message}"
        );
        let Some(cyril_core::types::MemoryProjectBinding::Unbound { reason }) =
            unbound.status_view()
        else {
            panic!("unbound status view expected");
        };
        assert!(reason.contains("/old/primary/.git/worktrees/x"), "{reason}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn in_process_runtime_serves_first_prompt_context_and_reports_starting() {
        let runtime = test_support::InProcessRuntime::start().await;
        let workspace = tempfile::tempdir().expect("workspace");
        let memory = runtime.bind(workspace.path());
        assert_eq!(
            memory.first_prompt_context().await.expect("no lessons yet"),
            None
        );
        memory
            .teach(LessonText::new("prefer boring Rust").expect("lesson"))
            .await
            .expect("teach");
        let block = memory
            .first_prompt_context()
            .await
            .expect("context")
            .expect("one lesson");
        assert!(block.starts_with("<CYRIL_LESSONS"));
        assert!(block.contains("- prefer boring Rust"));

        runtime.set_starting();
        let error = memory
            .first_prompt_context()
            .await
            .expect_err("starting is reported, not swallowed");
        assert!(error.retry_on_next_prompt(), "{error}");
        runtime.set_ready();
        assert!(
            memory
                .first_prompt_context()
                .await
                .expect("ready again")
                .is_some()
        );
        runtime.shutdown().await;
    }
}
