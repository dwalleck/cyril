use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use cyril_memory::ClientError;
use cyril_memory::{
    AdminClient, AdminCredential, CaptureBatch, HealthResponse, IpcError, LessonStatus, LessonText,
    MemoryEndpoint, MemoryErrorCode, MemoryPaths, PROTOCOL_VERSION, ProjectScope, RuntimeHealth,
    RuntimeLaunchConfig, SourceSessionId, SourceTurnDisposition, SourceTurnEvent,
    SourceTurnEventKind, SourceTurnId,
};
use tempfile::TempDir;
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};

#[path = "../../../.cyril-u62f/probe.rs"]
mod phase_probe;
use phase_probe::Phase;

// These wrappers observe the existing drop order; they do not reorder roots,
// add a failure-path wait, or repair the separate kill/reap hazards.
struct ObservedRoot {
    root: Option<TempDir>,
    name: &'static str,
}

impl ObservedRoot {
    fn new(root: TempDir, name: &'static str) -> Self {
        phase_probe::event("owned_root", (name, root.path()));
        Self {
            root: Some(root),
            name,
        }
    }

    fn path(&self) -> &Path {
        match self.root.as_ref() {
            Some(root) => root.path(),
            None => panic!("owned root"),
        }
    }

    fn into_inner(mut self) -> TempDir {
        match self.root.take() {
            Some(root) => root,
            None => panic!("transferred root"),
        }
    }
}

impl Drop for ObservedRoot {
    fn drop(&mut self) {
        if let Some(root) = self.root.take() {
            if phase_probe::enabled() {
                let phase = Phase::new(self.name);
                match root.close() {
                    Ok(()) => phase.finish("ok"),
                    Err(error) => phase.finish(error),
                }
            } else {
                drop(root);
            }
        }
    }
}

struct ObservedChild(Option<Child>);

impl std::ops::Deref for ObservedChild {
    type Target = Child;

    fn deref(&self) -> &Self::Target {
        match self.0.as_ref() {
            Some(child) => child,
            None => panic!("owned child"),
        }
    }
}

impl std::ops::DerefMut for ObservedChild {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self.0.as_mut() {
            Some(child) => child,
            None => panic!("owned child"),
        }
    }
}

impl Drop for ObservedChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            if phase_probe::enabled() {
                phase_probe::event("child_status_at_guard_drop", (child.id(), child.try_wait()));
                let phase = Phase::new("child_kill_on_drop_guard_not_reap");
                drop(child);
                phase.finish("guard dropped; no exit acknowledgement implied");
            } else {
                drop(child);
            }
        }
    }
}

struct ReadinessError {
    attempt: u64,
    occurrence_us: u128,
    error: ClientError,
}

impl ReadinessError {
    fn emit(&self, event: &'static str) {
        // Only typed categories/codes are formatted, never an error's private payload.
        let (kind, io_kind, os_code, protocol_code) = match &self.error {
            ClientError::Timeout => ("timeout", None, None, None),
            ClientError::Closed => ("closed", None, None, None),
            ClientError::Ipc(IpcError::Connect { source }) => (
                "ipc_connect",
                Some(source.kind()),
                source.raw_os_error(),
                None,
            ),
            ClientError::Ipc(_) => ("ipc_other", None, None, None),
            ClientError::Io(source) => ("io", Some(source.kind()), source.raw_os_error(), None),
            ClientError::Protocol(error) => ("protocol", None, None, Some(error.code())),
            ClientError::Random(_) => ("random", None, None, None),
            ClientError::InvalidCredential => ("invalid_credential", None, None, None),
            ClientError::UnexpectedResponse => ("unexpected_response", None, None, None),
        };
        phase_probe::event(
            event,
            (
                self.attempt,
                self.occurrence_us,
                kind,
                io_kind,
                os_code,
                protocol_code,
            ),
        );
    }
}

#[derive(Debug, Default)]
struct ReadinessFailureCounts {
    timeout: u64,
    closed: u64,
    ipc: u64,
    io: u64,
    protocol: u64,
    other: u64,
}

impl ReadinessFailureCounts {
    fn record(&mut self, error: &ClientError) {
        match error {
            ClientError::Timeout => self.timeout += 1,
            ClientError::Closed => self.closed += 1,
            ClientError::Ipc(_) => self.ipc += 1,
            ClientError::Io(_) => self.io += 1,
            ClientError::Protocol(_) => self.protocol += 1,
            ClientError::Random(_)
            | ClientError::InvalidCredential
            | ClientError::UnexpectedResponse => self.other += 1,
        }
    }
}

#[derive(Default)]
struct ReadinessErrors {
    started: Option<Instant>,
    origin_utc_sample_and_bracket_us: Option<(u128, u128, u128)>,
    attempts: u64,
    connect_successes: u64,
    health_attempts: u64,
    ready_responses: u64,
    failed_responses: u64,
    other_health_responses: u64,
    connect_counts: ReadinessFailureCounts,
    health_counts: ReadinessFailureCounts,
    first_connect: Option<ReadinessError>,
    last_connect: Option<ReadinessError>,
    first_health: Option<ReadinessError>,
    last_health: Option<ReadinessError>,
    first_connect_success: Option<(u64, u128)>,
    first_health_start: Option<(u64, u128)>,
    first_health_end: Option<(u64, u128)>,
    first_ready: Option<(u64, u128)>,
}

