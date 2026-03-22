use crate::types::*;

pub struct SessionController {
    status: SessionStatus,
    id: Option<SessionId>,
    modes: Vec<SessionMode>,
    current_mode_id: Option<String>,
    cached_model: Option<String>,
    context_usage: Option<ContextUsage>,
    agent_commands: Vec<CommandInfo>,
    credit_usage: Option<CreditUsage>,
}

impl SessionController {
    pub fn new() -> Self {
        Self {
            status: SessionStatus::Disconnected,
            id: None,
            modes: Vec::new(),
            current_mode_id: None,
            cached_model: None,
            context_usage: None,
            agent_commands: Vec::new(),
            credit_usage: None,
        }
    }

    // Accessors
    pub fn status(&self) -> &SessionStatus {
        &self.status
    }

    pub fn id(&self) -> Option<&SessionId> {
        self.id.as_ref()
    }

    pub fn modes(&self) -> &[SessionMode] {
        &self.modes
    }

    pub fn current_mode_id(&self) -> Option<&str> {
        self.current_mode_id.as_deref()
    }

    pub fn current_model(&self) -> Option<&str> {
        self.cached_model.as_deref()
    }

    pub fn context_usage(&self) -> Option<&ContextUsage> {
        self.context_usage.as_ref()
    }

    pub fn agent_commands(&self) -> &[CommandInfo] {
        &self.agent_commands
    }

    pub fn credit_usage(&self) -> Option<&CreditUsage> {
        self.credit_usage.as_ref()
    }

    // Mutators
    pub fn set_session(&mut self, id: SessionId, status: SessionStatus) {
        self.id = Some(id);
        self.status = status;
    }

    pub fn set_status(&mut self, status: SessionStatus) {
        self.status = status;
    }

    pub fn set_modes(&mut self, modes: Vec<SessionMode>) {
        self.modes = modes;
    }

    pub fn set_credit_usage(&mut self, usage: CreditUsage) {
        self.credit_usage = Some(usage);
    }

    /// Apply a notification to session state. Returns whether state changed.
    pub fn apply_notification(&mut self, notification: &Notification) -> bool {
        match notification {
            Notification::ModeChanged { mode_id } => {
                self.current_mode_id = Some(mode_id.clone());
                true
            }
            Notification::ContextUsageUpdated(usage) => {
                self.context_usage = Some(usage.clone());
                true
            }
            Notification::ConfigOptionsUpdated(options) => {
                if let Some(model_opt) = options.iter().find(|o| o.key == "model") {
                    self.cached_model = model_opt.value.clone();
                }
                true
            }
            Notification::CommandsUpdated(cmds) => {
                self.agent_commands = cmds.clone();
                true
            }
            Notification::AgentSwitched { name, .. } => {
                self.current_mode_id = Some(name.clone());
                self.status = SessionStatus::Active;
                true
            }
            Notification::TurnCompleted => {
                self.status = SessionStatus::Active;
                true
            }
            Notification::SessionCreated {
                session_id,
                current_mode,
            } => {
                self.id = Some(session_id.clone());
                self.current_mode_id = current_mode.clone();
                self.status = SessionStatus::Active;
                true
            }
            Notification::BridgeDisconnected { .. } => {
                self.status = SessionStatus::Disconnected;
                true
            }
            _ => false,
        }
    }
}

