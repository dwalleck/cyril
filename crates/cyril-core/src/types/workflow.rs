use std::fmt;

use super::session::SessionId;

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

/// Recursive, wire-neutral description of one node in a workflow recipe.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowNodeDescriptor {
    node_id: WorkflowNodeId,
    kind: WorkflowNodeDescriptorKind,
}

#[derive(Debug, Clone, PartialEq)]
enum WorkflowNodeDescriptorKind {
    Step {
        agent_name: String,
        model: Option<String>,
        effort: Option<String>,
    },
    Sequence {
        steps: Vec<WorkflowNodeDescriptor>,
    },
    Repeat {
        steps: Vec<WorkflowNodeDescriptor>,
        max_iterations: u32,
        on_max_iterations: WorkflowRepeatExhaustion,
        stop_condition: Option<serde_json::Value>,
        stop_when: Option<serde_json::Value>,
    },
    Parallel {
        branches: Vec<WorkflowNodeDescriptor>,
    },
    Watch {
        handler_name: String,
    },
}

impl WorkflowNodeDescriptor {
    /// Constructs a step descriptor.
    pub fn step(
        node_id: WorkflowNodeId,
        agent_name: String,
        model: Option<String>,
        effort: Option<String>,
    ) -> Self {
        Self {
            node_id,
            kind: WorkflowNodeDescriptorKind::Step {
                agent_name,
                model,
                effort,
            },
        }
    }

    /// Constructs a sequence descriptor.
    pub fn sequence(node_id: WorkflowNodeId, steps: Vec<Self>) -> Self {
        Self {
            node_id,
            kind: WorkflowNodeDescriptorKind::Sequence { steps },
        }
    }

    /// Constructs a repeat descriptor.
    pub fn repeat(
        node_id: WorkflowNodeId,
        steps: Vec<Self>,
        max_iterations: u32,
        on_max_iterations: WorkflowRepeatExhaustion,
        stop_condition: Option<serde_json::Value>,
        stop_when: Option<serde_json::Value>,
    ) -> Self {
        Self {
            node_id,
            kind: WorkflowNodeDescriptorKind::Repeat {
                steps,
                max_iterations,
                on_max_iterations,
                stop_condition,
                stop_when,
            },
        }
    }

    /// Constructs a parallel descriptor.
    pub fn parallel(node_id: WorkflowNodeId, branches: Vec<Self>) -> Self {
        Self {
            node_id,
            kind: WorkflowNodeDescriptorKind::Parallel { branches },
        }
    }

    /// Constructs a watch descriptor.
    pub fn watch(node_id: WorkflowNodeId, handler_name: String) -> Self {
        Self {
            node_id,
            kind: WorkflowNodeDescriptorKind::Watch { handler_name },
        }
    }

    /// Returns this descriptor's stable node identifier.
    pub fn node_id(&self) -> &WorkflowNodeId {
        &self.node_id
    }

    /// Returns this descriptor's structural type.
    pub fn node_type(&self) -> WorkflowNodeType {
        match self.kind {
            WorkflowNodeDescriptorKind::Step { .. } => WorkflowNodeType::Step,
            WorkflowNodeDescriptorKind::Sequence { .. } => WorkflowNodeType::Sequence,
            WorkflowNodeDescriptorKind::Repeat { .. } => WorkflowNodeType::Repeat,
            WorkflowNodeDescriptorKind::Parallel { .. } => WorkflowNodeType::Parallel,
            WorkflowNodeDescriptorKind::Watch { .. } => WorkflowNodeType::Watch,
        }
    }

    /// Returns the children declared by a sequence, repeat, or parallel node.
    pub fn children(&self) -> &[Self] {
        match &self.kind {
            WorkflowNodeDescriptorKind::Sequence { steps }
            | WorkflowNodeDescriptorKind::Repeat { steps, .. } => steps,
            WorkflowNodeDescriptorKind::Parallel { branches } => branches,
            WorkflowNodeDescriptorKind::Step { .. } | WorkflowNodeDescriptorKind::Watch { .. } => {
                &[]
            }
        }
    }

