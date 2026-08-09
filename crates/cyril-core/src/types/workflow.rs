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

/// Owned node-snapshot fields consumed by the workflow state module.
#[cfg(feature = "kas")]
pub(crate) struct WorkflowNodeSnapshotParts {
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

#[cfg(feature = "kas")]
pub(crate) type WorkflowNodeSnapshotValues = (
    WorkflowNodeDescriptor,
    WorkflowNodeStatus,
    Option<SessionId>,
    Option<serde_json::Value>,
    Option<serde_json::Value>,
    Option<String>,
    Option<u32>,
    Option<String>,
    Option<WorkflowCompletionSignal>,
    Option<WorkflowCompletionSignalSource>,
    Option<String>,
    Option<String>,
);

#[cfg(feature = "kas")]
impl WorkflowNodeSnapshotParts {
    pub(crate) fn descriptor(&self) -> &WorkflowNodeDescriptor {
        &self.descriptor
    }

    pub(crate) fn take_children(&mut self) -> Vec<WorkflowNodeSnapshot> {
        std::mem::take(&mut self.children)
    }

    pub(crate) fn into_values(self) -> WorkflowNodeSnapshotValues {
        (
            self.descriptor,
            self.status,
            self.session_id,
            self.artifacts,
            self.captured_output,
            self.failure_reason,
            self.iteration,
            self.branch_id,
            self.completion_signal,
            self.completion_signal_source,
            self.started_at,
            self.ended_at,
        )
    }
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

    /// Moves every field into the workflow state canonicalizer.
    #[cfg(feature = "kas")]
    pub(crate) fn into_parts(self) -> WorkflowNodeSnapshotParts {
        WorkflowNodeSnapshotParts {
            descriptor: self.descriptor,
            status: self.status,
            children: self.children,
            session_id: self.session_id,
            artifacts: self.artifacts,
            captured_output: self.captured_output,
            failure_reason: self.failure_reason,
            iteration: self.iteration,
            branch_id: self.branch_id,
            completion_signal: self.completion_signal,
            completion_signal_source: self.completion_signal_source,
            started_at: self.started_at,
            ended_at: self.ended_at,
        }
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
/// Owned snapshot fields consumed by the workflow state module.
#[cfg(feature = "kas")]
pub(crate) struct WorkflowSnapshotParts {
    workflow_id: WorkflowId,
    workflow_name: String,
    status: WorkflowRunStatus,
    inputs: serde_json::Value,
    artifacts: serde_json::Value,
    captured_outputs: serde_json::Value,
    root: WorkflowNodeSnapshot,
    created_at: String,
    plan_revision: u32,
    parent_session_id: Option<SessionId>,
    workspace_path: Option<String>,
}

#[cfg(feature = "kas")]
pub(crate) type WorkflowSnapshotValues = (
    WorkflowId,
    String,
    WorkflowRunStatus,
    serde_json::Value,
    serde_json::Value,
    serde_json::Value,
    WorkflowNodeSnapshot,
    String,
    u32,
    Option<SessionId>,
    Option<String>,
);

#[cfg(feature = "kas")]
impl WorkflowSnapshotParts {
    pub(crate) fn into_values(self) -> WorkflowSnapshotValues {
        (
            self.workflow_id,
            self.workflow_name,
            self.status,
            self.inputs,
            self.artifacts,
            self.captured_outputs,
            self.root,
            self.created_at,
            self.plan_revision,
            self.parent_session_id,
            self.workspace_path,
        )
    }
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

    /// Moves every field into the workflow state canonicalizer.
    #[cfg(feature = "kas")]
    pub(crate) fn into_parts(self) -> WorkflowSnapshotParts {
        WorkflowSnapshotParts {
            workflow_id: self.workflow_id,
            workflow_name: self.workflow_name,
            status: self.status,
            inputs: self.data.inputs,
            artifacts: self.data.artifacts,
            captured_outputs: self.data.captured_outputs,
            root: self.root,
            created_at: self.metadata.created_at,
            plan_revision: self.metadata.plan_revision,
            parent_session_id: self.metadata.parent_session_id,
            workspace_path: self.metadata.workspace_path,
        }
    }
}

/// Validation failure for a canonical workflow-node path.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkflowNodePathError {
    #[error("workflow node path cannot be empty")]
    Empty,
    #[error("workflow node path root `{actual}` does not match workflow id `{expected}`")]
    RootMismatch { expected: String, actual: String },
    #[error("workflow node path segment {index} cannot be empty")]
    EmptySegment { index: usize },
}

/// Canonical path of one runtime node within a workflow.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkflowNodePath(Box<[String]>);

impl WorkflowNodePath {
    /// Validates and constructs a canonical path for `workflow_id`.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty path, a root other than `workflow_id`, or
    /// an empty segment. Every accepted segment is moved into the path without
    /// copying.
    pub fn try_new(
        workflow_id: &WorkflowId,
        segments: Vec<String>,
    ) -> Result<Self, WorkflowNodePathError> {
        let Some(root) = segments.first() else {
            return Err(WorkflowNodePathError::Empty);
        };
        if root != workflow_id.as_str() {
            return Err(WorkflowNodePathError::RootMismatch {
                expected: workflow_id.as_str().to_owned(),
                actual: root.clone(),
            });
        }
        if let Some(index) = segments.iter().position(String::is_empty) {
            return Err(WorkflowNodePathError::EmptySegment { index });
        }
        Ok(Self(segments.into_boxed_slice()))
    }
    /// Returns every canonical path segment in order.
    pub fn segments(&self) -> &[String] {
        &self.0
    }
}

/// Optional fields carried by a `node_start` event.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkflowNodeStartDetails {
    agent_name: Option<String>,
    session_id: Option<SessionId>,
    prompt: Option<String>,
    iteration: Option<u32>,
    branch_id: Option<String>,
}

impl WorkflowNodeStartDetails {
    /// Constructs an event detail set with every optional field absent.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the supplied agent name.
    pub fn with_agent_name(mut self, agent_name: String) -> Self {
        self.agent_name = Some(agent_name);
        self
    }

