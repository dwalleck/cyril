/// Status of a plan entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanEntryStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

/// A single step in the agent's plan.
#[derive(Debug, Clone)]
pub struct PlanEntry {
    title: String,
    status: PlanEntryStatus,
}

impl PlanEntry {
    pub fn new(title: impl Into<String>, status: PlanEntryStatus) -> Self {
        Self {
            title: title.into(),
            status,
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn status(&self) -> PlanEntryStatus {
        self.status
    }
}

/// The agent's execution plan.
#[derive(Debug, Clone)]
pub struct Plan {
    entries: Vec<PlanEntry>,
}

impl Plan {
    pub fn new(entries: Vec<PlanEntry>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &[PlanEntry] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_entry_accessors() {
        let entry = PlanEntry::new("Implement feature", PlanEntryStatus::InProgress);
        assert_eq!(entry.title(), "Implement feature");
        assert_eq!(entry.status(), PlanEntryStatus::InProgress);
    }

    #[test]
    fn plan_entries_slice() {
        let plan = Plan::new(vec![
            PlanEntry::new("Step 1", PlanEntryStatus::Completed),
            PlanEntry::new("Step 2", PlanEntryStatus::Pending),
        ]);
        assert_eq!(plan.entries().len(), 2);
        assert_eq!(plan.entries()[0].title(), "Step 1");
        assert_eq!(plan.entries()[1].status(), PlanEntryStatus::Pending);
    }

    #[test]
    fn plan_empty() {
        let plan = Plan::new(vec![]);
        assert!(plan.entries().is_empty());
    }

    #[test]
    fn plan_entry_status_equality() {
        assert_eq!(PlanEntryStatus::Pending, PlanEntryStatus::Pending);
        assert_ne!(PlanEntryStatus::Pending, PlanEntryStatus::Completed);
    }

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[test]
    fn plan_is_send_sync() {
        assert_send::<Plan>();
        assert_sync::<Plan>();
    }
}
