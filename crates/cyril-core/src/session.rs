use crate::types::*;

pub struct SessionController {
    status: SessionStatus,
    id: Option<SessionId>,
    modes: Vec<SessionMode>,
    models: Vec<ModelInfo>,
    current_mode_id: Option<ModeId>,
    cached_model: Option<String>,
    context_usage: Option<ContextUsage>,
    agent_commands: Vec<CommandInfo>,
    credit_usage: Option<CreditUsage>,
    session_cost: SessionCost,
    pending_tokens: Option<TokenCounts>,
    pending_metering: Option<TurnMetering>,
    last_turn: Option<TurnSummary>,
    // Queue steering (Kiro 2.7.0+; ROADMAP K1a). Set on a -32601 from
    // `_session/steer` and remembered for the session; reset on a new session.
    // Steer-only: a `_session/steer/clear` -32601 must NOT set it (cyril-vgcm
    // C12). Queue depth is NOT mirrored here — the optimistic chip count lives
    // in UiState (cyril-7z7u); a session-side mirror was write-only and would
    // drift under id-scoped clears, so it was deleted (cyril-vgcm C13/D5).
    steering_unsupported: bool,
}

impl SessionController {
    pub fn new() -> Self {
        Self {
            status: SessionStatus::Disconnected,
            id: None,
            modes: Vec::new(),
            models: Vec::new(),
            current_mode_id: None,
            cached_model: None,
            context_usage: None,
            agent_commands: Vec::new(),
            credit_usage: None,
            session_cost: SessionCost::new(),
            pending_tokens: None,
            pending_metering: None,
            last_turn: None,
            steering_unsupported: false,
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

    pub fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    pub fn current_mode_id(&self) -> Option<&ModeId> {
        self.current_mode_id.as_ref()
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

    /// Whether `_session/steer` is known-unsupported for this session (set on -32601).
    pub fn steering_unsupported(&self) -> bool {
        self.steering_unsupported
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
                // `effort` is display-only; tracked in UiState for the toolbar,
                // not in session command-context. Intentionally ignored here.
                ..
            } => {
                // Retain-last: a frame that omits contextUsagePercentage
                // (real 2.4.1 wire shape) means "no update", not "context
                // dropped to zero" — same discipline as UiState's `effort`.
                if let Some(u) = context_usage {
                    self.context_usage = Some(u.clone());
                }
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
                // Kiro reports agent switches using the agent's name as the
                // mode identity — wrap into ModeId at this boundary.
                self.current_mode_id = Some(ModeId::new(name.clone()));
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
                available_modes,
                available_models,
            } => {
                self.id = Some(session_id.clone());
                self.current_mode_id = current_mode.clone();
                if let Some(model) = current_model {
                    self.cached_model = Some(model.clone());
                }
                self.modes = available_modes.clone();
                self.models = available_models.clone();
                self.session_cost = SessionCost::new();
                // Context usage is per-session and re-pushed by the new session;
                // reset so a prior session's value doesn't linger (mirrors UiState).
                self.context_usage = None;
                self.last_turn = None;
                self.pending_tokens = None;
                self.pending_metering = None;
                self.steering_unsupported = false;
                self.status = SessionStatus::Active;
                true
            }
            // Queue steering (K1a). The flag remembers a `_session/steer` -32601
            // for the session. The queued/consumed/cleared echoes carry no
            // session-side state (the chip count is UiState's, cyril-7z7u) and
            // a clear--32601 is advisory-only (cyril-vgcm C12) — all no-ops here.
            Notification::SteeringQueued { .. }
            | Notification::SteeringConsumed { .. }
            | Notification::SteeringCleared { .. }
            | Notification::SteeringClearUnsupported { .. } => false,
            Notification::SteeringUnsupported { .. } => {
                self.steering_unsupported = true;
                true
            }
            Notification::UsageUpdated { used, size } => {
                if *size == 0 {
                    // `size == 0` is protocol-meaningless (division would be undefined).
                    // Treat as a malformed update — don't claim state changed.
                    tracing::warn!(used, "UsageUpdated with size=0, ignoring");
                    return false;
                }
                let pct = (*used as f64 / *size as f64) * 100.0;
                self.context_usage = Some(ContextUsage::new(pct));
                true
            }
            // Under KAS the context-usage scalar arrives via ContextBreakdownUpdated
            // (KAS emits no kiro.dev/metadata), not MetadataUpdated/UsageUpdated.
            // Keep SessionController.context_usage in sync so the documented
            // "context usage flows to both state machines" invariant (the
            // `context_usage_flows_to_both` routing test) holds for the KAS path
            // too — otherwise a CommandContext reader of session.context_usage()
            // sees None under KAS while the toolbar shows a live value. Clamp via
            // ContextUsage::new like the sibling arms; the per-bucket breakdown is
            // UI-only and stays in UiState.
            Notification::ContextBreakdownUpdated {
                usage_percentage, ..
            } => {
                self.context_usage = Some(ContextUsage::new(*usage_percentage));
                true
            }
            Notification::BridgeDisconnected { .. } => {
                self.last_turn = None;
                self.pending_tokens = None;
                self.pending_metering = None;
                // A dead connection has no live context — clear it so a consumer
                // doesn't read a stale value as current (mirrors UiState).
                self.context_usage = None;
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
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn new_controller_is_disconnected() {
        let ctrl = SessionController::new();
        assert_eq!(ctrl.status(), &SessionStatus::Disconnected);
        assert!(ctrl.id().is_none());
        assert!(ctrl.current_model().is_none());
        assert!(ctrl.current_mode_id().is_none());
        assert!(ctrl.context_usage().is_none());
        assert!(!ctrl.steering_unsupported());
    }

    // K1a flag lifecycle + cyril-vgcm C13: the steering echoes carry no
    // session-side state (steering_depth was deleted — write-only mirror of
    // UiState's optimistic chip count, D5), so queued/consumed/cleared and the
    // advisory clear-unsupported must all be no-ops here; only the steer
    // -32601 flag transitions, and a NEW session resets it.
    #[test]
    fn steering_state_transitions_and_reset() {
        let mut ctrl = SessionController::new();
        // Echo notifications are session-side no-ops: no state change reported.
        assert!(!ctrl.apply_notification(&Notification::SteeringQueued {
            message: Some("a".into()),
            message_id: None,
        }));
        assert!(!ctrl.apply_notification(&Notification::SteeringConsumed {
            content: Some("a".into()),
            message_id: Some("steer-1".into()),
        }));
        assert!(!ctrl.apply_notification(&Notification::SteeringCleared {
            message_ids: vec!["steer-1".into()]
        }));
        // A clear--32601 advisory must NOT set the steer-unsupported flag
        // (cyril-vgcm C12 — the shared-set poisoning was findings F5).
        assert!(
            !ctrl.apply_notification(&Notification::SteeringClearUnsupported {
                message: "steer/clear not supported".into()
            })
        );
        assert!(!ctrl.steering_unsupported());

        // Steer -32601 sets the flag; a NEW session must reset it (a 2.6.1
        // session's "unsupported" must not leak onto a fresh 2.7.0 one).
        assert!(ctrl.apply_notification(&Notification::SteeringUnsupported {
            message: "steering requires kiro-cli 2.7.0+".into()
        }));
        assert!(ctrl.steering_unsupported());
        ctrl.apply_notification(&Notification::SessionCreated {
            session_id: SessionId::new("fresh"),
            current_mode: None,
            current_model: None,
            available_modes: Vec::new(),
            available_models: Vec::new(),
        });
        assert!(
            !ctrl.steering_unsupported(),
            "unsupported flag must reset on new session"
        );
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
            mode_id: ModeId::new("code"),
        });
        assert!(changed);
        assert_eq!(ctrl.current_mode_id().map(ModeId::as_str), Some("code"));
    }

    #[test]
    fn metadata_updated_stores_context_usage() {
        let mut ctrl = SessionController::new();
        let changed = ctrl.apply_notification(&Notification::MetadataUpdated {
            context_usage: Some(ContextUsage::new(75.0)),
            metering: None,
            tokens: None,
            effort: None,
            session_id: None,
        });
        assert!(changed);
        assert!(
            (ctrl.context_usage().map(|u| u.percentage()).unwrap_or(0.0) - 75.0).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn context_breakdown_updated_syncs_context_usage_under_kas() {
        // Under KAS the context scalar arrives via ContextBreakdownUpdated, not
        // MetadataUpdated/UsageUpdated, so SessionController must sync from it too
        // (the "context usage flows to both state machines" invariant). 142.0 also
        // exercises the ContextUsage::new clamp → 100.
        let mut ctrl = SessionController::new();
        let changed = ctrl.apply_notification(&Notification::ContextBreakdownUpdated {
            usage_percentage: 142.0,
            breakdown: None,
        });
        assert!(changed);
        assert!(
            (ctrl.context_usage().map(|u| u.percentage()).unwrap_or(-1.0) - 100.0).abs()
                < f64::EPSILON,
            "ContextBreakdownUpdated must update SessionController.context_usage, clamped to 100"
        );
    }

    #[test]
    fn metadata_updated_records_turn_metering() {
        let mut ctrl = SessionController::new();

        // Turn 1: MetadataUpdated + TurnCompleted
        ctrl.apply_notification(&Notification::MetadataUpdated {
            context_usage: Some(ContextUsage::new(5.0)),
            metering: Some(TurnMetering::new(0.018, Some(1948))),
            tokens: None,
            effort: None,
            session_id: None,
        });
        ctrl.apply_notification(&Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        });

        // Turn 2: MetadataUpdated + TurnCompleted
        ctrl.apply_notification(&Notification::MetadataUpdated {
            context_usage: Some(ContextUsage::new(6.0)),
            metering: Some(TurnMetering::new(0.042, Some(5200))),
            tokens: None,
            effort: None,
            session_id: None,
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
            context_usage: Some(ContextUsage::new(50.0)),
            metering: Some(TurnMetering::new(0.03, Some(2000))),
            tokens: Some(TokenCounts::new(800, 400, Some(100))),
            effort: None,
            session_id: None,
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
            context_usage: Some(ContextUsage::new(10.0)),
            metering: Some(TurnMetering::new(0.01, None)),
            tokens: None,
            effort: None,
            session_id: None,
        });
        ctrl.apply_notification(&Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        });
        assert!(ctrl.last_turn().is_some());

        ctrl.apply_notification(&Notification::SessionCreated {
            session_id: SessionId::new("s2"),
            current_mode: None,
            current_model: None,
            available_modes: Vec::new(),
            available_models: Vec::new(),
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
            context_usage: Some(ContextUsage::new(10.0)),
            metering: Some(TurnMetering::new(0.01, None)),
            tokens: Some(TokenCounts::new(100, 50, None)),
            effort: None,
            session_id: None,
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
            context_usage: Some(ContextUsage::new(20.0)),
            metering: Some(TurnMetering::new(0.05, Some(5000))),
            tokens: Some(TokenCounts::new(800, 400, Some(200))),
            effort: None,
            session_id: None,
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
            SessionMode::new(ModeId::new("code"), "Code", None::<&str>),
            SessionMode::new(ModeId::new("chat"), "Chat", Some("General chat")),
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
        assert_eq!(
            ctrl.current_mode_id().map(ModeId::as_str),
            Some("code-agent")
        );
    }

    #[test]
    fn session_created_sets_id_and_activates() {
        let mut ctrl = SessionController::new();
        assert_eq!(ctrl.status(), &SessionStatus::Disconnected);
        assert!(ctrl.id().is_none());

        let changed = ctrl.apply_notification(&Notification::SessionCreated {
            session_id: SessionId::new("sess_abc"),
            current_mode: Some(ModeId::new("kiro_default")),
            current_model: None,
            available_modes: Vec::new(),
            available_models: Vec::new(),
        });

        assert!(changed);
        assert_eq!(ctrl.status(), &SessionStatus::Active);
        assert_eq!(ctrl.id().map(SessionId::as_str), Some("sess_abc"));
        assert_eq!(
            ctrl.current_mode_id().map(ModeId::as_str),
            Some("kiro_default")
        );
    }

    #[test]
    fn session_created_overwrites_previous_session() {
        let mut ctrl = SessionController::new();
        ctrl.set_session(SessionId::new("old_sess"), SessionStatus::Active);

        ctrl.apply_notification(&Notification::SessionCreated {
            session_id: SessionId::new("new_sess"),
            current_mode: None,
            current_model: None,
            available_modes: Vec::new(),
            available_models: Vec::new(),
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
            available_modes: Vec::new(),
            available_models: Vec::new(),
        });
        assert_eq!(ctrl.current_model(), Some("claude-sonnet-4"));
    }

    #[test]
    fn session_created_resets_cost() {
        let mut ctrl = SessionController::new();
        ctrl.apply_notification(&Notification::MetadataUpdated {
            context_usage: Some(ContextUsage::new(10.0)),
            metering: Some(TurnMetering::new(0.05, Some(2000))),
            tokens: None,
            effort: None,
            session_id: None,
        });
        ctrl.apply_notification(&Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        });
        assert!(ctrl.session_cost().total_credits() > 0.0);

        ctrl.apply_notification(&Notification::SessionCreated {
            session_id: SessionId::new("s2"),
            current_mode: None,
            current_model: None,
            available_modes: Vec::new(),
            available_models: Vec::new(),
        });

        assert_eq!(ctrl.session_cost().total_credits(), 0.0);
        assert_eq!(ctrl.session_cost().turn_count(), 0);
    }

    #[test]
    fn context_usage_resets_on_session_created() {
        // Mirrors UiState (PR #33): context usage is per-session and re-pushed by
        // the new session, so a prior session's value must not linger.
        let mut ctrl = SessionController::new();
        ctrl.apply_notification(&Notification::MetadataUpdated {
            context_usage: Some(ContextUsage::new(75.0)),
            metering: None,
            tokens: None,
            effort: None,
            session_id: None,
        });
        assert!(ctrl.context_usage().is_some());

        ctrl.apply_notification(&Notification::SessionCreated {
            session_id: SessionId::new("s2"),
            current_mode: None,
            current_model: None,
            available_modes: Vec::new(),
            available_models: Vec::new(),
        });
        assert!(
            ctrl.context_usage().is_none(),
            "context usage must reset on new session"
        );
    }

    #[test]
    fn context_usage_resets_on_disconnect() {
        // Mirrors UiState (PR #33): a dead connection has no live context.
        // Set via ContextBreakdownUpdated to also cover the KAS ingest path.
        let mut ctrl = SessionController::new();
        ctrl.apply_notification(&Notification::ContextBreakdownUpdated {
            usage_percentage: 42.0,
            breakdown: None,
        });
        assert!(ctrl.context_usage().is_some());

        ctrl.apply_notification(&Notification::BridgeDisconnected {
            reason: "process exited".into(),
        });
        assert!(
            ctrl.context_usage().is_none(),
            "context usage must reset on disconnect"
        );
    }

    #[test]
    fn usage_updated_computes_context_percentage() {
        let mut ctrl = SessionController::new();
        let changed = ctrl.apply_notification(&Notification::UsageUpdated {
            used: 50_000,
            size: 200_000,
        });
        assert!(changed);
        let pct = ctrl.context_usage().map(|u| u.percentage()).unwrap_or(0.0);
        assert!((pct - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn usage_updated_zero_size_is_safe() {
        let mut ctrl = SessionController::new();
        let changed = ctrl.apply_notification(&Notification::UsageUpdated { used: 100, size: 0 });
        // size=0 is protocol-meaningless; return false to avoid spurious repaint
        // and leave context_usage untouched.
        assert!(!changed);
        assert!(ctrl.context_usage().is_none());
    }

    #[test]
    fn usage_updated_used_over_size_clamps_to_100() {
        // If Kiro ever reports used > size, clamp to 100% rather than display
        // a nonsensical 125%.
        let mut ctrl = SessionController::new();
        ctrl.apply_notification(&Notification::UsageUpdated {
            used: 250_000,
            size: 200_000,
        });
        let pct = ctrl.context_usage().map(|u| u.percentage()).unwrap_or(-1.0);
        assert!(
            (pct - 100.0).abs() < f64::EPSILON,
            "expected 100.0, got {pct}"
        );
    }

    #[test]
    fn session_created_stores_available_modes_and_models() {
        let mut ctrl = SessionController::new();
        let modes = vec![
            SessionMode::new(ModeId::new("kiro_default"), "Default", None::<&str>),
            SessionMode::new(
                ModeId::new("kiro_planner"),
                "Planner",
                Some("Planning mode"),
            )
            .with_welcome_message(Some("Transform any idea...".into())),
        ];
        let models = vec![
            ModelInfo::new(
                ModelId::new("claude-sonnet-4"),
                "Claude Sonnet 4",
                None::<&str>,
            ),
            ModelInfo::new(ModelId::new("claude-haiku"), "Claude Haiku", Some("Fast")),
        ];

        ctrl.apply_notification(&Notification::SessionCreated {
            session_id: SessionId::new("s1"),
            current_mode: Some(ModeId::new("kiro_planner")),
            current_model: Some("claude-sonnet-4".into()),
            available_modes: modes,
            available_models: models,
        });

        assert_eq!(ctrl.modes().len(), 2);
        assert_eq!(ctrl.modes()[1].id().as_str(), "kiro_planner");
        assert_eq!(
            ctrl.modes()[1].welcome_message(),
            Some("Transform any idea...")
        );
        assert_eq!(ctrl.models().len(), 2);
        assert_eq!(ctrl.models()[0].id().as_str(), "claude-sonnet-4");
        assert_eq!(ctrl.models()[0].name(), "Claude Sonnet 4");
        assert_eq!(ctrl.models()[1].description(), Some("Fast"));
    }

    #[test]
    fn session_created_replaces_previous_modes_and_models() {
        let mut ctrl = SessionController::new();
        ctrl.apply_notification(&Notification::SessionCreated {
            session_id: SessionId::new("s1"),
            current_mode: None,
            current_model: None,
            available_modes: vec![SessionMode::new(ModeId::new("old"), "old", None::<&str>)],
            available_models: vec![ModelInfo::new(
                ModelId::new("old-model"),
                "old",
                None::<&str>,
            )],
        });

        // Second SessionCreated must overwrite, not append.
        ctrl.apply_notification(&Notification::SessionCreated {
            session_id: SessionId::new("s2"),
            current_mode: None,
            current_model: None,
            available_modes: vec![SessionMode::new(ModeId::new("new"), "new", None::<&str>)],
            available_models: vec![ModelInfo::new(
                ModelId::new("new-model"),
                "new",
                None::<&str>,
            )],
        });

        assert_eq!(ctrl.modes().len(), 1);
        assert_eq!(ctrl.modes()[0].id().as_str(), "new");
        assert_eq!(ctrl.models().len(), 1);
        assert_eq!(ctrl.models()[0].id().as_str(), "new-model");
    }
}
