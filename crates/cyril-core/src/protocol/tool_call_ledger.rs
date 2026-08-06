use std::cell::RefCell;
use std::collections::HashMap;

use crate::types::{SessionId, ToolCall, ToolCallId};

/// Session-scoped snapshot of every tool call the protocol adapter has seen.
///
/// KAS permission requests carry a stub `toolCall`; the full producer payload
/// arrives earlier as a `session/update`. The exact `(SessionId, ToolCallId)`
/// pair is the join key, preventing a tool-call ID from another session from
/// leaking into an approval preview. The ledger owns cloned domain snapshots;
/// a permission request never holds a live view of later updates.
pub(crate) struct ToolCallLedger {
    calls: RefCell<HashMap<(SessionId, ToolCallId), ToolCall>>,
}

impl ToolCallLedger {
    pub(crate) fn new() -> Self {
        Self {
            calls: RefCell::new(HashMap::new()),
        }
    }

    /// Merge one converted tool call into the session-scoped snapshot.
    /// `kind_present`/`status_present` carry wire-level presence from the
    /// originating frame: KAS omits `kind` on `tool_call_update`s, and an
    /// unconditional overwrite would downgrade a tracked `Write` to `Other`.
    pub(crate) fn merge(
        &self,
        session_id: SessionId,
        update: &ToolCall,
        kind_present: bool,
        status_present: bool,
    ) {
        let key = (session_id, update.id().clone());
        let mut calls = self.calls.borrow_mut();
        match calls.get_mut(&key) {
            Some(existing) => {
                existing.merge_update_with_presence(update, kind_present, status_present)
            }
            None => {
                calls.insert(key, update.clone());
            }
        }
    }

    /// Clone the exact session-scoped snapshot for a permission request.
    pub(crate) fn snapshot(
        &self,
        session_id: &SessionId,
        tool_call_id: &ToolCallId,
    ) -> Option<ToolCall> {
        self.calls
            .borrow()
            .get(&(session_id.clone(), tool_call_id.clone()))
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::types::{ToolCallContent, ToolCallLocation, ToolCallStatus, ToolKind};

    fn call(
        id: &str,
        title: &str,
        status: ToolCallStatus,
        raw_input: Option<serde_json::Value>,
    ) -> ToolCall {
        ToolCall::new(
            ToolCallId::new(id),
            title.to_string(),
            ToolKind::Write,
            status,
            raw_input,
        )
        .with_content(vec![ToolCallContent::Diff {
            path: "a.py".to_string(),
            old_text: None,
            new_text: "new".to_string(),
        }])
        .with_locations(vec![ToolCallLocation {
            path: "a.py".to_string(),
            line: None,
        }])
        .with_raw_output(Some(serde_json::json!({"message": "ok"})))
    }

    #[test]
    fn ledger_snapshot_is_session_scoped() {
        let ledger = ToolCallLedger::new();
        let same_id = ToolCallId::new("tc");
        ledger.merge(
            SessionId::new("one"),
            &call("tc", "first", ToolCallStatus::Pending, None),
            true,
            true,
        );
        ledger.merge(
            SessionId::new("two"),
            &call(
                "tc",
                "second",
                ToolCallStatus::Completed,
                Some(serde_json::json!({"path": "two.py"})),
            ),
            true,
            true,
        );

        let first = ledger
            .snapshot(&SessionId::new("one"), &same_id)
            .expect("session one snapshot");
        assert_eq!(first.title(), "first");
        assert!(first.raw_input().is_none());
        let second = ledger
            .snapshot(&SessionId::new("two"), &same_id)
            .expect("session two snapshot");
        assert_eq!(second.title(), "second");
        assert_eq!(
            second.raw_input(),
            Some(&serde_json::json!({"path": "two.py"}))
        );
        assert!(
            ledger
                .snapshot(&SessionId::new("three"), &same_id)
                .is_none()
        );
    }

    #[test]
    fn ledger_partial_update_preserves_non_empty_fields() {
        let ledger = ToolCallLedger::new();
        let session = SessionId::new("s");
        let id = ToolCallId::new("tc");
        ledger.merge(
            session.clone(),
            &call(
                "tc",
                "Write File",
                ToolCallStatus::InProgress,
                Some(serde_json::json!({"path": "a.py", "text": "new"})),
            ),
            true,
            true,
        );
        // Model the KAS wire shape: the update omits `kind`, which the
        // converter collapses to `Other`; presence=false must preserve the
        // tracked `Write` kind rather than downgrade it.
        let partial = ToolCall::new(
            id.clone(),
            String::new(),
            ToolKind::Other,
            ToolCallStatus::Pending,
            None,
        );
        ledger.merge(session.clone(), &partial, false, true);

        let snapshot = ledger.snapshot(&session, &id).expect("merged snapshot");
        assert_eq!(snapshot.title(), "Write File");
        assert_eq!(
            snapshot.kind(),
            ToolKind::Write,
            "absent kind on the wire must not downgrade the tracked kind"
        );
        assert_eq!(
            snapshot.raw_input(),
            Some(&serde_json::json!({"path": "a.py", "text": "new"}))
        );
        assert_eq!(snapshot.content().len(), 1);
        assert_eq!(snapshot.locations().len(), 1);
        assert_eq!(
            snapshot.raw_output(),
            Some(&serde_json::json!({"message": "ok"}))
        );
        assert_eq!(snapshot.status(), ToolCallStatus::Pending);
    }
}
