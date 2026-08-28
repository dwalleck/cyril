use std::ffi::OsStr;
use std::fmt;
use std::time::Duration;

use thiserror::Error;

use crate::encoding::decode_fixed_hex;
use crate::ipc::{self, IpcError, MemoryEndpoint};
use crate::lesson::{LessonId, LessonText};
use crate::project::ProjectScope;
use crate::protocol::{
    HealthResponse, LessonListResponse, LessonRecord, MemoryProtocolError, MemoryRequest,
    MemoryResponse, PromptContext, SourceTurnListResponse, SourceTurnRecord, TeachResponse,
};
use crate::source_turn::{CaptureBatch, PromptQuery, SourceTurnId};
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
        decode_fixed_hex::<32>(value)
            .map(Self)
            .ok_or(ClientError::InvalidCredential)
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

    /// Prepare first-prompt context for `prompt`, the user's original first
    /// block. Any text is accepted: the query is normalized here so a pasted
    /// stack trace or a long prompt can never be rejected by the runtime.
    pub async fn prepare_prompt(
        &mut self,
        project: &ProjectScope,
        prompt: &str,
    ) -> Result<Option<PromptContext>, ClientError> {
        match self
            .request(MemoryRequest::PreparePrompt {
                project: project.clone(),
                query: PromptQuery::from_prompt(prompt),
            })
            .await?
        {
            MemoryResponse::Prompt(response) => Ok(response),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    pub async fn capture_batch(
        &mut self,
        project: &ProjectScope,
        batch: CaptureBatch,
    ) -> Result<(), ClientError> {
        match self
            .request(MemoryRequest::CaptureBatch {
                project: project.clone(),
                batch,
            })
            .await?
        {
            MemoryResponse::Captured => Ok(()),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    pub async fn list_turns(
        &mut self,
        project: &ProjectScope,
    ) -> Result<SourceTurnListResponse, ClientError> {
        match self
            .request(MemoryRequest::ListTurns {
                project: project.clone(),
            })
            .await?
        {
            MemoryResponse::Turns(response) => Ok(response),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    pub async fn inspect_turn(
        &mut self,
        project: &ProjectScope,
        id: SourceTurnId,
    ) -> Result<SourceTurnRecord, ClientError> {
        match self
            .request(MemoryRequest::InspectTurn {
                project: project.clone(),
                id,
            })
            .await?
        {
            MemoryResponse::Turn(response) => Ok(response),
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
