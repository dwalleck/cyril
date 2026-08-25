//! Private JSON envelopes and bounded length framing.

use std::io;
use std::str::FromStr;

use constant_time_eq::constant_time_eq;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::client::AdminCredential;
use crate::encoding::decode_fixed_hex;
use crate::lesson::{
    ContextBlock, LESSON_PREVIEW_CHARS, LessonId, LessonProvenance, LessonStatus, LessonText,
    LessonTrust, MAX_CONTEXT_CHARS,
};
use crate::project::ProjectScope;
use crate::protocol::{
    HealthResponse, LessonListResponse, LessonRecord, LessonRecordMetadata, MAX_FRAME_SIZE,
    MemoryErrorCode, MemoryProtocolError, MemoryRequest, MemoryResponse, PROTOCOL_VERSION,
    RuntimeHealth, TeachResponse,
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectPayload {
    project_id: String,
    display_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TeachPayload {
    project: ProjectPayload,
    text: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplacePayload {
    project: ProjectPayload,
    replaced_id: String,
    text: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InspectPayload {
    project: ProjectPayload,
    id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextPayload {
    project: ProjectPayload,
    max_chars: u16,
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
    #[serde(rename = "taught")]
    Taught {
        created: bool,
        lesson: LessonEnvelope<'a>,
    },
    #[serde(rename = "lessons")]
    Lessons {
        lessons: Vec<LessonEnvelope<'a>>,
        omitted_count: usize,
        corrupt_count: usize,
    },
    #[serde(rename = "lesson")]
    Lesson { lesson: LessonEnvelope<'a> },
    #[serde(rename = "context")]
    Context { block: Option<ContextEnvelope<'a>> },
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

#[derive(Debug, Serialize)]
struct LessonEnvelope<'a> {
    id: String,
    content: &'a str,
    provenance: &'static str,
    trust: &'static str,
    status: &'static str,
    supersedes_id: Option<String>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

#[derive(Debug, Serialize)]
struct ContextEnvelope<'a> {
    text: &'a str,
    omitted_count: usize,
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
    Health(HealthPayloadOwned),
    #[serde(rename = "taught")]
    Taught(TaughtPayloadOwned),
    #[serde(rename = "lessons")]
    Lessons(LessonsPayloadOwned),
    #[serde(rename = "lesson")]
    Lesson(LessonPayloadOwned),
    #[serde(rename = "context")]
    Context(ContextPayloadOwned),
    #[serde(rename = "shutdown")]
    Shutdown(EmptyPayloadOwned),
    #[serde(rename = "error")]
    Error(ErrorPayloadOwned),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyPayloadOwned {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HealthPayloadOwned {
    instance_id: String,
    status: String,
    protocol_version: u16,
    store_versions: Option<StoreVersionsEnvelopeOwned>,
    error: Option<ErrorEnvelopeOwned>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaughtPayloadOwned {
    created: bool,
    lesson: LessonEnvelopeOwned,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LessonsPayloadOwned {
    lessons: Vec<LessonEnvelopeOwned>,
    omitted_count: usize,
    corrupt_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LessonPayloadOwned {
    lesson: LessonEnvelopeOwned,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextPayloadOwned {
    block: Option<ContextEnvelopeOwned>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ErrorPayloadOwned {
    code: String,
    message: String,
    retryable: bool,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LessonEnvelopeOwned {
    id: String,
    content: String,
    provenance: String,
    trust: String,
    status: String,
    supersedes_id: Option<String>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextEnvelopeOwned {
    text: String,
    omitted_count: usize,
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
        MemoryRequest::Teach { project, text } => (
            "teach",
            Some(payload_value(TeachPayload {
                project: project_payload(&project, id)?,
                text: text.redacted().to_owned(),
            })?),
        ),
        MemoryRequest::Replace {
            project,
            replaced_id,
            text,
        } => (
            "replace",
            Some(payload_value(ReplacePayload {
                project: project_payload(&project, id)?,
                replaced_id: replaced_id.to_string(),
                text: text.redacted().to_owned(),
            })?),
        ),
        MemoryRequest::List { project } => {
            ("list", Some(payload_value(project_payload(&project, id)?)?))
        }
        MemoryRequest::Inspect {
            project,
            id: lesson_id,
        } => (
            "inspect",
            Some(payload_value(InspectPayload {
                project: project_payload(&project, id)?,
                id: lesson_id.to_string(),
            })?),
        ),
        MemoryRequest::Context { project, max_chars } => (
            "context",
            Some(payload_value(ContextPayload {
                project: project_payload(&project, id)?,
                max_chars,
            })?),
        ),
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

fn payload_value<T: Serialize>(payload: T) -> Result<serde_json::Value, WireError> {
    serde_json::to_value(payload)
        .map_err(|source| WireError::Io(io::Error::new(io::ErrorKind::InvalidData, source)))
}

fn project_payload(project: &ProjectScope, id: u64) -> Result<ProjectPayload, WireError> {
    let display_path =
        project
            .display_path()
            .to_str()
            .map(str::to_owned)
            .ok_or(WireError::Protocol {
                code: MemoryErrorCode::InvalidRequest,
                id: Some(id),
                version: Some(PROTOCOL_VERSION),
            })?;
    Ok(ProjectPayload {
        project_id: project.project_id().to_string(),
        display_path,
    })
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
    let decoded = decode_fixed_hex::<32>(encoded).ok_or_else(unauthorized)?;
    if !constant_time_eq(&decoded, credential.as_bytes()) {
        return Err(unauthorized());
    }
    let operation = envelope
        .operation
        .as_ref()
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| invalid_request(envelope.id, envelope.version))?;
    let invalid_field = |field: &'static str| {
        tracing::debug!(operation, field, "memory request field failed validation");
        invalid_request(envelope.id, envelope.version)
    };
    let lesson_text = |text: &str| {
        LessonText::new(text).map_err(|error| {
            tracing::debug!(operation, error = %error, "memory request lesson text rejected");
            invalid_request(envelope.id, envelope.version)
        })
    };
    let request = match operation {
        "health" => {
            require_null_payload(&envelope.payload, envelope.id, envelope.version)?;
            MemoryRequest::Health
        }
        "teach" => {
            let payload: TeachPayload =
                decode_payload(operation, envelope.payload, envelope.id, envelope.version)?;
            MemoryRequest::Teach {
                project: project_from_payload(payload.project, envelope.id, envelope.version)?,
                text: lesson_text(&payload.text)?,
            }
        }
        "replace" => {
            let payload: ReplacePayload =
                decode_payload(operation, envelope.payload, envelope.id, envelope.version)?;
            MemoryRequest::Replace {
                project: project_from_payload(payload.project, envelope.id, envelope.version)?,
                replaced_id: LessonId::from_str(&payload.replaced_id)
                    .map_err(|_| invalid_field("replaced_id"))?,
                text: lesson_text(&payload.text)?,
            }
        }
        "list" => {
            let payload: ProjectPayload =
                decode_payload(operation, envelope.payload, envelope.id, envelope.version)?;
            MemoryRequest::List {
                project: project_from_payload(payload, envelope.id, envelope.version)?,
            }
        }
        "inspect" => {
            let payload: InspectPayload =
                decode_payload(operation, envelope.payload, envelope.id, envelope.version)?;
            MemoryRequest::Inspect {
                project: project_from_payload(payload.project, envelope.id, envelope.version)?,
                id: LessonId::from_str(&payload.id).map_err(|_| invalid_field("id"))?,
            }
        }
        "context" => {
            let payload: ContextPayload =
                decode_payload(operation, envelope.payload, envelope.id, envelope.version)?;
            if payload.max_chars == 0 || payload.max_chars > MAX_CONTEXT_CHARS {
                return Err(invalid_field("max_chars"));
            }
            MemoryRequest::Context {
                project: project_from_payload(payload.project, envelope.id, envelope.version)?,
                max_chars: payload.max_chars,
            }
        }
        "shutdown" => {
            require_null_payload(&envelope.payload, envelope.id, envelope.version)?;
            MemoryRequest::Shutdown
        }
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

fn invalid_request(id: u64, version: u16) -> WireError {
    WireError::Protocol {
        code: MemoryErrorCode::InvalidRequest,
        id: Some(id),
        version: Some(version),
    }
}

fn require_null_payload(
    payload: &serde_json::Value,
    id: u64,
    version: u16,
) -> Result<(), WireError> {
    if payload.is_null() {
        Ok(())
    } else {
        Err(invalid_request(id, version))
    }
}

fn decode_payload<T: DeserializeOwned>(
    operation: &str,
    payload: serde_json::Value,
    id: u64,
    version: u16,
) -> Result<T, WireError> {
    serde_json::from_value(payload).map_err(|error| {
        tracing::debug!(operation, error = %error, "memory request payload failed to decode");
        invalid_request(id, version)
    })
}

fn project_from_payload(
    payload: ProjectPayload,
    id: u64,
    version: u16,
) -> Result<ProjectScope, WireError> {
    ProjectScope::from_wire(&payload.project_id, &payload.display_path).map_err(|error| {
        tracing::debug!(error = %error, "memory request project scope rejected");
        invalid_request(id, version)
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
        MemoryResponse::Taught(result) => ResponsePayload::Taught {
            created: result.created(),
            lesson: lesson_envelope(result.lesson()),
        },
        MemoryResponse::Lessons(result) => ResponsePayload::Lessons {
            lessons: result.lessons().iter().map(lesson_envelope).collect(),
            omitted_count: result.omitted_count(),
            corrupt_count: result.corrupt_count(),
        },
        MemoryResponse::Lesson(lesson) => ResponsePayload::Lesson {
            lesson: lesson_envelope(lesson),
        },
        MemoryResponse::Context(block) => ResponsePayload::Context {
            block: block.as_ref().map(|block| ContextEnvelope {
                text: block.text(),
                omitted_count: block.omitted_count(),
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

fn lesson_envelope(lesson: &LessonRecord) -> LessonEnvelope<'_> {
    LessonEnvelope {
        id: lesson.id().to_string(),
        content: lesson.content(),
        provenance: lesson.provenance().as_str(),
        trust: lesson.trust().as_str(),
        status: lesson.status().as_str(),
        supersedes_id: lesson.supersedes_id().map(|id| id.to_string()),
        created_at_ms: lesson.created_at_ms(),
        updated_at_ms: lesson.updated_at_ms(),
    }
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
        IncomingResponsePayload::Health(value) => {
            let HealthPayloadOwned {
                instance_id,
                status,
                protocol_version,
                store_versions,
                error,
            } = value;
            if protocol_version != PROTOCOL_VERSION
                || instance_id.len() != 32
                || !instance_id.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(invalid_response("health identity"));
            }
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
            let shape_valid = match status {
                RuntimeHealth::Starting => store_versions.is_none() && error.is_none(),
                RuntimeHealth::Ready => store_versions.is_some() && error.is_none(),
                RuntimeHealth::Failed => store_versions.is_none() && error.is_some(),
            };
            if !shape_valid {
                return Err(invalid_response("health shape"));
            }
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
        IncomingResponsePayload::Taught(value) => Ok(MemoryResponse::Taught(TeachResponse::new(
            decode_lesson_record(value.lesson, LessonContent::Full)?,
            value.created,
        ))),
        IncomingResponsePayload::Lessons(value) => {
            let LessonsPayloadOwned {
                lessons,
                omitted_count,
                corrupt_count,
            } = value;
            if lessons.len() > 100 {
                return Err(invalid_response("lesson list length"));
            }
            let lessons = lessons
                .into_iter()
                .map(|lesson| decode_lesson_record(lesson, LessonContent::Preview))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(MemoryResponse::Lessons(LessonListResponse::new(
                lessons,
                omitted_count,
                corrupt_count,
            )))
        }
        IncomingResponsePayload::Lesson(value) => Ok(MemoryResponse::Lesson(decode_lesson_record(
            value.lesson,
            LessonContent::Full,
        )?)),
        IncomingResponsePayload::Context(value) => {
            let block = value
                .block
                .map(|block| ContextBlock::from_wire(block.text, block.omitted_count))
                .transpose()
                .map_err(|_| invalid_response("context block"))?;
            Ok(MemoryResponse::Context(block))
        }
        IncomingResponsePayload::Shutdown(_) => Ok(MemoryResponse::Shutdown),
        IncomingResponsePayload::Error(value) => {
            let ErrorPayloadOwned {
                code,
                message,
                retryable,
            } = value;
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

/// What a lesson envelope's `content` carries, which decides how it is checked.
#[derive(Clone, Copy)]
enum LessonContent {
    /// Complete lesson text (teach, replace, inspect).
    Full,
    /// A `list` row: a prefix of at most [`LESSON_PREVIEW_CHARS`] characters,
    /// possibly cut mid-token, so it must never be re-validated as a lesson.
    Preview,
}

fn decode_lesson_record(
    value: LessonEnvelopeOwned,
    kind: LessonContent,
) -> Result<LessonRecord, WireError> {
    if value.created_at_ms < 0 || value.updated_at_ms < value.created_at_ms {
        return Err(invalid_response("lesson timestamps"));
    }
    let id = LessonId::from_str(&value.id).map_err(|_| invalid_response("lesson id"))?;
    let content = match kind {
        // Shape is checked by re-running the constructor, and the CLIENT's
        // redacted form is what gets served: a client whose redactor is
        // tighter than the runtime's redacts more, never rejects, so a
        // binary skew between the two is fail-safe rather than a hard
        // "malformed frame" on every read.
        LessonContent::Full => LessonText::new(&value.content)
            .map_err(|error| {
                tracing::debug!(error = %error, "lesson content failed validation");
                invalid_response("lesson content")
            })?
            .redacted()
            .to_owned(),
        LessonContent::Preview => {
            validate_preview(&value.content)?;
            value.content
        }
    };
    let provenance = LessonProvenance::from_stored(&value.provenance)
        .ok_or_else(|| invalid_response("lesson provenance"))?;
    let trust =
        LessonTrust::from_stored(&value.trust).ok_or_else(|| invalid_response("lesson trust"))?;
    let status = LessonStatus::from_stored(&value.status)
        .ok_or_else(|| invalid_response("lesson status"))?;
    let supersedes_id = value
        .supersedes_id
        .as_deref()
        .map(LessonId::from_str)
        .transpose()
        .map_err(|_| invalid_response("lesson supersedes_id"))?;
    Ok(LessonRecord::new(
        id,
        content,
        LessonRecordMetadata::new(
            provenance,
            trust,
            status,
            supersedes_id,
            value.created_at_ms,
            value.updated_at_ms,
        ),
    ))
}

fn validate_preview(preview: &str) -> Result<(), WireError> {
    if preview.is_empty() {
        return Err(invalid_response("lesson preview empty"));
    }
    if preview.chars().count() > LESSON_PREVIEW_CHARS {
        return Err(invalid_response("lesson preview length"));
    }
    if preview
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return Err(invalid_response("lesson preview control character"));
    }
    Ok(())
}

fn invalid_response(field: &'static str) -> WireError {
    tracing::debug!(field, "memory response field failed validation");
    WireError::Protocol {
        code: MemoryErrorCode::MalformedFrame,
        id: None,
        version: None,
    }
}

#[cfg(test)]
#[expect(clippy::expect_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::io::duplex;

    async fn decode_raw_request(
        credential: &AdminCredential,
        value: serde_json::Value,
    ) -> Result<IncomingRequest, WireError> {
        let bytes = serde_json::to_vec(&value).expect("request JSON");
        let (mut writer, reader) = duplex(bytes.len() + 4);
        writer
            .write_all(&(u32::try_from(bytes.len()).expect("bounded length")).to_be_bytes())
            .await
            .expect("length");
        writer.write_all(&bytes).await.expect("payload");
        drop(writer);
        let mut stream: BoxedStream = Box::new(reader);
        read_request(&mut stream, credential, None).await
    }

    fn request(
        credential: &AdminCredential,
        operation: &str,
        payload: serde_json::Value,
    ) -> serde_json::Value {
        serde_json::json!({
            "version": PROTOCOL_VERSION,
            "id": 1,
            "auth": credential.child_env_value(),
            "operation": operation,
            "payload": payload,
        })
    }

    #[tokio::test]
    async fn v1_operation_payload_matrix_is_strict_and_backward_compatible() {
        let credential = AdminCredential::generate().expect("credential");
        let root = TempDir::new().expect("workspace");
        let workspace = root.path().to_str().expect("UTF-8 fixture");
        let project = ProjectScope::resolve(root.path()).expect("project");
        let project_payload = serde_json::json!({
            "project_id": project.project_id().to_string(),
            "display_path": workspace,
        });
        let lesson_id = "00112233445566778899aabbccddeeff";
        let valid = [
            ("health", serde_json::Value::Null),
            ("shutdown", serde_json::Value::Null),
            (
                "teach",
                serde_json::json!({"project": project_payload.clone(), "text": "prefer boring Rust"}),
            ),
            (
                "replace",
                serde_json::json!({
                    "project": project_payload.clone(),
                    "replaced_id": lesson_id,
                    "text": "prefer explicit errors"
                }),
            ),
            ("list", project_payload.clone()),
            (
                "inspect",
                serde_json::json!({"project": project_payload.clone(), "id": lesson_id}),
            ),
            (
                "context",
                serde_json::json!({"project": project_payload.clone(), "max_chars": 4000}),
            ),
        ];
        for (operation, payload) in valid {
            decode_raw_request(&credential, request(&credential, operation, payload))
                .await
                .unwrap_or_else(|error| panic!("{operation} should decode: {error:?}"));
        }

        let invalid = [
            ("health", serde_json::json!({})),
            ("shutdown", serde_json::json!({})),
            ("teach", serde_json::Value::Null),
            (
                "list",
                serde_json::json!({"project_id": "wrong", "display_path": workspace}),
            ),
            (
                "list",
                serde_json::json!({
                    "project_id": project.project_id().to_string(),
                    "display_path": "relative"
                }),
            ),
            (
                "teach",
                serde_json::json!({
                    "project": project_payload.clone(),
                    "text": "valid",
                    "unexpected": true
                }),
            ),
            (
                "teach",
                serde_json::json!({"project": project_payload.clone(), "text": 7}),
            ),
            (
                "replace",
                serde_json::json!({
                    "project": project_payload.clone(),
                    "replaced_id": "wrong",
                    "text": "valid"
                }),
            ),
            (
                "inspect",
                serde_json::json!({"project": project_payload.clone(), "id": "wrong"}),
            ),
            (
                "context",
                serde_json::json!({"project": project_payload.clone(), "max_chars": 0}),
            ),
            (
                "context",
                serde_json::json!({"project": project_payload, "max_chars": 4001}),
            ),
        ];
        for (operation, payload) in invalid {
            let error = decode_raw_request(&credential, request(&credential, operation, payload))
                .await
                .expect_err(operation);
            assert_eq!(error.code(), Some(MemoryErrorCode::InvalidRequest));
        }
        let error = decode_raw_request(
            &credential,
            request(&credential, "unknown", serde_json::Value::Null),
        )
        .await
        .expect_err("unknown operation");
        assert_eq!(error.code(), Some(MemoryErrorCode::UnknownOperation));
        let removed = decode_raw_request(
            &credential,
            request(&credential, "bind_project", project_payload),
        )
        .await
        .expect_err("bind_project is not an operation");
        assert_eq!(removed.code(), Some(MemoryErrorCode::UnknownOperation));
    }

    fn lesson_json(content: &str) -> serde_json::Value {
        serde_json::json!({
            "id": "00112233445566778899aabbccddeeff",
            "content": content,
            "provenance": "user_explicit",
            "trust": "instruction",
            "status": "active",
            "supersedes_id": null,
            "created_at_ms": 1_000,
            "updated_at_ms": 1_000
        })
    }

    fn decode_payload_json(payload: serde_json::Value) -> Result<MemoryResponse, WireError> {
        let envelope: ResponseEnvelopeOwned =
            serde_json::from_value(serde_json::json!({"version": 1, "id": 1, "payload": payload}))
                .expect("response envelope shape");
        decode_response(envelope.payload)
    }

    #[test]
    fn list_previews_are_bounded_text_not_revalidated_lessons() {
        // Stored: 140×'a' + " password=[REDACTED] and more"; the 160-char
        // preview cuts inside the redaction marker. Re-running the lesson
        // constructor on that prefix used to yield a different string and
        // reject the whole list.
        let stored = format!("{} password=[REDACTED] and more", "a".repeat(140));
        let preview: String = stored
            .chars()
            .take(LESSON_PREVIEW_CHARS - 1)
            .chain(std::iter::once('…'))
            .collect();
        assert!(preview.ends_with("password=[REDACTED…"), "{preview}");
        // A preview cut right after the separator, and a raw-looking one
        // (a runtime whose redactor is looser than this client's).
        let after_separator = format!("{} password=…", "a".repeat(149));
        let looser_runtime = "token: hunter22 must never be logged".to_owned();
        for preview in [preview, after_separator, looser_runtime] {
            let decoded = decode_payload_json(serde_json::json!({
                "kind": "lessons",
                "lessons": [lesson_json(&preview)],
                "omitted_count": 2,
                "corrupt_count": 1
            }))
            .unwrap_or_else(|error| panic!("preview {preview:?} should decode: {error:?}"));
            let MemoryResponse::Lessons(list) = decoded else {
                panic!("lessons response expected");
            };
            assert_eq!(list.lessons()[0].content(), preview);
            assert_eq!(list.omitted_count(), 2);
            assert_eq!(list.corrupt_count(), 1);
        }

        for (label, preview) in [
            ("too long", "x".repeat(LESSON_PREVIEW_CHARS + 1)),
            ("empty", String::new()),
            ("control", "bad\u{0}preview".to_owned()),
        ] {
            let error = decode_payload_json(serde_json::json!({
                "kind": "lessons",
                "lessons": [lesson_json(&preview)],
                "omitted_count": 0,
                "corrupt_count": 0
            }))
            .expect_err(label);
            assert_eq!(
                error.code(),
                Some(MemoryErrorCode::MalformedFrame),
                "{label}"
            );
        }
    }

    #[test]
    fn full_lessons_are_served_through_the_client_redactor_not_rejected() {
        // A runtime binary with a looser redactor than this client serves a
        // row the client would rewrite: it is redacted on the way in, not
        // rejected as a malformed frame.
        let skewed = "use ghp_abcdefghijklmnopqrstuvwxyz1234 in CI";
        let decoded = decode_payload_json(
            serde_json::json!({"kind": "lesson", "lesson": lesson_json(skewed)}),
        )
        .expect("skewed row decodes");
        let MemoryResponse::Lesson(lesson) = decoded else {
            panic!("lesson response expected");
        };
        assert_eq!(lesson.content(), "use [REDACTED] in CI");

        let error = decode_payload_json(serde_json::json!({
            "kind": "lesson",
            "lesson": lesson_json("bad\u{0}lesson")
        }))
        .expect_err("control characters stay malformed");
        assert_eq!(error.code(), Some(MemoryErrorCode::MalformedFrame));
    }

    #[test]
    fn response_payloads_reject_drift_and_invalid_health_identity() {
        let health = |instance_id: &str, protocol_version: u16| {
            serde_json::json!({
                "version": 1,
                "id": 1,
                "payload": {
                    "kind": "health",
                    "instance_id": instance_id,
                    "status": "ready",
                    "protocol_version": protocol_version,
                    "store_versions": {"memory": 2, "knowledge": 1},
                    "error": null
                }
            })
        };
        let valid: ResponseEnvelopeOwned =
            serde_json::from_value(health("00112233445566778899aabbccddeeff", PROTOCOL_VERSION))
                .expect("strict health payload");
        assert!(matches!(
            decode_response(valid.payload),
            Ok(MemoryResponse::Health(_))
        ));

        let mut unknown = health("00112233445566778899aabbccddeeff", PROTOCOL_VERSION);
        unknown["payload"]["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<ResponseEnvelopeOwned>(unknown).is_err());
        assert!(
            serde_json::from_value::<ResponseEnvelopeOwned>(serde_json::json!({
                "version": 1,
                "id": 1,
                "payload": {"kind": "project_bound", "unexpected": true}
            }))
            .is_err()
        );

        for malformed in [
            health("", PROTOCOL_VERSION),
            health("not-a-runtime-instance", PROTOCOL_VERSION),
            health("00112233445566778899aabbccddeeff", PROTOCOL_VERSION + 1),
        ] {
            let envelope: ResponseEnvelopeOwned =
                serde_json::from_value(malformed).expect("response envelope shape");
            assert!(decode_response(envelope.payload).is_err());
        }
        let mut wrong_shape = health("00112233445566778899aabbccddeeff", PROTOCOL_VERSION);
        wrong_shape["payload"]["store_versions"] = serde_json::Value::Null;
        let envelope: ResponseEnvelopeOwned =
            serde_json::from_value(wrong_shape).expect("response envelope shape");
        assert!(decode_response(envelope.payload).is_err());
    }

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
