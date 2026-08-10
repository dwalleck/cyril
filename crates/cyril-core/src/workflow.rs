use std::collections::HashMap;

use crate::types::workflow::{
    WorkflowNodeCompletedParts, WorkflowNodeCompletionParts, WorkflowNodeSnapshotParts,
    WorkflowNodeStartParts, WorkflowNodeStartedParts, WorkflowRunStartedParts,
    WorkflowSnapshotParts,
};
use crate::types::{
    SessionId, WorkflowCompletionSignal, WorkflowCompletionSignalSource, WorkflowEvent, WorkflowId,
    WorkflowLoopIteration, WorkflowNodeCompleted, WorkflowNodeDescriptor, WorkflowNodeId,
    WorkflowNodePath, WorkflowNodePathError, WorkflowNodePaused, WorkflowNodeSnapshot,
    WorkflowNodeStarted, WorkflowNodeStatus, WorkflowNodeType, WorkflowPaused,
    WorkflowQueueResolution, WorkflowRunCompleted, WorkflowRunStarted, WorkflowRunStatus,
    WorkflowSnapshot, WorkflowStepsQueued, WorkflowWatchOutcome, WorkflowWatchPoll,
};

/// State-application failure that leaves the tracker byte-for-byte unchanged.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkflowStateError {
    /// Two runtime nodes map to the same canonical path.
    #[error("duplicate canonical workflow node path {path:?}")]
    DuplicateCanonicalPath { path: WorkflowNodePath },
    /// A canonical path could not satisfy the path-domain invariant.
    #[error("invalid canonical workflow node path: {source}")]
    InvalidCanonicalPath { source: WorkflowNodePathError },
    /// A direct snapshot cannot reopen or change a terminal incarnation.
    #[error(
        "terminal workflow `{workflow_id}` cannot reconcile snapshot status `{incoming}` from `{current}`"
    )]
    TerminalSnapshotConflict {
        workflow_id: WorkflowId,
        current: WorkflowRunStatus,
        incoming: WorkflowRunStatus,
    },
}

impl WorkflowStateError {
    /// Returns the stable diagnostic category for this failure.
    pub const fn error_kind(&self) -> &'static str {
        match self {
            Self::DuplicateCanonicalPath { .. } => "duplicate_canonical_path",
            Self::InvalidCanonicalPath { .. } => "invalid_canonical_path",
            Self::TerminalSnapshotConflict { .. } => "terminal_snapshot_conflict",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum WorkflowNodeIdentity {
    Opening {
        node_id: WorkflowNodeId,
        node_type: WorkflowNodeType,
        agent_name: Option<String>,
    },
    Snapshot {
        descriptor: WorkflowNodeDescriptor,
        event_agent_name: Option<String>,
    },
}

/// Immutable read model for one canonical runtime node.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowNodeState {
    identity: WorkflowNodeIdentity,
    status: Option<WorkflowNodeStatus>,
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
    watch_cursor: Option<serde_json::Value>,
    watch_terminal: Option<serde_json::Value>,
    prompt: Option<String>,
    node_pause_reason: Option<String>,
    latest_loop_iteration: Option<(u32, bool)>,
    latest_watch_poll: Option<(WorkflowWatchOutcome, String)>,
}

impl WorkflowNodeState {
    fn from_snapshot(parts: WorkflowNodeSnapshotParts) -> Self {
        let WorkflowNodeSnapshotParts {
            descriptor,
            status,
            session_id,
            artifacts,
            captured_output,
            failure_reason,
            iteration,
            branch_id,
            completion_signal,
            completion_signal_source,
            started_at,
            ended_at,
            watch_cursor,
            watch_terminal,
            children,
        } = parts;
        debug_assert!(
            children.is_empty(),
            "from_snapshot expects the caller to have taken children for recursion"
        );
        Self {
            identity: WorkflowNodeIdentity::Snapshot {
                descriptor,
                event_agent_name: None,
            },
            status: Some(status),
            session_id,
            artifacts,
            captured_output,
            failure_reason,
            iteration,
            branch_id,
            completion_signal,
            completion_signal_source,
            started_at,
            ended_at,
            watch_cursor,
            watch_terminal,
            prompt: None,
            node_pause_reason: None,
            latest_loop_iteration: None,
            latest_watch_poll: None,
        }
    }

    fn from_opening(
        node_id: WorkflowNodeId,
        node_type: WorkflowNodeType,
        agent_name: Option<String>,
        session_id: Option<SessionId>,
        prompt: Option<String>,
        iteration: Option<u32>,
        branch_id: Option<String>,
    ) -> Self {
        Self {
            identity: WorkflowNodeIdentity::Opening {
                node_id,
                node_type,
                agent_name,
            },
            status: None,
            session_id,
            artifacts: None,
            captured_output: None,
            failure_reason: None,
            iteration,
            branch_id,
            completion_signal: None,
            completion_signal_source: None,
            started_at: None,
            ended_at: None,
            watch_cursor: None,
            watch_terminal: None,
            prompt,
            node_pause_reason: None,
            latest_loop_iteration: None,
            latest_watch_poll: None,
        }
    }

    /// Returns the snapshot-authored descriptor after snapshot reconciliation.
    pub fn descriptor(&self) -> Option<&WorkflowNodeDescriptor> {
        match &self.identity {
            WorkflowNodeIdentity::Opening { .. } => None,
            WorkflowNodeIdentity::Snapshot { descriptor, .. } => Some(descriptor),
        }
    }

    /// Returns the exact node identifier from either opening or snapshot state.
    pub fn node_id(&self) -> &WorkflowNodeId {
        match &self.identity {
            WorkflowNodeIdentity::Opening { node_id, .. } => node_id,
            WorkflowNodeIdentity::Snapshot { descriptor, .. } => descriptor.node_id(),
        }
    }

    /// Returns the structural node type from either opening or snapshot state.
    pub fn node_type(&self) -> WorkflowNodeType {
        match &self.identity {
            WorkflowNodeIdentity::Opening { node_type, .. } => *node_type,
            WorkflowNodeIdentity::Snapshot { descriptor, .. } => descriptor.node_type(),
        }
    }

    /// Returns the opening or snapshot-authored step agent when supplied.
    pub fn agent_name(&self) -> Option<&str> {
        match &self.identity {
            WorkflowNodeIdentity::Opening { agent_name, .. } => agent_name.as_deref(),
            WorkflowNodeIdentity::Snapshot {
                descriptor,
                event_agent_name,
            } => event_agent_name
                .as_deref()
                .or_else(|| descriptor.agent_name()),
        }
    }

    /// Returns the snapshot-authored status when this node has been snapshotted.
    pub fn status(&self) -> Option<WorkflowNodeStatus> {
        self.status
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

    /// Returns completion-signal metadata when supplied.
    pub fn completion_signal(&self) -> Option<WorkflowCompletionSignal> {
        self.completion_signal
    }

    /// Returns completion-signal source metadata when supplied.
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

    /// Returns opaque snapshot-authored watch cursor metadata when supplied.
    pub fn watch_cursor(&self) -> Option<&serde_json::Value> {
        self.watch_cursor.as_ref()
    }

    /// Returns opaque snapshot-authored watch terminal metadata when supplied.
    pub fn watch_terminal(&self) -> Option<&serde_json::Value> {
        self.watch_terminal.as_ref()
    }

    /// Returns the most recent node-start prompt when supplied.
    pub fn prompt(&self) -> Option<&str> {
        self.prompt.as_deref()
    }

    /// Returns the latest node-specific pause reason.
    pub fn node_pause_reason(&self) -> Option<&str> {
        self.node_pause_reason.as_deref()
    }

    /// Returns the latest repeat iteration and exact stop-condition outcome.
    pub fn latest_loop_iteration(&self) -> Option<(u32, bool)> {
        self.latest_loop_iteration
    }

    /// Returns the latest watch outcome and opaque observation timestamp.
    pub fn latest_watch_poll(&self) -> Option<(WorkflowWatchOutcome, &str)> {
        self.latest_watch_poll
            .as_ref()
            .map(|(outcome, at)| (*outcome, at.as_str()))
    }
}

/// Immutable read model for one persisted workflow run.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowRun {
    workflow_name: String,
    status: Option<WorkflowRunStatus>,
    inputs: serde_json::Value,
    artifacts: Option<serde_json::Value>,
    captured_outputs: Option<serde_json::Value>,
    created_at: Option<String>,
    plan_revision: Option<u32>,
    parent_session_id: Option<SessionId>,
    workspace_path: Option<String>,
    opening_plan: Option<Vec<WorkflowNodeDescriptor>>,
    snapshot_plan: Option<WorkflowNodeDescriptor>,
    nodes: HashMap<WorkflowNodePath, WorkflowNodeState>,
    node_index: HashMap<WorkflowNodeId, Vec<WorkflowNodePath>>,
    pending_steps: Option<Vec<WorkflowNodeDescriptor>>,
    queue_resolution: Option<WorkflowQueueResolution>,
    run_pause_reason: Option<String>,
}

/// The authority behind a run's current node plan (see [`WorkflowRun::plan`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WorkflowPlan<'run> {
    /// Declared by `run_start`; no persisted snapshot has reconciled yet.
    Opening(&'run [WorkflowNodeDescriptor]),
    /// Authored by the latest reconciled snapshot's root descriptor.
    Snapshot(&'run WorkflowNodeDescriptor),
}

impl WorkflowRun {
    /// Returns the recipe name for this run.
    pub fn workflow_name(&self) -> &str {
        &self.workflow_name
    }

    /// Returns the authoritative run status after a persisted snapshot arrives.
    pub fn status(&self) -> Option<WorkflowRunStatus> {
        self.status
    }

    /// Returns opaque recipe inputs.
    pub fn inputs(&self) -> &serde_json::Value {
        &self.inputs
    }

    /// Returns opaque run artifacts after a persisted snapshot arrives.
    pub fn artifacts(&self) -> Option<&serde_json::Value> {
        self.artifacts.as_ref()
    }

    /// Returns opaque captured outputs after a persisted snapshot arrives.
    pub fn captured_outputs(&self) -> Option<&serde_json::Value> {
        self.captured_outputs.as_ref()
    }

    /// Returns the opaque creation timestamp after a persisted snapshot arrives.
    pub fn created_at(&self) -> Option<&str> {
        self.created_at.as_deref()
    }

    /// Returns the persisted plan revision after a persisted snapshot arrives.
    pub fn plan_revision(&self) -> Option<u32> {
        self.plan_revision
    }

    /// Returns the parent session when supplied.
    pub fn parent_session_id(&self) -> Option<&SessionId> {
        self.parent_session_id.as_ref()
    }

    /// Returns the opaque workspace path when supplied.
    pub fn workspace_path(&self) -> Option<&str> {
        self.workspace_path.as_deref()
    }

    /// Returns the current node plan, naming which authority supplied it —
    /// the single seam for "has a snapshot reconciled yet?" so consumers
    /// never have to learn the pairing between the individual accessors.
    pub fn plan(&self) -> Option<WorkflowPlan<'_>> {
        if let Some(snapshot_plan) = self.snapshot_plan.as_ref() {
            return Some(WorkflowPlan::Snapshot(snapshot_plan));
        }
        self.opening_plan.as_deref().map(WorkflowPlan::Opening)
    }

    /// Returns the opening descriptor forest until a persisted snapshot replaces it.
    pub fn opening_plan(&self) -> Option<&[WorkflowNodeDescriptor]> {
        if self.snapshot_plan.is_some() {
            return None;
        }
        self.opening_plan.as_deref()
    }

    /// Returns the persisted root descriptor after a snapshot replaces the opening plan.
    pub fn snapshot_plan(&self) -> Option<&WorkflowNodeDescriptor> {
        self.snapshot_plan.as_ref()
    }

    /// Returns the currently pending workflow descriptors after a queue update.
    pub fn pending_steps(&self) -> Option<&[WorkflowNodeDescriptor]> {
        self.pending_steps.as_deref()
    }

    /// Returns the most recent queue acknowledgement when supplied.
    pub fn queue_resolution(&self) -> Option<&WorkflowQueueResolution> {
        self.queue_resolution.as_ref()
    }

    /// Returns the latest run-level pause reason.
    pub fn run_pause_reason(&self) -> Option<&str> {
        self.run_pause_reason.as_deref()
    }

    /// Returns one node by exact canonical path.
    pub fn node(&self, path: &WorkflowNodePath) -> Option<&WorkflowNodeState> {
        self.nodes.get(path)
    }

    /// Iterates every canonical node without allocating or imposing an order.
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = (&WorkflowNodePath, &WorkflowNodeState)> {
        self.nodes.iter()
    }

    fn index_node(&mut self, node_id: WorkflowNodeId, path: WorkflowNodePath) {
        insert_indexed_path(&mut self.node_index, node_id, path);
    }

    fn move_index(
        &mut self,
        previous_id: &WorkflowNodeId,
        current_id: WorkflowNodeId,
        path: &WorkflowNodePath,
    ) {
        let remove_bucket = if let Some(paths) = self.node_index.get_mut(previous_id) {
            if let Some(index) = paths.iter().position(|candidate| candidate == path) {
                paths.remove(index);
            }
            paths.is_empty()
        } else {
            false
        };
        if remove_bucket {
            self.node_index.remove(previous_id);
        }
        self.index_node(current_id, path.clone());
    }
}

/// Pure state machine for workspace-persisted workflow runs.
#[derive(Debug, Default)]
pub struct WorkflowTracker {
    runs: HashMap<WorkflowId, WorkflowRun>,
}

impl WorkflowTracker {
    /// Constructs an empty workflow tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies one workflow lifecycle event to persisted state.
    ///
    /// # Errors
    ///
    /// Returns a structured error without changing state when an accepted
    /// completion snapshot fails canonical validation.
    pub fn apply_event(&mut self, event: WorkflowEvent) -> Result<bool, WorkflowStateError> {
        match event {
            WorkflowEvent::RunStarted(opening) => self.apply_opening(opening),
            WorkflowEvent::NodeStarted(started) => Ok(self.apply_node_started(started)),
            WorkflowEvent::NodeCompleted(completed) => Ok(self.apply_node_completed(completed)),
            WorkflowEvent::NodePaused(paused) => Ok(self.apply_node_paused(paused)),
            WorkflowEvent::LoopIteration(iteration) => Ok(self.apply_loop_iteration(iteration)),
            WorkflowEvent::WatchPoll(poll) => Ok(self.apply_watch_poll(poll)),
            WorkflowEvent::Paused(paused) => Ok(self.apply_paused(paused)),
            WorkflowEvent::RunCompleted(completion) => self.apply_completion(completion),
            WorkflowEvent::StepsQueued(queued) => Ok(self.apply_steps_queued(queued)),
        }
    }

