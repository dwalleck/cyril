use agent_client_protocol as acp;
use tokio::sync::oneshot;

use crate::kiro_ext::KiroExtCommand;

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
    HookFeedback { text: String },

    /// Kiro-specific: available commands received via ext_notification.
    KiroCommandsAvailable { commands: Vec<KiroExtCommand> },

    /// Kiro-specific: metadata update (context usage, etc.) via ext_notification.
    KiroMetadata {
        session_id: String,
        context_usage_pct: f64,
    },
}
