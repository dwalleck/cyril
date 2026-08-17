//! Conversion for KAS `kiro/workflow/*` lifecycle notifications.
//!
//! The ACP crate removes the leading underscore from extension method names,
//! so this module matches the normalized `kiro/workflow/*` spelling exactly.

use serde::Deserialize;
use serde::de::DeserializeOwned;

use super::WorkflowFrameOutcome;
use crate::types::{
    SessionId, WorkflowCompletionMismatchError, WorkflowCompletionSignal,
    WorkflowCompletionSignalSource, WorkflowCompletionStatus, WorkflowEnumParseError,
    WorkflowEvent, WorkflowId, WorkflowIdentifierError, WorkflowLoopIteration,
    WorkflowNodeCompleted, WorkflowNodeCompletionDetails, WorkflowNodeDescriptor, WorkflowNodeId,
    WorkflowNodePath, WorkflowNodePathError, WorkflowNodePaused, WorkflowNodeSnapshot,
    WorkflowNodeStartDetails, WorkflowNodeStarted, WorkflowNodeStatus, WorkflowNodeType,
    WorkflowPaused, WorkflowQueueOutcome, WorkflowQueueResolution, WorkflowRepeatExhaustion,
    WorkflowRunCompleted, WorkflowRunStarted, WorkflowRunStatus, WorkflowSnapshot,
    WorkflowSnapshotData, WorkflowSnapshotMetadata, WorkflowStepsQueued, WorkflowWatchOutcome,
    WorkflowWatchPoll,
};
use crate::types::{WorkflowRecipe, WorkflowRunSummary};

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

/// Wire enum field parsed through the domain vocabulary's `TryFrom<&str>`.
///
/// Every closed wire spelling lives once, in the `workflow_enum!` tables in
/// `types::workflow`; this wrapper keeps rejection at deserialization time so
/// `serde_path_to_error` still reports the exact field path.
#[derive(Debug, Clone, Copy)]
struct WireEnum<T>(T);

impl<'de, T> Deserialize<'de> for WireEnum<T>
where
    T: for<'a> TryFrom<&'a str, Error = WorkflowEnumParseError>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        T::try_from(value.as_str())
            .map(Self)
            .map_err(serde::de::Error::custom)
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
    #[error(
        "run completion workflow id `{outer}` does not match final snapshot workflow id `{final_id}`"
    )]
    SnapshotWorkflowMismatch { outer: String, final_id: String },
    #[error("reply workflow id `{outer}` does not match its state's workflow id `{inner}`")]
    ReplyWorkflowMismatch { outer: String, inner: String },
    #[error(transparent)]
    CompletionMismatch(#[from] WorkflowCompletionMismatchError),
}

impl WorkflowAdapterError {
    fn field_path(&self) -> &str {
        match self {
            Self::MalformedField { field_path, .. } => field_path,
            Self::InvalidWorkflowId { field, .. } | Self::InvalidNodeId { field, .. } => field,
            Self::InvalidNodePath(_) => "nodePath",
            Self::SnapshotWorkflowMismatch { .. } => "finalState.workflowId",
            Self::ReplyWorkflowMismatch { .. } => "state.workflowId",
            Self::CompletionMismatch(WorkflowCompletionMismatchError::Status { .. }) => "status",
            Self::CompletionMismatch(WorkflowCompletionMismatchError::WorkflowId { .. }) => {
                "finalState.workflowId"
            }
        }
    }

