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
///
/// The `title` field is the human-readable display text from ACP (e.g., "Reading main.rs").
/// ACP has a single `title` field; there is no separate "name" concept.
#[derive(Debug, Clone)]
pub struct ToolCall {
    id: ToolCallId,
    title: String,
    kind: ToolKind,
    status: ToolCallStatus,
    raw_input: Option<serde_json::Value>,
    content: Vec<ToolCallContent>,
    locations: Vec<ToolCallLocation>,
}

impl ToolCall {
    pub fn new(
        id: ToolCallId,
        title: String,
        kind: ToolKind,
        status: ToolCallStatus,
        raw_input: Option<serde_json::Value>,
    ) -> Self {
        Self {
            id,
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
    /// The human-readable display text from ACP (e.g., "Reading main.rs").
    pub fn title(&self) -> &str {
        &self.title
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

    /// Merge fields from an update into this tool call.
    /// Always overwrites `kind` and `status`. Conditionally overwrites `title`,
    /// `raw_input`, `content`, and `locations` only when the update carries non-empty values.
    pub fn merge_update(&mut self, update: &ToolCall) {
        if !update.title.is_empty() {
            self.title = update.title.clone();
        }
        self.kind = update.kind;
        self.status = update.status;
        if update.raw_input.is_some() {
            self.raw_input = update.raw_input.clone();
        }
        if !update.content.is_empty() {
            self.content = update.content.clone();
        }
        if !update.locations.is_empty() {
            self.locations = update.locations.clone();
        }
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
            "Reading main.rs".to_string(),
            ToolKind::Read,
            ToolCallStatus::InProgress,
            None,
        );
        assert_eq!(tc.id().as_str(), "tc_1");
        assert_eq!(tc.title(), "Reading main.rs");
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
            ToolKind::Write,
            ToolCallStatus::Completed,
            Some(input.clone()),
        );
        assert_eq!(tc.raw_input(), Some(&input));
        assert_eq!(tc.title(), "write_file");
    }

    #[test]
    fn tool_call_new_has_empty_content_and_locations() {
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "read".to_string(),
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

    // --- merge_update tests ---

    #[test]
    fn merge_update_preserves_content_when_update_has_none() {
        // Simulates: initial ToolCall has diff content, ToolCallUpdate only changes status
        let mut tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "Editing main.rs".into(),
            ToolKind::Write,
            ToolCallStatus::InProgress,
            None,
        )
        .with_content(vec![ToolCallContent::Diff {
            path: "src/main.rs".into(),
            old_text: Some("old code".into()),
            new_text: "new code".into(),
        }])
        .with_locations(vec![ToolCallLocation {
            path: "src/main.rs".into(),
            line: Some(1),
        }]);

        // Update only changes status — no content or locations
        let update = ToolCall::new(
            ToolCallId::new("tc_1"),
            "Editing main.rs".into(),
            ToolKind::Write,
            ToolCallStatus::Completed,
            None,
        );

        tc.merge_update(&update);

        assert_eq!(
            tc.status(),
            ToolCallStatus::Completed,
            "status should update"
        );
        assert_eq!(tc.content().len(), 1, "content should be preserved");
        assert_eq!(tc.locations().len(), 1, "locations should be preserved");
        assert!(
            matches!(&tc.content()[0], ToolCallContent::Diff { new_text, .. } if new_text == "new code"),
            "diff content should be intact"
        );
    }

    #[test]
    fn merge_update_overwrites_content_when_update_provides_it() {
        let mut tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "write".into(),
            ToolKind::Write,
            ToolCallStatus::InProgress,
            None,
        )
        .with_content(vec![ToolCallContent::Text("old content".into())]);

        let update = ToolCall::new(
            ToolCallId::new("tc_1"),
            "write".into(),
            ToolKind::Write,
            ToolCallStatus::Completed,
            None,
        )
        .with_content(vec![ToolCallContent::Diff {
            path: "file.rs".into(),
            old_text: None,
            new_text: "new file".into(),
        }]);

        tc.merge_update(&update);

        assert_eq!(tc.content().len(), 1);
        assert!(
            matches!(&tc.content()[0], ToolCallContent::Diff { .. }),
            "content should be replaced when update provides it"
        );
    }

    #[test]
    fn merge_update_preserves_title_when_update_is_empty() {
        let mut tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "Reading config.rs".into(),
            ToolKind::Read,
            ToolCallStatus::InProgress,
            None,
        );

        let update = ToolCall::new(
            ToolCallId::new("tc_1"),
            String::new(), // empty title in update
            ToolKind::Read,
            ToolCallStatus::Completed,
            None,
        );

        tc.merge_update(&update);

        assert_eq!(
            tc.title(),
            "Reading config.rs",
            "title should be preserved when update has empty title"
        );
        assert_eq!(tc.status(), ToolCallStatus::Completed);
    }

    #[test]
    fn merge_update_overwrites_title_when_update_provides_it() {
        let mut tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "Reading...".into(),
            ToolKind::Read,
            ToolCallStatus::InProgress,
            None,
        );

        let update = ToolCall::new(
            ToolCallId::new("tc_1"),
            "Reading config.rs:1-50".into(),
            ToolKind::Read,
            ToolCallStatus::Pending,
            None,
        );

        tc.merge_update(&update);

        assert_eq!(tc.title(), "Reading config.rs:1-50");
    }

    #[test]
    fn merge_update_preserves_raw_input_when_update_has_none() {
        let input = serde_json::json!({"path": "src/main.rs"});
        let mut tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "read".into(),
            ToolKind::Read,
            ToolCallStatus::InProgress,
            Some(input.clone()),
        );

        let update = ToolCall::new(
            ToolCallId::new("tc_1"),
            "read".into(),
            ToolKind::Read,
            ToolCallStatus::Completed,
            None, // no raw_input in update
        );

        tc.merge_update(&update);

        assert_eq!(
            tc.raw_input(),
            Some(&input),
            "raw_input should be preserved"
        );
    }

    #[test]
    fn merge_update_applies_kind_other() {
        let mut tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "read".into(),
            ToolKind::Read,
            ToolCallStatus::InProgress,
            None,
        );
        let update = ToolCall::new(
            ToolCallId::new("tc_1"),
            "planning".into(),
            ToolKind::Other,
            ToolCallStatus::InProgress,
            None,
        );
        tc.merge_update(&update);
        assert_eq!(tc.kind(), ToolKind::Other, "kind should update to Other");
    }
}