impl ReadinessErrors {
    fn offset_us(&self) -> u128 {
        self.started
            .map_or(0, |started| started.elapsed().as_micros())
    }

    fn record_connect(&mut self, occurrence_us: u128, error: ClientError) {
        self.connect_counts.record(&error);
        let error = ReadinessError {
            attempt: self.attempts,
            occurrence_us,
            error,
        };
        if self.first_connect.is_none() {
            self.first_connect = Some(error);
        } else {
            self.last_connect = Some(error);
        }
    }

    fn record_health(&mut self, occurrence_us: u128, error: ClientError) {
        self.health_counts.record(&error);
        let error = ReadinessError {
            attempt: self.attempts,
            occurrence_us,
            error,
        };
        if self.first_health.is_none() {
            self.first_health = Some(error);
        } else {
            self.last_health = Some(error);
        }
    }
}

impl Drop for ReadinessErrors {
    fn drop(&mut self) {
        // Deferred until polling stops, including return/error/unwind paths.
        // A sole retained error serves both first/last roles by borrow.
        if let Some(error) = &self.first_connect {
            error.emit("first_connect_error");
        }
        if let Some(error) = self.last_connect.as_ref().or(self.first_connect.as_ref()) {
            error.emit("last_connect_error");
        }
        if let Some(error) = &self.first_health {
            error.emit("first_health_error");
        }
        if let Some(error) = self.last_health.as_ref().or(self.first_health.as_ref()) {
            error.emit("last_health_error");
        }
        phase_probe::event(
            "parent_readiness_attempt_counts",
            (
                self.attempts,
                self.connect_successes,
                self.health_attempts,
                self.ready_responses,
                self.failed_responses,
                self.other_health_responses,
                &self.connect_counts,
                &self.health_counts,
            ),
        );
        phase_probe::event(
            "parent_readiness_first_occurrences",
            (
                self.first_connect_success,
                self.first_health_start,
                self.first_health_end,
                self.first_ready,
            ),
        );
        phase_probe::event(
            "parent_readiness_origin_utc_sample_and_monotonic_bracket_us",
            self.origin_utc_sample_and_bracket_us,
        );
    }
}

fn temporary_root() -> Result<TempDir> {
    Phase::new("parent_temporary_root_create")
        .result(TempDir::new())
        .map_err(Into::into)
}

fn run_in_observed_executor(future: impl Future<Output = Result<()>>) -> Result<()> {
    phase_probe::identity("fixture_executor");
    let build_phase = Phase::new("parent_executor_build");
    let executor = build_phase.result(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build(),
    )?;
    let body_phase = Phase::new("parent_test_body");
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| executor.block_on(future)));
    body_phase.finish(match &result {
        Ok(Ok(())) => "ok",
        Ok(Err(_)) => "error",
        Err(_) => "panic",
    });
    let drop_phase = Phase::new("parent_executor_drop");
    drop(executor);
    drop_phase.finish("executor ended");
    phase_probe::event("parent_executor_end", ());
    match result {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

struct ObservedTestResult(Result<()>);

impl std::process::Termination for ObservedTestResult {
    fn report(self) -> std::process::ExitCode {
        let report_phase = Phase::new("parent_result_report_error_format_and_stderr");
        let code = std::process::Termination::report(self.0);
        report_phase.finish(code);
        code
    }
}

const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const ATTEMPT_TIMEOUT: Duration = Duration::from_millis(250);
// Only the Unix-socket raw-wire helpers use this; Windows builds it dead otherwise.
#[cfg(unix)]
const RAW_TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(unix)]
const FRAME_CAP: usize = 1_048_576;

struct RunningRuntime {
    data_root: ObservedRoot,
    runtime_root: ObservedRoot,
    // Read only by the Unix-socket raw-wire tests.
    #[cfg(unix)]
    endpoint_path: PathBuf,
    endpoint: MemoryEndpoint,
    credential: AdminCredential,
    child: ObservedChild,
}

impl RunningRuntime {
    async fn start() -> Result<Self> {
        Self::start_in_roots(temporary_root()?, temporary_root()?).await
    }

