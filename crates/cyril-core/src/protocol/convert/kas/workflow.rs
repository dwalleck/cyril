//! Conversion for KAS `kiro/workflow/*` lifecycle notifications.
//!
//! The ACP crate removes the leading underscore from extension method names,
//! so this module matches the normalized `kiro/workflow/*` spelling exactly.

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::types::{
    Notification, SessionId, WorkflowCompletionMismatchError, WorkflowCompletionSignal,
    WorkflowCompletionSignalSource, WorkflowCompletionStatus, WorkflowEvent, WorkflowId,
    WorkflowIdentifierError, WorkflowNodeDescriptor, WorkflowNodeId, WorkflowNodeSnapshot,
    WorkflowNodeStatus, WorkflowRepeatExhaustion, WorkflowRunCompleted, WorkflowRunStarted,
    WorkflowRunStatus, WorkflowSnapshot, WorkflowSnapshotData, WorkflowSnapshotMetadata,
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

#[derive(Debug, thiserror::Error)]
enum WorkflowAdapterError {
    #[error("malformed workflow payload: {0}")]
    Malformed(#[from] serde_json::Error),
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
        stop_condition: OptionalField<serde_json::Value>,
        #[serde(rename = "stopWhen", default)]
        stop_when: OptionalField<serde_json::Value>,
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
    stop_condition: OptionalField<serde_json::Value>,
    #[serde(default)]
    stop_when: OptionalField<serde_json::Value>,
    #[serde(default)]
    handler_name: OptionalField<String>,
    #[serde(default)]
    session_id: OptionalField<String>,
    #[serde(default)]
    artifacts: OptionalField<serde_json::Value>,
    #[serde(default)]
    captured_output: OptionalField<serde_json::Value>,
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

/// Returns `None` when `method` is not owned by this adapter. For an exact
/// workflow method, returns `Some(Some(notification))` on success or
/// `Some(None)` after warning and dropping malformed input.
pub(super) fn to_notification(
    method: &str,
    params: &serde_json::Value,
) -> Option<Option<Notification>> {
    let event = match method {
        "kiro/workflow/run_start" => parse_run_started(params),
        "kiro/workflow/run_complete" => parse_run_completed(params),
        _ => return None,
    };

    Some(match event {
        Ok(event) => Some(Notification::Workflow(Box::new(event))),
        Err(error) => {
            tracing::warn!(
                method,
                error = %error,
                "malformed KAS workflow lifecycle notification; dropped"
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
    T::deserialize(value).map_err(WorkflowAdapterError::Malformed)
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
    use std::time::{Duration, Instant};

    use super::*;

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

    fn capture_params(source: &str, expected_status: &str) -> serde_json::Value {
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
                return envelope["params"].clone();
            }
        }
        panic!("capture contains no {expected_status} run_complete");
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
