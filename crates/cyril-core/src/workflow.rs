use std::collections::HashMap;

use crate::types::workflow::WorkflowNodeSnapshotParts;
use crate::types::{
    SessionId, WorkflowCompletionSignal, WorkflowCompletionSignalSource, WorkflowId,
    WorkflowNodeDescriptor, WorkflowNodePath, WorkflowNodePathError, WorkflowNodeSnapshot,
    WorkflowNodeStatus, WorkflowRunStatus, WorkflowSnapshot,
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
}

impl WorkflowStateError {
    /// Returns the stable diagnostic category for this failure.
    pub const fn error_kind(&self) -> &'static str {
        match self {
            Self::DuplicateCanonicalPath { .. } => "duplicate_canonical_path",
            Self::InvalidCanonicalPath { .. } => "invalid_canonical_path",
        }
    }
}

/// Immutable read model for one canonical runtime node.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowNodeState {
    descriptor: WorkflowNodeDescriptor,
    status: WorkflowNodeStatus,
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

impl WorkflowNodeState {
    fn from_snapshot(parts: WorkflowNodeSnapshotParts) -> Self {
        let (
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
        ) = parts.into_values();
        Self {
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
        }
    }

    /// Returns the snapshot-authored descriptor.
    pub fn descriptor(&self) -> &WorkflowNodeDescriptor {
        &self.descriptor
    }

    /// Returns the snapshot-authored status.
    pub fn status(&self) -> WorkflowNodeStatus {
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
}

/// Immutable read model for one persisted workflow run.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowRun {
    workflow_name: String,
    status: WorkflowRunStatus,
    inputs: serde_json::Value,
    artifacts: serde_json::Value,
    captured_outputs: serde_json::Value,
    created_at: String,
    plan_revision: u32,
    parent_session_id: Option<SessionId>,
    workspace_path: Option<String>,
    nodes: HashMap<WorkflowNodePath, WorkflowNodeState>,
}

impl WorkflowRun {
    /// Returns the recipe name for this run.
    pub fn workflow_name(&self) -> &str {
        &self.workflow_name
    }

    /// Returns the authoritative run status.
    pub fn status(&self) -> WorkflowRunStatus {
        self.status
    }

    /// Returns opaque recipe inputs.
    pub fn inputs(&self) -> &serde_json::Value {
        &self.inputs
    }

    /// Returns opaque run artifacts.
    pub fn artifacts(&self) -> &serde_json::Value {
        &self.artifacts
    }

    /// Returns opaque captured outputs.
    pub fn captured_outputs(&self) -> &serde_json::Value {
        &self.captured_outputs
    }

    /// Returns the opaque creation timestamp.
    pub fn created_at(&self) -> &str {
        &self.created_at
    }

