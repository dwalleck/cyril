use std::fmt;

/// Error returned when a workflow identifier violates its domain invariant.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{kind} identifier cannot be empty")]
pub struct WorkflowIdentifierError {
    kind: &'static str,
}

/// Error returned when a closed workflow wire vocabulary receives an unknown value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown {domain} value `{value}`")]
pub struct WorkflowEnumParseError {
    domain: &'static str,
    value: String,
}

/// Stable identifier for one persisted workflow run.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkflowId(String);

impl WorkflowId {
    /// Borrows the identifier exactly as received.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for WorkflowId {
    type Error = WorkflowIdentifierError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Err(WorkflowIdentifierError { kind: "workflow" })
        } else {
            Ok(Self(value))
        }
    }
}

impl fmt::Display for WorkflowId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Identifier declared by a workflow node descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkflowNodeId(String);

impl WorkflowNodeId {
    /// Borrows the identifier exactly as received.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for WorkflowNodeId {
    type Error = WorkflowIdentifierError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Err(WorkflowIdentifierError {
                kind: "workflow node",
            })
        } else {
            Ok(Self(value))
        }
    }
}

impl fmt::Display for WorkflowNodeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

macro_rules! workflow_enum {
    (
        $(#[$metadata:meta])*
        $visibility:vis enum $name:ident as $domain:literal {
            $($variant:ident => $wire:literal),+ $(,)?
        }
    ) => {
        $(#[$metadata])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $visibility enum $name {
            $($variant),+
        }

        impl $name {
            /// Returns the exact wire spelling for this value.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }
        }

        impl TryFrom<&str> for $name {
            type Error = WorkflowEnumParseError;

            fn try_from(value: &str) -> Result<Self, WorkflowEnumParseError> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    _ => Err(WorkflowEnumParseError {
                        domain: $domain,
                        value: value.to_owned(),
                    }),
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

workflow_enum! {
    /// Status carried by a complete workflow snapshot.
    pub enum WorkflowRunStatus as "workflow run status" {
        Running => "running",
        Paused => "paused",
        Completed => "completed",
        Failed => "failed",
        Aborted => "aborted",
    }
}

workflow_enum! {
    /// Status carried by a `run_complete` lifecycle event.
    pub enum WorkflowCompletionStatus as "workflow completion status" {
        Paused => "paused",
        Completed => "completed",
        Failed => "failed",
        Aborted => "aborted",
    }
}

workflow_enum! {
    /// Status carried by a workflow node snapshot or completion.
    pub enum WorkflowNodeStatus as "workflow node status" {
        Pending => "pending",
        Running => "running",
        Paused => "paused",
        Completed => "completed",
        Failed => "failed",
        Aborted => "aborted",
        Skipped => "skipped",
    }
}

workflow_enum! {
    /// Structural type of a workflow node.
    pub enum WorkflowNodeType as "workflow node type" {
        Step => "step",
        Sequence => "sequence",
        Repeat => "repeat",
        Parallel => "parallel",
        Watch => "watch",
    }
}

workflow_enum! {
    /// Latest outcome reported by a workflow watch node.
    pub enum WorkflowWatchOutcome as "workflow watch outcome" {
        NewActivity => "new-activity",
        Idle => "idle",
        IdleTimeout => "idle-timeout",
        TerminalState => "terminal-state",
    }
}

workflow_enum! {
    /// Resolution outcome for a queued workflow-step update.
    pub enum WorkflowQueueOutcome as "workflow queue outcome" {
        Applied => "applied",
        Rejected => "rejected",
        Dropped => "dropped",
    }
}

workflow_enum! {
    /// Completion signal metadata emitted by a workflow node.
    pub enum WorkflowCompletionSignal as "workflow completion signal" {
        Success => "success",
        NeedInput => "need_input",
        Error => "error",
    }
}

workflow_enum! {
    /// Origin of workflow completion-signal metadata.
    pub enum WorkflowCompletionSignalSource as "workflow completion source" {
        SendMessage => "send_message",
        StatusUpdate => "status_update",
    }
}

workflow_enum! {
    /// Action taken when a repeat node exhausts its iteration budget.
    pub enum WorkflowRepeatExhaustion as "workflow repeat exhaustion action" {
        Pause => "pause",
        Abort => "abort",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_identifier_string_matrix() {
        let accepted = ["id", "識別子", "with space", "#", "/", "\\"];
        for raw in accepted {
            let workflow = WorkflowId::try_from(raw.to_owned()).expect("non-empty workflow id");
            let node = WorkflowNodeId::try_from(raw.to_owned()).expect("non-empty node id");
            assert_eq!(workflow.as_str(), raw);
            assert_eq!(node.as_str(), raw);
        }

        let large = "x".repeat(65_536);
        let pointer = large.as_ptr();
        let workflow = WorkflowId::try_from(large).expect("large workflow id");
        assert_eq!(workflow.as_str().as_ptr(), pointer);
        assert_eq!(workflow.as_str().len(), 65_536);
        assert!(WorkflowId::try_from(String::new()).is_err());
        assert!(WorkflowNodeId::try_from(String::new()).is_err());
    }

    #[test]
    fn workflow_enum_domain_matrix() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../../../.cyril-6beh/oracle-manifest.json"))
                .expect("oracle manifest is valid JSON");
        let domains = manifest["enum_domains"]
            .as_object()
            .expect("enum_domains is an object");

        assert_domain(
            domains,
            "snapshot_run_status",
            &[
                WorkflowRunStatus::Running.as_str(),
                WorkflowRunStatus::Paused.as_str(),
                WorkflowRunStatus::Completed.as_str(),
                WorkflowRunStatus::Failed.as_str(),
                WorkflowRunStatus::Aborted.as_str(),
            ],
        );
        assert_domain(
            domains,
            "completion_status",
            &[
                WorkflowCompletionStatus::Paused.as_str(),
                WorkflowCompletionStatus::Completed.as_str(),
                WorkflowCompletionStatus::Failed.as_str(),
                WorkflowCompletionStatus::Aborted.as_str(),
            ],
        );
        assert_domain(
            domains,
            "node_status",
            &[
                WorkflowNodeStatus::Pending.as_str(),
                WorkflowNodeStatus::Running.as_str(),
                WorkflowNodeStatus::Paused.as_str(),
                WorkflowNodeStatus::Completed.as_str(),
                WorkflowNodeStatus::Failed.as_str(),
                WorkflowNodeStatus::Aborted.as_str(),
                WorkflowNodeStatus::Skipped.as_str(),
            ],
        );
        assert_domain(
            domains,
            "node_type",
            &[
                WorkflowNodeType::Step.as_str(),
                WorkflowNodeType::Sequence.as_str(),
                WorkflowNodeType::Repeat.as_str(),
                WorkflowNodeType::Parallel.as_str(),
                WorkflowNodeType::Watch.as_str(),
            ],
        );
        assert_domain(
            domains,
            "watch_outcome",
            &[
                WorkflowWatchOutcome::NewActivity.as_str(),
                WorkflowWatchOutcome::Idle.as_str(),
                WorkflowWatchOutcome::IdleTimeout.as_str(),
                WorkflowWatchOutcome::TerminalState.as_str(),
            ],
        );
        assert_domain(
            domains,
            "queue_outcome",
            &[
                WorkflowQueueOutcome::Applied.as_str(),
                WorkflowQueueOutcome::Rejected.as_str(),
                WorkflowQueueOutcome::Dropped.as_str(),
            ],
        );
        assert_domain(
            domains,
            "completion_signal",
            &[
                WorkflowCompletionSignal::Success.as_str(),
                WorkflowCompletionSignal::NeedInput.as_str(),
                WorkflowCompletionSignal::Error.as_str(),
            ],
        );
        assert_domain(
            domains,
            "completion_source",
            &[
                WorkflowCompletionSignalSource::SendMessage.as_str(),
                WorkflowCompletionSignalSource::StatusUpdate.as_str(),
            ],
        );
        assert_domain(
            domains,
            "repeat_exhaustion",
            &[
                WorkflowRepeatExhaustion::Pause.as_str(),
                WorkflowRepeatExhaustion::Abort.as_str(),
            ],
        );

        assert!(WorkflowRunStatus::try_from("unknown").is_err());
        assert!(WorkflowCompletionStatus::try_from("unknown").is_err());
        assert!(WorkflowNodeStatus::try_from("unknown").is_err());
        assert!(WorkflowNodeType::try_from("unknown").is_err());
        assert!(WorkflowWatchOutcome::try_from("unknown").is_err());
        assert!(WorkflowQueueOutcome::try_from("unknown").is_err());
        assert!(WorkflowCompletionSignal::try_from("unknown").is_err());
        assert!(WorkflowCompletionSignalSource::try_from("unknown").is_err());
        assert!(WorkflowRepeatExhaustion::try_from("unknown").is_err());

        assert_eq!([u32::MIN, u32::MAX], [0, 4_294_967_295]);
    }

    fn assert_domain(
        domains: &serde_json::Map<String, serde_json::Value>,
        name: &str,
        actual: &[&str],
    ) {
        let expected = domains[name]
            .as_array()
            .expect("enum domain is an array")
            .iter()
            .map(|value| value.as_str().expect("enum value is a string"))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}
