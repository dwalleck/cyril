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
    Error {
        message: String,
    },
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

/// Per-turn metering data from kiro.dev/metadata.
#[derive(Debug, Clone)]
pub struct TurnMetering {
    credits: f64,
    duration_ms: Option<u64>,
}

impl TurnMetering {
    pub fn new(credits: f64, duration_ms: Option<u64>) -> Self {
        Self {
            credits,
            duration_ms,
        }
    }

    pub fn credits(&self) -> f64 {
        self.credits
    }

    pub fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }

    pub fn duration_display(&self) -> Option<String> {
        self.duration_ms.map(|ms| {
            if ms < 1000 {
                format!("{ms}ms")
            } else if ms < 60_000 {
                format!("{:.1}s", ms as f64 / 1000.0)
            } else {
                let mins = ms / 60_000;
                let secs = (ms % 60_000) / 1000;
                format!("{mins}m {secs}s")
            }
        })
    }
}

/// Running session cost accumulator.
#[derive(Debug, Clone, Default)]
pub struct SessionCost {
    total_credits: f64,
    turn_count: u32,
    last_turn_credits: Option<f64>,
    last_turn_duration_ms: Option<u64>,
}

impl SessionCost {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_turn(&mut self, metering: &TurnMetering) {
        let credits = metering.credits();
        if credits.is_finite() {
            self.total_credits += credits;
        } else {
            tracing::warn!(
                credits,
                "TurnMetering credits is non-finite, skipping accumulation"
            );
        }
        self.turn_count = self.turn_count.saturating_add(1);
        self.last_turn_credits = Some(credits);
        self.last_turn_duration_ms = metering.duration_ms();
    }

    pub fn total_credits(&self) -> f64 {
        self.total_credits
    }

    pub fn turn_count(&self) -> u32 {
        self.turn_count
    }

    pub fn last_turn_credits(&self) -> Option<f64> {
        self.last_turn_credits
    }

    pub fn last_turn_duration_ms(&self) -> Option<u64> {
        self.last_turn_duration_ms
    }
}

/// Token counts from a single turn.
#[derive(Debug, Clone)]
pub struct TokenCounts {
    input: u64,
    output: u64,
    cached: Option<u64>,
}

impl TokenCounts {
    pub fn new(input: u64, output: u64, cached: Option<u64>) -> Self {
        Self {
            input,
            output,
            cached,
        }
    }

    pub fn input(&self) -> u64 {
        self.input
    }

    pub fn output(&self) -> u64 {
        self.output
    }

    pub fn cached(&self) -> Option<u64> {
        self.cached
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
        let status = SessionStatus::Error {
            message: "oops".into(),
        };
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

    #[test]
    fn session_cost_accumulates() {
        let mut cost = SessionCost::new();
        cost.record_turn(&TurnMetering::new(0.018, Some(1948)));
        cost.record_turn(&TurnMetering::new(0.042, Some(5200)));
        assert_eq!(cost.turn_count(), 2);
        assert!((cost.total_credits() - 0.060).abs() < 0.001);
        assert!((cost.last_turn_credits().unwrap() - 0.042).abs() < 0.001);
        assert_eq!(cost.last_turn_duration_ms(), Some(5200));
    }

    #[test]
    fn duration_display_formatting() {
        assert_eq!(
            TurnMetering::new(0.01, Some(500)).duration_display(),
            Some("500ms".into())
        );
        assert_eq!(
            TurnMetering::new(0.01, Some(1948)).duration_display(),
            Some("1.9s".into())
        );
        assert_eq!(
            TurnMetering::new(0.01, Some(135000)).duration_display(),
            Some("2m 15s".into())
        );
        assert!(TurnMetering::new(0.01, None).duration_display().is_none());
    }

    #[test]
    fn session_cost_default() {
        let cost = SessionCost::new();
        assert_eq!(cost.total_credits(), 0.0);
        assert_eq!(cost.turn_count(), 0);
        assert!(cost.last_turn_credits().is_none());
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
        assert_send::<TurnMetering>();
        assert_sync::<TurnMetering>();
        assert_send::<SessionCost>();
        assert_sync::<SessionCost>();
        assert_send::<TokenCounts>();
        assert_sync::<TokenCounts>();
    }
}
