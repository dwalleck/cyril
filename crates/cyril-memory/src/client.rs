use std::ffi::OsStr;
use std::fmt;
use std::time::Duration;

use thiserror::Error;

use crate::ipc::{self, IpcError, MemoryEndpoint};
use crate::lesson::{ContextBlock, LessonId, LessonText};
use crate::project::ProjectScope;
use crate::protocol::{
    HealthResponse, LessonListResponse, LessonRecord, MemoryProtocolError, MemoryRequest,
    MemoryResponse, TeachResponse,
};
use crate::wire::{BoxedStream, WireError};

/// The orchestration-only 256-bit runtime administrator credential.
#[derive(Clone, PartialEq, Eq)]
pub struct AdminCredential([u8; 32]);

impl AdminCredential {
    pub fn generate() -> Result<Self, ClientError> {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).map_err(ClientError::Random)?;
        Ok(Self(bytes))
    }

    pub(crate) fn from_child_env(value: &OsStr) -> Result<Self, ClientError> {
        let value = value.to_str().ok_or(ClientError::InvalidCredential)?;
        let decoded = hex::decode(value).map_err(|_| ClientError::InvalidCredential)?;
        let bytes: [u8; 32] = decoded
            .try_into()
            .map_err(|_| ClientError::InvalidCredential)?;
        Ok(Self(bytes))
    }

    /// Encode the secret exclusively for the child environment.
    pub fn child_env_value(&self) -> String {
        hex::encode(self.0)
    }

    pub(crate) fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for AdminCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AdminCredential([REDACTED])")
    }
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("could not generate runtime credential")]
    Random(#[source] getrandom::Error),
    #[error("runtime credential environment value is invalid")]
    InvalidCredential,
    #[error(transparent)]
    Ipc(#[from] IpcError),
    #[error("memory runtime request timed out")]
    Timeout,
    #[error("memory runtime connection closed")]
    Closed,
    #[error("memory runtime I/O failed: {0}")]
    Io(#[source] std::io::Error),
    #[error("memory runtime protocol failed: {0}")]
    Protocol(#[source] MemoryProtocolError),
    #[error("memory runtime returned the wrong response kind")]
    UnexpectedResponse,
}

/// Typed administrator client over the single versioned memory protocol seam.
pub struct AdminClient {
    stream: BoxedStream,
    credential: AdminCredential,
    timeout: Duration,
    next_id: u64,
}

impl AdminClient {
    pub async fn connect(
        endpoint: MemoryEndpoint,
        credential: AdminCredential,
        timeout: Duration,
    ) -> Result<Self, ClientError> {
        if timeout.is_zero() {
            return Err(ClientError::Timeout);
        }
        let stream = tokio::time::timeout(timeout, ipc::connect(&endpoint))
            .await
            .map_err(|_| ClientError::Timeout)??;
        Ok(Self {
            stream,
            credential,
            timeout,
            next_id: 1,
        })
    }

    pub async fn health(&mut self) -> Result<HealthResponse, ClientError> {
        match self.request(MemoryRequest::Health).await? {
            MemoryResponse::Health(health) => Ok(health),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    pub async fn shutdown(&mut self) -> Result<(), ClientError> {
        match self.request(MemoryRequest::Shutdown).await? {
            MemoryResponse::Shutdown => Ok(()),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    pub async fn bind_project(&mut self, project: &ProjectScope) -> Result<(), ClientError> {
        match self
            .request(MemoryRequest::BindProject {
                project: project.clone(),
            })
            .await?
        {
            MemoryResponse::ProjectBound => Ok(()),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    pub async fn teach(
        &mut self,
        project: &ProjectScope,
        text: LessonText,
    ) -> Result<TeachResponse, ClientError> {
        match self
            .request(MemoryRequest::Teach {
                project: project.clone(),
                text,
            })
            .await?
        {
            MemoryResponse::Taught(response) => Ok(response),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    pub async fn replace(
        &mut self,
        project: &ProjectScope,
        replaced_id: LessonId,
        text: LessonText,
    ) -> Result<TeachResponse, ClientError> {
        match self
            .request(MemoryRequest::Replace {
                project: project.clone(),
                replaced_id,
                text,
            })
            .await?
        {
            MemoryResponse::Taught(response) => Ok(response),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    pub async fn list(
        &mut self,
        project: &ProjectScope,
    ) -> Result<LessonListResponse, ClientError> {
        match self
            .request(MemoryRequest::List {
                project: project.clone(),
            })
            .await?
        {
            MemoryResponse::Lessons(response) => Ok(response),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    pub async fn inspect(
        &mut self,
        project: &ProjectScope,
        id: LessonId,
    ) -> Result<LessonRecord, ClientError> {
        match self
            .request(MemoryRequest::Inspect {
                project: project.clone(),
                id,
            })
            .await?
        {
            MemoryResponse::Lesson(response) => Ok(response),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    pub async fn context(
        &mut self,
        project: &ProjectScope,
        max_chars: u16,
    ) -> Result<Option<ContextBlock>, ClientError> {
        match self
            .request(MemoryRequest::Context {
                project: project.clone(),
                max_chars,
            })
            .await?
        {
            MemoryResponse::Context(response) => Ok(response),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    async fn request(&mut self, request: MemoryRequest) -> Result<MemoryResponse, ClientError> {
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(ClientError::UnexpectedResponse)?;
        let response = tokio::time::timeout(
            self.timeout,
            crate::wire::send_request(&mut self.stream, &self.credential, id, request),
        )
        .await
        .map_err(|_| ClientError::Timeout)?
        .map_err(map_wire_error)?;
        Ok(response)
    }
}

fn map_wire_error(error: WireError) -> ClientError {
    match error {
        WireError::Io(source) => ClientError::Io(source),
        WireError::Closed => ClientError::Closed,
        WireError::Response(error) => ClientError::Protocol(error),
        WireError::Protocol { code, .. } => ClientError::Protocol(MemoryProtocolError::new(code)),
    }
}

#[cfg(test)]
#[expect(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn credential_roundtrips_only_through_child_environment() {
        let credential = AdminCredential::generate().expect("credential");
        let encoded = credential.child_env_value();
        let decoded = AdminCredential::from_child_env(OsStr::new(&encoded)).expect("decode");
        assert_eq!(credential, decoded);
        assert_eq!(encoded.len(), 64);
        assert!(!format!("{credential:?}").contains(&encoded));
    }

    #[test]
    fn credentials_are_fresh() {
        let first = AdminCredential::generate().expect("first");
        let second = AdminCredential::generate().expect("second");
        assert_ne!(first, second);
    }
}
