use std::{collections::BTreeMap, ffi::OsString, path::PathBuf, time::Duration};

/// Owned captured text, never a path to be opened by the reviewer backend.
#[derive(Clone, Debug)]
pub struct EvidenceDocument {
    pub source_label: String,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct ReviewInput {
    pub instruction: String,
    pub documents: Vec<EvidenceDocument>,
    /// Explicit same-session continuations, dispatched only after EndTurn.
    /// Combined instruction bytes share `instruction_bytes`; empty entries fail.
    pub follow_up_instructions: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ReviewLimits {
    pub documents: usize,
    /// Total UTF-8 document text bytes (labels have a separate bound).
    pub evidence_bytes: usize,
    pub label_bytes: usize,
    pub instruction_bytes: usize,
    pub output_bytes: usize,
    pub startup_timeout: Duration,
    /// Silence only changes liveness; it never completes a review.
    pub stall_threshold: Duration,
}

impl Default for ReviewLimits {
    fn default() -> Self {
        Self {
            documents: 1_024,
            evidence_bytes: 64 * 1_024 * 1_024,
            label_bytes: 4_096,
            instruction_bytes: 64 * 1_024,
            output_bytes: 8 * 1_024 * 1_024,
            startup_timeout: Duration::from_secs(30),
            stall_threshold: Duration::from_secs(30),
        }
    }
}

/// Trusted deployment inputs. No arbitrary profile, command, or provider credential.
/// Paths are canonicalized by `Reviewer::new`; the executable must be absolute.
#[derive(Clone)]
pub struct ReviewerConfig {
    pub executable: PathBuf,
    pub runtime_parent: PathBuf,
    /// Existing native sign-in data location; credentials are never copied.
    pub auth_data_home: PathBuf,
    /// Explicit PATH and optional transport/certificate settings. Unknown names fail.
    pub transport_environment: BTreeMap<OsString, OsString>,
    pub limits: ReviewLimits,
}

impl std::fmt::Debug for ReviewerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReviewerConfig")
            .field("executable", &self.executable)
            .field("runtime_parent", &self.runtime_parent)
            .field("auth_data_home", &self.auth_data_home)
            .field("transport_environment", &"[redacted]")
            .field("limits", &self.limits)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewPhase {
    Starting,
    Inspecting,
    Stalled,
    Stopping,
    Completed,
    Cancelled,
    Incomplete,
}

/// Constant-size, latest-value projection. Observers never own bridge delivery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewStatus {
    pub phase: ReviewPhase,
    pub output_bytes: usize,
    pub denied_permissions: usize,
}

/// A protocol completion is not a finding-quality or coverage verdict.
#[derive(Debug)]
pub enum ReviewOutcome {
    Completed {
        text: String,
    },
    Cancelled {
        partial_text: String,
    },
    Incomplete {
        partial_text: String,
        reason: ReviewFailure,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ReviewFailure {
    #[error("native reviewer readiness deadline elapsed")]
    ReadinessTimeout,
    #[error("native reviewer mode was not advertised")]
    ModeUnavailable,
    #[error("native model or mode changed during inspection")]
    ConfigurationDrift,
    #[error("unexpected native session transition")]
    SessionChanged,
    #[error("native bridge disconnected or a command failed")]
    BridgeUnavailable,
    #[error("permission cancellation responder closed")]
    PermissionResponderClosed,
    #[error("review output exceeded the configured byte limit")]
    OutputLimit,
    #[error("native turn did not end successfully")]
    TurnInterrupted,
    #[error("native shutdown completion was lost")]
    ShutdownFailed,
    #[error("review task completion was lost")]
    TaskLost,
}

#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    #[error("invalid review input/configuration: {0}")]
    InvalidInput(&'static str),
    #[error("review preparation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("review configuration serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("review preparation task failed")]
    PreparationTask,
    #[error("native reviewer launch is unavailable")]
    LaunchUnavailable,
}