impl Default for SessionController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_controller_is_disconnected() {
        let ctrl = SessionController::new();
        assert_eq!(ctrl.status(), &SessionStatus::Disconnected);
        assert!(ctrl.id().is_none());
        assert!(ctrl.current_model().is_none());
        assert!(ctrl.current_mode_id().is_none());
        assert!(ctrl.context_usage().is_none());
    }

    #[test]
    fn set_session_updates_id_and_status() {
        let mut ctrl = SessionController::new();
        ctrl.set_session(SessionId::new("sess_1"), SessionStatus::Active);
        assert_eq!(ctrl.id().map(SessionId::as_str), Some("sess_1"));
        assert_eq!(ctrl.status(), &SessionStatus::Active);
    }

    #[test]
    fn turn_completed_transitions_busy_to_active() {
        let mut ctrl = SessionController::new();
        ctrl.set_session(SessionId::new("sess_1"), SessionStatus::Busy);
        let changed = ctrl.apply_notification(&Notification::TurnCompleted);
        assert!(changed);
        assert_eq!(ctrl.status(), &SessionStatus::Active);
    }

    #[test]
    fn bridge_disconnect_transitions_to_disconnected() {
        let mut ctrl = SessionController::new();
        ctrl.set_session(SessionId::new("sess_1"), SessionStatus::Active);
        let changed = ctrl.apply_notification(&Notification::BridgeDisconnected {
            reason: "process exited".into(),
        });
        assert!(changed);
        assert_eq!(ctrl.status(), &SessionStatus::Disconnected);
    }

    #[test]
    fn mode_changed_updates_mode() {
        let mut ctrl = SessionController::new();
        let changed = ctrl.apply_notification(&Notification::ModeChanged {
            mode_id: "code".into(),
        });
        assert!(changed);
        assert_eq!(ctrl.current_mode_id(), Some("code"));
    }

    #[test]
    fn context_usage_updated() {
        let mut ctrl = SessionController::new();
        let changed =
            ctrl.apply_notification(&Notification::ContextUsageUpdated(ContextUsage::new(75.0)));
        assert!(changed);
        assert!(
            (ctrl.context_usage().map(|u| u.percentage()).unwrap_or(0.0) - 75.0).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn commands_updated() {
        let mut ctrl = SessionController::new();
        let cmds = vec![CommandInfo::new(
            "model",
            "Model",
            None::<&str>,
            true,
            false,
            false,
        )];
        let changed = ctrl.apply_notification(&Notification::CommandsUpdated(cmds));
        assert!(changed);
        assert_eq!(ctrl.agent_commands().len(), 1);
    }

    #[test]
    fn agent_message_does_not_change_session() {
        let mut ctrl = SessionController::new();
        let changed = ctrl.apply_notification(&Notification::AgentMessage(AgentMessage {
            text: "hello".into(),
            is_streaming: true,
        }));
        assert!(!changed);
    }

    #[test]
    fn set_modes() {
        let mut ctrl = SessionController::new();
        let modes = vec![
            SessionMode::new("code", "Code", None::<&str>),
            SessionMode::new("chat", "Chat", Some("General chat")),
        ];
        ctrl.set_modes(modes);
        assert_eq!(ctrl.modes().len(), 2);
    }

    #[test]
    fn set_status_directly() {
        let mut ctrl = SessionController::new();
        ctrl.set_status(SessionStatus::Busy);
        assert_eq!(ctrl.status(), &SessionStatus::Busy);
    }

    #[test]
    fn agent_switched_updates_status_and_mode() {
        let mut ctrl = SessionController::new();
        ctrl.set_status(SessionStatus::Busy);
        let changed = ctrl.apply_notification(&Notification::AgentSwitched {
            name: "code-agent".into(),
            welcome: None,
        });
        assert!(changed);
        assert_eq!(ctrl.status(), &SessionStatus::Active);
        assert_eq!(ctrl.current_mode_id(), Some("code-agent"));
    }

    #[test]
    fn session_created_sets_id_and_activates() {
        let mut ctrl = SessionController::new();
        assert_eq!(ctrl.status(), &SessionStatus::Disconnected);
        assert!(ctrl.id().is_none());

        let changed = ctrl.apply_notification(&Notification::SessionCreated {
            session_id: SessionId::new("sess_abc"),
            current_mode: Some("kiro_default".into()),
        });

        assert!(changed);
        assert_eq!(ctrl.status(), &SessionStatus::Active);
        assert_eq!(ctrl.id().map(SessionId::as_str), Some("sess_abc"));
        assert_eq!(ctrl.current_mode_id(), Some("kiro_default"));
    }

    #[test]
    fn session_created_overwrites_previous_session() {
        let mut ctrl = SessionController::new();
        ctrl.set_session(SessionId::new("old_sess"), SessionStatus::Active);

        ctrl.apply_notification(&Notification::SessionCreated {
            session_id: SessionId::new("new_sess"),
            current_mode: None,
        });

        assert_eq!(ctrl.id().map(SessionId::as_str), Some("new_sess"));
    }
}
