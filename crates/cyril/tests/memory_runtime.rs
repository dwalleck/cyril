use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
#[cfg(windows)]
use cyril_memory::ClientError;
use cyril_memory::{
    AdminClient, AdminCredential, HealthResponse, MemoryEndpoint, MemoryErrorCode, MemoryPaths,
    PROTOCOL_VERSION, RuntimeHealth, RuntimeLaunchConfig,
};
use tempfile::TempDir;
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const ATTEMPT_TIMEOUT: Duration = Duration::from_millis(250);
const RAW_TIMEOUT: Duration = Duration::from_secs(2);
const FRAME_CAP: usize = 1_048_576;

struct RunningRuntime {
    data_root: TempDir,
    runtime_root: TempDir,
    endpoint_path: PathBuf,
    endpoint: MemoryEndpoint,
    credential: AdminCredential,
    child: Child,
}

impl RunningRuntime {
    async fn start() -> Result<Self> {
        Self::start_in_roots(TempDir::new()?, TempDir::new()?).await
    }

    async fn start_in_roots(data_root: TempDir, runtime_root: TempDir) -> Result<Self> {
        let data_paths = MemoryPaths::prepare(Some(data_root.path()))?;
        let endpoint_path = runtime_root.path().join("memory.sock");
        let endpoint = MemoryEndpoint::from_path(&endpoint_path)?;
        let credential = AdminCredential::generate()?;
        let config = RuntimeLaunchConfig::new(
            data_paths,
            endpoint.clone(),
            credential.clone(),
            ATTEMPT_TIMEOUT,
        );

        let mut command = Command::new(env!("CARGO_BIN_EXE_cyril-memory-runtime"));
        config.apply_to_command(&mut command);
        command.kill_on_drop(true);
        let child = command.spawn().context("spawn memory runtime")?;

        let mut runtime = Self {
            data_root,
            runtime_root,
            endpoint_path,
            endpoint,
            credential,
            child,
        };
        runtime.wait_ready().await?;
        Ok(runtime)
    }

    async fn wait_ready(&mut self) -> Result<()> {
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        loop {
            if Instant::now() >= deadline {
                bail!("memory runtime did not become ready before deadline");
            }
            if let Ok(mut client) = AdminClient::connect(
                self.endpoint.clone(),
                self.credential.clone(),
                ATTEMPT_TIMEOUT,
            )
            .await
                && let Ok(health) = client.health().await
                && health.status() == RuntimeHealth::Ready
            {
                return Ok(());
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
        let _roots = self.shutdown_with_roots().await?;
        Ok(())
    }

    async fn shutdown_with_roots(mut self) -> Result<(TempDir, TempDir)> {
        let mut client = self.client().await?;
        client.shutdown().await?;
        let status = timeout(STARTUP_TIMEOUT, self.child.wait()).await??;
        if !status.success() {
            bail!("memory runtime exited unsuccessfully after shutdown");
        }
        Ok((self.data_root, self.runtime_root))
    }

    async fn kill_with_roots(mut self) -> Result<(TempDir, TempDir)> {
        self.child.kill().await?;
        let _status = timeout(STARTUP_TIMEOUT, self.child.wait()).await??;
        Ok((self.data_root, self.runtime_root))
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
    assert_eq!(versions.memory(), 1);
    assert_eq!(versions.knowledge(), 1);
    assert!(health.error().is_none());
    Ok(())
}

#[tokio::test]
async fn real_process_reports_ready_v1_and_exact_store_schema() -> Result<()> {
    let runtime = RunningRuntime::start().await?;
    let paths = runtime.paths()?;
    {
        let mut client = runtime.client().await?;
        let health = client.health().await?;
        assert_ready(&health)?;
    }
    assert_store_schema(paths.memory_store_path())?;
    assert_store_schema(paths.knowledge_store_path())?;
    runtime.shutdown().await
}

#[tokio::test]
async fn real_process_shutdown_and_restart_preserve_stores() -> Result<()> {
    let runtime = RunningRuntime::start_in_roots(TempDir::new()?, TempDir::new()?).await?;
    let (data_root, runtime_root) = runtime.shutdown_with_roots().await?;

    let restarted = RunningRuntime::start_in_roots(data_root, runtime_root).await?;
    let paths = restarted.paths()?;
    assert_store_schema(paths.memory_store_path())?;
    assert_store_schema(paths.knowledge_store_path())?;
    restarted.shutdown().await
}

#[tokio::test]
async fn forced_kill_allows_reopen_with_same_roots() -> Result<()> {
    let runtime = RunningRuntime::start_in_roots(TempDir::new()?, TempDir::new()?).await?;
    let (data_root, _runtime_root) = runtime.kill_with_roots().await?;

    let reopened = RunningRuntime::start_in_roots(data_root, TempDir::new()?)
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

#[tokio::test]
async fn second_runtime_same_data_root_reports_typed_already_running() -> Result<()> {
    let data_root = TempDir::new()?;
    let first_runtime_root = TempDir::new()?;
    let second_runtime_root = TempDir::new()?;
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

fn assert_store_schema(path: &Path) -> Result<()> {
    let connection = rusqlite::Connection::open(path)?;
    let journal_mode: String = connection.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
    assert_eq!(journal_mode.to_ascii_lowercase(), "wal");

    let table_names = {
        let mut statement = connection
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    assert_eq!(table_names, ["schema_version"]);

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
    assert_eq!(row, (1, 1));
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
            match stream.write_all(body).await {
                Ok(()) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
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
            &request_body(1, 1, None, Some("health"), Value::Null),
        )
        .await?;
        assert_eq!(response_code(response)?, "unauthorized");
        followup_health(&runtime).await?;

        let wrong_auth = "0".repeat(64);
        let response = send_body(
            &runtime.endpoint_path,
            &request_body(1, 1, Some(&wrong_auth), Some("health"), Value::Null),
        )
        .await?;
        assert_eq!(response_code(response)?, "unauthorized");
        followup_health(&runtime).await?;

        let response = send_body(
            &runtime.endpoint_path,
            &request_body(1, 0, Some(&auth), Some("health"), Value::Null),
        )
        .await?;
        assert_eq!(response_code(response)?, "invalid_request");
        followup_health(&runtime).await?;

        let response = send_body(
            &runtime.endpoint_path,
            &request_body(2, 2, Some(&auth), Some("health"), Value::Null),
        )
        .await?;
        assert_eq!(response_code(response)?, "unsupported_version");
        followup_health(&runtime).await?;

        let response = send_body(
            &runtime.endpoint_path,
            &request_body(1, 3, Some(&auth), Some("unknown"), Value::Null),
        )
        .await?;
        assert_eq!(response_code(response)?, "unknown_operation");
        followup_health(&runtime).await?;

        let mut duplicate = UnixStream::connect(&runtime.endpoint_path).await?;
        let first = request_body(1, 7, Some(&auth), Some("health"), Value::Null);
        let first_len = u32::try_from(first.len())?;
        duplicate.write_all(&first_len.to_be_bytes()).await?;
        duplicate.write_all(&first).await?;
        let first_response = read_response(&mut duplicate)
            .await?
            .context("health response missing")?;
        assert_eq!(first_response["payload"]["kind"], "health");
        let second = request_body(1, 7, Some(&auth), Some("health"), Value::Null);
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
#[tokio::test]
async fn windows_endpoint_uses_current_user_protected_acl() -> Result<()> {
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