    /// Applies one complete persisted snapshot.
    ///
    /// Missing and active runs accept every valid snapshot status. A terminal
    /// run accepts only another direct snapshot with the same terminal status.
    ///
    /// # Errors
    ///
    /// Returns a structured error without changing the tracker when nodes map
    /// to the same canonical path, a path violates its domain invariant, or a
    /// direct snapshot conflicts with an existing terminal incarnation.
    pub fn apply_snapshot(
        &mut self,
        snapshot: WorkflowSnapshot,
    ) -> Result<bool, WorkflowStateError> {
        let workflow_id = snapshot.workflow_id().clone();
        let incoming_status = snapshot.status();
        if let Some(current) = self.runs.get(&workflow_id)
            && let Some(current_status) = current.status
            && is_terminal(Some(current_status))
            && current_status != incoming_status
        {
            return Err(WorkflowStateError::TerminalSnapshotConflict {
                workflow_id,
                current: current_status,
                incoming: incoming_status,
            });
        }
        let (workflow_id, mut run) = canonicalize_snapshot(snapshot)?;
        self.preserve_event_only(&workflow_id, &mut run);
        self.replace_if_changed(workflow_id, run)
    }

    /// Applies one `node_start` event to its active run.
    ///
    /// Merge-not-append (wire-audit hazard 2): `node_start` fires twice per
    /// step node — the re-emit carries `sessionId` — and the emission count is
    /// not fixed (the resume path skips the re-emit). Present fields replace,
    /// absent fields preserve, and a node is never duplicated.
    fn apply_node_started(&mut self, started: WorkflowNodeStarted) -> bool {
        let WorkflowNodeStartedParts {
            workflow_id,
            node_id,
            node_path,
            node_type,
            details,
        } = started.into_parts();
        let Some(run) = self.active_run_mut(&workflow_id, "node_start") else {
            return false;
        };
        let WorkflowNodeStartParts {
            agent_name,
            session_id,
            prompt,
            iteration,
            branch_id,
        } = details.into_parts();
        tally_node_lookup();
        let Some(node) = run.nodes.get_mut(&node_path) else {
            let node = WorkflowNodeState::from_opening(
                node_id, node_type, agent_name, session_id, prompt, iteration, branch_id,
            );
            let index_id = node.node_id().clone();
            run.nodes.insert(node_path.clone(), node);
            run.index_node(index_id, node_path);
            return true;
        };
        let moved_index =
            (node.node_id() != &node_id).then(|| (node.node_id().clone(), node_id.clone()));
        let mut changed = match &mut node.identity {
            WorkflowNodeIdentity::Opening {
                node_id: current_id,
                node_type: current_type,
                agent_name: current_agent,
            } => {
                let mut changed = replace(current_id, node_id);
                changed |= replace(current_type, node_type);
                changed |= replace_if_some(current_agent, agent_name);
                changed
            }
            WorkflowNodeIdentity::Snapshot {
                descriptor,
                event_agent_name,
            } => {
                if descriptor.node_type() == node_type {
                    let mut changed = descriptor.replace_node_id(node_id);
                    if let Some(agent_name) = agent_name
                        && event_agent_name
                            .as_deref()
                            .or_else(|| descriptor.agent_name())
                            != Some(agent_name.as_str())
                    {
                        *event_agent_name = Some(agent_name);
                        changed = true;
                    }
                    changed
                } else {
                    let preserved_agent = agent_name
                        .or_else(|| event_agent_name.take())
                        .or_else(|| descriptor.agent_name().map(str::to_owned));
                    node.identity = WorkflowNodeIdentity::Opening {
                        node_id,
                        node_type,
                        agent_name: preserved_agent,
                    };
                    true
                }
            }
        };
        changed |= replace_if_some(&mut node.session_id, session_id);
        changed |= replace_if_some(&mut node.prompt, prompt);
        changed |= replace_if_some(&mut node.iteration, iteration);
        changed |= replace_if_some(&mut node.branch_id, branch_id);
        if let Some((previous_id, current_id)) = moved_index {
            run.move_index(&previous_id, current_id, &node_path);
        }
        changed
    }

    fn apply_node_completed(&mut self, completed: WorkflowNodeCompleted) -> bool {
        let WorkflowNodeCompletedParts {
            workflow_id,
            node_path,
            status,
            details,
        } = completed.into_parts();
        let Some(run) = self.active_run_mut(&workflow_id, "node_complete") else {
            return false;
        };
        tally_node_lookup();
        let Some(node) = run.nodes.get_mut(&node_path) else {
            warn_ignored(&workflow_id, "node_complete", "unknown_node");
            return false;
        };
        let WorkflowNodeCompletionParts {
            artifacts,
            captured_output,
            failure_reason,
            completion_signal,
            completion_signal_source,
        } = details.into_parts();
        let mut changed = replace(&mut node.status, Some(status));
        changed |= replace_if_some(&mut node.artifacts, artifacts);
        changed |= replace_if_some(&mut node.captured_output, captured_output);
        changed |= replace_if_some(&mut node.failure_reason, failure_reason);
        changed |= replace_if_some(&mut node.completion_signal, completion_signal);
        changed |= replace_if_some(&mut node.completion_signal_source, completion_signal_source);
        changed
    }

    fn apply_node_paused(&mut self, paused: WorkflowNodePaused) -> bool {
        let (workflow_id, _node_id, node_path, reason) = paused.into_parts();
        let Some(run) = self.active_run_mut(&workflow_id, "node_paused") else {
            return false;
        };
        tally_node_lookup();
        let Some(node) = run.nodes.get_mut(&node_path) else {
            warn_ignored(&workflow_id, "node_paused", "unknown_node");
            return false;
        };
        replace(&mut node.status, Some(WorkflowNodeStatus::Paused))
            | replace(&mut node.node_pause_reason, Some(reason))
    }

    fn apply_loop_iteration(&mut self, iteration: WorkflowLoopIteration) -> bool {
        let (workflow_id, loop_id, value, stop_condition_met) = iteration.into_parts();
        let Some(run) = self.active_run_mut(&workflow_id, "loop_iteration") else {
            return false;
        };
        tally_id_bucket_lookup();
        let Some(paths) = run.node_index.get(&loop_id) else {
            warn_ignored(&workflow_id, "loop_iteration", "unknown_node");
            return false;
        };
        let mut matches = paths.iter().filter(|path| {
            tally_node_lookup();
            run.nodes
                .get(*path)
                .is_some_and(|node| node.node_type() == WorkflowNodeType::Repeat)
        });
        let Some(path) = matches.next().cloned() else {
            warn_ignored(&workflow_id, "loop_iteration", "unknown_node");
            return false;
        };
        if matches.next().is_some() {
            warn_ignored(&workflow_id, "loop_iteration", "ambiguous_repeat");
            return false;
        }
        tally_node_lookup();
        let Some(node) = run.nodes.get_mut(&path) else {
            // Structurally impossible today — the immutable probe above found
            // this path — but an index-maintenance regression must degrade to
            // a warned no-op, not panic the TUI on a wire frame.
            debug_assert!(
                false,
                "indexed workflow path {path:?} missing from the node map"
            );
            warn_ignored(&workflow_id, "loop_iteration", "index_desync");
            return false;
        };
        replace(
            &mut node.latest_loop_iteration,
            Some((value, stop_condition_met)),
        )
    }

    fn apply_watch_poll(&mut self, poll: WorkflowWatchPoll) -> bool {
        let (workflow_id, _node_id, node_path, outcome, at) = poll.into_parts();
        let Some(run) = self.active_run_mut(&workflow_id, "watch_poll") else {
            return false;
        };
        tally_node_lookup();
        let Some(node) = run.nodes.get_mut(&node_path) else {
            warn_ignored(&workflow_id, "watch_poll", "unknown_node");
            return false;
        };
        replace(&mut node.latest_watch_poll, Some((outcome, at)))
    }

    fn apply_paused(&mut self, paused: WorkflowPaused) -> bool {
        let (workflow_id, reason) = paused.into_parts();
        let Some(run) = self.active_run_mut(&workflow_id, "paused") else {
            return false;
        };
        replace(&mut run.status, Some(WorkflowRunStatus::Paused))
            | replace(&mut run.run_pause_reason, Some(reason))
    }

    /// A resolution-bearing frame is an acknowledgement only (D33, wire-audit
    /// hazard 3): its `pendingSteps` array — populated or empty — is discarded
    /// and the current pending descriptors survive. Only a resolution-free
    /// frame replaces pending work; an empty list there does not mean drained.
    fn apply_steps_queued(&mut self, queued: WorkflowStepsQueued) -> bool {
        let (workflow_id, pending_steps, resolution) = queued.into_parts();
        let Some(run) = self.active_run_mut(&workflow_id, "steps_queued") else {
            return false;
        };
        match resolution {
            Some(resolution) => replace(&mut run.queue_resolution, Some(resolution)),
            None => replace(&mut run.pending_steps, Some(pending_steps)),
        }
    }

    fn active_run_mut(
        &mut self,
        workflow_id: &WorkflowId,
        event_kind: &str,
    ) -> Option<&mut WorkflowRun> {
        tally_run_lookup();
        let Some(run) = self.runs.get_mut(workflow_id) else {
            warn_ignored(workflow_id, event_kind, "unknown_run");
            return None;
        };
        if is_terminal(run.status) {
            warn_ignored(workflow_id, event_kind, "post_terminal_event");
            return None;
        }
        Some(run)
    }

    fn apply_opening(&mut self, opening: WorkflowRunStarted) -> Result<bool, WorkflowStateError> {
        let WorkflowRunStartedParts {
            workflow_id,
            workflow_name,
            inputs,
            node_tree: opening_plan,
            parent_session_id,
        } = opening.into_parts();
        if let Some(current) = self.runs.get(&workflow_id) {
            if is_terminal(current.status) {
                return self.replace_if_changed(
                    workflow_id,
                    sparse_opening_run(workflow_name, inputs, opening_plan, parent_session_id),
                );
            }
            let exact_repeat = current.workflow_name == workflow_name
                && current.inputs == inputs
                && current.parent_session_id == parent_session_id
                && opening_plan_matches(current, &opening_plan);
            if exact_repeat {
                return Ok(false);
            }
            warn_ignored(&workflow_id, "run_start", "active_run_start_conflict");
            return Ok(false);
        }
        self.replace_if_changed(
            workflow_id,
            sparse_opening_run(workflow_name, inputs, opening_plan, parent_session_id),
        )
    }

    fn apply_completion(
        &mut self,
        completion: WorkflowRunCompleted,
    ) -> Result<bool, WorkflowStateError> {
        let workflow_id = completion.workflow_id().clone();
        let Some(current) = self.runs.get(&workflow_id) else {
            warn_ignored(&workflow_id, "run_complete", "unknown_run");
            return Ok(false);
        };
        if is_terminal(current.status) {
            // Absorbing state (D42): after a terminal incarnation every
            // non-exact completion is warned and ignored — including one
            // whose snapshot cannot even canonicalize, which by definition
            // cannot be the exact duplicate. Only the exact duplicate stays
            // a silent no-op.
            let Ok((_, mut incoming)) = canonicalize_snapshot(completion.into_final_state()) else {
                warn_ignored(&workflow_id, "run_complete", "terminal_completion_conflict");
                return Ok(false);
            };
            self.preserve_event_only(&workflow_id, &mut incoming);
            if current == &incoming {
                return Ok(false);
            }
            warn_ignored(&workflow_id, "run_complete", "terminal_completion_conflict");
            return Ok(false);
        }
        let (snapshot_id, mut incoming) = canonicalize_snapshot(completion.into_final_state())?;
        self.preserve_event_only(&workflow_id, &mut incoming);
        self.replace_if_changed(snapshot_id, incoming)
    }

    fn preserve_event_only(&self, workflow_id: &WorkflowId, incoming: &mut WorkflowRun) {
        let Some(current) = self.runs.get(workflow_id) else {
            return;
        };
        if incoming.opening_plan.is_none() {
            incoming.opening_plan.clone_from(&current.opening_plan);
        }
        incoming.pending_steps.clone_from(&current.pending_steps);
        incoming
            .queue_resolution
            .clone_from(&current.queue_resolution);
        incoming
            .run_pause_reason
            .clone_from(&current.run_pause_reason);
        for (path, node) in &mut incoming.nodes {
            let Some(previous) = current.nodes.get(path) else {
                continue;
            };
            node.prompt.clone_from(&previous.prompt);
            node.node_pause_reason
                .clone_from(&previous.node_pause_reason);
            node.latest_loop_iteration = previous.latest_loop_iteration;
            node.latest_watch_poll
                .clone_from(&previous.latest_watch_poll);
        }
    }

    fn replace_if_changed(
        &mut self,
        workflow_id: WorkflowId,
        run: WorkflowRun,
    ) -> Result<bool, WorkflowStateError> {
        if self.runs.get(&workflow_id) == Some(&run) {
            return Ok(false);
        }
        self.runs.insert(workflow_id, run);
        Ok(true)
    }

    /// Returns the run with the exact supplied identifier.
    pub fn get(&self, id: &WorkflowId) -> Option<&WorkflowRun> {
        self.runs.get(id)
    }

    /// Iterates every tracked run without allocating or imposing an order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&WorkflowId, &WorkflowRun)> {
        self.runs.iter()
    }
}

fn replace<T: PartialEq>(target: &mut T, incoming: T) -> bool {
    if *target == incoming {
        return false;
    }
    *target = incoming;
    true
}

fn replace_if_some<T: PartialEq>(target: &mut Option<T>, incoming: Option<T>) -> bool {
    match incoming {
        Some(incoming) => replace(target, Some(incoming)),
        None => false,
    }
}

fn opening_plan_matches(run: &WorkflowRun, incoming: &[WorkflowNodeDescriptor]) -> bool {
    match run.opening_plan.as_deref() {
        Some(opening_plan) => opening_plan == incoming,
        None => run
            .snapshot_plan
            .as_ref()
            .is_some_and(|snapshot_plan| snapshot_plan.children() == incoming),
    }
}

/// `Paused` is deliberately non-terminal (wire-audit hazard 1): `run_complete`
/// arrives with status `paused` on repeat exhaustion, and treating that
/// arrival as end-of-run tears down a live, resumable workflow.
fn is_terminal(status: Option<WorkflowRunStatus>) -> bool {
    matches!(
        status,
        Some(WorkflowRunStatus::Completed | WorkflowRunStatus::Failed | WorkflowRunStatus::Aborted)
    )
}