    /// Returns the agent name when this is a step descriptor.
    pub fn agent_name(&self) -> Option<&str> {
        match &self.kind {
            WorkflowNodeDescriptorKind::Step { agent_name, .. } => Some(agent_name),
            _ => None,
        }
    }

    /// Returns the optional model when this is a step descriptor.
    pub fn model(&self) -> Option<&str> {
        match &self.kind {
            WorkflowNodeDescriptorKind::Step { model, .. } => model.as_deref(),
            _ => None,
        }
    }

    /// Returns the optional effort when this is a step descriptor.
    pub fn effort(&self) -> Option<&str> {
        match &self.kind {
            WorkflowNodeDescriptorKind::Step { effort, .. } => effort.as_deref(),
            _ => None,
        }
    }

    /// Returns the maximum iteration count when this is a repeat descriptor.
    pub fn max_iterations(&self) -> Option<u32> {
        match self.kind {
            WorkflowNodeDescriptorKind::Repeat { max_iterations, .. } => Some(max_iterations),
            _ => None,
        }
    }

    /// Returns the exhaustion behavior when this is a repeat descriptor.
    pub fn on_max_iterations(&self) -> Option<WorkflowRepeatExhaustion> {
        match self.kind {
            WorkflowNodeDescriptorKind::Repeat {
                on_max_iterations, ..
            } => Some(on_max_iterations),
            _ => None,
        }
    }

    /// Returns the opaque stop condition when this is a repeat descriptor.
    pub fn stop_condition(&self) -> Option<&serde_json::Value> {
        match &self.kind {
            WorkflowNodeDescriptorKind::Repeat { stop_condition, .. } => stop_condition.as_ref(),
            _ => None,
        }
    }

    /// Returns the opaque stop predicate when this is a repeat descriptor.
    pub fn stop_when(&self) -> Option<&serde_json::Value> {
        match &self.kind {
            WorkflowNodeDescriptorKind::Repeat { stop_when, .. } => stop_when.as_ref(),
            _ => None,
        }
    }

    /// Returns the handler name when this is a watch descriptor.
    pub fn handler_name(&self) -> Option<&str> {
        match &self.kind {
            WorkflowNodeDescriptorKind::Watch { handler_name } => Some(handler_name),
            _ => None,
        }
    }
}

/// Snapshot-owned runtime state for one workflow node.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowNodeSnapshot {
    descriptor: WorkflowNodeDescriptor,
    status: WorkflowNodeStatus,
    children: Vec<WorkflowNodeSnapshot>,
    session_id: Option<SessionId>,
    artifacts: Option<serde_json::Value>,
    captured_output: Option<serde_json::Value>,
    failure_reason: Option<String>,
    iteration: Option<u32>,
    branch_id: Option<String>,
    completion_signal: Option<WorkflowCompletionSignal>,
    completion_signal_source: Option<WorkflowCompletionSignalSource>,
    started_at: Option<String>,
    ended_at: Option<String>,
}

impl WorkflowNodeSnapshot {
    /// Constructs a node snapshot without optional runtime fields.
    pub fn new(
        descriptor: WorkflowNodeDescriptor,
        status: WorkflowNodeStatus,
        children: Vec<Self>,
    ) -> Self {
        Self {
            descriptor,
            status,
            children,
            session_id: None,
            artifacts: None,
            captured_output: None,
            failure_reason: None,
            iteration: None,
            branch_id: None,
            completion_signal: None,
            completion_signal_source: None,
            started_at: None,
            ended_at: None,
        }
    }

