//! Cyril-owned local memory runtime domain and persistence.
//!
//! This crate owns strict memory configuration, private data locations,
//! versioned stores, and the runtime protocol. It contains no ACP or UI types.

mod client;
mod config;
mod encoding;
mod ipc;
mod lesson;
mod paths;
mod permissions;
mod project;
mod protocol;
mod redaction;
mod runtime;
mod source_turn;
mod store;
mod wire;

pub use client::{AdminClient, AdminCredential, ClientError};
pub use config::{
    ConfigDiagnostic, ConfigLoadReport, MemoryConfig, MemoryConfigState, load_config_report,
};
pub use ipc::{IpcError, MemoryEndpoint};
pub use lesson::{
    LESSON_PREVIEW_CHARS, LessonError, LessonId, LessonIdParseError, LessonProvenance,
    LessonStatus, LessonText, LessonTrust, MAX_LESSON_CHARS,
};
pub use paths::{MemoryPaths, PathError};
pub use project::{ProjectError, ProjectId, ProjectScope};
pub use protocol::{
    HealthResponse, LessonListResponse, LessonRecord, MemoryErrorCode, MemoryProtocolError,
    MemoryRequest, MemoryResponse, PROTOCOL_VERSION, PromptContext, RuntimeHealth,
    SourceTurnListResponse, SourceTurnRecord, SourceTurnStatus, TeachResponse,
};
pub use runtime::{RuntimeError, RuntimeLaunchConfig, run_runtime};
pub use source_turn::{
    CaptureBatch, SourceSessionId, SourceSessionIdError, SourceTurnDisposition, SourceTurnError,
    SourceTurnEvent, SourceTurnEventKind, SourceTurnId, SourceTurnIdParseError,
};
pub use store::{MemoryStoreVersions, StoreError, StoreSet};
