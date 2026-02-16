use std::path::PathBuf;

use agent_client_protocol as acp;

/// Tracks the state of the current ACP session.
#[derive(Debug)]
pub struct SessionState {
    pub session_id: Option<acp::SessionId>,
    pub cwd: PathBuf,
    pub agent_info: Option<acp::Implementation>,
    pub agent_capabilities: Option<acp::AgentCapabilities>,
}

impl SessionState {
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            session_id: None,
            cwd,
            agent_info: None,
            agent_capabilities: None,
        }
    }
}
