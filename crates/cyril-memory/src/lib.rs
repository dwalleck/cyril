//! Cyril-owned local memory runtime domain and persistence.
//!
//! This crate owns strict memory configuration, private data locations,
//! versioned stores, and the runtime protocol. It contains no ACP or UI types.

mod config;
mod paths;
mod permissions;
mod store;

pub use config::{
    ConfigDiagnostic, ConfigLoadReport, MemoryConfig, MemoryConfigState, load_config_report,
};
pub use paths::{MemoryPaths, PathError};
pub use store::{MemoryStoreVersions, StoreError, StoreSet};