fn warn_ignored(workflow_id: &WorkflowId, event_kind: &str, reason: &str) {
    tracing::warn!(
        workflow_id = workflow_id.as_str(),
        event_kind,
        reason,
        "workflow event ignored"
    );
}

/// Inserts `path` into its `node_id` bucket, keeping the bucket sorted.
///
/// Every index construction path — incremental events via
/// [`WorkflowRun::index_node`] and bulk [`canonicalize_snapshot`] — must go
/// through here: `WorkflowRun` equality includes the index, so bucket order
/// must be a function of bucket *contents*, never arrival order (2026-08-10
/// review, finding SP8).
fn insert_indexed_path(
    index: &mut HashMap<WorkflowNodeId, Vec<WorkflowNodePath>>,
    node_id: WorkflowNodeId,
    path: WorkflowNodePath,
) {
    let bucket = index.entry(node_id).or_default();
    let position = bucket
        .binary_search(&path)
        .unwrap_or_else(|insert_at| insert_at);
    bucket.insert(position, path);
}

fn canonicalize_snapshot(
    snapshot: WorkflowSnapshot,
) -> Result<(WorkflowId, WorkflowRun), WorkflowStateError> {
    enum SnapshotTask {
        Visit(WorkflowNodeSnapshot, WorkflowNodePath),
        Finish(
            WorkflowNodeSnapshotParts,
            WorkflowNodePath,
            Vec<WorkflowNodePath>,
        ),
    }

    let WorkflowSnapshotParts {
        workflow_id,
        workflow_name,
        status,
        inputs,
        artifacts,
        captured_outputs,
        root,
        created_at,
        plan_revision,
        parent_session_id,
        workspace_path,
    } = snapshot.into_parts();
    let root_path = WorkflowNodePath::try_new(&workflow_id, vec![workflow_id.as_str().to_owned()])
        .map_err(|source| WorkflowStateError::InvalidCanonicalPath { source })?;
    let mut nodes = HashMap::new();
    let mut descriptors = HashMap::new();
    let mut node_index: HashMap<WorkflowNodeId, Vec<WorkflowNodePath>> = HashMap::new();
    let mut stack = vec![SnapshotTask::Visit(root, root_path.clone())];
    while let Some(task) = stack.pop() {
        match task {
            SnapshotTask::Visit(node, path) => {
                let mut parts = node.into_parts();
                let children = parts.take_children();
                let mut child_paths = Vec::with_capacity(children.len());
                let mut child_tasks = Vec::with_capacity(children.len());
                for child in children {
                    let segment = canonical_child_segment(parts.descriptor(), &child);
                    let mut segments = path.segments().to_vec();
                    segments.push(segment);
                    let child_path = WorkflowNodePath::try_new(&workflow_id, segments)
                        .map_err(|source| WorkflowStateError::InvalidCanonicalPath { source })?;
                    child_paths.push(child_path.clone());
                    child_tasks.push(SnapshotTask::Visit(child, child_path));
                }
                stack.push(SnapshotTask::Finish(parts, path, child_paths));
                stack.extend(child_tasks.into_iter().rev());
            }
            SnapshotTask::Finish(parts, path, child_paths) => {
                let child_descriptors = child_paths
                    .into_iter()
                    .map(|child_path| {
                        let Some(descriptor) = descriptors.remove(&child_path) else {
                            unreachable!("child descriptors finish before their parent");
                        };
                        descriptor
                    })
                    .collect();
                let descriptor = parts
                    .descriptor()
                    .clone()
                    .with_runtime_children(child_descriptors);
                let state = WorkflowNodeState::from_snapshot(parts);
                let node_id = state.node_id().clone();
                if nodes.insert(path.clone(), state).is_some() {
                    return Err(WorkflowStateError::DuplicateCanonicalPath { path });
                }
                insert_indexed_path(&mut node_index, node_id, path.clone());
                descriptors.insert(path, descriptor);
            }
        }
    }
    let Some(snapshot_plan) = descriptors.remove(&root_path) else {
        unreachable!("the root descriptor is assembled by a non-empty snapshot");
    };
    Ok((
        workflow_id,
        WorkflowRun {
            workflow_name,
            status: Some(status),
            inputs,
            artifacts: Some(artifacts),
            captured_outputs: Some(captured_outputs),
            created_at: Some(created_at),
            plan_revision: Some(plan_revision),
            parent_session_id,
            workspace_path,
            opening_plan: None,
            snapshot_plan: Some(snapshot_plan),
            pending_steps: None,
            queue_resolution: None,
            run_pause_reason: None,
            nodes,
            node_index,
        },
    ))
}

fn sparse_opening_run(
    workflow_name: String,
    inputs: serde_json::Value,
    opening_plan: Vec<WorkflowNodeDescriptor>,
    parent_session_id: Option<SessionId>,
) -> WorkflowRun {
    WorkflowRun {
        workflow_name,
        status: None,
        inputs,
        artifacts: None,
        captured_outputs: None,
        created_at: None,
        plan_revision: None,
        parent_session_id,
        workspace_path: None,
        opening_plan: Some(opening_plan),
        snapshot_plan: None,
        nodes: HashMap::new(),
        node_index: HashMap::new(),
        pending_steps: None,
        queue_resolution: None,
        run_pause_reason: None,
    }
}

/// Canonical path segment for a snapshot child — a verbatim port of the KAS
/// reference flattener (`H1n` in `kiro-cli-chat` 2.16.0, carved from the
/// binary):
///
/// ```js
/// function H1n(e,n){if(n){if(e.iteration!==void 0)return`iter-${e.iteration}`;
///   let t=/#(\d+)$/.exec(e.nodeId)?.[1];if(t!==void 0)return`iter-${t}`}return e.nodeId}
/// ```
///
/// `n` is exactly "parent is a repeat". A present `iteration` wins outright;
/// otherwise a trailing `#<ascii-digits>` rewrites with the digits verbatim
/// (leading zeros preserved — `#007` → `iter-007`); the child's type and the
/// parent's node id are never consulted. Anything narrower orphans repeat
/// subtrees whose streamed `nodePath` says `iter-N` (2026-08-09 review,
/// finding CR1, superseding the D21 discriminator).
fn canonical_child_segment(
    parent: &WorkflowNodeDescriptor,
    child: &WorkflowNodeSnapshot,
) -> String {
    let child_id = child.descriptor().node_id().as_str();
    if parent.node_type() == WorkflowNodeType::Repeat {
        if let Some(iteration) = child.iteration() {
            return format!("iter-{iteration}");
        }
        if let Some((_, digits)) = child_id.rsplit_once('#')
            && !digits.is_empty()
            && digits.bytes().all(|byte| byte.is_ascii_digit())
        {
            return format!("iter-{digits}");
        }
    }
    child_id.to_owned()
}

/// Test-only per-event lookup telemetry (design decision D4): result equality
/// cannot distinguish a keyed lookup from a full-map scan, so the two fenced
/// event shapes — ordinary `node_start` and pathless `loop_iteration` — tally
/// their map read-lookups and the scale fence bounds them per event. Bulk
/// paths (openings, snapshots, completions) are deliberately untallied; add
/// tallies before fencing any further shape.
#[cfg(test)]
mod lookup_telemetry {
    use std::cell::Cell;

    thread_local! {
        static RUN: Cell<u64> = const { Cell::new(0) };
        static NODE: Cell<u64> = const { Cell::new(0) };
        static ID_BUCKET: Cell<u64> = const { Cell::new(0) };
    }

    /// Zeroes all three counters on the current thread.
    pub(super) fn reset() {
        RUN.set(0);
        NODE.set(0);
        ID_BUCKET.set(0);
    }

    /// Returns `(run, node, id_bucket)` read-lookup counts since the last reset.
    pub(super) fn counts() -> (u64, u64, u64) {
        (RUN.get(), NODE.get(), ID_BUCKET.get())
    }

    pub(super) fn tally_run() {
        RUN.set(RUN.get() + 1);
    }

    pub(super) fn tally_node() {
        NODE.set(NODE.get() + 1);
    }

    pub(super) fn tally_id_bucket() {
        ID_BUCKET.set(ID_BUCKET.get() + 1);
    }
}

/// Records one run-map read lookup; a no-op outside test builds.
fn tally_run_lookup() {
    #[cfg(test)]
    lookup_telemetry::tally_run();
}

/// Records one node-map read lookup; a no-op outside test builds.
fn tally_node_lookup() {
    #[cfg(test)]
    lookup_telemetry::tally_node();
}