    /// Sets the optional peer-session identifier.
    pub fn with_session_id(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Sets opaque per-node artifacts.
    pub fn with_artifacts(mut self, artifacts: serde_json::Value) -> Self {
        self.artifacts = Some(artifacts);
        self
    }

    /// Sets an opaque captured output.
    pub fn with_captured_output(mut self, output: serde_json::Value) -> Self {
        self.captured_output = Some(output);
        self
    }

    /// Sets the failure reason.
    pub fn with_failure_reason(mut self, reason: String) -> Self {
        self.failure_reason = Some(reason);
        self
    }

    /// Sets repeat-iteration metadata.
    pub fn with_iteration(mut self, iteration: u32) -> Self {
        self.iteration = Some(iteration);
        self
    }

    /// Sets parallel-branch metadata.
    pub fn with_branch_id(mut self, branch_id: String) -> Self {
        self.branch_id = Some(branch_id);
        self
    }

    /// Sets completion signal metadata.
    pub fn with_completion_signal(mut self, signal: WorkflowCompletionSignal) -> Self {
        self.completion_signal = Some(signal);
        self
    }

    /// Sets completion signal source metadata.
    pub fn with_completion_signal_source(mut self, source: WorkflowCompletionSignalSource) -> Self {
        self.completion_signal_source = Some(source);
        self
    }

    /// Sets the opaque start timestamp.
    pub fn with_started_at(mut self, started_at: String) -> Self {
        self.started_at = Some(started_at);
        self
    }

    /// Sets the opaque end timestamp.
    pub fn with_ended_at(mut self, ended_at: String) -> Self {
        self.ended_at = Some(ended_at);
        self
    }

    /// Returns the recipe descriptor for this runtime node.
    pub fn descriptor(&self) -> &WorkflowNodeDescriptor {
        &self.descriptor
    }

    /// Returns the snapshot-authored node status.
    pub fn status(&self) -> WorkflowNodeStatus {
        self.status
    }

    /// Returns nested runtime node snapshots.
    pub fn children(&self) -> &[Self] {
        &self.children
    }

    /// Returns the peer-session identifier when supplied.
    pub fn session_id(&self) -> Option<&SessionId> {
        self.session_id.as_ref()
    }

    /// Returns opaque per-node artifacts when supplied.
    pub fn artifacts(&self) -> Option<&serde_json::Value> {
        self.artifacts.as_ref()
    }

    /// Returns opaque captured output when supplied.
    pub fn captured_output(&self) -> Option<&serde_json::Value> {
        self.captured_output.as_ref()
    }

    /// Returns the failure reason when supplied.
    pub fn failure_reason(&self) -> Option<&str> {
        self.failure_reason.as_deref()
    }

    /// Returns repeat-iteration metadata when supplied.
    pub fn iteration(&self) -> Option<u32> {
        self.iteration
    }

    /// Returns parallel-branch metadata when supplied.
    pub fn branch_id(&self) -> Option<&str> {
        self.branch_id.as_deref()
    }

    /// Returns completion signal metadata when supplied.
    pub fn completion_signal(&self) -> Option<WorkflowCompletionSignal> {
        self.completion_signal
    }

    /// Returns completion signal source metadata when supplied.
    pub fn completion_signal_source(&self) -> Option<WorkflowCompletionSignalSource> {
        self.completion_signal_source
    }

    /// Returns the opaque start timestamp when supplied.
    pub fn started_at(&self) -> Option<&str> {
        self.started_at.as_deref()
    }

    /// Returns the opaque end timestamp when supplied.
    pub fn ended_at(&self) -> Option<&str> {
        self.ended_at.as_deref()
    }
}

/// Opaque recipe inputs and outputs carried by a full workflow snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowSnapshotData {
    inputs: serde_json::Value,
    artifacts: serde_json::Value,
    captured_outputs: serde_json::Value,
}

impl WorkflowSnapshotData {
    /// Constructs the required opaque snapshot data.
    pub fn new(
        inputs: serde_json::Value,
        artifacts: serde_json::Value,
        captured_outputs: serde_json::Value,
    ) -> Self {
        Self {
            inputs,
            artifacts,
            captured_outputs,
        }
    }
}

/// Persisted metadata carried by a full workflow snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowSnapshotMetadata {
    created_at: String,
    plan_revision: u32,
    parent_session_id: Option<SessionId>,
    workspace_path: Option<String>,
}

impl WorkflowSnapshotMetadata {
    /// Constructs required snapshot metadata without optional location fields.
    pub fn new(created_at: String, plan_revision: u32) -> Self {
        Self {
            created_at,
            plan_revision,
            parent_session_id: None,
            workspace_path: None,
        }
    }

    /// Sets the optional parent session.
    pub fn with_parent_session_id(mut self, session_id: SessionId) -> Self {
        self.parent_session_id = Some(session_id);
        self
    }