    /// Sets the supplied peer-session identifier.
    pub fn with_session_id(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Sets the supplied prompt.
    pub fn with_prompt(mut self, prompt: String) -> Self {
        self.prompt = Some(prompt);
        self
    }

    /// Sets the supplied repeat iteration.
    pub fn with_iteration(mut self, iteration: u32) -> Self {
        self.iteration = Some(iteration);
        self
    }

    /// Sets the supplied parallel branch identifier.
    pub fn with_branch_id(mut self, branch_id: String) -> Self {
        self.branch_id = Some(branch_id);
        self
    }

    /// Returns the supplied agent name.
    pub fn agent_name(&self) -> Option<&str> {
        self.agent_name.as_deref()
    }

    /// Returns the supplied peer-session identifier.
    pub fn session_id(&self) -> Option<&SessionId> {
        self.session_id.as_ref()
    }

    /// Returns the supplied prompt.
    pub fn prompt(&self) -> Option<&str> {
        self.prompt.as_deref()
    }

    /// Returns the supplied repeat iteration.
    pub fn iteration(&self) -> Option<u32> {
        self.iteration
    }

    /// Returns the supplied parallel branch identifier.
    pub fn branch_id(&self) -> Option<&str> {
        self.branch_id.as_deref()
    }
}

/// Optional fields carried by a `node_complete` event.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorkflowNodeCompletionDetails {
    artifacts: Option<serde_json::Value>,
    captured_output: Option<serde_json::Value>,
    failure_reason: Option<String>,
    completion_signal: Option<WorkflowCompletionSignal>,
    completion_signal_source: Option<WorkflowCompletionSignalSource>,
}

impl WorkflowNodeCompletionDetails {
    /// Constructs an event detail set with every optional field absent.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets supplied opaque artifacts.
    pub fn with_artifacts(mut self, artifacts: serde_json::Value) -> Self {
        self.artifacts = Some(artifacts);
        self
    }

    /// Sets supplied opaque captured output.
    pub fn with_captured_output(mut self, output: serde_json::Value) -> Self {
        self.captured_output = Some(output);
        self
    }

    /// Sets the supplied failure reason.
    pub fn with_failure_reason(mut self, reason: String) -> Self {
        self.failure_reason = Some(reason);
        self
    }

    /// Sets supplied completion-signal metadata.
    pub fn with_completion_signal(mut self, signal: WorkflowCompletionSignal) -> Self {
        self.completion_signal = Some(signal);
        self
    }

    /// Sets supplied completion-signal source metadata.
    pub fn with_completion_signal_source(mut self, source: WorkflowCompletionSignalSource) -> Self {
        self.completion_signal_source = Some(source);
        self
    }

    /// Returns supplied opaque artifacts.
    pub fn artifacts(&self) -> Option<&serde_json::Value> {
        self.artifacts.as_ref()
    }

    /// Returns supplied opaque captured output.
    pub fn captured_output(&self) -> Option<&serde_json::Value> {
        self.captured_output.as_ref()
    }

    /// Returns the supplied failure reason.
    pub fn failure_reason(&self) -> Option<&str> {
        self.failure_reason.as_deref()
    }

    /// Returns supplied completion-signal metadata.
    pub fn completion_signal(&self) -> Option<WorkflowCompletionSignal> {
        self.completion_signal
    }

    /// Returns supplied completion-signal source metadata.
    pub fn completion_signal_source(&self) -> Option<WorkflowCompletionSignalSource> {
        self.completion_signal_source
    }
}

/// Opening data from `run_start`.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowRunStarted {
    workflow_id: WorkflowId,
    workflow_name: String,
    inputs: serde_json::Value,
    node_tree: Vec<WorkflowNodeDescriptor>,
    parent_session_id: Option<SessionId>,
}

