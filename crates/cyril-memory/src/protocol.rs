//! Public, transport-independent memory runtime protocol domain types.

use crate::lesson::{LessonId, LessonProvenance, LessonStatus, LessonText, LessonTrust};
use crate::project::ProjectScope;
use crate::source_turn::{CaptureBatch, SourceSessionId, SourceTurnId};
use crate::store::MemoryStoreVersions;
use std::fmt;

/// Protocol version understood by this crate.
pub const PROTOCOL_VERSION: u16 = 2;

/// Maximum encoded request or response payload, excluding the four-byte length.
pub(crate) const MAX_FRAME_SIZE: usize = 1_048_576;

/// Operations supported by the runtime administration protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryRequest {
    /// Ask the runtime for its current health.
    Health,
    /// Persist one explicit project lesson.
    Teach {
        project: ProjectScope,
        text: LessonText,
    },
    /// Supersede one active project lesson.
    Replace {
        project: ProjectScope,
        replaced_id: LessonId,
        text: LessonText,
    },
    /// List at most 100 active project lessons.
    List { project: ProjectScope },
    /// Inspect one active or invalidated project lesson.
    Inspect { project: ProjectScope, id: LessonId },
    /// Prepare a query-aware first-prompt context block.
    PreparePrompt {
        project: ProjectScope,
        query: String,
    },
    /// Stage or commit a bounded source event batch.
    CaptureBatch {
        project: ProjectScope,
        batch: CaptureBatch,
    },
    /// List the newest completed and incomplete source turns.
    ListTurns { project: ProjectScope },
    /// Inspect one source turn by its durable identity.
    InspectTurn {
        project: ProjectScope,
        id: SourceTurnId,
    },
    /// Ask the runtime to stop after acknowledging this request.
    Shutdown,
}

/// Responses returned by the runtime administration protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryResponse {
    /// Current runtime health.
    Health(HealthResponse),
    /// Result of a teach or replacement attempt.
    Taught(TeachResponse),
    /// Bounded active lesson list.
    Lessons(LessonListResponse),
    /// Full lesson detail.
    Lesson(LessonRecord),
    /// Acknowledgement of a validated capture batch.
    Captured,
    /// Lessons and derived source episodes composed into one opaque block.
    Prompt(Option<PromptContext>),
    /// A bounded source-turn list.
    Turns(SourceTurnListResponse),
    /// One bounded source-turn inspection.
    Turn(SourceTurnRecord),
    /// Acknowledgement of an authenticated shutdown request.
    Shutdown,
    /// A bounded, typed protocol failure.
    Error(MemoryProtocolError),
}

/// Opaque prepared context. Callers may insert `text()` exactly once into a
/// prompt; they cannot inspect or rebuild lesson/episode policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptContext {
    text: String,
}