    /// Sets the opaque workspace path exactly as received.
    pub fn with_workspace_path(mut self, workspace_path: String) -> Self {
        self.workspace_path = Some(workspace_path);
        self
    }

    /// Returns the parent session when supplied.
    pub fn parent_session_id(&self) -> Option<&SessionId> {
        self.parent_session_id.as_ref()
    }

    /// Returns the opaque workspace path when supplied.
    pub fn workspace_path(&self) -> Option<&str> {
        self.workspace_path.as_deref()
    }
}

/// Complete persisted state of one workflow run.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowSnapshot {
    workflow_id: WorkflowId,
    workflow_name: String,
    status: WorkflowRunStatus,
    data: WorkflowSnapshotData,
    root: WorkflowNodeSnapshot,
    metadata: WorkflowSnapshotMetadata,
}

impl WorkflowSnapshot {
    /// Constructs a complete, wire-neutral snapshot.
    pub fn new(
        workflow_id: WorkflowId,
        workflow_name: String,
        status: WorkflowRunStatus,
        data: WorkflowSnapshotData,
        root: WorkflowNodeSnapshot,
        metadata: WorkflowSnapshotMetadata,
    ) -> Self {
        Self {
            workflow_id,
            workflow_name,
            status,
            data,
            root,
            metadata,
        }
    }

    /// Returns the persisted workflow identifier.
    pub fn workflow_id(&self) -> &WorkflowId {
        &self.workflow_id
    }

    /// Returns the recipe name.
    pub fn workflow_name(&self) -> &str {
        &self.workflow_name
    }

    /// Returns the persisted run status.
    pub fn status(&self) -> WorkflowRunStatus {
        self.status
    }

    /// Returns opaque recipe inputs.
    pub fn inputs(&self) -> &serde_json::Value {
        &self.data.inputs
    }

    /// Returns opaque run artifacts.
    pub fn artifacts(&self) -> &serde_json::Value {
        &self.data.artifacts
    }

    /// Returns opaque captured outputs.
    pub fn captured_outputs(&self) -> &serde_json::Value {
        &self.data.captured_outputs
    }

    /// Returns the runtime root.
    pub fn root(&self) -> &WorkflowNodeSnapshot {
        &self.root
    }

    /// Returns the opaque creation timestamp.
    pub fn created_at(&self) -> &str {
        &self.metadata.created_at
    }

    /// Returns the persisted plan revision.
    pub fn plan_revision(&self) -> u32 {
        self.metadata.plan_revision
    }

    /// Returns the parent session when supplied.
    pub fn parent_session_id(&self) -> Option<&SessionId> {
        self.metadata.parent_session_id()
    }

