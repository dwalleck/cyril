use std::path::PathBuf;

use agent_client_protocol as acp;

/// Config option ID used by the ACP server for the current model selection.
pub const CONFIG_KEY_MODEL: &str = "model";

/// An available agent mode from the session.
#[derive(Debug, Clone)]
pub struct AvailableMode {
    pub id: String,
    pub name: String,
}

/// Owns all session-level state: IDs, modes, config options, and working directory.
///
/// This is the single source of truth for session data. The toolbar borrows
/// from this struct at render time.
#[derive(Debug)]
pub struct SessionContext {
    pub id: Option<acp::SessionId>,
    pub available_modes: Vec<AvailableMode>,
    pub config_options: Vec<acp::SessionConfigOption>,
    pub cwd: PathBuf,
    pub context_usage_pct: Option<f64>,
    pub current_mode_id: Option<String>,
    cached_model: Option<String>,
}

impl SessionContext {
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            id: None,
            available_modes: Vec::new(),
            config_options: Vec::new(),
            cwd,
            context_usage_pct: None,
            current_mode_id: None,
            cached_model: None,
        }
    }

    pub fn set_session_id(&mut self, session_id: acp::SessionId) {
        self.id = Some(session_id);
    }

    /// Store mode info from a NewSessionResponse.
    pub fn set_modes(&mut self, modes: &acp::SessionModeState) {
        self.current_mode_id = Some(modes.current_mode_id.to_string());
        self.available_modes = modes
            .available_modes
            .iter()
            .map(|m| AvailableMode {
                id: m.id.to_string(),
                name: m.name.clone(),
            })
            .collect();
    }

    /// Store config options (model, etc.) from a session response or update notification.
    pub fn set_config_options(&mut self, options: Vec<acp::SessionConfigOption>) {
        self.config_options = options;
        self.cached_model = self.compute_current_model();
    }

    /// Optimistically update the cached model for immediate UI feedback.
    /// The server's `ConfigOptionsUpdated` event will confirm or overwrite this.
    pub fn set_optimistic_model(&mut self, model: String) {
        self.cached_model = Some(model);
    }

    /// Return the cached model value (O(1) per frame).
    pub fn current_model(&self) -> Option<&str> {
        self.cached_model.as_deref()
    }

    /// Extract the current model value from stored config options.
    fn compute_current_model(&self) -> Option<String> {
        self.config_options.iter().find_map(|opt| {
            if opt.id.to_string() == CONFIG_KEY_MODEL {
                if let acp::SessionConfigKind::Select(ref select) = opt.kind {
                    return Some(select.current_value.to_string());
                }
            }
            None
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_session_has_no_id() {
        let ctx = SessionContext::new(PathBuf::from("/tmp"));
        assert!(ctx.id.is_none());
        assert!(ctx.available_modes.is_empty());
        assert!(ctx.config_options.is_empty());
        assert!(ctx.context_usage_pct.is_none());
        assert!(ctx.current_mode_id.is_none());
        assert!(ctx.current_model().is_none());
    }

    #[test]
    fn set_session_id_stores_id() {
        let mut ctx = SessionContext::new(PathBuf::from("/tmp"));
        let id = acp::SessionId::from("test-session-123".to_string());
        ctx.set_session_id(id);
        assert!(ctx.id.is_some());
        assert_eq!(ctx.id.unwrap().to_string(), "test-session-123");
    }

    #[test]
    fn current_model_returns_none_when_no_config() {
        let ctx = SessionContext::new(PathBuf::from("/tmp"));
        assert!(ctx.current_model().is_none());
    }

    #[test]
    fn current_model_returns_value_from_config_options() {
        let mut ctx = SessionContext::new(PathBuf::from("/tmp"));
        let option = acp::SessionConfigOption::select(
            "model",
            "Model",
            "claude-sonnet-4-6",
            vec![acp::SessionConfigSelectOption::new(
                "claude-sonnet-4-6",
                "Claude Sonnet 4.6",
            )],
        );
        ctx.set_config_options(vec![option]);
        assert_eq!(ctx.current_model(), Some("claude-sonnet-4-6"));
    }

    #[test]
    fn set_modes_populates_available_modes() {
        let mut ctx = SessionContext::new(PathBuf::from("/tmp"));
        let modes = acp::SessionModeState::new(
            "code",
            vec![
                acp::SessionMode::new("code", "Code"),
                acp::SessionMode::new("chat", "Chat"),
            ],
        );
        ctx.set_modes(&modes);
        assert_eq!(ctx.current_mode_id.as_deref(), Some("code"));
        assert_eq!(ctx.available_modes.len(), 2);
        assert_eq!(ctx.available_modes[0].id, "code");
        assert_eq!(ctx.available_modes[0].name, "Code");
        assert_eq!(ctx.available_modes[1].id, "chat");
        assert_eq!(ctx.available_modes[1].name, "Chat");
    }

    #[test]
    fn set_optimistic_model_updates_cached_value() {
        let mut ctx = SessionContext::new(PathBuf::from("/tmp"));
        assert!(ctx.current_model().is_none());
        ctx.set_optimistic_model("claude-sonnet-4-6".to_string());
        assert_eq!(ctx.current_model(), Some("claude-sonnet-4-6"));
    }

    #[test]
    fn set_config_options_caches_model() {
        let mut ctx = SessionContext::new(PathBuf::from("/tmp"));
        let option = acp::SessionConfigOption::select(
            "model",
            "Model",
            "claude-opus-4",
            vec![
                acp::SessionConfigSelectOption::new("claude-opus-4", "Claude Opus 4"),
                acp::SessionConfigSelectOption::new("claude-sonnet-4-6", "Claude Sonnet 4.6"),
            ],
        );
        ctx.set_config_options(vec![option]);

        // current_model() returns the cached value without recomputing
        assert_eq!(ctx.current_model(), Some("claude-opus-4"));

        // Adding a non-model config option does not affect the model cache
        let other = acp::SessionConfigOption::select(
            "thought_level",
            "Thought Level",
            "high",
            vec![acp::SessionConfigSelectOption::new("high", "High")],
        );
        ctx.set_config_options(vec![other]);
        assert!(ctx.current_model().is_none());
    }
}
