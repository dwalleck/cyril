//! Cyril-owned local memory runtime domain and persistence.
//!
//! This crate owns strict memory configuration, private data locations,
//! versioned stores, and the runtime protocol. It contains no ACP or UI types.

mod client;
mod config;
mod ipc;
mod paths;
mod permissions;
mod protocol;
mod runtime;
mod store;
mod wire;

pub use client::{AdminClient, AdminCredential, ClientError};
pub use config::{
    ConfigDiagnostic, ConfigLoadReport, MemoryConfig, MemoryConfigState, load_config_report,
};
pub use ipc::{IpcError, MemoryEndpoint};
pub use paths::{MemoryPaths, PathError};
pub use protocol::{
    HealthResponse, MemoryErrorCode, MemoryProtocolError, MemoryRequest, MemoryResponse,
    PROTOCOL_VERSION, RuntimeHealth,
};
pub use runtime::{RuntimeError, RuntimeLaunchConfig, run_runtime};
pub use store::{MemoryStoreVersions, StoreError, StoreSet};