impl WorkflowRunStarted {
    /// Constructs a complete run-opening event.
    pub fn new(
        workflow_id: WorkflowId,
        workflow_name: String,
        inputs: serde_json::Value,
        node_tree: Vec<WorkflowNodeDescriptor>,
        parent_session_id: Option<SessionId>,
    ) -> Self {
        Self {
            workflow_id,
            workflow_name,
            inputs,
            node_tree,
            parent_session_id,
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

    /// Returns opaque recipe inputs.
    pub fn inputs(&self) -> &serde_json::Value {
        &self.inputs
    }

    /// Returns the declared recipe forest.
    pub fn node_tree(&self) -> &[WorkflowNodeDescriptor] {
        &self.node_tree
    }

    /// Returns the parent session when supplied.
    pub fn parent_session_id(&self) -> Option<&SessionId> {
        self.parent_session_id.as_ref()
    }
}

/// Runtime node opening from `node_start`.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowNodeStarted {
    workflow_id: WorkflowId,
    node_id: WorkflowNodeId,
    node_path: WorkflowNodePath,
    node_type: WorkflowNodeType,
    details: WorkflowNodeStartDetails,
}

impl WorkflowNodeStarted {
    /// Constructs a node-opening event.
    pub fn new(
        workflow_id: WorkflowId,
        node_id: WorkflowNodeId,
        node_path: WorkflowNodePath,
        node_type: WorkflowNodeType,
        details: WorkflowNodeStartDetails,
    ) -> Self {
        Self {
            workflow_id,
            node_id,
            node_path,
            node_type,
            details,
        }
    }

    /// Returns the persisted workflow identifier.
    pub fn workflow_id(&self) -> &WorkflowId {
        &self.workflow_id
    }

    /// Returns the event's node identifier.
    pub fn node_id(&self) -> &WorkflowNodeId {
        &self.node_id
    }

    /// Returns the canonical runtime path.
    pub fn node_path(&self) -> &WorkflowNodePath {
        &self.node_path
    }

    /// Returns the event's structural node type.
    pub fn node_type(&self) -> WorkflowNodeType {
        self.node_type
    }

    /// Returns optional node-opening details.
    pub fn details(&self) -> &WorkflowNodeStartDetails {
        &self.details
    }
}

/// Runtime node update from `node_complete`.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowNodeCompleted {
    workflow_id: WorkflowId,
    node_id: WorkflowNodeId,
    node_path: WorkflowNodePath,
    status: WorkflowNodeStatus,
    details: WorkflowNodeCompletionDetails,
}

impl WorkflowNodeCompleted {
    /// Constructs a node-completion update.
    pub fn new(
        workflow_id: WorkflowId,
        node_id: WorkflowNodeId,
        node_path: WorkflowNodePath,
        status: WorkflowNodeStatus,
        details: WorkflowNodeCompletionDetails,
    ) -> Self {
        Self {
            workflow_id,
            node_id,
            node_path,
            status,
            details,
        }
    }

    /// Returns the persisted workflow identifier.
    pub fn workflow_id(&self) -> &WorkflowId {
        &self.workflow_id
    }

    /// Returns the event's node identifier.
    pub fn node_id(&self) -> &WorkflowNodeId {
        &self.node_id
    }

    /// Returns the canonical runtime path.
    pub fn node_path(&self) -> &WorkflowNodePath {
        &self.node_path
    }

    /// Returns the authoritative event status.
    pub fn status(&self) -> WorkflowNodeStatus {
        self.status
    }

    /// Returns optional node-completion details.
    pub fn details(&self) -> &WorkflowNodeCompletionDetails {
        &self.details
    }
}

/// Runtime node pause from `node_paused`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowNodePaused {
    workflow_id: WorkflowId,
    node_id: WorkflowNodeId,
    node_path: WorkflowNodePath,
    reason: String,
}

impl WorkflowNodePaused {
    /// Constructs a node-pause update.
    pub fn new(
        workflow_id: WorkflowId,
        node_id: WorkflowNodeId,
        node_path: WorkflowNodePath,
        reason: String,
    ) -> Self {
        Self {
            workflow_id,
            node_id,
            node_path,
            reason,
        }
    }

    /// Returns the persisted workflow identifier.
    pub fn workflow_id(&self) -> &WorkflowId {
        &self.workflow_id
    }

    /// Returns the event's node identifier.
    pub fn node_id(&self) -> &WorkflowNodeId {
        &self.node_id
    }

    /// Returns the canonical runtime path.
    pub fn node_path(&self) -> &WorkflowNodePath {
        &self.node_path
    }

    /// Returns the opaque pause reason.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// Latest repeat progress from `loop_iteration`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowLoopIteration {
    workflow_id: WorkflowId,
    loop_id: WorkflowNodeId,
    iteration: u32,
    stop_condition_met: bool,
}

impl WorkflowLoopIteration {
    /// Constructs a repeat-progress update.
    pub fn new(
        workflow_id: WorkflowId,
        loop_id: WorkflowNodeId,
        iteration: u32,
        stop_condition_met: bool,
    ) -> Self {
        Self {
            workflow_id,
            loop_id,
            iteration,
            stop_condition_met,
        }
    }

