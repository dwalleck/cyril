use std::collections::HashMap;

use crate::types::WorkflowId;

/// Immutable read model for one persisted workflow run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRun {
    workflow_name: String,
}

impl WorkflowRun {
    /// Returns the recipe name for this run.
    pub fn workflow_name(&self) -> &str {
        &self.workflow_name
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

    /// Returns the run with the exact supplied identifier.
    pub fn get(&self, id: &WorkflowId) -> Option<&WorkflowRun> {
        self.runs.get(id)
    }

    /// Iterates every tracked run without allocating or imposing an order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&WorkflowId, &WorkflowRun)> {
        self.runs.iter()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    fn workflow_id(value: &str) -> WorkflowId {
        match WorkflowId::try_from(value.to_owned()) {
            Ok(id) => id,
            Err(error) => panic!("invalid workflow id fixture: {error}"),
        }
    }

    fn seed(tracker: &mut WorkflowTracker, id: &str, name: &str) {
        let previous = tracker.runs.insert(
            workflow_id(id),
            WorkflowRun {
                workflow_name: name.to_owned(),
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
}
