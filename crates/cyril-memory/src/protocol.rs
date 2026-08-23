//! Public, transport-independent memory runtime protocol domain types.

use std::fmt;

use crate::store::MemoryStoreVersions;

/// Protocol version understood by this crate.
pub const PROTOCOL_VERSION: u16 = 1;

/// Maximum encoded request or response payload, excluding the four-byte length.
pub(crate) const MAX_FRAME_SIZE: usize = 1_048_576;

/// Operations supported by the runtime administration protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRequest {
    /// Ask the runtime for its current health.
    Health,
    /// Ask the runtime to stop after acknowledging this request.
    Shutdown,
}

/// Responses returned by the runtime administration protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryResponse {
    /// Current runtime health.
    Health(HealthResponse),
    /// Acknowledgement of an authenticated shutdown request.
    Shutdown,
    /// A bounded, typed protocol failure.
    Error(MemoryProtocolError),
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
    pub(crate) fn ready(instance_id: String, store_versions: MemoryStoreVersions) -> Self {
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
            (MemoryErrorCode::MigrationFailed, "migration_failed", false),
            (MemoryErrorCode::Internal, "internal", true),
        ];
        for (code, name, retryable) in cases {
            let error = MemoryProtocolError::new(code);
            assert_eq!(code.as_str(), name);
            assert_eq!(code.to_string(), name);
            assert_eq!(error.code(), code);
            assert_eq!(error.retryable(), retryable);
            assert!(!error.message().is_empty());
            assert!(!error.message().contains('/'));
        }
    }
}
