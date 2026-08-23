//! Private JSON envelopes and bounded length framing.

use std::io;

use constant_time_eq::constant_time_eq;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::client::AdminCredential;
use crate::protocol::{
    HealthResponse, MAX_FRAME_SIZE, MemoryErrorCode, MemoryProtocolError, MemoryRequest,
    MemoryResponse, PROTOCOL_VERSION, RuntimeHealth,
};

pub(crate) trait AsyncStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T> AsyncStream for T where T: AsyncRead + AsyncWrite + Unpin + Send {}

pub(crate) type BoxedStream = Box<dyn AsyncStream>;

#[derive(Debug)]
pub(crate) enum WireError {
    Io(io::Error),
    Closed,
    Protocol {
        code: MemoryErrorCode,
        id: Option<u64>,
        version: Option<u16>,
    },
    Response(MemoryProtocolError),
}

impl WireError {
    pub(crate) const fn code(&self) -> Option<MemoryErrorCode> {
        match self {
            Self::Protocol { code, .. } => Some(*code),
            Self::Response(error) => Some(error.code()),
            Self::Io(_) | Self::Closed => None,
        }
    }

    pub(crate) const fn request_id(&self) -> Option<u64> {
        match self {
            Self::Protocol { id, .. } => *id,
            Self::Io(_) | Self::Closed | Self::Response(_) => None,
        }
    }