/// Records one node-id-index read lookup; a no-op outside test builds.
fn tally_id_bucket_lookup() {
    #[cfg(test)]
    lookup_telemetry::tally_id_bucket();
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::types::{
        SessionId, WorkflowCompletionStatus, WorkflowLoopIteration, WorkflowNodeCompleted,
        WorkflowNodeCompletionDetails, WorkflowNodeDescriptor, WorkflowNodePath,
        WorkflowNodePaused, WorkflowNodeSnapshot, WorkflowNodeStartDetails, WorkflowNodeStarted,
        WorkflowNodeStatus, WorkflowNodeType, WorkflowPaused, WorkflowQueueOutcome,
        WorkflowRepeatExhaustion, WorkflowRunCompleted, WorkflowRunStarted, WorkflowRunStatus,
        WorkflowSnapshot, WorkflowSnapshotData, WorkflowSnapshotMetadata, WorkflowStepsQueued,
        WorkflowWatchPoll,
    };

    fn workflow_id(value: &str) -> WorkflowId {
        match WorkflowId::try_from(value.to_owned()) {
            Ok(id) => id,
            Err(error) => panic!("invalid workflow id fixture: {error}"),
        }
    }

    fn node_id(value: &str) -> WorkflowNodeId {
        match WorkflowNodeId::try_from(value.to_owned()) {
            Ok(id) => id,
            Err(error) => panic!("invalid node id fixture: {error}"),
        }
    }

    fn node_path(workflow_id: &WorkflowId, segments: &[&str]) -> WorkflowNodePath {
        match WorkflowNodePath::try_new(
            workflow_id,
            segments
                .iter()
                .map(|segment| (*segment).to_owned())
                .collect(),
        ) {
            Ok(path) => path,
            Err(error) => panic!("invalid node path fixture: {error}"),
        }
    }

    fn with_captured_warnings<T>(f: impl FnOnce() -> T) -> (T, String) {
        let _capture_lock = crate::test_support::tracing_capture_lock();
        let capture = crate::test_support::CaptureWriter::default();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_ansi(false)
            .with_writer(capture.clone())
            .finish();
        let result = tracing::subscriber::with_default(subscriber, f);
        let logs = match String::from_utf8(capture.captured()) {
            Ok(logs) => logs,
            Err(error) => panic!("captured logs are not UTF-8: {error}"),
        };
        (result, logs)
    }

    fn step_node(id: &str) -> WorkflowNodeSnapshot {
        WorkflowNodeSnapshot::new(
            WorkflowNodeDescriptor::step(node_id(id), "agent".to_owned(), None, None),
            WorkflowNodeStatus::Completed,
            Vec::new(),
        )
    }

    fn snapshot_with_root(workflow_id: &str, root: WorkflowNodeSnapshot) -> WorkflowSnapshot {
        WorkflowSnapshot::new(
            self::workflow_id(workflow_id),
            "recipe".to_owned(),
            WorkflowRunStatus::Completed,
            WorkflowSnapshotData::new(
                serde_json::json!({"input": true}),
                serde_json::json!({"artifact": true}),
                serde_json::json!({"output": true}),
            ),
            root,
            WorkflowSnapshotMetadata::new("created".to_owned(), 1),
        )
    }

    fn snapshot_with_status(
        workflow_id: &str,
        status: WorkflowRunStatus,
        marker: &str,
    ) -> WorkflowSnapshot {
        let node_status = match status {
            WorkflowRunStatus::Running => WorkflowNodeStatus::Running,
            WorkflowRunStatus::Paused => WorkflowNodeStatus::Paused,
            WorkflowRunStatus::Completed => WorkflowNodeStatus::Completed,
            WorkflowRunStatus::Failed => WorkflowNodeStatus::Failed,
            WorkflowRunStatus::Aborted => WorkflowNodeStatus::Aborted,
        };
        WorkflowSnapshot::new(
            self::workflow_id(workflow_id),
            format!("recipe-{marker}"),
            status,
            WorkflowSnapshotData::new(
                serde_json::json!({"marker": marker}),
                serde_json::json!({}),
                serde_json::json!({}),
            ),
            WorkflowNodeSnapshot::new(
                WorkflowNodeDescriptor::sequence(node_id(workflow_id), Vec::new()),
                node_status,
                vec![WorkflowNodeSnapshot::new(
                    WorkflowNodeDescriptor::step(node_id(marker), "agent".to_owned(), None, None),
                    node_status,
                    Vec::new(),
                )],
            ),
            WorkflowSnapshotMetadata::new(format!("created-{marker}"), 1),
        )
    }

    fn completion(snapshot: WorkflowSnapshot) -> WorkflowEvent {
        let workflow_id = snapshot.workflow_id().clone();
        let status = match snapshot.status() {
            WorkflowRunStatus::Running => panic!("running is not a completion status"),
            WorkflowRunStatus::Paused => WorkflowCompletionStatus::Paused,
            WorkflowRunStatus::Completed => WorkflowCompletionStatus::Completed,
            WorkflowRunStatus::Failed => WorkflowCompletionStatus::Failed,
            WorkflowRunStatus::Aborted => WorkflowCompletionStatus::Aborted,
        };
        match WorkflowRunCompleted::new(workflow_id, status, snapshot) {
            Ok(completion) => WorkflowEvent::RunCompleted(completion),
            Err(error) => panic!("valid completion fixture rejected: {error}"),
        }
    }

    fn seed(tracker: &mut WorkflowTracker, id: &str, name: &str) {
        let previous = tracker.runs.insert(
            workflow_id(id),
            WorkflowRun {
                workflow_name: name.to_owned(),
                status: Some(WorkflowRunStatus::Running),
                inputs: serde_json::Value::Null,
                artifacts: None,
                captured_outputs: None,
                created_at: None,
                plan_revision: None,
                parent_session_id: None,
                workspace_path: None,
                opening_plan: None,
                snapshot_plan: None,
                pending_steps: None,
                queue_resolution: None,
                run_pause_reason: None,
                nodes: HashMap::new(),
                node_index: HashMap::new(),
            },
        );
        assert!(previous.is_none(), "test seed must not replace a run");
    }

    #[test]
    fn workflow_tracker_get_known_and_unknown() {
        let mut tracker = WorkflowTracker::new();
        for index in 0..64 {
            seed(
                &mut tracker,
                &format!("workflow-{index}"),
                &format!("recipe-{index}"),
            );
        }

        for (id, name) in [
            ("workflow-0", "recipe-0"),
            ("workflow-31", "recipe-31"),
            ("workflow-63", "recipe-63"),
        ] {
            assert_eq!(
                tracker
                    .get(&workflow_id(id))
                    .map(WorkflowRun::workflow_name),
                Some(name)
            );
        }
        assert!(tracker.get(&workflow_id("unknown")).is_none());

        let ids = (0..64)
            .map(|index| workflow_id(&format!("workflow-{index}")))
            .collect::<Vec<_>>();
        let started = Instant::now();
        for index in 0..100_000 {
            assert!(tracker.get(&ids[index % ids.len()]).is_some());
        }
        // CI-safe ceilings: these are complexity fences (a quadratic blowup
        // overshoots 5 s at this scale), not latency contracts — tight
        // millisecond budgets flake on loaded CI runners.
        assert!(
            started.elapsed() <= Duration::from_secs(5),
            "100,000 short-id lookups exceeded 5 s"
        );

        let large = workflow_id(&"x".repeat(65_536));
        let started = Instant::now();
        assert!(tracker.get(&large).is_none());
        assert!(
            started.elapsed() <= Duration::from_secs(5),
            "64 KiB workflow-id lookup exceeded 5 s"
        );
    }

    #[test]
    fn workflow_tracker_iter_empty_and_exact_size() {
        let mut tracker = WorkflowTracker::new();
        let empty = tracker.iter();
        assert_eq!(empty.len(), 0);
        assert_eq!(empty.count(), 0);

        for (id, name) in [
            ("workflow-a", "recipe-a"),
            ("workflow-b", "recipe-b"),
            ("workflow-c", "recipe-c"),
        ] {
            seed(&mut tracker, id, name);
        }
        let iter = tracker.iter();
        assert_eq!(iter.len(), 3);
        let mut actual = iter
            .map(|(id, run)| (id.as_str(), run.workflow_name()))
            .collect::<Vec<_>>();
        actual.sort_unstable();
        assert_eq!(
            actual,
            [
                ("workflow-a", "recipe-a"),
                ("workflow-b", "recipe-b"),
                ("workflow-c", "recipe-c")
            ]
        );
    }

    #[test]
    fn workflow_duplicate_canonical_path_rejected() {
        let duplicate_root = WorkflowNodeSnapshot::new(
            WorkflowNodeDescriptor::sequence(node_id("root"), Vec::new()),
            WorkflowNodeStatus::Completed,
            vec![step_node("duplicate"), step_node("duplicate")],
        );
        let mut tracker = WorkflowTracker::new();
        let result = tracker.apply_snapshot(snapshot_with_root("workflow", duplicate_root));
        assert!(matches!(
            result,
            Err(WorkflowStateError::DuplicateCanonicalPath { .. })
        ));
        assert!(tracker.get(&workflow_id("workflow")).is_none());
    }

    #[test]
    fn snapshot_root_and_child_paths_are_canonical() {
        let root = WorkflowNodeSnapshot::new(
            WorkflowNodeDescriptor::sequence(node_id("wire-root"), Vec::new()),
            WorkflowNodeStatus::Completed,
            vec![step_node("child")],
        );
        let id = workflow_id("workflow");
        let mut tracker = WorkflowTracker::new();
        assert_eq!(
            tracker.apply_snapshot(snapshot_with_root("workflow", root)),
            Ok(true)
        );
        let run = match tracker.get(&id) {
            Some(run) => run,
            None => panic!("snapshot did not seed run"),
        };
        assert!(run.node(&node_path(&id, &["workflow"])).is_some());
        assert!(run.node(&node_path(&id, &["workflow", "child"])).is_some());
        assert_eq!(run.nodes().len(), 2);
    }

    #[test]
    fn repeat_snapshot_paths_match_wire_paths() {
        let manifest: serde_json::Value = match serde_json::from_str(include_str!(
            "../tests/fixtures/kas/workflow/oracle-manifest.json"
        )) {
            Ok(value) => value,
            Err(error) => panic!("workflow manifest is invalid: {error}"),
        };
        let controls = match manifest["repeat_controls"].as_array() {
            Some(controls) => controls,
            None => panic!("repeat controls are not an array"),
        };
        assert_eq!(controls.len(), 9);
        for (index, control) in controls.iter().enumerate() {
            let child_id = match control["nodeId"].as_str() {
                Some(value) => value,
                None => panic!("repeat control lacks nodeId"),
            };
            let child_type = match control["type"].as_str() {
                Some("sequence") => WorkflowNodeType::Sequence,
                Some("step") => WorkflowNodeType::Step,
                Some(other) => panic!("unexpected repeat child type {other}"),
                None => panic!("repeat control lacks child type"),
            };
            let mut child = match child_type {
                WorkflowNodeType::Sequence => WorkflowNodeSnapshot::new(
                    WorkflowNodeDescriptor::sequence(node_id(child_id), Vec::new()),
                    WorkflowNodeStatus::Completed,
                    Vec::new(),
                ),
                WorkflowNodeType::Step => step_node(child_id),
                other => panic!("unsupported repeat child type {other:?}"),
            };
            if let Some(iteration) = control["iteration"].as_u64() {
                child = child.with_iteration(iteration as u32);
            }
            let parent_descriptor = match control["parentType"].as_str() {
                Some("repeat") => WorkflowNodeDescriptor::repeat(
                    node_id("loop"),
                    Vec::new(),
                    4,
                    WorkflowRepeatExhaustion::Pause,
                    None,
                    None,
                ),
                Some("sequence") => WorkflowNodeDescriptor::sequence(node_id("loop"), Vec::new()),
                other => panic!("unexpected repeat control parent type {other:?}"),
            };
            let parent = WorkflowNodeSnapshot::new(
                parent_descriptor,
                WorkflowNodeStatus::Completed,
                vec![child],
            );
            let root = WorkflowNodeSnapshot::new(
                WorkflowNodeDescriptor::sequence(node_id("root"), Vec::new()),
                WorkflowNodeStatus::Completed,
                vec![parent],
            );
            let workflow = format!("workflow-{index}");
            let workflow_id = workflow_id(&workflow);
            let mut tracker = WorkflowTracker::new();
            assert_eq!(
                tracker.apply_snapshot(snapshot_with_root(&workflow, root)),
                Ok(true)
            );
            let run = match tracker.get(&workflow_id) {
                Some(run) => run,
                None => panic!("repeat snapshot did not seed"),
            };
            let child_segment = match control["segment"].as_str() {
                Some(value) => value,
                None => panic!("repeat control lacks its expected segment"),
            };
            let path = node_path(&workflow_id, &[workflow.as_str(), "loop", child_segment]);
            let state = match run.node(&path) {
                Some(state) => state,
                None => panic!("repeat child missing at {path:?}"),
            };
            assert_eq!(state.node_id().as_str(), child_id);
            assert_eq!(
                state.iteration(),
                control["iteration"].as_u64().map(|value| value as u32)
            );
        }
    }

    #[test]
    fn snapshot_canonicalizer_is_atomic_at_design_scale() {
        let large_segment = "s".repeat(65_536);
        let mut chain = step_node(&large_segment);
        for depth in 1..9 {
            chain = WorkflowNodeSnapshot::new(
                WorkflowNodeDescriptor::sequence(node_id(&format!("chain-{depth}")), Vec::new()),
                WorkflowNodeStatus::Completed,
                vec![chain],
            );
        }
        let mut children = vec![chain];
        children.extend((0..246).map(|index| step_node(&format!("sibling-{index}"))));
        let root = WorkflowNodeSnapshot::new(
            WorkflowNodeDescriptor::sequence(node_id("root"), Vec::new()),
            WorkflowNodeStatus::Completed,
            children,
        );
        let snapshot = WorkflowSnapshot::new(
            workflow_id("workflow"),
            "recipe".to_owned(),
            WorkflowRunStatus::Completed,
            WorkflowSnapshotData::new(
                serde_json::Value::String("x".repeat(1_048_576)),
                serde_json::json!({}),
                serde_json::json!({}),
            ),
            root,
            WorkflowSnapshotMetadata::new("created".to_owned(), 1),
        );
        let mut tracker = WorkflowTracker::new();
        let started = Instant::now();
        assert_eq!(tracker.apply_snapshot(snapshot), Ok(true));
        assert!(
            started.elapsed() <= Duration::from_secs(5),
            "1 MiB/256-node/depth-10/64 KiB-segment snapshot exceeded 5 s"
        );
        let run = match tracker.get(&workflow_id("workflow")) {
            Some(run) => run,
            None => panic!("scale snapshot did not seed"),
        };
        assert_eq!(run.nodes().len(), 256);

        let deepest_duplicate = WorkflowNodeSnapshot::new(
            WorkflowNodeDescriptor::sequence(node_id("deep"), Vec::new()),
            WorkflowNodeStatus::Completed,
            vec![step_node("same"), step_node("same")],
        );
        let mut deep = deepest_duplicate;
        for depth in 0..10 {
            deep = WorkflowNodeSnapshot::new(
                WorkflowNodeDescriptor::sequence(node_id(&format!("outer-{depth}")), Vec::new()),
                WorkflowNodeStatus::Completed,
                vec![deep],
            );
        }
        let before = tracker.get(&workflow_id("workflow")).cloned();
        let result = tracker.apply_snapshot(snapshot_with_root("workflow", deep));
        assert!(matches!(
            result,
            Err(WorkflowStateError::DuplicateCanonicalPath { .. })
        ));
        assert_eq!(tracker.get(&workflow_id("workflow")), before.as_ref());
    }

    #[test]
    fn snapshot_entrypoint_status_matrix() {
        let statuses = [
            WorkflowRunStatus::Running,
            WorkflowRunStatus::Paused,
            WorkflowRunStatus::Completed,
            WorkflowRunStatus::Failed,
            WorkflowRunStatus::Aborted,
        ];
        for prior in [None].into_iter().chain(statuses.map(Some)) {
            for incoming in statuses {
                let mut tracker = WorkflowTracker::new();
                if let Some(prior) = prior {
                    assert_eq!(
                        tracker.apply_snapshot(snapshot_with_status("workflow", prior, "before")),
                        Ok(true)
                    );
                }
                let before = tracker.get(&workflow_id("workflow")).cloned();
                let result =
                    tracker.apply_snapshot(snapshot_with_status("workflow", incoming, "after"));
                if prior.is_some_and(|status| is_terminal(Some(status))) && prior != Some(incoming)
                {
                    assert!(matches!(
                        result,
                        Err(WorkflowStateError::TerminalSnapshotConflict { .. })
                    ));
                    assert_eq!(tracker.get(&workflow_id("workflow")), before.as_ref());
                } else {
                    assert_eq!(result, Ok(true));
                    assert_eq!(
                        tracker
                            .get(&workflow_id("workflow"))
                            .map(WorkflowRun::status),
                        Some(Some(incoming))
                    );
                }
            }
        }

        for prior in [None].into_iter().chain(statuses.map(Some)) {
            for incoming in statuses
                .into_iter()
                .filter(|status| *status != WorkflowRunStatus::Running)
            {
                let mut tracker = WorkflowTracker::new();
                if let Some(prior) = prior {
                    assert_eq!(
                        tracker.apply_snapshot(snapshot_with_status("workflow", prior, "before")),
                        Ok(true)
                    );
                }
                let before = tracker.get(&workflow_id("workflow")).cloned();
                let result = tracker.apply_event(completion(snapshot_with_status(
                    "workflow", incoming, "after",
                )));
                if prior.is_some_and(|status| !is_terminal(Some(status))) {
                    assert_eq!(result, Ok(true));
                    assert_eq!(
                        tracker
                            .get(&workflow_id("workflow"))
                            .map(WorkflowRun::status),
                        Some(Some(incoming))
                    );
                } else {
                    assert_eq!(result, Ok(false));
                    assert_eq!(tracker.get(&workflow_id("workflow")), before.as_ref());
                }
            }
        }
    }

    #[test]
    fn snapshot_entry_paths_are_equivalent() {
        let mut direct = WorkflowTracker::new();
        let mut completion_path = WorkflowTracker::new();
        for tracker in [&mut direct, &mut completion_path] {
            assert_eq!(
                tracker.apply_snapshot(snapshot_with_status(
                    "workflow",
                    WorkflowRunStatus::Running,
                    "before",
                )),
                Ok(true)
            );
        }
        let terminal = snapshot_with_status("workflow", WorkflowRunStatus::Completed, "terminal");
        assert_eq!(direct.apply_snapshot(terminal.clone()), Ok(true));
        assert_eq!(completion_path.apply_event(completion(terminal)), Ok(true));
        assert_eq!(
            direct.get(&workflow_id("workflow")),
            completion_path.get(&workflow_id("workflow"))
        );
    }

    #[test]
    fn active_snapshot_can_become_terminal() {
        for prior in [WorkflowRunStatus::Running, WorkflowRunStatus::Paused] {
            for terminal in [
                WorkflowRunStatus::Completed,
                WorkflowRunStatus::Failed,
                WorkflowRunStatus::Aborted,
            ] {
                let mut tracker = WorkflowTracker::new();
                assert_eq!(
                    tracker.apply_snapshot(snapshot_with_status("workflow", prior, "before")),
                    Ok(true)
                );
                assert_eq!(
                    tracker.apply_event(completion(snapshot_with_status(
                        "workflow", terminal, "after",
                    ))),
                    Ok(true)
                );
                assert_eq!(
                    tracker
                        .get(&workflow_id("workflow"))
                        .map(WorkflowRun::status),
                    Some(Some(terminal))
                );
            }
        }
    }

    #[test]
    fn invalid_snapshot_is_atomic() {
        let mut tracker = WorkflowTracker::new();
        assert_eq!(
            tracker.apply_snapshot(snapshot_with_status(
                "workflow",
                WorkflowRunStatus::Running,
                "before",
            )),
            Ok(true)
        );
        let before = tracker.get(&workflow_id("workflow")).cloned();
        let duplicate = WorkflowNodeSnapshot::new(
            WorkflowNodeDescriptor::sequence(node_id("workflow"), Vec::new()),
            WorkflowNodeStatus::Running,
            vec![
                WorkflowNodeSnapshot::new(
                    WorkflowNodeDescriptor::step(node_id("same"), "a".to_owned(), None, None),
                    WorkflowNodeStatus::Running,
                    Vec::new(),
                ),
                WorkflowNodeSnapshot::new(
                    WorkflowNodeDescriptor::step(node_id("same"), "b".to_owned(), None, None),
                    WorkflowNodeStatus::Running,
                    Vec::new(),
                ),
            ],
        );
        let invalid = WorkflowSnapshot::new(
            workflow_id("workflow"),
            "invalid".to_owned(),
            WorkflowRunStatus::Running,
            WorkflowSnapshotData::new(
                serde_json::json!({}),
                serde_json::json!({}),
                serde_json::json!({}),
            ),
            duplicate,
            WorkflowSnapshotMetadata::new("created".to_owned(), 0),
        );
        assert!(matches!(
            tracker.apply_snapshot(invalid),
            Err(WorkflowStateError::DuplicateCanonicalPath { .. })
        ));
        assert_eq!(tracker.get(&workflow_id("workflow")), before.as_ref());
    }

    #[test]
    fn terminal_snapshot_conflict_is_atomic() {
        let mut tracker = WorkflowTracker::new();
        assert_eq!(
            tracker.apply_snapshot(snapshot_with_status(
                "workflow",
                WorkflowRunStatus::Completed,
                "before",
            )),
            Ok(true)
        );
        let before = tracker.get(&workflow_id("workflow")).cloned();
        let result = tracker.apply_snapshot(snapshot_with_status(
            "workflow",
            WorkflowRunStatus::Running,
            "after",
        ));
        assert!(matches!(
            result,
            Err(WorkflowStateError::TerminalSnapshotConflict {
                current: WorkflowRunStatus::Completed,
                incoming: WorkflowRunStatus::Running,
                ..
            })
        ));
        assert_eq!(tracker.get(&workflow_id("workflow")), before.as_ref());
    }

    #[test]
    fn exact_terminal_completion_is_idempotent() {
        let snapshot = snapshot_with_status("workflow", WorkflowRunStatus::Completed, "terminal");
        let mut tracker = WorkflowTracker::new();
        assert_eq!(tracker.apply_snapshot(snapshot.clone()), Ok(true));
        assert_eq!(tracker.apply_event(completion(snapshot)), Ok(false));
    }

    #[test]
    fn snapshot_field_ownership_matrix() {
        let workflow_id = workflow_id("workflow");
        let child_path = node_path(&workflow_id, &["workflow", "step"]);
        let rich_child = WorkflowNodeSnapshot::new(
            WorkflowNodeDescriptor::step(node_id("step"), "agent".to_owned(), None, None),
            WorkflowNodeStatus::Running,
            Vec::new(),
        )
        .with_session_id(SessionId::new("session"))
        .with_artifacts(serde_json::json!({"artifact": true}))
        .with_captured_output(serde_json::json!({"output": true}))
        .with_failure_reason("reason".to_owned())
        .with_iteration(7)
        .with_branch_id("branch".to_owned())
        .with_completion_signal(WorkflowCompletionSignal::Success)
        .with_completion_signal_source(WorkflowCompletionSignalSource::SendMessage)
        .with_started_at("start".to_owned())
        .with_ended_at("end".to_owned())
        .with_watch_cursor(serde_json::json!({"seen": ["comment"]}))
        .with_watch_terminal(serde_json::Value::Bool(true));
        let rich = WorkflowSnapshot::new(
            workflow_id.clone(),
            "recipe".to_owned(),
            WorkflowRunStatus::Running,
            WorkflowSnapshotData::new(
                serde_json::json!({"input": true}),
                serde_json::json!({"artifact": true}),
                serde_json::json!({"output": true}),
            ),
            WorkflowNodeSnapshot::new(
                WorkflowNodeDescriptor::sequence(node_id("workflow"), Vec::new()),
                WorkflowNodeStatus::Running,
                vec![rich_child],
            ),
            WorkflowSnapshotMetadata::new("created".to_owned(), 1)
                .with_parent_session_id(SessionId::new("parent"))
                .with_workspace_path("/workspace".to_owned()),
        );
        let mut tracker = WorkflowTracker::new();
        assert_eq!(tracker.apply_snapshot(rich), Ok(true));
        let Some(rich_run) = tracker.get(&workflow_id) else {
            panic!("rich snapshot did not seed");
        };
        let Some(rich_node) = rich_run.node(&child_path) else {
            panic!("rich child missing");
        };
        assert_eq!(
            rich_node.watch_cursor(),
            Some(&serde_json::json!({"seen": ["comment"]}))
        );
        assert_eq!(
            rich_node.watch_terminal(),
            Some(&serde_json::Value::Bool(true))
        );
        let Some(run) = tracker.runs.get_mut(&workflow_id) else {
            panic!("rich snapshot did not seed");
        };
        run.pending_steps = Some(vec![WorkflowNodeDescriptor::step(
            node_id("pending"),
            "agent".to_owned(),
            None,
            None,
        )]);
        run.queue_resolution = Some(WorkflowQueueResolution::new(
            WorkflowQueueOutcome::Applied,
            Some("accepted".to_owned()),
        ));
        run.run_pause_reason = Some("operator".to_owned());
        let Some(node) = run.nodes.get_mut(&child_path) else {
            panic!("rich child missing");
        };
        node.prompt = Some("prompt".to_owned());
        node.node_pause_reason = Some("need-human".to_owned());
        node.latest_loop_iteration = Some((2, true));
        node.latest_watch_poll = Some((WorkflowWatchOutcome::Idle, "t1".to_owned()));

        let sparse = WorkflowSnapshot::new(
            workflow_id.clone(),
            "recipe".to_owned(),
            WorkflowRunStatus::Running,
            WorkflowSnapshotData::new(
                serde_json::json!({"input": false}),
                serde_json::json!({}),
                serde_json::json!({}),
            ),
            WorkflowNodeSnapshot::new(
                WorkflowNodeDescriptor::sequence(node_id("workflow"), Vec::new()),
                WorkflowNodeStatus::Running,
                vec![WorkflowNodeSnapshot::new(
                    WorkflowNodeDescriptor::step(node_id("step"), "agent".to_owned(), None, None),
                    WorkflowNodeStatus::Running,
                    Vec::new(),
                )],
            ),
            WorkflowSnapshotMetadata::new("created-2".to_owned(), 2),
        );
        assert_eq!(tracker.apply_snapshot(sparse), Ok(true));
        let Some(run) = tracker.get(&workflow_id) else {
            panic!("reconciled run missing");
        };
        assert!(run.parent_session_id().is_none());
        assert!(run.workspace_path().is_none());
        assert_eq!(run.pending_steps().map(<[_]>::len), Some(1));
        assert_eq!(
            run.queue_resolution().map(WorkflowQueueResolution::outcome),
            Some(WorkflowQueueOutcome::Applied)
        );
        assert_eq!(run.run_pause_reason(), Some("operator"));
        let Some(node) = run.node(&child_path) else {
            panic!("reconciled child missing");
        };
        assert!(node.session_id().is_none());
        assert!(node.artifacts().is_none());
        assert!(node.captured_output().is_none());
        assert!(node.failure_reason().is_none());
        assert!(node.iteration().is_none());
        assert!(node.branch_id().is_none());
        assert!(node.completion_signal().is_none());
        assert!(node.completion_signal_source().is_none());
        assert!(node.started_at().is_none());
        assert!(node.ended_at().is_none());
        assert!(node.watch_cursor().is_none());
        assert!(node.watch_terminal().is_none());
        assert_eq!(node.prompt(), Some("prompt"));
        assert_eq!(node.node_pause_reason(), Some("need-human"));
        assert_eq!(node.latest_loop_iteration(), Some((2, true)));
        assert_eq!(
            node.latest_watch_poll(),
            Some((WorkflowWatchOutcome::Idle, "t1"))
        );

        let manifest: serde_json::Value = match serde_json::from_str(include_str!(
            "../tests/fixtures/kas/workflow/oracle-manifest.json"
        )) {
            Ok(value) => value,
            Err(error) => panic!("oracle manifest is invalid: {error}"),
        };
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
            manifest["event_only_fields"],
            serde_json::json!([
                "prompt",
                "pendingSteps",
                "queueResolution",
                "runPauseReason",
                "nodePauseReason",
                "latestLoopIteration",
                "latestWatchPoll"
            ])
        );
    }

    #[test]
    fn active_run_start_conflict_presence_matrix() {
        fn opening(
            name: &str,
            inputs: serde_json::Value,
            tree: Vec<WorkflowNodeDescriptor>,
            parent: Option<SessionId>,
        ) -> WorkflowEvent {
            WorkflowEvent::RunStarted(WorkflowRunStarted::new(
                workflow_id("workflow"),
                name.to_owned(),
                inputs,
                tree,
                parent,
            ))
        }

        let base_tree = vec![WorkflowNodeDescriptor::step(
            node_id("step"),
            "agent".to_owned(),
            None,
            None,
        )];
        let base = opening(
            "recipe",
            serde_json::json!({"seed": 1}),
            base_tree.clone(),
            None,
        );
        let conflicts = [
            opening(
                "other",
                serde_json::json!({"seed": 1}),
                base_tree.clone(),
                None,
            ),
            opening(
                "recipe",
                serde_json::json!({"seed": 2}),
                base_tree.clone(),
                None,
            ),
            opening("recipe", serde_json::json!({"seed": 1}), Vec::new(), None),
            opening(
                "recipe",
                serde_json::json!({"seed": 1}),
                base_tree.clone(),
                Some(SessionId::new("parent")),
            ),
        ];
        for conflict in conflicts {
            let mut tracker = WorkflowTracker::new();
            assert_eq!(tracker.apply_event(base.clone()), Ok(true));
            assert_eq!(tracker.apply_event(base.clone()), Ok(false));
            let before = tracker.get(&workflow_id("workflow")).cloned();
            assert_eq!(tracker.apply_event(conflict), Ok(false));
            assert_eq!(tracker.get(&workflow_id("workflow")), before.as_ref());
            assert_eq!(tracker.iter().len(), 1);
        }

        let parent_base = opening(
            "recipe",
            serde_json::json!({"seed": 1}),
            base_tree.clone(),
            Some(SessionId::new("parent-a")),
        );
        for conflict in [
            opening(
                "recipe",
                serde_json::json!({"seed": 1}),
                base_tree.clone(),
                None,
            ),
            opening(
                "recipe",
                serde_json::json!({"seed": 1}),
                base_tree,
                Some(SessionId::new("parent-b")),
            ),
        ] {
            let mut tracker = WorkflowTracker::new();
            assert_eq!(tracker.apply_event(parent_base.clone()), Ok(true));
            let before = tracker.get(&workflow_id("workflow")).cloned();
            let (result, logs) = with_captured_warnings(|| tracker.apply_event(conflict));
            assert_eq!(result, Ok(false));
            assert_eq!(tracker.get(&workflow_id("workflow")), before.as_ref());
            assert!(
                logs.contains("active_run_start_conflict"),
                "conflicting active run_start must warn, got:\n{logs}"
            );
        }
    }

    #[test]
    fn snapshot_reconciled_node_start_preserves_descriptor_metadata() {
        let id = workflow_id("workflow");
        let path = node_path(&id, &["workflow", "step"]);
        let snapshot = WorkflowSnapshot::new(
            id.clone(),
            "recipe".to_owned(),
            WorkflowRunStatus::Running,
            WorkflowSnapshotData::new(
                serde_json::json!({"seed": 1}),
                serde_json::json!({}),
                serde_json::json!({}),
            ),
            WorkflowNodeSnapshot::new(
                WorkflowNodeDescriptor::sequence(node_id("workflow"), Vec::new()),
                WorkflowNodeStatus::Running,
                vec![WorkflowNodeSnapshot::new(
                    WorkflowNodeDescriptor::step(
                        node_id("step"),
                        "snapshot-agent".to_owned(),
                        Some("snapshot-model".to_owned()),
                        Some("snapshot-effort".to_owned()),
                    ),
                    WorkflowNodeStatus::Running,
                    Vec::new(),
                )],
            ),
            WorkflowSnapshotMetadata::new("created".to_owned(), 1),
        );
        let mut tracker = WorkflowTracker::new();
        assert_eq!(tracker.apply_snapshot(snapshot), Ok(true));

        assert_eq!(
            tracker.apply_event(WorkflowEvent::NodeStarted(WorkflowNodeStarted::new(
                id.clone(),
                node_id("step"),
                path.clone(),
                WorkflowNodeType::Step,
                WorkflowNodeStartDetails::new()
                    .with_agent_name("event-agent".to_owned())
                    .with_session_id(SessionId::new("session")),
            ))),
            Ok(true)
        );
        let Some(node) = tracker.get(&id).and_then(|run| run.node(&path)) else {
            panic!("snapshot node must survive a partial node_start");
        };
        let Some(descriptor) = node.descriptor() else {
            panic!("snapshot descriptor must survive a partial node_start");
        };
        assert_eq!(node.agent_name(), Some("event-agent"));
        assert_eq!(descriptor.model_id(), Some("snapshot-model"));
        assert_eq!(descriptor.effort_level(), Some("snapshot-effort"));
    }

    #[test]
    fn snapshot_reconciled_active_run_start_exact_repeat_is_silent() {
        let id = workflow_id("workflow");
        let step = WorkflowNodeDescriptor::step(
            node_id("step"),
            "agent".to_owned(),
            Some("model".to_owned()),
            Some("effort".to_owned()),
        );
        let opening = WorkflowEvent::RunStarted(WorkflowRunStarted::new(
            id.clone(),
            "recipe".to_owned(),
            serde_json::json!({"seed": 1}),
            vec![step.clone()],
            None,
        ));
        let mut tracker = WorkflowTracker::new();
        assert_eq!(tracker.apply_event(opening.clone()), Ok(true));
        assert_eq!(
            tracker.apply_snapshot(WorkflowSnapshot::new(
                id.clone(),
                "recipe".to_owned(),
                WorkflowRunStatus::Paused,
                WorkflowSnapshotData::new(
                    serde_json::json!({"seed": 1}),
                    serde_json::json!({}),
                    serde_json::json!({}),
                ),
                WorkflowNodeSnapshot::new(
                    WorkflowNodeDescriptor::sequence(node_id("workflow"), Vec::new()),
                    WorkflowNodeStatus::Paused,
                    vec![WorkflowNodeSnapshot::new(
                        step,
                        WorkflowNodeStatus::Paused,
                        Vec::new(),
                    )],
                ),
                WorkflowSnapshotMetadata::new("created".to_owned(), 1),
            )),
            Ok(true)
        );
        let before = tracker.get(&id).cloned();

        let (result, logs) = with_captured_warnings(|| tracker.apply_event(opening));

        assert_eq!(result, Ok(false));
        assert_eq!(tracker.get(&id), before.as_ref());
        assert!(
            !logs.contains("active_run_start_conflict"),
            "exact replay must not warn as a conflict, got:\n{logs}"
        );
    }

    #[test]
    fn node_start_merge_presence_matrix() {
        let id = workflow_id("workflow");
        let path = node_path(&id, &["workflow", "node"]);
        let mut tracker = WorkflowTracker::new();
        assert_eq!(
            tracker.apply_event(WorkflowEvent::RunStarted(WorkflowRunStarted::new(
                id.clone(),
                "recipe".to_owned(),
                serde_json::json!({}),
                Vec::new(),
                None,
            ))),
            Ok(true)
        );
        assert_eq!(
            tracker.apply_event(WorkflowEvent::NodeStarted(WorkflowNodeStarted::new(
                id.clone(),
                node_id("old"),
                path.clone(),
                WorkflowNodeType::Step,
                WorkflowNodeStartDetails::new()
                    .with_agent_name("agent-a".to_owned())
                    .with_session_id(SessionId::new("session-a"))
                    .with_prompt("prompt-a".to_owned())
                    .with_iteration(3)
                    .with_branch_id("branch-a".to_owned()),
            ))),
            Ok(true)
        );
        assert_eq!(
            tracker.apply_event(WorkflowEvent::NodeStarted(WorkflowNodeStarted::new(
                id.clone(),
                node_id("new"),
                path.clone(),
                WorkflowNodeType::Watch,
                WorkflowNodeStartDetails::new(),
            ))),
            Ok(true)
        );
        let Some(node) = tracker.get(&id).and_then(|run| run.node(&path)) else {
            panic!("merged node missing");
        };
        assert_eq!(node.node_id().as_str(), "new");
        assert_eq!(node.node_type(), WorkflowNodeType::Watch);
        assert_eq!(node.agent_name(), Some("agent-a"));
        assert_eq!(node.session_id().map(SessionId::as_str), Some("session-a"));
        assert_eq!(node.prompt(), Some("prompt-a"));
        assert_eq!(node.iteration(), Some(3));
        assert_eq!(node.branch_id(), Some("branch-a"));
    }

    #[test]
    fn node_complete_merge_presence_matrix() {
        let id = workflow_id("workflow");
        let path = node_path(&id, &["workflow", "node"]);
        let mut tracker = WorkflowTracker::new();
        assert_eq!(
            tracker.apply_event(WorkflowEvent::RunStarted(WorkflowRunStarted::new(
                id.clone(),
                "recipe".to_owned(),
                serde_json::json!({}),
                Vec::new(),
                None,
            ))),
            Ok(true)
        );
        assert_eq!(
            tracker.apply_event(WorkflowEvent::NodeStarted(WorkflowNodeStarted::new(
                id.clone(),
                node_id("node"),
                path.clone(),
                WorkflowNodeType::Step,
                WorkflowNodeStartDetails::new(),
            ))),
            Ok(true)
        );
        let details = WorkflowNodeCompletionDetails::new()
            .with_artifacts(serde_json::json!({"artifact": 1}))
            .with_captured_output(serde_json::json!({"output": 2}))
            .with_failure_reason("first".to_owned())
            .with_completion_signal(WorkflowCompletionSignal::Success)
            .with_completion_signal_source(WorkflowCompletionSignalSource::SendMessage);
        assert_eq!(
            tracker.apply_event(WorkflowEvent::NodeCompleted(WorkflowNodeCompleted::new(
                id.clone(),
                node_id("node"),
                path.clone(),
                WorkflowNodeStatus::Failed,
                details,
            ))),
            Ok(true)
        );
        assert_eq!(
            tracker.apply_event(WorkflowEvent::NodeCompleted(WorkflowNodeCompleted::new(
                id.clone(),
                node_id("ignored-on-path-update"),
                path.clone(),
                WorkflowNodeStatus::Completed,
                WorkflowNodeCompletionDetails::new(),
            ))),
            Ok(true)
        );
        let Some(node) = tracker.get(&id).and_then(|run| run.node(&path)) else {
            panic!("completed node missing");
        };
        assert_eq!(node.status(), Some(WorkflowNodeStatus::Completed));
        assert_eq!(node.artifacts(), Some(&serde_json::json!({"artifact": 1})));
        assert_eq!(
            node.captured_output(),
            Some(&serde_json::json!({"output": 2}))
        );
        assert_eq!(node.failure_reason(), Some("first"));
        assert_eq!(
            node.completion_signal(),
            Some(WorkflowCompletionSignal::Success)
        );
        assert_eq!(
            node.completion_signal_source(),
            Some(WorkflowCompletionSignalSource::SendMessage)
        );
    }

    #[test]
    fn node_index_bucket_cardinality_matrix() {
        let id = workflow_id("workflow");
        let first_path = node_path(&id, &["workflow", "first"]);
        let second_path = node_path(&id, &["workflow", "second"]);
        let mut tracker = WorkflowTracker::new();
        assert_eq!(
            tracker.apply_event(WorkflowEvent::RunStarted(WorkflowRunStarted::new(
                id.clone(),
                "recipe".to_owned(),
                serde_json::json!({}),
                Vec::new(),
                None,
            ))),
            Ok(true)
        );
        for path in [&first_path, &second_path] {
            assert_eq!(
                tracker.apply_event(WorkflowEvent::NodeStarted(WorkflowNodeStarted::new(
                    id.clone(),
                    node_id("shared"),
                    path.clone(),
                    WorkflowNodeType::Step,
                    WorkflowNodeStartDetails::new(),
                ))),
                Ok(true)
            );
        }
        assert_eq!(
            tracker.apply_event(WorkflowEvent::NodeStarted(WorkflowNodeStarted::new(
                id.clone(),
                node_id("moved"),
                first_path.clone(),
                WorkflowNodeType::Step,
                WorkflowNodeStartDetails::new(),
            ))),
            Ok(true)
        );
        let Some(run) = tracker.get(&id) else {
            panic!("run missing");
        };
        assert_eq!(
            run.node_index.get(&node_id("shared")),
            Some(&vec![second_path])
        );
        assert_eq!(
            run.node_index.get(&node_id("moved")),
            Some(&vec![first_path])
        );
    }

    #[test]
    fn queue_resolution_and_pending_matrix() {
        let id = workflow_id("workflow");
        let pending =
            WorkflowNodeDescriptor::step(node_id("pending"), "agent".to_owned(), None, None);
        let mut tracker = WorkflowTracker::new();
        assert_eq!(
            tracker.apply_event(WorkflowEvent::RunStarted(WorkflowRunStarted::new(
                id.clone(),
                "recipe".to_owned(),
                serde_json::json!({}),
                Vec::new(),
                None,
            ))),
            Ok(true)
        );
        assert_eq!(
            tracker.apply_event(WorkflowEvent::StepsQueued(WorkflowStepsQueued::new(
                id.clone(),
                vec![pending.clone()],
                None,
            ))),
            Ok(true)
        );
        assert_eq!(
            tracker.apply_event(WorkflowEvent::StepsQueued(WorkflowStepsQueued::new(
                id.clone(),
                Vec::new(),
                Some(WorkflowQueueResolution::new(
                    WorkflowQueueOutcome::Applied,
                    Some("approved".to_owned()),
                )),
            ))),
            Ok(true)
        );
        let Some(run) = tracker.get(&id) else {
            panic!("queued run missing");
        };
        assert_eq!(run.pending_steps(), Some(std::slice::from_ref(&pending)));
        assert_eq!(
            run.queue_resolution().map(WorkflowQueueResolution::outcome),
            Some(WorkflowQueueOutcome::Applied)
        );
        assert_eq!(
            run.queue_resolution()
                .and_then(WorkflowQueueResolution::reason),
            Some("approved")
        );
        assert_eq!(
            tracker.apply_event(WorkflowEvent::StepsQueued(WorkflowStepsQueued::new(
                id.clone(),
                Vec::new(),
                None,
            ))),
            Ok(true)
        );
        let Some(run) = tracker.get(&id) else {
            panic!("cleared run missing");
        };
        assert_eq!(run.pending_steps(), Some(&[][..]));
        assert_eq!(
            run.queue_resolution().map(WorkflowQueueResolution::outcome),
            Some(WorkflowQueueOutcome::Applied)
        );

        // Full acknowledgement cross (2026-08-10 review, finding SP12):
        // every outcome × reason-presence × array-cardinality acknowledgement
        // preserves the pending list while recording exactly its resolution;
        // resolution-free frames replace pending work at both cardinalities.
        let decoy = WorkflowNodeDescriptor::step(node_id("decoy"), "agent".to_owned(), None, None);
        for outcome in [
            WorkflowQueueOutcome::Applied,
            WorkflowQueueOutcome::Rejected,
            WorkflowQueueOutcome::Dropped,
        ] {
            for reason in [None, Some("why".to_owned())] {
                for ack_steps in [Vec::new(), vec![decoy.clone()]] {
                    assert!(
                        tracker
                            .apply_event(WorkflowEvent::StepsQueued(WorkflowStepsQueued::new(
                                id.clone(),
                                vec![pending.clone()],
                                None,
                            )))
                            .is_ok(),
                        "pending reset must apply"
                    );
                    assert!(
                        tracker
                            .apply_event(WorkflowEvent::StepsQueued(WorkflowStepsQueued::new(
                                id.clone(),
                                ack_steps.clone(),
                                Some(WorkflowQueueResolution::new(outcome, reason.clone())),
                            )))
                            .is_ok(),
                        "acknowledgement must apply"
                    );
                    let Some(run) = tracker.get(&id) else {
                        panic!("acknowledged run missing");
                    };
                    assert_eq!(
                        run.pending_steps(),
                        Some(std::slice::from_ref(&pending)),
                        "{outcome}/{reason:?}/{}-element ack must preserve pending work",
                        ack_steps.len()
                    );
                    assert_eq!(
                        run.queue_resolution().map(WorkflowQueueResolution::outcome),
                        Some(outcome)
                    );
                    assert_eq!(
                        run.queue_resolution()
                            .and_then(WorkflowQueueResolution::reason),
                        reason.as_deref()
                    );
                }
            }
        }
        assert!(
            tracker
                .apply_event(WorkflowEvent::StepsQueued(WorkflowStepsQueued::new(
                    id.clone(),
                    vec![decoy.clone()],
                    None,
                )))
                .is_ok(),
            "non-empty resolution-free frame must apply"
        );
        let Some(run) = tracker.get(&id) else {
            panic!("replaced run missing");
        };
        assert_eq!(
            run.pending_steps(),
            Some(std::slice::from_ref(&decoy)),
            "resolution-free non-empty frame replaces pending work"
        );
    }

    #[test]
    fn workflow_progress_fields_are_independent_and_current() {
        let id = workflow_id("workflow");
        let repeat_path = node_path(&id, &["workflow", "repeat"]);
        let watch_path = node_path(&id, &["workflow", "watch"]);
        let step_path = node_path(&id, &["workflow", "step"]);
        let mut tracker = WorkflowTracker::new();
        assert_eq!(
            tracker.apply_event(WorkflowEvent::RunStarted(WorkflowRunStarted::new(
                id.clone(),
                "recipe".to_owned(),
                serde_json::json!({}),
                Vec::new(),
                None,
            ))),
            Ok(true)
        );
        for (node, path, node_type) in [
            ("repeat", repeat_path.clone(), WorkflowNodeType::Repeat),
            ("watch", watch_path.clone(), WorkflowNodeType::Watch),
            ("step", step_path.clone(), WorkflowNodeType::Step),
        ] {
            assert_eq!(
                tracker.apply_event(WorkflowEvent::NodeStarted(WorkflowNodeStarted::new(
                    id.clone(),
                    node_id(node),
                    path,
                    node_type,
                    WorkflowNodeStartDetails::new(),
                ))),
                Ok(true)
            );
        }
        for event in [
            WorkflowEvent::Paused(WorkflowPaused::new(id.clone(), "operator".to_owned())),
            WorkflowEvent::NodePaused(WorkflowNodePaused::new(
                id.clone(),
                node_id("step"),
                step_path.clone(),
                "review".to_owned(),
            )),
            WorkflowEvent::LoopIteration(WorkflowLoopIteration::new(
                id.clone(),
                node_id("repeat"),
                1,
                false,
            )),
            WorkflowEvent::LoopIteration(WorkflowLoopIteration::new(
                id.clone(),
                node_id("repeat"),
                2,
                true,
            )),
            WorkflowEvent::WatchPoll(WorkflowWatchPoll::new(
                id.clone(),
                node_id("watch"),
                watch_path.clone(),
                WorkflowWatchOutcome::NewActivity,
                "t1".to_owned(),
            )),
            WorkflowEvent::WatchPoll(WorkflowWatchPoll::new(
                id.clone(),
                node_id("watch"),
                watch_path.clone(),
                WorkflowWatchOutcome::Idle,
                "t2".to_owned(),
            )),
        ] {
            assert_eq!(tracker.apply_event(event), Ok(true));
        }
        let Some(run) = tracker.get(&id) else {
            panic!("progress run missing");
        };
        assert_eq!(run.status(), Some(WorkflowRunStatus::Paused));
        assert_eq!(run.run_pause_reason(), Some("operator"));
        assert_eq!(
            run.node(&step_path)
                .and_then(WorkflowNodeState::node_pause_reason),
            Some("review")
        );
        assert_eq!(
            run.node(&repeat_path)
                .and_then(WorkflowNodeState::latest_loop_iteration),
            Some((2, true))
        );
        assert_eq!(
            run.node(&watch_path)
                .and_then(WorkflowNodeState::latest_watch_poll),
            Some((WorkflowWatchOutcome::Idle, "t2"))
        );
    }

    #[test]
    fn unknown_workflow_event_matrix_no_placeholders() {
        let id = workflow_id("unknown");
        let path = node_path(&id, &["unknown", "node"]);
        let events = [
            WorkflowEvent::NodeStarted(WorkflowNodeStarted::new(
                id.clone(),
                node_id("node"),
                path.clone(),
                WorkflowNodeType::Step,
                WorkflowNodeStartDetails::new(),
            )),
            WorkflowEvent::NodeCompleted(WorkflowNodeCompleted::new(
                id.clone(),
                node_id("node"),
                path.clone(),
                WorkflowNodeStatus::Completed,
                WorkflowNodeCompletionDetails::new(),
            )),
            WorkflowEvent::NodePaused(WorkflowNodePaused::new(
                id.clone(),
                node_id("node"),
                path.clone(),
                "pause".to_owned(),
            )),
            WorkflowEvent::LoopIteration(WorkflowLoopIteration::new(
                id.clone(),
                node_id("node"),
                1,
                false,
            )),
            WorkflowEvent::WatchPoll(WorkflowWatchPoll::new(
                id.clone(),
                node_id("node"),
                path,
                WorkflowWatchOutcome::Idle,
                "t1".to_owned(),
            )),
            WorkflowEvent::Paused(WorkflowPaused::new(id.clone(), "pause".to_owned())),
            completion(snapshot_with_status(
                "unknown",
                WorkflowRunStatus::Completed,
                "terminal",
            )),
            WorkflowEvent::StepsQueued(WorkflowStepsQueued::new(id, Vec::new(), None)),
        ];
        let mut tracker = WorkflowTracker::new();
        for event in events {
            let (result, logs) = with_captured_warnings(|| tracker.apply_event(event));
            assert_eq!(result, Ok(false));
            assert_eq!(tracker.iter().len(), 0);
            assert!(
                logs.contains("unknown_run"),
                "pre-opening event must warn, got:\n{logs}"
            );
        }
    }

    #[test]
    fn unknown_node_update_matrix_no_placeholders() {
        let id = workflow_id("workflow");
        let path = node_path(&id, &["workflow", "missing"]);
        let mut tracker = WorkflowTracker::new();
        assert_eq!(
            tracker.apply_event(WorkflowEvent::RunStarted(WorkflowRunStarted::new(
                id.clone(),
                "recipe".to_owned(),
                serde_json::json!({}),
                Vec::new(),
                None,
            ))),
            Ok(true)
        );
        for event in [
            WorkflowEvent::NodeCompleted(WorkflowNodeCompleted::new(
                id.clone(),
                node_id("missing"),
                path.clone(),
                WorkflowNodeStatus::Completed,
                WorkflowNodeCompletionDetails::new(),
            )),
            WorkflowEvent::NodePaused(WorkflowNodePaused::new(
                id.clone(),
                node_id("missing"),
                path.clone(),
                "pause".to_owned(),
            )),
            WorkflowEvent::WatchPoll(WorkflowWatchPoll::new(
                id.clone(),
                node_id("missing"),
                path,
                WorkflowWatchOutcome::Idle,
                "t1".to_owned(),
            )),
            WorkflowEvent::LoopIteration(WorkflowLoopIteration::new(
                id.clone(),
                node_id("missing"),
                1,
                false,
            )),
        ] {
            let (result, logs) = with_captured_warnings(|| tracker.apply_event(event));
            assert_eq!(result, Ok(false));
            assert_eq!(tracker.get(&id).map(|run| run.nodes().len()), Some(0));
            assert!(
                logs.contains("unknown_node"),
                "unknown node event must warn, got:\n{logs}"
            );
        }
    }

    #[test]
    fn loop_lookup_filters_type_and_rejects_ambiguity() {
        let id = workflow_id("workflow");
        let step_path = node_path(&id, &["workflow", "step"]);
        let first_repeat_path = node_path(&id, &["workflow", "repeat-a"]);
        let second_repeat_path = node_path(&id, &["workflow", "repeat-b"]);
        let mut tracker = WorkflowTracker::new();
        assert_eq!(
            tracker.apply_event(WorkflowEvent::RunStarted(WorkflowRunStarted::new(
                id.clone(),
                "recipe".to_owned(),
                serde_json::json!({}),
                Vec::new(),
                None,
            ))),
            Ok(true)
        );
        for (path, node_type) in [
            (step_path, WorkflowNodeType::Step),
            (first_repeat_path.clone(), WorkflowNodeType::Repeat),
        ] {
            assert_eq!(
                tracker.apply_event(WorkflowEvent::NodeStarted(WorkflowNodeStarted::new(
                    id.clone(),
                    node_id("shared"),
                    path,
                    node_type,
                    WorkflowNodeStartDetails::new(),
                ))),
                Ok(true)
            );
        }
        assert_eq!(
            tracker.apply_event(WorkflowEvent::LoopIteration(WorkflowLoopIteration::new(
                id.clone(),
                node_id("shared"),
                1,
                false
            ))),
            Ok(true)
        );
        assert_eq!(
            tracker
                .get(&id)
                .and_then(|run| run.node(&first_repeat_path))
                .and_then(WorkflowNodeState::latest_loop_iteration),
            Some((1, false))
        );
        assert_eq!(
            tracker.apply_event(WorkflowEvent::NodeStarted(WorkflowNodeStarted::new(
                id.clone(),
                node_id("shared"),
                second_repeat_path.clone(),
                WorkflowNodeType::Repeat,
                WorkflowNodeStartDetails::new(),
            ))),
            Ok(true)
        );
        let event = WorkflowEvent::LoopIteration(WorkflowLoopIteration::new(
            id.clone(),
            node_id("shared"),
            2,
            true,
        ));
        let (result, logs) = with_captured_warnings(|| tracker.apply_event(event));
        assert_eq!(result, Ok(false));
        assert!(
            logs.contains("ambiguous_repeat"),
            "ambiguous repeat update must warn, got:\n{logs}"
        );
        assert_eq!(
            tracker
                .get(&id)
                .and_then(|run| run.node(&first_repeat_path))
                .and_then(WorkflowNodeState::latest_loop_iteration),
            Some((1, false))
        );
        assert_eq!(
            tracker
                .get(&id)
                .and_then(|run| run.node(&second_repeat_path))
                .and_then(WorkflowNodeState::latest_loop_iteration),
            None
        );
    }

    #[test]
    fn workflow_completion_metadata_never_controls_status() {
        let run_statuses = [
            WorkflowRunStatus::Paused,
            WorkflowRunStatus::Completed,
            WorkflowRunStatus::Failed,
            WorkflowRunStatus::Aborted,
        ];
        let node_statuses = [
            WorkflowNodeStatus::Pending,
            WorkflowNodeStatus::Running,
            WorkflowNodeStatus::Paused,
            WorkflowNodeStatus::Completed,
            WorkflowNodeStatus::Failed,
            WorkflowNodeStatus::Aborted,
            WorkflowNodeStatus::Skipped,
        ];
        let signals = [
            None,
            Some(WorkflowCompletionSignal::Success),
            Some(WorkflowCompletionSignal::NeedInput),
            Some(WorkflowCompletionSignal::Error),
        ];
        let sources = [
            None,
            Some(WorkflowCompletionSignalSource::SendMessage),
            Some(WorkflowCompletionSignalSource::StatusUpdate),
        ];
        for run_status in run_statuses {
            for node_status in node_statuses {
                for signal in signals {
                    for source in sources {
                        let mut child = WorkflowNodeSnapshot::new(
                            WorkflowNodeDescriptor::step(
                                node_id("child"),
                                "agent".to_owned(),
                                None,
                                None,
                            ),
                            node_status,
                            Vec::new(),
                        );
                        if let Some(signal) = signal {
                            child = child.with_completion_signal(signal);
                        }
                        if let Some(source) = source {
                            child = child.with_completion_signal_source(source);
                        }
                        let snapshot = WorkflowSnapshot::new(
                            workflow_id("workflow"),
                            "recipe".to_owned(),
                            run_status,
                            WorkflowSnapshotData::new(
                                serde_json::json!({}),
                                serde_json::json!({}),
                                serde_json::json!({}),
                            ),
                            WorkflowNodeSnapshot::new(
                                WorkflowNodeDescriptor::sequence(node_id("workflow"), Vec::new()),
                                WorkflowNodeStatus::Running,
                                vec![child],
                            ),
                            WorkflowSnapshotMetadata::new("created".to_owned(), 0),
                        );
                        let mut tracker = WorkflowTracker::new();
                        assert_eq!(tracker.apply_snapshot(snapshot), Ok(true));
                        let id = workflow_id("workflow");
                        let path = node_path(&id, &["workflow", "child"]);
                        let Some(run) = tracker.get(&id) else {
                            panic!("metadata run missing");
                        };
                        assert_eq!(run.status(), Some(run_status));
                        let Some(node) = run.node(&path) else {
                            panic!("metadata node missing");
                        };
                        assert_eq!(node.status(), Some(node_status));
                        assert_eq!(node.completion_signal(), signal);
                        assert_eq!(node.completion_signal_source(), source);
                    }
                }
            }
        }
    }

    /// REGRESSION FENCE (2026-08-10 review, finding SP8): node-id index
    /// buckets stay sorted through insert-and-remove churn. `WorkflowRun`
    /// equality includes the index, so an order-perturbing removal (the old
    /// `swap_remove`) would make a semantically identical snapshot compare
    /// unequal and report a phantom change.
    #[test]
    fn node_index_buckets_stay_sorted_through_moves() {
        let id = workflow_id("workflow");
        let mut tracker = WorkflowTracker::new();
        assert_eq!(
            tracker.apply_event(WorkflowEvent::RunStarted(WorkflowRunStarted::new(
                id.clone(),
                "recipe".to_owned(),
                serde_json::json!({}),
                Vec::new(),
                None,
            ))),
            Ok(true)
        );
        for segment in ["a", "b", "c"] {
            assert_eq!(
                tracker.apply_event(WorkflowEvent::NodeStarted(WorkflowNodeStarted::new(
                    id.clone(),
                    node_id("shared"),
                    node_path(&id, &["workflow", segment]),
                    WorkflowNodeType::Step,
                    WorkflowNodeStartDetails::new(),
                ))),
                Ok(true)
            );
        }
        // Retire the FIRST path from the shared bucket via a changed-id
        // node_start — swap_remove would migrate the last entry into slot 0.
        assert_eq!(
            tracker.apply_event(WorkflowEvent::NodeStarted(WorkflowNodeStarted::new(
                id.clone(),
                node_id("moved"),
                node_path(&id, &["workflow", "a"]),
                WorkflowNodeType::Step,
                WorkflowNodeStartDetails::new(),
            ))),
            Ok(true)
        );
        let Some(run) = tracker.get(&id) else {
            panic!("run missing");
        };
        for (bucket_id, paths) in &run.node_index {
            assert!(
                paths.is_sorted(),
                "bucket {bucket_id:?} must stay sorted, got {paths:?}"
            );
        }
        let Some(shared) = run.node_index.get(&node_id("shared")) else {
            panic!("shared bucket missing");
        };
        assert_eq!(
            shared,
            &vec![
                node_path(&id, &["workflow", "b"]),
                node_path(&id, &["workflow", "c"]),
            ],
            "surviving paths keep canonical order"
        );
    }

    /// REGRESSION FENCE (2026-08-10 ultrareview, segment 3): snapshot
    /// canonicalization builds the node-id index through the same sorted
    /// insert as the events path. An ancestor and a descendant may legally
    /// share a node id (dedup checks canonical *paths*, not ids), and DFS
    /// finish order emits the descendant's longer path first — an unsorted
    /// push would order the bucket by arrival, making an event-built run and
    /// its canonicalized snapshot compare unequal (the SP8 phantom-change
    /// failure mode).
    #[test]
    fn snapshot_canonicalization_keeps_index_buckets_sorted() {
        let id = workflow_id("workflow");
        let root = WorkflowNodeSnapshot::new(
            WorkflowNodeDescriptor::sequence(node_id("workflow"), Vec::new()),
            WorkflowNodeStatus::Completed,
            vec![step_node("workflow")],
        );
        let mut tracker = WorkflowTracker::new();
        assert_eq!(
            tracker.apply_snapshot(snapshot_with_root("workflow", root)),
            Ok(true)
        );
        let Some(run) = tracker.get(&id) else {
            panic!("run missing");
        };
        for (bucket_id, paths) in &run.node_index {
            assert!(
                paths.is_sorted(),
                "bucket {bucket_id:?} must be sorted after canonicalization, got {paths:?}"
            );
        }
        let Some(shared) = run.node_index.get(&node_id("workflow")) else {
            panic!("shared bucket missing");
        };
        assert_eq!(
            shared,
            &vec![
                node_path(&id, &["workflow"]),
                node_path(&id, &["workflow", "workflow"]),
            ],
            "the ancestor's shorter path sorts before its descendant's"
        );
    }

    /// REGRESSION FENCE (2026-08-10 review, finding SP7): a post-terminal
    /// completion whose snapshot cannot even canonicalize (duplicate
    /// canonical path) is absorbed like every other non-exact post-terminal
    /// completion — warned and ignored, never surfaced as a state error.
    #[test]
    fn post_terminal_completion_with_invalid_snapshot_is_absorbed() {
        for status in [
            WorkflowRunStatus::Completed,
            WorkflowRunStatus::Failed,
            WorkflowRunStatus::Aborted,
        ] {
            let id = workflow_id("workflow");
            let mut tracker = WorkflowTracker::new();
            assert_eq!(
                tracker.apply_snapshot(snapshot_with_status("workflow", status, "before")),
                Ok(true)
            );
            let before = tracker.get(&id).cloned();
            let duplicate_root = WorkflowNodeSnapshot::new(
                WorkflowNodeDescriptor::sequence(node_id("workflow"), Vec::new()),
                WorkflowNodeStatus::Completed,
                vec![step_node("dup"), step_node("dup")],
            );
            let invalid = WorkflowSnapshot::new(
                id.clone(),
                "recipe".to_owned(),
                status,
                WorkflowSnapshotData::new(
                    serde_json::json!({"input": true}),
                    serde_json::json!({"artifact": true}),
                    serde_json::json!({"output": true}),
                ),
                duplicate_root,
                WorkflowSnapshotMetadata::new("created".to_owned(), 1),
            );
            let (result, logs) =
                with_captured_warnings(|| tracker.apply_event(completion(invalid)));
            assert_eq!(result, Ok(false), "{status}: absorbed, never Err");
            assert_eq!(
                tracker.get(&id),
                before.as_ref(),
                "{status}: state unchanged"
            );
            assert!(
                logs.contains("terminal_completion_conflict"),
                "{status}: absorbed invalid completion must warn, got:\n{logs}"
            );
        }
    }

    #[test]
    fn post_terminal_event_matrix_is_absorbing() {
        for status in [
            WorkflowRunStatus::Completed,
            WorkflowRunStatus::Failed,
            WorkflowRunStatus::Aborted,
        ] {
            let id = workflow_id("workflow");
            let path = node_path(&id, &["workflow", "before"]);
            let events = [
                WorkflowEvent::NodeStarted(WorkflowNodeStarted::new(
                    id.clone(),
                    node_id("new"),
                    path.clone(),
                    WorkflowNodeType::Step,
                    WorkflowNodeStartDetails::new(),
                )),
                WorkflowEvent::NodeCompleted(WorkflowNodeCompleted::new(
                    id.clone(),
                    node_id("before"),
                    path.clone(),
                    WorkflowNodeStatus::Completed,
                    WorkflowNodeCompletionDetails::new(),
                )),
                WorkflowEvent::NodePaused(WorkflowNodePaused::new(
                    id.clone(),
                    node_id("before"),
                    path.clone(),
                    "pause".to_owned(),
                )),
                WorkflowEvent::LoopIteration(WorkflowLoopIteration::new(
                    id.clone(),
                    node_id("before"),
                    1,
                    false,
                )),
                WorkflowEvent::WatchPoll(WorkflowWatchPoll::new(
                    id.clone(),
                    node_id("before"),
                    path,
                    WorkflowWatchOutcome::Idle,
                    "t1".to_owned(),
                )),
                WorkflowEvent::Paused(WorkflowPaused::new(id.clone(), "pause".to_owned())),
                WorkflowEvent::StepsQueued(WorkflowStepsQueued::new(id.clone(), Vec::new(), None)),
                completion(snapshot_with_status("workflow", status, "conflict")),
            ];
            for event in events {
                let original = snapshot_with_status("workflow", status, "before");
                let mut tracker = WorkflowTracker::new();
                assert_eq!(tracker.apply_snapshot(original), Ok(true));
                let before = tracker.get(&id).cloned();
                let (result, logs) = with_captured_warnings(|| tracker.apply_event(event));
                assert_eq!(result, Ok(false));
                assert_eq!(tracker.get(&id), before.as_ref());
                assert!(
                    logs.contains("post_terminal_event")
                        || logs.contains("terminal_completion_conflict"),
                    "post-terminal event must warn, got:\n{logs}"
                );
            }
            let exact = snapshot_with_status("workflow", status, "exact");
            let mut tracker = WorkflowTracker::new();
            assert_eq!(tracker.apply_snapshot(exact.clone()), Ok(true));
            let before = tracker.get(&id).cloned();
            let (result, logs) = with_captured_warnings(|| tracker.apply_event(completion(exact)));
            assert_eq!(result, Ok(false));
            assert_eq!(tracker.get(&id), before.as_ref());
            assert!(
                logs.is_empty(),
                "exact terminal duplicate must be silent, got:\n{logs}"
            );
        }
    }

    #[test]
    fn retry_opening_clears_full_prior_incarnation() {
        let id = workflow_id("workflow");
        let old_path = node_path(&id, &["workflow", "old"]);
        let mut tracker = WorkflowTracker::new();
        assert_eq!(
            tracker.apply_snapshot(snapshot_with_status(
                "workflow",
                WorkflowRunStatus::Completed,
                "old",
            )),
            Ok(true)
        );
        let Some(old) = tracker.runs.get_mut(&id) else {
            panic!("old incarnation missing");
        };
        old.pending_steps = Some(vec![WorkflowNodeDescriptor::step(
            node_id("pending"),
            "agent".to_owned(),
            None,
            None,
        )]);
        old.queue_resolution = Some(WorkflowQueueResolution::new(
            WorkflowQueueOutcome::Applied,
            Some("approved".to_owned()),
        ));
        old.run_pause_reason = Some("pause".to_owned());
        let Some(old_node) = old.nodes.get_mut(&old_path) else {
            panic!("old node missing");
        };
        old_node.prompt = Some("old prompt".to_owned());
        old_node.node_pause_reason = Some("old pause".to_owned());
        old_node.latest_loop_iteration = Some((9, true));
        old_node.latest_watch_poll = Some((WorkflowWatchOutcome::Idle, "old".to_owned()));
        // Claim-10 completeness (2026-08-10 review, finding SP13): the prior
        // incarnation also carries completion metadata and a SHARED node-id
        // index bucket, so the reset must drop those families too.
        old_node.completion_signal = Some(WorkflowCompletionSignal::Success);
        old_node.completion_signal_source = Some(WorkflowCompletionSignalSource::SendMessage);
        old_node.failure_reason = Some("old failure".to_owned());
        let twin_path = node_path(&id, &["workflow", "old-twin"]);
        let twin = WorkflowNodeState::from_opening(
            node_id("old"),
            crate::types::WorkflowNodeType::Step,
            None,
            None,
            None,
            None,
            None,
        );
        old.nodes.insert(twin_path.clone(), twin);
        old.index_node(node_id("old"), twin_path);
        assert!(
            old.node_index
                .get(&node_id("old"))
                .is_some_and(|bucket| bucket.len() == 2),
            "prior incarnation must hold a shared index bucket"
        );

        let new_tree = vec![WorkflowNodeDescriptor::step(
            node_id("fresh"),
            "fresh-agent".to_owned(),
            None,
            None,
        )];
        assert_eq!(
            tracker.apply_event(WorkflowEvent::RunStarted(WorkflowRunStarted::new(
                id.clone(),
                "new-recipe".to_owned(),
                serde_json::json!({"new": true}),
                new_tree.clone(),
                Some(SessionId::new("new-parent")),
            ))),
            Ok(true)
        );
        let Some(run) = tracker.get(&id) else {
            panic!("new incarnation missing");
        };
        assert_eq!(run.workflow_name(), "new-recipe");
        assert_eq!(run.inputs(), &serde_json::json!({"new": true}));
        assert_eq!(
            run.parent_session_id().map(SessionId::as_str),
            Some("new-parent")
        );
        assert_eq!(run.opening_plan(), Some(new_tree.as_slice()));
        assert!(run.status().is_none());
        assert!(run.artifacts().is_none());
        assert!(run.captured_outputs().is_none());
        assert!(run.created_at().is_none());
        assert!(run.plan_revision().is_none());
        assert!(run.workspace_path().is_none());
        assert!(run.snapshot_plan().is_none());
        assert!(run.pending_steps().is_none());
        assert!(run.queue_resolution().is_none());
        assert!(run.run_pause_reason().is_none());
        assert_eq!(run.nodes().len(), 0);
        assert!(run.node_index.is_empty());
    }

    #[test]
    fn workflow_plan_names_its_authority() {
        let id = workflow_id("workflow");
        let tree = vec![WorkflowNodeDescriptor::step(
            node_id("step"),
            "agent".to_owned(),
            None,
            None,
        )];
        let mut tracker = WorkflowTracker::new();
        assert_eq!(
            tracker.apply_event(WorkflowEvent::RunStarted(WorkflowRunStarted::new(
                id.clone(),
                "recipe".to_owned(),
                serde_json::json!({}),
                tree.clone(),
                None,
            ))),
            Ok(true)
        );
        let Some(run) = tracker.get(&id) else {
            panic!("opening did not seed");
        };
        assert_eq!(run.plan(), Some(WorkflowPlan::Opening(tree.as_slice())));

        assert_eq!(
            tracker.apply_snapshot(snapshot_with_status(
                "workflow",
                WorkflowRunStatus::Running,
                "snap"
            )),
            Ok(true)
        );
        let Some(run) = tracker.get(&id) else {
            panic!("snapshot did not reconcile");
        };
        let Some(WorkflowPlan::Snapshot(descriptor)) = run.plan() else {
            panic!("snapshot must own the plan, got {:?}", run.plan());
        };
        assert_eq!(Some(descriptor), run.snapshot_plan());
        assert_eq!(run.opening_plan(), None);
    }

    #[test]
    fn post_terminal_run_start_replaces_incarnation() {
        for status in [
            WorkflowRunStatus::Completed,
            WorkflowRunStatus::Failed,
            WorkflowRunStatus::Aborted,
        ] {
            let id = workflow_id("workflow");
            let mut tracker = WorkflowTracker::new();
            assert_eq!(
                tracker.apply_snapshot(snapshot_with_status("workflow", status, "old")),
                Ok(true)
            );
            assert_eq!(
                tracker.apply_event(WorkflowEvent::RunStarted(WorkflowRunStarted::new(
                    id.clone(),
                    "new".to_owned(),
                    serde_json::json!({"incarnation": 2}),
                    Vec::new(),
                    None,
                ))),
                Ok(true)
            );
            let Some(run) = tracker.get(&id) else {
                panic!("replacement incarnation missing");
            };
            assert_eq!(run.workflow_name(), "new");
            assert_eq!(run.status(), None);
            assert_eq!(run.nodes().len(), 0);
        }
    }

    #[test]
    fn workflow_tracker_scale_and_isolation() {
        fn fnv1a(rows: &[String]) -> u64 {
            let mut digest = 0xcbf2_9ce4_8422_2325_u64;
            for byte in rows.iter().flat_map(|row| row.as_bytes()) {
                digest ^= u64::from(*byte);
                digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
            }
            digest
        }

        let manifest: serde_json::Value = match serde_json::from_str(include_str!(
            "../tests/fixtures/kas/workflow/oracle-manifest.json"
        )) {
            Ok(value) => value,
            Err(error) => panic!("workflow manifest is invalid: {error}"),
        };
        let runs = manifest["scale"]["runs"].as_u64().unwrap_or_else(|| {
            panic!("scale.runs must be an integer");
        }) as usize;
        let nodes_per_run = manifest["scale"]["nodes_per_run"]
            .as_u64()
            .unwrap_or_else(|| {
                panic!("scale.nodes_per_run must be an integer");
            }) as usize;
        let events_per_node = manifest["scale"]["events_per_node"]
            .as_u64()
            .unwrap_or_else(|| {
                panic!("scale.events_per_node must be an integer");
            }) as usize;
        let lookup_bound = |kind: &str, map: &str| -> u64 {
            manifest["scale"][kind][map].as_u64().unwrap_or_else(|| {
                panic!("scale.{kind}.{map} must be an integer");
            })
        };
        let ordinary = (
            lookup_bound("ordinary_lookup_bound", "run"),
            lookup_bound("ordinary_lookup_bound", "node"),
            lookup_bound("ordinary_lookup_bound", "id_bucket"),
        );
        let pathless = (
            lookup_bound("pathless_lookup_bound", "run"),
            lookup_bound("pathless_lookup_bound", "node"),
            lookup_bound("pathless_lookup_bound", "id_bucket"),
        );
        let started = Instant::now();
        let mut tracker = WorkflowTracker::new();
        let mut event_count = 0_usize;
        for run_index in 0..runs {
            let workflow_name = format!("workflow-{run_index}");
            let id = workflow_id(&workflow_name);
            assert_eq!(
                tracker.apply_event(WorkflowEvent::RunStarted(WorkflowRunStarted::new(
                    id.clone(),
                    "recipe".to_owned(),
                    serde_json::json!({}),
                    Vec::new(),
                    None,
                ))),
                Ok(true)
            );
        }
        for repeat in 0..events_per_node {
            for run_index in 0..runs {
                let workflow_name = format!("workflow-{run_index}");
                let id = workflow_id(&workflow_name);
                for node_index in 0..nodes_per_run {
                    let node_name = format!("node-{node_index}");
                    let path = node_path(&id, &[workflow_name.as_str(), node_name.as_str()]);
                    lookup_telemetry::reset();
                    assert_eq!(
                        tracker.apply_event(WorkflowEvent::NodeStarted(WorkflowNodeStarted::new(
                            id.clone(),
                            node_id(&node_name),
                            path,
                            WorkflowNodeType::Step,
                            WorkflowNodeStartDetails::new(),
                        ))),
                        Ok(repeat == 0)
                    );
                    let counts = lookup_telemetry::counts();
                    assert!(
                        counts.0 <= ordinary.0 && counts.1 <= ordinary.1 && counts.2 <= ordinary.2,
                        "ordinary node_start exceeded its per-event lookup bound: \
                         {counts:?} > {ordinary:?}"
                    );
                    event_count += 1;
                }
            }
        }
        assert_eq!(
            event_count,
            manifest["scale"]["events"].as_u64().unwrap_or_else(|| {
                panic!("scale.events must be an integer");
            }) as usize
        );
        assert_eq!(tracker.iter().len(), runs);
        let node_count = tracker
            .iter()
            .map(|(_, run)| run.nodes().len())
            .sum::<usize>();
        assert_eq!(node_count, runs * nodes_per_run);
        let mut rows = tracker
            .iter()
            .flat_map(|(workflow_id, run)| {
                run.nodes().map(move |(path, node)| {
                    format!(
                        "{}|{}|{}|{}\n",
                        workflow_id.as_str(),
                        path.segments().join("/"),
                        node.node_id().as_str(),
                        node.node_type().as_str()
                    )
                })
            })
            .collect::<Vec<_>>();
        rows.sort_unstable();
        assert_eq!(fnv1a(&rows), 0x34b8_4227_7d78_e315);
        assert!(
            started.elapsed() <= Duration::from_secs(5),
            "163,840-event isolation fixture exceeded 5 s"
        );

        for run_index in 0..runs {
            let workflow_name = format!("workflow-{run_index}");
            let id = workflow_id(&workflow_name);
            let loop_name = "scale-repeat";
            let path = node_path(&id, &[workflow_name.as_str(), loop_name]);
            assert_eq!(
                tracker.apply_event(WorkflowEvent::NodeStarted(WorkflowNodeStarted::new(
                    id.clone(),
                    node_id(loop_name),
                    path,
                    WorkflowNodeType::Repeat,
                    WorkflowNodeStartDetails::new(),
                ))),
                Ok(true)
            );
            lookup_telemetry::reset();
            assert_eq!(
                tracker.apply_event(WorkflowEvent::LoopIteration(WorkflowLoopIteration::new(
                    id,
                    node_id(loop_name),
                    1,
                    false
                ))),
                Ok(true)
            );
            let counts = lookup_telemetry::counts();
            assert!(
                counts.0 <= pathless.0 && counts.1 <= pathless.1 && counts.2 <= pathless.2,
                "pathless loop_iteration exceeded its per-event lookup bound: \
                 {counts:?} > {pathless:?}"
            );
        }

        let large = "x".repeat(65_536);
        let large_id = workflow_id(&large);
        let large_path = node_path(&large_id, &[large.as_str(), large.as_str()]);
        let mut large_tracker = WorkflowTracker::new();
        assert_eq!(
            large_tracker.apply_event(WorkflowEvent::RunStarted(WorkflowRunStarted::new(
                large_id.clone(),
                "recipe".to_owned(),
                serde_json::json!({}),
                Vec::new(),
                None,
            ))),
            Ok(true)
        );
        let large_started = Instant::now();
        assert_eq!(
            large_tracker.apply_event(WorkflowEvent::NodeStarted(WorkflowNodeStarted::new(
                large_id,
                node_id(&large),
                large_path,
                WorkflowNodeType::Step,
                WorkflowNodeStartDetails::new(),
            ))),
            Ok(true)
        );
        // Generous CI-safe ceiling: a single 64 KiB event takes microseconds
        // unless string handling regresses to repeated copies, which
        // overshoots this bound by orders of magnitude.
        assert!(
            large_started.elapsed() <= Duration::from_secs(2),
            "64 KiB workflow/node/path event exceeded 2 s"
        );
    }
}
