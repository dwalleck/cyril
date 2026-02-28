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
/// This is the authoritative source for session data. The toolbar duplicates
/// some of these fields for display purposes until a future cleanup pass.
#[derive(Debug)]
pub struct SessionContext {
    pub id: Option<acp::SessionId>,
    pub available_modes: Vec<AvailableMode>,
    pub config_options: Vec<acp::SessionConfigOption>,
    pub cwd: PathBuf,
    pub context_usage_pct: Option<f64>,
    pub current_mode_id: Option<String>,
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
    }

    /// Extract the current model value from stored config options.
    pub fn current_model(&self) -> Option<String> {
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