    fn error_kind(&self) -> WorkflowErrorKind {
        match self {
            Self::MalformedField { error_kind, .. } => *error_kind,
            Self::InvalidWorkflowId { .. }
            | Self::InvalidNodeId { .. }
            | Self::InvalidNodePath(_)
            | Self::SnapshotWorkflowMismatch { .. } => WorkflowErrorKind::InvalidValue,
            Self::ReplyWorkflowMismatch { .. } => WorkflowErrorKind::InvalidValue,
            Self::CompletionMismatch(WorkflowCompletionMismatchError::Status { .. }) => {
                WorkflowErrorKind::StatusMismatch
            }
            Self::CompletionMismatch(WorkflowCompletionMismatchError::WorkflowId { .. }) => {
                WorkflowErrorKind::InvalidValue
            }
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
    status: WireEnum<WorkflowCompletionStatus>,
    final_state: WireSnapshot,
    #[serde(default)]
    parent_session_id: OptionalField<String>,
}

/// Reply of `_kiro/workflow/inspect` (field `state`) or `_kiro/workflow/new`
/// (field `initialState`). Extra reply members (`nodePlan`; `stepSessions`
/// on ≤2.16.2, dropped by 2.18.0) are deliberately ignored — only the
/// snapshot seeds the tracker.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireStateReply {
    workflow_id: String,
    #[serde(alias = "initialState")]
    state: WireSnapshot,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireNodeStarted {
    workflow_id: String,
    node_id: String,
    node_path: Vec<String>,
    #[serde(rename = "type")]
    node_type: WireEnum<WorkflowNodeType>,
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
    #[serde(default)]
    parent_session_id: OptionalField<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireNodeCompleted {
    workflow_id: String,
    node_id: String,
    node_path: Vec<String>,
    status: WireEnum<WorkflowNodeStatus>,
    #[serde(default)]
    artifacts: OptionalValue,
    #[serde(default)]
    captured_output: OptionalValue,
    #[serde(default)]
    failure_reason: OptionalField<String>,
    #[serde(default)]
    completion_signal: OptionalField<WireEnum<WorkflowCompletionSignal>>,
    #[serde(default)]
    completion_signal_source: OptionalField<WireEnum<WorkflowCompletionSignalSource>>,
    #[serde(default)]
    parent_session_id: OptionalField<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireNodePaused {
    workflow_id: String,
    node_id: String,
    node_path: Vec<String>,
    reason: String,
    #[serde(default)]
    parent_session_id: OptionalField<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireLoopIteration {
    workflow_id: String,
    loop_id: String,
    iteration: u32,
    stop_condition_met: bool,
    #[serde(default)]
    parent_session_id: OptionalField<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireWatchPoll {
    workflow_id: String,
    node_id: String,
    node_path: Vec<String>,
    outcome: WireEnum<WorkflowWatchOutcome>,
    at: String,
    #[serde(default)]
    parent_session_id: OptionalField<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WirePaused {
    workflow_id: String,
    pause_reason: String,
    #[serde(default)]
    parent_session_id: OptionalField<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireStepsQueued {
    workflow_id: String,
    pending_steps: Vec<WireNodeDescriptor>,
    #[serde(default)]
    resolution: OptionalField<WireQueueResolution>,
    #[serde(default)]
    parent_session_id: OptionalField<String>,
}

#[derive(Deserialize)]
struct WireQueueResolution {
    outcome: WireEnum<WorkflowQueueOutcome>,
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
        #[serde(rename = "modelId", default)]
        model_id: OptionalField<String>,
        #[serde(rename = "effortLevel", default)]
        effort_level: OptionalField<String>,
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
        on_max_iterations: WireEnum<WorkflowRepeatExhaustion>,
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
        #[serde(rename = "agentName")]
        agent_name: String,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireSnapshot {
    workflow_id: String,
    workflow_name: String,
    status: WireEnum<WorkflowRunStatus>,
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
    node_type: WireEnum<WorkflowNodeType>,
    status: WireEnum<WorkflowNodeStatus>,
    #[serde(default)]
    children: OptionalField<Vec<Self>>,
    #[serde(default)]
    agent_name: OptionalField<String>,
    #[serde(default)]
    model_id: OptionalField<String>,
    #[serde(default)]
    effort_level: OptionalField<String>,
    #[serde(default)]
    max_iterations: OptionalField<u32>,
    #[serde(default)]
    on_max_iterations: OptionalField<WireEnum<WorkflowRepeatExhaustion>>,
    #[serde(default)]
    stop_condition: OptionalValue,
    #[serde(default)]
    stop_when: OptionalValue,
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
    completion_signal: OptionalField<WireEnum<WorkflowCompletionSignal>>,
    #[serde(default)]
    completion_signal_source: OptionalField<WireEnum<WorkflowCompletionSignalSource>>,
    #[serde(default)]
    started_at: OptionalField<String>,
    #[serde(default)]
    ended_at: OptionalField<String>,
    #[serde(default)]
    watch_cursor: OptionalValue,
    #[serde(default)]
    watch_terminal: OptionalValue,
}

/// Converts an exact `kiro/workflow/<kind>` lifecycle method, naming each of
/// the three possible outcomes in [`WorkflowFrameOutcome`].
pub(crate) fn to_notification(method: &str, params: &serde_json::Value) -> WorkflowFrameOutcome {
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
        _ => {
            // An unknown member of the recognized family is vendor drift — a
            // tenth lifecycle kind would otherwise vanish into the generic
            // unknown-extension debug! at default log level.
            if method.starts_with("kiro/workflow/") {
                tracing::warn!(
                    method,
                    "unrecognized workflow lifecycle method; not converted"
                );
            }
            return WorkflowFrameOutcome::NotWorkflow;
        }
    };

    match event {
        Ok(event) => WorkflowFrameOutcome::Converted(Box::new(event)),
        Err(error) => {
            tracing::warn!(
                method,
                field_path = error.field_path(),
                error_kind = error.error_kind().as_str(),
                error = %error,
                "malformed workflow notification"
            );
            WorkflowFrameOutcome::Dropped
        }
    }
}

/// Error from parsing a `_kiro/workflow/*` request reply.
///
/// Distinct from lifecycle-notification handling on purpose: a malformed
/// notification warns-and-drops (the stream continues), but a reply the
/// client explicitly asked for must surface its failure to the user — the
/// bridge folds this into a `Failed` command outcome, never silence.
#[derive(Debug, thiserror::Error)]
#[error("{message} (at {field_path})")]
pub(crate) struct WorkflowReplyError {
    field_path: String,
    message: String,
}

impl From<WorkflowAdapterError> for WorkflowReplyError {
    fn from(error: WorkflowAdapterError) -> Self {
        Self {
            field_path: error.field_path().to_owned(),
            message: error.to_string(),
        }
    }
}

/// Parses the reply of `_kiro/workflow/inspect` (`{workflowId, state}`) or
/// `_kiro/workflow/new` (`{workflowId, initialState}`) into the run's
/// snapshot.
///
/// The outer `workflowId` must match the snapshot's own — a mismatched
/// reply errors rather than silently trusting either id.
pub(crate) fn parse_state_reply(
    reply: &serde_json::Value,
) -> Result<WorkflowSnapshot, WorkflowReplyError> {
    let wire: WireStateReply = deserialize(reply)?;
    let snapshot = wire.state.try_into_domain()?;
    if snapshot.workflow_id().as_str() != wire.workflow_id {
        return Err(WorkflowAdapterError::ReplyWorkflowMismatch {
            outer: wire.workflow_id,
            inner: snapshot.workflow_id().as_str().to_owned(),
        }
        .into());
    }
    Ok(snapshot)
}

/// One `_kiro/workflow/list` entry. Timestamps are genuinely optional on the
/// wire: a never-invoked run has no `startedAt`, a non-terminal run has no
/// `endedAt` (live-observed on 2.16.2 and 2.18.0).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireRunListEntry {
    workflow_id: String,
    name: String,
    status: WireEnum<WorkflowRunStatus>,
    #[serde(default)]
    created_at: OptionalField<String>,
    #[serde(default)]
    updated_at: OptionalField<String>,
    #[serde(default)]
    started_at: OptionalField<String>,
    #[serde(default)]
    ended_at: OptionalField<String>,
    #[serde(default)]
    parent_session_id: OptionalField<String>,
}

impl WireRunListEntry {
    fn try_into_domain(self) -> Result<WorkflowRunSummary, WorkflowAdapterError> {
        Ok(WorkflowRunSummary {
            workflow_id: workflow_id(self.workflow_id, "workflowId")?,
            name: self.name,
            status: self.status.0,
            created_at: self.created_at.into_option(),
            updated_at: self.updated_at.into_option(),
            started_at: self.started_at.into_option(),
            ended_at: self.ended_at.into_option(),
            parent_session_id: self.parent_session_id.into_option().map(SessionId::new),
        })
    }
}

/// Parsed `_kiro/workflow/list` reply: the entries that parsed, plus one
/// error per skipped entry (each already warned here — callers may surface
/// the skip count but need not re-log).
pub(crate) struct WorkflowListing {
    pub(crate) runs: Vec<WorkflowRunSummary>,
    pub(crate) skipped: Vec<WorkflowReplyError>,
}

/// Parses `_kiro/workflow/list`. The outer `{runs: […]}` shape is required;
/// entries are tolerant per-entry — one malformed entry (unknown status
/// string, missing id) is warned and skipped without killing the listing.
pub(crate) fn parse_list_reply(
    reply: &serde_json::Value,
) -> Result<WorkflowListing, WorkflowReplyError> {
    #[derive(Deserialize)]
    struct WireListReply {
        runs: Vec<serde_json::Value>,
    }
    let wire: WireListReply = deserialize(reply)?;
    let mut runs = Vec::with_capacity(wire.runs.len());
    let mut skipped = Vec::new();
    for (index, entry) in wire.runs.iter().enumerate() {
        match deserialize::<WireRunListEntry>(entry).and_then(WireRunListEntry::try_into_domain) {
            Ok(summary) => runs.push(summary),
            Err(error) => {
                tracing::warn!(index, error = %error, "skipping malformed workflow list entry");
                skipped.push(error.into());
            }
        }
    }
    Ok(WorkflowListing { runs, skipped })
}

/// One `listRecipes` entry. `builtIn`, `inputs`, and `plan` are deliberately
/// ignored — the control plane needs identity and provenance only; recipe
/// internals belong to the authoring surface (`kiro-workflow-authoring`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireRecipe {
    name: String,
    #[serde(default)]
    description: OptionalField<String>,
    #[serde(default)]
    source: OptionalField<String>,
}

/// Parsed `listRecipes` reply, tolerant per-entry like [`parse_list_reply`].
pub(crate) struct WorkflowRecipeListing {
    pub(crate) recipes: Vec<WorkflowRecipe>,
    pub(crate) skipped: Vec<WorkflowReplyError>,
}

/// Parses `_kiro/workflow/listRecipes` (`{recipes: […]}`).
pub(crate) fn parse_recipes_reply(
    reply: &serde_json::Value,
) -> Result<WorkflowRecipeListing, WorkflowReplyError> {
    #[derive(Deserialize)]
    struct WireRecipesReply {
        recipes: Vec<serde_json::Value>,
    }
    let wire: WireRecipesReply = deserialize(reply)?;
    let mut recipes = Vec::with_capacity(wire.recipes.len());
    let mut skipped = Vec::new();
    for (index, entry) in wire.recipes.iter().enumerate() {
        match deserialize::<WireRecipe>(entry) {
            Ok(recipe) => recipes.push(WorkflowRecipe {
                name: recipe.name,
                description: recipe.description.into_option(),
                source: recipe.source.into_option(),
            }),
            Err(error) => {
                tracing::warn!(index, error = %error, "skipping malformed workflow recipe entry");
                skipped.push(error.into());
            }
        }
    }
    Ok(WorkflowRecipeListing { recipes, skipped })
}

/// Parsed `_kiro/workflow/cancel` reply — `{ok, previousStatus}`, a
/// deliberately different shape from invoke/resume's `{workflowId, status}`.
pub(crate) struct WorkflowCancelReply {
    pub(crate) ok: bool,
    pub(crate) previous_status: Option<WorkflowRunStatus>,
}

/// Parses `_kiro/workflow/cancel`. An unknown `previousStatus` string is
/// warned and reported as unreported rather than failing the whole reply.
pub(crate) fn parse_cancel_reply(
    reply: &serde_json::Value,
) -> Result<WorkflowCancelReply, WorkflowReplyError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct WireCancelReply {
        ok: bool,
        #[serde(default)]
        previous_status: OptionalField<String>,
    }
    let wire: WireCancelReply = deserialize(reply)?;
    Ok(WorkflowCancelReply {
        ok: wire.ok,
        previous_status: lenient_run_status(wire.previous_status.into_option(), "cancel reply"),
    })
}

/// Parses an `_kiro/workflow/invoke` or `resume` reply (`{workflowId,
/// status}`). An unknown status string is warned and reported as unreported.
pub(crate) fn parse_run_status_reply(
    reply: &serde_json::Value,
) -> Result<(WorkflowId, Option<WorkflowRunStatus>), WorkflowReplyError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct WireRunStatusReply {
        workflow_id: String,
        #[serde(default)]
        status: OptionalField<String>,
    }
    let wire: WireRunStatusReply = deserialize(reply)?;
    let id = workflow_id(wire.workflow_id, "workflowId")?;
    Ok((
        id,
        lenient_run_status(wire.status.into_option(), "run-status reply"),
    ))
}

/// Maps a raw status string to the known vocabulary, warning (never
/// failing) on a value outside it — replies stay useful across vendor
/// status additions.
fn lenient_run_status(raw: Option<String>, context: &'static str) -> Option<WorkflowRunStatus> {
    let raw = raw?;
    match WorkflowRunStatus::try_from(raw.as_str()) {
        Ok(status) => Some(status),
        Err(error) => {
            tracing::warn!(%error, context, "unknown workflow run status in reply");
            None
        }
    }
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
    let mut event = WorkflowNodeStarted::new(
        workflow_id,
        node_id(wire.node_id)?,
        node_path,
        wire.node_type.0,
        details,
    );
    if let Some(parent_session_id) = wire.parent_session_id.into_option().map(SessionId::new) {
        event = event.with_parent_session_id(parent_session_id);
    }
    Ok(WorkflowEvent::NodeStarted(event))
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
        details = details.with_completion_signal(completion_signal.0);
    }
    if let Some(source) = wire.completion_signal_source.into_option() {
        details = details.with_completion_signal_source(source.0);
    }
    let mut event = WorkflowNodeCompleted::new(
        workflow_id,
        node_id(wire.node_id)?,
        node_path,
        wire.status.0,
        details,
    );
    if let Some(parent_session_id) = wire.parent_session_id.into_option().map(SessionId::new) {
        event = event.with_parent_session_id(parent_session_id);
    }
    Ok(WorkflowEvent::NodeCompleted(event))
}

fn parse_node_paused(params: &serde_json::Value) -> Result<WorkflowEvent, WorkflowAdapterError> {
    let wire: WireNodePaused = deserialize(params)?;
    let workflow_id = workflow_id(wire.workflow_id, "workflowId")?;
    let node_path = WorkflowNodePath::try_new(&workflow_id, wire.node_path)?;
    let mut event =
        WorkflowNodePaused::new(workflow_id, node_id(wire.node_id)?, node_path, wire.reason);
    if let Some(parent_session_id) = wire.parent_session_id.into_option().map(SessionId::new) {
        event = event.with_parent_session_id(parent_session_id);
    }
    Ok(WorkflowEvent::NodePaused(event))
}

fn parse_loop_iteration(params: &serde_json::Value) -> Result<WorkflowEvent, WorkflowAdapterError> {
    let wire: WireLoopIteration = deserialize(params)?;
    let mut event = WorkflowLoopIteration::new(
        workflow_id(wire.workflow_id, "workflowId")?,
        loop_id(wire.loop_id)?,
        wire.iteration,
        wire.stop_condition_met,
    );
    if let Some(parent_session_id) = wire.parent_session_id.into_option().map(SessionId::new) {
        event = event.with_parent_session_id(parent_session_id);
    }
    Ok(WorkflowEvent::LoopIteration(event))
}

fn parse_watch_poll(params: &serde_json::Value) -> Result<WorkflowEvent, WorkflowAdapterError> {
    let wire: WireWatchPoll = deserialize(params)?;
    let workflow_id = workflow_id(wire.workflow_id, "workflowId")?;
    let node_path = WorkflowNodePath::try_new(&workflow_id, wire.node_path)?;
    let mut event = WorkflowWatchPoll::new(
        workflow_id,
        node_id(wire.node_id)?,
        node_path,
        wire.outcome.0,
        wire.at,
    );
    if let Some(parent_session_id) = wire.parent_session_id.into_option().map(SessionId::new) {
        event = event.with_parent_session_id(parent_session_id);
    }
    Ok(WorkflowEvent::WatchPoll(event))
}

fn parse_paused(params: &serde_json::Value) -> Result<WorkflowEvent, WorkflowAdapterError> {
    let wire: WirePaused = deserialize(params)?;
    let mut event = WorkflowPaused::new(
        workflow_id(wire.workflow_id, "workflowId")?,
        wire.pause_reason,
    );
    if let Some(parent_session_id) = wire.parent_session_id.into_option().map(SessionId::new) {
        event = event.with_parent_session_id(parent_session_id);
    }
    Ok(WorkflowEvent::Paused(event))
}

fn parse_steps_queued(params: &serde_json::Value) -> Result<WorkflowEvent, WorkflowAdapterError> {
    let wire: WireStepsQueued = deserialize(params)?;
    let pending_steps = wire
        .pending_steps
        .into_iter()
        .map(WireNodeDescriptor::try_into_domain)
        .collect::<Result<Vec<_>, _>>()?;
    let resolution = wire.resolution.into_option().map(|resolution| {
        WorkflowQueueResolution::new(resolution.outcome.0, resolution.reason.into_option())
    });
    let mut event = WorkflowStepsQueued::new(
        workflow_id(wire.workflow_id, "workflowId")?,
        pending_steps,
        resolution,
    );
    if let Some(parent_session_id) = wire.parent_session_id.into_option().map(SessionId::new) {
        event = event.with_parent_session_id(parent_session_id);
    }
    Ok(WorkflowEvent::StepsQueued(event))
}

fn parse_run_completed(params: &serde_json::Value) -> Result<WorkflowEvent, WorkflowAdapterError> {
    let wire: WireRunCompleted = deserialize(params)?;
    let workflow_id = workflow_id(wire.workflow_id, "workflowId")?;
    let status = wire.status.0;
    let final_state = wire.final_state.try_into_domain()?;
    if final_state.workflow_id() != &workflow_id {
        return Err(WorkflowAdapterError::SnapshotWorkflowMismatch {
            outer: workflow_id.as_str().to_owned(),
            final_id: final_state.workflow_id().as_str().to_owned(),
        });
    }
    let mut event = WorkflowRunCompleted::new(workflow_id, status, final_state)?;
    if let Some(parent_session_id) = wire.parent_session_id.into_option().map(SessionId::new) {
        event = event.with_parent_session_id(parent_session_id);
    }
    Ok(WorkflowEvent::RunCompleted(event))
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

/// Serde's `de::Error` erases error structure into a message string before it
/// reaches this layer, so kind classification can only prefix-match the known
/// message shapes: serde's own `missing field` / `unknown variant` /
/// `invalid type` prefixes plus [`WorkflowEnumParseError`]'s
/// `unknown workflow …` Display. Anything unrecognized degrades to
/// `invalid_value`, and the raw message is always logged beside the kind.
fn classify_serde_error(message: &str) -> WorkflowErrorKind {
    if message.starts_with("missing field `") {
        WorkflowErrorKind::MissingRequired
    } else if message.starts_with("unknown variant `") || message.starts_with("unknown workflow ") {
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
                model_id,
                effort_level,
            } => Ok(WorkflowNodeDescriptor::step(
                node_id(raw_node_id)?,
                agent_name,
                model_id.into_option(),
                effort_level.into_option(),
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
                on_max_iterations.0,
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
                agent_name,
            } => Ok(WorkflowNodeDescriptor::watch(
                node_id(raw_node_id)?,
                agent_name,
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
            self.status.0,
            WorkflowSnapshotData::new(self.inputs, self.artifacts, self.captured_outputs),
            self.root.try_into_domain()?,
            metadata,
        ))
    }
}

impl WireNodeSnapshot {
    fn try_into_domain(self) -> Result<WorkflowNodeSnapshot, WorkflowAdapterError> {
        let descriptor = match self.node_type.0 {
            WorkflowNodeType::Step => WorkflowNodeDescriptor::snapshot_step(
                node_id(self.node_id)?,
                self.agent_name.into_option(),
                self.model_id.into_option(),
                self.effort_level.into_option(),
            ),
            WorkflowNodeType::Sequence => {
                WorkflowNodeDescriptor::sequence(node_id(self.node_id)?, Vec::new())
            }
            WorkflowNodeType::Repeat => WorkflowNodeDescriptor::snapshot_repeat(
                node_id(self.node_id)?,
                self.max_iterations.into_option(),
                self.on_max_iterations.into_option().map(|value| value.0),
                self.stop_condition.into_option(),
                self.stop_when.into_option(),
            ),
            WorkflowNodeType::Parallel => {
                WorkflowNodeDescriptor::parallel(node_id(self.node_id)?, Vec::new())
            }
            WorkflowNodeType::Watch => WorkflowNodeDescriptor::snapshot_watch(
                node_id(self.node_id)?,
                self.agent_name.into_option(),
            ),
        };
        let children = match self.children.into_option() {
            Some(children) => children
                .into_iter()
                .map(Self::try_into_domain)
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };
        let mut snapshot = WorkflowNodeSnapshot::new(descriptor, self.status.0, children);
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
            snapshot = snapshot.with_completion_signal(completion_signal.0);
        }
        if let Some(source) = self.completion_signal_source.into_option() {
            snapshot = snapshot.with_completion_signal_source(source.0);
        }
        if let Some(started_at) = self.started_at.into_option() {
            snapshot = snapshot.with_started_at(started_at);
        }
        if let Some(ended_at) = self.ended_at.into_option() {
            snapshot = snapshot.with_ended_at(ended_at);
        }
        if let Some(watch_cursor) = self.watch_cursor.into_option() {
            snapshot = snapshot.with_watch_cursor(watch_cursor);
        }
        if let Some(watch_terminal) = self.watch_terminal.into_option() {
            snapshot = snapshot.with_watch_terminal(watch_terminal);
        }
        Ok(snapshot)
    }
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::test_support::{CaptureWriter, must_succeed};
    use crate::workflow::{WorkflowNodeState, WorkflowRun, WorkflowTracker};

    const FAILED_CAPTURE: &str =
        include_str!("../../../../tests/fixtures/kas/workflow/terminal-failed-2.16.2.jsonl");
    const ABORTED_CAPTURE: &str =
        include_str!("../../../../tests/fixtures/kas/workflow/terminal-aborted-2.16.2.jsonl");
    const SYNTHETIC_REPLAY: &str =
        include_str!("../../../../tests/fixtures/kas/workflow/oracle-replay-events.jsonl");
    const REPEAT_WATCH_CAPTURE: &str =
        include_str!("../../../../tests/fixtures/kas/workflow/kas-repeat-watch-2.16.0.jsonl");
    const CUSTOM_DAG_CAPTURE: &str =
        include_str!("../../../../tests/fixtures/kas/workflow/kas-custom-dag-2.16.0.jsonl");
    const CSIG_2160_NEUTRAL_CAPTURE: &str =
        include_str!("../../../../tests/fixtures/kas/workflow/kas-csig-2.16.0-neutral.jsonl");
    const CSIG_2162_NEUTRAL_CAPTURE: &str =
        include_str!("../../../../tests/fixtures/kas/workflow/kas-csig-2.16.2-neutral.jsonl");
    const CSIG_2162_EXPLICIT_CAPTURE: &str =
        include_str!("../../../../tests/fixtures/kas/workflow/kas-csig-2.16.2-explicit.jsonl");
    const LATE_PAUSE_CAPTURE: &str = include_str!(
        "../../../../tests/fixtures/kas/workflow/pause-late-summary-2.18.0-source-derived.jsonl"
    );
    const REPLAY_SOURCES: [(&str, &str); 9] = [
        ("oracle-replay-events.jsonl", SYNTHETIC_REPLAY),
        ("terminal-failed-2.16.2.jsonl", FAILED_CAPTURE),
        ("terminal-aborted-2.16.2.jsonl", ABORTED_CAPTURE),
        ("kas-repeat-watch-2.16.0.jsonl", REPEAT_WATCH_CAPTURE),
        ("kas-custom-dag-2.16.0.jsonl", CUSTOM_DAG_CAPTURE),
        ("kas-csig-2.16.0-neutral.jsonl", CSIG_2160_NEUTRAL_CAPTURE),
        ("kas-csig-2.16.2-neutral.jsonl", CSIG_2162_NEUTRAL_CAPTURE),
        ("kas-csig-2.16.2-explicit.jsonl", CSIG_2162_EXPLICIT_CAPTURE),
        (
            "pause-late-summary-2.18.0-source-derived.jsonl",
            LATE_PAUSE_CAPTURE,
        ),
    ];

    fn event(result: WorkflowFrameOutcome, context: &str) -> WorkflowEvent {
        match result {
            WorkflowFrameOutcome::Converted(event) => *event,
            other => panic!("{context}: expected workflow notification, got {other:?}"),
        }
    }

    fn capture_rejection(
        method: &str,
        params: &serde_json::Value,
    ) -> (WorkflowFrameOutcome, serde_json::Value) {
        let _capture_lock = crate::test_support::tracing_capture_lock();
        let capture = CaptureWriter::default();
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_current_span(false)
            .with_span_list(false)
            .with_writer(capture.clone())
            .finish();
        let result =
            tracing::subscriber::with_default(subscriber, || to_notification(method, params));
        let log = must_succeed(
            serde_json::from_slice(&capture.captured()),
            "workflow warning must be one JSON event",
        );
        (result, log)
    }

    /// Capture lines come in two shapes: raw JSON-RPC frames, and proxy-log
    /// envelopes that nest the frame under `parsed` (whose outer object also
    /// carries its own `method` key). Consumers always want the frame itself,
    /// so the envelope is unwrapped here.
    fn capture_frames(source: &str) -> Vec<serde_json::Value> {
        source
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                let mut frame: serde_json::Value =
                    must_succeed(serde_json::from_str(line), "capture line is valid JSON");
                match frame.get("parsed") {
                    Some(parsed) if parsed.is_object() => frame["parsed"].take(),
                    _ => frame,
                }
            })
            .collect()
    }

    fn capture_params(source: &str, expected_status: &str) -> serde_json::Value {
        let mut matched = None;
        for envelope in capture_frames(source) {
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
            ("inputs".to_owned(), run.inputs().clone()),
        ]);
        insert_string(
            &mut run_projection,
            "status",
            run.status().map(WorkflowRunStatus::as_str),
        );
        insert_value(&mut run_projection, "artifacts", run.artifacts());
        insert_value(
            &mut run_projection,
            "capturedOutputs",
            run.captured_outputs(),
        );
        insert_string(&mut run_projection, "createdAt", run.created_at());
        insert_u32(&mut run_projection, "planRevision", run.plan_revision());
        insert_string(
            &mut run_projection,
            "parentSessionId",
            run.parent_session_id().map(SessionId::as_str),
        );
        insert_string(&mut run_projection, "workspacePath", run.workspace_path());
        if let Some(pending_steps) = run.pending_steps() {
            run_projection.insert(
                "pendingSteps".to_owned(),
                serde_json::Value::Array(
                    pending_steps
                        .iter()
                        .map(workflow_descriptor_projection)
                        .collect(),
                ),
            );
        }
        if let Some(resolution) = run.queue_resolution() {
            let mut projected = serde_json::Map::from_iter([(
                "outcome".to_owned(),
                serde_json::Value::String(resolution.outcome().as_str().to_owned()),
            )]);
            insert_string(&mut projected, "reason", resolution.reason());
            run_projection.insert(
                "queueResolution".to_owned(),
                serde_json::Value::Object(projected),
            );
        }
        insert_string(
            &mut run_projection,
            "runPauseReason",
            run.run_pause_reason(),
        );
        if let Some(opening_plan) = run.opening_plan() {
            run_projection.insert(
                "descriptor".to_owned(),
                serde_json::Value::Array(
                    opening_plan
                        .iter()
                        .map(workflow_descriptor_projection)
                        .collect(),
                ),
            );
        } else if let Some(snapshot_plan) = run.snapshot_plan() {
            run_projection.insert(
                "descriptor".to_owned(),
                workflow_descriptor_projection(snapshot_plan),
            );
        }

        let mut node_entries = run.nodes().collect::<Vec<_>>();
        node_entries.sort_by(|(left, _), (right, _)| left.segments().cmp(right.segments()));
        let nodes = node_entries
            .into_iter()
            .map(|(path, node)| {
                serde_json::json!({
                    "path": path.segments(),
                    "data": workflow_node_projection(node),
                })
            })
            .collect::<Vec<_>>();
        serde_json::json!({"run": run_projection, "nodes": nodes})
    }

