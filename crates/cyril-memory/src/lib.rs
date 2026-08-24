//! Cyril-owned local memory runtime domain and persistence.
//!
//! This crate owns strict memory configuration, private data locations,
//! versioned stores, and the runtime protocol. It contains no ACP or UI types.

mod client;
mod config;
mod ipc;
mod lesson;
mod paths;
mod permissions;
mod project;
mod protocol;
mod runtime;
mod store;
mod wire;

pub use client::{AdminClient, AdminCredential, ClientError};
pub use config::{
    ConfigDiagnostic, ConfigLoadReport, MemoryConfig, MemoryConfigState, load_config_report,
};
pub use ipc::{IpcError, MemoryEndpoint};
pub use lesson::{
    ContextBlock, ContextLesson, LessonError, LessonId, LessonProvenance, LessonStatus, LessonText,
    LessonTrust, render_context,
};
pub use paths::{MemoryPaths, PathError};
pub use project::{ProjectError, ProjectId, ProjectScope};
pub use protocol::{
    HealthResponse, MemoryErrorCode, MemoryProtocolError, MemoryRequest, MemoryResponse,
    PROTOCOL_VERSION, RuntimeHealth,
};
pub use runtime::{RuntimeError, RuntimeLaunchConfig, run_runtime};
pub use store::{MemoryStoreVersions, StoreError, StoreSet};
