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
    Search,
    Think,
    Fetch,
    Other,
}

/// A file location referenced by a tool call.
#[derive(Debug, Clone)]
pub struct ToolCallLocation {
    pub path: String,
    pub line: Option<u32>,
}

/// Content produced by a tool call.
#[derive(Debug, Clone)]
pub enum ToolCallContent {
    /// A file diff (edit operations).
    Diff {
        path: String,
        old_text: Option<String>,
        new_text: String,
    },
    /// Text output from the tool.
    Text(String),
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
    content: Vec<ToolCallContent>,
    locations: Vec<ToolCallLocation>,
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
            content: Vec::new(),
            locations: Vec::new(),
        }
    }

    /// Set the content produced by this tool call.
    #[must_use]
    pub fn with_content(mut self, content: Vec<ToolCallContent>) -> Self {
        self.content = content;
        self
    }

    /// Set the file locations referenced by this tool call.
    #[must_use]
    pub fn with_locations(mut self, locations: Vec<ToolCallLocation>) -> Self {
        self.locations = locations;
        self
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
    pub fn content(&self) -> &[ToolCallContent] {
        &self.content
    }
    pub fn locations(&self) -> &[ToolCallLocation] {
        &self.locations
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

    #[test]
    fn tool_call_new_has_empty_content_and_locations() {
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "read".to_string(),
            None,
            ToolKind::Read,
            ToolCallStatus::InProgress,
            None,
        );
        assert!(tc.content().is_empty());
        assert!(tc.locations().is_empty());
    }

    #[test]
    fn tool_call_with_content_builder() {
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "edit".to_string(),
            None,
            ToolKind::Write,
            ToolCallStatus::Completed,
            None,
        )
        .with_content(vec![ToolCallContent::Diff {
            path: "src/main.rs".to_string(),
            old_text: Some("old".to_string()),
            new_text: "new".to_string(),
        }]);
        assert_eq!(tc.content().len(), 1);
        assert!(matches!(
            &tc.content()[0],
            ToolCallContent::Diff { path, .. } if path == "src/main.rs"
        ));
    }

    #[test]
    fn tool_call_with_locations_builder() {
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "read".to_string(),
            None,
            ToolKind::Read,
            ToolCallStatus::InProgress,
            None,
        )
        .with_locations(vec![ToolCallLocation {
            path: "src/lib.rs".to_string(),
            line: Some(42),
        }]);
        assert_eq!(tc.locations().len(), 1);
        assert_eq!(tc.locations()[0].path, "src/lib.rs");
        assert_eq!(tc.locations()[0].line, Some(42));
    }

    #[test]
    fn tool_call_content_text_variant() {
        let content = ToolCallContent::Text("hello".to_string());
        assert!(matches!(content, ToolCallContent::Text(ref t) if t == "hello"));
    }

    #[test]
    fn tool_call_location_without_line() {
        let loc = ToolCallLocation {
            path: "Cargo.toml".to_string(),
            line: None,
        };
        assert_eq!(loc.path, "Cargo.toml");
        assert!(loc.line.is_none());
    }

    #[test]
    fn tool_kind_new_variants_equality() {
        assert_eq!(ToolKind::Search, ToolKind::Search);
        assert_eq!(ToolKind::Think, ToolKind::Think);
        assert_eq!(ToolKind::Fetch, ToolKind::Fetch);
        assert_ne!(ToolKind::Search, ToolKind::Think);
        assert_ne!(ToolKind::Fetch, ToolKind::Other);
    }

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[test]
    fn tool_call_is_send_sync() {
        assert_send::<ToolCall>();
        assert_sync::<ToolCall>();
    }

    #[test]
    fn tool_call_content_is_send_sync() {
        assert_send::<ToolCallContent>();
        assert_sync::<ToolCallContent>();
    }

    #[test]
    fn tool_call_location_is_send_sync() {
        assert_send::<ToolCallLocation>();
        assert_sync::<ToolCallLocation>();
    }
}
