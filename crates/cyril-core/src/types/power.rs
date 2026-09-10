/// One power installed in the agent's KAS environment, as announced by the
/// `_kiro/powers/items_changed` push (cyril-v19o).
///
/// A power is a user-level bundle that contributes MCP servers and/or steering
/// files to the agent. Cyril never reads a power's files: the agent computes
/// which of those a power carries and reports the result, and this type carries
/// that report verbatim for display.
///
/// Display-only, and deliberately not addressable. There is no `PowerId`
/// newtype because no wire method takes a power identifier: `_kiro/powers/list`
/// is unadvertised and `_kiro/powers/refresh` answers `-32603 Unknown ext
/// method`, so the identifier has exactly one consumer — matching a row to the
/// push it came from.
///
/// Fields are private because two of them are normalized on construction
/// (empty means absent) and `title()` derives from them; a struct literal would
/// let a caller bypass that.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PowerInfo {
    name: String,
    display_name: Option<String>,
    description: Option<String>,
    mcp_server_names: Vec<String>,
    has_steering_files: bool,
}

impl PowerInfo {
    /// Builds one power from its wire values.
    ///
    /// `display_name` and `description` are normalized here: the observed wire
    /// sends them as strings, and an empty string is the agent saying "not
    /// provided" rather than a title of length zero (AGENTS.md "Guard partial
    /// updates"). Normalizing at construction keeps every consumer — the
    /// panel, the sort, a future one — from re-deciding what empty means.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        display_name: Option<String>,
        description: Option<String>,
        mcp_server_names: Vec<String>,
        has_steering_files: bool,
    ) -> Self {
        Self {
            name: name.into(),
            display_name: display_name.filter(|value| !value.is_empty()),
            description: description.filter(|value| !value.is_empty()),
            mcp_server_names,
            has_steering_files,
        }
    }

    /// The power's identifier, unique among installed powers (`datadog`).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// What to show as the power's title: its display name when the agent
    /// provided one, otherwise the identifier.
    ///
    /// Always non-empty: `name` is a required wire field, so the fallback
    /// cannot produce a blank row.
    #[must_use]
    pub fn title(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.name)
    }

    /// The agent's description of the power, or `None` when it provided none.
    ///
    /// `Option` rather than an empty string: the panel omits the line entirely,
    /// and a caller that wants to know whether the agent described the power
    /// can ask.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// MCP servers this power contributes, in wire order. Empty is the common
    /// case for a power that only ships steering files.
    #[must_use]
    pub fn mcp_server_names(&self) -> &[String] {
        &self.mcp_server_names
    }

    /// Whether the agent found steering files in the power. The agent computes
    /// this by counting files; cyril never re-derives it from disk.
    #[must_use]
    pub fn has_steering_files(&self) -> bool {
        self.has_steering_files
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn title_falls_back_to_id_and_empty_strings_mean_absent() {
        let named = PowerInfo::new(
            "datadog",
            Some("Datadog Observability".to_owned()),
            Some("Query Datadog.".to_owned()),
            vec!["datadog".to_owned()],
            true,
        );
        assert_eq!(named.title(), "Datadog Observability");
        assert_eq!(named.description(), Some("Query Datadog."));

        // An empty display name is the agent not providing one, not a blank
        // title; the same rule makes an empty description absent.
        let bare = PowerInfo::new(
            "markdownlint",
            Some(String::new()),
            Some(String::new()),
            vec![],
            false,
        );
        assert_eq!(bare.title(), "markdownlint");
        assert_eq!(bare.description(), None);
        assert!(bare.mcp_server_names().is_empty());
        assert!(!bare.has_steering_files());

        let absent = PowerInfo::new("markdownlint", None, None, vec![], false);
        assert_eq!(absent.title(), "markdownlint");
        assert_eq!(absent.description(), None);
        assert_eq!(bare, absent);
    }

    #[test]
    fn name_and_servers_are_exposed_verbatim() {
        let power = PowerInfo::new(
            "aws-infrastructure-as-code",
            None,
            None,
            vec!["awslabs.aws-iac-mcp-server".to_owned(), "second".to_owned()],
            true,
        );
        assert_eq!(power.name(), "aws-infrastructure-as-code");
        assert_eq!(
            power.mcp_server_names(),
            ["awslabs.aws-iac-mcp-server", "second"]
        );
        assert!(power.has_steering_files());
    }
}
