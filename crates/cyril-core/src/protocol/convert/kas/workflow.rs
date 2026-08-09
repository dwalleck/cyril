//! Conversion for KAS `kiro/workflow/*` lifecycle notifications.
//!
//! The ACP crate removes the leading underscore from extension method names,
//! so this module matches the normalized `kiro/workflow/*` spelling exactly.

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::types::{
    Notification, SessionId, WorkflowCompletionMismatchError, WorkflowCompletionSignal,
    WorkflowCompletionSignalSource, WorkflowCompletionStatus, WorkflowEvent, WorkflowId,
    WorkflowIdentifierError, WorkflowLoopIteration, WorkflowNodeCompleted,
    WorkflowNodeCompletionDetails, WorkflowNodeDescriptor, WorkflowNodeId, WorkflowNodePath,
    WorkflowNodePathError, WorkflowNodePaused, WorkflowNodeSnapshot, WorkflowNodeStartDetails,
    WorkflowNodeStarted, WorkflowNodeStatus, WorkflowNodeType, WorkflowPaused,
    WorkflowQueueOutcome, WorkflowQueueResolution, WorkflowRepeatExhaustion, WorkflowRunCompleted,
    WorkflowRunStarted, WorkflowRunStatus, WorkflowSnapshot, WorkflowSnapshotData,
    WorkflowSnapshotMetadata, WorkflowStepsQueued, WorkflowWatchOutcome, WorkflowWatchPoll,
};

/// Distinguishes an absent optional field from a present non-null value.
///
/// Serde's `Option<T>` maps both missing and explicit `null` to `None`. The
/// workflow covenant allows omission but rejects explicit null for structured
/// optional fields, so the adapter must retain that distinction while parsing.
#[derive(Debug, Default)]
enum OptionalField<T> {
    #[default]
    Missing,
    Present(T),
}

impl<'de, T> Deserialize<'de> for OptionalField<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer)?
            .map(Self::Present)
            .ok_or_else(|| serde::de::Error::custom("explicit null is not allowed"))
    }
}

impl<T> OptionalField<T> {
    fn into_option(self) -> Option<T> {
        match self {
            Self::Missing => None,
            Self::Present(value) => Some(value),
        }
    }
}

/// Optional opaque JSON where explicit `null` is valid data, not absence.
#[derive(Debug, Default)]
struct OptionalValue(OptionalField<serde_json::Value>);

impl<'de> Deserialize<'de> for OptionalValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        serde_json::Value::deserialize(deserializer)
            .map(OptionalField::Present)
            .map(Self)
    }
}