    async fn start_in_roots(data_root: TempDir, runtime_root: TempDir) -> Result<Self> {
        let start_phase = Phase::new("parent_setup_and_start");
        let data_root = ObservedRoot::new(data_root, "parent_data_root_close");
        let runtime_root = ObservedRoot::new(runtime_root, "parent_runtime_root_close");
        let prepare_phase = Phase::new("parent_memory_paths_prepare");
        let data_paths = prepare_phase.result(MemoryPaths::prepare(Some(data_root.path())))?;
        let endpoint_path = runtime_root.path().join("memory.sock");
        let endpoint_phase = Phase::new("parent_endpoint_create");
        let endpoint = endpoint_phase.result(MemoryEndpoint::from_path(&endpoint_path))?;
        let credential_phase = Phase::new("parent_credential_generate");
        let credential = credential_phase.result(AdminCredential::generate())?;
        let config = RuntimeLaunchConfig::new(
            data_paths,
            endpoint.clone(),
            credential.clone(),
            ATTEMPT_TIMEOUT,
        );

        let mut command = Command::new(env!("CARGO_BIN_EXE_cyril-memory-runtime"));
        config.apply_to_command(&mut command);
        #[cfg(windows)]
        let post_bind_probe = phase_probe::enabled()
            && std::env::var_os("CYRIL_U62F_POST_BIND_DELAY_500MS")
                .is_some_and(|value| value == "1")
            && std::env::var_os("NEXTEST_TEST_NAME").is_some_and(|value| {
                value == "real_process_reports_ready_v3_and_exact_store_schema"
            });
        #[cfg(not(windows))]
        let post_bind_probe = false;
        if post_bind_probe {
            // The post-bind scenario never combines its input with the auth fault,
            // even if a stale outer environment still contains that old flag.
            command.env_remove("CYRIL_U62F_READY_AUTH_MISMATCH");
            phase_probe::event("controlled_post_bind_auth_mismatch_disabled", true);
        }
        if !post_bind_probe
            && phase_probe::enabled()
            && std::env::var_os("CYRIL_U62F_READY_AUTH_MISMATCH").is_some_and(|value| value == "1")
            && std::env::var_os("NEXTEST_TEST_NAME").is_some_and(|value| {
                value == "real_process_reports_ready_v3_and_exact_store_schema"
            })
        {
            // Copy only the already-encoded child value; never change the parent credential.
            let mut child_credential: [u8; 64] = command
                .as_std()
                .get_envs()
                .find(|(name, _)| *name == "CYRIL_MEMORY_ADMIN_CREDENTIAL")
                .and_then(|(_, value)| value)
                .and_then(|value| value.to_str())
                .context("controlled readiness fault requires an encoded child credential")?
                .as_bytes()
                .try_into()
                .context("controlled readiness fault requires a 64-byte child credential")?;
            child_credential[0] = if child_credential[0] == b'0' {
                b'1'
            } else {
                b'0'
            };
            command.env(
                "CYRIL_MEMORY_ADMIN_CREDENTIAL",
                std::str::from_utf8(&child_credential)
                    .context("controlled readiness fault requires UTF-8 credential encoding")?,
            );
            phase_probe::event("controlled_readiness_auth_mismatch", true);
        }
        command.kill_on_drop(true);
        let spawn_phase = Phase::new("parent_spawn");
        let child = spawn_phase
            .result(command.spawn())
            .context("spawn memory runtime")?;
        phase_probe::event(
            "owned_child",
            (child.id(), env!("CARGO_BIN_EXE_cyril-memory-runtime")),
        );
        let child = ObservedChild(Some(child));

        let mut runtime = Self {
            data_root,
            runtime_root,
            #[cfg(unix)]
            endpoint_path,
            endpoint,
            credential,
            child,
        };
        let readiness_phase = Phase::new("parent_readiness_wait");
        let result = runtime.wait_ready().await;
        // Do not Debug-format anyhow here: symbolizing the backtrace belongs to
        // the original post-executor Termination::report boundary below.
        readiness_phase.finish(if result.is_ok() { "ready" } else { "error" });
        result?;
        start_phase.finish("ready");
        Ok(runtime)
    }

