use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;
use tokio::process::Command;

use crate::client::{AdminCredential, ClientError};
use crate::ipc::{self, IpcError, MemoryEndpoint};
use crate::protocol::{
    HealthResponse, MemoryErrorCode, MemoryProtocolError, MemoryRequest, MemoryResponse,
    PROTOCOL_VERSION,
};
use crate::store::{StoreError, StoreSet};
use crate::wire::{BoxedStream, WireError};
use crate::{MemoryPaths, PathError};

const DATA_ROOT_ENV: &str = "CYRIL_MEMORY_DATA_ROOT";
const ENDPOINT_ENV: &str = "CYRIL_MEMORY_ENDPOINT";
const CREDENTIAL_ENV: &str = "CYRIL_MEMORY_ADMIN_CREDENTIAL";
const REQUEST_TIMEOUT_ENV: &str = "CYRIL_MEMORY_REQUEST_TIMEOUT_MS";

/// Validated launch state shared by the orchestrator and runtime child.
#[derive(Clone, Debug)]
pub struct RuntimeLaunchConfig {
    paths: MemoryPaths,
    endpoint: MemoryEndpoint,
    credential: AdminCredential,
    request_timeout: Duration,
}

impl RuntimeLaunchConfig {
    pub fn new(
        paths: MemoryPaths,
        endpoint: MemoryEndpoint,
        credential: AdminCredential,
        request_timeout: Duration,
    ) -> Self {
        Self {
            paths,
            endpoint,
            credential,
            request_timeout,
        }
    }

    pub fn from_env() -> Result<Self, RuntimeError> {
        let data_root = required_env(DATA_ROOT_ENV)?;
        let endpoint = required_env(ENDPOINT_ENV)?;
        let credential = required_env(CREDENTIAL_ENV)?;
        let request_timeout = required_env(REQUEST_TIMEOUT_ENV)?;
        let request_timeout = request_timeout
            .to_str()
            .ok_or(RuntimeError::InvalidEnvironment {
                variable: REQUEST_TIMEOUT_ENV,
            })?
            .parse::<u64>()
            .map_err(|source| RuntimeError::InvalidTimeout { source })?;
        if request_timeout == 0 {
            return Err(RuntimeError::ZeroTimeout);
        }
        let paths = MemoryPaths::prepare(Some(&PathBuf::from(data_root)))?;
        let endpoint = MemoryEndpoint::from_child_env(endpoint)?;
        let credential = AdminCredential::from_child_env(&credential)?;
        Ok(Self::new(
            paths,
            endpoint,
            credential,
            Duration::from_millis(request_timeout),
        ))
    }

    /// Apply secrets and locations to a child environment, never argv.
    pub fn apply_to_command(&self, command: &mut Command) {
        command
            .env(DATA_ROOT_ENV, self.paths.data_root())
            .env(ENDPOINT_ENV, self.endpoint.to_child_env())
            .env(CREDENTIAL_ENV, self.credential.child_env_value())
            .env(
                REQUEST_TIMEOUT_ENV,
                self.request_timeout.as_millis().to_string(),
            );
    }

    pub fn endpoint(&self) -> &MemoryEndpoint {
        &self.endpoint
    }

    pub fn credential(&self) -> &AdminCredential {
        &self.credential
    }

