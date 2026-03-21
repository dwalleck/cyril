use std::fmt;

/// Unique tool call identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolCallId(String);

impl ToolCallId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ToolCallId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The kind of operation a tool performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Read,
    Write,
    Execute,
    Other,
}

/// Lifecycle status of a tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallStatus {
    InProgress,
    Pending,
    Completed,
    Failed,
}

/// A tool call from the agent, with accessor methods.
#[derive(Debug, Clone)]
pub struct ToolCall {
    id: ToolCallId,
    name: String,
    title: Option<String>,
    kind: ToolKind,
    status: ToolCallStatus,
    raw_input: Option<serde_json::Value>,
}

impl ToolCall {
    pub fn new(
        id: ToolCallId,
        name: String,
        title: Option<String>,
        kind: ToolKind,
        status: ToolCallStatus,
        raw_input: Option<serde_json::Value>,
    ) -> Self {
        Self {
            id,
            name,
            title,
            kind,
            status,
            raw_input,
        }
    }

    pub fn id(&self) -> &ToolCallId {
        &self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
    pub fn kind(&self) -> ToolKind {
        self.kind
    }
    pub fn status(&self) -> ToolCallStatus {
        self.status
    }
    pub fn raw_input(&self) -> Option<&serde_json::Value> {
        self.raw_input.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn tool_call_id_roundtrip() {
        let id = ToolCallId::new("tc_abc");
        assert_eq!(id.as_str(), "tc_abc");
    }

    #[test]
    fn tool_call_id_display() {
        let id = ToolCallId::new("tc_abc");
        assert_eq!(format!("{id}"), "tc_abc");
    }

    #[test]
    fn tool_call_id_hashmap_key() {
        let mut map = HashMap::new();
        let id = ToolCallId::new("tc_1");
        map.insert(id.clone(), "value");
        assert_eq!(map.get(&ToolCallId::new("tc_1")), Some(&"value"));
    }

    #[test]
    fn tool_kind_equality() {
        assert_eq!(ToolKind::Read, ToolKind::Read);
        assert_ne!(ToolKind::Read, ToolKind::Write);
    }

    #[test]
    fn tool_call_status_equality() {
        assert_eq!(ToolCallStatus::InProgress, ToolCallStatus::InProgress);
        assert_ne!(ToolCallStatus::Completed, ToolCallStatus::Failed);
    }

    #[test]
    fn tool_call_accessors() {
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "read_file".to_string(),
            Some("Reading main.rs".to_string()),
            ToolKind::Read,
            ToolCallStatus::InProgress,
            None,
        );
        assert_eq!(tc.id().as_str(), "tc_1");
        assert_eq!(tc.name(), "read_file");
        assert_eq!(tc.title(), Some("Reading main.rs"));
        assert_eq!(tc.kind(), ToolKind::Read);
        assert_eq!(tc.status(), ToolCallStatus::InProgress);
        assert!(tc.raw_input().is_none());
    }

    #[test]
    fn tool_call_with_raw_input() {
        let input = serde_json::json!({"path": "src/main.rs"});
        let tc = ToolCall::new(
            ToolCallId::new("tc_2"),
            "write_file".to_string(),
            None,
            ToolKind::Write,
            ToolCallStatus::Completed,
            Some(input.clone()),
        );
        assert_eq!(tc.raw_input(), Some(&input));
        assert!(tc.title().is_none());
    }

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[test]
    fn tool_call_is_send_sync() {
        assert_send::<ToolCall>();
        assert_sync::<ToolCall>();
    }
}
