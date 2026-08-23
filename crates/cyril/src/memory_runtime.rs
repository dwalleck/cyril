use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use cyril_memory::{
    AdminClient, HealthResponse, MemoryConfig, MemoryConfigState, MemoryEndpoint, MemoryErrorCode,
    MemoryPaths, RuntimeHealth, RuntimeLaunchConfig,
};
use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;

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

pub(crate) struct MemoryRuntimeHandle {
    status: watch::Receiver<MemoryRuntimeStatus>,
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
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            supervise(config, options, status_tx, shutdown_rx).await;
        });
        Self {
            status,
            shutdown: Some(shutdown_tx),
            task: Some(task),
        }
    }

    fn terminal(status_value: MemoryRuntimeStatus) -> Self {
        let (_status_tx, status) = watch::channel(status_value);
        Self {
            status,
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

    pub(crate) async fn changed(&mut self) -> Option<cyril_core::types::MemoryStatusView> {
        self.status.changed().await.ok()?;
        Some(self.status_view())
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
        send_status(&status, MemoryRuntimeStatus::Ready(health));
    }

    let wait = child.wait();
    tokio::select! {
        exit = wait => {
            if was_ready {
                tracing::warn!(?exit, "memory runtime exited after readiness");
                send_status(
                    &status,
                    MemoryRuntimeStatus::Degraded(MemoryRuntimeFailure::RuntimeExited),
                );
            }
        }
        _ = &mut shutdown => {
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
}