    pub fn request_timeout(&self) -> Duration {
        self.request_timeout
    }
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("required runtime environment variable `{variable}` is missing")]
    MissingEnvironment { variable: &'static str },
    #[error("runtime environment variable `{variable}` is invalid")]
    InvalidEnvironment { variable: &'static str },
    #[error("runtime request timeout is not an integer")]
    InvalidTimeout {
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("runtime request timeout must be greater than zero")]
    ZeroTimeout,
    #[error(transparent)]
    Path(#[from] PathError),
    #[error(transparent)]
    Ipc(#[from] IpcError),
    #[error(transparent)]
    Credential(#[from] ClientError),
    #[error("could not create runtime instance identity")]
    Random(#[source] getrandom::Error),
}

pub async fn run_runtime(config: RuntimeLaunchConfig) -> Result<(), RuntimeError> {
    let listener = ipc::bind(&config.endpoint).await?;
    let instance_id = random_instance_id()?;
    let stores = StoreSet::open(&config.paths);
    let (stores, health) = match stores {
        Ok(stores) => {
            let health = HealthResponse::ready(instance_id, stores.versions());
            (Some(stores), health)
        }
        Err(error) => {
            let health = HealthResponse::failed(
                instance_id,
                MemoryProtocolError::new(map_store_error(&error)),
            );
            tracing::warn!(error = %error, "memory runtime store initialization failed");
            (None, health)
        }
    };
    let _stores = stores;

    loop {
        let stream = listener.accept().await?;
        if handle_connection(stream, &config.credential, &health, config.request_timeout).await {
            return Ok(());
        }
    }
}

async fn handle_connection(
    mut stream: BoxedStream,
    credential: &AdminCredential,
    health: &HealthResponse,
    request_timeout: Duration,
) -> bool {
    let mut previous_id = None;
    loop {
        let read = match tokio::time::timeout(
            request_timeout,
            crate::wire::read_request(&mut stream, credential, previous_id),
        )
        .await
        {
            Ok(read) => read,
            Err(_) => return false,
        };
        match read {
            Ok(request) => {
                previous_id = Some(request.id);
                let (response, shutdown) = match request.request {
                    MemoryRequest::Health => (MemoryResponse::Health(health.clone()), false),
                    MemoryRequest::Shutdown => (MemoryResponse::Shutdown, true),
                };
                let send = tokio::time::timeout(
                    request_timeout,
                    crate::wire::send_response(
                        &mut stream,
                        PROTOCOL_VERSION,
                        request.id,
                        &response,
                    ),
                )
                .await;
                match send {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        log_connection_error(&error);
                        return false;
                    }
                    Err(_) => return false,
                }
                if shutdown {
                    return true;
                }
            }
            Err(WireError::Closed) => return false,
            Err(error @ WireError::Protocol { .. }) => {
                let code = error.code().unwrap_or(MemoryErrorCode::Internal);
                let id = error.request_id().unwrap_or(0);
                let version = error.request_version().unwrap_or(PROTOCOL_VERSION);
                let send = tokio::time::timeout(
                    request_timeout,
                    crate::wire::send_protocol_error(&mut stream, version, id, code),
                )
                .await;
                if let Ok(Err(write_error)) = send {
                    log_connection_error(&write_error);
                }
                return false;
            }
            Err(error) => {
                log_connection_error(&error);
                return false;
            }
        }
    }
}

fn required_env(variable: &'static str) -> Result<OsString, RuntimeError> {
    env::var_os(variable).ok_or(RuntimeError::MissingEnvironment { variable })
}

fn random_instance_id() -> Result<String, RuntimeError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(RuntimeError::Random)?;
    Ok(hex::encode(bytes))
}

fn map_store_error(error: &StoreError) -> MemoryErrorCode {
    match error {
        StoreError::AlreadyRunning { .. } => MemoryErrorCode::AlreadyRunning,
        StoreError::Permission { .. } => MemoryErrorCode::PermissionDenied,
        StoreError::Missing { .. }
        | StoreError::Unreadable { .. }
        | StoreError::Malformed { .. }
        | StoreError::Invalid { .. }
        | StoreError::Lock { .. }
        | StoreError::MissingMetadata { .. }
        | StoreError::CorruptSchema { .. }
        | StoreError::DuplicateMetadata { .. }
        | StoreError::UnsupportedSchema { .. }
        | StoreError::Sqlite { .. } => MemoryErrorCode::MigrationFailed,
    }
}

fn log_connection_error(error: &WireError) {
    match error {
        WireError::Io(source) => tracing::warn!(error = %source, "memory IPC connection failed"),
        WireError::Closed => {}
        WireError::Protocol { code, .. } => {
            tracing::debug!(code = %code, "memory IPC request rejected");
        }
        WireError::Response(error) => {
            tracing::debug!(code = %error.code(), "memory IPC response rejected");
        }
    }
}

#[cfg(test)]
#[expect(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn runtime_config_debug_redacts_credential() {
        let root = tempfile::tempdir().expect("root");
        let paths = MemoryPaths::prepare(Some(root.path())).expect("paths");
        let endpoint =
            MemoryEndpoint::from_path(&root.path().join("runtime.sock")).expect("endpoint");
        let credential = AdminCredential::generate().expect("credential");
        let encoded = credential.child_env_value();
        let config = RuntimeLaunchConfig::new(
            paths,
            endpoint.clone(),
            credential.clone(),
            Duration::from_secs(2),
        );
        assert_eq!(config.endpoint(), &endpoint);
        assert_eq!(config.credential(), &credential);
        assert_eq!(config.request_timeout(), Duration::from_secs(2));
        assert!(!format!("{config:?}").contains(&encoded));
    }
}