    /// Returns the persisted workflow identifier.
    pub fn workflow_id(&self) -> &WorkflowId {
        &self.workflow_id
    }

    /// Returns the repeat node identifier.
    pub fn loop_id(&self) -> &WorkflowNodeId {
        &self.loop_id
    }

    /// Returns the current repeat iteration.
    pub fn iteration(&self) -> u32 {
        self.iteration
    }

    /// Reports whether the repeat's stop condition matched.
    pub fn stop_condition_met(&self) -> bool {
        self.stop_condition_met
    }
}

/// Latest watch progress from `watch_poll`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowWatchPoll {
    workflow_id: WorkflowId,
    node_id: WorkflowNodeId,
    node_path: WorkflowNodePath,
    outcome: WorkflowWatchOutcome,
    at: String,
}

impl WorkflowWatchPoll {
    /// Constructs a watch-progress update.
    pub fn new(
        workflow_id: WorkflowId,
        node_id: WorkflowNodeId,
        node_path: WorkflowNodePath,
        outcome: WorkflowWatchOutcome,
        at: String,
    ) -> Self {
        Self {
            workflow_id,
            node_id,
            node_path,
            outcome,
            at,
        }
    }

    /// Returns the persisted workflow identifier.
    pub fn workflow_id(&self) -> &WorkflowId {
        &self.workflow_id
    }

    /// Returns the watch node identifier.
    pub fn node_id(&self) -> &WorkflowNodeId {
        &self.node_id
    }

    /// Returns the canonical runtime path.
    pub fn node_path(&self) -> &WorkflowNodePath {
        &self.node_path
    }

    /// Returns the current watch outcome.
    pub fn outcome(&self) -> WorkflowWatchOutcome {
        self.outcome
    }

    /// Returns the opaque poll timestamp.
    pub fn at(&self) -> &str {
        &self.at
    }
}

/// Run pause from `paused`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowPaused {
    workflow_id: WorkflowId,
    pause_reason: String,
}

impl WorkflowPaused {
    /// Constructs a run-pause update.
    pub fn new(workflow_id: WorkflowId, pause_reason: String) -> Self {
        Self {
            workflow_id,
            pause_reason,
        }
    }

    /// Returns the persisted workflow identifier.
    pub fn workflow_id(&self) -> &WorkflowId {
        &self.workflow_id
    }

    /// Returns the opaque pause reason.
    pub fn pause_reason(&self) -> &str {
        &self.pause_reason
    }
}

/// Error returned when `run_complete` contradicts its final snapshot.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "workflow completion status `{completion}` does not match final snapshot status `{snapshot}`"
)]
pub struct WorkflowCompletionMismatchError {
    completion: WorkflowCompletionStatus,
    snapshot: WorkflowRunStatus,
}

/// Reconciled run result from `run_complete`.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowRunCompleted {
    workflow_id: WorkflowId,
    status: WorkflowCompletionStatus,
    final_state: Box<WorkflowSnapshot>,
}

impl WorkflowRunCompleted {
    /// Constructs a completion after verifying status agreement.
    pub fn new(
        workflow_id: WorkflowId,
        status: WorkflowCompletionStatus,
        final_state: WorkflowSnapshot,
    ) -> Result<Self, WorkflowCompletionMismatchError> {
        let matches = matches!(
            (status, final_state.status()),
            (WorkflowCompletionStatus::Paused, WorkflowRunStatus::Paused)
                | (
                    WorkflowCompletionStatus::Completed,
                    WorkflowRunStatus::Completed
                )
                | (WorkflowCompletionStatus::Failed, WorkflowRunStatus::Failed)
                | (
                    WorkflowCompletionStatus::Aborted,
                    WorkflowRunStatus::Aborted
                )
        );
        if !matches {
            return Err(WorkflowCompletionMismatchError {
                completion: status,
                snapshot: final_state.status(),
            });
        }
        Ok(Self {
            workflow_id,
            status,
            final_state: Box::new(final_state),
        })
    }

    /// Returns the persisted workflow identifier.
    pub fn workflow_id(&self) -> &WorkflowId {
        &self.workflow_id
    }

    /// Returns the completion-event status.
    pub fn status(&self) -> WorkflowCompletionStatus {
        self.status
    }

    /// Returns the authoritative final snapshot.
    pub fn final_state(&self) -> &WorkflowSnapshot {
        &self.final_state
    }

    /// Moves the authoritative final snapshot into the workflow state machine.
    #[cfg(feature = "kas")]
    pub(crate) fn into_final_state(self) -> WorkflowSnapshot {
        *self.final_state
    }
}

/// Optional acknowledgement attached to `steps_queued`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowQueueResolution {
    outcome: WorkflowQueueOutcome,
    reason: Option<String>,
}

impl WorkflowQueueResolution {
    /// Constructs a queue acknowledgement.
    pub fn new(outcome: WorkflowQueueOutcome, reason: Option<String>) -> Self {
        Self { outcome, reason }
    }

    /// Returns the acknowledgement outcome.
    pub fn outcome(&self) -> WorkflowQueueOutcome {
        self.outcome
    }