    /// Returns the opaque workspace path when supplied.
    pub fn workspace_path(&self) -> Option<&str> {
        self.metadata.workspace_path()
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

    #[test]
    fn workflow_node_descriptor_shape_matrix() {
        let step = WorkflowNodeDescriptor::step(
            node_id("step"),
            "agent".to_owned(),
            Some("model".to_owned()),
            Some("effort".to_owned()),
        );
        assert_eq!(step.node_id().as_str(), "step");
        assert_eq!(step.node_type(), WorkflowNodeType::Step);
        assert_eq!(step.agent_name(), Some("agent"));
        assert_eq!(step.model(), Some("model"));
        assert_eq!(step.effort(), Some("effort"));
        assert!(step.children().is_empty());

        let sequence = WorkflowNodeDescriptor::sequence(node_id("sequence"), vec![step.clone()]);
        assert_eq!(sequence.node_type(), WorkflowNodeType::Sequence);
        assert_eq!(sequence.children(), [step.clone()]);

        let repeat = WorkflowNodeDescriptor::repeat(
            node_id("repeat"),
            vec![step.clone()],
            u32::MAX,
            WorkflowRepeatExhaustion::Pause,
            Some(serde_json::json!({"kind": "condition"})),
            Some(serde_json::json!(["predicate"])),
        );
        assert_eq!(repeat.node_type(), WorkflowNodeType::Repeat);
        assert_eq!(repeat.max_iterations(), Some(u32::MAX));
        assert_eq!(
            repeat.on_max_iterations(),
            Some(WorkflowRepeatExhaustion::Pause)
        );
        assert_eq!(
            repeat.stop_condition(),
            Some(&serde_json::json!({"kind": "condition"}))
        );
        assert_eq!(repeat.stop_when(), Some(&serde_json::json!(["predicate"])));

        let parallel =
            WorkflowNodeDescriptor::parallel(node_id("parallel"), vec![step.clone(), step]);
        assert_eq!(parallel.node_type(), WorkflowNodeType::Parallel);
        assert_eq!(parallel.children().len(), 2);

        let watch = WorkflowNodeDescriptor::watch(node_id("watch"), "handler".to_owned());
        assert_eq!(watch.node_type(), WorkflowNodeType::Watch);
        assert_eq!(watch.handler_name(), Some("handler"));
        assert!(watch.agent_name().is_none());
        assert!(watch.model().is_none());
        assert!(watch.effort().is_none());
        assert!(watch.max_iterations().is_none());
        assert!(watch.on_max_iterations().is_none());
        assert!(watch.stop_condition().is_none());
        assert!(watch.stop_when().is_none());

        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../../../.cyril-6beh/oracle-manifest.json"))
                .expect("oracle manifest is valid JSON");
        let projection = serde_json::json!({
            "step": {
                "required": ["nodeId", "type", "agentName"],
                "optional": ["model", "effort"]
            },
            "sequence": {
                "required": ["nodeId", "type", "steps"],
                "optional": []
            },
            "repeat": {
                "required": ["nodeId", "type", "steps", "maxIterations", "onMaxIterations"],
                "optional": ["stopCondition", "stopWhen"]
            },
            "parallel": {
                "required": ["nodeId", "type", "branches"],
                "optional": []
            },
            "watch": {
                "required": ["nodeId", "type", "handlerName"],
                "optional": []
            }
        });
        assert_eq!(projection, manifest["descriptor_fields"]);
    }

    #[test]
    fn workflow_snapshot_shape_matrix() {
        let child = WorkflowNodeSnapshot::new(
            WorkflowNodeDescriptor::step(node_id("child"), "agent".to_owned(), None, None),
            WorkflowNodeStatus::Completed,
            Vec::new(),
        )
        .with_session_id(SessionId::new("session"))
        .with_artifacts(serde_json::json!({"artifact": [1, 1]}))
        .with_captured_output(serde_json::json!({"output": true}))
        .with_failure_reason("reason".to_owned())
        .with_iteration(u32::MAX)
        .with_branch_id("branch".to_owned())
        .with_completion_signal(WorkflowCompletionSignal::Success)
        .with_completion_signal_source(WorkflowCompletionSignalSource::StatusUpdate)
        .with_started_at("started".to_owned())
        .with_ended_at("ended".to_owned());
        let root = WorkflowNodeSnapshot::new(
            WorkflowNodeDescriptor::sequence(node_id("workflow"), Vec::new()),
            WorkflowNodeStatus::Completed,
            vec![child],
        );
        let metadata = WorkflowSnapshotMetadata::new("created".to_owned(), u32::MAX)
            .with_parent_session_id(SessionId::new("parent"))
            .with_workspace_path("relative workspace".to_owned());
        let data = WorkflowSnapshotData::new(
            serde_json::json!({"input": null}),
            serde_json::json!({"run": "artifact"}),
            serde_json::json!({"capture": [true, false]}),
        );
        let snapshot = WorkflowSnapshot::new(
            workflow_id("workflow"),
            "recipe".to_owned(),
            WorkflowRunStatus::Completed,
            data,
            root,
            metadata,
        );

        assert_eq!(snapshot.workflow_id().as_str(), "workflow");
        assert_eq!(snapshot.workflow_name(), "recipe");
        assert_eq!(snapshot.status(), WorkflowRunStatus::Completed);
        assert_eq!(snapshot.inputs(), &serde_json::json!({"input": null}));
        assert_eq!(
            snapshot.artifacts(),
            &serde_json::json!({"run": "artifact"})
        );
        assert_eq!(
            snapshot.captured_outputs(),
            &serde_json::json!({"capture": [true, false]})
        );
        assert_eq!(snapshot.created_at(), "created");
        assert_eq!(snapshot.plan_revision(), u32::MAX);
        assert_eq!(
            snapshot.parent_session_id().map(SessionId::as_str),
            Some("parent")
        );
        assert_eq!(snapshot.workspace_path(), Some("relative workspace"));
        assert_eq!(snapshot.root().children().len(), 1);

        let child = &snapshot.root().children()[0];
        assert_eq!(child.descriptor().node_id().as_str(), "child");
        assert_eq!(child.status(), WorkflowNodeStatus::Completed);
        assert!(child.children().is_empty());
        assert_eq!(child.session_id().map(SessionId::as_str), Some("session"));
        assert_eq!(
            child.artifacts(),
            Some(&serde_json::json!({"artifact": [1, 1]}))
        );
        assert_eq!(
            child.captured_output(),
            Some(&serde_json::json!({"output": true}))
        );
        assert_eq!(child.failure_reason(), Some("reason"));
        assert_eq!(child.iteration(), Some(u32::MAX));
        assert_eq!(child.branch_id(), Some("branch"));
        assert_eq!(
            child.completion_signal(),
            Some(WorkflowCompletionSignal::Success)
        );
        assert_eq!(
            child.completion_signal_source(),
            Some(WorkflowCompletionSignalSource::StatusUpdate)
        );
        assert_eq!(child.started_at(), Some("started"));
        assert_eq!(child.ended_at(), Some("ended"));

        let minimal = WorkflowNodeSnapshot::new(
            WorkflowNodeDescriptor::watch(node_id("watch"), String::new()),
            WorkflowNodeStatus::Pending,
            Vec::new(),
        );
        assert!(minimal.session_id().is_none());
        assert!(minimal.artifacts().is_none());
        assert!(minimal.captured_output().is_none());
        assert!(minimal.failure_reason().is_none());
        assert!(minimal.iteration().is_none());
        assert!(minimal.branch_id().is_none());
        assert!(minimal.completion_signal().is_none());
        assert!(minimal.completion_signal_source().is_none());
        assert!(minimal.started_at().is_none());
        assert!(minimal.ended_at().is_none());
        let metadata = WorkflowSnapshotMetadata::new(String::new(), 0);
        assert!(metadata.parent_session_id().is_none());
        assert!(metadata.workspace_path().is_none());
    }

    #[test]
    fn workflow_snapshot_manifest_field_matrix() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../../../.cyril-6beh/oracle-manifest.json"))
                .expect("oracle manifest is valid JSON");
        assert_eq!(
            manifest["snapshot_owned_run_fields"],
            serde_json::json!([
                "workflowName",
                "status",
                "inputs",
                "artifacts",
                "capturedOutputs",
                "createdAt",
                "planRevision",
                "parentSessionId",
                "workspacePath",
                "root"
            ])
        );
        assert_eq!(
            manifest["snapshot_owned_node_fields"],
            serde_json::json!([
                "descriptor",
                "status",
                "sessionId",
                "artifacts",
                "capturedOutput",
                "failureReason",
                "iteration",
                "branchId",
                "completionSignal",
                "completionSignalSource",
                "startedAt",
                "endedAt"
            ])
        );
    }