impl PromptContext {
    pub(crate) fn from_text(text: String) -> Result<Self, MemoryProtocolError> {
        if text.is_empty() || text.chars().count() > 7_601 {
            return Err(MemoryProtocolError::new(MemoryErrorCode::InvalidRequest));
        }
        Ok(Self { text })
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Durable source-turn status projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceTurnStatus {
    Incomplete,
    Completed,
    Interrupted,
    Failed,
    Abandoned,
    CaptureOverflow,
}

impl SourceTurnStatus {
    pub(crate) fn from_stored(value: &str) -> Option<Self> {
        Some(match value {
            "incomplete" => Self::Incomplete,
            "completed" => Self::Completed,
            "interrupted" => Self::Interrupted,
            "failed" => Self::Failed,
            "abandoned" => Self::Abandoned,
            "capture_overflow" => Self::CaptureOverflow,
            _ => return None,
        })
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Incomplete => "incomplete",
            Self::Completed => "completed",
            Self::Interrupted => "interrupted",
            Self::Failed => "failed",
            Self::Abandoned => "abandoned",
            Self::CaptureOverflow => "capture_overflow",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceTurnRecord {
    pub(crate) id: SourceTurnId,
    pub(crate) session_id: SourceSessionId,
    pub(crate) bridge_turn_id: u64,
    pub(crate) status: SourceTurnStatus,
    pub(crate) prompt: String,
    pub(crate) assistant: String,
    pub(crate) tools: String,
    pub(crate) source_hash: Option<[u8; 32]>,
    pub(crate) started_at_ms: i64,
    pub(crate) finished_at_ms: Option<i64>,
    pub(crate) next_sequence: u64,
}

impl SourceTurnRecord {
    pub const fn id(&self) -> SourceTurnId {
        self.id
    }
    pub fn session_id(&self) -> &SourceSessionId {
        &self.session_id
    }
    pub const fn bridge_turn_id(&self) -> u64 {
        self.bridge_turn_id
    }
    pub const fn status(&self) -> SourceTurnStatus {
        self.status
    }
    pub fn prompt(&self) -> &str {
        &self.prompt
    }
    pub fn assistant(&self) -> &str {
        &self.assistant
    }
    pub fn tools(&self) -> &str {
        &self.tools
    }
    pub const fn source_hash(&self) -> Option<[u8; 32]> {
        self.source_hash
    }
    pub const fn started_at_ms(&self) -> i64 {
        self.started_at_ms
    }
    pub const fn finished_at_ms(&self) -> Option<i64> {
        self.finished_at_ms
    }
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceTurnListResponse {
    turns: Vec<SourceTurnRecord>,
    omitted_count: usize,
    corrupt_count: usize,
}

impl SourceTurnListResponse {
    pub(crate) const fn new(
        turns: Vec<SourceTurnRecord>,
        omitted_count: usize,
        corrupt_count: usize,
    ) -> Self {
        Self {
            turns,
            omitted_count,
            corrupt_count,
        }
    }
    pub fn turns(&self) -> &[SourceTurnRecord] {
        &self.turns
    }
    pub const fn omitted_count(&self) -> usize {
        self.omitted_count
    }
    pub const fn corrupt_count(&self) -> usize {
        self.corrupt_count
    }
}

pub(crate) struct LessonRecordMetadata {
    provenance: LessonProvenance,
    trust: LessonTrust,
    status: LessonStatus,
    supersedes_id: Option<LessonId>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl LessonRecordMetadata {
    pub(crate) const fn new(
        provenance: LessonProvenance,
        trust: LessonTrust,
        status: LessonStatus,
        supersedes_id: Option<LessonId>,
        created_at_ms: i64,
        updated_at_ms: i64,
    ) -> Self {
        Self {
            provenance,
            trust,
            status,
            supersedes_id,
            created_at_ms,
            updated_at_ms,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct LessonRecord {
    id: LessonId,
    content: String,
    provenance: LessonProvenance,
    trust: LessonTrust,
    status: LessonStatus,
    supersedes_id: Option<LessonId>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl LessonRecord {
    pub(crate) fn new(id: LessonId, content: String, metadata: LessonRecordMetadata) -> Self {
        Self {
            id,
            content,
            provenance: metadata.provenance,
            trust: metadata.trust,
            status: metadata.status,
            supersedes_id: metadata.supersedes_id,
            created_at_ms: metadata.created_at_ms,
            updated_at_ms: metadata.updated_at_ms,
        }
    }

    pub const fn id(&self) -> LessonId {
        self.id
    }

    /// Lesson text. Full for teach/replace/inspect results; a preview of at
    /// most [`crate::LESSON_PREVIEW_CHARS`] characters in list rows.
    pub fn content(&self) -> &str {
        &self.content
    }

    pub const fn provenance(&self) -> LessonProvenance {
        self.provenance
    }

    pub const fn trust(&self) -> LessonTrust {
        self.trust
    }

    pub const fn status(&self) -> LessonStatus {
        self.status
    }

    pub const fn supersedes_id(&self) -> Option<LessonId> {
        self.supersedes_id
    }

    pub const fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }

    pub const fn updated_at_ms(&self) -> i64 {
        self.updated_at_ms
    }
}

impl fmt::Debug for LessonRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LessonRecord")
            .field("id", &self.id)
            .field("content", &"[REDACTED]")
            .field("provenance", &self.provenance)
            .field("trust", &self.trust)
            .field("status", &self.status)
            .field("supersedes_id", &self.supersedes_id)
            .field("created_at_ms", &self.created_at_ms)
            .field("updated_at_ms", &self.updated_at_ms)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeachResponse {
    lesson: LessonRecord,
    created: bool,
}

impl TeachResponse {
    pub(crate) const fn new(lesson: LessonRecord, created: bool) -> Self {
        Self { lesson, created }
    }

    pub const fn lesson(&self) -> &LessonRecord {
        &self.lesson
    }

    /// `true` when a new row was inserted; `false` when the text already
    /// matched an active lesson, which is the lesson returned.
    pub const fn created(&self) -> bool {
        self.created
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonListResponse {
    lessons: Vec<LessonRecord>,
    omitted_count: usize,
    corrupt_count: usize,
}

impl LessonListResponse {
    pub(crate) const fn new(
        lessons: Vec<LessonRecord>,
        omitted_count: usize,
        corrupt_count: usize,
    ) -> Self {
        Self {
            lessons,
            omitted_count,
            corrupt_count,
        }
    }

    pub fn lessons(&self) -> &[LessonRecord] {
        &self.lessons
    }

    /// Active lessons beyond the list cap.
    pub const fn omitted_count(&self) -> usize {
        self.omitted_count
    }

    /// Active rows skipped because their stored integrity check failed.
    pub const fn corrupt_count(&self) -> usize {
        self.corrupt_count
    }
}

/// Stable machine-readable protocol error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryErrorCode {
    Unauthorized,
    MalformedFrame,
    FrameTooLarge,
    UnsupportedVersion,
    UnknownOperation,
    InvalidRequest,
    DuplicateRequest,
    AlreadyRunning,
    NotFound,
    AlreadySuperseded,
    IntegrityConflict,
    CorruptLesson,
    PermissionDenied,
    MigrationFailed,
    Internal,
}

impl MemoryErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unauthorized => "unauthorized",
            Self::MalformedFrame => "malformed_frame",
            Self::FrameTooLarge => "frame_too_large",
            Self::UnsupportedVersion => "unsupported_version",
            Self::UnknownOperation => "unknown_operation",
            Self::InvalidRequest => "invalid_request",
            Self::DuplicateRequest => "duplicate_request",
            Self::AlreadyRunning => "already_running",
            Self::PermissionDenied => "permission_denied",
            Self::NotFound => "not_found",
            Self::AlreadySuperseded => "already_superseded",
            Self::IntegrityConflict => "integrity_conflict",
            Self::CorruptLesson => "corrupt_lesson",
            Self::MigrationFailed => "migration_failed",
            Self::Internal => "internal",
        }
    }

    pub(crate) fn from_wire_name(value: &str) -> Option<Self> {
        Some(match value {
            "unauthorized" => Self::Unauthorized,
            "malformed_frame" => Self::MalformedFrame,
            "frame_too_large" => Self::FrameTooLarge,
            "unsupported_version" => Self::UnsupportedVersion,
            "unknown_operation" => Self::UnknownOperation,
            "invalid_request" => Self::InvalidRequest,
            "duplicate_request" => Self::DuplicateRequest,
            "already_running" => Self::AlreadyRunning,
            "permission_denied" => Self::PermissionDenied,
            "migration_failed" => Self::MigrationFailed,
            "not_found" => Self::NotFound,
            "already_superseded" => Self::AlreadySuperseded,
            "integrity_conflict" => Self::IntegrityConflict,
            "corrupt_lesson" => Self::CorruptLesson,
            "internal" => Self::Internal,
            _ => return None,
        })
    }
    pub const fn retryable(self) -> bool {
        matches!(self, Self::AlreadyRunning | Self::Internal)
    }

    pub(crate) const fn safe_message(self) -> &'static str {
        match self {
            Self::Unauthorized => "authentication failed",
            Self::MalformedFrame => "malformed protocol frame",
            Self::FrameTooLarge => "protocol frame is too large",
            Self::UnsupportedVersion => "unsupported protocol version",
            Self::UnknownOperation => "unknown protocol operation",
            Self::InvalidRequest => "invalid protocol request",
            Self::DuplicateRequest => "duplicate protocol request",
            Self::AlreadyRunning => "memory runtime is already running",
            Self::PermissionDenied => "memory runtime permission denied",
            Self::MigrationFailed => "memory store migration failed",
            Self::NotFound => "project lesson not found",
            Self::AlreadySuperseded => "project lesson was already replaced",
            Self::IntegrityConflict => "source turn replay conflicts with immutable data",
            Self::CorruptLesson => "stored project lesson is corrupt",
            Self::Internal => "memory runtime internal error",
        }
    }
}

impl fmt::Display for MemoryErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A protocol failure that never carries a path, credential, or unbounded text.
#[derive(Clone, PartialEq, Eq)]
pub struct MemoryProtocolError {
    code: MemoryErrorCode,
}

impl MemoryProtocolError {
    pub const fn new(code: MemoryErrorCode) -> Self {
        Self { code }
    }

    pub const fn code(&self) -> MemoryErrorCode {
        self.code
    }

    pub const fn message(&self) -> &'static str {
        self.code.safe_message()
    }
    pub const fn retryable(&self) -> bool {
        self.code.retryable()
    }
}

impl fmt::Debug for MemoryProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MemoryProtocolError")
            .field("code", &self.code)
            .finish()
    }
}

impl fmt::Display for MemoryProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message())
    }
}

