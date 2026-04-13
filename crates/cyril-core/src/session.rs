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
    session_cost: SessionCost,
    pending_tokens: Option<TokenCounts>,
    pending_metering: Option<TurnMetering>,
    last_turn: Option<TurnSummary>,
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
            session_cost: SessionCost::new(),
            pending_tokens: None,
            pending_metering: None,
            last_turn: None,
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

    pub fn session_cost(&self) -> &SessionCost {
        &self.session_cost
    }

    pub fn last_turn(&self) -> Option<&TurnSummary> {
        self.last_turn.as_ref()
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
            Notification::MetadataUpdated {
                context_usage,
                metering,
                tokens,
            } => {
                self.context_usage = Some(context_usage.clone());
                if let Some(m) = metering {
                    self.pending_metering = Some(m.clone());
                }
                self.pending_tokens = tokens.clone();
                true
            }
            Notification::ConfigOptionsUpdated(options) => {
                if let Some(model_opt) = options.iter().find(|o| o.key == "model") {
                    self.cached_model = model_opt.value.clone();
                }
                true
            }
            Notification::CommandsUpdated { commands, .. } => {
                self.agent_commands = commands.clone();
                true
            }
            Notification::AgentSwitched { name, model, .. } => {
                self.current_mode_id = Some(name.clone());
                if let Some(m) = model {
                    self.cached_model = Some(m.clone());
                }
                self.status = SessionStatus::Active;
                true
            }
            Notification::TurnCompleted { stop_reason } => {
                self.last_turn = Some(TurnSummary::new(
                    *stop_reason,
                    self.pending_tokens.take(),
                    self.pending_metering.take(),
                ));
                if let Some(m) = self.last_turn.as_ref().and_then(|t| t.metering()) {
                    self.session_cost.record_turn(m);
                }
                self.status = SessionStatus::Active;
                true
            }
            Notification::SessionCreated {
                session_id,
                current_mode,
                current_model,
            } => {
                self.id = Some(session_id.clone());
                self.current_mode_id = current_mode.clone();
                if let Some(model) = current_model {
                    self.cached_model = Some(model.clone());
                }
                self.session_cost = SessionCost::new();
                self.last_turn = None;
                self.pending_tokens = None;
                self.pending_metering = None;
                self.status = SessionStatus::Active;
                true
            }
            Notification::BridgeDisconnected { .. } => {
                self.last_turn = None;
                self.pending_tokens = None;
                self.pending_metering = None;
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
        let changed = ctrl.apply_notification(&Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        });
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
    fn metadata_updated_stores_context_usage() {
        let mut ctrl = SessionController::new();
        let changed = ctrl.apply_notification(&Notification::MetadataUpdated {
            context_usage: ContextUsage::new(75.0),
            metering: None,
            tokens: None,
        });
        assert!(changed);
        assert!(
            (ctrl.context_usage().map(|u| u.percentage()).unwrap_or(0.0) - 75.0).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn metadata_updated_records_turn_metering() {
        let mut ctrl = SessionController::new();

        // Turn 1: MetadataUpdated + TurnCompleted
        ctrl.apply_notification(&Notification::MetadataUpdated {
            context_usage: ContextUsage::new(5.0),
            metering: Some(TurnMetering::new(0.018, Some(1948))),
            tokens: None,
        });
        ctrl.apply_notification(&Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        });

        // Turn 2: MetadataUpdated + TurnCompleted
        ctrl.apply_notification(&Notification::MetadataUpdated {
            context_usage: ContextUsage::new(6.0),
            metering: Some(TurnMetering::new(0.042, Some(5200))),
            tokens: None,
        });
        ctrl.apply_notification(&Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        });

        assert_eq!(ctrl.session_cost().turn_count(), 2);
        assert!((ctrl.session_cost().total_credits() - 0.060).abs() < 0.001);
        assert!((ctrl.session_cost().last_turn_credits().unwrap() - 0.042).abs() < 0.001);
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
        let changed = ctrl.apply_notification(&Notification::CommandsUpdated {
            commands: cmds,
            prompts: Vec::new(),
        });
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
    fn turn_summary_assembled_from_metadata_and_turn_completed() {
        let mut ctrl = SessionController::new();
        ctrl.set_status(SessionStatus::Busy);

        ctrl.apply_notification(&Notification::MetadataUpdated {
            context_usage: ContextUsage::new(50.0),
            metering: Some(TurnMetering::new(0.03, Some(2000))),
            tokens: Some(TokenCounts::new(800, 400, Some(100))),
        });
        assert!(
            ctrl.last_turn().is_none(),
            "no TurnSummary until turn completes"
        );

        ctrl.apply_notification(&Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        });
        let summary = ctrl
            .last_turn()
            .expect("TurnSummary should exist after TurnCompleted");
        assert_eq!(summary.stop_reason(), StopReason::EndTurn);
        assert!(summary.token_counts().is_some());
        assert_eq!(summary.token_counts().unwrap().input(), 800);
        assert!(summary.metering().is_some());
    }

    #[test]
    fn turn_summary_cleared_on_new_session() {
        let mut ctrl = SessionController::new();
        ctrl.apply_notification(&Notification::MetadataUpdated {
            context_usage: ContextUsage::new(10.0),
            metering: Some(TurnMetering::new(0.01, None)),
            tokens: None,
        });
        ctrl.apply_notification(&Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        });
        assert!(ctrl.last_turn().is_some());

        ctrl.apply_notification(&Notification::SessionCreated {
            session_id: SessionId::new("s2"),
            current_mode: None,
            current_model: None,
        });
        assert!(
            ctrl.last_turn().is_none(),
            "TurnSummary cleared on new session"
        );
    }

    #[test]
    fn turn_summary_cleared_on_bridge_disconnect() {
        let mut ctrl = SessionController::new();
        ctrl.apply_notification(&Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        });
        assert!(ctrl.last_turn().is_some());

        ctrl.apply_notification(&Notification::BridgeDisconnected {
            reason: "process exited".into(),
        });
        assert!(
            ctrl.last_turn().is_none(),
            "TurnSummary cleared on disconnect"
        );
    }

    #[test]
    fn turn_summary_without_metadata() {
        let mut ctrl = SessionController::new();
        ctrl.apply_notification(&Notification::TurnCompleted {
            stop_reason: StopReason::Cancelled,
        });
        let summary = ctrl
            .last_turn()
            .expect("TurnSummary even without prior metadata");
        assert_eq!(summary.stop_reason(), StopReason::Cancelled);
        assert!(summary.token_counts().is_none());
        assert!(summary.metering().is_none());
    }

    #[test]
    fn second_turn_overwrites_last_turn() {
        let mut ctrl = SessionController::new();

        // Turn 1
        ctrl.apply_notification(&Notification::MetadataUpdated {
            context_usage: ContextUsage::new(10.0),
            metering: Some(TurnMetering::new(0.01, None)),
            tokens: Some(TokenCounts::new(100, 50, None)),
        });
        ctrl.apply_notification(&Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        });
        assert_eq!(
            ctrl.last_turn().unwrap().token_counts().unwrap().input(),
            100
        );

        // Turn 2
        ctrl.apply_notification(&Notification::MetadataUpdated {
            context_usage: ContextUsage::new(20.0),
            metering: Some(TurnMetering::new(0.05, Some(5000))),
            tokens: Some(TokenCounts::new(800, 400, Some(200))),
        });
        ctrl.apply_notification(&Notification::TurnCompleted {
            stop_reason: StopReason::MaxTokens,
        });
        let summary = ctrl.last_turn().unwrap();
        assert_eq!(summary.stop_reason(), StopReason::MaxTokens);
        assert_eq!(summary.token_counts().unwrap().input(), 800);
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
            previous_agent: None,
            model: None,
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
            current_model: None,
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
            current_model: None,
        });

        assert_eq!(ctrl.id().map(SessionId::as_str), Some("new_sess"));
    }

    #[test]
    fn session_created_sets_model() {
        let mut ctrl = SessionController::new();
        ctrl.apply_notification(&Notification::SessionCreated {
            session_id: SessionId::new("s1"),
            current_mode: None,
            current_model: Some("claude-sonnet-4".to_string()),
        });
        assert_eq!(ctrl.current_model(), Some("claude-sonnet-4"));
    }

    #[test]
    fn session_created_resets_cost() {
        let mut ctrl = SessionController::new();
        ctrl.apply_notification(&Notification::MetadataUpdated {
            context_usage: ContextUsage::new(10.0),
            metering: Some(TurnMetering::new(0.05, Some(2000))),
            tokens: None,
        });
        ctrl.apply_notification(&Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        });
        assert!(ctrl.session_cost().total_credits() > 0.0);

        ctrl.apply_notification(&Notification::SessionCreated {
            session_id: SessionId::new("s2"),
            current_mode: None,
            current_model: None,
        });

        assert_eq!(ctrl.session_cost().total_credits(), 0.0);
        assert_eq!(ctrl.session_cost().turn_count(), 0);
    }
}