    /// Returns the persisted plan revision.
    pub fn plan_revision(&self) -> u32 {
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

    /// Returns one node by exact canonical path.
    pub fn node(&self, path: &WorkflowNodePath) -> Option<&WorkflowNodeState> {
        self.nodes.get(path)
    }

    /// Iterates every canonical node without allocating or imposing an order.
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = (&WorkflowNodePath, &WorkflowNodeState)> {
        self.nodes.iter()
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

    /// Validates, canonicalizes, and applies a complete persisted snapshot.
    ///
    /// # Errors
    ///
    /// Returns a structured error without changing the tracker when two nodes
    /// map to the same canonical path or a path violates its domain invariant.
    pub fn apply_snapshot(
        &mut self,
        snapshot: WorkflowSnapshot,
    ) -> Result<bool, WorkflowStateError> {
        let (workflow_id, run) = canonicalize_snapshot(snapshot)?;
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

fn canonicalize_snapshot(
    snapshot: WorkflowSnapshot,
) -> Result<(WorkflowId, WorkflowRun), WorkflowStateError> {
    let (
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
    ) = snapshot.into_parts().into_values();
    let root_path = WorkflowNodePath::try_new(&workflow_id, vec![workflow_id.as_str().to_owned()])
        .map_err(|source| WorkflowStateError::InvalidCanonicalPath { source })?;
    let mut nodes = HashMap::new();
    let mut stack = vec![(root, root_path)];
    while let Some((node, path)) = stack.pop() {
        let mut parts = node.into_parts();
        let children = parts.take_children();
        for child in children.into_iter().rev() {
            let segment = canonical_child_segment(parts.descriptor(), &child);
            let mut segments = path.segments().to_vec();
            segments.push(segment);
            let child_path = WorkflowNodePath::try_new(&workflow_id, segments)
                .map_err(|source| WorkflowStateError::InvalidCanonicalPath { source })?;
            stack.push((child, child_path));
        }
        let state = WorkflowNodeState::from_snapshot(parts);
        if nodes.insert(path.clone(), state).is_some() {
            return Err(WorkflowStateError::DuplicateCanonicalPath { path });
        }
    }
    Ok((
        workflow_id,
        WorkflowRun {
            workflow_name,
            status,
            inputs,
            artifacts,
            captured_outputs,
            created_at,
            plan_revision,
            parent_session_id,
            workspace_path,
            nodes,
        },
    ))
}

fn canonical_child_segment(
    parent: &WorkflowNodeDescriptor,
    child: &WorkflowNodeSnapshot,
) -> String {
    let child_id = child.descriptor().node_id().as_str();
    if parent.node_type() == crate::types::WorkflowNodeType::Repeat
        && child.descriptor().node_type() == crate::types::WorkflowNodeType::Sequence
    {
        let mut prefix = parent.node_id().as_str().to_owned();
        prefix.push('#');
        if let Some(suffix) = child_id.strip_prefix(&prefix)
            && let Ok(parsed) = suffix.parse::<u32>()
            && suffix == parsed.to_string()
            && child.iteration() == Some(parsed)
        {
            return format!("iter-{parsed}");
        }
    }
    child_id.to_owned()
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::types::{
        WorkflowNodeDescriptor, WorkflowNodePath, WorkflowNodeSnapshot, WorkflowNodeStatus,
        WorkflowNodeType, WorkflowRepeatExhaustion, WorkflowRunStatus, WorkflowSnapshot,
        WorkflowSnapshotData, WorkflowSnapshotMetadata,
    };
    fn workflow_id(value: &str) -> WorkflowId {
        match WorkflowId::try_from(value.to_owned()) {
            Ok(id) => id,
            Err(error) => panic!("invalid workflow id fixture: {error}"),
        }
    }

    fn node_id(value: &str) -> crate::types::WorkflowNodeId {
        match crate::types::WorkflowNodeId::try_from(value.to_owned()) {
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

    fn seed(tracker: &mut WorkflowTracker, id: &str, name: &str) {
        let previous = tracker.runs.insert(
            workflow_id(id),
            WorkflowRun {
                workflow_name: name.to_owned(),
                status: WorkflowRunStatus::Running,
                inputs: serde_json::Value::Null,
                artifacts: serde_json::Value::Null,
                captured_outputs: serde_json::Value::Null,
                created_at: String::new(),
                plan_revision: 0,
                parent_session_id: None,
                workspace_path: None,
                nodes: HashMap::new(),
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
        assert!(
            started.elapsed() <= Duration::from_millis(100),
            "100,000 short-id lookups exceeded 100 ms"
        );

        let large = workflow_id(&"x".repeat(65_536));
        let started = Instant::now();
        assert!(tracker.get(&large).is_none());
        assert!(
            started.elapsed() <= Duration::from_millis(50),
            "64 KiB workflow-id lookup exceeded 50 ms"
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
        let manifest: serde_json::Value =
            match serde_json::from_str(include_str!("../../../.cyril-6beh/oracle-manifest.json")) {
                Ok(value) => value,
                Err(error) => panic!("workflow manifest is invalid: {error}"),
            };
        let controls = match manifest["repeat_controls"].as_array() {
            Some(controls) => controls,
            None => panic!("repeat controls are not an array"),
        };
        assert_eq!(controls.len(), 8);
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
            let repeat = WorkflowNodeSnapshot::new(
                WorkflowNodeDescriptor::repeat(
                    node_id("loop"),
                    Vec::new(),
                    4,
                    WorkflowRepeatExhaustion::Pause,
                    None,
                    None,
                ),
                WorkflowNodeStatus::Completed,
                vec![child],
            );
            let root = WorkflowNodeSnapshot::new(
                WorkflowNodeDescriptor::sequence(node_id("root"), Vec::new()),
                WorkflowNodeStatus::Completed,
                vec![repeat],
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
            let wrapper = control["wrapper"].as_bool() == Some(true);
            let child_segment = if wrapper {
                let iteration = match control["iteration"].as_u64() {
                    Some(value) => value,
                    None => panic!("wrapper control lacks iteration"),
                };
                format!("iter-{iteration}")
            } else {
                child_id.to_owned()
            };
            let path = node_path(
                &workflow_id,
                &[workflow.as_str(), "loop", child_segment.as_str()],
            );
            let state = match run.node(&path) {
                Some(state) => state,
                None => panic!("repeat child missing at {path:?}"),
            };
            assert_eq!(state.descriptor().node_id().as_str(), child_id);
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
            started.elapsed() <= Duration::from_millis(50),
            "1 MiB/256-node/depth-10/64 KiB-segment snapshot exceeded 50 ms"
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
}