impl std::error::Error for MemoryProtocolError {}

/// Coarse runtime lifecycle state exposed by health responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeHealth {
    Starting,
    Ready,
    Failed,
}

/// Safe health snapshot returned by the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthResponse {
    instance_id: String,
    status: RuntimeHealth,
    protocol_version: u16,
    store_versions: Option<MemoryStoreVersions>,
    error: Option<MemoryProtocolError>,
}

impl HealthResponse {
    pub fn ready(instance_id: String, store_versions: MemoryStoreVersions) -> Self {
        Self {
            instance_id,
            status: RuntimeHealth::Ready,
            protocol_version: PROTOCOL_VERSION,
            store_versions: Some(store_versions),
            error: None,
        }
    }

    pub(crate) fn failed(instance_id: String, error: MemoryProtocolError) -> Self {
        Self {
            instance_id,
            status: RuntimeHealth::Failed,
            protocol_version: PROTOCOL_VERSION,
            store_versions: None,
            error: Some(error),
        }
    }
    pub(crate) fn from_wire(
        instance_id: String,
        status: RuntimeHealth,
        protocol_version: u16,
        store_versions: Option<MemoryStoreVersions>,
        error: Option<MemoryProtocolError>,
    ) -> Self {
        Self {
            instance_id,
            status,
            protocol_version,
            store_versions,
            error,
        }
    }

    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    pub const fn status(&self) -> RuntimeHealth {
        self.status
    }

