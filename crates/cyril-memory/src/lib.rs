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
    BoundedText, HealthResponse, INSPECT_TEXT_CHARS, INSPECT_TOOL_TEXT_CHARS, LessonListResponse,
    LessonRecord, MAX_PROMPT_CONTEXT_CHARS, MemoryErrorCode, MemoryProtocolError, MemoryRequest,
    MemoryResponse, PROTOCOL_VERSION, PromptContext, RuntimeHealth, SOURCE_TURN_PREVIEW_CHARS,
    SourceTurnListResponse, SourceTurnRecord, SourceTurnSummary, TeachResponse, ToolSummary,
};
pub use runtime::{RuntimeError, RuntimeLaunchConfig, run_runtime};
pub use source_turn::{
    CaptureBatch, MAX_CAPTURE_BYTES, MAX_CAPTURE_EVENTS, MAX_EPISODE_CHARS,
    MAX_EPISODE_TOTAL_CHARS, MAX_EPISODES, MAX_PROMPT_BLOCKS, MAX_QUERY_CHARS, MAX_QUERY_TERMS,
    MAX_SOURCE_EVENT_TEXT_CHARS, MAX_TOOL_CHARS, MAX_TOOL_ID_CHARS, MAX_TOOL_STATUS_CHARS,
    MAX_TOOLS, MAX_TOTAL_TOOL_CHARS, PromptQuery, PromptQueryError, SourceSessionId,
    SourceSessionIdError, SourceToolId, SourceToolIdError, SourceTurnDisposition, SourceTurnError,
    SourceTurnEvent, SourceTurnEventKind, SourceTurnId, SourceTurnIdParseError, SourceTurnStatus,
};
pub use store::{MemoryStoreVersions, StoreError, StoreSet};
