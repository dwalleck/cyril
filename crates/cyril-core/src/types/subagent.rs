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

/// Review-loop progress for a pipeline stage configured with `loop_to`
/// (Kiro 2.5.0+). Present only when the stage actually has a loop — a
/// non-looping stage has `SubagentInfo::loop_state() == None`, so the
/// "looping" and "not looping" states cannot be confused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopState {
    iteration: u32,
    max_iterations: u32,
}

impl LoopState {
    /// Construct loop progress, enforcing the type's invariants. Returns `None`
    /// when `max_iterations == 0` (a loop with a zero cap is meaningless and
    /// would render a misleading "↻ 1/0"). `iteration` is clamped to
    /// `max_iterations - 1` so a transposed or over-cap counter can never
    /// render past the cap — the invariant lives in the type, not at the one
    /// call site that parses the wire.
    pub fn new(iteration: u32, max_iterations: u32) -> Option<Self> {
        if max_iterations == 0 {
            return None;
        }
        Some(Self {
            iteration: iteration.min(max_iterations - 1),
            max_iterations,
        })
    }

    /// Current loop iteration. Observed 0-based on the wire (the first pass is
    /// 0); see docs/kiro-2.5.0-wire-audit.md. Re-verify if the displayed count
    /// ever looks off-by-one against Kiro's own TUI. For display, prefer
    /// [`Self::display_iteration`] so the 0-based→1-based conversion lives here.
    pub fn iteration(&self) -> u32 {
        self.iteration
    }

    /// The 1-based iteration to show in the UI ("↻ 1/2" on the first pass).
    /// Centralizes the 0-based-wire → 1-based-display conversion so every
    /// consumer renders the same value. Saturating, though `iteration` is
    /// clamped to `max_iterations - 1` at construction so it never overflows.
    pub fn display_iteration(&self) -> u32 {
        self.iteration.saturating_add(1)
    }

    /// The `max_iterations` safety cap configured for the loop.
    pub fn max_iterations(&self) -> u32 {
        self.max_iterations
    }
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
    /// The pipeline stage name (wire field `name`), distinct from
    /// `session_name`. Often absent for ad-hoc spawned subagents.
    stage_name: Option<String>,
    /// Stage creation time in epoch milliseconds (wire field `createdAtMs`).
    created_at_ms: Option<u64>,
    /// Review-loop progress, present only when the stage has a `loop_to`.
    loop_state: Option<LoopState>,
}

impl SubagentInfo {
    /// Construct with the required identity fields. Metadata (`group`,
    /// `role`, `depends_on`) defaults to absent; set it with the `with_*`
    /// builder methods.
    pub fn new(
        session_id: SessionId,
        session_name: impl Into<String>,
        agent_name: impl Into<String>,
        initial_query: impl Into<String>,
        status: SubagentStatus,
    ) -> Self {
        Self {
            session_id,
            session_name: session_name.into(),
            agent_name: agent_name.into(),
            initial_query: initial_query.into(),
            status,
            group: None,
            role: None,
            depends_on: Vec::new(),
            stage_name: None,
            created_at_ms: None,
            loop_state: None,
        }
    }

    #[must_use]
    pub fn with_group(mut self, group: Option<String>) -> Self {
        self.group = group;
        self
    }

    #[must_use]
    pub fn with_stage_name(mut self, stage_name: Option<String>) -> Self {
        self.stage_name = stage_name;
        self
    }

    #[must_use]
    pub fn with_created_at_ms(mut self, created_at_ms: Option<u64>) -> Self {
        self.created_at_ms = created_at_ms;
        self
    }

    #[must_use]
    pub fn with_loop_state(mut self, loop_state: Option<LoopState>) -> Self {
        self.loop_state = loop_state;
        self
    }

    #[must_use]
    pub fn with_role(mut self, role: Option<String>) -> Self {
        self.role = role;
        self
    }

    #[must_use]
    pub fn with_depends_on(mut self, depends_on: Vec<String>) -> Self {
        self.depends_on = depends_on;
        self
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

    /// The pipeline stage name (wire `name`), distinct from `session_name`.
    pub fn stage_name(&self) -> Option<&str> {
        self.stage_name.as_deref()
    }

    /// Stage creation time in epoch milliseconds (wire `createdAtMs`).
    pub fn created_at_ms(&self) -> Option<u64> {
        self.created_at_ms
    }

    /// Review-loop progress, present only when the stage has a `loop_to`.
    pub fn loop_state(&self) -> Option<LoopState> {
        self.loop_state
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
        )
        .with_group(Some("crew-Review".into()))
        .with_role(Some("code-reviewer".into()));
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

    #[test]
    fn subagent_info_loop_and_metadata_default_absent() {
        let info = SubagentInfo::new(
            SessionId::new("s1"),
            "writer",
            "writer",
            "query",
            SubagentStatus::Working { message: None },
        );
        assert_eq!(info.stage_name(), None);
        assert_eq!(info.created_at_ms(), None);
        assert_eq!(info.loop_state(), None);
    }

    #[test]
    fn subagent_info_loop_state_builder() {
        let info = SubagentInfo::new(
            SessionId::new("s1"),
            "checker",
            "checker",
            "query",
            SubagentStatus::Working {
                message: Some("Running".into()),
            },
        )
        .with_stage_name(Some("checker".into()))
        .with_created_at_ms(Some(1_780_023_672_042))
        .with_loop_state(LoopState::new(1, 2));

        assert_eq!(info.stage_name(), Some("checker"));
        assert_eq!(info.created_at_ms(), Some(1_780_023_672_042));
        assert_eq!(info.loop_state(), LoopState::new(1, 2));
    }

    #[test]
    fn loop_state_new_rejects_zero_max() {
        // A loop with a zero cap is meaningless and would render "↻ 1/0".
        assert_eq!(LoopState::new(0, 0), None);
        assert_eq!(LoopState::new(3, 0), None);
    }

    #[test]
    fn loop_state_new_clamps_iteration_to_cap() {
        // A transposed/over-cap counter must never render past the cap.
        let clamped = LoopState::new(99, 2);
        assert_eq!(
            clamped.map(|s| s.iteration()),
            Some(1),
            "clamped to max_iterations - 1"
        );
        assert_eq!(clamped.map(|s| s.max_iterations()), Some(2));
    }

    #[test]
    fn subagent_info_with_depends_on_builder() {
        let info = SubagentInfo::new(
            SessionId::new("s1"),
            "writer",
            "summary-writer",
            "Summarize results",
            SubagentStatus::Working { message: None },
        )
        .with_depends_on(vec!["reviewer".into(), "tester".into()]);
        assert_eq!(info.depends_on(), &["reviewer", "tester"]);
    }
}
