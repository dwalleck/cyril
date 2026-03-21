use std::fmt;

/// Unique session identifier. Newtype wrapper preventing string mixups.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(String);

impl SessionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Session lifecycle state machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SessionStatus {
    #[default]
    Disconnected,
    Initializing,
    Active,
    Busy,
    Compacting,
    Error { message: String },
}

/// An available agent mode (e.g., "code", "chat").
#[derive(Debug, Clone)]
pub struct SessionMode {
    id: String,
    label: String,
    description: Option<String>,
}

impl SessionMode {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        description: Option<impl Into<String>>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: description.map(Into::into),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

/// Context window usage percentage, clamped to [0.0, 100.0].
#[derive(Debug, Clone)]
pub struct ContextUsage {
    percentage: f64,
}

impl ContextUsage {
    pub fn new(percentage: f64) -> Self {
        Self {
            percentage: percentage.clamp(0.0, 100.0),
        }
    }

    pub fn percentage(&self) -> f64 {
        self.percentage
    }
}

/// Credit usage tracking.
#[derive(Debug, Clone)]
pub struct CreditUsage {
    used: f64,
    limit: f64,
}

impl CreditUsage {
    pub fn new(used: f64, limit: f64) -> Self {
        Self { used, limit }
    }

    pub fn used(&self) -> f64 {
        self.used
    }

    pub fn limit(&self) -> f64 {
        self.limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn session_id_roundtrip() {
        let id = SessionId::new("sess_123");
        assert_eq!(id.as_str(), "sess_123");
    }

    #[test]
    fn session_id_display() {
        let id = SessionId::new("sess_123");
        assert_eq!(format!("{id}"), "sess_123");
    }

    #[test]
    fn session_id_usable_as_hashmap_key() {
        let mut map = HashMap::new();
        let id = SessionId::new("sess_1");
        map.insert(id.clone(), 42);
        assert_eq!(map.get(&SessionId::new("sess_1")), Some(&42));
    }

    #[test]
    fn session_status_default_is_disconnected() {
        let status = SessionStatus::default();
        assert_eq!(status, SessionStatus::Disconnected);
    }

    #[test]
    fn session_status_error_carries_message() {
        let status = SessionStatus::Error { message: "oops".into() };
        assert!(matches!(status, SessionStatus::Error { message } if message == "oops"));
    }

    #[test]
    fn context_usage_stores_value() {
        let usage = ContextUsage::new(50.0);
        assert!((usage.percentage() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn context_usage_clamps_high() {
        let usage = ContextUsage::new(150.0);
        assert!((usage.percentage() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn context_usage_clamps_low() {
        let usage = ContextUsage::new(-10.0);
        assert!((usage.percentage() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn session_mode_accessors() {
        let mode = SessionMode::new("code", "Code Mode", Some("Write and edit code"));
        assert_eq!(mode.id(), "code");
        assert_eq!(mode.label(), "Code Mode");
        assert_eq!(mode.description(), Some("Write and edit code"));
    }

    #[test]
    fn session_mode_no_description() {
        let mode = SessionMode::new("chat", "Chat", None::<&str>);
        assert_eq!(mode.description(), None);
    }

    #[test]
    fn credit_usage_accessors() {
        let credits = CreditUsage::new(5.25, 10.0);
        assert!((credits.used() - 5.25).abs() < f64::EPSILON);
        assert!((credits.limit() - 10.0).abs() < f64::EPSILON);
    }

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[test]
    fn session_types_are_send_sync() {
        assert_send::<SessionId>();
        assert_sync::<SessionId>();
        assert_send::<SessionStatus>();
        assert_sync::<SessionStatus>();
        assert_send::<SessionMode>();
        assert_sync::<SessionMode>();
        assert_send::<ContextUsage>();
        assert_sync::<ContextUsage>();
        assert_send::<CreditUsage>();
        assert_sync::<CreditUsage>();
    }
}