    async fn wait_ready(&mut self) -> Result<()> {
        let started = Instant::now();
        let deadline = started + STARTUP_TIMEOUT;
        let mut connect_failures = 0;
        let mut health_failures = 0;
        let mut last_observation = None;
        let mut errors = ReadinessErrors::default();
        errors.started = phase_probe::enabled().then_some(started);
        if errors.started.is_some() {
            let before = errors.offset_us();
            let utc_us = match SystemTime::now().duration_since(UNIX_EPOCH) {
                Ok(duration) => Some(duration.as_micros()),
                Err(_) => None,
            };
            let after = errors.offset_us();
            errors.origin_utc_sample_and_bracket_us = utc_us.map(|utc| (utc, before, after));
        }
        loop {
            if let Some(status) = self.child.try_wait()? {
                let error_phase = Phase::new("parent_readiness_error_construct");
                let error = anyhow::anyhow!(
                    "memory runtime exited before readiness: {status}; last observation: {last_observation:?}"
                );
                error_phase.finish("error constructed");
                return Err(error);
            }
            if Instant::now() >= deadline {
                phase_probe::event(
                    "readiness_deadline",
                    (started.elapsed(), connect_failures, health_failures),
                );
                let error_phase = Phase::new("parent_readiness_error_construct");
                let error = anyhow::anyhow!(
                    "memory runtime did not become ready before deadline: elapsed={:?}, connect_failures={connect_failures}, health_failures={health_failures}, last_observation={last_observation:?}",
                    started.elapsed()
                );
                error_phase.finish("error constructed");
                return Err(error);
            }
            errors.attempts += 1;
            match AdminClient::connect(
                self.endpoint.clone(),
                self.credential.clone(),
                ATTEMPT_TIMEOUT,
            )
            .await
            {
                Ok(mut client) => {
                    errors.connect_successes += 1;
                    let attempt = errors.attempts;
                    let connected = errors.offset_us();
                    errors
                        .first_connect_success
                        .get_or_insert((attempt, connected));
                    errors.health_attempts += 1;
                    let health_start = errors.offset_us();
                    errors
                        .first_health_start
                        .get_or_insert((attempt, health_start));
                    let health = client.health().await;
                    let health_end = errors.offset_us();
                    errors.first_health_end.get_or_insert((attempt, health_end));
                    match health {
                        Ok(health) if health.status() == RuntimeHealth::Ready => {
                            errors.ready_responses += 1;
                            errors.first_ready = Some((attempt, errors.offset_us()));
                            return Ok(());
                        }
                        Ok(health) if health.status() == RuntimeHealth::Failed => {
                            errors.failed_responses += 1;
                            bail!(
                                "memory runtime reported failed health: {:?}",
                                health.error()
                            );
                        }
                        Ok(health) => {
                            errors.other_health_responses += 1;
                            last_observation =
                                Some(format!("health status: {:?}", health.status()));
                        }
                        Err(error) => {
                            health_failures += 1;
                            last_observation = Some(format!("health request: {error}"));
                            errors.record_health(health_end, error);
                        }
                    }
                }
                Err(error) => {
                    let occurrence_us = errors.offset_us();
                    connect_failures += 1;
                    last_observation = Some(format!("connect: {error}"));
                    errors.record_connect(occurrence_us, error);
                }
            }
            sleep(Duration::from_millis(20)).await;
        }
    }

    async fn client(&self) -> Result<AdminClient> {
        let mut client = AdminClient::connect(
            self.endpoint.clone(),
            self.credential.clone(),
            STARTUP_TIMEOUT,
        )
        .await?;
        let health = client.health().await?;
        assert_ready(&health)?;
        Ok(client)
    }

    async fn shutdown(self) -> Result<()> {
        let (data_root, runtime_root) = self.shutdown_with_roots().await?;
        drop(ObservedRoot::new(data_root, "parent_data_root_close"));
        drop(ObservedRoot::new(runtime_root, "parent_runtime_root_close"));
        Ok(())
    }

    async fn shutdown_with_roots(mut self) -> Result<(TempDir, TempDir)> {
        let shutdown_phase = Phase::new("parent_shutdown");
        let mut client = self.client().await?;
        let request_phase = Phase::new("parent_shutdown_request");
        request_phase.result(client.shutdown().await)?;
        let reap_phase = Phase::new("parent_shutdown_wait_reap");
        let status = reap_phase.result(timeout(STARTUP_TIMEOUT, self.child.wait()).await)??;
        phase_probe::event("owned_child_exit", status);
        if !status.success() {
            bail!("memory runtime exited unsuccessfully after shutdown");
        }
        shutdown_phase.finish("ok");
        Ok((self.data_root.into_inner(), self.runtime_root.into_inner()))
    }

    async fn kill_with_roots(mut self) -> Result<(TempDir, TempDir)> {
        let kill_phase = Phase::new("parent_kill_await_includes_wait");
        kill_phase.result(self.child.kill().await)?;
        let reap_phase = Phase::new("parent_post_kill_wait_reap");
        let status = reap_phase.result(timeout(STARTUP_TIMEOUT, self.child.wait()).await)??;
        phase_probe::event("owned_child_exit", status);
        Ok((self.data_root.into_inner(), self.runtime_root.into_inner()))
    }

    fn paths(&self) -> Result<MemoryPaths> {
        Ok(MemoryPaths::prepare(Some(self.data_root.path()))?)
    }
}

fn assert_ready(health: &HealthResponse) -> Result<()> {
    assert!(!health.instance_id().to_string().is_empty());
    assert_eq!(health.status(), RuntimeHealth::Ready);
    assert_eq!(health.protocol_version(), PROTOCOL_VERSION);
    let Some(versions) = health.store_versions() else {
        bail!("ready health omitted store versions");
    };
    assert_eq!(versions.memory(), 3);
    assert_eq!(versions.knowledge(), 1);
    assert!(health.error().is_none());
    Ok(())
}

#[test]
fn real_process_reports_ready_v3_and_exact_store_schema() -> ObservedTestResult {
    ObservedTestResult(run_in_observed_executor(
        real_process_reports_ready_v3_and_exact_store_schema_body(),
    ))
}

async fn real_process_reports_ready_v3_and_exact_store_schema_body() -> Result<()> {
    let runtime = RunningRuntime::start().await?;
    let paths = runtime.paths()?;
    {
        let mut client = runtime.client().await?;
        let health = client.health().await?;
        assert_ready(&health)?;
    }
    assert_store_schema(
        paths.memory_store_path(),
        3,
        &[
            "lessons",
            "memory_audit",
            "projects",
            "schema_version",
            "source_turn_audit",
            "source_turns",
            "source_turns_fts",
            "source_turns_fts_config",
            "source_turns_fts_data",
            "source_turns_fts_docsize",
            "source_turns_fts_idx",
        ],
    )?;
    assert_store_schema(paths.knowledge_store_path(), 1, &["schema_version"])?;
    runtime.shutdown().await
}