    /// Returns the opaque acknowledgement reason when supplied.
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// Queued recipe descriptors from `steps_queued`.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowStepsQueued {
    workflow_id: WorkflowId,
    pending_steps: Vec<WorkflowNodeDescriptor>,
    resolution: Option<WorkflowQueueResolution>,
}

impl WorkflowStepsQueued {
    /// Constructs a queued-step update.
    pub fn new(
        workflow_id: WorkflowId,
        pending_steps: Vec<WorkflowNodeDescriptor>,
        resolution: Option<WorkflowQueueResolution>,
    ) -> Self {
        Self {
            workflow_id,
            pending_steps,
            resolution,
        }
    }

    /// Returns the persisted workflow identifier.
    pub fn workflow_id(&self) -> &WorkflowId {
        &self.workflow_id
    }

    /// Returns the queued recipe descriptors.
    pub fn pending_steps(&self) -> &[WorkflowNodeDescriptor] {
        &self.pending_steps
    }

    /// Returns the acknowledgement when supplied.
    pub fn resolution(&self) -> Option<&WorkflowQueueResolution> {
        self.resolution.as_ref()
    }
}

/// One of KAS's nine workflow lifecycle notifications.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowEvent {
    RunStarted(WorkflowRunStarted),
    NodeStarted(WorkflowNodeStarted),
    NodeCompleted(WorkflowNodeCompleted),
    NodePaused(WorkflowNodePaused),
    LoopIteration(WorkflowLoopIteration),
    WatchPoll(WorkflowWatchPoll),
    Paused(WorkflowPaused),
    RunCompleted(WorkflowRunCompleted),
    StepsQueued(WorkflowStepsQueued),
}

