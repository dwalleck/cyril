use super::session::SessionId;

/// A saved session returned by the session list query.
#[derive(Debug, Clone)]
pub struct SessionEntry {
    session_id: SessionId,
    title: Option<String>,
    updated_at: Option<String>,
}

impl SessionEntry {
    pub fn new(session_id: SessionId, title: Option<String>, updated_at: Option<String>) -> Self {
        Self {
            session_id,
            title,
            updated_at,
        }
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn updated_at(&self) -> Option<&str> {
        self.updated_at.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_entry_accessors() {
        let entry = SessionEntry::new(
            SessionId::new("sess_abc"),
            Some("Fix the auth bug".into()),
            Some("2026-04-12T10:30:00Z".into()),
        );
        assert_eq!(entry.session_id().as_str(), "sess_abc");
        assert_eq!(entry.title(), Some("Fix the auth bug"));
        assert_eq!(entry.updated_at(), Some("2026-04-12T10:30:00Z"));
    }

    #[test]
    fn session_entry_optional_fields() {
        let entry = SessionEntry::new(SessionId::new("sess_1"), None, None);
        assert!(entry.title().is_none());
        assert!(entry.updated_at().is_none());
    }

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    fn assert_clone<T: Clone>() {}

    #[test]
    fn session_entry_is_send_sync_clone() {
        assert_send::<SessionEntry>();
        assert_sync::<SessionEntry>();
        assert_clone::<SessionEntry>();
    }
}
