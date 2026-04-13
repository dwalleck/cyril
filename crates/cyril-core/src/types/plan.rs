/// Status of a plan entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanEntryStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

/// Priority level of a plan entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlanEntryPriority {
    High,
    #[default]
    Medium,
    Low,
}

/// A single step in the agent's plan.
#[derive(Debug, Clone)]
pub struct PlanEntry {
    title: String,
    status: PlanEntryStatus,
    priority: PlanEntryPriority,
}

impl PlanEntry {
    pub fn new(
        title: impl Into<String>,
        status: PlanEntryStatus,
        priority: PlanEntryPriority,
    ) -> Self {
        Self {
            title: title.into(),
            status,
            priority,
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn status(&self) -> PlanEntryStatus {
        self.status
    }

    pub fn priority(&self) -> PlanEntryPriority {
        self.priority
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
        let entry = PlanEntry::new(
            "Implement feature",
            PlanEntryStatus::InProgress,
            PlanEntryPriority::Medium,
        );
        assert_eq!(entry.title(), "Implement feature");
        assert_eq!(entry.status(), PlanEntryStatus::InProgress);
    }

    #[test]
    fn plan_entries_slice() {
        let plan = Plan::new(vec![
            PlanEntry::new(
                "Step 1",
                PlanEntryStatus::Completed,
                PlanEntryPriority::Medium,
            ),
            PlanEntry::new(
                "Step 2",
                PlanEntryStatus::Pending,
                PlanEntryPriority::Medium,
            ),
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

    #[test]
    fn plan_entry_priority_default_is_medium() {
        assert_eq!(PlanEntryPriority::default(), PlanEntryPriority::Medium);
    }

    #[test]
    fn plan_entry_with_priority() {
        let entry = PlanEntry::new(
            "Critical fix",
            PlanEntryStatus::InProgress,
            PlanEntryPriority::High,
        );
        assert_eq!(entry.priority(), PlanEntryPriority::High);
    }

    #[test]
    fn plan_entry_priority_is_send_sync() {
        assert_send::<PlanEntryPriority>();
        assert_sync::<PlanEntryPriority>();
    }
}
