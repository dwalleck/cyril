use std::collections::HashMap;

use crate::types::*;

/// Tracks subagent metadata from `list_update` notifications.
/// Pure state machine — no async, no UI knowledge.
pub struct SubagentTracker {
    subagents: HashMap<SessionId, SubagentInfo>,
    pending_stages: Vec<PendingStage>,
    inbox_message_count: u32,
    inbox_escalation_count: u32,
}

impl SubagentTracker {
    pub fn new() -> Self {
        Self {
            subagents: HashMap::new(),
            pending_stages: Vec::new(),
            inbox_message_count: 0,
            inbox_escalation_count: 0,
        }
    }

    pub fn apply_notification(&mut self, notification: &Notification) -> bool {
        match notification {
            Notification::SubagentListUpdated {
                subagents,
                pending_stages,
            } => {
                self.subagents = subagents
                    .iter()
                    .map(|s| (s.session_id().clone(), s.clone()))
                    .collect();
                self.pending_stages = pending_stages.clone();
                true
            }
            Notification::InboxNotification {
                message_count,
                escalation_count,
                ..
            } => {
                self.inbox_message_count = *message_count;
                self.inbox_escalation_count = *escalation_count;
                true
            }
            _ => false,
        }
    }

    pub fn subagents(&self) -> &HashMap<SessionId, SubagentInfo> {
        &self.subagents
    }

    pub fn pending_stages(&self) -> &[PendingStage] {
        &self.pending_stages
    }

    pub fn get(&self, session_id: &SessionId) -> Option<&SubagentInfo> {
        self.subagents.get(session_id)
    }

    /// Find a subagent by its `session_name` (human-friendly identifier used
    /// in slash commands like `/kill reviewer`).
    ///
    /// If multiple subagents share the same name, returns an arbitrary match
    /// and logs a warning — HashMap iteration order is non-deterministic, so
    /// callers cannot rely on which one is returned. Name uniqueness is not
    /// enforced by the protocol, so this is a best-effort lookup.
    pub fn find_by_name(&self, name: &str) -> Option<&SubagentInfo> {
        let mut matches = self.subagents.values().filter(|s| s.session_name() == name);
        let first = matches.next()?;
        if matches.next().is_some() {
            let count = 1 + self
                .subagents
                .values()
                .filter(|s| s.session_name() == name)
                .count()
                .saturating_sub(1);
            tracing::warn!(
                name,
                count,
                "multiple subagents share the same name; returning arbitrary match"
            );
        }
        Some(first)
    }

    pub fn is_subagent(&self, session_id: &SessionId) -> bool {
        self.subagents.contains_key(session_id)
    }

    pub fn active_count(&self) -> usize {
        self.subagents.values().filter(|s| s.is_working()).count()
    }

    pub fn inbox_message_count(&self) -> u32 {
        self.inbox_message_count
    }

    pub fn inbox_escalation_count(&self) -> u32 {
        self.inbox_escalation_count
    }

    /// Returns distinct group names across active subagents and pending stages.
    pub fn groups(&self) -> Vec<&str> {
        let mut groups: Vec<&str> = self
            .subagents
            .values()
            .filter_map(|s| s.group())
            .chain(self.pending_stages.iter().filter_map(|s| s.group()))
            .collect();
        groups.sort_unstable();
        groups.dedup();
        groups
    }
}

impl Default for SubagentTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn working_info(id: &str, name: &str) -> SubagentInfo {
        SubagentInfo::new(
            SessionId::new(id),
            name,
            name,
            "query",
            SubagentStatus::Working {
                message: Some("Running".into()),
            },
            Some("crew-test".into()),
            Some(name.to_string()),
            vec![],
        )
    }

    fn terminated_info(id: &str, name: &str) -> SubagentInfo {
        SubagentInfo::new(
            SessionId::new(id),
            name,
            name,
            "query",
            SubagentStatus::Terminated,
            Some("crew-test".into()),
            None,
            vec![],
        )
    }

    #[test]
    fn empty_tracker() {
        let tracker = SubagentTracker::new();
        assert!(tracker.subagents().is_empty());
        assert!(tracker.pending_stages().is_empty());
        assert_eq!(tracker.active_count(), 0);
        assert_eq!(tracker.inbox_message_count(), 0);
    }

    #[test]
    fn apply_list_update_replaces_state() {
        let mut tracker = SubagentTracker::new();

        let notif = Notification::SubagentListUpdated {
            subagents: vec![working_info("s1", "reviewer")],
            pending_stages: vec![PendingStage::new(
                "summary",
                None,
                None,
                None,
                vec!["reviewer".into()],
            )],
        };
        assert!(tracker.apply_notification(&notif));
        assert_eq!(tracker.subagents().len(), 1);
        assert_eq!(tracker.pending_stages().len(), 1);
        assert_eq!(tracker.active_count(), 1);
        assert!(tracker.is_subagent(&SessionId::new("s1")));
        assert!(!tracker.is_subagent(&SessionId::new("unknown")));

        // Second update replaces entirely
        let notif2 = Notification::SubagentListUpdated {
            subagents: vec![
                working_info("s2", "analyzer"),
                terminated_info("s1", "reviewer"),
            ],
            pending_stages: vec![],
        };
        assert!(tracker.apply_notification(&notif2));
        assert_eq!(tracker.subagents().len(), 2);
        assert_eq!(tracker.active_count(), 1); // only s2 is working
        assert!(tracker.pending_stages().is_empty());
    }

    #[test]
    fn apply_inbox_notification() {
        let mut tracker = SubagentTracker::new();
        let notif = Notification::InboxNotification {
            session_id: SessionId::new("main"),
            message_count: 3,
            escalation_count: 1,
            senders: vec!["subagent".into()],
        };
        assert!(tracker.apply_notification(&notif));
        assert_eq!(tracker.inbox_message_count(), 3);
        assert_eq!(tracker.inbox_escalation_count(), 1);
    }

    #[test]
    fn groups_deduplicates() {
        let mut tracker = SubagentTracker::new();
        let notif = Notification::SubagentListUpdated {
            subagents: vec![working_info("s1", "a"), working_info("s2", "b")],
            pending_stages: vec![],
        };
        tracker.apply_notification(&notif);
        let groups = tracker.groups();
        assert_eq!(groups, vec!["crew-test"]);
    }

    #[test]
    fn groups_includes_pending_stages() {
        let mut tracker = SubagentTracker::new();
        let notif = Notification::SubagentListUpdated {
            subagents: vec![working_info("s1", "a")], // group: "crew-test"
            pending_stages: vec![PendingStage::new(
                "summary",
                None,
                Some("crew-other".into()),
                None,
                vec!["a".into()],
            )],
        };
        tracker.apply_notification(&notif);
        let groups = tracker.groups();
        assert_eq!(groups, vec!["crew-other", "crew-test"]);
    }

    #[test]
    fn ignores_unrelated_notifications() {
        let mut tracker = SubagentTracker::new();
        assert!(!tracker.apply_notification(&Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        }));
    }

    #[test]
    fn get_returns_subagent_by_session_id() {
        let mut tracker = SubagentTracker::new();
        let notif = Notification::SubagentListUpdated {
            subagents: vec![working_info("s1", "reviewer")],
            pending_stages: vec![],
        };
        tracker.apply_notification(&notif);
        assert!(tracker.get(&SessionId::new("s1")).is_some());
        assert_eq!(
            tracker.get(&SessionId::new("s1")).unwrap().session_name(),
            "reviewer"
        );
        assert!(tracker.get(&SessionId::new("missing")).is_none());
    }
}