    fn workflow_node_projection(node: &WorkflowNodeState) -> serde_json::Value {
        let mut data = serde_json::Map::from_iter([
            (
                "nodeId".to_owned(),
                serde_json::Value::String(node.node_id().as_str().to_owned()),
            ),
            (
                "type".to_owned(),
                serde_json::Value::String(node.node_type().as_str().to_owned()),
            ),
        ]);
        insert_string(
            &mut data,
            "status",
            node.status().map(WorkflowNodeStatus::as_str),
        );
        let agent_name = if node.node_type() == WorkflowNodeType::Watch {
            node.descriptor()
                .and_then(WorkflowNodeDescriptor::handler_name)
                .or_else(|| node.agent_name())
        } else {
            node.agent_name()
        };
        insert_string(&mut data, "agentName", agent_name);
        if let Some(descriptor) = node.descriptor() {
            insert_string(&mut data, "modelId", descriptor.model_id());
            insert_string(&mut data, "effortLevel", descriptor.effort_level());
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
        }
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
        insert_value(&mut data, "watchCursor", node.watch_cursor());
        insert_value(&mut data, "watchTerminal", node.watch_terminal());
        insert_string(&mut data, "prompt", node.prompt());
        insert_string(&mut data, "nodePauseReason", node.node_pause_reason());
        if let Some((iteration, stop_condition_met)) = node.latest_loop_iteration() {
            data.insert(
                "latestLoopIteration".to_owned(),
                serde_json::json!({
                    "iteration": iteration,
                    "stopConditionMet": stop_condition_met,
                }),
            );
        }
        if let Some((outcome, at)) = node.latest_watch_poll() {
            data.insert(
                "latestWatchPoll".to_owned(),
                serde_json::json!({
                    "outcome": outcome.as_str(),
                    "at": at,
                }),
            );
        }
        serde_json::Value::Object(data)
    }