#[test]
fn real_process_shutdown_and_restart_preserve_stores() -> ObservedTestResult {
    ObservedTestResult(run_in_observed_executor(
        real_process_shutdown_and_restart_preserve_stores_body(),
    ))
}

async fn real_process_shutdown_and_restart_preserve_stores_body() -> Result<()> {
    let runtime = RunningRuntime::start_in_roots(temporary_root()?, temporary_root()?).await?;
    let (data_root, runtime_root) = runtime.shutdown_with_roots().await?;

    let restarted = RunningRuntime::start_in_roots(data_root, runtime_root).await?;
    let paths = restarted.paths()?;
    assert_store_schema(
        paths.memory_store_path(),
        3,
        &[
            "lessons",
            "memory_audit",
            "projects",
            "schema_version",
            "source_turn_audit",
            "source_turns",
            "source_turns_fts",
            "source_turns_fts_config",
            "source_turns_fts_data",
            "source_turns_fts_docsize",
            "source_turns_fts_idx",
        ],
    )?;
    assert_store_schema(paths.knowledge_store_path(), 1, &["schema_version"])?;
    restarted.shutdown().await
}

#[test]
fn real_runtime_restart_preserves_lessons_and_audit() -> ObservedTestResult {
    ObservedTestResult(run_in_observed_executor(
        real_runtime_restart_preserves_lessons_and_audit_body(),
    ))
}