impl OptionalValue {
    fn into_option(self) -> Option<serde_json::Value> {
        self.0.into_option()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkflowErrorKind {
    MissingRequired,
    WrongType,
    InvalidEnum,
    InvalidValue,
    StatusMismatch,
}

impl WorkflowErrorKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::MissingRequired => "missing_required",
            Self::WrongType => "wrong_type",
            Self::InvalidEnum => "invalid_enum",
            Self::InvalidValue => "invalid_value",
            Self::StatusMismatch => "status_mismatch",
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum WorkflowAdapterError {
    #[error("{error}")]
    MalformedField {
        field_path: String,
        error_kind: WorkflowErrorKind,
        error: String,
    },
    #[error("invalid `{field}`: {source}")]
    InvalidWorkflowId {
        field: &'static str,
        #[source]
        source: WorkflowIdentifierError,
    },
    #[error("invalid `{field}`: {source}")]
    InvalidNodeId {
        field: &'static str,
        #[source]
        source: WorkflowIdentifierError,
    },
    #[error(transparent)]
    InvalidNodePath(#[from] WorkflowNodePathError),
    #[error("{node_type} node is missing required `{field}`")]
    MissingNodeField {
        node_type: &'static str,
        field: &'static str,
    },
    #[error(
        "run completion workflow id `{outer}` does not match final snapshot workflow id `{final_id}`"
    )]
    SnapshotWorkflowMismatch { outer: String, final_id: String },
    #[error(transparent)]
    CompletionMismatch(#[from] WorkflowCompletionMismatchError),
}

impl WorkflowAdapterError {
    fn field_path(&self) -> &str {
        match self {
            Self::MalformedField { field_path, .. } => field_path,
            Self::InvalidWorkflowId { field, .. } | Self::InvalidNodeId { field, .. } => field,
            Self::InvalidNodePath(_) => "nodePath",
            Self::MissingNodeField { field, .. } => field,
            Self::SnapshotWorkflowMismatch { .. } => "finalState.workflowId",
            Self::CompletionMismatch(_) => "status",
        }
    }

    fn error_kind(&self) -> WorkflowErrorKind {
        match self {
            Self::MalformedField { error_kind, .. } => *error_kind,
            Self::InvalidWorkflowId { .. }
            | Self::InvalidNodeId { .. }
            | Self::InvalidNodePath(_) => WorkflowErrorKind::InvalidValue,
            Self::MissingNodeField { .. } => WorkflowErrorKind::MissingRequired,
            Self::SnapshotWorkflowMismatch { .. } => WorkflowErrorKind::InvalidValue,
            Self::CompletionMismatch(_) => WorkflowErrorKind::StatusMismatch,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireRunStarted {
    workflow_id: String,
    workflow_name: String,
    inputs: serde_json::Value,
    node_tree: Vec<WireNodeDescriptor>,
    #[serde(default)]
    parent_session_id: OptionalField<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireRunCompleted {
    workflow_id: String,
    status: WireCompletionStatus,
    final_state: WireSnapshot,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireNodeStarted {
    workflow_id: String,
    node_id: String,
    node_path: Vec<String>,
    #[serde(rename = "type")]
    node_type: WireNodeType,
    #[serde(default)]
    agent_name: OptionalField<String>,
    #[serde(default)]
    session_id: OptionalField<String>,
    #[serde(default)]
    prompt: OptionalField<String>,
    #[serde(default)]
    iteration: OptionalField<u32>,
    #[serde(default)]
    branch_id: OptionalField<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireNodeCompleted {
    workflow_id: String,
    node_id: String,
    node_path: Vec<String>,
    status: WireNodeStatus,
    #[serde(default)]
    artifacts: OptionalValue,
    #[serde(default)]
    captured_output: OptionalValue,
    #[serde(default)]
    failure_reason: OptionalField<String>,
    #[serde(default)]
    completion_signal: OptionalField<WireCompletionSignal>,
    #[serde(default)]
    completion_signal_source: OptionalField<WireCompletionSignalSource>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireNodePaused {
    workflow_id: String,
    node_id: String,
    node_path: Vec<String>,
    reason: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireLoopIteration {
    workflow_id: String,
    loop_id: String,
    iteration: u32,
    stop_condition_met: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireWatchPoll {
    workflow_id: String,
    node_id: String,
    node_path: Vec<String>,
    outcome: WireWatchOutcome,
    at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WirePaused {
    workflow_id: String,
    pause_reason: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireStepsQueued {
    workflow_id: String,
    pending_steps: Vec<WireNodeDescriptor>,
    #[serde(default)]
    resolution: OptionalField<WireQueueResolution>,
}

#[derive(Deserialize)]
struct WireQueueResolution {
    outcome: WireQueueOutcome,
    #[serde(default)]
    reason: OptionalField<String>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum WireNodeDescriptor {
    #[serde(rename = "step")]
    Step {
        #[serde(rename = "nodeId")]
        node_id: String,
        #[serde(rename = "agentName")]
        agent_name: String,
        #[serde(rename = "model", default)]
        model: OptionalField<String>,
        #[serde(rename = "effort", default)]
        effort: OptionalField<String>,
    },
    #[serde(rename = "sequence")]
    Sequence {
        #[serde(rename = "nodeId")]
        node_id: String,
        steps: Vec<Self>,
    },
    #[serde(rename = "repeat")]
    Repeat {
        #[serde(rename = "nodeId")]
        node_id: String,
        steps: Vec<Self>,
        #[serde(rename = "maxIterations")]
        max_iterations: u32,
        #[serde(rename = "onMaxIterations")]
        on_max_iterations: WireRepeatExhaustion,
        #[serde(rename = "stopCondition", default)]
        stop_condition: OptionalValue,
        #[serde(rename = "stopWhen", default)]
        stop_when: OptionalValue,
    },
    #[serde(rename = "parallel")]
    Parallel {
        #[serde(rename = "nodeId")]
        node_id: String,
        branches: Vec<Self>,
    },
    #[serde(rename = "watch")]
    Watch {
        #[serde(rename = "nodeId")]
        node_id: String,
        #[serde(rename = "handlerName")]
        handler_name: String,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireSnapshot {
    workflow_id: String,
    workflow_name: String,
    status: WireRunStatus,
    inputs: serde_json::Value,
    artifacts: serde_json::Value,
    captured_outputs: serde_json::Value,
    root: WireNodeSnapshot,
    created_at: String,
    plan_revision: u32,
    #[serde(default)]
    parent_session_id: OptionalField<String>,
    #[serde(default)]
    workspace_path: OptionalField<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireNodeSnapshot {
    node_id: String,
    #[serde(rename = "type")]
    node_type: WireNodeType,
    status: WireNodeStatus,
    #[serde(default)]
    children: OptionalField<Vec<Self>>,
    #[serde(default)]
    agent_name: OptionalField<String>,
    #[serde(default)]
    model: OptionalField<String>,
    #[serde(default)]
    effort: OptionalField<String>,
    #[serde(default)]
    max_iterations: OptionalField<u32>,
    #[serde(default)]
    on_max_iterations: OptionalField<WireRepeatExhaustion>,
    #[serde(default)]
    stop_condition: OptionalValue,
    #[serde(default)]
    stop_when: OptionalValue,
    #[serde(default)]
    handler_name: OptionalField<String>,
    #[serde(default)]
    session_id: OptionalField<String>,
    #[serde(default)]
    artifacts: OptionalValue,
    #[serde(default)]
    captured_output: OptionalValue,
    #[serde(default)]
    failure_reason: OptionalField<String>,
    #[serde(default)]
    iteration: OptionalField<u32>,
    #[serde(default)]
    branch_id: OptionalField<String>,
    #[serde(default)]
    completion_signal: OptionalField<WireCompletionSignal>,
    #[serde(default)]
    completion_signal_source: OptionalField<WireCompletionSignalSource>,
    #[serde(default)]
    started_at: OptionalField<String>,
    #[serde(default)]
    ended_at: OptionalField<String>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum WireRunStatus {
    Running,
    Paused,
    Completed,
    Failed,
    Aborted,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum WireCompletionStatus {
    Paused,
    Completed,
    Failed,
    Aborted,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum WireNodeStatus {
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Aborted,
    Skipped,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum WireNodeType {
    Step,
    Sequence,
    Repeat,
    Parallel,
    Watch,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum WireRepeatExhaustion {
    Pause,
    Abort,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireCompletionSignal {
    Success,
    NeedInput,
    Error,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireCompletionSignalSource {
    SendMessage,
    StatusUpdate,
}

#[derive(Clone, Copy, Deserialize)]
enum WireWatchOutcome {
    #[serde(rename = "new-activity")]
    NewActivity,
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "idle-timeout")]
    IdleTimeout,
    #[serde(rename = "terminal-state")]
    TerminalState,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum WireQueueOutcome {
    Applied,
    Rejected,
    Dropped,
}

/// Returns `None` when `method` is not owned by this adapter. For an exact
/// workflow method, returns `Some(Some(notification))` on success or
/// `Some(None)` after warning and dropping malformed input.
pub(super) fn to_notification(
    method: &str,
    params: &serde_json::Value,
) -> Option<Option<Notification>> {
    let event = match method {
        "kiro/workflow/run_start" => parse_run_started(params),
        "kiro/workflow/node_start" => parse_node_started(params),
        "kiro/workflow/node_complete" => parse_node_completed(params),
        "kiro/workflow/node_paused" => parse_node_paused(params),
        "kiro/workflow/loop_iteration" => parse_loop_iteration(params),
        "kiro/workflow/watch_poll" => parse_watch_poll(params),
        "kiro/workflow/paused" => parse_paused(params),
        "kiro/workflow/run_complete" => parse_run_completed(params),
        "kiro/workflow/steps_queued" => parse_steps_queued(params),
        _ => return None,
    };

    Some(match event {
        Ok(event) => Some(Notification::Workflow(Box::new(event))),
        Err(error) => {
            tracing::warn!(
                method,
                field_path = error.field_path(),
                error_kind = error.error_kind().as_str(),
                error = %error,
                "malformed workflow notification"
            );
            None
        }
    })
}

fn parse_run_started(params: &serde_json::Value) -> Result<WorkflowEvent, WorkflowAdapterError> {
    let wire: WireRunStarted = deserialize(params)?;
    let workflow_id = workflow_id(wire.workflow_id, "workflowId")?;
    let node_tree = wire
        .node_tree
        .into_iter()
        .map(WireNodeDescriptor::try_into_domain)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(WorkflowEvent::RunStarted(WorkflowRunStarted::new(
        workflow_id,
        wire.workflow_name,
        wire.inputs,
        node_tree,
        wire.parent_session_id.into_option().map(SessionId::new),
    )))
}

fn parse_node_started(params: &serde_json::Value) -> Result<WorkflowEvent, WorkflowAdapterError> {
    let wire: WireNodeStarted = deserialize(params)?;
    let workflow_id = workflow_id(wire.workflow_id, "workflowId")?;
    let node_path = WorkflowNodePath::try_new(&workflow_id, wire.node_path)?;
    let mut details = WorkflowNodeStartDetails::new();
    if let Some(agent_name) = wire.agent_name.into_option() {
        details = details.with_agent_name(agent_name);
    }
    if let Some(session_id) = wire.session_id.into_option() {
        details = details.with_session_id(SessionId::new(session_id));
    }
    if let Some(prompt) = wire.prompt.into_option() {
        details = details.with_prompt(prompt);
    }
    if let Some(iteration) = wire.iteration.into_option() {
        details = details.with_iteration(iteration);
    }
    if let Some(branch_id) = wire.branch_id.into_option() {
        details = details.with_branch_id(branch_id);
    }
    Ok(WorkflowEvent::NodeStarted(WorkflowNodeStarted::new(
        workflow_id,
        node_id(wire.node_id)?,
        node_path,
        wire.node_type.into(),
        details,
    )))
}

fn parse_node_completed(params: &serde_json::Value) -> Result<WorkflowEvent, WorkflowAdapterError> {
    let wire: WireNodeCompleted = deserialize(params)?;
    let workflow_id = workflow_id(wire.workflow_id, "workflowId")?;
    let node_path = WorkflowNodePath::try_new(&workflow_id, wire.node_path)?;
    let mut details = WorkflowNodeCompletionDetails::new();
    if let Some(artifacts) = wire.artifacts.into_option() {
        details = details.with_artifacts(artifacts);
    }
    if let Some(captured_output) = wire.captured_output.into_option() {
        details = details.with_captured_output(captured_output);
    }
    if let Some(failure_reason) = wire.failure_reason.into_option() {
        details = details.with_failure_reason(failure_reason);
    }
    if let Some(completion_signal) = wire.completion_signal.into_option() {
        details = details.with_completion_signal(completion_signal.into());
    }
    if let Some(source) = wire.completion_signal_source.into_option() {
        details = details.with_completion_signal_source(source.into());
    }
    Ok(WorkflowEvent::NodeCompleted(WorkflowNodeCompleted::new(
        workflow_id,
        node_id(wire.node_id)?,
        node_path,
        wire.status.into(),
        details,
    )))
}

fn parse_node_paused(params: &serde_json::Value) -> Result<WorkflowEvent, WorkflowAdapterError> {
    let wire: WireNodePaused = deserialize(params)?;
    let workflow_id = workflow_id(wire.workflow_id, "workflowId")?;
    let node_path = WorkflowNodePath::try_new(&workflow_id, wire.node_path)?;
    Ok(WorkflowEvent::NodePaused(WorkflowNodePaused::new(
        workflow_id,
        node_id(wire.node_id)?,
        node_path,
        wire.reason,
    )))
}

fn parse_loop_iteration(params: &serde_json::Value) -> Result<WorkflowEvent, WorkflowAdapterError> {
    let wire: WireLoopIteration = deserialize(params)?;
    Ok(WorkflowEvent::LoopIteration(WorkflowLoopIteration::new(
        workflow_id(wire.workflow_id, "workflowId")?,
        loop_id(wire.loop_id)?,
        wire.iteration,
        wire.stop_condition_met,
    )))
}

fn parse_watch_poll(params: &serde_json::Value) -> Result<WorkflowEvent, WorkflowAdapterError> {
    let wire: WireWatchPoll = deserialize(params)?;
    let workflow_id = workflow_id(wire.workflow_id, "workflowId")?;
    let node_path = WorkflowNodePath::try_new(&workflow_id, wire.node_path)?;
    Ok(WorkflowEvent::WatchPoll(WorkflowWatchPoll::new(
        workflow_id,
        node_id(wire.node_id)?,
        node_path,
        wire.outcome.into(),
        wire.at,
    )))
}

fn parse_paused(params: &serde_json::Value) -> Result<WorkflowEvent, WorkflowAdapterError> {
    let wire: WirePaused = deserialize(params)?;
    Ok(WorkflowEvent::Paused(WorkflowPaused::new(
        workflow_id(wire.workflow_id, "workflowId")?,
        wire.pause_reason,
    )))
}

fn parse_steps_queued(params: &serde_json::Value) -> Result<WorkflowEvent, WorkflowAdapterError> {
    let wire: WireStepsQueued = deserialize(params)?;
    let pending_steps = wire
        .pending_steps
        .into_iter()
        .map(WireNodeDescriptor::try_into_domain)
        .collect::<Result<Vec<_>, _>>()?;
    let resolution = wire.resolution.into_option().map(|resolution| {
        WorkflowQueueResolution::new(resolution.outcome.into(), resolution.reason.into_option())
    });
    Ok(WorkflowEvent::StepsQueued(WorkflowStepsQueued::new(
        workflow_id(wire.workflow_id, "workflowId")?,
        pending_steps,
        resolution,
    )))
}

fn parse_run_completed(params: &serde_json::Value) -> Result<WorkflowEvent, WorkflowAdapterError> {
    let wire: WireRunCompleted = deserialize(params)?;
    let workflow_id = workflow_id(wire.workflow_id, "workflowId")?;
    let status = WorkflowCompletionStatus::from(wire.status);
    let final_state = wire.final_state.try_into_domain()?;
    if final_state.workflow_id() != &workflow_id {
        return Err(WorkflowAdapterError::SnapshotWorkflowMismatch {
            outer: workflow_id.as_str().to_owned(),
            final_id: final_state.workflow_id().as_str().to_owned(),
        });
    }
    Ok(WorkflowEvent::RunCompleted(WorkflowRunCompleted::new(
        workflow_id,
        status,
        final_state,
    )?))
}

fn deserialize<T: DeserializeOwned>(value: &serde_json::Value) -> Result<T, WorkflowAdapterError> {
    let result: Result<T, _> = serde_path_to_error::deserialize(value);
    result.map_err(|error| {
        let message = error.inner().to_string();
        let field_path = serde_field_path(error.path().to_string(), &message);
        WorkflowAdapterError::MalformedField {
            field_path,
            error_kind: classify_serde_error(&message),
            error: message,
        }
    })
}

fn serde_field_path(mut path: String, message: &str) -> String {
    if path == "." {
        path.clear();
    }
    if let Some(field) = missing_field_name(message) {
        if !path.is_empty() {
            path.push('.');
        }
        path.push_str(field);
    }
    if path.is_empty() {
        "$".to_owned()
    } else {
        path
    }
}

fn missing_field_name(message: &str) -> Option<&str> {
    message
        .strip_prefix("missing field `")
        .and_then(|rest| rest.split_once('`').map(|(field, _)| field))
}

fn classify_serde_error(message: &str) -> WorkflowErrorKind {
    if message.starts_with("missing field `") {
        WorkflowErrorKind::MissingRequired
    } else if message.starts_with("unknown variant `") {
        WorkflowErrorKind::InvalidEnum
    } else if message.starts_with("invalid type:") {
        WorkflowErrorKind::WrongType
    } else {
        WorkflowErrorKind::InvalidValue
    }
}

impl WireNodeDescriptor {
    fn try_into_domain(self) -> Result<WorkflowNodeDescriptor, WorkflowAdapterError> {
        match self {
            Self::Step {
                node_id: raw_node_id,
                agent_name,
                model,
                effort,
            } => Ok(WorkflowNodeDescriptor::step(
                node_id(raw_node_id)?,
                agent_name,
                model.into_option(),
                effort.into_option(),
            )),
            Self::Sequence {
                node_id: raw_node_id,
                steps,
            } => Ok(WorkflowNodeDescriptor::sequence(
                node_id(raw_node_id)?,
                convert_descriptors(steps)?,
            )),
            Self::Repeat {
                node_id: raw_node_id,
                steps,
                max_iterations,
                on_max_iterations,
                stop_condition,
                stop_when,
            } => Ok(WorkflowNodeDescriptor::repeat(
                node_id(raw_node_id)?,
                convert_descriptors(steps)?,
                max_iterations,
                on_max_iterations.into(),
                stop_condition.into_option(),
                stop_when.into_option(),
            )),
            Self::Parallel {
                node_id: raw_node_id,
                branches,
            } => Ok(WorkflowNodeDescriptor::parallel(
                node_id(raw_node_id)?,
                convert_descriptors(branches)?,
            )),
            Self::Watch {
                node_id: raw_node_id,
                handler_name,
            } => Ok(WorkflowNodeDescriptor::watch(
                node_id(raw_node_id)?,
                handler_name,
            )),
        }
    }
}

fn convert_descriptors(
    descriptors: Vec<WireNodeDescriptor>,
) -> Result<Vec<WorkflowNodeDescriptor>, WorkflowAdapterError> {
    descriptors
        .into_iter()
        .map(WireNodeDescriptor::try_into_domain)
        .collect()
}

impl WireSnapshot {
    fn try_into_domain(self) -> Result<WorkflowSnapshot, WorkflowAdapterError> {
        let mut metadata = WorkflowSnapshotMetadata::new(self.created_at, self.plan_revision);
        if let Some(parent_session_id) = self.parent_session_id.into_option() {
            metadata = metadata.with_parent_session_id(SessionId::new(parent_session_id));
        }
        if let Some(workspace_path) = self.workspace_path.into_option() {
            metadata = metadata.with_workspace_path(workspace_path);
        }
        Ok(WorkflowSnapshot::new(
            workflow_id(self.workflow_id, "finalState.workflowId")?,
            self.workflow_name,
            self.status.into(),
            WorkflowSnapshotData::new(self.inputs, self.artifacts, self.captured_outputs),
            self.root.try_into_domain()?,
            metadata,
        ))
    }
}

impl WireNodeSnapshot {
    fn try_into_domain(self) -> Result<WorkflowNodeSnapshot, WorkflowAdapterError> {
        let node_type_name = self.node_type.as_str();
        let descriptor = match self.node_type {
            WireNodeType::Step => WorkflowNodeDescriptor::step(
                node_id(self.node_id)?,
                required(self.agent_name, node_type_name, "agentName")?,
                self.model.into_option(),
                self.effort.into_option(),
            ),
            WireNodeType::Sequence => {
                WorkflowNodeDescriptor::sequence(node_id(self.node_id)?, Vec::new())
            }
            WireNodeType::Repeat => WorkflowNodeDescriptor::repeat(
                node_id(self.node_id)?,
                Vec::new(),
                required(self.max_iterations, node_type_name, "maxIterations")?,
                required(self.on_max_iterations, node_type_name, "onMaxIterations")?.into(),
                self.stop_condition.into_option(),
                self.stop_when.into_option(),
            ),
            WireNodeType::Parallel => {
                WorkflowNodeDescriptor::parallel(node_id(self.node_id)?, Vec::new())
            }
            WireNodeType::Watch => WorkflowNodeDescriptor::watch(
                node_id(self.node_id)?,
                required(self.handler_name, node_type_name, "handlerName")?,
            ),
        };
        let children = match self.children.into_option() {
            Some(children) => children
                .into_iter()
                .map(Self::try_into_domain)
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };
        let mut snapshot = WorkflowNodeSnapshot::new(descriptor, self.status.into(), children);
        if let Some(session_id) = self.session_id.into_option() {
            snapshot = snapshot.with_session_id(SessionId::new(session_id));
        }
        if let Some(artifacts) = self.artifacts.into_option() {
            snapshot = snapshot.with_artifacts(artifacts);
        }
        if let Some(captured_output) = self.captured_output.into_option() {
            snapshot = snapshot.with_captured_output(captured_output);
        }
        if let Some(failure_reason) = self.failure_reason.into_option() {
            snapshot = snapshot.with_failure_reason(failure_reason);
        }
        if let Some(iteration) = self.iteration.into_option() {
            snapshot = snapshot.with_iteration(iteration);
        }
        if let Some(branch_id) = self.branch_id.into_option() {
            snapshot = snapshot.with_branch_id(branch_id);
        }
        if let Some(completion_signal) = self.completion_signal.into_option() {
            snapshot = snapshot.with_completion_signal(completion_signal.into());
        }
        if let Some(source) = self.completion_signal_source.into_option() {
            snapshot = snapshot.with_completion_signal_source(source.into());
        }
        if let Some(started_at) = self.started_at.into_option() {
            snapshot = snapshot.with_started_at(started_at);
        }
        if let Some(ended_at) = self.ended_at.into_option() {
            snapshot = snapshot.with_ended_at(ended_at);
        }
        Ok(snapshot)
    }
}

fn required<T>(
    field: OptionalField<T>,
    node_type: &'static str,
    field_name: &'static str,
) -> Result<T, WorkflowAdapterError> {
    field
        .into_option()
        .ok_or(WorkflowAdapterError::MissingNodeField {
            node_type,
            field: field_name,
        })
}

fn workflow_id(value: String, field: &'static str) -> Result<WorkflowId, WorkflowAdapterError> {
    WorkflowId::try_from(value)
        .map_err(|source| WorkflowAdapterError::InvalidWorkflowId { field, source })
}

fn node_id(value: String) -> Result<WorkflowNodeId, WorkflowAdapterError> {
    WorkflowNodeId::try_from(value).map_err(|source| WorkflowAdapterError::InvalidNodeId {
        field: "nodeId",
        source,
    })
}

fn loop_id(value: String) -> Result<WorkflowNodeId, WorkflowAdapterError> {
    WorkflowNodeId::try_from(value).map_err(|source| WorkflowAdapterError::InvalidNodeId {
        field: "loopId",
        source,
    })
}

impl WireNodeType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Step => "step",
            Self::Sequence => "sequence",
            Self::Repeat => "repeat",
            Self::Parallel => "parallel",
            Self::Watch => "watch",
        }
    }
}

impl From<WireNodeType> for WorkflowNodeType {
    fn from(value: WireNodeType) -> Self {
        match value {
            WireNodeType::Step => Self::Step,
            WireNodeType::Sequence => Self::Sequence,
            WireNodeType::Repeat => Self::Repeat,
            WireNodeType::Parallel => Self::Parallel,
            WireNodeType::Watch => Self::Watch,
        }
    }
}

impl From<WireRunStatus> for WorkflowRunStatus {
    fn from(value: WireRunStatus) -> Self {
        match value {
            WireRunStatus::Running => Self::Running,
            WireRunStatus::Paused => Self::Paused,
            WireRunStatus::Completed => Self::Completed,
            WireRunStatus::Failed => Self::Failed,
            WireRunStatus::Aborted => Self::Aborted,
        }
    }
}

impl From<WireCompletionStatus> for WorkflowCompletionStatus {
    fn from(value: WireCompletionStatus) -> Self {
        match value {
            WireCompletionStatus::Paused => Self::Paused,
            WireCompletionStatus::Completed => Self::Completed,
            WireCompletionStatus::Failed => Self::Failed,
            WireCompletionStatus::Aborted => Self::Aborted,
        }
    }
}

impl From<WireNodeStatus> for WorkflowNodeStatus {
    fn from(value: WireNodeStatus) -> Self {
        match value {
            WireNodeStatus::Pending => Self::Pending,
            WireNodeStatus::Running => Self::Running,
            WireNodeStatus::Paused => Self::Paused,
            WireNodeStatus::Completed => Self::Completed,
            WireNodeStatus::Failed => Self::Failed,
            WireNodeStatus::Aborted => Self::Aborted,
            WireNodeStatus::Skipped => Self::Skipped,
        }
    }
}

impl From<WireRepeatExhaustion> for WorkflowRepeatExhaustion {
    fn from(value: WireRepeatExhaustion) -> Self {
        match value {
            WireRepeatExhaustion::Pause => Self::Pause,
            WireRepeatExhaustion::Abort => Self::Abort,
        }
    }
}

impl From<WireCompletionSignal> for WorkflowCompletionSignal {
    fn from(value: WireCompletionSignal) -> Self {
        match value {
            WireCompletionSignal::Success => Self::Success,
            WireCompletionSignal::NeedInput => Self::NeedInput,
            WireCompletionSignal::Error => Self::Error,
        }
    }
}

impl From<WireWatchOutcome> for WorkflowWatchOutcome {
    fn from(value: WireWatchOutcome) -> Self {
        match value {
            WireWatchOutcome::NewActivity => Self::NewActivity,
            WireWatchOutcome::Idle => Self::Idle,
            WireWatchOutcome::IdleTimeout => Self::IdleTimeout,
            WireWatchOutcome::TerminalState => Self::TerminalState,
        }
    }
}

impl From<WireQueueOutcome> for WorkflowQueueOutcome {
    fn from(value: WireQueueOutcome) -> Self {
        match value {
            WireQueueOutcome::Applied => Self::Applied,
            WireQueueOutcome::Rejected => Self::Rejected,
            WireQueueOutcome::Dropped => Self::Dropped,
        }
    }
}
impl From<WireCompletionSignalSource> for WorkflowCompletionSignalSource {
    fn from(value: WireCompletionSignalSource) -> Self {
        match value {
            WireCompletionSignalSource::SendMessage => Self::SendMessage,
            WireCompletionSignalSource::StatusUpdate => Self::StatusUpdate,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use super::*;
    use crate::workflow::{WorkflowNodeState, WorkflowRun, WorkflowTracker};

    const FAILED_CAPTURE: &str =
        include_str!("../../../../../../.cyril-6beh/terminal-failed-2.16.2.jsonl");
    const ABORTED_CAPTURE: &str =
        include_str!("../../../../../../.cyril-6beh/terminal-aborted-2.16.2.jsonl");

    fn must_succeed<T, E: std::fmt::Debug>(result: Result<T, E>, context: &str) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("{context}: {error:?}"),
        }
    }

    fn event(result: Option<Option<Notification>>, context: &str) -> WorkflowEvent {
        match result {
            Some(Some(Notification::Workflow(event))) => *event,
            other => panic!("{context}: expected workflow notification, got {other:?}"),
        }
    }

    #[derive(Clone, Default)]
    struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for CaptureWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let mut output = self
                .0
                .lock()
                .map_err(|error| io::Error::other(error.to_string()))?;
            output.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CaptureWriter {
        type Writer = Self;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    fn capture_rejection(
        method: &str,
        params: &serde_json::Value,
    ) -> (Option<Option<Notification>>, serde_json::Value) {
        let capture = CaptureWriter::default();
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_current_span(false)
            .with_span_list(false)
            .with_writer(capture.clone())
            .finish();
        let result =
            tracing::subscriber::with_default(subscriber, || to_notification(method, params));
        let bytes = match capture.0.lock() {
            Ok(output) => output.clone(),
            Err(error) => panic!("workflow warning capture lock poisoned: {error}"),
        };
        let log = must_succeed(
            serde_json::from_slice(&bytes),
            "workflow warning must be one JSON event",
        );
        (result, log)
    }

    fn capture_params(source: &str, expected_status: &str) -> serde_json::Value {
        let mut matched = None;
        for line in source.lines() {
            let frame: serde_json::Value =
                must_succeed(serde_json::from_str(line), "capture line is valid JSON");
            let envelope = frame.get("parsed").unwrap_or(&frame);
            if envelope.get("method").and_then(serde_json::Value::as_str)
                == Some("_kiro/workflow/run_complete")
                && envelope
                    .pointer("/params/status")
                    .and_then(serde_json::Value::as_str)
                    == Some(expected_status)
            {
                matched = Some(envelope["params"].clone());
            }
        }
        let Some(params) = matched else {
            panic!("capture contains no {expected_status} run_complete");
        };
        params
    }

    fn workflow_projection(workflow_id: &WorkflowId, run: &WorkflowRun) -> serde_json::Value {
        let mut run_projection = serde_json::Map::from_iter([
            (
                "workflowId".to_owned(),
                serde_json::Value::String(workflow_id.as_str().to_owned()),
            ),
            (
                "workflowName".to_owned(),
                serde_json::Value::String(run.workflow_name().to_owned()),
            ),
            (
                "status".to_owned(),
                serde_json::Value::String(run.status().as_str().to_owned()),
            ),
            ("inputs".to_owned(), run.inputs().clone()),
            ("artifacts".to_owned(), run.artifacts().clone()),
            ("capturedOutputs".to_owned(), run.captured_outputs().clone()),
            (
                "createdAt".to_owned(),
                serde_json::Value::String(run.created_at().to_owned()),
            ),
            (
                "planRevision".to_owned(),
                serde_json::Value::from(run.plan_revision()),
            ),
        ]);
        insert_string(
            &mut run_projection,
            "parentSessionId",
            run.parent_session_id().map(SessionId::as_str),
        );
        insert_string(&mut run_projection, "workspacePath", run.workspace_path());

        let mut node_entries = run
            .nodes()
            .map(|(path, node)| {
                (
                    must_succeed(
                        serde_json::to_string(path.segments()),
                        "canonical path serializes",
                    ),
                    path,
                    node,
                )
            })
            .collect::<Vec<_>>();
        node_entries.sort_by(|(left, _, _), (right, _, _)| left.cmp(right));
        let nodes = node_entries
            .into_iter()
            .map(|(_, path, node)| {
                serde_json::json!({
                    "path": path.segments(),
                    "data": workflow_node_projection(node),
                })
            })
            .collect::<Vec<_>>();
        serde_json::json!({"run": run_projection, "nodes": nodes})
    }

    fn workflow_node_projection(node: &WorkflowNodeState) -> serde_json::Value {
        let descriptor = node.descriptor();
        let mut data = serde_json::Map::from_iter([
            (
                "nodeId".to_owned(),
                serde_json::Value::String(descriptor.node_id().as_str().to_owned()),
            ),
            (
                "type".to_owned(),
                serde_json::Value::String(descriptor.node_type().as_str().to_owned()),
            ),
            (
                "status".to_owned(),
                serde_json::Value::String(node.status().as_str().to_owned()),
            ),
        ]);
        insert_string(&mut data, "agentName", descriptor.agent_name());
        insert_string(&mut data, "model", descriptor.model());
        insert_string(&mut data, "effort", descriptor.effort());
        insert_u32(&mut data, "maxIterations", descriptor.max_iterations());
        insert_string(
            &mut data,
            "onMaxIterations",
            descriptor
                .on_max_iterations()
                .map(WorkflowRepeatExhaustion::as_str),
        );
        insert_value(&mut data, "stopCondition", descriptor.stop_condition());
        insert_value(&mut data, "stopWhen", descriptor.stop_when());
        insert_string(&mut data, "handlerName", descriptor.handler_name());
        insert_string(
            &mut data,
            "sessionId",
            node.session_id().map(SessionId::as_str),
        );
        insert_value(&mut data, "artifacts", node.artifacts());
        insert_value(&mut data, "capturedOutput", node.captured_output());
        insert_string(&mut data, "failureReason", node.failure_reason());
        insert_u32(&mut data, "iteration", node.iteration());
        insert_string(&mut data, "branchId", node.branch_id());
        insert_string(
            &mut data,
            "completionSignal",
            node.completion_signal()
                .map(WorkflowCompletionSignal::as_str),
        );
        insert_string(
            &mut data,
            "completionSignalSource",
            node.completion_signal_source()
                .map(WorkflowCompletionSignalSource::as_str),
        );
        insert_string(&mut data, "startedAt", node.started_at());
        insert_string(&mut data, "endedAt", node.ended_at());
        serde_json::Value::Object(data)
    }

    fn insert_string(
        object: &mut serde_json::Map<String, serde_json::Value>,
        key: &str,
        value: Option<&str>,
    ) {
        if let Some(value) = value {
            object.insert(key.to_owned(), serde_json::Value::String(value.to_owned()));
        }
    }

    fn insert_u32(
        object: &mut serde_json::Map<String, serde_json::Value>,
        key: &str,
        value: Option<u32>,
    ) {
        if let Some(value) = value {
            object.insert(key.to_owned(), serde_json::Value::from(value));
        }
    }

    fn insert_value(
        object: &mut serde_json::Map<String, serde_json::Value>,
        key: &str,
        value: Option<&serde_json::Value>,
    ) {
        if let Some(value) = value {
            object.insert(key.to_owned(), value.clone());
        }
    }

    #[test]
    fn field_rich_run_start_preserves_descriptor_tree() {
        let params = serde_json::json!({
            "workflowId": "wf-rich",
            "workflowName": "識別子 with space",
            "inputs": {
                "null": null,
                "integer": 18446744073709551615_u64,
                "nested": [{"duplicates": [1, 1]}]
            },
            "nodeTree": [
                {
                    "nodeId": "step",
                    "type": "step",
                    "agentName": "agent",
                    "model": "",
                    "effort": "high",
                    "unknown": true
                },
                {
                    "nodeId": "sequence",
                    "type": "sequence",
                    "steps": [{"nodeId": "nested", "type": "watch", "handlerName": "files"}]
                },
                {
                    "nodeId": "repeat",
                    "type": "repeat",
                    "steps": [],
                    "maxIterations": 4294967295_u32,
                    "onMaxIterations": "pause",
                    "stopCondition": {"containsText": "done"},
                    "stopWhen": ["predicate"]
                },
                {
                    "nodeId": "parallel",
                    "type": "parallel",
                    "branches": [{"nodeId": "branch", "type": "step", "agentName": "worker"}]
                },
                {"nodeId": "watch", "type": "watch", "handlerName": "handler"}
            ],
            "parentSessionId": "",
            "futureField": {"ignored": true}
        });

        let WorkflowEvent::RunStarted(opening) = event(
            to_notification("kiro/workflow/run_start", &params),
            "field-rich run_start",
        ) else {
            panic!("expected run_start");
        };
        assert_eq!(opening.workflow_id().as_str(), "wf-rich");
        assert_eq!(opening.workflow_name(), "識別子 with space");
        assert_eq!(opening.inputs(), &params["inputs"]);
        assert_eq!(opening.parent_session_id().map(SessionId::as_str), Some(""));

        let nodes = opening.node_tree();
        assert_eq!(nodes.len(), 5);
        assert_eq!(nodes[0].node_id().as_str(), "step");
        assert_eq!(nodes[0].agent_name(), Some("agent"));
        assert_eq!(nodes[0].model(), Some(""));
        assert_eq!(nodes[0].effort(), Some("high"));
        assert_eq!(nodes[1].children().len(), 1);
        assert_eq!(nodes[1].children()[0].handler_name(), Some("files"));
        assert_eq!(nodes[2].max_iterations(), Some(u32::MAX));
        assert_eq!(
            nodes[2].on_max_iterations(),
            Some(WorkflowRepeatExhaustion::Pause)
        );
        assert_eq!(
            nodes[2].stop_condition(),
            Some(&serde_json::json!({"containsText": "done"}))
        );
        assert_eq!(
            nodes[2].stop_when(),
            Some(&serde_json::json!(["predicate"]))
        );
        assert_eq!(nodes[3].children()[0].agent_name(), Some("worker"));
        assert_eq!(nodes[4].handler_name(), Some("handler"));
    }

    #[test]
    fn live_failed_and_aborted_run_completions_preserve_snapshots() {
        for (source, expected) in [
            (FAILED_CAPTURE, WorkflowCompletionStatus::Failed),
            (ABORTED_CAPTURE, WorkflowCompletionStatus::Aborted),
        ] {
            let params = capture_params(source, expected.as_str());
            let WorkflowEvent::RunCompleted(completion) = event(
                to_notification("kiro/workflow/run_complete", &params),
                "live run_complete",
            ) else {
                panic!("expected run_complete");
            };
            assert_eq!(completion.status(), expected);
            let snapshot = completion.final_state();
            assert_eq!(snapshot.status().as_str(), expected.as_str());
            assert_eq!(snapshot.workflow_id(), completion.workflow_id());
            assert_eq!(snapshot.inputs(), &params["finalState"]["inputs"]);
            assert_eq!(snapshot.artifacts(), &params["finalState"]["artifacts"]);
            assert_eq!(
                snapshot.captured_outputs(),
                &params["finalState"]["capturedOutputs"]
            );
            assert_eq!(
                snapshot.parent_session_id().map(SessionId::as_str),
                params["finalState"]["parentSessionId"].as_str()
            );
            assert_eq!(
                snapshot.workspace_path(),
                params["finalState"]["workspacePath"].as_str()
            );
            assert_eq!(snapshot.root().status().as_str(), expected.as_str());
        }
    }

    #[test]
    fn workflow_capture_terminal_projection_matches_oracle() {
        let mut failed = 0_u64;
        let mut aborted = 0_u64;
        for source in [FAILED_CAPTURE, ABORTED_CAPTURE] {
            for line in source.lines() {
                let frame: serde_json::Value =
                    must_succeed(serde_json::from_str(line), "capture line is valid JSON");
                let envelope = frame.get("parsed").unwrap_or(&frame);
                if envelope.get("method").and_then(serde_json::Value::as_str)
                    != Some("_kiro/workflow/run_complete")
                {
                    continue;
                }
                let completion = event(
                    to_notification("kiro/workflow/run_complete", &envelope["params"]),
                    "captured run_complete",
                );
                let WorkflowEvent::RunCompleted(completion) = completion else {
                    panic!("captured terminal frame converted to the wrong event");
                };
                match completion.status() {
                    WorkflowCompletionStatus::Failed => failed += 1,
                    WorkflowCompletionStatus::Aborted => aborted += 1,
                    status => panic!("unexpected captured terminal status {status:?}"),
                }
            }
        }
        let actual = serde_json::json!({"failed": failed, "aborted": aborted});
        let expected = match std::env::var_os("CYRIL_WORKFLOW_ORACLE_EXPECTED") {
            Some(path) => {
                let text = must_succeed(
                    std::fs::read_to_string(path),
                    "terminal oracle output is readable",
                );
                must_succeed(
                    serde_json::from_str::<serde_json::Value>(&text),
                    "terminal oracle output is valid JSON",
                )
            }
            None => {
                let manifest: serde_json::Value = must_succeed(
                    serde_json::from_str(include_str!(
                        "../../../../../../.cyril-6beh/oracle-manifest.json"
                    )),
                    "oracle manifest is valid JSON",
                );
                manifest["terminal_counts"].clone()
            }
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn workflow_capture_state_matches_oracle() {
        let mut projections = Vec::new();
        for (source, expected_status) in [(FAILED_CAPTURE, "failed"), (ABORTED_CAPTURE, "aborted")]
        {
            let params = capture_params(source, expected_status);
            let WorkflowEvent::RunCompleted(completion) = event(
                to_notification("kiro/workflow/run_complete", &params),
                "captured run_complete",
            ) else {
                panic!("captured terminal frame converted to the wrong event");
            };
            let workflow_id = completion.workflow_id().clone();
            let mut tracker = WorkflowTracker::new();
            must_succeed(
                tracker.apply_snapshot(completion.final_state().clone()),
                "captured final snapshot canonicalizes",
            );
            let Some(run) = tracker.get(&workflow_id) else {
                panic!("canonicalized capture is retrievable");
            };
            projections.push(workflow_projection(&workflow_id, run));
        }
        let actual = serde_json::Value::Array(projections);
        let expected_text = match std::env::var_os("CYRIL_WORKFLOW_ORACLE_EXPECTED") {
            Some(path) => must_succeed(
                std::fs::read_to_string(path),
                "snapshot oracle output is readable",
            ),
            None => include_str!("../../../../../../.cyril-6beh/oracle-snapshot-expected.json")
                .to_owned(),
        };
        let expected = must_succeed(
            serde_json::from_str::<serde_json::Value>(&expected_text),
            "snapshot oracle output is valid JSON",
        );
        assert_eq!(actual, expected);
    }
    #[test]
    fn malformed_run_frames_drop_without_poisoning_successor() {
        let valid_opening = serde_json::json!({
            "workflowId": "workflow",
            "workflowName": "recipe",
            "inputs": {},
            "nodeTree": [{"nodeId": "step", "type": "step", "agentName": "agent"}]
        });
        let missing = serde_json::json!({
            "workflowId": "workflow",
            "inputs": {},
            "nodeTree": []
        });
        assert!(matches!(
            to_notification("kiro/workflow/run_start", &missing),
            Some(None)
        ));

        let wrong_type = serde_json::json!({
            "workflowId": "workflow",
            "status": 1,
            "finalState": {}
        });
        assert!(matches!(
            to_notification("kiro/workflow/run_complete", &wrong_type),
            Some(None)
        ));

        let mismatch = serde_json::json!({
            "workflowId": "outer",
            "status": "completed",
            "finalState": completed_snapshot("inner", "completed")
        });
        assert!(matches!(
            to_notification("kiro/workflow/run_complete", &mismatch),
            Some(None)
        ));

        let status_mismatch = serde_json::json!({
            "workflowId": "workflow",
            "status": "failed",
            "finalState": completed_snapshot("workflow", "completed")
        });
        assert!(matches!(
            to_notification("kiro/workflow/run_complete", &status_mismatch),
            Some(None)
        ));

        assert!(to_notification("kiro/workflow/run_started", &valid_opening).is_none());
        assert!(to_notification("_kiro/workflow/run_start", &valid_opening).is_none());
        assert!(matches!(
            to_notification("kiro/workflow/run_start", &valid_opening),
            Some(Some(Notification::Workflow(_)))
        ));
    }

    #[test]
    fn large_run_completion_meets_wall_budget() {
        let mut chain = step_snapshot("chain-step");
        for level in (1..=8).rev() {
            chain = serde_json::json!({
                "nodeId": format!("chain-{level}"),
                "type": "sequence",
                "status": "completed",
                "children": [chain]
            });
        }
        let mut children = Vec::with_capacity(247);
        children.push(chain);
        children.extend((0..246).map(|index| step_snapshot(&format!("wide-{index}"))));
        let params = serde_json::json!({
            "workflowId": "workflow",
            "status": "completed",
            "finalState": {
                "workflowId": "workflow",
                "workflowName": "large",
                "status": "completed",
                "inputs": {"payload": "x".repeat(1_048_576)},
                "artifacts": {},
                "capturedOutputs": {},
                "root": {
                    "nodeId": "workflow",
                    "type": "sequence",
                    "status": "completed",
                    "children": children
                },
                "createdAt": "",
                "planRevision": 4294967295_u32
            }
        });

        let started = Instant::now();
        let WorkflowEvent::RunCompleted(completion) = event(
            to_notification("kiro/workflow/run_complete", &params),
            "large run_complete",
        ) else {
            panic!("expected run_complete");
        };
        let elapsed = started.elapsed();

        assert_eq!(snapshot_node_count(completion.final_state().root()), 256);
        assert_eq!(snapshot_depth(completion.final_state().root()), 10);
        assert_eq!(
            completion.final_state().inputs()["payload"]
                .as_str()
                .map(str::len),
            Some(1_048_576)
        );
        assert!(
            elapsed <= Duration::from_millis(50),
            "1 MiB/256-node/depth-10 conversion exceeded 50 ms: {elapsed:?}"
        );
    }

    #[test]
    fn node_start_preserves_type_and_optional_presence() {
        for (wire_type, expected) in [
            ("step", WorkflowNodeType::Step),
            ("sequence", WorkflowNodeType::Sequence),
            ("repeat", WorkflowNodeType::Repeat),
            ("parallel", WorkflowNodeType::Parallel),
            ("watch", WorkflowNodeType::Watch),
        ] {
            let params = serde_json::json!({
                "workflowId": "workflow",
                "nodeId": "node",
                "nodePath": ["workflow", "node"],
                "type": wire_type
            });
            let WorkflowEvent::NodeStarted(started) = event(
                to_notification("kiro/workflow/node_start", &params),
                "minimal node_start",
            ) else {
                panic!("expected node_start");
            };
            assert_eq!(started.node_type(), expected);
            assert_eq!(started.node_path().segments(), ["workflow", "node"]);
            assert!(started.details().agent_name().is_none());
            assert!(started.details().session_id().is_none());
            assert!(started.details().prompt().is_none());
            assert!(started.details().iteration().is_none());
            assert!(started.details().branch_id().is_none());
        }

        let first = serde_json::json!({
            "workflowId": "workflow",
            "nodeId": "node",
            "nodePath": ["workflow", "node"],
            "type": "step",
            "agentName": ""
        });
        let second = serde_json::json!({
            "workflowId": "workflow",
            "nodeId": "node",
            "nodePath": ["workflow", "node"],
            "type": "step",
            "agentName": "",
            "sessionId": "",
            "prompt": "識別子 with space",
            "iteration": 0,
            "branchId": ""
        });
        let WorkflowEvent::NodeStarted(first) = event(
            to_notification("kiro/workflow/node_start", &first),
            "first node_start",
        ) else {
            panic!("expected first node_start");
        };
        let WorkflowEvent::NodeStarted(second) = event(
            to_notification("kiro/workflow/node_start", &second),
            "second node_start",
        ) else {
            panic!("expected second node_start");
        };
        assert!(first.details().session_id().is_none());
        assert_eq!(second.details().agent_name(), Some(""));
        assert_eq!(
            second.details().session_id().map(SessionId::as_str),
            Some("")
        );
        assert_eq!(second.details().prompt(), Some("識別子 with space"));
        assert_eq!(second.details().iteration(), Some(0));
        assert_eq!(second.details().branch_id(), Some(""));
    }

    #[test]
    fn node_completion_and_pause_preserve_every_documented_field() {
        for status in [
            "pending",
            "running",
            "paused",
            "completed",
            "failed",
            "aborted",
            "skipped",
        ] {
            let params = serde_json::json!({
                "workflowId": "workflow",
                "nodeId": "node",
                "nodePath": ["workflow", "iter-2", "node"],
                "status": status,
                "artifacts": null,
                "capturedOutput": {"nested": [1, 1]},
                "failureReason": "",
                "completionSignal": "need_input",
                "completionSignalSource": "send_message"
            });
            let WorkflowEvent::NodeCompleted(completed) = event(
                to_notification("kiro/workflow/node_complete", &params),
                "node_complete",
            ) else {
                panic!("expected node_complete");
            };
            assert_eq!(completed.status().as_str(), status);
            assert_eq!(
                completed.node_path().segments(),
                ["workflow", "iter-2", "node"]
            );
            assert_eq!(
                completed.details().artifacts(),
                Some(&serde_json::Value::Null)
            );
            assert_eq!(
                completed.details().captured_output(),
                Some(&serde_json::json!({"nested": [1, 1]}))
            );
            assert_eq!(completed.details().failure_reason(), Some(""));
            assert_eq!(
                completed.details().completion_signal(),
                Some(WorkflowCompletionSignal::NeedInput)
            );
            assert_eq!(
                completed.details().completion_signal_source(),
                Some(WorkflowCompletionSignalSource::SendMessage)
            );
        }

        let paused = serde_json::json!({
            "workflowId": "workflow",
            "nodeId": "node",
            "nodePath": ["workflow", "node"],
            "reason": "等待 human"
        });
        let WorkflowEvent::NodePaused(paused) = event(
            to_notification("kiro/workflow/node_paused", &paused),
            "node_paused",
        ) else {
            panic!("expected node_paused");
        };
        assert_eq!(paused.reason(), "等待 human");
    }

    #[test]
    fn node_path_validation_and_scale_budgets_hold() {
        let minimal = serde_json::json!({
            "workflowId": "workflow",
            "nodeId": "node",
            "nodePath": ["workflow", "node"],
            "type": "step"
        });
        let started = Instant::now();
        for _ in 0..10_000 {
            assert!(matches!(
                to_notification("kiro/workflow/node_start", &minimal),
                Some(Some(Notification::Workflow(_)))
            ));
        }
        let batch_elapsed = started.elapsed();
        assert!(
            batch_elapsed <= Duration::from_millis(100),
            "10,000 minimal node frames exceeded 100 ms: {batch_elapsed:?}"
        );

        let large_workflow = format!("識別子 {}", "w".repeat(65_536));
        let large_node = format!("node {}", "n".repeat(65_536));
        let large_segment = format!("path {}", "p".repeat(65_536));
        let large = serde_json::json!({
            "workflowId": large_workflow,
            "nodeId": large_node,
            "nodePath": [large_workflow, large_segment],
            "type": "watch",
            "prompt": "x".repeat(65_536)
        });
        let started = Instant::now();
        let WorkflowEvent::NodeStarted(large_event) = event(
            to_notification("kiro/workflow/node_start", &large),
            "large node_start",
        ) else {
            panic!("expected large node_start");
        };
        let large_elapsed = started.elapsed();
        assert_eq!(large_event.workflow_id().as_str(), large_workflow);
        assert_eq!(large_event.node_id().as_str(), large_node);
        assert_eq!(large_event.node_path().segments()[1], large_segment);
        assert!(
            large_elapsed <= Duration::from_millis(50),
            "64 KiB node frame exceeded 50 ms: {large_elapsed:?}"
        );

        let empty_path = serde_json::json!({
            "workflowId": "workflow",
            "nodeId": "node",
            "nodePath": [],
            "type": "step"
        });
        let wrong_root = serde_json::json!({
            "workflowId": "workflow",
            "nodeId": "node",
            "nodePath": ["other", "node"],
            "type": "step"
        });
        assert!(matches!(
            to_notification("kiro/workflow/node_start", &empty_path),
            Some(None)
        ));
        assert!(matches!(
            to_notification("kiro/workflow/node_start", &wrong_root),
            Some(None)
        ));
        assert!(matches!(
            to_notification("kiro/workflow/node_start", &minimal),
            Some(Some(Notification::Workflow(_)))
        ));
    }

    #[test]
    fn malformed_node_frames_drop_without_poisoning_successors() {
        let malformed = [
            (
                "kiro/workflow/node_start",
                serde_json::json!({
                    "workflowId": "workflow",
                    "nodeId": "",
                    "nodePath": ["workflow", "node"],
                    "type": "step"
                }),
            ),
            (
                "kiro/workflow/node_start",
                serde_json::json!({
                    "workflowId": "workflow",
                    "nodeId": "node",
                    "nodePath": ["workflow", "node"],
                    "type": "unknown"
                }),
            ),
            (
                "kiro/workflow/node_start",
                serde_json::json!({
                    "workflowId": "workflow",
                    "nodeId": "node",
                    "nodePath": ["workflow", "node"],
                    "type": "step",
                    "agentName": null
                }),
            ),
            (
                "kiro/workflow/node_complete",
                serde_json::json!({
                    "workflowId": "workflow",
                    "nodeId": "node",
                    "nodePath": ["workflow", "node"],
                    "status": "unknown"
                }),
            ),
            (
                "kiro/workflow/node_paused",
                serde_json::json!({
                    "workflowId": "workflow",
                    "nodeId": "node",
                    "nodePath": ["workflow", "node"]
                }),
            ),
        ];
        for (method, params) in malformed {
            assert!(
                matches!(to_notification(method, &params), Some(None)),
                "{method} malformed input must warn and drop"
            );
        }

        let duplicate: serde_json::Value = match serde_json::from_str(
            r#"{
                "workflowId":"workflow",
                "nodeId":"node",
                "nodePath":["wrong","node"],
                "nodePath":["workflow","node"],
                "type":"step",
                "agentName":"first",
                "agentName":"second",
                "futureField":{"ignored":true}
            }"#,
        ) {
            Ok(value) => value,
            Err(error) => panic!("duplicate-key fixture is invalid: {error}"),
        };
        let WorkflowEvent::NodeStarted(started) = event(
            to_notification("kiro/workflow/node_start", &duplicate),
            "duplicate node_start",
        ) else {
            panic!("expected duplicate node_start");
        };
        assert_eq!(started.node_path().segments(), ["workflow", "node"]);
        assert_eq!(started.details().agent_name(), Some("second"));

        let completion = serde_json::json!({
            "workflowId": "workflow",
            "nodeId": "node",
            "nodePath": ["workflow", "node"],
            "status": "completed"
        });
        let paused = serde_json::json!({
            "workflowId": "workflow",
            "nodeId": "node",
            "nodePath": ["workflow", "node"],
            "reason": ""
        });
        assert!(matches!(
            to_notification("kiro/workflow/node_complete", &completion),
            Some(Some(Notification::Workflow(_)))
        ));
        assert!(matches!(
            to_notification("kiro/workflow/node_paused", &paused),
            Some(Some(Notification::Workflow(_)))
        ));
    }

    #[test]
    fn progress_pause_and_queue_fields_match_manifest_domains() {
        for (iteration, stop_condition_met) in [(0, false), (u32::MAX, true)] {
            let params = serde_json::json!({
                "workflowId": "workflow",
                "loopId": "loop",
                "iteration": iteration,
                "stopConditionMet": stop_condition_met
            });
            let WorkflowEvent::LoopIteration(progress) = event(
                to_notification("kiro/workflow/loop_iteration", &params),
                "loop_iteration",
            ) else {
                panic!("expected loop_iteration");
            };
            assert_eq!(progress.loop_id().as_str(), "loop");
            assert_eq!(progress.iteration(), iteration);
            assert_eq!(progress.stop_condition_met(), stop_condition_met);
        }

        for outcome in ["new-activity", "idle", "idle-timeout", "terminal-state"] {
            let params = serde_json::json!({
                "workflowId": "workflow",
                "nodeId": "watch",
                "nodePath": ["workflow", "watch"],
                "outcome": outcome,
                "at": "t 2"
            });
            let WorkflowEvent::WatchPoll(poll) = event(
                to_notification("kiro/workflow/watch_poll", &params),
                "watch_poll",
            ) else {
                panic!("expected watch_poll");
            };
            assert_eq!(poll.outcome().as_str(), outcome);
            assert_eq!(poll.at(), "t 2");
            assert_eq!(poll.node_path().segments(), ["workflow", "watch"]);
        }

        for pause_reason in ["", "等待 human"] {
            let params = serde_json::json!({
                "workflowId": "workflow",
                "pauseReason": pause_reason
            });
            let WorkflowEvent::Paused(paused) =
                event(to_notification("kiro/workflow/paused", &params), "paused")
            else {
                panic!("expected paused");
            };
            assert_eq!(paused.pause_reason(), pause_reason);
        }

        let empty = serde_json::json!({
            "workflowId": "workflow",
            "pendingSteps": []
        });
        let WorkflowEvent::StepsQueued(empty) = event(
            to_notification("kiro/workflow/steps_queued", &empty),
            "empty steps_queued",
        ) else {
            panic!("expected steps_queued");
        };
        assert!(empty.pending_steps().is_empty());
        assert!(empty.resolution().is_none());

        let descriptors = serde_json::json!([
            {
                "nodeId": "repeat",
                "type": "repeat",
                "steps": [{
                    "nodeId": "step",
                    "type": "step",
                    "agentName": "agent"
                }],
                "maxIterations": 1,
                "onMaxIterations": "pause",
                "stopCondition": null,
                "stopWhen": {"done": false}
            },
            {
                "nodeId": "watch",
                "type": "watch",
                "handlerName": "files"
            }
        ]);
        for outcome in ["applied", "rejected", "dropped"] {
            for reason in [None, Some("")] {
                let resolution = match reason {
                    Some(reason) => serde_json::json!({"outcome": outcome, "reason": reason}),
                    None => serde_json::json!({"outcome": outcome}),
                };
                let params = serde_json::json!({
                    "workflowId": "workflow",
                    "pendingSteps": descriptors,
                    "resolution": resolution
                });
                let WorkflowEvent::StepsQueued(queued) = event(
                    to_notification("kiro/workflow/steps_queued", &params),
                    "resolved steps_queued",
                ) else {
                    panic!("expected resolved steps_queued");
                };
                assert_eq!(queued.pending_steps().len(), 2);
                let resolution = match queued.resolution() {
                    Some(value) => value,
                    None => panic!("expected queue resolution"),
                };
                assert_eq!(resolution.outcome().as_str(), outcome);
                assert_eq!(resolution.reason(), reason);
                assert_eq!(
                    queued.pending_steps()[0].stop_condition(),
                    Some(&serde_json::Value::Null)
                );
                assert_eq!(
                    queued.pending_steps()[0].stop_when(),
                    Some(&serde_json::json!({"done": false}))
                );
            }
        }
    }

    #[test]
    fn progress_and_queue_scale_budgets_hold() {
        let minimal = serde_json::json!({
            "workflowId": "workflow",
            "loopId": "loop",
            "iteration": 0,
            "stopConditionMet": false
        });
        let started = Instant::now();
        for _ in 0..100_000 {
            assert!(matches!(
                to_notification("kiro/workflow/loop_iteration", &minimal),
                Some(Some(Notification::Workflow(_)))
            ));
        }
        let fixed_elapsed = started.elapsed();
        assert!(
            fixed_elapsed <= Duration::from_millis(100),
            "100,000 minimal progress frames exceeded 100 ms: {fixed_elapsed:?}"
        );

        let mut chain = step_descriptor("chain-step");
        chain["agentName"] = serde_json::Value::String("x".repeat(1_048_576));
        for level in (1..=9).rev() {
            chain = serde_json::json!({
                "nodeId": format!("chain-{level}"),
                "type": "sequence",
                "steps": [chain]
            });
        }
        let mut pending_steps = Vec::with_capacity(247);
        pending_steps.push(chain);
        pending_steps.extend((0..246).map(|index| step_descriptor(&format!("wide-{index}"))));
        let params = serde_json::json!({
            "workflowId": "workflow",
            "pendingSteps": pending_steps,
            "resolution": {"outcome": "applied", "reason": "ok"}
        });
        let started = Instant::now();
        let WorkflowEvent::StepsQueued(queued) = event(
            to_notification("kiro/workflow/steps_queued", &params),
            "large steps_queued",
        ) else {
            panic!("expected large steps_queued");
        };
        let queue_elapsed = started.elapsed();
        assert_eq!(
            queued
                .pending_steps()
                .iter()
                .map(descriptor_count)
                .sum::<usize>(),
            256
        );
        assert_eq!(
            queued.pending_steps().iter().map(descriptor_depth).max(),
            Some(10)
        );
        assert!(
            queue_elapsed <= Duration::from_millis(50),
            "1 MiB/256-step/depth-10 queue conversion exceeded 50 ms: {queue_elapsed:?}"
        );
    }

    #[test]
    fn malformed_progress_frames_drop_without_poisoning_successors() {
        let malformed = [
            (
                "kiro/workflow/loop_iteration",
                serde_json::json!({
                    "workflowId": "workflow",
                    "loopId": "",
                    "iteration": 0,
                    "stopConditionMet": false
                }),
            ),
            (
                "kiro/workflow/loop_iteration",
                serde_json::json!({
                    "workflowId": "workflow",
                    "loopId": "loop",
                    "iteration": -1,
                    "stopConditionMet": false
                }),
            ),
            (
                "kiro/workflow/watch_poll",
                serde_json::json!({
                    "workflowId": "workflow",
                    "nodeId": "watch",
                    "nodePath": ["workflow", "watch"],
                    "outcome": "unknown",
                    "at": "t"
                }),
            ),
            (
                "kiro/workflow/paused",
                serde_json::json!({
                    "workflowId": "workflow"
                }),
            ),
            (
                "kiro/workflow/steps_queued",
                serde_json::json!({
                    "workflowId": "workflow",
                    "pendingSteps": [],
                    "resolution": null
                }),
            ),
            (
                "kiro/workflow/steps_queued",
                serde_json::json!({
                    "workflowId": "workflow",
                    "pendingSteps": [{"nodeId": "node", "type": "unknown"}]
                }),
            ),
        ];
        for (method, params) in malformed {
            assert!(
                matches!(to_notification(method, &params), Some(None)),
                "{method} malformed input must warn and drop"
            );
        }

        let valid = serde_json::json!({
            "workflowId": "workflow",
            "pendingSteps": [],
            "resolution": {"outcome": "dropped", "reason": ""}
        });
        assert!(matches!(
            to_notification("kiro/workflow/steps_queued", &valid),
            Some(Some(Notification::Workflow(_)))
        ));
    }

    struct MalformedCase {
        id: String,
        method: String,
        params: serde_json::Value,
        field_path: String,
        error_kind: &'static str,
    }

    #[test]
    fn malformed_workflow_field_matrix_isolated() {
        let started = Instant::now();
        let manifest: serde_json::Value = must_succeed(
            serde_json::from_str(include_str!(
                "../../../../../../.cyril-6beh/oracle-manifest.json"
            )),
            "workflow oracle manifest is valid JSON",
        );
        let field_contracts = must_object(&manifest["fields"], "manifest fields");
        let descriptor_contracts = must_object(&manifest["descriptor_fields"], "descriptor fields");
        let mut cases = Vec::new();

        for (event_kind, contract) in field_contracts {
            let method = format!("kiro/workflow/{event_kind}");
            let required = must_array(&contract["required"], "required event fields");
            for field in required {
                let field = must_string(field, "required event field");
                let mut params = valid_payload(&method);
                let removed = must_object_mut(&mut params, "event payload").remove(field);
                assert!(
                    removed.is_some(),
                    "manifest required field must exist in baseline: {event_kind}.{field}"
                );
                cases.push(MalformedCase {
                    id: format!("{event_kind}.missing.{field}"),
                    method: method.clone(),
                    params,
                    field_path: field.to_owned(),
                    error_kind: "missing_required",
                });
            }
        }

        for (node_type, contract) in descriptor_contracts {
            let required = must_array(&contract["required"], "required descriptor fields");
            for field in required {
                let field = must_string(field, "required descriptor field");
                let mut params = descriptor_event_payload(valid_descriptor(node_type));
                let descriptor = first_descriptor_mut(&mut params);
                let removed = descriptor.remove(field);
                assert!(
                    removed.is_some(),
                    "manifest descriptor field must exist in baseline: {node_type}.{field}"
                );
                cases.push(MalformedCase {
                    id: format!("descriptor.{node_type}.missing.{field}"),
                    method: "kiro/workflow/run_start".to_owned(),
                    params,
                    field_path: format!("nodeTree[0].{field}"),
                    error_kind: "missing_required",
                });
            }
        }

        for (method, field, optional) in [
            ("kiro/workflow/run_start", "workflowId", false),
            ("kiro/workflow/run_start", "workflowName", false),
            ("kiro/workflow/run_start", "parentSessionId", true),
            ("kiro/workflow/node_start", "workflowId", false),
            ("kiro/workflow/node_start", "nodeId", false),
            ("kiro/workflow/node_start", "agentName", true),
            ("kiro/workflow/node_start", "sessionId", true),
            ("kiro/workflow/node_start", "prompt", true),
            ("kiro/workflow/node_start", "branchId", true),
            ("kiro/workflow/node_complete", "workflowId", false),
            ("kiro/workflow/node_complete", "nodeId", false),
            ("kiro/workflow/node_complete", "failureReason", true),
            ("kiro/workflow/node_paused", "workflowId", false),
            ("kiro/workflow/node_paused", "nodeId", false),
            ("kiro/workflow/node_paused", "reason", false),
            ("kiro/workflow/loop_iteration", "workflowId", false),
            ("kiro/workflow/loop_iteration", "loopId", false),
            ("kiro/workflow/watch_poll", "workflowId", false),
            ("kiro/workflow/watch_poll", "nodeId", false),
            ("kiro/workflow/watch_poll", "at", false),
            ("kiro/workflow/paused", "workflowId", false),
            ("kiro/workflow/paused", "pauseReason", false),
            ("kiro/workflow/run_complete", "workflowId", false),
            ("kiro/workflow/steps_queued", "workflowId", false),
        ] {
            push_top_case(
                &mut cases,
                method,
                field,
                serde_json::Value::Bool(false),
                "wrong_type",
            );
            push_top_case(
                &mut cases,
                method,
                field,
                serde_json::Value::Null,
                if optional {
                    "invalid_value"
                } else {
                    "wrong_type"
                },
            );
        }

        for (method, field) in [
            ("kiro/workflow/run_start", "nodeTree"),
            ("kiro/workflow/node_start", "nodePath"),
            ("kiro/workflow/node_complete", "nodePath"),
            ("kiro/workflow/node_paused", "nodePath"),
            ("kiro/workflow/watch_poll", "nodePath"),
            ("kiro/workflow/run_complete", "finalState"),
            ("kiro/workflow/steps_queued", "pendingSteps"),
        ] {
            push_top_case(
                &mut cases,
                method,
                field,
                serde_json::Value::Bool(false),
                "wrong_type",
            );
            push_top_case(
                &mut cases,
                method,
                field,
                serde_json::Value::Null,
                "wrong_type",
            );
        }

        for (method, field, optional) in [
            ("kiro/workflow/node_start", "iteration", true),
            ("kiro/workflow/loop_iteration", "iteration", false),
        ] {
            push_top_case(
                &mut cases,
                method,
                field,
                serde_json::Value::String("zero".to_owned()),
                "wrong_type",
            );
            push_top_case(
                &mut cases,
                method,
                field,
                serde_json::Value::Null,
                if optional {
                    "invalid_value"
                } else {
                    "wrong_type"
                },
            );
            push_top_case(
                &mut cases,
                method,
                field,
                serde_json::json!(-1),
                "invalid_value",
            );
        }
        for value in [serde_json::json!("false"), serde_json::Value::Null] {
            push_top_case(
                &mut cases,
                "kiro/workflow/loop_iteration",
                "stopConditionMet",
                value,
                "wrong_type",
            );
        }

        for (method, field, optional) in [
            ("kiro/workflow/node_start", "type", false),
            ("kiro/workflow/node_complete", "status", false),
            ("kiro/workflow/watch_poll", "outcome", false),
            ("kiro/workflow/run_complete", "status", false),
            ("kiro/workflow/node_complete", "completionSignal", true),
            (
                "kiro/workflow/node_complete",
                "completionSignalSource",
                true,
            ),
        ] {
            push_top_case(
                &mut cases,
                method,
                field,
                serde_json::Value::String("unknown".to_owned()),
                "invalid_enum",
            );
            push_top_case(
                &mut cases,
                method,
                field,
                serde_json::Value::Bool(false),
                "wrong_type",
            );
            push_top_case(
                &mut cases,
                method,
                field,
                serde_json::Value::Null,
                if optional {
                    "invalid_value"
                } else {
                    "wrong_type"
                },
            );
        }

        push_top_case(
            &mut cases,
            "kiro/workflow/steps_queued",
            "resolution",
            serde_json::Value::Bool(false),
            "wrong_type",
        );
        push_top_case(
            &mut cases,
            "kiro/workflow/steps_queued",
            "resolution",
            serde_json::Value::Null,
            "invalid_value",
        );
        for (path, value, error_kind) in [
            (
                "resolution.outcome",
                serde_json::Value::String("unknown".to_owned()),
                "invalid_enum",
            ),
            (
                "resolution.outcome",
                serde_json::Value::Bool(false),
                "wrong_type",
            ),
            ("resolution.outcome", serde_json::Value::Null, "wrong_type"),
            (
                "resolution.reason",
                serde_json::Value::Bool(false),
                "wrong_type",
            ),
            (
                "resolution.reason",
                serde_json::Value::Null,
                "invalid_value",
            ),
        ] {
            let value_kind = json_value_kind(&value);
            let mut params = valid_payload("kiro/workflow/steps_queued");
            set_nested_field(&mut params, path, value);
            cases.push(MalformedCase {
                id: format!("steps_queued.{path}.{error_kind}.{value_kind}"),
                method: "kiro/workflow/steps_queued".to_owned(),
                params,
                field_path: path.to_owned(),
                error_kind,
            });
        }

        for node_type in ["step", "sequence", "repeat", "parallel", "watch"] {
            for (field, value, error_kind) in descriptor_bad_fields(node_type) {
                let value_kind = json_value_kind(&value);
                let mut params = descriptor_event_payload(valid_descriptor(node_type));
                first_descriptor_mut(&mut params).insert(field.to_owned(), value);
                cases.push(MalformedCase {
                    id: format!("descriptor.{node_type}.{field}.{error_kind}.{value_kind}"),
                    method: "kiro/workflow/run_start".to_owned(),
                    params,
                    field_path: if field == "type" {
                        "nodeTree[0].type".to_owned()
                    } else {
                        "nodeTree[0]".to_owned()
                    },
                    error_kind,
                });
            }
        }

        for (method, field) in [
            ("kiro/workflow/run_start", "workflowId"),
            ("kiro/workflow/node_start", "workflowId"),
            ("kiro/workflow/node_start", "nodeId"),
            ("kiro/workflow/node_complete", "workflowId"),
            ("kiro/workflow/node_complete", "nodeId"),
            ("kiro/workflow/node_paused", "workflowId"),
            ("kiro/workflow/node_paused", "nodeId"),
            ("kiro/workflow/loop_iteration", "workflowId"),
            ("kiro/workflow/loop_iteration", "loopId"),
            ("kiro/workflow/watch_poll", "workflowId"),
            ("kiro/workflow/watch_poll", "nodeId"),
            ("kiro/workflow/paused", "workflowId"),
            ("kiro/workflow/run_complete", "workflowId"),
            ("kiro/workflow/steps_queued", "workflowId"),
        ] {
            push_top_case(
                &mut cases,
                method,
                field,
                serde_json::Value::String(String::new()),
                "invalid_value",
            );
        }

        let mut wrong_path = valid_payload("kiro/workflow/node_start");
        set_top_field(
            &mut wrong_path,
            "nodePath",
            serde_json::json!(["other", "node"]),
        );
        cases.push(MalformedCase {
            id: "node_start.nodePath.invalid_root".to_owned(),
            method: "kiro/workflow/node_start".to_owned(),
            params: wrong_path,
            field_path: "nodePath".to_owned(),
            error_kind: "invalid_value",
        });
        let mut mismatch = valid_payload("kiro/workflow/run_complete");
        set_top_field(
            &mut mismatch,
            "status",
            serde_json::Value::String("failed".to_owned()),
        );
        cases.push(MalformedCase {
            id: "run_complete.status.status_mismatch".to_owned(),
            method: "kiro/workflow/run_complete".to_owned(),
            params: mismatch,
            field_path: "status".to_owned(),
            error_kind: "status_mismatch",
        });

        assert_eq!(cases.len(), 205, "malformed matrix row-set drift");
        let mut case_ids = cases.iter().map(|case| case.id.clone()).collect::<Vec<_>>();
        case_ids.sort_unstable();
        case_ids.dedup();
        assert_eq!(case_ids.len(), 205, "malformed case ids must be unique");
        for case in cases {
            let (result, log) = capture_rejection(&case.method, &case.params);
            assert!(
                matches!(result, Some(None)),
                "{}: malformed row must drop",
                case.id
            );
            assert_eq!(log["level"], "WARN", "{}: warning level", case.id);
            let fields = must_object(&log["fields"], "captured warning fields");
            let mut field_names = fields.keys().map(String::as_str).collect::<Vec<_>>();
            field_names.sort_unstable();
            assert_eq!(
                field_names,
                ["error", "error_kind", "field_path", "message", "method"],
                "{}: warning field set",
                case.id
            );
            assert_eq!(
                fields["message"], "malformed workflow notification",
                "{}: warning message",
                case.id
            );
            assert_eq!(fields["method"], case.method, "{}: method", case.id);
            assert_eq!(
                fields["field_path"], case.field_path,
                "{}: field path",
                case.id
            );
            assert_eq!(
                fields["error_kind"], case.error_kind,
                "{}: error kind",
                case.id
            );
            assert!(
                fields["error"]
                    .as_str()
                    .is_some_and(|error| !error.is_empty()),
                "{}: error text",
                case.id
            );
            assert!(
                matches!(
                    to_notification(&case.method, &valid_payload(&case.method)),
                    Some(Some(Notification::Workflow(_)))
                ),
                "{}: valid successor must convert",
                case.id
            );
        }
        assert!(
            started.elapsed() <= Duration::from_secs(5),
            "malformed matrix exceeded 5 seconds"
        );
    }

    fn valid_payload(method: &str) -> serde_json::Value {
        match method {
            "kiro/workflow/run_start" => serde_json::json!({
                "workflowId": "workflow",
                "workflowName": "recipe",
                "inputs": null,
                "nodeTree": [valid_descriptor("step")],
                "parentSessionId": ""
            }),
            "kiro/workflow/node_start" => serde_json::json!({
                "workflowId": "workflow",
                "nodeId": "node",
                "nodePath": ["workflow", "node"],
                "type": "step",
                "agentName": "",
                "sessionId": "",
                "prompt": "",
                "iteration": 0,
                "branchId": ""
            }),
            "kiro/workflow/node_complete" => serde_json::json!({
                "workflowId": "workflow",
                "nodeId": "node",
                "nodePath": ["workflow", "node"],
                "status": "completed",
                "artifacts": null,
                "capturedOutput": null,
                "failureReason": "",
                "completionSignal": "success",
                "completionSignalSource": "send_message"
            }),
            "kiro/workflow/node_paused" => serde_json::json!({
                "workflowId": "workflow",
                "nodeId": "node",
                "nodePath": ["workflow", "node"],
                "reason": ""
            }),
            "kiro/workflow/loop_iteration" => serde_json::json!({
                "workflowId": "workflow",
                "loopId": "loop",
                "iteration": 0,
                "stopConditionMet": false
            }),
            "kiro/workflow/watch_poll" => serde_json::json!({
                "workflowId": "workflow",
                "nodeId": "watch",
                "nodePath": ["workflow", "watch"],
                "outcome": "idle",
                "at": ""
            }),
            "kiro/workflow/paused" => serde_json::json!({
                "workflowId": "workflow",
                "pauseReason": ""
            }),
            "kiro/workflow/run_complete" => serde_json::json!({
                "workflowId": "workflow",
                "status": "completed",
                "finalState": completed_snapshot("workflow", "completed")
            }),
            "kiro/workflow/steps_queued" => serde_json::json!({
                "workflowId": "workflow",
                "pendingSteps": [],
                "resolution": {"outcome": "applied", "reason": ""}
            }),
            other => panic!("no valid workflow payload for {other}"),
        }
    }

    fn valid_descriptor(node_type: &str) -> serde_json::Value {
        match node_type {
            "step" => serde_json::json!({
                "nodeId": "node",
                "type": "step",
                "agentName": "",
                "model": "",
                "effort": ""
            }),
            "sequence" => serde_json::json!({
                "nodeId": "node",
                "type": "sequence",
                "steps": []
            }),
            "repeat" => serde_json::json!({
                "nodeId": "node",
                "type": "repeat",
                "steps": [],
                "maxIterations": 0,
                "onMaxIterations": "pause",
                "stopCondition": null,
                "stopWhen": null
            }),
            "parallel" => serde_json::json!({
                "nodeId": "node",
                "type": "parallel",
                "branches": []
            }),
            "watch" => serde_json::json!({
                "nodeId": "node",
                "type": "watch",
                "handlerName": ""
            }),
            other => panic!("unknown descriptor fixture type {other}"),
        }
    }

    fn descriptor_event_payload(descriptor: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "workflowId": "workflow",
            "workflowName": "recipe",
            "inputs": {},
            "nodeTree": [descriptor]
        })
    }

    fn descriptor_bad_fields(
        node_type: &str,
    ) -> Vec<(&'static str, serde_json::Value, &'static str)> {
        let mut fields = vec![
            ("nodeId", serde_json::Value::Bool(false), "wrong_type"),
            ("nodeId", serde_json::Value::Null, "wrong_type"),
            (
                "type",
                serde_json::Value::String("unknown".to_owned()),
                "invalid_enum",
            ),
            ("type", serde_json::Value::Bool(false), "wrong_type"),
            ("type", serde_json::Value::Null, "wrong_type"),
        ];
        match node_type {
            "step" => fields.extend([
                ("agentName", serde_json::Value::Bool(false), "wrong_type"),
                ("agentName", serde_json::Value::Null, "wrong_type"),
                ("model", serde_json::Value::Bool(false), "wrong_type"),
                ("model", serde_json::Value::Null, "invalid_value"),
                ("effort", serde_json::Value::Bool(false), "wrong_type"),
                ("effort", serde_json::Value::Null, "invalid_value"),
            ]),
            "sequence" => fields.extend([
                ("steps", serde_json::Value::Bool(false), "wrong_type"),
                ("steps", serde_json::Value::Null, "wrong_type"),
            ]),
            "repeat" => fields.extend([
                ("steps", serde_json::Value::Bool(false), "wrong_type"),
                ("steps", serde_json::Value::Null, "wrong_type"),
                (
                    "maxIterations",
                    serde_json::Value::String("zero".to_owned()),
                    "wrong_type",
                ),
                ("maxIterations", serde_json::Value::Null, "wrong_type"),
                ("maxIterations", serde_json::json!(-1), "invalid_value"),
                (
                    "onMaxIterations",
                    serde_json::Value::String("unknown".to_owned()),
                    "invalid_enum",
                ),
                (
                    "onMaxIterations",
                    serde_json::Value::Bool(false),
                    "wrong_type",
                ),
                ("onMaxIterations", serde_json::Value::Null, "wrong_type"),
            ]),
            "parallel" => fields.extend([
                ("branches", serde_json::Value::Bool(false), "wrong_type"),
                ("branches", serde_json::Value::Null, "wrong_type"),
            ]),
            "watch" => fields.extend([
                ("handlerName", serde_json::Value::Bool(false), "wrong_type"),
                ("handlerName", serde_json::Value::Null, "wrong_type"),
            ]),
            other => panic!("unknown descriptor fixture type {other}"),
        }
        fields
    }

    fn push_top_case(
        cases: &mut Vec<MalformedCase>,
        method: &str,
        field: &str,
        value: serde_json::Value,
        error_kind: &'static str,
    ) {
        let value_kind = json_value_kind(&value);
        let mut params = valid_payload(method);
        set_top_field(&mut params, field, value);
        let event_kind = method.rsplit('/').next().unwrap_or(method);
        cases.push(MalformedCase {
            id: format!("{event_kind}.{field}.{error_kind}.{value_kind}"),
            method: method.to_owned(),
            params,
            field_path: field.to_owned(),
            error_kind,
        });
    }

    fn json_value_kind(value: &serde_json::Value) -> &'static str {
        match value {
            serde_json::Value::Null => "null",
            serde_json::Value::Bool(_) => "boolean",
            serde_json::Value::Number(_) => "number",
            serde_json::Value::String(_) => "string",
            serde_json::Value::Array(_) => "array",
            serde_json::Value::Object(_) => "object",
        }
    }

    fn set_top_field(params: &mut serde_json::Value, field: &str, value: serde_json::Value) {
        must_object_mut(params, "event payload").insert(field.to_owned(), value);
    }

    fn set_nested_field(params: &mut serde_json::Value, path: &str, value: serde_json::Value) {
        let (parent, field) = match path.split_once('.') {
            Some(parts) => parts,
            None => panic!("nested field path lacks separator: {path}"),
        };
        let parent_value = match must_object_mut(params, "event payload").get_mut(parent) {
            Some(value) => value,
            None => panic!("nested parent does not exist: {parent}"),
        };
        must_object_mut(parent_value, parent).insert(field.to_owned(), value);
    }

    fn first_descriptor_mut(
        params: &mut serde_json::Value,
    ) -> &mut serde_json::Map<String, serde_json::Value> {
        let node_tree = match must_object_mut(params, "event payload").get_mut("nodeTree") {
            Some(value) => value,
            None => panic!("descriptor event lacks nodeTree"),
        };
        let descriptors = match node_tree.as_array_mut() {
            Some(value) => value,
            None => panic!("descriptor nodeTree is not an array"),
        };
        let first = match descriptors.first_mut() {
            Some(value) => value,
            None => panic!("descriptor nodeTree is empty"),
        };
        must_object_mut(first, "descriptor")
    }

    fn must_object<'a>(
        value: &'a serde_json::Value,
        context: &str,
    ) -> &'a serde_json::Map<String, serde_json::Value> {
        match value.as_object() {
            Some(object) => object,
            None => panic!("{context}: expected object"),
        }
    }

    fn must_object_mut<'a>(
        value: &'a mut serde_json::Value,
        context: &str,
    ) -> &'a mut serde_json::Map<String, serde_json::Value> {
        match value.as_object_mut() {
            Some(object) => object,
            None => panic!("{context}: expected mutable object"),
        }
    }

    fn must_array<'a>(value: &'a serde_json::Value, context: &str) -> &'a [serde_json::Value] {
        match value.as_array() {
            Some(array) => array,
            None => panic!("{context}: expected array"),
        }
    }

    fn must_string<'a>(value: &'a serde_json::Value, context: &str) -> &'a str {
        match value.as_str() {
            Some(string) => string,
            None => panic!("{context}: expected string"),
        }
    }

    fn workflow_manifest() -> serde_json::Value {
        must_succeed(
            serde_json::from_str(include_str!(
                "../../../../../../.cyril-6beh/oracle-manifest.json"
            )),
            "workflow oracle manifest",
        )
    }

    fn set_nested_snapshot_node_status(snapshot: &mut serde_json::Value, status: &str) {
        let root = match must_object_mut(snapshot, "snapshot").get_mut("root") {
            Some(value) => value,
            None => panic!("snapshot root is absent"),
        };
        must_object_mut(root, "snapshot root").insert(
            "status".to_owned(),
            serde_json::Value::String(status.to_owned()),
        );
    }

    fn opaque_json_values() -> Vec<(&'static str, serde_json::Value)> {
        let large_key = "k".repeat(65_536);
        let mut large_key_object = serde_json::Map::new();
        large_key_object.insert(large_key, serde_json::Value::String("value".to_owned()));
        let duplicate_resolved = must_succeed(
            serde_json::from_str(r#"{"same": 1, "same": 2}"#),
            "duplicate opaque object",
        );
        vec![
            ("null", serde_json::Value::Null),
            ("false", serde_json::Value::Bool(false)),
            ("true", serde_json::Value::Bool(true)),
            ("i64_min", serde_json::json!(i64::MIN)),
            ("u64_max", serde_json::json!(u64::MAX)),
            ("finite_fraction", serde_json::json!(1.25)),
            ("empty_string", serde_json::Value::String(String::new())),
            (
                "ascii_string",
                serde_json::Value::String("ascii".to_owned()),
            ),
            (
                "unicode_string",
                serde_json::Value::String("文字列".to_owned()),
            ),
            (
                "spaced_string",
                serde_json::Value::String(" with space ".to_owned()),
            ),
            (
                "large_string",
                serde_json::Value::String("x".repeat(1_048_576)),
            ),
            ("empty_array", serde_json::json!([])),
            ("single_array", serde_json::json!([1])),
            ("multi_array", serde_json::json!([1, 2, 3])),
            ("duplicate_array", serde_json::json!([1, 1])),
            ("empty_object", serde_json::json!({})),
            ("single_object", serde_json::json!({"one": 1})),
            ("multi_object", serde_json::json!({"one": 1, "two": 2})),
            ("empty_key", serde_json::json!({"": "value"})),
            ("unicode_key", serde_json::json!({"鍵": "value"})),
            ("spaced_key", serde_json::json!({" key ": "value"})),
            ("large_key", serde_json::Value::Object(large_key_object)),
            (
                "nested_array_object",
                serde_json::json!([{"nested": [true, null, {"value": 1}]}]),
            ),
            ("duplicate_raw_key_last_wins", duplicate_resolved),
        ]
    }

    #[test]
    fn workflow_oracle_manifest_matches_binary() {
        let manifest = workflow_manifest();
        assert_eq!(manifest["version"], 1);

        let methods = must_array(&manifest["methods"], "manifest methods")
            .iter()
            .map(|value| must_string(value, "manifest method"))
            .collect::<Vec<_>>();
        let actual_methods = methods
            .iter()
            .map(|method| {
                event(to_notification(method, &valid_payload(method)), method).method_name()
            })
            .collect::<Vec<_>>();
        let expected_event_kinds = methods
            .iter()
            .map(|method| method.rsplit('/').next().unwrap_or(method))
            .collect::<Vec<_>>();
        assert_eq!(actual_methods, expected_event_kinds);

        let families = must_array(
            &manifest["required_shape_families"],
            "required shape families",
        )
        .iter()
        .map(|value| must_string(value, "shape family"))
        .collect::<Vec<_>>();
        let outcomes = must_object(
            &manifest["shape_boundary_outcomes"],
            "shape boundary outcomes",
        );
        assert_eq!(families.len(), 13);
        assert_eq!(outcomes.len(), families.len());
        for family in families {
            assert!(
                outcomes.contains_key(family),
                "shape family lacks an expected outcome: {family}"
            );
        }

        let expected_domains = [
            (
                "snapshot_run_status",
                &["running", "paused", "completed", "failed", "aborted"][..],
            ),
            (
                "completion_status",
                &["paused", "completed", "failed", "aborted"][..],
            ),
            (
                "node_status",
                &[
                    "pending",
                    "running",
                    "paused",
                    "completed",
                    "failed",
                    "aborted",
                    "skipped",
                ][..],
            ),
            (
                "node_type",
                &["step", "sequence", "repeat", "parallel", "watch"][..],
            ),
            (
                "watch_outcome",
                &["new-activity", "idle", "idle-timeout", "terminal-state"][..],
            ),
            ("queue_outcome", &["applied", "rejected", "dropped"][..]),
            ("completion_signal", &["success", "need_input", "error"][..]),
            ("completion_source", &["send_message", "status_update"][..]),
            ("repeat_exhaustion", &["pause", "abort"][..]),
        ];
        let domains = must_object(&manifest["enum_domains"], "enum domains");
        assert_eq!(domains.len(), expected_domains.len());
        for (name, expected) in expected_domains {
            let actual = must_array(&domains[name], name)
                .iter()
                .map(|value| must_string(value, name))
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "enum domain drift: {name}");
        }

        let warning = must_object(
            &manifest["warning_schemas"]["converter_rejection"],
            "converter warning schema",
        );
        assert_eq!(warning["level"], "WARN");
        assert_eq!(warning["message"], "malformed workflow notification");
        let warning_fields = must_array(&warning["required_fields"], "warning required fields")
            .iter()
            .map(|value| must_string(value, "warning field"))
            .collect::<Vec<_>>();
        assert_eq!(
            warning_fields,
            ["method", "field_path", "error_kind", "error"]
        );

        let fields = must_object(&manifest["fields"], "event field contracts");
        for (event_kind, contract) in fields {
            let method = format!("kiro/workflow/{event_kind}");
            for field in must_array(&contract["optional"], "optional event fields") {
                let field = must_string(field, "optional event field");
                let mut params = valid_payload(&method);
                let removed = must_object_mut(&mut params, "event payload").remove(field);
                assert!(
                    removed.is_some(),
                    "optional baseline field missing: {event_kind}.{field}"
                );
                assert!(
                    matches!(
                        to_notification(&method, &params),
                        Some(Some(Notification::Workflow(_)))
                    ),
                    "optional field omission rejected: {event_kind}.{field}"
                );
            }
        }
        let descriptor_fields =
            must_object(&manifest["descriptor_fields"], "descriptor field contracts");
        for (node_type, contract) in descriptor_fields {
            for field in must_array(&contract["optional"], "optional descriptor fields") {
                let field = must_string(field, "optional descriptor field");
                let mut params = descriptor_event_payload(valid_descriptor(node_type));
                let removed = first_descriptor_mut(&mut params).remove(field);
                assert!(
                    removed.is_some(),
                    "optional descriptor baseline field missing: {node_type}.{field}"
                );
                assert!(
                    matches!(
                        to_notification("kiro/workflow/run_start", &params),
                        Some(Some(Notification::Workflow(_)))
                    ),
                    "optional descriptor omission rejected: {node_type}.{field}"
                );
            }
        }
    }

    #[test]
    fn workflow_node_descriptor_shape_matrix() {
        let types = ["step", "sequence", "repeat", "parallel", "watch"];
        let mut params = valid_payload("kiro/workflow/run_start");
        set_top_field(
            &mut params,
            "nodeTree",
            serde_json::Value::Array(types.iter().map(|kind| valid_descriptor(kind)).collect()),
        );
        let started = match event(
            to_notification("kiro/workflow/run_start", &params),
            "descriptor shape matrix",
        ) {
            WorkflowEvent::RunStarted(started) => started,
            other => panic!("expected run_start, got {other:?}"),
        };
        let actual_types = started
            .node_tree()
            .iter()
            .map(|descriptor| descriptor.node_type().as_str())
            .collect::<Vec<_>>();
        assert_eq!(actual_types, types);
        assert_eq!(started.node_tree()[0].agent_name(), Some(""));
        assert_eq!(started.node_tree()[0].model(), Some(""));
        assert_eq!(started.node_tree()[0].effort(), Some(""));
        assert_eq!(started.node_tree()[2].max_iterations(), Some(0));
        assert_eq!(
            started.node_tree()[2]
                .on_max_iterations()
                .map(WorkflowRepeatExhaustion::as_str),
            Some("pause")
        );
        assert_eq!(started.node_tree()[4].handler_name(), Some(""));

        let recursive = serde_json::json!({
            "nodeId": "root",
            "type": "sequence",
            "steps": [{
                "nodeId": "loop",
                "type": "repeat",
                "steps": [{
                    "nodeId": "parallel",
                    "type": "parallel",
                    "branches": [
                        valid_descriptor("step"),
                        valid_descriptor("watch")
                    ]
                }],
                "maxIterations": 2,
                "onMaxIterations": "abort"
            }]
        });
        let recursive_started = match event(
            to_notification(
                "kiro/workflow/run_start",
                &descriptor_event_payload(recursive),
            ),
            "recursive descriptor",
        ) {
            WorkflowEvent::RunStarted(started) => started,
            other => panic!("expected run_start, got {other:?}"),
        };
        assert_eq!(descriptor_count(&recursive_started.node_tree()[0]), 5);
        assert_eq!(descriptor_depth(&recursive_started.node_tree()[0]), 4);

        let mut unknown = descriptor_event_payload(valid_descriptor("step"));
        first_descriptor_mut(&mut unknown).insert(
            "type".to_owned(),
            serde_json::Value::String("unknown".to_owned()),
        );
        assert!(matches!(
            to_notification("kiro/workflow/run_start", &unknown),
            Some(None)
        ));
    }

    #[test]
    fn workflow_enum_domain_matrix() {
        for node_type in ["step", "sequence", "repeat", "parallel", "watch"] {
            let mut params = valid_payload("kiro/workflow/node_start");
            set_top_field(
                &mut params,
                "type",
                serde_json::Value::String(node_type.to_owned()),
            );
            let actual = match event(
                to_notification("kiro/workflow/node_start", &params),
                "node type",
            ) {
                WorkflowEvent::NodeStarted(started) => started.node_type().as_str(),
                other => panic!("expected node_start, got {other:?}"),
            };
            assert_eq!(actual, node_type);
        }
        for status in [
            "pending",
            "running",
            "paused",
            "completed",
            "failed",
            "aborted",
            "skipped",
        ] {
            let mut params = valid_payload("kiro/workflow/node_complete");
            set_top_field(
                &mut params,
                "status",
                serde_json::Value::String(status.to_owned()),
            );
            let actual = match event(
                to_notification("kiro/workflow/node_complete", &params),
                "node status",
            ) {
                WorkflowEvent::NodeCompleted(completed) => completed.status().as_str(),
                other => panic!("expected node_complete, got {other:?}"),
            };
            assert_eq!(actual, status);
        }
        for outcome in ["new-activity", "idle", "idle-timeout", "terminal-state"] {
            let mut params = valid_payload("kiro/workflow/watch_poll");
            set_top_field(
                &mut params,
                "outcome",
                serde_json::Value::String(outcome.to_owned()),
            );
            let actual = match event(
                to_notification("kiro/workflow/watch_poll", &params),
                "watch outcome",
            ) {
                WorkflowEvent::WatchPoll(poll) => poll.outcome().as_str(),
                other => panic!("expected watch_poll, got {other:?}"),
            };
            assert_eq!(actual, outcome);
        }
        for outcome in ["applied", "rejected", "dropped"] {
            let mut params = valid_payload("kiro/workflow/steps_queued");
            set_nested_field(
                &mut params,
                "resolution.outcome",
                serde_json::Value::String(outcome.to_owned()),
            );
            let actual = match event(
                to_notification("kiro/workflow/steps_queued", &params),
                "queue outcome",
            ) {
                WorkflowEvent::StepsQueued(queued) => queued
                    .resolution()
                    .map(WorkflowQueueResolution::outcome)
                    .map(WorkflowQueueOutcome::as_str),
                other => panic!("expected steps_queued, got {other:?}"),
            };
            assert_eq!(actual, Some(outcome));
        }
        for signal in ["success", "need_input", "error"] {
            let mut params = valid_payload("kiro/workflow/node_complete");
            set_top_field(
                &mut params,
                "completionSignal",
                serde_json::Value::String(signal.to_owned()),
            );
            let actual = match event(
                to_notification("kiro/workflow/node_complete", &params),
                "completion signal",
            ) {
                WorkflowEvent::NodeCompleted(completed) => completed
                    .details()
                    .completion_signal()
                    .map(WorkflowCompletionSignal::as_str),
                other => panic!("expected node_complete, got {other:?}"),
            };
            assert_eq!(actual, Some(signal));
        }
        for source in ["send_message", "status_update"] {
            let mut params = valid_payload("kiro/workflow/node_complete");
            set_top_field(
                &mut params,
                "completionSignalSource",
                serde_json::Value::String(source.to_owned()),
            );
            let actual = match event(
                to_notification("kiro/workflow/node_complete", &params),
                "completion source",
            ) {
                WorkflowEvent::NodeCompleted(completed) => completed
                    .details()
                    .completion_signal_source()
                    .map(WorkflowCompletionSignalSource::as_str),
                other => panic!("expected node_complete, got {other:?}"),
            };
            assert_eq!(actual, Some(source));
        }
        for exhaustion in ["pause", "abort"] {
            let mut descriptor = valid_descriptor("repeat");
            must_object_mut(&mut descriptor, "repeat descriptor").insert(
                "onMaxIterations".to_owned(),
                serde_json::Value::String(exhaustion.to_owned()),
            );
            let started = match event(
                to_notification(
                    "kiro/workflow/run_start",
                    &descriptor_event_payload(descriptor),
                ),
                "repeat exhaustion",
            ) {
                WorkflowEvent::RunStarted(started) => started,
                other => panic!("expected run_start, got {other:?}"),
            };
            assert_eq!(
                started.node_tree()[0]
                    .on_max_iterations()
                    .map(WorkflowRepeatExhaustion::as_str),
                Some(exhaustion)
            );
        }

        for status in ["running", "paused", "completed", "failed", "aborted"] {
            let mut snapshot = completed_snapshot("workflow", status);
            set_nested_snapshot_node_status(&mut snapshot, status);
            let wire: WireSnapshot = must_succeed(deserialize(&snapshot), "snapshot enum");
            let snapshot = must_succeed(wire.try_into_domain(), "snapshot domain");
            assert_eq!(snapshot.status().as_str(), status);
        }
        for status in ["paused", "completed", "failed", "aborted"] {
            let mut params = valid_payload("kiro/workflow/run_complete");
            set_top_field(
                &mut params,
                "status",
                serde_json::Value::String(status.to_owned()),
            );
            set_top_field(
                &mut params,
                "finalState",
                completed_snapshot("workflow", status),
            );
            let actual = match event(
                to_notification("kiro/workflow/run_complete", &params),
                "completion status",
            ) {
                WorkflowEvent::RunCompleted(completed) => completed.status().as_str(),
                other => panic!("expected run_complete, got {other:?}"),
            };
            assert_eq!(actual, status);
        }
    }

    #[test]
    fn workflow_arbitrary_json_shape_matrix() {
        let manifest = workflow_manifest();
        let expected_names = must_array(&manifest["opaque_json_cases"], "opaque JSON cases")
            .iter()
            .map(|value| must_string(value, "opaque case"))
            .collect::<Vec<_>>();
        let values = opaque_json_values();
        assert_eq!(
            values.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            expected_names
        );
        for (name, value) in values {
            let mut params = valid_payload("kiro/workflow/run_start");
            set_top_field(&mut params, "inputs", value.clone());
            let actual = match event(to_notification("kiro/workflow/run_start", &params), name) {
                WorkflowEvent::RunStarted(started) => started.inputs().clone(),
                other => panic!("{name}: expected run_start, got {other:?}"),
            };
            assert_eq!(actual, value, "{name}: run inputs");

            let mut params = valid_payload("kiro/workflow/node_complete");
            set_top_field(&mut params, "artifacts", value.clone());
            set_top_field(&mut params, "capturedOutput", value.clone());
            let completed = match event(
                to_notification("kiro/workflow/node_complete", &params),
                name,
            ) {
                WorkflowEvent::NodeCompleted(completed) => completed,
                other => panic!("{name}: expected node_complete, got {other:?}"),
            };
            assert_eq!(completed.details().artifacts(), Some(&value));
            assert_eq!(completed.details().captured_output(), Some(&value));

            let mut descriptor = valid_descriptor("repeat");
            let descriptor_object = must_object_mut(&mut descriptor, "repeat descriptor");
            descriptor_object.insert("stopCondition".to_owned(), value.clone());
            descriptor_object.insert("stopWhen".to_owned(), value.clone());
            let started = match event(
                to_notification(
                    "kiro/workflow/run_start",
                    &descriptor_event_payload(descriptor),
                ),
                name,
            ) {
                WorkflowEvent::RunStarted(started) => started,
                other => panic!("{name}: expected run_start, got {other:?}"),
            };
            assert_eq!(started.node_tree()[0].stop_condition(), Some(&value));
            assert_eq!(started.node_tree()[0].stop_when(), Some(&value));
        }

        let mut nested = serde_json::Value::String("x".repeat(1_048_576));
        for index in 0..32 {
            nested = serde_json::json!({format!("level-{index}"): nested});
        }
        let mut params = valid_payload("kiro/workflow/run_start");
        set_top_field(&mut params, "inputs", nested.clone());
        let started_at = Instant::now();
        let actual = match event(
            to_notification("kiro/workflow/run_start", &params),
            "depth-32 opaque JSON",
        ) {
            WorkflowEvent::RunStarted(started) => started.inputs().clone(),
            other => panic!("expected run_start, got {other:?}"),
        };
        assert_eq!(actual, nested);
        assert!(
            started_at.elapsed() <= Duration::from_millis(50),
            "1 MiB depth-32 opaque conversion exceeded 50 ms"
        );
    }

    #[test]
    fn workflow_scalar_string_matrix() {
        let large = "x".repeat(65_536);
        for value in ["", "plain", "識別", " with space ", large.as_str()] {
            let mut params = valid_payload("kiro/workflow/run_start");
            set_top_field(
                &mut params,
                "workflowName",
                serde_json::Value::String(value.to_owned()),
            );
            let started = match event(
                to_notification("kiro/workflow/run_start", &params),
                "workflow name",
            ) {
                WorkflowEvent::RunStarted(started) => started,
                other => panic!("expected run_start, got {other:?}"),
            };
            assert_eq!(started.workflow_name(), value);

            let mut params = valid_payload("kiro/workflow/node_start");
            for field in ["agentName", "prompt", "branchId"] {
                set_top_field(
                    &mut params,
                    field,
                    serde_json::Value::String(value.to_owned()),
                );
            }
            let started = match event(
                to_notification("kiro/workflow/node_start", &params),
                "node strings",
            ) {
                WorkflowEvent::NodeStarted(started) => started,
                other => panic!("expected node_start, got {other:?}"),
            };
            assert_eq!(started.details().agent_name(), Some(value));
            assert_eq!(started.details().prompt(), Some(value));
            assert_eq!(started.details().branch_id(), Some(value));

            let mut params = valid_payload("kiro/workflow/node_paused");
            set_top_field(
                &mut params,
                "reason",
                serde_json::Value::String(value.to_owned()),
            );
            let paused = match event(
                to_notification("kiro/workflow/node_paused", &params),
                "node pause reason",
            ) {
                WorkflowEvent::NodePaused(paused) => paused,
                other => panic!("expected node_paused, got {other:?}"),
            };
            assert_eq!(paused.reason(), value);

            let mut descriptor = valid_descriptor("step");
            let descriptor_object = must_object_mut(&mut descriptor, "step descriptor");
            for field in ["agentName", "model", "effort"] {
                descriptor_object.insert(
                    field.to_owned(),
                    serde_json::Value::String(value.to_owned()),
                );
            }
            let descriptor_started = match event(
                to_notification(
                    "kiro/workflow/run_start",
                    &descriptor_event_payload(descriptor),
                ),
                "descriptor strings",
            ) {
                WorkflowEvent::RunStarted(started) => started,
                other => panic!("expected run_start, got {other:?}"),
            };
            let descriptor = &descriptor_started.node_tree()[0];
            assert_eq!(descriptor.agent_name(), Some(value));
            assert_eq!(descriptor.model(), Some(value));
            assert_eq!(descriptor.effort(), Some(value));
        }
    }

    #[test]
    fn workflow_identifier_string_matrix() {
        let manifest = workflow_manifest();
        let accepted = must_array(
            &manifest["identifier_cases"]["accepted"],
            "accepted identifier cases",
        )
        .iter()
        .map(|value| must_string(value, "accepted identifier case"))
        .collect::<Vec<_>>();
        assert_eq!(
            accepted,
            [
                "id",
                "識別子",
                "with space",
                "#",
                "/",
                "\\",
                "large-65536-bytes"
            ]
        );

        let large = "i".repeat(65_536);
        for case in accepted {
            let value = if case == "large-65536-bytes" {
                large.as_str()
            } else {
                case
            };
            let started_at = Instant::now();

            let mut params = valid_payload("kiro/workflow/paused");
            set_top_field(
                &mut params,
                "workflowId",
                serde_json::Value::String(value.to_owned()),
            );
            let paused = match event(
                to_notification("kiro/workflow/paused", &params),
                "workflow identifier",
            ) {
                WorkflowEvent::Paused(paused) => paused,
                other => panic!("expected paused, got {other:?}"),
            };
            assert_eq!(paused.workflow_id().as_str(), value);

            let mut params = valid_payload("kiro/workflow/loop_iteration");
            set_top_field(
                &mut params,
                "loopId",
                serde_json::Value::String(value.to_owned()),
            );
            let iteration = match event(
                to_notification("kiro/workflow/loop_iteration", &params),
                "node identifier",
            ) {
                WorkflowEvent::LoopIteration(iteration) => iteration,
                other => panic!("expected loop_iteration, got {other:?}"),
            };
            assert_eq!(iteration.loop_id().as_str(), value);

            let mut descriptor = valid_descriptor("step");
            must_object_mut(&mut descriptor, "descriptor").insert(
                "nodeId".to_owned(),
                serde_json::Value::String(value.to_owned()),
            );
            let started = match event(
                to_notification(
                    "kiro/workflow/run_start",
                    &descriptor_event_payload(descriptor),
                ),
                "descriptor identifier",
            ) {
                WorkflowEvent::RunStarted(started) => started,
                other => panic!("expected run_start, got {other:?}"),
            };
            assert_eq!(started.node_tree()[0].node_id().as_str(), value);

            let mut snapshot = completed_snapshot("workflow", "completed");
            set_top_field(
                &mut snapshot,
                "workflowId",
                serde_json::Value::String(value.to_owned()),
            );
            let wire: WireSnapshot =
                must_succeed(deserialize(&snapshot), "snapshot identifier wire");
            let snapshot = must_succeed(wire.try_into_domain(), "snapshot identifier domain");
            assert_eq!(snapshot.workflow_id().as_str(), value);

            if case == "large-65536-bytes" {
                assert!(
                    started_at.elapsed() <= Duration::from_millis(50),
                    "64 KiB identifier conversions exceeded 50 ms"
                );
            }
        }

        let rejected = must_array(
            &manifest["identifier_cases"]["rejected"],
            "rejected identifier cases",
        );
        assert_eq!(rejected, &[serde_json::Value::String(String::new())]);
        for (method, field) in [
            ("kiro/workflow/paused", "workflowId"),
            ("kiro/workflow/loop_iteration", "loopId"),
        ] {
            let mut params = valid_payload(method);
            set_top_field(&mut params, field, serde_json::Value::String(String::new()));
            assert!(
                matches!(to_notification(method, &params), Some(None)),
                "empty identifier accepted: {method}.{field}"
            );
        }
        let mut descriptor = valid_descriptor("step");
        must_object_mut(&mut descriptor, "descriptor").insert(
            "nodeId".to_owned(),
            serde_json::Value::String(String::new()),
        );
        assert!(matches!(
            to_notification(
                "kiro/workflow/run_start",
                &descriptor_event_payload(descriptor)
            ),
            Some(None)
        ));
        let empty_snapshot = completed_snapshot("", "completed");
        let wire: WireSnapshot = must_succeed(
            deserialize(&empty_snapshot),
            "empty snapshot identifier wire",
        );
        assert!(wire.try_into_domain().is_err());
    }

    #[test]
    fn workflow_optional_field_presence_matrix() {
        for present in [false, true] {
            let mut params = valid_payload("kiro/workflow/run_start");
            if !present {
                let removed = must_object_mut(&mut params, "run_start").remove("parentSessionId");
                assert!(removed.is_some());
            }
            let started = match event(
                to_notification("kiro/workflow/run_start", &params),
                "run_start optional",
            ) {
                WorkflowEvent::RunStarted(started) => started,
                other => panic!("expected run_start, got {other:?}"),
            };
            assert_eq!(started.parent_session_id().is_some(), present);

            let mut params = valid_payload("kiro/workflow/steps_queued");
            if !present {
                let removed = must_object_mut(&mut params, "steps_queued").remove("resolution");
                assert!(removed.is_some());
            }
            let queued = match event(
                to_notification("kiro/workflow/steps_queued", &params),
                "queue optional",
            ) {
                WorkflowEvent::StepsQueued(queued) => queued,
                other => panic!("expected steps_queued, got {other:?}"),
            };
            assert_eq!(queued.resolution().is_some(), present);
        }

        let start_fields = ["agentName", "sessionId", "prompt", "iteration", "branchId"];
        for mask in 0..(1_u32 << start_fields.len()) {
            let mut params = valid_payload("kiro/workflow/node_start");
            for (index, field) in start_fields.iter().enumerate() {
                if mask & (1 << index) == 0 {
                    let removed = must_object_mut(&mut params, "node_start").remove(*field);
                    assert!(removed.is_some());
                }
            }
            let started = match event(
                to_notification("kiro/workflow/node_start", &params),
                "node_start optional mask",
            ) {
                WorkflowEvent::NodeStarted(started) => started,
                other => panic!("expected node_start, got {other:?}"),
            };
            let actual = [
                started.details().agent_name().is_some(),
                started.details().session_id().is_some(),
                started.details().prompt().is_some(),
                started.details().iteration().is_some(),
                started.details().branch_id().is_some(),
            ];
            for (index, value) in actual.into_iter().enumerate() {
                assert_eq!(value, mask & (1 << index) != 0);
            }
        }

        let completion_fields = [
            "artifacts",
            "capturedOutput",
            "failureReason",
            "completionSignal",
            "completionSignalSource",
        ];
        for mask in 0..(1_u32 << completion_fields.len()) {
            let mut params = valid_payload("kiro/workflow/node_complete");
            for (index, field) in completion_fields.iter().enumerate() {
                if mask & (1 << index) == 0 {
                    let removed = must_object_mut(&mut params, "node_complete").remove(*field);
                    assert!(removed.is_some());
                }
            }
            let completed = match event(
                to_notification("kiro/workflow/node_complete", &params),
                "node_complete optional mask",
            ) {
                WorkflowEvent::NodeCompleted(completed) => completed,
                other => panic!("expected node_complete, got {other:?}"),
            };
            let actual = [
                completed.details().artifacts().is_some(),
                completed.details().captured_output().is_some(),
                completed.details().failure_reason().is_some(),
                completed.details().completion_signal().is_some(),
                completed.details().completion_signal_source().is_some(),
            ];
            for (index, value) in actual.into_iter().enumerate() {
                assert_eq!(value, mask & (1 << index) != 0);
            }
        }

        for mask in 0..4 {
            let mut step = valid_descriptor("step");
            let step_object = must_object_mut(&mut step, "step descriptor");
            if mask & 1 == 0 {
                let removed = step_object.remove("model");
                assert!(removed.is_some());
            }
            if mask & 2 == 0 {
                let removed = step_object.remove("effort");
                assert!(removed.is_some());
            }
            let started = match event(
                to_notification("kiro/workflow/run_start", &descriptor_event_payload(step)),
                "step optional mask",
            ) {
                WorkflowEvent::RunStarted(started) => started,
                other => panic!("expected run_start, got {other:?}"),
            };
            assert_eq!(started.node_tree()[0].model().is_some(), mask & 1 != 0);
            assert_eq!(started.node_tree()[0].effort().is_some(), mask & 2 != 0);

            let mut repeat = valid_descriptor("repeat");
            let repeat_object = must_object_mut(&mut repeat, "repeat descriptor");
            if mask & 1 == 0 {
                let removed = repeat_object.remove("stopCondition");
                assert!(removed.is_some());
            }
            if mask & 2 == 0 {
                let removed = repeat_object.remove("stopWhen");
                assert!(removed.is_some());
            }
            let started = match event(
                to_notification("kiro/workflow/run_start", &descriptor_event_payload(repeat)),
                "repeat optional mask",
            ) {
                WorkflowEvent::RunStarted(started) => started,
                other => panic!("expected run_start, got {other:?}"),
            };
            assert_eq!(
                started.node_tree()[0].stop_condition().is_some(),
                mask & 1 != 0
            );
            assert_eq!(started.node_tree()[0].stop_when().is_some(), mask & 2 != 0);
        }

        let optional_snapshot_fields = ["parentSessionId", "workspacePath"];
        let optional_node_fields = [
            "sessionId",
            "artifacts",
            "capturedOutput",
            "failureReason",
            "iteration",
            "branchId",
            "completionSignal",
            "completionSignalSource",
            "startedAt",
            "endedAt",
        ];
        for field in optional_snapshot_fields {
            for present in [false, true] {
                let mut snapshot = completed_snapshot("workflow", "completed");
                if present {
                    set_top_field(
                        &mut snapshot,
                        field,
                        serde_json::Value::String(String::new()),
                    );
                }
                let wire: WireSnapshot =
                    must_succeed(deserialize(&snapshot), "snapshot optional field");
                let snapshot = must_succeed(wire.try_into_domain(), "snapshot optional domain");
                let actual = match field {
                    "parentSessionId" => snapshot.parent_session_id().is_some(),
                    "workspacePath" => snapshot.workspace_path().is_some(),
                    other => panic!("unknown snapshot optional field {other}"),
                };
                assert_eq!(actual, present);
            }
        }
        for field in optional_node_fields {
            for present in [false, true] {
                let mut snapshot = completed_snapshot("workflow", "completed");
                let root = match must_object_mut(&mut snapshot, "snapshot").get_mut("root") {
                    Some(value) => value,
                    None => panic!("snapshot root absent"),
                };
                if present {
                    let value = match field {
                        "iteration" => serde_json::json!(0),
                        "artifacts" | "capturedOutput" => serde_json::Value::Null,
                        "completionSignal" => serde_json::Value::String("success".to_owned()),
                        "completionSignalSource" => {
                            serde_json::Value::String("send_message".to_owned())
                        }
                        _ => serde_json::Value::String(String::new()),
                    };
                    must_object_mut(root, "snapshot root").insert(field.to_owned(), value);
                }
                let wire: WireSnapshot =
                    must_succeed(deserialize(&snapshot), "node optional field");
                let snapshot = must_succeed(wire.try_into_domain(), "node optional domain");
                let node = snapshot.root();
                let actual = match field {
                    "sessionId" => node.session_id().is_some(),
                    "artifacts" => node.artifacts().is_some(),
                    "capturedOutput" => node.captured_output().is_some(),
                    "failureReason" => node.failure_reason().is_some(),
                    "iteration" => node.iteration().is_some(),
                    "branchId" => node.branch_id().is_some(),
                    "completionSignal" => node.completion_signal().is_some(),
                    "completionSignalSource" => node.completion_signal_source().is_some(),
                    "startedAt" => node.started_at().is_some(),
                    "endedAt" => node.ended_at().is_some(),
                    other => panic!("unknown node optional field {other}"),
                };
                assert_eq!(actual, present, "snapshot root optional {field}");
            }
        }
    }

    #[test]
    fn workflow_collection_shape_matrix() {
        for descriptors in [
            Vec::new(),
            vec![step_descriptor("one")],
            vec![
                step_descriptor("one"),
                step_descriptor("two"),
                step_descriptor("one"),
            ],
        ] {
            let mut params = valid_payload("kiro/workflow/run_start");
            set_top_field(
                &mut params,
                "nodeTree",
                serde_json::Value::Array(descriptors.clone()),
            );
            let started = match event(
                to_notification("kiro/workflow/run_start", &params),
                "nodeTree cardinality",
            ) {
                WorkflowEvent::RunStarted(started) => started,
                other => panic!("expected run_start, got {other:?}"),
            };
            let actual = started
                .node_tree()
                .iter()
                .map(|descriptor| descriptor.node_id().as_str())
                .collect::<Vec<_>>();
            let expected = descriptors
                .iter()
                .map(|descriptor| {
                    must_string(
                        &must_object(descriptor, "descriptor")["nodeId"],
                        "descriptor nodeId",
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
        }

        let mut path_params = valid_payload("kiro/workflow/node_start");
        set_top_field(
            &mut path_params,
            "nodePath",
            serde_json::json!(["workflow", "same", "same"]),
        );
        let started = match event(
            to_notification("kiro/workflow/node_start", &path_params),
            "duplicate path segments",
        ) {
            WorkflowEvent::NodeStarted(started) => started,
            other => panic!("expected node_start, got {other:?}"),
        };
        assert_eq!(
            started.node_path().segments(),
            &["workflow", "same", "same"]
        );

        let mut queue_params = valid_payload("kiro/workflow/steps_queued");
        set_top_field(
            &mut queue_params,
            "pendingSteps",
            serde_json::json!([
                step_descriptor("one"),
                step_descriptor("two"),
                step_descriptor("one")
            ]),
        );
        let queued = match event(
            to_notification("kiro/workflow/steps_queued", &queue_params),
            "pendingSteps order",
        ) {
            WorkflowEvent::StepsQueued(queued) => queued,
            other => panic!("expected steps_queued, got {other:?}"),
        };
        assert_eq!(
            queued
                .pending_steps()
                .iter()
                .map(|descriptor| descriptor.node_id().as_str())
                .collect::<Vec<_>>(),
            ["one", "two", "one"]
        );

        let duplicate_canonical = serde_json::json!({
            "nodeId": "root",
            "type": "sequence",
            "steps": [
                step_descriptor("duplicate"),
                step_descriptor("duplicate")
            ]
        });
        let started = match event(
            to_notification(
                "kiro/workflow/run_start",
                &descriptor_event_payload(duplicate_canonical),
            ),
            "duplicate canonical paths",
        ) {
            WorkflowEvent::RunStarted(started) => started,
            other => panic!("expected run_start, got {other:?}"),
        };
        assert_eq!(descriptor_count(&started.node_tree()[0]), 3);
        assert_eq!(
            started.node_tree()[0].children()[0].node_id(),
            started.node_tree()[0].children()[1].node_id()
        );

        let nodes = (0..256)
            .map(|index| step_descriptor(&format!("node-{index}")))
            .collect::<Vec<_>>();
        let mut large = valid_payload("kiro/workflow/run_start");
        set_top_field(&mut large, "nodeTree", serde_json::Value::Array(nodes));
        let started_at = Instant::now();
        let started = match event(
            to_notification("kiro/workflow/run_start", &large),
            "256-node collection",
        ) {
            WorkflowEvent::RunStarted(started) => started,
            other => panic!("expected run_start, got {other:?}"),
        };
        assert_eq!(started.node_tree().len(), 256);
        assert!(
            started_at.elapsed() <= Duration::from_millis(50),
            "256-node conversion exceeded 50 ms"
        );
    }

    #[test]
    fn workflow_duplicate_raw_json_key_last_wins() {
        let params: serde_json::Value = must_succeed(
            serde_json::from_str(
                r#"{
                    "workflowId": "",
                    "workflowId": "workflow",
                    "workflowName": "first",
                    "workflowName": "last",
                    "inputs": {"value": 1, "value": 2},
                    "inputs": {"chosen": true},
                    "nodeTree": [{
                        "nodeId": "step",
                        "type": "step",
                        "agentName": "first",
                        "agentName": "last"
                    }]
                }"#,
            ),
            "duplicate raw run_start",
        );
        let started = match event(
            to_notification("kiro/workflow/run_start", &params),
            "duplicate raw run_start",
        ) {
            WorkflowEvent::RunStarted(started) => started,
            other => panic!("expected run_start, got {other:?}"),
        };
        assert_eq!(started.workflow_id().as_str(), "workflow");
        assert_eq!(started.workflow_name(), "last");
        assert_eq!(started.inputs(), &serde_json::json!({"chosen": true}));
        assert_eq!(started.node_tree()[0].agent_name(), Some("last"));

        let params: serde_json::Value = must_succeed(
            serde_json::from_str(
                r#"{
                    "workflowId": "workflow",
                    "nodeId": "node",
                    "nodePath": ["wrong", "node"],
                    "nodePath": ["workflow", "node"],
                    "status": "failed",
                    "status": "completed",
                    "artifacts": {"first": true},
                    "artifacts": {"last": true}
                }"#,
            ),
            "duplicate raw node_complete",
        );
        let completed = match event(
            to_notification("kiro/workflow/node_complete", &params),
            "duplicate raw node_complete",
        ) {
            WorkflowEvent::NodeCompleted(completed) => completed,
            other => panic!("expected node_complete, got {other:?}"),
        };
        assert_eq!(completed.status(), WorkflowNodeStatus::Completed);
        assert_eq!(
            completed.details().artifacts(),
            Some(&serde_json::json!({"last": true}))
        );
    }

    #[test]
    fn workflow_numeric_and_path_boundaries() {
        for iteration in [0_u32, u32::MAX] {
            let mut params = valid_payload("kiro/workflow/node_start");
            set_top_field(&mut params, "iteration", serde_json::json!(iteration));
            let started = match event(
                to_notification("kiro/workflow/node_start", &params),
                "node iteration",
            ) {
                WorkflowEvent::NodeStarted(started) => started,
                other => panic!("expected node_start, got {other:?}"),
            };
            assert_eq!(started.details().iteration(), Some(iteration));

            let mut params = valid_payload("kiro/workflow/loop_iteration");
            set_top_field(&mut params, "iteration", serde_json::json!(iteration));
            let progress = match event(
                to_notification("kiro/workflow/loop_iteration", &params),
                "loop iteration",
            ) {
                WorkflowEvent::LoopIteration(progress) => progress,
                other => panic!("expected loop_iteration, got {other:?}"),
            };
            assert_eq!(progress.iteration(), iteration);

            let mut descriptor = valid_descriptor("repeat");
            must_object_mut(&mut descriptor, "repeat descriptor")
                .insert("maxIterations".to_owned(), serde_json::json!(iteration));
            let started = match event(
                to_notification(
                    "kiro/workflow/run_start",
                    &descriptor_event_payload(descriptor),
                ),
                "repeat max iterations",
            ) {
                WorkflowEvent::RunStarted(started) => started,
                other => panic!("expected run_start, got {other:?}"),
            };
            assert_eq!(started.node_tree()[0].max_iterations(), Some(iteration));

            let mut snapshot = completed_snapshot("workflow", "completed");
            set_top_field(&mut snapshot, "planRevision", serde_json::json!(iteration));
            let wire: WireSnapshot = must_succeed(deserialize(&snapshot), "plan revision boundary");
            let snapshot = must_succeed(wire.try_into_domain(), "plan revision domain");
            assert_eq!(snapshot.plan_revision(), iteration);
        }

        for invalid in [
            serde_json::json!(-1),
            serde_json::json!(u64::from(u32::MAX) + 1),
        ] {
            for (method, field) in [
                ("kiro/workflow/node_start", "iteration"),
                ("kiro/workflow/loop_iteration", "iteration"),
            ] {
                let mut params = valid_payload(method);
                set_top_field(&mut params, field, invalid.clone());
                assert!(
                    matches!(to_notification(method, &params), Some(None)),
                    "out-of-range integer accepted: {method}.{field}"
                );
            }
            let mut descriptor = valid_descriptor("repeat");
            must_object_mut(&mut descriptor, "repeat descriptor")
                .insert("maxIterations".to_owned(), invalid.clone());
            assert!(matches!(
                to_notification(
                    "kiro/workflow/run_start",
                    &descriptor_event_payload(descriptor)
                ),
                Some(None)
            ));
            let mut snapshot = completed_snapshot("workflow", "completed");
            set_top_field(&mut snapshot, "planRevision", invalid);
            assert!(deserialize::<WireSnapshot>(&snapshot).is_err());
        }

        let large_segment = "p".repeat(65_536);
        for path in [
            vec!["workflow".to_owned()],
            vec!["workflow".to_owned(), "node".to_owned()],
            vec!["workflow".to_owned(), "節".to_owned(), "#".to_owned()],
            vec!["workflow".to_owned(), large_segment.clone()],
        ] {
            let mut params = valid_payload("kiro/workflow/node_start");
            set_top_field(&mut params, "nodePath", serde_json::json!(path));
            let started_at = Instant::now();
            let started = match event(
                to_notification("kiro/workflow/node_start", &params),
                "valid node path",
            ) {
                WorkflowEvent::NodeStarted(started) => started,
                other => panic!("expected node_start, got {other:?}"),
            };
            assert_eq!(started.node_path().segments(), path);
            if path.last().is_some_and(|segment| segment.len() == 65_536) {
                assert!(
                    started_at.elapsed() <= Duration::from_millis(50),
                    "64 KiB path segment conversion exceeded 50 ms"
                );
            }
        }

        for invalid in [
            serde_json::json!([]),
            serde_json::json!(["wrong", "node"]),
            serde_json::json!(["workflow", ""]),
            serde_json::json!(["workflow", false]),
            serde_json::Value::Bool(false),
            serde_json::Value::Null,
            serde_json::Value::String("workflow/node".to_owned()),
        ] {
            let mut params = valid_payload("kiro/workflow/node_start");
            set_top_field(&mut params, "nodePath", invalid);
            assert!(matches!(
                to_notification("kiro/workflow/node_start", &params),
                Some(None)
            ));
        }
    }

    #[test]
    fn workflow_workspace_path_is_opaque() {
        let large = "w".repeat(65_536);
        for path in [
            "",
            "/home/user/work",
            r"C:\work\recipe",
            r"\\wsl$\Distro\home\user",
            "相対/路徑",
            " with space ",
            large.as_str(),
        ] {
            let mut final_state = completed_snapshot("workflow", "completed");
            set_top_field(
                &mut final_state,
                "workspacePath",
                serde_json::Value::String(path.to_owned()),
            );
            let mut params = valid_payload("kiro/workflow/run_complete");
            set_top_field(&mut params, "finalState", final_state);
            let started_at = Instant::now();
            let completed = match event(
                to_notification("kiro/workflow/run_complete", &params),
                "workspace path",
            ) {
                WorkflowEvent::RunCompleted(completed) => completed,
                other => panic!("expected run_complete, got {other:?}"),
            };
            assert_eq!(completed.final_state().workspace_path(), Some(path));
            if path.len() == 65_536 {
                assert!(
                    started_at.elapsed() <= Duration::from_millis(50),
                    "64 KiB workspace path conversion exceeded 50 ms"
                );
            }
        }
    }

    #[test]
    fn workflow_typed_unknown_extra_is_ignored() {
        let unknown = serde_json::json!({
            "nested": [null, false, {"deep": "value"}],
            "large": "x".repeat(65_536)
        });
        for method in [
            "kiro/workflow/run_start",
            "kiro/workflow/node_start",
            "kiro/workflow/node_complete",
            "kiro/workflow/node_paused",
            "kiro/workflow/loop_iteration",
            "kiro/workflow/watch_poll",
            "kiro/workflow/paused",
            "kiro/workflow/run_complete",
            "kiro/workflow/steps_queued",
        ] {
            let baseline = event(to_notification(method, &valid_payload(method)), method);
            let mut params = valid_payload(method);
            set_top_field(&mut params, "futureField", unknown.clone());
            let with_extra = event(to_notification(method, &params), method);
            assert_eq!(with_extra, baseline, "top-level typed extra: {method}");
        }

        for node_type in ["step", "sequence", "repeat", "parallel", "watch"] {
            let baseline = event(
                to_notification(
                    "kiro/workflow/run_start",
                    &descriptor_event_payload(valid_descriptor(node_type)),
                ),
                node_type,
            );
            let mut descriptor = valid_descriptor(node_type);
            must_object_mut(&mut descriptor, "descriptor")
                .insert("futureField".to_owned(), unknown.clone());
            let with_extra = event(
                to_notification(
                    "kiro/workflow/run_start",
                    &descriptor_event_payload(descriptor),
                ),
                node_type,
            );
            assert_eq!(with_extra, baseline, "descriptor typed extra: {node_type}");
        }

        let baseline = valid_payload("kiro/workflow/run_complete");
        let baseline_event = event(
            to_notification("kiro/workflow/run_complete", &baseline),
            "snapshot baseline",
        );
        let mut with_extra = baseline;
        let final_state = match must_object_mut(&mut with_extra, "completion").get_mut("finalState")
        {
            Some(value) => value,
            None => panic!("completion lacks finalState"),
        };
        set_top_field(final_state, "futureField", unknown.clone());
        let root = match must_object_mut(final_state, "snapshot").get_mut("root") {
            Some(value) => value,
            None => panic!("snapshot lacks root"),
        };
        set_top_field(root, "futureField", unknown);
        let with_extra_event = event(
            to_notification("kiro/workflow/run_complete", &with_extra),
            "snapshot extras",
        );
        assert_eq!(with_extra_event, baseline_event);
    }

    #[test]
    fn workflow_run_complete_status_mismatch_rejected() {
        let completion_statuses = ["paused", "completed", "failed", "aborted"];
        let snapshot_statuses = ["running", "paused", "completed", "failed", "aborted"];
        for completion in completion_statuses {
            for snapshot in snapshot_statuses {
                let mut params = valid_payload("kiro/workflow/run_complete");
                set_top_field(
                    &mut params,
                    "status",
                    serde_json::Value::String(completion.to_owned()),
                );
                set_top_field(
                    &mut params,
                    "finalState",
                    completed_snapshot("workflow", snapshot),
                );
                let matches = completion == snapshot;
                if matches {
                    let completed = match event(
                        to_notification("kiro/workflow/run_complete", &params),
                        "matching completion",
                    ) {
                        WorkflowEvent::RunCompleted(completed) => completed,
                        other => panic!("expected run_complete, got {other:?}"),
                    };
                    assert_eq!(completed.status().as_str(), completion);
                    assert_eq!(completed.final_state().status().as_str(), snapshot);
                } else {
                    let (result, log) = capture_rejection("kiro/workflow/run_complete", &params);
                    assert!(matches!(result, Some(None)));
                    assert_eq!(log["level"], "WARN");
                    assert_eq!(log["fields"]["method"], "kiro/workflow/run_complete");
                    assert_eq!(log["fields"]["field_path"], "status");
                    assert_eq!(log["fields"]["error_kind"], "status_mismatch");
                    assert_eq!(log["fields"]["message"], "malformed workflow notification");
                }
            }
        }
    }

    fn completed_snapshot(workflow_id: &str, status: &str) -> serde_json::Value {
        serde_json::json!({
            "workflowId": workflow_id,
            "workflowName": "recipe",
            "status": status,
            "inputs": {},
            "artifacts": {},
            "capturedOutputs": {},
            "root": {
                "nodeId": workflow_id,
                "type": "sequence",
                "status": status,
                "children": []
            },
            "createdAt": "",
            "planRevision": 0
        })
    }

    fn step_snapshot(node_id: &str) -> serde_json::Value {
        serde_json::json!({
            "nodeId": node_id,
            "type": "step",
            "status": "completed",
            "agentName": "agent"
        })
    }

    fn step_descriptor(node_id: &str) -> serde_json::Value {
        serde_json::json!({
            "nodeId": node_id,
            "type": "step",
            "agentName": "agent"
        })
    }

    fn descriptor_count(node: &WorkflowNodeDescriptor) -> usize {
        1 + node.children().iter().map(descriptor_count).sum::<usize>()
    }

    fn descriptor_depth(node: &WorkflowNodeDescriptor) -> usize {
        1 + node
            .children()
            .iter()
            .map(descriptor_depth)
            .max()
            .unwrap_or(0)
    }

    fn snapshot_node_count(node: &WorkflowNodeSnapshot) -> usize {
        1 + node
            .children()
            .iter()
            .map(snapshot_node_count)
            .sum::<usize>()
    }

    fn snapshot_depth(node: &WorkflowNodeSnapshot) -> usize {
        1 + node
            .children()
            .iter()
            .map(snapshot_depth)
            .max()
            .unwrap_or(0)
    }
}