    pub(crate) const fn request_version(&self) -> Option<u16> {
        match self {
            Self::Protocol { version, .. } => *version,
            Self::Io(_) | Self::Closed | Self::Response(_) => None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct IncomingRequest {
    pub(crate) id: u64,
    pub(crate) request: MemoryRequest,
}

#[derive(Debug, Serialize)]
struct RequestEnvelope<'a> {
    version: u16,
    id: u64,
    auth: &'a str,
    operation: &'a str,
    payload: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IncomingEnvelope {
    version: u16,
    id: u64,
    auth: Option<serde_json::Value>,
    operation: Option<serde_json::Value>,
    payload: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ResponseEnvelope<'a> {
    version: u16,
    id: u64,
    payload: ResponsePayload<'a>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind")]
enum ResponsePayload<'a> {
    #[serde(rename = "health")]
    Health {
        instance_id: &'a str,
        status: &'static str,
        protocol_version: u16,
        store_versions: Option<StoreVersionsEnvelope>,
        error: Option<ErrorEnvelope<'a>>,
    },
    #[serde(rename = "shutdown")]
    Shutdown,
    #[serde(rename = "error")]
    Error {
        code: &'static str,
        message: &'static str,
        retryable: bool,
    },
}

#[derive(Debug, Serialize)]
struct StoreVersionsEnvelope {
    memory: u32,
    knowledge: u32,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope<'a> {
    code: &'static str,
    message: &'a str,
    retryable: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResponseEnvelopeOwned {
    version: u16,
    id: u64,
    payload: IncomingResponsePayload,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
enum IncomingResponsePayload {
    #[serde(rename = "health")]
    Health {
        instance_id: String,
        status: String,
        protocol_version: u16,
        store_versions: Option<StoreVersionsEnvelopeOwned>,
        error: Option<ErrorEnvelopeOwned>,
    },
    #[serde(rename = "shutdown")]
    Shutdown,
    #[serde(rename = "error")]
    Error {
        code: String,
        message: String,
        retryable: bool,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoreVersionsEnvelopeOwned {
    memory: u32,
    knowledge: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ErrorEnvelopeOwned {
    code: String,
    message: String,
    retryable: bool,
}

pub(crate) async fn send_request(
    stream: &mut BoxedStream,
    credential: &AdminCredential,
    id: u64,
    request: MemoryRequest,
) -> Result<MemoryResponse, WireError> {
    let auth = credential.child_env_value();
    let (operation, payload) = match request {
        MemoryRequest::Health => ("health", None),
        MemoryRequest::Shutdown => ("shutdown", None),
    };
    let envelope = RequestEnvelope {
        version: PROTOCOL_VERSION,
        id,
        auth: &auth,
        operation,
        payload,
    };
    let bytes = serde_json::to_vec(&envelope)
        .map_err(|source| WireError::Io(io::Error::new(io::ErrorKind::InvalidData, source)))?;
    write_frame(stream, &bytes).await?;
    let bytes = read_frame(stream).await?;
    let response: ResponseEnvelopeOwned =
        serde_json::from_slice(&bytes).map_err(|_| WireError::Protocol {
            code: MemoryErrorCode::MalformedFrame,
            id: None,
            version: None,
        })?;
    if response.version != PROTOCOL_VERSION || response.id != id {
        return Err(WireError::Protocol {
            code: if response.version != PROTOCOL_VERSION {
                MemoryErrorCode::UnsupportedVersion
            } else {
                MemoryErrorCode::InvalidRequest
            },
            id: Some(response.id),
            version: Some(response.version),
        });
    }
    decode_response(response.payload)
}

pub(crate) async fn read_request(
    stream: &mut BoxedStream,
    credential: &AdminCredential,
    previous_id: Option<u64>,
) -> Result<IncomingRequest, WireError> {
    let bytes = read_frame(stream).await?;
    let envelope: IncomingEnvelope =
        serde_json::from_slice(&bytes).map_err(|_| WireError::Protocol {
            code: MemoryErrorCode::MalformedFrame,
            id: None,
            version: None,
        })?;
    if envelope.version != PROTOCOL_VERSION {
        return Err(WireError::Protocol {
            code: MemoryErrorCode::UnsupportedVersion,
            id: Some(envelope.id),
            version: Some(envelope.version),
        });
    }
    if envelope.id == 0 {
        return Err(WireError::Protocol {
            code: MemoryErrorCode::InvalidRequest,
            id: None,
            version: Some(envelope.version),
        });
    }
    if previous_id.is_some_and(|previous| envelope.id <= previous) {
        return Err(WireError::Protocol {
            code: MemoryErrorCode::DuplicateRequest,
            id: Some(envelope.id),
            version: Some(envelope.version),
        });
    }
    let unauthorized = || WireError::Protocol {
        code: MemoryErrorCode::Unauthorized,
        id: Some(envelope.id),
        version: Some(envelope.version),
    };
    let encoded = envelope
        .auth
        .as_ref()
        .and_then(serde_json::Value::as_str)
        .ok_or_else(unauthorized)?;
    let decoded = hex::decode(encoded).map_err(|_| unauthorized())?;
    if decoded.len() != 32 || !constant_time_eq(&decoded, credential.as_bytes()) {
        return Err(unauthorized());
    }
    if !envelope.payload.is_null() {
        return Err(WireError::Protocol {
            code: MemoryErrorCode::InvalidRequest,
            id: Some(envelope.id),
            version: Some(envelope.version),
        });
    }
    let operation = envelope
        .operation
        .as_ref()
        .and_then(serde_json::Value::as_str)
        .ok_or(WireError::Protocol {
            code: MemoryErrorCode::InvalidRequest,
            id: Some(envelope.id),
            version: Some(envelope.version),
        })?;
    let request = match operation {
        "health" => MemoryRequest::Health,
        "shutdown" => MemoryRequest::Shutdown,
        _ => {
            return Err(WireError::Protocol {
                code: MemoryErrorCode::UnknownOperation,
                id: Some(envelope.id),
                version: Some(envelope.version),
            });
        }
    };
    Ok(IncomingRequest {
        id: envelope.id,
        request,
    })
}

pub(crate) async fn send_response(
    stream: &mut BoxedStream,
    version: u16,
    id: u64,
    response: &MemoryResponse,
) -> Result<(), WireError> {
    let payload = match response {
        MemoryResponse::Health(health) => ResponsePayload::Health {
            instance_id: health.instance_id(),
            status: match health.status() {
                RuntimeHealth::Starting => "starting",
                RuntimeHealth::Ready => "ready",
                RuntimeHealth::Failed => "failed",
            },
            protocol_version: health.protocol_version(),
            store_versions: health
                .store_versions()
                .map(|versions| StoreVersionsEnvelope {
                    memory: versions.memory(),
                    knowledge: versions.knowledge(),
                }),
            error: health.error().map(|error| ErrorEnvelope {
                code: error.code().as_str(),
                message: error.message(),
                retryable: error.retryable(),
            }),
        },
        MemoryResponse::Shutdown => ResponsePayload::Shutdown,
        MemoryResponse::Error(error) => ResponsePayload::Error {
            code: error.code().as_str(),
            message: error.message(),
            retryable: error.retryable(),
        },
    };
    let envelope = ResponseEnvelope {
        version,
        id,
        payload,
    };
    let bytes = serde_json::to_vec(&envelope)
        .map_err(|source| WireError::Io(io::Error::new(io::ErrorKind::InvalidData, source)))?;
    write_frame(stream, &bytes).await
}

pub(crate) async fn send_protocol_error(
    stream: &mut BoxedStream,
    version: u16,
    id: u64,
    code: MemoryErrorCode,
) -> Result<(), WireError> {
    send_response(
        stream,
        version,
        id,
        &MemoryResponse::Error(MemoryProtocolError::new(code)),
    )
    .await
}

async fn write_frame(stream: &mut BoxedStream, bytes: &[u8]) -> Result<(), WireError> {
    if bytes.len() > MAX_FRAME_SIZE {
        return Err(WireError::Protocol {
            code: MemoryErrorCode::FrameTooLarge,
            id: None,
            version: None,
        });
    }
    let length = u32::try_from(bytes.len()).map_err(|_| WireError::Protocol {
        code: MemoryErrorCode::FrameTooLarge,
        id: None,
        version: None,
    })?;
    stream
        .write_all(&length.to_be_bytes())
        .await
        .map_err(WireError::Io)?;
    stream.write_all(bytes).await.map_err(WireError::Io)
}

async fn read_frame(stream: &mut BoxedStream) -> Result<Vec<u8>, WireError> {
    let mut length = [0u8; 4];
    match stream.read_exact(&mut length).await {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
            return Err(WireError::Closed);
        }
        Err(error) => return Err(WireError::Io(error)),
    }
    let length = u32::from_be_bytes(length) as usize;
    if length > MAX_FRAME_SIZE {
        return Err(WireError::Protocol {
            code: MemoryErrorCode::FrameTooLarge,
            id: None,
            version: None,
        });
    }
    let mut bytes = vec![0u8; length];
    stream.read_exact(&mut bytes).await.map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            WireError::Protocol {
                code: MemoryErrorCode::MalformedFrame,
                id: None,
                version: None,
            }
        } else {
            WireError::Io(error)
        }
    })?;
    Ok(bytes)
}

fn decode_response(payload: IncomingResponsePayload) -> Result<MemoryResponse, WireError> {
    match payload {
        IncomingResponsePayload::Health {
            instance_id,
            status,
            protocol_version,
            store_versions,
            error,
        } => {
            let status = match status.as_str() {
                "starting" => RuntimeHealth::Starting,
                "ready" => RuntimeHealth::Ready,
                "failed" => RuntimeHealth::Failed,
                _ => {
                    return Err(WireError::Protocol {
                        code: MemoryErrorCode::InvalidRequest,
                        id: None,
                        version: None,
                    });
                }
            };
            let error = error
                .map(|value| {
                    let code = MemoryErrorCode::from_wire_name(&value.code).ok_or(())?;
                    if value.message != code.safe_message() || value.retryable != code.retryable() {
                        return Err(());
                    }
                    Ok(MemoryProtocolError::new(code))
                })
                .transpose()
                .map_err(|()| WireError::Protocol {
                    code: MemoryErrorCode::Internal,
                    id: None,
                    version: None,
                })?;
            let response = HealthResponse::from_wire(
                instance_id,
                status,
                protocol_version,
                store_versions.map(|versions| {
                    crate::store::MemoryStoreVersions::from_parts(
                        versions.memory,
                        versions.knowledge,
                    )
                }),
                error,
            );
            Ok(MemoryResponse::Health(response))
        }
        IncomingResponsePayload::Shutdown => Ok(MemoryResponse::Shutdown),
        IncomingResponsePayload::Error {
            code,
            message,
            retryable,
        } => {
            let code = MemoryErrorCode::from_wire_name(&code).ok_or(WireError::Protocol {
                code: MemoryErrorCode::MalformedFrame,
                id: None,
                version: None,
            })?;
            if message != code.safe_message() || retryable != code.retryable() {
                return Err(WireError::Protocol {
                    code: MemoryErrorCode::MalformedFrame,
                    id: None,
                    version: None,
                });
            }
            Err(WireError::Response(MemoryProtocolError::new(code)))
        }
    }
}

#[cfg(test)]
#[expect(clippy::expect_used)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    #[tokio::test]
    async fn exact_cap_frame_is_accepted() {
        let (mut left, right) = duplex(MAX_FRAME_SIZE + 4);
        let bytes = vec![b'x'; MAX_FRAME_SIZE];
        let writer = tokio::spawn(async move {
            let length = (MAX_FRAME_SIZE as u32).to_be_bytes();
            left.write_all(&length).await.expect("length");
            left.write_all(&bytes).await.expect("frame");
        });
        let mut boxed: BoxedStream = Box::new(right);
        let decoded = read_frame(&mut boxed).await.expect("exact cap");
        assert_eq!(decoded.len(), MAX_FRAME_SIZE);
        writer.await.expect("writer");
    }

    #[tokio::test]
    async fn cap_plus_one_is_rejected_before_allocation() {
        let (mut left, right) = duplex(16);
        left.write_all(&((MAX_FRAME_SIZE as u32 + 1).to_be_bytes()))
            .await
            .expect("length");
        let mut boxed: BoxedStream = Box::new(right);
        let error = read_frame(&mut boxed).await.expect_err("oversized frame");
        assert_eq!(error.code(), Some(MemoryErrorCode::FrameTooLarge));
    }
}
