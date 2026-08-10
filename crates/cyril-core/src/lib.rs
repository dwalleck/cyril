pub mod commands;
pub mod error;
pub mod kiro_agent_config;
pub mod platform;
pub mod protocol;
pub mod session;
pub mod subagent;
pub mod types;
pub mod voice;
pub mod workflow;

pub use error::{Error, ErrorKind, Result};

/// Test-only helpers (capture writer, fixture unwrapper, capture lock).
/// Gated so they compile into the lib only for our own tests or when a
/// downstream crate's dev-dependency opts in via the `test-support` feature.
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