    fn workflow_descriptor_projection(descriptor: &WorkflowNodeDescriptor) -> serde_json::Value {
        let mut data = serde_json::Map::from_iter([
            (
                "nodeId".to_owned(),
                serde_json::Value::String(descriptor.node_id().as_str().to_owned()),
            ),
            (
                "type".to_owned(),
                serde_json::Value::String(descriptor.node_type().as_str().to_owned()),
            ),
        ]);
        let agent_name = if descriptor.node_type() == WorkflowNodeType::Watch {
            descriptor.handler_name()
        } else {
            descriptor.agent_name()
        };
        insert_string(&mut data, "agentName", agent_name);
        insert_string(&mut data, "modelId", descriptor.model_id());
        insert_string(&mut data, "effortLevel", descriptor.effort_level());
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
        match descriptor.node_type() {
            WorkflowNodeType::Sequence | WorkflowNodeType::Repeat => {
                data.insert(
                    "steps".to_owned(),
                    serde_json::Value::Array(
                        descriptor
                            .children()
                            .iter()
                            .map(workflow_descriptor_projection)
                            .collect(),
                    ),
                );
            }
            WorkflowNodeType::Parallel => {
                data.insert(
                    "branches".to_owned(),
                    serde_json::Value::Array(
                        descriptor
                            .children()
                            .iter()
                            .map(workflow_descriptor_projection)
                            .collect(),
                    ),
                );
            }
            WorkflowNodeType::Step | WorkflowNodeType::Watch => {}
        }
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
                    "modelId": "",
                    "effortLevel": "high",
                    "unknown": true
                },
                {
                    "nodeId": "sequence",
                    "type": "sequence",
                    "steps": [{"nodeId": "nested", "type": "watch", "agentName": "files"}]
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
                {"nodeId": "watch", "type": "watch", "agentName": "handler"}
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
        assert_eq!(nodes[0].model_id(), Some(""));
        assert_eq!(nodes[0].effort_level(), Some("high"));
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
            for envelope in capture_frames(source) {
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
                        "../../../../tests/fixtures/kas/workflow/oracle-manifest.json"
                    )),
                    "oracle manifest is valid JSON",
                );
                manifest["terminal_counts"].clone()
            }
        };
        assert_eq!(actual, expected);
    }

    fn workflow_tracker_projection(tracker: &WorkflowTracker) -> serde_json::Value {
        let mut runs = tracker.iter().collect::<Vec<_>>();
        runs.sort_by(|(left, _), (right, _)| left.as_str().cmp(right.as_str()));
        serde_json::Value::Array(
            runs.into_iter()
                .map(|(workflow_id, run)| {
                    let serde_json::Value::Object(mut projection) =
                        workflow_projection(workflow_id, run)
                    else {
                        unreachable!("workflow projection is always an object");
                    };
                    projection.insert(
                        "workflowId".to_owned(),
                        serde_json::Value::String(workflow_id.as_str().to_owned()),
                    );
                    serde_json::Value::Object(projection)
                })
                .collect(),
        )
    }

    fn replay_projection(source: &str, passes: usize) -> serde_json::Value {
        let frames = capture_frames(source);
        let mut tracker = WorkflowTracker::new();
        let mut checkpoints = serde_json::Map::new();
        for _ in 0..passes {
            for envelope in &frames {
                let Some(method) = envelope.get("method").and_then(serde_json::Value::as_str)
                else {
                    continue;
                };
                let method = match method.strip_prefix('_') {
                    Some(normalized) => normalized,
                    None => method,
                };
                if let WorkflowFrameOutcome::Converted(event) =
                    to_notification(method, &envelope["params"])
                {
                    // Fold rejections the way the App does (D2/D36): a state
                    // error leaves the tracker atomically unchanged and the
                    // stream continues — a fixture may legitimately carry a
                    // frame the tracker rejects (2026-08-10 review, SP4's
                    // duplicate-canonical-path differential fence).
                    if let Err(error) = tracker.apply_event(*event) {
                        tracing::warn!(%error, "replay event rejected; state preserved");
                    }
                }
                if let Some(checkpoint) = envelope
                    .get("checkpoint")
                    .and_then(serde_json::Value::as_str)
                {
                    checkpoints
                        .insert(checkpoint.to_owned(), workflow_tracker_projection(&tracker));
                }
            }
        }
        serde_json::json!({
            "checkpoints": checkpoints,
            "final": workflow_tracker_projection(&tracker),
        })
    }

    #[test]
    fn workflow_capture_replay_matches_independent_folder() {
        let actual = serde_json::Value::Array(
            REPLAY_SOURCES
                .iter()
                .map(|(name, source)| {
                    let one = replay_projection(source, 1);
                    let one_equals_two = one == replay_projection(source, 2);
                    serde_json::json!({
                        "source": name,
                        "expected": one,
                        "oneEqualsTwo": one_equals_two,
                    })
                })
                .collect(),
        );
        let expected_text = match std::env::var_os("CYRIL_WORKFLOW_ORACLE_EXPECTED") {
            Some(path) => must_succeed(
                std::fs::read_to_string(path),
                "replay oracle output is readable",
            ),
            None => {
                include_str!("../../../../tests/fixtures/kas/workflow/oracle-replay-expected.json")
                    .to_owned()
            }
        };
        let expected = must_succeed(
            serde_json::from_str::<serde_json::Value>(&expected_text),
            "replay oracle output is valid JSON",
        );
        assert_eq!(actual, expected);
    }

    #[test]
    fn workflow_capture_replay_is_state_idempotent() {
        for (name, source) in REPLAY_SOURCES {
            assert_eq!(
                replay_projection(source, 1),
                replay_projection(source, 2),
                "{name} replay must be idempotent"
            );
        }
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
            None => include_str!(
                "../../../../tests/fixtures/kas/workflow/oracle-snapshot-expected.json"
            )
            .to_owned(),
        };
        let expected = must_succeed(
            serde_json::from_str::<serde_json::Value>(&expected_text),
            "snapshot oracle output is valid JSON",
        );
        assert_eq!(actual, expected);
    }

    /// Depth fences (2026-08-09 review, test finding 3). (a) Near the
    /// serde_json 128-depth parse cap, deeply nested descriptor and snapshot
    /// trees convert cleanly — the recursive deserialize → domain → drop
    /// chain must not overflow. (b) A depth bomb never reaches this layer as
    /// nesting: `serde_json::from_str` fails at the cap and the client
    /// substitutes `Value::Null` params — so whole-params `Null` and scalar
    /// params on a workflow method must warn-and-drop, not panic.
    #[test]
    fn workflow_frames_survive_depth_extremes() {
        let mut descriptor = serde_json::json!({
            "nodeId": "leaf", "type": "step", "agentName": "agent"
        });
        let mut snapshot_node = serde_json::json!({
            "nodeId": "leaf", "type": "step", "status": "completed"
        });
        for depth in 0..119 {
            descriptor = serde_json::json!({
                "nodeId": format!("seq-{depth}"), "type": "sequence", "steps": [descriptor]
            });
            snapshot_node = serde_json::json!({
                "nodeId": format!("seq-{depth}"), "type": "sequence",
                "status": "completed", "children": [snapshot_node]
            });
        }
        let opening = serde_json::json!({
            "workflowId": "deep",
            "workflowName": "deep-recipe",
            "inputs": {},
            "nodeTree": [descriptor]
        });
        let WorkflowEvent::RunStarted(started) = event(
            to_notification("kiro/workflow/run_start", &opening),
            "near-cap descriptor tree converts",
        ) else {
            panic!("deep opening converted to the wrong event");
        };
        let mut declared = started.node_tree();
        let mut declared_depth = 1;
        while let Some(child) = declared.first().map(WorkflowNodeDescriptor::children) {
            if child.is_empty() {
                break;
            }
            declared = child;
            declared_depth += 1;
        }
        assert_eq!(declared_depth, 120, "no descriptor level may be lost");

        let completion = serde_json::json!({
            "workflowId": "deep",
            "status": "completed",
            "finalState": {
                "workflowId": "deep", "workflowName": "deep-recipe",
                "status": "completed", "inputs": {}, "artifacts": {},
                "capturedOutputs": {}, "createdAt": "created", "planRevision": 1,
                "root": snapshot_node
            }
        });
        let WorkflowEvent::RunCompleted(completed) = event(
            to_notification("kiro/workflow/run_complete", &completion),
            "near-cap snapshot tree converts",
        ) else {
            panic!("deep completion converted to the wrong event");
        };
        let mut tracker = WorkflowTracker::new();
        must_succeed(
            tracker.apply_snapshot(completed.final_state().clone()),
            "near-cap snapshot canonicalizes",
        );

        for params in [serde_json::Value::Null, serde_json::json!("truncated")] {
            let (result, log) = capture_rejection("kiro/workflow/run_start", &params);
            assert!(
                matches!(result, WorkflowFrameOutcome::Dropped),
                "depth-bomb artifact params must warn and drop, got {result:?}"
            );
            assert_eq!(log["level"], "WARN");
        }
    }

    /// REGRESSION FENCE (2026-08-09 review, finding S1): the step descriptor
    /// wire keys are `modelId`/`effortLevel`. The live recipe catalog inside
    /// the committed aborted capture is the one artifact whose bytes were
    /// produced by the shipped engine, not by this codebase's own fixtures —
    /// a converter that reads any other spelling parses these pinned fields
    /// as absent and this test fails.
    #[test]
    fn descriptor_wire_spelling_matches_live_recipe_catalog() {
        let plan = capture_frames(ABORTED_CAPTURE)
            .into_iter()
            .find_map(|envelope| {
                let recipes = envelope.get("result")?.get("recipes")?.as_array()?;
                recipes
                    .iter()
                    .find(|recipe| {
                        recipe.get("name").and_then(serde_json::Value::as_str)
                            == Some("semantic-review-multi-model")
                    })
                    .and_then(|recipe| recipe.get("plan").cloned())
            });
        let plan = must_succeed(
            plan.ok_or("aborted capture must hold the multi-model recipe catalog"),
            "live recipe plan",
        );
        let params = serde_json::json!({
            "workflowId": "wf-live-catalog",
            "workflowName": "semantic-review-multi-model",
            "inputs": {},
            "nodeTree": plan,
        });
        let WorkflowEvent::RunStarted(started) = event(
            to_notification("kiro/workflow/run_start", &params),
            "live recipe plan parses as a run opening",
        ) else {
            panic!("live recipe plan converted to the wrong event");
        };

        fn collect_steps<'tree>(
            descriptor: &'tree WorkflowNodeDescriptor,
            steps: &mut Vec<&'tree WorkflowNodeDescriptor>,
        ) {
            if descriptor.node_type() == WorkflowNodeType::Step {
                steps.push(descriptor);
            }
            for child in descriptor.children() {
                collect_steps(child, steps);
            }
        }
        let mut steps = Vec::new();
        for descriptor in started.node_tree() {
            collect_steps(descriptor, &mut steps);
        }
        let observed: Vec<(&str, Option<&str>, Option<&str>)> = steps
            .iter()
            .map(|step| {
                (
                    step.node_id().as_str(),
                    step.model_id(),
                    step.effort_level(),
                )
            })
            .collect();
        assert_eq!(
            observed,
            [
                ("setup", None, Some("low")),
                ("review-fable", Some("claude-fable-5"), Some("xhigh")),
                ("review-gpt", Some("gpt-5.6-sol"), Some("xhigh")),
                ("aggregate", Some("claude-fable-5"), Some("xhigh")),
            ],
            "pinned model/effort fields must survive conversion byte-exactly"
        );
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
            WorkflowFrameOutcome::Dropped
        ));

        let wrong_type = serde_json::json!({
            "workflowId": "workflow",
            "status": 1,
            "finalState": {}
        });
        assert!(matches!(
            to_notification("kiro/workflow/run_complete", &wrong_type),
            WorkflowFrameOutcome::Dropped
        ));

        let mismatch = serde_json::json!({
            "workflowId": "outer",
            "status": "completed",
            "finalState": completed_snapshot("inner", "completed")
        });
        assert!(matches!(
            to_notification("kiro/workflow/run_complete", &mismatch),
            WorkflowFrameOutcome::Dropped
        ));

        let status_mismatch = serde_json::json!({
            "workflowId": "workflow",
            "status": "failed",
            "finalState": completed_snapshot("workflow", "completed")
        });
        assert!(matches!(
            to_notification("kiro/workflow/run_complete", &status_mismatch),
            WorkflowFrameOutcome::Dropped
        ));

        assert!(matches!(
            to_notification("kiro/workflow/run_started", &valid_opening),
            WorkflowFrameOutcome::NotWorkflow
        ));
        assert!(matches!(
            to_notification("_kiro/workflow/run_start", &valid_opening),
            WorkflowFrameOutcome::NotWorkflow
        ));
        assert!(matches!(
            to_notification("kiro/workflow/run_start", &valid_opening),
            WorkflowFrameOutcome::Converted(_)
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
        // CI-safe ceilings: these are complexity fences (a quadratic blowup
        // overshoots 5 s at this scale), not latency contracts — tight
        // millisecond budgets flake on loaded CI runners.
        assert!(
            elapsed <= Duration::from_secs(5),
            "1 MiB/256-node/depth-10 conversion exceeded 5 s: {elapsed:?}"
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
                WorkflowFrameOutcome::Converted(_)
            ));
        }
        let batch_elapsed = started.elapsed();
        assert!(
            batch_elapsed <= Duration::from_secs(5),
            "10,000 minimal node frames exceeded 5 s: {batch_elapsed:?}"
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
            large_elapsed <= Duration::from_secs(5),
            "64 KiB node frame exceeded 5 s: {large_elapsed:?}"
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
            WorkflowFrameOutcome::Dropped
        ));
        assert!(matches!(
            to_notification("kiro/workflow/node_start", &wrong_root),
            WorkflowFrameOutcome::Dropped
        ));
        assert!(matches!(
            to_notification("kiro/workflow/node_start", &minimal),
            WorkflowFrameOutcome::Converted(_)
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
                matches!(
                    to_notification(method, &params),
                    WorkflowFrameOutcome::Dropped
                ),
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
            WorkflowFrameOutcome::Converted(_)
        ));
        assert!(matches!(
            to_notification("kiro/workflow/node_paused", &paused),
            WorkflowFrameOutcome::Converted(_)
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
                "agentName": "files"
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
    fn pause_frames_tolerate_attribution_extras() {
        let mut converted = 0;
        for envelope in capture_frames(LATE_PAUSE_CAPTURE) {
            let Some(method) = envelope.get("method").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let params = &envelope["params"];
            let attributed_pause = method == "_kiro/workflow/paused"
                || (method == "_kiro/workflow/run_complete"
                    && params.get("status").and_then(serde_json::Value::as_str) == Some("paused"));
            if !attributed_pause {
                continue;
            }
            assert_eq!(params["initiator"], "user");
            assert_eq!(params["initiatorReason"], "operator requested pause");

            let normalized = method.strip_prefix('_').unwrap_or(method);
            match event(
                to_notification(normalized, params),
                "attributed pause frame",
            ) {
                WorkflowEvent::Paused(paused) => {
                    assert_eq!(paused.pause_reason(), "operator");
                }
                WorkflowEvent::RunCompleted(completed) => {
                    assert_eq!(completed.status(), WorkflowCompletionStatus::Paused);
                }
                other => panic!("unexpected attributed pause event: {other:?}"),
            }
            converted += 1;
        }
        assert_eq!(converted, 2);
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
                WorkflowFrameOutcome::Converted(_)
            ));
        }
        let fixed_elapsed = started.elapsed();
        assert!(
            fixed_elapsed <= Duration::from_secs(5),
            "100,000 minimal progress frames exceeded 5 s: {fixed_elapsed:?}"
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
            queue_elapsed <= Duration::from_secs(5),
            "1 MiB/256-step/depth-10 queue conversion exceeded 5 s: {queue_elapsed:?}"
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
                matches!(
                    to_notification(method, &params),
                    WorkflowFrameOutcome::Dropped
                ),
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
            WorkflowFrameOutcome::Converted(_)
        ));
    }

    struct MalformedCase {
        id: String,
        method: String,
        params: serde_json::Value,
        field_path: String,
        error_kind: &'static str,
    }

    /// Wire shape of one snapshot-node field, for generating its malformed
    /// rows. Consumes the manifest's `snapshot_node_fields` oracle — an
    /// unclassified name panics so a manifest addition forces new rows.
    enum SnapshotFieldShape {
        RequiredString,
        RequiredEnum,
        OptionalString,
        OptionalU32,
        OptionalEnum,
        OptionalChildren,
        Opaque,
    }

    fn snapshot_field_shape(field: &str) -> SnapshotFieldShape {
        match field {
            "nodeId" => SnapshotFieldShape::RequiredString,
            "type" | "status" => SnapshotFieldShape::RequiredEnum,
            "agentName" | "modelId" | "effortLevel" | "sessionId" | "failureReason"
            | "branchId" | "startedAt" | "endedAt" => SnapshotFieldShape::OptionalString,
            "maxIterations" | "iteration" => SnapshotFieldShape::OptionalU32,
            "onMaxIterations" | "completionSignal" | "completionSignalSource" => {
                SnapshotFieldShape::OptionalEnum
            }
            "children" => SnapshotFieldShape::OptionalChildren,
            "stopCondition" | "stopWhen" | "artifacts" | "capturedOutput" | "watchCursor"
            | "watchTerminal" => SnapshotFieldShape::Opaque,
            other => panic!("unclassified snapshot node field `{other}` — add its malformed rows"),
        }
    }

    /// Valid `run_complete` payload whose snapshot root carries one nested
    /// child — the two injection sites for the snapshot-node malformed rows.
    fn nested_completion_params() -> serde_json::Value {
        serde_json::json!({
            "workflowId": "workflow",
            "status": "completed",
            "finalState": {
                "workflowId": "workflow",
                "workflowName": "recipe",
                "status": "completed",
                "inputs": {},
                "artifacts": {},
                "capturedOutputs": {},
                "createdAt": "created",
                "planRevision": 1,
                "root": {
                    "nodeId": "workflow",
                    "type": "sequence",
                    "status": "completed",
                    "children": [{
                        "nodeId": "child",
                        "type": "step",
                        "status": "completed",
                        "agentName": "agent"
                    }]
                }
            }
        })
    }

    fn snapshot_site_mut(
        params: &mut serde_json::Value,
        nested: bool,
    ) -> &mut serde_json::Map<String, serde_json::Value> {
        let pointer = if nested {
            "/finalState/root/children/0"
        } else {
            "/finalState/root"
        };
        match params
            .pointer_mut(pointer)
            .and_then(serde_json::Value::as_object_mut)
        {
            Some(site) => site,
            None => panic!("snapshot injection site {pointer} missing from baseline"),
        }
    }

    /// Builds every malformed row the matrix fence asserts on: manifest-driven
    /// required-field omissions (event, descriptor, and snapshot-node at both
    /// the root and a nested child) followed by the hand-listed wrong-type,
    /// null, unknown-enum, empty-identifier, and mismatch rows. Kept apart
    /// from the fence itself so the row-set and the expectations it must meet
    /// stay separately readable.
    fn malformed_cases() -> Vec<MalformedCase> {
        let manifest = workflow_manifest();
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

        // Snapshot-node rows (2026-08-09 review, test finding 1): the
        // manifest's `snapshot_node_fields` oracle drives malformed injection
        // at BOTH the snapshot root and a nested child — the recursive site
        // no other family reaches.
        let snapshot_required = must_array(
            &manifest["snapshot_node_fields"]["required"],
            "required snapshot node fields",
        );
        let snapshot_optional = must_array(
            &manifest["snapshot_node_fields"]["optional"],
            "optional snapshot node fields",
        );
        for nested in [false, true] {
            let prefix = if nested {
                "finalState.root.children[0]"
            } else {
                "finalState.root"
            };
            let site = if nested { "child" } else { "root" };
            let push_case = |cases: &mut Vec<MalformedCase>,
                             kind: &str,
                             field: &str,
                             mutate: Option<serde_json::Value>,
                             error_kind: &'static str| {
                let mut params = nested_completion_params();
                match mutate {
                    Some(value) => {
                        snapshot_site_mut(&mut params, nested).insert(field.to_owned(), value);
                    }
                    None => {
                        let removed = snapshot_site_mut(&mut params, nested).remove(field);
                        assert!(
                            removed.is_some(),
                            "manifest snapshot field must exist in baseline: {site}.{field}"
                        );
                    }
                }
                cases.push(MalformedCase {
                    id: format!("snapshot.{site}.{kind}.{field}"),
                    method: "kiro/workflow/run_complete".to_owned(),
                    params,
                    field_path: format!("{prefix}.{field}"),
                    error_kind,
                });
            };
            for field in snapshot_required {
                let field = must_string(field, "required snapshot node field");
                push_case(&mut cases, "missing", field, None, "missing_required");
                push_case(
                    &mut cases,
                    "wrong_type",
                    field,
                    Some(serde_json::Value::Bool(false)),
                    "wrong_type",
                );
                if matches!(
                    snapshot_field_shape(field),
                    SnapshotFieldShape::RequiredEnum
                ) {
                    push_case(
                        &mut cases,
                        "unknown_enum",
                        field,
                        Some(serde_json::json!("nope")),
                        "invalid_enum",
                    );
                }
            }
            for field in snapshot_optional {
                let field = must_string(field, "optional snapshot node field");
                match snapshot_field_shape(field) {
                    SnapshotFieldShape::OptionalString | SnapshotFieldShape::OptionalU32 => {
                        push_case(
                            &mut cases,
                            "wrong_type",
                            field,
                            Some(serde_json::Value::Bool(false)),
                            "wrong_type",
                        );
                        push_case(
                            &mut cases,
                            "null",
                            field,
                            Some(serde_json::Value::Null),
                            "invalid_value",
                        );
                    }
                    SnapshotFieldShape::OptionalEnum => {
                        push_case(
                            &mut cases,
                            "unknown_enum",
                            field,
                            Some(serde_json::json!("nope")),
                            "invalid_enum",
                        );
                        push_case(
                            &mut cases,
                            "null",
                            field,
                            Some(serde_json::Value::Null),
                            "invalid_value",
                        );
                    }
                    SnapshotFieldShape::OptionalChildren => {
                        push_case(
                            &mut cases,
                            "wrong_type",
                            field,
                            Some(serde_json::Value::Bool(false)),
                            "wrong_type",
                        );
                        push_case(
                            &mut cases,
                            "null",
                            field,
                            Some(serde_json::Value::Null),
                            "invalid_value",
                        );
                    }
                    // Opaque JSON accepts every value including null (D18).
                    SnapshotFieldShape::Opaque => {}
                    SnapshotFieldShape::RequiredString | SnapshotFieldShape::RequiredEnum => {
                        panic!("required snapshot field `{field}` listed as optional")
                    }
                }
            }
        }

        // finalState run-metadata rows (2026-08-10 review, finding SP14):
        // the snapshot's own scalar metadata gets the same missing /
        // wrong-type / unknown-enum / null treatment as its nodes. The three
        // opaque JSON members (inputs/artifacts/capturedOutputs) accept any
        // value, so only their absence is a row.
        {
            fn final_state_mut(
                params: &mut serde_json::Value,
            ) -> &mut serde_json::Map<String, serde_json::Value> {
                match params
                    .get_mut("finalState")
                    .and_then(serde_json::Value::as_object_mut)
                {
                    Some(state) => state,
                    None => panic!("run_complete baseline lacks finalState"),
                }
            }
            let push_final_case = |cases: &mut Vec<MalformedCase>,
                                   kind: &str,
                                   field: &str,
                                   mutate: Option<serde_json::Value>,
                                   error_kind: &'static str| {
                let mut params = valid_payload("kiro/workflow/run_complete");
                match mutate {
                    Some(value) => {
                        final_state_mut(&mut params).insert(field.to_owned(), value);
                    }
                    None => {
                        let removed = final_state_mut(&mut params).remove(field);
                        assert!(
                            removed.is_some(),
                            "finalState baseline must carry required field {field}"
                        );
                    }
                }
                cases.push(MalformedCase {
                    id: format!("snapshot.metadata.{kind}.{field}"),
                    method: "kiro/workflow/run_complete".to_owned(),
                    params,
                    field_path: format!("finalState.{field}"),
                    error_kind,
                });
            };
            for field in [
                "workflowId",
                "workflowName",
                "status",
                "inputs",
                "artifacts",
                "capturedOutputs",
                "root",
                "createdAt",
                "planRevision",
            ] {
                push_final_case(&mut cases, "missing", field, None, "missing_required");
            }
            for field in [
                "workflowId",
                "workflowName",
                "status",
                "root",
                "createdAt",
                "planRevision",
            ] {
                push_final_case(
                    &mut cases,
                    "wrong_type",
                    field,
                    Some(serde_json::Value::Bool(false)),
                    "wrong_type",
                );
            }
            push_final_case(
                &mut cases,
                "unknown_enum",
                "status",
                Some(serde_json::json!("nope")),
                "invalid_enum",
            );
            for field in ["parentSessionId", "workspacePath"] {
                push_final_case(
                    &mut cases,
                    "wrong_type",
                    field,
                    Some(serde_json::Value::Bool(false)),
                    "wrong_type",
                );
                push_final_case(
                    &mut cases,
                    "null",
                    field,
                    Some(serde_json::Value::Null),
                    "invalid_value",
                );
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
        cases
    }

    #[test]
    fn malformed_workflow_field_matrix_isolated() {
        let started = Instant::now();
        let cases = malformed_cases();
        assert_eq!(cases.len(), 297, "malformed matrix row-set drift");
        let mut case_ids = cases.iter().map(|case| case.id.clone()).collect::<Vec<_>>();
        case_ids.sort_unstable();
        case_ids.dedup();
        assert_eq!(case_ids.len(), 297, "malformed case ids must be unique");
        for case in cases {
            let (result, log) = capture_rejection(&case.method, &case.params);
            assert!(
                matches!(result, WorkflowFrameOutcome::Dropped),
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
                    WorkflowFrameOutcome::Converted(_)
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
                "branchId": "",
                "parentSessionId": ""
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
                "completionSignalSource": "send_message",
                "parentSessionId": ""
            }),
            "kiro/workflow/node_paused" => serde_json::json!({
                "workflowId": "workflow",
                "nodeId": "node",
                "nodePath": ["workflow", "node"],
                "reason": "",
                "parentSessionId": ""
            }),
            "kiro/workflow/loop_iteration" => serde_json::json!({
                "workflowId": "workflow",
                "loopId": "loop",
                "iteration": 0,
                "stopConditionMet": false,
                "parentSessionId": ""
            }),
            "kiro/workflow/watch_poll" => serde_json::json!({
                "workflowId": "workflow",
                "nodeId": "watch",
                "nodePath": ["workflow", "watch"],
                "outcome": "idle",
                "at": "",
                "parentSessionId": ""
            }),
            "kiro/workflow/paused" => serde_json::json!({
                "workflowId": "workflow",
                "pauseReason": "",
                "parentSessionId": ""
            }),
            "kiro/workflow/run_complete" => serde_json::json!({
                "workflowId": "workflow",
                "status": "completed",
                "finalState": completed_snapshot("workflow", "completed"),
                "parentSessionId": ""
            }),
            "kiro/workflow/steps_queued" => serde_json::json!({
                "workflowId": "workflow",
                "pendingSteps": [],
                "resolution": {"outcome": "applied", "reason": ""},
                "parentSessionId": ""
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
                "modelId": "",
                "effortLevel": ""
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
                "agentName": ""
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
                ("modelId", serde_json::Value::Bool(false), "wrong_type"),
                ("modelId", serde_json::Value::Null, "invalid_value"),
                ("effortLevel", serde_json::Value::Bool(false), "wrong_type"),
                ("effortLevel", serde_json::Value::Null, "invalid_value"),
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
                ("agentName", serde_json::Value::Bool(false), "wrong_type"),
                ("agentName", serde_json::Value::Null, "wrong_type"),
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
                "../../../../tests/fixtures/kas/workflow/oracle-manifest.json"
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
                        WorkflowFrameOutcome::Converted(_)
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
                        WorkflowFrameOutcome::Converted(_)
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
        assert_eq!(started.node_tree()[0].model_id(), Some(""));
        assert_eq!(started.node_tree()[0].effort_level(), Some(""));
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
            WorkflowFrameOutcome::Dropped
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
            started_at.elapsed() <= Duration::from_secs(5),
            "1 MiB depth-32 opaque conversion exceeded 5 s"
        );
    }

    #[test]
    fn workflow_scalar_string_matrix() {
        let large = "x".repeat(65_536);
        for value in [
            "",
            "plain",
            "識別",
            " with space ",
            "path/with/slash",
            "back\\slash",
            large.as_str(),
        ] {
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
            for field in ["agentName", "modelId", "effortLevel"] {
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
            assert_eq!(descriptor.model_id(), Some(value));
            assert_eq!(descriptor.effort_level(), Some(value));

            // Remaining opaque scalars (2026-08-10 review, finding SP16):
            // poll timestamps, run/ack reasons, watch handler names, and the
            // snapshot's createdAt are byte-preserved like every other
            // non-ID string.
            let mut params = valid_payload("kiro/workflow/watch_poll");
            set_top_field(
                &mut params,
                "at",
                serde_json::Value::String(value.to_owned()),
            );
            let poll = match event(
                to_notification("kiro/workflow/watch_poll", &params),
                "watch poll timestamp",
            ) {
                WorkflowEvent::WatchPoll(poll) => poll,
                other => panic!("expected watch_poll, got {other:?}"),
            };
            assert_eq!(poll.at(), value);

            let mut params = valid_payload("kiro/workflow/paused");
            set_top_field(
                &mut params,
                "pauseReason",
                serde_json::Value::String(value.to_owned()),
            );
            let paused = match event(
                to_notification("kiro/workflow/paused", &params),
                "run pause reason",
            ) {
                WorkflowEvent::Paused(paused) => paused,
                other => panic!("expected paused, got {other:?}"),
            };
            assert_eq!(paused.pause_reason(), value);

            let params = serde_json::json!({
                "workflowId": "workflow",
                "pendingSteps": [],
                "resolution": {"outcome": "applied", "reason": value}
            });
            let queued = match event(
                to_notification("kiro/workflow/steps_queued", &params),
                "acknowledgement reason",
            ) {
                WorkflowEvent::StepsQueued(queued) => queued,
                other => panic!("expected steps_queued, got {other:?}"),
            };
            assert_eq!(
                queued
                    .resolution()
                    .and_then(WorkflowQueueResolution::reason),
                Some(value)
            );

            let watch = serde_json::json!({"nodeId": "watch", "type": "watch", "agentName": value});
            let watch_started = match event(
                to_notification("kiro/workflow/run_start", &descriptor_event_payload(watch)),
                "watch handler name",
            ) {
                WorkflowEvent::RunStarted(started) => started,
                other => panic!("expected run_start, got {other:?}"),
            };
            assert_eq!(watch_started.node_tree()[0].handler_name(), Some(value));

            let mut params = valid_payload("kiro/workflow/run_complete");
            let final_state = match params
                .get_mut("finalState")
                .and_then(serde_json::Value::as_object_mut)
            {
                Some(state) => state,
                None => panic!("run_complete baseline lacks finalState"),
            };
            final_state.insert(
                "createdAt".to_owned(),
                serde_json::Value::String(value.to_owned()),
            );
            let completed = match event(
                to_notification("kiro/workflow/run_complete", &params),
                "snapshot createdAt",
            ) {
                WorkflowEvent::RunCompleted(completed) => completed,
                other => panic!("expected run_complete, got {other:?}"),
            };
            assert_eq!(completed.final_state().created_at(), value);
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
                    started_at.elapsed() <= Duration::from_secs(5),
                    "64 KiB identifier conversions exceeded 5 s"
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
                matches!(
                    to_notification(method, &params),
                    WorkflowFrameOutcome::Dropped
                ),
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
            WorkflowFrameOutcome::Dropped
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
                let removed = step_object.remove("modelId");
                assert!(removed.is_some());
            }
            if mask & 2 == 0 {
                let removed = step_object.remove("effortLevel");
                assert!(removed.is_some());
            }
            let started = match event(
                to_notification("kiro/workflow/run_start", &descriptor_event_payload(step)),
                "step optional mask",
            ) {
                WorkflowEvent::RunStarted(started) => started,
                other => panic!("expected run_start, got {other:?}"),
            };
            assert_eq!(started.node_tree()[0].model_id().is_some(), mask & 1 != 0);
            assert_eq!(
                started.node_tree()[0].effort_level().is_some(),
                mask & 2 != 0
            );

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
            "watchCursor",
            "watchTerminal",
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
                        "artifacts" | "capturedOutput" | "watchCursor" | "watchTerminal" => {
                            serde_json::Value::Null
                        }
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
                    "watchCursor" => node.watch_cursor().is_some(),
                    "watchTerminal" => node.watch_terminal().is_some(),
                    other => panic!("unknown node optional field {other}"),
                };
                assert_eq!(actual, present, "snapshot root optional {field}");
            }
        }
        let methods = [
            "kiro/workflow/run_start",
            "kiro/workflow/node_start",
            "kiro/workflow/node_complete",
            "kiro/workflow/node_paused",
            "kiro/workflow/loop_iteration",
            "kiro/workflow/watch_poll",
            "kiro/workflow/paused",
            "kiro/workflow/run_complete",
            "kiro/workflow/steps_queued",
        ];
        for method in methods {
            for present in [false, true] {
                let mut params = valid_payload(method);
                if !present {
                    let removed =
                        must_object_mut(&mut params, "workflow event").remove("parentSessionId");
                    assert!(removed.is_some(), "{method}: parentSessionId baseline");
                }
                let actual = match event(to_notification(method, &params), "parent session") {
                    WorkflowEvent::RunStarted(event) => event.parent_session_id().is_some(),
                    WorkflowEvent::NodeStarted(event) => event.parent_session_id().is_some(),
                    WorkflowEvent::NodeCompleted(event) => event.parent_session_id().is_some(),
                    WorkflowEvent::NodePaused(event) => event.parent_session_id().is_some(),
                    WorkflowEvent::LoopIteration(event) => event.parent_session_id().is_some(),
                    WorkflowEvent::WatchPoll(event) => event.parent_session_id().is_some(),
                    WorkflowEvent::Paused(event) => event.parent_session_id().is_some(),
                    WorkflowEvent::RunCompleted(event) => event.parent_session_id().is_some(),
                    WorkflowEvent::StepsQueued(event) => event.parent_session_id().is_some(),
                };
                assert_eq!(actual, present, "{method}: parentSessionId");
            }
        }
    }

    #[test]
    fn workflow_sparse_snapshot_descriptor_matrix() {
        for node_type in ["step", "sequence", "repeat", "parallel", "watch"] {
            let mut snapshot = completed_snapshot("workflow", "completed");
            let root = match must_object_mut(&mut snapshot, "snapshot").get_mut("root") {
                Some(value) => value,
                None => panic!("snapshot root absent"),
            };
            set_top_field(
                root,
                "type",
                serde_json::Value::String(node_type.to_owned()),
            );
            let wire: WireSnapshot =
                must_succeed(deserialize(&snapshot), "sparse snapshot descriptor");
            let snapshot = must_succeed(wire.try_into_domain(), "sparse snapshot domain");
            let descriptor = snapshot.root().descriptor();
            assert_eq!(descriptor.node_type().as_str(), node_type);
            assert!(descriptor.agent_name().is_none());
            assert!(descriptor.model_id().is_none());
            assert!(descriptor.effort_level().is_none());
            assert!(descriptor.max_iterations().is_none());
            assert!(descriptor.on_max_iterations().is_none());
            assert!(descriptor.stop_condition().is_none());
            assert!(descriptor.stop_when().is_none());
            assert!(descriptor.handler_name().is_none());
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
            started_at.elapsed() <= Duration::from_secs(5),
            "256-node conversion exceeded 5 s"
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
                    matches!(
                        to_notification(method, &params),
                        WorkflowFrameOutcome::Dropped
                    ),
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
                WorkflowFrameOutcome::Dropped
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
                    started_at.elapsed() <= Duration::from_secs(5),
                    "64 KiB path segment conversion exceeded 5 s"
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
                WorkflowFrameOutcome::Dropped
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
                    started_at.elapsed() <= Duration::from_secs(5),
                    "64 KiB workspace path conversion exceeded 5 s"
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
                    assert!(matches!(result, WorkflowFrameOutcome::Dropped));
                    assert_eq!(log["level"], "WARN");
                    assert_eq!(log["fields"]["method"], "kiro/workflow/run_complete");
                    assert_eq!(log["fields"]["field_path"], "status");
                    assert_eq!(log["fields"]["error_kind"], "status_mismatch");
                    assert_eq!(log["fields"]["message"], "malformed workflow notification");
                }
            }
        }

        // (2026-08-10 review, finding SP15): `running` is not a completion
        // status — even the running/running pair must reject as an unknown
        // enum value at the outer field, never convert.
        for snapshot in snapshot_statuses {
            let mut params = valid_payload("kiro/workflow/run_complete");
            set_top_field(
                &mut params,
                "status",
                serde_json::Value::String("running".to_owned()),
            );
            set_top_field(
                &mut params,
                "finalState",
                completed_snapshot("workflow", snapshot),
            );
            let (result, log) = capture_rejection("kiro/workflow/run_complete", &params);
            assert!(
                matches!(result, WorkflowFrameOutcome::Dropped),
                "running completion status must drop (snapshot {snapshot})"
            );
            assert_eq!(log["fields"]["field_path"], "status");
            assert_eq!(log["fields"]["error_kind"], "invalid_enum");
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

    /// One live pipeline receives every malformed row in sequence: none may
    /// convert, none may perturb existing state, and the pipeline must stay
    /// fully usable afterwards. (Reworked 2026-08-10, review finding S6 —
    /// the previous body re-ran four sibling `#[test]`s, adding runtime but
    /// no cross-frame coverage.)
    #[test]
    fn malformed_workflow_pipeline_is_atomic() {
        let opening = event(
            to_notification(
                "kiro/workflow/run_start",
                &valid_payload("kiro/workflow/run_start"),
            ),
            "pipeline opening",
        );
        let workflow_id = opening.workflow_id().clone();
        let mut tracker = WorkflowTracker::new();
        assert_eq!(tracker.apply_event(opening), Ok(true));
        let before = tracker.get(&workflow_id).cloned();
        for case in malformed_cases() {
            assert!(
                matches!(
                    to_notification(&case.method, &case.params),
                    WorkflowFrameOutcome::Dropped
                ),
                "{}: malformed row must stay dropped mid-pipeline",
                case.id
            );
        }
        assert_eq!(
            tracker.get(&workflow_id),
            before.as_ref(),
            "no malformed frame may reach or perturb live state"
        );
        let successor = event(
            to_notification(
                "kiro/workflow/node_start",
                &valid_payload("kiro/workflow/node_start"),
            ),
            "pipeline successor",
        );
        assert_eq!(tracker.apply_event(successor), Ok(true));
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

    // ---- request-reply parsing (cyril-0qe6 C4) --------------------------

    const INSPECT_REPLY_2180: &str =
        include_str!("../../../../tests/fixtures/kas/workflow/inspect-reply-2.18.0.json");
    const NEW_REPLY_2180: &str =
        include_str!("../../../../tests/fixtures/kas/workflow/new-reply-2.18.0.json");

    fn fixture_value(raw: &str) -> serde_json::Value {
        must_succeed(serde_json::from_str(raw), "fixture is valid JSON")
    }

    #[test]
    fn state_reply_parses_live_inspect_fixture() {
        let snapshot = must_succeed(
            parse_state_reply(&fixture_value(INSPECT_REPLY_2180)),
            "live inspect reply",
        );
        assert_eq!(snapshot.workflow_name(), "cyril-reattach2");
        assert_eq!(snapshot.status(), WorkflowRunStatus::Completed);
        assert_eq!(snapshot.root().children().len(), 1);
    }

    #[test]
    fn state_reply_parses_live_new_fixture_via_initial_state_alias() {
        let snapshot = must_succeed(
            parse_state_reply(&fixture_value(NEW_REPLY_2180)),
            "live new reply",
        );
        assert_eq!(snapshot.workflow_name(), "cyril-reattach2");
        assert_eq!(snapshot.status(), WorkflowRunStatus::Running);
    }

    #[test]
    fn state_reply_rejects_workflow_id_mismatch() {
        let mut reply = fixture_value(INSPECT_REPLY_2180);
        reply["workflowId"] = serde_json::Value::String("wf_other".into());
        let error = parse_state_reply(&reply).map(|snapshot| snapshot.status());
        let Err(error) = error else {
            panic!("mismatched ids must not parse, got {error:?}");
        };
        assert!(
            error.to_string().contains("does not match"),
            "error must name the mismatch: {error}"
        );
    }

    const LIST_REPLY_2180: &str =
        include_str!("../../../../tests/fixtures/kas/workflow/list-reply-2.18.0.json");
    const LIST_REPLY_NEVER_INVOKED_2180: &str = include_str!(
        "../../../../tests/fixtures/kas/workflow/list-reply-never-invoked-2.18.0.json"
    );
    const RECIPES_REPLY_2162: &str =
        include_str!("../../../../tests/fixtures/kas/workflow/recipes-reply-2.16.2.json");
    const RECIPES_REPLY_DISK_2162: &str = include_str!(
        "../../../../tests/fixtures/kas/workflow/recipes-reply-diskrecipe-2.16.2.json"
    );
    const CANCEL_REPLY_2180: &str =
        include_str!("../../../../tests/fixtures/kas/workflow/cancel-reply-2.18.0.json");

    #[test]
    fn list_reply_parses_live_completed_fixture() {
        let listing = must_succeed(
            parse_list_reply(&fixture_value(LIST_REPLY_2180)),
            "live list reply",
        );
        assert_eq!(listing.runs.len(), 1);
        assert!(listing.skipped.is_empty());
        let run = &listing.runs[0];
        assert_eq!(run.status, WorkflowRunStatus::Completed);
        assert!(run.started_at.is_some(), "invoked run carries startedAt");
        assert!(run.ended_at.is_some(), "terminal run carries endedAt");
    }

    #[test]
    fn list_reply_never_invoked_entry_has_no_started_at() {
        let listing = must_succeed(
            parse_list_reply(&fixture_value(LIST_REPLY_NEVER_INVOKED_2180)),
            "live never-invoked list reply",
        );
        assert_eq!(listing.runs.len(), 1);
        let run = &listing.runs[0];
        assert_eq!(run.status, WorkflowRunStatus::Aborted);
        assert!(
            run.started_at.is_none(),
            "never-invoked run must have Option::None startedAt, not a sentinel"
        );
        assert!(run.ended_at.is_none());
    }

    #[test]
    fn list_reply_skips_unknown_status_entry_without_killing_listing() {
        let mut reply = fixture_value(LIST_REPLY_2180);
        let good = fixture_value(LIST_REPLY_NEVER_INVOKED_2180)["runs"][0].clone();
        let mut bogus = good.clone();
        bogus["status"] = serde_json::Value::String("quantum".into());
        let runs = must_succeed(
            reply["runs"].as_array_mut().ok_or("runs must be an array"),
            "runs array",
        );
        runs.push(bogus);
        runs.push(good);
        let listing = must_succeed(parse_list_reply(&reply), "tolerant list reply");
        assert_eq!(listing.runs.len(), 2, "both good entries survive");
        assert_eq!(listing.skipped.len(), 1, "the bogus entry is skipped");
        assert!(
            listing.skipped[0].to_string().contains("quantum"),
            "skip reason names the unknown value: {}",
            listing.skipped[0]
        );
    }

    #[test]
    fn list_reply_empty_runs_is_empty_listing() {
        let listing = must_succeed(
            parse_list_reply(&serde_json::json!({"runs": []})),
            "empty list reply",
        );
        assert!(listing.runs.is_empty());
        assert!(listing.skipped.is_empty());
    }

    #[test]
    fn recipes_reply_parses_all_seven_bundled() {
        let listing = must_succeed(
            parse_recipes_reply(&fixture_value(RECIPES_REPLY_2162)),
            "live recipes reply",
        );
        assert_eq!(listing.recipes.len(), 7);
        assert!(listing.skipped.is_empty());
        let names: Vec<&str> = listing
            .recipes
            .iter()
            .map(|recipe| recipe.name.as_str())
            .collect();
        assert!(names.contains(&"ralph"));
        assert!(names.contains(&"autoresearch"));
        assert!(
            listing
                .recipes
                .iter()
                .all(|recipe| recipe.source.as_deref().is_some_and(|s| !s.is_empty())),
            "bundled recipes carry bundled:// sources (live-observed)"
        );
    }

    #[test]
    fn recipes_reply_workspace_recipe_carries_its_path() {
        let listing = must_succeed(
            parse_recipes_reply(&fixture_value(RECIPES_REPLY_DISK_2162)),
            "live diskrecipe recipes reply",
        );
        assert!(
            listing.recipes.iter().any(|recipe| {
                recipe
                    .source
                    .as_deref()
                    .is_some_and(|source| source.starts_with('/'))
            }),
            "a workspace recipe must surface its absolute path"
        );
    }

    #[test]
    fn cancel_reply_parses_live_fixture() {
        let reply = must_succeed(
            parse_cancel_reply(&fixture_value(CANCEL_REPLY_2180)),
            "live cancel reply",
        );
        assert!(reply.ok);
        assert_eq!(reply.previous_status, Some(WorkflowRunStatus::Running));
    }

    #[test]
    fn cancel_reply_shape_is_not_the_run_status_shape() {
        // Guards against modeling cancel with the invoke/resume reply
        // struct: the cancel fixture has no workflowId, so the run-status
        // parser must refuse it rather than fabricate a value.
        let error = parse_run_status_reply(&fixture_value(CANCEL_REPLY_2180))
            .map(|(id, status)| (id.as_str().to_owned(), status));
        let Err(error) = error else {
            panic!("cancel reply must not parse as a run-status reply, got {error:?}");
        };
        assert!(
            error.to_string().contains("workflowId"),
            "error names the missing field: {error}"
        );
    }

    #[test]
    fn run_status_reply_tolerates_unknown_status() {
        let (id, status) = must_succeed(
            parse_run_status_reply(&serde_json::json!({"workflowId": "wf_x", "status": "quantum"})),
            "unknown status tolerated",
        );
        assert_eq!(id.as_str(), "wf_x");
        assert_eq!(status, None, "unknown status is unreported, not fatal");
    }

    /// cyril-0qe6 C4 end-to-end at the core level: the live inspect reply,
    /// parsed and applied, is visible in the tracker with the capture's
    /// status and node count. (Colocated here rather than in `tests/`
    /// because `parse_state_reply` is deliberately `pub(crate)`.)
    #[test]
    fn attach_snapshot_seeds_tracker_from_live_reply() {
        let snapshot = must_succeed(
            parse_state_reply(&fixture_value(INSPECT_REPLY_2180)),
            "live inspect reply",
        );
        let workflow_id = snapshot.workflow_id().clone();
        let expected_nodes = snapshot_node_count(snapshot.root());
        let mut tracker = WorkflowTracker::new();
        let changed = must_succeed(tracker.apply_snapshot(snapshot), "snapshot applies");
        assert!(changed);
        let run = tracker.get(&workflow_id).map(|run| {
            (
                run.status(),
                run.workflow_name().to_owned(),
                run.nodes().count(),
            )
        });
        let Some((status, name, nodes)) = run else {
            panic!("the fetched run must be visible in the tracker");
        };
        assert_eq!(status, Some(WorkflowRunStatus::Completed));
        assert_eq!(name, "cyril-reattach2");
        assert_eq!(nodes, expected_nodes, "every captured node is tracked");
    }

    /// cyril-0qe6 C5: a snapshot whose terminal status conflicts with the
    /// tracker's is rejected without state change (the ignored-Err bug
    /// class would silently flip history).
    #[test]
    fn conflicting_terminal_snapshot_is_rejected_without_change() {
        let snapshot = must_succeed(
            parse_state_reply(&fixture_value(INSPECT_REPLY_2180)),
            "live inspect reply",
        );
        let workflow_id = snapshot.workflow_id().clone();
        let mut tracker = WorkflowTracker::new();
        must_succeed(tracker.apply_snapshot(snapshot), "first snapshot applies");

        let mut doctored = fixture_value(INSPECT_REPLY_2180);
        doctored["state"]["status"] = serde_json::Value::String("failed".into());
        let conflicting = must_succeed(parse_state_reply(&doctored), "doctored reply still parses");
        let error = tracker.apply_snapshot(conflicting);
        assert!(
            error.is_err(),
            "a conflicting terminal snapshot must be refused, got {error:?}"
        );
        let status = tracker.get(&workflow_id).and_then(WorkflowRun::status);
        assert_eq!(
            status,
            Some(WorkflowRunStatus::Completed),
            "the tracker keeps its original terminal status"
        );
    }

    #[test]
    fn state_reply_rejects_missing_root() {
        let mut reply = fixture_value(INSPECT_REPLY_2180);
        let state = must_succeed(
            reply["state"]
                .as_object_mut()
                .ok_or("state must be an object"),
            "state object",
        );
        state.remove("root");
        let error = parse_state_reply(&reply).map(|snapshot| snapshot.status());
        let Err(error) = error else {
            panic!("a reply without state.root must not parse, got {error:?}");
        };
        assert!(
            error.to_string().contains("root"),
            "error must name the missing field: {error}"
        );
    }
}
