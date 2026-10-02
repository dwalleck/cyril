mod diagnostics;
mod facts;
mod gather;
mod git;
mod run;

mod clock;

pub use clock::{ReviewClock, SystemReviewClock};
pub use diagnostics::{
    Cancellation, DiagnosticsOptions, DiagnosticsOutcome, DiagnosticsResult, diagnostics,
};
pub use facts::facts;
pub use gather::gather;
pub use run::ReviewRun;

use std::io;
use std::path::PathBuf;

/// The leaf's default error type.  Callers may supply a different error type
/// when using the alias for small internal helpers.
pub type Result<T, E = ReviewError> = std::result::Result<T, E>;

/// Bytes printed by one native crtool operation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StepOutput {
    stdout: Vec<u8>,
}

impl StepOutput {
    pub(crate) fn from_text(text: String) -> Self {
        Self {
            stdout: text.into_bytes(),
        }
    }

    /// Return the operation's UTF-8 stdout bytes.
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }
}

/// Contextual errors returned by the native leaf.
#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    #[error("{operation} {path}: {source}")]
    Io {
        operation: String,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse JSON at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("git {args} failed: {message}")]
    GitFailure { args: String, message: String },
    #[error("git could not start for {args}: {source}")]
    GitSpawn {
        args: String,
        #[source]
        source: io::Error,
    },
    #[error("Git operation {operation} failed: {message}")]
    GitOperation { operation: String, message: String },
    #[error("invalid target {target:?}: a revision must not start with '-'")]
    InvalidTarget { target: String },
    #[error("empty diff for target={target:?} scope={scope:?}")]
    EmptyDiff { target: String, scope: Vec<String> },
    #[error("{path} already holds a different gathered run; use a fresh run directory")]
    ExistingRun { path: PathBuf },
    #[error("manifest.json has no crtool_version; refusing unstamped run")]
    MissingStamp,
    #[error("crtool_version mismatch: run={run}, current={current}")]
    StampMismatch { run: String, current: String },
    #[error("invalid manifest at {path}: {message}")]
    InvalidManifest { path: PathBuf, message: String },
    #[error("invalid regular expression: {source}")]
    Regex {
        #[source]
        source: regex::Error,
    },
    #[error("clock error: {message}")]
    Clock { message: String },
    #[error(transparent)]
    Diagnostics(#[from] DiagnosticsError),
}

impl ReviewError {
    /// Return crtool's process exit classification (2 for errors, 3 for an
    /// intentionally empty diff).
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::EmptyDiff { .. } => 3,
            _ => 2,
        }
    }
}

/// Command and direct-child failures from the diagnostics operation.
#[derive(Debug, thiserror::Error)]
pub enum DiagnosticsError {
    #[error("invalid diagnostics command: {message}")]
    InvalidCommand { message: String },
    #[error("cannot run diagnostics command {command:?}: {source}")]
    CannotStart {
        command: String,
        #[source]
        source: io::Error,
    },
    #[error("diagnostics lifecycle failure during {operation}: {source}")]
    Lifecycle {
        operation: String,
        #[source]
        source: io::Error,
    },
}

pub(crate) fn io_error(
    operation: impl Into<String>,
    path: impl Into<PathBuf>,
    source: io::Error,
) -> ReviewError {
    ReviewError::Io {
        operation: operation.into(),
        path: path.into(),
        source,
    }
}

pub(crate) fn json_error(path: impl Into<PathBuf>, source: serde_json::Error) -> ReviewError {
    ReviewError::Json {
        path: path.into(),
        source,
    }
}

pub(crate) fn regex_error(source: regex::Error) -> ReviewError {
    ReviewError::Regex { source }
}
