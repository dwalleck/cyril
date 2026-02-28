use agent_client_protocol as acp;
use tokio::sync::oneshot;

/// Events sent from the ACP Client impl to the TUI/application layer.
#[derive(Debug)]
pub enum AppEvent {
    /// Streaming text chunk from the agent.
    AgentMessage {
        session_id: acp::SessionId,
        chunk: acp::ContentChunk,
    },

    /// Agent thinking/reasoning chunk.
    AgentThought {
        session_id: acp::SessionId,
        chunk: acp::ContentChunk,
    },

    /// A tool call was started.
    ToolCallStarted {
        session_id: acp::SessionId,
        tool_call: acp::ToolCall,
    },

    /// A tool call was updated (progress or completion).
    ToolCallUpdated {
        session_id: acp::SessionId,
        update: acp::ToolCallUpdate,
    },

    /// Agent requests permission from the user.
    PermissionRequest {
        request: acp::RequestPermissionRequest,
        responder: oneshot::Sender<acp::RequestPermissionResponse>,
    },

    /// Available commands updated.
    CommandsUpdated {
        session_id: acp::SessionId,
        commands: acp::AvailableCommandsUpdate,
    },

    /// Agent mode changed.
    ModeChanged {
        session_id: acp::SessionId,
        mode: acp::CurrentModeUpdate,
    },

    /// Agent plan update.
    PlanUpdated {
        session_id: acp::SessionId,
        plan: acp::Plan,
    },

    /// Config options updated (model, etc.).
    ConfigOptionsUpdated {
        session_id: acp::SessionId,
        config_options: Vec<acp::SessionConfigOption>,
    },

    /// Hook feedback to inject as a follow-up prompt to the agent.
    HookFeedback {
        text: String,
    },

    /// Kiro-specific: available commands received via ext_notification.
    KiroCommandsAvailable {
        commands: Vec<KiroExtCommand>,
    },

    /// Kiro-specific: metadata update (context usage, etc.) via ext_notification.
    KiroMetadata {
        session_id: String,
        context_usage_pct: f64,
    },
}

/// A command received from the `kiro.dev/commands/available` extension notification.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct KiroExtCommand {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub input_hint: Option<String>,
    #[serde(default)]
    pub meta: Option<KiroCommandMeta>,
}

/// Metadata for a Kiro command.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KiroCommandMeta {
    /// "selection" requires a dropdown, "panel" needs special rendering.
    pub input_type: Option<String>,
    /// Extension method to call for options (e.g. `_kiro.dev/commands/model/options`).
    pub options_method: Option<String>,
    /// If true, the command is purely local (e.g. /quit).
    #[serde(default)]
    pub local: bool,
}

impl KiroExtCommand {
    /// Whether this command can be executed via `kiro.dev/commands/execute`.
    /// Panel commands (like /context, /help) are allowed — they return structured
    /// data that we display in chat. Only selection commands and local-only
    /// commands are excluded.
    pub fn is_executable(&self) -> bool {
        match &self.meta {
            None => true,
            Some(meta) => {
                !meta.local && meta.input_type.as_deref() != Some("selection")
            }
        }
    }
}
