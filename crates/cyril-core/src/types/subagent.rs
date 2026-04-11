use crate::types::session::SessionId;

/// Status of an active subagent session.
///
/// `Working` carries an optional status message from the agent (e.g., "Running").
/// `None` means the protocol did not provide a message — the UI should display
/// a default like "Working" rather than an empty string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubagentStatus {
    Working { message: Option<String> },
    Terminated,
}

/// An active subagent session reported by `subagent/list_update`.
#[derive(Debug, Clone)]
pub struct SubagentInfo {
    session_id: SessionId,
    session_name: String,
    agent_name: String,
    initial_query: String,
    status: SubagentStatus,
    group: Option<String>,
    role: Option<String>,
    /// Stage names this subagent depends on. These correspond to
    /// `PendingStage::name` or other `SubagentInfo::session_name` values.
    depends_on: Vec<String>,
}

impl SubagentInfo {
    pub fn new(
        session_id: SessionId,
        session_name: impl Into<String>,
        agent_name: impl Into<String>,
        initial_query: impl Into<String>,
        status: SubagentStatus,
        group: Option<String>,
        role: Option<String>,
        depends_on: Vec<String>,
    ) -> Self {
        Self {
            session_id,
            session_name: session_name.into(),
            agent_name: agent_name.into(),
            initial_query: initial_query.into(),
            status,
            group,
            role,
            depends_on,
        }
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn session_name(&self) -> &str {
        &self.session_name
    }

    pub fn agent_name(&self) -> &str {
        &self.agent_name
    }

    pub fn initial_query(&self) -> &str {
        &self.initial_query
    }

    pub fn status(&self) -> &SubagentStatus {
        &self.status
    }

    pub fn group(&self) -> Option<&str> {
        self.group.as_deref()
    }

    pub fn role(&self) -> Option<&str> {
        self.role.as_deref()
    }

    pub fn depends_on(&self) -> &[String] {
        &self.depends_on
    }

    pub fn is_working(&self) -> bool {
        matches!(self.status, SubagentStatus::Working { .. })
    }
}

/// A stage that hasn't been spawned yet (waiting on dependencies).
#[derive(Debug, Clone)]
pub struct PendingStage {
    /// The stage name — used as the identity key in dependency references.
    /// Other entries' `depends_on` lists reference this value.
    name: String,
    agent_name: Option<String>,
    group: Option<String>,
    role: Option<String>,
    /// Stage names this pending stage depends on. References `name` fields
    /// of other `PendingStage` or `SubagentInfo::session_name` values.
    depends_on: Vec<String>,
}

impl PendingStage {
    pub fn new(
        name: impl Into<String>,
        agent_name: Option<String>,
        group: Option<String>,
        role: Option<String>,
        depends_on: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            agent_name,
            group,
            role,
            depends_on,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn agent_name(&self) -> Option<&str> {
        self.agent_name.as_deref()
    }

    pub fn group(&self) -> Option<&str> {
        self.group.as_deref()
    }

    pub fn role(&self) -> Option<&str> {
        self.role.as_deref()
    }

    pub fn depends_on(&self) -> &[String] {
        &self.depends_on
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subagent_info_accessors() {
        let info = SubagentInfo::new(
            SessionId::new("s1"),
            "code-reviewer",
            "code-reviewer",
            "Review the code",
            SubagentStatus::Working {
                message: Some("Running".into()),
            },
            Some("crew-Review".into()),
            Some("code-reviewer".into()),
            vec![],
        );
        assert_eq!(info.session_name(), "code-reviewer");
        assert!(info.is_working());
        assert_eq!(info.group(), Some("crew-Review"));
        assert!(info.depends_on().is_empty());
    }

    #[test]
    fn subagent_terminated_is_not_working() {
        let info = SubagentInfo::new(
            SessionId::new("s2"),
            "done",
            "done",
            "query",
            SubagentStatus::Terminated,
            None,
            None,
            vec![],
        );
        assert!(!info.is_working());
    }

    #[test]
    fn pending_stage_accessors() {
        let stage = PendingStage::new(
            "summary-writer",
            Some("summary-writer".into()),
            Some("crew-Review".into()),
            Some("summary-writer".into()),
            vec!["code-reviewer".into(), "pr-test-analyzer".into()],
        );
        assert_eq!(stage.name(), "summary-writer");
        assert_eq!(stage.depends_on().len(), 2);
    }
}