    pub const fn protocol_version(&self) -> u16 {
        self.protocol_version
    }

    pub const fn store_versions(&self) -> Option<MemoryStoreVersions> {
        self.store_versions
    }

    pub fn error(&self) -> Option<&MemoryProtocolError> {
        self.error.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_have_stable_safe_contracts() {
        let cases = [
            (MemoryErrorCode::Unauthorized, "unauthorized", false),
            (MemoryErrorCode::MalformedFrame, "malformed_frame", false),
            (MemoryErrorCode::FrameTooLarge, "frame_too_large", false),
            (
                MemoryErrorCode::UnsupportedVersion,
                "unsupported_version",
                false,
            ),
            (
                MemoryErrorCode::UnknownOperation,
                "unknown_operation",
                false,
            ),
            (MemoryErrorCode::InvalidRequest, "invalid_request", false),
            (
                MemoryErrorCode::DuplicateRequest,
                "duplicate_request",
                false,
            ),
            (MemoryErrorCode::AlreadyRunning, "already_running", true),
            (
                MemoryErrorCode::PermissionDenied,
                "permission_denied",
                false,
            ),
            (MemoryErrorCode::NotFound, "not_found", false),
            (
                MemoryErrorCode::AlreadySuperseded,
                "already_superseded",
                false,
            ),
            (MemoryErrorCode::CorruptLesson, "corrupt_lesson", false),
            (MemoryErrorCode::MigrationFailed, "migration_failed", false),
            (MemoryErrorCode::Internal, "internal", true),
        ];
        for (code, name, retryable) in cases {
            let error = MemoryProtocolError::new(code);
            assert_eq!(code.as_str(), name);
            assert_eq!(code.to_string(), name);
            assert_eq!(MemoryErrorCode::from_wire_name(name), Some(code));
            assert_eq!(error.code(), code);
            assert_eq!(error.retryable(), retryable);
            assert!(!error.message().is_empty());
            assert!(!error.message().contains('/'));
        }
    }
}