    #[test]
    fn workflow_snapshot_optional_presence_matrix() {
        for mask in 0_u16..1 << 10 {
            let mut node = WorkflowNodeSnapshot::new(
                WorkflowNodeDescriptor::step(
                    node_id("node"),
                    String::new(),
                    (mask & 1 != 0).then(String::new),
                    (mask & 2 != 0).then(String::new),
                ),
                WorkflowNodeStatus::Pending,
                Vec::new(),
            );
            if mask & 1 != 0 {
                node = node.with_session_id(SessionId::new(""));
            }
            if mask & 2 != 0 {
                node = node.with_artifacts(serde_json::Value::Null);
            }
            if mask & 4 != 0 {
                node = node.with_captured_output(serde_json::Value::Null);
            }
            if mask & 8 != 0 {
                node = node.with_failure_reason(String::new());
            }
            if mask & 16 != 0 {
                node = node.with_iteration(0);
            }
            if mask & 32 != 0 {
                node = node.with_branch_id(String::new());
            }
            if mask & 64 != 0 {
                node = node.with_completion_signal(WorkflowCompletionSignal::NeedInput);
            }
            if mask & 128 != 0 {
                node =
                    node.with_completion_signal_source(WorkflowCompletionSignalSource::SendMessage);
            }
            if mask & 256 != 0 {
                node = node.with_started_at(String::new());
            }
            if mask & 512 != 0 {
                node = node.with_ended_at(String::new());
            }

            assert_eq!(node.session_id().is_some(), mask & 1 != 0);
            assert_eq!(node.artifacts().is_some(), mask & 2 != 0);
            assert_eq!(node.captured_output().is_some(), mask & 4 != 0);
            assert_eq!(node.failure_reason().is_some(), mask & 8 != 0);
            assert_eq!(node.iteration().is_some(), mask & 16 != 0);
            assert_eq!(node.branch_id().is_some(), mask & 32 != 0);
            assert_eq!(node.completion_signal().is_some(), mask & 64 != 0);
            assert_eq!(node.completion_signal_source().is_some(), mask & 128 != 0);
            assert_eq!(node.started_at().is_some(), mask & 256 != 0);
            assert_eq!(node.ended_at().is_some(), mask & 512 != 0);
            assert_eq!(node.descriptor().model().is_some(), mask & 1 != 0);
            assert_eq!(node.descriptor().effort().is_some(), mask & 2 != 0);
        }

        for mask in 0_u8..4 {
            let repeat = WorkflowNodeDescriptor::repeat(
                node_id("repeat"),
                Vec::new(),
                0,
                WorkflowRepeatExhaustion::Abort,
                (mask & 1 != 0).then_some(serde_json::Value::Null),
                (mask & 2 != 0).then_some(serde_json::Value::Null),
            );
            assert_eq!(repeat.stop_condition().is_some(), mask & 1 != 0);
            assert_eq!(repeat.stop_when().is_some(), mask & 2 != 0);

            let mut metadata = WorkflowSnapshotMetadata::new(String::new(), 0);
            if mask & 1 != 0 {
                metadata = metadata.with_parent_session_id(SessionId::new(""));
            }
            if mask & 2 != 0 {
                metadata = metadata.with_workspace_path(String::new());
            }
            assert_eq!(metadata.parent_session_id().is_some(), mask & 1 != 0);
            assert_eq!(metadata.workspace_path().is_some(), mask & 2 != 0);
        }
    }