async fn real_runtime_restart_preserves_lessons_and_audit_body() -> Result<()> {
    let workspace = temporary_root()?;
    let project = ProjectScope::resolve(workspace.path())?;
    let runtime = RunningRuntime::start_in_roots(temporary_root()?, temporary_root()?).await?;
    let memory_path = runtime.paths()?.memory_store_path().to_path_buf();
    let mut client = runtime.client().await?;
    let first = client
        .teach(
            &project,
            LessonText::new("use password=hunter2 only in fixtures")?,
        )
        .await?;
    assert!(first.created());
    assert_eq!(
        first.lesson().content(),
        "use password=[REDACTED] only in fixtures"
    );
    let duplicate = client
        .teach(
            &project,
            LessonText::new("use password=hunter2 only in fixtures")?,
        )
        .await?;
    assert!(!duplicate.created());
    assert_eq!(duplicate.lesson().id(), first.lesson().id());
    let replacement = client
        .replace(
            &project,
            first.lesson().id(),
            LessonText::new("prefer boring Rust")?,
        )
        .await?;
    let replacement_id = replacement.lesson().id();
    let first_id = first.lesson().id();
    drop(client);
    let (data_root, runtime_root) = runtime.shutdown_with_roots().await?;

    let restarted = RunningRuntime::start_in_roots(data_root, runtime_root).await?;
    let mut client = restarted.client().await?;
    assert_eq!(
        client.inspect(&project, first_id).await?.status(),
        LessonStatus::Invalidated
    );
    let replacement = client.inspect(&project, replacement_id).await?;
    assert_eq!(replacement.content(), "prefer boring Rust");
    let listed = client.list(&project).await?;
    assert_eq!(listed.lessons().len(), 1);
    assert_eq!(listed.lessons()[0].id(), replacement_id);
    let context = client
        .prepare_prompt(&project, "prefer boring Rust")
        .await?
        .context("context block")?;
    assert!(context.text().contains("prefer boring Rust"));
    assert!(!context.text().contains("hunter2"));

    let direct = rusqlite::Connection::open_with_flags(
        memory_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    assert_eq!(
        direct.query_row(
            "SELECT COUNT(*) FROM memory_audit WHERE project_id = ?1",
            [project.project_id().to_string()],
            |row| row.get::<_, i64>(0),
        )?,
        3
    );
    restarted.shutdown().await
}

#[test]
fn c3_real_runtime_restart_replay_is_exact_once_and_conflict_safe() -> ObservedTestResult {
    ObservedTestResult(run_in_observed_executor(
        c3_real_runtime_restart_replay_is_exact_once_and_conflict_safe_body(),
    ))
}

async fn c3_real_runtime_restart_replay_is_exact_once_and_conflict_safe_body() -> Result<()> {
    let workspace = temporary_root()?;
    let project = ProjectScope::resolve(workspace.path())?;
    let source_turn_id = SourceTurnId::from_bytes([7; 16]);
    let session_id = SourceSessionId::new("session-c3")?;
    let started = SourceTurnEvent::new(
        session_id.clone(),
        source_turn_id,
        0,
        SourceTurnEventKind::Started {
            bridge_turn_id: 0,
            started_at_ms: 1,
            block_count: 1,
        },
    )?;
    let prompt = SourceTurnEvent::new(
        session_id.clone(),
        source_turn_id,
        1,
        SourceTurnEventKind::PromptFragment {
            block_index: 0,
            fragment_index: 0,
            text: "distinctive restart decision".to_owned(),
            is_last: true,
        },
    )?;
    let assistant = SourceTurnEvent::new(
        session_id.clone(),
        source_turn_id,
        2,
        SourceTurnEventKind::AssistantFragment {
            fragment_index: 0,
            text: "keep the protocol typed".to_owned(),
        },
    )?;
    let finished = SourceTurnEvent::new(
        session_id.clone(),
        source_turn_id,
        3,
        SourceTurnEventKind::Finished {
            disposition: SourceTurnDisposition::Completed,
            finished_at_ms: 2,
        },
    )?;
    let first_batch = CaptureBatch::new(vec![started, prompt])?;
    let second_batch = CaptureBatch::new(vec![assistant, finished])?;

    let runtime = RunningRuntime::start().await?;
    let mut client = runtime.client().await?;
    client.capture_batch(&project, first_batch.clone()).await?;
    drop(client);
    let (data_root, runtime_root) = runtime.shutdown_with_roots().await?;

    let restarted = RunningRuntime::start_in_roots(data_root, runtime_root).await?;
    let mut client = restarted.client().await?;
    client.capture_batch(&project, second_batch.clone()).await?;
    client.capture_batch(&project, first_batch).await?;
    client.capture_batch(&project, second_batch).await?;
    let turns = client.list_turns(&project).await?;
    assert_eq!(
        turns.turns().len(),
        1,
        "C3: replay must keep one durable row"
    );
    assert_eq!(
        turns.turns()[0].status(),
        cyril_memory::SourceTurnStatus::Finished(SourceTurnDisposition::Completed),
        "C3: terminal replay must remain completed"
    );

    let conflict = CaptureBatch::new(vec![SourceTurnEvent::new(
        session_id,
        source_turn_id,
        1,
        SourceTurnEventKind::PromptFragment {
            block_index: 0,
            fragment_index: 0,
            text: "conflicting source".to_owned(),
            is_last: true,
        },
    )?])?;
    let Err(error) = client.capture_batch(&project, conflict).await else {
        panic!("C3: conflicting replay must fail");
    };
    assert!(
        matches!(error, ClientError::Protocol(ref error) if error.code() == MemoryErrorCode::IntegrityConflict),
        "C3: expected typed integrity conflict, got {error}"
    );
    restarted.shutdown().await
}
#[test]
fn forced_kill_allows_reopen_with_same_roots() -> ObservedTestResult {
    ObservedTestResult(run_in_observed_executor(
        forced_kill_allows_reopen_with_same_roots_body(),
    ))
}

async fn forced_kill_allows_reopen_with_same_roots_body() -> Result<()> {
    let runtime = RunningRuntime::start_in_roots(temporary_root()?, temporary_root()?).await?;
    let (data_root, _runtime_root) = runtime.kill_with_roots().await?;

    let reopened = RunningRuntime::start_in_roots(data_root, temporary_root()?)
        .await
        .context("restart runtime after forced kill")?;
    let mut client = reopened
        .client()
        .await
        .context("connect after forced kill")?;
    assert_ready(
        &client
            .health()
            .await
            .context("second health after forced kill")?,
    )?;
    drop(client);
    reopened.shutdown().await
}

#[test]
fn second_runtime_same_data_root_reports_typed_already_running() -> ObservedTestResult {
    ObservedTestResult(run_in_observed_executor(
        second_runtime_same_data_root_reports_typed_already_running_body(),
    ))
}

async fn second_runtime_same_data_root_reports_typed_already_running_body() -> Result<()> {
    let data_root = temporary_root()?;
    let first_runtime_root = temporary_root()?;
    let second_runtime_root = temporary_root()?;
    let first = RunningRuntime::start_in_roots(data_root, first_runtime_root).await?;
    let second_paths = MemoryPaths::prepare(Some(first.data_root.path()))?;
    let second_endpoint_path = second_runtime_root.path().join("memory.sock");
    let second_endpoint = MemoryEndpoint::from_path(&second_endpoint_path)?;
    let second_credential = AdminCredential::generate()?;
    let second_config = RuntimeLaunchConfig::new(
        second_paths,
        second_endpoint.clone(),
        second_credential.clone(),
        ATTEMPT_TIMEOUT,
    );
    let mut command = Command::new(env!("CARGO_BIN_EXE_cyril-memory-runtime"));
    second_config.apply_to_command(&mut command);
    command.kill_on_drop(true);
    let mut second = command.spawn()?;
    let mut client = wait_for_client(second_endpoint, second_credential).await?;
    let health = client.health().await?;
    assert_eq!(health.status(), RuntimeHealth::Failed);
    let Some(error) = health.error() else {
        bail!("failed health omitted its typed error");
    };
    assert_eq!(error.code(), MemoryErrorCode::AlreadyRunning);
    client.shutdown().await?;
    let second_status = timeout(STARTUP_TIMEOUT, second.wait()).await??;
    assert!(second_status.success());
    first.shutdown().await?;
    Ok(())
}

async fn wait_for_client(
    endpoint: MemoryEndpoint,
    credential: AdminCredential,
) -> Result<AdminClient> {
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    loop {
        if Instant::now() >= deadline {
            bail!("runtime client did not connect before deadline");
        }
        match AdminClient::connect(endpoint.clone(), credential.clone(), ATTEMPT_TIMEOUT).await {
            Ok(client) => return Ok(client),
            Err(_) => sleep(Duration::from_millis(20)).await,
        }
    }
}

fn assert_store_schema(path: &Path, version: i64, expected_tables: &[&str]) -> Result<()> {
    let connection = rusqlite::Connection::open(path)?;
    let journal_mode: String = connection.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
    assert_eq!(journal_mode.to_ascii_lowercase(), "wal");

    let table_names = {
        let mut statement = connection.prepare(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    assert_eq!(table_names, expected_tables);

    let columns = {
        let mut statement = connection.prepare("PRAGMA table_info(schema_version)")?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(5)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    assert_eq!(
        columns,
        [
            ("singleton".to_owned(), "INTEGER".to_owned(), 0, 1),
            ("version".to_owned(), "INTEGER".to_owned(), 1, 0),
        ]
    );

    let schema_sql: String = connection.query_row(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'schema_version'",
        [],
        |row| row.get(0),
    )?;
    let normalized = schema_sql
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    assert!(normalized.contains("check (singleton = 1)"));
    assert!(normalized.contains("check (version > 0)"));

    let row: (i64, i64) =
        connection.query_row("SELECT singleton, version FROM schema_version", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
    assert_eq!(row, (1, version));
    Ok(())
}

#[cfg(unix)]
mod unix_protocol {
    use super::*;
    use serde_json::{Map, Value, json};
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;

    fn request_body(
        version: u16,
        id: u64,
        auth: Option<&str>,
        operation: Option<&str>,
        payload: Value,
    ) -> Vec<u8> {
        let mut object = Map::new();
        object.insert("version".to_owned(), json!(version));
        object.insert("id".to_owned(), json!(id));
        if let Some(auth) = auth {
            object.insert("auth".to_owned(), json!(auth));
        }
        if let Some(operation) = operation {
            object.insert("operation".to_owned(), json!(operation));
        }
        object.insert("payload".to_owned(), payload);
        serde_json::to_vec(&Value::Object(object)).unwrap_or_default()
    }

    async fn send_body(path: &Path, body: &[u8]) -> Result<Option<Value>> {
        let mut stream = UnixStream::connect(path).await?;
        let length = u32::try_from(body.len()).context("test frame length exceeds u32")?;
        stream.write_all(&length.to_be_bytes()).await?;
        if !body.is_empty() {
            // Oversized-header rejection can close the peer before the body upload completes.
            // Callers still validate the exact reply and follow-up runtime health.
            match stream.write_all(body).await {
                Ok(()) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::BrokenPipe
                            | std::io::ErrorKind::ConnectionReset
                            | std::io::ErrorKind::NotConnected
                    ) => {}
                Err(error) => return Err(error.into()),
            }
        }
        read_response(&mut stream).await
    }

    async fn read_response(stream: &mut UnixStream) -> Result<Option<Value>> {
        let mut length = [0_u8; 4];
        let read = timeout(RAW_TIMEOUT, stream.read_exact(&mut length)).await;
        let Ok(Ok(_)) = read else {
            return Ok(None);
        };
        let length = u32::from_be_bytes(length) as usize;
        if length > FRAME_CAP {
            bail!("runtime returned an oversized response");
        }
        let mut body = vec![0_u8; length];
        timeout(RAW_TIMEOUT, stream.read_exact(&mut body)).await??;
        Ok(Some(serde_json::from_slice(&body)?))
    }

    fn response_code(response: Option<Value>) -> Result<String> {
        let Some(response) = response else {
            bail!("runtime closed connection without typed error response");
        };
        let payload = &response["payload"];
        assert_eq!(payload["kind"], "error");
        payload["message"]
            .as_str()
            .context("response omitted payload.message")?;
        payload["retryable"]
            .as_bool()
            .context("response omitted payload.retryable")?;
        Ok(payload["code"]
            .as_str()
            .context("response omitted payload.code")?
            .to_owned())
    }

    async fn followup_health(runtime: &RunningRuntime) -> Result<()> {
        let mut client = runtime.client().await?;
        assert_ready(&client.health().await?)?;
        Ok(())
    }

    #[cfg(target_family = "unix")]
    #[tokio::test]
    async fn unix_socket_modes_are_private() -> Result<()> {
        let runtime = RunningRuntime::start().await?;
        let runtime_mode = std::fs::metadata(runtime.runtime_root.path())?
            .permissions()
            .mode()
            & 0o777;
        let socket_mode = std::fs::metadata(&runtime.endpoint_path)?
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(runtime_mode, 0o700);
        assert_eq!(socket_mode, 0o600);
        runtime.shutdown().await
    }

    #[tokio::test]
    async fn offender_matrix_keeps_runtime_healthy() -> Result<()> {
        let runtime = RunningRuntime::start().await?;
        let auth = runtime.credential.child_env_value();

        let malformed = send_body(&runtime.endpoint_path, b"{").await?;
        assert_eq!(response_code(malformed)?, "malformed_frame");
        followup_health(&runtime).await?;

        let exact_cap = vec![b'{'; FRAME_CAP];
        let response = send_body(&runtime.endpoint_path, &exact_cap).await?;
        assert_eq!(response_code(response)?, "malformed_frame");
        followup_health(&runtime).await?;

        let oversized = vec![b' '; FRAME_CAP + 1];
        let response = send_body(&runtime.endpoint_path, &oversized).await?;
        assert_eq!(response_code(response)?, "frame_too_large");
        followup_health(&runtime).await?;

        let response = send_body(
            &runtime.endpoint_path,
            &request_body(2, 1, None, Some("health"), Value::Null),
        )
        .await?;
        assert_eq!(response_code(response)?, "unauthorized");
        followup_health(&runtime).await?;

        let wrong_auth = "0".repeat(64);
        let response = send_body(
            &runtime.endpoint_path,
            &request_body(2, 1, Some(&wrong_auth), Some("health"), Value::Null),
        )
        .await?;
        assert_eq!(response_code(response)?, "unauthorized");
        followup_health(&runtime).await?;

        let response = send_body(
            &runtime.endpoint_path,
            &request_body(2, 0, Some(&auth), Some("health"), Value::Null),
        )
        .await?;
        assert_eq!(response_code(response)?, "invalid_request");
        followup_health(&runtime).await?;

        let response = send_body(
            &runtime.endpoint_path,
            &request_body(1, 2, Some(&auth), Some("health"), Value::Null),
        )
        .await?;
        assert_eq!(response_code(response)?, "unsupported_version");
        followup_health(&runtime).await?;

        let response = send_body(
            &runtime.endpoint_path,
            &request_body(2, 3, Some(&auth), Some("unknown"), Value::Null),
        )
        .await?;
        assert_eq!(response_code(response)?, "unknown_operation");
        followup_health(&runtime).await?;

        let mut duplicate = UnixStream::connect(&runtime.endpoint_path).await?;
        let first = request_body(2, 7, Some(&auth), Some("health"), Value::Null);
        let first_len = u32::try_from(first.len())?;
        duplicate.write_all(&first_len.to_be_bytes()).await?;
        duplicate.write_all(&first).await?;
        let first_response = read_response(&mut duplicate)
            .await?
            .context("health response missing")?;
        assert_eq!(first_response["payload"]["kind"], "health");
        let second = request_body(2, 7, Some(&auth), Some("health"), Value::Null);
        let second_len = u32::try_from(second.len())?;
        duplicate.write_all(&second_len.to_be_bytes()).await?;
        duplicate.write_all(&second).await?;
        assert_eq!(
            response_code(read_response(&mut duplicate).await?)?,
            "duplicate_request"
        );
        drop(duplicate);
        followup_health(&runtime).await?;

        let mut truncated = UnixStream::connect(&runtime.endpoint_path).await?;
        truncated.write_all(&128_u32.to_be_bytes()).await?;
        truncated.write_all(b"{").await?;
        truncated.shutdown().await?;
        let truncated_response = read_response(&mut truncated).await?;
        if let Some(response) = truncated_response {
            assert_eq!(response_code(Some(response))?, "malformed_frame");
        }
        followup_health(&runtime).await?;

        drop(UnixStream::connect(&runtime.endpoint_path).await?);
        followup_health(&runtime).await?;
        let idle = UnixStream::connect(&runtime.endpoint_path).await?;
        sleep(ATTEMPT_TIMEOUT + Duration::from_millis(100)).await;
        followup_health(&runtime).await?;
        drop(idle);
        runtime.shutdown().await
    }
}

#[cfg(windows)]
#[test]
fn windows_endpoint_uses_current_user_protected_acl() -> ObservedTestResult {
    ObservedTestResult(run_in_observed_executor(
        windows_endpoint_uses_current_user_protected_acl_body(),
    ))
}

#[cfg(windows)]
async fn windows_endpoint_uses_current_user_protected_acl_body() -> Result<()> {
    let runtime = RunningRuntime::start().await?;
    assert!(runtime.endpoint.display().contains("pipe"));

    let wrong_credential = AdminCredential::generate()?;
    let mut unauthorized =
        AdminClient::connect(runtime.endpoint.clone(), wrong_credential, STARTUP_TIMEOUT).await?;
    match unauthorized.health().await {
        Err(ClientError::Protocol(error)) => {
            assert_eq!(error.code(), MemoryErrorCode::Unauthorized);
        }
        other => bail!("wrong Windows runtime credential was not rejected: {other:?}"),
    }
    drop(unauthorized);

    let mut valid = runtime.client().await?;
    assert_ready(&valid.health().await?)?;
    drop(valid);
    runtime.shutdown().await
}