impl WorkflowEvent {
    /// Returns the exact unprefixed lifecycle method suffix.
    pub fn method_name(&self) -> &'static str {
        match self {
            Self::RunStarted(_) => "run_start",
            Self::NodeStarted(_) => "node_start",
            Self::NodeCompleted(_) => "node_complete",
            Self::NodePaused(_) => "node_paused",
            Self::LoopIteration(_) => "loop_iteration",
            Self::WatchPoll(_) => "watch_poll",
            Self::Paused(_) => "paused",
            Self::RunCompleted(_) => "run_complete",
            Self::StepsQueued(_) => "steps_queued",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn must_succeed<T, E: std::fmt::Debug>(result: Result<T, E>, context: &str) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("{context}: {error:?}"),
        }
    }

    fn must_fail<T: std::fmt::Debug, E>(result: Result<T, E>, context: &str) -> E {
        match result {
            Ok(value) => panic!("{context}: unexpected value {value:?}"),
            Err(error) => error,
        }
    }

    fn must_exist<T>(value: Option<T>, context: &str) -> T {
        match value {
            Some(value) => value,
            None => panic!("{context}"),
        }
    }

    fn load_manifest() -> serde_json::Value {
        must_succeed(
            serde_json::from_str(include_str!("../../../../.cyril-6beh/oracle-manifest.json")),
            "oracle manifest is valid JSON",
        )
    }

    #[test]
    fn workflow_identifier_string_matrix() {
        let accepted = ["id", "識別子", "with space", "#", "/", "\\"];
        for raw in accepted {
            let workflow = must_succeed(
                WorkflowId::try_from(raw.to_owned()),
                "non-empty workflow id",
            );
            let node = must_succeed(
                WorkflowNodeId::try_from(raw.to_owned()),
                "non-empty node id",
            );
            assert_eq!(workflow.as_str(), raw);
            assert_eq!(node.as_str(), raw);
        }

        let large = "x".repeat(65_536);
        let pointer = large.as_ptr();
        let workflow = must_succeed(WorkflowId::try_from(large), "large workflow id");
        assert_eq!(workflow.as_str().as_ptr(), pointer);
        assert_eq!(workflow.as_str().len(), 65_536);
        assert!(WorkflowId::try_from(String::new()).is_err());
        assert!(WorkflowNodeId::try_from(String::new()).is_err());
    }

    #[test]
    fn workflow_enum_domain_matrix() {
        let manifest = load_manifest();
        let domains = must_exist(
            manifest["enum_domains"].as_object(),
            "enum_domains is an object",
        );

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
        assert_eq!(sequence.children(), std::slice::from_ref(&step));

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

        let manifest = load_manifest();
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
        let manifest = load_manifest();
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

    #[test]
    fn workflow_event_read_model_matrix() {
        let workflow_id = workflow_id("workflow");
        let node_id = node_id("node");
        let node_path = workflow_path(&workflow_id, &["workflow", "node"]);
        assert_eq!(node_path.segments(), ["workflow", "node"]);
        let descriptor =
            WorkflowNodeDescriptor::step(node_id.clone(), "agent".to_owned(), None, None);

        let run_started = WorkflowRunStarted::new(
            workflow_id.clone(),
            "recipe".to_owned(),
            serde_json::json!({"input": 1}),
            vec![descriptor.clone()],
            Some(SessionId::new("parent")),
        );
        assert_eq!(run_started.workflow_id(), &workflow_id);
        assert_eq!(run_started.workflow_name(), "recipe");
        assert_eq!(run_started.inputs(), &serde_json::json!({"input": 1}));
        assert_eq!(run_started.node_tree(), std::slice::from_ref(&descriptor));
        assert_eq!(
            run_started.parent_session_id().map(SessionId::as_str),
            Some("parent")
        );

        let start_details = WorkflowNodeStartDetails::new()
            .with_agent_name("agent".to_owned())
            .with_session_id(SessionId::new("session"))
            .with_prompt("prompt".to_owned())
            .with_iteration(u32::MAX)
            .with_branch_id("branch".to_owned());
        let node_started = WorkflowNodeStarted::new(
            workflow_id.clone(),
            node_id.clone(),
            node_path.clone(),
            WorkflowNodeType::Step,
            start_details,
        );
        assert_eq!(node_started.workflow_id(), &workflow_id);
        assert_eq!(node_started.node_id(), &node_id);
        assert_eq!(node_started.node_path(), &node_path);
        assert_eq!(node_started.node_type(), WorkflowNodeType::Step);
        assert_eq!(node_started.details().agent_name(), Some("agent"));
        assert_eq!(
            node_started.details().session_id().map(SessionId::as_str),
            Some("session")
        );
        assert_eq!(node_started.details().prompt(), Some("prompt"));
        assert_eq!(node_started.details().iteration(), Some(u32::MAX));
        assert_eq!(node_started.details().branch_id(), Some("branch"));

        let completion_details = WorkflowNodeCompletionDetails::new()
            .with_artifacts(serde_json::json!({"artifact": true}))
            .with_captured_output(serde_json::json!(["output"]))
            .with_failure_reason("failure".to_owned())
            .with_completion_signal(WorkflowCompletionSignal::Error)
            .with_completion_signal_source(WorkflowCompletionSignalSource::SendMessage);
        let node_completed = WorkflowNodeCompleted::new(
            workflow_id.clone(),
            node_id.clone(),
            node_path.clone(),
            WorkflowNodeStatus::Failed,
            completion_details,
        );
        assert_eq!(node_completed.workflow_id(), &workflow_id);
        assert_eq!(node_completed.node_id(), &node_id);
        assert_eq!(node_completed.node_path(), &node_path);
        assert_eq!(node_completed.status(), WorkflowNodeStatus::Failed);
        assert_eq!(
            node_completed.details().artifacts(),
            Some(&serde_json::json!({"artifact": true}))
        );
        assert_eq!(
            node_completed.details().captured_output(),
            Some(&serde_json::json!(["output"]))
        );
        assert_eq!(node_completed.details().failure_reason(), Some("failure"));
        assert_eq!(
            node_completed.details().completion_signal(),
            Some(WorkflowCompletionSignal::Error)
        );
        assert_eq!(
            node_completed.details().completion_signal_source(),
            Some(WorkflowCompletionSignalSource::SendMessage)
        );

        let node_paused = WorkflowNodePaused::new(
            workflow_id.clone(),
            node_id.clone(),
            node_path.clone(),
            "node reason".to_owned(),
        );
        assert_eq!(node_paused.workflow_id(), &workflow_id);
        assert_eq!(node_paused.node_id(), &node_id);
        assert_eq!(node_paused.node_path(), &node_path);
        assert_eq!(node_paused.reason(), "node reason");

        let loop_iteration =
            WorkflowLoopIteration::new(workflow_id.clone(), node_id.clone(), u32::MAX, false);
        assert_eq!(loop_iteration.workflow_id(), &workflow_id);
        assert_eq!(loop_iteration.loop_id(), &node_id);
        assert_eq!(loop_iteration.iteration(), u32::MAX);
        assert!(!loop_iteration.stop_condition_met());

        let watch_poll = WorkflowWatchPoll::new(
            workflow_id.clone(),
            node_id.clone(),
            node_path.clone(),
            WorkflowWatchOutcome::IdleTimeout,
            "timestamp".to_owned(),
        );
        assert_eq!(watch_poll.workflow_id(), &workflow_id);
        assert_eq!(watch_poll.node_id(), &node_id);
        assert_eq!(watch_poll.node_path(), &node_path);
        assert_eq!(watch_poll.outcome(), WorkflowWatchOutcome::IdleTimeout);
        assert_eq!(watch_poll.at(), "timestamp");

        let paused = WorkflowPaused::new(workflow_id.clone(), "run reason".to_owned());
        assert_eq!(paused.workflow_id(), &workflow_id);
        assert_eq!(paused.pause_reason(), "run reason");

        let snapshot = completed_snapshot(workflow_id.clone());
        let run_completed = must_succeed(
            WorkflowRunCompleted::new(
                workflow_id.clone(),
                WorkflowCompletionStatus::Completed,
                snapshot,
            ),
            "matching completion status",
        );
        assert_eq!(run_completed.workflow_id(), &workflow_id);
        assert_eq!(run_completed.status(), WorkflowCompletionStatus::Completed);
        assert_eq!(run_completed.final_state().workflow_id(), &workflow_id);

        let resolution = WorkflowQueueResolution::new(
            WorkflowQueueOutcome::Applied,
            Some("approved".to_owned()),
        );
        assert_eq!(resolution.outcome(), WorkflowQueueOutcome::Applied);
        assert_eq!(resolution.reason(), Some("approved"));
        let steps_queued =
            WorkflowStepsQueued::new(workflow_id.clone(), vec![descriptor], Some(resolution));
        assert_eq!(steps_queued.workflow_id(), &workflow_id);
        assert_eq!(steps_queued.pending_steps().len(), 1);
        assert_eq!(
            steps_queued
                .resolution()
                .map(WorkflowQueueResolution::outcome),
            Some(WorkflowQueueOutcome::Applied)
        );

        let events = [
            WorkflowEvent::RunStarted(run_started),
            WorkflowEvent::NodeStarted(node_started),
            WorkflowEvent::NodeCompleted(node_completed),
            WorkflowEvent::NodePaused(node_paused),
            WorkflowEvent::LoopIteration(loop_iteration),
            WorkflowEvent::WatchPoll(watch_poll),
            WorkflowEvent::Paused(paused),
            WorkflowEvent::RunCompleted(run_completed),
            WorkflowEvent::StepsQueued(steps_queued),
        ];
        assert_eq!(
            events.map(|event| event.method_name()),
            [
                "run_start",
                "node_start",
                "node_complete",
                "node_paused",
                "loop_iteration",
                "watch_poll",
                "paused",
                "run_complete",
                "steps_queued",
            ]
        );
    }

    #[test]
    fn workflow_run_completion_rejects_status_mismatch() {
        let workflow_id = workflow_id("workflow");
        let error = must_fail(
            WorkflowRunCompleted::new(
                workflow_id.clone(),
                WorkflowCompletionStatus::Failed,
                completed_snapshot(workflow_id),
            ),
            "mismatched completion status must fail",
        );
        assert_eq!(
            error.to_string(),
            "workflow completion status `failed` does not match final snapshot status `completed`"
        );
    }

    #[test]
    fn workflow_event_optional_presence_matrix() {
        let workflow_id = workflow_id("workflow");
        let node_id = node_id("node");
        let node_path = workflow_path(&workflow_id, &["workflow", "node"]);
        for mask in 0_u8..1 << 5 {
            let mut start = WorkflowNodeStartDetails::new();
            if mask & 1 != 0 {
                start = start.with_agent_name(String::new());
            }
            if mask & 2 != 0 {
                start = start.with_session_id(SessionId::new(""));
            }
            if mask & 4 != 0 {
                start = start.with_prompt(String::new());
            }
            if mask & 8 != 0 {
                start = start.with_iteration(0);
            }
            if mask & 16 != 0 {
                start = start.with_branch_id(String::new());
            }
            assert_eq!(start.agent_name().is_some(), mask & 1 != 0);
            assert_eq!(start.session_id().is_some(), mask & 2 != 0);
            assert_eq!(start.prompt().is_some(), mask & 4 != 0);
            assert_eq!(start.iteration().is_some(), mask & 8 != 0);
            assert_eq!(start.branch_id().is_some(), mask & 16 != 0);

            let mut completion = WorkflowNodeCompletionDetails::new();
            if mask & 1 != 0 {
                completion = completion.with_artifacts(serde_json::Value::Null);
            }
            if mask & 2 != 0 {
                completion = completion.with_captured_output(serde_json::Value::Null);
            }
            if mask & 4 != 0 {
                completion = completion.with_failure_reason(String::new());
            }
            if mask & 8 != 0 {
                completion = completion.with_completion_signal(WorkflowCompletionSignal::Success);
            }
            if mask & 16 != 0 {
                completion = completion
                    .with_completion_signal_source(WorkflowCompletionSignalSource::StatusUpdate);
            }
            assert_eq!(completion.artifacts().is_some(), mask & 1 != 0);
            assert_eq!(completion.captured_output().is_some(), mask & 2 != 0);
            assert_eq!(completion.failure_reason().is_some(), mask & 4 != 0);
            assert_eq!(completion.completion_signal().is_some(), mask & 8 != 0);
            assert_eq!(
                completion.completion_signal_source().is_some(),
                mask & 16 != 0
            );

            let started = WorkflowNodeStarted::new(
                workflow_id.clone(),
                node_id.clone(),
                node_path.clone(),
                WorkflowNodeType::Step,
                start,
            );
            let completed = WorkflowNodeCompleted::new(
                workflow_id.clone(),
                node_id.clone(),
                node_path.clone(),
                WorkflowNodeStatus::Pending,
                completion,
            );
            assert_eq!(started.details().agent_name().is_some(), mask & 1 != 0);
            assert_eq!(completed.details().artifacts().is_some(), mask & 1 != 0);
        }

        for mask in 0_u8..4 {
            let resolution = (mask & 1 != 0).then(|| {
                WorkflowQueueResolution::new(
                    WorkflowQueueOutcome::Rejected,
                    (mask & 2 != 0).then(String::new),
                )
            });
            let queued = WorkflowStepsQueued::new(workflow_id.clone(), Vec::new(), resolution);
            assert_eq!(queued.resolution().is_some(), mask & 1 != 0);
            assert_eq!(
                queued
                    .resolution()
                    .and_then(WorkflowQueueResolution::reason)
                    .is_some(),
                mask == 3
            );
        }
    }

    #[test]
    fn workflow_event_field_inventory_matches_manifest() {
        let manifest = load_manifest();
        let projection = serde_json::json!({
            "run_start": {
                "required": ["workflowId", "workflowName", "inputs", "nodeTree"],
                "optional": ["parentSessionId"]
            },
            "node_start": {
                "required": ["workflowId", "nodeId", "nodePath", "type"],
                "optional": ["agentName", "sessionId", "prompt", "iteration", "branchId"]
            },
            "node_complete": {
                "required": ["workflowId", "nodeId", "nodePath", "status"],
                "optional": ["artifacts", "capturedOutput", "failureReason", "completionSignal", "completionSignalSource"]
            },
            "node_paused": {
                "required": ["workflowId", "nodeId", "nodePath", "reason"],
                "optional": []
            },
            "loop_iteration": {
                "required": ["workflowId", "loopId", "iteration", "stopConditionMet"],
                "optional": []
            },
            "watch_poll": {
                "required": ["workflowId", "nodeId", "nodePath", "outcome", "at"],
                "optional": []
            },
            "paused": {
                "required": ["workflowId", "pauseReason"],
                "optional": []
            },
            "run_complete": {
                "required": ["workflowId", "status", "finalState"],
                "optional": []
            },
            "steps_queued": {
                "required": ["workflowId", "pendingSteps"],
                "optional": ["resolution"]
            }
        });
        assert_eq!(projection, manifest["fields"]);
    }

    #[test]
    fn workflow_node_path_validation_matrix() {
        let workflow_id = workflow_id("workflow");
        assert_eq!(
            WorkflowNodePath::try_new(&workflow_id, Vec::new()),
            Err(WorkflowNodePathError::Empty)
        );
        let mismatch = must_fail(
            WorkflowNodePath::try_new(&workflow_id, vec!["other".to_owned()]),
            "wrong root must fail",
        );
        assert_eq!(
            mismatch.to_string(),
            "workflow node path root `other` does not match workflow id `workflow`"
        );
        assert_eq!(
            WorkflowNodePath::try_new(&workflow_id, vec!["workflow".to_owned(), String::new()]),
            Err(WorkflowNodePathError::EmptySegment { index: 1 })
        );

        let root = must_succeed(
            WorkflowNodePath::try_new(&workflow_id, vec!["workflow".to_owned()]),
            "root-only path is valid",
        );
        assert_eq!(root.segments(), ["workflow"]);

        let large = "x".repeat(65_536);
        let pointer = large.as_ptr();
        let path = must_succeed(
            WorkflowNodePath::try_new(
                &workflow_id,
                vec![
                    "workflow".to_owned(),
                    "識別子 with space #".to_owned(),
                    large,
                ],
            ),
            "non-empty opaque segments are valid",
        );
        assert_eq!(path.segments()[2].as_ptr(), pointer);
    }

    fn workflow_path(workflow_id: &WorkflowId, segments: &[&str]) -> WorkflowNodePath {
        must_succeed(
            WorkflowNodePath::try_new(
                workflow_id,
                segments
                    .iter()
                    .map(|segment| (*segment).to_owned())
                    .collect(),
            ),
            "valid workflow path",
        )
    }

    fn completed_snapshot(workflow_id: WorkflowId) -> WorkflowSnapshot {
        let root_id = must_succeed(
            WorkflowNodeId::try_from(workflow_id.as_str().to_owned()),
            "valid root node id",
        );
        WorkflowSnapshot::new(
            workflow_id,
            "recipe".to_owned(),
            WorkflowRunStatus::Completed,
            WorkflowSnapshotData::new(
                serde_json::json!({}),
                serde_json::json!({}),
                serde_json::json!({}),
            ),
            WorkflowNodeSnapshot::new(
                WorkflowNodeDescriptor::sequence(root_id, Vec::new()),
                WorkflowNodeStatus::Completed,
                Vec::new(),
            ),
            WorkflowSnapshotMetadata::new(String::new(), 0),
        )
    }

    fn workflow_id(value: &str) -> WorkflowId {
        must_succeed(WorkflowId::try_from(value.to_owned()), "valid workflow id")
    }

    fn node_id(value: &str) -> WorkflowNodeId {
        must_succeed(WorkflowNodeId::try_from(value.to_owned()), "valid node id")
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
        let expected = must_exist(domains[name].as_array(), "enum domain is an array")
            .iter()
            .map(|value| must_exist(value.as_str(), "enum value is a string"))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}
