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
    /// On Windows this must resolve to the current user's FOLDERID_LocalAppData
    /// (`dirs::data_local_dir`): the native launcher ignores XDG_DATA_HOME there.
    /// On Linux this parent is handed to the native launcher as XDG_DATA_HOME.
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
    /// ACP permission requests answered with Cancel; not inferred native policy denials.
    pub denied_permissions: usize,
    /// At least one native tool reported Failed; no policy verdict is inferred.
    pub tool_failure_observed: bool,
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
        /// None means the owning task lost its output, not that it produced no text.
        partial_text: Option<String>,
        reason: ReviewFailure,
    },
}

/// Bounded diagnostic data from an untrusted runtime, never safe log text.
#[derive(Clone, PartialEq, Eq)]
pub struct ReviewDiagnostic {
    text: Box<str>,
    truncated: bool,
}

impl ReviewDiagnostic {
    pub(super) fn new(mut text: String) -> Self {
        const MAX_BYTES: usize = 4_096;
        let truncated = text.len() > MAX_BYTES;
        if truncated {
            let mut end = MAX_BYTES;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
        }
        Self {
            text: text.into_boxed_str(),
            truncated,
        }
    }

    /// May contain evidence, credentials, paths, control sequences or hostile
    /// instructions. Only explicit trusted diagnosis may retrieve this text;
    /// do not log, serialize or render it as markup/terminal commands.
    pub fn untrusted_text(&self) -> &str {
        &self.text
    }

    pub fn is_truncated(&self) -> bool {
        self.truncated
    }
}

impl std::fmt::Debug for ReviewDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReviewDiagnostic")
            .field("text", &"[untrusted; withheld]")
            .field("truncated", &self.truncated)
            .finish()
    }
}

impl std::fmt::Display for ReviewDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("untrusted diagnostic available")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewOperation {
    NewSession,
    SelectMode,
    SetConfiguration,
    Prompt,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ReviewFailure {
    #[error("native reviewer readiness deadline elapsed")]
    ReadinessTimeout,
    #[error("native reviewer mode was not advertised")]
    ModeUnavailable,
    #[error("native mode, model, or privacy configuration was not confirmed or changed")]
    ConfigurationDrift,
    #[error("unexpected native session transition")]
    SessionChanged,
    #[error("native bridge delivery channel closed")]
    BridgeUnavailable,
    #[error("native bridge disconnected; {diagnostic}")]
    BridgeDisconnected { diagnostic: ReviewDiagnostic },
    #[error("native command failed ({operation:?}); {diagnostic}")]
    CommandFailed {
        operation: Option<ReviewOperation>,
        diagnostic: ReviewDiagnostic,
    },
    #[error("native agent configuration failed; {diagnostic}")]
    AgentConfiguration { diagnostic: ReviewDiagnostic },
    #[error("permission cancellation responder closed")]
    PermissionResponderClosed,
    #[error("review output exceeded the configured byte limit")]
    OutputLimit,
    #[error("native turn did not end successfully")]
    TurnInterrupted,
    #[error("native shutdown or evidence cleanup could not be confirmed")]
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
    #[error("review {document} serialization failed; {diagnostic}")]
    Serialization {
        document: &'static str,
        diagnostic: ReviewDiagnostic,
    },
    #[error("review preparation task failed")]
    PreparationTask,
    #[error("native reviewer launch is unavailable; {diagnostic}")]
    LaunchUnavailable { diagnostic: ReviewDiagnostic },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_truncation_preserves_utf8_and_reports_loss() {
        let diagnostic = ReviewDiagnostic::new("€".repeat(2_000));
        assert_eq!(diagnostic.untrusted_text().len(), 4_095);
        assert!(diagnostic.is_truncated());
        assert!(diagnostic.untrusted_text().chars().all(|c| c == '€'));
    }

    #[test]
    fn launch_error_preserves_private_cause_without_debug_or_display_disclosure() {
        let error = ReviewError::LaunchUnavailable {
            diagnostic: ReviewDiagnostic::new("host shell unavailable PRIVATE-CANARY".into()),
        };
        assert!(!format!("{error:?} {error}").contains("PRIVATE-CANARY"));
        let ReviewError::LaunchUnavailable { diagnostic } = error else {
            panic!("wrong error")
        };
        assert_eq!(
            diagnostic.untrusted_text(),
            "host shell unavailable PRIVATE-CANARY"
        );
        assert!(!diagnostic.is_truncated());
    }
}