    #[test]
    fn workflow_recursive_shape_stress_preserves_fields() {
        let mut chain =
            WorkflowNodeDescriptor::step(node_id("shared"), "leaf".to_owned(), None, None);
        for depth in 1..10 {
            chain =
                WorkflowNodeDescriptor::sequence(node_id(&format!("depth-{depth}")), vec![chain]);
        }
        let mut forest = vec![chain];
        for _ in 0..246 {
            forest.push(WorkflowNodeDescriptor::step(
                node_id("shared"),
                "leaf".to_owned(),
                None,
                None,
            ));
        }

        assert_eq!(descriptor_count(&forest), 256);
        assert_eq!(descriptor_depth(&forest[0]), 10);
        assert_eq!(forest[1].node_id(), forest[2].node_id());
    }

    fn workflow_id(value: &str) -> WorkflowId {
        WorkflowId::try_from(value.to_owned()).expect("valid workflow id")
    }

    fn node_id(value: &str) -> WorkflowNodeId {
        WorkflowNodeId::try_from(value.to_owned()).expect("valid node id")
    }

    fn descriptor_count(nodes: &[WorkflowNodeDescriptor]) -> usize {
        nodes
            .iter()
            .map(|node| 1 + descriptor_count(node.children()))
            .sum()
    }

    fn descriptor_depth(node: &WorkflowNodeDescriptor) -> usize {
        1 + node
            .children()
            .iter()
            .map(descriptor_depth)
            .max()
            .unwrap_or(0)
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
