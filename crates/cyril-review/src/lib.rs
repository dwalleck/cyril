//! Native crtool: the deterministic data movement of the `code-review-max`
//! workflow. `docs/crtool-contract.md` is the specification; the Python
//! `.kiro/code-review/crtool.py` is the reference implementation.

mod assets;
mod check;
mod facts;
mod gather;
mod git;
mod merge;
mod record;
mod report;
mod run;
mod verdict;
mod verdicts;

pub use assets::{ASSETS, Asset, AssetKind, WORKFLOW_NAME};
pub use check::{CheckOutcome, CheckResult, run_check};
pub use facts::facts;
pub use gather::{base_branches, changed_paths, gather, touched_files, uncommitted_paths};
pub use merge::{merge, shard};
pub use report::{Finding, FindingsError, comments, finalize, read_findings};
pub use run::ReviewRun;
pub use verdict::Verdict;
pub use verdicts::{ballots, collate};

use std::io;
use std::path::PathBuf;

pub type Result<T, E = ReviewError> = std::result::Result<T, E>;

/// Errors from crtool steps and the check command.
#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    #[error("{operation} {path}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot parse JSON {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("cannot run git: {source}")]
    GitSpawn {
        #[source]
        source: io::Error,
    },
    #[error("git {args} failed: {message}")]
    Git { args: String, message: String },
    #[error("crtool must run from the repository root ({cdup} from here)")]
    NotRepositoryRoot { cdup: String },
    #[error("invalid target {target:?}: a revision must not start with '-'")]
    InvalidTarget { target: String },
    #[error("unsupported file name {name:?}: crtool needs UTF-8 paths")]
    NonUtf8Path { name: String },
    #[error("empty diff for target={target:?} scope={scope:?}")]
    EmptyDiff { target: String, scope: Vec<String> },
    #[error("{path} already holds a different gathered run; use a fresh run directory")]
    ExistingRun { path: PathBuf },
    #[error("{path} was gathered by crtool {found}, this is crtool {current}; start a new review")]
    StampMismatch {
        path: PathBuf,
        found: String,
        current: &'static str,
    },
    #[error("invalid check command {command:?}: {message}")]
    InvalidCommand {
        command: String,
        message: &'static str,
    },
    #[error("cannot run {command:?}: {source}")]
    CannotStart {
        command: String,
        #[source]
        source: io::Error,
    },
    #[error("{path} is corrupt: {message}")]
    CorruptRunFile { path: PathBuf, message: String },
    #[error("invalid argument: {message}")]
    InvalidArgument { message: &'static str },
    #[error("invalid regular expression: {0}")]
    Regex(#[from] regex::Error),
}

impl ReviewError {
    /// crtool's process exit status for this error: 3 for an empty diff, else 2.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::EmptyDiff { .. } => 3,
            _ => 2,
        }
    }
}

pub(crate) fn io_error(
    operation: &'static str,
    path: impl Into<PathBuf>,
    source: io::Error,
) -> ReviewError {
    ReviewError::Io {
        operation,
        path: path.into(),
        source,
    }
}
